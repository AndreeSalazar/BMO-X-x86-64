//! VOCES: `<bmo/sonido.h>` y las voces del orquestador (2026-09-22)
//!
//! Parte del banco de pruebas de BMO C. Aqui no suena nada: lo que se mira es
//! lo que CRUZA LA PUERTA. `bmo_voz_tocar` mete seis numeros --inicio,
//! muestras, canal, formato, pista, los dos lados y la frecuencia-- en dos
//! palabras de 64 bits, y un desplazamiento mal puesto no da error: da un
//! disparo que suena en otro canal, a otra frecuencia, o se sale del banco.
//! El juez del kernel lo pararia, pero el fallo seria de esta cabecera y se
//! buscaria en el sitio equivocado.

use super::*;

fn ejecutar_voces(cuerpo: &str) -> bmo_lower::emu::Machine {
    let src = format!("#include <bmo/sonido.h>\n{cuerpo}");
    let bef = compile_with_preprocessor(&src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    maquina_de_bef(&bef)
}

/// Las llamadas a `BMO_SONIDO_VOZ` (0x06), en orden: `(a0, a1, a2)`.
fn voces(m: &bmo_lower::emu::Machine) -> Vec<(u64, u64, u64)> {
    m.syscalls.iter().filter(|s| s.operation == 0x06).map(|s| (s.arg0, s.arg1, s.arg2)).collect()
}

#[test]
fn tocar_empaqueta_cada_numero_en_su_sitio() {
    let m = ejecutar_voces(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_voz_tocar(cap, 3, 1000, 5000, BMO_VOZ_U8, 11025, 200, 56, 7);
    return 0;
}
"#,
    );
    let v = voces(&m);
    assert_eq!(v.len(), 1, "una sola puerta por sonido");
    let (que, donde, como) = v[0];
    assert_eq!(que, 2, "BMO_VOZ_TOCAR");
    assert_eq!(donde, 1000 | (5000 << 32));
    assert_eq!(como, 3 | (7 << 10) | (200 << 18) | (56 << 27) | (11025 << 36));
}

#[test]
fn en_bucle_es_el_mismo_tocar_con_el_bit_56() {
    let m = ejecutar_voces(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_voz_tocar_bucle(cap, 15, 2097152, 2304000, BMO_VOZ_S16, 24000, 256, 256, 0);
    return 0;
}
"#,
    );
    let v = voces(&m);
    assert_eq!(v.len(), 1);
    let (que, donde, como) = v[0];
    assert_eq!(que, 2, "tambien es BMO_VOZ_TOCAR: el bucle es un bit, no un verbo");
    assert_eq!(donde, 2_097_152 | (2_304_000 << 32));
    assert_eq!(como, 15 | (1 << 8) | (256 << 18) | (256 << 27) | (24_000 << 36) | (1 << 56));
}

#[test]
fn ajustar_callar_y_suena_llevan_su_canal() {
    let m = ejecutar_voces(
        r#"
int main() {
    unsigned long long cap;
    cap = bmo_sonido_reclamar();
    bmo_voz_ajustar(cap, 5, 256, 12);
    bmo_voz_callar(cap, BMO_VOZ_TODOS);
    bmo_voz_suena(cap, 9);
    return 0;
}
"#,
    );
    assert_eq!(voces(&m), vec![(3, 5, 256 | (12 << 16)), (4, 0xFF, 0), (5, 9, 0)]);
}
