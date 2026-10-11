//! **ESTUDIO** (VE2 de `docs/plan/PLAN_VERRANO.md`, 11-10): VERRANO ESTUDIO
//! VIVO en BMO-X. La luz estilo RenderMan -- trazado de caminos --, en
//! tiempo real porque SE RECUERDA: cada fotograma calcula un cuarto de los
//! pixeles, la imagen se acumula y la memoria de la luz del mundo sobrevive
//! a la camara que se mueve. Con TODOS los nucleos (`smp all`: los
//! sub-directores); sin ellos, con este.
//!
//! ```text
//!    a d   la camara a un lado y al otro      w s   mas cerca, mas lejos
//!    r f   arriba, abajo                      m     la memoria de la luz si / no
//!    Esc o q   cierra
//! ```
//!
//! La consola dice, cada 32 fotogramas, cuanto tarda uno y en cuantas partes
//! va: lo que se mira en el metal (fila VE2 de EL_FOCO).

#![no_std]
#![no_main]

extern crate alloc;

#[path = "../../taller/src/window.rs"]
#[allow(dead_code)]
mod window;

#[path = "../../proton-x/src/monton.rs"]
#[allow(dead_code)]
mod monton;

mod obreros;

use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use bmo_userland as bmo;
use bmo_verrano_trama::estudio::{caja_de_cornell, pelicula, Camara, Escena, V3};
use bmo_verrano_trama::vivo::{Apunte, Cuaderno, Memoria, Parte, Vivo, APUNTES_POR_CAMINO};
use window::{Input, Window};

#[global_allocator]
static MONTON: monton::Monton = monton::Monton::vacio();
/// La imagen, sus muestras, la memoria de la luz (8 MiB) y las hojas.
const PARA_MONTON: u64 = 48 << 20;

/// Lo que se calcula; la ventana, al doble.
const W: u32 = 320;
const H: u32 = 240;
const ESCALA: u32 = 2;

fn say(s: &str) {
    bmo::consola(s);
}

