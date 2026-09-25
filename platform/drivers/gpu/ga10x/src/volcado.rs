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

// == 1b: EL VOLCADO EN CADA FOTOGRAMA ==========================================
//
// Lo de arriba es UNA copia verificada (el paso de `save mode`). Para cada
// fotograma cambian tres cosas:
//
//    el prestamo   vive lo que el escritorio (ARMAR), y se devuelve al
//                  soltarlo o al morir el propietario de la pantalla
//    lo que va     solo las CAJAS SUCIAS del fotograma, todas en UNA tanda:
//                  un segmento de ordenes, UNA entrada del GPFIFO, UN timbre
//    la valla      el semaforo lleva el NUMERO de la tanda; se espera a que
//                  la 3060 lo escriba antes de que la CPU vuelva a pintar
//                  en el lienzo (si no, copiaria medio fotograma nuevo)
//
// Y se escribe por PRAMIN SIN releer: releer cada palabra es una lectura por
// PCIe (~1 us) y aqui van ~100 por fotograma. La prueba de que llego no es
// releer las ordenes: es que la 3060 pague el semaforo con ESE numero.

/// Las suborden de `IOMMU_OP_GPU_VOLCADOR`, en los bits 63..60 del argumento.
pub const ARMAR: u64 = 1;
pub const CAJA: u64 = 2;
pub const SOLTAR: u64 = 3;
/// Solo lectura: `tandas | armado << 32`.
pub const COMO_VA: u64 = 4;

/// La suborden de un argumento.
pub const fn suborden(arg: u64) -> u64 {
    arg >> 60
}

/// `ARMAR` con la VA del lienzo (47 bits de usuario).
pub const fn armar(lienzo: u64) -> u64 {
    ARMAR << 60 | (lienzo & ((1 << 47) - 1))
}

/// La VA del lienzo de un `ARMAR`.
pub const fn lienzo_de(arg: u64) -> u64 {
    arg & ((1 << 47) - 1)
}

/// Una CAJA sucia `(x, y, ancho, alto)`, 13 bits cada una, y si es la
/// ULTIMA del fotograma (la que cierra la tanda y toca el timbre).
pub const fn caja(x: u32, y: u32, w: u32, h: u32, ultima: bool) -> u64 {
    let m = 0x1FFF;
    CAJA << 60 | (ultima as u64) << 52 | (h as u64 & m) << 39 | (w as u64 & m) << 26 | (y as u64 & m) << 13 | (x as u64 & m)
}

/// `(x, y, ancho, alto, ultima)` de una CAJA.
pub const fn caja_de(arg: u64) -> (u32, u32, u32, u32, bool) {
    let m = 0x1FFF;
    ((arg & m) as u32, (arg >> 13 & m) as u32, (arg >> 26 & m) as u32, (arg >> 39 & m) as u32, arg >> 52 & 1 != 0)
}

/// Lo mas que lleva una tanda (el escritorio parte lo sucio en 8 como mucho).
pub const MAX_CAJAS: usize = 16;
/// SET_OBJECT, 11 por caja y el semaforo de la ultima.
pub const PALABRAS_TANDA: usize = 2 + 11 * MAX_CAJAS + 4;

/// `LAUNCH_DMA` de una caja que NO es la ultima: como [`LANZAR`] pero sin
/// semaforo (el tipo, bits 4..3, a 0).
pub const LANZAR_SIN_SEMAFORO: u32 = (LANZAR & !(3 << 3)) | MULTI_LINE;

/// **Una tanda**: las ordenes de las cajas de UN fotograma.
pub struct Tanda {
    pub w: [u32; PALABRAS_TANDA],
    pub n: usize,
    pub cajas: usize,
}

impl Tanda {
    pub const fn nueva() -> Self {
        let mut w = [0u32; PALABRAS_TANDA];
        w[0] = cabecera(SET_OBJECT, 1);
        w[1] = AMPERE_DMA_COPY_B;
        Tanda { w, n: 2, cajas: 0 }
    }

    /// **Una caja mas**, del lienzo al MISMO sitio de la pantalla. `false`
    /// si se sale de la pantalla, esta vacia o ya no cabe (no se agrega).
    pub fn caja(&mut self, p: &Pantalla, x: u32, y: u32, w: u32, h: u32) -> bool {
        if w == 0 || h == 0 || x + w > p.ancho || y + h > p.alto || self.cajas >= MAX_CAJAS {
            return false;
        }
        let off = 4 * (y as u64 * p.pitch as u64 + x as u64);
        let (o, d) = (VA + off, crate::pantalla::VA + off);
        let paso = p.pitch * 4;
        let c = [
            cabecera(OFFSET_IN_UPPER, 8),
            (o >> 32) as u32,
            o as u32,
            (d >> 32) as u32,
            d as u32,
            paso,
            paso,
            w * 4,
            h,
            cabecera(LAUNCH_DMA, 1),
            LANZAR_SIN_SEMAFORO,
        ];
        self.w[self.n..self.n + c.len()].copy_from_slice(&c);
        self.n += c.len();
        self.cajas += 1;
        true
    }

