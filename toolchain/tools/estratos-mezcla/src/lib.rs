//! **MEZCLAR DOS RAMAS DE ESTRATOS, POR NODOS, sobre una imagen** (R4b de
//! `docs/plan/PLAN_LAS_RAMAS.md`).
//!
//! ```text
//!    1  la BASE      el ultimo estrato que las dos ramas comparten, por los
//!                    DOS padres de cada estrato (formato v2)
//!    2  aplanar      base, A (la punta de ahora) y B (la rama que entra):
//!                    cada uno como (ruta -> nodo) de sus hojas
//!    3  decidir      `bmo_estratos::mezcla`, la MISMA que usara el kernel;
//!                    cada choque lo resuelve quien llama (D3: una persona)
//!    4  escribir     SOLO las carpetas nuevas: los ficheros son los nodos
//!                    que ya estan (D4: nacen compartiendo, no se copia un
//!                    byte), y UN estrato con DOS padres (D2)
//!    5  releer       el arbol publicado se aplana otra vez y tiene que ser
//!                    exactamente lo decidido
//! ```
//!
//! El orden de escritura es el de `estratos-put`, que es el unico que no
//! pierde datos: reservar, escribir al final del log, barrera, superbloque
//! alterno, barrera. Un corte antes del commit deja el volumen como estaba.
//!
//! [!] Que NO hace todavia, dicho: no sabe partir un choque por dentro (no
//! debe: los nodos son enteros) y no es un gesto del kernel (eso es R4c, en
//! F: despues de las imagenes).

use std::io::{self, Read, Seek, SeekFrom, Write};

use bmo_estratos as es;
use es::flujo::{plan_de, Arbol};
use es::mezcla;
use es::objects::{Attr, BlockPtr, Entrada, Nodo, Tipo, ATTR_ENTRADAS, BLOQUE, ENTRADA_LEN, NIVELES_MAX};
use es::read::Fuente;
use es::{Autor, Estrato, Superblock, Transaccion, ESTRATO_LEN, SUPER_LEN};

/// E/S de una imagen o de un volumen ya abierto, con su barrera.
pub trait Almacen: Read + Write + Seek {
    fn barrera(&mut self) -> io::Result<()>;
}

impl Almacen for std::fs::File {
    fn barrera(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

/// Una hoja de un arbol: un fichero, o una carpeta VACIA (que tambien es algo
/// que se puede tener o no tener). `ruta` son los nombres en BYTES, con `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hoja {
    pub ruta: Vec<u8>,
    pub nodo: BlockPtr,
}

/// Un choque: los dos lados cambiaron la misma ruta distinto. `None` es "lo
/// quito".
#[derive(Debug, Clone)]
pub struct Choque {
    pub ruta: Vec<u8>,
    pub a: Option<BlockPtr>,
    pub b: Option<BlockPtr>,
}

/// Lo que una persona elige ante un choque. Un nodo ENTERO, nunca lineas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eleccion {
    A,
    B,
    Quitar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resultado {
    pub generacion: u64,
    pub estrato: BlockPtr,
    pub raiz: BlockPtr,
    pub cuenta: mezcla::Cuenta,
    pub bloques_nuevos: u64,
}

// -- leer ------------------------------------------------------------------------

struct Lector<'a, R>(&'a mut R);

impl<R: Read + Seek> Fuente for Lector<'_, R> {
    fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
        let Some(off) = lba.checked_mul(BLOQUE as u64) else { return false };
        self.0.seek(SeekFrom::Start(off)).is_ok() && self.0.read_exact(dst).is_ok()
    }
}

fn leer_bloque<R: Read + Seek>(r: &mut R, lba: u64) -> Result<[u8; BLOQUE], String> {
    let mut b = [0u8; BLOQUE];
    if Lector(r).bloque(lba, &mut b) {
        Ok(b)
    } else {
        Err(format!("no se pudo leer el bloque {lba}"))
    }
}

fn trozo(b: &[u8; BLOQUE], off: u32, len: u32) -> Result<&[u8], String> {
    let (i, f) = (off as usize, off as usize + len as usize);
    b.get(i..f).ok_or_else(|| "un puntero se sale de su bloque".to_string())
}

