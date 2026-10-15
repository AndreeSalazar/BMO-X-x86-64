//! ** A4 DE `docs/plan/EL_FOCO.md` (14-10): TITAN++ TOMA LAS LLAMADAS A LA
//! RAM. Todo lo que un programa le pide de memoria al kernel -- el monton de
//! sus listas, su ventana, su lamina de VERRANO, lo tenido y un fichero --
//! sale por UNA puerta: la subrutina de `src/e1/ram.rs`, emitida una vez.
//!
//! Dos jueces, y no se hablan:
//!
//! ```text
//!    el fuente      ningun fichero del emisor, salvo ram.rs, nombra la
//!                   operacion de pedir memoria
//!    el programa    corriendo en el emulador, TODAS sus peticiones de RAM
//!                   salen de la MISMA instruccion (`ObservedSyscall::rip`)
//! ```

use bmo_abi::syscalls::surface::{CURRENT_TASK, TASK_OP_MEMORIA_PEDIR};
use bmo_lower::emu::{cargar_bex, run};

const SCREEN: &str = "[package]\nname = \"x\"\n\n[permissions]\nscreen = true\n";

/// Los cinco caminos de E1 en un programa: la ventana, el monton de la
/// lista, lo tenido, el fichero y la lamina. Son cinco peticiones; el kernel
/// da ocho (y el emulador, desde el 14-10, tambien: decia cuatro).
const CINCO: &str = "mod main \"pide ram por sus cinco caminos\"
use director

fn main()
    if not director.ventana(64, 32)
        print(\"sin ventana\")
        return
    let mut l: [int] = []
    for i in range(100)
        push(mut l, i)
    print(len(l))
    if director.fichero(\"datos/ejemplo.lam\")
        print(director.medida())
    print(director.lamina(6))
";

/// Corre `src` (lanzado por el escritorio, con un fichero en el disco) y da
/// lo que escribio y, de cada peticion de RAM, sus bytes y de donde salio.
fn peticiones(src: &str) -> (String, Vec<(u64, u64)>) {
    let bex = bmo_titan_x86_64::build_package("src/main.titan", src, &mut |p| (p == "Titan.toml").then(|| SCREEN.to_string())).unwrap_or_else(|e| panic!("{:?}", e));
    let mut m = cargar_bex(&bex).expect("el .bex carga");
    m.padre = 7;
    m.archivos.insert("datos/ejemplo.lam".into(), b"LAMINA 1 2 0\n".to_vec());
    let m = run(m, 4_000_000);
    assert!(m.exited, "{}", m.console);
    let p = m.syscalls.iter().filter(|c| c.capability == CURRENT_TASK && c.operation == TASK_OP_MEMORIA_PEDIR).map(|c| (c.arg0, c.rip)).collect();
    (m.console, p)
}

/// Todas salen de UNA instruccion.
fn una_puerta(p: &[(u64, u64)]) {
    assert!(p.iter().all(|&(_, rip)| rip == p[0].1), "una peticion de RAM salio por otra puerta: {:?}", p);
}

/// El juez del FUENTE: la operacion de pedir memoria, nombrada en codigo
/// (no en un comentario), solo en `ram.rs`.
#[test]
fn only_ram_rs_names_the_memory_request() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut quien = Vec::new();
    let mut vistos = 0;
    let mut pila = vec![dir];
    while let Some(d) = pila.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                pila.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            vistos += 1;
            let texto = std::fs::read_to_string(&p).unwrap();
            if texto.lines().any(|l| !l.trim_start().starts_with("//") && l.contains("TASK_OP_MEMORIA_PEDIR")) {
                quien.push(p.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    assert!(vistos > 10, "el juez no ha mirado nada ({vistos} ficheros)");
    assert_eq!(quien, vec!["ram.rs".to_string()], "solo la puerta pide RAM");
}

/// El juez del PROGRAMA: corriendo, cada peticion de RAM sale de la misma
/// instruccion, por los cinco caminos.
#[test]
fn every_ram_request_leaves_by_one_door() {
    let (consola, p) = peticiones(CINCO);
    assert_eq!(consola, "100\n13\ntrue\n");
    // La ventana (64 x 32 pixeles de 4 B, su cabecera y su buzon), el primer
    // trozo del monton, lo tenido (80 B desde Q2: el asa del sonido), el fichero (sus 13 bytes) y la
    // lamina de VERRANO (su cabecera y dos ranuras de 6 vertices).
    assert_eq!(p.iter().map(|x| x.0).skip(1).collect::<Vec<_>>(), vec![16 << 20, 80, 13, 64 + 2 * 6 * 32 + 16]);
    una_puerta(&p);
}
