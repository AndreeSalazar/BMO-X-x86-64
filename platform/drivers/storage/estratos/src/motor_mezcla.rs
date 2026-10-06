//! **EL MOTOR DE LA MEZCLA, SIN `alloc`** (`docs/plan/PLAN_LAS_RAMAS.md`, R4c-2).
//!
//! [carril]  AMARILLO  lee el disco y escribe bloques NUEVOS al final del log; el
//!                     commit no es suyo
//! [consumo] NADA      corre cuando alguien pide mezclar
//!
//! La mezcla POR CARPETAS de `estratos-mezcla` (`decide::por_arbol`), hecha
//! para que la pueda correr el KERNEL, que no tiene memoria dinamica:
//!
//! ```text
//!    las listas      tres tablas FIJAS por nivel (base, A, B) y una de
//!                    salida, que presta quien llama: el kernel en `static`,
//!                    el anfitrion en un `Vec`. Una carpeta con mas de CAP
//!                    entradas por lado se DICE (`Fallo::CarpetaGrande`)
//!    dos pasadas     CONTAR: cuantos bloques hay que reservar y cuantos
//!                    choques hay, sin escribir nada. ESCRIBIR: lo mismo,
//!                    poniendo los bloques desde donde diga la reserva. Las
//!                    dos deciden igual porque `elegir` contesta igual
//!    lo que escribe  SOLO carpetas: los ficheros son los nodos que ya estan
//!                    (D4). Una carpeta que acaba igual que en A se apunta
//! ```
//!
//! La regla es `mezcla::regla`, la misma de la mezcla plana y de la de
//! carpetas; y el hash de una carpeta es un ATAJO, no la verdad: si no
//! coincide se baja, y un lado que no la tiene cuenta como vacia.
//!
//! ** Quien lo prueba: `toolchain/tools/estratos-mezcla`, que corre ESTE motor
//! sobre imagenes y lo compara con la mezcla de carpetas con `Vec` y con la
//! plana, al azar. Un solo recorrido para todo el sistema, como `read.rs`.

use crate::carpeta;
use crate::flujo::{plan_de, Arbol};
use crate::mezcla::{regla, Regla};
use crate::objects::{Attr, BlockPtr, Entrada, Nodo, Tipo, ATTR_ENTRADAS, BLOQUE, ENTRADA_LEN, NIVELES_MAX};
use crate::read::Fuente;
use crate::FormatError;

/// Entradas por carpeta y por lado. Las mismas 64 que el kernel ya lista de
/// una vez (`fsys/estratos/dir.rs`, `MAX_ENTRIES`).
pub const CAP: usize = 64;
/// La salida junta A y B: hasta el doble.
pub const SALIDA_MAX: usize = 2 * CAP;
/// Lo que cabe de una ruta de choque.
pub const RUTA_MAX: usize = 255;

/// Lo que elige una persona ante un choque (D3). Un nodo ENTERO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eleccion {
    A,
    B,
    Quitar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallo {
    /// Algo del disco no cuadra o no se pudo leer.
    Formato(FormatError),
    /// Una carpeta con mas de [`CAP`] entradas en un lado.
    CarpetaGrande,
    /// Mas niveles de carpetas que tablas prestadas.
    MuyHondo,
    /// Una ruta de choque mas larga que [`RUTA_MAX`].
    RutaLarga,
    /// `poner` dijo que no.
    NoEscribio,
}

impl From<FormatError> for Fallo {
    fn from(e: FormatError) -> Self {
        Fallo::Formato(e)
    }
}

/// Las tablas de UN nivel: lo que cada lado tiene en esa carpeta, y lo que sale.
#[derive(Clone)]
pub struct Nivel {
    base: [Entrada; CAP],
    nb: usize,
    a: [Entrada; CAP],
    na: usize,
    b: [Entrada; CAP],
    nx: usize,
    out: [Entrada; SALIDA_MAX],
    no: usize,
}

impl Nivel {
    /// Todo a cero: cada campo es un entero o bytes, asi que una tabla a cero
    /// ES esta. El kernel la saca de marcos limpios (`fsys/estratos/ramas.rs`).
    pub const VACIO: Nivel = Nivel {
        base: [Entrada::VACIA; CAP],
        nb: 0,
        a: [Entrada::VACIA; CAP],
        na: 0,
        b: [Entrada::VACIA; CAP],
        nx: 0,
        out: [Entrada::VACIA; SALIDA_MAX],
        no: 0,
    };
}

