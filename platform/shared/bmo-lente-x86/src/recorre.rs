//! **EL RECORRIDO** -- leer el codigo de un `.bex` como lo correra la CPU:
//! desde la ENTRADA, por los saltos y las llamadas que se conocen. LB7 de
//! `docs/plan/PLAN_LA_LENTE_BINARIA.md`, B1 de `docs/plan/EL_FOCO.md`.
//!
//! [carril]  VERDE     un mapa de bits y una pila que da quien llama
//! [consumo] UNA VEZ   por `.bex` al admitirlo, y lo demostrado se recuerda
//!
//! ** Por que no a ciegas desde el byte 0: x86 es de largo variable, y un dato
//! metido entre el codigo se lee como instrucciones que no existen -- y las
//! de detras, desalineadas. Desde la entrada, cada byte que se lee es uno al
//! que la CPU PUEDE llegar.
//!
//! ** Un salto INDIRECTO (`call rax`, `jmp [tabla]`) no dice adonde va. Pero
//! el `.bex` si: cada puntero a una funcion que el programa guarda es una
//! RELOCALIZACION con destino la region de codigo, y su `addend` es el sitio
//! exacto. Esas son las [`recorre`] `semillas`: no se adivina ninguna, las
//! escribio el enlazador. Lo que ni asi se alcanza se dice:
//! [`Recorrido::bytes`] contra el largo del codigo es esa cuenta. (DOOM, el
//! 14-10: sin semillas, el 44 %; con ellas, el 84 %, y lo que falta son
//! funciones a las que nada llama y punteros tomados con `lea`.)
//!
//! ** Y NO se siembra desde un `lea r, [rip+x]` que caiga en el codigo,
//! aunque en C es justo tomar la direccion de una funcion: TITAN++ guarda
//! DATOS en la region de codigo y los direcciona asi, y la lente los leeria
//! como instrucciones (probado el 14-10: `cubo_gira`, `resolucion`). Una
//! semilla tiene que ser EXACTA o no es semilla.
//!
//! ** Y no necesita memoria propia: quien llama da el mapa de lo ya leido (un
//! bit por byte del codigo) y la pila de lo pendiente, y los bytes por
//! ventanas de 15 -- el kernel los saca de las paginas del proceso, y una
//! instruccion puede caer a caballo de dos.

use crate::{lee, NoSe, Sigue, LARGO_MAX};
use crate::tabla::Ext;

/// **Lo que se encontro al recorrer.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recorrido {
    /// Instrucciones leidas (cada una una vez).
    pub leidas: usize,
    /// Bytes de codigo que cubren.
    pub bytes: usize,
    /// Saltos y llamadas indirectos: lo que hay detras no se sabe.
    pub indirectos: usize,
    /// La pila de pendientes se lleno, o el mapa no cubria el codigo: el
    /// recorrido quedo a medias.
    pub desborda: bool,
    /// **La primera instruccion de una extension que el `.bex` NO declara**
    /// y que SIN ella falla (`#UD`): su offset y la extension. Esto es un NO.
    pub no_declarada: Option<(usize, &'static str)>,
    /// Cuantas hay asi en total (la primera es la de arriba).
    pub no_declaradas: usize,
    /// Las que piden una extension no declarada pero SIN ella no fallan: se
    /// ejecutan como otra (`tzcnt` como `bsf`, `lzcnt` como `bsr`; ver
    /// [`crate::Fila::sin_ella_falla`]). No son un NO: se cuentan.
    pub degradan: usize,
    /// La primera que no se sabe leer: su offset y por que.
    pub no_sabe: Option<(usize, NoSe)>,
    /// El primer salto (o caida) que se sale del codigo: de donde y adonde.
    pub fuera: Option<(usize, u64)>,
}

impl Recorrido {
    /// **Demostrado**: todo lo alcanzable se leyo, es de lo declarado y no se
    /// sale del codigo. Un NO ([`Recorrido::no_declarada`]) no lo es, y lo
    /// demas tampoco -- pero eso no es un NO: es "no se pudo demostrar".
    pub fn demostrado(&self) -> bool {
        !self.desborda && self.no_declarada.is_none() && self.no_sabe.is_none() && self.fuera.is_none()
    }
}

