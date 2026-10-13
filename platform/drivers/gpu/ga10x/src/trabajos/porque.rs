//! **EL PORQUE DE CADA NO de la puerta de las apps** (12-10) -- el kernel
//! decia `motivo 79` (`IOMMU_NO_BLUR_PREPARAR`) por CUARENTA caminos
//! distintos, y la mitad no dejaba nada en `cabina`. El metal de las 12:58
//! (Cyberpunk, 22 lotes de 32 pasaban las puertas de PROTON-X) dio dos NO
//! con ese numero y ninguna forma de saber cual. Esto es el vocabulario: cada
//! camino tiene su nombre y su frase, el kernel apunta el ULTIMO NO y PROTON-X
//! lo lee (`INFO_GPU_PORQUE`) y lo dice con palabras.
//!
//! capa: puro -- numeros y frases; quien apunta es el kernel
//! (`gpu_trabajo::porque`), quien lee es la app.
//!
//! ```text
//!    el paquete  [0..16) el porque   [16..24) el sub (cual paso, cual caso)
//!                [32..64) un detalle (una VA, unos bytes, una instruccion)
//!    0           nada que decir (ningun NO desde el arranque)
//! ```
//!
//! Los numeros NO se reusan: un informe viejo se sigue leyendo igual.

/// Por que la puerta de las apps dijo que no.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum Porque {
    /// La receta no es un bloque de quien la manda (su cabecera).
    RecetaNoEsSuya = 1,
    /// La cabecera de la receta (VRN2) no se sostiene.
    RecetaCabecera = 2,
    /// La receta entera no cabe en UN bloque de quien la manda.
    RecetaEnteraNoEsSuya = 3,
    /// `receta::leer` no la sostiene (medidas, destino, cargas, datos o indices).
    RecetaNoSeSostiene = 4,
    /// Las tablas de las texturas no se escribieron o no se releyeron.
    TexturasTablas = 5,
    /// Una textura no es un bloque de quien manda la receta (`sub` = la ranura).
    TexturaNoEsSuya = 6,
    /// Una textura no se pudo prestar a la 3060 (`sub` = la ranura).
    TexturaNoSePresta = 7,
    /// Los TIC y TSC no quedaron en las piscinas.
    TexturaPiscinas = 8,
    /// Las tablas de la Z no se escribieron o no se releyeron.
    ZTablas = 9,
    /// Las tablas de la sombra del color no se escribieron o no se releyeron.
    SombraTablas = 10,
    /// El destino no es un bloque ESCRIBIBLE de quien lo pide (`sub`: [`DESTINO`]).
    DestinoNoEscribible = 11,
    /// El destino no empieza en pagina, o se sale del physmap.
    DestinoNoAlineado = 12,
    /// Las tablas del destino no se escribieron o no se releyeron.
    DestinoTablas = 13,
    /// La RAM del destino no se pudo prestar a la 3060 (la IOMMU).
    DestinoNoSePresta = 14,
    /// El mapa de la pantalla no quedo puesto.
    PantallaMapa = 15,
    /// El tramo no quedo preparado (`sub`: [`PASOS`], el que fallo).
    Preparar = 16,
    /// El juez dijo BODRIO (`sub` = 1 vertice / 2 pixel, detalle = la instruccion).
    Juez = 17,
    /// El pegamento del kernel no puede (`sub` = 1 vertice / 2 pixel).
    Pegamento = 18,
    /// La receta no tiene forma (lo que pega el kernel no se sostiene).
    Forma = 19,
    /// El taller del kernel esta en uso (otra receta a la vez).
    TallerOcupado = 20,
    /// El GR de la 3060 esta ocupado (otro trabajo, o la entrada no vale).
    GrOcupado = 21,
    /// El escritorio no preparo la 3060 (`gpu verrano`, hasta `lienzo`).
    SinEscritorio = 22,
    /// El canal de GR esta muerto (tomo un Xid).
    CanalMuerto = 23,
    /// ** LI2f (13-10): la receta de COMPUTO (VRC1) no es de quien la manda,
    /// o no se sostiene (`sub`: [`COMPUTO`], el caso; detalle: el indice).
    ComputoReceta = 24,
    /// Un bufer (UAV o SRV) no es de quien manda el computo, o un UAV no es
    /// escribible (`sub` = el bufer; detalle: su VA).
    BuferNoEsSuyo = 25,
    /// Un bufer no se pudo prestar a la 3060, o pasa de su ranura (`sub` = el bufer).
    BuferNoSePresta = 26,
    /// El despacho no quedo preparado en el tramo (el pegado, el QMD, las ordenes).
    ComputoPreparar = 27,
    /// El despacho no volvio pagado en su plazo (lo corto el vigilante).
    ComputoSinPagar = 28,
}

