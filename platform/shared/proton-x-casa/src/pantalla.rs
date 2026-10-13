//! **La pantalla que ve el juego** (LI2j, 12-10): ancho, alto y la barra de
//! tareas del UNICO monitor de la casa. La pone QUIEN CARGA, antes de saltar
//! al `.exe`; user32 (GetSystemMetrics, el area de trabajo, GetMonitorInfo,
//! EnumDisplaySettings) y DXGI (la salida, sus modos, FindClosestMatchingMode)
//! leen de aqui, asi que todo Windows dice lo mismo.
//!
//! ```text
//!    BMO-X le da al juego UNA pantalla (la que quien carga diga)
//!       |  el juego pregunta: medidas, modos, el monitor
//!       |  y elige: su ventana, su cadena (lo que QUIERE, de lo ofrecido)
//!       v
//!    el propietario manda despues: --pantalla WxH, o el menu del juego (entre
//!    los modos que la casa ofrece)
//! ```
//!
//! Por que importa (el metal del 12-10): con 1920x1080 Cyberpunk abrio una
//! ventana de 1738x1064 y la puerta de la 3060 solo dibuja la medida de
//! VERRANO; DOOM, a pantalla completa, pidio 1280x720 y fue a la 3060. Con
//! `--3060`, PROTON-X pone aqui la medida de la puerta (sin barra: el area
//! de trabajo es la pantalla entera). Sin decir nada, 1920x1080 con 40 de
//! barra, la de siempre.
//!
//! La casa no la cambia sola: ni `empezar` ni `reiniciar` la tocan (la pone
//! quien carga, una vez, y vale para el `.exe` entero).

use core::sync::atomic::{AtomicU64, Ordering};

/// Una pantalla: medida en pixeles y la barra de tareas abajo (lo que queda
/// fuera del area de trabajo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pantalla {
    pub ancho: u32,
    pub alto: u32,
    pub barra: u32,
}

/// La de siempre: Full HD con la barra de Windows 10 a 96 DPI.
pub const FULL_HD: Pantalla = Pantalla { ancho: 1920, alto: 1080, barra: 40 };

/// La menor y la mayor que la casa acepta (VGA y 8K).
pub const MENOR: (u32, u32) = (640, 480);
pub const MAYOR: (u32, u32) = (7680, 4320);

/// Las medidas de monitor de toda la vida (4:3, 16:10, 16:9), de menor a
/// mayor: los modos que se ofrecen son las que CABEN en la pantalla, y la
/// pantalla misma.
const COMUNES: [(u32, u32); 12] = [
    (640, 480),
    (800, 600),
    (1024, 768),
    (1280, 720),
    (1280, 800),
    (1366, 768),
    (1600, 900),
    (1680, 1050),
    (1920, 1080),
    (2560, 1440),
    (3440, 1440),
    (3840, 2160),
];

/// Cuantos modos puede haber como mucho: las comunes y la pantalla.
pub const MAX_MODOS: usize = COMUNES.len() + 1;

// ancho | alto << 16 | barra << 32 | puesta << 63 (la puso el propietario).
static ACTUAL: AtomicU64 = AtomicU64::new(empaquetar(FULL_HD, false));

const DEL_PROPIETARIO: u64 = 1 << 63;

const fn empaquetar(p: Pantalla, propietario: bool) -> u64 {
    p.ancho as u64 | (p.alto as u64) << 16 | (p.barra as u64) << 32 | if propietario { DEL_PROPIETARIO } else { 0 }
}

/// La pantalla, si es una que la casa sabe presentar.
pub const fn valida(p: Pantalla) -> Result<Pantalla, &'static str> {
    if p.ancho < MENOR.0 || p.alto < MENOR.1 || p.ancho > MAYOR.0 || p.alto > MAYOR.1 {
        return Err("una pantalla entre 640x480 y 7680x4320");
    }
    if p.barra > p.alto / 4 {
        return Err("una barra de tareas de mas de un cuarto de la pantalla");
    }
    Ok(p)
}

/// **Poner la pantalla** (quien carga, antes del `.exe`). `del_propietario`: la
/// eligio el propietario (`--pantalla`), y quien carga ya no la cambia por la suya.
pub fn poner(p: Pantalla, del_propietario: bool) -> Result<(), &'static str> {
    let p = valida(p)?;
    ACTUAL.store(empaquetar(p, del_propietario), Ordering::Relaxed);
    Ok(())
}

/// La pantalla de ahora.
pub fn actual() -> Pantalla {
    let v = ACTUAL.load(Ordering::Relaxed);
    Pantalla { ancho: v as u32 & 0xFFFF, alto: (v >> 16) as u32 & 0xFFFF, barra: (v >> 32) as u32 & 0xFFFF }
}