    /// **Cerrar la tanda**: la ultima caja paga el semaforo con `numero` (el
    /// SET_SEMAPHORE va DELANTE de su LAUNCH_DMA, y ese LAUNCH lo suelta).
    /// `false` si no hay ninguna caja.
    pub fn cerrar(&mut self, numero: u32) -> bool {
        if self.cajas == 0 {
            return false;
        }
        let s = va(SEMAFORO);
        // La ultima caja acaba en [LAUNCH_DMA, flags]: el semaforo se mete
        // delante y su LAUNCH pasa a soltarlo.
        let launch = self.n - 2;
        let sem = [cabecera(SET_SEMAPHORE_A, 3), (s >> 32) as u32, s as u32, numero];
        self.w.copy_within(launch..self.n, launch + sem.len());
        self.w[launch..launch + sem.len()].copy_from_slice(&sem);
        self.n += sem.len();
        self.w[self.n - 1] = LANZAR | MULTI_LINE;
        true
    }
}

/// Palabras seguidas por la ventana, SIN releerlas (ver arriba). La ventana
/// queda como estaba.
fn escribir_sin_releer<R: Registros>(r: &mut R, dir: u64, p: &[u32]) {
    let (base, off) = crate::vram::ventana(dir);
    let antes = r.leer(crate::vram::VENTANA_REG);
    r.escribir(crate::vram::VENTANA_REG, base);
    for (k, &v) in p.iter().enumerate() {
        r.escribir(crate::vram::VENTANA + off + 4 * k as u32, v);
    }
    r.escribir(crate::vram::VENTANA_REG, antes);
}

/// **Enviar una tanda cerrada** por la entrada `e`: el semaforo a 0, las
/// ordenes, la entrada, GP_PUT a la siguiente y el timbre. Sin invalidar la
/// MMU: el mapa no cambia de un fotograma a otro (se invalida al ARMAR).
pub fn enviar<R: Registros>(r: &mut R, t: &Tanda, e: u32, ficha: u32) -> bool {
    if t.cajas == 0 || e >= GPFIFO_ENTRADAS || t.n > 512 {
        return false;
    }
    let en = entrada(va(EMPUJE), t.n as u32);
    escribir_sin_releer(r, SEMAFORO, &[0]);
    escribir_sin_releer(r, EMPUJE, &t.w[..t.n]);
    escribir_sin_releer(r, GPFIFO + 8 * e as u64, &[en as u32, (en >> 32) as u32]);
    escribir_sin_releer(r, USERD + GP_PUT, &[siguiente(e)]);
    r.escribir(TIMBRE, ficha);
    true
}

/// La 3060 ya pago el semaforo de la tanda `numero`.
pub fn pagada<R: Registros>(r: &mut R, numero: u32) -> bool {
    leer32(r, SEMAFORO) == numero
}

/// La invalidacion de la MMU que hace falta UNA vez al armar.
pub fn invalidar_mmu<R: Registros>(r: &mut R) -> bool {
    invalidar(r)
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
    fn una_tanda_de_cajas() {
        assert_eq!(caja_de(caja(10, 20, 300, 40, true)), (10, 20, 300, 40, true));
        assert_eq!(caja_de(caja(1919, 1079, 1, 1, false)), (1919, 1079, 1, 1, false));
        assert_eq!(suborden(caja(0, 0, 1, 1, false)), CAJA);
        assert_eq!(lienzo_de(armar(0x4000_1000)), 0x4000_1000);
        assert_eq!(suborden(armar(0x4000_1000)), ARMAR);
        let mut t = Tanda::nueva();
        assert!(!t.cerrar(1), "sin cajas no se cierra");
        assert!(t.caja(&FHD, 100, 50, 200, 30));
        assert!(t.caja(&FHD, 0, 0, 1920, 1080));
        assert!(!t.caja(&FHD, 1900, 0, 30, 1), "se sale por la derecha");
        assert!(!t.caja(&FHD, 0, 0, 0, 5), "vacia");
        assert!(t.cerrar(7));
        assert_eq!(t.n, 2 + 11 + 11 + 4);
        // La primera caja: su origen y su destino, en el mismo sitio.
        let off = 4 * (50 * 1920 + 100) as u64;
        assert_eq!((t.w[3] as u64) << 32 | t.w[4] as u64, VA + off);
        assert_eq!((t.w[5] as u64) << 32 | t.w[6] as u64, crate::pantalla::VA + off);
        assert_eq!((t.w[9], t.w[10]), (800, 30));
        assert_eq!(t.w[12], LANZAR_SIN_SEMAFORO);
        assert_eq!(LANZAR_SIN_SEMAFORO >> 3 & 3, 0, "sin semaforo");
        // La ultima: el semaforo DELANTE de su LAUNCH, con el numero, y el
        // LAUNCH lo suelta.
        assert_eq!(t.w[t.n - 6], cabecera_en(4, SET_SEMAPHORE_A, 3));
        assert_eq!(t.w[t.n - 3], 7);
        assert_eq!(t.w[t.n - 2], cabecera_en(4, LAUNCH_DMA, 1));
        assert_eq!(t.w[t.n - 1], LANZAR | MULTI_LINE);
        assert!(PALABRAS_TANDA <= 512, "cabe en la media pagina de ordenes");
        let mut llena = Tanda::nueva();
        for k in 0..MAX_CAJAS as u32 {
            assert!(llena.caja(&FHD, k, 0, 1, 1));
        }
        assert!(!llena.caja(&FHD, 99, 0, 1, 1), "no cabe una mas");
        assert!(llena.cerrar(1) && llena.n <= PALABRAS_TANDA);
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
