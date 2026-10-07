//! A9b (06-10): la HUELLA del codigo de la casa (`BMO_HUELLA_FUENTE`). Los
//! mapas que la CPU recuerda (`src/cifra.rs`: lo que entendio de cada PSO)
//! se guardan con ella; si cambia UNA linea de este crate -- el compilador
//! de DXIL o de SM5, el enlace, cualquier cosa --, los de antes ya no se
//! creen y se vuelve a compilar. Asi nadie tiene que acordarse de subir un
//! numero de version.
//!
//! FNV-1a de 64 bits sobre la ruta y los bytes de cada `.rs` de `src/`, en
//! orden; los `\r` se quitan (el mismo codigo en Windows da la misma huella).

use std::path::{Path, PathBuf};

fn ficheros(dir: &Path, v: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("src se lee") {
        let p = e.expect("una entrada de src").path();
        if p.is_dir() {
            ficheros(&p, v);
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
}

fn main() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut v = Vec::new();
    ficheros(&raiz.join("src"), &mut v);
    v.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut sumar = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for p in &v {
        let rel = p.strip_prefix(raiz).expect("dentro del crate").to_string_lossy().replace('\\', "/");
        rel.bytes().for_each(&mut sumar);
        sumar(0);
        std::fs::read(p).expect("un .rs se lee").into_iter().filter(|&b| b != b'\r').for_each(&mut sumar);
        sumar(0);
    }
    println!("cargo:rustc-env=BMO_HUELLA_FUENTE={h:016x}");
    println!("cargo:rerun-if-changed=src");
}
