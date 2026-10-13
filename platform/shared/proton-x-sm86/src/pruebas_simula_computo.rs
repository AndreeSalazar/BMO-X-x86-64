//! ** LI2f (12-10): la 3060 de mentira corre COMPUTO -- los ids (S2R), la
//! compartida con su barrera, los atomicos y la memoria global --, con las
//! palabras de oro de `bmo_sm86::computo`. Y lo que toca fuera de la
//! memoria que hay es un `Fallo`, nunca un "seguramente".

extern crate std;

use alloc::vec::Vec;

use bmo_sm86::codifica::{self as c, Fuente, RZ};
use bmo_sm86::computo::{self as k, Atomo, Sr, SIEMPRE};

use crate::simula::{correr, Maquina, Memoria, NoSimula};
use crate::simula_grupo::correr_grupo;

/// Una memoria global de prueba: palabras desde `base`.
struct Global {
    base: u64,
    w: Vec<u32>,
}

impl Memoria for Global {
    fn leer(&mut self, dir: u64) -> Option<u32> {
        let i = dir.checked_sub(self.base)?;
        (i % 4 == 0).then_some(())?;
        self.w.get((i / 4) as usize).copied()
    }
    fn escribir(&mut self, dir: u64, v: u32) -> bool {
        let Some(i) = dir.checked_sub(self.base) else { return false };
        match self.w.get_mut((i / 4) as usize) {
            Some(x) if i % 4 == 0 => {
                *x = v;
                true
            }
            _ => false,
        }
    }
}

const BASE: u64 = 0x1_2000_0000;

/// *** Cada hilo escribe su tid en la compartida, BARRERA, lee el del de
/// al lado y lo deja en la global: sin la barrera, el ultimo leeria un 0.
/// Y el ctaid de un grupo que no es el 0.
#[test]
fn la_compartida_con_su_barrera() {
    let codigo = [
        k::s2r(0, Sr::TidX, 0),
        c::shl(1, 0, Fuente::Imm(2), 0),
        k::sts(1, 0, 0, 0),
        k::bar_sync(0),
        k::lds(2, 1, 4, 0),
        k::s2r(3, Sr::CtaidY, 0),
        c::iadd3(2, 2, c::r(3), 0),
        k::stg(SIEMPRE, 4, 0, 2, 0),
        c::exit(0),
    ];
    let n = 32u32;
    let mut compartida = std::vec![0u8; 4 * (n as usize + 1)];
    let mut g = Global { base: BASE, w: std::vec![0; n as usize] };
    let pasos = correr_grupo(&codigo, [n, 1, 1], [0, 7, 0], [&[]; 8], &mut compartida, &mut g, |t, m| {
        let dir = BASE + 4 * t as u64;
        m.r[4] = dir as u32;
        m.r[5] = (dir >> 32) as u32;
    })
    .unwrap();
    assert!(pasos > 0);
    for t in 0..n {
        let esperado = if t + 1 < n { t + 1 } else { 0 } + 7;
        assert_eq!(g.w[t as usize], esperado, "hilo {t}");
    }
}

/// *** Los atomicos de la compartida, hilo tras hilo (el orden de la casa):
/// lo de antes de cada uno, y lo que queda.
#[test]
fn los_atomicos_en_orden() {
    let codigo = [
        k::s2r(0, Sr::TidX, 0),
        c::mov(1, Fuente::Imm(1), 0),
        k::atoms(Atomo::Add, false, 2, RZ, 1, 0),
        k::atoms(Atomo::Max, false, 3, RZ, 0, 0),
        k::stg(SIEMPRE, 4, 0, 2, 0),
        c::exit(0),
    ];
    let mut compartida = std::vec![0u8; 16];
    let mut g = Global { base: BASE, w: std::vec![0; 8] };
    correr_grupo(&codigo, [8, 1, 1], [0; 3], [&[]; 8], &mut compartida, &mut g, |t, m| {
        let dir = BASE + 4 * t as u64;
        m.r[4] = dir as u32;
        m.r[5] = (dir >> 32) as u32;
    })
    .unwrap();
    assert_eq!(g.w, (0..8).collect::<Vec<u32>>(), "ADD: lo de antes de cada uno");
    // cada hilo suma 1 y luego MAX(lo sumado, t) deja la suma: 8 hilos, 8
    assert_eq!(u32::from_le_bytes(compartida[..4].try_into().unwrap()), 8, "MAX no baja lo sumado");
}

/// *** Lo que toca fuera es un Fallo: la global, la compartida, y un STG
/// con su guarda en falso no toca nada.
#[test]
fn fuera_es_un_fallo() {
    let mut g = Global { base: BASE, w: std::vec![0; 4] };
    let mut m = Maquina::nueva([&[]; 8]);
    let dir = BASE + 64;
    m.r[4] = dir as u32;
    m.r[5] = (dir >> 32) as u32;
    m.global = Some(&mut g);
    assert_eq!(correr(&[k::ldg(SIEMPRE, 0, 4, 0, 0), c::exit(0)], &mut m), Err(NoSimula::Fallo(0)));
    m.p[3] = false;
    assert!(correr(&[k::stg(3, 4, 0, 0, 0), c::exit(0)], &mut m).is_ok(), "con la guarda en falso, no toca");
    let mut s = [0u8; 8];
    let mut m = Maquina::nueva([&[]; 8]);
    m.compartida = Some(&mut s);
    m.r[1] = 8;
    assert_eq!(correr(&[k::sts(1, 0, 1, 0), c::exit(0)], &mut m), Err(NoSimula::Fallo(0)));
    let mut m = Maquina::nueva([&[]; 8]);
    assert_eq!(correr(&[k::bar_sync(0), c::exit(0)], &mut m), Err(NoSimula::Instruccion(0)), "una barrera, solo en un grupo");
}
