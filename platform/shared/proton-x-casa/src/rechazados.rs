//! **Los sombreadores que la casa no sabe correr, guardados** (M1, 14-10).
//!
//! El metal de Cyberpunk dice "el CS no se sabe correr todavia:
//! Forma(...)" y nada mas: sin el DXIL no se sabe QUE escribio `dxc`, y
//! adivinar la forma de una instruccion es justo lo que no se hace. Cada
//! sombreador rechazado se guarda en `proton-x/<juego>/rechazados`, como
//! vino del `.exe`, con su porque al lado:
//!
//! ```text
//!    <huella>_cs.dxil     los bytes del CS (o _vs, _gs, _ps de un PSO)
//!    <huella>.txt         el porque, la frase que dijo la casa
//!    INDICE.TXT           TODOS los de la corrida: huella, etapas, porque,
//!                         y al final cada porque con cuantos sombreadores
//! ```
//!
//! La huella es FNV-1a de 64 bits de todos sus bytes: el mismo sombreador,
//! el mismo nombre, y no se escribe dos veces en la corrida. Con el DXIL en
//! la mano, `dxc -dumpbin` dice la instruccion exacta y la pieza que falta
//! se hace contra eso, con su banco.
//!
//! [carril]  VERDE     escribe en ESTRATOS, en la carpeta del juego; nada mas

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

struct Estado {
    carpeta: Option<String>,
    vistos: Vec<u64>,
    /// Lo de cada uno para el INDICE: huella, etapas y porque.
    indice: Vec<(u64, String, String)>,
}

struct Global {
    ocupado: AtomicBool,
    estado: UnsafeCell<Estado>,
}
// SAFETY: solo Acceso::estado crea un &mut, ligado a un Acceso exclusivo.
// El CAS Acquire admite uno; Drop publica los cambios con Release. Estado es Send.
// [hilos] hecho -- CAS exclusivo y guardia RAII; la reentrada se rechaza sin girar
unsafe impl Sync for Global {}

impl Global {
    const fn nuevo() -> Self {
        Self { ocupado: AtomicBool::new(false), estado: UnsafeCell::new(Estado {
            carpeta: None, vistos: Vec::new(), indice: Vec::new(),
        }) }
    }
    fn tomar(&self) -> Option<Acceso<'_>> {
        self.ocupado.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).ok()?;
        Some(Acceso(self))
    }
}
struct Acceso<'a>(&'a Global);
impl Acceso<'_> {
    fn estado(&mut self) -> &mut Estado {
        // SAFETY: tomar concedio el unico Acceso; la referencia no vive mas que el.
        unsafe { &mut *self.0.estado.get() }
    }
}
impl Drop for Acceso<'_> {
    fn drop(&mut self) { self.0.ocupado.store(false, Ordering::Release); }
}
static ESTADO: Global = Global::nuevo();

/// La carpeta del juego. False conserva el estado anterior si hay escritura
/// en curso; no gira ni mantiene un &mut sin propietario durante un callback.
#[must_use]
pub fn poner_carpeta(carpeta: Option<String>) -> bool {
    let Some(mut acceso) = ESTADO.tomar() else { return false; };
    let e = acceso.estado();
    e.carpeta = carpeta;
    e.vistos.clear();
    e.indice.clear();
    true
}

