//! **LA SOLAPA `numeros`**: como esta el almacen, de un vistazo.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! === Por que es un fichero y no un trozo de `data.rs` ===
//!
//! Por L6a: `data.rs` paso de las mil lineas y el censo dijo que no. Pero el
//! corte no se eligio por medida -- se eligio porque **este trozo no comparte
//! NADA con el explorador**.
//!
//! ```text
//!   [numeros]      contesta "COMO ESTA el almacen"  -- generacion, sitio,
//!                  identidad, nivel. No mira ni un nodo.
//!   [explorador]   contesta "QUE HAY dentro"        -- arbol, rejilla, grafo.
//!                  No mira ni un numero del volumen.
//! ```
//!
//! Ni una funcion de aqui la llama el otro lado, y al reves tampoco. `level_text`
//! y `magnitude` solo se usaban aqui y se vienen con el: **el corte se elige por
//! nombres libres**, y estos dos lo estaban.
//!
//! === Lo que muestra, y por que en este orden ===
//!
//! Generacion primero, porque es lo que cambia al escribir y por tanto lo
//! primero que uno viene a mirar despues de sellar. Despues el sitio, la
//! identidad del disco y el nivel de ocupacion -- que es el que puede decir
//! SOLO LECTURA y por eso va con su color.

use bmo_userland as bmo;

use super::data::DataWindow;
use super::*;
use crate::text::decimal;

/// Los cuatro niveles de `bmo_estratos::espacio`, con su color.
///
/// El orden es el del ABI (`INFO_ES_NIVEL`), no uno inventado aqui: si
/// divergieran, el panel pintaria en verde un volumen en solo lectura.
fn level_text(n: u64) -> (&'static str, u32) {
    match n {
        0 => ("holgado", INK_OK),
        1 => ("AVISO: por encima del 70%", 0x00F0_D070),
        2 => ("FAULT: por encima del 85%", INK_BAD),
        _ => ("SOLO LECTURA: por encima del 95%", INK_BAD),
    }
}

