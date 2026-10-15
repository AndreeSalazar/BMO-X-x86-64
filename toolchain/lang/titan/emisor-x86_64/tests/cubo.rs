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

/// ** V4 DE `docs/plan/PLAN_VERRANO.md` (D1 de EL_FOCO, 15-10): el `cubo.bsf`
/// que VERRANO toma (`gspcubo/sm86.rs`, `include_bytes!`) ES este sobre -- el
/// de las dos gpu fn de TITAN++, emitidas por PROMETEO desde el Programa de la
/// casa --, y ya no el del SASS a mano. Para volver a fabricarlo tras cambiar
/// el cubo, el emisor o el pegamento: `BSF_FIJAR=1 cargo test -p
/// bmo-titan-x86-64 --test cubo`.
#[test]
fn verrano_s_cubo_bsf_is_the_one_titan_makes() {
    let m = cubo();
    let ks = bmo_titan_prometeo::kernels(&m, &[&RTX_3060_12G]).unwrap_or_else(|e| panic!("{}", e));
    let (_, sobre) = bmo_titan_x86_64::sobre(&ks).unwrap_or_else(|e| panic!("{}", e)).expect("el cubo dibuja");
    let fichero = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../platform/drivers/gpu/ga10x/sombreadores/cubo.bsf");
    if std::env::var_os("BSF_FIJAR").is_some() {
        std::fs::write(&fichero, &sobre).unwrap();
    }
    let guardado = std::fs::read(&fichero).expect("sombreadores/cubo.bsf (BSF_FIJAR=1 lo fabrica)");
    assert_eq!(guardado, sobre, "cubo.bsf no es el que sale del cubo de TITAN++ de hoy");
}

/// El cubo de V3 (`ejemplos/nivel11/cubo3`): la de vertice recibe sus
/// CONSTANTES -- la matriz por columnas -- en su segundo valor.
fn cubo3() -> bmo_titan_front::ir::Module {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("ejemplos").join("nivel11").join("cubo3");
    let src = std::fs::read_to_string(pkg.join("src").join("main.titan")).expect("el ejemplo del banco");
    bmo_titan_front::lower_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

/// ** V3 DE `docs/plan/PLAN_VERRANO.md` (D1 de EL_FOCO, 15-10): LA MATRIZ EN
/// EL CBUFFER Y LA TARJETA MULTIPLICANDO. Los vertices del cubo SIN
/// transformar y la `wvp` de `bmo_cubo` como constantes dan, en cada tarjeta,
/// en la casa y en el calculo, la posicion de recorte de la tanda BIT A BIT
/// -- la misma que da la huella de D3D12 --: las cuentas ya no las hace la
/// CPU de la app.
#[test]
fn the_v3_cube_multiplies_on_the_card_and_gives_the_bits_of_the_tanda() {
    let m = cubo3();
    let ks = bmo_titan_prometeo::kernels(&m, &bmo_titan_x86_64::TARJETAS).unwrap_or_else(|e| panic!("{}", e));
    let fv = m.functions.iter().position(|f| f.name == "cubo_vertice").unwrap();
    let (vs, is) = (bmo_cubo::vertices(), bmo_cubo::indices());
    for t in bmo_titan_x86_64::TARJETAS {
        let kv = ks.iter().find(|k| k.name == "cubo_vertice" && k.tarjeta.ficha() == t.ficha()).unwrap();
        let dv = kv.dibujo.clone().unwrap();
        assert_eq!((dv.entradas.clone(), dv.constantes.clone()), (vec![4, 4], vec![4, 4, 4, 4]), "dos elementos y cuatro filas de cbuffer");
        assert_eq!(kv.programa.filas_cb, 4);
        let mut vistos = 0;
        for f in FOTOGRAMAS {
            let tanda = tanda::de_fotograma(f, 1280, 720).expect("la tanda");
            let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
            let columnas: Vec<[u32; 4]> = (0..4).map(|k| [c.wvp[4 * k], c.wvp[4 * k + 1], c.wvp[4 * k + 2], c.wvp[4 * k + 3]].map(f32::to_bits)).collect();
            let mut celdas = Vec::new();
            for (i, tri) in tanda.tris().iter().enumerate() {
                for j in 0..3 {
                    let v = vs[is[3 * tanda.caras[i] + j] as usize];
                    let mut celda = vec![[v.pos[0], v.pos[1], v.pos[2], 1.0].map(f32::to_bits), tri.color.map(f32::to_bits)];
                    celda.extend_from_slice(&columnas);
                    celdas.push(celda);
                }
            }
            let sale = dibuja::run(kv, &dv, &celdas).unwrap_or_else(|e| panic!("{}", e));
            assert_eq!(sale, dibuja::run_casa(kv, &dv, &celdas), "{}, fotograma {}: la casa", t.ficha().nombre, f);
            assert_eq!(sale, bmo_titan_front::calc::run_gpu_dibujo(&m, fv, &celdas).unwrap(), "{}, fotograma {}: el calculo", t.ficha().nombre, f);
            for (i, tri) in tanda.tris().iter().enumerate() {
                for j in 0..3 {
                    assert_eq!(sale[3 * i + j][0], tri.clip[j].map(f32::to_bits), "{}, fotograma {}, triangulo {}, vertice {}: la posicion de recorte", t.ficha().nombre, f, i, j);
                    assert_eq!(sale[3 * i + j][1], tri.color.map(f32::to_bits), "y el color, tal cual");
                }
            }
            vistos += celdas.len();
        }
        assert_eq!(vistos, 54, "{}: los vertices de cuatro fotogramas", t.ficha().nombre);
    }
}

/// Su sobre dice el cbuffer: el bufer de la de vertice se lee desde el byte
/// `16 * 4` (las cuatro filas van DELANTE de los vertices en los DATOS).
#[test]
fn the_v3_cube_s_sobre_says_its_cbuffer() {
    let m = cubo3();
    let ks = bmo_titan_prometeo::kernels(&m, &[&RTX_3060_12G]).unwrap_or_else(|e| panic!("{}", e));
    let (pegadas, a) = bmo_titan_x86_64::sobre(&ks).unwrap_or_else(|e| panic!("{}", e)).expect("el cubo dibuja");
    assert!(pegadas.iter().all(|g| g.veredicto.contains("PERFECTO Y PRECISO")));
    let s = Bsf::parse(&a).expect("el sobre se lee");
    s.verify_all().unwrap();
    let b0 = s.find(b"cubo_vertice").unwrap().binding(0);
    assert_eq!((b0.base_bytes, b0.stride), (64, 32));
}
