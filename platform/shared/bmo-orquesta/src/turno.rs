//! **EL TURNO AL DESPERTAR** (10-10, el cuello de botella de
//! `docs/plan/EL_FOCO.md`): quien se lleva el CPU en el tic en que alguien se
//! pone en pie. Lo pregunta `on_timer` del planificador (Ring 0); aqui se
//! decide y se prueba.
//!
//! [carril]  VERDE     solo contesta si/no; el planificador hace el cambio
//!
//! Hasta hoy una tarea que despertaba solo entraba si tenia MAS rango que la
//! que corria; con el mismo rango esperaba a que a la otra se le acabara el
//! quantum (4 u 8 tics). La app y el escritorio estan en el mismo rango: S0 de
//! `PLAN_VERRANO` midio ~3,5 ms de retraso medio en un reposo de 1 ms, y el
//! fotograma del cubo en 18,7 ms para 16 pedidos.
//!
//! ```text
//!    mas rango, lista              entra (lo de siempre)
//!    el MISMO rango, recien        entra, si la que corre ya gasto un tic
//!    despertada de un reposo       de su quantum: quien durmio cumplio su
//!                                  parte, y no espera el turno entero de otra
//!    el mismo rango, lista sin     espera su turno: si no, dos iguales se
//!    haber dormido                 quitarian el CPU en cada tic
//!    apartada por su compas        no entra
//! ```

/// Una tarea, vista para decidir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quien {
    pub rango: u8,
    /// Lista para correr.
    pub lista: bool,
    /// Su compas la tiene apartada hasta mas tarde.
    pub apartada: bool,
    /// Desperto EN ESTE TIC: un reposo por plazo que vencio, o el latido.
    pub desperto: bool,
}

/// **Se le quita el CPU a la que corre** (de `rango`, con `resto` tics de su
/// `quantum` por gastar) por alguna de `otras`?
pub fn expropiar(rango: u8, resto: u16, quantum: u16, otras: impl IntoIterator<Item = Quien>) -> bool {
    let gasto_un_tic = resto < quantum;
    otras
        .into_iter()
        .any(|q| q.lista && !q.apartada && (q.rango > rango || (q.rango == rango && q.desperto && gasto_un_tic)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTA: Quien = Quien { rango: 0, lista: true, apartada: false, desperto: false };

    #[test]
    fn mas_rango_entra_siempre() {
        assert!(expropiar(0, 8, 8, [Quien { rango: 2, ..LISTA }]));
        assert!(!expropiar(2, 3, 8, [Quien { rango: 1, desperto: true, ..LISTA }]), "menos rango no");
    }

    #[test]
    fn el_mismo_rango_recien_despierto_entra_si_la_otra_ya_gasto_un_tic() {
        let desperto = Quien { desperto: true, ..LISTA };
        assert!(expropiar(0, 7, 8, [desperto]), "la otra ya corrio un tic");
        assert!(!expropiar(0, 8, 8, [desperto]), "la otra acaba de entrar: su primer tic es suyo");
        assert!(!expropiar(0, 3, 8, [LISTA]), "lista sin haber dormido: espera su turno");
    }

    #[test]
    fn la_apartada_y_la_que_no_esta_lista_no_entran() {
        assert!(!expropiar(0, 1, 8, [Quien { apartada: true, desperto: true, rango: 5, ..LISTA }]));
        assert!(!expropiar(0, 1, 8, [Quien { lista: false, desperto: true, rango: 5, ..LISTA }]));
        assert!(!expropiar(0, 1, 8, core::iter::empty()));
    }
}