impl Porque {
    /// Todos, para leer un numero y para las pruebas.
    pub const TODOS: [Porque; 28] = [
        Porque::RecetaNoEsSuya,
        Porque::RecetaCabecera,
        Porque::RecetaEnteraNoEsSuya,
        Porque::RecetaNoSeSostiene,
        Porque::TexturasTablas,
        Porque::TexturaNoEsSuya,
        Porque::TexturaNoSePresta,
        Porque::TexturaPiscinas,
        Porque::ZTablas,
        Porque::SombraTablas,
        Porque::DestinoNoEscribible,
        Porque::DestinoNoAlineado,
        Porque::DestinoTablas,
        Porque::DestinoNoSePresta,
        Porque::PantallaMapa,
        Porque::Preparar,
        Porque::Juez,
        Porque::Pegamento,
        Porque::Forma,
        Porque::TallerOcupado,
        Porque::GrOcupado,
        Porque::SinEscritorio,
        Porque::CanalMuerto,
        Porque::ComputoReceta,
        Porque::BuferNoEsSuyo,
        Porque::BuferNoSePresta,
        Porque::ComputoPreparar,
        Porque::ComputoSinPagar,
    ];

    /// El porque de un numero (`None` = un numero que este no conoce).
    pub fn de(n: u16) -> Option<Porque> {
        Porque::TODOS.iter().copied().find(|p| *p as u16 == n)
    }

    /// La frase.
    pub fn texto(self) -> &'static str {
        match self {
            Porque::RecetaNoEsSuya => "la receta no es un bloque de quien la manda",
            Porque::RecetaCabecera => "la cabecera de la receta (VRN2) no se sostiene",
            Porque::RecetaEnteraNoEsSuya => "la receta entera no cabe en UN bloque de quien la manda",
            Porque::RecetaNoSeSostiene => "la receta no se sostiene (medidas, destino, cargas, datos o indices)",
            Porque::TexturasTablas => "las tablas de las texturas no se escribieron o no se releyeron",
            Porque::TexturaNoEsSuya => "una textura no es un bloque de quien manda la receta",
            Porque::TexturaNoSePresta => "una textura no se pudo prestar a la 3060",
            Porque::TexturaPiscinas => "los TIC y TSC no quedaron en las piscinas",
            Porque::ZTablas => "las tablas de la Z no se escribieron o no se releyeron",
            Porque::SombraTablas => "las tablas de la sombra del color no se escribieron o no se releyeron",
            Porque::DestinoNoEscribible => "el destino no es un bloque ESCRIBIBLE de quien lo pide",
            Porque::DestinoNoAlineado => "el destino no empieza en pagina, o se sale del physmap",
            Porque::DestinoTablas => "las tablas del destino no se escribieron o no se releyeron",
            Porque::DestinoNoSePresta => "la RAM del destino no se pudo prestar a la 3060 (la IOMMU)",
            Porque::PantallaMapa => "el mapa de la pantalla no quedo puesto",
            Porque::Preparar => "el tramo de la 3060 no quedo preparado",
            Porque::Juez => "el juez del SASS dijo BODRIO",
            Porque::Pegamento => "el pegamento del kernel no puede con esta receta (cargas, registros o salidas)",
            Porque::Forma => "lo que pega el kernel no se sostiene",
            Porque::TallerOcupado => "el taller del kernel estaba en uso (otra receta a la vez)",
            Porque::GrOcupado => "el GR de la 3060 estaba ocupado (otro trabajo en marcha)",
            Porque::SinEscritorio => "el escritorio no preparo la 3060 (`gpu verrano`, hasta `lienzo`)",
            Porque::CanalMuerto => "el canal de GR de la 3060 esta muerto (tomo un Xid)",
            Porque::ComputoReceta => "la receta de computo (VRC1) no es de quien la manda, o no se sostiene",
            Porque::BuferNoEsSuyo => "un bufer del computo no es de quien lo manda (o un UAV no es escribible)",
            Porque::BuferNoSePresta => "un bufer del computo no se pudo prestar a la 3060 (o pasa de los 16 MiB de su ranura)",
            Porque::ComputoPreparar => "el despacho no quedo preparado en el tramo (el pegado, el QMD o las ordenes)",
            Porque::ComputoSinPagar => "el despacho no volvio pagado en su plazo (lo corto el vigilante)",
        }
    }

    /// La frase del `sub`, si este porque tiene casos con nombre.
    pub fn sub_texto(self, sub: u8) -> Option<&'static str> {
        match self {
            Porque::DestinoNoEscribible => DESTINO.get(sub as usize).copied(),
            Porque::Preparar => PASOS.get(sub as usize).copied(),
            Porque::Juez | Porque::Pegamento => match sub {
                1 => Some("el de vertice"),
                2 => Some("el de pixel"),
                3 => Some("el de computo"),
                4 => Some("el de computo, ya pegado (R0..R6)"),
                _ => None,
            },
            Porque::ComputoReceta => COMPUTO.get(sub as usize).copied(),
            _ => None,
        }
    }
}