/// ** EL PANEL, AL ESTILO DEL INICIO (01-10). El propietario: *"rehaz numeros
/// e historial al estilo inicio, pero todo mejor"*. Las mismas cifras de
/// siempre --generacion, cuando, espacio, estado, identidad, cuantas caben y
/// la verdad sobre la escritura--, en tarjetas: arriba el volumen en grande
/// con su estado, debajo una tarjeta por cifra y la de la escritura.
///
/// `tx` se sigue recibiendo para que el pie empiece en la misma columna que
/// las demas solapas.
pub(crate) fn paint(p: &bmo::Pantalla, c: &DataWindow, tx: u32) {
    use super::data::{DATA_EDGE, DATA_TITLE};
    let z = c.bib_zona();
    let pie = c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8;
    p.texto(tx, pie, "F12 o ESC cierran   Ctrl+n su consola   mientras este abierta, el teclado es de esta ventana", INK_DIM);

    let (x0, y0) = (z.x + 12, z.y + 12);
    let ancho = z.w.saturating_sub(24);
    let fondo = 0x0011_261A;
    let caja = |x: u32, y: u32, w: u32, h: u32, filo: u32| {
        super::borde::marco(p, x, y, w, h, RADIUS, filo, fondo);
        p.rect(x + RADIUS, y, w.saturating_sub(2 * RADIUS), 1, DATA_TITLE);
    };

    // -- el encabezado: el volumen en grande y su estado --
    let cab_h = 92u32;
    caja(x0, y0, ancho, cab_h, DATA_EDGE);
    super::iconos::vector(p, x0 + 16, y0 + 14, 64, &super::iconos::dibujos::ESTRATOS, &super::iconos::paleta(DATA_TITLE, DATA_TITLE), fondo);
    p.texto_escala(x0 + 96, y0 + 16, "ESTRATOS (F:)", INK, 2);
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(x0 + 96, y0 + 58, "ningun volumen ESTRATOS montado: se formatea desde el anfitrion con estratos-fmt", INK_BAD);
        return;
    }
    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let (estado, color_estado) = level_text(bmo::info(bmo::INFO_ES_NIVEL));
    // La pastilla del estado, y la identidad a su lado.
    let py = y0 + 58;
    let pw = (estado.len() as u32 + 2) * bmo::GLIFO_ANCHO;
    super::borde::marco(p, x0 + 96, py - 3, pw, bmo::GLIFO_ALTO + 6, 6, color_estado, fondo);
    p.texto(x0 + 96 + bmo::GLIFO_ANCHO, py, estado, color_estado);
    let (ident, color_ident) = if bmo::info(bmo::INFO_ES_IDENTIDAD) != 0 {
        ("nacio en ESTE disco", INK_OK)
    } else {
        ("NO nacio aqui: clonado? no se escribira", INK_BAD)
    };
    p.texto(x0 + 96 + pw + 16, py, ident, color_ident);

    // -- las cuatro cifras --
    let ty = y0 + cab_h + 12;
    let cols: u32 = if ancho >= 4 * 200 + 36 { 4 } else { 2 };
    let cw = (ancho - (cols - 1) * 12) / cols;
    let ch = 96u32;
    let tarjeta = |k: u32| (x0 + (k % cols) * (cw + 12), ty + (k / cols) * (ch + 12));
    let mut b = [0u8; 10];
    let titulo = |x: u32, y: u32, t: &str| p.texto(x + 14, y + 12, t, INK_DIM);

    // 1. Generacion
    let (x, y) = tarjeta(0);
    caja(x, y, cw, ch, DATA_EDGE);
    titulo(x, y, "GENERACION");
    let n = decimal(bmo::info(bmo::INFO_ES_GENERACION), &mut b);
    p.texto_escala(x + 14, y + 34, core::str::from_utf8(&b[..n]).unwrap_or("?"), DATA_TITLE, 2);
    p.texto(x + 14, y + 72, "transacciones desde el formateo", INK_DIM);

    // 2. La version de ahora: cuando se hizo. Sin fecha no se inventa una.
    let (x, y) = tarjeta(1);
    caja(x, y, cw, ch, DATA_EDGE);
    titulo(x, y, "ULTIMA VERSION");
    match bmo_rtc::desempaquetar(bmo::info(bmo::INFO_ES_FECHA)) {
        Some(f) => {
            let mut fb = [0u8; 24];
            let n = bmo_rtc::escribir(&f, &mut fb);
            // La fecha en grande y la hora debajo.
            let fecha = core::str::from_utf8(&fb[..n.min(10)]).unwrap_or("?");
            p.texto_escala(x + 14, y + 34, fecha, INK, 2);
            if n > 11 {
                let x2 = p.texto_bytes(x + 14, y + 72, &fb[11..n.min(19)], 0x00C9_D8CF);
                p.texto(x2 + 8, y + 72, "cuando se hizo", INK_DIM);
            }
        }
        None => {
            p.texto_escala(x + 14, y + 34, "sin fechar", INK_DIM, 2);
            p.texto(x + 14, y + 72, "la placa no dio una hora creible", INK_DIM);
        }
    }

    // 3. Espacio, con su barra
    let (x, y) = tarjeta(2);
    caja(x, y, cw, ch, DATA_EDGE);
    titulo(x, y, "ESPACIO");
    let pct = if bloques == 0 { 0 } else { usados * 100 / bloques };
    let n = decimal(pct, &mut b);
    let xe = p.texto_escala(x + 14, y + 34, core::str::from_utf8(&b[..n]).unwrap_or("?"), DATA_TITLE, 2);
    p.texto(xe + 4, y + 34 + bmo::GLIFO_ALTO, "% usado", INK_DIM);
    let bw = cw.saturating_sub(28);
    p.rect(x + 14, y + 66, bw, 6, 0x0007_110B);
    let lleno = if bloques == 0 { 0 } else { ((usados as u128 * bw as u128) / bloques as u128) as u32 };
    p.rect(x + 14, y + 66, lleno.max(2), 6, color_estado);
    let xm = magnitude(p, x + 14, y + 76, usados * tam, INK);
    let xm = p.texto(xm, y + 76, " de ", INK_DIM);
    magnitude(p, xm, y + 76, bloques * tam, INK);

    // 4. Cuantas VERSIONES mas caben: lo que contesta "cuando hara falta el
    // recolector?". Un porcentaje no lo dice; con 414 GiB son millones.
    let (x, y) = tarjeta(3);
    caja(x, y, cw, ch, DATA_EDGE);
    titulo(x, y, "CABEN TODAVIA");
    let libres = bloques.saturating_sub(usados);
    let por_obj = (20 * 1024u64).div_ceil(tam).max(1);
    let n = decimal(libres / por_obj, &mut b);
    p.texto_escala(x + 14, y + 34, core::str::from_utf8(&b[..n]).unwrap_or("?"), DATA_TITLE, 2);
    p.texto(x + 14, y + 72, "objetos de mas de 20 KiB", INK_DIM);

    // -- la escritura: ABIERTA o por que no --
    let filas = 4 / cols;
    let ey = ty + filas * (ch + 12);
    let eh = 84u32;
    if ey + eh > z.y + z.h {
        return;
    }
    let abierta = bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0;
    let color = if abierta { INK_OK } else { 0x00F0_D070 };
    caja(x0, ey, ancho, eh, if abierta { DATA_EDGE } else { color });
    p.rect(x0 + 18, ey + 18, 10, 10, color);
    p.texto(x0 + 36, ey + 15, if abierta { "escritura ABIERTA" } else { "escritura CERRADA" }, color);
    let (l1, l2): (&str, &str) = if abierta {
        ("sellar cierra un estrato y sube la generacion, con FLUSH CACHE de verdad.", "cada carpeta y cada fichero nuevo deja su version en historial.")
    } else {
        // ** UN "NO" QUE NO DICE CUAL ES UN "NO" QUE NO SIRVE: la bandera es una
        // Y de cuatro condiciones, y cada una manda a mirar un sitio distinto.
        let montado = bmo::info(bmo::INFO_ES_MONTADO) != 0;
        let mio = bmo::info(bmo::INFO_ES_IDENTIDAD) != 0;
        let cabe = bmo::info(bmo::INFO_ES_NIVEL) < 3;
        let porque = if !montado {
            "no hay volumen montado: se formatea con estratos-fmt."
        } else if !mio {
            "el volumen NO nacio en este disco: no se le escribe."
        } else if !cabe {
            "por encima del 95%: solo lectura hasta que se recoja."
        } else {
            "el gate de identidad del disco no armo la escritura."
        };
        (porque, "sin esto, sellar no escribe y el recorte tampoco.")
    };
    p.texto(x0 + 36, ey + 15 + bmo::GLIFO_ALTO + 8, l1, INK_DIM);
    p.texto(x0 + 36, ey + 15 + 2 * (bmo::GLIFO_ALTO + 6), l2, INK_DIM);
}

/// Un numero de bytes con su unidad. Devuelve la x donde acabo.
///
/// Sin coma flotante: la parte fraccionaria sale de multiplicar el resto por
/// cien antes de dividir. Es la misma cuenta que hace el panel del kernel, y
/// esta aqui duplicada a proposito -- cruzar el anillo para formatear un numero
/// seria exactamente lo que un library OS no hace.
fn magnitude(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unit, div) = if bytes >= G {
        ("GiB", G)
    } else if bytes >= M {
        ("MiB", M)
    } else if bytes >= K {
        ("KiB", K)
    } else {
        ("B", 1)
    };
    let mut b = [0u8; 10];
    let n = decimal(bytes / div, &mut b);
    let mut x = p.texto_bytes(x, y, &b[..n], color);
    if div > 1 {
        let frac = (bytes % div) * 100 / div;
        x = p.texto(x, y, ".", color);
        if frac < 10 {
            x = p.texto(x, y, "0", color);
        }
        let n = decimal(frac, &mut b);
        x = p.texto_bytes(x, y, &b[..n], color);
    }
    x = p.texto(x, y, " ", color);
    p.texto(x, y, unit, color)
}
