//! **LA TRAYECTORIA** -- el instrumento de F12 ESTRATOS con el escritorio de
//! mision (HM6d de `docs/plan/PLAN_EL_HUD.md`, 07-10): *"versiones =
//! encendidos, marcadas = puntos de paso, ramas = trayectorias que se juntan
//! en la MEZCLA"*.
//!
//! [consumo] NADA      se pinta con la solapa `historial`, que solo se pinta
//!                     cuando algo cambia (L6h)
//!
//! ```text
//!    TRAYECTORIA  //  ESTRATOS                                  F12  rama
//!    VERSIONES  47     PUNTOS DE PASO  3     MEZCLAS  1     RAMA  principal
//!                                      .--------.
//!    o----o----O----o----o----o----o--'----o----@
//!              v1                          MEZCLA   ahora
//! ```
//!
//! Va en el sitio del encabezado de `historial` (la solapa solo se maneja con
//! el teclado: no hay zonas de raton que bajen con ella). La de ahora es la
//! nave (oro), las que llevan NOMBRE son puntos de paso (el aro del ojo, con su
//! nombre debajo), y una MEZCLA (dos padres, `estratos::dos_padres`) recibe
//! la otra trayectoria, en neon, que baja a juntarse con ella.
//!
//! ** Todo sale de la historia que la solapa YA leyo (`hist_releer` al
//! entrar): pintar no toca el disco.

use bmo_userland as bmo;

use super::hud::{lectura, marco, miles};
use super::tema_gen::{MISION_BORDE, MISION_CUIDADO, MISION_GO, MISION_NEON, MISION_OJO, MISION_TENUE, MISION_TINTA};

/// Lo que ocupa, con su aire de abajo.
pub(crate) const ALTO: u32 = 168;
/// Lo que hay de un encendido al siguiente en la linea.
const PASO: i32 = 44;

/// Lo que la trayectoria sabe de la version `i` (0 = la de ahora).
#[derive(Clone, Copy)]
pub(crate) struct Punto {
    pub(crate) nombre: bool,
    pub(crate) mezcla: bool,
}

/// **Pinta la trayectoria** de la historia ya leida en `caja`.
pub(crate) fn pintar(p: &bmo::Pantalla, caja: (u32, u32, u32, u32)) {
    let cuantas = bmo::estratos::hist_cuantas() as usize;
    let caben = (caja.2 as i32 - 64).max(PASO) as usize / PASO as usize + 1;
    let mut puntos = [Punto { nombre: false, mezcla: false }; 48];
    let ve = cuantas.min(caben).min(puntos.len());
    for (i, pt) in puntos.iter_mut().enumerate().take(ve) {
        *pt = Punto { nombre: bmo::estratos::hist_con_nombre(i as u64), mezcla: bmo::estratos::dos_padres(i as u64) };
    }
    let marcadas = (0..cuantas as u64).filter(|&i| bmo::estratos::hist_con_nombre(i)).count();
    let mezclas = (0..cuantas as u64).filter(|&i| bmo::estratos::dos_padres(i)).count();
    let mut rama = [0u8; 32];
    let mut largo = 0;
    for i in 0..16 {
        match bmo::estratos::rama(i, &mut rama) {
            Some((n, true)) => {
                largo = n.min(rama.len());
                break;
            }
            Some(_) => {}
            None => break,
        }
    }
    let mut nombres = [[0u8; 16]; 48];
    let mut largos = [0usize; 48];
    for i in 0..ve {
        if puntos[i].nombre {
            largos[i] = bmo::estratos::hist_nombre(i as u64, &mut nombres[i]).min(16);
        }
    }
    let nombre = |i: usize| -> &[u8] { &nombres[i][..largos[i]] };
    pintar_con(p, caja, cuantas, marcadas, mezclas, &rama[..largo], &puntos[..ve], &nombre);
}

