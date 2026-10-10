//! **DE BMP Y QOI A BICO, EN TITAN++: ejecutado, y contra ficheros ROTOS**
//! (corte 4e de INTI, 1d de `docs/plan/EL_FOCO.md`, 10-10).
//!
//! Las MISMAS pruebas que tenia `bico.inti` (`toolchain/lang/inti/
//! emisor-x86_64/tests/bico.rs`, que se fue con el), contra el de TITAN++
//! (`Ultra_userspace/apps/bico`): los mismos ficheros construidos byte a byte,
//! los mismos pixeles escritos a mano, los mismos codigos. No hay un
//! decodificador de referencia en Rust: si tuviera el mismo fallo, las dos
//! mitades estarian de acuerdo y la prueba aprobaria.
//!
//! *** Y la mitad que importa es la de los ficheros ROTOS: uno MALICIOSO
//! --medidas mentirosas, bytes que faltan-- da un codigo y NINGUN fichero.
//!
//! [!] Un cambio con INTI: el codigo 2 (mas de 256 KiB) es ahora el 1 --
//! `director.fichero` dice si lo leyo ENTERO, no por que no --.

use std::path::Path;

use bmo_lower::emu::{cargar_bex, run, Machine};

fn bico() -> Vec<u8> {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/bico");
    let src = std::fs::read_to_string(pkg.join("src/main.titan")).expect("el main de BICO");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

/// Corre el conversor con los ficheros que se le den. Devuelve la maquina.
fn convierte(bmp: Option<&[u8]>, qoi: Option<&[u8]>) -> Machine {
    let mut m = cargar_bex(&bico()).expect("el .bex carga");
    if let Some(b) = bmp {
        m.archivos.insert("datos/foto.bmp".into(), b.to_vec());
    }
    if let Some(q) = qoi {
        m.archivos.insert("datos/foto.qoi".into(), q.to_vec());
    }
    run(m, 200_000_000)
}

/// Lo que dijo por consola.
fn dice(m: &Machine) -> String {
    m.console.clone()
}

/// El codigo que dio un formato: la cifra detras de su etiqueta.
fn codigo(m: &Machine, etiqueta: &str) -> char {
    let d = dice(m);
    let i = d.find(etiqueta).unwrap_or_else(|| panic!("no dijo `{}`: {:?}", etiqueta, d));
    d[i + etiqueta.len()..].chars().next().expect("etiqueta sin codigo")
}

trait Archivo {
    fn archivo(&self, ruta: &str) -> Option<&[u8]>;
}
impl Archivo for Machine {
    fn archivo(&self, ruta: &str) -> Option<&[u8]> {
        self.archivos.get(ruta).map(Vec::as_slice)
    }
}

// ===================================================================
//  Construir ficheros de entrada
// ===================================================================

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Un BMP con estas medidas y estos datos de pixel YA ORDENADOS como van en el
/// fichero (filas con su relleno incluido).
fn bmp(ancho: i32, alto: i32, bits: u16, compresion: u32, pixeles: &[u8]) -> Vec<u8> {
    let mut v = vec![b'B', b'M'];
    u32le(&mut v, (54 + pixeles.len()) as u32);
    u32le(&mut v, 0);
    u32le(&mut v, 54);
    u32le(&mut v, 40);
    u32le(&mut v, ancho as u32);
    u32le(&mut v, alto as u32);
    u16le(&mut v, 1);
    u16le(&mut v, bits);
    u32le(&mut v, compresion);
    u32le(&mut v, pixeles.len() as u32);
    u32le(&mut v, 2835);
    u32le(&mut v, 2835);
    u32le(&mut v, 0);
    u32le(&mut v, 0);
    v.extend_from_slice(pixeles);
    v
}

/// 3x2, 24 bits, DE ABAJO ARRIBA, con tres bytes de relleno por fila (0xEE
/// para que un relleno leido como pixel se vea).
fn bmp_3x2() -> Vec<u8> {
    let mut p = Vec::new();
    // La fila de ABAJO va primero en el fichero.
    p.extend_from_slice(&[10, 11, 12, 13, 14, 15, 16, 17, 18, 0xEE, 0xEE, 0xEE]);
    p.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 0xEE, 0xEE, 0xEE]);
    bmp(3, 2, 24, 0, &p)
}

fn cabecera_bico(ancho: u16, alto: u16) -> Vec<u8> {
    let mut v = b"BICO".to_vec();
    u16le(&mut v, ancho);
    u16le(&mut v, alto);
    v
}

// ===================================================================
//  BMP
// ===================================================================

