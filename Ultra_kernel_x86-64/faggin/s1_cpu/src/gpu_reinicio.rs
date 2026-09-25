//! **THE GPU RESET, BEFORE ANYONE LOOKS** (2026-09-25).
//!
//! The owner, after booting BMO-X from a Windows restart: *"que la GPU reciba
//! orden de reinicio por completo por si sola"*. A card that never lost power
//! since another OS (or a BMO-X reboot without `gpu apagar`) started its GSP
//! cannot be woken again: `despertar` answers 67 or the booter 0x15. Linux
//! answers that with a PCIe reset of the function. So does this file.
//!
//! === Why HERE and not in the kernel ===
//!
//! A secondary bus reset takes the WHOLE card back to power-on -- including
//! the display engine and the mode the firmware set. BMO-X draws on that GOP
//! framebuffer and has no modeset of its own: reset from the kernel and the
//! monitor goes black for good. Here, before `ExitBootServices`, the card's
//! own UEFI driver (from its option ROM) is still loaded: stop it, pulse the
//! reset, wait for the card's firmware (GFW) to boot again, and give the card
//! back to its driver, which lights the monitor. Only then is GOP asked.
//!
//! ```text
//!    look      NVIDIA display controller? its bridge? warm? (read only)
//!    stop      DisconnectController: its UEFI driver lets go of it
//!    save      config space of every function of the card
//!    pulse     Bridge Control bit 6 (Secondary Bus Reset), 2 ms
//!    wait      config answers again; restore it; GFW boot complete
//!    give back ConnectController: the monitor comes back
//! ```
//!
//! === When ===
//!
//! Only WARM, by the two signals that cannot lie on a cold card: the GSP
//! RISC-V running (someone's GSP-RM is alive) or the WPR2 up (only FWSEC or a
//! booter raise it, and only a driver runs them). Not the PCIe link speed:
//! the metal (24-09 14:09) brought Gen3 on a card that woke fine.
//!
//! The switch is at BUILD time, because this board has no UEFI FAT driver
//! (see `build.ps1`: the whole chain is embedded) and the loader cannot read
//! a file: `BMO_GPU_REINICIO=no` never resets, `=siempre` resets even cold
//! (to test it on purpose), anything else resets only warm.
//!
//! If it goes wrong the fallback is the one that always worked: power off
//! for 15 s. A cold card is never touched (unless built with `siempre`).
//! Everything it saw and did travels to the kernel in `gpu_reinicio*` and
//! shows as the `cargador` row of `gpu salud`.

#[allow(unused_imports)]
use crate::*;
use boot_context::*;

// -- PCI config through the legacy ports (the first 256 bytes: enough) ------

const PCI_ADDR: u16 = 0xCF8;
const PCI_DATA: u16 = 0xCFC;

unsafe fn outl(port: u16, v: u32) { asm!("out dx, eax", in("dx") port, in("eax") v, options(nostack, preserves_flags)); }
unsafe fn inl(port: u16) -> u32 { let v: u32; asm!("in eax, dx", in("dx") port, out("eax") v, options(nostack, preserves_flags)); v }

unsafe fn cfg(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    outl(PCI_ADDR, 0x8000_0000 | (bus as u32) << 16 | (dev as u32) << 11 | (func as u32) << 8 | (off as u32 & 0xFC));
    inl(PCI_DATA)
}

unsafe fn cfg_w(bus: u8, dev: u8, func: u8, off: u8, v: u32) {
    outl(PCI_ADDR, 0x8000_0000 | (bus as u32) << 16 | (dev as u32) << 11 | (func as u32) << 8 | (off as u32 & 0xFC));
    outl(PCI_DATA, v);
}

/// The PCI Express capability, if any (offset in config space).
unsafe fn pcie_cap(bus: u8, dev: u8, func: u8) -> Option<u8> {
    if cfg(bus, dev, func, 0x04) >> 16 & 0x10 == 0 { return None; }
    let mut p = (cfg(bus, dev, func, 0x34) & 0xFC) as u8;
    let mut n = 0;
    while p >= 0x40 && n < 48 {
        let c = cfg(bus, dev, func, p);
        if c & 0xFF == 0x10 { return Some(p); }
        p = (c >> 8 & 0xFC) as u8;
        n += 1;
    }
    None
}

/// PCIe link speed (Gen 1..5), 0 if unknown.
unsafe fn velocidad(bus: u8, dev: u8, func: u8) -> u64 {
    match pcie_cap(bus, dev, func) {
        Some(c) if c <= 0xEC => (cfg(bus, dev, func, c + 0x10) >> 16 & 0xF) as u64,
        Some(_) => 0,
        None => 0,
    }
}

