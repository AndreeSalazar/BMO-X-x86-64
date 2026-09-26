//! **D2a: UNA IMAGEN DE 32 BITS POR LA 3060** -- la hermana de `video` para
//! lo que ya viene en color: un fotograma `0x00RRGGBB` (el `DG_ScreenBuffer`
//! de DOOM, 320 x 200) se agranda por un entero y se escribe DIRECTAMENTE en
//! el framebuffer del GOP, por un programa de computo. Es el carril D de
//! `docs/plan/PLAN_VERRANO.md`: que agrandar DOOM deje de ser trabajo de la
//! CPU (su propia medida: a x5, ~4,4 ms y 6,4 MB por fotograma).
//!
//! capa: puro -- el programa, el encaje, los parametros y la cuenta de
//! referencia; la RAM, la IOMMU y los registros los toca el kernel (L8)
//!
//! [eje]     RENDIMIENTO -- la CPU deja de expandir y copiar: por el PCIe
//!           viaja el fotograma de DOOM (256 KB) y no el agrandado (hasta
//!           6,4 MB); cada pixel de origen se lee UNA vez
//!
//! # Lo mismo que `video`, a proposito
//!
//! El ORIGEN se presta y se ve donde el de `video` ([`crate::video::VA`],
//! [`crate::video::IOVA`], su mismo mapa): un fotograma a la vez, del mismo
//! modo. Los 9 parametros van en el MISMO orden (origen, medidas, esquina del
//! destino, paso, escala, rgb) y el encaje es el mismo (la escala entera mas
//! grande que cabe, centrado). Lo unico distinto es el formato y el programa.
//!
//! # El programa (`ptxas -arch=sm_86`, CUDA 12.9: 15 registros, 46 instrucciones)
//!
//! Un hilo por pixel del ORIGEN: lo lee, le quita el alfa, cambia rojo y azul
//! si el GOP es RGB (`prmt 0x3012`), y escribe su cuadrado de s x s. Como
//! todos: `c[0x0][0x28]` y `ULDC.64 UR6, c[0x0][0x118]` en NOP con sus bits de
//! planificacion, y los LDG/STG sin descriptor. El PTX de origen:
//!
//! ```text
//! .version 7.1
//! .target sm_86
//! .address_size 64
//! .visible .entry img()
//! {
//!   .reg .b32 %r<40>;
//!   .reg .b64 %rd<20>;
//!   .reg .pred %p<8>;
//!   // los 9 parametros, en VA 0x2_0000_E5C0 (el mismo orden que `video`)
//!   mov.b64 %rd10, {58816, 2};
//!   ld.volatile.global.u32 %r1, [%rd10];
//!   ld.volatile.global.u32 %r2, [%rd10+4];
//!   ld.volatile.global.u32 %r3, [%rd10+8];
//!   ld.volatile.global.u32 %r4, [%rd10+12];
//!   ld.volatile.global.u32 %r5, [%rd10+16];
//!   ld.volatile.global.u32 %r6, [%rd10+20];
//!   ld.volatile.global.u32 %r7, [%rd10+24];
//!   ld.volatile.global.u32 %r8, [%rd10+28];
//!   ld.volatile.global.u32 %r9, [%rd10+32];
//!   // un hilo por pixel del ORIGEN: se lee UNA vez por el PCIe
//!   mov.u32 %r10, %tid.x;
//!   mov.u32 %r11, %ctaid.x;
//!   mad.lo.u32 %r12, %r11, 256, %r10;
//!   setp.ge.u32 %p1, %r12, %r3;
//!   @%p1 bra FIN;
//!   mov.u32 %r13, %ctaid.y;
//!   mad.lo.u32 %r14, %r13, %r3, %r12;
//!   mov.b64 %rd1, {%r1, %r2};
//!   mul.wide.u32 %rd2, %r14, 4;
//!   add.s64 %rd3, %rd1, %rd2;
//!   ld.global.u32 %r15, [%rd3];
//!   // 0x00RRGGBB, y rojo y azul cambiados si el GOP es RGB
//!   and.b32 %r15, %r15, 16777215;
//!   setp.ne.u32 %p2, %r9, 0;
//!   @%p2 prmt.b32 %r15, %r15, 0, 12306;
//!   // el cuadrado de s x s: esquina (x s, y s) desde la del destino
//!   mul.lo.u32 %r16, %r13, %r8;
//!   mul.lo.u32 %r17, %r12, %r8;
//!   mad.lo.u32 %r18, %r16, %r7, %r17;
//!   mov.b64 %rd4, {%r5, %r6};
//!   mul.wide.u32 %rd5, %r18, 4;
//!   add.s64 %rd6, %rd4, %rd5;
//!   mul.wide.u32 %rd8, %r7, 4;
//!   mov.u32 %r20, 0;
//! FILA:
//!   .pragma "nounroll";
//!   mov.u32 %r21, 0;
//!   mov.b64 %rd7, %rd6;
//! COL:
//!   .pragma "nounroll";
//!   st.global.u32 [%rd7], %r15;
//!   add.s64 %rd7, %rd7, 4;
//!   add.u32 %r21, %r21, 1;
//!   setp.lt.u32 %p3, %r21, %r8;
//!   @%p3 bra COL;
//!   add.s64 %rd6, %rd6, %rd8;
//!   add.u32 %r20, %r20, 1;
//!   setp.lt.u32 %p4, %r20, %r8;
//!   @%p4 bra FILA;
//! FIN:
//!   ret;
//! }
//! ```
//!
//! Se FABRICO en la nube el 26-09, con el `ptxas` 12.9 y el `nvdisasm` 13.4
//! de PyPI (los mismos de M5d S4). La cadena se valido antes rehaciendo el
//! primer sombreador desde su PTX: 8 de 10 instrucciones bit a bit iguales, y
//! las 2 restantes son las dos que el se cambiaron por NOP.
//!
//! # Como se sabe
//!
//! Aqui: el juez del SASS lo da por bueno, y la cuenta de referencia es la
//! de DOOM (`escalado_de_doom.rs`: el pixel `(x, y)` de una escala `s` es el
//! `(x / s, y / s)` del origen). En el metal: la CPU rehace [`MUESTRAS`]
//! pixeles de la pantalla desde el MISMO origen y los lee del framebuffer.

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::pantalla::Pantalla;
use crate::sombreador::{ordenes_con, qmd_rejilla, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::Registros;

pub use crate::video::{Encaje, IOVA, VA};

pub const HILOS: u32 = 256;
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_1A10;
pub const PAGA_FIN: u32 = 0x3060_1AF0;
/// Por encima de las marcas de VERRANO (hasta +0x200) y por debajo de los
/// escalones de `raster` (+0x300).
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x200;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x210;
/// Donde lee el programa sus 9 parametros (VA 0x2_0000_E5C0): tras los de
/// `video` y antes de la tabla de `tuberia`.
pub const PARAMETROS: u64 = SEMAFOROS + 0x5C0;
pub const N_PARAMETROS: usize = 9;
/// Pixeles de la pantalla que la CPU comprueba por fotograma (16 x 16).
pub const MUESTRAS: u32 = 256;

/// **El formato** de una imagen de 32 bits por pixel, sin relleno.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Formato {
    pub ancho: u32,
    pub alto: u32,
}

