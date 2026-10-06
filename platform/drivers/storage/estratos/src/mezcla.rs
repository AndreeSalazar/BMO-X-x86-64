//! **LA MEZCLA, POR NODOS** -- la parte que DECIDE, pura (`docs/plan/PLAN_LAS_RAMAS.md`,
//! seccion 2 y escalon R2).
//!
//! [carril]  VERDE     no lee ni escribe un sector: tres listas dentro, una decision fuera
//! [consumo] NADA      corre cuando alguien pide mezclar
//!
//! Tres arboles aplanados: la BASE (el ultimo estrato que las dos ramas
//! comparten), la rama A y la rama B, cada uno como una lista de (ruta, nodo).
//! Para cada ruta se mira QUE NODO hay en cada uno -- su BLAKE3, no su sitio
//! en el disco -- y sale una de dos cosas:
//!
//! ```text
//!    Queda(ruta, nodo)     va al arbol mezclado, ENTERO
//!    Choque(ruta, a, b)    los dos lo cambiaron distinto: elige una PERSONA
//!                          (D3 a), y elige un nodo entero, nunca lineas
//! ```
//!
//! ** POR QUE EL HASH Y NO EL PUNTERO. Un `BlockPtr` dice DONDE esta un nodo
//! (`lba`, `off`) y QUE es (`hash`). Dos ramas que escribieron lo mismo por
//! separado tienen el mismo QUE en dos DONDE distintos: para la mezcla son el
//! mismo nodo, y no es un choque. Y un nodo INDEPENDIZADO (sus bloques
//! copiados aparte para no compartirlos con nadie) sigue siendo el mismo QUE:
//! independizar no cambia nada de lo que la mezcla ve.
//!
//! [!] Lo que NO hace, dicho: no junta dos ediciones de UN fichero en una. Si
//! A y B tocaron `mundo.titan`, sale un Choque y se elige uno de los dos.
//!
//! El coste: cada ruta se busca en las otras listas recorriendolas. Es
//! cuadratico y sin `alloc` -- bien para los cientos de ficheros de un
//! paquete; un volumen entero pediria ordenar antes, y se dira cuando llegue.

use crate::objects::Entrada;
use crate::Hash;

/// Una ruta y el nodo que hay en ella, en uno de los tres arboles.
#[derive(Debug, Clone, Copy)]
pub struct Lado<'a> {
    pub ruta: &'a [u8],
    pub nodo: Hash,
}

/// Lo que la mezcla decide para una ruta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sale<'a> {
    /// Va al arbol mezclado con este nodo.
    Queda(&'a [u8], Hash),
    /// Los dos lados la cambiaron distinto. `None` es "lo quito".
    Choque { ruta: &'a [u8], a: Option<Hash>, b: Option<Hash> },
}

/// Cuantas de cada: lo que el panel dice antes de que nadie elija.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cuenta {
    pub quedan: usize,
    pub de_a: usize,
    pub de_b: usize,
    pub choques: usize,
    pub quitadas: usize,
}

/// La misma ruta? Sin distinguir mayusculas, en Latin-1, igual que las
/// entradas de un directorio (`Entrada::se_llama`): `Mundo` y `mundo` son UN
/// fichero en ESTRATOS, asi que tambien en la mezcla.
fn misma(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| crate::objects::baja(x) == crate::objects::baja(y))
}

fn en(lista: &[Lado], ruta: &[u8]) -> Option<Hash> {
    lista.iter().find(|l| misma(l.ruta, ruta)).map(|l| l.nodo)
}

