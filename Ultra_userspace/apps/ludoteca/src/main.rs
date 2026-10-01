//! **LUDOTECA** -- F4: tus juegos de todas las tiendas (`sys/ludoteca.bex`).
//!
//! [consumo] LATIDO    mientras se ve, un fotograma cada 33 ms (los iconos y
//!                     el juego elegido estan vivos); quieta un minuto, cada
//!                     100 ms; si nadie la ve, no pinta y mira el buzon cada
//!                     200 ms (el byte de VISTA, R-APP8)
//!
//! J1 de `docs/plan/PLAN_LA_LUDOTECA.md`, con la cara que eligio el
//! propietario el 01-10: el armazon de una comunidad (riel de tiendas,
//! canales, el juego fijado y las piezas), sin retro, cada tienda con su
//! animacion propia, y una entrada con el gato de BMO-X y glitch.
//!
//! ```text
//!    los juegos   ludoteca/ludoteca.txt (las lineas de J0), o lo que HAY en
//!                 los discos: Cyberpunk en D:, DOOM en apps/ (`catalogo`)
//!    JUGAR        no lanza nada ella misma: la ventana del juego tiene que
//!                 colgar del ESCRITORIO, asi que se lo PIDE con una linea de
//!                 consola que empieza por 0x1E (`desktop::pide`): `personal
//!                 diario <exe>` o un `.bex` de apps/ o sys/
//! ```

#![no_std]
#![no_main]

extern crate alloc;

mod catalogo;
mod entrada;
mod iconos;
mod mates;
mod pintar;
mod tiendas;

/// La ventana y el lienzo son los del TALLER (F1): las mismas piezas, sin copia.
#[path = "../../taller/src/canvas.rs"]
#[allow(dead_code)]
mod canvas;
#[path = "../../taller/src/window.rs"]
#[allow(dead_code)]
mod window;
/// El monton de PROTON-X: un bloque que se reparte (el catalogo se lee una vez).
#[path = "../../proton-x/src/monton.rs"]
#[allow(dead_code)]
mod monton;
/// El gato de BMO-X, las mismas mascaras que el escritorio.
#[path = "../../../services/director/src/scene/gato.rs"]
#[allow(dead_code)]
mod gato;

use alloc::vec::Vec;
use bmo_userland as bmo;
use canvas::Canvas;
use catalogo::Catalogo;
use pintar::{Golpe, Vista, ALTO, ANCHO};
use tiendas::PUESTOS;
use window::{Input, Window};

#[global_allocator]
static MONTON: monton::Monton = monton::Monton::vacio();
const PARA_MONTON: u64 = 1 << 20;

/// Quieta tanto tiempo, la animacion baja el ritmo.
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

struct Reloj {
    hz: u64,
}

impl Reloj {
    fn ms(&self) -> u32 {
        ((bmo::ciclos() as u128 * 1000 / self.hz.max(1) as u128) & 0xFFFF_FFFF) as u32
    }
}

struct Estado {
    tienda: usize,
    visibles: Vec<usize>,
    sel: usize,
    desde_tienda: u32,
    desde_juego: u32,
    aviso: Vec<u8>,
}

impl Estado {
    fn elegir_tienda(&mut self, cat: &Catalogo, i: usize, ahora: u32) {
        self.tienda = i % PUESTOS.len();
        self.visibles = cat.de(PUESTOS[self.tienda].tienda);
        self.sel = 0;
        self.desde_tienda = ahora;
        self.desde_juego = ahora;
        self.aviso.clear();
    }

    fn elegir_juego(&mut self, k: usize, ahora: u32) {
        if k < self.visibles.len() && k != self.sel {
            self.sel = k;
            self.desde_juego = ahora;
            self.aviso.clear();
        }
    }

    /// **JUGAR**: se le pide al escritorio con una linea que empieza por 0x1E.
    fn jugar(&mut self, cat: &Catalogo) {
        let Some(&i) = self.visibles.get(self.sel) else { return };
        self.aviso.clear();
        match cat.orden(i) {
            Some(o) => {
                say("\u{1E}");
                say(&o);
                say("\n");
                self.aviso.extend_from_slice(b"pedido al escritorio: ");
                self.aviso.extend_from_slice(o.as_bytes());
            }
            None => self.aviso.extend_from_slice(b"todavia no hay como jugarlo aqui"),
        }
    }
}

