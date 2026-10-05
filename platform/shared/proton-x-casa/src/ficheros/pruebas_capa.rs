//! Las pruebas de `ficheros.rs` (la capa, el volumen de mentira, los
//! solapados y el mapeo): movidas tal cual el 05-10, un nivel menos de
//! sangria, para que `ficheros.rs` no pase de las 1.000 lineas (L6a).

use super::*;
extern crate std;
use bmo_proton_x::ficheros::Entrada;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

#[derive(Default)]
struct Volumen {
    carpetas: BTreeSet<String>,
    ficheros: BTreeMap<String, Vec<u8>>,
}

static VOLUMEN: Mutex<Volumen> = Mutex::new(Volumen {
    carpetas: BTreeSet::new(),
    ficheros: BTreeMap::new(),
});
static UNA_A_LA_VEZ: Mutex<()> = Mutex::new(());

fn leer(r: &[u8]) -> Option<Vec<u8>> {
    VOLUMEN
        .lock()
        .unwrap()
        .ficheros
        .get(core::str::from_utf8(r).ok()?)
        .cloned()
}

fn escribir(r: &[u8], bytes: &[u8]) -> bool {
    let Ok(r) = core::str::from_utf8(r) else {
        return false;
    };
    VOLUMEN
        .lock()
        .unwrap()
        .ficheros
        .insert(String::from(r), bytes.to_vec());
    true
}

fn listar(r: &[u8]) -> Option<Vec<Entrada>> {
    let r = core::str::from_utf8(r).ok()?;
    let v = VOLUMEN.lock().unwrap();
    if !v.carpetas.contains(r) {
        return None;
    }
    let prefijo = if r.is_empty() {
        String::new()
    } else {
        alloc::format!("{r}/")
    };
    let mut entradas = Vec::new();
    for d in &v.carpetas {
        let Some(resto) = d.strip_prefix(&prefijo) else {
            continue;
        };
        if !resto.is_empty() && !resto.contains('/') {
            entradas.push(Entrada {
                nombre: String::from(resto),
                carpeta: true,
                atributos: 0x10,
                ..Default::default()
            });
        }
    }
    for (p, b) in &v.ficheros {
        let Some(resto) = p.strip_prefix(&prefijo) else {
            continue;
        };
        if !resto.is_empty() && !resto.contains('/') {
            entradas.push(Entrada {
                nombre: String::from(resto),
                bytes: b.len() as u64,
                atributos: 0x20,
                ..Default::default()
            });
        }
    }
    Some(entradas)
}

fn crear(r: &[u8]) -> bool {
    let Ok(r) = core::str::from_utf8(r) else {
        return false;
    };
    let padre = r.rsplit_once('/').map_or("", |(p, _)| p);
    let mut v = VOLUMEN.lock().unwrap();
    if v.carpetas.contains(r) || v.ficheros.contains_key(r) || !v.carpetas.contains(padre) {
        return false;
    }
    v.carpetas.insert(String::from(r))
}

fn no_quitar(r: &[u8]) -> bool {
    let Ok(r) = core::str::from_utf8(r) else {
        return false;
    };
    let mut v = VOLUMEN.lock().unwrap();
    if v.ficheros.remove(r).is_some() {
        return true;
    }
    let prefijo = alloc::format!("{r}/");
    let vacia = v.carpetas.contains(r)
        && !v.ficheros.keys().any(|p| p.starts_with(&prefijo))
        && !v.carpetas.iter().any(|p| p != r && p.starts_with(&prefijo));
    vacia && v.carpetas.remove(r)
}
fn no_renombrar(_: &[u8], _: &[u8]) -> bool {
    false
}
fn escribir_consola(_: &[u8]) {}
fn salir(_: u32) -> ! {
    panic!("la plataforma de prueba no sale")
}
fn superficie(_: u32, _: u32) -> Option<crate::Superficie> {
    None
}
fn mostrar(_: &crate::Superficie) -> bool {
    false
}
fn presentar(_: &crate::Superficie) {}
fn evento(_: &crate::Superficie) -> u64 {
    0
}
fn dormir() {}
fn poner_gs(_: u64) {}
fn ahora() -> u64 {
    0
}
fn sellar(_: &[u8]) -> Option<u64> {
    None
}
fn soltar(_: u64, _: usize) {}
fn memoria(_: usize) -> Option<u64> {
    None
}

