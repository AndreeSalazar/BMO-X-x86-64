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

/// ** EL WRAPPER Y EL MOTOR, hablando (BC2): las ordenes las escribe
/// `bmo-bankcat` (Rust), las ejecuta el motor COBOL compilado a x86-64, y
/// las respuestas las lee otra vez `bmo-bankcat`. Si los dos lados no
/// dijeran lo mismo, esto no cuadraria al centimo.
#[test]
fn el_wrapper_y_el_motor_hablan_el_mismo_idioma() {
    use bmo_bankcat::{escribir, Charla, Estado, Orden, Respuesta};
    let ordenes = [Orden::Abrir(125_000), Orden::Cobrar(5_000), Orden::Veces(3), Orden::Pagar(1_999), Orden::Pagar(9_999_900), Orden::Cerrar];
    let mut entrada = String::new();
    for o in &ordenes {
        let mut b = [0u8; 64];
        let n = escribir(o, &mut b).unwrap();
        entrada.push_str(std::str::from_utf8(&b[..n]).unwrap());
    }
    let src = bmo_cobol_front::copia::expandir(LIBRO, &mut libreria).unwrap();
    let salida = run_cobol_con_entrada(&src, &entrada);
    let mut ch = Charla::nueva();
    let r: Vec<Respuesta> = salida.bytes().filter_map(|b| ch.empujar(b)).collect::<Result<_, _>>().expect("el wrapper entiende al motor");
    let hecho = |saldo| Respuesta { estado: Estado::Hecho, saldo };
    assert_eq!(r, [hecho(125_000), hecho(130_000), hecho(130_000), hecho(124_003), Respuesta { estado: Estado::SinSaldo, saldo: 124_003 }, hecho(124_003)]);
}

/// El motor con DISCO (BC4): `libro` es lo que ya habia en `bankcat.dat`,
/// y `sin_disco` hace que el disco se niegue a guardar. Devuelve lo que
/// contesto y lo que quedo escrito.
fn motor_con_disco(ordenes: &str, libro: Option<&str>, sin_disco: bool) -> (Vec<String>, Option<String>) {
    use bmo_lower::emu::{run, Machine};
    let src = bmo_cobol_front::copia::expandir(LIBRO, &mut libreria).unwrap();
    let bef = crate::compile_source_to_bef(&src).expect("el motor compila");
    let mut m = Machine::new(code_section(&bef));
    m.poner_entrada(ordenes);
    if let Some(l) = libro {
        m.poner_archivo("bankcat.dat", l.as_bytes());
    }
    if sin_disco {
        m.fallar_al_guardar("bankcat.dat");
    }
    let m = run(m, 2_000_000);
    assert!(m.exited, "el motor tiene que acabar");
    (m.console.lines().map(str::to_string).collect(), m.archivo_texto("bankcat.dat"))
}

/// ** EL LIBRO SOBREVIVE A UN REINICIO (BC4): el segundo motor carga lo que
/// dejo el primero, y ABRIR no lo pisa.
#[test]
fn el_libro_sobrevive_al_reinicio() {
    let (r, libro) = motor_con_disco("1\n1250.00\n3\n19.99\n9\n0\n", None, false);
    assert_eq!(r, ["0", "1250.00", "0", "1230.01", "0", "1230.01"]);
    let libro = libro.expect("el libro quedo en el disco");
    // Otro arranque: abrir NO pisa (contesta el saldo), cobrar y cuadrar.
    let (r, _) = motor_con_disco("1\n1250.00\n2\n10.00\n9\n0\n", Some(&libro), false);
    assert_eq!(r, ["0", "1230.01", "0", "1240.01", "0", "1240.01"], "libro: {libro:?}");
}

/// Si el disco no guarda, el movimiento se hizo en memoria y se DICE (5).
#[test]
fn si_el_disco_no_guarda_se_dice() {
    let (r, _) = motor_con_disco("1\n100.00\n2\n5.00\n9\n0\n", None, true);
    assert_eq!(r, ["5", "100.00", "5", "105.00", "0", "105.00"]);
}
