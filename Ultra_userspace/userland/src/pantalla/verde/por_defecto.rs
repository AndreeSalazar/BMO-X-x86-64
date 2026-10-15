//! **EL VOLCADO POR LA 3060, POR DEFECTO** (C3 de `docs/plan/EL_FOCO.md`,
//! 15-10). Hasta hoy la 3060 llevaba el lienzo a la pantalla en cada
//! fotograma (649 us la entera, visto en el metal el 25-09) solo si lo
//! armaba `save mode` o `gpu volcado`; sin eso, la CPU copiaba con `rep
//! movsb` -- los 27,6 ms de la pantalla entera --. Esto decide CUANDO se arma
//! solo: es aritmetica, y por eso vive aqui con sus pruebas, no en el
//! escritorio (que no corre un test).
//!
//! [carril]  VERDE     un estado de cuatro numeros; no toca la pantalla
//! [consumo] NADA      una mirada por fotograma del escritorio
//!
//! ```text
//!    la 3060 lista (GSP_INIT_DONE) y hay lienzo
//!      y no la apago el propietario (`gpu volcado off`)
//!        la primera vez     PROBAR: una copia entera VERIFICADA (1024
//!                           muestras) y despues armar
//!        despues            ARMAR otra vez (volvio la pantalla de un juego,
//!                           o se cayo una tanda)
//!    cada intento que falla, o cada caida sola, cuenta; a las CAIDAS_MAX la
//!    3060 se queda fuera hasta que el propietario lo pida (`gpu volcado`):
//!    lo de la ley L32 -- la CPU, con los mismos bits, y se dice --
//! ```
//!
//! ** Entre un fallo y el siguiente intento se esperan [`PorDefecto::ESPERA`]
//! miradas: un intento que falla no se repite en cada fotograma.

/// Lo que se ve al mirar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mirada {
    /// El GSP-RM dijo `GSP_INIT_DONE`.
    pub lista: bool,
    /// El escritorio pinta en un lienzo (doble bufer): hay que volcar.
    pub lienzo: bool,
    /// Ahora mismo vuelca la 3060.
    pub por_gpu: bool,
    /// Las veces que el volcado se quito solo ([`crate::Pantalla::caidas_gpu`]).
    pub caidas: u32,
}

/// Lo que toca hacer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paso {
    Nada,
    /// Una copia verificada, y si sale, armar.
    Probar,
    /// Armar (ya se verifico antes).
    Armar,
    /// Se acabo: la CPU vuelca hasta que el propietario lo pida. Sale UNA vez.
    Rendirse,
}

/// **La decision**, de fotograma en fotograma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PorDefecto {
    apagado: bool,
    rendido: bool,
    probado: bool,
    fallos: u32,
    caidas_vistas: u32,
    espera: u32,
}

impl PorDefecto {
    /// A los cuantos fallos (intentos o caidas) se deja.
    pub const CAIDAS_MAX: u32 = 3;
    /// Miradas entre un fallo y el siguiente intento (~10 s a 60 por segundo).
    pub const ESPERA: u32 = 600;

    pub const fn nuevo() -> Self {
        PorDefecto { apagado: false, rendido: false, probado: false, fallos: 0, caidas_vistas: 0, espera: 0 }
    }

    /// `gpu volcado off`: el propietario lo quiere en la CPU. Se respeta.
    pub fn apagar(&mut self) {
        self.apagado = true;
    }

    /// `gpu volcado`: el propietario lo pide otra vez; los fallos se olvidan.
    pub fn encender(&mut self) {
        *self = PorDefecto { probado: self.probado, caidas_vistas: self.caidas_vistas, ..PorDefecto::nuevo() };
    }

    /// Si la 3060 se quedo fuera (para decirlo en la fila).
    pub fn rendido(&self) -> bool {
        self.rendido
    }

    /// **Mirar**: que toca en este fotograma.
    pub fn mirar(&mut self, m: Mirada) -> Paso {
        // Una caida sola cuenta como un fallo (aunque nadie mire en ese momento).
        if m.caidas > self.caidas_vistas {
            self.fallos += m.caidas - self.caidas_vistas;
            self.caidas_vistas = m.caidas;
            self.espera = Self::ESPERA;
            if self.fallos >= Self::CAIDAS_MAX && !self.rendido && !self.apagado {
                self.rendido = true;
                return Paso::Rendirse;
            }
            return Paso::Nada;
        }
        if self.apagado || self.rendido || !m.lista || !m.lienzo || m.por_gpu {
            return Paso::Nada;
        }
        if self.espera > 0 {
            self.espera -= 1;
            return Paso::Nada;
        }
        if self.probado { Paso::Armar } else { Paso::Probar }
    }

