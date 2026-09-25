//! Los ceros silenciosos: identificadores y llamadas que no existen
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// =============== Los tres silencios que escondian todo esto ===============

/// * `#include` tiraba los `#define` de la cabecera.
///
/// Y no fallaba: la directiva se consumia, el identificador seguia en el
/// texto y el codegen lo ponia a cero. Dos constantes distintas se volvian
/// la MISMA variable inventada, asi que compararlas era cierto.
#[test]
fn una_cabecera_incluida_deja_sus_constantes() {
    let out = run_c_con_pp(
        "#include <bmo/entrada.h>
         int main() { printf(\"%d %d\\n\", BMO_TECLA_REPAG, BMO_TECLA_AVPAG); return 0; }",
    );
    assert_eq!(out.trim(), "135 136", "REPAG y AVPAG no pueden valer lo mismo");
}

/// La misma cabecera dos veces no duplica lo que trae. El guardia
/// `#ifndef` solo puede funcionar si el `#define` del guardia sobrevive al
/// `#include` -- antes no sobrevivia, asi que el guardia no guardaba nada.
#[test]
fn incluir_dos_veces_no_duplica_la_cabecera() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>
#include <bmo/entrada.h>
#include <bmo/bmo.h>
         int main() { printf(\"%d\\n\", bmo_scroll_mover(0, 4, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "4");
}

/// Un nombre que no existe NO VALE CERO. Un cero inventado es la peor
/// respuesta posible: es legitimo en cualquier expresion, asi que el error
/// viaja hasta donde ya no se puede rastrear.
#[test]
fn un_identificador_que_no_existe_es_un_error_no_un_cero() {
    let err = compile_source_to_bef("int main() { return NO_EXISTE; }")
        .expect_err("un nombre sin declarar tiene que fallar");
    assert!(err.message.contains("NO_EXISTE"), "mensaje: {}", err.message);
}

/// Y una llamada sin destino tampoco es un hueco: `E8 00000000` es "llama a
/// la siguiente instruccion", o sea un no-op con direccion de retorno.
#[test]
fn llamar_a_una_funcion_que_no_existe_es_un_error() {
    let err = compile_source_to_bef("int main() { fantasma(1); return 0; }")
        .expect_err("llamar a lo que no existe tiene que fallar");
    assert!(err.message.contains("fantasma"), "mensaje: {}", err.message);
}

// ======= Los cuatro que encontro ESPEJO (25-09): compilaban y daban OTRA cosa =======
//
// `toolchain/tools/espejo` compila cada programa por los dos lados (GCC y Clang
// en el anfitrion, BMO en el emulador) y compara lo impreso. Estos cuatro no
// daban ningun error: daban un numero equivocado. Aqui quedan fijos sin
// necesitar un GCC delante.

/// Etiquetas APILADAS: el parser solo guardaba un caso si tenia cuerpo, asi que
/// `case 0: case 1: case 2:` se quedaba en `case 2` y el 0 y el 1 iban al
/// `default`. Salio de un programa al azar, reducido por el espejo.
#[test]
fn las_etiquetas_apiladas_de_un_switch_caen_juntas() {
    let out = run_c(
        "int main() { int r = 0, s = 0; unsigned int i;
           for (i = 0; i < 5u; i++) {
             switch (i) { case 0: case 1: case 2: s += 10; break; default: r++; }
           }
           printf(\"%d %d\", r, s); return 0; }",
    );
    assert_eq!(out, "2 30");
}

/// Lo de ANTES del primer `case` no se ejecuta nunca: se guardaba con
/// `value: None`, que es la marca del `default`, y corria como tal.
#[test]
fn lo_de_antes_del_primer_case_no_se_ejecuta() {
    let out = run_c(
        "int main() { int g = 7; int h = 5;
           switch (1) { g = 99; case 1: break; }
           switch (h & 3) { h = 100; }
           printf(\"%d %d\", g, h); return 0; }",
    );
    assert_eq!(out, "7 5");
}

/// `++k` sobre una `static` local no pasaba por el alias `funcion.k`, y el
/// codegen, sin variable, ponia un cero callado: devolvia 0 en cada llamada.
#[test]
fn el_preincremento_de_una_static_local_cuenta() {
    let out = run_c(
        "int cuenta(void) { static int k = 0; return ++k; }
         int baja(void) { static int j = 10; return --j; }
         int main() { cuenta(); cuenta(); baja();
           printf(\"%d %d\", cuenta(), baja()); return 0; }",
    );
    assert_eq!(out, "3 8");
}

/// Y el cero callado mismo ya no existe: `++x` y `x = v` sobre un nombre que
/// no esta son un ERROR con el nombre delante, como ya lo era leerlo.
#[test]
fn incrementar_o_escribir_lo_que_no_existe_es_un_error() {
    let err = compile_source_to_bef("int main() { ++fantasma; return 0; }")
        .expect_err("++ de un nombre sin declarar tiene que fallar");
    assert!(err.message.contains("fantasma"), "mensaje: {}", err.message);
    let err = compile_source_to_bef("int main() { fantasma = 3; return 0; }")
        .expect_err("escribir en un nombre sin declarar tiene que fallar");
    assert!(err.message.contains("fantasma"), "mensaje: {}", err.message);
}

/// Un campo de bits guarda su valor RECORTADO a su ancho, por `=` y por los
/// compuestos, y con signo si su tipo lo tiene. Antes se guardaba entero:
/// `f.a = 9` en 3 bits daba 9.
#[test]
fn un_campo_de_bits_se_recorta_a_su_ancho() {
    let out = run_c(
        "struct B { int s : 4; unsigned u : 5; };
         int main() { struct B b; struct B *p = &b;
           b.s = 9; int x = b.s;
           p->u = 40; int y = p->u;
           b.u = 30; b.u += 5; int z = b.u;
           printf(\"%d %d %d %d\", x, y, z, (int)(b.u = 33)); return 0; }",
    );
    assert_eq!(out, "-7 8 3 1");
}

// ======= Y los tres de la segunda vuelta (25-09): 500 al azar, reducidos =======

/// `c ? a : b` vale el tipo COMUN de sus ramas: con `0u` delante y 64 bits
/// detras, se tomaba por `unsigned int` y la suma de fuera recortaba.
#[test]
fn el_ternario_vale_el_tipo_comun_de_sus_ramas() {
    let out = run_c(
        "int main() { unsigned long long big = 15796813855956853689ull;
           printf(\"%llu\", (0u > 0u ? 0u : big) + 0u); return 0; }",
    );
    assert_eq!(out, "15796813855956853689");
}

/// La suma de 32 bits pierde su acarreo aunque lo siguiente sea un `|`: el
/// `&` de dentro no tenia tipo y dejaba a la suma sin el suyo.
#[test]
fn el_acarreo_se_pierde_bajo_un_bit_a_bit() {
    let out = run_c(
        "int main() { unsigned int l0 = 1623127984u; unsigned int v = 3033899948u;
           printf(\"%u\", (v + (l0 & 1734225868u)) | 1u); return 0; }",
    );
    assert_eq!(out, "351521581");
}

/// Una cuenta de constantes sin signo, plegada, sale como el long path la
/// dejaria: recortada a 32 bits, no extendida con signo.
#[test]
fn una_constante_plegada_sin_signo_no_se_extiende_con_signo() {
    let out = run_c(
        "int main() { unsigned int x = 3000000000u;
           printf(\"%u\", (unsigned int)(x > (424671700u - 2218538477u))); return 0; }",
    );
    assert_eq!(out, "1");
}
