//! **El banco del BSF.** Que un fichero sano pasa las cinco capas, que NINGUN
//! byte se puede cambiar sin que se note, y que cada mentira de un fabricante
//! que si sabe rehacer los hashes la caza la capa que le toca.

use bmo_bsf::*;
use bmo_bsf_x86_64::*;
use bmo_spirv_front::read;
use bmo_spirv_x86_64::{emit, tables_words};

const SUMA: &[u8] = include_bytes!("../../pruebas/suma.spv");
const MANDELBROT: &[u8] = include_bytes!("../../pruebas/mandelbrot.spv");
const SAXPY: &[u8] = include_bytes!("../../pruebas/saxpy.spv");
const DX_MANDELBROT: &[u8] = include_bytes!("../../pruebas/hlsl/mandelbrot.spv");

/// Lo que hace `bmo-bsf-x86-64 fabricar`, en chico.
fn fabricar(entradas: &[(&str, &[u8])]) -> Vec<u8> {
    let mut hechos = Vec::new();
    let mut codigos = Vec::new();
    for (_, spv) in entradas {
        let mut ids = vec![0u32; 1 << 12];
        let m = read(spv, &mut ids).unwrap();
        hechos.push(facts(&m).unwrap());
        let mut tablas = vec![0u32; tables_words(&m)];
        let mut code = vec![0u8; 1 << 20];
        let p = emit(&m, &mut tablas, &mut code).unwrap();
        code.truncate(p.code_len);
        codigos.push((p, code));
    }
    let mut ranuras = vec![[0xFFu8; MAX_BINDINGS]; entradas.len()];
    let objetivos: Vec<[TargetIn; 1]> = codigos
        .iter()
        .zip(&hechos)
        .zip(ranuras.iter_mut())
        .map(|(((p, code), h), s)| [x86_64_target(p, code, h.bindings(), s).unwrap()])
        .collect();
    let modulos: Vec<ModuleIn> = entradas
        .iter()
        .zip(&hechos)
        .zip(&objetivos)
        .map(|(((nombre, spv), h), t)| ModuleIn {
            model: h.model,
            name: nombre.as_bytes(),
            local_size: h.local_size,
            capabilities: h.capabilities,
            caps_high: h.caps_high,
            spirv: spv,
            bindings: h.bindings(),
            targets: t,
        })
        .collect();
    let mut out = vec![0u8; size(&modulos).unwrap()];
    let n = write(&modulos, &mut out).unwrap();
    assert_eq!(n, out.len());
    out
}

/// Un fabricante que miente BIEN: rehace el hash del indice tras tocar una fila.
fn resellar(b: &mut [u8]) {
    let n = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]) as usize;
    let fin = HEADER_BYTES + n(12) * MODULE_BYTES + n(14) * BINDING_BYTES + n(16) * TARGET_BYTES;
    let mut h = bmo_hash::Hasher::new();
    h.update(&b[..32]);
    h.update(&b[HEADER_BYTES..fin]);
    let h = h.finalize();
    b[32..64].copy_from_slice(&h);
}

fn todas(bytes: &[u8]) -> Result<(), Fault> {
    let bsf = Bsf::parse(bytes)?;
    bsf.verify_all()?;
    for i in 0..bsf.module_count() {
        let mut ids = vec![0u32; 1 << 12];
        bsf.deep(i, &mut ids)?;
        let mut tablas = vec![0u32; 1 << 13];
        let mut code = vec![0u8; 1 << 20];
        assert_eq!(reproducir(&bsf, i, &mut ids, &mut tablas, &mut code)?, 1);
    }
    Ok(())
}

fn que(r: Result<(), Fault>) -> What {
    r.expect_err("tenia que decir que no").what
}

#[test]
fn sano_pasa_las_cinco_capas_y_es_determinista() {
    let a = fabricar(&[("suma", SUMA), ("mandelbrot", MANDELBROT), ("saxpy", SAXPY), ("dx_mandelbrot", DX_MANDELBROT)]);
    todas(&a).unwrap();
    let b = fabricar(&[("suma", SUMA), ("mandelbrot", MANDELBROT), ("saxpy", SAXPY), ("dx_mandelbrot", DX_MANDELBROT)]);
    assert_eq!(a, b, "los mismos .spv dan los mismos bytes");
    let bsf = Bsf::parse(&a).unwrap();
    let m = bsf.find(b"mandelbrot").unwrap();
    assert_eq!(m.local_size(), [8, 8, 1]);
    assert_eq!(m.spirv().unwrap(), MANDELBROT);
    let filas: Vec<_> = m.bindings().map(|b| (b.set, b.binding, b.storage, b.access, b.base_bytes, b.stride)).collect();
    assert_eq!(filas, [(0, 0, true, WRITES, 0, 4), (0, 1, false, READS, 24, 0)]);
    assert!(bsf.find(b"nadie").is_none());
}