// -- The card's registers (BAR0), read only until the reset -----------------

/// `NV_PRISCV_RISCV_CPUCTL` of the GSP falcon; bit 7 = active.
const GSP_RISCV_CPUCTL: u64 = 0x0011_0000 + 0x1388;
const RISCV_ACTIVO: u32 = 1 << 7;
/// The WPR2 top (`0x1FA828`): `>> 4` non-zero = up.
const WPR2_HI: u64 = 0x001F_A828;
/// GFW boot: PLM readable (bit 0) and progress `0xFF` = done (as nouveau).
const GFW_PLM: u64 = 0x0011_8128;
const GFW_PROGRESO: u64 = 0x0011_8234;

unsafe fn rd(bar0: u64, reg: u64) -> u32 { ((bar0 + reg) as *const u32).read_volatile() }

/// A PRI error or a dead bus reads like a value; it is not one.
fn es_error(v: u32) -> bool { v == 0xFFFF_FFFF || v >> 20 == 0xBAD }

unsafe fn gfw_listo(bar0: u64) -> bool {
    let plm = rd(bar0, GFW_PLM);
    !es_error(plm) && plm & 1 != 0 && rd(bar0, GFW_PROGRESO) & 0xFF == 0xFF
}

/// `(GSP RISC-V CPUCTL, WPR2_HI)`, raw.
unsafe fn lecturas(bar0: u64) -> (u32, u32) { (rd(bar0, GSP_RISCV_CPUCTL), rd(bar0, WPR2_HI)) }

// -- UEFI -------------------------------------------------------------------

static mut PCI_HANDLES: [EfiHandle; 256] = [core::ptr::null_mut(); 256];

/// The UEFI handle of the PCI function `bus:dev.func`, through PciIo's
/// GetLocation (the 15th entry of the protocol).
unsafe fn handle_de(system_table: *mut EfiSystemTable, bus: u8, dev: u8, func: u8) -> Option<EfiHandle> {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let locate_handle: extern "efiapi" fn(u32, *mut EfiGuid, *mut core::ffi::c_void, &mut usize, *mut EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 19));
    let handle_protocol: extern "efiapi" fn(EfiHandle, *mut EfiGuid, &mut *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 16));
    let todos = &mut *core::ptr::addr_of_mut!(PCI_HANDLES);
    let mut bytes = todos.len() * core::mem::size_of::<EfiHandle>();
    if locate_handle(2, &raw mut PCI_IO_GUID, core::ptr::null_mut(), &mut bytes, todos.as_mut_ptr()) != EFI_SUCCESS {
        return None;
    }
    let n = (bytes / core::mem::size_of::<EfiHandle>()).min(todos.len());
    for &h in &todos[..n] {
        let mut io: *mut core::ffi::c_void = core::ptr::null_mut();
        if handle_protocol(h, &raw mut PCI_IO_GUID, &mut io) != EFI_SUCCESS || io.is_null() { continue; }
        let get_location: extern "efiapi" fn(*mut core::ffi::c_void, &mut usize, &mut usize, &mut usize, &mut usize) -> EfiStatus =
            core::mem::transmute(*(io as *const *mut core::ffi::c_void).add(14));
        let (mut s, mut b, mut d, mut f) = (0usize, 0usize, 0usize, 0usize);
        if get_location(io, &mut s, &mut b, &mut d, &mut f) == EFI_SUCCESS
            && s == 0 && b == bus as usize && d == dev as usize && f == func as usize
        {
            return Some(h);
        }
    }
    None
}

// -- Finding it -------------------------------------------------------------

/// THE 3060 12G AND ONLY IT (2026-09-25): the same list as
/// `bmo_gpu_ga10x::identidad::DISPOSITIVOS` (this stage does not link that
/// crate; the `la-3060` guardian demands both lists match). Any other NVIDIA
/// card is never reset: nothing of BMO-X was written for it.
const DISPOSITIVOS: [u16; 2] = [0x2503, 0x2504];

/// The 3060 12G display controller: `(bus, dev)` (function 0).
unsafe fn buscar() -> Option<(u8, u8)> {
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            let id = cfg(bus, dev, 0, 0);
            if id & 0xFFFF == 0x10DE
                && DISPOSITIVOS.contains(&((id >> 16) as u16))
                && cfg(bus, dev, 0, 0x08) >> 24 == 0x03
            {
                return Some((bus, dev));
            }
        }
    }
    None
}

