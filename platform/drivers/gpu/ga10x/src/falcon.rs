//! **LA PRUEBA DE FUEGO (M0d3)** -- el DMA de un falcon de la 3060 LEE una
//! pagina que BMO-X le presto por la IOMMU, y no puede leer una que no.
//!
//! capa: puro -- decide el ORDEN de las lecturas y escrituras; quien las hace es el kernel, por [`Registros`] y [`Reloj`] (L8)
//!
//! [eje]     CORRECCION -- el primer DMA de la 3060 que BMO-X pide
//!
//! # Por que existe (2026-09-24)
//!
//! M0d2 dejo la 3060 TRADUCIDA y una pagina prestada en `0x10000000`, y el
//! oraculo del kernel relee las tablas. Pero eso prueba lo que BMO-X ESCRIBIO,
//! no lo que la TARJETA ve. Lo que la tarjeta ve solo lo dice un DMA de la
//! tarjeta, y el primero que se puede pedir sin firmware es el del FALCON: su
//! motor de DMA se programa desde fuera (`DMATRF*`) con el procesador parado,
//! y copia de la RAM del PC a su memoria de datos (DMEM), que despues se lee
//! por PIO. Es EXACTAMENTE como nova-core y nouveau cargan el firmware del GSP
//! -- y es donde murio FastOS (su paso 2: el DMA del SEC2). Aqui, el falcon
//! del GSP, el mismo que corre FWSEC-FRTS.
//!
//! # De donde sale cada numero (Linux v6.17 nova-core `falcon.rs`, `regs.rs`,
//! `falcon/hal/ga102.rs`; y nouveau v6.10 `nvkm/falcon/ga102.c`, `gm200.c`)
//!
//! ```text
//!    +0x0f4  HWCFG2    bit 12 limpiando la memoria, bit 31 listo para reset
//!    +0x108  HWCFG     IMEM (bits 0..8) y DMEM (9..17) en bloques de 256 B
//!    +0x12c  HWCFG1    revision y modelo de seguridad
//!    +0x3c0  ENGINE    bit 0 = RESET del motor (el de GA102, no el de PMC)
//!    +0x1668 BCR_CTRL  bit 4 = el nucleo es el RISC-V: se pide el FALCON
//!    +0x084  RM        se escribe BOOT_0 tras el reset
//!    +0x624  FBIF_CTL  bit 7 = direcciones fisicas sin contexto
//!    +0x10c  DMACTL    0 = sin contexto obligatorio
//!    +0x600  TRANSCFG  destino 1 (memoria del PC coherente), bit 2 fisico
//!    +0x110  DMATRFBASE    la direccion >> 8 (la del APARATO: la IOVA)
//!    +0x128  DMATRFBASE1   la direccion >> 40
//!    +0x114  DMATRFMOFFS   donde en la DMEM
//!    +0x11c  DMATRFFBOFFS  desde donde en la base
//!    +0x118  DMATRFCMD     medida 6 = 256 B (bits 8..10); bit 1 = libre
//!    +0x1c0  DMEMC     bit 25 | offset: leer la DMEM con autoincremento
//!    +0x1c4  DMEMD     la palabra siguiente
//! ```
//!
//! Los cuatro fallos de FastOS (`GPU_NVIDIA_MAESTRO.md` 6b), uno por uno:
//! el reset por el registro de MOTOR y esperando el borrado de memoria; el
//! FBIF programado; la direccion que se da es la del APARATO (la IOVA que
//! traduce la IOMMU), nunca un puntero virtual; y la firma no entra aqui:
//! este DMA no ejecuta nada, solo copia.
//!
//! # Lo que NO hace, a proposito
//!
//! No arranca el procesador del falcon (ni `BOOTVEC` ni `STARTCPU`): la DMEM
//! se llena y se lee con el nucleo parado. Y al acabar, el falcon se vuelve a
//! resetear: su DMEM se borra y queda como estaba.

use crate::{es_error_pri, Registros};

/// El falcon del GSP (`NV_PGSP_*`, nova-core `falcon/gsp.rs`).
pub const GSP: u32 = 0x0011_0000;
/// El del SEC2 (`falcon/sec2.rs`), por si hace falta el otro.
pub const SEC2: u32 = 0x0084_0000;

