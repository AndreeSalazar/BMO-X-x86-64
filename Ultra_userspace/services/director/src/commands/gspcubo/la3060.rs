//! **`gpu cubo 3060 [fotograma]`: EL CUBO POR LA 3060, SIN WINDOWS (X5)** --
//! el mismo fotograma que `gpu cubo` dibuja por CPU, dibujado ahora por el
//! pipeline 3D de la 3060 en una ventana de 1280x720 de la pantalla, leido de
//! vuelta y medido tres veces:
//!
//! ```text
//!    su huella        contra la captura de D3D12 en la 3060 (fotogramas 0, 30, 60)
//!    cada pixel       contra el MODELO de la 3060 (`referencia::como_la_3060`:
//!                     el juez con la regla 4 del silicio y el pixel sin
//!                     explicar), en cualquier fotograma de 0 a 359
//!    el primero malo  donde, lo que dio la 3060 y lo que dice el modelo
//!    el juez exacto   cuantos pixeles cambia la regla 4 (lo del silicio)
//! ```
//!
//! Lo que se ve en la ventana es lo que LEYO de la pantalla: los pixeles que
//! escribio la 3060, tal cual.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: un dibujo de la
//!                     3060, 460.800 lecturas de dos pixeles y el juez (~25 ms)

use bmo_cubo::referencia as rf;
use bmo_gpu_ga10x::cubo as cu;
use bmo_userland as bmo;

