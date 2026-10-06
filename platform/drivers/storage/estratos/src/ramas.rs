//! **LA TABLA DE RAMAS** -- la punta de cada rama, FUERA de la historia
//! (`docs/plan/PLAN_LAS_RAMAS.md`, D5 (a); el formato en `ESTRATOS.md`, "la
//! tabla de ramas").
//!
//! [carril]  VERDE     un bloque dentro, un bloque fuera: no toca el disco
//! [consumo] NADA      corre cuando alguien crea, cambia o quita una rama
//!
//! ```text
//!    0..8     "BMORAMAS"
//!    8..12    cuantas (u32)
//!   12..16    cual es la ACTUAL (u32)
//!   16..      por rama, 112 bytes: largo del nombre (1), nombre (63, Latin-1),
//!             punta (BlockPtr, 48)
//! ```
//!
//! ** La punta de la rama ACTUAL no se guarda aqui: es `estrato` del
//! superbloque, que cada commit mueve sin saber que hay ramas. Por eso su fila
//! va con la punta a NULO, y cambiar de rama es dejar la punta de ahora en su
//! fila y sacar la de la otra.

use crate::objects::{BlockPtr, BLOQUE, NOMBRE_MAX, PTR_LEN};
use crate::FormatError;

pub const MAGIA: [u8; 8] = *b"BMORAMAS";
const CABEZA: usize = 16;
const FILA: usize = 1 + NOMBRE_MAX + PTR_LEN; // 112
/// Cuantas ramas caben en el bloque de la tabla.
pub const RAMAS_MAX: usize = (BLOQUE - CABEZA) / FILA;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamaError {
    /// Ya hay una rama con ese nombre (sin mirar mayusculas).
    YaEsta,
    /// No hay rama con ese nombre.
    NoEsta,
    /// La tabla esta llena.
    Llena,
    /// Un nombre vacio o de mas de 63 bytes.
    MalNombre,
    /// La rama ACTUAL no se quita: primero se cambia a otra.
    EsLaActual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rama {
    nombre: [u8; NOMBRE_MAX],
    largo: usize,
    punta: BlockPtr,
}

impl Rama {
    const VACIA: Rama = Rama { nombre: [0; NOMBRE_MAX], largo: 0, punta: BlockPtr::NULO };

    fn nombre(&self) -> &[u8] {
        &self.nombre[..self.largo]
    }
}

/// La tabla, en memoria: sin `alloc`, como la del disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ramas {
    filas: [Rama; RAMAS_MAX],
    n: usize,
    actual: usize,
}

fn baja(c: u8) -> u8 {
    crate::objects::baja(c)
}

fn mismo(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| baja(x) == baja(y))
}

impl Ramas {
    /// Sin ninguna rama: lo que llena un `static` antes de cargar una tabla
    /// (el kernel no la quiere en la pila: son ~4 KiB). No es una tabla que
    /// se pueda escribir: `encode` de esta no la acepta `decode`.
    pub const VACIA: Ramas = Ramas { filas: [Rama::VACIA; RAMAS_MAX], n: 0, actual: 0 };

    /// Una tabla nueva con UNA rama, la actual (la punta es la de siempre).
    pub fn nueva(actual: &[u8]) -> Result<Self, RamaError> {
        let mut t = Ramas { filas: [Rama::VACIA; RAMAS_MAX], n: 0, actual: 0 };
        t.meter(actual, BlockPtr::NULO)?;
        Ok(t)
    }

    fn meter(&mut self, nombre: &[u8], punta: BlockPtr) -> Result<(), RamaError> {
        if nombre.is_empty() || nombre.len() > NOMBRE_MAX {
            return Err(RamaError::MalNombre);
        }
        if self.donde(nombre).is_some() {
            return Err(RamaError::YaEsta);
        }
        if self.n == RAMAS_MAX {
            return Err(RamaError::Llena);
        }
        let mut f = Rama::VACIA;
        f.nombre[..nombre.len()].copy_from_slice(nombre);
        f.largo = nombre.len();
        f.punta = punta;
        self.filas[self.n] = f;
        self.n += 1;
        Ok(())
    }

    fn donde(&self, nombre: &[u8]) -> Option<usize> {
        self.filas[..self.n].iter().position(|f| mismo(f.nombre(), nombre))
    }

    pub fn cuantas(&self) -> usize {
        self.n
    }

    /// El nombre de la rama `i`, y si es la actual.
    pub fn rama(&self, i: usize) -> Option<(&[u8], bool)> {
        self.filas[..self.n].get(i).map(|f| (f.nombre(), i == self.actual))
    }

    pub fn actual(&self) -> &[u8] {
        self.filas[self.actual].nombre()
    }

    /// **Una rama nueva** que empieza en `punta` (un estrato que ya esta: una
    /// version marcada, o la punta de ahora). No cambia de rama.
    pub fn crear(&mut self, nombre: &[u8], punta: BlockPtr) -> Result<(), RamaError> {
        self.meter(nombre, punta)
    }

    /// La punta de una rama. La de la actual es la que el superbloque dice.
    pub fn punta(&self, nombre: &[u8], la_de_ahora: BlockPtr) -> Result<BlockPtr, RamaError> {
        let i = self.donde(nombre).ok_or(RamaError::NoEsta)?;
        Ok(if i == self.actual { la_de_ahora } else { self.filas[i].punta })
    }