fn leer_objeto<R: Read + Seek>(r: &mut R, p: &BlockPtr) -> Result<Vec<u8>, String> {
    let b = leer_bloque(r, p.lba)?;
    let d = trozo(&b, p.off, p.len)?.to_vec();
    if !p.verifica(&d) {
        return Err(format!("no cuadra la suma del objeto en el bloque {}", p.lba));
    }
    Ok(d)
}

fn leer_nodo<R: Read + Seek>(r: &mut R, p: &BlockPtr) -> Result<Nodo, String> {
    Nodo::decode(&leer_objeto(r, p)?).map_err(|e| e.name().into())
}

fn leer_flujo<R: Read + Seek>(r: &mut R, a: &Attr) -> Result<Vec<u8>, String> {
    if let Some(d) = a.datos_residentes() {
        return Ok(d.to_vec());
    }
    let raiz = a.raiz().ok_or("atributo sin raiz")?;
    let mut out = Vec::new();
    let mut scratch = [[0u8; BLOQUE]; NIVELES_MAX + 1];
    es::descender(&mut Lector(r), &raiz, a.levels, &mut scratch[..a.levels as usize + 1], &mut |t| {
        out.extend_from_slice(t);
        true
    })
    .map_err(|e| e.name().to_string())?;
    let n = usize::try_from(a.size).map_err(|_| "flujo demasiado grande")?;
    out.truncate(n);
    if out.len() != n {
        return Err("el flujo no mide lo que declara".into());
    }
    Ok(out)
}

fn leer_estrato<R: Read + Seek>(r: &mut R, p: &BlockPtr) -> Result<Estrato, String> {
    Estrato::decode(&leer_objeto(r, p)?).map_err(|e| format!("estrato: {}", e.name()))
}

/// Un segundo padre lleva sitio y HUELLA, no el hash entero (formato v2): se
/// lee el estrato (que trae su propia suma) y se comprueba la huella.
fn segundo_padre<R: Read + Seek>(r: &mut R, s: &es::SegundoPadre) -> Result<BlockPtr, String> {
    let b = leer_bloque(r, s.lba)?;
    let d = trozo(&b, s.off, ESTRATO_LEN as u32)?;
    let p = BlockPtr::nuevo(s.lba, s.off, d);
    if !s.es(&p) {
        return Err("el segundo padre no es el estrato que se apunto (la huella no cuadra)".into());
    }
    Estrato::decode(d).map_err(|e| format!("segundo padre: {}", e.name()))?;
    Ok(p)
}

/// **Las hojas de un arbol**, en el orden de sus carpetas.
pub fn aplanar<R: Read + Seek>(r: &mut R, raiz: &BlockPtr) -> Result<Vec<Hoja>, String> {
    let mut out = Vec::new();
    bajar(r, raiz, &mut Vec::new(), &mut out, 0)?;
    Ok(out)
}

fn bajar<R: Read + Seek>(r: &mut R, p: &BlockPtr, ruta: &mut Vec<u8>, out: &mut Vec<Hoja>, hondo: usize) -> Result<(), String> {
    if hondo > 64 {
        return Err("mas de 64 niveles de carpetas: se para en vez de adivinar".into());
    }
    let nodo = leer_nodo(r, p)?;
    if nodo.tipo != Tipo::Directorio {
        out.push(Hoja { ruta: ruta.clone(), nodo: *p });
        return Ok(());
    }
    let Some(a) = nodo.attr(ATTR_ENTRADAS).copied() else {
        if !ruta.is_empty() {
            out.push(Hoja { ruta: ruta.clone(), nodo: *p });
        }
        return Ok(());
    };
    let lista = leer_flujo(r, &a)?;
    if lista.len() % ENTRADA_LEN != 0 {
        return Err("una lista de entradas que no acaba en una entrada entera".into());
    }
    for e in lista.chunks_exact(ENTRADA_LEN) {
        let e = Entrada::decode(e).map_err(|e| e.name().to_string())?;
        let largo = ruta.len();
        if largo > 0 {
            ruta.push(b'/');
        }
        ruta.extend_from_slice(e.nombre_bytes());
        bajar(r, &e.nodo, ruta, out, hondo + 1)?;
        ruta.truncate(largo);
    }
    Ok(())
}

