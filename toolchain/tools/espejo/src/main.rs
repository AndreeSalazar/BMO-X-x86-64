//! **ESPEJO** (2026-09-25): BMO C y BMO C++ contra GCC y Clang, programa a
//! programa, EJECUTADOS.
//!
//! El propietario: *"una app que verifique mi C y C++ por completo, para tomar
//! la base y compararse con C y C++ de terceros en tiempo real, para
//! autocompletar y ahorrar el esfuerzo"*.
//!
//! # Tres espejos y un reloj
//!
//! ```text
//!    espejo casos [c|cpp]        los programas de `casos/`: cada rasgo del
//!                                lenguaje, escrito a mano y comentado
//!    espejo azar [N] [--desde S] [--cpp]
//!                                N programas que no escribio nadie (Csmith en
//!                                chico, sin comportamiento indefinido)
//!    espejo uno <fichero>        UN programa, con lo que imprimio BMO
//!    espejo reducir <fichero>    un fallo, reducido al programa mas chico
//!                                que lo sigue dando
//!    espejo corpus <carpeta> [--cpp]
//!                                el C de un juego de verdad, pelado capa a capa
//!    espejo vigilar [lo de arriba]
//!                                EN TIEMPO REAL: vuelve a correr cada vez que
//!                                cambia el compilador, y dice que se arreglo y
//!                                que se rompio desde la vez anterior
//!    espejo                      casos de C y C++, y 200 al azar
//!    ... --informe               ademas escribe `ESPEJO.md` al lado de esto
//! ```
//!
//! # El juez
//!
//! La salida de GCC y la de Clang tienen que COINCIDIR; si no, el programa
//! depende de algo que el estandar no fija y se aparta. Y entonces BMO tiene
//! que imprimir exactamente eso. Todo es x86-64 por los dos lados: el emisor
//! de BMO y el anfitrion.
//!
//! [!] Se compara lo IMPRESO y no lo que devuelve `main`: en BMO-X `EXIT` no
//! lleva codigo (`bmo_lower::task::exit`, una operacion sin argumentos), asi
//! que ese numero no llega a ningun sitio. Un caso que quiera comprobarlo lo
//! imprime.
//!
//! Un fallo al azar se guarda en `target/espejo/fallos/` con su semilla: se
//! reproduce con `espejo azar 1 --desde <semilla>` y se mira a mano.

mod azar;
mod bmo;
mod corpus;
mod oraculo;

use std::path::{Path, PathBuf};

use bmo::{Fase, Lengua, Salida};
use oraculo::{Externo, Veredicto};

/// Lo que paso con un caso.
enum Juicio {
    Igual,
    /// La primera linea distinta: (numero, lo esperado, lo de BMO).
    Distinto(usize, String, String),
    Bmo(Fase, String),
    /// GCC y Clang no se ponen de acuerdo, o ninguno compila: no juzga.
    Apartado(String),
}

fn raiz() -> PathBuf {
    let r = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..");
    // Sin `..` en lo que se imprime: una ruta de fallo se copia y se abre.
    r.canonicalize().unwrap_or(r)
}

fn trabajo() -> PathBuf {
    let t = raiz().join("target").join("espejo");
    let _ = std::fs::create_dir_all(t.join("fallos"));
    t
}

/// **Juzga un programa**: lo corren los de fuera, y despues BMO.
fn juzgar(lengua: Lengua, ruta: &Path, externos: &[Externo], tmp: &Path) -> Juicio {
    let mut esperado: Option<String> = None;
    for x in externos {
        match x.correr(lengua, ruta, tmp) {
            Veredicto::Salida { texto } => match &esperado {
                None => esperado = Some(texto),
                Some(t) if *t == texto => {}
                Some(_) => return Juicio::Apartado(format!("{} no coincide con los demas", x.nombre)),
            },
            Veredicto::NoCompila(m) => return Juicio::Apartado(format!("{} no lo compila: {m}", x.nombre)),
            Veredicto::NoTermina => return Juicio::Apartado(format!("{} no termina", x.nombre)),
        }
    }
    let Some(texto) = esperado else {
        return Juicio::Apartado("no hay GCC ni Clang con que comparar".into());
    };
    let fuente = match std::fs::read_to_string(ruta) {
        Ok(f) => f,
        Err(e) => return Juicio::Apartado(e.to_string()),
    };
    match bmo::correr(lengua, &fuente, ruta) {
        Err((fase, m)) => Juicio::Bmo(fase, m),
        Ok(Salida { texto: b, .. }) => {
            if b != texto {
                let (mut ea, mut eb) = (texto.lines(), b.lines());
                let mut n = 1;
                loop {
                    match (ea.next(), eb.next()) {
                        (Some(x), Some(y)) if x == y => n += 1,
                        (x, y) => {
                            return Juicio::Distinto(
                                n,
                                x.unwrap_or("(nada)").to_string(),
                                y.unwrap_or("(nada)").to_string(),
                            )
                        }
                    }
                }
            } else {
                Juicio::Igual
            }
        }
    }
}

