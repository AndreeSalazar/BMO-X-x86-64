//! **LA FOTO** -- una maquetacion pintada en el ANFITRION con el pintor de
//! verdad (`bmo-pinta`, el mismo del escritorio y de las apps) y su letra.
//!
//! Es la mitad de MAQUETA que faltaba para que el navegador sea la REGLA
//! (`PLAN_MAQUETA.md`, seccion 1: "el navegador como REGLA, no como
//! destino"): las etiquetas son las de HTML y SVG, asi que el mismo
//! `.maqueta` se abre en un navegador; esto pinta lo que BMO-X pintara; y
//! el ESPEJO de cara (`toolchain/tools/espejo-cara`) compara las dos fotos.
//!
//! ```text
//!    foto(l)        la lista de trazos, pintada         (lo que el emisor A pinta)
//!    foto_cara(b)   la CARA que viaja, leida y pintada  (lo que el emisor B pinta)
//! ```
//!
//! Y las dos tienen que dar los MISMOS pixeles: es la prueba de que la cara
//! no pierde nada al viajar (`tests/cara_que_viaja.rs`).

use bmo_maqueta_layout::Laid;
use bmo_pinta::Lienzo;

use crate::orden::{lista, Estado, Trazo};

/// La letra de PIXEL de BMO-X, la MISMA tabla que el escritorio (generada por
/// `fontgen`).
static FONT16: [[u8; 16]; 120] = include!("../../../../../Ultra_userspace/userland/src/font16_data.rs");

/// Una imagen en memoria: `ancho * alto` pixeles `0x00RRGGBB`.
pub struct Foto {
    pub ancho: u32,
    pub alto: u32,
    pub px: Vec<u32>,
}

impl Lienzo for Foto {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        for j in y.max(0)..(y + h).min(self.alto as i32) {
            for i in x.max(0)..(x + w).min(self.ancho as i32) {
                self.px[(j as u32 * self.ancho + i as u32) as usize] = c;
            }
        }
    }
    fn mezclar(&mut self, x: i32, y: i32, c: u32, alfa: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.ancho && (y as u32) < self.alto {
            let k = (y as u32 * self.ancho + x as u32) as usize;
            self.px[k] = bmo_pinta::sobre(self.px[k], c, alfa);
        }
    }
}

impl Foto {
    fn nueva(ancho: u32, alto: u32) -> Foto {
        Foto { ancho, alto, px: vec![0; (ancho * alto) as usize] }
    }

    /// La letra de pixel, como `Pantalla::texto` (8 x 16, solo ASCII).
    fn texto(&mut self, x: i32, y: i32, s: &[u8], c: u32) {
        for (k, &ch) in s.iter().enumerate() {
            if !(32..=126).contains(&ch) {
                continue;
            }
            for (fila, &bits) in FONT16[ch as usize - 32].iter().enumerate() {
                for col in 0..8 {
                    if bits & (0x80 >> col) != 0 {
                        self.rect(x + k as i32 * 8 + col, y + fila as i32, 1, 1, c);
                    }
                }
            }
        }
    }
}

/// **La foto de una maquetacion**, en reposo.
pub fn foto(l: &Laid) -> Foto {
    let mut im = Foto::nueva(l.canvas.0, l.canvas.1);
    let mut letra = bmo_letra::Letra::nueva();
    for o in lista(&crate::desplaza::podar(l)).iter().filter(|o| o.estado == Estado::Reposo) {
        un_trazo(&mut im, &mut letra, &o.trazo);
    }
    // H7: lo de dentro de cada ventana, recortado y en su arranque (desde 0),
    // y su barra -- la misma cuenta que el aparato.
    for f in crate::desplaza::cajas(l) {
        desplazado(&mut im, &mut letra, f, l.canvas, 0);
    }
    im
}

