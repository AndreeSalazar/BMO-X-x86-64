//! **EL CARTEL ROJO de PROTON-X** (05-10): lo que la casa no supo hacer,
//! dicho EN la ventana del `.exe`, no solo en la consola.
//!
//! El propietario: *"que PROTON-X siempre te diga que fue mal: el mensaje
//! simple, con rojo completo pero bordes blancos"*, y *"que hable
//! internamente honesto"*. Un `.exe` hecho con Visual Studio hace MUCHAS
//! llamadas que nadie ve (los de StarCraft lo contaban: el runtime llamaba
//! a cosas que no hacian falta); cuando una le falta a la casa, la casa lo
//! dice con un `aviso` ("PROTON-X: ..."), y ese aviso se pierde si nadie mira
//! la consola. Aqui no se pierde:
//!
//! ```text
//!    oir       cada linea que la casa escribe; las que empiezan por
//!              "PROTON-X: " son AVISOS: se guarda la ultima y se cuentan
//!              (la casa ya dice cada aviso distinto UNA vez)
//!    pintar    en cada `presentar`, abajo de la ventana: un cartel ROJO
//!              entero con borde BLANCO de 2 pixeles y, en blanco,
//!              "PROTON-X (n): <el ultimo aviso>"; lo que no cabe, con "..."
//! ```
//!
//! Se queda puesto: lo que fue mal, fue mal (la lista entera, en la consola).
//! Lo que NO cubre, dicho: cuando la 3060 dibuja DIRECTO en la pantalla (Z1)
//! los pixeles de la ventana no se ven, y el cartel tampoco; y un `.exe` de
//! GDI que lea su propia ventana leeria el cartel.
//!
//! Puro (solo `core` y la letra de la casa): se prueba pintandolo en el
//! anfitrion.

use bmo_letra::{base_en_caja, Estilo, Fuente, Letra};
use core::cell::UnsafeCell;

/// Lo que empieza un aviso de la casa (`bmo_proton_x_casa::aviso`).
const PREFIJO: &[u8] = b"PROTON-X: ";
/// Lo que se guarda de una linea: un aviso largo se corta aqui.
const LINEA: usize = 256;

/// El rojo entero y el blanco, en BGRA32 (`0xAARRGGBB`).
pub const ROJO: u32 = 0xFFFF_0000;
pub const BLANCO: u32 = 0xFFFF_FFFF;
/// El cartel: su alto, su margen con la ventana y su borde.
const ALTO: u32 = 30;
const MARGEN: u32 = 8;
const BORDE: u32 = 2;

struct Estado {
    linea: [u8; LINEA],
    n: usize,
    ultimo: [u8; LINEA],
    largo: usize,
    cuantos: u32,
    letra: Letra,
}

struct Celda(UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos y ni `oir` ni
// `pintar` ceden el turno.
// [hilos] cerrojo -- estado del proceso que tocan los hilos del juego: necesita un cerrojo (H2.1)
unsafe impl Sync for Celda {}
static ESTADO: Celda = Celda(UnsafeCell::new(Estado { linea: [0; LINEA], n: 0, ultimo: [0; LINEA], largo: 0, cuantos: 0, letra: Letra::nueva() }));

fn estado() -> &'static mut Estado {
    // SAFETY: ver `Celda`; nadie guarda la referencia.
    unsafe { &mut *ESTADO.0.get() }
}

/// **Oir lo que la casa escribe** (todo lo de la consola pasa por aqui).
pub fn oir(bytes: &[u8]) {
    let e = estado();
    for &c in bytes {
        match c {
            b'\n' => {
                if e.linea[..e.n].starts_with(PREFIJO) {
                    let t = &e.linea[PREFIJO.len()..e.n];
                    e.ultimo[..t.len()].copy_from_slice(t);
                    e.largo = t.len();
                    e.cuantos += 1;
                }
                e.n = 0;
            }
            b'\r' => {}
            _ if e.n < LINEA => {
                e.linea[e.n] = c;
                e.n += 1;
            }
            _ => {}
        }
    }
}

/// Cuantos avisos se han oido (0: el cartel no se pinta).
pub fn cuantos() -> u32 {
    estado().cuantos
}

/// **Pintar el cartel** abajo de una ventana de `ancho` x `alto` (filas de
/// `paso` pixeles), si hubo algun aviso y la ventana da para el.
pub fn pintar(px: &mut [u32], ancho: u32, alto: u32, paso: u32) {
    let e = estado();
    if e.cuantos == 0 || ancho < 4 * MARGEN + 64 || alto < ALTO + 2 * MARGEN || px.len() < (paso * alto) as usize {
        return;
    }
    let (x0, y0, x1, y1) = (MARGEN, alto - MARGEN - ALTO, ancho - MARGEN, alto - MARGEN);
    for y in y0..y1 {
        for x in x0..x1 {
            let borde = x < x0 + BORDE || x >= x1 - BORDE || y < y0 + BORDE || y >= y1 - BORDE;
            px[(y * paso + x) as usize] = if borde { BLANCO } else { ROJO };
        }
    }
    // "PROTON-X (n): " y el ultimo aviso.
    let mut texto = [0u8; LINEA + 32];
    let mut k = 0;
    let mut poner = |b: &[u8]| {
        let n = b.len().min(texto.len() - k);
        texto[k..k + n].copy_from_slice(&b[..n]);
        k += n;
    };
    poner(b"PROTON-X (");
    let mut cifras = [0u8; 10];
    let (mut v, mut c) = (e.cuantos, cifras.len());
    loop {
        c -= 1;
        cifras[c] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    poner(&cifras[c..]);
    poner(b"): ");
    poner(&e.ultimo[..e.largo]);
    let estilo = Estilo::negrita(14);
    let base = y0 as i32 + base_en_caja(14, ALTO as i32);
    let (dentro0, dentro1) = ((x0 + BORDE) as i32, (x1 - BORDE) as i32);
    let caja = (y0 + BORDE) as i32..(y1 - BORDE) as i32;
    e.letra.escribir_cabe(&texto[..k], estilo, x0 as i32 + 10, base, (x1 - x0) as i32 - 20, |x, y, a| {
        if x >= dentro0 && x < dentro1 && caja.contains(&y) {
            // Blanco sobre rojo: el rojo se queda, el verde y el azul suben.
            px[(y as u32 * paso + x as u32) as usize] = ROJO | (a as u32) << 8 | a as u32;
        }
    });
}
