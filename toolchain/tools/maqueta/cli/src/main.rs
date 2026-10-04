//! # `maqueta` -- el binario
//!
//! generacion: ninguna -- el binario que las junta
//!
//! ```text
//!   cargo run -p bmo-maqueta -- entrada.maqueta salida.rs
//! ```
//!
//! Las cinco generaciones en orden, y el veredicto ANTES de emitir: un emisor
//! que escribe una maquetacion que el bisnieto rechaza estaria escribiendo el
//! fallo en un fichero que despues nadie vuelve a mirar.
//!
//! El artefacto se COMMITEA, como `font16_data.rs` de `fontgen`. Es la
//! convencion de esta casa: el generador se puede volver a correr, y mientras
//! tanto el arbol no depende de que alguien lo corra.

use std::process::ExitCode;

mod cobertura;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // *** `--paleta`: emitir los COLORES en vez de la maquetacion.
    //
    // ** Son dos preguntas sobre el mismo fichero --"donde va esto" y "de que
    // color es"-- y `tema.maqueta` solo contesta la segunda: no tiene una sola
    // caja. Pasarlo por el emisor de siempre daria un modulo que pinta el
    // vacio, que compila y no sirve.
    //
    // [!] Y es una bandera y no un ejecutable aparte a proposito: el LEXER, el
    // parser y los diagnosticos son los mismos, y un segundo binario habria
    // acabado con su propia copia de `procedencia()` -- el fallo que un
    // guardian ya cazo el 18-08.
    // ** `--cobertura`: cuanto del CSS de una maqueta HTML acepta MAQUETA,
    // declaracion a declaracion y con el compilador de verdad. Ver
    // `cobertura.rs`.
    if args.first().map(String::as_str) == Some("--cobertura") {
        return cobertura(&args[1..]);
    }
    // ** P3 (04-10): `--estado abierta` pinta ESE estado; `--tira reposo
    // abierta 8` pinta 8 fotogramas de la transicion, uno debajo de otro.
    let mut estado: Option<String> = None;
    let mut tira: Option<(String, String, u32)> = None;
    if let Some(k) = args.iter().position(|a| a == "--estado") {
        if k + 1 < args.len() {
            estado = Some(args.remove(k + 1));
        }
        args.remove(k);
    }
    if let Some(k) = args.iter().position(|a| a == "--tira") {
        if k + 3 < args.len() {
            let n = args.remove(k + 3).parse().unwrap_or(8u32).clamp(2, 60);
            let b = args.remove(k + 2);
            let a = args.remove(k + 1);
            tira = Some((a, b, n));
        }
        args.remove(k);
    }
    let solo_paleta = args.first().map(|a| a == "--paleta").unwrap_or(false);
    if solo_paleta {
        args.remove(0);
    }
    // ** `--foto` (MAQUETA 2): la cara PINTADA a PNG, con el pintor y la
    // letra de verdad -- la mitad que el ESPEJO de cara compara con el
    // navegador. `--foto-cara`: lo mismo, pero pasando por los BYTES que
    // viajan (emisor B), para ver que no se pierde nada por el camino.
    let foto = match args.first().map(String::as_str) {
        Some("--foto") => Some(false),
        Some("--foto-cara") => Some(true),
        _ => None,
    };
    if foto.is_some() {
        args.remove(0);
    }
    let mut args = args.into_iter();
    let (Some(entrada), Some(salida)) = (args.next(), args.next()) else {
        eprintln!("uso: maqueta [--paleta | --foto | --foto-cara] <entrada.maqueta> <salida.rs | salida.png>\n     maqueta --cobertura <maqueta.html>...");
        return ExitCode::from(2);
    };

    let src = match std::fs::read(&entrada) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("maqueta: no puedo leer {entrada}: {e}");
            return ExitCode::from(2);
        }
    };

    let doc = match bmo_maqueta_node::parse(&src) {
        Ok(d) => d,
        Err(e) => return fallo(&entrada, &src, &e),
    };
    // La paleta se emite del DOCUMENTO, antes de la cascada: un color con
    // nombre no compite con nadie ni tiene sitio, asi que maquetarlo seria
    // trabajo tirado -- y un `judge` sobre un arbol vacio, un reparo inventado.
    if solo_paleta {
        let codigo = bmo_maqueta_emit::paleta::paleta(&procedencia(&entrada), &doc);
        if let Err(e) = std::fs::write(&salida, codigo) {
            eprintln!("maqueta: no puedo escribir {salida}: {e}");
            return ExitCode::from(2);
        }
        eprintln!("maqueta: paleta de {entrada} -> {salida}");
        return ExitCode::SUCCESS;
    }

    // ** Por COMPONE siempre (04-10): una maqueta sin `<usa>` sale igual que
    // antes (lo prueba `sin_piezas_es_lo_de_siempre`), y una con piezas
    // compila cada una SOLA antes de ponerla. El fallo se cuenta en el
    // fichero donde esta, que puede ser una pieza y no este.
    let _ = doc;
    let compilado = if estado.is_some() || tira.is_some() {
        bmo_maqueta_compone::compilar_estados(std::path::Path::new(&entrada)).and_then(|e| {
            if let Some((a, b, n)) = &tira {
                let (Some(la), Some(lb)) = (e.de(a), e.de(b)) else {
                    eprintln!("maqueta: no hay estado `{a}` o `{b}` en {entrada}");
                    std::process::exit(2);
                };
                std::process::exit(match tira_png(&salida, la, lb, *n) {
                    true => 0,
                    false => 2,
                });
            }
            let n = estado.as_deref().unwrap_or("reposo");
            match e.de(n) {
                Some(l) => Ok(l.clone()),
                None => {
                    eprintln!("maqueta: no hay estado `{n}` en {entrada}");
                    std::process::exit(2);
                }
            }
        })
    } else {
        bmo_maqueta_compone::compilar(std::path::Path::new(&entrada))
    };
    let puesto = match compilado {
        Ok(l) => l,
        Err(f) => {
            eprint!("{}", f.render());
            eprintln!(
                "maqueta: {} reparo{} en {}, no se ha escrito nada.",
                f.errores.len(),
                if f.errores.len() == 1 { "" } else { "s" },
                f.fichero
            );
            return ExitCode::FAILURE;
        }
    };

    if let Some(por_la_cara) = foto {
        let im = if por_la_cara {
            let ordenes = bmo_maqueta_emit::orden::lista(&puesto);
            let golpes = bmo_maqueta_emit::orden::golpes(&puesto);
            let bytes = match bmo_maqueta_emit::bef::escribir(&ordenes, &golpes, puesto.canvas.0 as i64, puesto.canvas.1 as i64) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("maqueta: la cara no cabe en su formato: {e:?}");
                    return ExitCode::from(1);
                }
            };
            match bmo_maqueta_emit::foto::foto_cara(&bytes) {
                Some(im) => im,
                None => {
                    eprintln!("maqueta: la cara escrita no se deja leer");
                    return ExitCode::from(1);
                }
            }
        } else {
            bmo_maqueta_emit::foto::foto(&puesto)
        };
        return png(&salida, &im);
    }

    // ** Con sus ESTADOS (P3b): si el `.maqueta` declara `@estado`, el modulo
    // lleva sus transiciones; si no, sale EXACTAMENTE como siempre.
    let otros = match bmo_maqueta_compone::compilar_estados(std::path::Path::new(&entrada)) {
        Ok(e) => e.otros,
        Err(f) => {
            eprint!("{}", f.render());
            eprintln!("maqueta: un estado no compila en {}, no se ha escrito nada.", f.fichero);
            return ExitCode::FAILURE;
        }
    };
    // ** Con sus DATOS (H1): los colores `--dato-*` del fichero; los huecos
    // de texto los lleva la maquetacion.
    let colores = std::fs::read(&entrada).ok().and_then(|f| bmo_maqueta_node::parse(&f).ok()).map(|d| d.datos).unwrap_or_default();
    let con_datos = bmo_maqueta_emit::rust::tiene_datos(&puesto, &colores);
    if con_datos && !otros.is_empty() {
        eprintln!("maqueta: {entrada} tiene datos y estados a la vez; una pieza con datos no lleva estados todavia (P3c).");
        return ExitCode::FAILURE;
    }
    let codigo = bmo_maqueta_emit::rust::modulo_entero(&procedencia(&entrada), &puesto, &otros, &colores);
    if let Err(e) = std::fs::write(&salida, codigo) {
        eprintln!("maqueta: no puedo escribir {salida}: {e}");
        return ExitCode::from(2);
    }
    println!(
        "maqueta: {entrada} -> {salida}   {}x{}, {} cajas, {} golpes, {} islas",
        puesto.canvas.0,
        puesto.canvas.1,
        puesto.all().len(),
        puesto.hits().len(),
        puesto.islands().len()
    );
    ExitCode::SUCCESS
}

