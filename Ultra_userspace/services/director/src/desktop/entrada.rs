//! **LA ENTRADA DE EJECUTAR** (2026-09-29). Pedido del propietario: *"cambiar
//! el estilo de mi control + alt que se vea mucho mejor MAS EPICO ... con ASCII
//! de entrada epica"*.
//!
//! [consumo] LATE      ~1,1 s a ~30 fotogramas por segundo tras cada Ctrl+Alt
//!                     que la invoca ([`anima`], que solo mira el reloj); en
//!                     reposo, nada (L6h)
//!
//! Al invocarla, su SALIDA se enciende un segundo con el titulo en ASCII:
//!
//! ```text
//!    0..450 ms     el titulo aparece columna a columna, con el borde del
//!                  barrido en blanco y alguna fila que salta (el fallo de
//!                  imagen de un monitor de Night City)
//!    450..900 ms   el lema se teclea debajo
//!    900..1100 ms  todo se apaga hacia el fondo
//! ```
//!
//! Se pinta DENTRO de la salida, que es de esta ventana: no tapa nada ajeno y
//! no hace falta guardar lo de debajo. Al acabar se pide repintar la salida y
//! vuelve lo que habia. Si se escribe una orden mientras tanto, se corta en
//! seco: lo que conteste la orden importa mas que la entrada.
//!
//! [!] Y NO escribe nada en la salida. El 24-09 cada Ctrl+Alt dejaba tres
//! lineas entre las respuestas y la caja parecia "MAS mezclada" (el
//! propietario). Esto es luz, no texto: se va y no deja rastro.

use bmo_userland as bmo;

use crate::scene::globo::mezcla;
use crate::scene::{RunBox, BOX_BG, OUT_COLS};

const DURA_MS: u64 = 1100;
const REVELA_MS: u64 = 450;
const LEMA_MS: u64 = 450;
const APAGA_MS: u64 = 200;
const FOTOGRAMA_MS: u64 = 33;

const AMARILLO: u32 = 0x00FC_EE0A;
const CIAN: u32 = 0x0000_F0FF;
const MAGENTA: u32 = 0x00FF_2BD6;
const BLANCO: u32 = 0x00FF_FFFF;

/// `BMO-X` en la letra `standard` de figlet.
const ARTE: [&[u8]; 5] = [
    b" ____  __  __  ___        __  __",
    b"| __ )|  \\/  |/ _ \\       \\ \\/ /",
    b"|  _ \\| |\\/| | | | |_____  \\  / ",
    b"| |_) | |  | | |_| |_____| /  \\ ",
    b"|____/|_|  |_|\\___/       /_/\\_\\",
];
const LEMA: &[u8] = b">> ENLACE ESTABLECIDO // BMO-X x86-64 // escribe una orden, o TAB";

struct Estado {
    por_ms: u64,
    desde: u64,
    pintado: u64,
    /// La linea escrita al invocarla: si sube, alguien escribio y se corta.
    marca: usize,
    vivo: bool,
}

static mut ESTADO: Estado = Estado { por_ms: 0, desde: 0, pintado: 0, marca: 0, vivo: false };

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

fn edad_ms(e: &Estado) -> u64 {
    bmo::ciclos().wrapping_sub(e.desde) / e.por_ms.max(1)
}

/// **Empieza la entrada**: Ctrl+Alt acaba de invocar la caja. `marca` es la
/// linea escrita de la salida ahora (`Output::mark`).
pub(crate) fn empezar(marca: usize) {
    let e = estado();
    if e.por_ms == 0 {
        e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
    }
    e.desde = bmo::ciclos();
    e.pintado = 0;
    e.marca = marca;
    e.vivo = true;
}

/// La caja se escondio: la entrada no sigue.
pub(crate) fn cancelar() {
    estado().vivo = false;
}

/// **Pide fotograma** mientras vive. Solo lee el reloj.
pub(crate) fn anima() -> bool {
    let e = estado();
    e.vivo && bmo::ciclos().wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms
}

