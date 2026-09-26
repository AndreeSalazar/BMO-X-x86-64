//! **VERRANO V0 en su sobre**: los dos programas de la tuberia fija
//! (`tuberia::vertice` y `tuberia::pixel`) viajan en un BSF con `kind` SM86,
//! junto al SPIR-V del que son la traduccion (`sombreadores/cubo.vert` y
//! `cubo.frag`). Lo que la 3060 ejecuta sale del BSF comprobado, byte a byte
//! lo mismo que escribe el kernel. `sombreadores/cubo.bsf` es el sobre
//! fabricado; este banco exige que se fabrique IGUAL (determinista).
//!
//! Para volver a fabricarlo tras cambiar un programa: `BSF_FIJAR=1 cargo test
//! -p bmo-gpu-ga10x --test bsf_sm86`.

use bmo_bsf::{abi, kind, write, Binding, Bsf, ModuleIn, TargetIn, READS};
use bmo_gpu_ga10x::sass::juez;
use bmo_gpu_ga10x::tuberia;

const VS_SPV: &[u8] = include_bytes!("../sombreadores/cubo.vert.spv");
const PS_SPV: &[u8] = include_bytes!("../sombreadores/cubo.frag.spv");
const FICHERO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/sombreadores/cubo.bsf");

/// Las capacidades que declara un SPIR-V (`OpCapability`, opcode 17).
fn capacidades(spv: &[u8]) -> u64 {
    let w: Vec<u32> = spv.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let (mut i, mut caps) = (5, 0u64);
    while i < w.len() {
        let (n, op) = ((w[i] >> 16) as usize, w[i] & 0xFFFF);
        if op == 17 && w[i + 1] < 64 {
            caps |= 1 << w[i + 1];
        }
        i += n.max(1);
    }
    caps
}

fn bytes<const N: usize>(p: &[u32; N]) -> Vec<u8> {
    let mut b = vec![0u8; 4 * N];
    tuberia::bytes(p, &mut b);
    b
}

/// ** EL JUEZ DEL SASS, antes del sobre (J2 de PLAN_LA_LENGUA_DE_LA_3060):
/// si dice `TOMA TU BODRIO`, no hay BSF. Si dice `PERFECTO Y PRECISO`, se
/// fabrica -- y se dice.
fn juzgar_antes() {
    let (sv, sp) = (tuberia::sph_vertice(), tuberia::sph_pixel());
    let registros = bmo_gpu_ga10x::raster::REGISTROS;
    for (nombre, codigo, sph) in [("cubo_vertice", &tuberia::codigo_vs()[..], &sv), ("cubo_pixel", &tuberia::codigo_ps()[..], &sp)] {
        match juez::juzgar(codigo, &juez::Contexto { registros, sph: Some(sph) }) {
            Ok(v) => eprintln!("cubo.bsf {nombre}: {v}"),
            Err(b) => panic!("cubo.bsf {nombre}: {b}\n{} -- no se fabrica el sobre", juez::REMATE),
        }
    }
}

fn fabricar() -> Vec<u8> {
    juzgar_antes();
    let (vs, ps) = (bytes(&tuberia::vertice()), bytes(&tuberia::pixel()));
    let buffers = [Binding { set: 0, binding: 0, storage: true, access: READS, base_bytes: 0, stride: tuberia::BYTES_VERTICE as u32 }];
    let objetivo = |code: &'static [u8], slots: &'static [u8]| TargetIn {
        kind: kind::SM86,
        abi: abi::SM86_V1,
        requires: 0,
        code,
        init: 0,
        main: 128,
        frame_words: 0,
        slots,
        emitter: b"a-mano",
    };
    let vs: &'static [u8] = Box::leak(vs.into_boxed_slice());
    let ps: &'static [u8] = Box::leak(ps.into_boxed_slice());
    let (tv, tp) = ([objetivo(vs, &[0])], [objetivo(ps, &[])]);
    let modulos = [
        ModuleIn { model: 0, name: b"cubo_vertice", local_size: [0; 3], capabilities: capacidades(VS_SPV), caps_high: 0, spirv: VS_SPV, bindings: &buffers, targets: &tv },
        ModuleIn { model: 4, name: b"cubo_pixel", local_size: [0; 3], capabilities: capacidades(PS_SPV), caps_high: 0, spirv: PS_SPV, bindings: &[], targets: &tp },
    ];
    let mut out = vec![0u8; 1 << 16];
    let n = write(&modulos, &mut out).expect("el BSF se escribe y se relee");
    out.truncate(n);
    out
}

/// *** Lo que la 3060 ejecuta SALE DEL BSF: los programas tomados del sobre
/// (con sus hashes comprobados) son los que escribe el kernel.
#[test]
fn la_tuberia_viaja_en_su_bsf() {
    let b = fabricar();
    let bsf = Bsf::parse(&b).expect("capas 1 a 3");
    bsf.verify_all().expect("capa 4: cada hash");
    let (vs, ps) = (bsf.module(0), bsf.module(1));
    assert_eq!((vs.name(), vs.model(), ps.name(), ps.model()), (&b"cubo_vertice"[..], 0, &b"cubo_pixel"[..], 4));
    let tv = vs.target(kind::SM86, abi::SM86_V1, 0).expect("hay codigo para la 3060");
    let tp = ps.target(kind::SM86, abi::SM86_V1, 0).expect("hay codigo para la 3060");
    assert_eq!(tv.code().unwrap(), &bytes(&tuberia::vertice())[..]);
    assert_eq!(tp.code().unwrap(), &bytes(&tuberia::pixel())[..]);
    assert_eq!(tv.slots(), &[0], "la ranura 0: los vertices");
    // Para una CPU no hay codigo: el backend CPU de VERRANO dibuja con el juez.
    assert!(vs.target(kind::X86_64_SCALAR, abi::X86_64_V1, u32::MAX).is_none());
}

/// Determinista, y el fichero del repositorio es el que sale.
#[test]
fn el_sobre_del_repositorio_es_el_que_sale() {
    let b = fabricar();
    assert_eq!(b, fabricar());
    if std::env::var_os("BSF_FIJAR").is_some() {
        std::fs::write(FICHERO, &b).unwrap();
    }
    let guardado = std::fs::read(FICHERO).expect("sombreadores/cubo.bsf (BSF_FIJAR=1 lo fabrica)");
    assert_eq!(guardado, b, "cubo.bsf no es el que sale de los programas de hoy");
}

/// Cambiar UN bit del codigo de la 3060 no pasa: el hash lo caza al tomarlo.
#[test]
fn un_bit_del_codigo_no_pasa() {
    let b = fabricar();
    let bsf = Bsf::parse(&b).unwrap();
    let t = bsf.module(0).target(kind::SM86, abi::SM86_V1, 0).unwrap();
    let donde = t.code().unwrap().as_ptr() as usize - b.as_ptr() as usize + 128 + 16 * 4;
    let mut malo = b.clone();
    malo[donde] ^= 1;
    let m = Bsf::parse(&malo).unwrap();
    assert!(m.module(0).target(kind::SM86, abi::SM86_V1, 0).unwrap().code().is_err());
}
