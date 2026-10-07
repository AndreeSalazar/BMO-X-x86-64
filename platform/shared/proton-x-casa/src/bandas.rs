//! **LAS FRANJAS, REPARTIDAS ENTRE NUCLEOS** (H4.3 de
//! `PLAN_LOS_DOCE_DIRECTORES`, 07-10).
//!
//! [carril]  ROJO      parte el destino de un dibujo entre varios nucleos
//! [cuesta]  DATO -- una franja que pisa a otra no falla: deja pixeles de
//!           otra cuenta. El banco compara byte a byte con el dibujo entero.
//! [riesgo]  UNICO -- cada franja escribe SUS filas y SU casilla
//! [consumo] NADA      solo cuando hay obreros y el dibujo se deja partir
//!
//! Lo que decide (que filas, que tijera, si se parte, como se suman las
//! cuentas) es puro y vive en `bmo_proton_x::bandas`, con su banco. Aqui lo
//! que necesita `unsafe`: el destino a trozos CRUDOS para que cada nucleo
//! rehaga su `&mut` (solo pinta dentro de su tijera, sus filas), una casilla
//! por franja para su cuenta, y el [`Recuerdo`] de las texturas dinamicas
//! que aguantan varios a la vez.
//!
//! [!] Una franja que fallo a medias y se rehace con MEZCLA (o stencil que
//! suma) mezcla dos veces lo que ya habia pintado. Solo pasa si un obrero
//! toma una excepcion o hay que rescatarlo; se cuenta (`smp`: `sub=`).

use alloc::vec::Vec;
use core::cell::UnsafeCell;

use bmo_proton_x::bandas::{antes, cuantas, de_franja, filas, juntar, tijera, Franja};
use bmo_proton_x::lote::{Lote, NoDibuja};
use bmo_proton_x::trama::{Cuenta, Destino, Otro};

/// **Quien reparte**: `repartir(n, f)` corre `f(k)` para cada `k` en `0..n`
/// (en los nucleos que haya) y devuelve una mascara con las partes que NO
/// corrieron enteras (las rehace quien llama, en orden).
pub type Repartir<'a> = &'a dyn Fn(u32, &(dyn Fn(u32) + Sync)) -> u64;

/// Un trozo de memoria del destino, crudo: cada franja rehace su `&mut`.
#[derive(Clone, Copy)]
struct Trozo<T> {
    p: *mut T,
    n: usize,
}

impl<T> Trozo<T> {
    fn de(s: &mut [T]) -> Self {
        Trozo { p: s.as_mut_ptr(), n: s.len() }
    }
    /// # Safety
    /// El trozo sigue vivo, y quien lo usa solo escribe sus filas.
    unsafe fn rehacer<'x>(self) -> &'x mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.p, self.n) }
    }
}

/// El destino entero, crudo (para rehacer uno por franja).
struct Crudo {
    pixeles: Trozo<u32>,
    ancho: u32,
    alto: u32,
    bgra: bool,
    z: Option<Trozo<u32>>,
    cadena: bool,
    otros: Vec<(Option<Trozo<u32>>, bool, Option<u32>)>,
    flotante: Option<u32>,
    stencil: Option<Trozo<u8>>,
}

/// Lo que se comparte entre los nucleos: solo se LEE (el lote, el destino
/// crudo, quien pinta) o cada uno escribe SU casilla (`cuentas[k]`).
struct Compartido<'l, 'a> {
    lote: &'l Lote<'a>,
    crudo: &'l Crudo,
    franja: Franja<'l>,
    n: u32,
    cuentas: Vec<UnsafeCell<Option<Result<Cuenta, NoDibuja>>>>,
}

// SAFETY: el lote y el destino crudo solo se leen; cada franja escribe sus
// filas del destino (disjuntas, `filas`) y su casilla de `cuentas`. Las
// texturas dinamicas del lote: solo con `dinamicas_seguras` (`se_parte`).
// [hilos] hecho -- cada franja escribe sus filas y su casilla; lo demas solo se lee
unsafe impl Sync for Compartido<'_, '_> {}

impl Compartido<'_, '_> {
    /// La franja `k`: su lote, su destino, y la cuenta en su casilla.
    fn pintar(&self, k: u32) {
        let (y0, y1) = filas(k, self.n, self.crudo.alto);
        let r = match tijera(self.lote.reglas.tijera, y0, y1) {
            None => Ok(Cuenta::default()),
            Some(t) => {
                let l = de_franja(self.lote, t);
                let c = self.crudo;
                // SAFETY: el destino vive mientras dura el reparto; esta
                // franja solo pinta dentro de su tijera (sus filas).
                let mut otros: Vec<Otro> = c.otros.iter().map(|&(p, bgra, flotante)| Otro { pixeles: p.map(|p| unsafe { p.rehacer() }), bgra, flotante }).collect();
                let mut d = Destino {
                    pixeles: unsafe { c.pixeles.rehacer() },
                    ancho: c.ancho,
                    alto: c.alto,
                    bgra: c.bgra,
                    z: c.z.map(|z| unsafe { z.rehacer() }),
                    cadena: c.cadena,
                    otros: &mut otros,
                    flotante: c.flotante,
                    stencil: c.stencil.map(|s| unsafe { s.rehacer() }),
                };
                (self.franja)(&l, &mut d)
            }
        };
        // SAFETY: la casilla `k` solo la escribe la franja `k`.
        unsafe { *self.cuentas[k as usize].get() = Some(r) };
    }
}

