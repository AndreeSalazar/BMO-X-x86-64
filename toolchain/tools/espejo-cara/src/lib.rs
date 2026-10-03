//! # ESPEJO DE CARA -- la app contra su maqueta, medido y no a ojo
//!
//! El propietario (03-10): *"HTML y CSS, esos dos son motivos: me gustaria
//! que mi BMO-X refleje las maquetas que hiciste... que sea IGUAL"*. Igual se
//! mide: el ESPEJO de COBOL ejecuta el mismo programa en BMO y en GnuCOBOL y
//! compara lo que imprimen; este pinta la MISMA pantalla en la app (con su
//! codigo de verdad, en el anfitrion) y en el navegador (la maqueta), y
//! compara los pixeles.
//!
//! ```text
//!    foto.js           la maqueta en el navegador -> maqueta.png (tu PC)
//!    cara-bankcat      BANK CAT pintado por SU codigo -> app.png
//!    cara-hermes       HERMES, igual
//!    espejo-cara comparar maqueta.png app.png [--mapa diff.png]
//!                      cuanto se parecen, DONDE no, y el mapa en rojo
//!    espejo-cara tinta maqueta.html [--comprobar tinta.rs]
//!                      la paleta (`:root { --oro: #FFD45E }`) hecha
//!                      constantes de Rust: UN sitio para los colores
//! ```
//!
//! ## La cuenta
//!
//! - **igual**: los pixeles cuya peor diferencia de canal es <= 24 (de 255).
//!   Es lo que el ojo no distingue en una pantalla oscura.
//! - **parecido**: lo mismo, comparando cada pixel con el mejor de sus
//!   vecinos (un pixel de corrimiento no es una diferencia de dibujo, es de
//!   redondeo).
//! - Las ZONAS: la ventana en una rejilla de 8 x 6, y las peores primero.
//!   Eso dice donde trabajar.

pub mod png;

/// Una imagen: `ancho * alto` pixeles `0x00RRGGBB`.
#[derive(Clone, Debug)]
pub struct Imagen {
    pub ancho: usize,
    pub alto: usize,
    pub px: Vec<u32>,
}

/// La diferencia de dos colores: la del canal que mas difiere.
pub fn distancia(a: u32, b: u32) -> u32 {
    let c = |s: u32| ((a >> s & 255) as i32 - (b >> s & 255) as i32).unsigned_abs();
    c(16).max(c(8)).max(c(0))
}

/// Lo que se tolera sin llamarlo distinto.
pub const TOLERANCIA: u32 = 24;

/// Lo que salio de comparar.
#[derive(Clone, Debug)]
pub struct Informe {
    pub ancho: usize,
    pub alto: usize,
    /// Por diezmilesimas: 10000 es todo.
    pub igual: u32,
    pub parecido: u32,
    /// Las zonas (8 x 6): (columna, fila, parecido en diezmilesimas).
    pub zonas: Vec<(usize, usize, u32)>,
    /// La diferencia de cada pixel (con vecinos), para el mapa.
    pub dif: Vec<u32>,
}

pub const COLUMNAS: usize = 8;
pub const FILAS: usize = 6;

/// **Compara** la maqueta con la app, en lo que las dos cubren.
pub fn comparar(maqueta: &Imagen, app: &Imagen) -> Informe {
    let (w, h) = (maqueta.ancho.min(app.ancho), maqueta.alto.min(app.alto));
    let m = |x: usize, y: usize| maqueta.px[y * maqueta.ancho + x];
    let a = |x: usize, y: usize| app.px[y * app.ancho + x];
    let (mut igual, mut parecido) = (0u64, 0u64);
    let mut dif = vec![0u32; w * h];
    let mut zona = vec![(0u64, 0u64); COLUMNAS * FILAS];
    for y in 0..h {
        for x in 0..w {
            let d = distancia(m(x, y), a(x, y));
            if d <= TOLERANCIA {
                igual += 1;
            }
            // El mejor de los vecinos de la app (un pixel de corrimiento).
            let mut mejor = d;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                    mejor = mejor.min(distancia(m(x, y), a(nx as usize, ny as usize)));
                }
            }
            dif[y * w + x] = mejor;
            let z = &mut zona[(y * FILAS / h) * COLUMNAS + x * COLUMNAS / w];
            z.1 += 1;
            if mejor <= TOLERANCIA {
                parecido += 1;
                z.0 += 1;
            }
        }
    }
    let n = (w * h).max(1) as u64;
    let mut zonas: Vec<(usize, usize, u32)> = zona
        .iter()
        .enumerate()
        .map(|(k, &(bien, total))| (k % COLUMNAS, k / COLUMNAS, (bien * 10000 / total.max(1)) as u32))
        .collect();
    zonas.sort_by_key(|z| z.2);
    Informe { ancho: w, alto: h, igual: (igual * 10000 / n) as u32, parecido: (parecido * 10000 / n) as u32, zonas, dif }
}

