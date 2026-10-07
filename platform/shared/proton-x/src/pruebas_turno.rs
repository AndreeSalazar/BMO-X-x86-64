//! **El turno PRESTADO** (T1 y T2 de `PLAN_LOS_DOCE_DIRECTORES`, 07-10): lo
//! largo le presta el turno al hilo del sonido, y el turno vuelve a quien lo
//! presto, no a la rueda.

use crate::hilos::{Objeto, Planificador, Turno, WAIT_OBJECT_0};

/// El 0 esta dentro de algo largo (una lista de dibujo); el 1 es el del
/// sonido y espera su evento; el 2 es otro hilo del juego, listo. Cuando el
/// latido enciende el evento, el 1 es URGENTE; y cuando vuelve a esperar,
/// el turno es del 0, no del 2 (que entraria en D3D12 a mitad de la lista).
#[test]
fn el_sonido_corre_a_mitad_de_lo_largo_y_el_turno_vuelve_a_quien_lo_presto() {
    let mut p = Planificador::nuevo();
    let ev = p.nuevo_objeto(Objeto::Evento { manual: false, encendido: false });
    p.del_sonido(ev);
    let (sonido, _) = p.crear(false);
    let (otro, _) = p.crear(false);
    // El del sonido espera su evento (como hace un hilo de WASAPI).
    p.actual = sonido;
    assert_eq!(p.esperar(&[ev], false, None, 0), None);
    p.actual = 0;
    assert_eq!(p.urgente(0), None, "su evento sigue apagado: nadie urgente");
    p.encender(ev, true);
    assert_eq!(p.urgente(0), Some(sonido));
    assert_eq!(p.resultado(sonido), WAIT_OBJECT_0, "su espera se cumplio (y el automatico se gasto)");
    assert_eq!(p.objeto(ev), Some(Objeto::Evento { manual: false, encendido: false }));
    // El 0 presta el turno; el del sonido rellena y vuelve a esperar.
    p.prestado = Some(0);
    p.actual = sonido;
    assert_eq!(p.esperar(&[ev], false, None, 0), None);
    assert_eq!(p.siguiente(0), Turno::Hilo(0), "vuelve a quien lo presto");
    assert_eq!(p.prestado, None, "un prestamo, una vuelta");
    // La prueba que dice NO: sin prestamo, la rueda le daria el turno al otro.
    assert_eq!(p.siguiente(0), Turno::Hilo(otro));
    // Su evento se enciende y una vuelta de la rueda lo deja LISTO sin
    // elegirlo (eligio al otro antes): sigue siendo el del sonido.
    p.actual = 0;
    p.encender(ev, true);
    assert_eq!(p.siguiente(0), Turno::Hilo(sonido));
    p.actual = 0;
    assert_eq!(p.urgente(0), Some(sonido), "listo y del sonido: urgente");
}

/// La prioridad (SetThreadPriority) se guarda y se devuelve, pero NO presta
/// el turno: un hilo HIGHEST del juego puede ser el que manda listas, y
/// entraria en D3D12 a mitad de la de otro.
#[test]
fn la_prioridad_se_guarda_y_no_presta_el_turno() {
    let mut p = Planificador::nuevo();
    let (h1, _) = p.crear(false);
    let (h2, _) = p.crear(false);
    assert!(p.poner_prioridad(h2, 15));
    assert_eq!(p.prioridad(h2), Some(15));
    assert_eq!(p.prioridad(h1), Some(0));
    assert_eq!(p.urgente(0), None, "TIME_CRITICAL y listo: no es del sonido");
    assert!(!p.poner_prioridad(9, 1), "no es un hilo");
    // Un prestamo a quien ya no puede seguir (suspendido) no bloquea la rueda.
    p.actual = h2;
    p.prestado = Some(0);
    assert_eq!(p.suspender(0), Some(0));
    // Rueda desde el 2 (actual): el 0 esta suspendido, asi que el 1.
    assert_eq!(p.siguiente(0), Turno::Hilo(h1));
}