/// Lo que el motor necesita prestado. Nada se pide durante la mezcla.
pub struct Taller<'t> {
    /// Uno por nivel de carpetas: la hondura maxima es su largo.
    pub niveles: &'t mut [Nivel],
    /// Para bajar por las listas de entradas (`NIVELES_MAX + 1` filas).
    pub scratch: &'t mut [[u8; BLOQUE]],
    /// Para escribir la lista de una carpeta (`NIVELES_MAX` filas).
    pub indice: &'t mut [[u8; BLOQUE]],
    /// Un bloque de trabajo: leer un nodo, juntar la lista que se escribe.
    pub bloque: &'t mut [u8; BLOQUE],
    /// La ruta del choque, para preguntarle a quien elige.
    pub ruta: &'t mut [u8; RUTA_MAX],
}

/// Lo que dice CONTAR, antes de reservar un solo bloque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cuenta {
    pub bloques: u64,
    pub choques: u32,
}

/// Que salio de una carpeta.
#[derive(Clone, Copy)]
enum Hecho {
    /// Igual que la de A: se apunta la de A.
    Igual(BlockPtr),
    /// Escrita nueva (en CONTAR, el puntero no vale: solo cuenta).
    Nueva(BlockPtr),
    /// Vacia: se va.
    Vacia,
}

type Elegir<'e> = dyn FnMut(&[u8], Option<&BlockPtr>, Option<&BlockPtr>) -> Eleccion + 'e;

enum Fase<'p> {
    Contar(Cuenta),
    Escribir { cursor: u64, poner: &'p mut dyn FnMut(u64, &[u8]) -> bool },
}

struct Ctx<'c, 'p> {
    src: &'c mut dyn Fuente,
    scratch: &'c mut [[u8; BLOQUE]],
    indice: &'c mut [[u8; BLOQUE]],
    bloque: &'c mut [u8; BLOQUE],
    ruta: &'c mut [u8; RUTA_MAX],
    ruta_len: usize,
    elegir: &'c mut Elegir<'c>,
    fase: Fase<'p>,
}

/// **CONTAR**: cuantos bloques hay que reservar para mezclar `b` en `a` sobre
/// `base`, y cuantos choques le van a preguntar a `elegir`. No escribe nada.
pub fn contar(src: &mut dyn Fuente, t: &mut Taller, base: BlockPtr, a: BlockPtr, b: BlockPtr, elegir: &mut Elegir) -> Result<Cuenta, Fallo> {
    let mut c = Ctx {
        src,
        scratch: t.scratch,
        indice: t.indice,
        bloque: t.bloque,
        ruta: t.ruta,
        ruta_len: 0,
        elegir,
        fase: Fase::Contar(Cuenta::default()),
    };
    let raiz = c.carpeta(t.niveles, Some(base), Some(a), Some(b))?;
    let Fase::Contar(mut cuenta) = c.fase else { unreachable!("contar cuenta") };
    if matches!(raiz, Hecho::Vacia) {
        cuenta.bloques += 1; // la raiz vacia es un nodo
    }
    Ok(cuenta)
}

/// **ESCRIBIR**: la misma mezcla, poniendo los bloques desde `desde`. Devuelve
/// la RAIZ que sale y hasta donde escribio. `elegir` tiene que contestar lo
/// mismo que en [`contar`]: si no, lo escrito no es lo reservado.
pub fn escribir(
    src: &mut dyn Fuente,
    t: &mut Taller,
    base: BlockPtr,
    a: BlockPtr,
    b: BlockPtr,
    elegir: &mut Elegir,
    desde: u64,
    poner: &mut dyn FnMut(u64, &[u8]) -> bool,
) -> Result<(BlockPtr, u64), Fallo> {
    let mut c = Ctx {
        src,
        scratch: t.scratch,
        indice: t.indice,
        bloque: t.bloque,
        ruta: t.ruta,
        ruta_len: 0,
        elegir,
        fase: Fase::Escribir { cursor: desde, poner },
    };
    let raiz = c.carpeta(t.niveles, Some(base), Some(a), Some(b))?;
    let Fase::Escribir { mut cursor, poner } = c.fase else { unreachable!("escribir escribe") };
    let p = match raiz {
        Hecho::Igual(p) | Hecho::Nueva(p) => p,
        Hecho::Vacia => {
            let n = crate::escritura::nodo_de_directorio_vacio();
            if !poner(cursor, &n) {
                return Err(Fallo::NoEscribio);
            }
            cursor += 1;
            BlockPtr::nuevo(cursor - 1, 0, &n)
        }
    };
    Ok((p, cursor))
}

