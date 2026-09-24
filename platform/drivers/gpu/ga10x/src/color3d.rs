//! **M5 T2a: EL TRIANGULO CON COLOR, MEZCLADO POR EL RASTERIZADOR** -- el
//! mismo dibujo que T1c, pero cada vertice lleva su color (rojo arriba, verde
//! abajo a la derecha, azul abajo a la izquierda) y la 3060 lo MEZCLA en cada
//! pixel: el rasterizador saca los pesos del pixel dentro del triangulo y el
//! programa de pixel los lee con `IPA`. Es el triangulo de T0, ahora por el
//! hardware de triangulos en vez de por computo.
//!
//! capa: puro -- los programas, sus cabeceras y el juez; el estado 3D y el
//! dibujo son los de `raster` (L8)
//!
//! [eje]     CORRECCION -- cada instruccion desensamblada con `nvdisasm -b
//!           SM86`; el juez, entero
//!
//! # Los programas
//!
//! ```text
//!    vertice  el de T1c + el color por vertice:
//!             AST.128 a[0x80], RZ, R8      el vector generico 0 (r, g, b, a)
//!             AST.128 a[0x70], RZ, R4      la posicion
//!    pixel    MOV R3, 1.0                  alfa
//!             IPA.PASS R0..R2, a[0x80..0x88]   rojo, verde y azul, mezclados
//!             EXIT                         (espera la barrera 0 de los IPA)
//!
//!    IPA  0x326  destino 16..24, atributo/4 64..74, predicado de salida
//!                81..84 (7 = ninguno), modo 78..79 (0 PASS)
//! ```
//!
//! En la cabecera del de pixel, el vector 0 en `ScreenLinear` (3): la mezcla
//! lineal EN PANTALLA, la misma que hace la CPU con las aristas. (Con w = 1
//! en los tres vertices, la de perspectiva daria lo mismo.)
//!
//! # El juez
//!
//! Dentro: el peso de cada vertice en el CENTRO del pixel, `w_i / A`, por 255
//! y redondeado; la 3060 lo cuenta en coma flotante y su ROP redondea al
//! pasar a 8 bits, asi que se acepta 1 de diferencia por canal (ni una mas).
//! Fuera: el magenta de la limpieza, exacto.

use crate::fractal::{LADO, PIXELES};
use crate::raster::{self as ra, arista_centro, bit, programa, PIXEL, SPH, VERTICE};
use crate::sombreador::SEMAFOROS;
use crate::triangulo::{V0, V1, V2};
use crate::tresde::PIXEL_LIMPIO;
use crate::Registros;

pub const PAGA_FIN: u32 = 0x3060_7C0A;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0xE0;

/// **El programa de VERTICE** (23 instrucciones, SASS de `ptxas` con los dos
/// STG cambiados por AST y el S2R por ALD).
pub const CODIGO_VS: [(u64, u64); 23] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP
    (0x0002fcffff007321, 0x000e220000000000), // ALD R0, a[0x2fc]
    (0x3f380000ff057424, 0x000fe200078e00ff), // MOV R5, 0.71875
    (0x0000000000007918, 0x000fe20000000000), // NOP
    (0x0000000000007918, 0x000fe40000000000), // NOP
    (0x0000000000007918, 0x000fe40000000000), // NOP
    (0x3f800000ff077424, 0x000fe400078e00ff), // MOV R7, 1.0 (w)
    (0x000000ffff067224, 0x000fe400078e00ff), // MOV R6, RZ (z)
    (0x3f800000ff0b7424, 0x000fe200078e00ff), // MOV R11, 1.0 (alfa)
    (0x000000010000780c, 0x001fc40003f05070), // ISETP.NE.U32.AND P0, PT, R0, 0x1, PT
    (0x000000020000780c, 0x040fe40003f25070), // ISETP.NE.U32.AND P1, PT, R0, 0x2, PT
    (0x000000ff0000720c, 0x000fe40003f45070), // ISETP.NE.U32.AND P2, PT, R0, RZ, PT
    (0xbf58000005057807, 0x000fe40004000000), // SEL R5, R5, -0.84375, !P0
    (0x3f580000ff007807, 0x000fe40000000000), // SEL R0, RZ, 0.84375, P0
    (0x3f800000ff087807, 0x000fe40001000000), // SEL R8, RZ, 1.0, P2 (rojo: v0)
    (0x3f800000ff097807, 0x000fc40000000000), // SEL R9, RZ, 1.0, P0 (verde: v1)
    (0x3f800000ff0a7807, 0x000fe40000800000), // SEL R10, RZ, 1.0, P1 (azul: v2)
    (0x3f38000005057807, 0x000fe40000800000), // SEL R5, R5, 0.71875, P1
    (0xbf58000000047807, 0x000fe20000800000), // SEL R4, R0, -0.84375, P1
    (0x000080ffff007322, 0x000fe80000000c08), // AST.128 a[0x80], RZ, R8
    (0x000070ffff007322, 0x000fe20000000c04), // AST.128 a[0x70], RZ, R4
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA .
];

