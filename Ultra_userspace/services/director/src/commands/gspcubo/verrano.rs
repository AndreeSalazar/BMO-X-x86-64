//! **`gpu verrano [fotograma]`: VERRANO V0** -- el cubo del estudio D3D por
//! la API de dibujo de BMO-X, con sus DOS backends y el mismo fotograma:
//!
//! ```text
//!    la 3060   los dos programas FIJOS, tomados del BSF (`kind` SM86, sus
//!              hashes comprobados al tomarlos), y los vertices en un buffer:
//!              la 3060 no compila nada; el kernel sube lo que le dan
//!    la CPU    `bmo_verrano::cpu::Cpu::LA_3060`: el juez con la regla 4
//! ```
//!
//! Se comparan pixel a pixel, y la huella de la 3060 contra D3D12 (0, 30,
//! 60) y contra lo medido bajo BMO-X (32). Lo que se ve en la ventana es lo
//! que escribio la 3060, leido de la pantalla.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea

use bmo_bsf::{abi, kind, Bsf};
use bmo_cubo::referencia as rf;
use bmo_gpu_ga10x::{cubo as cu, tuberia as tu};
use bmo_userland as bmo;
use bmo_verrano::cpu::Cpu;
use bmo_verrano::{check, Backend, Error, Frame, Image, Stats, Vertex, Viewport};

use super::super::After;
use super::{numero, Texto, CLARO, FONDO, ROJO, TENUE, VERDE};
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// El sobre de los programas: fabricado en el anfitrion
/// (`ga10x/tests/bsf_sm86.rs`), viaja dentro de `d.bex`.
const SOBRE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../platform/drivers/gpu/ga10x/sombreadores/cubo.bsf"));

/// **El backend de la 3060**: el paquete (programas del BSF + vertices) al
/// kernel, y la ventana leida de vuelta.
struct La3060<'a> {
    ficha: u64,
    paquete: &'a mut [u8],
    vs: &'a [u8],
    ps: &'a [u8],
    leer_ms: u64,
}

impl Backend for La3060<'_> {
    fn draw(&mut self, frame: &Frame, out: &mut Image) -> Result<Stats, Error> {
        check(frame, out, tu::MAX_VERTICES)?;
        // V0: la ventana del cubo y su FONDO son los de las ordenes de X5.
        if (frame.viewport.width, frame.viewport.height) != (cu::ANCHO, cu::ALTO) || frame.clear.map(f32::to_bits) != cu::FONDO {
            return Err(Error::Image);
        }
        let mut v = [tu::Vertice::default(); tu::MAX_VERTICES];
        for (d, s) in v.iter_mut().zip(frame.vertices) {
            *d = tu::Vertice { posicion: s.position.map(f32::to_bits), color: s.color.map(f32::to_bits) };
        }
        tu::escribir_paquete(self.paquete, self.ficha as u32, self.vs, self.ps, &v[..frame.vertices.len()]).ok_or(Error::Vertices)?;
        let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_VERRANO | self.paquete.as_ptr() as u64).map_err(Error::Device)?;
        let (us, tris, etapas, _) = cu::desempaquetar(r);
        if !cu::sano(r) {
            return Err(Error::Device(0x5E00 | etapas));
        }
        let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
        let desde = bmo::ciclos();
        let n = (out.width * out.height) as usize;
        for k in 0..n / 2 {
            let d = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_LEER | k as u64).map_err(Error::Device)?;
            out.pixels[2 * k] = 0xFF00_0000 | d as u32;
            out.pixels[2 * k + 1] = 0xFF00_0000 | (d >> 32) as u32;
        }
        self.leer_ms = (bmo::ciclos() - desde) * 1000 / hz;
        Ok(Stats { triangles: tris, device_us: us })
    }
}

/// Los dos programas del sobre, comprobados (capas 1 a 4 de lo que se toma).
fn programas(bsf: &Bsf<'static>) -> Option<(&'static [u8], &'static [u8])> {
    let mut vs = None;
    let mut ps = None;
    for m in bsf.modules() {
        let codigo = m.target(kind::SM86, abi::SM86_V1, 0).and_then(|t| t.code().ok());
        match m.name() {
            b"cubo_vertice" => vs = codigo,
            b"cubo_pixel" => ps = codigo,
            _ => {}
        }
    }
    Some((vs?, ps?))
}

