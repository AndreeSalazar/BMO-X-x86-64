//! **COMO se escribe una cara para que VIAJE.** El emisor B.
//!
//! El escalon 8 de `PLAN_MAQUETA.md` y el 2 de `PLAN_LA_CARA_VIAJA.md`. Son el
//! mismo escalon escrito en dos documentos, y se marcan juntos o uno de los dos
//! miente.
//!
//! ```text
//!    rust.rs   ->  codigo, para compilar DENTRO del servicio
//!    bef.rs    ->  bytes, para cambiar la cara SIN recompilar   <- este
//! ```
//!
//! ## Las tres cosas que este modulo NO hace, y son lo que lo mantiene corto
//!
//! ```text
//!    NO decide que se dibuja     eso es `orden::lista` y `orden::golpes`
//!    NO es propietario del formato     eso es la crate `bmo-maqueta-cara`
//!    NO escribe el .bex          eso es `bmo_abi::bef::recursos` + bmo-pack
//! ```
//!
//! *** LA TERCERA ES LA QUE MAS FACIL SE ROMPE, y `bmo-pack` ya lo dejo escrito
//! cuando le paso: el formato de la seccion `Resources 0x0B` tiene **un solo
//! propietario**, y *"si alguien tuviera que mirar dos sitios para saber donde empieza
//! un recurso, seria porque alguien escribio el formato dos veces"*.
//!
//! Asi que esto produce **el contenido del recurso** y se para ahi. Meterlo en
//! un `.bex` es una orden de `bmo-pack`:
//!
//! ```text
//!    bmo-pack app.bex -r cara=calc.cara -o app.bex
//! ```
//!
//! ## Y por que el `de` de cada trazo NO viaja
//!
//! `Orden::de` --*"de que caja salio este trazo"*-- va al comentario del codigo
//! que emite `rust.rs`, y es por donde se sigue un fotograma raro hasta la caja
//! que lo causo. **Aqui se tira a proposito.**
//!
//! Son ~5 bytes por trazo sobre una cara de ~950: cerca del 20% del fichero para
//! algo que en ejecucion no lee nadie. Y la consecuencia hay que decirla en vez
//! de descubrirla: **un recurso no sirve para diagnosticar como sirve el codigo
//! generado.** Cuando una cara salga rara, se mira el emisor A, no este.
//!
//! Lo que si viaja son los nombres de los **golpes**, porque esos no son
//! diagnostico: son lo que el programa recibe cuando alguien pulsa.

use bmo_maqueta_cara as cara;

use crate::orden::{Estado, Golpe, Orden, Trazo};

/// **Por que esta cara no se pudo escribir.**
///
/// [!] Las tres son de DESBORDE, y ninguna es un fallo del `.maqueta`: son el
/// formato diciendo hasta donde llega. Un emisor que recortara en silencio
/// produciria una cara que se abre, se pinta, y esta mal -- que es peor que no
/// producir nada.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NoCabe {
    /// El lienzo no cabe en `u16`.
    Lienzo { ancho: i64, alto: i64 },
    /// Un rectangulo tiene una coordenada o un medida que no cabe en el campo.
    Rect { de: String, x: i64, y: i64, w: i64, h: i64 },
    /// Hay mas trazos, mas golpes o mas bytes de cadenas de los que la cabecera
    /// puede contar.
    Demasiado { que: &'static str, cuantos: usize },
    /// (MAQUETA 3, MA2) Un trazo que un `overflow` corta a medias: la CARA
    /// todavia no lleva recortes, y sin el saldria entero.
    Recorte { de: String },
}

/// El bloque de cadenas, con las repetidas puestas UNA vez.
///
/// * Los nombres se repiten mucho --diecisiete botones que dicen `#boton0`..
/// `#boton9`, los textos de una fila-- y guardarlos una vez sale gratis: la
/// busqueda es lineal sobre unas decenas de entradas, en el anfitrion, una vez
/// por compilacion.
#[derive(Default)]
struct Cadenas {
    bytes: Vec<u8>,
}

impl Cadenas {
    /// Devuelve `(offset, largo)`. Si la cadena ya estaba, no la vuelve a meter.
    fn mete(&mut self, s: &[u8]) -> (u16, u16) {
        if s.is_empty() {
            return (0, 0);
        }
        if let Some(p) = self.bytes.windows(s.len()).position(|w| w == s) {
            return (p as u16, s.len() as u16);
        }
        let off = self.bytes.len();
        self.bytes.extend_from_slice(s);
        (off as u16, s.len() as u16)
    }
}

