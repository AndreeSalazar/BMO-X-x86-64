//! **EL MIRADOR** -- lo que CABINA MUESTRA y VUELCA, separado de lo que APUNTA.
//!
//! [familia] mirador  nivel 13 -- lo que CABINA muestra y vuelca: cockpit, vigilancias y caja negra
//! [conecta] cabina, core, dev, fsys, mm, plat, task, uconsole
//!
//! [carril]  AMARILLO  presentacion y volcado: lo que se ve a las 3 de la luego
//! [consumo] NADA      pinta o vuelca cuando `core` lo llama
//!
//! ## Por que se partio CABINA (L8b, 2026-09-13)
//!
//! Eddi pidio desanudar el kernel empezando por CABINA, y la medida dijo por que
//! era el primer nudo: hacia DOS oficios con un solo nombre.
//!
//! ```text
//!    el REGISTRO   info/warn/fault, el anillo, la caida   lo llaman los 11
//!                                                          subsistemas
//!    el MIRADOR    cockpit, vigilancias, caja negra       lee dev, mm, plat,
//!                                                          task, fsys...
//! ```
//!
//! Juntos, la familia que todo el kernel llama tenia que importar a todo el
//! kernel: **54 usos que subian desde el suelo**. Separados, el registro queda
//! abajo --solo sabe la hora, que le da `reloj`-- y el mirador arriba, donde leer
//! a todos es precisamente su oficio.
//!
//! [!] Lo que queda, dicho: el mirador pinta con el tablero de `core::splash` y
//! lee la generacion de pantalla de `core::shell`, y `core` lo llama a el. Esa
//! arista sube, esta en la linea base de L8b, y es el siguiente corte.

use cabina_core::{Event, Layer, Severity, TelemetrySnapshot};
use crate::ring0::cabina::*;
use crate::ring0::core::splash::{splash_dashboard_log_color, DASH_LOG_W};

/// THE BLACK BOX: the ring, on the disk. The only part that survives a power
/// cut, and the only one that can fail for reasons unrelated to logging.
pub(crate) mod blackbox;
pub use blackbox::*;
/// THE WATCHES: this IS polling, and the reason is that a device which stops
/// answering does not send an event saying so.
pub(crate) mod watch;
pub(crate) use watch::*;
/// THE COCKPIT: severity colours, filters, layout. Presentation only -- none of
/// it changes what is recorded.
pub(crate) mod cockpit;
pub use cockpit::*;

/// **Volcar lo que la RAM conservo del arranque anterior a `CAIDA.TXT`**, cuando
/// el disco ya esta montado.
///
/// Devuelve los bytes escritos. Cero si no habia nada, o si el disco dijo que no
/// -- y en ese caso CABINA ya lo conto. Vivia en `cabina::caida` como `volcar`:
/// el registro entrega los bytes (`texto_recuperado`) y quien escribe en el
/// disco es quien puede importar el disco.
pub fn volcar_caida() -> usize {
    let texto = crate::ring0::cabina::caida::texto_recuperado();
    let n = texto.len();
    if n == 0 {
        return 0;
    }
    // *** 07-10: era `fs::create`, que NO PISA un fichero que ya existe. O
    // sea: se guardo la PRIMERA caida de la historia y todas las demas
    // chocaban con "ya existe" -- un aviso en CABINA y la caja negra
    // perdida al reusar su RAM. El propietario mando un CAIDA.TXT de 41 KB
    // de una sesion vieja cuando el arranque decia "recuperado 196576
    // bytes". Ahora: `CAIDA.TXT` es SIEMPRE la ultima (se reemplaza), y
    // `CAIDAnnn.TXT` la guarda con su generacion, para que la de antes no
    // se pierda tampoco.
    let ultima = crate::ring0::fsys::fs::guardar(b"CAIDA   TXT", texto);
    let g = crate::ring0::cabina::caida::generacion() % 1000;
    let numerada = [b'C', b'A', b'I', b'D', b'A', b'0' + (g / 100) as u8, b'0' + (g / 10 % 10) as u8, b'0' + (g % 10) as u8, b'T', b'X', b'T'];
    let copia = crate::ring0::fsys::fs::guardar(&numerada, texto);
    match (ultima, copia) {
        (Ok(()), Ok(())) => {
            info("caida", "guardado en CAIDA.TXT (y en CAIDAnnn.TXT, nnn = generacion): bytes", n as u64);
            n
        }
        (Ok(()), Err(_)) => {
            warn("caida", "CAIDA.TXT guardado, la copia numerada NO; generacion", g);
            n
        }
        (Err(_), Ok(())) => {
            warn("caida", "CAIDA.TXT no se pudo reemplazar; esta en CAIDAnnn.TXT, generacion", g);
            n
        }
        (Err(_), Err(_)) => {
            warn("caida", "el disco no acepto CAIDA.TXT; sigue en RAM", n as u64);
            0
        }
    }
}
