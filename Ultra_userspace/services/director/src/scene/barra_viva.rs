//! **LA BARRA VIVA** (04-10) -- lo que se mueve en el panel de la izquierda.
//!
//! [consumo] LATE      solo mientras se mueve (`scene::vida`): ~420 ms cuando
//!                     cambia el foco o el minuto; en reposo NADA (L6h)
//!
//! El propietario: *"la barra lateral, con animaciones especiales y
//! unicas"*. Dos, y las dos por EVENTO -- se mueven cuando pasa algo, no en
//! bucle:
//!
//! ```text
//!    la marca     la pastilla de la ventana de delante VIAJA de una ficha a
//!                 otra con rebote cuando cambia el foco: se ve a donde fue
//!                 el teclado, no solo que se fue
//!    el reloj     al cambiar el minuto, la hora vieja sube y se apaga y la
//!                 nueva entra desde abajo, recortadas a su franja: un
//!                 contador de los de antes, pero con la letra de la casa
//! ```
//!
//! Aqui no se sabe de ventanas ni de la hora: llegan las fichas y los
//! numeros. Lo pinta `lateral.rs`, que es quien sabe donde va cada cosa.

use bmo_dibujo::Recorte;
use bmo_userland as bmo;

use super::fino;
use super::tema_gen::MARCA_FONDO;
use super::vida::{self, Paso, REBOTE, SALIDA};

/// Lo que tarda la marca en ir de una ficha a otra.
const VIAJE_MS: u32 = 460;
/// Lo que tarda el reloj en cambiar de minuto.
const RUEDA_MS: u32 = 420;

/// Lo que se pinta de una ficha.
#[derive(Clone, Copy)]
pub(crate) struct Vista<'a> {
    pub nombre: &'a [u8],
    pub color: u32,
    pub activa: bool,
    pub minimizada: bool,
}

/// Donde van las fichas.
#[derive(Clone, Copy)]
pub(crate) struct Sitio {
    pub x0: u32,
    pub iw: u32,
    pub y0: u32,
    pub fila: u32,
    pub alto: u32,
    pub max: u32,
}

struct Marca {
    /// La fila de la que sale y a la que va, en MILESIMAS de fila: si cambia
    /// el foco a mitad de un viaje, sale de donde esta, no de donde salio.
    desde: i32,
    hacia: i32,
    paso: Paso,
    /// La fila de delante la ultima vez (para saber si cambio).
    antes: Option<u32>,
    /// Se movia en el fotograma anterior: hace falta uno mas, el final.
    movia: bool,
}

static mut MARCA: Marca = Marca { desde: 0, hacia: 0, paso: Paso::QUIETO, antes: None, movia: false };

struct Rueda {
    viejo: [u8; 5],
    paso: Paso,
    movia: bool,
}

static mut RUEDA: Rueda = Rueda { viejo: [b' '; 5], paso: Paso::QUIETO, movia: false };

fn marca() -> &'static mut Marca {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(MARCA) }
}

fn rueda() -> &'static mut Rueda {
    // SAFETY: el mismo hilo, el mismo motivo.
    unsafe { &mut *core::ptr::addr_of_mut!(RUEDA) }
}

/// **Llegan fichas nuevas**: si la de delante cambio de fila, la marca viaja.
/// La primera vez no viaja: aparece donde esta.
pub(crate) fn delante(fila: Option<u32>) {
    let m = marca();
    match (m.antes, fila) {
        (Some(a), Some(b)) if a != b => {
            m.desde = if m.paso.vivo() { vida::entre(m.desde, m.hacia, m.paso.k(REBOTE)) } else { a as i32 * 1000 };
            m.hacia = b as i32 * 1000;
            m.paso = Paso::empezar(VIAJE_MS);
        }
        (_, Some(b)) => {
            m.desde = b as i32 * 1000;
            m.hacia = m.desde;
        }
        _ => {}
    }
    m.antes = fila;
}

/// Las fichas hay que repintarlas en este fotograma por la marca? Tambien
/// el fotograma de DESPUES de acabar: el ultimo tiene que ser el final.
pub(crate) fn fichas_se_mueven() -> bool {
    let m = marca();
    let vivo = m.paso.vivo();
    let pide = vivo || m.movia;
    m.movia = vivo;
    pide
}