fn u16_de(v: i64) -> Option<u16> {
    if (0..=u16::MAX as i64).contains(&v) {
        Some(v as u16)
    } else {
        None
    }
}
fn i16_de(v: i64) -> Option<i16> {
    if (i16::MIN as i64..=i16::MAX as i64).contains(&v) {
        Some(v as i16)
    } else {
        None
    }
}

/// **Escribir la cara.** Entra lo que `orden` decidio, salen los bytes del
/// recurso.
///
/// `ancho` y `alto` son el lienzo, y se pasan en vez de deducirse de los trazos:
/// una cara cuyo lienzo fuera *"lo que ocupan sus cajas"* cambiaria de medida al
/// quitar la ultima, y el lector no tendria contra que comprobar nada.
///
/// # El orden de escritura es el del plano, y no es casualidad
///
/// ```text
///    1. se recogen las CADENAS      porque los offsets hacen falta despues
///    2. se arman trazos y golpes    que ya pueden apuntar a ellas
///    3. se escribe la CABECERA      la ultima, porque cuenta lo de arriba
/// ```
///
/// Escribir la cabecera primero obligaria a volver a pisarla con las cuentas de
/// verdad, y **un campo que se escribe dos veces es un campo que un dia se
/// escribe una**.
pub fn escribir(ordenes: &[Orden], golpes: &[Golpe], ancho: i64, alto: i64) -> Result<Vec<u8>, NoCabe> {
    let (Some(ancho_u), Some(alto_u)) = (u16_de(ancho), u16_de(alto)) else {
        return Err(NoCabe::Lienzo { ancho, alto });
    };

    if let Some(o) = ordenes.iter().find(|o| o.recorte.is_some()) {
        return Err(NoCabe::Recorte { de: o.de.clone() });
    }
    let mut cad = Cadenas::default();
    let mut trazos: Vec<[u8; cara::TRAZO]> = Vec::with_capacity(ordenes.len());

    for o in ordenes {
        let Some(d) = de_cada_clase(&o.trazo) else {
            return Err(NoCabe::Rect { de: o.de.clone(), x: 0, y: 0, w: -1, h: -1 });
        };
        let r = d.caja;
        let (x, y, w, h) = (r.x as i64, r.y as i64, r.w as i64, r.h as i64);
        let (Some(xi), Some(yi), Some(wu), Some(hu)) =
            (i16_de(x), i16_de(y), u16_de(w), u16_de(h))
        else {
            return Err(NoCabe::Rect { de: o.de.clone(), x, y, w, h });
        };

        let (clase, color, cadena) = (d.clase, d.color, d.datos.as_slice());
        let (off, len) = cad.mete(cadena);

        let mut t = [0u8; cara::TRAZO];
        t[cara::trazo::CLASE] = clase;
        t[cara::trazo::ESTADO] = match o.estado {
            Estado::Reposo => cara::ESTADO_REPOSO,
            Estado::Encima => cara::ESTADO_ENCIMA,
        };
        pon_i16(&mut t, cara::trazo::X, xi);
        pon_i16(&mut t, cara::trazo::Y, yi);
        pon_u16(&mut t, cara::trazo::W, wu);
        pon_u16(&mut t, cara::trazo::H, hu);
        pon_u32(&mut t, cara::trazo::COLOR, color);
        pon_u16(&mut t, cara::trazo::CAD_OFF, off);
        pon_u16(&mut t, cara::trazo::CAD_LEN, len);
        // En RECT y TEXTO el EXTRA se queda en cero (era el reservado de la
        // version 1); las clases suaves llevan ahi lo suyo.
        pon_u16(&mut t, cara::trazo::EXTRA, d.extra);
        trazos.push(t);
    }

    let mut golpes_b: Vec<[u8; cara::GOLPE]> = Vec::with_capacity(golpes.len());
    for g in golpes {
        let (x, y, w, h) = (g.r.x as i64, g.r.y as i64, g.r.w as i64, g.r.h as i64);
        let (Some(xi), Some(yi), Some(wu), Some(hu)) =
            (i16_de(x), i16_de(y), u16_de(w), u16_de(h))
        else {
            return Err(NoCabe::Rect { de: g.nombre.clone(), x, y, w, h });
        };
        let (off, len) = cad.mete(g.nombre.as_bytes());
        let mut b = [0u8; cara::GOLPE];
        pon_i16(&mut b, cara::golpe::X, xi);
        pon_i16(&mut b, cara::golpe::Y, yi);
        pon_u16(&mut b, cara::golpe::W, wu);
        pon_u16(&mut b, cara::golpe::H, hu);
        pon_u16(&mut b, cara::golpe::CAD_OFF, off);
        pon_u16(&mut b, cara::golpe::CAD_LEN, len);
        golpes_b.push(b);
    }

    // ** Las tres cuentas, comprobadas ANTES de escribir la cabecera.
    //
    // Sin esto, un `as u16` las recortaria y saldria una cara que declara menos
    // trazos de los que trae: se abriria sin protestar y pintaria a medias. El
    // lector no puede cazar eso -- las cuentas cuadrarian.
    if trazos.len() > u16::MAX as usize {
        return Err(NoCabe::Demasiado { que: "trazos", cuantos: trazos.len() });
    }
    if golpes_b.len() > u16::MAX as usize {
        return Err(NoCabe::Demasiado { que: "golpes", cuantos: golpes_b.len() });
    }
    if cad.bytes.len() > u16::MAX as usize {
        return Err(NoCabe::Demasiado { que: "bytes de cadenas", cuantos: cad.bytes.len() });
    }

    let mut out = Vec::with_capacity(
        cara::CABECERA + trazos.len() * cara::TRAZO + golpes_b.len() * cara::GOLPE + cad.bytes.len(),
    );
    out.extend_from_slice(&cara::MAGICO.to_le_bytes());
    out.extend_from_slice(&cara::VERSION.to_le_bytes());
    out.extend_from_slice(&ancho_u.to_le_bytes());
    out.extend_from_slice(&alto_u.to_le_bytes());
    out.extend_from_slice(&(trazos.len() as u16).to_le_bytes());
    out.extend_from_slice(&(golpes_b.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cad.bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(out.len(), cara::CABECERA);
    for t in &trazos {
        out.extend_from_slice(t);
    }
    for g in &golpes_b {
        out.extend_from_slice(g);
    }
    out.extend_from_slice(&cad.bytes);
    Ok(out)
}

/// Lo que un trazo escribe en su fila: clase, caja, color, `EXTRA` y datos.
struct DeLaClase {
    clase: u8,
    caja: bmo_maqueta_layout::Rect,
    color: u32,
    extra: u16,
    datos: Vec<u8>,
}

/// **Cada trazo, en el plano de la version 2.** `None` si algo no cabe en
/// su campo (un radio de mas de 255, un camino de mas de 2047 px).
fn de_cada_clase(t: &Trazo) -> Option<DeLaClase> {
    let byte = |v: u32| u8::try_from(v).ok().map(u16::from);
    Some(match t {
        Trazo::Rect { r, color } => DeLaClase { clase: cara::CLASE_RECT, caja: *r, color: *color, extra: 0, datos: Vec::new() },
        Trazo::Texto { r, texto, color } => {
            DeLaClase { clase: cara::CLASE_TEXTO, caja: *r, color: *color, extra: 0, datos: texto.as_bytes().to_vec() }
        }
        Trazo::Caja { r, radio, color } => DeLaClase { clase: cara::CLASE_CAJA, caja: *r, color: *color, extra: u16::try_from(*radio).ok()?, datos: Vec::new() },
        Trazo::Borde { r, radio, grosor, color } => {
            DeLaClase { clase: cara::CLASE_BORDE, caja: *r, color: *color, extra: byte(*radio)? | byte(*grosor)? << 8, datos: Vec::new() }
        }
        Trazo::Resplandor { r, radio, alcance, argb } => {
            DeLaClase { clase: cara::CLASE_RESPLANDOR, caja: *r, color: *argb, extra: byte(*radio)? | byte(*alcance)? << 8, datos: Vec::new() }
        }
        Trazo::Degradado { r, radio, de, a, vertical } => {
            let radio = u16::try_from(*radio).ok().filter(|&r| r < 0x8000)?;
            DeLaClase { clase: cara::CLASE_DEGRADADO, caja: *r, color: *de, extra: radio | (*vertical as u16) << 15, datos: a.to_le_bytes().to_vec() }
        }
        Trazo::Letra { r, texto, color, px, peso, espacio, mayusculas } => {
            let p = match peso {
                500 => 1,
                600 => 2,
                700 => 3,
                _ => 0,
            };
            let e = (*espacio / 10).clamp(0, 63) as u16;
            let extra = (*px as u16 & 0x7F) | p << 7 | (*mayusculas as u16) << 9 | e << 10;
            DeLaClase { clase: cara::CLASE_LETRA, caja: *r, color: *color, extra, datos: texto.as_bytes().to_vec() }
        }
        Trazo::Linea { caminos, cerrados, grosor64, color } => {
            let (caja, datos) = puntos(caminos, cerrados)?;
            DeLaClase { clase: cara::CLASE_LINEA, caja, color: *color, extra: u16::try_from(grosor64 / 4).ok()?, datos }
        }
        // La CARA que viaja todavia no lleva pixeles (H4 es del emisor de
        // Rust): una imagen no cabe en su formato, y se dice.
        Trazo::Imagen { .. } => return None,
        Trazo::Relleno { caminos, color } => {
            let cerrados = vec![true; caminos.len()];
            let (caja, datos) = puntos(caminos, &cerrados)?;
            DeLaClase { clase: cara::CLASE_RELLENO, caja, color: *color, extra: 0, datos }
        }
        // ** MAQUETA 3: la figura, con su tinta (`CLASE_FIGURA`).
        Trazo::Figura { caminos, cerrados, pluma, tinta, alfa, par_impar } => {
            let (caja, pts) = puntos(caminos, cerrados)?;
            let (ox, oy) = (caja.x * 64, caja.y * 64);
            let i16_de = |v: i32| i16::try_from(v / 4).ok();
            let rel = |p: (i32, i32)| -> Option<[i16; 2]> { Some([i16_de(p.0 - ox)?, i16_de(p.1 - oy)?]) };
            let vec_ = |p: (i32, i32)| -> Option<[i16; 2]> { Some([i16_de(p.0)?, i16_de(p.1)?]) };
            use crate::orden::TintaFija as T;
            let (clase_tinta, geo, paradas, color): (u8, Vec<[i16; 2]>, &[bmo_pinta::Parada], u32) = match tinta {
                T::Liso(c) => (cara::figura::LISA, Vec::new(), &[], *c),
                T::Lineal { de, a, paradas } => (cara::figura::LINEAL, vec![rel(*de)?, rel(*a)?], paradas, 0),
                T::Radial { centro, eje_x, eje_y, paradas } => (cara::figura::RADIAL, vec![rel(*centro)?, vec_(*eje_x)?, vec_(*eje_y)?], paradas, 0),
            };
            if paradas.len() > cara::figura::PARADAS {
                return None;
            }
            let mut datos = vec![clase_tinta, paradas.len() as u8];
            datos.extend_from_slice(&u16::try_from(pluma / 4).ok()?.to_le_bytes());
            for g in geo {
                datos.extend_from_slice(&g[0].to_le_bytes());
                datos.extend_from_slice(&g[1].to_le_bytes());
            }
            for p in paradas {
                datos.extend_from_slice(&p.en.to_le_bytes());
                datos.extend_from_slice(&p.c.to_le_bytes());
                datos.extend_from_slice(&[p.alfa, 0]);
            }
            datos.extend(pts);
            DeLaClase { clase: cara::CLASE_FIGURA, caja, color, extra: *par_impar as u16 | (*alfa as u16) << 8, datos }
        }
    })
}

/// Lo que el pintor de una cara descodifica por trazo (`bmo_pinta`, sin
/// monton): lo que pase de ahi no cabe, y se dice al escribir en vez de
/// cortarse en el aparato.
const PUNTOS_MAX: usize = 1024;
const SUBS_MAX: usize = 32;

/// Los puntos de unos caminos (1/64 px, en el lienzo) en el plano de la
/// version 2: su caja en pixeles, y los pares en 1/16 RELATIVOS a ella.
fn puntos(caminos: &[Vec<(i32, i32)>], cerrados: &[bool]) -> Option<(bmo_maqueta_layout::Rect, Vec<u8>)> {
    if caminos.len() > SUBS_MAX || caminos.iter().map(Vec::len).sum::<usize>() > PUNTOS_MAX {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for c in caminos {
        for &(x, y) in c {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 {
        return None;
    }
    let (px0, py0) = (x0.div_euclid(64), y0.div_euclid(64));
    let (px1, py1) = ((x1 + 63).div_euclid(64), (y1 + 63).div_euclid(64));
    let caja = bmo_maqueta_layout::Rect { x: px0, y: py0, w: (px1 - px0).max(1) as u32, h: (py1 - py0).max(1) as u32 };
    let mut d = Vec::new();
    for (k, c) in caminos.iter().enumerate() {
        d.extend_from_slice(&cara::puntos::SEPARA.to_le_bytes());
        d.extend_from_slice(&(cerrados.get(k).copied().unwrap_or(false) as i16).to_le_bytes());
        for &(x, y) in c {
            let rx = i16::try_from((x - px0 * 64) / 4).ok()?;
            let ry = i16::try_from((y - py0 * 64) / 4).ok()?;
            d.extend_from_slice(&rx.to_le_bytes());
            d.extend_from_slice(&ry.to_le_bytes());
        }
    }
    Some((caja, d))
}

fn pon_u16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn pon_i16(b: &mut [u8], i: usize, v: i16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn pon_u32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}
