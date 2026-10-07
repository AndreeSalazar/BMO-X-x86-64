//! **LA TABLA DE FIGURAS** (`src/tabla.rs`, 06-10): un dibujo grande se
//! escribe UNA vez, en `static FIGURAS`, y no figura a figura dos veces.

use bmo_maqueta_emit::rust;
use bmo_maqueta_layout::Laid;

/// Una maqueta con un `<svg>` de `n` circulos, compilada desde un fichero.
fn circulos(n: usize) -> Laid {
    let mut svg = String::new();
    for k in 0..n {
        svg.push_str(&format!("<circle cx=\"{}\" cy=\"12\" r=\"2\" fill=\"#5EF2E6\"/>", 4 + k * 5));
    }
    let texto = format!(
        "<maqueta>\n  <style>\n    .d {{ width:{w}px; height:24px }}\n  </style>\n  <div>\n    <svg class=\"d\" viewBox=\"0 0 {w} 24\">{svg}</svg>\n  </div>\n</maqueta>\n",
        w = 8 + n * 5
    );
    let dir = std::env::temp_dir().join(format!("bmo-tabla-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("circulos.maqueta");
    std::fs::write(&ruta, texto).unwrap();
    bmo_maqueta_compone::compilar(&ruta).unwrap_or_else(|f| panic!("{}", f.render()))
}

#[test]
fn un_dibujo_grande_va_a_una_tabla_que_recorren_los_dos_pintados() {
    let m = rust::modulo("pruebas/circulos.maqueta", &circulos(12));
    assert!(m.contains("static FIGURAS: [(bmo::Pieza<'static>, [i32; 4]); 12] = ["), "la tabla, con las doce");
    assert_eq!(m.matches("for f in &FIGURAS[0..12] {").count(), 2, "`pintar` y `pintar_en`");
    assert!(m.contains("p.pieza(&f.0, ox as i32, oy as i32, None);"), "`pintar`, sin recorte");
    assert!(m.contains("p.pieza(&f.0, ox as i32, oy as i32, Some(limite));"), "`pintar_en`, recortado");
    assert!(m.contains("Recorte::nuevo(ox as i32 + f.1[0], oy as i32 + f.1[1], f.1[2], f.1[3])"), "solo las que tocan el recorte");
    // Cada figura sale UNA vez: en la tabla.
    assert_eq!(m.matches("bmo::Pieza::Figura {").count(), 12);
}

#[test]
fn un_dibujo_chico_se_escribe_como_siempre() {
    let m = rust::modulo("pruebas/circulos.maqueta", &circulos(3));
    assert!(!m.contains("FIGURAS"), "tres figuras no compensan una tabla");
    assert_eq!(m.matches("bmo::Pieza::Figura {").count(), 6, "cada una en `pintar` y en `pintar_en`");
}

#[test]
fn el_sol_del_escritorio_cabe_en_su_tabla() {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../escritorio/sol.maqueta");
    let l = bmo_maqueta_compone::compilar(&ruta).unwrap_or_else(|f| panic!("{}", f.render()));
    let m = rust::modulo("toolchain/tools/maqueta/escritorio/sol.maqueta", &l);
    assert!(m.contains("for f in &FIGURAS[0..225] {"), "las 225 figuras del SOL DE PLASMA, en un bucle");
    assert!(m.lines().count() < 1000, "y el modulo no pasa de mil lineas: {}", m.lines().count());
}