/// **Las fichas**, con la marca donde toque a esta altura del viaje.
pub(crate) fn pintar_fichas(p: &bmo::Pantalla, s: Sitio, fichas: &[Vista], fondo: u32, tinta: u32, tenue: u32) {
    p.rect(s.x0 - 6, s.y0, s.iw + 12, s.max * s.fila, fondo);
    let m = marca();
    if let Some(k) = fichas.iter().position(|f| f.activa).filter(|&k| (k as u32) < s.max) {
        let rebote = m.paso.k(REBOTE);
        let fila = vida::entre(m.desde, m.hacia, rebote);
        // Mientras viaja, la marca se estira un poco en la direccion del
        // viaje y vuelve a su medida al llegar: se ve que VA, no que salta.
        let y = s.y0 as i32 + fila * s.fila as i32 / 1000;
        let estira = if m.paso.vivo() { ((1000 - (rebote - 500).abs() * 2).max(0) * 6 / 1000) as u32 } else { 0 };
        let color = fichas[k].color;
        let (x, w) = ((s.x0 - 6) as i32, (s.iw + 12) as i32);
        let alto = (s.alto + estira) as i32;
        p.caja_redonda(x, y - estira as i32 / 2, w, alto, 9, MARCA_FONDO);
        p.caja_redonda(x + 3, y + 6, 3, s.alto as i32 - 12, 1, color);
    }
    for (k, f) in fichas.iter().take(s.max as usize).enumerate() {
        let y = s.y0 + k as u32 * s.fila;
        let punto = if f.minimizada { tenue } else { f.color };
        p.caja_redonda((s.x0 + 4) as i32, (y + s.alto / 2 - 4) as i32, 8, 8, 4, punto);
        let t = if f.minimizada { tenue } else { tinta };
        let e = if f.activa { bmo::Estilo::media(13) } else { bmo::Estilo::normal(13) };
        // Lo que no cabe se corta con `..`, medido con la letra que lo pinta.
        let cabe = s.iw as i32 - 20;
        let mut corto = [0u8; 24];
        let mut nombre = f.nombre;
        if p.medir(nombre, e) > cabe {
            let mut n = nombre.len().min(corto.len() - 2);
            while n > 1 {
                corto[..n].copy_from_slice(&nombre[..n]);
                corto[n] = b'.';
                corto[n + 1] = b'.';
                if p.medir(&corto[..n + 2], e) <= cabe {
                    break;
                }
                n -= 1;
            }
            nombre = &corto[..n + 2];
        }
        fino::texto(p, s.x0 + 18, y, s.alto, nombre, t, e);
    }
}

/// **Cambio el minuto**: la hora vieja se va y la nueva entra. `viejo` es lo
/// que estaba pintado.
pub(crate) fn rueda_minuto(viejo: [u8; 5]) {
    let r = rueda();
    r.viejo = viejo;
    r.paso = Paso::empezar(RUEDA_MS);
}

/// El reloj hay que repintarlo en este fotograma por la rueda?
pub(crate) fn reloj_se_mueve() -> bool {
    let r = rueda();
    let vivo = r.paso.vivo();
    let pide = vivo || r.movia;
    r.movia = vivo;
    pide
}

/// **El reloj**: la hora (con su rueda, si gira) y el dia a la derecha, en
/// su franja de `alto` desde `y`.
pub(crate) fn pintar_reloj(p: &bmo::Pantalla, x0: u32, y: u32, iw: u32, alto: u32, hhmm: &[u8; 5], fecha: &[u8; 5], fondo: u32, tinta: u32, tenue: u32) {
    p.rect(x0, y, iw, alto, fondo);
    let franja = Recorte::nuevo(x0 as i32, y as i32, iw as i32, alto as i32);
    let r = rueda();
    let k = r.paso.k(SALIDA).clamp(0, 1000);
    let hora = |p: &bmo::Pantalla, texto: &[u8], dy: i32, c: u32| {
        p.pieza(
            &bmo::Pieza::Letra { x: x0 as i32, y: y as i32 + dy, alto: alto as i32, texto, c, px: 16, peso: 500, espacio: 0, mayusculas: false },
            0,
            0,
            Some(franja),
        );
    };
    if r.paso.vivo() {
        let sube = (alto as i32 * k) / 1000;
        hora(p, &r.viejo, -sube, bmo::entre_color(tinta, fondo, k));
        hora(p, hhmm, alto as i32 - sube, bmo::entre_color(fondo, tinta, k));
    } else {
        hora(p, hhmm, 0, tinta);
    }
    let e = bmo::Estilo::normal(12);
    let fx = (x0 + iw) as i32 - p.medir(fecha, e);
    p.letra_en_caja(fx, y as i32, alto as i32, fecha, tenue, e);
}
