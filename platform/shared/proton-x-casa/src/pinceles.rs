//! **Los pinceles de GDI** (03-10, N4.2): de que color pinta un HBRUSH.
//!
//! [carril]  VERDE     un numero a un color; no toca la maquina
//! [cuesta]  DATO      un pincel mal leido pinta otro color
//! [consumo] NADA      solo cuando el `.exe` crea un pincel o pinta con uno
//!
//! Cyberpunk registra su clase con un pincel de serie (`GetStockObject`) y en
//! su WM_PAINT llama a FillRect con el; la casa solo sabia los de color de
//! sistema (`COLOR_x + 1`), avisaba y no pintaba: la ventana se quedaba con
//! lo que hubiera debajo.
//!
//! ```text
//!    1..=31            COLOR_x + 1: el color de sistema x (lo resuelve user32)
//!    serie + 0..5      blanco, gris claro, gris, gris oscuro, negro, HUECO
//!    serie + 18        DC_BRUSH: blanco (SetDCBrushColor no esta todavia)
//!    propio + i        CreateSolidBrush, hasta que DeleteObject lo suelta
//! ```

use alloc::vec::Vec;
use core::cell::UnsafeCell;

/// Donde empiezan los handles de serie (los de `GetStockObject`).
pub(crate) const SERIE: u64 = 0x5A1E_5000;
/// Donde empiezan los de `CreateSolidBrush`.
const PROPIO: u64 = 0x5A1E_6000;
/// Cuantos pinceles propios a la vez.
const PROPIOS: usize = 1024;

/// Con que pinta un pincel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Relleno {
    /// Un color de sistema (su indice): lo resuelve quien sabe la tabla.
    Sistema(usize),
    /// Un COLORREF (0x00BBGGRR).
    Color(u32),
    /// NULL_BRUSH: no pinta nada.
    Hueco,
}

struct Global(UnsafeCell<Vec<Option<u32>>>);
// SAFETY: la casa corre en un hilo a la vez (ver `Global` en lib.rs).
unsafe impl Sync for Global {}
static PINCELES: Global = Global(UnsafeCell::new(Vec::new()));

fn tabla() -> &'static mut Vec<Option<u32>> {
    // SAFETY: ver `Global`; nadie guarda la referencia.
    unsafe { &mut *PINCELES.0.get() }
}

pub(crate) fn reiniciar() {
    tabla().clear();
}

/// **De que pinta `pincel`**, o `None` si no es un pincel.
pub(crate) fn de(pincel: u64) -> Option<Relleno> {
    match pincel {
        1..=31 => Some(Relleno::Sistema(pincel as usize - 1)),
        p if (SERIE..SERIE + 20).contains(&p) => match p - SERIE {
            0 => Some(Relleno::Color(0xFFFFFF)),
            1 => Some(Relleno::Color(0xC0C0C0)),
            2 => Some(Relleno::Color(0x808080)),
            3 => Some(Relleno::Color(0x404040)),
            4 => Some(Relleno::Color(0x000000)),
            5 => Some(Relleno::Hueco),
            18 => Some(Relleno::Color(0xFFFFFF)),
            // Los demas de serie son plumas, fuentes y paletas: no pintan.
            _ => None,
        },
        p if (PROPIO..PROPIO + PROPIOS as u64).contains(&p) => tabla().get((p - PROPIO) as usize).copied().flatten().map(Relleno::Color),
        _ => None,
    }
}

/// `CreateSolidBrush(color)`: un pincel nuevo (reusa los soltados), o NULL
/// sin sitio.
extern "win64" fn create_solid_brush(color: u32) -> u64 {
    let t = tabla();
    let i = match t.iter().position(Option::is_none) {
        Some(i) => i,
        None if t.len() < PROPIOS => {
            t.push(None);
            t.len() - 1
        }
        None => return 0,
    };
    t[i] = Some(color & 0x00FF_FFFF);
    PROPIO + i as u64
}

/// `DeleteObject(h)`: suelta un pincel propio. Los de serie no se sueltan
/// (Windows dice TRUE y no hace nada); lo que no es de aqui, FALSE.
extern "win64" fn delete_object(h: u64) -> i32 {
    if (SERIE..SERIE + 20).contains(&h) {
        return 1;
    }
    if (PROPIO..PROPIO + PROPIOS as u64).contains(&h) {
        if let Some(c) = tabla().get_mut((h - PROPIO) as usize) {
            if c.take().is_some() {
                return 1;
            }
        }
    }
    0
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "CreateSolidBrush" => crate::dir!(create_solid_brush),
        "DeleteObject" => crate::dir!(delete_object),
        _ => return None,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_de_serie_y_los_de_sistema() {
        assert_eq!(de(SERIE + 4), Some(Relleno::Color(0x000000)), "BLACK_BRUSH");
        assert_eq!(de(SERIE), Some(Relleno::Color(0xFFFFFF)), "WHITE_BRUSH");
        assert_eq!(de(SERIE + 5), Some(Relleno::Hueco), "NULL_BRUSH");
        assert_eq!(de(SERIE + 7), None, "BLACK_PEN no pinta un relleno");
        assert_eq!(de(6), Some(Relleno::Sistema(5)), "COLOR_WINDOW + 1");
        assert_eq!(de(0), None);
        assert_eq!(de(0x1234_5678), None);
    }

    #[test]
    fn los_propios_se_crean_y_se_sueltan() {
        reiniciar();
        let a = create_solid_brush(0x00FF_8000);
        let b = create_solid_brush(0xAA12_3456);
        assert_ne!(a, b);
        assert_eq!(de(a), Some(Relleno::Color(0x00FF_8000)));
        assert_eq!(de(b), Some(Relleno::Color(0x0012_3456)), "el byte alto no es color");
        assert_eq!(delete_object(a), 1);
        assert_eq!(de(a), None, "soltado, ya no pinta");
        assert_eq!(delete_object(a), 0, "dos veces no");
        assert_eq!(create_solid_brush(0x11), a, "su sitio se reusa");
        assert_eq!(delete_object(SERIE + 4), 1, "los de serie: TRUE y nada");
        assert_eq!(delete_object(0x77), 0);
        reiniciar();
    }
}
