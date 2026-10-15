//! **LA LAMINA EN SU VENTANA** (Q0a3 de `docs/plan/EL_FOCO.md`, 10-10): si la
//! app que publica una lamina de VERRANO tiene ventana, cada fotograma NUEVO
//! de su lamina lo dibuja la 3060 en un bloque del escritorio de la medida de
//! esa ventana, y la ventana se compone desde ahi (`Surface::fuente`). El
//! cubo de TITAN++ deja de verse solo con `gpu verrano banco inti`: gira en
//! su ventana, como cualquier app.
//!
//! [carril]  AMARILLO  manda dibujos a la 3060 por VERRANO; si algo falla, se
//!                     dice UNA vez y la ventana se queda con lo que pinte la
//!                     app (nada se cuelga)
//! [consumo] LATE      un dibujo por publicacion de la app; sin publicacion
//!                     nueva se mira UNA palabra (`Lamina::leer_si_nueva`)
//!
//! ** EL DIRECTOR HABLA VERRANO (1c de EL_FOCO): aqui no se nombra la 3060
//! -- ni un registro, ni una orden --: un `Frame` a una `Image`
//! (`Backend::draw`) en el `Aparato`, la puerta. La `Image` ES el bloque que
//! se compone (`Opciones::en_la_imagen`): la 3060 escribe ahi, y nada se
//! copia de vuelta.
//!
//! [!] Una ventana de ancho que no sea multiplo de 32 (filas de 128 bytes) o
//! de mas de 1280x720 no se dibuja aqui: se dice y la app sigue con lo suyo.
//!
//! ** EL JUEZ, A CUALQUIER MEDIDA (10-10): la huella de D3D12 solo existe a
//! 1280x720, y una ventana casi nunca mide eso. El juez es el backend CPU de
//! VERRANO (`Cpu::juzgar`), que da esas huellas: el PRIMER fotograma de cada
//! app y despues uno de cada [`JUZGAR_CADA`] se dibujan tambien en la CPU y se
//! comparan pixel a pixel con lo de la 3060. Se dice por la consola: el primer
//! juicio de cada app, y el primero que salga distinto.

use bmo_userland as bmo;
use bmo_verrano::lamina::{Lamina, Leido};
use bmo_verrano::{Backend, Frame, Image, Vertex, Viewport};

use super::sm86 as destino;
use crate::desktop::Desktop;
use crate::scene::output::Output;

/// V2 (11-10): 256 triangulos por fotograma -- dos cubos enteros caben de
/// sobra (24), y el descarte y la profundidad los hace VERRANO, no la app --.
const MAX_VERTICES: usize = 3 * 256;
/// El bloque, para la ventana mas grande que se dibuja aqui: 1280x720.
const PIXELES_MAX: usize = 1280 * 720;
/// Cada cuantos fotogramas se juzga uno (el primero de cada app, siempre). El
/// juez cuesta un fotograma de CPU: a 1 de 1024, ~1 cada 17 s a 60 Hz.
const JUZGAR_CADA: u32 = 1024;

struct Estado {
    /// La puerta, abierta UNA vez (la primera lamina con ventana).
    aparato: Option<destino::Aparato<'static>>,
    /// No se pudo (abrir o dibujar): se dijo, y no se reintenta.
    imposible: bool,
    /// El bloque donde dibuja la 3060 y desde donde se compone.
    bloque: Option<bmo::Memoria>,
    /// De que app es lo leido, y su ultima secuencia.
    tid: u32,
    secuencia: u32,
    v: [Vertex; MAX_VERTICES],
    /// La hoja del juez (pedida al primer juicio), los fotogramas de esta
    /// app y si ya se dijo uno distinto.
    hoja: Option<bmo::Memoria>,
    /// V2: el z-buffer del juez, si la lamina pide profundidad.
    hoja_z: Option<bmo::Memoria>,
    dibujados: u32,
    dicho_distinto: bool,
    /// ** C1: el kernel no tomo la lamina PRESTADA una vez: se dijo, y se
    /// vuelve a copiar (lo de antes del 15-10) sin reintentar.
    sin_prestar: bool,
    /// Cuantos fotogramas fueron prestados y cuantos copiados: lo que el
    /// metal mira para saber que C1 corre (`gpu laminas`).
    prestados: u32,
    copiados: u32,
}