fn fecha() -> Option<u64> {
    None
}

const ARCHIVO_GIGANTE: &str = "d:Cyberpunk 2077/archive/pc/content/basegame_4_gamedata.archive";
const MEDIDA_GIGANTE: u64 = (5 << 30) + 37;

fn medida_gigante(ruta: &[u8]) -> Option<u64> {
    (ruta == ARCHIVO_GIGANTE.as_bytes()).then_some(MEDIDA_GIGANTE)
}

fn leer_gigante(ruta: &[u8], desde: u64, dst: &mut [u8]) -> Option<usize> {
    if ruta != ARCHIVO_GIGANTE.as_bytes() || desde >= MEDIDA_GIGANTE {
        return None;
    }
    let n = dst
        .len()
        .min((MEDIDA_GIGANTE - desde).min(usize::MAX as u64) as usize);
    for (k, byte) in dst[..n].iter_mut().enumerate() {
        *byte = desde.wrapping_add(k as u64) as u8;
    }
    Some(n)
}

// El TEB del hilo de la prueba (Linux): ver el fichero.
mod teb_de_prueba;
use teb_de_prueba::teb_de_prueba;

fn plataforma_prueba_trozos() -> crate::Plataforma {
    let mut p = plataforma_prueba();
    p.trozos = Some(crate::Trozos {
        medida: medida_gigante,
        leer: leer_gigante,
        umbral: 1,
    });
    p
}

fn plataforma_prueba() -> crate::Plataforma {
    crate::Plataforma {
        escribir: escribir_consola,
        salir,
        superficie,
        mostrar,
        presentar,
        evento,
        dormir,
        poner_gs,
        ahora_ns: ahora,
        dibujar: bmo_proton_x::lote::en_cpu,
        sellar_codigo: sellar,
        soltar_codigo: soltar,
        leer_fichero: leer,
        escribir_fichero: escribir,
        memoria,
        fecha,
        listar,
        carpetas: Some(crate::Carpetas {
            crear,
            quitar: no_quitar,
            renombrar: no_renombrar,
        }),
        reserva: None,
        trozos: None,
        sonido: None,
    }
}

