//! `cargo run --release -p bmo-maqueta-dibujo --example censo -- carpeta/`:
//! cuantos SVG de una carpeta se leen y pintan, y POR QUE no los demas
//! (cada motivo con cuantos ficheros lo tienen).
use std::collections::BTreeMap;

fn svgs(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            svgs(&p, out);
        } else if p.extension().is_some_and(|x| x == "svg") {
            out.push(p);
        }
    }
}

fn main() {
    let raiz = std::env::args().nth(1).expect("uso: censo <carpeta>");
    let mut todos = Vec::new();
    svgs(std::path::Path::new(&raiz), &mut todos);
    let (mut bien, mut animados) = (0usize, 0usize);
    let mut motivos: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for p in &todos {
        let texto = std::fs::read(p).unwrap();
        let nombre = p.display().to_string();
        let mut vistos = std::collections::BTreeSet::new();
        let mut apunta = |t: &str| {
            let corto = match t.find(": ") {
                Some(k) if t[..k].contains(".svg:") => &t[k + 2..],
                _ => t,
            };
            let clave: String = corto.chars().take(90).collect();
            if vistos.insert(clave.clone()) {
                let e = motivos.entry(clave).or_insert((0, nombre.clone()));
                e.0 += 1;
            }
        };
        match bmo_maqueta_dibujo::leer_fichero(&nombre, &texto, bmo_maqueta_diag::Span::new(0, 1, 1, 1)) {
            Err(e) => e.iter().for_each(|x| apunta(&x.title)),
            Ok(s) => {
                let v = s.vista_o_medida().unwrap_or([0.0, 0.0, 24.0, 24.0]);
                let (_, e) = bmo_maqueta_dibujo::figuras(&s, &Default::default(), (0.0, 0.0, 96.0, 96.0 * v[3] / v[2]));
                let anima = bmo_maqueta_dibujo::pasos(&s, &Default::default(), (0.0, 0.0, 96.0, 96.0));
                let mut malos: Vec<String> = e.iter().map(|x| x.title.clone()).collect();
                if let Some(r) = &anima {
                    match r {
                        Ok(_) => animados += 1,
                        Err(e) => malos.extend(e.iter().map(|x| x.title.clone())),
                    }
                }
                if malos.is_empty() {
                    bien += 1;
                } else {
                    malos.iter().for_each(|t| apunta(t));
                }
            }
        }
    }
    println!("{bien} de {} se pintan ({:.1} %), {animados} animados", todos.len(), bien as f64 * 100.0 / todos.len().max(1) as f64);
    let mut v: Vec<_> = motivos.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (m, (n, ej)) in v.iter().take(40) {
        println!("{n:6}  {m}    [{ej}]");
    }
}
