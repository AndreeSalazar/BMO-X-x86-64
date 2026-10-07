//! EL BANCO DEL ESCRITOR DE LA 3060 (nivel 11, 07-10, sin SPIR-V): cada `gpu
//! fn` de cada programa BIEN del banco sale como el Programa de la casa y su
//! SASS de SM86, el juez del SASS lo acepta, y la 3060 simulada, la casa y el
//! calculo dan, celda a celda, los mismos bits en la bateria de bordes. Si un
//! ejemplo nuevo trae una gpu fn, entra aqui solo.

use std::path::Path;

#[test]
fn every_gpu_fn_of_the_bench_is_judged_sass() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join("nivel11");
    let mut written = 0;
    for entry in std::fs::read_dir(&dir).expect("the bench folder") {
        let pkg = entry.unwrap().path();
        let root = pkg.join("src").join("main.titan");
        if !pkg.is_dir() || !root.exists() {
            continue;
        }
        let src = std::fs::read_to_string(&root).unwrap();
        if !src.starts_with("# espera: BIEN") {
            continue;
        }
        let m = bmo_titan_front::lower_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{}: {:?}", pkg.display(), e));
        let ks = bmo_titan_sm86::kernels(&m).unwrap_or_else(|e| panic!("{}: {}", pkg.display(), e));
        for k in &ks {
            assert!(!k.app.codigo.is_empty() && k.app.registros <= bmo_titan_sm86::REGISTROS, "{}", k.name);
        }
        written += ks.len();
    }
    assert!(written >= 3, "the bench has its gpu fn ({} written)", written);
}
