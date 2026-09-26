//! **`gpu verrano [fotograma]`: VERRANO V0 y V1** -- el cubo del estudio D3D
//! por la API de dibujo de BMO-X, con sus DOS backends y el mismo fotograma:
//!
//! ```text
//!    el aparato  lo que haya detras de la puerta `destino` (hoy la RTX 3060
//!                12G: `sm86.rs`). Este fichero NO la nombra: habla VERRANO
//!    la CPU      `bmo_verrano::cpu::Cpu::LA_3060`: el juez con la regla 4
//! ```
//!
//! Se comparan pixel a pixel, y la huella del aparato contra D3D12 (0, 30,
//! 60) y contra lo medido bajo BMO-X (32). Lo que se ve en la ventana es lo
//! que escribio el aparato, leido de la pantalla.
//!
//! ** AISLADO (26-09, `docs/plan/PLAN_EL_AISLAMIENTO.md` A2): aqui no entra
//! ni el driver de la 3060, ni `kind::SM86`, ni su juez. Lo vigila `la-3060`
//! (regla S): otra GPU es otro `destino`, y este fichero no cambia.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea

use bmo_cubo::referencia as rf;
use bmo_userland as bmo;
use bmo_verrano::cpu::Cpu;
use bmo_verrano::{Backend, Frame, Image, Vertex, Viewport};

use super::super::After;
use super::sm86 as destino;
use super::tablero::Tablero;
use super::{numero, Texto, CLARO, FONDO, ROJO, TENUE, VERDE};
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Los vertices que caben en un fotograma de la escena (3 por triangulo).
const MAX_VERTICES: usize = 3 * bmo_cubo::tanda::CABEN;

