//! **LA LUZ DEL GSP**: en el panel, por donde va el arranque del GSP de la
//! 3060 -- siete casillas, una por escalon de L0 que se ve desde fuera, y una
//! palabra. Sin abrir `gpu` ni leer el INFORME.
//!
//! [consumo] LATE      ocho preguntas al kernel cada 250 ms, con el panel a la
//!                     vista; se repinta solo si algo cambio
//!
//! ```text
//!    gsp              LISTO        la palabra: el ultimo escalon, o donde fallo
//!    [W][R][L][S][D][Q][I]         las casillas, de izquierda a derecha:
//!
//!      W  WPR2 montada por FWSEC (L0b)       D  el RISC-V se vio activo (L0c3b)
//!      R  el GSP-RM prestado, radix3 (L0c2)  Q  secuenciador corrido (L0c4b2c)
//!      L  LIBOS prestado y comprobado (L0c3a) I  GSP_INIT_DONE (L0c4b2c)
//!      S  SetSystemInfo escrito (L0c4b2a)
//! ```
//!
//! Hecha, del acento (verdes todas cuando llega `GSP_INIT_DONE`); la que
//! fallo, roja; las de despues, apagadas. Lo pidio el propietario (24-09):
//! *"que si despierta algo mi GPU se lee"*.

use bmo_userland as bmo;

use super::estilo::estilo;
use super::{acento, INK, INK_DIM};

/// Las casillas y como se llaman cuando son la ultima hecha.
const PASOS: usize = 7;
const PALABRAS: [&str; PASOS] = ["WPR2", "prestado", "libos", "escrito", "despierto", "reanudado", "LISTO"];
const NOMBRES_NO: [&str; PASOS] = ["NO fwsec", "NO radix", "NO libos", "NO sistema", "NO despierta", "NO secuen", "NO init"];

const VERDE: u32 = 0x0022_C55E;
const ROJO: u32 = 0x00EF_4444;

/// Lo que mide el bloque: la palabra y las casillas.
pub(crate) const ALTO: u32 = bmo::GLIFO_ALTO + 4 + CASILLA_H;
const CASILLA_H: u32 = 8;
const ENTRE: u32 = 4;

/// Como va, leido: las casillas hechas (bit k = escalon k), el que fallo
/// (`PASOS` = ninguno), si hay NVIDIA y si su RISC-V esta activo ahora.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Estado {
    hechos: u8,
    fallo: usize,
    hallada: bool,
    riscv: bool,
}

/// Lo pintado, para no repintar si no cambio.
static mut PINTADO: Option<Estado> = None;
/// Como acabo `gpu init` (solo lo sabe el escritorio, no el kernel): lo
/// apunta `commands::gspinit` -- `commands` puede llamar a `scene`, no al
/// reves (L8, `capas.py`). 0 sin intentar, 1 GSP_INIT_DONE, 2 no llego.
static mut INIT: u8 = 0;

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
    Estado { hechos: bits, fallo, hallada, riscv: d & bmo::DESPIERTO_RISCV_ACTIVO != 0 }
}

/// **Pintar el bloque** en `(x0, y)`, `iw` de ancho, si cambio.
pub(crate) fn pintar(p: &bmo::Pantalla, x0: u32, y: u32, iw: u32) {
    let e = leer();
    // SAFETY: el escritorio es un solo hilo.
    if unsafe { PINTADO } == Some(e) {
        return;
    }
    unsafe { PINTADO = Some(e) };
    let fondo = estilo().barra_fondo;
    p.rect(x0, y, iw, ALTO, fondo);

    // La palabra.
    let listo = e.hechos & 1 << 6 != 0;
    let ultimo = (0..PASOS).rev().find(|&k| e.hechos & 1 << k != 0);
    let (palabra, tinta) = if !e.hallada {
        ("sin NVIDIA", INK_DIM)
    } else if e.fallo < PASOS {
        (NOMBRES_NO[e.fallo], ROJO)
    } else if listo {
        ("LISTO", VERDE)
    } else {
        match ultimo {
            None => ("dormida", INK_DIM),
            // Despierto, sin secuenciador corrido y el RISC-V parado: el GSP-RM
            // se paro solo y espera al secuenciador.
            Some(4) if !e.riscv => ("espera sec", INK),
            Some(k) => (PALABRAS[k], INK),
        }
    };
    p.texto(x0, y, "gsp", INK_DIM);
    let px = (x0 + iw).saturating_sub(palabra.len() as u32 * bmo::GLIFO_ANCHO);
    p.texto(px, y, palabra, tinta);

    // Las casillas.
    let cw = (iw - (PASOS as u32 - 1) * ENTRE) / PASOS as u32;
    let cy = y + bmo::GLIFO_ALTO + 4;
    for k in 0..PASOS {
        let cx = x0 + k as u32 * (cw + ENTRE);
        let color = if k == e.fallo {
            ROJO
        } else if e.hechos & 1 << k != 0 {
            if listo { VERDE } else { acento() }
        } else {
            estilo().barra_borde
        };
        p.rect(cx, cy, cw, CASILLA_H, color);
    }
}