/// La eligio el propietario.
pub fn del_propietario() -> bool {
    ACTUAL.load(Ordering::Relaxed) & DEL_PROPIETARIO != 0
}

/// `WxH` (o `WXH`), como lo escribe el propietario: `1920x1080`.
pub fn leer(texto: &[u8]) -> Option<(u32, u32)> {
    let k = texto.iter().position(|&c| c | 0x20 == b'x')?;
    let num = |t: &[u8]| -> Option<u32> {
        if t.is_empty() || t.len() > 5 || !t.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Some(t.iter().fold(0u32, |n, &c| n * 10 + (c - b'0') as u32))
    };
    Some((num(&texto[..k])?, num(&texto[k + 1..])?))
}

/// Ancho y alto, como los usa user32.
pub(crate) fn medida() -> (i32, i32) {
    let p = actual();
    (p.ancho as i32, p.alto as i32)
}

/// La barra de tareas, en pixeles.
pub(crate) fn barra() -> i32 {
    actual().barra as i32
}

/// **Los modos que se ofrecen**, de menor a mayor: las comunes que caben, y
/// la pantalla la ultima (aunque no sea comun). Devuelve cuantos.
pub fn modos(sale: &mut [(u32, u32); MAX_MODOS]) -> usize {
    let p = actual();
    let mut n = 0;
    for &(w, h) in COMUNES.iter().filter(|&&(w, h)| w <= p.ancho && h <= p.alto && (w, h) != (p.ancho, p.alto)) {
        sale[n] = (w, h);
        n += 1;
    }
    sale[n] = (p.ancho, p.alto);
    n + 1
}

/// Una ventana de arriba con marco (WS_CAPTION o WS_THICKFRAME) no pasa de
/// la pantalla: Windows la recorta a SM_CXMAXTRACK en CreateWindowEx, y
/// aqui la ventana ES su area de cliente.
pub(crate) fn recortar_ventana(w: u32, h: u32) -> (u32, u32) {
    let p = actual();
    (w.min(p.ancho), h.min(p.alto))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Todo lo que cambia la pantalla, en UNA prueba (es una para el
    /// proceso): y al final, la de siempre.
    #[test]
    fn la_pantalla_la_pone_quien_carga_y_todo_windows_la_dice() {
        assert_eq!(actual(), FULL_HD);
        assert!(!del_propietario());
        let mut m = [(0, 0); MAX_MODOS];
        let n = modos(&mut m);
        assert_eq!(&m[..n], &[(640, 480), (800, 600), (1024, 768), (1280, 720), (1280, 800), (1366, 768), (1600, 900), (1680, 1050), (1920, 1080)]);

        // La de la puerta de la 3060, sin barra.
        poner(Pantalla { ancho: 1280, alto: 720, barra: 0 }, false).unwrap();
        let n = modos(&mut m);
        assert_eq!(&m[..n], &[(640, 480), (800, 600), (1280, 720)], "nada que no quepa: 1024x768 tiene 768 filas");
        assert_eq!(crate::user32_medidas::metrica(0), 1280);
        assert_eq!(crate::user32_medidas::metrica(1), 720);
        assert_eq!(crate::user32_medidas::area_de_trabajo(), [0, 0, 1280, 720], "sin barra: la pantalla entera");
        assert_eq!(recortar_ventana(1738, 1064), (1280, 720), "la ventana de Cyberpunk del 12-10");
        assert_eq!(recortar_ventana(800, 600), (800, 600));

        // Una que no es comun: la ultima de la lista.
        poner(Pantalla { ancho: 1440, alto: 900, barra: 40 }, true).unwrap();
        assert!(del_propietario());
        let n = modos(&mut m);
        assert_eq!(m[n - 1], (1440, 900));
        assert_eq!(m[n - 2], (1366, 768));
        assert_eq!(crate::user32_medidas::area_de_trabajo(), [0, 0, 1440, 860]);

        assert!(poner(Pantalla { ancho: 320, alto: 200, barra: 0 }, true).is_err());
        assert!(poner(Pantalla { ancho: 1280, alto: 720, barra: 400 }, true).is_err());
        assert_eq!(actual().ancho, 1440, "la que no vale no cambia nada");

        poner(FULL_HD, false).unwrap();
        assert_eq!(actual(), FULL_HD);
    }

    #[test]
    fn lo_que_escribe_el_propietario() {
        assert_eq!(leer(b"1920x1080"), Some((1920, 1080)));
        assert_eq!(leer(b"1280X720"), Some((1280, 720)));
        assert_eq!(leer(b"1280"), None);
        assert_eq!(leer(b"x720"), None);
        assert_eq!(leer(b"12a0x720"), None);
        assert_eq!(leer(b"999999x720"), None);
    }
}