/// **El programa de PIXEL** (6 instrucciones).
pub const CODIGO_PS: [(u64, u64); 6] = [
    (0x3f800000ff037424, 0x000fe200078e00ff), // MOV R3, 1.0 (alfa)
    (0x000000ffff007326, 0x000e2200000e0020), // IPA.PASS R0, a[0x80] (barrera 0)
    (0x000000ffff017326, 0x000e2200000e0021), // IPA.PASS R1, a[0x84] (barrera 0)
    (0x000000ffff027326, 0x000e2800000e0022), // IPA.PASS R2, a[0x88] (barrera 0)
    (0x000000000000794d, 0x001fea0003800000), // EXIT, esperando la barrera 0
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA .
];

/// `PixelImap::ScreenLinear`.
pub const LINEAL: u32 = 3;

/// **La SPH del de vertice**: la de T1c y ademas `OmapGenericVector[0]`
/// entero (bits 432..435).
pub const fn sph_vertice() -> [u32; SPH] {
    let mut h = ra::sph_vertice();
    let mut k = 432;
    while k < 436 {
        h = bit(h, k);
        k += 1;
    }
    h
}

/// **La SPH del de pixel**: la de T1c y ademas `ImapGenericVector[0]` X, Y y
/// Z en ScreenLinear (2 bits cada uno desde el 192: la palabra 6).
pub const fn sph_pixel() -> [u32; SPH] {
    let mut h = ra::sph_pixel();
    h[6] |= LINEAL | LINEAL << 2 | LINEAL << 4;
    h
}

pub const PALABRAS_VS: usize = SPH + CODIGO_VS.len() * 4;
pub const PALABRAS_PS: usize = SPH + CODIGO_PS.len() * 4;

pub const fn vertice() -> [u32; PALABRAS_VS] {
    programa(sph_vertice(), &CODIGO_VS)
}

pub const fn pixel() -> [u32; PALABRAS_PS] {
    programa(sph_pixel(), &CODIGO_PS)
}

/// El estado 3D, los programas y el dibujo: los de `raster`, con estos dos
/// programas y este semaforo.
pub fn preparar<R: Registros>(r: &mut R, e: u32) -> bool {
    ra::preparar_con(r, e, &vertice(), &pixel(), SEMAFORO_FIN, PAGA_FIN)
}

pub use crate::raster::lanzar;

/// `(GP_GET, semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    ra::mirar_en(r, SEMAFORO_FIN)
}

// == El juez =================================================================

/// El area doble con las coordenadas al doble (como `arista_centro`).
const AREA4: i64 = 4 * crate::triangulo::AREA as i64;

/// Un canal: `255 * w / A`, redondeado.
const fn canal(w: i32) -> u32 {
    ((255 * 2 * w as i64 + AREA4) / (2 * AREA4)) as u32
}

/// **Lo que tiene que haber en `(x, y)`**: el color mezclado dentro (rojo por
/// el peso de v0, verde por el de v1, azul por el de v2), magenta fuera.
pub const fn esperado(x: u32, y: u32) -> u32 {
    if !ra::dentro(x, y) {
        return PIXEL_LIMPIO;
    }
    let w0 = arista_centro(V1, V2, x, y);
    let w1 = arista_centro(V2, V0, x, y);
    let w2 = arista_centro(V0, V1, x, y);
    0xFF00_0000 | canal(w0) << 16 | canal(w1) << 8 | canal(w2)
}