/// La cuenta de una tanda: cuantos iguales de cuantos juzgados.
#[derive(Default)]
struct Cuenta {
    iguales: usize,
    juzgados: usize,
    apartados: usize,
    /// Los que fallan: (nombre, que paso).
    fallos: Vec<(String, String)>,
}

impl Cuenta {
    fn apuntar(&mut self, nombre: &str, j: &Juicio) {
        match j {
            Juicio::Apartado(_) => {
                self.apartados += 1;
                return;
            }
            Juicio::Igual => self.iguales += 1,
            Juicio::Distinto(n, a, b) => {
                self.fallos.push((nombre.into(), format!("linea {n}: se esperaba `{a}` y BMO dio `{b}`")))
            }
            Juicio::Bmo(f, m) => self.fallos.push((nombre.into(), format!("{}: {}", f.nombre(), recorte(m, 140)))),
        }
        self.juzgados += 1;
    }
}

fn recorte(s: &str, n: usize) -> String {
    let s = s.lines().next().unwrap_or("");
    if s.chars().count() > n {
        format!("{}...", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn texto_juicio(j: &Juicio) -> String {
    match j {
        Juicio::Igual => "IGUAL".into(),
        Juicio::Distinto(n, a, b) => format!("DISTINTO en la linea {n}: se esperaba `{}` y BMO dio `{}`", recorte(a, 60), recorte(b, 60)),
        Juicio::Bmo(f, m) => format!("BMO {}: {}", f.nombre(), recorte(m, 110)),
        Juicio::Apartado(m) => format!("apartado ({})", recorte(m, 100)),
    }
}

fn nombre_lengua(l: Lengua) -> &'static str {
    match l {
        Lengua::C => "C",
        Lengua::Cpp => "C++",
    }
}

/// **REDUCIR un fallo** al programa mas chico que lo sigue dando.
///
/// Se van quitando trozos de lineas --de 32 en 32, despues de 16... hasta de
/// una en una-- y un trozo se queda fuera si el programa que queda SIGUE
/// siendo un fallo del mismo tipo: GCC y Clang lo compilan, coinciden, y BMO da
/// otra cosa. Quitar una llave suelta rompe el programa, GCC dice que no, y ese
/// trozo vuelve: no hace falta entender C para reducir C.
///
/// Es lo que convierte "el azar 4 falla" en tres lineas que se pueden mirar.
fn reducir(ruta: &Path, externos: &[Externo]) -> Option<PathBuf> {
    let lengua = if ruta.extension().and_then(|x| x.to_str()) == Some("cpp") { Lengua::Cpp } else { Lengua::C };
    let tmp = trabajo();
    let prueba = tmp.join(format!("reducir.{}", if lengua == Lengua::C { "c" } else { "cpp" }));
    let original = std::fs::read_to_string(ruta).ok()?;
    // El MISMO fallo, no uno cualquiera: si era "g3 sale distinto", tiene que
    // seguir siendo g3. Sin esto la reduccion se va detras del primer otro
    // fallo que encuentre por el camino (y lo encuentra).
    let tipo = |j: &Juicio| match j {
        Juicio::Distinto(_, a, _) => Some((0u8, a.split_whitespace().next().unwrap_or("").to_string())),
        Juicio::Bmo(f, _) => Some((1 + *f as u8, String::new())),
        _ => None,
    };
    let objetivo = tipo(&juzgar(lengua, ruta, externos, &tmp))?;
    let mut lineas: Vec<String> = original.lines().map(|l| l.to_string()).collect();
    let mut trozo = 32usize;
    let mut pruebas = 0usize;
    while trozo >= 1 {
        let mut i = 0;
        let mut quito_algo = false;
        while i < lineas.len() {
            let fin = (i + trozo).min(lineas.len());
            // Las etiquetas de un `switch` no se quitan nunca: sin ellas el
            // codigo queda ANTES del primer `case`, y ese es otro fallo de BMO
            // (el 25-09 lo encontro esto mismo) que taparia el que se busca.
            if lineas[i..fin].iter().any(|l| {
                let t = l.trim_start();
                t.starts_with("case ") || t.starts_with("default:")
            }) {
                if trozo == 1 {
                    i += 1;
                } else {
                    // Se intenta con trozos mas chicos en la siguiente vuelta.
                    i += trozo;
                }
                continue;
            }
            let candidato: Vec<String> = lineas[..i].iter().chain(lineas[fin..].iter()).cloned().collect();
            let _ = std::fs::write(&prueba, candidato.join("\n") + "\n");
            pruebas += 1;
            if tipo(&juzgar(lengua, &prueba, externos, &tmp)).as_ref() == Some(&objetivo) {
                lineas = candidato;
                quito_algo = true;
            } else {
                i += trozo;
            }
        }
        if !quito_algo {
            trozo /= 2;
        }
    }
    let nombre = ruta.file_stem().and_then(|n| n.to_str()).unwrap_or("caso");
    let salida = tmp.join("fallos").join(format!("{nombre}.reducido.{}", if lengua == Lengua::C { "c" } else { "cpp" }));
    std::fs::write(&salida, lineas.join("\n") + "\n").ok()?;
    println!("  reducido de {} a {} lineas en {pruebas} pruebas", original.lines().count(), lineas.len());
    Some(salida)
}

/// **Los casos escritos a mano**, de `casos/c` o `casos/cpp`.
fn casos(lengua: Lengua, externos: &[Externo], ver: bool) -> Cuenta {
    let (dir, ext) = match lengua {
        Lengua::C => ("c", "c"),
        Lengua::Cpp => ("cpp", "cpp"),
    };
    let carpeta = Path::new(env!("CARGO_MANIFEST_DIR")).join("casos").join(dir);
    let mut rutas: Vec<PathBuf> = std::fs::read_dir(&carpeta)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    rutas.retain(|p| p.extension().and_then(|x| x.to_str()) == Some(ext));
    rutas.sort();
    let tmp = trabajo();
    let mut c = Cuenta::default();
    for r in &rutas {
        let nombre = r.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let j = juzgar(lengua, r, externos, &tmp);
        if ver {
            println!("  {:<28} {}", nombre, texto_juicio(&j));
        }
        c.apuntar(&nombre, &j);
    }
    c
}

/// **N programas al azar** desde la semilla `desde`.
fn azar(lengua: Lengua, n: u64, desde: u64, externos: &[Externo], ver: bool) -> Cuenta {
    let tmp = trabajo();
    let ext = if lengua == Lengua::C { "c" } else { "cpp" };
    let ruta = tmp.join(format!("azar.{ext}"));
    let mut c = Cuenta::default();
    for s in desde..desde + n {
        let _ = std::fs::write(&ruta, azar::programa(s));
        let j = juzgar(lengua, &ruta, externos, &tmp);
        let nombre = format!("azar {s}");
        if !matches!(j, Juicio::Igual | Juicio::Apartado(_)) {
            // Se guarda para mirarlo: el programa y lo que paso.
            let guardado = tmp.join("fallos").join(format!("azar_{s}.{ext}"));
            let _ = std::fs::copy(&ruta, &guardado);
            if ver {
                println!("  {:<12} {}   -> {}", nombre, texto_juicio(&j), guardado.display());
            }
        }
        c.apuntar(&nombre, &j);
    }
    c
}

/// Una linea de resumen que `vigilar` sabe leer: `RESUMEN clave iguales juzgados`.
fn resumen(clave: &str, c: &Cuenta) {
    println!("RESUMEN {clave} {} {}", c.iguales, c.juzgados);
    for (n, m) in &c.fallos {
        println!("FALLO {clave} {n} | {m}");
    }
}

fn tabla(titulo: &str, c: &Cuenta) {
    let pct = if c.juzgados == 0 { 0 } else { c.iguales * 100 / c.juzgados };
    println!(
        "== {titulo}: {} de {} IGUALES ({pct} %){}",
        c.iguales,
        c.juzgados,
        if c.apartados > 0 { format!(", {} apartados", c.apartados) } else { String::new() }
    );
}

/// **Tiempo real**: mira las fuentes del compilador y, cuando cambian, vuelve
/// a correr el espejo (recompilado: el compilador va DENTRO de este binario) y
/// dice que se arreglo y que se rompio.
fn vigilar(args: &[String]) -> ! {
    let r = raiz();
    let vigiladas = [
        "toolchain/lang/c/src",
        "toolchain/lang/c/emisor-x86_64/src",
        "toolchain/lang/cpp/src",
        "toolchain/lang/cpp/emisor-x86_64/src",
        "toolchain/forge/sem-asm/tables/standards/C",
        "toolchain/forge/bmo-lower/src",
        "toolchain/tools/espejo/casos",
    ];
    let huella = || -> u128 {
        let mut h = 0u128;
        for d in &vigiladas {
            let mut pila = vec![r.join(d)];
            while let Some(p) = pila.pop() {
                let Ok(rd) = std::fs::read_dir(&p) else { continue };
                for e in rd.flatten() {
                    let q = e.path();
                    if q.is_dir() {
                        pila.push(q);
                    } else if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                        let t = m.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
                        h = h.wrapping_mul(31).wrapping_add(t);
                    }
                }
            }
        }
        h
    };
    let mut antes: std::collections::BTreeMap<String, String> = Default::default();
    let mut ultima = 0u128;
    let mut primera = true;
    println!("espejo vigilar: mirando el compilador de C y C++ (Ctrl+C para salir)");
    loop {
        let h = huella();
        if h != ultima {
            ultima = h;
            println!("\n-- cambio el compilador: se vuelve a mirar ------------------------");
            let salida = std::process::Command::new("cargo")
                .args(["run", "--release", "-q", "-p", "bmo-espejo", "--"])
                .args(args)
                .arg("--resumen")
                .current_dir(&r)
                .output();
            match salida {
                Err(e) => println!("  no se pudo lanzar cargo: {e}"),
                Ok(o) if !o.status.success() && o.stdout.is_empty() => {
                    println!("  el compilador no compila ahora mismo:");
                    for l in String::from_utf8_lossy(&o.stderr).lines().filter(|l| l.starts_with("error")).take(5) {
                        println!("    {l}");
                    }
                }
                Ok(o) => {
                    let texto = String::from_utf8_lossy(&o.stdout);
                    let mut ahora: std::collections::BTreeMap<String, String> = Default::default();
                    for l in texto.lines() {
                        if let Some(r) = l.strip_prefix("RESUMEN ") {
                            let p: Vec<&str> = r.split_whitespace().collect();
                            if p.len() == 3 {
                                println!("  {:<10} {} de {} iguales", p[0], p[1], p[2]);
                            }
                        } else if let Some(r) = l.strip_prefix("FALLO ") {
                            if let Some((k, m)) = r.split_once(" | ") {
                                ahora.insert(k.to_string(), m.to_string());
                            }
                        }
                    }
                    if primera {
                        // La primera vez no hay con que comparar: lo que falla
                        // FALLA, no se ha roto.
                        for (k, m) in &ahora {
                            println!("  falla      {k}: {m}");
                        }
                        primera = false;
                    } else {
                        let mut cambio = false;
                        for k in antes.keys().filter(|k| !ahora.contains_key(*k)) {
                            println!("  + ARREGLADO  {k}");
                            cambio = true;
                        }
                        for (k, m) in ahora.iter().filter(|(k, _)| !antes.contains_key(*k)) {
                            println!("  - ROTO       {k}: {m}");
                            cambio = true;
                        }
                        if !cambio {
                            println!("  (nada arreglado ni roto desde la vez anterior)");
                        }
                    }
                    antes = ahora;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// `ESPEJO.md`: la tabla, para leerla sin correr nada.
fn informe(partes: &[(String, Cuenta)], corpus: Option<(String, Vec<(String, usize, String)>, usize, usize)>) {
    let mut s = String::from(
        "# ESPEJO -- BMO C y C++ contra GCC y Clang\n\n\
         > **AUTO-GENERADO** por `toolchain/tools/espejo` (`cargo run --release -p bmo-espejo -- --informe`).\n\
         > No editar a mano.\n\n\
         Cada programa se compila y se EJECUTA por los dos lados --GCC y Clang en el\n\
         anfitrion, BMO en el emulador de x86-64-- y se compara lo que imprime y lo\n\
         que devuelve `main`. Si GCC y Clang no coinciden, el caso se aparta.\n\n",
    );
    for (t, c) in partes {
        let pct = if c.juzgados == 0 { 0 } else { c.iguales * 100 / c.juzgados };
        s.push_str(&format!("## {t}: **{} de {}** iguales ({pct} %)\n\n", c.iguales, c.juzgados));
        if !c.fallos.is_empty() {
            s.push_str("| caso | que paso |\n|---|---|\n");
            for (n, m) in c.fallos.iter().take(40) {
                s.push_str(&format!("| {n} | {} |\n", m.replace('|', "/")));
            }
            s.push('\n');
        }
    }
    if let Some((dir, filas, ok, total)) = corpus {
        s.push_str(&format!("## Corpus `{dir}`: **{ok} de {total}** ficheros compilan\n\n| para a | error | ejemplo |\n|---|---|---|\n"));
        for (forma, n, ej) in filas {
            s.push_str(&format!("| {n} | {} | {ej} |\n", forma.replace('|', "/")));
        }
        s.push('\n');
    }
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join("ESPEJO.md");
    match std::fs::write(&ruta, a_ascii(&s)) {
        Ok(()) => println!("informe: {}", ruta.display()),
        Err(e) => println!("informe: no se pudo escribir: {e}"),
    }
}

/// El informe va en ASCII, como todo el arbol (`ascii_sweep`): los mensajes
/// del compilador de C++ traen tildes y rayas largas, y se copian tal cual.
fn a_ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' => c,
            c if c.is_ascii() => c,
            '\u{e1}' | '\u{e0}' => 'a',
            '\u{e9}' | '\u{e8}' => 'e',
            '\u{ed}' | '\u{ec}' => 'i',
            '\u{f3}' | '\u{f2}' => 'o',
            '\u{fa}' | '\u{f9}' | '\u{fc}' => 'u',
            '\u{c1}' => 'A',
            '\u{c9}' => 'E',
            '\u{cd}' => 'I',
            '\u{d3}' => 'O',
            '\u{da}' => 'U',
            '\u{f1}' => 'n',
            '\u{d1}' => 'N',
            _ => '-',
        })
        .collect()
}

fn main() {
    // Los panicos del compilador o del emulador son DATOS aqui: se recogen con
    // `catch_unwind` y se dicen en su fila. Sin esto llenarian la pantalla.
    std::panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let resumen_modo = args.iter().any(|a| a == "--resumen");
    let con_informe = args.iter().any(|a| a == "--informe");
    let cpp = args.iter().any(|a| a == "--cpp");
    let lengua = if cpp { Lengua::Cpp } else { Lengua::C };
    let libres: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let valor = |nombre: &str| -> Option<u64> {
        args.iter().position(|a| a == nombre).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok())
    };
    let ver = !resumen_modo;

    let modo = libres.first().map(|s| s.as_str()).unwrap_or("todo");
    if modo == "vigilar" {
        let resto: Vec<String> = args.iter().skip(1).filter(|a| *a != "--resumen").cloned().collect();
        vigilar(&resto);
    }
    let ext_c = oraculo::buscar(Lengua::C);
    let ext_cpp = oraculo::buscar(Lengua::Cpp);
    if ver {
        let nombres = |v: &[Externo]| v.iter().map(|x| x.nombre).collect::<Vec<_>>().join(" y ");
        println!("espejo: C contra {}; C++ contra {}", nombres(&ext_c), nombres(&ext_cpp));
    }
    let ext = |l: Lengua| if l == Lengua::C { &ext_c } else { &ext_cpp };

    let mut partes: Vec<(String, Cuenta)> = Vec::new();
    let mut corpus_inf = None;
    match modo {
        "casos" => {
            let l = match libres.get(1).map(|s| s.as_str()) {
                Some("cpp") | Some("c++") => Lengua::Cpp,
                _ => lengua,
            };
            let c = casos(l, ext(l), ver);
            partes.push((format!("casos de {}", nombre_lengua(l)), c));
        }
        "uno" => {
            let Some(f) = libres.get(1) else {
                eprintln!("espejo uno <fichero.c|.cpp>");
                std::process::exit(2);
            };
            let ruta = PathBuf::from(f.as_str());
            let l = if ruta.extension().and_then(|x| x.to_str()) == Some("cpp") { Lengua::Cpp } else { lengua };
            let j = juzgar(l, &ruta, ext(l), &trabajo());
            println!("  {}  {}", f, texto_juicio(&j));
            if let Ok(fuente) = std::fs::read_to_string(&ruta) {
                if let Ok(s) = bmo::correr(l, &fuente, &ruta) {
                    println!("  BMO imprimio ({} instrucciones):", s.pasos);
                    for x in s.texto.lines().take(20) {
                        println!("    {x}");
                    }
                }
            }
            let mut c = Cuenta::default();
            c.apuntar(f, &j);
            partes.push((format!("uno {f}"), c));
        }
        "reducir" => {
            let Some(f) = libres.get(1) else {
                eprintln!("espejo reducir <fichero que falla>");
                std::process::exit(2);
            };
            let ruta = PathBuf::from(f.as_str());
            let l = if ruta.extension().and_then(|x| x.to_str()) == Some("cpp") { Lengua::Cpp } else { Lengua::C };
            match reducir(&ruta, ext(l)) {
                None => println!("  {f}: no es un fallo de BMO (o GCC y Clang no se ponen de acuerdo): nada que reducir"),
                Some(r) => {
                    println!("  {}", r.display());
                    if let Ok(t) = std::fs::read_to_string(&r) {
                        for x in t.lines() {
                            println!("    {x}");
                        }
                    }
                    let j = juzgar(l, &r, ext(l), &trabajo());
                    println!("  {}", texto_juicio(&j));
                }
            }
            return;
        }
        "azar" => {
            let n = libres.get(1).and_then(|s| s.parse().ok()).unwrap_or(100);
            let desde = valor("--desde").unwrap_or(1);
            let c = azar(lengua, n, desde, ext(lengua), ver);
            partes.push((format!("azar {} ({n} desde {desde})", nombre_lengua(lengua)), c));
        }
        "corpus" => {
            let Some(dir) = libres.get(1) else {
                eprintln!("espejo corpus <carpeta con .c>");
                std::process::exit(2);
            };
            match corpus::pelar(Path::new(dir.as_str()), lengua, &trabajo()) {
                Err(e) => {
                    eprintln!("espejo corpus: {e}");
                    std::process::exit(2);
                }
                Ok(fs) => {
                    let ok = fs.iter().filter(|f| f.resultado.is_ok()).count();
                    let tapadas: usize = fs.iter().map(|f| f.tapadas).sum();
                    let filas = corpus::agrupar(&fs);
                    if ver {
                        println!("== corpus {dir}: {ok} de {} ficheros compilan ({tapadas} cabeceras tapadas)", fs.len());
                        for (forma, n, ej) in filas.iter().take(25) {
                            println!("  {n:>4}  {forma}   (p. ej. {ej})");
                        }
                    }
                    let mut c = Cuenta { iguales: ok, juzgados: fs.len(), ..Default::default() };
                    for f in &fs {
                        if let Err((fase, m)) = &f.resultado {
                            c.fallos.push((f.nombre.clone(), format!("{}: {}", fase.nombre(), recorte(m, 140))));
                        }
                    }
                    if resumen_modo {
                        resumen("corpus", &c);
                    }
                    corpus_inf = Some((dir.to_string(), filas, ok, fs.len()));
                }
            }
        }
        _ => {
            for l in [Lengua::C, Lengua::Cpp] {
                if ver {
                    println!("-- casos de {} --", nombre_lengua(l));
                }
                let c = casos(l, ext(l), ver);
                partes.push((format!("casos de {}", nombre_lengua(l)), c));
            }
            if ver {
                println!("-- 200 programas al azar (C) --");
            }
            let c = azar(Lengua::C, 200, 1, &ext_c, ver);
            partes.push(("azar C (200 desde 1)".into(), c));
        }
    }
    for (t, c) in &partes {
        if resumen_modo {
            let clave = t.split_whitespace().take(3).collect::<Vec<_>>().join("_");
            resumen(&clave, c);
        } else {
            tabla(t, c);
        }
    }
    if con_informe {
        informe(&partes, corpus_inf);
    }
}
