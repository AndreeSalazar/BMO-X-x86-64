//! El banco: un `.exe` de Windows REAL (`prueba/hola.exe`, de clang y lld-link;
//! como se rehace, en `prueba/HACER.txt`) y sus mutaciones.

use alloc::string::ToString;
use alloc::vec::Vec;

use crate::*;

const HOLA: &[u8] = include_bytes!("../prueba/hola.exe");
const FRASE: &[u8] = b"hola desde un .exe de Windows\r\n";

fn rva_de_la_frase(img: &[u8]) -> usize {
    img.windows(FRASE.len()).position(|w| w == FRASE).expect("la frase esta en la imagen")
}

/// La tabla de la casa del banco: las tres de `hola.exe`, en direcciones
/// inventadas y distintas.
fn tabla(dll: &str, f: &Funcion) -> Option<u64> {
    if !dll.eq_ignore_ascii_case("kernel32.dll") {
        return None;
    }
    match f {
        Funcion::Nombre(n) if n == "GetStdHandle" => Some(0x7000_0010),
        Funcion::Nombre(n) if n == "WriteFile" => Some(0x7000_0020),
        Funcion::Nombre(n) if n == "ExitProcess" => Some(0x7000_0030),
        _ => None,
    }
}

#[test]
fn hola_exe_esta_dentro_y_se_lee_entero() {
    let pe = leer(HOLA).unwrap();
    assert_eq!(pe.base, 0x1_4000_0000);
    assert_eq!(pe.entrada, 0x1000);
    let nombres: Vec<_> = pe.secciones.iter().map(|s| (s.nombre.as_str(), s.permiso())).collect();
    assert_eq!(nombres, [(".text", Permiso::Codigo), (".rdata", Permiso::Lectura), (".reloc", Permiso::Lectura)]);
    assert_eq!(pe.tls.rva, 0);
}