fn buscar<'l>(lista: &'l [Entrada], nombre: &[u8]) -> Option<&'l Entrada> {
    let baja = |c: u8| if c.is_ascii_uppercase() || ((0xC0..=0xDE).contains(&c) && c != 0xD7) { c + 32 } else { c };
    lista
        .iter()
        .find(|e| e.nombre_bytes().len() == nombre.len() && e.nombre_bytes().iter().zip(nombre).all(|(&x, &y)| baja(x) == baja(y)))
}

impl Ctx<'_, '_> {
    fn nodo(&mut self, p: &BlockPtr) -> Result<Nodo, Fallo> {
        if !self.src.bloque(p.lba, self.bloque) {
            return Err(FormatError::Io.into());
        }
        let (i, f) = (p.off as usize, p.off as usize + p.len as usize);
        let d = self.bloque.get(i..f).ok_or(FormatError::BadField)?;
        if !p.verifica(d) {
            return Err(FormatError::BadChecksum.into());
        }
        Ok(Nodo::decode(d)?)
    }

    fn es_carpeta(&mut self, p: &BlockPtr) -> Result<bool, Fallo> {
        Ok(self.nodo(p)?.tipo == Tipo::Directorio)
    }

    /// Las entradas de la carpeta `p` en `tabla`; `None` (o un fichero) es vacia.
    fn cargar(&mut self, p: Option<BlockPtr>, tabla: &mut [Entrada; CAP]) -> Result<usize, Fallo> {
        let Some(p) = p else { return Ok(0) };
        let n = self.nodo(&p)?;
        if n.tipo != Tipo::Directorio {
            return Ok(0);
        }
        let (mut k, mut grande, mut mala) = (0usize, false, false);
        carpeta::recorrer(self.src, n.attr(ATTR_ENTRADAS), self.scratch, &mut |e| {
            if k == CAP {
                grande = true;
                return false;
            }
            match Entrada::decode(e) {
                Ok(e) => {
                    tabla[k] = e;
                    k += 1;
                    true
                }
                Err(_) => {
                    mala = true;
                    false
                }
            }
        })?;
        if grande {
            return Err(Fallo::CarpetaGrande);
        }
        if mala {
            return Err(FormatError::BadField.into());
        }
        Ok(k)
    }

    fn entrar(&mut self, nombre: &[u8]) -> Result<usize, Fallo> {
        let antes = self.ruta_len;
        let sep = usize::from(antes > 0);
        if antes + sep + nombre.len() > RUTA_MAX {
            return Err(Fallo::RutaLarga);
        }
        if sep == 1 {
            self.ruta[antes] = b'/';
        }
        self.ruta[antes + sep..antes + sep + nombre.len()].copy_from_slice(nombre);
        self.ruta_len = antes + sep + nombre.len();
        Ok(antes)
    }

    /// Mezcla UNA carpeta (y baja donde haga falta) con las tablas `niveles[0]`.
    fn carpeta(&mut self, niveles: &mut [Nivel], base: Option<BlockPtr>, a: Option<BlockPtr>, b: Option<BlockPtr>) -> Result<Hecho, Fallo> {
        let Some((n, abajo)) = niveles.split_first_mut() else { return Err(Fallo::MuyHondo) };
        n.nb = self.cargar(base, &mut n.base)?;
        n.na = self.cargar(a, &mut n.a)?;
        n.nx = self.cargar(b, &mut n.b)?;
        n.no = 0;
        // Primero los nombres de A, despues los que solo tiene B: el orden de
        // la mezcla plana y el de `por_carpeta`.
        for i in 0..n.na + n.nx {
            let (nombre, de_b) = if i < n.na { (n.a[i], false) } else { (n.b[i - n.na], true) };
            let nombre = nombre.nombre_bytes();
            if de_b && buscar(&n.a[..n.na], nombre).is_some() {
                continue;
            }
            let e0 = buscar(&n.base[..n.nb], nombre).copied();
            let ea = buscar(&n.a[..n.na], nombre).copied();
            let eb = buscar(&n.b[..n.nx], nombre).copied();
            let sale = match regla(e0.map(|e| e.nodo.hash), ea.map(|e| e.nodo.hash), eb.map(|e| e.nodo.hash)) {
                Regla::A => ea,
                Regla::B => eb,
                Regla::Distintos => {
                    let mut bajar = ea.is_some() || eb.is_some();
                    for e in [e0, ea, eb].into_iter().flatten() {
                        bajar = bajar && self.es_carpeta(&e.nodo)?;
                    }
                    let quien = ea.or(eb).expect("Distintos trae un lado");
                    let antes = self.entrar(quien.nombre_bytes())?;
                    let r = if bajar {
                        match self.carpeta(abajo, e0.map(|e| e.nodo), ea.map(|e| e.nodo), eb.map(|e| e.nodo))? {
                            Hecho::Igual(p) | Hecho::Nueva(p) => Some(quien.con_nodo(p)),
                            Hecho::Vacia => None,
                        }
                    } else {
                        if let Fase::Contar(c) = &mut self.fase {
                            c.choques += 1;
                        }
                        let ruta = &self.ruta[..self.ruta_len];
                        match (self.elegir)(ruta, ea.as_ref().map(|e| &e.nodo), eb.as_ref().map(|e| &e.nodo)) {
                            Eleccion::A => ea,
                            Eleccion::B => eb,
                            Eleccion::Quitar => None,
                        }
                    };
                    self.ruta_len = antes;
                    r
                }
            };
            if let Some(e) = sale {
                if n.no == SALIDA_MAX {
                    return Err(Fallo::CarpetaGrande);
                }
                n.out[n.no] = e;
                n.no += 1;
            }
        }
        // Igual que en A: se apunta la de A, y no se escribe nada.
        let igual = n.no == n.na
            && n.out[..n.no].iter().zip(&n.a[..n.na]).all(|(o, x)| o.nombre_bytes() == x.nombre_bytes() && o.nodo == x.nodo);
        if let (true, Some(a)) = (igual, a) {
            return Ok(Hecho::Igual(a));
        }
        if n.no == 0 {
            return Ok(Hecho::Vacia);
        }
        self.publicar_lista(&n.out[..n.no])
    }

