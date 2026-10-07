//! A9b (06-10): la cifra del mapa de la CPU, ida y vuelta -- CADA sombreador
//! de `prueba/` (los DXIL de dxc y los SM5 de FXC) y Enlaces enteros (con
//! GS, sin input layout, con texturas), el MISMO al volver; y lo que no se
//! descifra (otra huella del codigo, un byte de mas, uno de menos).

extern crate std;

use alloc::vec::Vec;

use crate::cifra::{cifrar, descifrar, Cifra, Lector, HUELLA_FUENTE};
use crate::dxil::{self, programa::compilar, programa::Programa};
use crate::lote::{self, ElementoIa, Enlace};

fn leer(n: &str) -> Vec<u8> {
    std::fs::read(std::format!("{}/prueba/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Ida y vuelta, BIT A BIT: lo tomado se vuelve a poner y da los MISMOS
/// bytes (con un `==` de f32, un NaN no se iguala ni a si mismo; aqui
/// cuentan sus bits, la carga incluida), y su `Debug` es el mismo.
fn ida_y_vuelta<T: Cifra + core::fmt::Debug>(x: &T) {
    let mut b = Vec::new();
    x.poner(&mut b);
    let mut e = Lector::nuevo(&b);
    let y = T::tomar(&mut e).expect("se toma");
    assert!(e.al_final(), "ni un byte de mas");
    let mut b2 = Vec::new();
    y.poner(&mut b2);
    assert_eq!(b, b2);
    assert_eq!(std::format!("{x:?}"), std::format!("{y:?}"));
}

/// *** Cada sombreador de `prueba/` que la casa compila, ida y vuelta.
#[test]
fn cada_programa_de_las_pruebas_va_y_vuelve() {
    let mut nombres = Vec::new();
    for dir in ["", "sombras/"] {
        for e in std::fs::read_dir(std::format!("{}/prueba/{dir}", env!("CARGO_MANIFEST_DIR"))).unwrap() {
            let n = e.unwrap().file_name().into_string().unwrap();
            if n.ends_with(".dxil") || n.ends_with(".cso") {
                nombres.push(std::format!("{dir}{n}"));
            }
        }
    }
    nombres.sort();
    let mut hechos = 0;
    for n in &nombres {
        let Ok(s) = dxil::leer(&leer(n)) else { continue };
        let Ok(p) = compilar(&s) else { continue };
        ida_y_vuelta::<Programa>(&p);
        hechos += 1;
    }
    assert!(hechos >= 80, "{hechos} de {} sombreadores", nombres.len());
}

fn e(s: &str, formato: u32, desde: u32) -> ElementoIa {
    ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None }
}

fn enlaces() -> Vec<Enlace> {
    let s = |n: &str| dxil::leer(&leer(n)).unwrap();
    std::vec![
        lote::enlazar(&s("cubo_vs.dxil"), &s("cubo_ps.dxil"), &[e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)]).unwrap(),
        lote::enlazar(&s("textura_vs.dxil"), &s("textura_ps.dxil"), &[e("POSITION", 6, 0), e("TEXCOORD", 16, 12)]).unwrap(),
        // Con GS (triangleadj, SV_PrimitiveID) y sin input layout.
        lote::enlazar_con_gs(&s("ady_vs.dxil"), Some(&s("ady_gstri.dxil")), Some(&s("ady_ps.dxil")), &[]).unwrap(),
    ]
}

/// *** Enlaces enteros, cifrados y descifrados: los mismos.
#[test]
fn un_enlace_cifrado_vuelve_el_mismo() {
    for en in enlaces() {
        let b = cifrar(&en);
        assert!(b.starts_with(b"CIFRA1\0\0"));
        let de = descifrar(&b).expect("se descifra");
        assert_eq!(cifrar(&de), b, "bit a bit");
        assert_eq!(std::format!("{de:?}"), std::format!("{en:?}"));
        ida_y_vuelta(&en);
    }
    assert_eq!(HUELLA_FUENTE.len(), 16);
}

/// *** Lo que NO se descifra: otra huella del codigo (lo cifro otra
/// version de la casa), un byte de mas, uno de menos, otra cabecera.
#[test]
fn lo_cifrado_por_otro_codigo_o_tocado_no_se_cree() {
    let en = &enlaces()[0];
    let b = cifrar(en);
    let mut otra = b.clone();
    otra[8 + 4] ^= 1; // un caracter de la huella
    assert!(descifrar(&otra).is_none(), "otra huella del codigo");
    let mut mas = b.clone();
    mas.push(0);
    assert!(descifrar(&mas).is_none(), "un byte de mas");
    assert!(descifrar(&b[..b.len() - 1]).is_none(), "uno de menos");
    let mut cab = b.clone();
    cab[0] = b'X';
    assert!(descifrar(&cab).is_none(), "otra cabecera");
}
