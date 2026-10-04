//! **El conmutador de ventanas** -- la ventanita de Alt+Tab.
//!
//! [consumo] LATE      solo los ~300 ms de abrirse y de deslizar la marca
//!                     (`scene::vida`); quieto, NADA: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! La politica vive en `bmo_foco::focus` y **se prueba alli**; aqui solo se
//! pinta lo que esa politica ya decidio. Es el mismo reparto de siempre: quien
//! decide no dibuja, y quien dibuja no decide.
//!
//! === Por que hay que ensenarlo y no basta con conmutar ===
//!
//! Sin esta ventanita, Alt+Tab es adivinar. Con dos ventanas se aguanta; con
//! tres ya no se sabe cuantos Tabs faltan, y el resultado es pulsar de mas y
//! acabar donde no querias. Mostrar **la lista y cual esta marcada** convierte
//! un atajo de memoria en uno que se mira.
//!
//! Es lo que Eddi pidio con estas palabras: *"requiere la chica ventana que
//! hace ver todo para facilitar que prioridad le da, porque si no chocan"*.

use bmo_userland as bmo;

use super::tema_gen::{FINO_FONDO, LATON, MARCA_FONDO};
use super::vida::{self, Paso, REBOTE, SALIDA};
use super::*;
use crate::ventana::Ventana;

// ** LA CARA (04-10): la ventanita es una TARJETA de `fino.rs` -- la
// curva, la sombra y el hilo de laton --, con la letra de la casa. Era un
// marco de dos pixeles con `> ` delante de la elegida: se leia como una
// consola. Las medidas, en pixeles:
//
//    margen 24 | el rotulo (base a 30) | el filete (a 42) | las filas (de
//    34, desde 52) | el filete | el modo y la ayuda (dos lineas de 20) | 16
const SW_W: u32 = 400;
const MARGEN: u32 = 24;
const ROW_H: u32 = 34;
const FILAS_Y: u32 = 52;
const PIE_H: u32 = 20;
/// Las filas que caben; si hay mas ventanas, la ultima dice cuantas faltan.
const MAX_FILAS: usize = 10;

/// Lo que se sale la tarjeta de su caja al pasarse con el rebote, por lado.
const HOLGURA: u32 = 6;
/// La sombra mas el rebote: todo lo que puede llegar a pintarse fuera.
const FUERA: u32 = fino::HALO + HOLGURA;

/// Lo que habia debajo, para repintar sin mezclar dos veces: la tarjeta con
/// su sombra y su rebote, del alto que tiene con [`MAX_FILAS`].
const GUARDADO: usize = ((SW_W + 2 * FUERA) * (FILAS_Y + MAX_FILAS as u32 * ROW_H + 16 + 2 * PIE_H + 16 + 2 * FUERA)) as usize;

// ** LA VIDA (04-10): *"que TODOS tengan vida"*. Dos movimientos, y los dos
// por EVENTO:
//
//    al abrirse   la tarjeta crece del 92 % al 100 % con rebote, y lo de
//                 dentro aparece cuando ya casi ha llegado: se abre, no salta
//    cada Tab     la marca se DESLIZA a la fila nueva (con rebote) y la
//                 letra de cada fila se enciende segun la marca pasa por ella
//
// Para repintar mientras se mueve hace falta lo ultimo que se pidio: se
// guarda aqui (la lista cabe: `bmo_foco` no pasa de `MAX_VENTANAS`).
const ABRE_MS: u32 = 300;
const DESLIZA_MS: u32 = 240;

struct Vivo {
    lista: [u8; 16],
    n: usize,
    sel: usize,
    modo: &'static str,
    abierto: bool,
    abre: Paso,
    /// La fila de la marca DENTRO de lo que se ve, en milesimas de fila.
    desde: i32,
    hacia: i32,
    desliza: Paso,
    /// Se movia en el fotograma anterior: falta el ultimo, el quieto.
    movia: bool,
}

static mut VIVO: Vivo = Vivo {
    lista: [0; 16],
    n: 0,
    sel: 0,
    modo: "",
    abierto: false,
    abre: Paso::QUIETO,
    desde: 0,
    hacia: 0,
    desliza: Paso::QUIETO,
    movia: false,
};

fn vivo() -> &'static mut Vivo {
    // SAFETY: el mismo hilo de siempre; solo lo toca el conmutador.
    unsafe { &mut *core::ptr::addr_of_mut!(VIVO) }
}

