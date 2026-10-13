//! **LA RECETA DE COMPUTO (VRC1)** (LI2f, 13-10) -- lo que manda una APP para
//! que la 3060 corra un `Dispatch`: el CUERPO que emitio PROTON-X, que
//! precarga en que registro, sus buferes (UAV y SRV: su VA en el proceso y su
//! vista) y las filas de su cbuffer. El pegado, las direcciones y lo que se
//! presta los pone el KERNEL: la app no elige a donde va la 3060.
//!
//! capa: puro -- bytes que se leen y se comprueban
//!
//! ```text
//!    0   "VRC1"                    36  registros por hilo
//!    4   bytes de la receta        40  instrucciones   44  desde (16 B c/u)
//!    8   grupos x, y, z            48  precargas       52  desde (4 B c/u)
//!    20  hilos x, y, z             56  buferes         60  desde (24 B c/u)
//!    32  compartida (bytes)        64  filas del cb    68  desde (16 B c/u)
//!
//!    una precarga  [que, a, b, reg]:  1 = la fila (a | b << 8) del cbuffer
//!                                     en reg..reg+4; 5 = lo `b` (DelUav) del
//!                                     bufer de ranura `a` en reg (la base, en
//!                                     el par reg, reg+1)
//!    un bufer      va (u64), bytes, elementos, paso (u32), ranura (u8: la
//!                  del UAV, o 0x80 | t la del SRV tN), y 3 a cero
//! ```
//!
//! Lo que se comprueba aqui: que todo cae dentro de la receta y alineado, los
//! techos, que cada precarga nombra un bufer o una fila que hay y un registro
//! del cuerpo, que la base va en un par, y que no hay dos buferes con la
//! misma ranura. Lo que NO: que el cuerpo sea bueno (el juez), ni que la VA
//! de un bufer sea del proceso (el kernel, con sus cuentas).

use super::computo_app::{DelUav, Despacho};

pub const MAGIA: [u8; 4] = *b"VRC1";
pub const CABECERA: usize = 72;
/// Las instrucciones de un cuerpo de computo, como mucho (las de nBody son
/// 18993: un CS de un juego puede ser largo). El kernel COPIA la receta y
/// arma el pegado en dos buferes fijos (`MAX_RECETA` y `MAX_PEGADO`): no se
/// juzga lo que la app aun puede cambiar.
pub const MAX_INSTRUCCIONES: usize = 24 * 1024;
pub const MAX_PRECARGAS: usize = 256;
/// Los buferes (UAV y SRV) que ata el kernel: los que deja el juez.
pub const MAX_BUFERES: usize = crate::sass::juez_computo::MAX_UAVS;
/// Las filas del cbuffer (las 4096 de D3D12).
pub const MAX_FILAS: usize = 4096;
/// La receta mas grande que se lee.
pub const MAX_RECETA: usize = CABECERA.next_multiple_of(16) + 16 * MAX_INSTRUCCIONES + 16 * MAX_FILAS + 24 * MAX_BUFERES + 4 * MAX_PRECARGAS;
/// El pegado mas largo: los MOV de todas las precargas (4 una fila) y el cuerpo.
pub const MAX_PEGADO: usize = 4 * MAX_PRECARGAS + MAX_INSTRUCCIONES;

/// El `sub` de `Porque::ComputoReceta` para cada NO (`porque::COMPUTO`).
pub fn sub_porque(e: NoReceta) -> (u8, u32) {
    match e {
        NoReceta::Cabecera => (2, 0),
        NoReceta::Tabla => (3, 0),
        NoReceta::Techo => (4, 0),
        NoReceta::Precarga(k) => (5, k as u32),
        NoReceta::Bufer(k) => (6, k as u32),
        NoReceta::Despacho(_) => (7, 0),
    }
}

/// El bit de un SRV en la ranura de un bufer.
pub const SRV: u8 = 0x80;

/// Un bufer de la receta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bufer {
    pub va: u64,
    pub bytes: u32,
    pub elementos: u32,
    pub paso: u32,
    pub ranura: u8,
}

impl Bufer {
    /// Un SRV: se presta solo para leer.
    pub fn solo_lee(&self) -> bool {
        self.ranura & SRV != 0
    }
}