    /// **CAMBIAR de rama**: la punta de ahora se queda en la fila de la
    /// actual, y sale la punta de `nombre`, que es la que el superbloque pasa
    /// a seguir. Nadie se vuelve antepasado de nadie.
    pub fn cambiar(&mut self, nombre: &[u8], la_de_ahora: BlockPtr) -> Result<BlockPtr, RamaError> {
        let i = self.donde(nombre).ok_or(RamaError::NoEsta)?;
        if i == self.actual {
            return Ok(la_de_ahora);
        }
        self.filas[self.actual].punta = la_de_ahora;
        let nueva = self.filas[i].punta;
        self.filas[i].punta = BlockPtr::NULO;
        self.actual = i;
        Ok(nueva)
    }

    /// Quitar una rama: deja de NOMBRARLA. Sus estratos siguen en el disco (y
    /// las versiones marcadas, para siempre); solo se pierde la entrada.
    pub fn quitar(&mut self, nombre: &[u8]) -> Result<(), RamaError> {
        let i = self.donde(nombre).ok_or(RamaError::NoEsta)?;
        if i == self.actual {
            return Err(RamaError::EsLaActual);
        }
        self.filas.copy_within(i + 1..self.n, i);
        self.n -= 1;
        self.filas[self.n] = Rama::VACIA;
        if self.actual > i {
            self.actual -= 1;
        }
        Ok(())
    }

    pub fn encode(&self) -> [u8; BLOQUE] {
        let mut b = [0u8; BLOQUE];
        b[..8].copy_from_slice(&MAGIA);
        b[8..12].copy_from_slice(&(self.n as u32).to_le_bytes());
        b[12..16].copy_from_slice(&(self.actual as u32).to_le_bytes());
        for (k, f) in self.filas[..self.n].iter().enumerate() {
            let o = CABEZA + k * FILA;
            b[o] = f.largo as u8;
            b[o + 1..o + 1 + NOMBRE_MAX].copy_from_slice(&f.nombre);
            b[o + 1 + NOMBRE_MAX..o + FILA].copy_from_slice(&f.punta.encode());
        }
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < CABEZA || b[..8] != MAGIA {
            return Err(FormatError::BadMagic);
        }
        let n = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
        let actual = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
        if n == 0 || n > RAMAS_MAX || actual >= n || b.len() < CABEZA + n * FILA {
            return Err(FormatError::BadField);
        }
        let mut t = Ramas { filas: [Rama::VACIA; RAMAS_MAX], n, actual };
        for k in 0..n {
            let o = CABEZA + k * FILA;
            let largo = b[o] as usize;
            if largo == 0 || largo > NOMBRE_MAX {
                return Err(FormatError::BadField);
            }
            t.filas[k].nombre.copy_from_slice(&b[o + 1..o + 1 + NOMBRE_MAX]);
            t.filas[k].largo = largo;
            t.filas[k].punta = BlockPtr::decode(&b[o + 1 + NOMBRE_MAX..o + FILA])?;
        }
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: u8) -> BlockPtr {
        BlockPtr { lba: n as u64, off: 0, len: 224, hash: [n; 32] }
    }

    #[test]
    fn cambiar_de_rama_deja_la_punta_y_saca_la_otra() {
        let mut t = Ramas::nueva(b"principal").unwrap();
        t.crear(b"pruebas", p(1)).unwrap();
        // En principal, la punta de ahora es 5: cambiar a pruebas saca 1.
        assert_eq!(t.cambiar(b"pruebas", p(5)).unwrap(), p(1));
        assert_eq!(t.actual(), b"pruebas");
        // La de principal se quedo guardada; la de la actual es la del superbloque.
        assert_eq!(t.punta(b"principal", p(9)).unwrap(), p(5));
        assert_eq!(t.punta(b"PRUEBAS", p(9)).unwrap(), p(9));
        // Y vuelta: la de pruebas (9) se guarda, sale la de principal (5).
        assert_eq!(t.cambiar(b"principal", p(9)).unwrap(), p(5));
        assert_eq!(t.punta(b"pruebas", p(0)).unwrap(), p(9));
    }

    #[test]
    fn la_tabla_va_y_vuelve_por_su_bloque() {
        let mut t = Ramas::nueva(b"principal").unwrap();
        t.crear(b"a\xF1o", p(3)).unwrap();
        t.crear(b"otra", p(4)).unwrap();
        t.cambiar(b"otra", p(8)).unwrap();
        let d = Ramas::decode(&t.encode()).unwrap();
        assert_eq!(d, t);
        assert_eq!(d.rama(1), Some((&b"a\xF1o"[..], false)));
        assert_eq!(d.actual(), b"otra");
    }

    #[test]
    fn lo_que_no_se_puede_se_dice() {
        let mut t = Ramas::nueva(b"principal").unwrap();
        assert_eq!(t.crear(b"PRINCIPAL", p(1)), Err(RamaError::YaEsta));
        assert_eq!(t.crear(b"", p(1)), Err(RamaError::MalNombre));
        assert_eq!(t.cambiar(b"nadie", p(1)), Err(RamaError::NoEsta));
        assert_eq!(t.quitar(b"principal"), Err(RamaError::EsLaActual));
        for k in 1..RAMAS_MAX {
            t.crear(format_args_nombre(k).as_slice(), p(1)).unwrap();
        }
        assert_eq!(t.crear(b"una mas", p(1)), Err(RamaError::Llena));
        // Quitar una deja sitio, y la actual sigue siendo la misma.
        t.quitar(b"r01").unwrap();
        assert_eq!((t.cuantas(), t.actual()), (RAMAS_MAX - 1, &b"principal"[..]));
        assert!(Ramas::decode(&[0u8; BLOQUE]).is_err());
    }

    fn format_args_nombre(k: usize) -> [u8; 3] {
        [b'r', b'0' + (k / 10) as u8, b'0' + (k % 10) as u8]
    }
}
