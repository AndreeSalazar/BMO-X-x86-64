//! **EL BACKEND DE LA RTX 3060 12G PARA VERRANO** -- la UNICA puerta del
//! escritorio entre la API de dibujo y la 3060 (`PLAN_EL_AISLAMIENTO.md` A2).
//!
//! [carril]  ROJO      arma el paquete que el kernel sube a la VRAM de la 3060
//! [consumo] NADA      corre cuando el propietario teclea `gpu verrano`
//!
//! # Por que una puerta
//!
//! `verrano.rs` (el cubo, el banco, la comparacion con la CPU) y `tablero.rs`
//! hablan VERRANO: `Frame`, `Vertex`, `Backend::draw`, `Stats`. No saben que
//! hay una 3060 debajo, ni que su codigo es SM86, ni que tiene un juez. Todo
//! eso vive AQUI, y en ningun otro fichero de `gspcubo/` (lo vigila
//! `la-3060`, regla S). El dia que haya otra GPU, trae SU fichero como este
//! -- su `kind` del BSF, su juez, sus ordenes -- y la API no cambia.
//!
//! # Lo que pasa al abrir
//!
//! ```text
//!    1  el sobre      el BSF con el `kind` de ESTA tarjeta (SM86, ABI
//!                     SM86_V1) y sus hashes; sin el, NO: no se compila nada
//!                     en marcha para salir del paso
//!    2  el juez       los dos programas, tal como viajan, con los registros
//!                     que la tarjeta les va a dar: un BODRIO no se manda
//!    3  la tarjeta    el motor grafico (lo que falte hasta `lienzo`) y la
//!                     ficha del GR
//! ```
//!
//! Y el kernel vuelve a juzgar en su puerta (`CUBO_VERRANO`): lo de aqui es
//! para decirlo claro en la pantalla, lo de alli es lo que no se puede saltar.

use bmo_bsf::{abi, kind, Bsf};
use bmo_gpu_ga10x::sass::juez;
use bmo_gpu_ga10x::{cubo as cu, raster, tuberia as tu};
use bmo_userland as bmo;
use bmo_verrano::{check, Backend, Error, Frame, Image, Stats};

use super::super::After;
use super::Texto;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

/// Como se dice esta tarjeta en la pantalla, y en las etiquetas del tablero.
pub(super) const NOMBRE: &[u8] = b"la RTX 3060 12G (SM86)";
pub(super) const ETIQUETA: &[u8] = b"LA 3060";

/// El sobre de los programas: fabricado en el anfitrion
/// (`ga10x/tests/bsf_sm86.rs`, que lo juzga antes), viaja dentro de `d.bex`.
const SOBRE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../platform/drivers/gpu/ga10x/sombreadores/cubo.bsf"));

/// Lo que mide la caja del paquete que se le pasa a [`abrir`].
pub(super) const CAJA: u64 = 4096;

/// Las opciones que son de ESTA tarjeta (las palabras que las piden).
#[derive(Clone, Copy, Default)]
pub(super) struct Opciones {
    /// `sinldg`: el programa de vertice SIN sus LDG, fabricado aqui (la
    /// prueba de una variable del cuelgue del 25-09). No dibuja el cubo.
    pub sin_ldg: bool,
    /// `ligero`: las ordenes sin la escalera de T1c.
    pub ligero: bool,
}

impl Opciones {
    pub(super) fn de(palabras: &[u8]) -> Self {
        let mut o = Opciones::default();
        for w in palabras.split(|&c| c == b' ') {
            match w {
                b"sinldg" => o.sin_ldg = true,
                b"ligero" => o.ligero = true,
                _ => {}
            }
        }
        o
    }

    /// Como se dice el modo en el tablero.
    pub(super) fn modo(&self) -> &'static [u8] {
        if self.ligero {
            b"ligero, sin escalera"
        } else {
            b"con la escalera de T1c"
        }
    }
}

/// Lo que se supo al abrir.
pub(super) struct Abierto {
    /// Instrucciones que el juez miro (las dos, juntas).
    pub instrucciones: usize,
    pub bytes_vs: usize,
    pub bytes_ps: usize,
}

/// **El backend**: el paquete (programas del BSF + vertices) al kernel, y la
/// ventana leida de vuelta si se pide.
pub(super) struct Aparato<'a> {
    ficha: u64,
    paquete: &'a mut [u8],
    vs: &'static [u8],
    ps: &'static [u8],
    /// El de vertice de `sinldg`, fabricado aqui (0 bytes = el del sobre).
    propio: [u8; 4 * tu::PALABRAS_VS],
    propio_n: usize,
    ligero: bool,
    /// Leer la imagen de vuelta (la comparacion la quiere); el banco no.
    pub leer: bool,
    pub leer_ms: u64,
}

/// Donde cae la ventana de 1280x720 en esta pantalla (la misma cuenta que el
/// kernel: `cubo::ventana`).
pub(super) fn ventana(p: &bmo::Pantalla) -> (u32, u32) {
    (((p.ancho - cu::ANCHO) / 2) & !31, (p.alto - cu::ALTO) / 2)
}

/// Los dos programas de ESTA tarjeta en el sobre, comprobados (capas 1 a 4
/// de lo que se toma).
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