#[test]
fn ningun_byte_cambia_sin_que_se_note() {
    // El corazon de "atomico": cada byte del fichero tiene propietario --un
    // hash o la regla del cero--, asi que cambiar CUALQUIERA, un bit, lo
    // rechaza. Se prueban los 8 bits de cada byte.
    let bsf = fabricar(&[("suma", SUMA)]);
    let mut b = bsf.clone();
    for i in 0..b.len() {
        for bit in 0..8 {
            b[i] ^= 1 << bit;
            // Abrir y TOMAR todo lo que hay: ningun cambio llega a usarse.
            assert!(Bsf::parse(&b).and_then(|f| f.verify_all()).is_err(), "el bit {} del byte {} cambio y nadie lo vio", bit, i);
            b[i] ^= 1 << bit;
        }
    }
    assert_eq!(b, bsf);
}

#[test]
fn cortado_o_alargado_no_pasa() {
    let bsf = fabricar(&[("suma", SUMA)]);
    for n in 0..bsf.len() {
        assert!(Bsf::parse(&bsf[..n]).is_err(), "cortado a {} bytes", n);
    }
    let mut largo = bsf.clone();
    largo.push(0);
    assert!(matches!(Bsf::parse(&largo).unwrap_err().what, What::TotalBytes { .. }));
}

