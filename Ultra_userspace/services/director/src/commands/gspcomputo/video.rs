//! **M6 V0: EL VIDEO POR LA 3060** -- la orden `gpu video` y su fila. Lee un
//! video NV12 crudo del disco, fotograma a fotograma, y la 3060 lo convierte
//! a color y lo agranda DIRECTAMENTE en la pantalla. La CPU solo lee el
//! fichero y comprueba 256 pixeles de cada fotograma.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: un fotograma
//!                     del disco y un trabajo de la 3060 por fotograma, hasta
//!                     el final del fichero o [`TOPE_S`]; en reposo, nada
//!
//! # De donde sale el NV12 (un `.mp4` cualquiera)
//!
//! ```text
//!    ffmpeg -i video.mp4 -vf scale=640:360 -pix_fmt nv12 -f rawvideo video.nv12
//! ```
//!
//! En Windows o en la antena (Termux). Es el reparto de `PLAN_CLOUD_LOCAL`:
//! lo sucio (el contenedor y el codec) fuera; aqui entran bytes con una forma
//! fija que se comprueba. Sin sonido todavia: el tubo de audio va aparte.

use bmo_gpu_ga10x::video as vi;
use bmo_userland as bmo;

use super::super::tabla::campo;
use super::super::After;
use super::{con, estado, hasta_el_lienzo, no, Linea, NO_TRABAJO_SIN_FICHA, PANEL_ABIERTO};
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo mas que dura una reproduccion: el escritorio no ve el teclado mientras
/// tanto (una orden no lo tiene), asi que no puede ser un video entero.
pub(super) const TOPE_S: u64 = 120;

/// Por que se acabo.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fin {
    /// El fichero se acabo.
    Final,
    /// Llego a [`TOPE_S`].
    Tope,
    /// Un fotograma no se pudo leer entero del disco.
    Disco,
    /// La 3060 dijo NO o una muestra no cuadro.
    Mal,
}

/// Lo que dejo una reproduccion.
#[derive(Clone, Copy)]
pub(super) struct Repro {
    ancho: u32,
    alto: u32,
    escala: u32,
    fps: u32,
    /// Fotogramas en el fichero, pedidos a la 3060 y buenos.
    en_fichero: u64,
    pedidos: u32,
    buenos: u32,
    /// Sumados, en us: leer del disco, la 3060, comprobar; y todo, de reloj.
    disco_us: u64,
    gpu_us: u64,
    cpu_us: u64,
    total_us: u64,
    /// Los que llegaron tarde a su hora (el disco o la 3060 no dieron abasto).
    tarde: u32,
    fin: Fin,
    /// El primero que no salio: su NO, o lo que dijo la 3060.
    malo: Option<(u32, Result<u64, u32>)>,
}

impl Repro {
    fn bien(&self) -> bool {
        self.pedidos > 0 && self.buenos == self.pedidos && self.fin != Fin::Mal && self.fin != Fin::Disco
    }

    /// Fotogramas por segundo que se VIERON, en decimas.
    fn fps10(&self) -> u64 {
        self.buenos as u64 * 10_000_000 / self.total_us.max(1)
    }
}

fn ficha() -> Result<u64, u32> {
    hasta_el_lienzo()?;
    estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA)
}

/// `<ruta> <ancho>x<alto> [fps]`.
fn parsear(args: &[u8]) -> Option<(&[u8], u32, u32, u32)> {
    let mut t = args.split(|&b| b == b' ').filter(|t| !t.is_empty());
    let ruta = t.next()?;
    let medida = t.next()?;
    let x = medida.iter().position(|&b| b == b'x' || b == b'X')?;
    let (w, h) = (numero(&medida[..x])?, numero(&medida[x + 1..])?);
    let fps = match t.next() {
        Some(f) => numero(f)?,
        None => 30,
    };
    (t.next().is_none() && (1..=60).contains(&fps)).then_some((ruta, w, h, fps))
}

fn numero(s: &[u8]) -> Option<u32> {
    if s.is_empty() || s.len() > 5 {
        return None;
    }
    s.iter().try_fold(0u32, |v, &c| c.is_ascii_digit().then(|| v * 10 + (c - b'0') as u32))
}