/// **Abrir la 3060 para VERRANO**: el sobre, el juez y la tarjeta. `Err` ya
/// lo dijo en el panel.
pub(super) fn abrir<'a>(dsk: &mut Desktop, p: &bmo::Pantalla, caja: &'a mut [u8], op: Opciones) -> Result<(Aparato<'a>, Abierto), After> {
    let Some((vs, ps)) = Bsf::parse(SOBRE).ok().and_then(|b| programas(&b)) else {
        return Err(linea(dsk, b"  NO  el BSF no trae codigo SM86 (ABI SM86_V1) que se sostenga: no se dibuja nada", INK_ERR));
    };
    // El juez, ANTES de que la 3060 vea nada.
    let r = raster::REGISTROS;
    let juicio = juez::juzgar_programa(vs, r).map_err(|b| ("vertice", b)).and_then(|a| juez::juzgar_programa(ps, r).map(|b| a.instrucciones + b.instrucciones).map_err(|b| ("pixel", b)));
    let instrucciones = match juicio {
        Ok(n) => n,
        Err((cual, b)) => {
            let mut t = Texto::nuevo();
            let _ = core::fmt::write(&mut t, format_args!("  NO  el programa de {cual} del BSF: {b}"));
            return Err(linea(dsk, t.s(), INK_ERR));
        }
    };
    let mut propio = [0u8; 4 * tu::PALABRAS_VS];
    let propio_n = if op.sin_ldg { tu::bytes(&tu::vertice_sin_ldg(), &mut propio) } else { 0 };
    // Lo que falte del motor grafico (con `init` por defecto, casi todo).
    if super::super::verificar::preparar_hasta(dsk, p, b"lienzo").is_err() {
        dsk.field.n = 0;
        return Err(After::Settle);
    }
    let ficha = super::super::gspcomputo::ficha_del_gr().map_err(|m| motivo(dsk, m))?;
    let abierto = Abierto { instrucciones, bytes_vs: vs.len(), bytes_ps: ps.len() };
    Ok((Aparato { ficha, paquete: caja, vs, ps, propio, propio_n, ligero: op.ligero, leer: true, leer_ms: 0 }, abierto))
}

impl Backend for Aparato<'_> {
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
        let vs = if self.propio_n > 0 { &self.propio[..self.propio_n] } else { self.vs };
        tu::escribir_paquete(self.paquete, self.ficha as u32, vs, self.ps, &v[..frame.vertices.len()]).ok_or(Error::Vertices)?;
        let ligero = if self.ligero { bmo::CUBO_LIGERO } else { 0 };
        let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_VERRANO | ligero | self.paquete.as_ptr() as u64).map_err(Error::Device)?;
        let (us, tris, etapas, _) = cu::desempaquetar(r);
        if !cu::sano(r) {
            return Err(Error::Device(ESCALERA | etapas));
        }
        let (warm, prepare_us) = cu::preparado(r);
        let st = Stats { triangles: tris, device_us: us, prepare_us, warm };
        if !self.leer {
            return Ok(st);
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
        Ok(st)
    }
}

/// `Error::Device(ESCALERA | etapas)`: se lanzo y no se pago entero.
const ESCALERA: u32 = 0x5E00;

/// **Decir por que la 3060 no dibujo**, con lo que ESTA tarjeta sabe contar:
/// la escalera de T1c y los avisos del GSP. `en` = el fotograma del banco.
pub(super) fn fallo(dsk: &mut Desktop, e: Error, en: Option<u32>, op: Opciones) -> After {
    match e {
        Error::Device(m) if m & 0xFF00 == ESCALERA => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            match en {
                Some(i) => {
                    g.text(b"  NO  el banco se paro en el fotograma ");
                    g.dec(i as u64);
                }
                None => g.text(b"  NO  VERRANO en la 3060 no se pago entero"),
            }
            if op.ligero {
                g.text(b" (ligero: sin escalera; sin `ligero` dice donde)");
            }
            g.text(b"; la escalera:\n");
            g.with_ink(INK_PLAIN);
            super::super::gspcomputo::escalera(g);
            // Y lo que el GSP conto: un Xid 31 es un FALLO DE PAGINA (la
            // direccion que lee el LDG), un 13 una excepcion del sombreador.
            super::super::gspcola::avisos(g, 4);
            if op.sin_ldg {
                g.with_ink(INK_ERR);
                g.text(b"  SIN LDG y aun asi colgado en los VERTICES: los LDG quedan ABSUELTOS; es otra cosa del programa\n");
                g.with_ink(INK_PLAIN);
            }
            dsk.field.n = 0;
            After::Settle
        }
        Error::Device(m) => motivo(dsk, m),
        _ => linea(dsk, b"  NO  el fotograma no es valido para la tuberia fija de la 3060 (1280x720, el fondo del estudio)", INK_ERR),
    }
}

/// `sinldg` salio bien: lo que eso quiere decir.
pub(super) fn sin_ldg_pagado(dsk: &mut Desktop) -> After {
    linea(dsk, b"  SIN LDG la 3060 PAGO los VERTICES y el dibujo: el cuelgue es del LDG (o de la direccion que lee). El cubo sale vacio a proposito", INK_GOOD)
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
