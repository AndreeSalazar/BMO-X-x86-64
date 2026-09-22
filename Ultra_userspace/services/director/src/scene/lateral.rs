//! **LA BARRA LATERAL EN VIVO** (HUD 3, 2026-09-22): lo que la maquina hace
//! AHORA, sin abrir nada.
//!
//! [consumo] LATE      4 muestras por segundo con la barra a la vista; ninguna
//!                     escondida o con una app a pantalla completa (L6h)
//!
//! El motivo, uno: **ver lo que hace la maquina sin abrir nada.** Hasta hoy la
//! CPU, la memoria y los vatios eran una cifra suelta en la barra de arriba, y
//! la HISTORIA --si sube, si baja, si hubo un pico hace diez segundos-- solo
//! estaba dentro de F7/F8. Aqui cada instrumento es una cifra y su grafica de
//! los ultimos once segundos.
//!
//! # El sitio: una columna RESERVADA
//!
//! Como la `exclusive zone` de una barra de Hyprland: ninguna ventana entra en
//! la columna. `margen()` la dice, y la leen `chrome::area_util` (encajar,
//! maximizar, el mosaico), los topes del arrastre y la rejilla de iconos, que
//! se corre a la derecha. Por eso la barra nunca queda tapada, y su repintado
//! de 4 Hz no puede pintar encima de una ventana.
//!
//! # Lo que NO hace
//!
//! No es un lanzador ni una lista de ventanas: eso ya lo son la rejilla y las
//! fichas de arriba, y dos sitios para lo mismo es la incoherencia que el
//! estudio del 22-09 cuenta de Windows (Panel de control + Configuracion).

use bmo_userland as bmo;
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, Ordering};

use super::chrome::HUECO;
use super::estilo::estilo;
use super::{acento, inside_rounded, rounded_rect, INK, INK_DIM, TASKBAR_H};

/// El ancho de la columna: trece letras (`sonido -12 dB`) y el aire.
pub(crate) const ANCHO: u32 = 112;

static VISIBLE: AtomicBool = AtomicBool::new(true);

/// Esta a la vista?
pub(crate) fn visible() -> bool {
    VISIBLE.load(Ordering::Relaxed)
}

/// Super+B: esconderla o traerla. Quien llama repinta el escritorio: la
/// rejilla y el area util cambian de sitio.
pub(crate) fn alternar() {
    VISIBLE.store(!visible(), Ordering::Relaxed);
    olvidar();
}

/// **Lo que la columna le quita a las ventanas**, por la izquierda: la barra y
/// el hueco que la separa del borde. Cero si esta escondida.
pub(crate) fn margen() -> u32 {
    if visible() {
        HUECO + ANCHO
    } else {
        0
    }
}

/// `(x, y, ancho, alto)` de la pastilla.
pub(crate) fn caja(p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
    (HUECO, TASKBAR_H + HUECO, ANCHO, p.alto.saturating_sub(TASKBAR_H + 2 * HUECO))
}

/// **El color en `(x, y)` si es de la barra lateral**: la pastilla y su borde.
/// Lo pregunta `scene_color`, que es quien contesta al borrar el cursor. Las
/// graficas no se modelan: se repintan solas a 4 Hz, y ninguna ventana puede
/// taparlas.
pub(crate) fn color_en(x: u32, y: u32, alto: u32) -> Option<u32> {
    if !visible() || x >= HUECO + ANCHO || y < TASKBAR_H {
        return None;
    }
    let (bx, by, bw, bh) = (HUECO, TASKBAR_H + HUECO, ANCHO, alto.saturating_sub(TASKBAR_H + 2 * HUECO));
    if !inside_rounded(x, y, bx, by, bw, bh) {
        return None;
    }
    let e = estilo();
    if !inside_rounded(x, y, bx + 1, by + 1, bw - 2, bh - 2) {
        return Some(e.barra_borde);
    }
    Some(e.barra_fondo)
}

