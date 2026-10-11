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
//!    c     completar si / no (VE5)            Esc o q   cierra
//! ```
//!
//! **COMPLETAR** (VE5): lo que se ve no es la imagen cruda sino la
//! COMPLETADA -- el filtro que quita el grano guiado por la superficie de
//! cada pixel (`Vivo::completa`, el juez de lo que hara la 3060) --, con sus
//! pasadas tambien repartidas por filas: desde el primer fotograma despues
//! de mover la camara se ve limpia.
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
use bmo_verrano_trama::vivo::{Apunte, Cuaderno, Guia, Memoria, Parte, Vivo, APUNTES_POR_CAMINO};
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

/// Las pasadas de COMPLETAR (a 320x240, tres: `completa_prueba`).
const PASADAS: u32 = 3;

/// `parte(k)` en todos los nucleos; lo que un obrero no acabe, aqui.
fn en_todos(n: u32, parte: &(dyn Fn(u32) + Sync)) {
    let mal = obreros::repartir(n, parte);
    for k in 1..n {
        if mal & 1 << k != 0 {
            parte(k);
        }
    }
}

/// Las filas `y0..y1` de un bufer de toda la imagen, para UNA parte.
///
/// SAFETY: quien llama da a cada parte filas que no pisa ninguna otra, y el
/// bufer (de `T`, `W * H` de largo) vive todo el reparto.
unsafe fn sus_filas<'a, T>(base: usize, y0: u32, y1: u32) -> &'a mut [T] {
    unsafe { core::slice::from_raw_parts_mut((base as *mut T).add((y0 * W) as usize), ((y1 - y0) * W) as usize) }
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
    let mut completar = true;
    let mut vivo = Vivo::nuevo(W, H, Some(memoria()));
    let mut nuevas: Vec<V3> = vec![[0.0; 3]; (W * H) as usize];
    let mut hojas: Vec<Apunte> = vec![Apunte { p: [0.0; 3], n: [0.0; 3], l: [0.0; 3] }; (W * H) as usize * APUNTES_POR_CAMINO];
    // los de COMPLETAR: la guia nueva y la luz, de una pasada a la otra
    let mut guia: Vec<Guia> = vec![Guia::default(); (W * H) as usize];
    let (mut luz_a, mut luz_b): (Vec<V3>, Vec<V3>) = (vec![[0.0; 3]; (W * H) as usize], vec![[0.0; 3]; (W * H) as usize]);
    say("ESTUDIO: VERRANO ESTUDIO VIVO -- la luz estilo RenderMan que se recuerda (VE2)\n");
    let mut desde = ms();
    let mut movida = true;
    loop {
        // la entrada: mover la camara olvida la imagen, NO la memoria
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
                    b'c' => {
                        completar = !completar;
                        say(if completar { "ESTUDIO: COMPLETADA\n" } else { "ESTUDIO: cruda\n" });
                        continue;
                    }
                    _ => continue,
                }
                movida = true;
            }
        }
        let escena = Escena { objetos: &objetos, luz, camara: Camara { ojo, ..base } };
        let n = obreros::cuantas();
        if movida {
            vivo.olvida_imagen();
            // la guia de la camara nueva (un rayo por pixel, sin luz), por filas
            let (v, e, g) = (&vivo, &escena, guia.as_mut_ptr() as usize);
            en_todos(n, &move |k: u32| {
                let (y0, y1) = filas(k, n);
                // SAFETY: las filas de la parte k son solo suyas.
                v.guia_en(e, y0, y1, unsafe { sus_filas(g, y0, y1) });
            });
            core::mem::swap(&mut vivo.guia, &mut guia);
            movida = false;
        }
        // ** el fotograma, repartido por filas entre los nucleos
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
            en_todos(n, &parte);
        }
        for k in 0..n {
            let (y0, y1) = filas(k, n);
            let (a, z) = ((y0 * W) as usize, (y1 * W) as usize);
            let h0 = a * APUNTES_POR_CAMINO;
            vivo.pon_filas(y0, y1, Parte::Cuarto, &nuevas[a..z], &hojas[h0..h0 + b.usadas[k as usize].load(Relaxed)]);
        }
        vivo.cierra();
        // ** COMPLETAR, cada pasada por filas en todos los nucleos (entre
        // una y otra se espera a todos: la siguiente lee las filas vecinas)
        let pasadas = if completar { PASADAS } else { 0 };
        {
            let v = &vivo;
            let a = luz_a.as_mut_ptr() as usize;
            en_todos(n, &move |k: u32| {
                let (y0, y1) = filas(k, n);
                // SAFETY: las filas de la parte k son solo suyas.
                v.separa(y0, y1, unsafe { sus_filas(a, y0, y1) });
            });
        }
        let mut tolerancia = vivo.tolerancia();
        for paso in 0..pasadas {
            let (v, lee, b) = (&vivo, &luz_a[..], luz_b.as_mut_ptr() as usize);
            en_todos(n, &move |k: u32| {
                let (y0, y1) = filas(k, n);
                // SAFETY: las filas de la parte k son solo suyas (y `lee` es
                // el OTRO bufer).
                v.alisa(paso, tolerancia, lee, y0, y1, unsafe { sus_filas(b, y0, y1) });
            });
            core::mem::swap(&mut luz_a, &mut luz_b);
            tolerancia *= 0.5;
        }
        // ** a la ventana: otra vez por el color de cada material, y la
        // pelicula, al doble
        {
            let (v, a, px, ww, wh) = (&vivo, luz_a.as_mut_ptr() as usize, win.px as usize, win.w, win.h);
            en_todos(n, &move |k: u32| {
                let (y0, y1) = filas(k, n);
                // SAFETY: las filas de la parte k son solo suyas, en la luz y
                // en la ventana (las filas y0..y1, al doble).
                let mia: &mut [V3] = unsafe { sus_filas(a, y0, y1) };
                v.vuelve(y0, y1, mia);
                for y in y0 * ESCALA..(y1 * ESCALA).min(wh) {
                    for x in 0..(W * ESCALA).min(ww) {
                        let l = mia[((y / ESCALA - y0) * W + x / ESCALA) as usize];
                        // SAFETY: (x, y) dentro de la ventana, en filas de k.
                        unsafe { *(px as *mut u32).add((y * ww + x) as usize) = 0xFF00_0000 | pelicula(l, 1.0) };
                    }
                }
            });
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
            say(if con_memoria { ", con memoria" } else { ", sin memoria" });
            say(if completar { ", completada\n" } else { ", cruda\n" });
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