/// `gpu verrano [fotograma] | banco [N] [opciones del aparato]`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, resto: &[u8]) -> After {
    // [!] `gpu.rs` pasa el resto CON su espacio delante (" sinldg"): sin
    // recortarlo, `sinldg` no se reconocia nunca y el 26-09 06:33 corrio el
    // programa normal con ese nombre. Se recorta aqui, una vez, para todos.
    let resto = resto.trim_ascii();
    let f = numero(resto).unwrap_or(30).min(359);
    let banco_pedido = resto.strip_prefix(b"banco");
    // Lo que no es de VERRANO (`ligero`, `sinldg`...) lo entiende el aparato.
    let op = destino::Opciones::de(resto);
    paint_status(p, &dsk.run_box, "VERRANO: el cubo por la API de BMO-X, en el aparato y en la CPU", INK_DIM);
    let (w, h) = (rf::ANCHO, rf::ALTO);
    if p.ancho < w || p.alto < h {
        return linea(dsk, b"  NO  la pantalla es mas chica que 1280x720", INK_ERR);
    }
    let n = (w * h) as usize;
    let (Some(bloque), Some(caja)) = (bmo::Memoria::request(2 * n as u64 * 4), bmo::Memoria::request(destino::CAJA)) else {
        return linea(dsk, b"  NO  sin memoria para dos fotogramas de 1280x720", INK_ERR);
    };
    // SAFETY: el bloque mide 2n palabras de 32 bits, alineado a pagina, es de
    // este proceso y solo se usa aqui; las dos mitades no se pisan. La caja,
    // `destino::CAJA` bytes del proceso, para lo que el aparato arme.
    let (gpu, cpu) = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut u32, 2 * n).split_at_mut(n) };
    // SAFETY: como arriba.
    let caja = unsafe { core::slice::from_raw_parts_mut(caja.base() as *mut u8, destino::CAJA as usize) };

    // El fotograma: la tanda del juez en vertices de VERRANO.
    let mut v = [Vertex::default(); MAX_VERTICES];
    let Some(k) = vertices(f, w, h, &mut v) else { return linea(dsk, b"  NO  la tanda no cabe", INK_ERR) };
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &v[..k], viewport: Viewport { width: w, height: h } };

    // La puerta: el sobre con el codigo de ESTE aparato, su juez, la tarjeta.
    let (mut aparato, abierto) = match destino::abrir(dsk, p, caja, op) {
        Ok(x) => x,
        Err(a) => return a,
    };

    // La pantalla ANTES del dibujo, como `gpu cubo 3060`.
    let (x0, y0) = destino::ventana(p);
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    let titulo = if banco_pedido.is_some() { "VERRANO V1: EL CUBO GIRANDO" } else { "VERRANO V0: EL CUBO POR LA API DE BMO-X" };
    p.texto_escala(40, 24, titulo, CLARO, 2);
    p.vaciar();

    // ** `gpu verrano banco [N]` (V1): N fotogramas seguidos, sin leer.
    if let Some(r) = banco_pedido {
        let n = r.split(|&c| c == b' ').find_map(numero).unwrap_or(360).clamp(1, 3600);
        aparato.leer = false;
        return banco(dsk, p, aparato, op, gpu, n, y0 + h);
    }
    let s_aparato = aparato.draw(&frame, &mut Image { pixels: gpu, width: w, height: h });
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let mut juez = Cpu::LA_3060;
    let scpu = juez.draw(&frame, &mut Image { pixels: cpu, width: w, height: h });
    let cpu_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    let st = match (s_aparato, scpu) {
        (Ok(a), Ok(_)) => a,
        (Err(e), _) => return destino::fallo(dsk, e, None, op),
        (_, Err(_)) => return linea(dsk, b"  NO  la CPU no dibujo el fotograma (el juez no sabe hacerlo)", INK_ERR),
    };

    if op.sin_ldg {
        return destino::sin_ldg_pagado(dsk);
    }

    // Las dos imagenes, pixel a pixel; lo que el juez no explica, aparte.
    let distinto = |i: usize| gpu[i] & 0x00FF_FFFF != cpu[i] & 0x00FF_FFFF;
    let sabido = |i: usize| rf::SIN_EXPLICAR.iter().any(|s| s.0 == f && (s.2 * w + s.1) as usize == i);
    let malos = (0..n).filter(|&i| distinto(i) && !sabido(i)).count();
    let sin_explicar = (0..n).filter(|&i| distinto(i) && sabido(i)).count();
    let primero = (0..n).find(|&i| distinto(i) && !sabido(i));
    let huella = rf::huella(gpu);
    let d3d = rf::de_la_3060(f) == Some(huella);
    let medida = rf::HUELLAS_BMO_X.iter().any(|&(g, hh)| g == f && hh == huella);

    p.marcar(x0, y0, w, h);
    for y in 0..h {
        for x in 0..w {
            p.punto_ya_marcado(x0 + x, y0 + y, gpu[(y * w + x) as usize] & 0x00FF_FFFF);
        }
    }
    let mut a = Texto::nuevo();
    a.t(b"fotograma ").d(f as u64).t(b": el aparato en ").d(st.device_us as u64).t(b" us, la CPU en ").d(cpu_us).t(b" us; huella ").x(huella);
    let mut b = Texto::nuevo();
    let (veredicto, color): (&[u8], u32) = if malos > 0 {
        b.t(b"DISTINTO: ").d(malos as u64).t(b" pixeles entre el aparato y la CPU");
        (b.s(), ROJO)
    } else if d3d {
        (b"IGUAL: VERRANO en el aparato = VERRANO en la CPU = D3D12 en la 3060 bajo Windows", VERDE)
    } else if medida {
        (b"IGUAL: VERRANO en el aparato = lo que la 3060 dibujo en X5, y = la CPU", VERDE)
    } else {
        (b"IGUAL, pixel a pixel: VERRANO en el aparato = VERRANO en la CPU", VERDE)
    };
    let mut c = Texto::nuevo();
    if let Some(i) = primero {
        c.t(b"el primero en (").d((i as u32 % w) as u64).t(b", ").d((i as u32 / w) as u64).t(b"): el aparato ").x(gpu[i] as u64 & 0xFF_FFFF).t(b", la CPU ").x(cpu[i] as u64 & 0xFF_FFFF);
    } else if sin_explicar > 0 {
        c.t(b"y ").d(sin_explicar as u64).t(b" pixel sin explicar (del silicio, como con X5 y en Windows)");
    }
    let mut d = Texto::nuevo();
    d.t(destino::NOMBRE).t(b": programas del BSF ").d(abierto.bytes_vs as u64).t(b" + ").d(abierto.bytes_ps as u64).t(b" B; ").d(st.triangles as u64).t(b" triangulos en UN dibujo; juez: PERFECTO Y PRECISO (").d(abierto.instrucciones as u64).t(b")");
    let yt = p.alto.saturating_sub(144);
    p.texto_bytes(40, yt, a.s(), CLARO);
    p.texto_bytes(40, yt + 24, veredicto, color);
    if !c.s().is_empty() {
        p.texto_bytes(40, yt + 48, c.s(), TENUE);
    }
    p.texto_bytes(40, yt + 72, d.s(), TENUE);
    p.texto_bytes(40, p.alto.saturating_sub(40), b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    super::super::gspcomputo::abrir_panel();

    let g = &mut dsk.out.grid;
    g.with_ink(if malos == 0 { INK_GOOD } else { INK_ERR });
    g.text(b"  verrano: ");
    g.text(a.s());
    g.text(b"\n           ");
    g.text(veredicto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    for extra in [c.s(), d.s()] {
        if !extra.is_empty() {
            g.text(b"           ");
            g.text(extra);
            g.byte(b'\n');
        }
    }
    g.text(b"           leido de la pantalla en ");
    g.dec(aparato.leer_ms);
    g.text(b" ms\n");
    super::super::datos::anotar(b"gpu verrano us", st.device_us as u64, b"");
    dsk.field.n = 0;
    After::Settle
}

/// Los vertices de VERRANO del fotograma `f` (la tanda del juez).
fn vertices(f: u32, w: u32, h: u32, v: &mut [Vertex; MAX_VERTICES]) -> Option<usize> {
    let t = bmo_cubo::tanda::de_fotograma(f, w, h)?;
    let mut k = 0;
    for tri in t.tris() {
        for &pos in tri.clip.iter() {
            v[k] = Vertex { position: pos, color: tri.color };
            k += 1;
        }
    }
    Some(k)
}

/// ** V1 -- EL CUBO EN MOVIMIENTO, CON FPS (`gpu verrano banco [N]`).
///
/// N fotogramas seguidos por VERRANO (el cubo gira: fotograma `i` = angulo
/// `i` de 360), cada uno dibujado por el aparato directo en su ventana de la
/// pantalla -- se VE girar --, SIN leerlo de vuelta: leer 1280x720 por la
/// puerta cuesta ~2 s y es justo lo que el banco no mide. Al final, UNO se
/// lee y se juzga (el 30, contra la huella de D3D12): unos fps que dibujan
/// otra cosa no valen nada.
///
/// Lo que se mide, dicho: `pared` es todo (preparar el paquete, la puerta,
/// subir los vertices y esperar el semaforo); `aparato` es solo lo
/// que la tarjeta tardo en dibujar. La tabla de `estudio-d3d` (D3D12 ~3.800
/// fps en Windows) mide un bucle de presentacion, no esto: se ponen al lado,
/// no se igualan.
///
/// ** Y se ve MIENTRAS (26-09): el tablero (`tablero.rs`) debajo del cubo,
/// repintado cada 66 ms, con lo que cuesta el propio tablero descontado de
/// los fps y dicho aparte. `preparar` es lo que el kernel tarda en dejar el
/// fotograma listo en la VRAM: en frio relee ~4.000 palabras por PCIe, en
/// caliente (del segundo fotograma en adelante) solo escribe los vertices.
/// Las opciones del aparato (en la 3060, `ligero`: sin la escalera de T1c;
/// `anillo`: el fotograma EN VUELO) las entiende el aparato, no este
/// fichero.
///
/// ** V1b, "la CPU no espera: ORQUESTA" (26-09). Dos cosas de aqui:
///
/// ```text
///    las tandas   los vertices de los 360 angulos se cuentan ANTES, una
///                 vez: en el escritorio la coma flotante es por software
///                 (decenas de us por fotograma), y un juego trae su
///                 animacion hecha. El banco mide ORQUESTAR y dibujar, no
///                 la trigonometria; lo que costo contarlas se dice aparte
///    el cierre    con un aparato que deja fotogramas EN VUELO, lo que
///                 quede se espera (`Backend::finish`) DENTRO del reloj:
///                 unos fps que no esperan al ultimo fotograma no valen
/// ```
fn banco(dsk: &mut Desktop, p: &bmo::Pantalla, mut aparato: destino::Aparato, op: destino::Opciones, gpu: &mut [u32], n: u32, fin_y: u32) -> After {
    let (w, h) = (rf::ANCHO, rf::ALTO);
    let modo = op.modo();
    let mut tablero = Tablero::nuevo(p, fin_y, n, destino::ETIQUETA, modo, b"PERFECTO Y PRECISO");
    // Sin banda (pantalla de 1280x720), las mismas cuentas sin pintarlas.
    let mut cuentas = Cuentas::default();
    let mut v = [Vertex::default(); MAX_VERTICES];
    // Las tandas, antes (ver arriba).
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let angulos = n.min(360) as usize;
    let Some(bloque) = bmo::Memoria::request((angulos * MAX_VERTICES * core::mem::size_of::<Vertex>()) as u64) else {
        return linea(dsk, b"  NO  sin memoria para las tandas del banco", INK_ERR);
    };
    // SAFETY: el bloque mide `angulos * MAX_VERTICES` vertices (`repr(C)`,
    // de f32: cualquier bit vale), alineado a pagina, es de este proceso y
    // solo se usa aqui; vive hasta el final de esta funcion.
    let tandas = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut Vertex, angulos * MAX_VERTICES) };
    let mut cuantos = [0u8; 360];
    let desde = bmo::ciclos();
    for (f, (t, c)) in tandas.chunks_exact_mut(MAX_VERTICES).zip(cuantos.iter_mut()).enumerate() {
        let Some(k) = vertices(f as u32, w, h, &mut v) else { return linea(dsk, b"  NO  la tanda no cabe", INK_ERR) };
        t.copy_from_slice(&v);
        *c = k as u8;
    }
    let tandas_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    // `exige`: la tarjeta al maximo ANTES del reloj del banco.
    let mut exigido = Texto::nuevo();
    if op.exige {
        aparato.exigir(&mut exigido);
    }
    for i in 0..n {
        let f = (i % 360) as usize;
        let k = cuantos[f] as usize;
        let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &tandas[f * MAX_VERTICES..][..k], viewport: Viewport { width: w, height: h } };
        let desde = bmo::ciclos();
        match aparato.draw(&frame, &mut Image { pixels: gpu, width: w, height: h }) {
            Ok(st) => {
                let ciclos = bmo::ciclos() - desde;
                cuentas.apuntar(ciclos, &st);
                if let Some(t) = tablero.as_mut() {
                    t.apuntar(ciclos, &st);
                    t.quizas(p);
                }
            }
            Err(e) => return destino::fallo(dsk, e, Some(i), op),
        }
    }
    // El cierre: lo que quedo en vuelo, dentro del reloj.
    let desde = bmo::ciclos();
    let cierre_us = match aparato.finish() {
        Ok(us) => us,
        Err(e) => return destino::fallo(dsk, e, Some(n), op),
    };
    let cierre = bmo::ciclos() - desde;
    cuentas.ciclos += cierre;
    if let Some(t) = tablero.as_mut() {
        t.cierre(cierre);
    }
    // El juicio: el 30 otra vez, por el MISMO camino (con lo que el aparato
    // reuse y en el mismo modo), leido y comparado con D3D12.
    aparato.leer = true;
    let k = vertices(30, w, h, &mut v).unwrap_or(0);
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &v[..k], viewport: Viewport { width: w, height: h } };
    let igual = aparato.draw(&frame, &mut Image { pixels: gpu, width: w, height: h }).is_ok() && rf::de_la_3060(30) == Some(rf::huella(gpu));
    let veredicto: &[u8] = if igual {
        b"el fotograma 30, leido al final: IGUAL a D3D12 en la 3060 bajo Windows"
    } else {
        b"PERO el fotograma 30, leido al final, NO es el de D3D12: esos fps no valen"
    };

    let (mut a, mut b) = (Texto::nuevo(), Texto::nuevo());
    let (fps, tarjeta) = match tablero.as_mut() {
        Some(t) => {
            t.final_(p, veredicto, igual);
            t.resumen(&mut a, &mut b);
            (t.fps(), t.tarjeta_media())
        }
        None => {
            cuentas.resumen(&mut a, &mut b, modo);
            let yt = p.alto.saturating_sub(120);
            p.texto_bytes(40, yt, a.s(), CLARO);
            p.texto_bytes(40, yt + 24, b.s(), TENUE);
            p.texto_bytes(40, yt + 48, veredicto, if igual { VERDE } else { ROJO });
            p.texto_bytes(40, p.alto.saturating_sub(40), b"pulsa cualquier tecla para volver al escritorio", TENUE);
            p.vaciar();
            (cuentas.fps(), cuentas.tarjeta / cuentas.muestras.max(1) as u64)
        }
    };
    super::super::gspcomputo::abrir_panel();
    let g = &mut dsk.out.grid;
    g.with_ink(if igual { INK_GOOD } else { INK_ERR });
    g.text(b"  verrano ");
    g.text(a.s());
    g.text(b"\n           ");
    g.text(b.s());
    g.text(b"\n           ");
    g.text(veredicto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    let mut c = Texto::nuevo();
    c.t(b"tandas de vertices contadas ANTES: ").d(angulos as u64).t(b" angulos en ").d(tandas_us).t(b" us (fuera del reloj); el cierre, ").d(cierre_us as u64).t(b" us esperando lo que quedo en vuelo (dentro)");
    g.text(b"           ");
    g.text(c.s());
    g.byte(b'\n');
    if !exigido.s().is_empty() {
        g.text(b"           ");
        g.text(exigido.s());
        g.byte(b'\n');
    }
    let mut gobierno = Texto::nuevo();
    aparato.cerrar(&mut gobierno);
    if !gobierno.s().is_empty() {
        g.text(b"           ");
        g.text(gobierno.s());
        g.byte(b'\n');
    }
    let mut nota = Texto::nuevo();
    aparato.nota(&mut nota);
    if !nota.s().is_empty() {
        g.text(b"           ");
        g.text(nota.s());
        g.byte(b'\n');
    }
    let (kf, ku): (&[u8], &[u8]) = if op.exige && op.coopera {
        (b"gpu verrano banco maximo fps", b"gpu verrano banco maximo us")
    } else if op.coopera {
        (b"gpu verrano banco coopera fps", b"gpu verrano banco coopera us")
    } else if op.anillo {
        (b"gpu verrano banco anillo fps", b"gpu verrano banco anillo us")
    } else if op.ligero {
        (b"gpu verrano banco ligero fps", b"gpu verrano banco ligero us")
    } else {
        (b"gpu verrano banco fps", b"gpu verrano banco us")
    };
    super::super::datos::anotar(kf, fps, b"fps");
    super::super::datos::anotar(ku, tarjeta, b"us");
    dsk.field.n = 0;
    After::Settle
}