static mut ESTADO: Estado = Estado {
    aparato: None,
    imposible: false,
    bloque: None,
    tid: 0,
    secuencia: 0,
    v: [Vertex { position: [0.0; 4], color: [0.0; 4] }; MAX_VERTICES],
    hoja: None,
    hoja_z: None,
    dibujados: 0,
    dicho_distinto: false,
    sin_prestar: false,
    prestados: 0,
    copiados: 0,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo, y esto solo lo toca `vuelta`.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// ** 11-10: se dice TAMBIEN en la caja de Ejecutar (`g`), donde mira el
/// propietario -- no solo en el panel del kernel (F11) --. MUNDO, en el
/// metal, salio con su ventana vacia y sin una linea que dijera por que.
fn decir(e: &mut Estado, g: &mut Output, que: &str) {
    e.imposible = true;
    bmo::consola("[Q0a3] la lamina NO va a su ventana: ");
    bmo::consola(que);
    bmo::consola("\n");
    g.text(b"  [Q0a3] la lamina NO va a su ventana: ");
    g.text(que.as_bytes());
    g.text(b"\n");
}

/// **Cada vuelta del escritorio**: si hay una lamina con ventana y un
/// fotograma nuevo, lo dibuja la 3060 en el bloque y la ventana pasa a
/// componerse de ahi. `true` si dibujo (hay que pintar).
pub(crate) fn vuelta(dsk: &mut Desktop, p: &bmo::Pantalla) -> bool {
    let e = estado();
    if e.imposible {
        return false;
    }
    let Some(t) = dsk.table.lamina() else {
        e.tid = 0;
        return false;
    };
    let Some(i) = dsk.table.ventana_de(t.tid) else { return false };
    if dsk.table.minimizada(i) || crate::commands::gspcomputo::la_3060_lista().is_none() {
        return false;
    }
    let Some((w, h)) = dsk.table.medida_de(i) else { return false };
    if t.tid != e.tid {
        e.tid = t.tid;
        e.secuencia = 0;
        e.dibujados = 0;
        e.dicho_distinto = false;
    }
    // SAFETY: `tomar_prestado_de` mapeo `bytes` desde `base` en este proceso y
    // la lamina vive hasta que su app muere (`reap_dead`, en este hilo). Se
    // ve como palabras ATOMICAS: la app las escribe a la vez.
    let palabras = unsafe { core::slice::from_raw_parts(t.base as *const core::sync::atomic::AtomicU32, (t.bytes / 4) as usize) };
    let Ok(l) = Lamina::abrir(palabras) else { return false };
    // Sin publicacion nueva, UNA palabra y fuera (antes de reservar nada).
    if l.secuencia() == e.secuencia {
        return false;
    }
    if w % 32 != 0 || (w as usize) * (h as usize) > PIXELES_MAX {
        decir(e, &mut dsk.out.grid, "su ventana no mide un multiplo de 32 de ancho o pasa de 1280x720"); dsk.tick.repaint_field = true;
        return false;
    }
    if e.bloque.is_none() {
        e.bloque = bmo::Memoria::residente(4 * PIXELES_MAX as u64);
        if e.bloque.is_none() {
            decir(e, &mut dsk.out.grid, "sin memoria para el bloque"); dsk.tick.repaint_field = true;
            return false;
        }
    }
    if e.aparato.is_none() {
        let Some(m) = bmo::Memoria::residente(destino::CAJA) else {
            decir(e, &mut dsk.out.grid, "sin memoria para el paquete"); dsk.tick.repaint_field = true;
            return false;
        };
        // SAFETY: residente y OLVIDADA (no se devuelve nunca: el aparato vive
        // lo que el escritorio), `CAJA` bytes de este proceso, y solo la usa
        // este aparato.
        let caja: &'static mut [u8] = unsafe { core::slice::from_raw_parts_mut(m.base() as *mut u8, destino::CAJA as usize) };
        core::mem::forget(m);
        let op = destino::Opciones { en_la_imagen: true, ligero: true, ..destino::Opciones::default() };
        match destino::abrir(dsk, p, caja, op) {
            Ok((mut a, _)) => {
                a.leer = false;
                e.aparato = Some(a);
            }
            Err(_) => {
                decir(e, &mut dsk.out.grid, "la puerta no se abrio (lo dice la salida)"); dsk.tick.repaint_field = true;
                return false;
            }
        }
    }
    let (Some(a), Some(b)) = (e.aparato.as_mut(), e.bloque.as_ref()) else { return false };
    // ** C1 (15-10, EL_FOCO): si la app RESPETA LA VUELTA (`Lamina::reservar`),
    // la ranura publicada se reserva -- el escritor no la toca hasta soltarla
    // -- y la 3060 lee los vertices de ahi, PRESTADOS: sin copiarlos. Si no,
    // o si el escritor estaba dentro, se copian como antes.
    let reserva = if e.sin_prestar { None } else { l.reservar() };
    let reserva = match reserva {
        Some(r) if r.vertices == 0 || r.vertices > MAX_VERTICES || r.secuencia == e.secuencia => {
            l.soltar();
            if r.secuencia == e.secuencia {
                return false;
            }
            None
        }
        r => r,
    };
    let (s, vertices): (u32, &[Vertex]) = match reserva {
        Some(r) => {
            // SAFETY: `reservar` comprobo que la ranura cabe en la lamina
            // (`capacidad` cabe en sus palabras, `abrir`; alineada a 4), y hasta
            // `soltar` (abajo, en todos los caminos) el escritor no la toca:
            // aqui se lee como `Vertex`, y la 3060, por su direccion.
            let v = unsafe { core::slice::from_raw_parts((t.base + r.desde_byte as u64) as *const Vertex, r.vertices) };
            (r.secuencia, v)
        }
        None => {
            let (s, leido) = l.leer_si_nueva(e.secuencia, &mut e.v);
            let Leido::Fotograma { vertices: n, .. } = leido else { return false };
            (s, &e.v[..n])
        }
    };
    let prestada = reserva.is_some();
    e.secuencia = s;
    let va = b.base() as u64;
    // SAFETY: el bloque residente mide `PIXELES_MAX` pixeles y `w * h` cabe
    // (comprobado arriba); solo lo escriben la 3060 (dentro de `draw`) y nadie
    // mas, y la ventana lo lee al componer, en este mismo hilo.
    let pixeles = unsafe { core::slice::from_raw_parts_mut(va as *mut u32, (w * h) as usize) };
    // V2 (11-10): como pide la app que se dibuje (la cabecera de su lamina).
    let (depth, cull) = l.estado();
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices, viewport: Viewport { width: w, height: h }, depth, cull };
    a.prestar = prestada;
    let dibujo = a.draw(&frame, &mut Image { pixels: pixeles, width: w, height: h , depth: None });
    a.prestar = false;
    // El juez lee los mismos vertices: con la ranura aun reservada.
    let dicho = match dibujo {
        Ok(_) if e.dibujados % JUZGAR_CADA == 0 => juzgar(&mut e.hoja, &mut e.hoja_z, &mut e.dicho_distinto, e.dibujados == 0, &frame, pixeles, &mut dsk.out.grid),
        _ => false,
    };
    if prestada {
        l.soltar();
    }
    if dicho {
        dsk.tick.repaint_field = true;
    }
    match dibujo {
        Ok(_) => {
            if prestada {
                if e.prestados == 0 {
                    bmo::consola("[C1] la lamina va PRESTADA a la 3060: sin copia\n");
                }
                e.prestados = e.prestados.wrapping_add(1);
            } else {
                e.copiados = e.copiados.wrapping_add(1);
            }
            e.dibujados = e.dibujados.wrapping_add(1);
            dsk.table.poner_fuente(i, va, s);
            true
        }
        Err(_) if prestada => {
            // El kernel no la tomo (un kernel de antes, o `Porque` 29/30):
            // se dice UNA vez y se copia desde el proximo fotograma.
            e.sin_prestar = true;
            e.secuencia = 0;
            bmo::consola("[C1] la 3060 no tomo la lamina prestada: se copia (lo de antes)\n");
            dsk.out.grid.text(b"  [C1] la 3060 no tomo la lamina prestada: se copia (lo de antes)\n");
            dsk.tick.repaint_field = true;
            false
        }
        Err(_) => {
            decir(e, &mut dsk.out.grid, "la 3060 no dibujo el fotograma (VERRANO dijo que no)"); dsk.tick.repaint_field = true;
            false
        }
    }
}

