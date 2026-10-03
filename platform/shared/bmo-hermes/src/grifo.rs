//! **EL GRIFO DEL ZUMBIDO** -- uno cada 10 segundos por amigo.
//!
//! [carril]  AMARILLO  decide si un aviso de otra maquina se ejecuta
//!
//! Un ZUMBIDO hace temblar la ventana, destellar la pantalla y sonar el
//! altavoz. Sin grifo, un amigo con un bucle (o alguien que robo su maquina)
//! convierte el escritorio en una sirena. La regla del plan: *"uno cada 10 s
//! por amigo como mucho: el resto se cuenta, no se ejecuta"*.
//!
//! La hora ENTRA como argumento, como en `bmo-pila`: sin reloj propio, el
//! grifo es determinista y se prueba sin esperar diez segundos.
//!
//! [!] Si la hora va hacia ATRAS (un reloj que se corrige), el grifo no deja
//! pasar: con `saturating_sub` la espera sale cero, y cero es menos que diez
//! segundos. Equivocarse hacia el silencio es lo barato.

/// Cuanto hay que esperar entre dos zumbidos del mismo amigo.
pub const ESPERA_MS: u64 = 10_000;

/// **El grifo de UN amigo.**
#[derive(Clone, Copy, Debug, Default)]
pub struct Grifo {
    ultimo: Option<u64>,
    contados: u32,
}

impl Grifo {
    pub const fn nuevo() -> Self {
        Grifo { ultimo: None, contados: 0 }
    }

    /// **Pasa este zumbido?** `ahora_ms` es la hora de la maquina.
    pub fn admitir(&mut self, ahora_ms: u64) -> bool {
        match self.ultimo {
            Some(t) if ahora_ms.saturating_sub(t) < ESPERA_MS || ahora_ms < t => {
                self.contados = self.contados.saturating_add(1);
                false
            }
            _ => {
                self.ultimo = Some(ahora_ms);
                true
            }
        }
    }

    /// Cuantos zumbidos se contaron sin ejecutarse.
    pub fn contados(&self) -> u32 {
        self.contados
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn uno_cada_diez_segundos() {
        let mut g = Grifo::nuevo();
        assert!(g.admitir(1_000));
        assert!(!g.admitir(1_001));
        assert!(!g.admitir(10_999));
        assert!(g.admitir(11_000));
        assert_eq!(g.contados(), 2);
    }

    #[test]
    fn la_hora_hacia_atras_no_deja_pasar() {
        let mut g = Grifo::nuevo();
        assert!(g.admitir(50_000));
        assert!(!g.admitir(5_000));
        assert!(!g.admitir(0));
        assert!(g.admitir(60_000));
    }

    #[test]
    fn un_bucle_no_es_una_sirena() {
        let mut g = Grifo::nuevo();
        let pasan = (0..100_000u64).filter(|t| g.admitir(t * 10)).count();
        // 1.000.000 ms de bucle: 100 zumbidos, no 100.000.
        assert_eq!(pasan, 100);
        assert_eq!(g.contados(), 99_900);
    }
}
