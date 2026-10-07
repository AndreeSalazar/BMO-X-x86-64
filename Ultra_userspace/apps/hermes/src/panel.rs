//! **EL PANEL de las tarjetas**, a la derecha, como el de la maqueta: lo que
//! la seccion ES, la conexion, las tres jaulas, lo que suena y por donde
//! sale. Si la ventana es estrecha no se pinta (`pintar::ancho_panel`).
//!
//! *** Lo mismo que el resto de HERMES: nada inventado. Sin red todavia, la
//! conexion dice con que se HARA (Noise IK, X25519, AES-256-GCM: lo de
//! `bmo-hermes`) y que el camino aun no existe; la huella, que no hay clave.

use crate::canvas::Canvas;
use crate::piezas::{caja, cara, negrita, negrita_fit, pildora, redonda, rotulo, ancho_txt, txt};
use crate::pintar::{
    suelo, ancho_panel, nivel, Vista, AMBAR, BARRA, BLANCO, CANAL, CIAN, ENVIOS, LIMA, LINEA, MENSAJES, MURO, ONDA,
    PAGINAS, PANEL, ROJO, ROSA, SECCIONES, TENUE, TERTULIAS, TEXTO, VERDE, AMIGOS,
};
use bmo_dibujo::{mezclar, Color, Lienzo};
use bmo_fondo::PIEZAS;

/// El fondo de las tarjetas: entre el panel y su borde.
const TARJETA: Color = 0x000D_1118;

/// Una tarjeta con su rotulo; devuelve donde empieza su contenido.
fn tarjeta(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, titulo: &[u8]) -> i32 {
    caja(cv, x, y, w, h, 10, TARJETA, LINEA);
    rotulo(cv, x + 14, y + 14, titulo, mezclar(TENUE, TARJETA, 200, 256));
    y + 42
}

/// Una fila de clave y valor, el valor pegado a la derecha.
fn par(cv: &mut Canvas, x: i32, y: i32, w: i32, clave: &[u8], valor: &[u8], c: Color) {
    txt(cv, x, y, clave, TENUE);
    txt(cv, x + w - ancho_txt(valor), y, valor, c);
}

/// Un texto partido en la tarjeta; devuelve cuantas lineas.
fn parrafo(cv: &mut Canvas, x: i32, y: i32, w: i32, t: &[u8], c: Color) -> i32 {
    let lineas = crate::piezas::partir(t, crate::piezas::T, w);
    for (k, l) in lineas.iter().enumerate() {
        txt(cv, x, y + k as i32 * 18, l, c);
    }
    lineas.len() as i32
}

/// Cuantas lineas ocupa un texto en una tarjeta de contenido `w`.
fn lineas(t: &[u8], w: i32) -> i32 {
    crate::piezas::partir(t, crate::piezas::T, w).len() as i32
}

