//! **HERMES** -- F3: dos BMO-X que se hablan, sin servidor de nadie
//! (`sys/hermes.bex`).
//!
//! [consumo] LATIDO    mientras se ve, un fotograma cada 33 ms (cada seccion
//!                     esta viva); quieta un minuto, cada 100 ms; si nadie la
//!                     ve, no pinta y mira el buzon cada 200 ms (el byte de
//!                     VISTA, R-APP8)
//!
//! H5 de `docs/plan/PLAN_HERMES.md`, *"la app en F3, en una sola maquina"*,
//! con la cara de la maqueta (H1): la entrada con el gato ALADO y su glitch,
//! el riel de nueve secciones, cada una con su gesto y su manera de entrar.
//!
//! ```text
//!    de verdad    lo escrito (tus notas y las tertulias) se guarda en
//!                 sys/hermsg.txt y sigue ahi al volver a abrir; la ONDA
//!                 toca en el ESCRITORIO y sus barras son el medidor del
//!                 maestro; el ZUMBIDO sacude, destella y suena
//!    aun no       la red (H4, H6), los amigos y su huella (H6), las fotos
//!                 (H9), el video (H10), las paginas (H13-H15): cada seccion
//!                 lo dice con su escalon, sin inventar a nadie
//! ```
//!
//! La musica y los avisos son del ESCRITORIO, no de HERMES: se le PIDEN con
//! una linea de consola que empieza por 0x1E (`desktop::pide`), como el JUGAR
//! de la LUDOTECA. Asi cerrar HERMES no corta la musica (la PASTILLA sigue).

#![no_std]
#![no_main]

extern crate alloc;

mod charla;
mod entrada;
mod iconos;
mod onda;
mod panel;
/// Compartidas con BANK CAT (que las usa todas: la sombra, los caminos de
/// SVG...); HERMES aun no usa algunas.
#[allow(dead_code)]
mod piezas;
mod pintar;
mod reproductor;

/// La ventana y el lienzo son los del TALLER (F1): las mismas piezas, sin copia.
#[path = "../../taller/src/canvas.rs"]
#[allow(dead_code)]
mod canvas;
#[path = "../../taller/src/window.rs"]
#[allow(dead_code)]
mod window;
/// El monton de PROTON-X: un bloque que se reparte.
#[path = "../../proton-x/src/monton.rs"]
#[allow(dead_code)]
mod monton;
/// El gato de BMO-X, las mismas mascaras que el escritorio.
#[path = "../../../services/director/src/scene/gato.rs"]
#[allow(dead_code)]
mod gato;
/// Las cuentas de las animaciones, las de la LUDOTECA.
#[path = "../../ludoteca/src/mates.rs"]
#[allow(dead_code)]
mod mates;

use alloc::vec::Vec;
use bmo_userland as bmo;
use canvas::Canvas;
use pintar::{Golpe, Vista, ALTO, ANCHO, SECCIONES};
use window::{Input, Window};

#[global_allocator]
static MONTON: monton::Monton = monton::Monton::vacio();
const PARA_MONTON: u64 = 1 << 20;

/// Quieta tanto tiempo, la animacion baja el ritmo.
const REPOSO_MS: u32 = 60_000;
/// Un ZUMBIDO cada 10 s, como en la maqueta: es para llamar, no para molestar.
const ZUMBIDO_CADA_MS: u32 = 10_000;
const BOTON: u8 = 1;

fn say(s: &str) {
    bmo::consola(s);
}