/// `gpu verrano [fotograma]`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, resto: &[u8]) -> After {
    let f = numero(resto).unwrap_or(30).min(359);
    paint_status(p, &dsk.run_box, "VERRANO V0: el cubo por la API de BMO-X, en la 3060 y en la CPU", INK_DIM);
    let (w, h) = (cu::ANCHO, cu::ALTO);
    if p.ancho < w || p.alto < h {
        return linea(dsk, b"  NO  la pantalla es mas chica que 1280x720", INK_ERR);
    }
    let Some((vs, ps)) = Bsf::parse(SOBRE).ok().and_then(|b| programas(&b)) else {
        return linea(dsk, b"  NO  el BSF de la tuberia no se sostiene: no se dibuja nada", INK_ERR);
    };
    let n = (w * h) as usize;
    let (Some(bloque), Some(caja)) = (bmo::Memoria::request(2 * n as u64 * 4), bmo::Memoria::request(4096)) else {
        return linea(dsk, b"  NO  sin memoria para dos fotogramas de 1280x720", INK_ERR);
    };
    // SAFETY: el bloque mide 2n palabras de 32 bits, alineado a pagina, es de
    // este proceso y solo se usa aqui; las dos mitades no se pisan. La caja,
    // 4096 bytes del proceso, para el paquete.
    let (gpu, cpu) = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut u32, 2 * n).split_at_mut(n) };
    // SAFETY: como arriba.
    let paquete = unsafe { core::slice::from_raw_parts_mut(caja.base() as *mut u8, 4096) };

    // El fotograma: la tanda del juez en vertices de VERRANO.
    let Some(t) = bmo_cubo::tanda::de_fotograma(f, w, h) else { return linea(dsk, b"  NO  la tanda no cabe", INK_ERR) };
    let mut v = [Vertex::default(); tu::MAX_VERTICES];
    let mut k = 0;
    for tri in t.tris() {
        for &pos in tri.clip.iter() {
            v[k] = Vertex { position: pos, color: tri.color };
            k += 1;
        }
    }
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &v[..k], viewport: Viewport { width: w, height: h } };

    // La pantalla ANTES del dibujo, como `gpu cubo 3060`.
    let (x0, y0) = (((p.ancho - w) / 2) & !31, (p.alto - h) / 2);
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    p.texto_escala(40, 24, "VERRANO V0: EL CUBO POR LA API DE BMO-X", CLARO, 2);
    p.vaciar();

    let ficha = match super::super::gspcomputo::ficha_del_gr() {
        Ok(fi) => fi,
        Err(m) => return motivo(dsk, m),
    };
    let mut la3060 = La3060 { ficha, paquete, vs, ps, leer_ms: 0 };
    let s3060 = la3060.draw(&frame, &mut Image { pixels: gpu, width: w, height: h });
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let mut juez = Cpu::LA_3060;
    let scpu = juez.draw(&frame, &mut Image { pixels: cpu, width: w, height: h });
    let cpu_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    let (st, _) = match (s3060, scpu) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(Error::Device(m)), _) if m & 0xFF00 == 0x5E00 => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            g.text(b"  NO  VERRANO en la 3060 no se pago entero; la escalera:\n");
            g.with_ink(INK_PLAIN);
            super::super::gspcomputo::escalera(g);
            dsk.field.n = 0;
            return After::Settle;
        }
        (Err(Error::Device(m)), _) => return motivo(dsk, m),
        _ => return linea(dsk, b"  NO  el fotograma no es valido para VERRANO V0", INK_ERR),
    };

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
    a.t(b"fotograma ").d(f as u64).t(b": la 3060 en ").d(st.device_us as u64).t(b" us, la CPU en ").d(cpu_us).t(b" us; huella ").x(huella);
    let mut b = Texto::nuevo();
    let (veredicto, color): (&[u8], u32) = if malos > 0 {
        b.t(b"DISTINTO: ").d(malos as u64).t(b" pixeles entre la 3060 y la CPU");
        (b.s(), ROJO)
    } else if d3d {
        (b"IGUAL: VERRANO en la 3060 = VERRANO en la CPU = D3D12 en la 3060 bajo Windows", VERDE)
    } else if medida {
        (b"IGUAL: VERRANO en la 3060 = lo que la 3060 dibujo en X5, y = la CPU", VERDE)
    } else {
        (b"IGUAL, pixel a pixel: VERRANO en la 3060 = VERRANO en la CPU", VERDE)
    };
    let mut c = Texto::nuevo();
    if let Some(i) = primero {
        c.t(b"el primero en (").d((i as u32 % w) as u64).t(b", ").d((i as u32 / w) as u64).t(b"): la 3060 ").x(gpu[i] as u64 & 0xFF_FFFF).t(b", la CPU ").x(cpu[i] as u64 & 0xFF_FFFF);
    } else if sin_explicar > 0 {
        c.t(b"y ").d(sin_explicar as u64).t(b" pixel sin explicar (del silicio, como con X5 y en Windows)");
    }
    let mut d = Texto::nuevo();
    d.t(b"programas del BSF (SM86): vertice ").d(vs.len() as u64).t(b" B, pixel ").d(ps.len() as u64).t(b" B; ").d(st.triangles as u64).t(b" triangulos en UN dibujo");
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
    g.dec(la3060.leer_ms);
    g.text(b" ms\n");
    super::super::datos::anotar(b"gpu verrano us", st.device_us as u64, b"");
    dsk.field.n = 0;
    After::Settle
}

fn motivo(dsk: &mut Desktop, m: u32) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ERR);
    g.text(b"  NO  VERRANO en la 3060 no dibujo: motivo ");
    g.dec(m as u64);
    g.text(b" (`gpu` lo explica)\n");
    g.with_ink(INK_PLAIN);
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
