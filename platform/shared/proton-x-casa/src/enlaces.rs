//! **Lo compilado de cada PSO, sin repetir** (03-10): los PSO que comparten
//! sombreador de vertices, de pixeles e input layout comparten UN enlace.
//!
//! [carril]  VERDE     memoria de la casa; no toca la maquina
//! [cuesta]  DATO      una llave mal hecha da a un PSO los sombreadores de
//!                     otro: dibuja otra cosa
//! [riesgo]  UNICO     la llave es la huella de 16 bytes que el compilador
//!                     pone en cada contenedor (y sin ella, una propia de
//!                     todos sus bytes)
//! [consumo] NADA      solo al crear un PSO
//!
//! # Por que
//!
//! En el metal (03-10, tercera corrida) Cyberpunk murio a los 9 s con el
//! monton de la casa LLENO: `memory allocation of 57344 bytes failed;
//! monton 67098656 B en uso de 67108864`, con solo 176 PSO. Cada PSO
//! guardaba sus dos `Sombreador` enteros -- el modulo de LLVM leido, con
//! cada registro en su propio `Vec`: unas SEIS veces lo que mide el DXIL --
//! para sacar de ellos, al dibujar, solo el NOMBRE de su funcion. Antes no
//! se notaba porque casi todos los PSO se negaban (el formato de vertice,
//! N3.1); al abrirlos, se quedaron.
//!
//! ```text
//!    la llave     huella del VS, huella del PS, el input layout
//!    lo que vale  los nombres de las dos funciones y el enlace (o por que
//!                 no se puede), en un `Rc`: lo comparten todos sus PSO
//!    al acertar   no se lee ni el DXIL: los PSO de Cyberpunk que solo
//!                 cambian mezcla, profundidad o formato salen gratis
//! ```

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::lote::{ElementoIa, Enlace};

/// **Lo que se queda de los sombreadores de un PSO.**
#[derive(Debug)]
pub struct Compilado {
    /// El nombre de la funcion de entrada del de vertices y del de pixeles.
    pub nombres: (String, String),
    /// Los dos, compilados y cosidos, o por que no se pueden correr.
    pub enlace: Result<Enlace, String>,
}

type Llave = ([u8; 16], [u8; 16]);

struct Global(UnsafeCell<BTreeMap<Llave, Vec<(Vec<ElementoIa>, Rc<Compilado>)>>>);
// SAFETY: la casa corre en un hilo a la vez (ver `Global` en lib.rs).
unsafe impl Sync for Global {}
static HECHOS: Global = Global(UnsafeCell::new(BTreeMap::new()));

fn hechos() -> &'static mut BTreeMap<Llave, Vec<(Vec<ElementoIa>, Rc<Compilado>)>> {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *HECHOS.0.get() }
}

pub(crate) fn reiniciar() {
    hechos().clear();
}

/// **Cuantos enlaces distintos hay, y cuantos se pueden correr** (para el
/// pulso: lo que de verdad ocupa la casa por sus PSO).
pub(crate) fn cuantos() -> (usize, usize) {
    hechos().values().flatten().fold((0, 0), |(n, ok), (_, c)| (n + 1, ok + c.enlace.is_ok() as usize))
}

/// **La huella de un contenedor DXBC**: la que trae (bytes 4..20, la que
/// pone `dxc` o FXC al firmarlo) o, si no trae (ceros), una propia: FNV-1a
/// de todos sus bytes, dos veces con semillas distintas, y su medida.
pub fn huella(d: &[u8]) -> [u8; 16] {
    if let Some(h) = d.get(4..20) {
        if d.starts_with(b"DXBC") && h.iter().any(|&b| b != 0) {
            let mut x = [0u8; 16];
            x.copy_from_slice(h);
            return x;
        }
    }
    let fnv = |semilla: u64| d.iter().fold(semilla, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let mut x = [0u8; 16];
    x[..8].copy_from_slice(&(fnv(0xcbf2_9ce4_8422_2325) ^ d.len() as u64).to_le_bytes());
    x[8..].copy_from_slice(&fnv(0x8422_2325_cbf2_9ce4).to_le_bytes());
    x
}

/// **Lo compilado de `(vs, ps, entradas)`**: lo de antes si ya se hizo, o lo
/// que haga `hacer` (y se guarda). `true` si es nuevo. Lo que `hacer` niega
/// no se guarda (es raro, y su texto lo dice el PSO).
pub(crate) fn de(vs: &[u8], ps: &[u8], entradas: &[ElementoIa], hacer: impl FnOnce() -> Result<Compilado, &'static str>) -> Result<(Rc<Compilado>, bool), &'static str> {
    let llave = (huella(vs), huella(ps));
    if let Some(v) = hechos().get(&llave) {
        if let Some((_, c)) = v.iter().find(|(e, _)| e.as_slice() == entradas) {
            return Ok((c.clone(), false));
        }
    }
    let c = Rc::new(hacer()?);
    hechos().entry(llave).or_default().push((entradas.to_vec(), c.clone()));
    Ok((c, true))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::string::ToString;

    fn ia(s: &str) -> ElementoIa {
        ElementoIa { semantica: s.into(), indice: 0, formato: 2, ranura: 0, desde: 0 }
    }

    fn hecho(n: &str) -> Result<Compilado, &'static str> {
        Ok(Compilado { nombres: (n.to_string(), n.to_string()), enlace: Err("sin enlace".to_string()) })
    }

    #[test]
    fn el_mismo_vs_ps_y_layout_se_comparte() {
        reiniciar();
        let mut vs = alloc::vec![0u8; 64];
        vs[..4].copy_from_slice(b"DXBC");
        vs[4] = 7;
        let ps = alloc::vec![1u8; 40];
        let (a, nuevo) = de(&vs, &ps, &[ia("POSITION")], || hecho("a")).unwrap();
        assert!(nuevo);
        let (b, nuevo) = de(&vs, &ps, &[ia("POSITION")], || panic!("ya estaba: no se vuelve a leer")).unwrap();
        assert!(!nuevo && Rc::ptr_eq(&a, &b));
        // Otro layout: otro enlace.
        let (c, nuevo) = de(&vs, &ps, &[ia("NORMAL")], || hecho("c")).unwrap();
        assert!(nuevo && !Rc::ptr_eq(&a, &c));
        // Lo negado no se guarda.
        assert!(de(&ps, &vs, &[], || Err("no")).is_err());
        assert!(de(&ps, &vs, &[], || hecho("d")).unwrap().1);
        reiniciar();
    }

    #[test]
    fn la_huella_de_dxbc_y_la_propia() {
        let mut d = alloc::vec![0u8; 32];
        d[..4].copy_from_slice(b"DXBC");
        let sin = huella(&d);
        d[31] = 1;
        assert_ne!(huella(&d), sin, "sin firma: la huella es de todos los bytes");
        d[4..20].copy_from_slice(&[9; 16]);
        assert_eq!(huella(&d), [9; 16], "con firma: la del compilador");
        d[31] = 2;
        assert_eq!(huella(&d), [9; 16]);
    }
}