/// Escribe la foto como PNG, con el codificador de la casa (`bmo-imagen`).
fn png(salida: &str, im: &bmo_maqueta_emit::foto::Foto) -> ExitCode {
    use bmo_imagen::png_escribir as pe;
    let mut crudo = vec![0u8; pe::crudo(im.ancho, im.alto)];
    let mut taller = vec![0u32; bmo_imagen::deflar::TALLER];
    let mut dst = vec![0u8; pe::cota(im.ancho, im.alto)];
    let ancho = im.ancho;
    let mut pixel = |x: u32, y: u32| im.px[(y * ancho + x) as usize];
    match pe::codificar(im.ancho, im.alto, &mut pixel, &mut crudo, &mut taller, &mut dst) {
        Ok(n) => match std::fs::write(salida, &dst[..n]) {
            Ok(()) => {
                println!("maqueta: foto {}x{} -> {salida}", im.ancho, im.alto);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("maqueta: no puedo escribir {salida}: {e}");
                ExitCode::from(2)
            }
        },
        Err(e) => {
            eprintln!("maqueta: el PNG no sale: {}", e.motivo());
            ExitCode::from(1)
        }
    }
}

/// **De donde salio esta cara, dicho igual lo escriba quien lo escriba.**
///
/// === El defecto que esto cierra, y lo cazo un guardian el 2026-08-18 ===
///
/// El emisor recibia la ruta **tal como se tecleo**, y la escribe en la primera
/// linea del modulo generado. O sea que generar la MISMA cara desde el mismo
/// fichero daba DOS artefactos distintos:
///
/// ```text
///   maqueta pruebas/calc.maqueta ...        -> //! ... DESDE `pruebas/calc.maqueta`
///   maqueta C:/Users/.../calc.maqueta ...   -> //! ... DESDE `C:/Users/...`
/// ```
///
/// Con el fichero commiteado eso no es cosmetico: **un artefacto que depende de
/// quien lo genera no se puede comparar**, y comparar es lo unico que impide que
/// la cara pintada y su `.maqueta` se separen. El guardian de `build.ps1` lo
/// invocaba con ruta absoluta y veia una deriva que no existia.
///
/// La procedencia se deduce **del fichero**, no de la invocacion: se sube hasta
/// el `.git` y se cuenta desde ahi, con barras hacia adelante. Fuera de un
/// repositorio se contesta lo que se tecleo, normalizado -- decir algo cierto
/// vale mas que no decir nada.
fn procedencia(entrada: &str) -> String {
    let barras = |s: String| s.replace(std::path::MAIN_SEPARATOR, "/");
    let Ok(abs) = std::fs::canonicalize(entrada) else {
        return barras(entrada.to_string());
    };
    let mut raiz = abs.as_path();
    while let Some(padre) = raiz.parent() {
        if padre.join(".git").exists() {
            if let Ok(rel) = abs.strip_prefix(padre) {
                return barras(rel.to_string_lossy().into_owned());
            }
            break;
        }
        raiz = padre;
    }
    barras(entrada.to_string())
}