/// **El mapa**: la app apagada, y en rojo (mas rojo cuanto mas difiere) lo
/// que no es como la maqueta.
pub fn mapa(app: &Imagen, inf: &Informe) -> Imagen {
    let (w, h) = (inf.ancho, inf.alto);
    let mut px = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            let c = app.px[y * app.ancho + x];
            let gris = ((c >> 16 & 255) + (c >> 8 & 255) + (c & 255)) / 3 / 3;
            let d = inf.dif[y * w + x];
            px[y * w + x] = if d > TOLERANCIA {
                let r = (96 + d * 159 / 255).min(255);
                r << 16 | (gris / 2) << 8 | gris / 2
            } else {
                gris << 16 | gris << 8 | gris
            };
        }
    }
    Imagen { ancho: w, alto: h, px }
}

/// Un porcentaje con dos decimales, de diezmilesimas.
pub fn por_ciento(d: u32) -> String {
    format!("{}.{:02} %", d / 100, d % 100)
}

// ---------------------------------------------------------------------------
// LA TINTA: la paleta de la maqueta, hecha Rust
// ---------------------------------------------------------------------------

/// Los colores de `:root { --nombre: #RRGGBB; }`, en su orden.
pub fn paleta(html: &str) -> Vec<(String, u32)> {
    let Some(ini) = html.find(":root") else { return Vec::new() };
    let resto = &html[ini..];
    let Some(abre) = resto.find('{') else { return Vec::new() };
    let Some(cierra) = resto.find('}') else { return Vec::new() };
    let mut v = Vec::new();
    for decl in resto[abre + 1..cierra].split(';') {
        let Some((k, val)) = decl.split_once(':') else { continue };
        let (k, val) = (k.trim(), val.trim());
        let Some(nombre) = k.strip_prefix("--") else { continue };
        let Some(hex) = val.strip_prefix('#') else { continue };
        if hex.len() != 6 {
            continue;
        }
        let Ok(c) = u32::from_str_radix(hex, 16) else { continue };
        v.push((nombre.to_uppercase().replace('-', "_"), c));
    }
    v
}

/// **El fichero `tinta.rs`** de una app, generado de su maqueta.
pub fn tinta(html: &str, de: &str) -> String {
    let mut s = String::new();
    s.push_str("//! **LA TINTA** -- la paleta de la maqueta, hecha constantes.\n//!\n");
    s.push_str(&format!("//! GENERADO por `espejo-cara tinta {de}`: NO se edita a mano.\n"));
    s.push_str("//! Un color nuevo va en el `:root` de la maqueta y se regenera; asi la\n");
    s.push_str("//! maqueta y la app no pueden decir dos colores distintos.\n\n");
    s.push_str("#![allow(dead_code)]\n\nuse bmo_dibujo::Color;\n\n");
    for (k, c) in paleta(html) {
        s.push_str(&format!("pub const {k}: Color = 0x00{:02X}_{:04X};\n", c >> 16, c & 0xFFFF));
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_paleta_sale_del_root() {
        let h = "<style>:root {\n  --negro: #05060A;\n  --oro2: #C98A1B;\n  --f-texto: \"IBM\";\n}\n.x{color:#fff}</style>";
        assert_eq!(paleta(h), vec![("NEGRO".to_string(), 0x05060A), ("ORO2".to_string(), 0xC98A1B)]);
        assert!(tinta(h, "x.html").contains("pub const ORO2: Color = 0x00C9_8A1B;"));
    }

    #[test]
    fn un_pixel_corrido_es_parecido_y_no_igual() {
        let mut a = Imagen { ancho: 10, alto: 10, px: vec![0; 100] };
        let mut b = a.clone();
        a.px[5 * 10 + 4] = 0xFFFFFF;
        b.px[5 * 10 + 5] = 0xFFFFFF;
        let i = comparar(&a, &b);
        assert_eq!(i.igual, 9800);
        assert_eq!(i.parecido, 10000);
    }
}
