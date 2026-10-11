//! **`gpu completa`** (VE5, 11-10): COMPLETAR -- el filtro que quita el
//! grano de ESTUDIO VIVO guiado por la superficie de cada pixel -- por la
//! 3060, y JUZGADO aqui con la cuenta de VERRANO (`bmo_verrano::completa`),
//! pixel a pixel, pasada a pasada.
//!
//! Una escena de mentira de 320x240 (las paredes roja y verde, el fondo, el
//! suelo y una esfera, con su guia de verdad) y una luz con GRANO (lo que da
//! una muestra por pixel); tres pasadas en la 3060, A -> B -> A -> B, cada
//! una juzgada ENTERA contra la CPU antes de la siguiente. Arriba a la
//! derecha, la luz con grano y, al lado, COMPLETADA POR LA 3060.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: tres pasadas
//!                     de la 3060 y su juez; en reposo, nada
//!
//! El bloque es el de VERRANO (`bmo_verrano::completa::BYTES_POR_PIXEL`): la
//! luz A, la guia y la luz B; el kernel lo presta entero a la 3060 durante
//! cada pasada y lo devuelve (`gpu_trabajo/completa.rs`).

use bmo_gpu_ga10x::completa as co;
use bmo_userland as bmo;
use bmo_verrano::completa as v;

use super::super::After;
use super::{estado, hasta_el_lienzo, NO_TRABAJO_SIN_FICHA};
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Una pasada de la 3060 no salio igual que la CPU (la fila de `gpu completa`).
pub(crate) const NO_COMPLETA_MAL: u32 = 0x156;
/// No hubo un bloque de memoria para `gpu completa`.
pub(crate) const NO_COMPLETA_SIN_MEMORIA: u32 = 0x157;

/// Las pasadas (a 320x240, tres: `completa_prueba` de ESTUDIO VIVO).
const PASADAS: u32 = 3;
/// La tolerancia de la luz de una muestra por pixel (`vivo::FRENO`).
const FRENO: f32 = v::FRENO;