#[test]
fn cada_mentira_la_caza_su_capa() {
    let sano = fabricar(&[("mandelbrot", MANDELBROT), ("suma", SUMA)]);
    let t0 = HEADER_BYTES + 2 * MODULE_BYTES + 5 * BINDING_BYTES; // objetivo de mandelbrot
    let b0 = HEADER_BYTES + 2 * MODULE_BYTES; // primer buffer de mandelbrot

    // Capa 3: la etiqueta de acceso no cabe en la clase (un uniforme que escribe).
    let mut b = sano.clone();
    b[b0 + BINDING_BYTES + 9] = WRITES;
    resellar(&mut b);
    assert_eq!(que(todas(&b)), What::Access);

    // Capa 3: dos ranuras a la misma fila.
    let mut b = sano.clone();
    b[t0 + 33] = b[t0 + 32];
    resellar(&mut b);
    assert_eq!(que(todas(&b)), What::Target);

    // Capa 3: un buffer que el codigo toca y no tiene ranura (el codigo no
    // podria llegar a el: la tabla y el codigo no hablan del mismo modulo).
    let mut b = sano.clone();
    b[t0 + 28] = 1;
    b[t0 + 33] = 0xFF;
    resellar(&mut b);
    assert_eq!(Bsf::parse(&b).unwrap_err().what, What::Target);

    // Capa 3: el codigo dice venir de otro SPIR-V.
    let mut b = sano.clone();
    b[t0 + 64] ^= 1;
    resellar(&mut b);
    assert_eq!(que(todas(&b)), What::StaleCode);

    // Capa 3: una entrada fuera del codigo.
    let mut b = sano.clone();
    b[t0 + 20..t0 + 24].copy_from_slice(&u32::MAX.to_le_bytes());
    resellar(&mut b);
    assert_eq!(que(todas(&b)), What::Entry);

    // Capa 5: la tabla dice que la salida solo se LEE. La forma es buena, los
    // hashes tambien; solo el SPIR-V sabe que miente.
    let mut b = sano.clone();
    b[b0 + 9] = READS;
    resellar(&mut b);
    Bsf::parse(&b).unwrap();
    assert_eq!(que(todas(&b)), What::Lies("los buffers"));

    // Capa 5: el LocalSize.
    let mut b = sano.clone();
    b[HEADER_BYTES + 4] = 16;
    resellar(&mut b);
    assert_eq!(que(todas(&b)), What::Lies("el LocalSize"));

    // Capa 5: otro codigo con SU hash bien puesto. Solo re-emitir lo ve.
    let mut b = sano.clone();
    let off = u32::from_le_bytes(b[t0 + 8..t0 + 12].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(b[t0 + 12..t0 + 16].try_into().unwrap()) as usize;
    b[off + len - 1] ^= 0x5A;
    let h = bmo_hash::hash(&b[off..off + len]);
    b[t0 + 96..t0 + 128].copy_from_slice(&h);
    resellar(&mut b);
    Bsf::parse(&b).unwrap();
    assert_eq!(que(todas(&b)), What::Lies("el codigo no es el que sale de su SPIR-V"));
}

#[test]
fn el_escritor_se_niega_a_lo_que_no_es_canonico() {
    let mut ids = vec![0u32; 1 << 12];
    let m = read(SUMA, &mut ids).unwrap();
    let h = facts(&m).unwrap();
    let mut filas = h.bindings().to_vec();
    filas.swap(0, 1);
    let modulo = ModuleIn {
        model: h.model,
        name: b"suma",
        local_size: h.local_size,
        capabilities: h.capabilities,
        caps_high: h.caps_high,
        spirv: SUMA,
        bindings: &filas,
        targets: &[],
    };
    let mut out = vec![0u8; size(&[modulo]).unwrap()];
    assert_eq!(write(&[modulo], &mut out).unwrap_err().what, What::Order);
    // Dos modulos con el mismo nombre.
    let modulo = ModuleIn { bindings: h.bindings(), ..modulo };
    let mut out = vec![0u8; size(&[modulo, modulo]).unwrap()];
    assert_eq!(write(&[modulo, modulo], &mut out).unwrap_err().what, What::Name);
    // Chico.
    let mut out = vec![0u8; 10];
    assert!(matches!(write(&[modulo], &mut out).unwrap_err().what, What::NoRoom { .. }));
}

#[test]
fn la_gpu_mira_los_buffers_antes_de_despachar() {
    let bytes = fabricar(&[("mandelbrot", MANDELBROT)]);
    let bsf = Bsf::parse(&bytes).unwrap();
    let m = bsf.module(0);
    let salida = Given { set: 0, binding: 0, addr: 0x1000, bytes: 64 * 64 * 4, writable: true };
    let ventana = Given { set: 0, binding: 1, addr: 0x9000, bytes: 24, writable: false };
    m.check(&[salida, ventana]).unwrap();
    m.check(&[ventana, salida]).unwrap();

    assert_eq!(m.check(&[salida]).unwrap_err().what, What::Missing { set: 0, binding: 1 });
    let chica = Given { bytes: 23, ..ventana };
    assert_eq!(m.check(&[salida, chica]).unwrap_err().what, What::TooSmall { set: 0, binding: 1, need: 24 });
    let fija = Given { writable: false, ..salida };
    assert_eq!(m.check(&[fija, ventana]).unwrap_err().what, What::ReadOnly { set: 0, binding: 0 });
    let otra = Given { binding: 7, ..ventana };
    assert_eq!(m.check(&[salida, ventana, otra]).unwrap_err().what, What::Extra { set: 0, binding: 7 });
    assert_eq!(m.check(&[salida, ventana, ventana]).unwrap_err().what, What::Extra { set: 0, binding: 1 });

    // La tabla, en el orden del CODIGO (las ranuras), no en el de quien llama.
    let t = m.target(kind::X86_64_SCALAR, abi::X86_64_V1, cpu::SSE2).unwrap();
    let mut tabla = [0u64; 2 * MAX_BINDINGS];
    let n = t.table(&m, &[salida, ventana], &mut tabla).unwrap();
    let esperado: Vec<u64> = t.slots().iter().flat_map(|&k| if k == 0 { [0x1000, 64 * 64 * 4] } else { [0x9000, 24] }).collect();
    assert_eq!(&tabla[..n], &esperado[..]);
}

#[test]
fn sin_codigo_para_esta_maquina_es_el_jit_no_un_fallo() {
    let bytes = fabricar(&[("suma", SUMA)]);
    let bsf = Bsf::parse(&bytes).unwrap();
    let m = bsf.module(0);
    assert!(m.target(kind::X86_64_SCALAR, abi::X86_64_V1, cpu::SSE2 | cpu::AVX2).is_some());
    assert!(m.target(kind::X86_64_SCALAR, abi::X86_64_V1, 0).is_none(), "una CPU sin SSE2");
    assert!(m.target(kind::X86_64_SCALAR, 2, cpu::SSE2).is_none(), "otra ABI");
    assert!(m.target(2, abi::X86_64_V1, cpu::SSE2).is_none(), "otra maquina");
    // Y el SPIR-V sigue ahi para el JIT.
    assert_eq!(m.spirv().unwrap(), SUMA);
}

#[test]
fn bytes_hostiles_no_hacen_panico() {
    // Bytes al azar con la cabecera buena: nada de lo que traigan puede
    // hacer que el lector entre en panico.
    let base = fabricar(&[("suma", SUMA)]);
    let mut x: u32 = 0x2545_F491;
    for _ in 0..3000 {
        let mut b = base.clone();
        for _ in 0..8 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = (x as usize) % b.len();
            b[i] = (x >> 24) as u8;
        }
        if Bsf::parse(&b).and_then(|f| f.verify_all()).is_ok() {
            assert_eq!(b, base);
        }
    }
}

#[test]
fn el_hash_de_un_blob_se_paga_al_tomarlo() {
    // Abrir cuesta las capas 1 a 3; el hash de cada blob se paga cuando se
    // TOMA. Un codigo tocado deja abrir el fichero, pero no deja tomarlo: no
    // hay otra puerta a esos bytes.
    let mut b = fabricar(&[("mandelbrot", MANDELBROT), ("suma", SUMA)]);
    let t0 = HEADER_BYTES + 2 * MODULE_BYTES + 5 * BINDING_BYTES;
    let off = u32::from_le_bytes(b[t0 + 8..t0 + 12].try_into().unwrap()) as usize;
    b[off] ^= 0x90;
    let bsf = Bsf::parse(&b).unwrap();
    let t = bsf.module(0).target(kind::X86_64_SCALAR, abi::X86_64_V1, cpu::SSE2).unwrap();
    assert_eq!(t.code().unwrap_err().what, What::CodeHash);
    // El otro modulo no se toco, y se toma.
    bsf.find(b"suma").unwrap().targets().next().unwrap().code().unwrap();
    bsf.find(b"suma").unwrap().spirv().unwrap();
    assert_eq!(bsf.verify_all().unwrap_err().what, What::CodeHash);
}
