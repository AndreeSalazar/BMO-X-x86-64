//! **CARRIL VERDE** -- donde caen los pixeles. Se puede tocar sin miedo.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! [carril]  VERDE     no decide nada: recibe el `Dictamen` ya resuelto y lo
//!           pone en pantalla. Equivocarse aqui se VE
//!
//! [cuesta]  NADA -- son coordenadas, anchos y tintas de un trozo de la linea
//!           de instrumentos de CABINA. Un fallo aqui deja la caja torcida, corrida o sin
//!           pintar, y se nota en el acto porque **la esta mirando el propietario**.
//!           No puede mentir: para mentir hay que afirmar algo, y aqui no se
//!           afirma nada.
//!
//! [riesgo]  -- ninguno declarado. Es el carril que no tiene letrero, y por eso
//!           existe el fichero: **saber que algo es verde tambien es saber**.
//!
//! # Por que esto NO esta con lo otro
//!
//! Porque las dos averias del pulso fueron las dos de significado, y ninguna de
//! sitio. Mientras las dos cosas vivian en la misma funcion, un cambio de donde
//! cae un numero y un cambio de que dice ese numero se leian igual en el diff.
//!
//! ```text
//!    amarilla.rs   que es verdad     se equivoca CALLANDO   -> con dos manos
//!    verde.rs      donde va          se equivoca A LA VISTA -> se puede jugar
//! ```
//!
//! # Donde vive (2026-09-22)
//!
//! Iba en la barra de arriba, al lado del testigo, y se encogia antes de
//! callarse: primero lo que dice si el escritorio vive, luego el reparto. La
//! barra se fundio en el panel de la izquierda y el pulso se partio por esa
//! misma costura: **lo que no se puede perder** --el numero y su aguja-- va
//! en el panel, siempre a la vista (`lateral::latido`); **el reparto** --pinta,
//! cuerpo, puerta-- va aqui, en la linea de instrumentos de CABINA, que es
//! donde se mira cuando algo va lento.
//!
//! ```text
//!    latido 1000/s  pinta 4  cuerpo 40  puerta 950
//! ```

use bmo_userland as bmo;

use super::amarilla::Dictamen;
use crate::scene::{INK, INK_DIM};
use crate::text::decimal;

/// Lo que ocupa: `latido 12345/s  pinta 4  cuerpo 900  puerta 40`.
pub(crate) const ANCHO: u32 = 400;

/// **Pintar lo ya decidido** en `(x, y)`, sobre `fondo`. Aqui no se pregunta
/// nada: llega resuelto. Devuelve donde acaba la caja.
pub(crate) fn pintar(p: &bmo::Pantalla, x: u32, y: u32, fondo: u32, d: &Dictamen) -> u32 {
    p.rect(x, y, ANCHO, bmo::GLIFO_ALTO, fondo);
    let modo = if d.en_reposo { "reposo " } else if d.en_latido { "latido " } else { "pulso " };
    let tx = p.texto(x, y, modo, INK_DIM);
    // ** SIN RELOJ se dice con palabras, y en claro. Ver `amarilla`: el numero
    // no existe, asi que no se pinta ni un cero.
    let tx = match d.ritmo {
        None => p.texto(tx, y, "SIN RELOJ", INK),
        Some(v) => {
            let mut buf = [0u8; 10];
            let n = decimal(v as u64, &mut buf);
            let tinta = if d.alarma { INK } else { INK_DIM };
            let tx = p.texto_bytes(tx, y, &buf[..n], tinta);
            p.texto(tx, y, "/s", INK_DIM)
        }
    };
    if d.ritmo.is_some() {
        // El que MANDA va en claro: es el lado del que hay que tirar.
        let tx = renglon(p, tx, y, "  pinta ", d.pinta, false);
        let tx = renglon(p, tx, y, "  cuerpo ", d.cuerpo_ms, d.manda_cuerpo);
        renglon(p, tx, y, "  puerta ", d.puerta_ms, !d.manda_cuerpo);
    }
    x + ANCHO
}

fn renglon(p: &bmo::Pantalla, x: u32, y: u32, nombre: &str, ms: u32, manda: bool) -> u32 {
    let tx = p.texto(x, y, nombre, INK_DIM);
    let mut buf = [0u8; 10];
    let n = decimal(ms as u64, &mut buf);
    p.texto_bytes(tx, y, &buf[..n], if manda { INK } else { INK_DIM })
}
