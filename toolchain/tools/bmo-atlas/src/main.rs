//! `bmo-atlas [FUENTE.ttf] -o SALIDA.blet [--medida N] [--negrita] [--muestra M.pgm "texto"]`
//!
//! El atlas de Latin-1 de la letra de la casa (o de un TrueType, si se da) a
//! N pixeles (16 si no se dice), con el borde suave. `--negrita`, la de la
//! casa en negrita. `--muestra` escribe ademas una linea de texto en gris
//! (PGM) para verla. Ver la cabecera de la libreria.

use std::process::ExitCode;

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mut entrada, mut salida, mut medida, mut muestra, mut negrita) = (None, None, 16u8, None, false);
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "-o" => {
                salida = a.get(i + 1).cloned();
                i += 1;
            }
            "--medida" => {
                medida = match a.get(i + 1).and_then(|n| n.parse().ok()) {
                    Some(n) => n,
                    None => return mal("`--medida` pide un numero de 6 a 128"),
                };
                i += 1;
            }
            "--negrita" => negrita = true,
            "--muestra" => {
                match (a.get(i + 1), a.get(i + 2)) {
                    (Some(f), Some(t)) => muestra = Some((f.clone(), t.clone())),
                    _ => return mal("`--muestra` pide un fichero .pgm y un texto"),
                }
                i += 2;
            }
            x if entrada.is_none() => entrada = Some(x.to_string()),
            x => return mal(&format!("no conozco `{x}`")),
        }
        i += 1;
    }
    let Some(salida) = salida else {
        return mal("uso: bmo-atlas [FUENTE.ttf] -o SALIDA.blet [--medida N] [--negrita] [--muestra M.pgm \"texto\"]");
    };
    let hecho = match &entrada {
        None => bmo_atlas::atlas_de_casa(medida, if negrita { bmo_letra::Peso::Negrita } else { bmo_letra::Peso::Normal }),
        Some(f) => match std::fs::read(f) {
            Ok(b) => bmo_atlas::atlas(&b, medida).map_err(|e| format!("{f}: {e}")),
            Err(e) => return mal(&format!("no se lee {f}: {e}")),
        },
    };
    let atlas = match hecho {
        Ok(a) => a,
        Err(e) => return mal(&e),
    };
    if let Err(e) = std::fs::write(&salida, atlas.bytes()) {
        return mal(&format!("no se escribe {salida}: {e}"));
    }
    if let Some((f, t)) = muestra {
        // Latin-1: cada caracter de 0 a 255 es su byte.
        let texto: Vec<u8> = t.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
        let (w, h, px) = atlas.muestra(&texto);
        let mut v = format!("P5\n{w} {h}\n255\n").into_bytes();
        v.extend_from_slice(&px);
        if let Err(e) = std::fs::write(&f, v) {
            return mal(&format!("no se escribe {f}: {e}"));
        }
    }
    ExitCode::SUCCESS
}

fn mal(m: &str) -> ExitCode {
    eprintln!("bmo-atlas: {m}");
    ExitCode::FAILURE
}
