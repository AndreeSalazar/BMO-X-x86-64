//! **L1/L2: LA LENTE DE LAS DERIVADAS** (14-10) -- lo que la 3060 HACE con
//! los cuadros de 2x2 de un sombreador de pixeles, medido en el metal: un
//! dibujo a pantalla entera cuyo programa de pixeles saca `ddx` y `ddy` de
//! la posicion de su pixel (`Carga::Posicion`: columna + 0,5 y fila + 0,5).
//! Lo que tiene que dar no se discute: `ddx(x) = 1`, `ddy(y) = 1`,
//! `ddx(y) = ddy(x) = 0`, en todos los pixeles.
//!
//! capa: puro -- los programas, la receta y como se lee un pixel; el
//! director dibuja y cuenta (`gpu verrano derivadas`)
//!
//! [eje]     CORRECCION -- lo que Mesa no dice (`docs/metal/mesa_nak/
//!           INFORME_NAK.md`, DUDAS 1 y 2): que hilo del cuadro es cada uno
//!           en la pantalla, y el signo de FSWZADD
//!
//! # Las dos pruebas
//!
//! ```text
//!    GRUESA   con SHFL.IDX (el de las derivadas de la casa, `prometeo::olas`:
//!             p1 - p0 en x, p2 - p0 en y): R = ddx(x), G = ddy(y),
//!             B = ddx(y), A = ddy(x). Si el hilo 1 no es el de la DERECHA o
//!             el 2 el de ABAJO, sale -1 o 0 donde va 1.
//!    FSWZADD  con SHFL.BFLY y FSWZADD (lo de NAK): R = ddx(x) con el patron
//!             0x66, G = ddy(y) con 0x5A; B = la fina en x por PARES (SHFL.IDX
//!             0x1e01); A = 0. Un -1 en R o en G: el signo de FSWZADD es el
//!             contrario del que se supone.
//! ```
//!
//! Cada canal sale como `0,5 + 0,25 d`: el 1 da 191, el 0 da 127 o 128, el
//! -1 da 64 ([`valor`] los separa).

use crate::cubo::{con_control, ALU};
use crate::destino::Destino;
use crate::pegamento::{Carga, Elemento};
use crate::receta::{self, Receta, MAX_CARGAS, MAX_ELEMENTOS, MAX_GENERICOS, NINGUNA, NINGUNO};
use crate::tuberia::{carga, espera, Dibujo};

const SIEMPRE: u64 = 7 << 12;

/// `SHFL.<modo> PT, Rd, Ra, lane, c` (0xF89, `bmo_sm86::olas::shfl`: su oro).
pub const fn shfl(rd: u64, ra: u64, modo: u64, lane: u64, c: u64, control: u64) -> (u64, u64) {
    (0xF89 | SIEMPRE | rd << 16 | ra << 24 | (c & 0x1FFF) << 40 | (lane & 0x1F) << 53 | modo << 58, con_control(7 << 17, control))
}

/// `FSWZADD Rd, Ra, Rb, patron` (`bmo_sm86::olas::fswzadd`).
pub const fn fswzadd(rd: u64, ra: u64, rb: u64, patron: u64, control: u64) -> (u64, u64) {
    (0x822 | SIEMPRE | rd << 16 | ra << 24 | patron << 32, con_control(rb, control))
}

/// `FADD Rd, Ra, -Rb`.
const fn resta(rd: u64, ra: u64, rb: u64, control: u64) -> (u64, u64) {
    (0x021 | 1 << 9 | SIEMPRE | rd << 16 | ra << 24 | rb << 32 | 1 << 63, con_control(0, control))
}

/// `FFMA Rd, Ra, 0.25, 0.5` en dos: `FMUL Rd, Ra, 0.25` y `FADD Rd, Rd, 0.5`.
const fn por(rd: u64, ra: u64, imm: f32) -> (u64, u64) {
    (0x020 | 4 << 9 | SIEMPRE | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, con_control(1 << 22, ALU))
}
const fn mas(rd: u64, ra: u64, imm: f32) -> (u64, u64) {
    (0x021 | 2 << 9 | SIEMPRE | rd << 16 | ra << 24 | (imm.to_bits() as u64) << 32, con_control(0, ALU))
}

/// Los modos de SHFL y las c del cuadro y de los pares.
pub const IDX: u64 = 0;
pub const BFLY: u64 = 3;
pub const CUADRO: u64 = 0x1c03;
pub const PARES: u64 = 0x1e01;
/// Los patrones de NAK (`from_nir.rs`): ddx sub, subr, sub, subr; ddy sub,
/// sub, subr, subr (el hilo 0 en los bits altos).
pub const DDX: u64 = 0x66;
pub const DDY: u64 = 0x5A;