/// ***Un BMP de abajo arriba sale DE ARRIBA ABAJO, sin su relleno.***
#[test]
fn un_bmp_de_24_bits_de_abajo_arriba_sale_bien() {
    let m = convierte(Some(&bmp_3x2()), None);
    assert_eq!(codigo(&m, "bmp "), '0', "{}", dice(&m));
    let mut esperado = cabecera_bico(3, 2);
    esperado.extend_from_slice(&[1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255]);
    esperado.extend_from_slice(&[10, 11, 12, 255, 13, 14, 15, 255, 16, 17, 18, 255]);
    assert_eq!(m.archivo("datos/fotob.bic").expect("no dejo el BICO"), &esperado[..]);
}

/// De ARRIBA ABAJO (alto negativo) y de 32 bits: el cuarto byte es reservado y
/// sale OPACO aunque venga a cero.
#[test]
fn un_bmp_de_32_bits_de_arriba_abajo_sale_opaco() {
    let p = [1, 2, 3, 0, 4, 5, 6, 0];
    let m = convierte(Some(&bmp(2, -1, 32, 0, &p)), None);
    assert_eq!(codigo(&m, "bmp "), '0', "{}", dice(&m));
    let mut esperado = cabecera_bico(2, 1);
    esperado.extend_from_slice(&[1, 2, 3, 255, 4, 5, 6, 255]);
    assert_eq!(m.archivo("datos/fotob.bic").unwrap(), &esperado[..]);
}

// ===================================================================
//  QOI -- las seis operaciones, con los pixeles calculados a mano
// ===================================================================

/// 3x2 y seis pixeles, uno por operacion:
///
/// ```text
///   RGB   10,20,30          -> (10,20,30,255)
///   DIFF  +1, 0, -1         -> (11,20,29,255)    01 11 10 01 = 121
///   LUMA  dg +4, dr-dg +1, db-dg -2
///                           -> (16,24,31,255)    164, 0x96
///   INDEX 9 = (10*3+20*5+30*7+255*11) % 64
///                           -> (10,20,30,255)
///   RGBA  200,100,50,0      -> transparente
///   RUN   1                 -> el mismo otra vez 11 000000 = 192
/// ```
fn qoi_3x2() -> Vec<u8> {
    let mut v = b"qoif".to_vec();
    v.extend_from_slice(&3u32.to_be_bytes());
    v.extend_from_slice(&2u32.to_be_bytes());
    v.extend_from_slice(&[4, 0]);
    v.extend_from_slice(&[254, 10, 20, 30]);
    v.push(121);
    v.extend_from_slice(&[164, 0x96]);
    v.push(9);
    v.extend_from_slice(&[255, 200, 100, 50, 0]);
    v.push(192);
    v.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    v
}

#[test]
fn un_qoi_con_las_seis_operaciones_sale_bien() {
    assert_eq!((10 * 3 + 20 * 5 + 30 * 7 + 255 * 11) % 64, 9, "el indice de la prueba");
    let m = convierte(None, Some(&qoi_3x2()));
    assert_eq!(codigo(&m, "qoi "), '0', "{}", dice(&m));
    let mut esperado = cabecera_bico(3, 2);
    esperado.extend_from_slice(&[30, 20, 10, 255, 29, 20, 11, 255, 31, 24, 16, 255]);
    esperado.extend_from_slice(&[30, 20, 10, 255, 50, 100, 200, 0, 50, 100, 200, 0]);
    assert_eq!(m.archivo("datos/fotoq.bic").expect("no dejo el BICO"), &esperado[..]);
}

/// ** DIFF que DA LA VUELTA: rojo 0 menos 2 es 254, no una trampa.
///
/// En C es un `unsigned char` y nadie lo ve. En TITAN++ se escribe como suma
/// modulo 256 (`% 256`), y aqui se comprueba que la cuenta es la del formato.
#[test]
fn un_diff_que_da_la_vuelta_es_la_cuenta_del_formato() {
    let mut v = b"qoif".to_vec();
    v.extend_from_slice(&1u32.to_be_bytes());
    v.extend_from_slice(&1u32.to_be_bytes());
    v.extend_from_slice(&[4, 0]);
    // Del (0,0,0,255) inicial: dr -2, dg +1, db -2  ->  01 00 11 00 = 76
    v.push(76);
    v.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    let m = convierte(None, Some(&v));
    assert_eq!(codigo(&m, "qoi "), '0', "{}", dice(&m));
    let mut esperado = cabecera_bico(1, 1);
    esperado.extend_from_slice(&[254, 1, 254, 255]);
    assert_eq!(m.archivo("datos/fotoq.bic").unwrap(), &esperado[..]);
}

