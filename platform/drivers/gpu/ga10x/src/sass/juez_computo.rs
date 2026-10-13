//! **R7 PARA EL COMPUTO DE UNA APP** (LI2f, 12-10) -- lo que el juez le deja
//! a un cuerpo de computo ademas de lo de siempre: sus ids (S2R TID y
//! CTAID), la barrera del grupo (BAR.SYNC) y la memoria de SUS UAV (LDG y
//! STG), con la direccion SUJETA a lo que el kernel le presto.
//!
//! [carril]  VERDE     lee palabras y dice si o no
//!
//! # La cadena de la direccion
//!
//! El cuerpo NO elige a donde va la 3060: el kernel le pone de cada UAV su
//! BASE (un par de registros) y su TOPE (la mayor direccion de palabra de lo
//! prestado, desde la base), y el cuerpo no los puede pisar. Cada LDG o STG
//! tiene que venir JUSTO detras de estas cuatro, palabra a palabra las de
//! `bmo_sm86::codifica` (las que emite `proton-x-sm86/src/uav.rs`):
//!
//! ```text
//!    LOP3.LUT  Ra, x, 0xfffffffc, RZ, 0xc0, !PT   alinear a la palabra
//!    IMNMX.U32 Ra, Ra, TOPE, PT                   el menor: NUNCA pasa del tope
//!    IADD3     Ra, Pp, BASE, Ra, RZ               sumar a la base (el acarreo a Pp)
//!    IADD3.X   Ra+1, BASE+1, RZ, RZ, Pp, !PT      la mitad alta, con su acarreo
//!    LDG.E / @P STG.E  [Ra.64]                    sin inmediato
//! ```
//!
//! (El juez no enlaza el codificador -- el crate del driver no tiene
//! dependencias, por esquema --: las palabras van escritas aqui con sus
//! campos, y la prueba de abajo las compara con las de `bmo_sm86`.)
//!
//! Con eso la direccion es `BASE + min(x & ~3, TOPE)`: alineada y dentro de
//! lo prestado, valga `x` lo que valga. Seguidas, sin nada en medio, y
//! ningun BRA del cuerpo cae entre la primera y la memoria (se saltaria la
//! sujecion). El par Ra, Ra+1, alineado.
//!
//! Que la palabra VALGA (el predicado del STG, el SEL tras el LDG) es cosa
//! del cuerpo: si se equivoca, lee o escribe mal SU UAV. Eso es correccion,
//! no seguridad, y el juez no lo mira.
//!
//! [!] La barrera: si un hilo no llega (una barrera dentro de un `si` que no
//! todos toman), la 3060 se queda esperando. El emisor solo la pone fuera de
//! todo `si` y bucle; el juez no puede demostrar que todos lleguen, como no
//! puede demostrar que un bucle acabe (E6): lo que lo para es del kernel.

use super::juez::{Bodrio, Regla};

const RZ: u8 = 255;
/// La guarda PT (siempre), en 12..16.
const PT: u64 = 7 << 12;