pub const HWCFG2: u32 = 0x0f4;
pub const HWCFG: u32 = 0x108;
pub const HWCFG1: u32 = 0x12c;
pub const ENGINE: u32 = 0x3c0;
pub const BCR_CTRL: u32 = 0x1668;
pub const RM: u32 = 0x084;
pub const FBIF_CTL: u32 = 0x624;
pub const DMACTL: u32 = 0x10c;
pub const TRANSCFG: u32 = 0x600;
pub const DMATRFBASE: u32 = 0x110;
pub const DMATRFBASE1: u32 = 0x128;
pub const DMATRFMOFFS: u32 = 0x114;
pub const DMATRFFBOFFS: u32 = 0x11c;
pub const DMATRFCMD: u32 = 0x118;
pub const DMEMC: u32 = 0x1c0;
pub const DMEMD: u32 = 0x1c4;

const HWCFG2_LIMPIANDO: u32 = 1 << 12;
const HWCFG2_LISTO_RESET: u32 = 1 << 31;
const BCR_RISCV: u32 = 1 << 4;
const BCR_VALIDO: u32 = 1 << 0;
const FBIF_FISICA_SIN_CTX: u32 = 1 << 7;
/// TRANSCFG del contexto 0: destino (bits 0..1), tipo (bit 2), y el 16 que
/// nouveau limpia con ellos (`nvkm_falcon_mask(0x600, 0x00010007, ...)`).
const TRANSCFG_MASCARA: u32 = 0x0001_0007;
const TRANSCFG_PC_COHERENTE_FISICA: u32 = 1 | 1 << 2;
const CMD_256B: u32 = 6 << 8;
const CMD_LIBRE: u32 = 1 << 1;
const DMEMC_LEER: u32 = 1 << 25;

/// Un trozo de DMA: 256 bytes (`DmaTrfCmdSize::Size256B`).
pub const TROZO: u32 = 256;

/// **Cuanto tiempo lleva pasando**, en microsegundos. El kernel lo da por el
/// TSC; las pruebas, con un reloj que avanza solo.
pub trait Reloj {
    fn us(&mut self) -> u64;
}

/// Por que no se pudo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoFuego {
    /// Un registro del falcon contesto con un error del anillo PRIV.
    NoContesta(u32),
    /// El falcon no acabo de limpiar su memoria en 20 ms.
    NoLimpia,
    /// Pidio el nucleo falcon y no dijo que lo tenia en 10 ms.
    NoNucleo,
    /// Su DMEM es mas chica que lo que se quiere copiar.
    DmemChica(u32),
    /// Un trozo de DMA no acabo en su plazo: el desplazamiento.
    DmaNoAcaba(u32),
}

/// Lo que dijo el falcon al preguntarle, antes de tocar nada.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Ficha {
    pub hwcfg1: u32,
    pub hwcfg2: u32,
    /// Bytes de IMEM y de DMEM.
    pub imem: u32,
    pub dmem: u32,
}

impl Ficha {
    /// El modelo de seguridad (bits 4..5 de HWCFG1): 0 ninguno, 2 ligero,
    /// 3 PESADO (solo ejecuta codigo firmado).
    pub const fn seguridad(&self) -> u32 {
        (self.hwcfg1 >> 4) & 3
    }
    pub const fn tiene_riscv(&self) -> bool {
        self.hwcfg2 & (1 << 10) != 0
    }
}

/// **Preguntar al falcon** de `base`. Solo lee.
pub fn ficha(r: &mut impl Registros, base: u32) -> Result<Ficha, NoFuego> {
    let hwcfg1 = leer(r, base + HWCFG1)?;
    let hwcfg2 = leer(r, base + HWCFG2)?;
    let caps = leer(r, base + HWCFG)?;
    Ok(Ficha { hwcfg1, hwcfg2, imem: (caps & 0x1FF) << 8, dmem: (caps & 0x3_FE00) >> 1 })
}

fn leer(r: &mut impl Registros, reg: u32) -> Result<u32, NoFuego> {
    let v = r.leer(reg);
    if es_error_pri(v) {
        Err(NoFuego::NoContesta(reg))
    } else {
        Ok(v)
    }
}

/// Espera a que `hecho(leido)` diga que si, o `plazo_us`.
fn esperar(
    r: &mut impl Registros,
    t: &mut impl Reloj,
    reg: u32,
    plazo_us: u64,
    hecho: impl Fn(u32) -> bool,
) -> Result<bool, NoFuego> {
    let fin = t.us() + plazo_us;
    loop {
        if hecho(leer(r, reg)?) {
            return Ok(true);
        }
        if t.us() >= fin {
            return Ok(false);
        }
    }
}