// ===================================================================
//  *** LOS FICHEROS ROTOS: un codigo, y NINGUN fichero
// ===================================================================

fn roto(bmp_: Option<Vec<u8>>, qoi: Option<Vec<u8>>, etiqueta: &str, esperado: char, salida: &str) {
    let m = convierte(bmp_.as_deref(), qoi.as_deref());
    assert!(m.exited, "el conversor tiene que TERMINAR, no morir: {}", dice(&m));
    assert_eq!(codigo(&m, etiqueta), esperado, "{}", dice(&m));
    assert!(m.archivo(salida).is_none(), "un fichero roto no deja salida");
}

/// Un BMP cortado a la mitad: la cabecera promete dos filas y llega una.
#[test]
fn un_bmp_cortado_da_6() {
    let mut b = bmp_3x2();
    b.truncate(b.len() - 12);
    roto(Some(b), None, "bmp ", '6', "datos/fotob.bic");
}

/// ***EL FICHERO MALICIOSO: una cabecera que dice 3x60000.***
///
/// Es la forma de los fallos de libpng: medidas grandes, datos chicos. Aqui
/// la medida se para en el tope y no se multiplica nunca.
#[test]
fn un_bmp_que_miente_en_el_alto_da_5() {
    let p = bmp_3x2();
    let mut b = p.clone();
    b[22..26].copy_from_slice(&60000u32.to_le_bytes());
    roto(Some(b), None, "bmp ", '5', "datos/fotob.bic");
}

#[test]
fn un_bmp_comprimido_da_4_y_no_se_intenta() {
    let b = bmp(3, 2, 24, 1, &[0; 24]);
    roto(Some(b), None, "bmp ", '4', "datos/fotob.bic");
}

#[test]
fn lo_que_no_es_un_bmp_da_3() {
    let mut b = bmp_3x2();
    b[0] = b'X';
    roto(Some(b), None, "bmp ", '3', "datos/fotob.bic");
}

/// Un QOI que promete 256x256 y trae UN pixel: se para en el primer byte que
/// falta, no escribe 65.536 pixeles inventados.
#[test]
fn un_qoi_que_promete_mas_de_lo_que_trae_da_6() {
    let mut q = qoi_3x2();
    q[4..8].copy_from_slice(&256u32.to_be_bytes());
    q[8..12].copy_from_slice(&256u32.to_be_bytes());
    roto(None, Some(q), "qoi ", '6', "datos/fotoq.bic");
}

/// Sin ficheros no hay nada que convertir, y se dice con un 1 por cada uno.
#[test]
fn sin_ficheros_dice_1_y_1() {
    let m = convierte(None, None);
    assert!(m.exited);
    assert_eq!(codigo(&m, "bmp "), '1');
    assert_eq!(codigo(&m, "qoi "), '1');
}

/// La consola entera, como la lee el propietario.
#[test]
fn lo_que_dice_por_consola() {
    let m = convierte(Some(&bmp_3x2()), None);
    assert!(m.exited);
    assert_eq!(dice(&m), "-- bico\nbmp 0\nqoi 1\nfin\n");
}

/// Un fichero de mas de 256 KiB no se lee: 1, y ninguna salida.
#[test]
fn un_fichero_de_mas_de_256_kib_da_1() {
    roto(Some(vec![b'B'; 256 * 1024 + 1]), None, "bmp ", '1', "datos/fotob.bic");
}

/// ** La MAYOR que acepta: 256 x 256 (un RGB y tandas de 62), entera --
/// 262.152 bytes de salida, la lista de pixeles en el monton y soltada --.
#[test]
fn una_de_256_de_lado_entera() {
    let mut q = b"qoif".to_vec();
    q.extend_from_slice(&256u32.to_be_bytes());
    q.extend_from_slice(&256u32.to_be_bytes());
    q.extend_from_slice(&[3, 0]);
    q.extend_from_slice(&[254, 7, 8, 9]);
    let mut quedan = 256 * 256 - 1;
    while quedan > 0 {
        let n = quedan.min(62);
        q.push(0xC0 | (n - 1) as u8);
        quedan -= n;
    }
    q.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    let m = convierte(None, Some(&q));
    assert_eq!(codigo(&m, "qoi "), '0', "{}", dice(&m));
    let s = m.archivo("datos/fotoq.bic").expect("no dejo el BICO");
    assert_eq!(s.len(), 8 + 256 * 256 * 4);
    assert_eq!(&s[..8], &[b'B', b'I', b'C', b'O', 0, 1, 0, 1]);
    assert!(s[8..].chunks(4).all(|p| p == [9, 8, 7, 255]));
}
