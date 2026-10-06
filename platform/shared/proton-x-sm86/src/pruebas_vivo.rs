//! A9 (06-10): el .bsf vivo -- generar, comprobar bit a bit, guardar y
//! recordar; y lo que NO se recuerda (un sobre tocado, otro mapa, un
//! cuerpo que no da los bits de la CPU).

extern crate std;

use alloc::string::String;
use alloc::vec::Vec;
use std::collections::BTreeMap;

use bmo_proton_x::dxil;
use bmo_proton_x::lote::{self, ElementoIa, Enlace};

use crate::puerta::cuerpos_con;
use crate::vivo::{a_bsf, comprobar, de_bsf, malo, mapa, nombre, Origen, Recuerdo};
use crate::{emitir_con, Abi};

const CUBO_VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const CUBO_PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");
const TEX_VS: &[u8] = include_bytes!("../../proton-x/prueba/textura_vs.dxil");
const TEX_PS: &[u8] = include_bytes!("../../proton-x/prueba/textura_ps.dxil");

fn e(s: &str, formato: u32, desde: u32) -> ElementoIa {
    ElementoIa { semantica: s.into(), indice: 0, formato, ranura: 0, desde, por_instancia: None }
}

fn cubo() -> (Enlace, Vec<ElementoIa>) {
    let ia = std::vec![e("POSITION", 6, 0), e("NORMAL", 6, 12), e("COLOR", 2, 24)];
    (lote::enlazar(&dxil::leer(CUBO_VS).unwrap(), &dxil::leer(CUBO_PS).unwrap(), &ia).unwrap(), ia)
}

fn textura() -> (Enlace, Vec<ElementoIa>) {
    let ia = std::vec![e("POSITION", 6, 0), e("TEXCOORD", 16, 12)];
    (lote::enlazar(&dxil::leer(TEX_VS).unwrap(), &dxil::leer(TEX_PS).unwrap(), &ia).unwrap(), ia)
}

/// Un recuerdo en la memoria, que cuenta lo que se le pide.
#[derive(Default)]
struct Memoria {
    ficheros: BTreeMap<String, Vec<u8>>,
    leidos: usize,
    guardados: usize,
}

impl Recuerdo for Memoria {
    fn leer(&mut self, nombre: &str) -> Option<Vec<u8>> {
        self.leidos += 1;
        self.ficheros.get(nombre).cloned()
    }
    fn guardar(&mut self, nombre: &str, bsf: &[u8]) -> bool {
        self.guardados += 1;
        self.ficheros.insert(nombre.into(), bsf.to_vec());
        true
    }
}

/// *** El mapa es el MISMO para el mismo PSO (dos enlaces de los mismos
/// bytes), otro para otro PSO, empieza por "MAPA" y mide un multiplo de 4.
#[test]
fn el_mapa_es_la_identidad_del_pso() {
    let (a, b, t) = (cubo().0, cubo().0, textura().0);
    assert_eq!(mapa(&a), mapa(&b));
    assert_ne!(mapa(&a), mapa(&t));
    assert!(mapa(&a).starts_with(b"MAPA 1\nemisor 2\nvs Programa"));
    assert_eq!(mapa(&a).len() % 4, 0);
    let n = nombre(&mapa(&a));
    assert!(n.len() == 68 && n.ends_with(".bsf") && n[..64].bytes().all(|c| c.is_ascii_hexdigit()), "{n}");
}

/// *** Lo emitido para el cubo y para el de la textura PASA la comprobacion
/// bit a bit (las cuatro tandas, cada salida escrita); y el cuerpo de OTRO
/// programa NO pasa.
#[test]
fn la_comprobacion_caza_un_cuerpo_que_no_es_el_suyo() {
    for (en, _) in [cubo(), textura()] {
        for p in [&en.vs, &en.ps] {
            let x = emitir_con(p, 64, Abi::Registros).unwrap();
            assert!(comprobar(p, &x, "prueba").unwrap() > 0);
        }
    }
    let (c, t) = (cubo().0, textura().0);
    let otro = emitir_con(&t.vs, 64, Abi::Registros).unwrap();
    let m = comprobar(&c.vs, &otro, "de vertice").unwrap_err();
    assert!(m.contains("en la 3060") && m.contains("en la CPU"), "{m}");
}