// Las palabras que el juez acepta (lo, y los 41 bits bajos de hi), con sus
// campos: `bmo_sm86::{codifica, computo}`, de oro.
fn s2r(rd: u8, sr: u8) -> (u64, u64) {
    (0x7919 | (rd as u64) << 16, (sr as u64) << 8)
}
const BAR_SYNC: (u64, u64) = (0x7b1d, 1 << 16);
fn ldg(rd: u8, a: u8) -> (u64, u64) {
    (0x0981 | PT | (rd as u64) << 16 | (a as u64) << 24 | 0x04 << 32, 0x0c1e_1900)
}
fn stg(guarda: u8, a: u8, rb: u8) -> (u64, u64) {
    (0x0986 | (guarda as u64 & 0xF) << 12 | (a as u64) << 24 | (rb as u64) << 32, 0x0c10_1904)
}
fn atomg(guarda: u8, rd: u8, a: u8, rb: u8, op: u8, signo: bool) -> (u64, u64) {
    (0x09a8 | (guarda as u64 & 0xF) << 12 | (rd as u64) << 16 | (a as u64) << 24 | (rb as u64) << 32, 0x081e_e1c4 & !(0xF << 23) | (op as u64) << 23 | (signo as u64) << 9)
}
fn atomg_cas(guarda: u8, rd: u8, a: u8, rb: u8, rc: u8) -> (u64, u64) {
    (0x03a9 | (guarda as u64 & 0xF) << 12 | (rd as u64) << 16 | (a as u64) << 24 | (rb as u64) << 32, 0x001e_e100 | rc as u64)
}
/// `LOP3.LUT a, x, 0xfffffffc, RZ, 0xc0, !PT`.
fn alinea(a: u8, x: u8) -> (u64, u64) {
    (0x012 | PT | 4 << 9 | (a as u64) << 16 | (x as u64) << 24 | 0xFFFF_FFFC << 32, 0x078e_0000 | 0xC0 << 8 | RZ as u64)
}
/// `IMNMX.U32 a, a, tope, PT`.
fn sujeta(a: u8, tope: u8) -> (u64, u64) {
    (0x017 | PT | 1 << 9 | (a as u64) << 16 | (a as u64) << 24 | (tope as u64) << 32, 7 << 23)
}
/// `IADD3 a, Pp, base, a, RZ`.
fn suma(a: u8, p: u8, base: u8) -> (u64, u64) {
    (0x010 | PT | 1 << 9 | (a as u64) << 16 | (base as u64) << 24 | (a as u64) << 32, (0x07ff_e000 & !(7 << 17)) | (p as u64) << 17 | RZ as u64)
}
/// `IADD3.X a+1, base+1, RZ, RZ, Pp, !PT`.
fn alta(a: u8, p: u8, base: u8) -> (u64, u64) {
    (0x010 | PT | 1 << 9 | (a as u64 + 1) << 16 | (base as u64 + 1) << 24 | (RZ as u64) << 32, (0x07ff_e000 & !(0xF << 23)) | (p as u64) << 23 | 1 << 10 | RZ as u64)
}

/// Cuantos UAV puede atar el kernel a un computo.
pub const MAX_UAVS: usize = 8;

/// **Los UAV de un computo**: de cada uno, el registro de su BASE (el par
/// `base`, `base + 1`) y el de su TOPE, los dos del kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct UavsDeApp {
    pub n: u8,
    pub base: [u8; MAX_UAVS],
    pub tope: [u8; MAX_UAVS],
    /// Los de SOLO LEER (un SRV de bufer, bit k el k-esimo): el kernel los
    /// presta sin escritura, y el juez solo deja leerlos (LDG).
    pub solo_lee: u8,
}

impl UavsDeApp {
    /// Si el registro `r` es del kernel (una base, su mitad alta, un tope).
    pub fn protege(&self, r: u8) -> bool {
        (0..self.n as usize).any(|k| r == self.base[k] || r == self.base[k].wrapping_add(1) || r == self.tope[k])
    }
}

const BAJOS: u64 = (1 << 41) - 1;

/// Dos palabras iguales, sin el control.
fn igual(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 == b.0 && a.1 & BAJOS == b.1 & BAJOS
}

fn ajeno(instruccion: usize, que: u32, detalle: &'static str) -> Result<(), Bodrio> {
    Err(Bodrio { regla: Regla::R7CuerpoAjeno, instruccion, que, detalle })
}

/// Un S2R de un id de hilo o de grupo, y nada mas.
pub(super) fn s2r_de_app(w: (u64, u64), k: usize) -> Result<(), Bodrio> {
    let rd = (w.0 >> 16 & 0xFF) as u8;
    let sr = (w.1 >> 8 & 0xFF) as u8;
    // TID.X/Y/Z 0x21..0x23 y CTAID.X/Y/Z 0x25..0x27.
    if matches!(sr, 0x21..=0x23 | 0x25..=0x27) && igual(w, s2r(rd, sr)) {
        Ok(())
    } else {
        ajeno(k, sr as u32, "un S2R que no es un id de hilo ni de grupo")
    }
}

