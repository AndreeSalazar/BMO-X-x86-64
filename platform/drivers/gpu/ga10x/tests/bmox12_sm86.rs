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
                // Los cubos no muestrean: la unica textura seria la 0.
                Precarga::Asa { reg, .. } | Precarga::AsaPar { reg, .. } => Carga::Asa { textura: 0, reg },
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
        let dibujo = tuberia::Dibujo { indices: Some(desde as u32), vertices: 24, descarte: tuberia::Descarte::Traseras, antihorario: false, destino: None, z: None, color: None, texturas: 0, cadena: false, pantalla: false, banco: 0 };
        let m = tuberia::escribir_paquete_dibujo(&mut caja, 1, &bv, &bp, n, d, dibujo).expect("el paquete VRN1 se sostiene");
        assert_eq!(tuberia::leer(&caja[..m]).unwrap().dibujo, dibujo);
        // P3b4c: el mismo, SIN descarte y CON Z (LESS, se escribe, se limpia
        // a 1.0): el de `gpu verrano bmox12 z`. Viaja y vuelve igual.
        let z = bmo_gpu_ga10x::profundidad::Z { funcion: 2, escribir: true, limpiar: Some(bmo_gpu_ga10x::profundidad::UNO) };
        let con_z = tuberia::Dibujo { descarte: tuberia::Descarte::Ninguna, z: Some(z), ..dibujo };
        let m = tuberia::escribir_paquete_dibujo(&mut caja, 1, &bv, &bp, n, d, con_z).expect("con Z se sostiene");
        assert_eq!(tuberia::leer(&caja[..m]).unwrap().dibujo, con_z);
        // Otra limpieza que 1.0 no cabe en VRN1.
        let otra = tuberia::Dibujo { z: Some(bmo_gpu_ga10x::profundidad::Z { limpiar: Some(0.5f32.to_bits()), ..z }), ..con_z };
        assert_eq!(tuberia::escribir_paquete_dibujo(&mut caja, 1, &bv, &bp, n, d, otra), None);
    }
}

