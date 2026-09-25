#![no_std]
/// Boot context ??? shared struct passed between boot layers.
///
/// Each layer of the Ultra_kernel_x86-64 boot chain receives a `BootContext`,
/// fills the fields it owns, and jumps to the next layer. There are no
/// function calls between layers ??? only raw jumps, so this struct IS the
/// ABI.

pub const MAGIC: u64 = 0x464F_5343_424F_4F54; // "FOSCBOOT"
pub const VERSION: u32 = 3;

// == THE LOADER'S GPU RESET (2026-09-25) ======================================
//
// A card that never lost power since another OS (or a BMO-X reboot without
// `gpu apagar`) started its GSP cannot be woken again: `despertar` answers
// 67 or the booter 0x15. Linux answers that with a PCIe reset; BMO-X does it
// in `s1_cpu`, BEFORE ExitBootServices, so the card's own UEFI driver can
// light the monitor again. What happened travels in `gpu_reinicio`.

/// The loader looked (0 everywhere = an older loader).
pub const GPU_REINICIO_MIRADO: u64 = 1 << 0;
/// An NVIDIA display controller was on the bus.
pub const GPU_REINICIO_HALLADA: u64 = 1 << 1;
/// It came WARM: the GSP RISC-V active or the WPR2 up.
pub const GPU_REINICIO_CALIENTE: u64 = 1 << 2;
/// Built with `BMO_GPU_REINICIO=no`: never reset.
pub const GPU_REINICIO_APAGADO: u64 = 1 << 3;
/// Built with `BMO_GPU_REINICIO=siempre`: reset even when cold (to test it).
pub const GPU_REINICIO_SIEMPRE: u64 = 1 << 4;
/// The secondary bus reset was pulsed.
pub const GPU_REINICIO_HECHO: u64 = 1 << 5;
/// The card answered its config space again afterwards.
pub const GPU_REINICIO_VOLVIO: u64 = 1 << 6;
/// Its own firmware (GFW) finished booting afterwards.
pub const GPU_REINICIO_GFW: u64 = 1 << 7;
/// ConnectController gave the card back to its UEFI driver (the monitor).
pub const GPU_REINICIO_GOP: u64 = 1 << 8;
/// No bridge above it: nothing to pulse.
pub const GPU_REINICIO_SIN_PUENTE: u64 = 1 << 9;
/// No UEFI handle for it: its driver could not be stopped, so no reset.
pub const GPU_REINICIO_SIN_HANDLE: u64 = 1 << 10;
/// Why it was warm: the GSP RISC-V was active...
pub const GPU_REINICIO_POR_RISCV: u64 = 1 << 11;
/// ...or the WPR2 was up.
pub const GPU_REINICIO_POR_WPR2: u64 = 1 << 12;
/// PCIe link speed (Gen) before and after, 4 bits each.
pub const GPU_REINICIO_GEN_ANTES_SHIFT: u64 = 16;
pub const GPU_REINICIO_GEN_DESPUES_SHIFT: u64 = 20;
/// How long the whole reset took, in ms (16 bits).
pub const GPU_REINICIO_MS_SHIFT: u64 = 32;
pub const MAX_MEMORY_ENTRIES: usize = 64;
pub const MAX_STAGES: usize = 13; // 12 Faggin stages + kernel
pub const KERNEL_STAGE_INDEX: usize = MAX_STAGES - 1;
pub const MAX_CHANNEL_PAGES: usize = 16; // bmo-platform's NUM_ESTUARIES

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryEntry {
    pub base: u64,
    pub size: u64,
    pub kind: u32, // 1=usable, 2=reserved, 3=ACPI, 4=ACPI NVS, 5=bad
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub class: u8,
    pub subclass: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub bar0: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootContext {
    // ?????? Layer 0: uefi_enter puts ???????????????????????????????????????????????????????????????????????????
    pub magic: u64,
    pub version: u32,

    // ?????? Layer 1: uefi_efi_getmem puts ?????????????????????????????????????????????????????????
    pub memory_map_count: u32,
    pub memory_map: [MemoryEntry; MAX_MEMORY_ENTRIES],

    // ?????? Layer 2: uefi_efi_getgop puts ?????????????????????????????????????????????????????????
    pub fb_addr: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_stride: u32,
    pub fb_pixel_format: u32,

    // ?????? Layer 3: uefi_loader puts ????????????????????????????????????????????????????????????????????????
    pub rsdp: u64,
    pub stage_base: [u64; MAX_STAGES],
    pub stage_size: [u64; MAX_STAGES],
    pub stage_entry: [u64; MAX_STAGES],

    // ?????? Layer 5: stage1_arch puts ????????????????????????????????????????????????????????????????????????
    pub gdt_ptr: u64,
    pub idt_ptr: u64,
    pub tss_ptr: u64,
    pub syscall_entry: u64,
    pub tsc_freq: u64,
    pub kernel_stack_top: u64,

    // ?????? Layer 6: stage2_mm puts ??????????????????????????????????????????????????????????????????????????????
    pub pml4: u64,
    pub heap_base: u64,
    pub heap_size: u64,

    // ?????? Layer 7: stage3_dev puts ???????????????????????????????????????????????????????????????????????????
    pub ioapic_base: u64,
    pub hpet_base: u64,
    pub pci_count: u32,
    pub pci_devices: [PciDevice; 32],

    // ?????? Layer 8: kernel puts ???????????????????????????????????????????????????????????????????????????????????????
    pub kernel_stack: u64,
    pub ring3_stack: u64,

    // ?????? Layer 9: bmo-platform estuaries ??????????????????????????????????????????????????????????????????????????
    // Physical addresses of the estuary shared pages. The kernel
    // allocates these at boot, maps them into both Ring 0 and each
    // Ring 3 process's address space, and writes the physical
    // addresses here. Index 0 = Input, 1 = Framebuffer, 2 = Syscall,
    // 3 = Log, 4..15 = available for custom estuaries.
    pub channel_pages: [u64; MAX_CHANNEL_PAGES],

    // Layer 10: first Ring 3 launch image and kernel-owned workspace.
    // s1 reserves both ranges through UEFI before ExitBootServices.
    pub ring3_payload_phys: u64,
    pub ring3_payload_size: u64,
    pub ring3_workspace_phys: u64,
    pub ring3_workspace_size: u64,

    // Layer 11 (2026-09-25): the loader's GPU reset (`s1_cpu::gpu_reinicio`).
    // `gpu_reinicio` carries the GPU_REINICIO_* flags; `_antes` and
    // `_despues` the raw signals before and after (GSP RISC-V CPUCTL in the
    // low half, WPR2_HI in the high half). All zero = an older loader.
    pub gpu_reinicio: u64,
    pub gpu_reinicio_antes: u64,
    pub gpu_reinicio_despues: u64,

    // ?????? Padding for future use ?????????????????????????????????????????????????????????????????????????????????
    _reserved: [u64; 9],
}

impl BootContext {
    pub const fn new() -> Self {
        Self {
            magic: 0,
            version: 0,
            memory_map_count: 0,
            memory_map: [MemoryEntry { base: 0, size: 0, kind: 0 }; MAX_MEMORY_ENTRIES],
            fb_addr: 0,
            fb_width: 0,
            fb_height: 0,
            fb_stride: 0,
            fb_pixel_format: 0,
            rsdp: 0,
            stage_base: [0; MAX_STAGES],
            stage_size: [0; MAX_STAGES],
            stage_entry: [0; MAX_STAGES],
            gdt_ptr: 0,
            idt_ptr: 0,
            tss_ptr: 0,
            syscall_entry: 0,
            tsc_freq: 0,
            kernel_stack_top: 0,
            pml4: 0,
            heap_base: 0,
            heap_size: 0,
            ioapic_base: 0,
            hpet_base: 0,
            pci_count: 0,
            pci_devices: [PciDevice { bus: 0, device: 0, function: 0, class: 0, subclass: 0, vendor_id: 0, device_id: 0, bar0: 0 }; 32],
            kernel_stack: 0,
            ring3_stack: 0,
            channel_pages: [0; MAX_CHANNEL_PAGES],
            ring3_payload_phys: 0,
            ring3_payload_size: 0,
            ring3_workspace_phys: 0,
            ring3_workspace_size: 0,
            gpu_reinicio: 0,
            gpu_reinicio_antes: 0,
            gpu_reinicio_despues: 0,
            _reserved: [0; 9],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC && self.version == VERSION
    }

    pub fn set_memory_map(&mut self, entries: &[MemoryEntry]) {
        let count = entries.len().min(MAX_MEMORY_ENTRIES);
        self.memory_map_count = count as u32;
        for i in 0..count {
            self.memory_map[i] = entries[i];
        }
    }

    pub fn usable_memory(&self) -> impl core::iter::Iterator<Item = &MemoryEntry> {
        self.memory_map[..self.memory_map_count as usize]
            .iter()
            .filter(|e| e.kind == 1 && e.size > 0)
    }
}

// ===================================================================
//  Preload handoff (unified-EFI boot)
// ===================================================================

/// "BMOPRLD1" -- magic of the shim->s1 preload handoff block.
pub const PRELOAD_MAGIC: u64 = 0x424D_4F50_524C_4431;

/// Handoff from the UEFI shim when s2_mem and the kernel were embedded in
/// BOOTX64.EFI and copied to their fixed load addresses before s1 runs.
/// Passed to `s1_entry` as the third (r8) argument; null or a wrong magic
/// means "not preloaded" and s1 falls back to loading from the ESP.
///
/// Rationale: some firmwares (MSI A320M AMI fast path) load the boot
/// application with an internal FAT reader and never bind
/// SimpleFileSystem to any handle -- the ESP is unreadable through UEFI
/// protocols even after a recursive ConnectController pass.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PreloadInfo {
    pub magic: u64,
    pub s2_size: u64,
    pub kernel_size: u64,
    /// Address of the kernel image bytes (inside the shim's loaded PE).
    /// The kernel is NOT copied to 0x400000 while Boot Services are alive:
    /// firmwares keep allocations in that range, so AllocateAddress on the
    /// 16 MiB slot can fail (observed on MSI A320M). s1 performs the copy
    /// right AFTER ExitBootServices, when that memory becomes ours.
    pub kernel_src: u64,
}
