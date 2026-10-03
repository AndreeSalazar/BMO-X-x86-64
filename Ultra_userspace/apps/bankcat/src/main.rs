//! **BANK CAT** -- F5: la cartera de CAB y el gato hucha (`sys/bankcat.bex`).
//!
//! [consumo] LATIDO    mientras se ve, un fotograma cada 33 ms (el gato esta
//!                     vivo); quieta un minuto, cada 100 ms; si nadie la ve,
//!                     no pinta y mira su consola cada 200 ms
//!
//! BC3 de `docs/plan/PLAN_BANK_CAT.md`, con la cara de la maqueta
//! (`docs/arte/maqueta_bankcat.html`).
//!
//! ** Esta app NO lleva el dinero. El libro lo lleva el MOTOR COBOL
//! (`cobol/11/libro.bex`, compilado a x86-64 con la libreria de copybooks
//! CABDATOS + CABLIBRO), y el motor es del ESCRITORIO: una app nace sin la
//! autoridad de lanzar (`task/autoridad.rs`). Asi que se PIDE, como la ONDA
//! de HERMES:
//!
//! ```text
//!    yo -> escritorio     0x1E bankcat pagar 19.99      (mi consola)
//!    escritorio -> yo     0x1E bank 0 124003            (mi entrada)
//! ```
//!
//! y el saldo que se ve es el que el motor contesto, al centimo.

#![no_std]
#![no_main]

extern crate alloc;

mod gato;
mod pintar;
/// La paleta de la maqueta, GENERADA (`espejo-cara tinta`).
mod tinta;

/// La ventana y el lienzo son los del TALLER (F1).
#[path = "../../taller/src/canvas.rs"]
#[allow(dead_code)]
mod canvas;
#[path = "../../taller/src/window.rs"]
#[allow(dead_code)]
mod window;
#[path = "../../proton-x/src/monton.rs"]
#[allow(dead_code)]
mod monton;
/// Las cuentas de las animaciones, las de la LUDOTECA.
#[path = "../../ludoteca/src/mates.rs"]
#[allow(dead_code)]
mod mates;
/// Las piezas de la cara (cajas redondas, negrita, rotulos, trazos): las de
/// HERMES, la misma familia.
#[path = "../../hermes/src/piezas.rs"]
#[allow(dead_code)]
mod piezas;

use alloc::vec::Vec;
use bmo_bankcat::{Centimos, Estado};
use bmo_userland as bmo;
use canvas::Canvas;
use gato::Humor;
use pintar::{Asiento, Golpe, Vista, ALTO, ANCHO, SECCIONES};
use window::{Input, Window};

#[global_allocator]
static MONTON: monton::Monton = monton::Monton::vacio();
const PARA_MONTON: u64 = 1 << 20;

const REPOSO_MS: u32 = 60_000;
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

const FRASE_QUIETO: &[u8] = b"\"Miau. Cada centimo, apuntado dos veces.\"";
const FRASES_RICO: [&[u8]; 3] = [b"\"Miau! Al libro, y por partida doble.\"", b"\"Purr... huele a interes compuesto.\"", b"\"Esto va derecho a la ranura.\""];
const FRASES_TRISTE: [&[u8]; 3] = [b"\"Snif. Apuntado: no se borra.\"", b"\"Que no se acabe el atun, eh.\"", b"\"Pagar tambien es cuadrar.\""];
const FRASE_NO_FIA: &[u8] = b"\"NO: no hay saldo. El gato no fia.\"";

struct App {
    sec: usize,
    desde_sec: u32,
    saldo: Option<Centimos>,
    libro: Vec<Asiento>,
    humor: Humor,
    humor_desde: u32,
    frase: &'static [u8],
    esperando: u32,
    aviso: Vec<u8>,
    /// Lo que llega por la entrada, hasta su `\n`.
    linea: Vec<u8>,
    vueltas: usize,
}