struct Debajo {
    px: [u32; GUARDADO],
    caja: (u32, u32, u32, u32),
    puesto: bool,
}

static mut DEBAJO: Debajo = Debajo { px: [0; GUARDADO], caja: (0, 0, 0, 0), puesto: false };

fn debajo() -> &'static mut Debajo {
    // SAFETY: el director es un solo hilo, y esto solo lo toca el conmutador.
    unsafe { &mut *core::ptr::addr_of_mut!(DEBAJO) }
}

/// **Olvida lo guardado** sin devolverlo: lo de debajo ya no es lo que se
/// guardo (se pinto una ventana encima, o se repinto todo al soltar Alt).
pub(crate) fn olvidar() {
    debajo().puesto = false;
}

/// **Se cerro** (se solto Alt): lo de debajo ya se devolvio, y la proxima
/// vez se abre otra vez creciendo.
pub(crate) fn cerrar() {
    olvidar();
    let v = vivo();
    v.abierto = false;
    v.abre = Paso::QUIETO;
    v.desliza = Paso::QUIETO;
    v.movia = false;
}

/// **Un fotograma de vida**: si se esta abriendo o la marca viaja, se
/// repinta con lo ultimo que se pidio. Tambien el fotograma de DESPUES de
/// acabar, que tiene que ser el quieto. Lo llama el compositor solo con el
/// conmutador pintado.
pub(crate) fn vivir(p: &bmo::Pantalla) {
    let v = vivo();
    let mueve = v.abre.vivo() || v.desliza.vivo();
    let pide = mueve || v.movia;
    v.movia = mueve;
    if pide && v.abierto {
        pintar(p);
    }
}

/// El nombre de cada ventana.
///
/// ** LA TABLA YA NO ESTA AQUI, y ese es el arreglo. Estuvo, con un `_ =>
/// "?"` al final, y mintio dos veces: CABINA y Sonido salieron como `?` por
/// no ampliarla al nacer, y la ventana de CPU se anunciaba como "Sonido"
/// porque compartia el id 3 con ella.
///
/// El sintoma de una tabla que se queda corta es suave y por eso dura: Alt+Tab
/// funciona, conmuta bien, y solo miente en el nombre. Ahora los nombres son
/// de `Ventana`, donde el `match` es EXHAUSTIVO --sin `_`-- y una ventana
/// nueva no compila hasta que tiene el suyo. Aqui solo queda la traduccion
/// desde el id que guarda la politica, y el `?` que ya no puede pasar.
pub(crate) fn name(id: u8) -> &'static str {
    match Ventana::de_id(id) {
        Some(v) => v.nombre(),
        None => "?",
    }
}

/// El rectangulo del conmutador. **Una sola cuenta**, porque la usan dos.
///
/// `paint` y `area` la tenian copiada, con un comentario que advertia justo de
/// esto. Mientras las dos copias fueran identicas daba igual; en cuanto una
/// crece --la fila de la ayuda de las flechas-- la otra borra un rectangulo mas
/// corto que el pintado y deja una franja de la ventanita pegada en el
/// escritorio hasta el siguiente repintado.
fn run_box(p: &bmo::Pantalla, count: usize) -> (u32, u32, u32, u32) {
    let filas = count.min(MAX_FILAS) as u32;
    let height = FILAS_Y + ROW_H * filas + 16 + 2 * PIE_H + 16;
    let width = SW_W.min(p.ancho.saturating_sub(40 + 2 * FUERA));
    (
        (p.ancho.saturating_sub(width)) / 2,
        (p.alto.saturating_sub(height)) / 2,
        width,
        height,
    )
}

/// Pinta el conmutador centrado, con la marcada resaltada. La primera vez
/// se abre creciendo; las siguientes, la marca viaja a la fila nueva.
pub(crate) fn paint(p: &bmo::Pantalla, lista: &[u8], pointed_at: usize, modo: &'static str) {
    if lista.is_empty() {
        return;
    }
    let v = vivo();
    let n = lista.len().min(v.lista.len());
    v.lista[..n].copy_from_slice(&lista[..n]);
    v.n = n;
    v.sel = pointed_at.min(n - 1);
    v.modo = modo;
    let fila = visible(v.n, v.sel) as i32 * 1000;
    if !v.abierto {
        v.abierto = true;
        v.abre = Paso::empezar(ABRE_MS);
        v.desde = fila;
        v.hacia = fila;
        v.desliza = Paso::QUIETO;
    } else if fila != v.hacia {
        // Si cambia a mitad de un viaje, sale de donde esta, no de donde salio.
        v.desde = if v.desliza.vivo() { vida::entre(v.desde, v.hacia, v.desliza.k(REBOTE)) } else { v.hacia };
        v.hacia = fila;
        v.desliza = Paso::empezar(DESLIZA_MS);
    }
    v.movia = true;
    pintar(p);
}