/// **Acabo en este fotograma?** Al principio del fotograma que pinta, ANTES de
/// pintar la salida: si acabo (el tiempo, o alguien escribio), la salida se
/// repinta en este mismo fotograma y lo de la entrada se va.
pub(crate) fn acabo(marca: usize) -> bool {
    let e = estado();
    if !e.vivo {
        return false;
    }
    if marca != e.marca || edad_ms(e) >= DURA_MS {
        e.vivo = false;
        return true;
    }
    false
}

/// Un numero que salta de fotograma en fotograma, sin estado: el fallo de imagen.
fn ruido(k: u64) -> u64 {
    let mut x = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

/// **Pinta la entrada** encima de la salida, si vive. Despues de pintar la
/// salida, con Ejecutar arriba.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &RunBox) {
    let e = estado();
    if !e.vivo {
        return;
    }
    let t = edad_ms(e);
    e.pintado = bmo::ciclos();
    let (gw, gh) = (bmo::GLIFO_ANCHO, bmo::GLIFO_ALTO);
    let ancho = OUT_COLS as u32 * gw;
    let alto = c.out_h();
    p.rect(c.out_x, c.out_y, ancho, alto, BOX_BG);

    // Cuanto brilla: entero hasta el final, y se apaga hacia el fondo.
    let fuera = DURA_MS - APAGA_MS;
    let luz = if t < fuera { 256 } else { 256u64.saturating_sub((t - fuera) * 256 / APAGA_MS) as u32 };
    let tinta = |col: u32| mezcla(BOX_BG, col, luz);

    let cols = ARTE[0].len() as u32;
    let bloque_h = (ARTE.len() as u32 + 2) * gh;
    if alto < bloque_h || ancho < LEMA.len() as u32 * gw {
        return;
    }
    let x0 = c.out_x + (ancho - cols * gw) / 2;
    let y0 = c.out_y + (alto - bloque_h) / 2;
    // El barrido: cuantas columnas se ven ya.
    let visto = if t >= REVELA_MS { cols as usize } else { (t * cols as u64 / REVELA_MS) as usize };
    let cuadro = t / FOTOGRAMA_MS;
    for (f, linea) in ARTE.iter().enumerate() {
        let n = visto.min(linea.len());
        let y = y0 + f as u32 * gh;
        // El fallo de imagen: mientras se revela, alguna fila salta un glifo y
        // se tine de magenta, un fotograma de cada pocos.
        let r = ruido(cuadro * 7 + f as u64);
        let salta = t < REVELA_MS + 150 && r % 9 == 0;
        let dx = if salta { gw } else { 0 };
        let x = if salta && r & 1 == 0 { x0 + dx } else { x0.saturating_sub(dx) };
        // La aberracion: el cian un poco desplazado detras, el amarillo encima.
        p.texto_bytes(x + 2, y + 1, &linea[..n], tinta(if salta { MAGENTA } else { CIAN }));
        p.texto_bytes(x, y, &linea[..n], tinta(AMARILLO));
        // El borde del barrido, en blanco.
        if n < linea.len() {
            p.rect(x + n as u32 * gw, y, gw, gh, tinta(BLANCO));
        }
    }
    // Las lineas del tubo: una de cada tres, oscura, sobre el titulo.
    let alto_arte = ARTE.len() as u32 * gh;
    let mut yy = y0;
    while yy < y0 + alto_arte {
        p.rect(x0, yy, cols * gw + 2, 1, BOX_BG);
        yy += 3;
    }
    // El lema, tecleado.
    if t >= REVELA_MS {
        let k = (((t - REVELA_MS) * LEMA.len() as u64 / LEMA_MS) as usize).min(LEMA.len());
        let lx = c.out_x + (ancho - LEMA.len() as u32 * gw) / 2;
        let ly = y0 + (ARTE.len() as u32 + 1) * gh;
        let fin = p.texto_bytes(lx, ly, &LEMA[..k], tinta(CIAN));
        // El cursor del lema parpadea mientras se escribe.
        if k < LEMA.len() || cuadro % 8 < 4 {
            p.rect(fin, ly + gh - 3, gw, 2, tinta(AMARILLO));
        }
    }
}
