//! **M5d P: LA 3060 TOMA LA PANTALLA** -- la orden `gpu pantalla` y su fila.
//! Cada fotograma lo pinta la 3060 ENTERO, a la resolucion del monitor,
//! directamente en el framebuffer del GOP: la CPU no mueve ni un pixel. Un
//! fractal que se acerca al valle de los caballitos de mar y vuelve, con la
//! paleta girando; la CPU rehace 1024 pixeles de cada fotograma y los lee de
//! vuelta de la pantalla.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea (dos ciclos, 400
//!                     fotogramas), o en `save mode` (8); en reposo, nada

use bmo_gpu_ga10x::pantalla as pa;
use bmo_userland as bmo;

use super::super::tabla::campo;
use super::super::After;
use super::{con, estado, hasta_el_lienzo, no, Linea, NO_PANTALLA_MAL, NO_TRABAJO_SIN_FICHA, PANEL_ABIERTO};
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Lo que dejo una tanda.
#[derive(Clone, Copy, Default)]
pub(super) struct Tanda {
    pedidos: u32,
    buenos: u32,
    /// Lo que tardo la 3060, sumado; y la tanda entera, de reloj.
    gpu_us: u64,
    total_us: u64,
    ancho: u32,
    alto: u32,
    /// El primero que no salio, si alguno.
    malo: Option<(u32, u64)>,
    /// B1 (25-09): lo que tardo la CPU en COMPROBAR, sumado, y cuantas
    /// muestras comprobo (1024 por fotograma, o 16 con el muestreo de C1).
    cpu_us: u64,
    muestras: u64,
}

impl Tanda {
    fn bien(&self) -> bool {
        self.pedidos > 0 && self.buenos == self.pedidos
    }

    /// Fotogramas por segundo, en decimas.
    fn fps10(&self) -> u64 {
        self.buenos as u64 * 10_000_000 / self.total_us.max(1)
    }

    /// **B1: un fotograma, partido** en us: `(total, la 3060, comprobar,
    /// el resto)`. El resto es preparar por PRAMIN, el timbre y el syscall.
    fn partido(&self) -> (u64, u64, u64, u64) {
        let n = self.pedidos.max(1) as u64;
        let (total, gpu, cpu) = (self.total_us / n, self.gpu_us / n, self.cpu_us / n);
        (total, gpu, cpu, total.saturating_sub(gpu + cpu))
    }
}

fn ficha() -> Result<u64, u32> {
    hasta_el_lienzo()?;
    estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA)
}

/// Los bits de un fotograma: el primero carga el programa y comprueba las
/// 1024; con `todas` (lo de `save mode`) TODOS las 1024; si no, los de paso
/// comprueban [`pa::POCAS`] que rotan (C1). `(bits, muestras esperadas)`.
fn bits(f: u32, todas: bool) -> (u64, u32) {
    let cargar = if f == 0 { bmo::PANTALLA_CARGAR } else { 0 };
    if todas || f == 0 {
        (cargar, pa::MUESTRAS)
    } else {
        (bmo::PANTALLA_POCAS, pa::POCAS)
    }
}

const _: () = assert!(bmo::PANTALLA_POCAS == 1 << 57 && bmo::PANTALLA_CARGAR == 1 << 56);

/// **Una tanda de `n` fotogramas**, seguidos; se para en el primero malo.
fn tanda(n: u32, ancho: u32, alto: u32, todas: bool) -> Result<Tanda, u32> {
    let ficha = ficha()?;
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let mut t = Tanda { ancho, alto, ..Tanda::default() };
    for f in 0..n {
        let (b, esperadas) = bits(f, todas);
        let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PANTALLA, ficha | (f as u64) << 32 | b)?;
        t.pedidos += 1;
        let (_, _, _, _, gpu_us, cpu_us) = pa::desempaquetar(r);
        t.gpu_us += gpu_us as u64;
        t.cpu_us += cpu_us as u64;
        t.muestras += esperadas as u64;
        if !pa::sano_con(r, esperadas) {
            t.malo = Some((f, r));
            break;
        }
        t.buenos += 1;
    }
    t.total_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    Ok(t)
}

fn guardar(r: Result<Tanda, u32>) -> Result<u64, u32> {
    con(|c| c.pantalla = Some(r));
    match r {
        Ok(t) if t.bien() => Ok(t.buenos as u64),
        Ok(_) => Err(NO_PANTALLA_MAL),
        Err(m) => Err(m),
    }
}

/// Lo que da `save mode`: 8 fotogramas (la pantalla se repinta despues).
pub(crate) fn dibujar_pantalla() -> Result<u64, u32> {
    let (ancho, alto) = medidas();
    guardar(tanda(8, ancho, alto, true))
}

pub(crate) fn pantalla_hecha() -> bool {
    matches!(estado().pantalla, Some(Ok(t)) if t.bien())
}

/// **Un fotograma suelto** a pantalla completa, para el arranque orquestado
/// (`desktop::arranque`): sin tanda ni fila, y SOLO si `pantalla` ya salio
/// -- nunca da pasos que falten (`hasta_el_lienzo` si los daria). `cargar`:
/// el programa, en el primero. `Ok(false)`: la 3060 lo pinto mal.
pub(crate) fn fotograma(f: u32, cargar: bool) -> Result<bool, u32> {
    if !pantalla_hecha() {
        return Err(NO_PANTALLA_MAL);
    }
    let ficha = estado().timbre.map(|(v, _)| v as u64).ok_or(NO_TRABAJO_SIN_FICHA)?;
    // El que carga comprueba las 1024; los de paso, las POCAS que rotan (C1).
    let (b, esperadas) = if cargar { (bmo::PANTALLA_CARGAR, pa::MUESTRAS) } else { (bmo::PANTALLA_POCAS, pa::POCAS) };
    let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_PANTALLA, ficha | (f as u64) << 32 | b)?;
    Ok(pa::sano_con(r, esperadas))
}

