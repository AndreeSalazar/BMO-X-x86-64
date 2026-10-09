//! ** LB6 DE `docs/plan/PLAN_LAS_LIBRERIAS.md`, EN EL ANFITRION: el cubo de V0
//! escrito en TITAN++ (`ejemplos/nivel11/cubo`), sus dos gpu fn que DIBUJAN.
//!
//! - Dan los bits de la tanda de `bmo_cubo` en los fotogramas 0, 30, 60 y 123
//!   -- cada vertice, su posicion de recorte bit a bit; cada cara, su color en
//!   8 bits -- en CADA tarjeta que da `titan` (la RTX 3060 12G simulada y la
//!   CPU), en la casa y en el calculo.
//! - Pegadas a la tuberia de VERRANO, su juez dice PERFECTO; y su sobre sale
//!   IGUAL dos veces, con lo que VERRANO le pide a `cubo.bsf`: un modulo
//!   `cubo_vertice` que lee el `Vertex` de VERRANO (32 bytes: la posicion y el
//!   color), y un `cubo_pixel`, en SM86 con el ABI `SM86_V1`.
//!
//! El metal es del propietario: `gpu verrano` con este sobre donde hoy va
//! `cubo.bsf` (SASS a mano), en el 0, el 30 y el 60.

use bmo_bsf::{abi, kind, Bsf, READS};
use bmo_cubo::tanda;
use bmo_tarjeta_rtx3060_12g::RTX_3060_12G;
use bmo_titan_prometeo::{dibuja, Forma, Kernel};
use std::path::Path;

const FOTOGRAMAS: [u32; 4] = [0, 30, 60, 123];

