//! **LA LUZ DEL GSP**: en el panel, por donde va el arranque del GSP de la
//! 3060 -- un camino de siete nodos, uno por escalon de L0 que se ve desde
//! fuera, y una palabra con su luz. Sin abrir `gpu` ni leer el INFORME.
//!
//! [consumo] LATE      ocho preguntas al kernel cada 250 ms, con el panel a la
//!                     vista; se repinta si algo cambio, o a 4 Hz mientras la
//!                     luz respira
//!
//! ```text
//!    gsp           * despierto     la palabra: el ultimo escalon, o donde fallo
//!    o---o---o---o---o---o---o     el camino, de izquierda a derecha:
//!    W   R   L   S   D   Q   I
//!
//!      W  WPR2 montada por FWSEC (L0b)       D  el RISC-V se vio activo (L0c3b)
//!      R  el GSP-RM prestado, radix3 (L0c2)  Q  secuenciador corrido (L0c4b2c)
//!      L  LIBOS prestado y comprobado (L0c3a) I  GSP_INIT_DONE (L0c4b2c)
//!      S  SetSystemInfo escrito (L0c4b2a)
//! ```
//!
//! Hecho, un punto del acento y la via hasta el, del acento (todo verde
//! cuando llega `GSP_INIT_DONE`); el que fallo, rojo; el siguiente, un anillo
//! del acento; los de despues, anillos apagados. La luz de delante de la
//! palabra RESPIRA mientras el RISC-V del GSP esta activo: vivo se ve vivo,
//! como la aguja del pulso. Lo pidio el propietario (24-09): *"que si
//! despierta algo mi GPU se lee"*, y luego *"mas elegante"*.

use bmo_userland as bmo;

use super::estilo::estilo;
use super::{acento, INK, INK_DIM};

/// Las casillas y como se llaman cuando son la ultima hecha.
const PASOS: usize = 7;
const PALABRAS: [&str; PASOS] = ["WPR2", "prestado", "libos", "escrito", "despierto", "reanudado", "LISTO"];
/// Las de fallo, de 11 letras como mucho: con la luz y `gsp` delante, 12 ya
/// las pegan (visto en la maqueta del 24-09).
const NOMBRES_NO: [&str; PASOS] = ["NO fwsec", "NO radix", "NO libos", "NO sistema", "NO desperto", "NO secuen", "NO init"];

const VERDE: u32 = 0x0022_C55E;
const ROJO: u32 = 0x00EF_4444;

/// Lo que mide el bloque: la palabra, el camino de nodos y sus letras.
pub(crate) const ALTO: u32 = bmo::GLIFO_ALTO + 8 + NODO + 4 + bmo::GLIFO_ALTO;
/// Un nodo: un punto de 8 px.
const NODO: u32 = 8;
/// La letra de cada nodo, bajo el.
const LETRAS: [u8; PASOS] = *b"WRLSDQI";

/// Como va, leido: las casillas hechas (bit k = escalon k), el que fallo
/// (`PASOS` = ninguno), si hay NVIDIA y si su RISC-V esta activo ahora.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Estado {
    hechos: u8,
    fallo: usize,
    hallada: bool,
    riscv: bool,
    /// Con el RISC-V vivo, la luz respira: cambia de tono cada muestra, y el
    /// bloque se repinta a 4 Hz solo mientras tanto.
    respira: bool,
}

/// Lo pintado, para no repintar si no cambio.
static mut PINTADO: Option<Estado> = None;
/// Como acabo `gpu init` (solo lo sabe el escritorio, no el kernel): lo
/// apunta `commands::gspinit` -- `commands` puede llamar a `scene`, no al
/// reves (L8, `capas.py`). 0 sin intentar, 1 GSP_INIT_DONE, 2 no llego.
static mut INIT: u8 = 0;
/// La fase de la respiracion de la luz.
static mut FASE: bool = false;