/// Lo que hay en el disco, sin traerlo.
fn existe(ruta: &[u8]) -> bool {
    match bmo::Archivo::reflejar(ruta) {
        Ok(a) => {
            a.close();
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let Some(bloque) = bmo::Memoria::request(PARA_MONTON) else {
        say("LUDOTECA: NO -- sin memoria para el monton\n");
        bmo::salir();
    };
    // SAFETY: el bloque es nuestro, mide PARA_MONTON y vive lo que el proceso.
    unsafe { MONTON.poner(bloque.base() as usize, PARA_MONTON as usize) };
    core::mem::forget(bloque);
    let Some(mut win) = Window::open(ANCHO, ALTO) else {
        say("LUDOTECA: NO -- sin ventana (no hay memoria, o nadie me lanzo)\n");
        bmo::salir();
    };
    let mut cv = Canvas::new(win.px, win.w, win.h);
    let cat = Catalogo::abrir();
    let reloj = Reloj { hz: bmo::info(bmo::INFO_TSC_HZ) };
    let abierta = reloj.ms();
    let mut st = Estado { tienda: 0, visibles: Vec::new(), sel: 0, desde_tienda: abierta, desde_juego: abierta, aviso: Vec::new() };
    st.elegir_tienda(&cat, 0, abierta);
    let estratos = bmo::info(bmo::INFO_ES_MONTADO) != 0;
    let proton = existe(b"sys/proton-x.bex");
    // Los logos oficiales que el propietario ya dejo (ver `tiendas`).
    let logos: Vec<bool> = PUESTOS.iter().map(|p| p.logo()).collect();
    let mut tocada = abierta;
    let mut entrada = true;
    let mut vista_antes = u8::MAX;
    say(match cat.origen {
        catalogo::Origen::Fichero => "LUDOTECA: F4 abierta -- tus lineas de ludoteca/ludoteca.txt\n",
        catalogo::Origen::Discos => "LUDOTECA: F4 abierta -- lo que hay en los discos\n",
    });

    loop {
        let ahora = reloj.ms();
        let desde = ahora.wrapping_sub(abierta);
        entrada &= desde < entrada::DURA;

        while let Some(ev) = win.next() {
            tocada = ahora;
            // ** MAXIMIZAR LLENA LA VENTANA (01-10): el DIRECTOR dice el hueco
            // nuevo y la LUDOTECA se vuelve a pintar a esa medida, nitida --
            // no se estira: un estirado son pixeles gordos y dientes. Con menos
            // de lo minimo contesta con su minimo y el DIRECTOR la centra.
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
                st.desde_juego = ahora;
                continue;
            }
            match ev {
                Input::Mouse { x, y, buttons, down: true } if buttons & BOTON != 0 => match pintar::golpe(x, y, st.visibles.len()) {
                    Some(Golpe::Tienda(i)) => st.elegir_tienda(&cat, i, ahora),
                    Some(Golpe::Juego(k)) => st.elegir_juego(k, ahora),
                    Some(Golpe::Jugar) => st.jugar(&cat),
                    None => {}
                },
                Input::Char(0x80) => st.elegir_juego(st.sel.saturating_sub(1), ahora),
                Input::Char(0x81) => st.elegir_juego(st.sel + 1, ahora),
                Input::Char(0x82) => st.elegir_tienda(&cat, st.tienda + PUESTOS.len() - 1, ahora),
                Input::Char(0x83 | b'\t' | b't' | b'T') => st.elegir_tienda(&cat, st.tienda + 1, ahora),
                Input::Char(b'\r' | b'\n') => st.jugar(&cat),
                Input::Char(0x1B) => {
                    say("LUDOTECA: cerrada con Esc\n");
                    bmo::salir();
                }
                _ => {}
            }
        }

        let ptr = win.pointer();
        let se_ve = ptr.view as u64 == bmo::SUP_VISTA_SE_VE;
        if ptr.view != vista_antes {
            vista_antes = ptr.view;
        }
        if se_ve {
            if entrada && desde < entrada::FUNDIDO {
                entrada::pintar(&mut cv, desde);
            } else {
                let v = Vista {
                    cat: &cat,
                    tienda: st.tienda,
                    visibles: &st.visibles,
                    sel: st.sel,
                    ms: ahora,
                    desde_tienda: st.desde_tienda,
                    desde_juego: if entrada { abierta.wrapping_add(entrada::FUNDIDO) } else { st.desde_juego },
                    puntero: ptr.inside.then_some((ptr.x, ptr.y)),
                    aviso: &st.aviso,
                    estratos,
                    proton,
                    logos: &logos,
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
    say("LUDOTECA: panico\n");
    if let Some(s) = info.message().as_str() {
        say(s);
        say("\n");
    }
    bmo::salir();
}
