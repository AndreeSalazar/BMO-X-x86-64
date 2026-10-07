//! **EL INICIO DE MISION** -- la entrada a Ring 3 con `fondo = mision` (HM3b de
//! `docs/plan/PLAN_EL_HUD.md`, 07-10).
//!
//! [consumo] NADA      pinta UNA vez, al tomar la maquina; despues el
//!                     escritorio la tapa (L6h)
//!
//! Es la pantalla INICIO de `docs/arte/maqueta_escritorio_mision.html` en la
//! CPU: la NEBULOSA quieta en el sitio del sol y la ENCUESTA GO / NO-GO. Dos
//! fotogramas, sin reloj: este, y el escritorio, que trae el SOL -- la
//! nebulosa "se condensa" en el instante en que el escritorio esta listo. Viva,
//! con la 3060, es HM3c.
//!
//! ```text
//!    ENCUESTA  //  GO / NO-GO
//!    KERNEL     ..........  GO       ring 3 tiene la maquina
//!    MEMORIA    ..........  GO       1.234 MiB libres
//!    ...
//!    LA 3060    ..........  ESPERA   la despierta `save mode`
//! ```
//!
//! ** Cada GO sale de lo que el kernel YA mide (`bmo::info`) o de lo que este
//! proceso acaba de recibir, nunca de un reloj ni de un "seguro que si". Lo
//! que es opcional (la red, el sonido, la 3060) dice ESPERA en ambar, no
//! NO-GO: no tenerlo no es un fallo. NO-GO es lo que el escritorio necesita y
//! no llego -- y entonces, como la entrada de siempre (`splash`), se deja leer.

use bmo_userland as bmo;

use super::tema_gen::{MISION_BORDE, MISION_CUIDADO, MISION_FONDO, MISION_GO, MISION_NOGO, MISION_OJO, MISION_TENUE, MISION_TINTA};
use super::{mision, nebulosa_gen};

/// Como contesta un sistema a la encuesta.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Voto {
    Go,
    /// Opcional y no esta (todavia): no es un fallo.
    Espera,
    NoGo,
}

/// Un renglon de la encuesta: el sistema, su voto y el porque.
pub(crate) struct Renglon {
    nombre: &'static [u8],
    voto: Voto,
    nota: [u8; 40],
    largo: usize,
}

impl Renglon {
    pub(crate) fn nuevo(nombre: &'static [u8], voto: Voto, nota: &[&[u8]]) -> Self {
        let mut r = Renglon { nombre, voto, nota: [0; 40], largo: 0 };
        for trozo in nota {
            for &c in trozo.iter() {
                if r.largo < r.nota.len() {
                    r.nota[r.largo] = c;
                    r.largo += 1;
                }
            }
        }
        r
    }
}

