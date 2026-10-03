//! **ESPEJO de COBOL** (2026-10-03, CM0 de `docs/plan/PLAN_COBOL_MAESTRO.md`):
//! BMO COBOL contra GnuCOBOL, programa a programa, EJECUTADOS.
//!
//! El propietario: *"el ESPEJO de COBOL"*, para saber de verdad en que parte
//! esta BMO COBOL -- y para poder decirle a un banco "da lo mismo que el
//! COBOL que ya usais".
//!
//! ```text
//!    espejo-cobol                 los casos de `casos/` y los ejemplos de
//!                                 `toolchain/lang/cobol/examples`
//!    espejo-cobol uno <f.cob>     UN programa, con lo que imprimio cada uno
//!    ... --informe                ademas escribe `ESPEJO_COBOL.md` al lado
//! ```
//!
//! # El juez y sus veredictos
//!
//! ```text
//!    IGUAL           lo mismo, byte a byte
//!    SOLO DISPLAY    solo difiere como se muestra un numero SIN mascara
//!                    (tarea 1.5 de PLAN_BANCA): el valor es el mismo
//!    DISTINTO        un valor, o una linea, distinta: BMO esta MAL
//!    BMO NO          BMO no lo compila o no lo corre (y GnuCOBOL si)
//!    NO ESTANDAR     GnuCOBOL lo RECHAZA y BMO lo acepta: BMO deja pasar algo
//!                    que no es COBOL. Tambien es un fallo, del otro lado
//!    NO JUZGA        GnuCOBOL no esta, o los dos lo rechazan
//! ```
//!
//! La entrada de un `ACCEPT` va en `<nombre>.entrada`, al lado del `.cob` o en
//! `entradas/` de este espejo. Los ficheros de datos son los de
//! `toolchain/lang/cobol/examples/datos`, con la misma ruta (`datos/x.txt`) en
//! el emulador y en la carpeta de trabajo de GnuCOBOL.

mod bmo;
mod gnu;
mod juez;

use std::path::{Path, PathBuf};

use juez::Comparacion;

/// Lo que paso con un programa.
enum Juicio {
    Igual,
    SoloDisplay(usize, String, String),
    Distinto(usize, String, String),
    BmoNo(bmo::Fase, String),
    NoEstandar(String),
    NoJuzga(String),
}

impl Juicio {
    fn marca(&self) -> &'static str {
        match self {
            Juicio::Igual => "IGUAL",
            Juicio::SoloDisplay(..) => "SOLO DISPLAY",
            Juicio::Distinto(..) => "DISTINTO",
            Juicio::BmoNo(..) => "BMO NO",
            Juicio::NoEstandar(..) => "NO ESTANDAR",
            Juicio::NoJuzga(..) => "NO JUZGA",
        }
    }

    fn detalle(&self) -> String {
        match self {
            Juicio::Igual => String::new(),
            Juicio::SoloDisplay(k, g, b) | Juicio::Distinto(k, g, b) => format!("linea {k}: GnuCOBOL `{g}` / BMO `{b}`"),
            Juicio::BmoNo(f, m) => format!("{}: {m}", f.nombre()),
            Juicio::NoEstandar(m) => format!("GnuCOBOL: {m}"),
            Juicio::NoJuzga(m) => m.clone(),
        }
    }

    /// Lo que cuenta como fallo de BMO.
    fn falla(&self) -> bool {
        matches!(self, Juicio::Distinto(..) | Juicio::BmoNo(..) | Juicio::NoEstandar(..))
    }
}

/// El informe va en ASCII, como todo el repositorio: un mensaje con tilde
/// pierde la tilde, y lo demas que no sea ASCII sale como `?`.
fn ascii(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            '\u{e1}' => 'a',
            '\u{e9}' => 'e',
            '\u{ed}' => 'i',
            '\u{f3}' => 'o',
            '\u{fa}' => 'u',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Los programas: los casos de este espejo y los ejemplos de la casa.
fn programas() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let mut de = |d: PathBuf| {
        if let Ok(l) = std::fs::read_dir(&d) {
            let mut fs: Vec<PathBuf> = l.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "cob")).collect();
            fs.sort();
            v.extend(fs);
        }
    };
    de(Path::new(env!("CARGO_MANIFEST_DIR")).join("casos"));
    let ej = raiz().join("toolchain/lang/cobol/examples");
    let mut niveles: Vec<PathBuf> = std::fs::read_dir(&ej).map(|l| l.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "datos")).collect()).unwrap_or_default();
    // Por su numero de nivel: 1, 2, ..., 10, 11.
    niveles.sort_by_key(|p| p.file_name().and_then(|n| n.to_str()).and_then(|n| n.split('-').next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(99));
    for n in niveles {
        de(n);
    }
    v
}

/// La entrada de un `ACCEPT`, si el programa la tiene.
fn entrada_de(cob: &Path) -> String {
    let al_lado = cob.with_extension("entrada");
    let aqui = Path::new(env!("CARGO_MANIFEST_DIR")).join("entradas").join(cob.with_extension("entrada").file_name().unwrap_or_default());
    std::fs::read_to_string(al_lado).or_else(|_| std::fs::read_to_string(aqui)).unwrap_or_default()
}