/// **Mezcla A y B sobre su BASE**, ruta a ruta, y le pasa cada decision a `f`.
///
/// ```text
///    a == b            sale a (o nada, si los dos la quitaron)
///    a == base         solo B la toco: sale b
///    b == base         solo A la toco: sale a
///    si no             CHOQUE
/// ```
///
/// Cada ruta sale UNA vez: primero las de A, luego las que solo tiene B, y
/// por ultimo las de la base que ninguno tiene ya (no sale nada: las quitaron
/// los dos). Devuelve la cuenta.
pub fn mezclar<'a>(base: &[Lado<'a>], a: &[Lado<'a>], b: &[Lado<'a>], mut f: impl FnMut(Sale<'a>)) -> Cuenta {
    let mut c = Cuenta::default();
    let mut decide = |ruta: &'a [u8], c: &mut Cuenta| {
        let (x0, xa, xb) = (en(base, ruta), en(a, ruta), en(b, ruta));
        let (sale, de) = if xa == xb {
            (xa, 0)
        } else if xa == x0 {
            (xb, 2)
        } else if xb == x0 {
            (xa, 1)
        } else {
            c.choques += 1;
            f(Sale::Choque { ruta, a: xa, b: xb });
            return;
        };
        match sale {
            Some(nodo) => {
                match de {
                    1 => c.de_a += 1,
                    2 => c.de_b += 1,
                    _ => c.quedan += 1,
                }
                f(Sale::Queda(ruta, nodo));
            }
            None => c.quitadas += 1,
        }
    };
    for l in a {
        decide(l.ruta, &mut c);
    }
    for l in b {
        if en(a, l.ruta).is_none() {
            decide(l.ruta, &mut c);
        }
    }
    for l in base {
        if en(a, l.ruta).is_none() && en(b, l.ruta).is_none() {
            c.quitadas += 1;
        }
    }
    c
}

/// **LA REGLA DE TRES**, sola: de que lado sale una ruta, mirando el QUE de
/// la base, de A y de B. La usa el motor sin `alloc` (`motor_mezcla`);
/// [`mezclar`] y [`por_carpeta`] la escriben con sus cuentas al lado, y la
/// prueba al azar de `estratos-mezcla` comprueba que las tres dicen lo mismo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regla {
    /// Sale lo que tenga A (o nada, si A no lo tiene).
    A,
    /// Sale lo que tenga B (o nada).
    B,
    /// Los dos lo cambiaron distinto.
    Distintos,
}

pub fn regla(base: Option<Hash>, a: Option<Hash>, b: Option<Hash>) -> Regla {
    if a == b || b == base {
        Regla::A
    } else if a == base {
        Regla::B
    } else {
        Regla::Distintos
    }
}

/// Lo que la mezcla POR CARPETA decide para un nombre (`por_carpeta`).
#[derive(Debug, Clone, Copy)]
pub enum Paso<'e> {
    /// Esta entrada va a la carpeta mezclada TAL CUAL. Si es una subcarpeta,
    /// va ENTERA, con todo lo de dentro, sin bajar a mirarlo.
    Queda(&'e Entrada),
    /// Los dos lados la cambiaron distinto. Si las dos son CARPETAS, quien
    /// llama BAJA y mezcla dentro; si no, es un choque para una persona.
    /// `None` es "no esta en ese lado".
    Distintos { base: Option<&'e Entrada>, a: Option<&'e Entrada>, b: Option<&'e Entrada> },
}

fn en_carpeta<'e>(lista: &'e [Entrada], nombre: &[u8]) -> Option<&'e Entrada> {
    lista.iter().find(|e| misma(e.nombre_bytes(), nombre))
}

/// **La mezcla POR CARPETA**: la misma regla de tres que [`mezclar`], pero
/// sobre las entradas de UNA carpeta en cada arbol, comparando el QUE de cada
/// nodo (su BLAKE3).
///
/// ** POR QUE ES MEJOR QUE APLANAR. Una subcarpeta que solo un lado toco
/// tiene en el otro el MISMO nodo que en la base: sale entera de un golpe, y
/// no se lee ni una de sus hojas. Solo se baja donde los DOS lados cambiaron
/// algo. Cuesta lo que cambio, no lo que hay -- y cada nivel cabe en tres
/// listas fijas, sin `alloc`: es la forma que puede usar el kernel (R4c).
///
/// `cuenta.choques` cuenta aqui los `Distintos` (algunos acabaran siendo
/// carpetas que se bajan, no choques): el resto, como en [`mezclar`].
pub fn por_carpeta<'e>(base: &'e [Entrada], a: &'e [Entrada], b: &'e [Entrada], mut f: impl FnMut(Paso<'e>)) -> Cuenta {
    let mut c = Cuenta::default();
    let hash = |e: Option<&Entrada>| e.map(|e| e.nodo.hash);
    let mut decide = |nombre: &[u8], c: &mut Cuenta| {
        let (e0, ea, eb) = (en_carpeta(base, nombre), en_carpeta(a, nombre), en_carpeta(b, nombre));
        let (x0, xa, xb) = (hash(e0), hash(ea), hash(eb));
        let (sale, de) = if xa == xb {
            (ea, 0)
        } else if xa == x0 {
            (eb, 2)
        } else if xb == x0 {
            (ea, 1)
        } else {
            c.choques += 1;
            f(Paso::Distintos { base: e0, a: ea, b: eb });
            return;
        };
        match sale {
            Some(e) => {
                match de {
                    1 => c.de_a += 1,
                    2 => c.de_b += 1,
                    _ => c.quedan += 1,
                }
                f(Paso::Queda(e));
            }
            None => c.quitadas += 1,
        }
    };
    for e in a {
        decide(e.nombre_bytes(), &mut c);
    }
    for e in b {
        if en_carpeta(a, e.nombre_bytes()).is_none() {
            decide(e.nombre_bytes(), &mut c);
        }
    }
    for e in base {
        if en_carpeta(a, e.nombre_bytes()).is_none() && en_carpeta(b, e.nombre_bytes()).is_none() {
            c.quitadas += 1;
        }
    }
    c
}