fn numero(mut v: u64) {
    let mut d = [0u8; 20];
    let mut k = d.len();
    loop {
        k -= 1;
        d[k] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    say(core::str::from_utf8(&d[k..]).unwrap_or("?"));
}

fn memoria() -> Memoria {
    Memoria::nueva(18, 0.05, 8)
}

/// Los buferes de un fotograma, repartidos por filas: cada parte toca SOLO
/// los suyos (por eso se pasan como direcciones).
struct Buferes {
    nuevas: usize,
    hojas: usize,
    usadas: [AtomicUsize; obreros::MAX as usize],
}

fn filas(k: u32, n: u32) -> (u32, u32) {
    (k * H / n, (k + 1) * H / n)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let Some(bloque) = bmo::Memoria::request(PARA_MONTON) else {
        say("ESTUDIO: NO -- sin memoria para el monton\n");
        bmo::salir();
    };
    // SAFETY: el bloque es nuestro, mide PARA_MONTON y vive lo que el proceso.
    unsafe { MONTON.poner(bloque.base() as usize, PARA_MONTON as usize, bloque.handle()) };
    core::mem::forget(bloque);
    let Some(mut win) = Window::open(W * ESCALA, H * ESCALA) else {
        say("ESTUDIO: NO -- sin ventana (abrelo desde el escritorio)\n");
        bmo::salir();
    };
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let ms = || (bmo::ciclos() as u128 * 1000 / hz as u128) as u64;
    let (objetos, luz, base) = caja_de_cornell();
    let mut ojo = base.ojo;
    let mut con_memoria = true;
    let mut vivo = Vivo::nuevo(W, H, Some(memoria()));
    let mut nuevas: Vec<V3> = vec![[0.0; 3]; (W * H) as usize];
    let mut hojas: Vec<Apunte> = vec![Apunte { p: [0.0; 3], n: [0.0; 3], l: [0.0; 3] }; (W * H) as usize * APUNTES_POR_CAMINO];
    say("ESTUDIO: VERRANO ESTUDIO VIVO -- la luz estilo RenderMan que se recuerda (VE2)\n");
    let mut desde = ms();
    loop {
        // la entrada: mover la camara olvida la imagen, NO la memoria
        let mut movida = false;
        while let Some(ev) = win.next() {
            if let Input::Char(c) = ev {
                match c {
                    27 | b'q' => bmo::salir(),
                    b'a' => ojo[0] = (ojo[0] - 0.1).max(-0.8),
                    b'd' => ojo[0] = (ojo[0] + 0.1).min(0.8),
                    b'w' => ojo[2] = (ojo[2] - 0.1).max(2.0),
                    b's' => ojo[2] = (ojo[2] + 0.1).min(3.8),
                    b'r' => ojo[1] = (ojo[1] + 0.1).min(1.8),
                    b'f' => ojo[1] = (ojo[1] - 0.1).max(0.3),
                    b'm' => {
                        con_memoria = !con_memoria;
                        vivo = Vivo::nuevo(W, H, con_memoria.then(memoria));
                        say(if con_memoria { "ESTUDIO: con la memoria de la luz\n" } else { "ESTUDIO: SIN la memoria de la luz\n" });
                    }
                    _ => continue,
                }
                movida = true;
            }
        }
        if movida {
            vivo.olvida_imagen();
        }
        let escena = Escena { objetos: &objetos, luz, camara: Camara { ojo, ..base } };
        // ** el fotograma, repartido por filas entre los nucleos
        let n = obreros::cuantas();
        let b = Buferes { nuevas: nuevas.as_mut_ptr() as usize, hojas: hojas.as_mut_ptr() as usize, usadas: [const { AtomicUsize::new(0) }; obreros::MAX as usize] };
        {
            let (v, e, b) = (&vivo, &escena, &b);
            let parte = move |k: u32| {
                let (y0, y1) = filas(k, n);
                let (desde_px, cuantos) = ((y0 * W) as usize, ((y1 - y0) * W) as usize);
                // SAFETY: las filas y0..y1 son SOLO de la parte k (las de las
                // demas no se tocan), y los buferes viven todo el reparto.
                let mios = unsafe { core::slice::from_raw_parts_mut((b.nuevas as *mut V3).add(desde_px), cuantos) };
                let sus_hojas = unsafe { core::slice::from_raw_parts_mut((b.hojas as *mut Apunte).add(desde_px * APUNTES_POR_CAMINO), cuantos * APUNTES_POR_CAMINO) };
                let mut c = Cuaderno { hojas: sus_hojas, n: 0 };
                v.calcula_en(e, y0, y1, Parte::Cuarto, mios, &mut c);
                b.usadas[k as usize].store(c.n, Relaxed);
            };
            let mal = obreros::repartir(n, &parte);
            // lo que no salio, aqui
            for k in 1..n {
                if mal & 1 << k != 0 {
                    parte(k);
                }
            }
        }
        for k in 0..n {
            let (y0, y1) = filas(k, n);
            let (a, z) = ((y0 * W) as usize, (y1 * W) as usize);
            let h0 = a * APUNTES_POR_CAMINO;
            vivo.pon_filas(y0, y1, Parte::Cuarto, &nuevas[a..z], &hojas[h0..h0 + b.usadas[k as usize].load(Relaxed)]);
        }
        vivo.cierra();
        // ** a la ventana: la pelicula, al doble
        let img = vivo.imagen();
        for y in 0..H * ESCALA {
            for x in 0..W * ESCALA {
                let l = img[((y / ESCALA) * W + x / ESCALA) as usize];
                if x < win.w && y < win.h {
                    // SAFETY: (x, y) dentro de la ventana.
                    unsafe { *win.px.add((y * win.w + x) as usize) = 0xFF00_0000 | pelicula(l, 1.0) };
                }
            }
        }
        win.present();
        if vivo.fotograma % 32 == 0 {
            let ahora = ms();
            say("ESTUDIO: ");
            numero((ahora - desde) / 32);
            say(" ms por fotograma, en ");
            numero(n as u64);
            say(" partes; fotograma ");
            numero(vivo.fotograma as u64);
            say(if con_memoria { ", con memoria\n" } else { ", sin memoria\n" });
            desde = ahora;
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if obreros::en_obrero() {
        // un obrero no tiene puerta: la parte falla, y la rehace este nucleo
        // SAFETY: parar la parte.
        unsafe { core::arch::asm!("ud2", options(noreturn)) };
    }
    say("ESTUDIO: panico\n");
    if let Some(s) = info.message().as_str() {
        say(s);
        say("\n");
    }
    bmo::salir();
}