/// **Reproducir**: el formato a la 3060, y fotograma a fotograma, a su hora.
fn reproducir(ruta: &[u8], f: vi::Formato, fps: u32) -> Result<Repro, u32> {
    let ficha = ficha()?;
    let encaje = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_VIDEO_FORMATO, f.ancho as u64 | (f.alto as u64) << 16 | ficha << 32)?;
    let a = bmo::Archivo::reflejar(ruta)?;
    let bytes = f.bytes();
    let bloque = bmo::Memoria::request(bytes).ok_or(super::NO_VIDEO_SIN_MEMORIA)?;
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let us = |c: u64| c * 1_000_000 / hz;
    let periodo = hz / fps as u64;
    let mut r = Repro {
        ancho: f.ancho,
        alto: f.alto,
        escala: (encaje & 0xFF) as u32,
        fps,
        en_fichero: a.size() / bytes,
        pedidos: 0,
        buenos: 0,
        disco_us: 0,
        gpu_us: 0,
        cpu_us: 0,
        total_us: 0,
        tarde: 0,
        fin: Fin::Final,
        malo: None,
    };
    let desde = bmo::ciclos();
    let mut k = 0u64;
    while k < r.en_fichero {
        if us(bmo::ciclos() - desde) >= TOPE_S * 1_000_000 {
            r.fin = Fin::Tope;
            break;
        }
        let t = bmo::ciclos();
        a.saltar(k * bytes);
        if a.leer_en(&bloque, 0, bytes) != bytes {
            r.fin = Fin::Disco;
            break;
        }
        r.disco_us += us(bmo::ciclos() - t);
        let cargar = if k == 0 { bmo::VIDEO_CARGAR } else { 0 };
        let v = match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_VIDEO, bloque.base() as u64 | cargar) {
            Ok(v) => v,
            Err(m) => {
                r.fin = Fin::Mal;
                r.malo = Some((k as u32, Err(m)));
                break;
            }
        };
        r.pedidos += 1;
        let (_, _, _, _, gpu_us, cpu_us) = vi::desempaquetar(v);
        r.gpu_us += gpu_us as u64;
        r.cpu_us += cpu_us as u64;
        if !vi::sano(v) {
            r.fin = Fin::Mal;
            r.malo = Some((k as u32, Ok(v)));
            break;
        }
        r.buenos += 1;
        k += 1;
        // A su hora: el fotograma k se ve en `desde + k * periodo`.
        let hora = desde + k * periodo;
        if bmo::ciclos() > hora {
            r.tarde += 1;
        }
        while bmo::ciclos() < hora {
            bmo::yield_screen();
        }
    }
    r.total_us = us(bmo::ciclos() - desde);
    a.close();
    Ok(r)
}

const VERDE: u32 = 0x0076_B900;
const CLARO: u32 = 0x00E6_EDF6;
const TENUE: u32 = 0x008A_94A6;
const CAJA: u32 = 0x000B_0D12;

