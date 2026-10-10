//! **LO QUE VA ENCIMA DE TODO** (10-10): el conmutador de Alt+Tab manda en
//! la pantalla, tambien sobre una app a pantalla completa -- la compuesta por
//! la CPU y la que pone la 3060 --, y al soltar Alt todo vuelve como estaba.
//!
//! El propietario: *"que SIEMPRE dominen en la pantalla asi sea con pantalla
//! completa en GPU y CPU porque se parpadea"*. Parpadeaba por dos caminos:
//!
//! ```text
//!    la CPU    una app a pantalla completa se pegaba SIN mirar lo que tenia
//!              delante: su fotograma nuevo tapaba la tarjeta, y la tarjeta
//!              solo volvia mientras se animaba
//!    la 3060   la imagen (DOOM, `SUP_A_LA_3060`) y la receta que dibuja
//!              directo (PROTON-X, `SUP_LA_3060_DIRECTA`) escriben la
//!              pantalla ENTERA cada fotograma: lo que el escritorio volco
//!              encima duraba lo que tardaba el siguiente
//! ```
//!
//! La regla, aqui y probada; el DIRECTOR solo la aplica:
//!
//! ```text
//!    la caja de encima   NINGUNA app se pega en ella, ni a pantalla
//!                        completa ([`juntar`] con lo que ya tapaba)
//!    la 3060, QUIETA     mientras haya algo encima, no se le manda
//!                        fotograma y la receta directa no tiene la pantalla
//!                        (sus dibujos van a su RAM, como sin ella); la app
//!                        SIGUE corriendo. Se ve su ultimo fotograma
//!    al quitarse         VUELVE: primero se vuelca lo devuelto del lienzo y
//!                        despues la 3060 pone un fotograma, aunque sea el
//!                        mismo: lo que tapaba la tarjeta no se queda viejo
//! ```
//!
//! [!] Que el juego siga MOVIENDOSE detras de la tarjeta pide que la 3060
//! componga la tarjeta encima (Q0b de `docs/plan/EL_FOCO.md`): hasta
//! entonces, quieta antes que parpadeando.

/// Una caja de la pantalla: `(x, y, ancho, alto)`.
pub type Caja = (u32, u32, u32, u32);

/// **La caja que cubre las dos** (o la que haya). Una app no se pega en
/// ninguna: con dos cosas delante -- Ejecutar y el conmutador -- se aparta
/// de la caja que las cubre, y lo que quede entre ellas se repinta al
/// quitarse (`repaint_all`).
pub fn juntar(a: Option<Caja>, b: Option<Caja>) -> Option<Caja> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some((ax, ay, aw, ah)), Some((bx, by, bw, bh))) => {
            let (x0, y0) = (ax.min(bx), ay.min(by));
            let x1 = (ax.saturating_add(aw)).max(bx.saturating_add(bw));
            let y1 = (ay.saturating_add(ah)).max(by.saturating_add(bh));
            Some((x0, y0, x1 - x0, y1 - y0))
        }
    }
}

/// Que hace la 3060 en ESTE fotograma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum La3060 {
    /// Como siempre: un fotograma nuevo, si lo hay.
    Sigue,
    /// Hay algo encima: ni un fotograma, y la pantalla no es de la app.
    Quieta,
    /// Se quito lo de encima: volcar lo devuelto, y un fotograma aunque sea
    /// el mismo.
    Vuelve,
}

/// **El estado de un fotograma al siguiente.** Uno por escritorio.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pausa {
    quieta: bool,
}

impl Pausa {
    pub const fn nueva() -> Self {
        Pausa { quieta: false }
    }

    /// La vuelta de este fotograma: `encima`, si algo del escritorio esta
    /// delante de todo.
    pub fn vuelta(&mut self, encima: bool) -> La3060 {
        let antes = core::mem::replace(&mut self.quieta, encima);
        match (antes, encima) {
            (_, true) => La3060::Quieta,
            (true, false) => La3060::Vuelve,
            (false, false) => La3060::Sigue,
        }
    }

    /// Esta quieta ahora.
    pub fn quieta(&self) -> bool {
        self.quieta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn juntar_cubre_las_dos() {
        assert_eq!(juntar(None, None), None);
        assert_eq!(juntar(Some((1, 2, 3, 4)), None), Some((1, 2, 3, 4)));
        assert_eq!(juntar(None, Some((1, 2, 3, 4))), Some((1, 2, 3, 4)));
        // Separadas: la caja de las dos.
        assert_eq!(juntar(Some((10, 10, 20, 20)), Some((100, 5, 10, 10))), Some((10, 5, 100, 25)));
        // Una dentro de otra: la grande.
        assert_eq!(juntar(Some((0, 0, 1920, 1080)), Some((760, 400, 400, 280))), Some((0, 0, 1920, 1080)));
        // En el borde del u32 no se da la vuelta.
        assert_eq!(juntar(Some((u32::MAX - 1, 0, 5, 1)), Some((0, 0, 1, 1))), Some((0, 0, u32::MAX, 1)));
    }

    #[test]
    fn la_3060_se_queda_quieta_mientras_hay_algo_encima_y_vuelve_una_vez() {
        let mut p = Pausa::nueva();
        assert_eq!(p.vuelta(false), La3060::Sigue);
        // Alt+Tab: quieta todo el rato que se mantiene...
        for _ in 0..3 {
            assert_eq!(p.vuelta(true), La3060::Quieta);
            assert!(p.quieta());
        }
        // ...al soltar, vuelve UNA vez, y despues sigue.
        assert_eq!(p.vuelta(false), La3060::Vuelve);
        assert!(!p.quieta());
        assert_eq!(p.vuelta(false), La3060::Sigue);
        // Abrir y cerrar en dos fotogramas seguidos tambien vuelve.
        assert_eq!(p.vuelta(true), La3060::Quieta);
        assert_eq!(p.vuelta(false), La3060::Vuelve);
    }
}