fn fallo(entrada: &str, src: &[u8], errores: &[bmo_maqueta_diag::Error]) -> ExitCode {
    eprint!("{}", bmo_maqueta_diag::render(entrada, src, errores));
    eprintln!(
        "maqueta: {} reparo{}, no se ha escrito nada.",
        errores.len(),
        if errores.len() == 1 { "" } else { "s" }
    );
    ExitCode::FAILURE
}

/// `--cobertura a.html b.html ...`: cada maqueta y la suma.
fn cobertura(ficheros: &[String]) -> ExitCode {
    if ficheros.is_empty() {
        eprintln!("uso: maqueta --cobertura <maqueta.html>...");
        return ExitCode::from(2);
    }
    let (mut total, mut bien) = (0, 0);
    let mut suma: std::collections::HashMap<String, (usize, String)> = std::collections::HashMap::new();
    for f in ficheros {
        let html = match std::fs::read_to_string(f) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("maqueta: no puedo leer {f}: {e}");
                return ExitCode::from(2);
            }
        };
        let inf = cobertura::medir(&html);
        println!(
            "{f}: {} de {} declaraciones = {}   ({} plantillas de JS, fuera)",
            inf.aceptadas,
            inf.total,
            cobertura::por_ciento(inf.aceptadas, inf.total),
            inf.plantillas
        );
        total += inf.total;
        bien += inf.aceptadas;
        for (p, n, m) in inf.rechazos {
            let e = suma.entry(p).or_insert((0, m));
            e.0 += n;
        }
    }
    println!("\nCOBERTURA: {bien} de {total} = {}\n\nlo que falta, lo mas usado primero:", cobertura::por_ciento(bien, total));
    let mut v: Vec<_> = suma.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    for (p, (n, m)) in v.iter().take(30) {
        println!("  {n:>4}  {p:<22} {m}");
    }
    ExitCode::SUCCESS
}

