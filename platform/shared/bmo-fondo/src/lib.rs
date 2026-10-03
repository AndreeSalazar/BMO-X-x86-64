//! **LA MUSICA DE FONDO DE BMO-X**, compuesta aqui y no traida de fuera.
//!
//! generacion: hijo -- depende de `bmo-amplificador`
//!
//! capa: puro -- enteros sobre muestras; ni E/S, ni asignador, ni `unsafe`
//!
//! # De donde sale
//!
//! El propietario, el 2026-10-03, despues de oir la ONDA de la maqueta: *"me
//! encantan las musicas que pusiste, puedes integrar? como si fuera fondo,
//! integrado, que relaja al usuario hasta que aparece una notificacion para
//! avisar"*.
//!
//! Esas musicas no eran ficheros: eran PARTITURAS. Una semilla, un tempo, una
//! nota raiz, una escala y un timbre, y de ahi el patron de cada compas. Este
//! crate es la misma partitura y la misma sintesis, en Rust y en enteros, asi
//! que lo que suena en el Ryzen es lo que sonaba en la maqueta -- y ocupa
//! diez lineas, no diez megas de MP3.
//!
//! ```text
//!    la PIEZA      semilla, tempo, raiz, escala, timbre       (PIEZAS)
//!    el PATRON     que toca cada uno de los 16 pasos           (Patron)
//!    la MEZCLA     cuanto suena cada parte                     (Mezcla)
//!    el COMPOSITOR la pieza sonando, muestra a muestra         (Compositor)
//!    los AVISOS    los sonidos del sistema                     (avisos)
//! ```
//!
//! # Fondo, no cancion: la mezcla [`Mezcla::FONDO`]
//!
//! La musica de la ONDA es para escucharla; la de fondo, para NO tener que
//! hacerlo. La misma partitura con la bateria muy atras y los acordes
//! delante: un colchon que acompana y no pide atencion. Y por debajo, el
//! orquestador la AGACHA sola cuando suena un aviso o el juego
//! (`bmo_amplificador::agacha`).
//!
//! # Por que en enteros
//!
//! Por lo mismo que el amplificador: el mismo byte en todas las maquinas, sin
//! estado de coma flotante, y probado aqui sin encender el Ryzen. La prueba
//! `el_patron_es_el_de_la_maqueta` compara con lo que da el JavaScript de la
//! maqueta, paso a paso.

#![no_std]
#![forbid(unsafe_code)]

use bmo_amplificador::{MilesimasDb, DB};

mod tablas;

/// Una nota, muestra a muestra.
pub mod sintesis;

/// La partitura sonando.
pub mod compositor;

/// Los sonidos del sistema.
pub mod avisos;

/// Las voces del gato: cencerro, 808, maullidos, ronroneo y bufido (S4h).
pub mod neko;
mod neko_tablas;

pub use avisos::Aviso;
pub use compositor::Compositor;
pub use sintesis::{Timbre, HZ};

/// **La escala** de una pieza: de donde salen las notas del bajo y el arpegio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escala {
    /// La del phonk: la segunda MENOR (medio tono sobre la raiz) es la tension.
    Frigia,
    Menor,
    Mayor,
    Penta,
    Dorica,
}

/// **Una pieza**: todo lo que hace falta para componerla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pieza {
    pub nombre: &'static str,
    pub semilla: u32,
    pub bpm: u32,
    /// La nota raiz, en MIDI (60 = do central).
    pub raiz: i32,
    pub escala: Escala,
    pub timbre: Timbre,
    /// **Su estilo**: de que esta hecha.
    pub estilo: Estilo,
    /// **Su nivel**, para que todas suenen igual de fuertes: cada pieza tiene
    /// su densidad, y sin esto una sonaria el doble que otra. Medido por la
    /// prueba `cada_pieza_suena_igual_de_fuerte`.
    pub nivel: MilesimasDb,
}

/// **De que esta hecha una pieza.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estilo {
    /// Las de la maqueta: bombo, caja, plato, bajo, arpegio y acordes.
    Ambiente,
    /// **NEKO PHONK** (S4h): el 808 saturado, el cencerro en frigia, los
    /// "nya" al final de cada frase, los redobles del plato y TODO lo demas
    /// bombeando con el bombo. Ver `neko.rs`.
    NekoPhonk,
}