pub(crate) fn panel(cv: &mut Canvas, v: &Vista, x0: i32) {
    let w = ancho_panel();
    cv.rect(x0, BARRA, w, suelo() - BARRA, PANEL);
    cv.rect(x0, BARRA, 1, suelo() - BARRA, LINEA);
    let (x, tw) = (x0 + 14, w - 28);
    let cw = tw - 28;
    let mut y = BARRA + 14;
    // Las tarjetas bajan un poco al cambiar de seccion, una tras otra.
    let baja = |y: i32, k: u32| y + (256 - crate::pintar::llega(v.ms, v.desde_sec, k * 70, 260)) * 14 / 256;

    if v.sec == ONDA {
        y = sonando(cv, v, x, baja(y, 0), tw);
        let siguiente: &[u8] = if v.sonando { b"la misma: el bucle no tiene costura y no se para" } else { b"La cola esta vacia." };
        let h = 42 + lineas(siguiente, cw) * 18 + 12;
        let yc = tarjeta(cv, x, baja(y, 1), tw, h, b"a continuacion");
        parrafo(cv, x + 14, yc, cw, siguiente, TEXTO);
        y += h + 14;
        let sale: &[u8] = b"La compone el ESCRITORIO en este procesador, pasa por el MAESTRO (oido, empuje y limite) y sale por el auricular USB. Sin cuentas, sin anuncios, sin algoritmo.";
        let h = 42 + lineas(sale, cw) * 18 + 12;
        let yc = tarjeta(cv, x, baja(y, 2), tw, h, b"por donde sale");
        parrafo(cv, x + 14, yc, cw, sale, TEXTO);
        return;
    }

    if crate::mision::es_mision() {
        // HM6b: con el escritorio de mision, arriba va el instrumento -- la
        // red de espacio profundo -- en el sitio de QUE ES (la seccion ya la
        // dice el centro, y con las dos el panel no cabe sobre el reproductor).
        crate::antena::pintar(cv, x, baja(y, 0), tw, v.ms);
        y += crate::antena::ALTO + 14;
    } else {
        // ** QUE ES: lo que la seccion es hoy, con su escalon.
        let (_, color, sub) = SECCIONES[v.sec];
        let h = 42 + lineas(sub, cw) * 18 + 12;
        let yc = tarjeta(cv, x, baja(y, 0), tw, h, b"que es");
        redonda(cv, x + tw - 22, baja(y, 0) + 16, 8, 8, 4, color);
        parrafo(cv, x + 14, yc, cw, sub, TEXTO);
        y += h + 14;
    }

    match v.sec {
        MENSAJES | TERTULIAS => {
            y = conexion(cv, x, baja(y, 1), tw) + 14;
            jaulas(cv, x, baja(y, 2), tw, v.ms);
        }
        MURO => {
            let yc = tarjeta(cv, x, baja(y, 1), tw, 42 + 3 * 22 + 6, b"lo que se lee");
            par(cv, x + 14, yc, cw, b"PNG", b"si", VERDE);
            par(cv, x + 14, yc + 22, cw, b"JPEG", b"si", VERDE);
            par(cv, x + 14, yc + 44, cw, b"de un amigo", b"H9", AMBAR);
        }
        CANAL => {
            let yc = tarjeta(cv, x, baja(y, 1), tw, 42 + 2 * 22 + 6, b"la emision");
            par(cv, x + 14, yc, cw, b"ahora", b"sin emision", ROJO);
            par(cv, x + 14, yc + 22, cw, b"llega con", b"H10", AMBAR);
        }
        PAGINAS => {
            let yc = tarjeta(cv, x, baja(y, 1), tw, 42 + 3 * 22 + 6, b"una pagina es");
            par(cv, x + 14, yc, cw, b"scripts", b"ninguno", VERDE);
            par(cv, x + 14, yc + 22, cw, b"Google", b"nunca", VERDE);
            par(cv, x + 14, yc + 44, cw, b"de amigos", b"H13-H15", AMBAR);
        }
        AMIGOS => {
            let yc = tarjeta(cv, x, baja(y, 1), tw, 42 + 2 * 22 + 6, b"tu huella x25519");
            par(cv, x + 14, yc, cw, b"clave", b"aun no", AMBAR);
            par(cv, x + 14, yc + 22, cw, b"se crea con", b"H6", AMBAR);
            y += 42 + 2 * 22 + 6 + 14;
            jaulas(cv, x, baja(y, 2), tw, v.ms);
        }
        ENVIOS | _ => jaulas(cv, x, baja(y, 1), tw, v.ms),
    }
}

/// **LA CONEXION**: con que se hablaran dos BMO-X, y que el camino aun no
/// existe. Devuelve donde acaba.
fn conexion(cv: &mut Canvas, x: i32, y: i32, tw: i32) -> i32 {
    let h = 42 + 44 + 4 * 22 + 8;
    let yc = tarjeta(cv, x, y, tw, h, b"la conexion");
    let cw = tw - 28;
    cara(cv, x + 30, yc + 14, 16, CIAN, b'N', TARJETA);
    negrita(cv, x + 54, yc + 0, b"tus notas", BLANCO);
    txt(cv, x + 54, yc + 18, b"esta maquina, sin red", TENUE);
    let yk = yc + 44;
    par(cv, x + 14, yk, cw, b"Saludo", b"Noise IK", TEXTO);
    par(cv, x + 14, yk + 22, cw, b"Acuerdo", b"X25519", TEXTO);
    par(cv, x + 14, yk + 44, cw, b"Cifrado", b"AES-256-GCM", TEXTO);
    par(cv, x + 14, yk + 66, cw, b"Camino", b"aun no: H4", AMBAR);
    y + h
}

