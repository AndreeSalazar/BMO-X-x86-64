//! **EL RADAR DEL VBLANK** -- mira el pase desde fuera, una vez por barrido, y
//! decide si sigue.
//!
//! [carril]  VERDE     solo cuenta y compara: no toca memoria ni aparato
//! [cuesta]  NADA      unas restas por barrido, y solo con un pase abierto
//! [riesgo]  RELOJ     ve DESPUES: un fotograma mal copiado ya se vio un barrido
//!
//! # RADAR, no rayos X (`docs/identidad/LA_RUTA.md`)
//!
//! No esta en el camino del fotograma -- estar ahi seria la burocracia que el
//! pase acaba de quitar. En cada VBLANK mira lo que ya paso: el buzon, la valla
//! de la GPU (su semaforo, no lo que diga el proceso) y, uno de cada
//! [`MIRAR_CADA`] barridos, unas muestras de la pantalla contra el lienzo.
//!
//! ```text
//!    el buzon miente (numero, cajas)     el proceso esta roto o ataca    AL MOMENTO
//!    la GPU se apago en orden            ya no acepta trabajo            AL MOMENTO
//!    el proceso murio / solto la pantalla lo pedido, cumplido            AL MOMENTO
//!    el semaforo dice un numero raro     la GPU escribio basura          AL MOMENTO
//!    la GPU no paga una tanda            colgada: 6 barridos (~100 ms)   al 6
//!    las muestras no cuadran             3 miradas SEGUIDAS              a la 3
//! ```
//!
//! # *** Por que las muestras malas no revocan a la primera
//!
//! Porque el lienzo es del proceso y el protocolo le pide no pintar hasta
//! PAGADO -- si lo hace, la muestra sale mala sin que la GPU tenga la culpa.
//! Una vez es una carrera; tres miradas seguidas (~0,8 s) es una GPU que
//! pinta mal o un escritorio que no respeta la valla, y en los dos casos el
//! atajo se acaba. La comprobacion no se quita (regla 8 de OPTIMIZACION): se
//! saca del camino del fotograma y se pone aqui.
//!
//! [!] Lo que el radar NO para: la GPU leyendo lo que se le presto. Eso lo
//! acota la IOMMU (solo lectura, solo el lienzo); al revocar se devuelve.

use crate::buzon::Mal;

/// Barridos seguidos con una tanda enviada y sin pagar: colgada.
pub const LATIDOS_COLGADA: u32 = 6;
/// Uno de cada tantos barridos se miran muestras (~4 por segundo a 60 Hz).
pub const MIRAR_CADA: u64 = 16;
/// Miradas SEGUIDAS con muestras malas para revocar.
pub const MIRADAS_MALAS: u32 = 3;

/// **Por que se cerro un pase.** Viaja en `buzon::campo::ESTADO`. El cero no
/// existe: cero es "abierto".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Motivo {
    CerradoPorElPropietario = 1,
    PropietarioMurio = 2,
    PantallaSoltada = 3,
    AparatoApagado = 4,
    NumeroImposible = 5,
    CajasImposibles = 6,
    CajaFuera = 7,
    BuzonRoto = 8,
    Colgada = 9,
    PintaMal = 10,
    SemaforoRaro = 11,
}

impl Motivo {
    pub const fn codigo(self) -> u32 {
        self as u32
    }

    pub fn desde_codigo(c: u32) -> Option<Motivo> {
        Some(match c {
            1 => Motivo::CerradoPorElPropietario,
            2 => Motivo::PropietarioMurio,
            3 => Motivo::PantallaSoltada,
            4 => Motivo::AparatoApagado,
            5 => Motivo::NumeroImposible,
            6 => Motivo::CajasImposibles,
            7 => Motivo::CajaFuera,
            8 => Motivo::BuzonRoto,
            9 => Motivo::Colgada,
            10 => Motivo::PintaMal,
            11 => Motivo::SemaforoRaro,
            _ => return None,
        })
    }