/// FNV-1a de 64 bits de las etapas, en orden.
pub fn huella(etapas: &[(&str, &[u8])]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (_, b) in etapas {
        for &x in b.iter() {
            h ^= x as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xFF;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// **Guardar un sombreador rechazado**: cada etapa (`"cs"`, `"vs"`...) con
/// sus bytes, y el porque. Devuelve la huella si se escribio ahora; `None`
/// sin carpeta, si ya se guardo, si otro acceso esta en curso o si el disco no quiso.
pub fn guardar(etapas: &[(&str, &[u8])], porque: &str) -> Option<u64> {
    let mut acceso = ESTADO.tomar()?;
    let e = acceso.estado();
    let carpeta = e.carpeta.clone()?;
    let h = huella(etapas);
    if e.vistos.contains(&h) {
        return None;
    }
    e.vistos.push(h);
    let p = crate::plataforma_si()?;
    let mut bien = true;
    for (etapa, b) in etapas.iter().filter(|(_, b)| !b.is_empty()) {
        bien &= (p.escribir_fichero)(alloc::format!("{carpeta}/{h:016x}_{etapa}.dxil").as_bytes(), b);
    }
    let mut texto = String::from(porque);
    texto.push('\n');
    bien &= (p.escribir_fichero)(alloc::format!("{carpeta}/{h:016x}.txt").as_bytes(), texto.as_bytes());
    let nombres: Vec<&str> = etapas.iter().filter(|(_, b)| !b.is_empty()).map(|(n, _)| *n).collect();
    e.indice.push((h, nombres.join("+"), String::from(porque)));
    bien &= (p.escribir_fichero)(alloc::format!("{carpeta}/INDICE.TXT").as_bytes(), indice(&e.indice).as_bytes());
    bien.then_some(h)
}

/// **Guardar y decirlo**: el porque, con donde quedo si se guardo ahora.
pub fn guardar_y_decir(etapas: &[(&str, &[u8])], porque: &str) -> String {
    match guardar(etapas, porque) {
        Some(h) => alloc::format!("{porque} (guardado en rechazados/{h:016x}_*.dxil)"),
        None => String::from(porque),
    }
}

/// **El INDICE**: cada rechazado de la corrida y, al final, cada porque con
/// cuantos sombreadores lo tienen (de mas a menos): la lista de lo que falta.
pub fn indice(lista: &[(u64, String, String)]) -> String {
    let mut s = String::from("# PROTON-X: los sombreadores que la casa no supo correr en esta corrida\n# huella           etapas  porque\n");
    for (h, etapas, porque) in lista {
        s.push_str(&alloc::format!("{h:016x}  {etapas:<6}  {porque}\n"));
    }
    let mut cuantos: Vec<(&str, usize)> = Vec::new();
    for (_, _, porque) in lista {
        match cuantos.iter_mut().find(|x| x.0 == porque.as_str()) {
            Some(x) => x.1 += 1,
            None => cuantos.push((porque.as_str(), 1)),
        }
    }
    cuantos.sort_by(|a, b| b.1.cmp(&a.1));
    s.push_str("# lo que falta, por porque:\n");
    for (porque, n) in cuantos {
        s.push_str(&alloc::format!("#   {n:>4} x {porque}\n"));
    }
    s
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;

    #[test]
    fn la_huella_separa_etapas_y_bytes() {
        let a = huella(&[("cs", b"abc")]);
        assert_eq!(a, huella(&[("cs", b"abc")]), "el mismo, la misma");
        assert_ne!(a, huella(&[("cs", b"abd")]));
        assert_ne!(huella(&[("vs", b"ab"), ("ps", b"c")]), huella(&[("vs", b"a"), ("ps", b"bc")]), "el corte entre etapas cuenta");
    }

    #[test]
    fn el_indice_cuenta_cada_porque() {
        let l = [(1, String::from("cs"), String::from("a")), (2, String::from("vs+ps"), String::from("b")), (3, String::from("cs"), String::from("a"))];
        let s = indice(&l);
        assert!(s.contains("0000000000000002  vs+ps   b\n"));
        assert!(s.find("#      2 x a").unwrap() < s.find("#      1 x b").unwrap(), "de mas a menos");
    }

    #[test]
    fn sin_carpeta_no_se_guarda() {
        let _ = poner_carpeta(None);
        assert_eq!(guardar(&[("cs", b"x")], "porque"), None);
    }

    #[test]
    fn acceso_exclusivo_reentrada_y_liberacion() {
        let global = Global::nuevo();
        let mut a = global.tomar().unwrap();
        a.estado().vistos.push(7);
        assert!(global.tomar().is_none(), "reentrada no espera ni crea otro &mut");
        drop(a);
        assert_eq!(global.tomar().unwrap().estado().vistos, [7]);
    }

    #[test]
    fn unwinding_libera_el_acceso() {
        let global = Global::nuevo();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _a = global.tomar().unwrap();
            panic!("salida de prueba");
        }));
        assert!(r.is_err());
        assert!(global.tomar().is_some());
    }

    #[test]
    fn ocho_hilos_no_pierden_ni_comparten_mutaciones() {
        let global = std::sync::Arc::new(Global::nuevo());
        let mut hilos = Vec::new();
        for hilo in 0..8u64 {
            let g = global.clone();
            hilos.push(std::thread::spawn(move || {
                for n in 0..1000u64 {
                    loop {
                        if let Some(mut a) = g.tomar() {
                            a.estado().vistos.push(hilo * 1000 + n);
                            break;
                        }
                        std::thread::yield_now();
                    }
                }
            }));
        }
        for hilo in hilos { hilo.join().unwrap(); }
        let mut a = global.tomar().unwrap();
        let v = &mut a.estado().vistos;
        v.sort_unstable();
        assert_eq!(v.len(), 8000);
        assert!(v.iter().copied().eq(0..8000));
    }

}