// ===================================================================
//  Los instrumentos y su historia
// ===================================================================

/// Muestras de historia: 44 a 4 por segundo son once segundos, y a 2 px cada
/// una llenan la grafica de 88.
const HISTORIA: usize = 44;
const INSTRUMENTOS: usize = 5;
const CPU: usize = 0;
const MEM: usize = 1;
const VATIOS: usize = 2;
const PULSO: usize = 3;
const SONIDO: usize = 4;

const NOMBRES: [&str; INSTRUMENTOS] = ["cpu", "memoria", "vatios", "pulso", "sonido"];

static mut HIST: [[u32; HISTORIA]; INSTRUMENTOS] = [[0; HISTORIA]; INSTRUMENTOS];
static mut LLENAS: usize = 0;
static mut PROXIMO: u64 = 0;
static mut FORZAR: bool = true;
static mut PREV_TSC: u64 = 0;
static mut PREV_REPOSO: u64 = 0;

/// Lo pintado se da por perdido: el fondo se repinto debajo.
pub(crate) fn olvidar() {
    unsafe { FORZAR = true };
}

/// El alto de cada instrumento: su renglon y su grafica.
const RENGLON: u32 = bmo::GLIFO_ALTO + 4;
const GRAF_ALTO: u32 = 30;
const SECCION: u32 = RENGLON + GRAF_ALTO + 14;

fn empuja(i: usize, v: u32) {
    unsafe {
        let h = &mut (*addr_of_mut!(HIST))[i];
        h.copy_within(1.., 0);
        h[HISTORIA - 1] = v;
    }
}

