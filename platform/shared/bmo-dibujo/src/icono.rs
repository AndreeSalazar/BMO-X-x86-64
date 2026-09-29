//! # Escalon 4 -- EL ICONO VECTORIAL: un SVG chico, sin SVG (2026-09-29)
//!
//! El propietario, viendo los iconos de 16x16 dibujados con `o` y `#`
//! agrandados al doble: *"investigar los mejores iconos y eso en svg ...
//! porque se ven feo"*. Tenia razon por una cuenta: un dibujo de 16 pixeles
//! agrandado a 32 son CUADROS de 2x2 -- escaleras en cada curva y en cada
//! diagonal. Un icono de verdad se DESCRIBE (formas) y se rasteriza a la medida
//! en que se pinta, con el borde a medias donde cae a medias.
//!
//! ## Lo que se aprendio mirando los mejores, y lo que se tomo
//!
//! Los juegos de iconos que mejor se ven en pantalla --Fluent UI System Icons
//! (Microsoft, los de Windows 11), Lucide, Phosphor, Material Symbols--
//! comparten cuatro reglas, y son las que sigue esto:
//!
//! 1. **Una rejilla de 24.** Todos dibujan en un `viewBox="0 0 24 24"` y se
//!    pintan a 16, 20, 24, 32 o 48. Aqui la rejilla es de 24 y las coordenadas
//!    van en CUARTOS de unidad (`0..96`), para poder decir 2,5 sin coma.
//! 2. **Pocas formas y gordas**: cajas redondeadas, circulos, poligonos y
//!    trazos con punta redonda (el `stroke-linecap="round"` de Lucide). Nada
//!    mas fino que una unidad de la rejilla.
//! 3. **Suavizado**: el borde a medias. Aqui, el MISMO muestreo que
//!    [`crate::triangulo_suave`] -- 4x4 muestras por pixel.
//! 4. **Dos o tres tonos del MISMO color** (el relleno de Fluent: claro arriba,
//!    oscuro abajo) en vez de un color plano; se lee como un objeto y no como
//!    una mancha.
//!
//! [!] Las FORMAS de cada icono son de BMO-X, dibujadas sobre esa rejilla: no
//! se copio ningun trazado de esos juegos (independencia, y sus licencias
//! piden nombrarlos). Lo que se tomo son las reglas de arriba.
//!
//! ## El color va por PAPELES, como en `iconos.rs`
//!
//! Un icono no lleva colores: lleva indices a una `paleta` que pone quien
//! pinta (el color de la clase, su claro, su oscuro, la luz de un disco...).
//! Un dibujo de disco, y cuatro discos de cuatro colores.
//!
//! ## Se mezcla contra un fondo CONOCIDO
//!
//! Como el triangulo suave: el framebuffer no se lee (write-combining), asi que
//! quien pinta dice sobre que color cae el icono, y las capas se componen de
//! abajo arriba sobre el. Solo se entregan los pixeles que alguna capa toca.

use crate::{mezclar, Color};

/// La rejilla: 24 unidades, en cuartos.
pub const VISTA: i32 = 96;

