//! **EL CUBO DE VERRANO EN INTI, BIT A BIT CONTRA EL JUEZ** (2026-09-26).
//!
//! Peticion del propietario: *"es hora de darle a INTI PRECISO su
//! oportunidad"*. Lo que la CPU calcula para que dibuje la 3060 --girar el
//! cubo, proyectarlo, quitar lo de atras, iluminar-- escrito en INTI
//! (`ejemplos/cubo.inti`) y ejecutado: sus 360 fotogramas tienen que ser los
//! MISMOS bits que `bmo_cubo::tanda::de_fotograma`, el juez que VERRANO usa y
//! que dio las huellas de D3D12 en la 3060.
//!
//! ** Y no hay tolerancia: se comparan los BYTES del fichero. IEEE-754 da un
//! unico resultado para cada + - * /, e INTI ni funde ni reordena (Regla 11):
//! o salen los mismos bits, o el compilador tiene un fallo. Escribir este
//! programa destapo cuatro (ver `GRAMATICA.md` 14d).

use std::path::PathBuf;

use bmo_lower::emu::{run, Machine};

const POR_FOTOGRAMA: usize = 516;

fn emitido() -> bmo_inti_x86_64::Emitido {
    let texto = std::fs::read_to_string(PathBuf::from("../ejemplos/cubo.inti")).expect("no encuentro `ejemplos/cubo.inti`");
    let arbol = bmo_inti_front::armar(&texto);
    assert!(!arbol.hay_errores(), "el programa no se lee: {}", arbol.pintar("cubo.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(&arbol.valor, bmo_inti_front::disposicion::Medidas::cargar(&raices));
    assert!(!plano.hay_errores(), "el plano: {:?}", plano.codigos());
    let tipos = bmo_inti_front::tipos::comprobar(&arbol.valor, &plano.valor);
    assert!(tipos.codigos().is_empty(), "tipos: {}", tipos.pintar("cubo.inti"));
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec).valor;
    bmo_inti_x86_64::emitir(&ir)
}

/// Lo que TIENE que salir: la tanda del juez, en el formato de `cubo.inti`.
fn del_juez() -> Vec<u8> {
    let mut v = vec![0u8; 360 * POR_FOTOGRAMA];
    for f in 0..360u32 {
        let t = bmo_cubo::tanda::de_fotograma(f, 1280, 720).expect("la tanda cabe");
        let r = &mut v[f as usize * POR_FOTOGRAMA..][..POR_FOTOGRAMA];
        r[..4].copy_from_slice(&(t.n as u32).to_le_bytes());
        for (k, tri) in t.tris().iter().enumerate() {
            let mut w = Vec::new();
            for p in tri.clip.iter() {
                w.extend_from_slice(p);
            }
            w.extend_from_slice(&tri.color);
            for (j, x) in w.iter().enumerate() {
                let o = 4 + (k * 16 + j) * 4;
                r[o..o + 4].copy_from_slice(&x.to_bits().to_le_bytes());
            }
        }
    }
    v
}

#[test]
fn la_tanda_en_inti_es_la_del_juez_en_los_360() {
    let e = emitido();
    assert!(e.arranca, "`cubo.inti` tiene `principal`");
    let m = run(Machine::new(e.codigo), 400_000_000);
    let salio = m.archivo("/inti/cubo.tanda").unwrap_or_else(|| panic!("no dejo `/inti/cubo.tanda` (devolvio {})", m.regs[0]));
    let juez = del_juez();
    assert_eq!(salio.len(), juez.len(), "la medida del fichero");
    if let Some(i) = (0..juez.len()).find(|&i| salio[i] != juez[i]) {
        let (f, dentro) = (i / POR_FOTOGRAMA, i % POR_FOTOGRAMA);
        let palabra = |b: &[u8]| {
            let o = f * POR_FOTOGRAMA + (dentro / 4) * 4;
            u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
        };
        panic!(
            "DISTINTO en el fotograma {f}, palabra {} (0 = cuantos; luego 16 por triangulo): INTI {:#010x} ({}), el juez {:#010x} ({})",
            dentro / 4,
            palabra(salio),
            f32::from_bits(palabra(salio)),
            palabra(&juez),
            f32::from_bits(palabra(&juez))
        );
    }
}