/// **Dibujar `l` en `n` franjas**, repartidas por `repartir` y pintadas por
/// `franja`. Quien llama ya miro `bandas::se_parte`.
pub fn dibujar(l: &Lote, d: &mut Destino, n: u32, repartir: Repartir, franja: Franja) -> Result<Cuenta, NoDibuja> {
    antes(l, d)?;
    let n = cuantas(n, d.alto);
    let crudo = Crudo {
        pixeles: Trozo::de(d.pixeles),
        ancho: d.ancho,
        alto: d.alto,
        bgra: d.bgra,
        z: d.z.as_deref_mut().map(Trozo::de),
        cadena: d.cadena,
        otros: d.otros.iter_mut().map(|o| (o.pixeles.as_deref_mut().map(Trozo::de), o.bgra, o.flotante)).collect(),
        flotante: d.flotante,
        stencil: d.stencil.as_deref_mut().map(Trozo::de),
    };
    let c = Compartido { lote: l, crudo: &crudo, franja, n, cuentas: (0..n).map(|_| UnsafeCell::new(None)).collect() };
    let mal = repartir(n, &|k| c.pintar(k));
    // Las que no salieron, aqui y en orden.
    for k in 0..n {
        // SAFETY: `repartir` volvio: ninguna franja escribe ya.
        let hecha = unsafe { (*c.cuentas[k as usize].get()).is_some() };
        if mal & (1 << k) != 0 || !hecha {
            c.pintar(k);
        }
    }
    let mut cuentas = Vec::with_capacity(n as usize);
    for celda in c.cuentas.into_iter() {
        cuentas.push(celda.into_inner().expect("cada franja pinto")?);
    }
    Ok(juntar(cuentas))
}

/// **Un recuerdo chico que aguantan varios nucleos a la vez**: hasta `N`
/// pares (clave, valor) bajo un cerrojo de giro, SIN pedir memoria dentro
/// (un nucleo que fallara dentro con el cerrojo tomado lo dejaria tomado
/// para siempre; aqui dentro no hay nada que pueda fallar). Lleno, no
/// recuerda mas: quien pregunta lo busca otra vez, que da lo mismo.
///
/// Es la cache de las texturas dinamicas de un dibujo (`tuberia`: una por
/// textura distinta, no por pixel), que el dibujo puede ir en franjas.
pub struct Recuerdo<K: Copy + Eq, V: Copy, const N: usize> {
    tomado: core::sync::atomic::AtomicBool,
    casillas: UnsafeCell<[Option<(K, V)>; N]>,
}

// SAFETY: las casillas solo se tocan con el cerrojo tomado.
// [hilos] hecho -- su cerrojo de giro (sin pedir memoria dentro)
unsafe impl<K: Copy + Eq + Send, V: Copy + Send, const N: usize> Sync for Recuerdo<K, V, N> {}

impl<K: Copy + Eq, V: Copy, const N: usize> Default for Recuerdo<K, V, N> {
    fn default() -> Self {
        Recuerdo { tomado: core::sync::atomic::AtomicBool::new(false), casillas: UnsafeCell::new([None; N]) }
    }
}

impl<K: Copy + Eq, V: Copy, const N: usize> Recuerdo<K, V, N> {
    fn con<R>(&self, f: impl FnOnce(&mut [Option<(K, V)>; N]) -> R) -> R {
        use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};
        while self.tomado.compare_exchange_weak(false, true, Acquire, Relaxed).is_err() {
            core::hint::spin_loop();
        }
        // SAFETY: el cerrojo es nuestro.
        let r = f(unsafe { &mut *self.casillas.get() });
        self.tomado.store(false, Release);
        r
    }

    /// Lo recordado de `k`, o `buscar(k)` (FUERA del cerrojo) recordado.
    pub fn o_buscar(&self, k: K, buscar: impl FnOnce() -> V) -> V {
        if let Some(v) = self.con(|c| c.iter().flatten().find(|(x, _)| *x == k).map(|&(_, v)| v)) {
            return v;
        }
        let v = buscar();
        self.con(|c| {
            if !c.iter().flatten().any(|(x, _)| *x == k) {
                if let Some(libre) = c.iter_mut().find(|x| x.is_none()) {
                    *libre = Some((k, v));
                }
            }
        });
        v
    }
}