/// Un numero en decimal, en `out`; devuelve cuantos bytes.
pub fn fmt_num(mut v: u64, out: &mut [u8]) -> usize {
    let mut d = [0u8; 20];
    let mut k = 0;
    loop {
        d[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 || k == d.len() {
            break;
        }
    }
    let n = k.min(out.len());
    for i in 0..n {
        out[i] = d[k - 1 - i];
    }
    n
}

/// Los ms del reloj de la maquina, para quien no tiene el `Reloj` a mano.
fn ahora_ms() -> u32 {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    ((bmo::ciclos() as u128 * 1000 / hz as u128) & 0xFFFF_FFFF) as u32
}

struct Reloj {
    hz: u64,
}

impl Reloj {
    fn ms(&self) -> u32 {
        ((bmo::ciclos() as u128 * 1000 / self.hz.max(1) as u128) & 0xFFFF_FFFF) as u32
    }

    fn segundos(&self) -> u64 {
        bmo::ciclos() / self.hz.max(1)
    }
}

struct Estado {
    sec: usize,
    item: usize,
    desde_sec: u32,
    desde_item: u32,
    escribiendo: bool,
    borrador: Vec<u8>,
    enviado: u32,
    zumbido: Option<u32>,
    pedida: Option<usize>,
    pedida_desde: u32,
    aviso: Vec<u8>,
    /// El REPRODUCTOR: la pausa pedida y el volumen pedido. El escritorio
    /// empieza a 70 (`desktop::musica`).
    pausada: bool,
    volumen: u32,
}

impl Estado {
    fn elegir_seccion(&mut self, s: usize, ahora: u32) {
        let s = s % SECCIONES.len();
        if s != self.sec {
            self.sec = s;
            self.item = 0;
            self.desde_sec = ahora;
            self.desde_item = ahora;
            self.escribiendo = false;
            self.aviso.clear();
        }
    }

    fn elegir_item(&mut self, k: usize, ahora: u32) {
        let n = pintar::cuantos_items(self.sec);
        if k < n && k != self.item {
            self.item = k;
            self.desde_item = ahora;
            self.aviso.clear();
        }
        // En la ONDA, elegir es TOCAR (como un clic en una lista de musica).
        if self.sec == pintar::ONDA && k < n {
            self.tocar(k);
        }
    }

    /// **La ONDA**: se le pide al escritorio que toque la pieza `k`.
    fn tocar(&mut self, k: usize) {
        let mut d = [0u8; 4];
        let n = fmt_num(k as u64, &mut d);
        say("\u{1E}fondo ");
        // SAFETY: son cifras ASCII.
        say(unsafe { core::str::from_utf8_unchecked(&d[..n]) });
        say("\n");
        self.pedida = Some(k);
        self.pedida_desde = crate::ahora_ms();
        // Una pieza nueva suena: la pausa, si la habia, ya no.
        self.pausada = false;
    }

    /// **Un mando del REPRODUCTOR**, pedido al escritorio como la PASTILLA.
    fn mando(&mut self, m: reproductor::Mando) {
        use reproductor::Mando;
        let n = pintar::PIEZAS_N;
        match m {
            Mando::PausaOSigue if self.pedida.is_none() => self.tocar(if self.sec == pintar::ONDA { self.item } else { 0 }),
            Mando::PausaOSigue => {
                say("\u{1E}fondo pausa\n");
                self.pausada = !self.pausada;
            }
            Mando::Anterior => self.tocar((self.pedida.unwrap_or(0) + n - 1) % n),
            Mando::Siguiente => self.tocar((self.pedida.unwrap_or(n - 1) + 1) % n),
            Mando::Azar => {
                let otra = (crate::ahora_ms() as usize / 7 + 1) % n;
                self.tocar(if Some(otra) == self.pedida { (otra + 1) % n } else { otra });
            }
            Mando::Volumen(v) => {
                let mut d = [0u8; 4];
                let k = fmt_num(v.min(100) as u64, &mut d);
                say("\u{1E}fondo volumen ");
                // SAFETY: son cifras ASCII.
                say(unsafe { core::str::from_utf8_unchecked(&d[..k]) });
                say("\n");
                self.volumen = v.min(100);
            }
        }
    }

    /// **El ZUMBIDO**: sacude, destella y suena (el sonido lo pone el
    /// escritorio, que es quien tiene la musica y sus avisos).
    fn zumbar(&mut self, ahora: u32) {
        if let Some(t) = self.zumbido {
            if ahora.wrapping_sub(t) < ZUMBIDO_CADA_MS {
                self.aviso.clear();
                self.aviso.extend_from_slice(b"un zumbido cada 10 s: es para llamar, no para molestar");
                return;
            }
        }
        self.zumbido = Some(ahora);
        self.aviso.clear();
        say("\u{1E}aviso zumbido\n");
    }
}

/// La X de la barra: como Esc.
fn cerrar() -> ! {
    say("HERMES: cerrada con la X\n");
    bmo::salir();
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let Some(bloque) = bmo::Memoria::request(PARA_MONTON) else {
        say("HERMES: NO -- sin memoria para el monton\n");
        bmo::salir();
    };
    // SAFETY: el bloque es nuestro, mide PARA_MONTON y vive lo que el proceso.
    unsafe { MONTON.poner(bloque.base() as usize, PARA_MONTON as usize, bloque.handle()) };
    core::mem::forget(bloque);
    let Some(mut win) = Window::open(ANCHO, ALTO) else {
        say("HERMES: NO -- sin ventana (no hay memoria, o nadie me lanzo)\n");
        bmo::salir();
    };
    let mut cv = Canvas::new(win.px, win.w, win.h);
    let reloj = Reloj { hz: bmo::info(bmo::INFO_TSC_HZ) };
    let abierta = reloj.ms();
    let mut st = Estado {
        sec: 0,
        item: 0,
        desde_sec: abierta,
        desde_item: abierta,
        escribiendo: false,
        borrador: Vec::new(),
        enviado: 0,
        zumbido: None,
        pedida: None,
        pedida_desde: 0,
        pausada: false,
        volumen: 70,
        aviso: Vec::new(),
    };
    let mut charla = charla::Charla::abrir();
    let mut tocada = abierta;
    let mut entrada = true;
    say("HERMES: F3 abierta -- en una sola maquina (H5): lo escrito, en sys/hermsg.txt\n");

    loop {
        let ahora = reloj.ms();
        let desde = ahora.wrapping_sub(abierta);
        entrada &= desde < entrada::DURA;

        while let Some(ev) = win.next() {
            tocada = ahora;
            // Maximizar llena la ventana: se vuelve a pintar a la medida
            // nueva, nitida (como la LUDOTECA).
            if let Input::Resize { w, h } = ev {
                let (w, h) = (w.max(pintar::MINIMO.0), h.max(pintar::MINIMO.1));
                if win.resize(w, h) {
                    pintar::medir(w, h);
                    cv = Canvas::new(win.px, win.w, win.h);
                }
                continue;
            }
            if entrada {
                // La primera tecla o clic solo corta la entrada.
                entrada = false;
                st.desde_sec = ahora;
                st.desde_item = ahora;
                continue;
            }
            // ** ESCRIBIENDO: las letras son del mensaje.
            if st.escribiendo {
                match ev {
                    Input::Char(0x1B) => st.escribiendo = false,
                    Input::Char(b'\r' | b'\n') => {
                        if let Some(c) = pintar::canal_de(st.sec, st.item) {
                            if !st.borrador.is_empty() {
                                charla.mandar(c, &st.borrador, reloj.segundos());
                                st.enviado = ahora;
                                if charla.sin_guardar {
                                    say("HERMES: el mensaje no se pudo guardar en sys/hermsg.txt\n");
                                }
                            }
                        }
                        st.borrador.clear();
                    }
                    Input::Char(0x08 | 0x7F) => {
                        st.borrador.pop();
                    }
                    Input::Char(c) if (0x20..0x7F).contains(&c) || c >= 0xA0 => {
                        if st.borrador.len() < charla::LARGO {
                            st.borrador.push(c);
                        }
                    }
                    Input::Mouse { x, y, buttons, down: true } if buttons & BOTON != 0 => {
                        match pintar::golpe(x, y, st.sec, st.item) {
                            Some(Golpe::Escribir) => {}
                            Some(Golpe::Zumbido) => st.zumbar(ahora),
                            Some(Golpe::Cerrar) => cerrar(),
                            Some(Golpe::Repro(m)) => st.mando(m),
                            Some(Golpe::Poner(t)) => {
                                if st.borrador.len() + t.len() <= charla::LARGO {
                                    st.borrador.extend_from_slice(t);
                                }
                            }
                            otro => {
                                st.escribiendo = false;
                                match otro {
                                    Some(Golpe::Seccion(s)) => st.elegir_seccion(s, ahora),
                                    Some(Golpe::Item(k)) => st.elegir_item(k, ahora),
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
            let escribible = pintar::canal_de(st.sec, st.item).is_some();
            match ev {
                Input::Mouse { x, y, buttons, down: true } if buttons & BOTON != 0 => match pintar::golpe(x, y, st.sec, st.item) {
                    Some(Golpe::Seccion(s)) => st.elegir_seccion(s, ahora),
                    Some(Golpe::Item(k)) => st.elegir_item(k, ahora),
                    Some(Golpe::Escribir) => st.escribiendo = true,
                    Some(Golpe::Zumbido) => st.zumbar(ahora),
                    Some(Golpe::Cerrar) => cerrar(),
                    Some(Golpe::Repro(m)) => st.mando(m),
                    Some(Golpe::Poner(t)) => {
                        st.escribiendo = true;
                        if st.borrador.len() + t.len() <= charla::LARGO {
                            st.borrador.extend_from_slice(t);
                        }
                    }
                    None => {}
                },
                Input::Char(b'/') if escribible => st.escribiendo = true,
                // Como la LUDOTECA: arriba y abajo, la lista; izquierda y
                // derecha (y Tab), las secciones del riel.
                Input::Char(0x80) => {
                    let n = pintar::cuantos_items(st.sec);
                    st.elegir_item((st.item + n - 1) % n, ahora);
                }
                Input::Char(0x81) => {
                    let n = pintar::cuantos_items(st.sec);
                    st.elegir_item((st.item + 1) % n, ahora);
                }
                Input::Char(0x82) => st.elegir_seccion(st.sec + SECCIONES.len() - 1, ahora),
                Input::Char(0x83 | b'\t') => st.elegir_seccion(st.sec + 1, ahora),
                Input::Char(c @ b'1'..=b'9') => st.elegir_seccion((c - b'1') as usize, ahora),
                Input::Char(b'z' | b'Z') if st.sec == pintar::MENSAJES => st.zumbar(ahora),
                Input::Char(b'\r' | b'\n') if st.sec == pintar::ONDA => st.tocar(st.item),
                Input::Char(b'\r' | b'\n') if escribible => st.escribiendo = true,
                Input::Char(0x1B) => {
                    say("HERMES: cerrada con Esc\n");
                    bmo::salir();
                }
                _ => {}
            }
        }

        let ptr = win.pointer();
        let se_ve = ptr.view as u64 == bmo::SUP_VISTA_SE_VE;
        if se_ve {
            if entrada && desde < entrada::FUNDIDO {
                entrada::pintar(&mut cv, desde);
            } else {
                let canal = pintar::canal_de(st.sec, st.item);
                let del_canal = canal.map(|c| charla.de(c)).unwrap_or_default();
                let cuentas: Vec<usize> =
                    (0..pintar::cuantos_items(st.sec)).map(|k| pintar::canal_de(st.sec, k).map_or(0, |c| charla.cuantos(c))).collect();
                let (pico, rms) = pintar::medidor();
                let fondo = bmo::info(bmo::INFO_AUDIO_FONDO);
                let v = Vista {
                    sec: st.sec,
                    item: st.item,
                    ms: ahora,
                    desde_sec: if entrada { abierta.wrapping_add(entrada::FUNDIDO) } else { st.desde_sec },
                    desde_item: if entrada { abierta.wrapping_add(entrada::FUNDIDO) } else { st.desde_item },
                    puntero: ptr.inside.then_some((ptr.x, ptr.y)),
                    charla: &del_canal,
                    cuentas: &cuentas,
                    escribiendo: st.escribiendo,
                    borrador: &st.borrador,
                    enviado: st.enviado,
                    zumbido: st.zumbido,
                    zumbido_listo: st.zumbido.map_or(true, |t| ahora.wrapping_sub(t) >= ZUMBIDO_CADA_MS),
                    pedida: st.pedida,
                    pedida_desde: st.pedida_desde,
                    // Los canales de musica del fondo (los 8 de abajo) suenan.
                    sonando: fondo & 0xFF != 0 && fondo >> 48 != 0,
                    pico,
                    rms,
                    sin_guardar: charla.sin_guardar,
                    aviso: &st.aviso,
                    pausada: st.pausada,
                    volumen: st.volumen,
                };
                pintar::pintar(&mut cv, &v);
                if entrada {
                    entrada::fundido(&mut cv, desde);
                }
            }
            win.present();
        }
        let quieta = ahora.wrapping_sub(tocada) > REPOSO_MS;
        let siesta: u64 = if !se_ve {
            200
        } else if entrada || !quieta {
            33
        } else {
            100
        };
        bmo::wait(0, 0, siesta * 1_000_000);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    say("HERMES: panico\n");
    if let Some(s) = info.message().as_str() {
        say(s);
        say("\n");
    }
    bmo::salir();
}