impl App {
    /// **Le pide al escritorio** una orden para el motor.
    fn pedir(&mut self, orden: &str) {
        say("\u{1E}bankcat ");
        say(orden);
        say("\n");
        self.esperando += 1;
    }

    fn elegir(&mut self, s: usize, ahora: u32) {
        let s = s % SECCIONES.len();
        if s != self.sec {
            self.sec = s;
            self.desde_sec = ahora;
            self.aviso.clear();
        }
    }

    fn boton(&mut self, k: usize) {
        match k {
            0 => self.pedir("cobrar 50.00"),
            1 => self.pedir("pagar 19.99"),
            _ => {
                self.pedir("veces 3");
                self.pedir("pagar 19.99");
            }
        }
    }

    /// **Una linea de la entrada.** Solo cuentan las del escritorio para
    /// BANK CAT (`0x1E bank ...`); el resto no es para mi.
    fn oir(&mut self, ahora: u32) {
        let Some(resto) = self.linea.strip_prefix(b"\x1Ebank ") else { return };
        self.esperando = self.esperando.saturating_sub(1);
        if let Some(motivo) = resto.strip_prefix(b"no ") {
            self.aviso.clear();
            self.aviso.extend_from_slice(motivo);
            return;
        }
        let estado = resto.first().and_then(|&c| Estado::de(c));
        let saldo = resto.get(2..).and_then(|s| {
            let (neg, s) = match s.first() {
                Some(b'-') => (true, &s[1..]),
                _ => (false, s),
            };
            if s.is_empty() || !s.iter().all(u8::is_ascii_digit) || s.len() > 18 {
                return None;
            }
            let v = s.iter().fold(0i64, |a, &c| a * 10 + (c - b'0') as i64);
            Some(if neg { -v } else { v })
        });
        let (Some(estado), Some(saldo)) = (estado, saldo) else {
            self.aviso.clear();
            self.aviso.extend_from_slice(b"el escritorio contesto algo que no se entiende");
            return;
        };
        let cambio = self.saldo.map_or(0, |antes| saldo - antes);
        self.saldo = Some(saldo);
        self.vueltas += 1;
        let k = self.vueltas % 3;
        // Sin disco (5): el movimiento SI se hizo, y se avisa de que el libro
        // no quedo guardado.
        let sin_disco = estado == Estado::SinDisco;
        let estado = if sin_disco { Estado::Hecho } else { estado };
        if estado != Estado::Hecho {
            self.humor = Humor::Triste;
            self.humor_desde = ahora;
            self.frase = if estado == Estado::SinSaldo { FRASE_NO_FIA } else { FRASES_TRISTE[k] };
            self.libro.push(Asiento { cambio: 0, estado, saldo });
        } else if cambio != 0 {
            self.humor = if cambio > 0 { Humor::Rico } else { Humor::Triste };
            self.humor_desde = ahora;
            self.frase = if cambio > 0 { FRASES_RICO[k] } else { FRASES_TRISTE[k] };
            self.libro.push(Asiento { cambio, estado, saldo });
        }
        // Lo mas viejo se olvida: el libro de verdad es el del motor.
        if self.libro.len() > 200 {
            self.libro.remove(0);
        }
        self.aviso.clear();
        if sin_disco {
            self.aviso.extend_from_slice(Estado::SinDisco.texto().as_bytes());
        }
    }
}

