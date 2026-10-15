//! **VERRANO en su sobre**: los dos programas que dibujan el cubo viajan en
//! un BSF con `kind` SM86. Lo que la 3060 ejecuta sale del BSF comprobado.
//!
//! ** V4 (D1 de EL_FOCO, 15-10): `sombreadores/cubo.bsf` ya NO es SASS a
//! mano. Lo fabrica el cubo de TITAN++ (`ejemplos/nivel11/cubo`) por el
//! emisor de la casa -- PROMETEO, desde el Programa de BMO-X -- y el
//! pegamento de VERRANO; la prueba que lo fabrica y lo ata es
//! `toolchain/lang/titan/emisor-x86_64/tests/cubo.rs`
//! (`BSF_FIJAR=1 cargo test -p bmo-titan-x86-64 --test cubo`). Aqui se mira el
//! fichero como lo tomara la puerta: sus hashes, su contrato, y el juez con
//! los registros de la tuberia -- lo mismo que hace el kernel al prepararlo.
//!
//! Los programas a mano (`tuberia::vertice`/`pixel`) se quedan: los usan la
//! prueba de la 3060 sin LDG y el corpus del juez. Se siguen juzgando.

use bmo_bsf::{abi, kind, Bsf, READS};
use bmo_gpu_ga10x::sass::juez;
use bmo_gpu_ga10x::tuberia;

const FICHERO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/sombreadores/cubo.bsf");

fn bytes<const N: usize>(p: &[u32; N]) -> Vec<u8> {
    let mut b = vec![0u8; 4 * N];
    tuberia::bytes(p, &mut b);
    b
}

/// ** EL JUEZ DEL SASS sobre los programas a mano (J2 de
/// PLAN_LA_LENGUA_DE_LA_3060): siguen en el arbol, y siguen PERFECTOS.
#[test]
fn los_programas_a_mano_siguen_perfectos() {
    let (sv, sp) = (tuberia::sph_vertice(), tuberia::sph_pixel());
    let registros = tuberia::REGISTROS;
    for (nombre, codigo, sph) in [("cubo_vertice", &tuberia::codigo_vs()[..], &sv), ("cubo_pixel", &tuberia::codigo_ps()[..], &sp)] {
        match juez::juzgar(codigo, &juez::Contexto { registros, sph: Some(sph) }) {
            Ok(v) => eprintln!("a mano, {nombre}: {v}"),
            Err(b) => panic!("a mano, {nombre}: {b}\n{}", juez::REMATE),
        }
    }
}

fn el_sobre() -> Vec<u8> {
    std::fs::read(FICHERO).expect("sombreadores/cubo.bsf (lo fabrica tests/cubo.rs de TITAN++)")
}

/// *** Lo que la 3060 ejecuta SALE DEL BSF: los dos modulos, su contrato, el
/// emisor de la casa, y el juez con los registros de la tuberia.
#[test]
fn la_tuberia_viaja_en_su_bsf() {
    let b = el_sobre();
    let bsf = Bsf::parse(&b).expect("capas 1 a 3");
    bsf.verify_all().expect("capa 4: cada hash");
    let (vs, ps) = (bsf.module(0), bsf.module(1));
    assert_eq!((vs.name(), vs.model(), ps.name(), ps.model()), (&b"cubo_vertice"[..], 0, &b"cubo_pixel"[..], 4));
    // ** El contrato (26-09): el programa de vertice lee `Vertex` de VERRANO
    // -- elementos de 32 bytes desde el 0 del buffer 0 --, que es lo que la
    // app escribe directo. La puerta (`gspcubo/sm86.rs::contrato`) lo exige al abrir.
    let b = vs.binding(0);
    assert_eq!((vs.binding_count(), b.set, b.binding, b.storage, b.access, b.base_bytes, b.stride), (1, 0, 0, true, READS, 0, bmo_verrano::VERTEX_BYTES as u32));
    // ** V4: su fuente es el MAPA de la casa, no SPIR-V, y lo escribio el
    // emisor de la casa, no una mano.
    assert!(bsf.modules().all(|m| m.es_mapa()), "la fuente, el MAPA de la casa");
    for m in [vs, ps] {
        let t = m.target(kind::SM86, abi::SM86_V1, 0).expect("hay codigo para la 3060");
        assert_eq!(t.emitter(), &b"titan-prometeo"[..], "{:?}", core::str::from_utf8(m.name()));
        let codigo = t.code().unwrap();
        assert_ne!(codigo, &bytes(&tuberia::vertice())[..], "ya no es el SASS a mano");
        juez::juzgar_programa(codigo, tuberia::REGISTROS).unwrap_or_else(|e| panic!("{}: {}\n{}", core::str::from_utf8(m.name()).unwrap(), e, juez::REMATE));
    }
    assert_eq!(vs.target(kind::SM86, abi::SM86_V1, 0).unwrap().slots(), &[0], "la ranura 0: los vertices");
    // Para una CPU no hay codigo: el backend CPU de VERRANO dibuja con el juez.
    assert!(vs.target(kind::X86_64_SCALAR, abi::X86_64_V1, u32::MAX).is_none());
}

/// Cambiar UN bit del codigo de la 3060 no pasa: el hash lo caza al tomarlo.
#[test]
fn un_bit_del_codigo_no_pasa() {
    let b = el_sobre();
    let bsf = Bsf::parse(&b).unwrap();
    let t = bsf.module(0).target(kind::SM86, abi::SM86_V1, 0).unwrap();
    let donde = t.code().unwrap().as_ptr() as usize - b.as_ptr() as usize + 128 + 16 * 4;
    let mut malo = b.clone();
    malo[donde] ^= 1;
    let m = Bsf::parse(&malo).unwrap();
    assert!(m.module(0).target(kind::SM86, abi::SM86_V1, 0).unwrap().code().is_err());
}