/// `--tira a b n`: `n` fotogramas de la transicion de `a` a `b`, de 0 al
/// final, uno debajo de otro con una raya entre medias.
fn tira_png(salida: &str, a: &bmo_maqueta_layout::Laid, b: &bmo_maqueta_layout::Laid, n: u32) -> bool {
    // ** PAR A PAR (P3b): la misma mezcla de piezas que hace el escritorio,
    // asi que la tira es el oraculo del codigo generado.
    use bmo_maqueta_emit::{foto, movimiento};
    let pares = movimiento::pares(a, b);
    let total = movimiento::duracion(&pares).max(1);
    let lienzo = (a.canvas.0.max(b.canvas.0), a.canvas.1.max(b.canvas.1));
    let fotos: Vec<_> = (0..n).map(|k| foto::foto_en(&pares, lienzo, total * k / (n - 1))).collect();
    let ancho = fotos.iter().map(|f| f.ancho).max().unwrap_or(1);
    let alto: u32 = fotos.iter().map(|f| f.alto + 4).sum();
    let mut px = vec![0x0030_3040u32; (ancho * alto) as usize];
    let mut y0 = 0;
    for f in &fotos {
        for y in 0..f.alto {
            for x in 0..f.ancho {
                px[((y0 + y) * ancho + x) as usize] = f.px[(y * f.ancho + x) as usize];
            }
        }
        y0 += f.alto + 4;
    }
    let im = bmo_maqueta_emit::foto::Foto { ancho, alto, px };
    println!("maqueta: transicion de {total} ms en {n} fotogramas");
    png(salida, &im) == ExitCode::SUCCESS
}