/// **Un programa por los dos lados.**
fn juzgar(cob: &Path, cobc: Option<&Path>, tmp: &Path) -> (Juicio, Option<String>, Option<String>) {
    let fuente = match std::fs::read_to_string(cob) {
        Ok(f) => f,
        Err(e) => return (Juicio::NoJuzga(format!("no se lee: {e}")), None, None),
    };
    let copias = vec![cob.parent().unwrap_or(Path::new(".")).to_path_buf(), raiz().join("toolchain/lang/cobol/copy")];
    let entrada = entrada_de(cob);
    let carpeta_datos = raiz().join("toolchain/lang/cobol/examples/datos");
    let datos = bmo::datos_de(&carpeta_datos, "datos");
    let de_bmo = bmo::correr(&fuente, &copias, &entrada, &datos);
    let Some(cobc) = cobc else {
        return (Juicio::NoJuzga("no hay GnuCOBOL (cobc) en el PATH".into()), de_bmo.ok(), None);
    };
    // La carpeta de trabajo de GnuCOBOL, con los datos donde el programa los busca.
    let _ = std::fs::create_dir_all(tmp.join("datos"));
    for (ruta, bytes) in &datos {
        let _ = std::fs::write(tmp.join(ruta), bytes);
    }
    let de_gnu = gnu::correr(cobc, cob, &copias, &entrada, tmp);
    let juicio = match (&de_gnu, &de_bmo) {
        (gnu::Veredicto::Salida(g), Ok(b)) => match juez::comparar(g, b) {
            Comparacion::Igual => Juicio::Igual,
            Comparacion::SoloDisplay(k, x, y) => Juicio::SoloDisplay(k, x, y),
            Comparacion::Distinto(k, x, y) => Juicio::Distinto(k, x, y),
        },
        (gnu::Veredicto::Salida(_), Err((f, m))) => Juicio::BmoNo(*f, m.clone()),
        (gnu::Veredicto::NoCompila(m), Ok(_)) => Juicio::NoEstandar(m.clone()),
        (gnu::Veredicto::NoCompila(m), Err(_)) => Juicio::NoJuzga(format!("los dos lo rechazan ({m})")),
        (gnu::Veredicto::NoTermina, _) => Juicio::NoJuzga("GnuCOBOL no termino en 10 s".into()),
        (gnu::Veredicto::Revienta, _) => Juicio::NoJuzga("GnuCOBOL revento".into()),
    };
    let g = match de_gnu {
        gnu::Veredicto::Salida(s) => Some(s),
        _ => None,
    };
    (juicio, de_bmo.ok(), g)
}

fn nombre(cob: &Path) -> String {
    let r = raiz();
    let r = std::fs::canonicalize(&r).unwrap_or(r);
    let c = std::fs::canonicalize(cob).unwrap_or(cob.to_path_buf());
    c.strip_prefix(&r).map(|p| p.display().to_string()).unwrap_or_else(|_| cob.display().to_string())
}

fn main() {
    // Un panico del compilador o del emulador ya es un veredicto (con su
    // mensaje): no hace falta la traza en la pantalla.
    std::panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let informe = args.iter().any(|a| a == "--informe");
    let cobc = gnu::buscar();
    let tmp = std::env::temp_dir().join(format!("espejo-cobol-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);

    if args.first().map(String::as_str) == Some("uno") {
        let Some(f) = args.get(1) else {
            eprintln!("uso: espejo-cobol uno <fichero.cob>");
            std::process::exit(2);
        };
        let (j, b, g) = juzgar(Path::new(f), cobc.as_deref(), &tmp);
        println!("== {} {}", j.marca(), j.detalle());
        println!("-- GnuCOBOL --\n{}", g.as_deref().unwrap_or("(nada)"));
        println!("-- BMO --\n{}", b.as_deref().unwrap_or("(nada)"));
        let _ = std::fs::remove_dir_all(&tmp);
        std::process::exit(if j.falla() { 1 } else { 0 });
    }

    match &cobc {
        Some(c) => println!("ESPEJO de COBOL: BMO COBOL contra GnuCOBOL ({})", c.display()),
        None => println!("ESPEJO de COBOL: SIN GnuCOBOL (cobc) -- se compila y se corre BMO, pero no se juzga"),
    }
    let mut filas = Vec::new();
    for cob in programas() {
        let (j, _, _) = juzgar(&cob, cobc.as_deref(), &tmp);
        let n = nombre(&cob);
        println!("  {:<13} {:<52} {}", j.marca(), n, j.detalle());
        filas.push((n, j));
    }
    let cuenta = |m: &str| filas.iter().filter(|(_, j)| j.marca() == m).count();
    let resumen = format!(
        "{} programas: {} IGUAL, {} SOLO DISPLAY, {} DISTINTO, {} BMO NO, {} NO ESTANDAR, {} NO JUZGA",
        filas.len(),
        cuenta("IGUAL"),
        cuenta("SOLO DISPLAY"),
        cuenta("DISTINTO"),
        cuenta("BMO NO"),
        cuenta("NO ESTANDAR"),
        cuenta("NO JUZGA")
    );
    println!("\n{resumen}");
    if informe {
        let mut md = String::from("# ESPEJO de COBOL -- el ultimo informe\n\n");
        md.push_str("> Lo escribe `cargo run -p bmo-espejo-cobol -- --informe` (CM0 de\n");
        md.push_str("> `docs/plan/PLAN_COBOL_MAESTRO.md`). BMO COBOL, ejecutado en el emulador de\n");
        md.push_str("> x86-64, contra GnuCOBOL. No se edita a mano.\n\n");
        md.push_str(&format!("**{resumen}**\n\n| veredicto | programa | detalle |\n|---|---|---|\n"));
        for (n, j) in &filas {
            md.push_str(&format!("| {} | `{}` | {} |\n", j.marca(), n, ascii(&j.detalle()).replace('|', "\\|")));
        }
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join("ESPEJO_COBOL.md");
        if std::fs::write(&ruta, md).is_ok() {
            println!("informe: {}", ruta.display());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    std::process::exit(if filas.iter().any(|(_, j)| j.falla()) { 1 } else { 0 });
}
