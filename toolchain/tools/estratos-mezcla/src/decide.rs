//! **DECIDIR la mezcla**: que arbol sale, antes de escribir nada.
//!
//! ```text
//!    por_arbol   LA DE VERDAD: carpeta a carpeta (`mezcla::por_carpeta`).
//!                Una subcarpeta que solo un lado toco entra ENTERA, sin
//!                leer lo de dentro; solo se baja donde los dos cambiaron.
//!                Una carpeta que acaba igual que en A se apunta, no se
//!                reescribe. Cuesta lo que cambio, no lo que hay
//!    plano       EL ORACULO: aplanar los tres arboles enteros y decidir
//!                fichero a fichero (`mezcla::mezclar`). Solo en pruebas:
//!                lo que diga por_arbol tiene que ser lo mismo
//! ```
//!
//! ** Y DICEN LO MISMO, siempre: la prueba al azar (`E1_AZAR_MEZCLA`, 60 por
//! defecto; se corrio con 2.000) compara las dos con las tres respuestas
//! posibles a un choque. Al principio no coincidian, y la prueba mostro por
//! que: el hash de una carpeta cambia si se REESCRIBE aunque dentro todo sea
//! igual (sus entradas guardan el `lba` de cada hijo). Por eso aqui el hash
//! de una carpeta es un ATAJO -- si coincide, sale entera sin bajar --, nunca
//! la verdad: si no coincide, se baja, y un lado que no la tiene cuenta como
//! una carpeta vacia. Y una carpeta que la mezcla deja vacia se va.

use super::*;

/// Una decision de `por_carpeta`, con las entradas copiadas: para poder bajar
/// (leer del disco) sin tener prestadas las listas.
enum Paso {
    Queda(Entrada),
    Distintos { base: Option<Entrada>, a: Option<Entrada>, b: Option<Entrada> },
}

fn entradas<R: Read + Seek>(r: &mut R, dir: &BlockPtr) -> Result<Vec<Entrada>, String> {
    let n = leer_nodo(r, dir)?;
    if n.tipo != Tipo::Directorio {
        return Err("se esperaba una carpeta".into());
    }
    let Some(a) = n.attr(ATTR_ENTRADAS).copied() else { return Ok(Vec::new()) };
    let lista = leer_flujo(r, &a)?;
    if lista.len() % ENTRADA_LEN != 0 {
        return Err("una lista de entradas que no acaba en una entrada entera".into());
    }
    lista.chunks_exact(ENTRADA_LEN).map(|e| Entrada::decode(e).map_err(|e| e.name().to_string())).collect()
}

fn es_carpeta<R: Read + Seek>(r: &mut R, p: &BlockPtr) -> Result<bool, String> {
    Ok(leer_nodo(r, p)?.tipo == Tipo::Directorio)
}

/// Se puede BAJAR? Todos los lados que la tienen la tienen como CARPETA.
fn carpetas<R: Read + Seek>(r: &mut R, base: Option<Entrada>, a: Option<Entrada>, b: Option<Entrada>) -> Result<bool, String> {
    for e in [base, a, b].into_iter().flatten() {
        if !es_carpeta(r, &e.nodo)? {
            return Ok(false);
        }
    }
    Ok(a.is_some() || b.is_some())
}

fn junto(ruta: &[u8], nombre: &[u8]) -> Vec<u8> {
    let mut v = ruta.to_vec();
    if !v.is_empty() {
        v.push(b'/');
    }
    v.extend_from_slice(nombre);
    v
}

/// **La mezcla carpeta a carpeta**, desde las tres raices. Devuelve el arbol
/// que sale (un `Hijo::Nodo` si acaba igual que A) y la cuenta.
pub(crate) fn por_arbol<R: Read + Seek>(
    r: &mut R,
    base: BlockPtr,
    a: BlockPtr,
    b: BlockPtr,
    elegir: &mut dyn FnMut(&Choque) -> Eleccion,
) -> Result<(Hijo, mezcla::Cuenta), String> {
    let mut cuenta = mezcla::Cuenta::default();
    let arbol = carpeta(r, &[], Some(base), Some(a), Some(b), elegir, &mut cuenta, 0)?;
    Ok((arbol, cuenta))
}

