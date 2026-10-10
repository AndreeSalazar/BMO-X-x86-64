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

// ===================================================================
//  EL APILADO (10-10): quien esta delante de quien, para TODAS
// ===================================================================
//
// El propietario: *"al estar con una ventana con cualquier app choca, se
// mezclan o uno predomina"*. El compositor pegaba las apps en el orden de su
// hueco -- la que cambiaba despues tapaba a la otra, estuviera delante o no
// -- y siempre ENCIMA de las ventanas del sistema; el clic iba a la primera
// del hueco. Ahora el orden es UNO, el de la lista del foco (la de delante
// primero), y de el salen las tres cosas:
//
//    pegar     cada ventana se pega SIN los trozos que le tapan las de
//              delante ([`tapan`] y [`tramos`])
//    el clic   va a la de delante en ese punto ([`delante`])
//    tapada    una ventana esta tapada si una de DELANTE la pisa

/// Las que caben en un apilado: las ventanas del sistema, las apps y lo de
/// encima de todo.
pub const MAX_CAJAS: usize = 16;

/// Si dos cajas se pisan.
pub fn se_pisan((x, y, w, h): Caja, (a, b, c, d): Caja) -> bool {
    x < a.saturating_add(c) && a < x.saturating_add(w) && y < b.saturating_add(d) && b < y.saturating_add(h)
}

/// Si la caja tiene el punto.
pub fn tiene((x, y, w, h): Caja, px: u32, py: u32) -> bool {
    px >= x && py >= y && px - x < w && py - y < h
}

/// **La de delante en el punto**: `cajas` va de delante hacia atras (`None`:
/// cerrada o minimizada). Su indice.
pub fn delante(cajas: &[Option<Caja>], px: u32, py: u32) -> Option<usize> {
    cajas.iter().position(|c| c.is_some_and(|c| tiene(c, px, py)))
}

/// **Lo que tapa a la `k`**: las cajas de DELANTE de ella que la pisan, en
/// `fuera`. Cuantas.
pub fn tapan(cajas: &[Option<Caja>], k: usize, fuera: &mut [Caja; MAX_CAJAS]) -> usize {
    let Some(mia) = cajas.get(k).copied().flatten() else { return 0 };
    let mut n = 0;
    for c in cajas[..k].iter().flatten() {
        if se_pisan(mia, *c) && n < MAX_CAJAS {
            fuera[n] = *c;
            n += 1;
        }
    }
    n
}

/// **Los tramos que se VEN de una fila**: la fila `y`, de `x0` a `x0 +
/// ancho`, menos lo que le tapan las `tapas`. En `fuera`, como `(desde,
/// hasta)` relativos a `x0`, de izquierda a derecha y sin solaparse. Cuantos.
pub fn tramos(y: u32, x0: u32, ancho: u32, tapas: &[Caja], fuera: &mut [(u32, u32); MAX_CAJAS + 1]) -> usize {
    let fin = x0.saturating_add(ancho);
    let mut n = 0;
    let mut x = x0;
    while x < fin {
        // La tapa que cubre `x` y llega mas lejos: se salta.
        let cubre = tapas.iter().filter(|t| t.1 <= y && y - t.1 < t.3 && t.0 <= x && x - t.0 < t.2).map(|t| t.0.saturating_add(t.2)).max();
        if let Some(hasta) = cubre {
            x = hasta.min(fin);
            continue;
        }
        // Se ve hasta la tapa siguiente de esta fila.
        let corta = tapas.iter().filter(|t| t.1 <= y && y - t.1 < t.3 && t.0 > x && t.0 < fin).map(|t| t.0).min().unwrap_or(fin);
        if n < fuera.len() {
            fuera[n] = (x - x0, corta - x0);
            n += 1;
        }
        x = corta;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_clic_va_a_la_de_delante() {
        // Delante una chica, detras una grande que la contiene.
        let cajas = [Some((100, 100, 50, 50)), None, Some((0, 0, 400, 300))];
        assert_eq!(delante(&cajas, 120, 120), Some(0));
        assert_eq!(delante(&cajas, 10, 10), Some(2));
        assert_eq!(delante(&cajas, 150, 120), Some(2), "el borde derecho no es suyo");
        assert_eq!(delante(&cajas, 500, 10), None);
    }

    #[test]
    fn solo_tapan_las_de_delante_que_la_pisan() {
        let cajas = [Some((0, 0, 10, 10)), Some((100, 100, 50, 50)), Some((90, 90, 300, 300)), Some((500, 500, 5, 5))];
        let mut t = [(0, 0, 0, 0); MAX_CAJAS];
        // A la tercera la tapa la segunda (la pisa); la primera no la toca.
        assert_eq!(tapan(&cajas, 2, &mut t), 1);
        assert_eq!(t[0], (100, 100, 50, 50));
        // A la de delante no la tapa nadie; a la ultima tampoco (no la pisan).
        assert_eq!(tapan(&cajas, 0, &mut t), 0);
        assert_eq!(tapan(&cajas, 3, &mut t), 0);
        // Cerrada: nada.
        assert_eq!(tapan(&[Some((0, 0, 9, 9)), None], 1, &mut t), 0);
    }

    #[test]
    fn los_tramos_que_se_ven_de_una_fila() {
        let mut f = [(0, 0); MAX_CAJAS + 1];
        // Sin tapas: entera.
        assert_eq!(tramos(5, 10, 100, &[], &mut f), 1);
        assert_eq!(f[0], (0, 100));
        // Una en medio: dos tramos.
        let tapa = [(40, 0, 20, 10)];
        assert_eq!(tramos(5, 10, 100, &tapa, &mut f), 2);
        assert_eq!(&f[..2], &[(0, 30), (50, 100)]);
        // Fuera de su alto no tapa.
        assert_eq!(tramos(10, 10, 100, &tapa, &mut f), 1);
        // Dos que se solapan, y una que tapa el principio y otra el final.
        let tapas = [(0, 0, 30, 10), (20, 0, 20, 10), (100, 0, 50, 10)];
        assert_eq!(tramos(0, 10, 100, &tapas, &mut f), 1);
        assert_eq!(f[0], (30, 90));
        // Tapada entera.
        assert_eq!(tramos(0, 10, 100, &[(0, 0, 500, 1)], &mut f), 0);
    }

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