    pub fn texto(self) -> &'static str {
        match self {
            Motivo::CerradoPorElPropietario => "lo cerro quien lo abrio",
            Motivo::PropietarioMurio => "el proceso murio",
            Motivo::PantallaSoltada => "el proceso solto la pantalla",
            Motivo::AparatoApagado => "la GPU se apago en orden",
            Motivo::NumeroImposible => "el escritorio cerro una tanda sin esperar a la anterior",
            Motivo::CajasImposibles => "el buzon declara cero cajas o mas de 64",
            Motivo::CajaFuera => "una caja vacia o fuera de la pantalla",
            Motivo::BuzonRoto => "el buzon del kernel no tiene su forma",
            Motivo::Colgada => "la GPU no pago una tanda en 6 barridos (~100 ms)",
            Motivo::PintaMal => "3 miradas seguidas con muestras que no cuadran",
            Motivo::SemaforoRaro => "el semaforo de la GPU dice un numero que no se envio",
        }
    }

    /// Lo decidio el propietario o el sistema, y no un fallo: no se grita en CABINA.
    pub fn es_normal(self) -> bool {
        matches!(self, Motivo::CerradoPorElPropietario | Motivo::PropietarioMurio | Motivo::PantallaSoltada | Motivo::AparatoApagado)
    }
}

impl From<Mal> for Motivo {
    fn from(m: Mal) -> Motivo {
        match m {
            Mal::BuzonCorto => Motivo::BuzonRoto,
            Mal::NumeroImposible => Motivo::NumeroImposible,
            Mal::CajasImposibles => Motivo::CajasImposibles,
            Mal::CajaFuera => Motivo::CajaFuera,
        }
    }
}

/// **Lo que paso en un barrido**, contado por el kernel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Vuelta {
    /// Lo que devolvio el buzon este barrido, si mintio.
    pub mal: Option<Mal>,
    /// La ultima tanda con el timbre tocado (el numero del KERNEL).
    pub enviado: u32,
    /// Lo que dice el semaforo de la GPU.
    pub pagado: u32,
    /// La anterior a `enviado` (lo unico, ademas de `enviado`, que puede decir
    /// un semaforo sano mientras la ultima esta en vuelo).
    pub anterior: u32,
    pub propietario_vivo: bool,
    pub propietario_pantalla: bool,
    pub aparato_vivo: bool,
    /// Solo en los barridos que tocaba mirar: cuantas muestras NO cuadraron.
    pub malas: Option<u32>,
}

/// El estado del radar de UN pase. Nace con el pase y muere con el.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Radar {
    sin_pagar: u32,
    miradas_malas: u32,
    pub latidos: u64,
}

impl Radar {
    pub const fn nuevo() -> Self {
        Self { sin_pagar: 0, miradas_malas: 0, latidos: 0 }
    }

    /// Si en ESTE barrido (el siguiente a mirar) tocan muestras.
    pub const fn toca_mirar(&self) -> bool {
        self.latidos % MIRAR_CADA == MIRAR_CADA - 1
    }