/// Una precarga de la receta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precarga {
    Fila { fila: u16, reg: u8 },
    Bufer { k: usize, que: DelUav, reg: u8 },
}

/// Por que una receta de computo no se sostiene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoReceta {
    /// La cabecera: la magia, o los bytes.
    Cabecera,
    /// Una tabla se sale de la receta, o no va alineada.
    Tabla,
    /// Mas de lo que cabe (instrucciones, precargas, buferes o filas).
    Techo,
    /// Una precarga que no se entiende, o que nombra lo que no hay.
    Precarga(usize),
    /// Un bufer mal dicho (sin bytes, la VA sin alinear, una ranura repetida).
    Bufer(usize),
    /// El despacho (hilos, grupos, compartida, registros).
    Despacho(super::computo_app::NoDespacha),
}

impl NoReceta {
    /// La frase (el lente).
    pub fn texto(self) -> &'static str {
        match self {
            NoReceta::Cabecera => "la cabecera de la receta de computo (VRC1) no se sostiene",
            NoReceta::Tabla => "una tabla de la receta de computo se sale de ella o va sin alinear",
            NoReceta::Techo => "la receta de computo pide mas de lo que cabe (instrucciones, precargas, buferes o filas)",
            NoReceta::Precarga(_) => "una precarga que no se entiende, o que nombra un bufer, una fila o un registro que no hay",
            NoReceta::Bufer(_) => "un bufer mal dicho: sin bytes, la VA sin alinear, o su ranura repetida",
            NoReceta::Despacho(d) => d.texto(),
        }
    }
}

/// **Una receta de computo leida**: sus tablas, sin copiar.
#[derive(Clone, Copy, Debug)]
pub struct Receta<'a> {
    pub despacho: Despacho,
    codigo: &'a [u8],
    precargas: &'a [u8],
    buferes: &'a [u8],
    filas: &'a [u8],
}