/// Raiz cuadrada sin `std` (Newton desde el exponente a la mitad).
fn raiz(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut r = f32::from_bits((x.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..4 {
        r = 0.5 * (r + x / r);
    }
    r
}

/// **La escena de mentira**: la guia de cada pixel y su luz con grano, en
/// la luz A del bloque.
fn escena(b: &mut [f32], f: &co::Formato) {
    let n = f.pixeles() as usize;
    let (w, h) = (f.ancho as f32, f.alto as f32);
    let mut s = 0x2545_F491u32;
    for y in 0..f.alto {
        for x in 0..f.ancho {
            let i = (y * f.ancho + x) as usize;
            let (fx, fy) = (x as f32 / w, y as f32 / h);
            // la esfera: un disco en el centro de abajo
            let (dx, dy) = ((fx - 0.5) * w / h, fy - 0.62);
            let r2 = (dx * dx + dy * dy) / (0.17 * 0.17);
            let g = if r2 < 1.0 {
                let nz = raiz(1.0 - r2);
                v::Guia { n: [dx / 0.17, -dy / 0.17, nz], z: 2.4 - 0.17 * nz, albedo: [0.78, 0.62, 0.48] }
            } else if fx < 0.2 {
                v::Guia { n: [1.0, 0.0, 0.0], z: 2.0 + fx * 4.0, albedo: [0.65, 0.05, 0.05] }
            } else if fx > 0.8 {
                v::Guia { n: [-1.0, 0.0, 0.0], z: 2.0 + (1.0 - fx) * 4.0, albedo: [0.12, 0.45, 0.15] }
            } else if fy > 0.7 {
                v::Guia { n: [0.0, 1.0, 0.0], z: 2.0 + (1.0 - fy) * 6.0, albedo: [0.73, 0.73, 0.73] }
            } else {
                v::Guia { n: [0.0, 0.0, 1.0], z: 3.2, albedo: [0.73, 0.73, 0.73] }
            };
            v::pon_guia(b, n, i, &g);
            // la luz: de arriba (mas en lo que mira arriba), con el grano de
            // UNA muestra (de 0 a 2,5 veces)
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let grano = 2.5 * (s >> 8) as f32 / (1u32 << 24) as f32;
            let luz = (0.55 + 0.45 * g.n[1].max(0.0)) * grano;
            v::pon_luz(b, n, false, i, g.albedo.map(|a| a * luz));
        }
    }
}

/// De la luz a la pantalla: Reinhard y gamma 2.
fn pelicula(c: v::V3) -> u32 {
    let k = |x: f32| (raiz(x / (1.0 + x)) * 255.0 + 0.5).min(255.0) as u32;
    k(c[0]) << 16 | k(c[1]) << 8 | k(c[2])
}

fn pintar(p: &bmo::Pantalla, b: &[f32], f: &co::Formato, de_b: bool, x0: u32, y0: u32) {
    let n = f.pixeles() as usize;
    p.rect(x0.saturating_sub(3), y0.saturating_sub(3), f.ancho + 6, f.alto + 6, 0x0076_B900);
    for i in 0..n {
        let (x, y) = (i as u32 % f.ancho, i as u32 / f.ancho);
        p.rect(x0 + x, y0 + y, 1, 1, pelicula(v::luz_del_bloque(b, n, de_b, i)));
    }
}

/// Lo que dio una pasada: pixeles iguales al juez, us de la 3060, us de la CPU.
#[derive(Clone, Copy, Default)]
struct Pasada {
    iguales: u32,
    gpu_us: u32,
    cpu_us: u64,
}

pub(crate) fn orden_completa(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 completa la luz de ESTUDIO VIVO (VE5)", INK_DIM);
    let f = co::Formato::ESTUDIO;
    let n = f.pixeles() as usize;
    let mut hechas = [Pasada::default(); PASADAS as usize];
    let r = (|| -> Result<(), u32> {
        hasta_el_lienzo()?;
        let ficha = estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA)?;
        bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_COMPLETA_FORMATO, f.ancho as u64 | (f.alto as u64) << 16 | ficha << 32)?;
        let bloque = bmo::Memoria::request(f.bytes()).ok_or(NO_COMPLETA_SIN_MEMORIA)?;
        // SAFETY: el bloque es nuestro, mide `f.bytes()` (64 B por pixel: 16 f32) y
        // vive hasta el final de esta funcion; la 3060 lo ve solo DENTRO de
        // cada llamada (el kernel lo presta y lo devuelve).
        let b = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut f32, 16 * n) };
        escena(b, &f);
        pintar(p, b, &f, false, p.ancho.saturating_sub(2 * f.ancho + 64), 96);
        let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
        let mut tolerancia = FRENO;
        for paso in 0..PASADAS {
            let de_b = paso % 2 == 1;
            // la tolerancia viaja en 16 bits: la CPU juzga con la MISMA
            let corta = co::tolerancia_corta(tolerancia);
            let t = co::tolerancia_de(corta);
            let arg = bloque.base() as u64 | paso as u64 | if de_b { bmo::COMPLETA_DE_B } else { 0 } | if paso == 0 { bmo::COMPLETA_CARGAR } else { 0 } | (corta as u64) << 48;
            let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_COMPLETA, arg)?;
            let (_, _, _, _, gpu_us, _) = co::desempaquetar(r);
            if !co::sano(r) {
                return Err(NO_COMPLETA_MAL);
            }
            // ** EL JUEZ: cada pixel, por la cuenta de VERRANO, desde la luz
            // que la 3060 leyo (sigue ahi: escribio la otra)
            let desde = bmo::ciclos();
            let mut iguales = 0u32;
            for y in 0..f.alto {
                for x in 0..f.ancho {
                    let cpu = v::del_bloque(b, f.ancho, f.alto, paso, de_b, t, x, y);
                    let gpu = v::luz_del_bloque(b, n, !de_b, (y * f.ancho + x) as usize);
                    iguales += co::igual(gpu, cpu) as u32;
                }
            }
            hechas[paso as usize] = Pasada { iguales, gpu_us, cpu_us: (bmo::ciclos() - desde) * 1_000_000 / hz };
            if iguales as usize != n {
                return Err(NO_COMPLETA_MAL);
            }
            tolerancia *= 0.5;
        }
        // tres pasadas: lo ultimo, en la B
        pintar(p, b, &f, true, p.ancho.saturating_sub(f.ancho + 32), 96);
        Ok(())
    })();
    let g = &mut dsk.out.grid;
    for (k, h) in hechas.iter().enumerate().filter(|(_, h)| h.iguales > 0) {
        g.with_ink(if h.iguales as usize == n { INK_GOOD } else { INK_ERR });
        g.text(b"  pasada ");
        g.dec(k as u64 + 1);
        g.text(b" (vecinos a ");
        g.dec(1 << k);
        g.text(b"): ");
        g.dec(h.iguales as u64);
        g.text(b" de ");
        g.dec(n as u64);
        g.text(b" pixeles iguales al juez; la 3060 ");
        g.dec(h.gpu_us as u64);
        g.text(b" us, el juez de la CPU ");
        g.dec(h.cpu_us / 1000);
        g.text(b" ms\n");
    }
    match r {
        Ok(()) => {
            g.with_ink(INK_GOOD);
            g.text(b"  ARRIBA A LA DERECHA: la luz con grano de UNA muestra, y al lado, COMPLETADA POR TU 3060 (VE5), igual que la CPU\n");
        }
        Err(m) => {
            g.with_ink(INK_ERR);
            g.text(b"  completar no salio; motivo ");
            g.dec(m as u64);
            g.text(b" (sin el lienzo, sin memoria, la 3060 no pago, o un pixel distinto del juez)\n");
        }
    }
    g.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, "completa", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}