    /// **Mira un barrido.** `None` = sigue abierto. De lo mas grave a lo mas
    /// normal: si mintio Y murio, el motivo es que mintio.
    pub fn mirar(&mut self, v: &Vuelta) -> Option<Motivo> {
        self.latidos += 1;
        if let Some(m) = v.mal {
            return Some(m.into());
        }
        if v.pagado != v.enviado && v.pagado != v.anterior {
            return Some(Motivo::SemaforoRaro);
        }
        if !v.aparato_vivo {
            return Some(Motivo::AparatoApagado);
        }
        if !v.propietario_vivo {
            return Some(Motivo::PropietarioMurio);
        }
        if !v.propietario_pantalla {
            return Some(Motivo::PantallaSoltada);
        }
        if v.pagado != v.enviado {
            self.sin_pagar += 1;
            if self.sin_pagar >= LATIDOS_COLGADA {
                return Some(Motivo::Colgada);
            }
        } else {
            self.sin_pagar = 0;
        }
        match v.malas {
            Some(0) => self.miradas_malas = 0,
            Some(_) => {
                self.miradas_malas += 1;
                if self.miradas_malas >= MIRADAS_MALAS {
                    return Some(Motivo::PintaMal);
                }
            }
            None => {}
        }
        None
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tranquila(n: u32) -> Vuelta {
        Vuelta {
            enviado: n,
            pagado: n,
            anterior: n.saturating_sub(1),
            propietario_vivo: true,
            propietario_pantalla: true,
            aparato_vivo: true,
            ..Vuelta::default()
        }
    }

    #[test]
    fn una_hora_tranquila_sigue() {
        let mut r = Radar::nuevo();
        for t in 0..60 * 3600 {
            let mut v = tranquila(t);
            if r.toca_mirar() {
                v.malas = Some(0);
            }
            assert_eq!(r.mirar(&v), None);
        }
    }

    #[test]
    fn se_mira_uno_de_cada_dieciseis() {
        let mut r = Radar::nuevo();
        let mut miradas = 0;
        for _ in 0..MIRAR_CADA * 10 {
            miradas += r.toca_mirar() as u32;
            r.mirar(&tranquila(1));
        }
        assert_eq!(miradas, 10);
    }

    #[test]
    fn el_buzon_que_miente_revoca_con_su_nombre() {
        for (mal, motivo) in [
            (Mal::BuzonCorto, Motivo::BuzonRoto),
            (Mal::NumeroImposible, Motivo::NumeroImposible),
            (Mal::CajasImposibles, Motivo::CajasImposibles),
            (Mal::CajaFuera, Motivo::CajaFuera),
        ] {
            let mut v = tranquila(3);
            v.mal = Some(mal);
            v.propietario_vivo = false;
            assert_eq!(Radar::nuevo().mirar(&v), Some(motivo), "mentir gana a morir");
        }
    }

    /// En vuelo, el semaforo dice la anterior: sano. Otra cosa: basura.
    #[test]
    fn el_semaforo_solo_puede_decir_dos_numeros() {
        let mut v = tranquila(10);
        v.pagado = 9;
        assert_eq!(Radar::nuevo().mirar(&v), None);
        v.pagado = 11;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::SemaforoRaro));
        v.pagado = 0xDEAD;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::SemaforoRaro));
    }

    #[test]
    fn colgada_al_sexto_barrido_sin_pagar() {
        let mut r = Radar::nuevo();
        let mut v = tranquila(10);
        v.pagado = 9;
        for _ in 0..LATIDOS_COLGADA - 1 {
            assert_eq!(r.mirar(&v), None);
        }
        assert_eq!(r.mirar(&v), Some(Motivo::Colgada));
    }

    #[test]
    fn un_pago_a_tiempo_pone_la_cuenta_a_cero() {
        let mut r = Radar::nuevo();
        let mut lenta = tranquila(10);
        lenta.pagado = 9;
        for _ in 0..20 {
            for _ in 0..LATIDOS_COLGADA - 1 {
                assert_eq!(r.mirar(&lenta), None);
            }
            assert_eq!(r.mirar(&tranquila(10)), None);
        }
    }

    /// *** Una carrera del escritorio se perdona; tres miradas seguidas, no.
    #[test]
    fn las_muestras_malas_revocan_a_la_tercera_seguida() {
        let mut r = Radar::nuevo();
        let mut mala = tranquila(1);
        mala.malas = Some(4);
        let mut buena = tranquila(1);
        buena.malas = Some(0);
        for _ in 0..10 {
            assert_eq!(r.mirar(&mala), None);
            assert_eq!(r.mirar(&mala), None);
            assert_eq!(r.mirar(&tranquila(1)), None, "sin mirar no cuenta ni borra");
            assert_eq!(r.mirar(&buena), None, "una buena pone la cuenta a cero");
        }
        r.mirar(&mala);
        r.mirar(&mala);
        assert_eq!(r.mirar(&mala), Some(Motivo::PintaMal));
    }

    #[test]
    fn lo_normal_y_lo_que_no() {
        let mut v = tranquila(1);
        v.aparato_vivo = false;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::AparatoApagado));
        let mut v = tranquila(1);
        v.propietario_pantalla = false;
        assert_eq!(Radar::nuevo().mirar(&v), Some(Motivo::PantallaSoltada));
        assert!(Motivo::PantallaSoltada.es_normal());
        assert!(!Motivo::Colgada.es_normal());
        assert!(!Motivo::PintaMal.es_normal());
    }

    #[test]
    fn los_codigos_van_y_vuelven() {
        for c in 1..=11 {
            let m = Motivo::desde_codigo(c).unwrap();
            assert_eq!(m.codigo(), c);
            assert!(!m.texto().is_empty());
        }
        assert_eq!(Motivo::desde_codigo(0), None, "cero es abierto, no un motivo");
        assert_eq!(Motivo::desde_codigo(12), None);
    }
}