fn u32_(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn u64_(b: &[u8], o: usize) -> u64 {
    u32_(b, o) as u64 | (u32_(b, o + 4) as u64) << 32
}

impl<'a> Receta<'a> {
    /// Las instrucciones del cuerpo.
    pub fn instrucciones(&self) -> usize {
        self.codigo.len() / 16
    }

    /// La instruccion `k`.
    pub fn instruccion(&self, k: usize) -> (u64, u64) {
        (u64_(self.codigo, 16 * k), u64_(self.codigo, 16 * k + 8))
    }

    pub fn n_precargas(&self) -> usize {
        self.precargas.len() / 4
    }

    /// La precarga `k` (ya comprobada al leer).
    pub fn precarga(&self, k: usize) -> Precarga {
        let x = &self.precargas[4 * k..4 * k + 4];
        match x[0] {
            1 => Precarga::Fila { fila: u16::from_le_bytes([x[1], x[2]]), reg: x[3] },
            _ => Precarga::Bufer { k: self.bufer_de(x[1]).unwrap_or(0), que: DelUav::de(x[2]).unwrap_or(DelUav::Base), reg: x[3] },
        }
    }

    pub fn n_buferes(&self) -> usize {
        self.buferes.len() / 24
    }

    pub fn bufer(&self, k: usize) -> Bufer {
        let o = 24 * k;
        Bufer { va: u64_(self.buferes, o), bytes: u32_(self.buferes, o + 8), elementos: u32_(self.buferes, o + 12), paso: u32_(self.buferes, o + 16), ranura: self.buferes[o + 20] }
    }

    /// El bufer de la ranura `ranura`.
    pub fn bufer_de(&self, ranura: u8) -> Option<usize> {
        (0..self.n_buferes()).find(|&k| self.bufer(k).ranura == ranura)
    }

    pub fn n_filas(&self) -> usize {
        self.filas.len() / 16
    }

    /// La palabra `k` (0..4) de la fila `f` del cbuffer.
    pub fn fila(&self, f: usize, k: usize) -> u32 {
        u32_(self.filas, 16 * f + 4 * k)
    }
}

/// **Leer** una receta de computo, y comprobar lo que se puede sin el
/// proceso (ver el modulo).
pub fn leer(b: &[u8]) -> Result<Receta<'_>, NoReceta> {
    if b.len() < CABECERA || b[..4] != MAGIA || u32_(b, 4) as usize != b.len() {
        return Err(NoReceta::Cabecera);
    }
    let tabla = |n_o: usize, paso: usize, techo: usize| -> Result<&[u8], NoReceta> {
        let (n, desde) = (u32_(b, n_o) as usize, u32_(b, n_o + 4) as usize);
        if n > techo {
            return Err(NoReceta::Techo);
        }
        let hasta = desde.checked_add(n * paso).ok_or(NoReceta::Tabla)?;
        if n > 0 && (desde < CABECERA || hasta > b.len() || desde % paso.min(16) != 0) {
            return Err(NoReceta::Tabla);
        }
        Ok(if n == 0 { &b[..0] } else { &b[desde..hasta] })
    };
    let codigo = tabla(40, 16, MAX_INSTRUCCIONES)?;
    let precargas = tabla(48, 4, MAX_PRECARGAS)?;
    let buferes = tabla(56, 24, MAX_BUFERES)?;
    let filas = tabla(64, 16, MAX_FILAS)?;
    let despacho = Despacho { grupos: [u32_(b, 8), u32_(b, 12), u32_(b, 16)], hilos: [u32_(b, 20), u32_(b, 24), u32_(b, 28)], compartida: u32_(b, 32), registros: u32_(b, 36) };
    // El QMD dice que no a lo que no se puede (los techos de D3D12).
    super::computo_app::qmd(0, &despacho, 0, 0).map_err(NoReceta::Despacho)?;
    let r = Receta { despacho, codigo, precargas, buferes, filas };
    if r.instrucciones() == 0 {
        return Err(NoReceta::Despacho(super::computo_app::NoDespacha::Cuerpo));
    }
    for k in 0..r.n_buferes() {
        let x = r.bufer(k);
        let repetida = (0..k).any(|j| r.bufer(j).ranura == x.ranura);
        if x.bytes < 4 || x.va % 4 != 0 || x.va.checked_add(x.bytes as u64).is_none() || repetida || buferes[24 * k + 21..24 * k + 24] != [0; 3] {
            return Err(NoReceta::Bufer(k));
        }
    }
    let regs = r.despacho.registros;
    for k in 0..r.n_precargas() {
        let x = &precargas[4 * k..4 * k + 4];
        let bien = match x[0] {
            1 => (u16::from_le_bytes([x[1], x[2]]) as usize) < r.n_filas() && x[3] as u32 + 4 <= regs,
            5 => match (r.bufer_de(x[1]), DelUav::de(x[2])) {
                (Some(_), Some(DelUav::Base)) => x[3] % 2 == 0 && x[3] as u32 + 2 <= regs,
                (Some(_), Some(_)) => (x[3] as u32) < regs,
                _ => false,
            },
            _ => false,
        };
        if !bien {
            return Err(NoReceta::Precarga(k));
        }
    }
    Ok(r)
}