#[test]
fn escribir_en_d_publica_una_copia_y_listar_la_prefiere() {
    let _una = UNA_A_LA_VEZ.lock().unwrap();
    {
        let mut v = VOLUMEN.lock().unwrap();
        *v = Volumen::default();
        for d in [
            "",
            "d:",
            "d:Cyberpunk 2077",
            "d:Cyberpunk 2077/bin",
            "d:Cyberpunk 2077/bin/x64",
        ] {
            v.carpetas.insert(String::from(d));
        }
        v.ficheros.insert(
            String::from("d:Cyberpunk 2077/bin/x64/settings.ini"),
            b"original".to_vec(),
        );
        v.ficheros.insert(
            String::from("d:Cyberpunk 2077/bin/x64/engine.dll"),
            b"base".to_vec(),
        );
    }
    // SAFETY: prueba serializada; las funciones usan solo el volumen de arriba.
    unsafe { crate::empezar(plataforma_prueba()) };
    teb_de_prueba();
    poner_directorio("d:Cyberpunk 2077/bin/x64");
    poner_capa(Some("proton-x/cyberpunk2077/capa"));
    let nombre: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\settings.ini"
        .encode_utf16()
        .chain([0])
        .collect();
    let h = create_file_dentro(
        nombre.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        0,
        0,
        OPEN_EXISTING,
        0,
        0,
    );
    assert_ne!(
        h, NO_VALE,
        "un OPEN_EXISTING para escribir crea la copia en la capa"
    );
    let mut antes = [0u8; 8];
    assert_eq!(leer_de(h, &mut antes, None), Ok(8));
    assert_eq!(&antes, b"original");
    assert_eq!(abierto(h).unwrap().mover(0, 0), Some(0));
    assert_eq!(escribir_en(h, b"patched!", None), Ok(8));
    assert_eq!(cerrar(h), 1);

    let original = "d:Cyberpunk 2077/bin/x64/settings.ini";
    let copia = "proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/settings.ini";
    assert_eq!(
        leer(original.as_bytes()).unwrap(),
        b"original",
        "D: nunca cambia"
    );
    assert_eq!(leer(copia.as_bytes()).unwrap(), b"patched!");
    assert_eq!(
        ruta_para_leer(original),
        copia,
        "la siguiente lectura usa la capa"
    );

    let nuevas: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\trace.log"
        .encode_utf16()
        .chain([0])
        .collect();
    let h = create_file_dentro(nuevas.as_ptr(), GENERIC_WRITE, 0, 0, CREATE_NEW, 0, 0);
    assert_ne!(h, NO_VALE, "CREATE_NEW de D: se dirige a ESTRATOS");
    assert_eq!(escribir_en(h, b"log", None), Ok(3));
    assert_eq!(cerrar(h), 1);
    assert_eq!(
        leer(b"d:Cyberpunk 2077/bin/x64/trace.log"),
        None,
        "un fichero nuevo tampoco aparece en D:"
    );
    assert_eq!(
        leer(b"proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/trace.log").unwrap(),
        b"log"
    );

    let entradas = crate::carpetas::listar("d:Cyberpunk 2077/bin/x64").unwrap();
    assert_eq!(
        entradas
            .iter()
            .find(|e| e.nombre == "settings.ini")
            .unwrap()
            .bytes,
        8
    );
    assert!(
        entradas.iter().any(|e| e.nombre == "engine.dll"),
        "D: sigue visible debajo de la capa"
    );
    assert!(
        entradas.iter().any(|e| e.nombre == "trace.log"),
        "los ficheros de la capa tambien se listan"
    );

    let origen: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine.dll"
        .encode_utf16()
        .chain([0])
        .collect();
    let destino: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine-copy.dll"
        .encode_utf16()
        .chain([0])
        .collect();
    assert_eq!(
        crate::carpetas::move_file_ex_w(origen.as_ptr(), destino.as_ptr(), 0),
        1
    );
    assert_eq!(
        leer(b"d:Cyberpunk 2077/bin/x64/engine.dll").unwrap(),
        b"base",
        "mover tambien conserva el original"
    );
    assert!(crate::carpetas::entrada("d:Cyberpunk 2077/bin/x64/engine.dll").is_none());
    assert_eq!(
        leer(b"proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/engine-copy.dll").unwrap(),
        b"base"
    );

    let copia: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine-copy.dll"
        .encode_utf16()
        .chain([0])
        .collect();
    let respaldo: Vec<u16> = "D:\\Cyberpunk 2077\\bin\\x64\\engine-backup.dll"
        .encode_utf16()
        .chain([0])
        .collect();
    assert_eq!(
        crate::carpetas::copy_file_ex_w(copia.as_ptr(), respaldo.as_ptr(), 0, 0, 0, 0),
        1
    );
    assert_eq!(
        leer(b"proton-x/cyberpunk2077/capa/Cyberpunk 2077/bin/x64/engine-backup.dll").unwrap(),
        b"base"
    );

    assert_eq!(
        crate::carpetas::delete_file_w(nombre.as_ptr()),
        1,
        "borrar marca el nombre sin tocar D:"
    );
    assert_eq!(leer(original.as_bytes()).unwrap(), b"original");
    assert!(
        crate::carpetas::entrada(original).is_none(),
        "el original queda oculto"
    );
    let entradas = crate::carpetas::listar("d:Cyberpunk 2077/bin/x64").unwrap();
    assert!(
        !entradas.iter().any(|e| e.nombre == "settings.ini"),
        "el listado respeta la marca"
    );

    poner_capa(None);
    let denied = create_file_dentro(nombre.as_ptr(), GENERIC_WRITE, 0, 0, OPEN_EXISTING, 0, 0);
    assert_eq!(denied, NO_VALE, "sin perfil, D: conserva el solo lectura");
}