impl Formato {
    /// DOOM: `DOOMGENERIC_RESX` x `DOOMGENERIC_RESY` en `doomgeneric_bmo.c`.
    pub const DOOM: Formato = Formato { ancho: 320, alto: 200 };

    pub const fn bytes(&self) -> u64 {
        self.ancho as u64 * self.alto as u64 * 4
    }

    /// Que no este vacia y quepa en el mapa del origen de `video`.
    pub const fn valido(&self) -> bool {
        self.ancho >= 1 && self.alto >= 1 && self.ancho <= 4096 && self.alto <= 0xFFFF && self.bytes() <= crate::video::MAX_BYTES
    }

    /// Bloques de [`HILOS`] por linea del origen.
    pub const fn bloques(&self) -> u32 {
        self.ancho.div_ceil(HILOS)
    }
}

/// **Como cae en la pantalla**: la escala entera mas grande que cabe y la
/// esquina para centrarla (la misma cuenta que `video::encaje`). `None` si
/// el formato no vale o la imagen es MAS GRANDE que la pantalla.
pub const fn encaje(f: &Formato, p: &Pantalla) -> Option<Encaje> {
    if !f.valido() {
        return None;
    }
    let (sx, sy) = (p.ancho / f.ancho, p.alto / f.alto);
    let escala = if sx < sy { sx } else { sy };
    if escala == 0 {
        return None;
    }
    Some(Encaje { escala, x0: (p.ancho - f.ancho * escala) / 2, y0: (p.alto - f.alto * escala) / 2 })
}

