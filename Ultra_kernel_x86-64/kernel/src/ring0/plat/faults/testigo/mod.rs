//! **LOS TESTIGOS: lo que se le pregunta al resto del kernel con la maquina ya
//! rota, y como se dice una respuesta que no se sabe.**
//!
//! [carril]  VERDE     el reparto de los dos carriles de dentro
//! [consumo] NADA      solo corre cuando algo falla
//!
//! # *** POR QUE ESTO SE PARTIO DE `amarilla.rs` (2026-09-20)
//!
//! Porque el 20-09 la pantalla azul **mintio en dos sitios a la vez** y las dos
//! mentiras eran de la misma forma:
//!
//! ```text
//!    DESMONTANDO ... estacion 17   un valor RANCIO presentado como fresco
//!    en .text del kernel, +0x      una respuesta VACIA presentada como un dato
//! ```
//!
//! Las dos salieron de un fichero donde **preguntar, juzgar y pintar eran la
//! misma linea**: `fault_report` eran 483 lineas seguidas en las que cada hecho
//! se pedia, se creia y se escribia sin que nada separara los tres momentos. En
//! esa forma no hay sitio donde poner la pregunta *"y esto se sabe?"*, porque
//! no hay un momento en el que el dato exista sin estar ya escrito.
//!
//! ** Asi que se corta por donde los nombres estan libres:
//!
//! ```text
//!    verde.rs   EL DATO. Un hecho que sabe si se sabe, y que no se puede
//!               pintar como numero cuando no se sabe. Equivocarse pinta feo
//!    roja.rs    PREGUNTAR CON LA MAQUINA ROTA. Camina la tabla de tareas y el
//!               mapa de marcos sin cerrojos. Colgarse aqui cambia un volcado
//!               legible por una maquina muda
//! ```
//!
//! Y `amarilla.rs` se queda con lo unico que de verdad es suyo: **el ORDEN de
//! la pantalla** -- que renglon sale, con que palabras y detras de cual.
//!
//! # Lo que este corte NO promete
//!
//! No hace honesto al instrumento entero de golpe: los renglones viejos siguen
//! pidiendo su numero a pelo. Lo que hace es que **exista el sitio** donde un
//! hecho puede decir que no se sabe, y que el primero que hacia falta --si esta
//! pila esta entera-- nazca ya sabiendolo decir. Un tipo que nadie usa no
//! educa a nadie; este nace con tres usuarios.

/// EL DATO: un hecho que sabe si se sabe.
pub(super) mod verde;
/// PREGUNTAR CON LA MAQUINA ROTA.
pub(super) mod roja;
