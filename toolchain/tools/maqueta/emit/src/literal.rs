//! **Una pieza escrita en Rust** (sacado de `rust.rs` en MAQUETA 3, 06-10,
//! cuando la figura de SVG lo dejaba a las puertas de las mil lineas).
//!
//! Lo que pinta el codigo generado es `bmo::Pieza` -- la `Pieza` de
//! `bmo-pinta`, que `bmo-userland` reexporta --, escrita como literal.

/// **Una pieza suave escrita en Rust**, para `bmo::Pieza` (la `Pieza` de
/// `bmo-pinta`, que `bmo-userland` reexporta).
pub fn pieza_literal(p: &bmo_pinta::Pieza) -> String {
    use bmo_pinta::Pieza;
    let caminos = |c: &[&[(i32, i32)]]| -> String {
        let v: Vec<String> = c
            .iter()
            .map(|s| format!("&[{}]", s.iter().map(|(x, y)| format!("({x}, {y})")).collect::<Vec<_>>().join(", ")))
            .collect();
        format!("&[{}]", v.join(", "))
    };
    match *p {
        Pieza::Caja { x, y, w, h, r, c } => format!("bmo::Pieza::Caja {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, c: 0x{c:08X} }}"),
        Pieza::Borde { x, y, w, h, r, grosor, c } => {
            format!("bmo::Pieza::Borde {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, grosor: {grosor}, c: 0x{c:08X} }}")
        }
        Pieza::Resplandor { x, y, w, h, r, alcance, argb } => {
            format!("bmo::Pieza::Resplandor {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, alcance: {alcance}, argb: 0x{argb:08X} }}")
        }
        Pieza::Degradado { x, y, w, h, r, de, a, vertical } => format!(
            "bmo::Pieza::Degradado {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, de: 0x{de:08X}, a: 0x{a:08X}, vertical: {vertical} }}"
        ),
        Pieza::Letra { x, y, alto, texto, c, px, peso, espacio, mayusculas } => format!(
            "bmo::Pieza::Letra {{ x: {x}, y: {y}, alto: {alto}, texto: b{:?}, c: 0x{c:08X}, px: {px}, peso: {peso}, espacio: {espacio}, mayusculas: {mayusculas} }}",
            String::from_utf8_lossy(texto)
        ),
        Pieza::Trazo { caminos: cs, cerrados, grosor64, c } => format!(
            "bmo::Pieza::Trazo {{ caminos: {}, cerrados: &{:?}, grosor64: {grosor64}, c: 0x{c:08X} }}",
            caminos(cs),
            cerrados
        ),
        Pieza::Relleno { caminos: cs, c } => format!("bmo::Pieza::Relleno {{ caminos: {}, c: 0x{c:08X} }}", caminos(cs)),
        // Los pixeles los pone quien sabe de donde salen (`llamada_con`): un
        // `static IMAGEN_n` o un dato.
        Pieza::Imagen { x, y, w, h, r, .. } => format!("bmo::Pieza::Imagen {{ x: {x}, y: {y}, w: {w}, h: {h}, r: {r}, px: {PX} }}"),
        // ** MAQUETA 3: la figura de SVG, con su tinta.
        Pieza::Figura { caminos: cs, cerrados, pluma, tinta, alfa, par_impar } => format!(
            "bmo::Pieza::Figura {{ caminos: {}, cerrados: &{:?}, pluma: {pluma}, tinta: {}, alfa: {alfa}, par_impar: {par_impar} }}",
            caminos(cs),
            cerrados,
            tinta_literal(&tinta)
        ),
    }
}

/// La tinta de una figura, en Rust.
fn tinta_literal(t: &bmo_pinta::Tinta) -> String {
    use bmo_pinta::Tinta;
    let paradas = |ps: &[bmo_pinta::Parada]| {
        let v: Vec<String> = ps.iter().map(|p| format!("bmo::Parada {{ en: {}, c: 0x{:08X}, alfa: {} }}", p.en, p.c, p.alfa)).collect();
        format!("&[{}]", v.join(", "))
    };
    match *t {
        Tinta::Liso(c) => format!("bmo::Tinta::Liso(0x{c:08X})"),
        Tinta::Lineal { de, a, paradas: ps } => format!("bmo::Tinta::Lineal {{ de: {de:?}, a: {a:?}, paradas: {} }}", paradas(ps)),
        Tinta::Radial { centro, eje_x, eje_y, paradas: ps } => {
            format!("bmo::Tinta::Radial {{ centro: {centro:?}, eje_x: {eje_x:?}, eje_y: {eje_y:?}, paradas: {} }}", paradas(ps))
        }
    }
}

/// Donde van los pixeles de una imagen en su literal.
pub const PX: &str = "__PIXELES__";