/// Lo que sale si falta algo: como se usa y de donde sale el fichero.
fn uso(dsk: &mut Desktop) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ERR);
    g.text(b"  gpu video <fichero> <ancho>x<alto> [fps]   (p. ej. `gpu video datos/clip.nv12 640x360 30`)\n");
    g.with_ink(INK_ECHO);
    g.text(b"  el fichero es NV12 crudo, de cualquier .mp4: ffmpeg -i video.mp4 -vf scale=640:360 -pix_fmt nv12 -f rawvideo clip.nv12\n");
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu video <fichero> <ancho>x<alto> [fps]`.
pub(crate) fn orden_video(dsk: &mut Desktop, p: &bmo::Pantalla, args: &[u8]) -> After {
    let Some((ruta, ancho, alto, fps)) = parsear(args) else { return uso(dsk) };
    let f = vi::Formato { ancho, alto };
    if !f.valido() {
        return uso(dsk);
    }
    paint_status(p, &dsk.run_box, "la 3060 pone el video en la pantalla", INK_DIM);
    // El fondo, negro (las bandas si el video no llena la pantalla).
    p.rect(0, 0, p.ancho, p.alto, 0);
    p.vaciar();
    let r = reproducir(ruta, f, fps);
    con(|c| c.video = Some(r));
    let y = p.alto.saturating_sub(150);
    let ancho_caja = p.ancho.saturating_sub(48).min(1100);
    p.rect(24, y, ancho_caja, 126, CAJA);
    p.rect(24, y, ancho_caja, 3, VERDE);
    p.texto_escala(40, y + 14, "EL VIDEO, POR LA 3060", VERDE, 2);
    let bien = matches!(r, Ok(v) if v.bien());
    match r {
        Ok(v) => {
            let mut l = Linea::nueva();
            l.t(b"").d(v.buenos as u64).t(b" fotogramas de ").d(v.ancho as u64).t(b"x").d(v.alto as u64).t(b" (x").d(v.escala as u64).t(b"), ");
            l.d(v.fps10() / 10).t(b".").d(v.fps10() % 10).t(b" fps de ").d(v.fps as u64).t(b"; la 3060: ");
            l.d(v.gpu_us / v.pedidos.max(1) as u64).t(b" us por fotograma");
            p.texto_bytes(40, y + 54, &l.b[..l.n], if bien { CLARO } else { 0x00FF_5555 });
        }
        Err(m) => {
            p.texto_bytes(40, y + 54, super::super::iommu::motivo(m), 0x00FF_5555);
        }
    }
    p.texto_bytes(40, y + 78, b"la 3060 pasa cada fotograma de NV12 a color y lo agranda en la pantalla; la CPU solo lee el disco", TENUE);
    p.texto_bytes(40, y + 100, b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = true };
    let g = &mut dsk.out.grid;
    g.with_ink(if bien { INK_GOOD } else { INK_ERR });
    g.text(if bien {
        b"  LA 3060 PUSO EL VIDEO EN LA PANTALLA (M6 V0): mira la fila `video`\n" as &[u8]
    } else {
        b"  el video no salio entero: mira la fila `video`\n"
    });
    g.with_ink(INK_PLAIN);
    super::fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// La fila `video`.
pub(super) fn fila(s: &mut Output, c: &super::Computo) {
    let Some(r) = c.video else { return };
    campo(s, b"video");
    let v = match r {
        Err(m) => return no(s, m),
        Ok(v) => v,
    };
    s.with_ink(if v.bien() { INK_GOOD } else { INK_ERR });
    s.text(if v.bien() { b"LA 3060 PUSO EL VIDEO: " as &[u8] } else { b"el video NO salio entero: " });
    s.dec(v.buenos as u64);
    s.text(b" de ");
    s.dec(v.en_fichero);
    s.text(b" fotogramas NV12 de ");
    s.dec(v.ancho as u64);
    s.byte(b'x');
    s.dec(v.alto as u64);
    s.text(b" (x");
    s.dec(v.escala as u64);
    s.byte(b')');
    s.with_ink(INK_ECHO);
    s.text(b"; ");
    s.dec(v.fps10() / 10);
    s.byte(b'.');
    s.dec(v.fps10() % 10);
    s.text(b" fps de ");
    s.dec(v.fps as u64);
    let n = v.pedidos.max(1) as u64;
    s.text(b"; por fotograma: disco ");
    s.dec(v.disco_us / n);
    s.text(b" us, la 3060 ");
    s.dec(v.gpu_us / n);
    s.text(b" us, comprobar ");
    s.dec(v.cpu_us / n);
    s.text(b" us; ");
    s.dec(v.tarde as u64);
    s.text(b" tarde");
    s.text(match v.fin {
        Fin::Final => b"; hasta el final" as &[u8],
        Fin::Tope => b"; cortado a los 120 s",
        Fin::Disco => b"; un fotograma no se leyo entero del disco",
        Fin::Mal => b"",
    });
    if let Some((f, r)) = v.malo {
        s.with_ink(INK_ERR);
        s.text(b"; el ");
        s.dec(f as u64);
        match r {
            Err(m) => {
                s.text(b": NO: ");
                s.text(super::super::iommu::motivo(m));
            }
            Ok(r) => {
                let (buenos, qmd, fin, _, _, _) = vi::desempaquetar(r);
                s.text(b": ");
                s.dec(buenos as u64);
                s.text(b" de 256 muestras, semaforos ");
                s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
            }
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::super::datos::anotar(b"gpu video fps10", v.fps10(), b"");
}
