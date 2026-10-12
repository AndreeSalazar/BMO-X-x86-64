//! **EL PAT Y LOS MTRR DEL OBRERO, IGUALES A LOS DEL BSP** (12-10, contra
//! Linux: `arch/x86/kernel/cpu/cacheinfo.c`, `cache_cpu_init`).
//!
//! [carril]  ROJO      el tipo de memoria de cada pagina, en cada nucleo
//! [consumo] NADA      corre una vez por obrero, al llegar
//!
//! [cuesta]  MAQUINA -- dos nucleos que ven la MISMA pagina fisica con tipos
//!           de memoria distintos (WC en uno, WB en otro) es lo que el manual
//!           de AMD y el de Intel llaman "aliasing" y prohiben: el resultado
//!           no esta definido, y en un BAR de PCI puede ser un error de maquina
//!
//! # El hueco, visto al leer Linux
//!
//! `faggin/s1_cpu` pone en el BSP la entrada 4 del PAT en **WC** (el
//! framebuffer, como `ioremap_wc` de Linux). Los obreros NUNCA escribian su
//! PAT: se quedaban con el de reset, cuya entrada 4 es **WB**. Y el
//! framebuffer se mapea con esa entrada tambien en el espacio de las apps
//! (`obj/fb/roja.rs`, `map_page_wc`): con `smp all`, una parte de Ring 3 que
//! pinta veia el framebuffer WC en el BSP y WB en su obrero.
//!
//! Linux nunca deja que pase: cada AP, al subir (`cache_ap_online` ->
//! `cache_cpu_init`), copia los MTRR del BSP (`mtrr_generic_set_state`) y su
//! PAT (`pat_cpu_init`), con la cache apagada y el TLB vaciado. Aqui se hace lo
//! mismo, en el mismo orden (`cache_disable` / `cache_enable`), y ademas se
//! CUENTA cuantos obreros llegaron distintos: si el firmware ya los dejaba
//! iguales, el numero lo dice.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst};

const MSR_MTRRCAP: u32 = 0xFE;
const MSR_PAT: u32 = 0x277;
const MSR_DEFTYPE: u32 = 0x2FF;
/// Los once MTRR fijos (el primer MiB), si el CPU los tiene.
const FIJOS: [u32; 11] = [0x250, 0x258, 0x259, 0x268, 0x269, 0x26A, 0x26B, 0x26C, 0x26D, 0x26E, 0x26F];
/// Los variables que se copian como mucho (el 5600X tiene 8).
const VARIABLES_MAX: usize = 16;

struct Estado {
    pat: Option<u64>,
    /// `None` = el CPU no tiene MTRR.
    deftype: Option<u64>,
    fijos: Option<[u64; 11]>,
    vars: [(u64, u64); VARIABLES_MAX],
    n_vars: usize,
}

static mut BSP: Estado = Estado { pat: None, deftype: None, fijos: None, vars: [(0, 0); VARIABLES_MAX], n_vars: 0 };
static TOMADO: AtomicBool = AtomicBool::new(false);
/// Obreros que llegaron con OTRO PAT que el BSP (y se igualaron).
static PAT_DISTINTO: AtomicU32 = AtomicU32::new(0);
/// Obreros que llegaron con OTROS MTRR que el BSP (y se igualaron).
static MTRR_DISTINTO: AtomicU32 = AtomicU32::new(0);

unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    unsafe { core::arch::asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack)) };
    (hi as u64) << 32 | lo as u64
}

unsafe fn wrmsr(msr: u32, v: u64) {
    unsafe { core::arch::asm!("wrmsr", in("ecx") msr, in("eax") v as u32, in("edx") (v >> 32) as u32, options(nostack)) };
}

/// Lo que tiene ESTE nucleo.
unsafe fn leer() -> Estado {
    let edx = core::arch::x86_64::__cpuid(1).edx;
    let hay_pat = edx & (1 << 16) != 0;
    let hay_mtrr = edx & (1 << 12) != 0;
    let mut e = Estado { pat: None, deftype: None, fijos: None, vars: [(0, 0); VARIABLES_MAX], n_vars: 0 };
    unsafe {
        if hay_pat {
            e.pat = Some(rdmsr(MSR_PAT));
        }
        if hay_mtrr {
            let cap = rdmsr(MSR_MTRRCAP);
            e.deftype = Some(rdmsr(MSR_DEFTYPE));
            if cap & (1 << 8) != 0 {
                let mut f = [0u64; 11];
                for (k, &m) in FIJOS.iter().enumerate() {
                    f[k] = rdmsr(m);
                }
                e.fijos = Some(f);
            }
            e.n_vars = ((cap & 0xFF) as usize).min(VARIABLES_MAX);
            for i in 0..e.n_vars {
                e.vars[i] = (rdmsr(0x200 + 2 * i as u32), rdmsr(0x201 + 2 * i as u32));
            }
        }
    }
    e
}

