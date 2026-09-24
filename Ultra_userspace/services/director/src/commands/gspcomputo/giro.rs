//! **M5d G: la esfera que gira y bota** -- la orden `gpu giro`, su animacion a
//! pantalla completa y su fila. La 3060 dibuja los 32 fotogramas de una vuelta
//! (un trabajo cada uno) y la CPU comprueba cada uno bit a bit; luego el
//! escritorio los PASA en bucle unos segundos, sin volver a pedirlos.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     32 trabajos de la 3060 y unos segundos de animacion; en
//!                     reposo no hace nada

use bmo_gpu_ga10x::giro::{self as gi, FOTOGRAMAS, LADO, PIXELES};
use bmo_userland as bmo;

use super::super::tabla::campo;
use super::super::After;
use super::{con, estado, hasta_el_lienzo, no, Computo, NO_GIRO_MAL, NO_TRABAJO_SIN_FICHA, PANEL_ABIERTO};
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo que dejo una vuelta: cuantos fotogramas salieron iguales y lo que
/// tardaron entre todos.
#[derive(Clone, Copy, Default)]
pub(super) struct Vuelta {
    buenos: u32,
    gpu_us: u64,
    cpu_us: u64,
    /// El primero que no salio, si alguno.
    malo: Option<(u32, u64)>,
}

/// La ficha del timbre, o por que no.
fn ficha() -> Result<u64, u32> {
    hasta_el_lienzo()?;
    estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA)
}

/// **La vuelta entera.** Por cada fotograma, `pintar(f)` justo despues de
/// comprobarlo (para verlo mientras se dibuja); se para en el primero malo.
fn vuelta(mut pintar: impl FnMut(u32)) -> Result<Vuelta, u32> {
    let ficha = ficha()?;
    let mut v = Vuelta::default();
    for f in 0..FOTOGRAMAS {
        let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_GIRO, ficha | (f as u64) << 32)?;
        let (_, _, _, _, gpu_us, cpu_us) = gi::desempaquetar(r);
        v.gpu_us += gpu_us as u64;
        v.cpu_us += cpu_us as u64;
        if !gi::sano(r) {
            v.malo = Some((f, r));
            break;
        }
        v.buenos += 1;
        pintar(f);
    }
    Ok(v)
}