#[test]
fn lo_que_pide_son_tres_funciones_de_kernel32() {
    let pe = leer(HOLA).unwrap();
    let img = colocar(&pe, HOLA, pe.base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    let nombres: Vec<_> = imps.iter().map(|i| (i.dll.as_str(), i.funcion.to_string())).collect();
    assert_eq!(nombres, [("kernel32.dll", "ExitProcess".to_string()), ("kernel32.dll", "GetStdHandle".to_string()), ("kernel32.dll", "WriteFile".to_string())]);
}

/// Cada `call [rip+x]` del codigo (`FF 15 disp32`) cae en una ranura de la
/// IAT: las ranuras que se rellenan son las que el codigo usa de verdad.
#[test]
fn cada_llamada_del_codigo_cae_en_una_ranura_de_la_iat() {
    let pe = leer(HOLA).unwrap();
    let img = colocar(&pe, HOLA, pe.base).unwrap();
    let ranuras: Vec<u32> = importaciones(&pe, &img).unwrap().iter().map(|i| i.ranura).collect();
    let texto = &pe.secciones[0];
    let codigo = &img[texto.rva as usize..(texto.rva + texto.tam_virtual) as usize];
    let mut llamadas = 0;
    for k in 0..codigo.len().saturating_sub(6) {
        if codigo[k] == 0xFF && codigo[k + 1] == 0x15 {
            let disp = i32::from_le_bytes([codigo[k + 2], codigo[k + 3], codigo[k + 4], codigo[k + 5]]);
            let destino = (texto.rva as i64 + k as i64 + 6 + disp as i64) as u32;
            assert!(ranuras.contains(&destino), "call [rip] a {destino:#x}, fuera de la IAT {ranuras:x?}");
            llamadas += 1;
        }
    }
    assert_eq!(llamadas, 3, "WriteFile, GetStdHandle y ExitProcess");
}

#[test]
fn en_su_base_el_puntero_absoluto_vale_lo_que_dejo_el_enlazador() {
    let pe = leer(HOLA).unwrap();
    let img = colocar(&pe, HOLA, pe.base).unwrap();
    let frase = rva_de_la_frase(&img) as u64;
    let punteros: Vec<usize> = (0..img.len() - 8).step_by(8).filter(|&o| u64::from_le_bytes(img[o..o + 8].try_into().unwrap()) == pe.base + frase).collect();
    assert_eq!(punteros.len(), 1, "`mensaje` apunta a la frase");
}

/// Movido a otra base, la relocalizacion DIR64 lleva `mensaje` con el.
#[test]
fn movido_a_otra_base_la_relocalizacion_lo_sigue() {
    let pe = leer(HOLA).unwrap();
    let otra = 0x5000_0000u64;
    let img = colocar(&pe, HOLA, otra).unwrap();
    let frase = rva_de_la_frase(&img) as u64;
    let en_su_base = colocar(&pe, HOLA, pe.base).unwrap();
    let o = (0..en_su_base.len() - 8).step_by(8).find(|&o| u64::from_le_bytes(en_su_base[o..o + 8].try_into().unwrap()) == pe.base + frase).unwrap();
    assert_eq!(u64::from_le_bytes(img[o..o + 8].try_into().unwrap()), otra + frase);
    // Y NADA MAS cambio: una relocalizacion de mas seria un byte pisado.
    let distintos = img.iter().zip(&en_su_base).filter(|(a, b)| a != b).count();
    assert!(distintos <= 8, "{distintos} bytes distintos entre las dos bases");
}

#[test]
fn con_la_tabla_entera_cada_ranura_recibe_su_funcion() {
    let pe = leer(HOLA).unwrap();
    let mut img = colocar(&pe, HOLA, pe.base).unwrap();
    let imps = importaciones(&pe, &img).unwrap();
    resolver(&mut img, &imps, tabla).unwrap();
    for i in &imps {
        let o = i.ranura as usize;
        assert_eq!(Some(u64::from_le_bytes(img[o..o + 8].try_into().unwrap())), tabla(&i.dll, &i.funcion));
    }
}

/// Sin `WriteFile` en la tabla: NO arranca, dice cual, y no escribio nada.
#[test]
fn si_falta_una_no_arranca_y_dice_cual() {
    let pe = leer(HOLA).unwrap();
    let mut img = colocar(&pe, HOLA, pe.base).unwrap();
    let antes = img.clone();
    let imps = importaciones(&pe, &img).unwrap();
    let e = resolver(&mut img, &imps, |d, f| if matches!(f, Funcion::Nombre(n) if n == "WriteFile") { None } else { tabla(d, f) }).unwrap_err();
    assert_eq!(e.to_string(), "no arranca: faltan 1 funcion(es) en la tabla de la casa: kernel32.dll!WriteFile");
    assert_eq!(img, antes, "a medias no se escribe nada");
}

fn firma(d: &[u8]) -> usize {
    u32::from_le_bytes(d[0x3C..0x40].try_into().unwrap()) as usize
}

#[test]
fn fuera_con_su_motivo_como_dice_rayosx() {
    for (maquina, palabra) in [(0x14C, "32 bits"), (0xAA64, "ARM64 ("), (0xA641, "ARM64EC"), (0xA64E, "ARM64X"), (0x1C4, "ARM de 32"), (0x1C0, "desconocida")] {
        let mut m = HOLA.to_vec();
        let e = firma(&m);
        m[e + 4..e + 6].copy_from_slice(&(maquina as u16).to_le_bytes());
        match leer(&m) {
            Err(Fallo::Fuera(motivo)) => assert!(motivo.contains(palabra), "{maquina:#x}: {motivo}"),
            otro => panic!("{maquina:#x}: {otro:?}"),
        }
    }
    let mut m = HOLA.to_vec();
    let e = firma(&m);
    m[e + 24..e + 26].copy_from_slice(&0x10Bu16.to_le_bytes());
    assert!(matches!(leer(&m), Err(Fallo::Fuera(t)) if t.contains("PE32 ")));
    let mut m = HOLA.to_vec();
    m[e + 24 + 112 + 14 * 8..e + 24 + 112 + 14 * 8 + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    assert!(matches!(leer(&m), Err(Fallo::Fuera(t)) if t.starts_with(".NET")));
    assert_eq!(leer(b"nada"), Err(Fallo::NoEsPe));
}

/// `.text` marcada ademas como escribible: el W^X de la casa la rechaza.
#[test]
fn una_seccion_que_escribe_y_ejecuta_se_rechaza() {
    let mut m = HOLA.to_vec();
    let e = firma(&m);
    let tam_opc = u16::from_le_bytes([m[e + 20], m[e + 21]]) as usize;
    let s = e + 24 + tam_opc;
    let c = u32::from_le_bytes(m[s + 36..s + 40].try_into().unwrap()) | 0x8000_0000;
    m[s + 36..s + 40].copy_from_slice(&c.to_le_bytes());
    assert_eq!(leer(&m), Err(Fallo::EscribeYEjecuta(".text".into())));
}

/// Una relocalizacion de otro tipo (HIGHLOW, de 32 bits) no se aplica a
/// ciegas: se dice.
#[test]
fn una_relocalizacion_de_otro_tipo_se_dice() {
    let pe = leer(HOLA).unwrap();
    let reloc = pe.secciones.iter().find(|s| s.nombre == ".reloc").unwrap();
    let mut m = HOLA.to_vec();
    let entrada = reloc.desde as usize + 8;
    let e = u16::from_le_bytes([m[entrada], m[entrada + 1]]);
    assert_eq!(e >> 12, 10, "la primera es DIR64");
    m[entrada..entrada + 2].copy_from_slice(&((3 << 12) | (e & 0xFFF)).to_le_bytes());
    let pe = leer(&m).unwrap();
    assert!(matches!(colocar(&pe, &m, 0x5000_0000), Err(Fallo::Relocalizacion { tipo: 3, .. })));
    // En su base no hay nada que mover: carga.
    assert!(colocar(&pe, &m, pe.base).is_ok());
}

#[test]
fn un_fichero_cortado_dice_donde() {
    for largo in [0x3E, 0x90, 0x150, 0x300] {
        let r = leer(&HOLA[..largo]);
        assert!(r.is_err(), "cortado en {largo:#x} no puede leerse entero");
    }
}