/// Las cuentas del banco cuando no hay banda para el tablero.
#[derive(Default)]
struct Cuentas {
    n: u32,
    ciclos: u64,
    tarjeta: u64,
    /// Las muestras del aparato (en vuelo, no cada fotograma trae una).
    muestras: u32,
    peor: u32,
}

impl Cuentas {
    fn apuntar(&mut self, ciclos: u64, st: &bmo_verrano::Stats) {
        self.n += 1;
        self.ciclos += ciclos;
        if st.device_us > 0 || !st.in_flight {
            self.muestras += 1;
            self.tarjeta += st.device_us as u64;
            self.peor = self.peor.max(st.device_us);
        }
    }

    fn fps(&self) -> u64 {
        self.n as u64 * bmo::info(bmo::INFO_TSC_HZ).max(1000) / self.ciclos.max(1)
    }

    fn resumen(&self, a: &mut Texto, b: &mut Texto, modo: &[u8]) {
        a.t(b"banco: ").d(self.n as u64).t(b" fotogramas = ").d(self.fps()).t(b" fps de pared (").t(modo).t(b")");
        b.t(b"el aparato ").d(self.tarjeta / self.muestras.max(1) as u64).t(b" us de media, el peor ").d(self.peor as u64).t(b" us");
    }
}

fn linea(dsk: &mut Desktop, texto: &[u8], tinta: u8) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(tinta);
    g.text(texto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}
