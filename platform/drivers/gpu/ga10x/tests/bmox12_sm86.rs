//! **VERRANO E5: LOS PROGRAMAS DE BMOX-12 PARA LA 3060** -- los `.cso` que
//! FXC compilo para BMOX-12 (el de vertice `f3ef42a0` y el de pixel
//! `4d67f5e4`), por la casa de PROTON-X a su `Programa`, por el emisor a
//! SASS con las entradas en registros, y por el pegamento a la tuberia de
//! VERRANO. Lo que sale es lo que sube el kernel: `sombreadores/bmox12_vs.sm86`
//! y `bmox12_ps.sm86` (SPH y codigo, tal como viajan), y este banco exige que
//! se fabriquen IGUAL (deterministas) y que el juez diga PERFECTO.
//!
//! Y que den lo mismo que V0 (lo que ya dibujo el metal con la huella de
//! D3D12): con los DATOS del fotograma (`bmo_cubo::tanda::datos`), la casa
//! -- que el emisor iguala BIT A BIT (E3) -- saca en cada vertice la MISMA
//! posicion de recorte que la tanda de V0, y en cada cara el MISMO color en
//! 8 bits.
//!
//! Para volver a fabricarlos: `SM86_FIJAR=1 cargo test -p bmo-gpu-ga10x --test bmox12_sm86`.

use bmo_cubo::tanda;
use bmo_gpu_ga10x::pegamento::{pixel, vertice, Carga, Datos, Pegado};
use bmo_gpu_ga10x::sass::juez::{juzgar, juzgar_programa, Contexto};
use bmo_gpu_ga10x::tuberia;
use bmo_proton_x::dxil::{self, programa::compilar, programa::Programa};
use bmo_proton_x_sm86::{emitir_con, Abi, Precarga};

const VS: &[u8] = include_bytes!("../../../../shared/proton-x/prueba/sombras/f3ef42a0.cso");
const PS: &[u8] = include_bytes!("../../../../shared/proton-x/prueba/sombras/4d67f5e4.cso");
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/sombreadores/");
const DATOS: Datos = Datos::float4(tanda::FILAS_CB as u32, tanda::ENTRADAS as u32);
const FOTOGRAMAS: [u32; 4] = [0, 30, 60, 123];

fn programa(d: &[u8]) -> Programa {
    compilar(&dxil::leer(d).unwrap()).unwrap()
}

fn pegados() -> (Pegado, Pegado) {
    let cargas = |e: &bmo_proton_x_sm86::Emitido| -> Vec<Carga> {
        e.precargas
            .iter()
            .map(|p| match *p {
                Precarga::Entrada { elemento, componente, reg } => Carga::Entrada { elemento, componente, reg },
                Precarga::Fila { fila, reg } => Carga::Fila { fila, reg },
            })
            .collect()
    };
    let (pv, pp) = (programa(VS), programa(PS));
    let (ev, ep) = (emitir_con(&pv, 64, Abi::Registros).unwrap(), emitir_con(&pp, 64, Abi::Registros).unwrap());
    // La posicion es la salida 0 del de vertice y la entrada 0 del de pixel.
    let v = vertice(&ev.codigo, ev.registros, &cargas(&ev), DATOS, pv.salidas as u32, 0).unwrap();
    let p = pixel(&ep.codigo, ep.registros, &cargas(&ep), DATOS, &[None, Some(0), Some(1)]).unwrap();
    (v, p)
}

fn bytes(p: &Pegado) -> Vec<u8> {
    let mut b = vec![0u8; tuberia::HUECO];
    let n = p.bytes(&mut b);
    b.truncate(n);
    b
}

/// *** El juez, antes que nada, con los registros de VERRANO; y como
/// viajan (la puerta del kernel).
#[test]
fn el_juez_dice_perfecto() {
    let (v, p) = pegados();
    for (nombre, g) in [("vertice", &v), ("pixel", &p)] {
        let r = juzgar(g.codigo(), &Contexto { registros: tuberia::REGISTROS, sph: Some(&g.sph) });
        match r {
            Ok(x) => eprintln!("bmox12 {nombre}: {x}"),
            Err(b) => panic!("bmox12 {nombre}: {b}"),
        }
        assert!(juzgar_programa(&bytes(g), tuberia::REGISTROS).is_ok());
    }
}

/// Deterministas, y los ficheros del repositorio son los que salen.
#[test]
fn los_ficheros_son_los_que_salen() {
    let (v, p) = pegados();
    let (bv, bp) = (bytes(&v), bytes(&p));
    assert_eq!((bytes(&pegados().0), bytes(&pegados().1)), (bv.clone(), bp.clone()));
    if std::env::var_os("SM86_FIJAR").is_some() {
        std::fs::write(format!("{DIR}bmox12_vs.sm86"), &bv).unwrap();
        std::fs::write(format!("{DIR}bmox12_ps.sm86"), &bp).unwrap();
    }
    assert_eq!(std::fs::read(format!("{DIR}bmox12_vs.sm86")).expect("SM86_FIJAR=1 los fabrica"), bv, "bmox12_vs.sm86 no es el que sale");
    assert_eq!(std::fs::read(format!("{DIR}bmox12_ps.sm86")).expect("SM86_FIJAR=1 los fabrica"), bp, "bmox12_ps.sm86 no es el que sale");
}

