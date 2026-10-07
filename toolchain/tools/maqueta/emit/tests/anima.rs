//! **S7: los dibujos que animan, escritos** (MAQUETA 3, 06-10).

use bmo_maqueta_emit::{anima, foto, rust};
use bmo_maqueta_layout::Laid;

fn compilar(nombre: &str) -> Laid {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pruebas").join(nombre);
    bmo_maqueta_compone::compilar(&ruta).unwrap_or_else(|f| panic!("{}", f.render()))
}

#[test]
fn una_maqueta_con_dibujos_que_animan_trae_su_reproductor() {
    let l = compilar("anima.maqueta");
    assert!(anima::hay(&l));
    let m = rust::modulo("pruebas/anima.maqueta", &l);
    assert!(m.contains("pub const ANIMA_MS: u32 = 2000;"), "el ciclo mas largo");
    assert!(m.contains("pub fn pintar_anima(p: &bmo::Pantalla, ox: u32, oy: u32, ms: u32)"));
    assert!(m.contains("pub fn pintar_en_fijo("), "lo de debajo, sin los dibujos");
    assert!(m.contains("p.pieza_entre(&PASOS[k][f], &PASOS[k + 1][f]"), "dos pasos mezclados");
    // Tres dibujos que animan: tres reproductores.
    assert_eq!(m.matches("fn anima_").count(), 3);
}

#[test]
fn sin_dibujos_que_animen_el_modulo_es_el_de_siempre() {
    let l = compilar("dibujos.maqueta");
    assert!(!anima::hay(&l));
    let m = rust::modulo("pruebas/dibujos.maqueta", &l);
    assert!(!m.contains("ANIMA_MS") && !m.contains("pintar_en_fijo"));
}

#[test]
fn el_primer_instante_es_la_foto_de_reposo_y_luego_se_mueve() {
    let l = compilar("anima.maqueta");
    let reposo = foto::foto(&l);
    let en_0 = foto::foto_anima(&l, 0);
    let distintos = reposo.px.iter().zip(&en_0.px).filter(|(a, b)| a != b).count();
    assert_eq!(distintos, 0, "a los 0 ms el aparato pinta lo mismo que `pintar`: {distintos} pixeles distintos");
    let en_500 = foto::foto_anima(&l, 500);
    assert!(reposo.px != en_500.px, "a los 500 ms algo se movio");
    // Cada dibujo se repite en SU ciclo (2 s, 1 s y 1,2 s): la maqueta
    // entera vuelve al principio a los 6 s, y no antes.
    assert!(foto::foto_anima(&l, 6000).px == en_0.px, "a los 6 s, otra vez el principio");
    assert!(foto::foto_anima(&l, 2000).px != en_0.px, "a los 2 s el cargando (1,2 s) va por otro sitio");
}