/// Las medidas del modo que barre la 3060 (`INFO_GPU_MODO`): las del GOP.
fn medidas() -> (u32, u32) {
    let v = bmo::info(bmo::INFO_GPU_MODO);
    ((v & 0xFFFF) as u32, (v >> 16 & 0xFFFF) as u32)
}

const VERDE: u32 = 0x0076_B900;
const CLARO: u32 = 0x00E6_EDF6;
const TENUE: u32 = 0x008A_94A6;
const CAJA: u32 = 0x000B_0D12;

/// `gpu pantalla`: dos ciclos a pantalla completa, y lo que dijo encima.
pub(crate) fn orden_pantalla(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 toma la pantalla entera", INK_DIM);
    let r = tanda(2 * pa::CICLO, p.ancho, p.alto, false);
    let _ = guardar(r);
    // La caja de abajo, pintada por la CPU ENCIMA de lo ultimo de la 3060:
    // solo esa caja se vuelca, el resto es de la 3060.
    let y = p.alto.saturating_sub(150);
    p.rect(24, y, p.ancho.saturating_sub(48).min(1100), 126, CAJA);
    p.rect(24, y, p.ancho.saturating_sub(48).min(1100), 3, VERDE);
    p.texto_escala(40, y + 14, "LA 3060 PINTA TU PANTALLA ENTERA", VERDE, 2);
    let bien = matches!(r, Ok(t) if t.bien());
    match r {
        Ok(t) => {
            let mut l = Linea::nueva();
            l.t(b"").d(t.buenos as u64).t(b" fotogramas de ").d(t.ancho as u64).t(b"x").d(t.alto as u64).t(b", ");
            l.d(t.fps10() / 10).t(b".").d(t.fps10() % 10).t(b" por segundo; la 3060: ");
            l.d(t.gpu_us / t.pedidos.max(1) as u64 / 1000).t(b".").d(t.gpu_us / t.pedidos.max(1) as u64 / 100 % 10).t(b" ms por fotograma");
            p.texto_bytes(40, y + 54, &l.b[..l.n], if bien { CLARO } else { 0x00FF_5555 });
        }
        Err(m) => {
            p.texto_bytes(40, y + 54, super::super::iommu::motivo(m), 0x00FF_5555);
        }
    }
    // B1: el fotograma partido en sus trozos.
    if let Ok(t) = r {
        let (total, gpu, cpu, resto) = t.partido();
        let mut l = Linea::nueva();
        l.t(b"un fotograma: ").d(total).t(b" us = la 3060 ").d(gpu).t(b" + comprobar ").d(cpu);
        l.t(b" (").d(t.muestras / t.pedidos.max(1) as u64).t(b" muestras) + preparar y syscall ").d(resto);
        p.texto_bytes(40, y + 78, &l.b[..l.n], TENUE);
    }
    p.texto_bytes(40, y + 100, b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = true };
    let g = &mut dsk.out.grid;
    g.with_ink(if bien { INK_GOOD } else { INK_ERR });
    g.text(if bien {
        b"  LA 3060 PINTO LA PANTALLA ENTERA (M5d P): mira la fila `pantalla`\n" as &[u8]
    } else {
        b"  la 3060 no pinto la pantalla entera: mira la fila `pantalla`\n"
    });
    g.with_ink(INK_PLAIN);
    super::fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// La fila `pantalla`.
pub(super) fn fila(s: &mut Output, c: &super::Computo) {
    let Some(r) = c.pantalla else { return };
    campo(s, b"pantalla");
    let t = match r {
        Err(m) => return no(s, m),
        Ok(t) => t,
    };
    s.with_ink(if t.bien() { INK_GOOD } else { INK_ERR });
    s.text(if t.bien() { b"LA 3060 PINTA TU PANTALLA ENTERA: " as &[u8] } else { b"la pantalla entera NO salio: " });
    s.dec(t.buenos as u64);
    s.text(b" de ");
    s.dec(t.pedidos as u64);
    s.text(b" fotogramas de ");
    s.dec(t.ancho as u64);
    s.byte(b'x');
    s.dec(t.alto as u64);
    s.with_ink(INK_ECHO);
    s.text(b"; ");
    s.dec(t.fps10() / 10);
    s.byte(b'.');
    s.dec(t.fps10() % 10);
    s.text(b" fps, la 3060 en ");
    s.dec(t.gpu_us / t.pedidos.max(1) as u64);
    s.text(b" us por fotograma");
    // B1 (25-09): el fotograma partido, y cuantas muestras comprobo cada uno.
    let (total, gpu, cpu, resto) = t.partido();
    s.text(b"; partido: ");
    s.dec(total);
    s.text(b" us = 3060 ");
    s.dec(gpu);
    s.text(b" + comprobar ");
    s.dec(cpu);
    s.text(b" (");
    s.dec(t.muestras / t.pedidos.max(1) as u64);
    s.text(b" muestras) + preparar y syscall ");
    s.dec(resto);
    if let Some((f, r)) = t.malo {
        let (buenos, qmd, fin, _, _, _) = pa::desempaquetar(r);
        s.text(b"; el ");
        s.dec(f as u64);
        s.text(b": ");
        s.dec(buenos as u64);
        s.text(b" de 1024 muestras, semaforos ");
        s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::super::datos::anotar(b"gpu pantalla fps10", t.fps10(), b"");
    super::super::datos::anotar(b"gpu pantalla comprobar us", cpu, b"");
    super::super::datos::anotar(b"gpu pantalla resto us", resto, b"");
}