/// **`gpu init` acabo**: con `GSP_INIT_DONE` o sin el.
pub(crate) fn init(llego: bool) {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { INIT = if llego { 1 } else { 2 } };
}

/// Donde esta, en `INFO_GPU_GSP_MEM`, el `writePtr` de la cola de la CPU:
/// tras los tres logs (3 x 16 paginas), GspMem + 0x1000 + 16. El mismo sitio
/// que lee `commands::gspsistema`.
const CPU_ESCRITO: u64 = 3 * 16 * 4096 + 0x1000 + 16;

/// Lo pintado se da por perdido (el panel se repinto entero).
pub(crate) fn olvidar() {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { PINTADO = None };
}

fn leer() -> Estado {
    let hallada = bmo::info(bmo::INFO_GPU_CHIP) & bmo::GPU_HALLADA != 0;
    let d = bmo::info(bmo::INFO_GPU_DESPIERTO);
    let sec = bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 2 << 8);
    let como = sec >> 24 & 0xFF;
    let g = bmo::info(bmo::INFO_GPU_GSP);
    // SAFETY: el escritorio es un solo hilo.
    let init = unsafe { INIT };
    // Las mismas preguntas que los pasos de `save mode`, hechas aqui.
    let hechos = [
        (bmo::info(bmo::INFO_GPU_WPR2) >> 32) as u32 >> 4 != 0,
        g & bmo::GSP_PRESTADO != 0 && g & bmo::GSP_CUADRA != 0 && g & bmo::GSP_ES_570 != 0,
        bmo::info(bmo::INFO_GPU_LIBOS) & bmo::LIBOS_COMPROBADO != 0,
        bmo::info(bmo::INFO_GPU_GSP_MEM | CPU_ESCRITO << 8) & 0xFFFF_FFFF != 0,
        d & bmo::DESPIERTO_VISTO != 0,
        como & bmo::SEC_HECHO != 0,
        init == 1,
    ];
    let mut bits = 0u8;
    for (k, &h) in hechos.iter().enumerate() {
        bits |= (h as u8) << k;
    }
    // Donde fallo, si fallo: el despertar con su NO apuntado, el secuenciador
    // parado en una orden, o el GSP-RM sin su GSP_INIT_DONE.
    let fallo = if d & bmo::DESPIERTO_VALIDO != 0 && d & bmo::DESPIERTO_VISTO == 0 && (d >> bmo::DESPIERTO_MOTIVO_SHIFT) & 0xFF != 0 {
        4
    } else if como & 0x3F != 0 {
        5
    } else if init == 2 {
        6
    } else {
        PASOS
    };
    let riscv = d & bmo::DESPIERTO_RISCV_ACTIVO != 0;
    // SAFETY: el escritorio es un solo hilo.
    let respira = riscv && unsafe {
        FASE = !FASE;
        FASE
    };
    Estado { hechos: bits, fallo, hallada, riscv, respira }
}

/// `a` hacia `b`, `t` de 256.
fn mezcla(a: u32, b: u32, t: u32) -> u32 {
    let c = |d: u32| {
        let (x, y) = ((a >> d) & 0xFF, (b >> d) & 0xFF);
        ((x * (256 - t) + y * t) / 256) << d
    };
    c(16) | c(8) | c(0)
}

/// Un punto de 8 px: cuatro filas de sangria por arriba y por abajo (el mismo
/// truco de la tabla de `rounded_rect`, a escala de un nodo).
fn punto(p: &bmo::Pantalla, x: u32, y: u32, color: u32) {
    for (f, s) in [2u32, 1, 0, 0, 0, 0, 1, 2].into_iter().enumerate() {
        p.rect(x + s, y + f as u32, NODO - 2 * s, 1, color);
    }
}

/// Un anillo: el punto, y dentro uno de 6 px del fondo.
fn anillo(p: &bmo::Pantalla, x: u32, y: u32, color: u32, fondo: u32) {
    punto(p, x, y, color);
    for (f, s) in [1u32, 0, 0, 0, 0, 1].into_iter().enumerate() {
        p.rect(x + 1 + s, y + 1 + f as u32, NODO - 2 - 2 * s, 1, fondo);
    }
}

