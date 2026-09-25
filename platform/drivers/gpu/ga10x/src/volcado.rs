//! **EL VOLCADO POR LA 3060** (compositor por GPU, paso 1; 2026-09-25) -- el
//! motor de COPIA (COPY2, el de L1d3) lleva el lienzo del escritorio --RAM del
//! PC-- al framebuffer del GOP --VRAM, donde mira el monitor-- en UNA orden,
//! en vez de que la CPU mueva 8 MiB con `rep movsb`.
//!
//! capa: puro -- el mapa, las ordenes y lo que se espera; la RAM, la IOMMU y
//! los registros los toca el kernel (L8)
//!
//! [eje]     RENDIMIENTO -- la CPU deja de mover pixeles del escritorio; es la
//!           costura que `userland::pantalla::Volcador` dejo escrita el 09-09
//!           ("esta es la costura donde entra una GPU")
//!
//! # Donde vive cada cosa
//!
//! ```text
//!    ORIGEN   el lienzo del escritorio (KIND_MEMORIA CONTIGUO, residente):
//!             prestado a la 3060 SOLO LECTURA en la IOVA 0x5000_0000, y
//!             visto por la GPU en la VA 0x5_0000_0000 (la PD1 del tramo,
//!             entrada 40; tablas en VRAM 0x0450_0000), PTE de SISTEMA
//!    DESTINO  la pantalla del GOP en la VA de `pantalla` (0x4_0000_0000)
//!    ORDENES  el canal de COPIA: la pagina de ordenes de L1d3 en +0x800,
//!             el semaforo en la pagina de L1d3 +0x100, y la entrada
//!             siguiente de su GPFIFO (la 0 fue la copia de L1d3)
//! ```
//!
//! # La orden (`clc7b5.h`)
//!
//! Una copia PITCH a PITCH de `alto` lineas de `ancho x 4` bytes, con el
//! MISMO paso en los dos lados (el lienzo tiene el stride del panel:
//! `Pantalla::activar_doble_bufer`), y `LAUNCH_DMA` con MULTI_LINE (bit 9):
//! sin el, `LINE_COUNT` no cuenta y se copiaria una linea.
//!
//! # Como se sabe
//!
//! Antes de tocar el timbre, el kernel pinta en la pantalla las 1024
//! muestras de `pantalla::muestra` con el color CONTRARIO al del lienzo; tras
//! el semaforo, las relee. Una muestra igual al lienzo solo la pudo poner la
//! 3060.

use crate::canal::{GPFIFO, GPFIFO_ENTRADAS, USERD};
use crate::copia::{cabecera, entrada, escribir, invalidar, leer32, va, AMPERE_DMA_COPY_B, GP_GET, GP_PUT, LANZAR, TIMBRE};
use crate::mmu::{indices, pde_vram, pte_sistema};
use crate::pantalla::Pantalla;
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

/// Donde ve la GPU el lienzo del escritorio.
pub const VA: u64 = 0x5_0000_0000;
/// Donde lo ve la 3060 por la IOMMU.
pub const IOVA: u64 = 0x5000_0000;
/// La PD0 y las PT del lienzo, en VRAM (tras las de la pantalla, 0x0440_0000).
pub const TABLAS: u64 = 0x0450_0000;
/// Cuantas PT caben: 16 x 2 MiB (hasta 3840x2160, como la pantalla).
pub const PTS: usize = 16;
pub const MAX_BYTES: u64 = PTS as u64 * (2 << 20);

const PAGINA: u64 = 0x1000;
/// Las ordenes y el semaforo, en las paginas del canal de copia.
pub const EMPUJE: u64 = crate::copia::EMPUJE + 0x800;
pub const SEMAFORO: u64 = crate::copia::SEMAFORO + 0x100;
/// Lo que escribe la 3060 al acabar.
pub const PAGA: u32 = 0x3060_B117;
/// La primera entrada del GPFIFO de copia (la 0 fue L1d3).
pub const PRIMERA_ENTRADA: u32 = 1;

/// Los metodos de `clc7b5.h` que no estan en `copia`.
const SET_OBJECT: u32 = 0x000;
const SET_SEMAPHORE_A: u32 = 0x240;
const LAUNCH_DMA: u32 = 0x300;
const OFFSET_IN_UPPER: u32 = 0x400;
/// `LAUNCH_DMA_MULTI_LINE_ENABLE`.
pub const MULTI_LINE: u32 = 1 << 9;

pub const ORDENES: usize = 17;

/// La entrada de la PD1 del tramo que cuelga [`VA`].
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// Paginas del lienzo de una pantalla.
pub const fn paginas(p: &Pantalla) -> u64 {
    p.bytes().div_ceil(PAGINA)
}