/// The bridge whose secondary bus is `bus`: `(bus, dev, func)`.
unsafe fn puente(bus: u8) -> Option<(u8, u8, u8)> {
    for b in 0..=255u8 {
        for d in 0..32u8 {
            if cfg(b, d, 0, 0) & 0xFFFF == 0xFFFF { continue; }
            let multi = cfg(b, d, 0, 0x0C) >> 16 & 0x80 != 0;
            for f in 0..if multi { 8u8 } else { 1 } {
                let id = cfg(b, d, f, 0);
                if id & 0xFFFF == 0xFFFF { continue; }
                // Header type 1 = a bridge; its secondary bus at 0x19.
                if cfg(b, d, f, 0x0C) >> 16 & 0x7F == 1 && (cfg(b, d, f, 0x18) >> 8) as u8 == bus {
                    return Some((b, d, f));
                }
            }
        }
    }
    None
}

// -- Saving and restoring what the firmware set up ----------------------------

#[derive(Clone, Copy)]
struct Guardado {
    vivo: bool,
    cfg: [u32; 64],
}

const NADA: Guardado = Guardado { vivo: false, cfg: [0; 64] };

unsafe fn guardar(bus: u8, dev: u8, func: u8) -> Guardado {
    let mut g = NADA;
    if cfg(bus, dev, func, 0) & 0xFFFF == 0xFFFF { return g; }
    g.vivo = true;
    for i in 0..64u8 { g.cfg[i as usize] = cfg(bus, dev, func, i * 4); }
    g
}

/// Back what the reset wiped: BARs, ROM, cache line and latency, interrupt
/// line, the PCIe control registers. The COMMAND register is left for the
/// end (`cmd`): with decoding on while BARs move, a stray access lands
/// anywhere.
unsafe fn restaurar(bus: u8, dev: u8, func: u8, g: &Guardado) {
    if !g.vivo { return; }
    for off in (0x10u8..=0x24).step_by(4) { cfg_w(bus, dev, func, off, g.cfg[off as usize / 4]); }
    cfg_w(bus, dev, func, 0x30, g.cfg[0x30 / 4]);
    cfg_w(bus, dev, func, 0x0C, g.cfg[0x0C / 4]);
    cfg_w(bus, dev, func, 0x3C, g.cfg[0x3C / 4]);
    if let Some(c) = pcie_cap(bus, dev, func) {
        // Only the low half of each: the high halves are status (RW1C) and
        // writing zeros there clears nothing.
        for k in [0x08u8, 0x10, 0x28, 0x30] {
            let Some(off) = c.checked_add(k) else { continue };
            cfg_w(bus, dev, func, off, g.cfg[off as usize / 4] & 0xFFFF);
        }
    }
}

unsafe fn comando(bus: u8, dev: u8, func: u8, cmd: u32) {
    // The high half is STATUS, write-one-to-clear: zeros leave it alone.
    cfg_w(bus, dev, func, 0x04, cmd & 0xFFFF);
}

/// The build-time switch: `no`, `siempre`, or (anything else) only warm.
fn interruptor() -> u64 {
    match option_env!("BMO_GPU_REINICIO") {
        Some("no") => GPU_REINICIO_APAGADO,
        Some("siempre") => GPU_REINICIO_SIEMPRE,
        _ => 0,
    }
}