/// Los 9 parametros, en el orden de `video`: el origen, sus medidas, la
/// esquina del destino (ya con `x0, y0`), el paso de la pantalla, la escala
/// y si el GOP es RGB.
pub fn parametros(f: &Formato, e: &Encaje, p: &Pantalla) -> [u32; N_PARAMETROS] {
    let d = crate::pantalla::VA + 4 * (e.y0 as u64 * p.pitch as u64 + e.x0 as u64);
    [VA as u32, (VA >> 32) as u32, f.ancho, f.alto, d as u32, (d >> 32) as u32, p.pitch, e.escala, p.rgb as u32]
}

/// El color que escribe el programa: sin el alfa, y rojo y azul cambiados si
/// el GOP es RGB.
pub const fn color(v: u32, rgb: bool) -> u32 {
    let v = v & 0x00FF_FFFF;
    if rgb {
        (v & 0x00_FF00) | (v >> 16) | (v & 0xFF) << 16
    } else {
        v
    }
}

/// Lo que tiene que haber en `(dx, dy)` DENTRO del rectangulo de la imagen
/// (sin `x0, y0`): el `(dx / s, dy / s)` del origen, como el agrandado de DOOM.
pub fn pixel(origen: &[u32], f: &Formato, e: &Encaje, rgb: bool, dx: u32, dy: u32) -> u32 {
    let (x, y) = (dx / e.escala, dy / e.escala);
    color(origen[(y * f.ancho + x) as usize], rgb)
}

/// La muestra `k` (de [`MUESTRAS`]): una rejilla de 16 x 16 dentro del
/// rectangulo, de borde a borde. `(dx, dy)` sin la esquina.
pub const fn muestra(f: &Formato, e: &Encaje, k: u32) -> (u32, u32) {
    let (w, h) = (f.ancho * e.escala, f.alto * e.escala);
    let (i, j) = (k % 16, k / 16);
    ((i * (w - 1)) / 15, (j * (h - 1)) / 15)
}