/// Donde deja el pegamento la posicion del pixel.
pub const X: u64 = 4;
pub const Y: u64 = 5;

/// Las dos pruebas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prueba {
    Gruesa,
    Fswzadd,
}

/// Lo que tiene que salir en (R, G, B, A): 1, 0 o -1.
pub const fn esperado(p: Prueba) -> [i8; 4] {
    match p {
        Prueba::Gruesa => [1, 1, 0, 0],
        Prueba::Fswzadd => [1, 1, 1, 0],
    }
}

/// `EXIT` esperando las barreras de `mascara`.
const fn exit(mascara: u64) -> (u64, u64) {
    (crate::cubo::EXIT.0, con_control(crate::cubo::EXIT.1, espera(mascara)))
}

/// **El cuerpo de pixeles de la prueba `p`**: las derivadas a R0..R3 (la
/// salida 0), cada una `0,5 + 0,25 d`. Los SHFL, cada uno con su barrera
/// (0..5); las restas las esperan todas.
pub fn cuerpo(p: Prueba) -> [(u64, u64); 15] {
    let s = |rd, ra, modo, lane, c, b| shfl(rd, ra, modo, lane, c, carga(b));
    let todas = espera(0x3F);
    let [a, b, c, d, e, f, g, h, i, j] = match p {
        Prueba::Gruesa => [
            s(8, X, IDX, 0, CUADRO, 0),
            s(9, X, IDX, 1, CUADRO, 1),
            s(10, Y, IDX, 0, CUADRO, 2),
            s(11, Y, IDX, 2, CUADRO, 3),
            s(12, Y, IDX, 1, CUADRO, 4),
            s(13, X, IDX, 2, CUADRO, 5),
            resta(0, 9, 8, todas),
            resta(1, 11, 10, ALU),
            resta(2, 12, 10, ALU),
            resta(3, 13, 8, ALU),
        ],
        Prueba::Fswzadd => [
            s(8, X, BFLY, 1, CUADRO, 0),
            s(9, Y, BFLY, 2, CUADRO, 1),
            s(10, X, IDX, 0, PARES, 2),
            s(11, X, IDX, 1, PARES, 3),
            s(12, X, IDX, 0, CUADRO, 4),
            s(13, X, IDX, 0, CUADRO, 5),
            fswzadd(0, 8, X, DDX, todas),
            fswzadd(1, 9, Y, DDY, ALU),
            resta(2, 11, 10, ALU),
            resta(3, 13, 12, ALU),
        ],
    };
    [a, b, c, d, e, f, g, h, i, j, por(0, 0, 0.25), mas(0, 0, 0.5), por(1, 1, 0.25), mas(1, 1, 0.5), exit(0)]
}

/// Lo que se ve de un canal de 8 bits: 1, 0, -1, o `None` (otra cosa).
pub const fn valor(b: u8) -> Option<i8> {
    match b {
        180..=200 => Some(1),
        120..=135 => Some(0),
        55..=72 => Some(-1),
        _ => None,
    }
}

/// Los DATOS: dos triangulos que cubren la pantalla (la posicion en
/// recorte, 4 floats por vertice).
pub const VERTICES: usize = 6;
pub const BYTES_DATOS: usize = 16 * VERTICES;

/// Las B y A de las restas que no pasan por FMUL/FADD: el cuerpo las deja
/// en d; lo de `0,5 + 0,25 d` se hace en R0 y R1 (ver `cuerpo`) y en R2, R3
/// con el mismo par detras ([`cuerpo_entero`]).
pub fn cuerpo_entero(p: Prueba) -> [(u64, u64); 19] {
    let c = cuerpo(p);
    let mut out = [(0u64, 0u64); 19];
    out[..14].copy_from_slice(&c[..14]);
    out[14] = por(2, 2, 0.25);
    out[15] = mas(2, 2, 0.5);
    out[16] = por(3, 3, 0.25);
    out[17] = mas(3, 3, 0.5);
    out[18] = c[14];
    out
}

fn bytes(w: &[(u64, u64)], out: &mut [u8]) -> usize {
    for (k, &(lo, hi)) in w.iter().enumerate() {
        out[16 * k..16 * k + 8].copy_from_slice(&lo.to_le_bytes());
        out[16 * k + 8..16 * k + 16].copy_from_slice(&hi.to_le_bytes());
    }
    16 * w.len()
}

