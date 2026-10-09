//! DL12 (09-10, del propietario): el juez con el KILL -- el `discard` de un
//! programa de pixel --. Lo que se sabe de el, cada regla con su bodrio.

use bmo_sm86::codifica::{self as c, Cmp, Fuente, LEIDAS_DL12, PT};

use super::juez::{conoce, juzgar, juzgar_cuerpo_de_app, Contexto, Regla};
use crate::raster::{self, MATA_PIXELES};

/// El control: espera, sin barreras.
const fn k(espera: u64) -> u64 {
    espera | 1 << 4 | 7 << 5 | 7 << 8
}

/// `if (x < 0) discard;` con el color en R0..R3: el FSETP, su KILL 13
/// ciclos despues (`espera` en el FSETP), el color y el EXIT.
fn cuerpo(espera: u64) -> [(u64, u64); 7] {
    [
        c::fsetp(0, Cmp::Lt, c::r(4), Fuente::Imm(0), k(espera)),
        c::kill(0, k(1)),
        c::mov(0, c::r(4), k(1)),
        c::mov(1, c::r(4), k(1)),
        c::mov(2, c::r(4), k(1)),
        c::mov(3, Fuente::Imm(0x3F80_0000), k(4)),
        c::exit(k(5)),
    ]
}

/// El juez conoce el KILL que leyo `nvdisasm` (sin guarda y con los de P0,
/// !P0, P3 y !P5) y NINGUNA otra forma: con otro predicado en 87..91
/// (`KILL P0`) o con bits de mas, es R0.
#[test]
fn el_juez_conoce_el_kill_que_se_sabe_y_nada_mas() {
    for (texto, lo, hi) in LEIDAS_DL12 {
        assert!(conoce(*lo, *hi), "{texto}");
    }
    let (lo, hi) = c::kill(0, k(1));
    assert!(!conoce(lo, hi & !(7 << 23)), "KILL P0");
    assert!(!conoce(lo, hi | 1 << 26), "KILL !PT");
    assert!(!conoce(lo | 1 << 16, hi), "un registro donde no va");
    assert!(!conoce(lo & !(7 << 9) | 1 << 9, hi), "otra forma");
}

/// R7: un cuerpo de app puede tirar su pixel -- un KILL con guarda (o sin
/// el) --, pero no al final: el cuerpo acaba en EXIT.
#[test]
fn r7_deja_el_kill_con_su_guarda() {
    assert_eq!(juzgar_cuerpo_de_app(&cuerpo(13), 5), Ok(()));
    let mut sin_guarda = cuerpo(13);
    sin_guarda[1] = c::kill(PT, k(1));
    assert_eq!(juzgar_cuerpo_de_app(&sin_guarda, 5), Ok(()));
    let b = juzgar_cuerpo_de_app(&[c::kill(0, k(1))], 5).unwrap_err();
    assert_eq!((b.regla, b.instruccion), (Regla::R7CuerpoAjeno, 0), "un cuerpo que acaba en KILL");
}

/// R5: el KILL en un programa de pixel con KillsPixels (bit 15), perfecto;
/// sin el, R5 en el KILL -- NVIDIA: seria un NOP y una excepcion --; y en un
/// programa de vertice, R5 aunque lo diga: KillsPixels es solo del de pixel.
#[test]
fn r5_el_kill_pide_kills_pixels_y_un_programa_de_pixel() {
    let codigo = cuerpo(13);
    let pixel = raster::sph_pixel();
    let mut con = pixel;
    con[0] |= MATA_PIXELES;
    let ctx = |h| Contexto { registros: raster::REGISTROS, sph: Some(h) };
    assert!(juzgar(&codigo, &ctx(&con)).is_ok());
    let b = juzgar(&codigo, &ctx(&pixel)).unwrap_err();
    assert_eq!((b.regla, b.instruccion, b.que), (Regla::R5CabeceraMiente, 1, 15));
    let mut vertice = raster::sph_vertice();
    vertice[0] |= MATA_PIXELES;
    let b = juzgar(&codigo, &ctx(&vertice)).unwrap_err();
    assert_eq!((b.regla, b.instruccion), (Regla::R5CabeceraMiente, 1));
    // Sin KILL, la SPH sin el bit es la de siempre.
    let mut sin = codigo;
    sin[1] = c::nop(k(1));
    assert!(juzgar(&sin, &ctx(&pixel)).is_ok());
}

/// R9: el guarda del KILL espera a su FSETP como el de un BRA (13 ciclos).
#[test]
fn r9_el_guarda_del_kill_espera_a_su_predicado() {
    let mut h = raster::sph_pixel();
    h[0] |= MATA_PIXELES;
    let ctx = Contexto { registros: raster::REGISTROS, sph: Some(&h) };
    assert!(juzgar(&cuerpo(13), &ctx).is_ok());
    assert_eq!(juzgar(&cuerpo(12), &ctx).unwrap_err().regla, Regla::R9PredicadoAntesDeLlegar);
}