/// La pintura con la historia ya contada (el banco del anfitrion la llama asi).
#[allow(clippy::too_many_arguments)]
pub(crate) fn pintar_con<'n>(
    p: &bmo::Pantalla,
    caja: (u32, u32, u32, u32),
    cuantas: usize,
    marcadas: usize,
    mezclas: usize,
    rama: &[u8],
    puntos: &[Punto],
    nombre: &dyn Fn(usize) -> &'n [u8],
) {
    let (x, y, w, _) = caja;
    marco(p, caja, b"TRAYECTORIA  //  ESTRATOS", b"F12");
    let mut b = [0u8; 24];
    if cuantas == 0 {
        lectura(p, x + 16, y + 30, b"VERSIONES", b"--", b"el volumen no monta, o no tiene ni un estrato", MISION_TENUE);
        return;
    }
    let n = miles(cuantas as u64, &mut b);
    lectura(p, x + 16, y + 30, b"VERSIONES", &b[..n], b"encendidos", MISION_TINTA);
    let n = miles(marcadas as u64, &mut b);
    lectura(p, x + 220, y + 30, b"PUNTOS DE PASO", &b[..n], b"con nombre", if marcadas > 0 { MISION_GO } else { MISION_TENUE });
    let n = miles(mezclas as u64, &mut b);
    lectura(p, x + 420, y + 30, b"MEZCLAS", &b[..n], b"", if mezclas > 0 { MISION_NEON } else { MISION_TENUE });
    if !rama.is_empty() && w > 760 {
        lectura(p, x + 560, y + 30, b"RAMA", rama, b"", MISION_OJO);
    }

    // -- la linea: la de ahora a la derecha, las de antes hacia la izquierda --
    let ly = (y + 112) as i32;
    let xd = (x + w) as i32 - 40;
    let px = |i: usize| xd - i as i32 * PASO;
    let ve = puntos.len();
    let xi = px(ve.saturating_sub(1));
    // Si hay mas atras de lo que cabe, la linea sigue hasta el marco.
    let desde = if cuantas > ve { (x + 16) as i32 } else { xi };
    p.rect(desde as u32, ly as u32, (xd - desde).max(0) as u32, 2, MISION_OJO);
    let r = p.recorte();
    let chica = bmo::Estilo::normal(10);
    for i in (0..ve).rev() {
        let (cx, pt) = (px(i), puntos[i]);
        if pt.mezcla {
            // La otra trayectoria llega desde arriba a la izquierda y se junta.
            let a = (cx - 3 * PASO, ly - 2);
            let d = (cx, ly);
            p.curva(&r, a, (a.0 + PASO, ly - 46), (cx - PASO, ly - 46), d, MISION_NEON);
            p.curva(&r, (a.0, a.1 + 1), (a.0 + PASO, ly - 45), (cx - PASO, ly - 45), (d.0, d.1 + 1), MISION_NEON);
            p.caja_redonda(cx - 2 * PASO + PASO / 2 - 5, ly - 41, 10, 10, 5, MISION_NEON);
            let l = p.medir(b"MEZCLA", chica);
            p.letra(cx - l / 2, ly + 34, b"MEZCLA", MISION_NEON, chica);
        }
        if i == 0 {
            // La nave: la de ahora, en oro.
            p.caja_redonda(cx - 9, ly - 8, 18, 18, 9, MISION_CUIDADO);
            p.letra(cx - 12, ly + 24, b"ahora", MISION_CUIDADO, chica);
        } else if pt.nombre {
            p.caja_redonda(cx - 8, ly - 7, 16, 16, 8, MISION_OJO);
            p.caja_redonda(cx - 5, ly - 4, 10, 10, 5, super::tema_gen::MISION_FONDO);
            let s = nombre(i);
            let l = p.medir(s, chica).min(PASO * 2 - 8);
            if !pt.mezcla {
                p.letra(cx - l / 2, ly + 24, s, MISION_GO, chica);
            }
        } else {
            p.caja_redonda(cx - 5, ly - 4, 10, 10, 5, MISION_BORDE);
            p.caja_redonda(cx - 3, ly - 2, 6, 6, 3, MISION_TENUE);
        }
    }
}