/// **La vuelta**: si toca (cada 250 ms) se toma una muestra de cada
/// instrumento y se repinta la columna. `mw` son los milivatios del paquete y
/// `vueltas` las del escritorio en el ultimo segundo.
pub(crate) fn latido(p: &bmo::Pantalla, mw: Option<u64>, vueltas: u32) {
    if !visible() {
        return;
    }
    let ahora = bmo::ciclos();
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let forzar = unsafe { FORZAR };
    if !forzar && ahora < unsafe { PROXIMO } {
        return;
    }
    unsafe {
        PROXIMO = ahora + hz / 4;
        FORZAR = false;
    }

    // -- Las muestras, las mismas cuentas que la barra de arriba --
    let reposo = bmo::info(bmo::INFO_BSP_TICKS_REPOSO);
    let cpu = unsafe {
        let v = if PREV_TSC != 0 && ahora > PREV_TSC && reposo >= PREV_REPOSO {
            let dt = ahora - PREV_TSC;
            let dr = (reposo - PREV_REPOSO).min(dt);
            (100 - dr * 100 / dt) as u32
        } else {
            0
        };
        PREV_TSC = ahora;
        PREV_REPOSO = reposo;
        v
    };
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let usada_mib = (total.saturating_sub(bmo::info(bmo::INFO_RAM_LIBRE)) / (1024 * 1024)) as u32;
    let watios_d = mw.map(|m| (m / 100) as u32).unwrap_or(0); // decimas de W
    // El medidor del maestro: lo mas alto de los dos lados, de -60 a 0 dBFS.
    let med = bmo::info(bmo::INFO_AUDIO_MEDIDOR);
    let lado = |bit: u32| ((med >> bit) & 0xFFFF) as u16 as i16 as i32;
    let pico_db = lado(0).max(lado(16)); // 1/256 dB
    let sonido = ((pico_db + 60 * 256).max(0) * 100 / (60 * 256)).min(100) as u32;

    if !forzar {
        empuja(CPU, cpu);
        empuja(MEM, usada_mib);
        empuja(VATIOS, watios_d);
        empuja(PULSO, vueltas);
        empuja(SONIDO, sonido);
        unsafe { LLENAS = (LLENAS + 1).min(HISTORIA) };
    }

    // -- Pintar --
    let (bx, by, bw, bh) = caja(p);
    let e = estilo();
    if forzar {
        rounded_rect(p, bx, by, bw, bh, e.barra_borde);
        rounded_rect(p, bx + 1, by + 1, bw - 2, bh - 2, e.barra_fondo);
    }
    let x0 = bx + 12;
    let gw = (HISTORIA as u32) * 2;
    let mut y = by + 12;
    for i in 0..INSTRUMENTOS {
        if y + SECCION > by + bh {
            break;
        }
        let hist = unsafe { &(*addr_of!(HIST))[i] };
        let ultimo = hist[HISTORIA - 1];
        // El renglon: el nombre apagado y la cifra en claro.
        p.rect(x0, y, bw - 24, RENGLON, e.barra_fondo);
        let mut t = [0u8; 12];
        let mut n = 0usize;
        let mut pon = |s: &[u8]| {
            for &b in s {
                if n < t.len() {
                    t[n] = b;
                    n += 1;
                }
            }
        };
        let mut d = [0u8; 10];
        match i {
            CPU => {
                let k = crate::text::decimal(ultimo as u64, &mut d);
                pon(&d[..k]);
                pon(b"%");
            }
            MEM => {
                let k = crate::text::decimal(ultimo as u64, &mut d);
                pon(&d[..k]);
                pon(b"M");
            }
            VATIOS => {
                let k = crate::text::decimal((ultimo / 10) as u64, &mut d);
                pon(&d[..k]);
                pon(b".");
                pon(&[b'0' + (ultimo % 10) as u8]);
            }
            PULSO => {
                let k = crate::text::decimal(ultimo as u64, &mut d);
                pon(&d[..k]);
                pon(b"/s");
            }
            _ => {
                if pico_db <= -60 * 256 {
                    pon(b"--");
                } else {
                    let db = (-pico_db + 128) / 256;
                    pon(b"-");
                    let k = crate::text::decimal(db as u64, &mut d);
                    pon(&d[..k]);
                }
            }
        }
        p.texto(x0, y + 2, NOMBRES[i], INK_DIM);
        let cx = (x0 + gw).saturating_sub(n as u32 * bmo::GLIFO_ANCHO);
        p.texto_bytes(cx, y + 2, &t[..n], INK);

        // La grafica: de la mas vieja (izquierda) a la de ahora (derecha).
        let gy = y + RENGLON;
        p.rect(x0, gy, gw, GRAF_ALTO, e.barra_fondo);
        p.rect(x0, gy + GRAF_ALTO - 1, gw, 1, e.barra_borde);
        // La escala: fija donde el numero tiene techo (%), y si no, la mayor
        // de la ventana con aire -- una grafica que siempre toca el techo no
        // dice nada, y una que nunca sube tampoco.
        let techo = match i {
            CPU | SONIDO => 100,
            _ => {
                let mut m = 1u32;
                for &v in hist.iter() {
                    m = m.max(v);
                }
                m + m / 4 + 1
            }
        };
        let llenas = unsafe { LLENAS };
        for (k, &v) in hist.iter().enumerate() {
            if k + llenas < HISTORIA {
                continue;
            }
            let h = (v.min(techo) * (GRAF_ALTO - 2) / techo).max(if v > 0 { 1 } else { 0 });
            if h == 0 {
                continue;
            }
            let color = match i {
                SONIDO if v >= 95 => 0x00EF_4444,
                SONIDO if v >= 80 => 0x00EA_B308,
                SONIDO => 0x0022_C55E,
                CPU if v >= 90 => 0x00EF_4444,
                _ => acento(),
            };
            p.rect(x0 + k as u32 * 2, gy + GRAF_ALTO - 1 - h, 2, h, color);
        }
        y += SECCION;
    }
    // Al pie, como se esconde: el atajo dicho donde se mira.
    let pie = by + bh - RENGLON - 6;
    if pie > y {
        p.rect(x0, pie, bw - 24, RENGLON, e.barra_fondo);
        p.texto(x0, pie + 2, "Super+B", INK_DIM);
    }
}