    /// Escribe (o cuenta) la lista de una carpeta y su nodo.
    fn publicar_lista(&mut self, lista: &[Entrada]) -> Result<Hecho, Fallo> {
        let bytes = (lista.len() * ENTRADA_LEN) as u64;
        let plan = plan_de(bytes).ok_or(Fallo::CarpetaGrande)?;
        let (cursor, poner) = match &mut self.fase {
            Fase::Contar(c) => {
                c.bloques += plan.total + 1;
                return Ok(Hecho::Nueva(BlockPtr::NULO));
            }
            Fase::Escribir { cursor, poner } => (cursor, poner),
        };
        let indice = &mut self.indice[..plan.niveles as usize];
        let mut arbol = Arbol::nuevo(plan, *cursor, indice)?;
        // La lista, en trozos de un bloque: las entradas pasan por encima de
        // los bordes (112 no divide a 4096), asi que se juntan aqui.
        let mut lleno = 0usize;
        for e in lista {
            let enc = e.encode();
            let mut t = &enc[..];
            while !t.is_empty() {
                let k = (BLOQUE - lleno).min(t.len());
                self.bloque[lleno..lleno + k].copy_from_slice(&t[..k]);
                lleno += k;
                t = &t[k..];
                if lleno == BLOQUE {
                    arbol.empujar(&self.bloque[..], *poner)?;
                    lleno = 0;
                }
            }
        }
        if lleno > 0 {
            arbol.empujar(&self.bloque[..lleno], *poner)?;
        }
        let raiz = arbol.cerrar(*poner)?;
        *cursor += plan.total;
        let a = Attr::en_bloques(ATTR_ENTRADAS, bytes, plan.niveles, raiz)?;
        let nodo = Nodo::nuevo(Tipo::Directorio).con(a)?.encode();
        if !poner(*cursor, &nodo) {
            return Err(Fallo::NoEscribio);
        }
        let p = BlockPtr::nuevo(*cursor, 0, &nodo);
        *cursor += 1;
        Ok(Hecho::Nueva(p))
    }
}

const _: () = assert!(NIVELES_MAX >= 1);

#[cfg(test)]
mod tests {
    use super::*;

    /// ** El kernel presta las tablas de marcos A CERO, sin construirlas: eso
    /// solo vale mientras cero sea `Nivel::VACIO`. Si un campo deja de serlo,
    /// esto lo dice antes que el metal.
    #[test]
    fn una_tabla_a_cero_es_la_vacia() {
        let z: Nivel = unsafe { core::mem::zeroed() };
        let v = Nivel::VACIO;
        assert_eq!((z.nb, z.na, z.nx, z.no), (v.nb, v.na, v.nx, v.no));
        let igual = |a: &Entrada, b: &Entrada| a.nombre_bytes() == b.nombre_bytes() && a.nodo == b.nodo;
        assert!(z.base.iter().chain(&z.a).chain(&z.b).chain(&z.out).all(|e| igual(e, &Entrada::VACIA)));
        assert_eq!(BlockPtr::NULO, unsafe { core::mem::zeroed::<BlockPtr>() });
    }
}