/// **Una forma**, en cuartos de la rejilla de 24.
#[derive(Clone, Copy, Debug)]
pub enum Figura {
    /// Caja de esquina `(x, y)`, medidas `w x h` y radio `r`.
    Caja { x: i32, y: i32, w: i32, h: i32, r: i32 },
    /// Circulo de centro `(x, y)` y radio `r`.
    Circulo { x: i32, y: i32, r: i32 },
    /// Poligono cerrado (par-impar).
    Poligono(&'static [(i32, i32)]),
    /// Linea quebrada de `ancho`, con puntas y codos redondos.
    Trazo { puntos: &'static [(i32, i32)], ancho: i32 },
}

/// **Con que se pinta una forma.** Los numeros son indices de la paleta.
#[derive(Clone, Copy, Debug)]
pub enum Tinta {
    Lisa(u8),
    /// De arriba (`.0`) abajo (`.1`) en lo que mide la forma.
    Degradado(u8, u8),
    /// El fondo: para RECORTAR (la esquina doblada de una hoja).
    Fondo,
}

/// Una capa: una forma y su tinta. El icono son capas de abajo arriba.
pub type Capa = (Figura, Tinta);

/// Las coordenadas de dentro: cuartos x 16, para que las muestras (4x4 por
/// pixel) caigan en enteros hasta a 96 px de lado.
const FINO: i64 = 16;

fn dentro(f: &Figura, u: i64, v: i64) -> bool {
    match *f {
        Figura::Caja { x, y, w, h, r } => {
            let (x0, y0, x1, y1, r) = (x as i64 * FINO, y as i64 * FINO, (x + w) as i64 * FINO, (y + h) as i64 * FINO, r as i64 * FINO);
            if u < x0 || u >= x1 || v < y0 || v >= y1 {
                return false;
            }
            let dx = (x0 + r - u).max(u - (x1 - r)).max(0);
            let dy = (y0 + r - v).max(v - (y1 - r)).max(0);
            dx * dx + dy * dy <= r * r
        }
        Figura::Circulo { x, y, r } => {
            let (dx, dy, r) = (u - x as i64 * FINO, v - y as i64 * FINO, r as i64 * FINO);
            dx * dx + dy * dy <= r * r
        }
        Figura::Poligono(p) => {
            let mut par = false;
            let n = p.len();
            for i in 0..n {
                let (ax, ay) = (p[i].0 as i64 * FINO, p[i].1 as i64 * FINO);
                let (bx, by) = (p[(i + 1) % n].0 as i64 * FINO, p[(i + 1) % n].1 as i64 * FINO);
                if (ay > v) != (by > v) {
                    // Donde la arista corta la horizontal de `v`, comparado sin dividir.
                    let (num, den) = ((bx - ax) * (v - ay), by - ay);
                    let cruza = if den > 0 { (u - ax) * den < num } else { (u - ax) * den > num };
                    if cruza {
                        par = !par;
                    }
                }
            }
            par
        }
        Figura::Trazo { puntos, ancho } => {
            let m = ancho as i64 * FINO / 2;
            let m2 = m * m;
            if puntos.len() == 1 {
                let (dx, dy) = (u - puntos[0].0 as i64 * FINO, v - puntos[0].1 as i64 * FINO);
                return dx * dx + dy * dy <= m2;
            }
            puntos.windows(2).any(|s| {
                let (ax, ay) = (s[0].0 as i64 * FINO, s[0].1 as i64 * FINO);
                let (bx, by) = (s[1].0 as i64 * FINO, s[1].1 as i64 * FINO);
                let (ex, ey, px, py) = (bx - ax, by - ay, u - ax, v - ay);
                let largo2 = ex * ex + ey * ey;
                let t = px * ex + py * ey;
                if largo2 == 0 || t <= 0 {
                    px * px + py * py <= m2
                } else if t >= largo2 {
                    let (qx, qy) = (u - bx, v - by);
                    qx * qx + qy * qy <= m2
                } else {
                    // La distancia a la recta: (cruz)^2 / largo2, sin dividir.
                    let cruz = px * ey - py * ex;
                    (cruz as i128) * (cruz as i128) <= (m2 as i128) * (largo2 as i128)
                }
            })
        }
    }
}

/// Lo que mide la forma en vertical (para el degradado), en unidades finas.
fn alto(f: &Figura) -> (i64, i64) {
    let (a, b) = match *f {
        Figura::Caja { y, h, .. } => (y as i64, (y + h) as i64),
        Figura::Circulo { y, r, .. } => ((y - r) as i64, (y + r) as i64),
        Figura::Poligono(p) => (p.iter().map(|q| q.1).min().unwrap_or(0) as i64, p.iter().map(|q| q.1).max().unwrap_or(0) as i64),
        Figura::Trazo { puntos, ancho } => (
            (puntos.iter().map(|q| q.1).min().unwrap_or(0) - ancho / 2) as i64,
            (puntos.iter().map(|q| q.1).max().unwrap_or(0) + ancho / 2) as i64,
        ),
    };
    (a * FINO, b * FINO)
}

fn color(t: Tinta, paleta: &[Color], fondo: Color, v: i64, (a, b): (i64, i64)) -> Option<Color> {
    Some(match t {
        Tinta::Lisa(k) => *paleta.get(k as usize)?,
        Tinta::Fondo => fondo,
        Tinta::Degradado(k0, k1) => {
            let (c0, c1) = (*paleta.get(k0 as usize)?, *paleta.get(k1 as usize)?);
            let (parte, total) = ((v - a).clamp(0, (b - a).max(1)), (b - a).max(1));
            mezclar(c1, c0, parte as u32, total as u32)
        }
    })
}

/// **Rasteriza un icono de `lado` x `lado` pixeles**: las `capas` de abajo
/// arriba sobre `fondo`, cada una con su cobertura de 16 muestras. Entrega
/// `pixel(x, y, color)` solo donde alguna capa toca. Un indice de paleta que no
/// existe deja esa capa sin pintar (no revienta: los dibujos son datos).
pub fn icono(capas: &[Capa], lado: u32, paleta: &[Color], fondo: Color, mut pixel: impl FnMut(u32, u32, Color)) {
    if lado == 0 {
        return;
    }
    const M: i64 = 4;
    let escala = VISTA as i64 * FINO;
    let den = lado as i64 * M * 2;
    let altos: [(i64, i64); 32] = {
        let mut a = [(0, 0); 32];
        for (k, c) in capas.iter().take(32).enumerate() {
            a[k] = alto(&c.0);
        }
        a
    };
    for j in 0..lado {
        for i in 0..lado {
            let mut acc = fondo;
            let mut tocado = false;
            for (k, c) in capas.iter().take(32).enumerate() {
                let mut n = 0u32;
                for b in 0..M {
                    let v = ((j as i64 * M + b) * 2 + 1) * escala / den;
                    for a in 0..M {
                        let u = ((i as i64 * M + a) * 2 + 1) * escala / den;
                        n += dentro(&c.0, u, v) as u32;
                    }
                }
                if n == 0 {
                    continue;
                }
                let centro = ((j as i64 * 2 + 1) * escala) / (lado as i64 * 2);
                if let Some(col) = color(c.1, paleta, fondo, centro, altos[k]) {
                    acc = mezclar(col, acc, n, (M * M) as u32);
                    tocado = true;
                }
            }
            if tocado {
                pixel(i, j, acc);
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pinta(capas: &[Capa], lado: u32, paleta: &[Color]) -> Vec<Option<Color>> {
        let mut v = vec![None; (lado * lado) as usize];
        icono(capas, lado, paleta, 0xFF00_0000, |x, y, c| v[(y * lado + x) as usize] = Some(c));
        v
    }

    const BLANCO: Color = 0xFFFF_FFFF;

    #[test]
    fn una_caja_que_llena_la_vista_es_toda_de_su_color() {
        let c = [(Figura::Caja { x: 0, y: 0, w: VISTA, h: VISTA, r: 0 }, Tinta::Lisa(0))];
        assert!(pinta(&c, 24, &[BLANCO]).iter().all(|p| *p == Some(BLANCO)));
    }

    #[test]
    fn lo_que_no_se_toca_no_se_entrega() {
        let c = [(Figura::Caja { x: 0, y: 0, w: 48, h: 96, r: 0 }, Tinta::Lisa(0))];
        let v = pinta(&c, 24, &[BLANCO]);
        for y in 0..24 {
            for x in 0..24 {
                assert_eq!(v[y * 24 + x].is_some(), x < 12, "({x},{y})");
            }
        }
    }

    /// Un circulo de radio 10 unidades (40 cuartos) en 24 px: el area pintada
    /// (la cobertura sumada) es pi*r^2 = 314 px, con el error del muestreo.
    #[test]
    fn el_circulo_mide_pi_r_cuadrado() {
        let c = [(Figura::Circulo { x: 48, y: 48, r: 40 }, Tinta::Lisa(0))];
        let v = pinta(&c, 24, &[BLANCO]);
        let area: u32 = v.iter().map(|p| p.map_or(0, |c| c & 0xFF)).sum::<u32>() / 255;
        assert!((306..=322).contains(&area), "area {area}");
        // Y el borde va a medias: hay pixeles que no son ni fondo ni blanco.
        assert!(v.iter().any(|p| matches!(p, Some(c) if *c & 0xFF > 0 && *c & 0xFF < 0xFF)));
    }

    #[test]
    fn medio_cuadrado_por_la_diagonal_es_la_mitad() {
        static T: [(i32, i32); 3] = [(0, 0), (96, 0), (0, 96)];
        let c = [(Figura::Poligono(&T), Tinta::Lisa(0))];
        let v = pinta(&c, 16, &[BLANCO]);
        let area: u32 = v.iter().map(|p| p.map_or(0, |c| c & 0xFF)).sum::<u32>();
        let total = 16 * 16 * 255;
        assert!((area * 100 / total).abs_diff(50) <= 2, "{}%", area * 100 / total);
    }

    #[test]
    fn el_degradado_va_de_arriba_abajo() {
        let c = [(Figura::Caja { x: 0, y: 0, w: 96, h: 96, r: 0 }, Tinta::Degradado(0, 1))];
        let v = pinta(&c, 32, &[0xFFFF_FFFF, 0xFF00_0000]);
        let arriba = v[0].unwrap() & 0xFF;
        let medio = v[16 * 32].unwrap() & 0xFF;
        let abajo = v[31 * 32].unwrap() & 0xFF;
        assert!(arriba > 240 && abajo < 15 && (110..=145).contains(&medio), "{arriba} {medio} {abajo}");
    }

    /// Un trazo horizontal de UNA unidad a 24 px cubre una fila y poco mas, y
    /// sus puntas son redondas: no llega a las esquinas de su caja.
    #[test]
    fn un_trazo_es_una_linea_con_puntas_redondas() {
        static P: [(i32, i32); 2] = [(16, 50), (80, 50)];
        let c = [(Figura::Trazo { puntos: &P, ancho: 4 }, Tinta::Lisa(0))];
        let v = pinta(&c, 24, &[BLANCO]);
        assert_eq!(v[12 * 24 + 12], Some(BLANCO), "el centro de la linea");
        assert_eq!(v[6 * 24 + 12], None, "lejos de la linea");
        assert!(v[12 * 24 + 3].is_some_and(|c| c != BLANCO), "la punta redonda, a medias");
        assert!(v[12 * 24 + 2].is_none(), "pasada la punta");
    }

    /// Las capas se componen: una encima tapa a la de abajo, y `Fondo` recorta.
    #[test]
    fn las_capas_de_arriba_tapan_y_el_fondo_recorta() {
        let c = [
            (Figura::Caja { x: 0, y: 0, w: 96, h: 96, r: 0 }, Tinta::Lisa(0)),
            (Figura::Caja { x: 0, y: 0, w: 48, h: 96, r: 0 }, Tinta::Lisa(1)),
            (Figura::Caja { x: 0, y: 0, w: 24, h: 24, r: 0 }, Tinta::Fondo),
        ];
        let v = pinta(&c, 8, &[BLANCO, 0xFF12_3456]);
        assert_eq!(v[7 * 8 + 7], Some(BLANCO));
        assert_eq!(v[7 * 8], Some(0xFF12_3456));
        assert_eq!(v[0], Some(0xFF00_0000), "recortado al fondo");
    }

    #[test]
    fn un_papel_que_no_esta_en_la_paleta_no_se_pinta() {
        let c = [(Figura::Circulo { x: 48, y: 48, r: 40 }, Tinta::Lisa(9))];
        assert!(pinta(&c, 16, &[BLANCO]).iter().all(|p| p.is_none()));
    }
}