/// La barrera del grupo, en su forma.
pub(super) fn bar_de_app(w: (u64, u64), k: usize) -> Result<(), Bodrio> {
    if igual(w, BAR_SYNC) {
        Ok(())
    } else {
        ajeno(k, 0, "una barrera que no es BAR.SYNC 0x0")
    }
}

/// **Un LDG o un STG de app** en `k`: su forma (sin inmediato; guarda solo
/// el STG) y la cadena que lo sujeta (ver el modulo).
pub(super) fn memoria_de_app(codigo: &[(u64, u64)], k: usize, uavs: &UavsDeApp) -> Result<(), Bodrio> {
    let w = codigo[k];
    let op = w.0 & 0x1FF;
    let a = (w.0 >> 24 & 0xFF) as u8;
    let (guarda, rd, rb) = ((w.0 >> 12 & 0xF) as u8, (w.0 >> 16 & 0xFF) as u8, (w.0 >> 32 & 0xFF) as u8);
    let forma = match op {
        0x181 => igual(w, ldg(rd, a)),
        0x186 => igual(w, stg(guarda, a, rb)),
        0x1A8 => {
            let aop = (w.1 >> 23 & 0xF) as u8;
            matches!(aop, 0 | 1 | 2 | 5 | 6 | 7 | 8) && igual(w, atomg(guarda, rd, a, rb, aop, w.1 >> 9 & 1 == 1))
        }
        _ => igual(w, atomg_cas(guarda, rd, a, rb, (w.1 & 0xFF) as u8)),
    };
    if !forma {
        return ajeno(k, op as u32, "un LDG, STG o ATOMG que no es de la forma de una app (.E, 32 bits, sin inmediato; guarda solo el STG y el ATOMG)");
    }
    if a % 2 != 0 || a >= RZ - 1 {
        return ajeno(k, a as u32, "la direccion de un LDG o un STG no es un par alineado");
    }
    let Some(j) = k.checked_sub(4) else {
        return ajeno(k, a as u32, "un LDG o un STG sin la cadena que sujeta su direccion");
    };
    let [w_alinea, w_sujeta, w_suma, w_alta] = [codigo[j], codigo[j + 1], codigo[j + 2], codigo[j + 3]];
    let x = (w_alinea.0 >> 24 & 0xFF) as u8;
    if !igual(w_alinea, alinea(a, x)) {
        return ajeno(j, a as u32, "antes de un LDG o un STG no esta el LOP3 que alinea su direccion");
    }
    let p = (w_suma.1 >> 17 & 7) as u8;
    let base = (w_suma.0 >> 24 & 0xFF) as u8;
    let Some(u) = (0..uavs.n as usize).find(|&u| uavs.base[u] == base) else {
        return ajeno(j + 2, base as u32, "la direccion de un LDG o un STG no se suma a la base de un UAV del kernel");
    };
    if op != 0x181 && uavs.solo_lee >> u & 1 == 1 {
        return ajeno(k, u as u32, "escribe en un bufer de SOLO LEER (un SRV): el kernel no lo presto para eso");
    }
    if !igual(w_sujeta, sujeta(a, uavs.tope[u])) {
        return ajeno(j + 1, a as u32, "la direccion de un LDG o un STG no la sujeta un IMNMX.U32 con el tope de SU UAV");
    }
    if p == 7 || base >= RZ - 1 || !igual(w_suma, suma(a, p, base)) {
        return ajeno(j + 2, a as u32, "la direccion de un LDG o un STG no es la base de su UAV mas lo sujeto");
    }
    if !igual(w_alta, alta(a, p, base)) {
        return ajeno(j + 3, a as u32 + 1, "la mitad alta de la direccion de un LDG o un STG no es la de su base con su acarreo");
    }
    // Ningun salto cae dentro de la cadena (la esquivaria).
    for (m, &(blo, bhi)) in codigo.iter().enumerate() {
        if blo & 0x1FF == 0x147 {
            let d = (((bhi & 0x3_FFFF) << 32 | blo >> 32) << 14) as i64 >> 14;
            let destino = (16 * (m as i64 + 1) + d) / 16;
            if destino > j as i64 && destino <= k as i64 {
                return ajeno(m, destino as u32, "un salto cae dentro de la cadena que sujeta un LDG o un STG: la esquivaria");
            }
        }
    }
    Ok(())
}