/// La fila de la marcada DENTRO de lo que se ve, y desde cual se empieza a
/// ver. Si no caben todas, la marcada SIEMPRE se ve: la lista corre para que
/// quede dentro.
fn visible(n: usize, sel: usize) -> usize {
    sel - desde_fila(n, sel)
}

fn desde_fila(n: usize, sel: usize) -> usize {
    let caben = n.min(MAX_FILAS);
    if sel >= caben {
        sel + 1 - caben
    } else {
        0
    }
}

/// **Pinta lo ultimo que se pidio**, a la altura del movimiento que toque.
fn pintar(p: &bmo::Pantalla) {
    let v = vivo();
    let lista = &v.lista[..v.n];
    let pointed_at = v.sel;
    let modo = v.modo;
    let (x, y, width, height) = run_box(p, lista.len());
    guardar(p, (x, y, width, height));

    // Al abrirse: la tarjeta crece desde el centro (el rebote la pasa un
    // pelo del 100 % y vuelve), y lo de dentro no sale hasta que la caja casi
    // ha llegado -- texto en una caja que todavia crece se lee como un fallo.
    let k = v.abre.k(REBOTE);
    let escala = 920 + 80 * k / 1000;
    let (ew, eh) = ((width as i32 * escala / 1000) as u32, (height as i32 * escala / 1000) as u32);
    let (ex, ey) = ((x as i32 + (width as i32 - ew as i32) / 2) as u32, (y as i32 + (height as i32 - eh as i32) / 2) as u32);
    fino::tarjeta(p, ex, ey, ew, eh);
    let luz = if v.abre.vivo() { ((v.abre.k(SALIDA) - 550) * 1000 / 450).clamp(0, 1000) } else { 1000 };
    if luz == 0 {
        return;
    }
    // Lo de dentro se enciende desde el fondo de la tarjeta.
    let c = |tinta: u32| if luz >= 1000 { tinta } else { bmo::entre_color(FINO_FONDO, tinta, luz) };

    let dentro = width - 2 * MARGEN;
    fino::rotulo(p, x + MARGEN, y + 30, b"Ventanas", c(LATON));
    if luz >= 1000 {
        fino::filete(p, x + MARGEN, y + 42, dentro);
    }

    let caben = lista.len().min(MAX_FILAS);
    let desde = desde_fila(lista.len(), pointed_at);
    // La marca, donde toque a esta altura del viaje. Va de borde a borde: una
    // a media anchura se lee como "hay mas columnas" y no las hay.
    let fila = vida::entre(v.desde, v.hacia, v.desliza.k(REBOTE));
    let my = (y + FILAS_Y) as i32 + fila * ROW_H as i32 / 1000;
    let (mx, mw) = ((x + MARGEN - 8) as i32, (dentro + 16) as i32);
    p.caja_redonda(mx, my + 2, mw, ROW_H as i32 - 4, 9, c(MARCA_FONDO));
    p.caja_redonda(mx + 12, my + ROW_H as i32 / 2 - 3, 6, 6, 3, c(acento()));
    let mut fy = y + FILAS_Y;
    for (i, &w) in lista.iter().enumerate().skip(desde).take(caben) {
        // La letra se enciende segun la marca pasa por su fila: cerca es
        // marfil y firme, lejos es perla.
        let aqui = ((i - desde) as i32) * 1000;
        let cerca = (1000 - (fila - aqui).abs()).clamp(0, 1000);
        let e = if cerca > 500 { fino::CUERPO_FIRME } else { fino::CUERPO };
        let tinta = bmo::entre_color(fino::TENUE, fino::TINTA, cerca);
        fino::texto(p, x + MARGEN + 18, fy, ROW_H, name(w).as_bytes(), c(tinta), e);
        fy += ROW_H;
    }
    if lista.len() > caben {
        let mut n = [0u8; 16];
        let k = cuantas_mas(&mut n, lista.len() - caben);
        let w = p.medir(&n[..k], fino::PIE) as u32;
        fino::texto(p, x + width - MARGEN - w, fy - ROW_H, ROW_H, &n[..k], c(fino::TENUE), fino::PIE);
    }
    fy += 8;
    if luz >= 1000 {
        fino::filete(p, x + MARGEN, fy, dentro);
    }
    fy += 8;

    // El modo, abajo: sin esto no hay forma de saber por que el foco se
    // comporta distinto de lo que esperabas. Y con el la tecla que lo cambia:
    // un modo que se lee pero no se toca invita a pensar que esta averiado.
    let mx = x + MARGEN;
    let mx = mx + fino::texto(p, mx, fy, PIE_H, b"modo  ", c(fino::TENUE), fino::PIE);
    let mx = mx + fino::texto(p, mx, fy, PIE_H, modo.as_bytes(), c(acento()), fino::PIE);
    fino::texto(p, mx, fy, PIE_H, b"    Ctrl+Tab", c(fino::TENUE), fino::PIE);
    fy += PIE_H;

    // ** Las flechas se anuncian AQUI y no en el pie de cada ventana.
    //
    // Porque este es el unico momento en que la mano ya tiene el Alt pulsado:
    // se lee la frase con el dedo puesto en la tecla que hace falta. En el pie
    // de CABINA seria una linea mas que se lee una vez y se olvida, y ademas
    // habria que repetirla en las tres ventanas -- tres sitios que actualizar
    // cuando el atajo cambie.
    let hx = x + MARGEN;
    let hx = hx + fino::texto(p, hx, fy, PIE_H, b"Ctrl+flechas  ", c(fino::TENUE), fino::PIE);
    let hx = hx + fino::texto(p, hx, fy, PIE_H, b"encajar", c(fino::TINTA), fino::PIE);
    let hx = hx + fino::texto(p, hx, fy, PIE_H, b"     +Shift  ", c(fino::TENUE), fino::PIE);
    fino::texto(p, hx, fy, PIE_H, b"mover", c(fino::TINTA), fino::PIE);
}