/// Se puede volcar: una pantalla valida que cabe en las 16 PT.
pub const fn cabe(p: &Pantalla) -> bool {
    p.valida() && p.bytes() <= MAX_BYTES
}

/// **Mapear el lienzo** en [`VA`] con PTE de SISTEMA hacia [`IOVA`]: la PD0 y
/// las PT a cero, las PTE, las PDE de la PD0 y al final la de la PD1 (de la
/// hoja a la raiz), todo RELEIDO. Solo si la entrada de la PD1 esta VACIA o
/// ya es la nuestra. El mapa no depende de DONDE esta el lienzo en la RAM
/// (eso lo pone la IOMMU), asi que se hace una vez. `(escrituras, releidas)`.
pub fn mapear<R: Registros>(r: &mut R, p: &Pantalla) -> Option<(u32, u32)> {
    if !cabe(p) || VA % (2 << 20) != 0 {
        return None;
    }
    let pd1 = leer64(r, entrada_pd1());
    if pd1 != 0 && pd1 != pde_vram(TABLAS) {
        return None;
    }
    let paginas = paginas(p);
    let pts = paginas.div_ceil(512) as usize;
    let pt = |k: usize| TABLAS + PAGINA * (1 + k as u64);
    if pd1 == 0 {
        for k in 0..=pts {
            a_cero(r, TABLAS + PAGINA * k as u64);
        }
    }
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    for q in 0..paginas {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_sistema(IOVA + q * PAGINA));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..pts {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

/// **Las ordenes**: atar la clase, el lienzo entero a la pantalla entera en
/// una copia de `alto` lineas, el semaforo, y lanzar.
pub fn ordenes(p: &Pantalla) -> [u32; ORDENES] {
    let (o, d, s) = (VA, crate::pantalla::VA, va(SEMAFORO));
    let paso = p.pitch * 4;
    [
        cabecera(SET_OBJECT, 1),
        AMPERE_DMA_COPY_B,
        cabecera(OFFSET_IN_UPPER, 8),
        (o >> 32) as u32,
        o as u32,
        (d >> 32) as u32,
        d as u32,
        paso,        // PITCH_IN
        paso,        // PITCH_OUT
        p.ancho * 4, // LINE_LENGTH_IN
        p.alto,      // LINE_COUNT
        cabecera(SET_SEMAPHORE_A, 3),
        (s >> 32) as u32,
        s as u32,
        PAGA,
        cabecera(LAUNCH_DMA, 1),
        LANZAR | MULTI_LINE,
    ]
}

/// La entrada del GPFIFO que sigue a `e`.
pub const fn siguiente(e: u32) -> u32 {
    (e + 1) % GPFIFO_ENTRADAS
}

/// **Preparar**: el semaforo a cero, las ordenes y la entrada `e` del GPFIFO
/// de copia, todo RELEIDO. `true` si todo quedo.
pub fn preparar<R: Registros>(r: &mut R, e: u32, p: &Pantalla) -> bool {
    if e >= GPFIFO_ENTRADAS || !cabe(p) {
        return false;
    }
    let o = ordenes(p);
    let en = entrada(va(EMPUJE), ORDENES as u32);
    escribir(r, SEMAFORO, &[0]) == 1
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GPFIFO + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

/// **Lanzar** la entrada `e`: la MMU invalidada, GP_PUT a la siguiente y la
/// ficha en el timbre. `false` (y el timbre sin tocar) si algo no quedo.
pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = invalidar(r) && escribir(r, USERD + GP_PUT, &[siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// Lo que se mira mientras se espera: `(GP_GET, el semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    (leer32(r, USERD + GP_GET), leer32(r, SEMAFORO))
}

/// El color que el kernel pone en una muestra ANTES de la copia: el contrario
/// del lienzo, asi una muestra igual al lienzo solo la pudo poner la 3060.
pub const fn contrario(c: u32) -> u32 {
    !c & 0x00FF_FFFF
}

/// Lo que devuelve el kernel en un `u64`: `buenas | pagado << 16 | lanzado
/// << 17 | us de la 3060 << 32`.
pub const fn empaquetar(buenas: u32, pagado: bool, lanzado: bool, us: u32) -> u64 {
    (buenas as u64 & 0xFFFF) | (pagado as u64) << 16 | (lanzado as u64) << 17 | (us as u64) << 32
}

/// `(buenas, pagado, lanzado, us)`.
pub const fn desempaquetar(v: u64) -> (u32, bool, bool, u32) {
    ((v & 0xFFFF) as u32, v >> 16 & 1 != 0, v >> 17 & 1 != 0, (v >> 32) as u32)
}

/// Salio entero: lanzado, pagado y las 1024 muestras como el lienzo.
pub const fn sano(v: u64) -> bool {
    let (buenas, pagado, lanzado, _) = desempaquetar(v);
    lanzado && pagado && buenas == crate::pantalla::MUESTRAS
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::copia::cabecera_en;

    const FHD: Pantalla = Pantalla { vram: 0, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };

    #[test]
    fn no_pisa_a_nadie() {
        // La PD1 del tramo: el tramo 16, GR 24, la pantalla 32; esto, 40.
        assert_eq!(indices(VA)[2], 40);
        assert_ne!(entrada_pd1(), crate::pantalla::entrada_pd1());
        // Las tablas, tras las de la pantalla (PD0 + 16 PT) y antes de GR.
        assert!(TABLAS >= crate::pantalla::TABLAS + (1 + crate::pantalla::PTS as u64) * PAGINA);
        assert!(TABLAS + (1 + PTS as u64) * PAGINA <= 0x0800_0000);
        // Las ordenes y el semaforo, en las paginas de L1d3, sin pisar las suyas.
        assert!(EMPUJE >= crate::copia::EMPUJE + 4 * crate::copia::ORDENES as u64);
        assert!(EMPUJE + 4 * ORDENES as u64 <= crate::copia::EMPUJE + PAGINA);
        assert!(SEMAFORO < crate::copia::SEMAFORO + PAGINA);
        assert_eq!(siguiente(GPFIFO_ENTRADAS - 1), 0);
        assert!(cabe(&FHD) && cabe(&Pantalla { pitch: 3840, ancho: 3840, alto: 2160, ..FHD }));
    }

    #[test]
    fn la_orden_copia_la_pantalla_entera() {
        let o = ordenes(&FHD);
        assert_eq!(o[2], cabecera_en(4, OFFSET_IN_UPPER, 8));
        assert_eq!((o[3] as u64) << 32 | o[4] as u64, VA);
        assert_eq!((o[5] as u64) << 32 | o[6] as u64, crate::pantalla::VA);
        assert_eq!((o[7], o[8], o[9], o[10]), (7680, 7680, 7680, 1080));
        assert_eq!(o[14], PAGA);
        assert_eq!(o[16] & MULTI_LINE, MULTI_LINE, "sin MULTI_LINE se copia una linea");
        assert_eq!(o[16] & 3, 2, "NON_PIPELINED");
        assert_eq!(o[16] >> 12 & 3, 0, "origen y destino VIRTUALES");
    }

    #[test]
    fn el_paquete_ida_y_vuelta() {
        let v = empaquetar(1024, true, true, 812);
        assert_eq!(desempaquetar(v), (1024, true, true, 812));
        assert!(sano(v));
        assert!(!sano(empaquetar(1023, true, true, 812)));
        assert!(!sano(empaquetar(1024, false, true, 812)));
        assert_eq!(contrario(0x0012_3456), 0x00ED_CBA9);
        assert_ne!(contrario(0), 0);
    }

    #[test]
    fn el_mapa_de_la_hoja_a_la_raiz() {
        extern crate std;
        struct Vram(std::collections::BTreeMap<u64, u32>, u32);
        impl Registros for Vram {
            fn leer(&mut self, reg: u32) -> u32 {
                if reg == crate::vram::VENTANA_REG {
                    return self.1;
                }
                let d = ((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64;
                *self.0.get(&d).unwrap_or(&0)
            }
            fn escribir(&mut self, reg: u32, v: u32) {
                if reg == crate::vram::VENTANA_REG {
                    self.1 = v;
                    return;
                }
                let d = ((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64;
                self.0.insert(d, v);
            }
        }
        let mut r = Vram(Default::default(), 0);
        let (n, bien) = mapear(&mut r, &FHD).unwrap();
        assert_eq!(n as u64, paginas(&FHD) + 4 + 1, "2025 PTE, 4 PDE de la PD0 y la de la PD1");
        assert_eq!(bien, n);
        assert_eq!(leer64(&mut r, TABLAS + PAGINA), pte_sistema(IOVA));
        assert_eq!(leer64(&mut r, TABLAS + PAGINA + 8 * 7), pte_sistema(IOVA + 7 * PAGINA));
        assert_eq!(leer64(&mut r, entrada_pd1()), pde_vram(TABLAS));
        assert!(mapear(&mut r, &FHD).is_some(), "otra vez, la misma: vale");
        escribir64(&mut r, entrada_pd1(), pde_vram(0x0999_0000));
        assert!(mapear(&mut r, &FHD).is_none(), "otra cosa colgada ahi: no se pisa");
    }
}
