//! E6 (02-10): el juez con los saltos -- R8 (salto sucio), R9 (predicado
//! antes de llegar) y R7 con BRA. Cada regla nueva, con su bodrio.

use bmo_sm86::codifica::{self as c, Cmp, Fuente, Mufu, PT};

use super::juez::{conoce, juzgar, juzgar_cuerpo_de_app, juzgar_drenado, Contexto, Regla};

/// El control: espera, barrera de escritura (7 = ninguna), mascara que espera.
const fn k(espera: u64, wbar: u64, mascara: u64) -> u64 {
    espera | 1 << 4 | wbar << 5 | 7 << 8 | mascara << 11
}

const CTX: Contexto = Contexto { registros: 64, sph: None };

fn regla(codigo: &[(u64, u64)]) -> Result<(), Regla> {
    juzgar_drenado(codigo, &CTX).map(|_| ()).map_err(|b| b.regla)
}

#[test]
fn el_juez_conoce_las_palabras_de_e6_e6c_y_e6d() {
    for (texto, lo, hi) in c::ORO_E6.iter().chain(c::LEIDAS_E6).chain(c::ORO_E6C).chain(c::LEIDAS_E6C).chain(c::ORO_E6D) {
        assert!(conoce(*lo, *hi), "{texto}");
    }
}

/// R9: un `@P0 BRA` 13 ciclos despues de su FSETP, si; antes, no. Y SEL lee
/// su predicado como operando: 4 ciclos.
#[test]
fn r9_un_predicado_se_lee_cuando_llega() {
    let si = |espera| {
        [
            c::fsetp(0, Cmp::Lt, c::r(1), c::r(2), k(espera, 7, 0)),
            c::bra(0, 0, k(5, 7, 0)),
            c::exit(k(5, 7, 0)),
        ]
    };
    assert_eq!(regla(&si(13)), Ok(()));
    assert_eq!(regla(&si(12)), Err(Regla::R9PredicadoAntesDeLlegar));
    let sel = |espera| [c::fsetp(0, Cmp::Lt, c::r(1), c::r(2), k(espera, 7, 0)), c::sel(3, 4, c::r(5), 0, false, k(6, 7, 0)), c::exit(k(5, 7, 0))];
    assert_eq!(regla(&sel(4)), Ok(()));
    assert_eq!(regla(&sel(3)), Err(Regla::R9PredicadoAntesDeLlegar));
}

/// R8: un BRA con un MUFU sin esperar, o con una FADD que no habra llegado
/// cuando corra el destino, es salto sucio. Con la barrera esperada en el
/// BRA y 6 ciclos entre la FADD y el destino, no.
#[test]
fn r8_un_salto_no_deja_nada_en_vuelo() {
    let mufu = |mascara| [c::mufu(1, Mufu::Rsq, 2, k(2, 0, 0)), c::bra(PT, 0, k(5, 7, mascara)), c::exit(k(5, 7, 0))];
    assert_eq!(regla(&mufu(0)), Err(Regla::R8SaltoSucio));
    assert_eq!(regla(&mufu(1)), Ok(()));
    // FADD y el BRA al ciclo siguiente: el destino sale a 1 + 5 = 6, justo.
    let fadd = |espera_bra| [c::fadd(1, c::r(2), c::r(3), false, k(1, 7, 0)), c::bra(PT, 0, k(espera_bra, 7, 0)), c::exit(k(5, 7, 0))];
    assert_eq!(regla(&fadd(5)), Ok(()));
    assert_eq!(regla(&fadd(4)), Err(Regla::R8SaltoSucio));
    // Sin SPH ni `juzgar_drenado` (el computo de `ptxas`), R8 no se mira.
    assert!(juzgar(&mufu(0), &CTX).is_ok());
}

/// R7 con saltos: un BRA (con su guarda) dentro del cuerpo, si; fuera, o un
/// guarda en otra instruccion, no.
#[test]
fn r7_un_salto_de_una_app_se_queda_en_su_cuerpo() {
    let cuerpo = |d: i64| [c::fsetp(0, Cmp::Lt, c::r(1), Fuente::Imm(0), k(13, 7, 0)), c::bra(8, d, k(5, 7, 0)), c::fadd(2, c::r(1), c::r(1), false, k(5, 7, 0)), c::exit(k(5, 7, 0))];
    // A la FADD (+0) y al EXIT (+16): dentro.
    assert_eq!(juzgar_cuerpo_de_app(&cuerpo(0), 8), Ok(()));
    assert_eq!(juzgar_cuerpo_de_app(&cuerpo(16), 8), Ok(()));
    // Detras del EXIT, o antes del principio: fuera.
    assert_eq!(juzgar_cuerpo_de_app(&cuerpo(32), 8).map_err(|b| b.regla), Err(Regla::R7CuerpoAjeno));
    assert_eq!(juzgar_cuerpo_de_app(&cuerpo(-48), 8).map_err(|b| b.regla), Err(Regla::R7CuerpoAjeno));
    // Un guarda en una FADD: no.
    let mut otro = cuerpo(0);
    otro[2].0 = (otro[2].0 & !(0xF << 12)) | 8 << 12;
    assert_eq!(juzgar_cuerpo_de_app(&otro, 8).map_err(|b| b.regla), Err(Regla::R7CuerpoAjeno));
}

/// E6d: el acarreo de IADD3 es un predicado: IADD3.X lo lee como operando
/// (4 ciclos, como `ptxas` en `oro_division.ptx`); antes, R9.
#[test]
fn r9_el_acarreo_se_lee_cuando_llega() {
    let x = |espera| [c::iadd3_acarreo(1, 0, 2, c::neg(3), k(espera, 7, 0)), c::iadd3_x(4, 4, c::r(c::RZ), 0, k(6, 7, 0)), c::exit(k(5, 7, 0))];
    assert_eq!(regla(&x(4)), Ok(()));
    assert_eq!(regla(&x(3)), Err(Regla::R9PredicadoAntesDeLlegar));
    // Y una app puede usarlos (R7).
    assert_eq!(juzgar_cuerpo_de_app(&x(4), 8), Ok(()));
}