fn cerrar(por: &str) -> ! {
    say("BANK CAT: cerrada con ");
    say(por);
    say("\n");
    bmo::salir();
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let Some(bloque) = bmo::Memoria::request(PARA_MONTON) else {
        say("BANK CAT: NO -- sin memoria para el monton\n");
        bmo::salir();
    };
    // SAFETY: el bloque es nuestro, mide PARA_MONTON y vive lo que el proceso.
    unsafe { MONTON.poner(bloque.base() as usize, PARA_MONTON as usize, bloque.handle()) };
    core::mem::forget(bloque);
    let Some(mut win) = Window::open(ANCHO, ALTO) else {
        say("BANK CAT: NO -- sin ventana (no hay memoria, o nadie me lanzo)\n");
        bmo::salir();
    };
    let mut cv = Canvas::new(win.px, win.w, win.h);
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let reloj = || ((bmo::ciclos() as u128 * 1000 / hz as u128) & 0xFFFF_FFFF) as u32;
    let abierta = reloj();
    let mut st = App {
        sec: 0,
        desde_sec: abierta,
        saldo: None,
        libro: Vec::new(),
        humor: Humor::Quieto,
        humor_desde: 0,
        frase: FRASE_QUIETO,
        esperando: 0,
        aviso: Vec::new(),
        linea: Vec::new(),
        vueltas: 0,
    };
    say("BANK CAT: F5 abierta -- el libro lo lleva el motor COBOL del escritorio\n");
    // El saldo de ahora (y, si el motor nace, el libro se abre).
    st.pedir("hola");
    let mut tocada = abierta;

    loop {
        let ahora = reloj();
        // Lo que el escritorio contesto.
        for _ in 0..32 {
            let mut b = [0u8; 8];
            let n = bmo::leer_consola(&mut b);
            if n == 0 {
                break;
            }
            for &x in &b[..n] {
                if x == b'\n' {
                    st.oir(ahora);
                    st.linea.clear();
                } else if st.linea.len() < 96 {
                    st.linea.push(x);
                }
            }
            tocada = ahora;
        }

        while let Some(ev) = win.next() {
            tocada = ahora;
            if let Input::Resize { w, h } = ev {
                let (w, h) = (w.max(pintar::MINIMO.0), h.max(pintar::MINIMO.1));
                if win.resize(w, h) {
                    pintar::medir(w, h);
                    cv = Canvas::new(win.px, win.w, win.h);
                }
                continue;
            }
            match ev {
                Input::Mouse { x, y, buttons, down: true } if buttons & BOTON != 0 => match pintar::golpe(x, y, st.sec) {
                    Some(Golpe::Seccion(s)) => st.elegir(s, ahora),
                    Some(Golpe::Boton(k)) => st.boton(k),
                    Some(Golpe::Cerrar) => cerrar("la X"),
                    None => {}
                },
                Input::Char(0x82) => st.elegir(st.sec + SECCIONES.len() - 1, ahora),
                Input::Char(0x83 | b'\t') => st.elegir(st.sec + 1, ahora),
                Input::Char(c @ b'1'..=b'6') => st.elegir((c - b'1') as usize, ahora),
                Input::Char(b'r' | b'R' | b'c' | b'C') if st.sec == pintar::CARTERA => st.boton(0),
                Input::Char(b'e' | b'E' | b'p' | b'P') if st.sec == pintar::CARTERA => st.boton(1),
                Input::Char(b't' | b'T') if st.sec == pintar::CARTERA => st.boton(2),
                Input::Char(0x1B) => cerrar("Esc"),
                _ => {}
            }
        }

        let ptr = win.pointer();
        let se_ve = ptr.view as u64 == bmo::SUP_VISTA_SE_VE;
        if se_ve {
            let v = Vista {
                sec: st.sec,
                ms: ahora,
                desde_sec: st.desde_sec,
                puntero: ptr.inside.then_some((ptr.x, ptr.y)),
                saldo: st.saldo,
                libro: &st.libro,
                humor: st.humor,
                humor_desde: st.humor_desde,
                frase: st.frase,
                esperando: st.esperando,
                aviso: &st.aviso,
            };
            pintar::pintar(&mut cv, &v);
            win.present();
        }
        let quieta = ahora.wrapping_sub(tocada) > REPOSO_MS;
        let siesta: u64 = if !se_ve {
            200
        } else if !quieta || st.esperando > 0 {
            33
        } else {
            100
        };
        bmo::wait(0, 0, siesta * 1_000_000);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    say("BANK CAT: panico\n");
    if let Some(s) = info.message().as_str() {
        say(s);
        say("\n");
    }
    bmo::salir();
}