    /// **Lo que salio** de un [`Paso::Probar`] o [`Paso::Armar`]. `true` si
    /// con este fallo se rinde (decirlo: UNA vez).
    pub fn salio(&mut self, bien: bool) -> bool {
        if bien {
            self.probado = true;
            return false;
        }
        self.fallos += 1;
        self.espera = Self::ESPERA;
        if self.fallos >= Self::CAIDAS_MAX && !self.rendido {
            self.rendido = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const LISTA: Mirada = Mirada { lista: true, lienzo: true, por_gpu: false, caidas: 0 };

    fn esperar(d: &mut PorDefecto, m: Mirada) {
        for _ in 0..PorDefecto::ESPERA {
            assert_eq!(d.mirar(m), Paso::Nada, "esperando despues de un fallo");
        }
    }

    /// Sin la 3060 lista, o sin lienzo, nada: la CPU vuelca como siempre.
    #[test]
    fn without_the_3060_nothing_changes() {
        let mut d = PorDefecto::nuevo();
        assert_eq!(d.mirar(Mirada { lista: false, ..LISTA }), Paso::Nada);
        assert_eq!(d.mirar(Mirada { lienzo: false, ..LISTA }), Paso::Nada);
    }

    /// La primera vez se PRUEBA (la copia verificada); despues, si vuelve la
    /// pantalla de un juego (se solto a proposito: no es una caida), se ARMA.
    #[test]
    fn first_verified_then_rearmed_when_the_screen_comes_back() {
        let mut d = PorDefecto::nuevo();
        assert_eq!(d.mirar(LISTA), Paso::Probar);
        assert!(!d.salio(true));
        assert_eq!(d.mirar(Mirada { por_gpu: true, ..LISTA }), Paso::Nada, "ya vuelca la 3060");
        // Se presto la pantalla a un juego y volvio: `por_gpu` en false, sin caida.
        assert_eq!(d.mirar(LISTA), Paso::Armar, "se vuelve a armar, sin otra copia de prueba");
    }

    /// El propietario manda: con `gpu volcado off` no se arma solo; con `gpu
    /// volcado` otra vez si.
    #[test]
    fn the_owner_off_is_respected() {
        let mut d = PorDefecto::nuevo();
        d.apagar();
        for _ in 0..10 {
            assert_eq!(d.mirar(LISTA), Paso::Nada);
        }
        d.encender();
        assert_eq!(d.mirar(LISTA), Paso::Probar);
    }

    /// Las caidas solas y los intentos que fallan cuentan juntos; entre uno y
    /// otro se espera; a las tres, se rinde UNA vez y se queda en la CPU.
    #[test]
    fn three_failures_and_it_stays_on_the_cpu_saying_so_once() {
        let mut d = PorDefecto::nuevo();
        assert_eq!(d.mirar(LISTA), Paso::Probar);
        assert!(!d.salio(false), "un fallo");
        esperar(&mut d, LISTA);
        assert_eq!(d.mirar(LISTA), Paso::Probar, "aun no se probo con exito");
        assert!(!d.salio(true));
        // Armado, y se cae solo (una tanda que fallo): segundo fallo.
        assert_eq!(d.mirar(Mirada { caidas: 1, ..LISTA }), Paso::Nada);
        esperar(&mut d, Mirada { caidas: 1, ..LISTA });
        assert_eq!(d.mirar(Mirada { caidas: 1, ..LISTA }), Paso::Armar);
        // Y otra caida: el tercero.
        assert_eq!(d.mirar(Mirada { caidas: 2, ..LISTA }), Paso::Rendirse);
        assert!(d.rendido());
        for _ in 0..2 * PorDefecto::ESPERA {
            assert_eq!(d.mirar(Mirada { caidas: 2, ..LISTA }), Paso::Nada, "rendido: la CPU, sin repetirlo");
        }
        d.encender();
        assert_eq!(d.mirar(Mirada { caidas: 2, ..LISTA }), Paso::Armar, "el propietario lo pide: otra vez");
    }
}