// == La compartida del grupo (LDS, STS, ATOMS) ==============================
//
// La direccion de una palabra de la compartida, justo detras de:
//
//    IMNMX.U32 Ra, x, TOPE, PT      TOPE un inmediato
//    IMAD      Ra, Ra, 0x4, RZ      a bytes
//    LDS / [@P] STS / [@P] ATOMS(.CAS)  [Ra]   sin inmediato
//
// y 4 * TOPE + 4 <= la compartida que el kernel da al grupo (`compartida`,
// en bytes; la del QMD). Una direccion fuera de la compartida es una
// excepcion de la 3060 (o memoria de otro grupo): nunca.

fn lds(rd: u8, a: u8) -> (u64, u64) {
    (0x7984 | (rd as u64) << 16 | (a as u64) << 24, 0x800)
}
fn sts(guarda: u8, a: u8, rb: u8) -> (u64, u64) {
    (0x0388 | (guarda as u64 & 0xF) << 12 | (a as u64) << 24 | (rb as u64) << 32, 0x800)
}
fn atoms(guarda: u8, rd: u8, a: u8, rb: u8, op: u8, signo: bool) -> (u64, u64) {
    (0x038c | (guarda as u64 & 0xF) << 12 | (rd as u64) << 16 | (a as u64) << 24 | (rb as u64) << 32, (op as u64) << 23 | (signo as u64) << 9)
}
fn atoms_cas(guarda: u8, rd: u8, a: u8, rb: u8, rc: u8) -> (u64, u64) {
    (0x038d | (guarda as u64 & 0xF) << 12 | (rd as u64) << 16 | (a as u64) << 24 | (rb as u64) << 32, rc as u64)
}
/// `IMNMX.U32 a, x, tope, PT` con el tope inmediato.
fn sujeta_imm(a: u8, x: u8, tope: u32) -> (u64, u64) {
    (0x017 | PT | 4 << 9 | (a as u64) << 16 | (x as u64) << 24 | (tope as u64) << 32, 7 << 23)
}
/// `IMAD a, a, 0x4, RZ`.
fn por_cuatro(a: u8) -> (u64, u64) {
    (0x024 | PT | 4 << 9 | (a as u64) << 16 | (a as u64) << 24 | 4 << 32, 0x078e_0200 | RZ as u64)
}