/// **RESETEAR el falcon** como nova-core (`Falcon::reset`): esperar al listo
/// (sin fallar: el hardware a veces no lo dice), RESET del motor, esperar el
/// borrado de su memoria, pedir el nucleo FALCON, y apuntar BOOT_0 en `RM`.
pub fn resetear(r: &mut impl Registros, t: &mut impl Reloj, base: u32, boot0: u32) -> Result<(), NoFuego> {
    esperar(r, t, base + HWCFG2, 150, |v| v & HWCFG2_LISTO_RESET != 0)?;
    let e = leer(r, base + ENGINE)?;
    r.escribir(base + ENGINE, e | 1);
    let desde = t.us();
    while t.us() < desde + 10 {}
    r.escribir(base + ENGINE, e & !1);
    if !esperar(r, t, base + HWCFG2, 20_000, |v| v & HWCFG2_LIMPIANDO == 0)? {
        return Err(NoFuego::NoLimpia);
    }
    let bcr = leer(r, base + BCR_CTRL)?;
    if bcr & BCR_RISCV != 0 {
        r.escribir(base + BCR_CTRL, 0);
        if !esperar(r, t, base + BCR_CTRL, 10_000, |v| v & BCR_VALIDO != 0)? {
            return Err(NoFuego::NoNucleo);
        }
    }
    if !esperar(r, t, base + HWCFG2, 20_000, |v| v & HWCFG2_LIMPIANDO == 0)? {
        return Err(NoFuego::NoLimpia);
    }
    r.escribir(base + RM, boot0);
    Ok(())
}

/// **Traer `bytes` desde la direccion del APARATO `iova` a la DMEM** en
/// `dmem`, a trozos de 256 B (`dma_load` + `dma_wr` de nova-core, sin firma
/// y sin arrancar nada). `iova` y `bytes` van a 256.
pub fn traer(
    r: &mut impl Registros,
    t: &mut impl Reloj,
    base: u32,
    iova: u64,
    dmem: u32,
    bytes: u32,
    plazo_trozo_us: u64,
) -> Result<(), NoFuego> {
    let fbif = leer(r, base + FBIF_CTL)?;
    r.escribir(base + FBIF_CTL, fbif | FBIF_FISICA_SIN_CTX);
    r.escribir(base + DMACTL, 0);
    let tc = leer(r, base + TRANSCFG)?;
    r.escribir(base + TRANSCFG, (tc & !TRANSCFG_MASCARA) | TRANSCFG_PC_COHERENTE_FISICA);
    r.escribir(base + DMATRFBASE, (iova >> 8) as u32);
    r.escribir(base + DMATRFBASE1, ((iova >> 40) & 0x1FF) as u32);
    let mut pos = 0;
    while pos < bytes {
        r.escribir(base + DMATRFMOFFS, dmem + pos);
        r.escribir(base + DMATRFFBOFFS, pos);
        r.escribir(base + DMATRFCMD, CMD_256B);
        if !esperar(r, t, base + DMATRFCMD, plazo_trozo_us, |v| v & CMD_LIBRE != 0)? {
            return Err(NoFuego::DmaNoAcaba(pos));
        }
        pos += TROZO;
    }
    Ok(())
}

/// **Leer la DMEM por PIO**: `n` palabras desde `dmem`, con autoincremento.
pub fn leer_dmem(r: &mut impl Registros, base: u32, dmem: u32, fuera: &mut [u32]) {
    r.escribir(base + DMEMC, DMEMC_LEER | dmem);
    for w in fuera.iter_mut() {
        *w = r.leer(base + DMEMD);
    }
}

/// Lo que salio de comparar la DMEM con el patron.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cuenta {
    pub bien: u32,
    /// La primera que no cuadra: `(indice, leido)`.
    pub primera_mal: Option<(u32, u32)>,
}

/// **Comparar** con `patron | i`, que es como `dev/gpu_prestamo.rs` llena la
/// pagina de prueba.
pub fn contar(leido: &[u32], patron: u32) -> Cuenta {
    let mut c = Cuenta { bien: 0, primera_mal: None };
    for (i, &w) in leido.iter().enumerate() {
        if w == patron | i as u32 {
            c.bien += 1;
        } else if c.primera_mal.is_none() {
            c.primera_mal = Some((i as u32, w));
        }
    }
    c
}