/// **El juez**: el fotograma, otra vez por la CPU de VERRANO, contra lo que
/// dejo la 3060. Se dice el primero de cada app y el primero distinto.
/// `true` si dijo algo en la caja (`g`).
fn juzgar(hoja: &mut Option<bmo::Memoria>, hoja_z: &mut Option<bmo::Memoria>, dicho_distinto: &mut bool, primero: bool, frame: &Frame, visto: &[u32], g: &mut Output) -> bool {
    if hoja.is_none() {
        *hoja = bmo::Memoria::residente(4 * PIXELES_MAX as u64);
    }
    let Some(m) = hoja.as_ref() else {
        if primero {
            bmo::consola("[Q0a3] el juez: sin memoria para su hoja, no se juzga\n");
            g.text(b"  [Q0a3] el juez: sin memoria para su hoja, no se juzga\n");
            return true;
        }
        return false;
    };
    // SAFETY: residente, `PIXELES_MAX` pixeles de este proceso, y solo la usa
    // el juez, en este hilo.
    let hoja = unsafe { core::slice::from_raw_parts_mut(m.base() as *mut u32, PIXELES_MAX) };
    let juez = bmo_verrano::cpu::Cpu { max_vertices: MAX_VERTICES, ..bmo_verrano::cpu::Cpu::LA_3060 };
    // V2: con profundidad, el juez lleva su z-buffer (otro bloque, una vez).
    if frame.depth && hoja_z.is_none() {
        *hoja_z = bmo::Memoria::residente(4 * PIXELES_MAX as u64);
    }
    let z = match (frame.depth, hoja_z.as_ref()) {
        // SAFETY: residente, `PIXELES_MAX` f32 de este proceso, y solo la usa
        // el juez, en este hilo.
        (true, Some(m)) => Some(unsafe { core::slice::from_raw_parts_mut(m.base() as *mut f32, PIXELES_MAX) }),
        (true, None) => return false,
        _ => None,
    };
    let Ok(j) = juez.juzgar(frame, visto, hoja, z) else { return false };
    let distinto = j.distintos != 0;
    if !(primero || distinto && !*dicho_distinto) {
        return false;
    }
    *dicho_distinto |= distinto;
    let mut d = [0u8; 10];
    // lo mismo al panel del kernel y a la caja
    let mut num = |g: &mut Output, v: u64| {
        let k = crate::text::decimal(v, &mut d);
        bmo::consola(core::str::from_utf8(&d[..k]).unwrap_or("?"));
        g.text(&d[..k]);
    };
    let di = |g: &mut Output, t: &str| {
        bmo::consola(t);
        g.text(t.as_bytes());
    };
    di(g, "  [Q0a3] el juez (la CPU de VERRANO) a ");
    num(g, frame.viewport.width as u64);
    di(g, "x");
    num(g, frame.viewport.height as u64);
    if !distinto {
        di(g, ": la 3060 da lo MISMO, pixel a pixel\n");
        return true;
    }
    di(g, ": la 3060 da DISTINTO en ");
    num(g, j.distintos as u64);
    di(g, " de ");
    num(g, j.pixeles as u64);
    di(g, " pixeles");
    if let Some((x, y, _, _)) = j.primero {
        di(g, ", el primero en ");
        num(g, x as u64);
        di(g, ",");
        num(g, y as u64);
    }
    di(g, "\n");
    true
}