use super::{numero, Texto, CLARO, FONDO, ROJO, TENUE, VERDE};
use super::super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// `gpu cubo 3060 [fotograma]`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, resto: &[u8]) -> After {
    let f = numero(resto).unwrap_or(30).min(359);
    paint_status(p, &dsk.run_box, "el cubo del estudio D3D, dibujado por la 3060 sin Windows", INK_DIM);
    let (w, h) = (cu::ANCHO, cu::ALTO);
    if p.ancho < w || p.alto < h {
        return linea(dsk, b"  NO  la pantalla es mas chica que 1280x720: el cubo no cabe", INK_ERR);
    }
    let n = (w * h) as usize;
    let Some(bloque) = bmo::Memoria::request(2 * n as u64 * 4) else {
        return linea(dsk, b"  NO  sin memoria para dos fotogramas de 1280x720", INK_ERR);
    };
    // SAFETY: el bloque mide 2n palabras de 32 bits, alineado a pagina, es de
    // este proceso y solo se usa aqui; las dos mitades no se pisan.
    let (gpu, juez) = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut u32, 2 * n).split_at_mut(n) };

    // La pantalla ANTES del dibujo: lo que se pinte despues por la CPU no
    // puede caer encima del cubo entre el dibujo y la lectura.
    let (x0, y0) = (((p.ancho - w) / 2) & !31, (p.alto - h) / 2);
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    p.texto_escala(40, 24, "EL CUBO DEL ESTUDIO D3D, POR LA RTX 3060, SIN WINDOWS", CLARO, 2);
    p.vaciar();

    let r = super::super::gspcomputo::ficha_del_gr().and_then(|ficha| bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, ficha | (f as u64) << 32));
    let v = match r {
        Ok(v) if cu::sano(v) => v,
        otro => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            match otro {
                Err(m) => {
                    g.text(b"  NO  la 3060 no dibujo el cubo: motivo ");
                    g.dec(m as u64);
                    g.text(b" (`gpu` lo explica)\n");
                }
                Ok(v) => {
                    let (us, tris, etapas, lanzado) = cu::desempaquetar(v);
                    g.text(if lanzado { b"  NO  el cubo no se pago entero: " as &[u8] } else { b"  NO  no se lanzo: " });
                    g.dec(tris as u64);
                    g.text(b" triangulos, escalera ");
                    g.dec(etapas as u64);
                    g.text(b", ");
                    g.dec(us as u64);
                    g.text(b" us\n");
                    g.with_ink(INK_PLAIN);
                    super::super::gspcomputo::escalera(g);
                }
            }
            g.with_ink(INK_PLAIN);
            dsk.field.n = 0;
            return After::Settle;
        }
    };
    let (gpu_us, tris, _, _) = cu::desempaquetar(v);

    // Lo que escribio la 3060, de vuelta, de dos en dos.
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let mut leidos = 0;
    for k in 0..n / 2 {
        match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_LEER | k as u64) {
            Ok(d) => {
                gpu[2 * k] = d as u32;
                gpu[2 * k + 1] = (d >> 32) as u32;
                leidos += 2;
            }
            Err(_) => break,
        }
    }
    let leer_ms = (bmo::ciclos() - desde) * 1000 / hz;
    let distinto = |a: &u32, b: &u32| *a & 0x00FF_FFFF != *b & 0x00FF_FFFF;
    // Dos jueces: el de D3D10 con el redondeo exacto, y el MODELO de la 3060
    // (la regla 4 del silicio y el pixel sin explicar: visto el 25-09 17:18).
    bmo_cubo::dibujar_por_cpu(bmo_cubo::angulo_de_fotograma(f), w, h, juez);
    let exactos = gpu.iter().zip(juez.iter()).filter(|(a, b)| distinto(a, b)).count();
    rf::como_la_3060(f, juez);
    let huella = rf::huella(gpu);
    let d3d = rf::de_la_3060(f);
    let malos = gpu.iter().zip(juez.iter()).filter(|(a, b)| distinto(a, b)).count();
    let primero = gpu.iter().zip(juez.iter()).position(|(a, b)| distinto(a, b));

    // A la pantalla: lo leido, donde lo dibujo la 3060, y el veredicto.
    p.marcar(x0, y0, w, h);
    for y in 0..h {
        for x in 0..w {
            p.punto_ya_marcado(x0 + x, y0 + y, gpu[(y * w + x) as usize] & 0x00FF_FFFF);
        }
    }
    let mut t = Texto::nuevo();
    t.t(b"fotograma ").d(f as u64).t(b": la 3060 en ").d(gpu_us as u64).t(b" us (").d(tris as u64).t(b" triangulos); huella ").x(huella);
    let mut u = Texto::nuevo();
    let (veredicto, color): (&[u8], u32) = if leidos != n {
        u.t(b"la lectura de vuelta se corto en el pixel ").d(leidos as u64);
        (u.s(), ROJO)
    } else if d3d == Some(huella) {
        (b"IGUAL, bit a bit, a lo que D3D12 dibujo en la RTX 3060 bajo Windows: ahora SIN Windows", VERDE)
    } else if malos == 0 {
        u.t(b"IGUAL, pixel a pixel, al modelo de la 3060 (el juez con la regla 4 del silicio)");
        (u.s(), VERDE)
    } else {
        u.t(b"DISTINTO: ").d(malos as u64).t(b" pixeles no son los del modelo de la 3060");
        (u.s(), ROJO)
    };
    // Debajo: el primer pixel malo; o, si no hay, lo que el juez EXACTO no
    // explica (la regla 4 y los pixeles sin explicar), si algo.
    let mut q = Texto::nuevo();
    if let Some(i) = primero {
        let (x, y) = (i as u32 % w, i as u32 / w);
        q.t(b"el primero en (").d(x as u64).t(b", ").d(y as u64).t(b"): la 3060 ").x(gpu[i] as u64 & 0xFF_FFFF).t(b", el modelo ").x(juez[i] as u64 & 0xFF_FFFF);
    } else if exactos > 0 {
        q.t(b"el juez exacto difiere en ").d(exactos as u64).t(b": la regla 4 y lo sin explicar, del SILICIO");
    }
    let yt = p.alto.saturating_sub(120);
    p.texto_bytes(40, yt, t.s(), CLARO);
    p.texto_bytes(40, yt + 24, veredicto, color);
    if !q.s().is_empty() {
        p.texto_bytes(40, yt + 48, q.s(), TENUE);
    }
    p.texto_bytes(40, p.alto.saturating_sub(40), b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    super::super::gspcomputo::abrir_panel();

    let igual = leidos == n && malos == 0;
    let g = &mut dsk.out.grid;
    g.with_ink(if igual { INK_GOOD } else { INK_ERR });
    g.text(b"  cubo 3060: ");
    g.text(t.s());
    g.text(b"\n              ");
    g.text(veredicto);
    g.byte(b'\n');
    if !q.s().is_empty() {
        g.text(b"              ");
        g.text(q.s());
        g.byte(b'\n');
    }
    g.with_ink(INK_PLAIN);
    g.text(b"              leido de la pantalla en ");
    g.dec(leer_ms);
    g.text(b" ms\n");
    super::super::datos::anotar(b"gpu cubo 3060 us", gpu_us as u64, b"");
    dsk.field.n = 0;
    After::Settle
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
