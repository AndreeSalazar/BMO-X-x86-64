//! `cargo run -p bmo-maqueta-dibujo --example leer -- dibujo.svg`: lo que el
//! lector hace de un SVG -- sus figuras, o la lista entera de lo que no.
fn main() {
    let ruta = std::env::args().nth(1).expect("uso: leer <dibujo.svg>");
    let texto = std::fs::read(&ruta).expect("no se puede leer");
    match bmo_maqueta_dibujo::leer_fichero(&ruta, &texto, bmo_maqueta_diag::Span::new(0, 1, 1, 1)) {
        Err(e) => {
            for x in &e {
                println!("NO  {}", x.title);
            }
            println!("{} reparos", e.len());
        }
        Ok(s) => {
            let v = s.vista_o_medida().unwrap_or([0.0, 0.0, 100.0, 100.0]);
            let (f, e) = bmo_maqueta_dibujo::figuras(&s, &Default::default(), (0.0, 0.0, v[2], v[3]));
            let puntos: usize = f.iter().map(|f| f.caminos.iter().map(Vec::len).sum::<usize>()).sum();
            println!("{} figuras, {puntos} puntos, {} animaciones, {} reparos al pintar", f.len(), s.animaciones.len(), e.len());
            for x in &e {
                println!("NO  {}", x.title);
            }
        }
    }
}