/// **El programa**, como lo leyo `nvdisasm` (el PTX de origen, arriba).
pub const CODIGO: [(u64, u64); 46] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x00000000000b7919, 0x000e220000002100), // S2R R11, SR_TID.X
    (0x0000000000067802, 0x000fe20000000f00), // MOV R6, 0x0
    (0x00000002ff077424, 0x000fe200078e00ff), // IMAD.MOV.U32 R7, RZ, RZ, 0x2
    (0x0000000000007918, 0x000fe20000000000), // NOP (era ULDC.64 UR6, c[0x0][0x118])
    (0x0000000000047919, 0x000e280000002500), // S2R R4, SR_CTAID.X
    (0x00e5c00606027981, 0x000368000c1f5900), // LDG.E.STRONG.SYS R2, [R6.64+0xe5c0]
    (0x00e5c40606037981, 0x000368000c1f5900), // LDG.E.STRONG.SYS R3, [R6.64+0xe5c4]
    (0x00e5c80606007981, 0x000ea8000c1f5900), // LDG.E.STRONG.SYS R0, [R6.64+0xe5c8]
    (0x00e5cc06060c7981, 0x000362000c1f5900), // LDG.E.STRONG.SYS R12, [R6.64+0xe5cc]
    (0x0000000b040b7211, 0x001fc600078e40ff), // LEA R11, R4, R11, 0x8
    (0x00e5d00606047981, 0x000368000c1f5900), // LDG.E.STRONG.SYS R4, [R6.64+0xe5d0]
    (0x00e5d40606057981, 0x000368000c1f5900), // LDG.E.STRONG.SYS R5, [R6.64+0xe5d4]
    (0x00e5d80606097981, 0x000368000c1f5900), // LDG.E.STRONG.SYS R9, [R6.64+0xe5d8]
    (0x00e5dc0606087981, 0x000368000c1f5900), // LDG.E.STRONG.SYS R8, [R6.64+0xe5dc]
    (0x00e5e006060a7981, 0x000362000c1f5900), // LDG.E.STRONG.SYS R10, [R6.64+0xe5e0]
    (0x000000000b00720c, 0x004fda0003f06070), // ISETP.GE.U32.AND P0, PT, R11, R0, PT
    (0x000000000000094d, 0x000fea0003800000), // @P0 EXIT
    (0x00000000000479c3, 0x002e240000002600), // S2UR UR4, SR_CTAID.Y
    (0x0000000400077c24, 0x001fc8000f8e020b), // IMAD R7, R0, UR4, R11
    (0x0000000407027825, 0x020fcc00078e0002), // IMAD.WIDE.U32 R2, R7, 0x4, R2
    (0x0000000602027981, 0x000ea2000c1e1900), // LDG.E R2, [R2.64]
    (0x000000ff0a00720c, 0x000fe20003f05070), // ISETP.NE.U32.AND P0, PT, R10, RZ, PT
    (0x00000004090b7c24, 0x000fe2000f8e020b), // IMAD R11, R9, UR4, R11
    (0x0000003f00047c82, 0x000fc60008000000), // UMOV UR4, URZ
    (0x0000000b080b7224, 0x000fc800078e02ff), // IMAD R11, R8, R11, RZ
    (0x000000040b047825, 0x000fe200078e0004), // IMAD.WIDE.U32 R4, R11, 0x4, R4
    (0x00ffffff02077812, 0x004fc800078ec0ff), // LOP3.LUT R7, R2, 0xffffff, RZ, 0xc0, !PT
    (0x0000301207070816, 0x000fe400000000ff), // @P0 PRMT R7, R7, 0x3012, RZ
    (0x0000000104047890, 0x000fe2000fffe03f), // UIADD3 UR4, UR4, 0x1, URZ
    (0x000000ffff007224, 0x000fe200078e0004), // IMAD.MOV.U32 R0, RZ, RZ, R4
    (0x00000005000b7202, 0x000fe20000000f00), // MOV R11, R5
    (0x0000003f00057c82, 0x000fc60008000000), // UMOV UR5, URZ
    (0x0000000408007c0c, 0x000fe4000bf43070), // ISETP.LE.U32.AND P2, PT, R8, UR4, PT
    (0x000000ffff027224, 0x000fe200078e0000), // IMAD.MOV.U32 R2, RZ, RZ, R0
    (0x0000000b00037202, 0x000fe20000000f00), // MOV R3, R11
    (0x0000000105057890, 0x000fe2000fffe03f), // UIADD3 UR5, UR5, 0x1, URZ
    (0x0000000400007810, 0x000fc60007f3e0ff), // IADD3 R0, P1, R0, 0x4, RZ
    (0x0000000702007986, 0x0001e4000c101906), // STG.E [R2.64], R7
    (0x0000000508007c0c, 0x000fe2000bf03070), // ISETP.LE.U32.AND P0, PT, R8, UR5, PT
    (0x000000ffff0b7224, 0x000fd800008e060b), // IMAD.X R11, RZ, RZ, R11, P1
    (0xffffff8000008947, 0x001fea000383ffff), // @!P0 BRA 0x220
    (0x0000000409047825, 0x000fe200078e0004), // IMAD.WIDE.U32 R4, R9, 0x4, R4
    (0xffffff100000a947, 0x000fea000383ffff), // @!P2 BRA 0x1d0
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA 0x2d0
];