#[allow(clippy::too_many_arguments)]
fn carpeta<R: Read + Seek>(
    r: &mut R,
    ruta: &[u8],
    base: Option<BlockPtr>,
    a: Option<BlockPtr>,
    b: Option<BlockPtr>,
    elegir: &mut dyn FnMut(&Choque) -> Eleccion,
    cuenta: &mut mezcla::Cuenta,
    hondo: usize,
) -> Result<Hijo, String> {
    if hondo > 64 {
        return Err("mas de 64 niveles de carpetas: se para en vez de adivinar".into());
    }
    let eb = match base {
        Some(p) if es_carpeta(r, &p)? => entradas(r, &p)?,
        _ => Vec::new(),
    };
    // Un lado que no tiene la carpeta cuenta como una carpeta VACIA.
    let ea = match a { Some(p) => entradas(r, &p)?, None => Vec::new() };
    let ex = match b { Some(p) => entradas(r, &p)?, None => Vec::new() };
    let mut pasos = Vec::new();
    let c = mezcla::por_carpeta(&eb, &ea, &ex, |p| {
        pasos.push(match p {
            mezcla::Paso::Queda(e) => Paso::Queda(*e),
            mezcla::Paso::Distintos { base, a, b } => Paso::Distintos { base: base.copied(), a: a.copied(), b: b.copied() },
        })
    });
    cuenta.quedan += c.quedan;
    cuenta.de_a += c.de_a;
    cuenta.de_b += c.de_b;
    cuenta.quitadas += c.quitadas;

    let mut out = Carpeta::default();
    for p in pasos {
        match p {
            Paso::Queda(e) => out.hijos.push((e.nombre_bytes().to_vec(), Hijo::Nodo(e.nodo))),
            Paso::Distintos { base, a: pa, b: pb } if carpetas(r, base, pa, pb)? => {
                // Carpetas (o carpeta y nada) con algo distinto: se BAJA. El
                // hash de una carpeta dice si sus entradas son las mismas
                // bytes a bytes, y una carpeta REESCRITA (por una mezcla
                // anterior, p. ej.) tiene otro hash con el mismo contenido:
                // el hash es un atajo, no la verdad -- la verdad esta dentro.
                let e = pa.or(pb).expect("Distintos trae un lado");
                let dentro = junto(ruta, e.nombre_bytes());
                let hijo = carpeta(r, &dentro, base.map(|e| e.nodo), pa.map(|e| e.nodo), pb.map(|e| e.nodo), elegir, cuenta, hondo + 1)?;
                // Una carpeta que la MEZCLA deja vacia (lo de dentro se quito,
                // o se eligio quitarlo) se va, como en la plana: una carpeta
                // vacia que YA lo era en los dos lados tiene el mismo hash, y
                // sale arriba por `Queda`, sin bajar.
                if !matches!(&hijo, Hijo::Carpeta(c) if c.hijos.is_empty()) {
                    out.hijos.push((e.nombre_bytes().to_vec(), hijo));
                }
            }
            Paso::Distintos { a, b, .. } => {
                cuenta.choques += 1;
                let nombre = a.or(b).map(|e| e.nombre_bytes().to_vec()).unwrap_or_default();
                let choque = Choque { ruta: junto(ruta, &nombre), a: a.map(|e| e.nodo), b: b.map(|e| e.nodo) };
                let elegido = match elegir(&choque) {
                    Eleccion::A => a,
                    Eleccion::B => b,
                    Eleccion::Quitar => None,
                };
                if let Some(e) = elegido {
                    out.hijos.push((e.nombre_bytes().to_vec(), Hijo::Nodo(e.nodo)));
                }
            }
        }
    }
    // Acaba IGUAL que en A (mismos nombres, mismos nodos, mismo orden): se
    // apunta la de A y no se reescribe nada.
    let igual = out.hijos.len() == ea.len()
        && out.hijos.iter().zip(&ea).all(|((n, h), e)| n.as_slice() == e.nombre_bytes() && matches!(h, Hijo::Nodo(p) if *p == e.nodo));
    Ok(match a {
        Some(a) if igual => Hijo::Nodo(a),
        _ => Hijo::Carpeta(out),
    })
}