/// Los antepasados de un estrato, por sus DOS padres, el mas cercano antes.
fn antepasados<R: Read + Seek>(r: &mut R, desde: &BlockPtr, max: usize) -> Result<Vec<BlockPtr>, String> {
    let mut vistos: Vec<BlockPtr> = Vec::new();
    let mut cola = std::collections::VecDeque::from([*desde]);
    while let Some(p) = cola.pop_front() {
        if p.es_nulo() || vistos.iter().any(|v| v.lba == p.lba && v.off == p.off) {
            continue;
        }
        if vistos.len() == max {
            break;
        }
        let e = leer_estrato(r, &p)?;
        vistos.push(p);
        cola.push_back(e.padre);
        if let Some(s) = e.segundo {
            cola.push_back(segundo_padre(r, &s)?);
        }
    }
    Ok(vistos)
}

// -- escribir --------------------------------------------------------------------

fn escribir_bloque<W: Write + Seek>(w: &mut W, lba: u64, d: &[u8]) -> Result<(), String> {
    let mut b = [0u8; BLOQUE];
    b.get_mut(..d.len()).ok_or("objeto mayor que un bloque")?.copy_from_slice(d);
    let off = lba.checked_mul(BLOQUE as u64).ok_or("LBA fuera de rango")?;
    w.seek(SeekFrom::Start(off)).and_then(|_| w.write_all(&b)).map_err(|e| format!("escribiendo el bloque {lba}: {e}"))
}

fn escribir_objeto<W: Write + Seek>(w: &mut W, lba: u64, d: &[u8]) -> Result<BlockPtr, String> {
    escribir_bloque(w, lba, d)?;
    Ok(BlockPtr::nuevo(lba, 0, d))
}

/// Lo que cuelga de una carpeta del arbol que se va a escribir.
#[derive(Debug)]
pub(crate) enum Hijo {
    /// Un nodo que YA esta en el disco: se apunta, no se copia.
    Nodo(BlockPtr),
    /// Un fichero chico nuevo (para preparar imagenes de prueba).
    #[cfg_attr(not(test), allow(dead_code))]
    Contenido(Vec<u8>),
    Carpeta(Carpeta),
}

#[derive(Debug, Default)]
pub(crate) struct Carpeta {
    hijos: Vec<(Vec<u8>, Hijo)>,
}