/// **La receta de la prueba `p`** con el destino de 1280x720 RGBA8 en
/// `va_destino`. En `caja`; devuelve cuanto mide.
pub fn receta(p: Prueba, va_destino: u64, datos: &mut [u8; BYTES_DATOS], caja: &mut [u8]) -> Option<usize> {
    let esquinas = [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
    for (j, &(x, y)) in esquinas.iter().enumerate() {
        for (i, f) in [x, y, 0.5, 1.0].iter().enumerate() {
            datos[16 * j + 4 * i..16 * j + 4 * i + 4].copy_from_slice(&f.to_le_bytes());
        }
    }
    let mut vs = [0u8; 32];
    let mut ps = [0u8; 16 * 19];
    // El de vertice deja la posicion como llega (R0..R3).
    let mov = (0x202 | SIEMPRE | 0u64 << 32, con_control(0xF << 8, ALU));
    let nv = bytes(&[mov, exit(0)], &mut vs);
    let np = bytes(&cuerpo_entero(p), &mut ps);
    let dst = Destino { fila: 4 * 1280, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: true };
    let mut r = Receta {
        n: VERTICES,
        vs: &vs[..nv],
        ps: &ps[..np],
        registros_vs: 8,
        registros_ps: 16,
        salidas: 1,
        posicion: 0,
        filas: 0,
        paso: 16,
        elementos: [NINGUNO; MAX_ELEMENTOS],
        n_elementos: 1,
        cargas_vs: [NINGUNA; MAX_CARGAS],
        n_cargas_vs: 4,
        cargas_ps: [NINGUNA; MAX_CARGAS],
        n_cargas_ps: 2,
        genericos: [None; MAX_GENERICOS],
        n_genericos: 0,
        datos: &datos[..],
        dibujo: Dibujo { indices: None, vertices: VERTICES as u32, destino: Some((va_destino, dst)), color: Some(0xFF00_0000), texturas: 0, ..Dibujo::default() },
        texturas: [crate::texturas::DeApp::NINGUNA; crate::texturas::MAX_TEXTURAS],
        termometro_vs: None,
        termometro_ps: None,
    };
    r.elementos[0] = Elemento::memoria(0, 4);
    for k in 0..4u8 {
        r.cargas_vs[k as usize] = Carga::Entrada { elemento: 0, componente: k, reg: k };
    }
    r.cargas_ps[0] = Carga::Posicion { componente: 0, reg: X as u8 };
    r.cargas_ps[1] = Carga::Posicion { componente: 1, reg: Y as u8 };
    receta::escribir(caja, &r)
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;

    #[test]
    fn los_codificadores_son_los_de_bmo_sm86() {
        use bmo_sm86::olas as o;
        for (rd, ra, lane, c) in [(8u8, 4u8, 1u8, o::CUADRO), (11, 5, 2, o::CUADRO), (13, 4, 1, o::PARES)] {
            assert_eq!(shfl(rd as u64, ra as u64, IDX, lane as u64, c as u64, ALU), o::shfl(rd, ra, o::Shfl::Idx, lane, c, ALU));
            assert_eq!(shfl(rd as u64, ra as u64, BFLY, lane as u64, c as u64, ALU), o::shfl(rd, ra, o::Shfl::Bfly, lane, c, ALU));
        }
        assert_eq!(fswzadd(0, 8, 4, DDX, ALU), o::fswzadd(0, 8, 4, o::DDX, false, ALU));
        assert_eq!(fswzadd(1, 9, 5, DDY, ALU), o::fswzadd(1, 9, 5, o::DDY, false, ALU));
        assert_eq!((DDX as u8, DDY as u8, CUADRO as u16, PARES as u16), (o::DDX, o::DDY, o::CUADRO, o::PARES));
    }

    /// Las dos recetas se escriben, se releen, se PEGAN como en el kernel y
    /// el juez (R7 con SHFL y FSWZADD, y R0..R6 entero) las aprueba.
    #[test]
    fn las_dos_recetas_pasan_el_kernel() {
        let mut datos = [0u8; BYTES_DATOS];
        let mut caja = std::vec![0u8; receta::MAX_RECETA];
        let mut taller = std::boxed::Box::new(receta::Taller::nuevo());
        for p in [Prueba::Gruesa, Prueba::Fswzadd] {
            let n = receta(p, 0x1000_0000, &mut datos, &mut caja).expect("se escribe");
            let r = receta::leer(&caja[..n]).expect("se relee");
            receta::pegar(&r, &mut taller).unwrap_or_else(|e| panic!("{p:?}: {e:?}"));
        }
        assert_eq!([valor(191), valor(127), valor(128), valor(64), valor(255)], [Some(1), Some(0), Some(0), Some(-1), None]);
    }
}