/// **LAS TRES JAULAS**: quien tiene red y quien no.
fn jaulas(cv: &mut Canvas, x: i32, y: i32, tw: i32, ms: u32) {
    const TRES: [(&[u8], &[u8], Color, &[u8], bool); 3] = [
        (b"PUERTA", b"la red y tu clave", AMBAR, b"RED", false),
        (b"HERMES", b"esta ventana", CIAN, b"NINGUNA", true),
        (b"JUEZ", b"lo que llega", ROSA, b"NINGUNA", false),
    ];
    let h = 42 + 3 * 42 + 6;
    let yc = tarjeta(cv, x, y, tw, h, b"las tres jaulas");
    for (k, &(nombre, que, c, red, vive)) in TRES.iter().enumerate() {
        let yy = yc + k as i32 * 42;
        let luz = if vive && ms / 600 % 2 == 1 { mezclar(c, TARJETA, 110, 256) } else { c };
        redonda(cv, x + 14, yy + 4, 8, 8, 2, luz);
        negrita_fit(cv, x + 30, yy, nombre, BLANCO, 80);
        txt(cv, x + 30, yy + 18, que, TENUE);
        let rc = if red == b"RED" { ROJO } else { mezclar(TEXTO, TARJETA, 150, 256) };
        pildora(cv, x + tw - 12, yy + 2, red, rc, TARJETA);
    }
}

/// **SONANDO**: tres ondas que siguen al medidor del maestro, y la pieza.
fn sonando(cv: &mut Canvas, v: &Vista, x: i32, y: i32, tw: i32) -> i32 {
    let h = 42 + 120 + 12 + 40 + 6;
    let yc = tarjeta(cv, x, y, tw, h, b"sonando");
    let (gx, gw, gh) = (x + 14, tw - 28, 120);
    redonda(cv, gx, yc, gw, gh, 8, 0x0006_080C);
    let fuerza = if v.sonando { (nivel(v.rms[0]) + nivel(v.rms[1])) / 2 } else { 0 };
    let k = v.pedida.unwrap_or(v.item) % PIEZAS.len();
    let color = if v.sonando { crate::onda::color_pieza(k) } else { LIMA };
    let medio = yc + gh / 2;
    for (o, &(tono, parte)) in [(LIMA, 256u32), (CIAN, 160), (color, 110)].iter().enumerate() {
        let amp = 6 + (fuerza * (gh / 2 - 12) / 256) * (3 - o as i32) / 3;
        let periodo = 70 + o as i32 * 23;
        let mut antes = (gx, medio);
        for px in 0..gw {
            let a = (px * 256 / periodo) + (v.ms / (9 + o as u32 * 4)) as i32;
            let py = medio + crate::mates::seno(a) * amp / 256 + (o as i32 - 1) * 10;
            let ahora = (gx + px, py);
            cv.line(antes, ahora, mezclar(tono, 0x0006_080C, parte, 256));
            antes = ahora;
        }
    }
    let ty = yc + gh + 12;
    if v.sonando {
        negrita_fit(cv, x + 14, ty, PIEZAS[k].nombre.as_bytes(), BLANCO, tw - 28);
        txt(cv, x + 14, ty + 20, b"suena en el escritorio", mezclar(color, TEXTO, 120, 256));
    } else {
        txt(cv, x + 14, ty, b"Nada todavia. Elige una", TENUE);
        txt(cv, x + 14, ty + 20, b"pieza y dale a tocar.", TENUE);
    }
    y + h + 14
}
