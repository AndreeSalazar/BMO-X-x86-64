//! **UN PNG ESCRITO EN TITAN++, ejecutado y validado** (corte 4e de INTI,
//! 1d de `docs/plan/EL_FOCO.md`, 10-10). `Ultra_userspace/apps/png` cuenta una
//! imagen de 8 x 8, la envuelve en un PNG y la deja en el disco; aqui se corre
//! en el emulador y **se valida el formato byte a byte** -- firma, trozos,
//! longitudes, los CRC-32 y el adler de zlib --, y los pixeles se sacan del
//! bloque `stored` y se comparan con la cuenta.
//!
//! *** La validacion se hace AQUI, con otro codigo, a proposito: si el
//! programa comprobara su CRC, comprobaria su aritmetica contra si misma. Y
//! en TITAN++ el CRC va sin operaciones de bits (`%` y divisiones exactas):
//! que cuadre con el de Rust, con `^` y `>>`, es lo que dice que la cuenta es
//! la del formato.
//!
//! ** Y los MISMOS BYTES que `png.inti`: el 10-10, antes de quitarlo, los dos
//! dieron el mismo fichero (`HUELLA`).

use std::path::Path;

use bmo_lower::emu::{cargar_bex, run};

/// FNV-1a del `hola.png` de `png.inti` y del de TITAN++: el mismo.
const HUELLA: u64 = 0x5a1a_c5e4_e85a_121d;

fn crc32(datos: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for b in datos {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(datos: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in datos {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    b << 16 | a
}

fn fnv(datos: &[u8]) -> u64 {
    datos.iter().fold(0xcbf2_9ce4_8422_2325u64, |f, &b| (f ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn hola_png() -> (String, Vec<u8>) {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/png");
    let src = std::fs::read_to_string(pkg.join("src/main.titan")).expect("el main de PNG");
    let bex = bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e));
    let m = run(cargar_bex(&bex).expect("el .bex carga"), 200_000_000);
    assert!(m.exited, "{}", m.console);
    let png = m.archivos.get("datos/hola.png").expect("no dejo el fichero en el disco").clone();
    (m.console.clone(), png)
}

/// ***TITAN++ ESCRIBE UN PNG VALIDO***, recorrido como lo recorreria un
/// lector de verdad.
#[test]
fn el_png_que_escribe_titan_es_un_png() {
    let (dice, png) = hola_png();
    assert_eq!(dice, format!("PNG: datos/hola.png, {} bytes\n", png.len()));
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "la firma");
    let mut i = 8usize;
    let mut vistos = Vec::new();
    let mut idat = Vec::new();
    while i + 12 <= png.len() {
        let largo = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        let nombre = String::from_utf8_lossy(&png[i + 4..i + 8]).to_string();
        let fin = i + 8 + largo;
        assert!(fin + 4 <= png.len(), "el trozo `{}` se sale del fichero", nombre);
        // *** El CRC cubre el NOMBRE y los datos, no la longitud.
        let dice = u32::from_be_bytes(png[fin..fin + 4].try_into().unwrap());
        assert_eq!(dice, crc32(&png[i + 4..fin]), "el CRC del trozo `{}`", nombre);
        if nombre == "IHDR" {
            assert_eq!(&png[i + 8..i + 21], &[0, 0, 0, 8, 0, 0, 0, 8, 8, 2, 0, 0, 0], "8 x 8, 8 bits, RGB");
        }
        if nombre == "IDAT" {
            idat = png[i + 8..fin].to_vec();
        }
        vistos.push(nombre.clone());
        i = fin + 4;
        if nombre == "IEND" {
            break;
        }
    }
    assert_eq!(vistos, ["IHDR", "IDAT", "IEND"]);
    assert_eq!(i, png.len(), "sobran bytes despues del IEND");
    // -- El zlib: cabecera, UN bloque stored final, su LEN y su NLEN, y el adler.
    assert_eq!(&idat[..3], &[120, 1, 1]);
    let n = u16::from_le_bytes([idat[3], idat[4]]) as usize;
    assert_eq!(u16::from_le_bytes([idat[5], idat[6]]), !(n as u16));
    let crudos = &idat[7..7 + n];
    assert_eq!(idat.len(), 7 + n + 4);
    assert_eq!(u32::from_be_bytes(idat[7 + n..].try_into().unwrap()), adler32(crudos));
    // -- Y los pixeles, los de la cuenta: filtro 0, y (x*32, y*32, 128).
    let mut esperado = Vec::new();
    for y in 0..8u8 {
        esperado.push(0);
        for x in 0..8u8 {
            esperado.extend_from_slice(&[x * 32, y * 32, 128]);
        }
    }
    assert_eq!(crudos, &esperado[..]);
}

/// ** Los mismos bytes que `png.inti`.
#[test]
fn los_mismos_bytes_que_el_de_inti() {
    let (_, png) = hola_png();
    assert_eq!(fnv(&png), HUELLA, "cambio un byte del PNG: {:#018x}", fnv(&png));
}