/// `+N mas`, sin formato (no hay `alloc`). Devuelve cuantos bytes.
fn cuantas_mas(b: &mut [u8; 16], n: usize) -> usize {
    let mut k = 0;
    b[k] = b'+';
    k += 1;
    let mut d = [0u8; 8];
    let (mut m, mut j) = (n.min(9_999_999), 0);
    loop {
        d[j] = b'0' + (m % 10) as u8;
        j += 1;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    while j > 0 {
        j -= 1;
        b[k] = d[j];
        k += 1;
    }
    for &c in b" mas" {
        b[k] = c;
        k += 1;
    }
    k
}

/// **Devuelve lo de debajo y guarda lo nuevo.** La tarjeta MEZCLA su borde y
/// su sombra con lo que hay: repintarla encima de si misma (cada Tab) los
/// oscureceria. Asi cada pintado empieza sobre lo que habia antes del
/// primero.
fn guardar(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32)) {
    let d = debajo();
    if d.puesto {
        let (gx, gy, gw, gh) = d.caja;
        p.marcar(gx, gy, gw, gh);
        for fy in 0..gh {
            for fx in 0..gw {
                p.punto_ya_marcado(gx + fx, gy + fy, d.px[(fy * gw + fx) as usize]);
            }
        }
    }
    let (gx, gy) = (x.saturating_sub(FUERA), y.saturating_sub(FUERA));
    let gw = (w + 2 * FUERA).min(p.ancho.saturating_sub(gx));
    let gh = (h + 2 * FUERA).min(p.alto.saturating_sub(gy));
    if (gw * gh) as usize > GUARDADO {
        d.puesto = false;
        return;
    }
    p.sincronizar_lectura();
    for fy in 0..gh {
        for fx in 0..gw {
            d.px[(fy * gw + fx) as usize] = p.read(gx + fx, gy + fy);
        }
    }
    d.caja = (gx, gy, gw, gh);
    d.puesto = true;
}

/// Que rectangulo ocupo, para poder borrarlo despues.
pub(crate) fn area(p: &bmo::Pantalla, count: usize) -> (u32, u32, u32, u32) {
    // Con la sombra y el rebote: lo que se borra al soltar Alt es todo lo que
    // se pudo pintar, aunque se suelte a mitad de abrirse.
    let (x, y, w, h) = run_box(p, count);
    let (gx, gy) = (x.saturating_sub(FUERA), y.saturating_sub(FUERA));
    (gx, gy, (w + 2 * FUERA).min(p.ancho.saturating_sub(gx)), (h + 2 * FUERA).min(p.alto.saturating_sub(gy)))
}