/// **Pintar el bloque** en `(x0, y)`, `iw` de ancho, si cambio.
///
/// ```text
///    gsp             * LISTO     la palabra, con su luz delante
///    o---o---o---o---o---o---o   el camino: hecho, del acento; lo que falta,
///    W   R   L   S   D   Q   I   un anillo; el siguiente, un anillo del acento
/// ```
pub(crate) fn pintar(p: &bmo::Pantalla, x0: u32, y: u32, iw: u32) {
    let e = leer();
    // SAFETY: el escritorio es un solo hilo.
    if unsafe { PINTADO } == Some(e) {
        return;
    }
    unsafe { PINTADO = Some(e) };
    let est = estilo();
    let (fondo, borde) = (est.barra_fondo, est.barra_borde);
    p.rect(x0, y, iw, ALTO, fondo);

    // La palabra, y su luz.
    let listo = e.hechos & 1 << 6 != 0;
    let ultimo = (0..PASOS).rev().find(|&k| e.hechos & 1 << k != 0);
    let hecho = if listo { VERDE } else { acento() };
    let (palabra, tinta, luz) = if !e.hallada {
        ("sin NVIDIA", INK_DIM, borde)
    } else if e.fallo < PASOS {
        (NOMBRES_NO[e.fallo], ROJO, ROJO)
    } else if listo {
        ("LISTO", VERDE, VERDE)
    } else {
        match ultimo {
            None => ("dormida", INK_DIM, borde),
            // Despierto, sin secuenciador corrido y el RISC-V parado: el GSP-RM
            // se paro solo y espera al secuenciador.
            Some(4) if !e.riscv => ("espera sec", INK, acento()),
            Some(k) => (PALABRAS[k], INK, acento()),
        }
    };
    // Vivo, respira: la mitad del tono una muestra si y otra no.
    let luz = if e.respira { mezcla(luz, fondo, 128) } else { luz };
    p.texto(x0, y, "gsp", INK_DIM);
    let px = (x0 + iw).saturating_sub(palabra.len() as u32 * bmo::GLIFO_ANCHO);
    p.texto(px, y, palabra, tinta);
    punto(p, px.saturating_sub(NODO + 6), y + (bmo::GLIFO_ALTO - NODO) / 2, luz);

    // El camino: los nodos repartidos de punta a punta, y la via entre ellos.
    let ny = y + bmo::GLIFO_ALTO + 8;
    let paso = (iw - NODO) / (PASOS as u32 - 1);
    let nx = |k: usize| x0 + k as u32 * paso;
    for k in 0..PASOS - 1 {
        let via = if e.hechos & 1 << (k + 1) != 0 { hecho } else { borde };
        p.rect(nx(k) + NODO, ny + NODO / 2 - 1, paso - NODO, 2, via);
    }
    let siguiente = ultimo.map_or(0, |k| k + 1);
    for k in 0..PASOS {
        let x = nx(k);
        if k == e.fallo {
            punto(p, x, ny, ROJO);
        } else if e.hechos & 1 << k != 0 {
            punto(p, x, ny, hecho);
        } else if k == siguiente && e.hallada && e.fallo == PASOS {
            anillo(p, x, ny, acento(), fondo);
        } else {
            anillo(p, x, ny, borde, fondo);
        }
        // Su letra, centrada bajo el nodo: clara la ultima hecha, roja la que
        // fallo, apagadas las demas.
        let tinta = if k == e.fallo {
            ROJO
        } else if Some(k) == ultimo {
            INK
        } else {
            INK_DIM
        };
        let lx = (x + NODO / 2).saturating_sub(bmo::GLIFO_ANCHO / 2);
        p.texto_bytes(lx, ny + NODO + 4, &LETRAS[k..k + 1], tinta);
    }
}