/// `n` en decimal, con el punto de los miles.
fn miles(n: u64, b: &mut [u8; 16]) -> usize {
    let mut d = [0u8; 20];
    let mut k = 0;
    let mut v = n;
    loop {
        d[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let mut o = 0;
    for i in (0..k).rev() {
        if o < b.len() {
            b[o] = d[i];
            o += 1;
        }
        if i > 0 && i % 3 == 0 && o < b.len() {
            b[o] = b'.';
            o += 1;
        }
    }
    o
}

/// **La encuesta**: lo que el kernel mide, y lo que este proceso recibio.
fn encuesta(has_input: bool, has_console: bool) -> [Renglon; 9] {
    let mut b = [0u8; 16];
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let n = miles(libre / (1024 * 1024), &mut b);
    let memoria = if libre > 0 { Renglon::nuevo(b"MEMORIA", Voto::Go, &[&b[..n], b" MiB libres"]) } else { Renglon::nuevo(b"MEMORIA", Voto::NoGo, &[b"el kernel no da la cuenta"]) };
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let n = miles(hz / 1_000_000, &mut b);
    let reloj = if hz > 0 { Renglon::nuevo(b"RELOJ", Voto::Go, &[&b[..n], b" MHz medidos"]) } else { Renglon::nuevo(b"RELOJ", Voto::NoGo, &[b"sin frecuencia medida"]) };
    let si = |c: bool, n: &'static [u8], bien: &'static [u8], mal: &'static [u8], falta: Voto| if c { Renglon::nuevo(n, Voto::Go, &[bien]) } else { Renglon::nuevo(n, falta, &[mal]) };
    let gpu = bmo::info(bmo::INFO_IOMMU_GPU);
    [
        Renglon::nuevo(b"KERNEL", Voto::Go, &[b"ring 3 tiene la maquina"]),
        memoria,
        reloj,
        si(bmo::info(bmo::INFO_DISCO_LISTO) != 0, b"DISCO", b"listo", b"no contesta", Voto::NoGo),
        si(bmo::info(bmo::INFO_DATOS_MONTADO) != 0, b"ESTRATOS", b"volumen de datos montado", b"sin volumen de datos", Voto::NoGo),
        si(has_input, b"ENTRADA", b"teclado y raton", b"el escritorio sera mudo", Voto::NoGo),
        si(has_console, b"CONSOLA", b"lo que lance escribe aqui", b"los hijos escriben en el kernel", Voto::NoGo),
        si(bmo::info(bmo::INFO_NET_PRESENTE) != 0, b"RED", b"tarjeta presente", b"sin tarjeta", Voto::Espera),
        si(gpu & bmo::IOMMU_GPU_TRADUCIDA != 0, b"LA 3060", b"traducida y lista", b"la despierta `save mode`", Voto::Espera),
    ]
}

/// **Pinta el INICIO.** Devuelve si todo lo NECESARIO voto GO: si no, quien
/// llama deja leerlo, como la entrada de siempre.
pub(crate) fn pintar(p: &bmo::Pantalla, has_input: bool, has_console: bool) -> bool {
    pintar_con(p, &encuesta(has_input, has_console))
}

/// La pintura, con la encuesta ya hecha: lo que el banco del anfitrion pinta
/// con una encuesta de muestra (alli no hay kernel al que preguntar).
pub(crate) fn pintar_con(p: &bmo::Pantalla, renglones: &[Renglon]) -> bool {
    let (w, h) = (p.ancho, p.alto);
    if w == 0 || h == 0 {
        return true;
    }
    mision::cielo_solo(p);
    let (cx, cy) = mision::centro(w, h);
    nebulosa_gen::pintar(p, cx.saturating_sub(nebulosa_gen::ANCHO / 2), cy.saturating_sub(nebulosa_gen::ALTO / 2));
    mision::angulos(p);

    // La encuesta, a la izquierda, en su marco del HUD (el de `hud/marco.maqueta`).
    let rotulo = bmo::Estilo::media(13).espaciado(220).mayusculas();
    let linea = bmo::Estilo::normal(15);
    let nota = bmo::Estilo::normal(13);
    let (x0, alto) = ((w / 16).max(24), 36u32);
    let caja = (x0, h / 2 - (renglones.len() as u32 * alto) / 2 - 64, 620u32.min(w.saturating_sub(2 * x0)), renglones.len() as u32 * alto + 96);
    marco(p, caja);
    let (x, mut y) = (caja.0 + 24, caja.1 + 24);
    p.letra(x as i32, y as i32, b"ENCUESTA  //  GO / NO-GO", MISION_OJO, rotulo);
    y += 48;
    let mut todo = true;
    for r in renglones {
        p.letra(x as i32, y as i32, r.nombre, MISION_TINTA, linea);
        // La raya de puntos hasta el voto, como en la maqueta.
        let mut px = x + 120;
        while px < x + 230 {
            p.rect(px, y.saturating_sub(4), 2, 1, MISION_BORDE);
            px += 6;
        }
        let (color, voto): (u32, &[u8]) = match r.voto {
            Voto::Go => (MISION_GO, b"GO"),
            Voto::Espera => (MISION_CUIDADO, b"ESPERA"),
            Voto::NoGo => (MISION_NOGO, b"NO-GO"),
        };
        todo &= r.voto != Voto::NoGo;
        p.letra((x + 244) as i32, y as i32, voto, color, linea);
        p.letra((x + 330) as i32, (y + 2) as i32, &r.nota[..r.largo], MISION_TENUE, nota);
        y += alto;
    }
    // El lema, bajo la estrella.
    let lema: &[u8] = if todo { b"la nebulosa se junta..." } else { b"falta algo: una tecla para entrar" };
    let e = bmo::Estilo::normal(15).espaciado(260);
    let largo = p.medir(lema, e).max(0) as u32;
    p.letra(cx.saturating_sub(largo / 2) as i32, (cy + 300).min(h.saturating_sub(40)) as i32, lema, if todo { MISION_TENUE } else { MISION_CUIDADO }, e);
    todo
}

/// El marco de instrumento: el panel, su raya y las cuatro esquinas en L.
fn marco(p: &bmo::Pantalla, (x, y, w, h): (u32, u32, u32, u32)) {
    p.rect(x, y, w, h, MISION_BORDE);
    p.rect(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2), MISION_FONDO);
    const L: u32 = 14;
    for (ex, ey, hx, vy) in [(x, y, x, y), (x + w - L, y, x + w - 2, y), (x, y + h - 2, x, y + h - L), (x + w - L, y + h - 2, x + w - 2, y + h - L)] {
        p.rect(ex, ey, L, 2, MISION_OJO);
        p.rect(hx, vy, 2, L, MISION_OJO);
    }
}
