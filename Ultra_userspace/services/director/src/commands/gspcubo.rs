//! **`gpu cubo`: EL CUBO DEL ESTUDIO D3D, POR CPU (X4, 25-09)** -- el juez de
//! `bmo-cubo` (el `cubo-neutro` del estudio de Direct3D, traido) dibuja un
//! fotograma del cubo en el Ryzen y se compara, por su huella, con lo que
//! D3D12 dibujo en la 3060 bajo Windows. Si cuadra, BMO-X tiene el JUEZ del
//! cubo: la imagen exacta contra la que se medira la 3060 dibujandolo sin
//! Windows (X5). Por que existe: `docs/plan/PLAN_EL_CUBO.md`.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: un fotograma de
//!                     1280x720 por CPU (unos ms) y una huella
//!
//! ```text
//!    gpu cubo        el fotograma 30
//!    gpu cubo 60     otro (con huella de la 3060: 0, 30 y 60; los demas se
//!                    dibujan, pero no hay contra que medirlos)
//!    gpu cubo 3060 [N]   X5: el mismo, dibujado por la 3060 SIN Windows
//!                        (`gspcubo/la3060.rs`)
//! ```

/// X5: el cubo por la 3060, sin Windows.
mod la3060;
/// VERRANO V0: el cubo por la API, con sus dos backends.
mod verrano;
/// VERRANO V1: el tablero del banco, en vivo.
mod tablero;
/// VERRANO: la UNICA puerta a la RTX 3060 12G (SM86). Ver PLAN_EL_AISLAMIENTO.
mod sm86;
pub(crate) use verrano::orden as orden_verrano;

use bmo_cubo::referencia as rf;
use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

const FONDO: u32 = 0x0010_1018;
const VERDE: u32 = 0x0055_FF88;
const ROJO: u32 = 0x00FF_5555;
const CLARO: u32 = 0x00DD_DDDD;
const TENUE: u32 = 0x0088_8888;

/// Un texto con numeros, sin `alloc`.
struct Texto {
    b: [u8; 256],
    n: usize,
}

impl Texto {
    fn nuevo() -> Self {
        Texto { b: [0; 256], n: 0 }
    }
    fn t(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }
    fn d(&mut self, mut v: u64) -> &mut Self {
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        loop {
            i -= 1;
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.t(&tmp[i..])
    }
    fn x(&mut self, v: u64) -> &mut Self {
        let h = b"0123456789abcdef";
        let mut tmp = [0u8; 18];
        tmp[0] = b'0';
        tmp[1] = b'x';
        for k in 0..16 {
            tmp[2 + k] = h[(v >> (60 - 4 * k)) as usize & 0xF];
        }
        self.t(&tmp)
    }
    fn s(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

/// Para escribir un `Display` (el BODRIO del juez) sin `alloc`.
impl core::fmt::Write for Texto {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.t(s.as_bytes());
        Ok(())
    }
}

/// `gpu cubo [fotograma]`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, resto: &[u8]) -> After {
    if let Some(r) = resto.trim_ascii().strip_prefix(b"3060") {
        if r.is_empty() || r[0] == b' ' {
            return la3060::orden(dsk, p, r);
        }
    }
    let f = numero(resto).unwrap_or(30);
    paint_status(p, &dsk.run_box, "el cubo del estudio D3D, dibujado por la CPU", INK_DIM);
    let (w, h) = (rf::ANCHO, rf::ALTO);
    let bytes = w as u64 * h as u64 * 4;
    let Some(bloque) = bmo::Memoria::request(bytes) else {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ERR);
        g.text(b"  NO  sin memoria para un fotograma de 1280x720\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    };
    // SAFETY: el bloque mide `bytes` (w*h palabras de 32 bits), alineado a
    // pagina, es de este proceso y solo se usa aqui.
    let px = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut u32, (w * h) as usize) };
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    bmo_cubo::dibujar_por_cpu(bmo_cubo::angulo_de_fotograma(f), w, h, px);
    let us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    let huella = rf::huella(px);
    let esperada = rf::de_la_3060(f);
    let igual = esperada == Some(huella);

    // A la pantalla, centrado, y el veredicto debajo.
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    let (x0, y0) = (p.ancho.saturating_sub(w) / 2, p.alto.saturating_sub(h) / 2);
    if p.ancho >= w && p.alto >= h {
        p.marcar(x0, y0, w, h);
        for y in 0..h {
            for x in 0..w {
                p.punto_ya_marcado(x0 + x, y0 + y, px[(y * w + x) as usize] & 0x00FF_FFFF);
            }
        }
    }
    p.texto_escala(40, 24, "EL CUBO DEL ESTUDIO D3D, POR LA CPU DE BMO-X", CLARO, 2);
    let mut t = Texto::nuevo();
    t.t(b"fotograma ").d(f as u64).t(b": ").d(us).t(b" us de CPU; huella ").x(huella);
    let y = p.alto.saturating_sub(96);
    p.texto_bytes(40, y, t.s(), CLARO);
    let (veredicto, color): (&[u8], u32) = match esperada {
        Some(_) if igual => (b"IGUAL, bit a bit, a lo que D3D12 dibujo en la RTX 3060 bajo Windows", VERDE),
        Some(_) => (b"DISTINTO de la captura de D3D12 en la 3060: el juez no dibujo lo mismo", ROJO),
        None => (b"sin captura de la 3060 para este fotograma (las hay del 0, el 30 y el 60)", TENUE),
    };
    p.texto_bytes(40, y + 24, veredicto, color);
    p.texto_bytes(40, p.alto.saturating_sub(40), b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    super::gspcomputo::abrir_panel();

    let g = &mut dsk.out.grid;
    g.with_ink(if igual { INK_GOOD } else if esperada.is_some() { INK_ERR } else { INK_PLAIN });
    g.text(b"  cubo: ");
    g.text(t.s());
    g.text(b"\n        ");
    g.text(veredicto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    super::datos::anotar(b"gpu cubo us", us, b"");
    dsk.field.n = 0;
    After::Settle
}

fn numero(s: &[u8]) -> Option<u32> {
    let s = s.trim_ascii();
    if s.is_empty() || s.len() > 4 {
        return None;
    }
    s.iter().try_fold(0u32, |a, &c| c.is_ascii_digit().then(|| a * 10 + (c - b'0') as u32))
}
