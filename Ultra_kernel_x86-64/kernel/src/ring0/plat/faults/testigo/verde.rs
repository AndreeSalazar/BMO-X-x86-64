//! **CARRIL VERDE** -- EL DATO: un hecho que sabe si se sabe.
//!
//! [carril]  VERDE     el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//! [consumo] NADA      solo corre cuando algo falla
//!
//! [cuesta]  NADA -- no lee memoria de nadie, no pregunta a ningun subsistema y
//!           no decide nada. Es un `enum` de dos casos y como se escribe cada
//!           uno. Equivocarse aqui pinta feo un renglon.
//!
//! [riesgo]  SILENCIO -- un `NoSabido` que se pintara como hueco seria
//!           exactamente el fallo que este fichero existe para cerrar: un
//!           renglon en blanco se lee como *"ese campo es cero"* y no como
//!           *"no se pudo preguntar"*. Por eso el caso malo escribe `??` y su
//!           motivo, y nunca nada.
//!
//! # *** LA FAMILIA ENTERA DE MENTIRAS QUE ESTO CIERRA
//!
//! Cuatro veces en un mes, y las cuatro son el mismo error escrito distinto:
//!
//! ```text
//!    25-08  cinco ceros presentados como un marco de `iretq`
//!    04-09  `estacion 11` en hexadecimal, leido como el once
//!    20-09  `estacion 17` rancia, de un desmontaje que acabo hace rato
//!    20-09  `+0x` vacio, de un `hex(v, 0)` que no escribia un solo digito
//! ```
//!
//! ** Ninguna es un numero MAL CALCULADO. Las cuatro son **un numero presentado
//! sin decir de que clase de respuesta viene**. Un `u64` a secas no puede
//! distinguir *"la respuesta es cero"* de *"no hay respuesta"*, y mientras el
//! tipo no sepa distinguirlas, ningun cuidado del que escribe el renglon lo va
//! a arreglar dos meses seguidos.
//!
//! [!] Y NO ES DARLE UN CEREBRO A LA PANTALLA. No deduce, no infiere y no
//! corrige a nadie: transporta lo que el testigo ya sabia y que hasta hoy se
//! tiraba en el camino. Es el mismo movimiento que `desmontaje::donde()`, que
//! devuelve `Option` porque *"no fue desmontando"* tambien es una respuesta.

use super::super::verde::Line;

/// **Un hecho de la pantalla azul.**
///
/// Los dos casos son igual de validos y se escriben los dos. Lo que no existe
/// es el tercero --callarse-- que es el que costo las cuatro de la cabecera.
pub(in crate::ring0::plat::faults) enum Dato {
    /// Se pregunto y esta es la respuesta.
    Sabido(u64),
    /// Se pregunto y no se puede contestar. El texto dice POR QUE, en corto:
    /// quien lee esto tiene la maquina parada y una foto del movil.
    NoSabido(&'static str),
}

impl Dato {
    /// Escribe el dato detras de lo que ya haya en el renglon.
    ///
    /// `ancho` son los digitos del caso bueno, con el mismo convenio que
    /// [`Line::hex`]: **0 son los que hagan falta**. El caso malo no lo usa --
    /// no hay numero que alinear-- y ahi esta medio valor de esto: un `??` de
    /// dos caracteres no se puede confundir con una direccion corta.
    pub(in crate::ring0::plat::faults) fn pinta(&self, l: &mut Line, ancho: usize) {
        match self {
            Dato::Sabido(v) => {
                l.s("0x");
                l.hex(*v, ancho);
            }
            Dato::NoSabido(motivo) => {
                l.s("?? ");
                l.s(motivo);
            }
        }
    }

    /// `true` si hay numero. Lo usa quien tiene que decidir si un renglon
    /// entero cambia de forma, no quien lo pinta -- pintar ya sabe hacerlo.
    pub(in crate::ring0::plat::faults) fn se_sabe(&self) -> bool {
        matches!(self, Dato::Sabido(_))
    }

    /// **Un ORDINAL, en base diez.** La misma respuesta contada en vez de
    /// direccionada.
    ///
    /// *** SON DOS METODOS A PROPOSITO, y la fecha dice por que. El 04-09 la
    /// pantalla escribio `estacion 11` con `hex` y la once de la tabla es
    /// `console`; era la diecisiete. Nadie mintio y el renglon se leyo mal.
    /// **Una direccion en base 16 esta bien; un ordinal en base 16 es una
    /// trampa**, asi que elegir mal tiene que costar escribir otro nombre y no
    /// cambiar un numero de ancho.
    pub(in crate::ring0::plat::faults) fn cuenta(&self, l: &mut Line) {
        match self {
            Dato::Sabido(v) => l.dec(*v),
            Dato::NoSabido(motivo) => {
                l.s("?? ");
                l.s(motivo);
            }
        }
    }
}