/// **Escribir** una receta (lo que hace PROTON-X; aqui, para las pruebas y
/// para quien la arme): `precargas` como en la receta, `buferes` y `filas`.
#[allow(clippy::too_many_arguments)]
pub fn escribir(out: &mut [u8], d: &Despacho, codigo: &[(u64, u64)], precargas: &[[u8; 4]], buferes: &[Bufer], filas: &[[u32; 4]]) -> Option<usize> {
    let (n_c, n_p, n_b, n_f) = (codigo.len(), precargas.len(), buferes.len(), filas.len());
    let o_c = CABECERA.next_multiple_of(16);
    let o_f = o_c + 16 * n_c;
    let o_b = o_f + 16 * n_f;
    let o_p = o_b + 24 * n_b;
    let total = o_p + 4 * n_p;
    if out.len() < total {
        return None;
    }
    out[..total].fill(0);
    let mut w = |o: usize, v: u32| out[o..o + 4].copy_from_slice(&v.to_le_bytes());
    for (k, v) in [u32::from_le_bytes(MAGIA), total as u32, d.grupos[0], d.grupos[1], d.grupos[2], d.hilos[0], d.hilos[1], d.hilos[2], d.compartida, d.registros, n_c as u32, o_c as u32, n_p as u32, o_p as u32, n_b as u32, o_b as u32, n_f as u32, o_f as u32].into_iter().enumerate() {
        w(4 * k, v);
    }
    for (k, &(lo, hi)) in codigo.iter().enumerate() {
        out[o_c + 16 * k..o_c + 16 * k + 8].copy_from_slice(&lo.to_le_bytes());
        out[o_c + 16 * k + 8..o_c + 16 * k + 16].copy_from_slice(&hi.to_le_bytes());
    }
    for (k, f) in filas.iter().enumerate() {
        for (j, v) in f.iter().enumerate() {
            out[o_f + 16 * k + 4 * j..o_f + 16 * k + 4 * j + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    for (k, x) in buferes.iter().enumerate() {
        let o = o_b + 24 * k;
        out[o..o + 8].copy_from_slice(&x.va.to_le_bytes());
        out[o + 8..o + 12].copy_from_slice(&x.bytes.to_le_bytes());
        out[o + 12..o + 16].copy_from_slice(&x.elementos.to_le_bytes());
        out[o + 16..o + 20].copy_from_slice(&x.paso.to_le_bytes());
        out[o + 20] = x.ranura;
    }
    for (k, p) in precargas.iter().enumerate() {
        out[o_p + 4 * k..o_p + 4 * k + 4].copy_from_slice(p);
    }
    Some(total)
}

/// **Los valores de las precargas**: `(registro, valor)` de cada una, con la
/// VA que el KERNEL le dio a cada bufer en la 3060 (`va_3060(k)`) y lo que
/// presto (`bytes`). Una fila son cuatro; una base, dos (el par). Devuelve
/// cuantos pone en `out`, o `None` si no caben o un bufer no se presta.
pub fn valores(r: &Receta, va_3060: impl Fn(usize) -> u64, out: &mut [(u8, u32)]) -> Option<usize> {
    let mut n = 0;
    let mut poner = |reg: u8, v: u32| -> Option<()> {
        *out.get_mut(n)? = (reg, v);
        n += 1;
        Some(())
    };
    for k in 0..r.n_precargas() {
        match r.precarga(k) {
            Precarga::Fila { fila, reg } => {
                for j in 0..4 {
                    poner(reg + j as u8, r.fila(fila as usize, j))?;
                }
            }
            Precarga::Bufer { k: b, que, reg } => {
                let x = r.bufer(b);
                let v = que.valor(va_3060(b), x.bytes, x.elementos, x.paso)?;
                poner(reg, v as u32)?;
                if que == DelUav::Base {
                    poner(reg + 1, (v >> 32) as u32)?;
                }
            }
        }
    }
    Some(n)
}

/// **Lo que el juez le deja al cuerpo** de esta receta: sus ids y su
/// barrera, la base y el tope de cada bufer (un SRV, de solo leer) y los
/// bytes de su compartida.
pub fn permisos(r: &Receta) -> Option<crate::sass::juez::Permisos> {
    use crate::sass::juez_computo::UavsDeApp;
    let mut uavs = UavsDeApp::default();
    for k in 0..r.n_precargas() {
        if let Precarga::Bufer { k: b, que: DelUav::Base, reg } = r.precarga(k) {
            let tope = (0..r.n_precargas()).find_map(|j| match r.precarga(j) {
                Precarga::Bufer { k: b2, que: DelUav::Tope, reg } if b2 == b => Some(reg),
                _ => None,
            })?;
            let n = uavs.n as usize;
            if n == crate::sass::juez_computo::MAX_UAVS {
                return None;
            }
            uavs.base[n] = reg;
            uavs.tope[n] = tope;
            uavs.solo_lee |= (r.bufer(b).solo_lee() as u8) << n;
            uavs.n += 1;
        }
    }
    Some(crate::sass::juez::Permisos { computo: true, uavs, compartida: r.despacho.compartida, ..Default::default() })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::trabajos::computo_app::NoDespacha;

    fn d() -> Despacho {
        Despacho { grupos: [2, 1, 1], hilos: [64, 1, 1], compartida: 256, registros: 16 }
    }

    fn una(out: &mut [u8], precargas: &[[u8; 4]], buferes: &[Bufer]) -> usize {
        let codigo = [(0x794d_u64, 0x000f_ea00_0380_0000_u64)];
        escribir(out, &d(), &codigo, precargas, buferes, &[[1, 2, 3, 4]]).unwrap()
    }

    const UAV: Bufer = Bufer { va: 0x7000_0000, bytes: 4096, elementos: 1024, paso: 0, ranura: 0 };
    const SRV_0: Bufer = Bufer { va: 0x7100_0000, bytes: 64, elementos: 2, paso: 32, ranura: SRV };

    #[test]
    fn se_escribe_y_se_lee_igual() {
        let mut b = [0u8; 512];
        let n = una(&mut b, &[[5, 0, 0, 4], [5, 0, 1, 6], [5, SRV, 0, 8], [5, SRV, 1, 10], [1, 0, 0, 12]], &[UAV, SRV_0]);
        let r = leer(&b[..n]).unwrap();
        assert_eq!(r.despacho, d());
        assert_eq!((r.instrucciones(), r.n_precargas(), r.n_buferes(), r.n_filas()), (1, 5, 2, 1));
        assert_eq!(r.bufer(1), SRV_0);
        assert_eq!(r.precarga(2), Precarga::Bufer { k: 1, que: DelUav::Base, reg: 8 });
        // Los valores, con la VA de la 3060 de cada bufer.
        let mut v = [(0u8, 0u32); 16];
        let k = valores(&r, |b| 0x2_4000_0000 + 0x10_0000 * b as u64, &mut v).unwrap();
        assert_eq!(&v[..k], &[(4, 0x4000_0000), (5, 2), (6, 4092), (8, 0x4010_0000), (9, 2), (10, 60), (12, 1), (13, 2), (14, 3), (15, 4)]);
        let p = permisos(&r).unwrap();
        assert_eq!((p.uavs.n, p.uavs.base[..2].to_vec(), p.uavs.tope[..2].to_vec(), p.uavs.solo_lee), (2, vec![4, 8], vec![6, 10], 0b10));
        assert_eq!(p.compartida, 256);
    }

    extern crate std;
    use std::vec;

    #[test]
    fn lo_que_no_se_sostiene_dice_que_no() {
        let mut b = [0u8; 512];
        let n = una(&mut b, &[[5, 0, 0, 4]], &[UAV]);
        assert!(leer(&b[..n]).is_ok());
        assert_eq!(leer(&b[..n - 1]).unwrap_err(), NoReceta::Cabecera);
        let mut x = b;
        x[0] = b'X';
        assert_eq!(leer(&x[..n]).unwrap_err(), NoReceta::Cabecera);
        // La base en un registro impar, o fuera de los del cuerpo.
        let n2 = una(&mut x, &[[5, 0, 0, 5]], &[UAV]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Precarga(0));
        let n2 = una(&mut x, &[[5, 0, 1, 16]], &[UAV]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Precarga(0));
        // Un bufer que no hay, una fila que no hay, un `que` que no existe.
        let n2 = una(&mut x, &[[5, 3, 0, 4]], &[UAV]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Precarga(0));
        let n2 = una(&mut x, &[[1, 1, 0, 4]], &[UAV]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Precarga(0));
        let n2 = una(&mut x, &[[5, 0, 9, 4]], &[UAV]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Precarga(0));
        // Dos buferes con la misma ranura; uno sin bytes; la VA sin alinear.
        let n2 = una(&mut x, &[], &[UAV, UAV]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Bufer(1));
        let n2 = una(&mut x, &[], &[Bufer { bytes: 0, ..UAV }]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Bufer(0));
        let n2 = una(&mut x, &[], &[Bufer { va: 2, ..UAV }]);
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Bufer(0));
        // Un despacho que no se puede.
        let mut d2 = d();
        d2.hilos = [2048, 1, 1];
        let n2 = escribir(&mut x, &d2, &[(0x794d, 0)], &[], &[], &[]).unwrap();
        assert_eq!(leer(&x[..n2]).unwrap_err(), NoReceta::Despacho(NoDespacha::Hilos));
        // Una tabla que se sale.
        let mut y = b;
        y[44..48].copy_from_slice(&4096u32.to_le_bytes());
        assert_eq!(leer(&y[..n]).unwrap_err(), NoReceta::Tabla);
        assert!(!NoReceta::Tabla.texto().is_empty());
        let _ = vec![0u8];
    }
}