/// **El BSP apunta su PAT y sus MTRR** antes de despertar a nadie
/// (`smp::despertar`). Una vez basta: no cambian en marcha.
pub fn tomar_del_bsp() {
    if TOMADO.load(SeqCst) {
        return;
    }
    // SAFETY: solo el BSP, antes de publicar TOMADO; los obreros solo leen
    // despues de verlo.
    unsafe {
        *core::ptr::addr_of_mut!(BSP) = leer();
    }
    TOMADO.store(true, SeqCst);
}

/// **El obrero se iguala al BSP** (Linux, `cache_cpu_init`). Lo llama el
/// propio obrero en `smp_ap_entrada`, con su TSS ya puesto. Si ya era igual,
/// no toca nada.
///
/// # Safety
/// Cambia CR0, CR4, el TLB, los MTRR y el PAT de ESTE nucleo. Solo desde un
/// obrero recien llegado, con IF=0 y sin nada mapeado con tipos propios.
pub unsafe fn igualar_en_este_nucleo() {
    if !TOMADO.load(SeqCst) {
        return;
    }
    // SAFETY: escrito una vez antes de TOMADO; aqui solo se lee.
    let bsp = unsafe { &*core::ptr::addr_of!(BSP) };
    let yo = unsafe { leer() };
    let pat_mal = bsp.pat.is_some() && yo.pat != bsp.pat;
    let mtrr_mal = bsp.deftype.is_some()
        && (yo.deftype != bsp.deftype || yo.fijos != bsp.fijos || yo.n_vars != bsp.n_vars || yo.vars[..yo.n_vars] != bsp.vars[..bsp.n_vars]);
    if !pat_mal && !mtrr_mal {
        return;
    }
    if pat_mal {
        PAT_DISTINTO.fetch_add(1, SeqCst);
    }
    if mtrr_mal {
        MTRR_DISTINTO.fetch_add(1, SeqCst);
    }
    unsafe {
        // -- cache_disable: CD=1 NW=0, wbinvd, sin PGE, TLB fuera, MTRR
        // desarmados, wbinvd otra vez.
        let cr0: u64;
        let cr4: u64;
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack));
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack));
        core::arch::asm!("mov cr0, {}", in(reg) (cr0 | 1 << 30) & !(1 << 29), options(nostack));
        core::arch::asm!("wbinvd", options(nostack));
        core::arch::asm!("mov cr4, {}", in(reg) cr4 & !(1 << 7), options(nostack));
        tlb_fuera();
        if let Some(d) = yo.deftype {
            // E (bit 11) y FE (bit 10) a cero: `mtrr_disable`.
            wrmsr(MSR_DEFTYPE, d & !(3 << 10));
        }
        core::arch::asm!("wbinvd", options(nostack));

        // -- mtrr_generic_set_state y pat_cpu_init.
        if mtrr_mal {
            if let (Some(f), Some(_)) = (bsp.fijos, yo.fijos) {
                for (k, &m) in FIJOS.iter().enumerate() {
                    wrmsr(m, f[k]);
                }
            }
            for i in 0..bsp.n_vars.min(yo.n_vars) {
                wrmsr(0x200 + 2 * i as u32, bsp.vars[i].0);
                wrmsr(0x201 + 2 * i as u32, bsp.vars[i].1);
            }
        }
        if let Some(p) = bsp.pat.filter(|_| pat_mal) {
            wrmsr(MSR_PAT, p);
        }

        // -- cache_enable: TLB fuera, MTRR armados (los del BSP), cache y CR4.
        tlb_fuera();
        if let Some(d) = if mtrr_mal { bsp.deftype } else { yo.deftype } {
            wrmsr(MSR_DEFTYPE, d);
        }
        core::arch::asm!("mov cr0, {}", in(reg) cr0 & !(1 << 30 | 1 << 29), options(nostack));
        core::arch::asm!("mov cr4, {}", in(reg) cr4, options(nostack));
    }
}

unsafe fn tlb_fuera() {
    unsafe {
        let cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
        core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nostack));
    }
}

/// `(obreros que llegaron con otro PAT, con otros MTRR)`: los dos se
/// igualaron al llegar. Lo dice `smp`.
pub fn distintos() -> (u32, u32) {
    (PAT_DISTINTO.load(SeqCst), MTRR_DISTINTO.load(SeqCst))
}
