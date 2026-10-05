//! EL BANCO DEL ESCRITOR (nivel 11, G2): cada `gpu fn` de cada programa BIEN
//! del banco sale como SPIR-V que el juez de spirv acepta, y el oraculo de
//! spirv da, celda a celda, los mismos bits que el f32 de Rust con el que hoy
//! cuenta el calculo. Si un ejemplo nuevo trae una gpu fn, entra aqui solo.

use std::path::Path;

#[test]
fn every_gpu_fn_of_the_bench_is_valid_spirv() {
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
        let ks = bmo_titan_spirv::kernels(&m).unwrap_or_else(|e| panic!("{}: {}", pkg.display(), e));
        written += ks.len();
    }
    assert!(written >= 3, "the bench has its gpu fn ({} written)", written);
}
