//! **LO QUE FALTA, CON EL BINARIO AL LADO** (LX2 de `PLAN_PROTON_X.md`,
//! 15-10).
//!
//! [consumo] NADA      solo cuando un `.exe` no arranca por lo que la casa no
//!                     tiene: una vez, antes de salir
//!
//! > El propietario: *"quizas nos sorprenda que PROTON-X no deja claro aunque
//! > printf ayude [...] el binario PRECISO en 0 y 1 diga algo"*.
//!
//! "faltan N funciones" decia QUE falta. Esto dice ademas DESDE DONDE las
//! llama el juego y QUE HACIA JUSTO ANTES, leido del codigo ya cargado con
//! la lente del kernel y dicho en castellano (`bmo_radiografia::pe`): los
//! argumentos que prepara (rcx, rdx, r8, r9), en que funcion del juego esta
//! la llamada, y si antes llamo a otra cosa de Windows. Es lo que hace
//! falta para escribir la funcion que falta: que espera recibir.

use alloc::string::String;
use alloc::vec;

use bmo_radiografia::pe;

use crate::cargador::Modulo;

/// Sitios que se cuentan por funcion (los demas, solo se cuentan).
const SITIOS: usize = 2;

/// **El texto de lo que falta en `m`**: para cada `(nombre, ranura)`, sus
/// sitios y lo de antes. Vacio si el modulo no se deja leer.
pub(crate) fn contar(m: &Modulo, faltan: &[(String, u32)]) -> String {
    let mut t = String::new();
    let img: &[u8] = m.imagen();
    let Ok(p) = pe::leer_colocada(img) else { return t };
    let Some(c) = p.codigo().copied() else { return t };
    let mut visto = vec![0u8; c.tam_en_imagen() as usize / 8 + 1];
    let mut pila = vec![0u32; 1 << 16];
    if p.lente(img, &mut visto, &mut pila, &mut pe::Extensiones::default()).is_none() {
        return t;
    }
    t.push_str(&alloc::format!("\n-- {}: DESDE DONDE las pide, con el binario al lado --", m.nombre));
    for (nombre, ranura) in faltan {
        t.push_str(&alloc::format!("\n  {nombre}\n"));
        p.cuenta_falta(img, &visto, m.base, *ranura, nombre, SITIOS, &mut t);
    }
    t
}