fn f4(b: &[u8], i: usize) -> [f32; 4] {
    core::array::from_fn(|k| f32::from_le_bytes(b[i + 4 * k..i + 4 * k + 4].try_into().unwrap()))
}

/// *** Los DATOS de cada fotograma, por la casa: la MISMA posicion de
/// recorte (bit a bit) que la tanda de V0 en cada vertice, y el MISMO color
/// en 8 bits en cada cara. Y el paquete con los dos programas cabe.
#[test]
fn da_lo_mismo_que_v0() {
    let (pv, pp) = (programa(VS), programa(PS));
    let (v, p) = pegados();
    let (bv, bp) = (bytes(&v), bytes(&p));
    let mut b = vec![0u8; tuberia::DATOS_MAX];
    let mut caja = vec![0u8; tuberia::MAX_PAQUETE];
    for f in FOTOGRAMAS {
        let (n, total) = tanda::datos(f, 1280, 720, &mut b).expect("caben");
        let t = tanda::de_fotograma(f, 1280, 720).unwrap();
        assert_eq!(n, 3 * t.n);
        let d = &b[..total];
        assert_eq!(total, DATOS.bytes(n));
        let cb = &d[..16 * tanda::FILAS_CB];
        let (mut sv, mut sp, mut regs) = (vec![[0f32; 4]; pv.salidas], vec![[0f32; 4]; pp.salidas], Vec::new());
        for i in 0..n {
            let base = 16 * tanda::FILAS_CB + 16 * tanda::ENTRADAS * i;
            let ent = [f4(d, base), f4(d, base + 16), f4(d, base + 32)];
            pv.correr(&ent, cb, &mut sv, &mut regs);
            let clip = t.tris[i / 3].clip[i % 3];
            assert_eq!(sv[0].map(f32::to_bits), clip.map(f32::to_bits), "fotograma {f}, vertice {i}: la posicion");
            // El de pixel con lo que sale del de vertice (la cara es de UN
            // valor: lo que interpola la 3060 es eso mismo).
            pp.correr(&[[0.0; 4], sv[1], sv[2]], cb, &mut sp, &mut regs);
            assert_eq!(bmo_cubo::empaquetar(sp[0]), bmo_cubo::empaquetar(t.tris[i / 3].color), "fotograma {f}, vertice {i}: el color");
        }
        let m = tuberia::escribir_paquete_datos(&mut caja, 1, &bv, &bp, n, d).expect("el paquete se sostiene");
        let q = tuberia::leer(&caja[..m]).unwrap();
        assert_eq!((q.n, q.vertices), (n, d));
    }
}

/// *** P3b4b: los DATOS CON INDICES (los 24 vertices del cubo y sus 36
/// indices; el descarte, del hardware): cada triangulo que la tanda de V0
/// deja ver tiene, por sus indices, los MISMOS vertices de recorte (bit a
/// bit) por la casa; y el paquete VRN1 con los dos programas se sostiene.
#[test]
fn con_indices_da_lo_mismo_que_v0() {
    let pv = programa(VS);
    let (v, p) = pegados();
    let (bv, bp) = (bytes(&v), bytes(&p));
    let mut b = vec![0u8; tuberia::DATOS_MAX];
    let mut caja = vec![0u8; tuberia::MAX_PAQUETE];
    for f in FOTOGRAMAS {
        let (n, total, desde) = tanda::datos_indexados(f, 1280, 720, &mut b).expect("caben");
        let d = &b[..total];
        let cb = &d[..16 * tanda::FILAS_CB];
        let indice = |k: usize| u32::from_le_bytes(d[desde + 4 * k..desde + 4 * k + 4].try_into().unwrap()) as usize;
        let t = tanda::de_fotograma(f, 1280, 720).unwrap();
        let (mut sv, mut regs) = (vec![[0f32; 4]; pv.salidas], Vec::new());
        for (j, &k) in t.caras[..t.n].iter().enumerate() {
            for c in 0..3 {
                let base = 16 * tanda::FILAS_CB + 16 * tanda::ENTRADAS * indice(3 * k + c);
                pv.correr(&[f4(d, base), f4(d, base + 16), f4(d, base + 32)], cb, &mut sv, &mut regs);
                assert_eq!(sv[0].map(f32::to_bits), t.tris[j].clip[c].map(f32::to_bits), "fotograma {f}, cara {k}, vertice {c}");
            }
        }
        let dibujo = tuberia::Dibujo { indices: Some(desde as u32), vertices: 24, descarte: tuberia::Descarte::Traseras, antihorario: false, destino: None };
        let m = tuberia::escribir_paquete_dibujo(&mut caja, 1, &bv, &bp, n, d, dibujo).expect("el paquete VRN1 se sostiene");
        assert_eq!(tuberia::leer(&caja[..m]).unwrap().dibujo, dibujo);
    }
}