/// La BASE de dos ramas: el estrato mas reciente que las dos cadenas tienen.
///
/// `padres_a` y `padres_b` son las cadenas de cada punta hacia atras, la punta
/// primero (lo que `historia` ya recorre). Cuadratico, y bien: la historia
/// guardada son 32 versiones (`historia::MAX` del kernel). `None`: no
/// comparten nada, y sin base no hay mezcla de tres -- se dice, no se adivina.
pub fn base<T: PartialEq + Copy>(padres_a: &[T], padres_b: &[T]) -> Option<T> {
    padres_a.iter().find(|x| padres_b.contains(x)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    fn h(n: u8) -> Hash {
        [n; 32]
    }

    fn l(ruta: &str, n: u8) -> Lado<'_> {
        Lado { ruta: ruta.as_bytes(), nodo: h(n) }
    }

    fn todo<'a>(base: &[Lado<'a>], a: &[Lado<'a>], b: &[Lado<'a>]) -> (Vec<Sale<'a>>, Cuenta) {
        let mut v = Vec::new();
        let c = mezclar(base, a, b, |s| v.push(s));
        (v, c)
    }

    fn queda(v: &[Sale], ruta: &str) -> Option<u8> {
        v.iter().find_map(|s| match s {
            Sale::Queda(r, n) if *r == ruta.as_bytes() => Some(n[0]),
            _ => None,
        })
    }

    /// La tabla de `PLAN_LAS_RAMAS.md`, seccion 2, fila a fila.
    #[test]
    fn la_tabla_del_plan_fila_a_fila() {
        let base = [l("nadie", 1), l("solo_a", 2), l("solo_b", 3), l("igual", 4), l("choque", 5), l("quita_a", 7)];
        let a = [l("nadie", 1), l("solo_a", 20), l("solo_b", 3), l("igual", 40), l("choque", 50), l("nuevo_a", 6)];
        let b = [l("nadie", 1), l("solo_a", 2), l("solo_b", 30), l("igual", 40), l("choque", 51), l("quita_a", 7)];
        // La ultima fila (A lo quita, B lo cambia) va en su prueba propia.
        let (v, c) = todo(&base, &a, &b);
        assert_eq!(queda(&v, "nadie"), Some(1));
        assert_eq!(queda(&v, "solo_a"), Some(20));
        assert_eq!(queda(&v, "solo_b"), Some(30));
        assert_eq!(queda(&v, "igual"), Some(40));
        assert_eq!(queda(&v, "nuevo_a"), Some(6));
        assert_eq!(queda(&v, "quita_a"), None, "A lo quito y B no lo toco: no sale");
        assert!(v.contains(&Sale::Choque { ruta: b"choque", a: Some(h(50)), b: Some(h(51)) }));
        assert_eq!(c.choques, 1);
        assert_eq!(c.quitadas, 1);
    }

    #[test]
    fn quitar_de_un_lado_y_cambiar_del_otro_es_un_choque() {
        let base = [l("mundo.titan", 1)];
        let a: [Lado; 0] = [];
        let b = [l("mundo.titan", 2)];
        let (v, c) = todo(&base, &a, &b);
        assert_eq!(v, [Sale::Choque { ruta: b"mundo.titan", a: None, b: Some(h(2)) }]);
        assert_eq!(c.choques, 1);
    }

    #[test]
    fn quitarlo_los_dos_no_deja_nada_y_no_es_choque() {
        let base = [l("viejo", 1)];
        let (v, c) = todo(&base, &[], &[]);
        assert!(v.is_empty());
        assert_eq!((c.choques, c.quitadas), (0, 1));
    }

    #[test]
    fn crear_lo_mismo_en_los_dos_lados_no_es_choque() {
        // El mismo QUE escrito por separado: un nodo, no un choque.
        let (v, c) = todo(&[], &[l("nota", 9)], &[l("nota", 9)]);
        assert_eq!(v, [Sale::Queda(b"nota", h(9))]);
        assert_eq!(c.choques, 0);
        // Creado distinto en cada lado, sin base: choque.
        let (v, _) = todo(&[], &[l("nota", 9)], &[l("nota", 10)]);
        assert_eq!(v, [Sale::Choque { ruta: b"nota", a: Some(h(9)), b: Some(h(10)) }]);
    }

    #[test]
    fn cada_ruta_sale_una_vez_y_sin_mirar_mayusculas() {
        let base = [l("Mundo", 1)];
        let a = [l("mundo", 1), l("otra", 2)];
        let b = [l("MUNDO", 3), l("otra", 2)];
        let (v, c) = todo(&base, &a, &b);
        let rutas: Vec<&[u8]> = v.iter().map(|s| match s {
            Sale::Queda(r, _) | Sale::Choque { ruta: r, .. } => *r,
        }).collect();
        assert_eq!(rutas.len(), 2, "{v:?}");
        assert_eq!(c.de_b, 1, "solo B cambio `mundo`");
        assert_eq!(c.quedan, 1);
    }

    fn ent(nombre: &str, n: u8) -> Entrada {
        Entrada::de_bytes(nombre.as_bytes(), crate::objects::BlockPtr { lba: n as u64, off: 0, len: 1, hash: h(n) }).unwrap()
    }

    #[test]
    fn por_carpeta_una_subcarpeta_tocada_por_un_lado_sale_entera() {
        // `mundo/` solo cambio en B: sale el nodo de B ENTERO, sin bajar.
        let base = [ent("mundo", 1), ent("leeme", 2)];
        let a = [ent("mundo", 1), ent("leeme", 20)];
        let b = [ent("mundo", 10), ent("leeme", 2)];
        let mut v = Vec::new();
        let c = por_carpeta(&base, &a, &b, |p| v.push(p));
        let quedan: Vec<(&[u8], u8)> = v.iter().filter_map(|p| match p {
            Paso::Queda(e) => Some((e.nombre_bytes(), e.nodo.hash[0])),
            _ => None,
        }).collect();
        assert_eq!(quedan, [(&b"mundo"[..], 10), (&b"leeme"[..], 20)]);
        assert_eq!((c.de_a, c.de_b, c.choques), (1, 1, 0));
    }

    #[test]
    fn por_carpeta_lo_que_cambiaron_los_dos_es_distintos_y_quien_llama_decide() {
        let base = [ent("mundo", 1)];
        let (a, b) = ([ent("Mundo", 2)], [ent("MUNDO", 3)]);
        let mut v = Vec::new();
        por_carpeta(&base, &a, &b, |p| v.push(p));
        match v.as_slice() {
            [Paso::Distintos { base: Some(x), a: Some(y), b: Some(z) }] => {
                assert_eq!((x.nodo.hash[0], y.nodo.hash[0], z.nodo.hash[0]), (1, 2, 3));
            }
            otro => panic!("{otro:?}"),
        }
        // Quitar de un lado y cambiar del otro: Distintos con un `None`.
        let solo_b = [ent("mundo", 3)];
        let mut v = Vec::new();
        por_carpeta(&base, &[], &solo_b, |p| v.push(p));
        assert!(matches!(v.as_slice(), [Paso::Distintos { a: None, b: Some(_), .. }]));
    }

    #[test]
    fn la_base_es_el_ultimo_estrato_que_las_dos_cadenas_comparten() {
        // Las cadenas van de la punta hacia atras.
        assert_eq!(base(&[9, 7, 5, 3, 1], &[8, 6, 5, 3, 1]), Some(5));
        assert_eq!(base(&[4, 1], &[4, 1]), Some(4));
        assert_eq!(base(&[2], &[3]), None);
    }
}