/// **Recorre `largo` bytes de codigo desde `entrada`.**
///
/// `semillas` son los demas sitios donde puede empezar a correr el codigo:
/// los destinos en el codigo de las relocalizaciones (punteros a funcion).
/// Una que caiga fuera del codigo se dice como [`Recorrido::fuera`].
///
/// `ventana(offset, buf)` copia los bytes desde `offset` (hasta 15, o hasta el
/// final del codigo) y dice cuantos. `declara(nombre)` contesta si el `.bex`
/// declara esa extension. `visto` es el mapa (al menos `largo / 8` redondeado
/// arriba) y `pila`, lo pendiente; los dos se pisan.
pub fn recorre(
    largo: usize,
    entrada: usize,
    semillas: impl IntoIterator<Item = u64>,
    mut ventana: impl FnMut(usize, &mut [u8; LARGO_MAX]) -> usize,
    declara: impl Fn(&'static str) -> bool,
    visto: &mut [u8],
    pila: &mut [u32],
) -> Recorrido {
    let mut r = Recorrido::default();
    if visto.len() < largo.div_ceil(8) || largo > u32::MAX as usize {
        r.desborda = true;
        return r;
    }
    for b in visto[..largo.div_ceil(8)].iter_mut() {
        *b = 0;
    }
    let mut cima = 0usize;
    let empuja = |r: &mut Recorrido, de: usize, a: u64, pila: &mut [u32], cima: &mut usize| {
        if a >= largo as u64 {
            r.fuera.get_or_insert((de, a));
        } else if *cima < pila.len() {
            pila[*cima] = a as u32;
            *cima += 1;
        } else {
            r.desborda = true;
        }
    };
    empuja(&mut r, entrada, entrada as u64, pila, &mut cima);
    for s in semillas {
        empuja(&mut r, entrada, s, pila, &mut cima);
    }
    let mut buf = [0u8; LARGO_MAX];
    while cima > 0 {
        cima -= 1;
        let mut en = pila[cima] as usize;
        loop {
            if visto[en / 8] >> (en % 8) & 1 != 0 {
                break;
            }
            visto[en / 8] |= 1 << (en % 8);
            let n = ventana(en, &mut buf).min(LARGO_MAX).min(largo - en);
            let i = match lee(&buf[..n]) {
                Ok(i) => i,
                Err(e) => {
                    r.no_sabe.get_or_insert((en, e));
                    break;
                }
            };
            if !i.fila.pide.is_empty() && !declara(i.fila.pide) {
                if i.fila.sin_ella_falla() {
                    r.no_declarada.get_or_insert((en, i.fila.pide));
                    r.no_declaradas += 1;
                } else {
                    r.degradan += 1;
                }
            }
            r.leidas += 1;
            r.bytes += i.largo;
            let siguiente = match i.sigue(en as u64) {
                Sigue::Siguiente(d) => Some(d),
                Sigue::Condicional { cae, salta } => {
                    empuja(&mut r, en, salta, pila, &mut cima);
                    Some(cae)
                }
                Sigue::Salta(t) => Some(t),
                Sigue::Llama { destino, vuelve } => {
                    empuja(&mut r, en, destino, pila, &mut cima);
                    Some(vuelve)
                }
                Sigue::Vuelve | Sigue::Para => None,
                Sigue::Indirecto => {
                    r.indirectos += 1;
                    // Un `call` indirecto VUELVE: lo de detras se sigue leyendo.
                    // Un `jmp` indirecto no.
                    matches!(i.fila.ext, Ext::Es(2)).then(|| en as u64 + i.largo as u64)
                }
            };
            match siguiente {
                Some(d) if d < largo as u64 => en = d as usize,
                Some(d) => {
                    r.fuera.get_or_insert((en, d));
                    break;
                }
                None => break,
            }
        }
    }
    r
}