/// **Una ventana desplazada `desde` pixeles** (H7): limpia, lo de dentro
/// corrido y recortado, y la barra.
pub fn desplazado(im: &mut Foto, letra: &mut bmo_letra::Letra, f: &bmo_maqueta_layout::Frame, lienzo: (u32, u32), desde: u32) {
    use crate::desplaza;
    let v = desplaza::ventana(f);
    bmo_pinta::caja(im, v.x, v.y, v.w as i32, v.h as i32, desplaza::radio(f), f.style.background.unwrap_or(0));
    let mut r = bmo_pinta::Recortado { dentro: im, x0: v.x, y0: v.y, x1: v.x + v.w as i32, y1: v.y + v.h as i32 };
    let desde = desde.min(desplaza::maximo(f));
    for o in desplaza::contenido(f, lienzo) {
        o.trazo.con_pieza(|p| bmo_pinta::pieza(&mut r, letra, p, 0, -(desde as i32)));
    }
    if let Some(b) = desplaza::barra(f, desde) {
        bmo_pinta::caja(im, b.x, b.y, b.w as i32, b.h as i32, (b.w / 2) as i32, desplaza::color_barra(f));
    }
}

fn un_trazo(im: &mut Foto, letra: &mut bmo_letra::Letra, t: &Trazo) {
    match t {
        Trazo::Rect { r, color } => im.rect(r.x, r.y, r.w as i32, r.h as i32, *color),
        Trazo::Texto { r, texto, color } => im.texto(r.x, r.y, texto.as_bytes(), *color),
        otro => {
            otro.con_pieza(|p| bmo_pinta::pieza(im, letra, p, 0, 0));
        }
    }
}

/// **La foto de una transicion a los `ms`** (P3b), pintada PAR a PAR con
/// `bmo_pinta::entre_piezas`: la misma mezcla que hace el escritorio. Es el
/// oraculo del codigo generado.
pub fn foto_en(pares: &[crate::movimiento::Par], lienzo: (u32, u32), ms: u32) -> Foto {
    let mut im = Foto::nueva(lienzo.0, lienzo.1);
    let mut letra = bmo_letra::Letra::nueva();
    for p in pares {
        let k = bmo_pinta::avance(ms, p.retraso, p.dura, p.curva);
        let llegada = k.clamp(0, 1000) >= 500;
        match (&p.a, &p.b) {
            (Some(Trazo::Texto { r: ra, texto, color: ca }), Some(Trazo::Texto { r: rb, color: cb, .. })) => {
                let (x, y) = (bmo_pinta::entre_i(ra.x, rb.x, k), bmo_pinta::entre_i(ra.y, rb.y, k));
                im.texto(x, y, texto.as_bytes(), bmo_pinta::entre_color(*ca, *cb, k));
            }
            (Some(a), Some(b)) => {
                let hecho = a.con_pieza_o_caja(|pa| b.con_pieza_o_caja(|pb| bmo_pinta::pieza(&mut im, &mut letra, &bmo_pinta::entre_piezas(pa, pb, k), 0, 0)));
                if hecho.flatten().is_none() {
                    un_trazo(&mut im, &mut letra, if llegada { b } else { a });
                }
            }
            (Some(a), None) if !llegada => un_trazo(&mut im, &mut letra, a),
            (None, Some(b)) if llegada => un_trazo(&mut im, &mut letra, b),
            _ => {}
        }
    }
    im
}

/// **La foto de una CARA que viaja** (version 2), leida con el lector que
/// desconfia y pintada trazo a trazo. `None` si la cara no se deja leer.
pub fn foto_cara(bytes: &[u8]) -> Option<Foto> {
    let c = bmo_maqueta_cara::leer(bytes, u16::MAX, u16::MAX).ok()?;
    let (w, h) = c.lienzo();
    let mut im = Foto::nueva(w as u32, h as u32);
    let mut letra = bmo_letra::Letra::nueva();
    for i in 0..c.trazos() {
        let p = c.trazo(i)?;
        if p.estado != bmo_maqueta_cara::ESTADO_REPOSO {
            continue;
        }
        if p.clase == bmo_maqueta_cara::CLASE_TEXTO {
            im.texto(p.x as i32, p.y as i32, p.texto, p.color);
        } else {
            bmo_pinta::pincelada(&mut im, &mut letra, &p, 0, 0);
        }
    }
    Some(im)
}