/// Latin-1 en minusculas, como `Entrada::se_llama`: `Mundo/` y `mundo/` son UNA
/// carpeta en ESTRATOS.
#[cfg_attr(not(test), allow(dead_code))]
fn baja(c: u8) -> u8 {
    if c.is_ascii_uppercase() || ((0xC0..=0xDE).contains(&c) && c != 0xD7) {
        c + 32
    } else {
        c
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn mismo(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| baja(x) == baja(y))
}

impl Carpeta {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn poner(&mut self, ruta: &[u8], hijo: Hijo) -> Result<(), String> {
        let (nombre, resto) = match ruta.iter().position(|&c| c == b'/') {
            Some(i) => (&ruta[..i], Some(&ruta[i + 1..])),
            None => (ruta, None),
        };
        if nombre.is_empty() {
            return Err("una ruta con un nombre vacio".into());
        }
        let ya = self.hijos.iter().position(|(n, _)| mismo(n, nombre));
        match (resto, ya) {
            (None, None) => {
                self.hijos.push((nombre.to_vec(), hijo));
                Ok(())
            }
            (None, Some(_)) => Err(format!("la ruta {} sale dos veces", String::from_utf8_lossy(ruta))),
            (Some(resto), None) => {
                let mut c = Carpeta::default();
                c.poner(resto, hijo)?;
                self.hijos.push((nombre.to_vec(), Hijo::Carpeta(c)));
                Ok(())
            }
            (Some(resto), Some(i)) => match &mut self.hijos[i].1 {
                Hijo::Carpeta(c) => c.poner(resto, hijo),
                _ => Err(format!(
                    "{} es un fichero en un lado y una carpeta en el otro: elige en F1",
                    String::from_utf8_lossy(nombre)
                )),
            },
        }
    }

    /// Cuantos bloques cuesta escribirla entera: lo que hay que reservar antes.
    fn bloques(&self) -> Result<u64, String> {
        let mut n = 1; // su nodo
        if !self.hijos.is_empty() {
            n += plan_de((self.hijos.len() * ENTRADA_LEN) as u64).ok_or("carpeta demasiado grande")?.total;
        }
        for (_, h) in &self.hijos {
            n += match h {
                Hijo::Nodo(_) => 0,
                Hijo::Contenido(_) => 1,
                Hijo::Carpeta(c) => c.bloques()?,
            };
        }
        Ok(n)
    }

    /// La escribe de abajo arriba desde `*cursor` y devuelve su nodo.
    fn escribir<W: Write + Seek>(&self, w: &mut W, cursor: &mut u64) -> Result<BlockPtr, String> {
        let mut lista = Vec::with_capacity(self.hijos.len() * ENTRADA_LEN);
        for (nombre, h) in &self.hijos {
            let p = match h {
                Hijo::Nodo(p) => *p,
                Hijo::Contenido(d) => {
                    let n = es::escritura::nodo_de_fichero(d).map_err(|e| e.name().to_string())?;
                    let p = escribir_objeto(w, *cursor, &n)?;
                    *cursor += 1;
                    p
                }
                Hijo::Carpeta(c) => c.escribir(w, cursor)?,
            };
            lista.extend_from_slice(&Entrada::de_bytes(nombre, p).map_err(|e| e.name().to_string())?.encode());
        }
        let nodo = if lista.is_empty() {
            es::escritura::nodo_de_directorio_vacio()
        } else {
            let plan = plan_de(lista.len() as u64).ok_or("carpeta demasiado grande")?;
            let mut indice = [[0u8; BLOQUE]; NIVELES_MAX];
            let mut arbol = Arbol::nuevo(plan, *cursor, &mut indice[..plan.niveles as usize]).map_err(|e| e.name().to_string())?;
            let mut poner = |lba: u64, d: &[u8]| escribir_bloque(w, lba, d).is_ok();
            for t in lista.chunks(BLOQUE) {
                arbol.empujar(t, &mut poner).map_err(|e| e.name().to_string())?;
            }
            let raiz = arbol.cerrar(&mut poner).map_err(|e| e.name().to_string())?;
            *cursor += plan.total;
            let a = Attr::en_bloques(ATTR_ENTRADAS, lista.len() as u64, plan.niveles, raiz).map_err(|e| e.name().to_string())?;
            Nodo::nuevo(Tipo::Directorio).con(a).map_err(|e| e.name().to_string())?.encode()
        };
        let p = escribir_objeto(w, *cursor, &nodo)?;
        *cursor += 1;
        Ok(p)
    }
}

/// El volumen abierto: su superbloque en uso y cual de las dos copias es.
fn abrir<R: Read + Seek>(r: &mut R, disk_id: [u8; 32], generacion: u64) -> Result<(Superblock, u64), String> {
    let a = leer_bloque(r, es::SUPER_A_BLOCK)?;
    let b = leer_bloque(r, es::SUPER_B_BLOCK)?;
    let (sb, cual) = es::pick_superblock(&a[..SUPER_LEN], &b[..SUPER_LEN]).map_err(|e| format!("superbloque: {}", e.name()))?;
    if sb.disk_id != disk_id {
        return Err("no es el disco esperado: no se escribe".into());
    }
    if sb.generation != generacion {
        return Err(format!("generacion inesperada: se esperaba {generacion}, hay {}", sb.generation));
    }
    Ok((sb, cual))
}

/// **Publica `arbol` como UN estrato nuevo**, con el orden que no pierde datos.
/// `estrato` recibe la raiz y el estrato de ahora (el padre). Una raiz que es
/// un nodo que YA esta (nada cambio respecto a A) no gasta mas que el estrato.
pub(crate) fn publicar<W: Almacen>(
    w: &mut W,
    disk_id: [u8; 32],
    generacion: u64,
    arbol: &Hijo,
    estrato: impl FnOnce(BlockPtr, BlockPtr) -> Estrato,
) -> Result<(u64, BlockPtr, BlockPtr, u64), String> {
    let (sb, cual) = abrir(w, disk_id, generacion)?;
    let bloques = match arbol {
        Hijo::Carpeta(c) => c.bloques()?,
        Hijo::Nodo(_) => 0,
        Hijo::Contenido(_) => return Err("la raiz tiene que ser una carpeta".into()),
    } + 1;
    let mut t = Transaccion::open(&sb, cual, true).map_err(|e| e.name().to_string())?;
    let base = t.reserve(bloques).map_err(|e| e.name().to_string())?;
    let mut cursor = base;
    let raiz = match arbol {
        Hijo::Carpeta(c) => c.escribir(w, &mut cursor)?,
        Hijo::Nodo(p) => *p,
        Hijo::Contenido(_) => unreachable!("rechazado arriba"),
    };
    let e = estrato(raiz, sb.estrato);
    let ep = escribir_objeto(w, cursor, &e.encode())?;
    cursor += 1;
    if cursor != base + bloques {
        return Err("la transaccion no escribio lo que reservo".into());
    }
    t.cerrar_datos().map_err(|e| e.name().to_string())?;
    w.barrera().map_err(|e| format!("barrera antes del commit: {e}"))?;
    t.barrera_hecha().map_err(|e| e.name().to_string())?;
    let (destino, nuevo) = t.commit(ep).map_err(|e| e.name().to_string())?;
    let off = destino.checked_mul(BLOQUE as u64).ok_or("superbloque fuera de rango")?;
    w.seek(SeekFrom::Start(off))
        .and_then(|_| w.write_all(&nuevo.encode()))
        .map_err(|e| format!("publicando el superbloque alterno: {e}"))?;
    w.barrera().map_err(|e| format!("barrera del commit: {e}"))?;
    Ok((nuevo.generation, ep, raiz, bloques))
}

/// Las tres raices de una mezcla: la BASE, la de ahora (A) y la que entra (B).
/// Error si las dos ramas no comparten historia, o si B ya esta dentro de A.
fn raices<R: Read + Seek>(r: &mut R, ahora: &BlockPtr, otra: &BlockPtr) -> Result<[BlockPtr; 3], String> {
    let de_a = antepasados(r, ahora, 4096)?;
    if de_a.iter().any(|p| p.lba == otra.lba && p.off == otra.off) {
        return Err("nada que mezclar: esa rama ya esta dentro de la de ahora".into());
    }
    let de_b = antepasados(r, otra, 4096)?;
    let llave = |p: &BlockPtr| (p.lba, p.off);
    let ka: Vec<_> = de_a.iter().map(llave).collect();
    let kb: Vec<_> = de_b.iter().map(llave).collect();
    let base_k = mezcla::base(&ka, &kb).ok_or("las dos ramas no comparten historia: sin base no hay mezcla de tres")?;
    let base = *de_a.iter().find(|p| llave(p) == base_k).ok_or("base perdida")?;
    Ok([leer_estrato(r, &base)?.raiz, leer_estrato(r, ahora)?.raiz, leer_estrato(r, otra)?.raiz])
}

/// **MEZCLA la rama `otra` en la punta de ahora** y la publica como UN
/// estrato de DOS padres. Cada choque se le pregunta a `elegir`.
///
/// Mezcla POR CARPETAS (`decide::por_arbol`): baja solo donde los dos lados
/// cambiaron algo; lo que un solo lado toco entra ENTERO, sin leerlo.
///
/// No escribe nada si: el disco o la generacion no son los esperados, las dos
/// ramas no comparten historia, o `otra` ya esta dentro de la de ahora.
pub fn mezclar<W: Almacen>(
    disco: &mut W,
    disk_id: [u8; 32],
    generacion: u64,
    otra: &BlockPtr,
    motivo: &str,
    elegir: &mut dyn FnMut(&Choque) -> Eleccion,
) -> Result<Resultado, String> {
    let (sb, _) = abrir(disco, disk_id, generacion)?;
    let ahora = sb.estrato;
    let [rb, ra, rx] = raices(disco, &ahora, otra)?;
    let (arbol, cuenta) = decide::por_arbol(disco, rb, ra, rx, elegir)?;
    let (generacion, estrato, raiz, bloques_nuevos) =
        publicar(disco, disk_id, generacion, &arbol, |raiz, padre| Estrato::mezcla(raiz, padre, otra, 0, Autor::Herramienta, motivo))?;
    // Releer: lo publicado es exactamente lo decidido, y el estrato lleva
    // sus dos padres.
    let e = leer_estrato(disco, &estrato)?;
    if e.padre != ahora || !e.segundo.is_some_and(|s| s.es(otra)) {
        return Err("releido: el estrato de mezcla no lleva sus dos padres".into());
    }
    let mut esperado = decide::hojas_de(disco, &arbol)?;
    let mut leido = decide::sumas(aplanar(disco, &raiz)?);
    esperado.sort();
    leido.sort();
    if esperado != leido {
        return Err("releido: el arbol publicado no es el decidido".into());
    }
    Ok(Resultado { generacion, estrato, raiz, cuenta, bloques_nuevos })
}

mod decide;

#[cfg(test)]
mod pruebas;