/// *** El .bsf: ida y vuelta, el sobre dice que su fuente es un MAPA (y la
/// capa profunda no tiene SPIR-V que releer), otro mapa no lo abre, y un
/// byte cambiado tampoco.
#[test]
fn el_bsf_del_pso_va_y_vuelve_y_no_se_deja_tocar() {
    let (en, _) = cubo();
    let (ev, ep) = (emitir_con(&en.vs, 64, Abi::Registros).unwrap(), emitir_con(&en.ps, 64, Abi::Registros).unwrap());
    let m = mapa(&en);
    let b = a_bsf(&m, &ev, &ep);
    let (v2, p2) = de_bsf(&b, &m).unwrap();
    assert_eq!((v2.codigo, v2.registros, v2.precargas), (ev.codigo.clone(), ev.registros, ev.precargas.clone()));
    assert_eq!((p2.codigo, p2.registros, p2.precargas), (ep.codigo.clone(), ep.registros, ep.precargas.clone()));
    let sobre = bmo_bsf::Bsf::parse(&b).unwrap();
    assert!(sobre.module(0).es_mapa());
    let mut ids = [0u32; 64];
    assert_eq!(sobre.deep(0, &mut ids).unwrap_err().what, bmo_bsf::What::Mapa);
    assert_eq!(de_bsf(&b, &mapa(&textura().0)).unwrap_err(), "su mapa no es el de ahora");
    let mut tocado = b.clone();
    let k = tocado.len() - 20;
    tocado[k] ^= 1;
    assert!(de_bsf(&tocado, &m).is_err(), "un bit del codigo cambiado");
}

/// *** VIVO Y RECORDADO: la primera vez se traduce, se comprueba y se
/// guarda; la segunda (otra puerta: otro arranque) sale del recuerdo, con
/// los MISMOS cuerpos, sin traducir. Y un .bsf tocado en el disco se
/// vuelve a traducir (y se guarda bien).
#[test]
fn la_segunda_vez_no_se_traduce_nada() {
    let (en, ia) = cubo();
    let mut r = Memoria::default();
    let (c1, v1) = cuerpos_con(&en, &ia, Some(&mut r)).unwrap();
    assert_eq!((v1.origen, r.guardados, r.ficheros.len()), (Origen::Traducido, 1, 1));
    // Otro enlace de los mismos bytes: lo que un arranque nuevo tendria.
    let (en2, ia2) = cubo();
    let (c2, v2) = cuerpos_con(&en2, &ia2, Some(&mut r)).unwrap();
    assert_eq!((v2.origen, r.guardados), (Origen::Recordado, 1));
    assert_eq!(v2.nombre.as_deref(), Some(nombre(&mapa(&en)).as_str()));
    assert_eq!((c1.vs, c1.ps, c1.cargas_vs, c1.cargas_ps), (c2.vs, c2.ps, c2.cargas_vs, c2.cargas_ps));
    // El de la textura es OTRO .bsf.
    let (t, ia_t) = textura();
    assert_eq!(cuerpos_con(&t, &ia_t, Some(&mut r)).unwrap().1.origen, Origen::Traducido);
    assert_eq!(r.ficheros.len(), 2);
    // Un .bsf tocado: no se cree, se traduce y se guarda otra vez.
    let n = nombre(&mapa(&en));
    let f = r.ficheros.get_mut(&n).unwrap();
    let k = f.len() - 20;
    f[k] ^= 1;
    assert_eq!(cuerpos_con(&en, &ia, Some(&mut r)).unwrap().1.origen, Origen::Traducido);
    assert_eq!(cuerpos_con(&en, &ia, Some(&mut r)).unwrap().1.origen, Origen::Recordado, "el bueno, guardado encima");
}

/// *** A9c: un PSO que el vigia marco MALO (con los datos de un juego) no
/// vuelve a la 3060 en el arranque siguiente: ni se lee su .bsf ni se
/// traduce; se dice por que.
#[test]
fn un_pso_marcado_malo_no_vuelve_a_la_3060() {
    let (en, ia) = cubo();
    let mut r = Memoria::default();
    cuerpos_con(&en, &ia, Some(&mut r)).unwrap();
    let n = nombre(&mapa(&en));
    assert_eq!(malo(&n), std::format!("{}.malo", &n[..64]));
    r.guardar(&malo(&n), b"la salida 1.2 no cuadra");
    let e = cuerpos_con(&en, &ia, Some(&mut r)).unwrap_err();
    assert_eq!(e, crate::pso::NoVa::Juez("una revision con los datos del juego lo marco malo", "la salida 1.2 no cuadra".into()));
}