/// **Look, and if warm, reset.** Before the memory map and before GOP.
pub unsafe fn reiniciar_si_caliente(ctx: &mut BootContext, system_table: *mut EfiSystemTable) {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let stall: extern "efiapi" fn(usize) -> EfiStatus = core::mem::transmute(*base.add(3 + 28));
    let connect: extern "efiapi" fn(EfiHandle, *mut EfiHandle, *mut core::ffi::c_void, u8) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 30));
    let disconnect: extern "efiapi" fn(EfiHandle, EfiHandle, EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 31));

    let mut f = GPU_REINICIO_MIRADO | interruptor();
    let Some((bus, dev)) = buscar() else {
        ctx.gpu_reinicio = f;
        return;
    };
    f |= GPU_REINICIO_HALLADA;
    let bar = cfg(bus, dev, 0, 0x10);
    let bar0 = (bar & 0xFFFF_FFF0) as u64;
    let decodifica = cfg(bus, dev, 0, 0x04) & 0x2 != 0;
    f |= velocidad(bus, dev, 0) << GPU_REINICIO_GEN_ANTES_SHIFT;
    // The two signals, only if BAR0 is a memory BAR the firmware turned on.
    if bar & 1 == 0 && bar0 != 0 && decodifica {
        let (riscv, wpr2) = lecturas(bar0);
        ctx.gpu_reinicio_antes = riscv as u64 | (wpr2 as u64) << 32;
        if !es_error(riscv) && riscv & RISCV_ACTIVO != 0 { f |= GPU_REINICIO_POR_RISCV; }
        if !es_error(wpr2) && wpr2 >> 4 != 0 { f |= GPU_REINICIO_POR_WPR2; }
        if f & (GPU_REINICIO_POR_RISCV | GPU_REINICIO_POR_WPR2) != 0 { f |= GPU_REINICIO_CALIENTE; }
    }
    ser_print!("[s1_cpu] gpu: NVIDIA at bus 0x"); ser_hex!(bus as u64);
    ser_print!(" riscv/wpr2 0x"); ser_hex!(ctx.gpu_reinicio_antes);
    ser_print!(if f & GPU_REINICIO_CALIENTE != 0 { " WARM\n" } else { " cold\n" });

    let toca = f & GPU_REINICIO_APAGADO == 0 && f & (GPU_REINICIO_CALIENTE | GPU_REINICIO_SIEMPRE) != 0;
    if !toca || bar & 1 != 0 || bar0 == 0 {
        ctx.gpu_reinicio = f;
        return;
    }
    let Some((pb, pd, pf)) = puente(bus) else {
        ctx.gpu_reinicio = f | GPU_REINICIO_SIN_PUENTE;
        return;
    };
    // Without its UEFI handle its driver cannot be stopped, and resetting a
    // card under a running driver is how firmwares hang: no reset.
    let Some(h) = handle_de(system_table, bus, dev, 0) else {
        ctx.gpu_reinicio = f | GPU_REINICIO_SIN_HANDLE;
        return;
    };
    con_mark(system_table, "gpu-reset ");
    // The time it takes, counted from the waits (the TSC is not measured yet).
    let mut us: u64 = 0;
    let mut dormir = |u: usize| {
        stall(u);
        us += u as u64;
    };

    // Stop its driver (and the GOP it published), then save every function.
    let _ = disconnect(h, core::ptr::null_mut(), core::ptr::null_mut());
    let multi = cfg(bus, dev, 0, 0x0C) >> 16 & 0x80 != 0;
    let mut guardado = [NADA; 8];
    for func in 0..if multi { 8u8 } else { 1 } { guardado[func as usize] = guardar(bus, dev, func); }
    // What the stopped driver left in COMMAND (maybe decoding off): the end state.
    let mut cmd_fin = [0u32; 8];
    for func in 0..8 { cmd_fin[func] = guardado[func].cfg[1] & 0xFFFF; }

    // The pulse.
    let ctrl = cfg(pb, pd, pf, 0x3C);
    cfg_w(pb, pd, pf, 0x3C, ctrl | 0x0040 << 16);
    dormir(2_000);
    cfg_w(pb, pd, pf, 0x3C, ctrl & !(0x0040 << 16));
    f |= GPU_REINICIO_HECHO;
    // PCIe: 100 ms before the first config request; then until it answers
    // for real (not all-ones, not the 0x0001 of a "retry later").
    dormir(100_000);
    for _ in 0..200 {
        let id = cfg(bus, dev, 0, 0) & 0xFFFF;
        if id != 0xFFFF && id != 0x0001 { f |= GPU_REINICIO_VOLVIO; break; }
        dormir(10_000);
    }
    if f & GPU_REINICIO_VOLVIO != 0 {
        for func in 0..8u8 { restaurar(bus, dev, func, &guardado[func as usize]); }
        // Memory decoding on to watch its firmware boot again.
        comando(bus, dev, 0, cmd_fin[0] | 0x2);
        for _ in 0..400 {
            if gfw_listo(bar0) { f |= GPU_REINICIO_GFW; break; }
            dormir(10_000);
        }
        let (riscv, wpr2) = lecturas(bar0);
        ctx.gpu_reinicio_despues = riscv as u64 | (wpr2 as u64) << 32;
        for func in 0..8u8 {
            if guardado[func as usize].vivo { comando(bus, dev, func, cmd_fin[func as usize]); }
        }
        f |= velocidad(bus, dev, 0) << GPU_REINICIO_GEN_DESPUES_SHIFT;
    }
    // Back to its driver, recursively: the GOP and the console come back.
    if connect(h, core::ptr::null_mut(), core::ptr::null_mut(), 1) == EFI_SUCCESS {
        f |= GPU_REINICIO_GOP;
    }
    f |= (us / 1000 & 0xFFFF) << GPU_REINICIO_MS_SHIFT;
    ctx.gpu_reinicio = f;
    ser_print!("[s1_cpu] gpu reset: flags 0x"); ser_hex!(f);
    ser_print!(" after riscv/wpr2 0x"); ser_hex!(ctx.gpu_reinicio_despues); ser_print!("\n");
    con_mark(system_table, if f & GPU_REINICIO_GOP != 0 { "gpu-ok " } else { "gpu-nogop " });
}