/// Si `p` vale por `e`: exacto fuera; dentro, 1 de diferencia por canal.
pub const fn vale(p: u32, e: u32) -> bool {
    if e == PIXEL_LIMPIO || p == PIXEL_LIMPIO {
        return p == e;
    }
    let mut k = 0;
    while k < 32 {
        let (a, b) = ((p >> k) & 0xFF, (e >> k) & 0xFF);
        if a.abs_diff(b) > 1 {
            return false;
        }
        k += 8;
    }
    true
}

/// Cuantos pixeles valen.
pub fn comprobar(salida: &[u32]) -> u32 {
    salida.iter().take(PIXELES).enumerate().filter(|&(k, &p)| vale(p, esperado(k as u32 % LADO, k as u32 / LADO))).count() as u32
}

pub use crate::fractal::{desempaquetar, empaquetar, sano};

const _: () = assert!(PALABRAS_VS * 4 <= (ra::PS - ra::VS) as usize);
const _: () = assert!(SEMAFORO_FIN > ra::SEMAFORO_FIN && SEMAFORO_FIN + 16 <= SEMAFOROS + 4096);
const _: () = assert!(VERTICE == 1 && PIXEL == 5);

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_cabeceras() {
        let v = sph_vertice();
        assert_eq!(v[13], 0xF << 12 | 0xF << 16); // posicion + generico 0
        assert_eq!(v[10], 1 << 31);
        let p = sph_pixel();
        assert_eq!(p[6], 0b11_1111); // X, Y, Z lineales
        assert_eq!((p[5], p[18]), (1 << 31, 0xF));
    }

    #[test]
    fn los_programas() {
        let v = vertice();
        assert_eq!(v[SPH + 4 * 19], 0xFF00_7322); // AST, el del color
        assert_eq!((v[SPH + 4 * 19 + 1] >> 8) & 0x3FF, 0x80); // a[0x80], bits 40..50
        assert_eq!((CODIGO_VS[19].0 >> 40) & 0x3FF, 0x80);
        assert_eq!((CODIGO_VS[20].0 >> 40) & 0x3FF, 0x70);
        // IPA: el atributo / 4 en 64..74 y el destino en 16..24.
        for (i, a) in [0x80u64, 0x84, 0x88].into_iter().enumerate() {
            let (lo, hi) = CODIGO_PS[1 + i];
            assert_eq!((lo & 0xFFF, (lo >> 16) & 0xFF, hi & 0x3FF), (0x326, i as u64, a / 4));
        }
    }

    #[test]
    fn el_juez_en_los_vertices() {
        // Junto a cada vertice, su color casi puro.
        let e = esperado(256, 42);
        assert!((e >> 16) & 0xFF >= 250 && e & 0xFFFF < 0x0606, "{e:08x}");
        let e = esperado(468, 438);
        assert!((e >> 8) & 0xFF >= 245, "{e:08x}");
        let e = esperado(44, 438);
        assert!(e & 0xFF >= 245, "{e:08x}");
        assert_eq!(esperado(10, 500), PIXEL_LIMPIO);
    }

    #[test]
    fn los_pesos_suman_255() {
        // Cada canal redondeado por separado: la suma anda en 255 +- 1.
        for (x, y) in [(256, 240), (300, 400), (100, 430), (256, 45)] {
            let e = esperado(x, y);
            let s = (e >> 16 & 0xFF) + (e >> 8 & 0xFF) + (e & 0xFF);
            assert!((254..=256).contains(&s), "({x},{y}) {s}");
        }
    }

    #[test]
    fn vale_con_uno_de_diferencia() {
        let e = esperado(256, 240);
        assert!(vale(e, e) && vale(e + 1, e) && vale(e - 0x100, e));
        assert!(!vale(e + 2, e) && !vale(PIXEL_LIMPIO, e) && !vale(e, PIXEL_LIMPIO));
        let mut img = [0u32; PIXELES];
        for (k, p) in img.iter_mut().enumerate() {
            *p = esperado(k as u32 % LADO, k as u32 / LADO);
        }
        assert_eq!(comprobar(&img), PIXELES as u32);
        img[256 * 512 + 256] ^= 0x0000_0404;
        assert_eq!(comprobar(&img), PIXELES as u32 - 1);
    }
}