/// Las hojas (ruta, QUE) de un arbol decidido, bajando por el disco donde el
/// arbol apunta a un nodo que ya estaba (una subcarpeta entera, p. ej.).
pub(crate) fn hojas_de<R: Read + Seek>(r: &mut R, arbol: &Hijo) -> Result<Vec<(Vec<u8>, es::Hash)>, String> {
    let mut out = Vec::new();
    match arbol {
        Hijo::Nodo(p) => out = sumas(aplanar(r, p)?),
        Hijo::Carpeta(c) => hojas_carpeta(r, c, &mut Vec::new(), &mut out)?,
        Hijo::Contenido(_) => {}
    }
    Ok(out)
}

fn hojas_carpeta<R: Read + Seek>(r: &mut R, c: &Carpeta, ruta: &mut Vec<u8>, out: &mut Vec<(Vec<u8>, es::Hash)>) -> Result<(), String> {
    for (n, h) in &c.hijos {
        let largo = ruta.len();
        if largo > 0 {
            ruta.push(b'/');
        }
        ruta.extend_from_slice(n);
        match h {
            Hijo::Nodo(p) => {
                let mut v = Vec::new();
                bajar(r, p, ruta, &mut v, 0)?;
                out.extend(sumas(v));
            }
            Hijo::Carpeta(c) if c.hijos.is_empty() => {}
            Hijo::Carpeta(c) => hojas_carpeta(r, c, ruta, out)?,
            Hijo::Contenido(_) => {}
        }
        ruta.truncate(largo);
    }
    Ok(())
}

#[cfg(test)]
fn como(v: &[(Vec<u8>, es::Hash)]) -> Vec<mezcla::Lado<'_>> {
    v.iter().map(|(r, n)| mezcla::Lado { ruta: r, nodo: *n }).collect()
}

pub(crate) fn sumas(v: Vec<Hoja>) -> Vec<(Vec<u8>, es::Hash)> {
    v.into_iter().map(|h| (h.ruta, h.nodo.hash)).collect()
}

/// **EL ORACULO**: la mezcla fichero a fichero sobre los arboles aplanados.
#[cfg(test)]
pub(crate) fn plano<R: Read + Seek>(
    r: &mut R,
    base: BlockPtr,
    a: BlockPtr,
    b: BlockPtr,
    elegir: &mut dyn FnMut(&Choque) -> Eleccion,
) -> Result<Carpeta, String> {
    use es::mezcla::Sale;
    let (hb, ha, hx) = (aplanar(r, &base)?, aplanar(r, &a)?, aplanar(r, &b)?);
    let lados = |v: &[Hoja]| -> Vec<(Vec<u8>, es::Hash)> { v.iter().map(|h| (h.ruta.clone(), h.nodo.hash)).collect() };
    let (lb, la, lx) = (lados(&hb), lados(&ha), lados(&hx));
    let (lb, la, lx) = (como(&lb), como(&la), como(&lx));
    let donde = |ruta: &[u8], hash: &es::Hash| {
        [&ha, &hx, &hb].into_iter().flat_map(|v| v.iter()).find(|h| mismo(&h.ruta, ruta) && h.nodo.hash == *hash).map(|h| h.nodo)
    };
    let mut arbol = Carpeta::default();
    let mut fallo: Option<String> = None;
    mezcla::mezclar(&lb, &la, &lx, |s| {
        if fallo.is_some() {
            return;
        }
        let r = match s {
            Sale::Queda(ruta, hash) => match donde(ruta, &hash) {
                Some(p) => arbol.poner(ruta, Hijo::Nodo(p)),
                None => Err("un nodo decidido no esta en ningun lado".into()),
            },
            Sale::Choque { ruta, a, b } => {
                let (a, b) = (a.and_then(|h| donde(ruta, &h)), b.and_then(|h| donde(ruta, &h)));
                match elegir(&Choque { ruta: ruta.to_vec(), a, b }) {
                    Eleccion::A => a.map_or(Ok(()), |p| arbol.poner(ruta, Hijo::Nodo(p))),
                    Eleccion::B => b.map_or(Ok(()), |p| arbol.poner(ruta, Hijo::Nodo(p))),
                    Eleccion::Quitar => Ok(()),
                }
            }
        };
        if let Err(e) = r {
            fallo = Some(e);
        }
    });
    match fallo {
        Some(e) => Err(e),
        None => Ok(arbol),
    }
}