fn cubo() -> bmo_titan_front::ir::Module {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join("nivel11").join("cubo");
    let src = std::fs::read_to_string(pkg.join("src").join("main.titan")).expect("el ejemplo del banco");
    bmo_titan_front::lower_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

/// ** LOS BITS DE LA TANDA, en cada tarjeta, en la casa y en el calculo.
#[test]
fn the_cube_written_in_titan_gives_the_bits_of_the_tanda_on_every_card() {
    let m = cubo();
    let ks = bmo_titan_prometeo::kernels(&m, &bmo_titan_x86_64::TARJETAS).unwrap_or_else(|e| panic!("{}", e));
    let func = |n: &str| m.functions.iter().position(|f| f.name == n).unwrap();
    let (fv, fp) = (func("cubo_vertice"), func("cubo_pixel"));
    for t in bmo_titan_x86_64::TARJETAS {
        let suya = |n: &str| -> &Kernel { ks.iter().find(|k| k.name == n && k.tarjeta.ficha() == t.ficha()).unwrap() };
        let (kv, kp) = (suya("cubo_vertice"), suya("cubo_pixel"));
        let (dv, dp) = (kv.dibujo.clone().unwrap(), kp.dibujo.clone().unwrap());
        assert_eq!((dv.forma, dp.forma), (Forma::Vertice { posicion: 0 }, Forma::Pixel { posicion: 0 }));
        let mut vistos = 0;
        for f in FOTOGRAMAS {
            let tanda = tanda::de_fotograma(f, 1280, 720).expect("la tanda de V0");
            let tris = &tanda.tris[..tanda.n];
            // Cada vertice de cada triangulo que se ve, como lo escribe la
            // app: su posicion de recorte y el color de su cara.
            let celdas: Vec<Vec<[u32; 4]>> = tris.iter().flat_map(|tri| (0..3).map(move |c| vec![tri.clip[c].map(f32::to_bits), tri.color.map(f32::to_bits)])).collect();
            let v = dibuja::run(kv, &dv, &celdas).unwrap_or_else(|e| panic!("{}", e));
            assert_eq!(v, dibuja::run_casa(kv, &dv, &celdas), "{}, fotograma {}: la casa", t.ficha().nombre, f);
            assert_eq!(v, bmo_titan_front::calc::run_gpu_dibujo(&m, fv, &celdas).unwrap(), "{}, fotograma {}: el calculo", t.ficha().nombre, f);
            // La de pixel, con lo que dejo la de vertice (la cara es de UN
            // color: lo que la tarjeta interpola es eso mismo).
            let p = dibuja::run(kp, &dp, &v).unwrap_or_else(|e| panic!("{}", e));
            assert_eq!(p, dibuja::run_casa(kp, &dp, &v), "{}, fotograma {}: la casa", t.ficha().nombre, f);
            assert_eq!(p, bmo_titan_front::calc::run_gpu_dibujo(&m, fp, &v).unwrap(), "{}, fotograma {}: el calculo", t.ficha().nombre, f);
            for (i, tri) in tris.iter().enumerate() {
                for c in 0..3 {
                    let k = 3 * i + c;
                    assert_eq!(v[k][0], tri.clip[c].map(f32::to_bits), "{}, fotograma {}, triangulo {}, vertice {}: la posicion", t.ficha().nombre, f, i, c);
                    assert_eq!(bmo_cubo::empaquetar(p[k][0].map(f32::from_bits)), bmo_cubo::empaquetar(tri.color), "{}, fotograma {}, triangulo {}, vertice {}: el color", t.ficha().nombre, f, i, c);
                }
            }
            vistos += celdas.len();
        }
        // Las caras que mira la camara en los cuatro: 18 triangulos.
        assert_eq!(vistos, 54, "{}: los vertices de cuatro fotogramas", t.ficha().nombre);
    }
}

/// ** EL SOBRE: igual dos veces, juzgado, y con el contrato de `cubo.bsf`.
#[test]
fn the_cube_s_sobre_is_the_same_twice_and_keeps_the_contract_of_cubo_bsf() {
    let sobre = || {
        let m = cubo();
        let ks = bmo_titan_prometeo::kernels(&m, &[&RTX_3060_12G]).unwrap_or_else(|e| panic!("{}", e));
        bmo_titan_x86_64::sobre(&ks).unwrap_or_else(|e| panic!("{}", e)).expect("el cubo dibuja")
    };
    let (pegadas, a) = sobre();
    let (_, b) = sobre();
    assert_eq!(a, b, "el sobre es determinista");
    for g in &pegadas {
        assert!(g.veredicto.contains("PERFECTO Y PRECISO"), "{}: {}", g.nombre, g.veredicto);
    }
    let s = Bsf::parse(&a).expect("el sobre se lee");
    s.verify_all().expect("cada hash del sobre cuadra");
    let modulos: Vec<(&[u8], u8)> = s.modules().map(|m| (m.name(), m.model())).collect();
    assert_eq!(modulos, vec![(&b"cubo_vertice"[..], 0), (&b"cubo_pixel"[..], 4)]);
    // Su fuente: el MAPA de la casa (A9), no SPIR-V.
    assert!(s.modules().all(|m| m.es_mapa()));
    // ** EL CONTRATO que VERRANO exige al abrir (`contrato` del escritorio):
    // un bufer, set 0 binding 0, de almacenamiento, que se lee desde su byte
    // 0, de un `Vertex` de VERRANO por vertice -- la posicion en el 0 y el
    // color en el 16: los elementos 0 y 1 de lo que recibe `cubo_vertice`.
    let vs = s.find(b"cubo_vertice").unwrap();
    assert_eq!(vs.binding_count(), 1);
    let b0 = vs.binding(0);
    assert_eq!((b0.set, b0.binding, b0.storage, b0.access, b0.base_bytes, b0.stride), (0, 0, true, READS, 0, bmo_verrano::VERTEX_BYTES as u32));
    assert_eq!((bmo_verrano::VERTEX_POSITION, bmo_verrano::VERTEX_COLOR), (0, 16));
    assert_eq!(s.find(b"cubo_pixel").unwrap().binding_count(), 0);
    // El codigo, en SM86 con el ABI SM86_V1: lo que se pego, tal cual viaja.
    for (nombre, g) in [(&b"cubo_vertice"[..], &pegadas[0]), (&b"cubo_pixel"[..], &pegadas[1])] {
        let t = s.find(nombre).unwrap().target(kind::SM86, abi::SM86_V1, 0).expect("SM86_V1");
        assert_eq!(t.code().unwrap(), &g.bytes[..]);
        assert_eq!((t.main(), t.emitter()), (128, &b"titan-prometeo"[..]));
    }
}