// ===================================================================
//  PRUEBAS -- un falcon de mentira que COPIA de verdad
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec;
    use std::vec::Vec;

    /// Un falcon: registros, una DMEM de 64 KiB, y una "RAM del PC" vista por
    /// la IOMMU -- solo lo prestado existe. El DMA copia de verdad cuando se
    /// escribe `DMATRFCMD`; lo no prestado lee todo unos y se apunta como fallo
    /// (lo que haria la IOMMU con un IO_PAGE_FAULT).
    struct Falcon {
        regs: Vec<(u32, u32)>,
        dmem: Vec<u32>,
        prestada: (u64, Vec<u32>),
        fallos: Vec<u64>,
        dmemc: u32,
        riscv: bool,
        nunca_limpia: bool,
        dma_colgado: bool,
        escritos: Vec<(u32, u32)>,
    }

    impl Falcon {
        fn nuevo() -> Self {
            let pagina: Vec<u32> = (0..1024).map(|i| 0xB0B0_0000 | i).collect();
            Falcon {
                regs: Vec::new(),
                dmem: vec![0; 16384],
                prestada: (0x1000_0000, pagina),
                fallos: Vec::new(),
                dmemc: 0,
                riscv: true,
                nunca_limpia: false,
                dma_colgado: false,
                escritos: Vec::new(),
            }
        }
        fn reg(&self, r: u32) -> u32 {
            self.regs.iter().find(|(k, _)| *k == r).map_or(0, |e| e.1)
        }
        fn poner(&mut self, r: u32, v: u32) {
            match self.regs.iter_mut().find(|(k, _)| *k == r) {
                Some(e) => e.1 = v,
                None => self.regs.push((r, v)),
            }
        }
        fn copiar(&mut self) {
            let iova = (self.reg(GSP + DMATRFBASE) as u64) << 8 | (self.reg(GSP + DMATRFBASE1) as u64) << 40;
            let desde = iova + self.reg(GSP + DMATRFFBOFFS) as u64;
            let a = self.reg(GSP + DMATRFMOFFS) as usize / 4;
            for k in 0..(TROZO as usize / 4) {
                let dir = desde + k as u64 * 4;
                let (base, ref pag) = self.prestada;
                let w = if dir >= base && dir < base + 4096 {
                    pag[((dir - base) / 4) as usize]
                } else {
                    if !self.fallos.contains(&(dir & !0xFFF)) {
                        self.fallos.push(dir & !0xFFF);
                    }
                    0xFFFF_FFFF
                };
                self.dmem[a + k] = w;
            }
        }
    }

    impl Registros for Falcon {
        fn leer(&mut self, reg: u32) -> u32 {
            match reg - GSP {
                HWCFG2 => {
                    (if self.nunca_limpia { HWCFG2_LIMPIANDO } else { 0 })
                        | HWCFG2_LISTO_RESET
                        | if self.riscv { 1 << 10 } else { 0 }
                }
                HWCFG => (0x100 << 9) | 0x100, // 64 KiB de IMEM y de DMEM
                HWCFG1 => 0x0000_0036,
                BCR_CTRL => (if self.riscv { BCR_RISCV } else { 0 }) | BCR_VALIDO,
                DMATRFCMD => {
                    if self.dma_colgado {
                        0
                    } else {
                        CMD_LIBRE
                    }
                }
                DMEMD => {
                    let i = (self.dmemc & 0xFFFF) as usize / 4;
                    self.dmemc += 4;
                    self.dmem[i]
                }
                _ => self.reg(reg),
            }
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            self.escritos.push((reg, v));
            match reg - GSP {
                DMATRFCMD => self.copiar(),
                DMEMC => self.dmemc = v & 0xFFFF,
                BCR_CTRL => {
                    self.riscv = v & BCR_RISCV != 0;
                    self.poner(reg, v);
                }
                _ => self.poner(reg, v),
            }
        }
    }

    struct Tic(u64);
    impl Reloj for Tic {
        fn us(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
    }

    #[test]
    fn el_fuego_lee_la_pagina_prestada_entera() {
        let mut f = Falcon::nuevo();
        let mut t = Tic(0);
        let ficha = ficha(&mut f, GSP).unwrap();
        assert_eq!(ficha.dmem, 64 * 1024);
        assert_eq!(ficha.seguridad(), 3, "el del GSP es PESADO");
        resetear(&mut f, &mut t, GSP, 0xB760_00A1).unwrap();
        assert_eq!(f.reg(GSP + RM), 0xB760_00A1, "BOOT_0 en RM tras el reset");
        assert_eq!(f.reg(GSP + BCR_CTRL), 0, "se pidio el nucleo FALCON");
        traer(&mut f, &mut t, GSP, 0x1000_0000, 0, 4096, 1000).unwrap();
        let mut leido = [0u32; 1024];
        leer_dmem(&mut f, GSP, 0, &mut leido);
        let c = contar(&leido, 0xB0B0_0000);
        assert_eq!(c, Cuenta { bien: 1024, primera_mal: None });
        assert!(f.fallos.is_empty());
        assert_eq!(f.escritos.iter().filter(|(r, _)| *r == GSP + DMATRFCMD).count(), 16, "16 trozos de 256 B");
        // El FBIF: fisico, a la RAM del PC, sin contexto.
        assert_eq!(f.reg(GSP + TRANSCFG) & TRANSCFG_MASCARA, 1 | 1 << 2);
        assert_eq!(f.reg(GSP + FBIF_CTL) & FBIF_FISICA_SIN_CTX, FBIF_FISICA_SIN_CTX);
        assert_eq!(f.reg(GSP + DMACTL), 0);
        assert_eq!(f.reg(GSP + DMATRFBASE), 0x0010_0000, "la IOVA >> 8, no un puntero");
    }

    #[test]
    fn la_frontera_lo_no_prestado_no_se_lee() {
        let mut f = Falcon::nuevo();
        let mut t = Tic(0);
        resetear(&mut f, &mut t, GSP, 0).unwrap();
        traer(&mut f, &mut t, GSP, 0x2000_0000, 0x1000, TROZO, 1000).unwrap();
        let mut leido = [0u32; 64];
        leer_dmem(&mut f, GSP, 0x1000, &mut leido);
        assert_eq!(contar(&leido, 0xB0B0_0000).bien, 0);
        assert_eq!(f.fallos, [0x2000_0000], "el fallo lleva la direccion del APARATO");
    }

    #[test]
    fn una_pagina_a_medias_dice_cual_es_la_primera_mala() {
        let mut leido: Vec<u32> = (0..1024).map(|i| 0xB0B0_0000 | i).collect();
        leido[700] = 0xFFFF_FFFF;
        leido[900] = 0;
        assert_eq!(contar(&leido, 0xB0B0_0000), Cuenta { bien: 1022, primera_mal: Some((700, 0xFFFF_FFFF)) });
    }

    #[test]
    fn un_falcon_que_no_contesta_no_se_toca() {
        struct Muerto(Vec<(u32, u32)>);
        impl Registros for Muerto {
            fn leer(&mut self, _: u32) -> u32 {
                0xBADF_5620
            }
            fn escribir(&mut self, r: u32, v: u32) {
                self.0.push((r, v));
            }
        }
        let mut m = Muerto(Vec::new());
        assert_eq!(ficha(&mut m, GSP), Err(NoFuego::NoContesta(GSP + HWCFG1)));
        assert!(matches!(resetear(&mut m, &mut Tic(0), GSP, 0), Err(NoFuego::NoContesta(_))));
        assert!(m.0.is_empty(), "el 0xBADF de FastOS: sin contestar no se escribe NADA");
    }

    #[test]
    fn si_no_limpia_o_el_dma_no_acaba_se_dice_y_no_se_espera_para_siempre() {
        let mut f = Falcon::nuevo();
        f.nunca_limpia = true;
        assert_eq!(resetear(&mut f, &mut Tic(0), GSP, 0), Err(NoFuego::NoLimpia));
        let mut f = Falcon::nuevo();
        resetear(&mut f, &mut Tic(0), GSP, 0).unwrap();
        f.dma_colgado = true;
        assert_eq!(traer(&mut f, &mut Tic(0), GSP, 0x1000_0000, 0, 4096, 100), Err(NoFuego::DmaNoAcaba(0)));
    }

    #[test]
    fn los_numeros_son_los_de_nova_core() {
        assert_eq!(GSP + HWCFG2, 0x1100f4);
        assert_eq!(GSP + ENGINE, 0x1103c0, "el reset por el registro de MOTOR (fallo 1 de FastOS)");
        assert_eq!(GSP + TRANSCFG, 0x110600);
        assert_eq!(GSP + FBIF_CTL, 0x110624, "el FBIF (fallo 2 de FastOS)");
        assert_eq!(GSP + BCR_CTRL, 0x111668);
        assert_eq!(CMD_256B, 0x600);
        assert_eq!(SEC2 + DMATRFCMD, 0x840118);
    }
}