#[test]
fn archivo_mayor_de_4_gib_se_mide_y_lee_sin_cargarlo_entero() {
    let _una = UNA_A_LA_VEZ.lock().unwrap();
    {
        let mut v = VOLUMEN.lock().unwrap();
        *v = Volumen::default();
        for d in [
            "",
            "d:",
            "d:Cyberpunk 2077",
            "d:Cyberpunk 2077/archive",
            "d:Cyberpunk 2077/archive/pc",
            "d:Cyberpunk 2077/archive/pc/content",
        ] {
            v.carpetas.insert(String::from(d));
        }
        // Solo se registra el nombre. Los 5 GiB existen virtualmente en
        // los callbacks: la prueba no reserva memoria proporcional.
        v.ficheros.insert(String::from(ARCHIVO_GIGANTE), Vec::new());
    }
    // SAFETY: prueba serializada; todas las E/S van al volumen simulado.
    unsafe { crate::empezar(plataforma_prueba_trozos()) };
    teb_de_prueba();
    crate::ficheros::poner_directorio("d:Cyberpunk 2077/bin/x64");
    crate::ficheros::poner_capa(None);

    let nombre: Vec<u16> =
        "D:\\Cyberpunk 2077\\archive\\pc\\content\\basegame_4_gamedata.archive"
            .encode_utf16()
            .chain([0])
            .collect();
    let h = create_file_dentro(nombre.as_ptr(), GENERIC_READ, 0, 0, OPEN_EXISTING, 0, 0);
    assert_ne!(h, NO_VALE);
    assert_eq!(
        abierto(h).unwrap().bytes.len(),
        0,
        "no se carga el contenido"
    );
    assert_eq!(abierto(h).unwrap().medida(), MEDIDA_GIGANTE);

    let mut medida = 0i64;
    assert_eq!(get_file_size_ex(h, &mut medida), 1);
    assert_eq!(medida as u64, MEDIDA_GIGANTE);
    let mut alto = 0u32;
    assert_eq!(get_file_size(h, &mut alto), MEDIDA_GIGANTE as u32);
    assert_eq!(alto, (MEDIDA_GIGANTE >> 32) as u32);

    let desde = (1u64 << 32) + 19;
    let mut nueva = 0i64;
    assert_eq!(set_file_pointer_ex(h, desde as i64, &mut nueva, 0), 1);
    assert_eq!(nueva as u64, desde);
    let mut bytes = [0u8; 32];
    let mut leidos = 0u32;
    assert_eq!(
        read_file(h, bytes.as_mut_ptr(), bytes.len() as u32, &mut leidos, 0),
        1
    );
    assert_eq!(leidos as usize, bytes.len());
    for (k, byte) in bytes.iter().enumerate() {
        assert_eq!(*byte, desde.wrapping_add(k as u64) as u8);
    }

    assert_eq!(set_file_pointer_ex(h, -3, &mut nueva, 2), 1);
    assert_eq!(nueva as u64, MEDIDA_GIGANTE - 3);
    let mut final_bytes = [0xFF; 8];
    assert_eq!(
        read_file(
            h,
            final_bytes.as_mut_ptr(),
            final_bytes.len() as u32,
            &mut leidos,
            0
        ),
        1
    );
    assert_eq!(leidos, 3);
    assert_eq!(
        &final_bytes[..3],
        &[
            (MEDIDA_GIGANTE - 3) as u8,
            (MEDIDA_GIGANTE - 2) as u8,
            (MEDIDA_GIGANTE - 1) as u8
        ]
    );
    assert_eq!(
        read_file(
            h,
            final_bytes.as_mut_ptr(),
            final_bytes.len() as u32,
            &mut leidos,
            0
        ),
        1
    );
    assert_eq!(leidos, 0, "EOF es exito con 0 bytes");
    assert_eq!(cerrar(h), 1);
}

// En su carpeta (`ficheros/pruebas_capa/`) y sin `#[path]`: el `../` de
// antes pasaba por una carpeta que no existe, y eso Windows lo resuelve
// en el texto pero Linux no (02-10).
mod pruebas_mapeo_archive;