/// **Un LDS, STS o ATOMS de app** en `k`: su forma y la cadena que lo sujeta
/// a los `bytes` de la compartida.
pub(super) fn compartida_de_app(codigo: &[(u64, u64)], k: usize, bytes: u32) -> Result<(), Bodrio> {
    let w = codigo[k];
    let (lo, hi) = w;
    let guarda = (lo >> 12 & 0xF) as u8;
    let (rd, a, rb) = ((lo >> 16 & 0xFF) as u8, (lo >> 24 & 0xFF) as u8, (lo >> 32 & 0xFF) as u8);
    let forma = match lo & 0x1FF {
        0x184 => igual(w, lds(rd, a)),
        0x188 => igual(w, sts(guarda, a, rb)),
        0x18C => {
            let op = (hi >> 23 & 0xF) as u8;
            matches!(op, 0 | 1 | 2 | 5 | 6 | 7 | 8) && igual(w, atoms(guarda, rd, a, rb, op, hi >> 9 & 1 == 1))
        }
        _ => igual(w, atoms_cas(guarda, rd, a, rb, (hi & 0xFF) as u8)),
    };
    if !forma {
        return ajeno(k, (lo & 0x1FF) as u32, "un LDS, STS o ATOMS que no es de la forma de una app (sin inmediato)");
    }
    let Some(j) = k.checked_sub(2) else {
        return ajeno(k, a as u32, "un acceso a la compartida sin la cadena que lo sujeta");
    };
    let x = (codigo[j].0 >> 24 & 0xFF) as u8;
    let tope = (codigo[j].0 >> 32) as u32;
    if !igual(codigo[j], sujeta_imm(a, x, tope)) || !igual(codigo[j + 1], por_cuatro(a)) {
        return ajeno(j, a as u32, "un acceso a la compartida que no viene de IMNMX.U32 con su tope y IMAD por 4");
    }
    if 4 * tope as u64 + 4 > bytes as u64 {
        return ajeno(j, tope, "un acceso que puede salir de la compartida del grupo");
    }
    for (m, &(blo, bhi)) in codigo.iter().enumerate() {
        if blo & 0x1FF == 0x147 {
            let d = (((bhi & 0x3_FFFF) << 32 | blo >> 32) << 14) as i64 >> 14;
            let destino = (16 * (m as i64 + 1) + d) / 16;
            if destino > j as i64 && destino <= k as i64 {
                return ajeno(m, destino as u32, "un salto cae dentro de la cadena que sujeta un acceso a la compartida");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_sm86::codifica::{self as c, Fuente};
    use bmo_sm86::computo::{self as g, Sr};

    /// Las palabras del juez son las del codificador (de oro), campo a campo.
    #[test]
    fn las_palabras_son_las_del_codificador() {
        for sr in Sr::TID.iter().chain(Sr::CTAID.iter()) {
            assert!(igual(s2r(9, *sr as u8), g::s2r(9, *sr, 0)));
        }
        assert!(igual(BAR_SYNC, g::bar_sync(0)));
        assert!(igual(ldg(13, 6), g::ldg(g::SIEMPRE, 13, 6, 0, 0)));
        assert!(igual(stg(0xA, 6, 13), g::stg(0xA, 6, 0, 13, 0)));
        assert!(igual(alinea(6, 21), c::lop3(6, 21, Fuente::Imm(!3), c::Y, 0)));
        assert!(igual(sujeta(6, 33), c::imnmx(6, 6, c::r(33), false, false, 0)));
        assert!(igual(suma(6, 3, 30), c::iadd3_acarreo(6, 3, 30, c::r(6), 0)));
        assert!(igual(alta(6, 3, 30), c::iadd3_x(7, 31, c::r(c::RZ), 3, 0)));
        assert!(igual(lds(5, 6), g::lds(5, 6, 0, 0)));
        for (op, signo) in [(g::Atomo::Add, false), (g::Atomo::Max, true), (g::Atomo::Exch, false)] {
            assert!(igual(atomg(7, 5, 6, 9, op as u8, signo), g::atomg(op, signo, 5, 6, 9, 0)));
        }
        assert!(igual(atomg_cas(7, 5, 6, 9, 10), g::atomg_cas(5, 6, 9, 10, 0)));
        let mut w = g::sts(6, 0, 9, 0);
        w.0 = w.0 & !(0xF << 12) | 0xA << 12;
        assert!(igual(sts(0xA, 6, 9), w));
        assert!(igual(sts(7, 6, 9), g::sts(6, 0, 9, 0)));
        for (op, signo) in [(g::Atomo::Add, false), (g::Atomo::Min, true), (g::Atomo::Exch, false)] {
            assert!(igual(atoms(7, 5, 6, 9, op as u8, signo), g::atoms(op, signo, 5, 6, 9, 0)));
        }
        assert!(igual(atoms_cas(7, 5, 6, 9, 10), g::atoms_cas(5, 6, 9, 10, 0)));
        assert!(igual(sujeta_imm(6, 21, 127), c::imnmx(6, 21, Fuente::Imm(127), false, false, 0)));
        assert!(igual(por_cuatro(6), c::imad(6, 6, Fuente::Imm(4), c::RZ, 0)));
    }
}
