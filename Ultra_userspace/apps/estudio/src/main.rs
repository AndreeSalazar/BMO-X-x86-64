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
//!    c     completar si / no (VE5)            g     en la 3060 si / no (VEL2)
//!    Esc o q   cierra
//! ```
//!
//! **EN LA 3060** (VEL2): si el escritorio preparo la 3060 (`gpu lienzo`),
//! las pasadas de COMPLETAR las hace ELLA, por la puerta estrecha de las apps
//! (`IOMMU_OP_GPU_COMPLETAR`: el programa es el del kernel, el bloque es
//! nuestro). Cada fotograma se juzgan 32 pixeles de la ultima pasada con la
//! cuenta de VERRANO; si la puerta dice que no o un pixel no es el del juez,
//! lo dice y la CPU sigue.
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

use bmo_gpu_ga10x::completa as co;
use bmo_userland as bmo;
use bmo_verrano::completa as vc;
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

/// **COMPLETAR EN LA 3060** (VEL2): la luz separada `luz` (y la guia, si
/// `guia_lista` es falso) al bloque, UNA llamada con las [`PASADAS`], el juez
/// en 32 pixeles de la ultima, y lo que salio, en `luz`. `Err(por que)`: la
/// CPU sigue.
fn completar_en_la_3060(base: usize, vivo: &Vivo, luz: &mut [V3], guia_lista: &mut bool, t: f32) -> Result<(), &'static str> {
    let n = (W * H) as usize;
    // SAFETY: el bloque es nuestro (pedido al arrancar y olvidado: vive lo
    // que el proceso) y mide `co::CABECERA` + el de VERRANO; la 3060 lo ve
    // solo DENTRO de la llamada.
    let (cabecera, b) = unsafe { (&mut *(base as *mut [u32; 5]), core::slice::from_raw_parts_mut((base + co::CABECERA as usize) as *mut f32, 16 * n)) };
    *cabecera = [co::MAGIA, W, H, PASADAS, t.to_bits()];
    if !*guia_lista {
        for (i, g) in vivo.guia.iter().enumerate() {
            vc::pon_guia(b, n, i, g);
        }
        *guia_lista = true;
    }
    for (i, l) in luz.iter().enumerate() {
        vc::pon_luz(b, n, false, i, *l);
    }
    let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_COMPLETAR, base as u64).map_err(|_| "la puerta dijo que no (sin `gpu lienzo` en el escritorio?)")?;
    let (hechas, ..) = co::desempaquetar(r);
    if hechas != PASADAS || !co::sano(r) {
        return Err("una pasada no se pago");
    }
    // ** el juez: la ultima pasada, desde lo que leyo (la otra luz sigue ahi)
    let (paso, de_b, tk) = co::pasada(PASADAS - 1, t);
    for k in 0..32u32 {
        let (x, y) = ((k * 37 + 5) % W, (k * 53 + 3) % H);
        let cpu = vc::del_bloque(b, W, H, paso, de_b, tk, x, y);
        let gpu = vc::luz_del_bloque(b, n, !de_b, (y * W + x) as usize);
        if !co::igual(gpu, cpu) {
            return Err("un pixel de la 3060 no es el del juez");
        }
    }
    let en_b = co::acaba_en_b(PASADAS);
    for (i, l) in luz.iter_mut().enumerate() {
        *l = vc::luz_del_bloque(b, n, en_b, i);
    }
    Ok(())
}

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

/// **La entrada del proceso.** El kernel entra con la pila ALINEADA a 16 (lo
/// que pide el ABI de SysV al arrancar un proceso: `USER_STACK_TOP`), y una
/// funcion de Rust la espera como queda despues de un `call` (8 mas). Con la
/// coma flotante por SOFTWARE daba igual; con la de hardware (el objetivo
/// `x86_64-bmo-ring3`), un `movaps` a la pila con ella mal alineada es un #GP.
/// Aqui se alinea y se LLAMA: lo que hace el `_start` de cualquier sistema.
#[unsafe(naked)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!("and rsp, -16", "call {inicio}", "ud2", inicio = sym inicio)
}

extern "C" fn inicio() -> ! {
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
    // ** LA 3060 (VEL2): su bloque, de una vez -- la cabecera y el de VERRANO
    let la_3060 = bmo::Memoria::request(co::CABECERA + co::Formato { ancho: W, alto: H }.bytes()).map(|b| {
        let base = b.base() as usize;
        core::mem::forget(b);
        base
    });
    let mut usar_3060 = la_3060.is_some();
    let mut guia_en_3060 = false;
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
                    b'g' => {
                        usar_3060 = !usar_3060 && la_3060.is_some();
                        say(if usar_3060 { "ESTUDIO: completa la 3060\n" } else { "ESTUDIO: completa la CPU\n" });
                        continue;
                    }
                    _ => continue,
                }
                movida = true;
            }
        }
        let escena = Escena { objetos: &objetos, luz, camara: Camara { ojo, ..base }, mallas: &[], niebla: None };
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
            guia_en_3060 = false;
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
        let mut tolerancia = vivo.tolerancia();
        {
            let v = &vivo;
            let a = luz_a.as_mut_ptr() as usize;
            en_todos(n, &move |k: u32| {
                let (y0, y1) = filas(k, n);
                // SAFETY: las filas de la parte k son solo suyas.
                v.separa(y0, y1, unsafe { sus_filas(a, y0, y1) });
            });
        }
        // ** en la 3060, si se puede (VEL2); si no, en la CPU
        let mut en_3060 = false;
        if let (true, true, Some(base)) = (completar, usar_3060, la_3060) {
            match completar_en_la_3060(base, &vivo, &mut luz_a, &mut guia_en_3060, tolerancia) {
                Ok(()) => en_3060 = true,
                Err(porque) => {
                    usar_3060 = false;
                    say("ESTUDIO: la 3060 no completa: ");
                    say(porque);
                    say("; completa la CPU (`g` para volver a probar)\n");
                }
            }
        }
        let pasadas = if completar && !en_3060 { PASADAS } else { 0 };
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
            say(if !completar { ", cruda\n" } else if usar_3060 { ", completada en la 3060\n" } else { ", completada en la CPU\n" });
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