/// Por que el destino no es escribible (el `sub` de
/// [`Porque::DestinoNoEscribible`]): lo mira el kernel en las cuentas del
/// proceso, no se adivina.
pub const DESTINO: [&str; 5] = [
    "sin mirar",
    "su VA no cae en ningun bloque del proceso ni en su reserva",
    "empieza en un bloque del proceso y se sale de el (el render target cruza dos bloques)",
    "toca un bloque SELLADO (codigo)",
    "una pagina suya en la RESERVA no esta hecha, o no es escribible (detalle: esa pagina)",
];
pub const DESTINO_EN_NINGUNO: u8 = 1;
pub const DESTINO_SE_SALE: u8 = 2;
pub const DESTINO_SELLADO: u8 = 3;
pub const DESTINO_RESERVA: u8 = 4;

/// Por que la receta de computo no vale (el `sub` de
/// [`Porque::ComputoReceta`]): los de `receta_computo::NoReceta`, y si no es
/// de quien la manda.
pub const COMPUTO: [&str; 9] = [
    "sin mirar",
    "no es un bloque de quien la manda ni paginas hechas de su reserva",
    "la cabecera (VRC1) no se sostiene",
    "una tabla se sale o va sin alinear",
    "pide mas de lo que cabe (instrucciones, precargas, buferes o filas)",
    "una precarga que no se entiende, o que nombra lo que no hay (detalle: cual)",
    "un bufer mal dicho (detalle: cual)",
    "el despacho no se puede (hilos, grupos, compartida o registros)",
    "pasa de lo que cabe en la copia del kernel",
];

/// Los pasos de `tuberia::preparar_paso`, en orden (el `sub` de
/// [`Porque::Preparar`]).
pub const PASOS: [&str; 11] = [
    "las medidas (la entrada del GPFIFO, o los vertices: 0, no multiplo de 3, o mas del tope)",
    "el semaforo del fin",
    "los escalones",
    "la pagina sustituta a cero",
    "el programa de vertice",
    "el programa de pixel",
    "la tabla de las constantes",
    "la libreta",
    "los datos (vertices y cbuffer)",
    "las ordenes",
    "la entrada del GPFIFO",
];

/// El paquete de `INFO_GPU_PORQUE`.
pub const fn empaquetar(p: Porque, sub: u8, detalle: u32) -> u64 {
    p as u64 | (sub as u64) << 16 | (detalle as u64) << 32
}

/// `(el numero, el sub, el detalle)`; el numero puede ser uno que este no conoce.
pub const fn desempaquetar(x: u64) -> (u16, u8, u32) {
    (x as u16, (x >> 16) as u8, (x >> 32) as u32)
}

/// Lo que dice una linea: "el destino no es ... -- empieza en un bloque y se
/// sale (detalle 0x...)". Sin alloc: escribe en `f`.
pub fn decir(x: u64, f: &mut impl core::fmt::Write) -> core::fmt::Result {
    let (n, sub, detalle) = desempaquetar(x);
    match Porque::de(n) {
        None if n == 0 => f.write_str("el kernel no apunto porque"),
        None => write!(f, "porque {n} (este PROTON-X no lo conoce: es mas viejo que el kernel)"),
        Some(p) => {
            f.write_str(p.texto())?;
            if let Some(s) = p.sub_texto(sub) {
                write!(f, " -- {s}")?;
            } else if sub != 0 {
                write!(f, " -- caso {sub}")?;
            }
            if detalle != 0 {
                write!(f, " (detalle {detalle:#x})")?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use std::string::String;

    use super::*;

    #[test]
    fn cada_numero_es_uno_y_vuelve() {
        for (k, p) in Porque::TODOS.iter().enumerate() {
            assert_eq!(*p as u16, k as u16 + 1, "los numeros van seguidos y no se reusan");
            assert_eq!(Porque::de(*p as u16), Some(*p));
            assert!(!p.texto().is_empty());
        }
        assert_eq!(Porque::de(0), None);
        assert_eq!(Porque::de(999), None);
    }

    #[test]
    fn el_paquete_se_lee_igual() {
        let x = empaquetar(Porque::DestinoNoEscribible, DESTINO_SE_SALE, 0x8000_d400);
        assert_eq!(desempaquetar(x), (11, 2, 0x8000_d400));
        let mut s = String::new();
        decir(x, &mut s).unwrap();
        assert_eq!(s, "el destino no es un bloque ESCRIBIBLE de quien lo pide -- empieza en un bloque del proceso y se sale de el (el render target cruza dos bloques) (detalle 0x8000d400)");
        let mut s = String::new();
        decir(empaquetar(Porque::Preparar, 4, 0), &mut s).unwrap();
        assert!(s.ends_with("-- el programa de vertice"), "{s}");
        let mut s = String::new();
        decir(0, &mut s).unwrap();
        assert_eq!(s, "el kernel no apunto porque");
        let mut s = String::new();
        decir(500, &mut s).unwrap();
        assert!(s.starts_with("porque 500"));
    }
}
