//! **BANK CAT**: la libreria de copybooks (`copy/CABDATOS.cpy`,
//! `copy/CABLIBRO.cpy`) y su motor (`examples/11-bankcat/libro.cob`),
//! EJECUTADOS en el emulador. Es la primera vez que un `COPY` corre.

use super::comun::*;

const LIBRO: &str = include_str!("../../../examples/11-bankcat/libro.cob");

fn libreria(nombre: &str) -> Option<String> {
    match nombre {
        "CABDATOS" => Some(include_str!("../../../copy/CABDATOS.cpy").into()),
        "CABLIBRO" => Some(include_str!("../../../copy/CABLIBRO.cpy").into()),
        _ => None,
    }
}

/// El motor, con sus `COPY` ya expandidos, contestando a `ordenes`.
fn motor(ordenes: &str) -> Vec<String> {
    let src = bmo_cobol_front::copia::expandir(LIBRO, &mut libreria).expect("los COPY se expanden");
    run_cobol_con_entrada(&src, ordenes).lines().map(str::to_string).collect()
}

#[test]
fn diecinueve_noventa_y_nueve_por_tres_son_cincuenta_y_nueve_noventa_y_siete() {
    // Abrir con 1250.00, cobrar 50.00, y pagar 3 x 19.99: 1240.03 exacto.
    let r = motor("1\n1250.00\n2\n50.00\n4\n3\n3\n19.99\n9\n0\n");
    assert_eq!(r, ["0", "1250.00", "0", "1300.00", "0", "1300.00", "0", "1240.03", "0", "1240.03"]);
}

#[test]
fn sin_saldo_no_se_paga_y_no_se_toca_nada() {
    let r = motor("1\n10.00\n3\n10.01\n3\n10.00\n9\n0\n");
    assert_eq!(r, ["0", "10.00", "2", "10.00", "0", "0.00", "0", "0.00"]);
}

#[test]
fn un_importe_malo_se_rechaza() {
    let r = motor("1\n5.00\n2\n0\n3\n-1.00\n7\n1\n9\n0\n");
    assert_eq!(r, ["0", "5.00", "3", "5.00", "3", "5.00", "3", "5.00", "0", "5.00"]);
}

#[test]
fn lo_que_no_cabe_no_entra() {
    // El saldo es S9(13)V99: lo mas son 9999999999999.99.
    let r = motor("1\n9999999999999.00\n2\n0.99\n2\n0.01\n9\n0\n");
    assert_eq!(r, ["0", "9999999999999.00", "0", "9999999999999.99", "1", "9999999999999.99", "0", "9999999999999.99"]);
}