fn guardar(r: Result<Vuelta, u32>) -> Result<u64, u32> {
    con(|c| c.giro = Some(r));
    match r {
        Ok(v) if v.buenos == FOTOGRAMAS => Ok(v.buenos as u64),
        Ok(_) => Err(NO_GIRO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo que da `save mode`: la vuelta, sin animacion.
pub(crate) fn dibujar_giro() -> Result<u64, u32> {
    guardar(vuelta(|_| {}))
}

pub(crate) fn giro_hecho() -> bool {
    matches!(estado().giro, Some(Ok(v)) if v.buenos == FOTOGRAMAS)
}

const FONDO: u32 = 0x000B_0D12;
const VERDE: u32 = 0x0076_B900;
const CLARO: u32 = 0x00E6_EDF6;
const TENUE: u32 = 0x008A_94A6;

/// Donde va el fotograma y a que escala.
fn marco(p: &bmo::Pantalla) -> (u32, u32, u32) {
    let e = ((p.alto.saturating_sub(80)) / LADO).clamp(1, 3);
    let lado = LADO * e;
    (p.ancho.saturating_sub(lado + 40), (p.alto.saturating_sub(lado)) / 2, e)
}

/// Un fotograma de `px` (0xAARRGGBB) en su marco.
fn pintar(p: &bmo::Pantalla, px: &[u32]) {
    let (x0, y0, e) = marco(p);
    for y in 0..LADO {
        for x in 0..LADO {
            let c = px[(y * LADO + x) as usize] & 0x00FF_FFFF;
            p.rect(x0 + x * e, y0 + y * e, e, e, c);
        }
    }
}

/// Lee el fotograma que acaba de dejar la 3060, de dos en dos pixeles.
fn leer(px: &mut [u32]) -> bool {
    for k in 0..(PIXELES / 2) as u64 {
        let Ok(dos) = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIENZO_LEER, k | 1 << 33) else { return false };
        px[2 * k as usize] = dos as u32;
        px[2 * k as usize + 1] = (dos >> 32) as u32;
    }
    true
}

/// `gpu giro`: la vuelta, vista mientras se dibuja, y luego en bucle.
pub(crate) fn orden_giro(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 dibuja una esfera que gira y bota", INK_DIM);
    // 32 fotogramas de 256 KiB: 8 MiB prestados, que vuelven solos al acabar.
    let bytes = (FOTOGRAMAS as usize * PIXELES * 4) as u64;
    let bloque = bmo::Memoria::request(bytes);
    let mut suelto = [0u32; 0];
    let cine: &mut [u32] = match &bloque {
        // SAFETY: el bloque mide `bytes`, es de este proceso y solo se usa aqui.
        Some(b) => unsafe { core::slice::from_raw_parts_mut(b.base() as *mut u32, FOTOGRAMAS as usize * PIXELES) },
        None => &mut suelto,
    };
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    p.rect(0, 0, p.ancho, 4, VERDE);
    p.texto_escala(40, 60, "BMO-X  |  RTX 3060", VERDE, 3);
    p.texto_escala(40, 120, "UNA ESFERA QUE GIRA Y BOTA", CLARO, 2);
    p.texto_bytes(40, 180, b"cada fotograma, un trabajo de la 3060: 65536 hilos, uno por pixel", CLARO);
    p.texto_bytes(40, 208, b"gira sobre su eje, bota, su sombra crece y el suelo avanza", CLARO);
    p.texto_bytes(40, 236, b"la CPU NO dibuja: rehace la cuenta de cada fotograma y compara", TENUE);
    p.vaciar();
    let mut uno = [0u32; 0];
    let r = vuelta(|f| {
        let px: &mut [u32] = if cine.is_empty() { &mut uno } else { &mut cine[f as usize * PIXELES..(f as usize + 1) * PIXELES] };
        if !px.is_empty() && leer(px) {
            pintar(p, px);
            p.vaciar();
        }
    });
    let bien = matches!(r, Ok(v) if v.buenos == FOTOGRAMAS);
    // La pasada: unos segundos en bucle, sin pedir nada mas a la 3060.
    if bien && !cine.is_empty() {
        let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
        let fin = bmo::ciclos() + hz * 6;
        let mut f = 0usize;
        while bmo::ciclos() < fin {
            let t = bmo::ciclos();
            pintar(p, &cine[f * PIXELES..(f + 1) * PIXELES]);
            p.vaciar();
            f = (f + 1) % FOTOGRAMAS as usize;
            // Unos 25 fotogramas por segundo.
            while bmo::ciclos() < t + hz / 25 {
                bmo::yield_screen();
            }
        }
    }
    let _ = guardar(r);
    let y = p.alto.saturating_sub(120);
    match r {
        Ok(v) if bien => {
            let mut t = super::Linea::nueva();
            t.t(b"32 de 32 fotogramas iguales a la CPU; la 3060: ").d(v.gpu_us / FOTOGRAMAS as u64).t(b" us por fotograma");
            p.texto_bytes(40, y, &t.b[..t.n], VERDE);
        }
        _ => {
            p.texto_bytes(40, y, b"la vuelta NO salio entera: mira la fila `giro`", 0x00FF_5555);
        }
    }
    p.texto_bytes(40, p.alto.saturating_sub(48), b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = true };
    let g = &mut dsk.out.grid;
    g.with_ink(if bien { INK_GOOD } else { INK_ERR });
    g.text(if bien { b"  LA 3060 DIBUJO UNA ESFERA QUE GIRA Y BOTA (M5d G): mira la fila `giro`\n" as &[u8] } else { b"  la esfera que gira no salio entera: mira la fila `giro`\n" });
    g.with_ink(INK_PLAIN);
    super::fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// La fila `giro`.
pub(super) fn fila(s: &mut Output, c: &Computo) {
    let Some(r) = c.giro else { return };
    campo(s, b"giro");
    match r {
        Err(m) => no(s, m),
        Ok(v) => {
            s.with_ink(if v.buenos == FOTOGRAMAS { INK_GOOD } else { INK_ERR });
            s.text(if v.buenos == FOTOGRAMAS { b"LA 3060 DIBUJO LA ESFERA QUE GIRA: " as &[u8] } else { b"la esfera que gira NO salio entera: " });
            s.dec(v.buenos as u64);
            s.text(b" de 32 fotogramas de 256x256 iguales a la CPU");
            s.with_ink(INK_ECHO);
            let n = (v.buenos as u64 + v.malo.is_some() as u64).max(1);
            s.text(b"; por fotograma la 3060 en ");
            s.dec(v.gpu_us / n);
            s.text(b" us, la CPU en ");
            s.dec(v.cpu_us / n);
            s.text(b" us");
            if let Some((f, r)) = v.malo {
                let (buenos, qmd, fin, _, _, _) = gi::desempaquetar(r);
                s.text(b"; el ");
                s.dec(f as u64);
                s.text(b": ");
                s.dec(buenos as u64);
                s.text(b" de 65536, semaforos ");
                s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
            }
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
}