pub const PALABRAS_CODIGO: usize = CODIGO.len() * 4;

pub fn codigo() -> [u32; PALABRAS_CODIGO] {
    let mut w = [0u32; PALABRAS_CODIGO];
    for (k, &(lo, hi)) in CODIGO.iter().enumerate() {
        w[4 * k] = lo as u32;
        w[4 * k + 1] = (lo >> 32) as u32;
        w[4 * k + 2] = hi as u32;
        w[4 * k + 3] = (hi >> 32) as u32;
    }
    w
}

/// Un hilo por pixel del origen: `bloques` x `alto` bloques de 256.
pub fn qmd(f: &Formato) -> [u32; QMD_PALABRAS] {
    qmd_rejilla(sombreador_va(PROGRAMA), HILOS, f.bloques(), f.alto, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

/// **Preparar el fotograma** con la entrada `e` del GPFIFO de GR, como
/// `video::preparar`: `todo` carga tambien el programa, el QMD y las ordenes.
pub fn preparar<R: Registros>(r: &mut R, e: u32, f: &Formato, parametros: &[u32; N_PARAMETROS], todo: bool) -> bool {
    if !crate::blur::entrada_valida(e) || !f.valido() {
        return false;
    }
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let base = escribir(r, SEMAFORO_QMD, &[0; 8]) == 8 && escribir(r, PARAMETROS, parametros) == N_PARAMETROS;
    let cargado = !todo || {
        let (c, q, o) = (codigo(), qmd(f), ordenes());
        escribir(r, PROGRAMA, &c) == PALABRAS_CODIGO && escribir(r, QMD, &q) == QMD_PALABRAS && escribir(r, EMPUJE, &o) == ORDENES
    };
    base && cargado && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = crate::blur::entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[crate::blur::siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

pub use crate::fractal::{desempaquetar, empaquetar};

/// Sano: los dos semaforos pagados y las muestras iguales.
pub const fn sano(v: u64) -> bool {
    let (buenos, qmd, fin, lanzado, _, _) = desempaquetar(v);
    buenos == MUESTRAS && qmd && fin && lanzado
}

const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);
const _: () = assert!(PARAMETROS >= crate::video::PARAMETROS + 4 * crate::video::N_PARAMETROS as u64);
const _: () = assert!(PARAMETROS + 4 * N_PARAMETROS as u64 <= crate::tuberia::TABLA);
const _: () = assert!(SEMAFORO_QMD >= crate::anillo::MARCAS + 0x80 && SEMAFORO_FIN + 16 <= crate::raster::ESCALONES);
const _: () = assert!(crate::lienzo::sombreador_va(PARAMETROS) == 0x2_0000_E5C0);
const _: () = assert!(Formato::DOOM.bytes() <= crate::video::MAX_BYTES);

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    const FHD: Pantalla = Pantalla { vram: 0, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };

    #[test]
    fn doom_cae_x5_centrado_en_1920x1080() {
        assert_eq!(encaje(&Formato::DOOM, &FHD), Some(Encaje { escala: 5, x0: 160, y0: 40 }));
        let ventana = Pantalla { vram: 0, pitch: 960, ancho: 960, alto: 600, rgb: false };
        assert_eq!(encaje(&Formato::DOOM, &ventana), Some(Encaje { escala: 3, x0: 0, y0: 0 }), "la ventana de DOOM: x3");
        assert_eq!(encaje(&Formato { ancho: 3840, alto: 2160 }, &FHD), None, "mas grande que la pantalla");
        assert_eq!(encaje(&Formato { ancho: 0, alto: 200 }, &FHD), None);
    }

    /// La referencia ES el agrandado de DOOM por la CPU: el pixel `(x, y)` de
    /// escala `s` es el `(x / s, y / s)` del origen, cada uno repetido s x s.
    #[test]
    fn el_pixel_es_el_agrandado_de_doom() {
        let f = Formato::DOOM;
        let origen: Vec<u32> = (0..f.ancho * f.alto).map(|k| 0xFF00_0000 | k.wrapping_mul(2_654_435_761) >> 8).collect();
        for s in 1..=5 {
            let e = Encaje { escala: s, x0: 0, y0: 0 };
            for (dx, dy) in [(0, 0), (s - 1, s - 1), (s, 0), (319 * s + s - 1, 199 * s), (123, 77)] {
                let esperado = origen[((dy / s) * 320 + dx / s) as usize] & 0x00FF_FFFF;
                assert_eq!(pixel(&origen, &f, &e, false, dx, dy), esperado, "x{s} en ({dx}, {dy})");
            }
        }
    }

    #[test]
    fn el_color_quita_el_alfa_y_cambia_rojo_y_azul_en_un_gop_rgb() {
        assert_eq!(color(0xFF12_3456, false), 0x0012_3456);
        assert_eq!(color(0xFF12_3456, true), 0x0056_3412);
        assert_eq!(color(color(0x0012_3456, true), true), 0x0012_3456, "dos veces, lo mismo");
    }

    #[test]
    fn los_parametros_son_los_de_video_con_la_esquina() {
        let f = Formato::DOOM;
        let e = encaje(&f, &FHD).unwrap();
        let p = parametros(&f, &e, &FHD);
        assert_eq!((p[0] as u64) | (p[1] as u64) << 32, crate::video::VA);
        assert_eq!((p[4] as u64) | (p[5] as u64) << 32, crate::pantalla::VA + 4 * (40 * 1920 + 160));
        assert_eq!(&p[2..4], &[320, 200]);
        assert_eq!(&p[6..], &[1920, 5, 0]);
    }

    #[test]
    fn las_muestras_tocan_las_esquinas_de_la_imagen() {
        let e = encaje(&Formato::DOOM, &FHD).unwrap();
        assert_eq!(muestra(&Formato::DOOM, &e, 0), (0, 0));
        assert_eq!(muestra(&Formato::DOOM, &e, MUESTRAS - 1), (1599, 999));
    }

    #[test]
    fn el_programa_lee_sus_parametros_y_un_pixel_y_escribe_su_cuadrado() {
        let fuertes: Vec<u64> = CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x981 && c.1 & 0xFFFF == 0x5900).map(|c| c.0 >> 40 & 0xFFFF).collect();
        assert_eq!(fuertes, (0..9).map(|k| 0xE5C0 + 4 * k).collect::<Vec<u64>>(), "los 9 parametros, en su sitio");
        assert_eq!(CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x981).count(), 10, "9 parametros y UNA lectura del origen");
        assert!(CODIGO.iter().filter(|c| matches!(c.0 & 0xFFF, 0x981 | 0x986)).all(|c| c.1 >> 37 & 1 == 0), "sin descriptor");
        let nops: Vec<usize> = CODIGO.iter().enumerate().filter(|(_, c)| c.0 == 0x7918).map(|(k, _)| k).collect();
        assert_eq!(nops.len(), 2, "las dos lecturas de CUDA, en NOP, y ningun otro");
        assert!(nops.iter().all(|&k| k < 8 && CODIGO[k].1 & ((1 << 41) - 1) == 0), "al principio, con solo sus bits de planificacion");
        assert_eq!(CODIGO[CODIGO.len() - 2].0 & 0xFFF, 0x94D, "acaba en EXIT");
    }

    #[test]
    fn el_juez_del_sass_lo_da_por_bueno() {
        let ctx = crate::sass::juez::Contexto { registros: REGISTROS, sph: None };
        let v = crate::sass::juez::juzgar(&CODIGO, &ctx).unwrap_or_else(|b| panic!("TOMA TU BODRIO: {b:?}"));
        // Hasta el EXIT incluido: el BRA a si mismo de detras no se ejecuta.
        assert_eq!(v.instrucciones, CODIGO.len() - 1);
    }
}