/// **Las piezas**: las diez de la maqueta y las tres del NEKO PHONK.
pub const PIEZAS: [Pieza; 13] = [
    Pieza { nombre: "Kernel a medianoche", semilla: 11, bpm: 88, raiz: 57, escala: Escala::Menor, timbre: Timbre::Cuadrada, estilo: Estilo::Ambiente, nivel: 0 },
    Pieza { nombre: "Ring 0", semilla: 23, bpm: 104, raiz: 52, escala: Escala::Dorica, timbre: Timbre::Sierra, estilo: Estilo::Ambiente, nivel: -128 },
    Pieza { nombre: "La ciudad de neon", semilla: 5, bpm: 96, raiz: 60, escala: Escala::Penta, timbre: Timbre::Triangulo, estilo: Estilo::Ambiente, nivel: 154 },
    Pieza { nombre: "Compilando", semilla: 41, bpm: 120, raiz: 55, escala: Escala::Menor, timbre: Timbre::Cuadrada, estilo: Estilo::Ambiente, nivel: -128 },
    Pieza { nombre: "Pasillos de ladrillo", semilla: 66, bpm: 132, raiz: 52, escala: Escala::Menor, timbre: Timbre::Sierra, estilo: Estilo::Ambiente, nivel: 102 },
    Pieza { nombre: "La llave azul", semilla: 71, bpm: 140, raiz: 50, escala: Escala::Dorica, timbre: Timbre::Cuadrada, estilo: Estilo::Ambiente, nivel: 77 },
    Pieza { nombre: "Sierra al atardecer", semilla: 7, bpm: 76, raiz: 62, escala: Escala::Mayor, timbre: Timbre::Seno, estilo: Estilo::Ambiente, nivel: 282 },
    Pieza { nombre: "El ecualizador", semilla: 9, bpm: 84, raiz: 57, escala: Escala::Penta, timbre: Timbre::Triangulo, estilo: Estilo::Ambiente, nivel: 410 },
    Pieza { nombre: "Ocho bits por pixel", semilla: 13, bpm: 150, raiz: 60, escala: Escala::Mayor, timbre: Timbre::Cuadrada, estilo: Estilo::Ambiente, nivel: -205 },
    Pieza { nombre: "El emisor salta", semilla: 17, bpm: 128, raiz: 59, escala: Escala::Penta, timbre: Timbre::Cuadrada, estilo: Estilo::Ambiente, nivel: -102 },
    Pieza { nombre: "Neko drift", semilla: 31, bpm: 144, raiz: 50, escala: Escala::Frigia, timbre: Timbre::Sierra, estilo: Estilo::NekoPhonk, nivel: -2637 },
    Pieza { nombre: "Gato de neon", semilla: 47, bpm: 128, raiz: 53, escala: Escala::Frigia, timbre: Timbre::Sierra, estilo: Estilo::NekoPhonk, nivel: -2278 },
    Pieza { nombre: "Nyan de medianoche", semilla: 58, bpm: 136, raiz: 49, escala: Escala::Menor, timbre: Timbre::Sierra, estilo: Estilo::NekoPhonk, nivel: -2586 },
];

/// **Las que relajan**: las lentas y redondas, en el orden en que suenan de
/// fondo. Las rapidas (DOOM II, 8 bits) son para la ONDA, no para trabajar.
pub const TRANQUILAS: [usize; 4] = [6, 7, 2, 0];

/// Busca una pieza por su nombre, sin mirar mayusculas.
pub fn buscar(nombre: &[u8]) -> Option<usize> {
    PIEZAS.iter().position(|p| p.nombre.as_bytes().eq_ignore_ascii_case(nombre))
}

/// **La mezcla**: cuanto suena cada parte, en 1/256 dB sobre la maqueta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mezcla {
    pub bombo: MilesimasDb,
    pub caja: MilesimasDb,
    pub plato: MilesimasDb,
    pub bajo: MilesimasDb,
    pub arpegio: MilesimasDb,
    pub acordes: MilesimasDb,
    /// Lo que se suma a todo al final.
    pub maestro: MilesimasDb,
}

/// Por debajo de esto una parte no se toca: ni se oiria ni gasta.
const CALLADA: MilesimasDb = -60 * DB;

impl Mezcla {
    /// **La de la ONDA**: tal cual la maqueta.
    pub const CANCION: Mezcla = Mezcla { bombo: 0, caja: 0, plato: 0, bajo: 0, arpegio: 0, acordes: 0, maestro: 0 };

    /// **La de fondo**: la bateria muy atras, los acordes delante, y todo
    /// mas bajo. Acompana y no pide atencion.
    pub const FONDO: Mezcla =
        Mezcla { bombo: -10 * DB, caja: -18 * DB, plato: -14 * DB, bajo: -2 * DB, arpegio: -3 * DB, acordes: 4 * DB, maestro: -DB };

    pub(crate) fn suena(&self, parte: MilesimasDb) -> bool {
        parte > CALLADA
    }
}

#[cfg(test)]
mod pruebas;