/// *** P3b4c: LA RECETA (VRN2) de BMOX-12 -- los CUERPOS que emite PROTON-X,
/// sus cargas, el input layout y los DATOS con indices --, leida y PEGADA
/// como la pega el kernel, da los MISMOS dos programas (byte a byte) que
/// los que ya dibujaron en el metal; el paquete, los mismos DATOS y el
/// mismo dibujo. Y lo que no es de la app, no pasa.
#[test]
fn la_receta_pega_lo_mismo_que_el_metal() {
    use bmo_gpu_ga10x::destino::Destino;
    use bmo_gpu_ga10x::profundidad::{Z, UNO};
    use bmo_gpu_ga10x::receta::{self, NoReceta, Receta, Taller, MAX_CARGAS, MAX_ELEMENTOS, MAX_GENERICOS, NINGUNA, NINGUNO};
    let (pv, pp) = (programa(VS), programa(PS));
    let (ev, ep) = (emitir_con(&pv, 64, Abi::Registros).unwrap(), emitir_con(&pp, 64, Abi::Registros).unwrap());
    let cuerpo = |e: &bmo_proton_x_sm86::Emitido| -> Vec<u8> { e.codigo.iter().flat_map(|&(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect() };
    let carga = |p: &Precarga| match *p {
        Precarga::Entrada { elemento, componente, reg } => Carga::Entrada { elemento, componente, reg },
        Precarga::Fila { fila, reg } => Carga::Fila { fila, reg },
        Precarga::Asa { reg, .. } | Precarga::AsaPar { reg, .. } => Carga::Asa { textura: 0, reg },
    };
    let (cv, cp) = (cuerpo(&ev), cuerpo(&ep));
    let mut b = vec![0u8; tuberia::DATOS_MAX];
    let (n, total, desde) = tanda::datos_indexados(30, 1280, 720, &mut b).expect("caben");
    let dst = Destino { fila: 5120, ancho: 1280, alto: 720, rgb: false };
    let z = Z { funcion: 2, escribir: true, limpiar: Some(UNO) };
    let dibujo = tuberia::Dibujo { indices: Some(desde as u32), vertices: 24, descarte: tuberia::Descarte::Ninguna, antihorario: false, destino: Some((0x1000_0000, dst)), z: Some(z), color: None, texturas: 0, cadena: false, pantalla: false, banco: 0 };
    let mut r = Receta {
        n,
        vs: &cv,
        ps: &cp,
        registros_vs: ev.registros,
        registros_ps: ep.registros,
        salidas: pv.salidas as u32,
        posicion: 0,
        filas: DATOS.filas,
        paso: DATOS.paso,
        elementos: [NINGUNO; MAX_ELEMENTOS],
        n_elementos: DATOS.elementos.len(),
        cargas_vs: [NINGUNA; MAX_CARGAS],
        n_cargas_vs: ev.precargas.len(),
        cargas_ps: [NINGUNA; MAX_CARGAS],
        n_cargas_ps: ep.precargas.len(),
        genericos: [None; MAX_GENERICOS],
        n_genericos: 3,
        datos: &b[..total],
        dibujo,
        texturas: [bmo_gpu_ga10x::texturas::DeApp::NINGUNA; bmo_gpu_ga10x::texturas::MAX_TEXTURAS],
        termometro_vs: None,
        termometro_ps: None,
    };
    r.elementos[..DATOS.elementos.len()].copy_from_slice(DATOS.elementos);
    for (d, p) in r.cargas_vs.iter_mut().zip(&ev.precargas) {
        *d = carga(p);
    }
    for (d, p) in r.cargas_ps.iter_mut().zip(&ep.precargas) {
        *d = carga(p);
    }
    r.genericos[..3].copy_from_slice(&[None, Some(0), Some(1)]);
    let mut caja = vec![0u8; receta::MAX_RECETA];
    let m = receta::escribir(&mut caja, &r).expect("la receta se sostiene");
    assert_eq!(receta::medida(&caja[..receta::CABECERA_2]), Some(m), "la medida sale de la cabecera sola");
    let leida = receta::leer(&caja[..m]).unwrap();
    assert_eq!((leida.n, leida.vs, leida.ps, leida.datos, leida.dibujo), (r.n, r.vs, r.ps, r.datos, r.dibujo));
    let mut t = Box::new(Taller::nuevo());
    receta::pegar(&leida, &mut t).expect("el kernel la pega");
    let (v, p) = pegados();
    assert_eq!(&t.vs[..t.bytes_vs], &bytes(&v)[..], "el de vertice: el MISMO que dibujo en el metal");
    assert_eq!(&t.ps[..t.bytes_ps], &bytes(&p)[..], "el de pixel: el MISMO");
    let q = receta::paquete(&leida, &t, 7);
    assert_eq!((q.ficha, q.n, q.vertices, q.dibujo), (7, n, &b[..total], dibujo));
    // La limpieza del destino (el ClearRenderTargetView) viaja en la receta,
    // y VRN1 no la lleva.
    let limpia = Receta { dibujo: tuberia::Dibujo { color: Some(0xFF10_1018), ..dibujo }, ..r };
    let mc = receta::escribir(&mut caja, &limpia).unwrap();
    assert_eq!(receta::leer(&caja[..mc]).unwrap().dibujo.color, Some(0xFF10_1018));
    assert_eq!(tuberia::escribir_paquete_dibujo(&mut vec![0u8; tuberia::MAX_PAQUETE], 1, &bytes(&pegados().0), &bytes(&pegados().1), n, &b[..total], tuberia::Dibujo { destino: None, color: Some(1), ..dibujo }), None);
    // El color de limpieza: los floats que la 3060 vuelve a redondear al
    // MISMO byte, en el orden de cada formato.
    assert_eq!(tuberia::color_de_limpieza(0xFF10_1018, false), [16.0f32 / 255.0, 16.0 / 255.0, 24.0 / 255.0, 1.0].map(f32::to_bits));
    assert_eq!(tuberia::color_de_limpieza(0xFF10_1018, true), [24.0f32 / 255.0, 16.0 / 255.0, 16.0 / 255.0, 1.0].map(f32::to_bits));
    for byte in 0..=255u32 {
        let x = f32::from_bits(tuberia::color_de_limpieza(byte, true)[0]);
        assert_eq!((x * 255.0).round() as u32, byte, "el byte {byte} vuelve a ser el mismo");
    }
    // Una limpieza de Z que no es 1.0 SI viaja en la receta (VRN1 no).
    let medio = Receta { dibujo: tuberia::Dibujo { z: Some(Z { limpiar: Some(0.5f32.to_bits()), ..z }), ..dibujo }, ..r };
    let m2 = receta::escribir(&mut caja, &medio).unwrap();
    assert_eq!(receta::leer(&caja[..m2]).unwrap().dibujo.z.unwrap().limpiar, Some(0.5f32.to_bits()));

    // *** Lo que NO pasa.
    let no_se_lee = |x: &Receta| receta::escribir(&mut vec![0u8; receta::MAX_RECETA], x);
    // Sin destino: una receta nunca dibuja en la pantalla.
    assert_eq!(no_se_lee(&Receta { dibujo: tuberia::Dibujo { destino: None, ..dibujo }, ..r }), None);
    // Mas vertices de los que caben en los DATOS: el pegamento leeria fuera.
    // (Detras de los 24 van los indices: hasta ahi se lee DENTRO.)
    let caben = (total - 16 * DATOS.filas as usize) / DATOS.paso as usize;
    assert!(no_se_lee(&Receta { dibujo: tuberia::Dibujo { vertices: caben as u32, ..dibujo }, ..r }).is_some());
    assert_eq!(no_se_lee(&Receta { dibujo: tuberia::Dibujo { vertices: caben as u32 + 1, ..dibujo }, ..r }), None);
    // Un paso que se come el final de los datos.
    assert_eq!(no_se_lee(&Receta { paso: DATOS.paso + 16, ..r }), None);
    // Una carga de una fila que no hay.
    let mut fuera = r;
    fuera.cargas_vs[0] = Carga::Fila { fila: DATOS.filas as u16, reg: 0 };
    assert_eq!(no_se_lee(&fuera), None);
    // La receta a medias, o con la ficha puesta: no.
    assert!(receta::leer(&caja[..m2 - 16]).is_none());
    let mut con_ficha = caja[..m2].to_vec();
    con_ficha[4] = 1;
    assert!(receta::leer(&con_ficha).is_none());
    // Un STG en el cuerpo: R7, no sube.
    let stg = (0x186u64 | 1 << 9 | 7 << 12, 0u64);
    let mut malo = cv.clone();
    malo[..8].copy_from_slice(&stg.0.to_le_bytes());
    malo[8..16].copy_from_slice(&stg.1.to_le_bytes());
    let m3 = receta::escribir(&mut caja, &Receta { vs: &malo, ..r }).unwrap();
    let e = receta::pegar(&receta::leer(&caja[..m3]).unwrap(), &mut t).unwrap_err();
    assert!(matches!(e, NoReceta::Cuerpo("vertice", b) if b.instruccion == 0), "{e:?}");
    // Una carga a un registro del PEGAMENTO (el puntero de los datos va
    // detras de los del cuerpo): el pegamento la para.
    let mut ajena = r;
    ajena.cargas_vs[0] = Carga::Entrada { elemento: 0, componente: 0, reg: ev.registros as u8 + 2 };
    let m4 = receta::escribir(&mut caja, &ajena).unwrap();
    let e = receta::pegar(&receta::leer(&caja[..m4]).unwrap(), &mut t).unwrap_err();
    assert!(matches!(e, NoReceta::Pegamento("vertice", bmo_gpu_ga10x::pegamento::NoPega::Carga)), "{e:?}");
}
