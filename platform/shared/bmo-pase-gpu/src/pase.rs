//! **EL PASE** -- las preguntas que se hacen UNA vez, en orden, antes de abrir.
//!
//! [carril]  VERDE     un veredicto sobre hechos que ya junto el kernel
//! [cuesta]  NADA      corre cuando alguien pide abrir el pase
//! [riesgo]  UNICO     es la unica puerta del atajo: una pregunta de menos aqui
//!                     no la hace nadie despues (el radar no mira la IOMMU)
//!
//! # El orden, y por que la pantalla va PRIMERO
//!
//! ```text
//!    1 pantalla     quien pide ES el propietario de la pantalla (`obj::fb`). El
//!                   lienzo se copia ENCIMA de lo que ven todos
//!    2 gsp          el GSP no se despidio (`gpu_apagar::despedido`)
//!    3 iommu        la entrada de la 3060 es TRADUCIDA: sin eso prestar es
//!                   dar TODA la RAM
//!    4 bar1         BAR1 en modo fisico: la pantalla del GOP es alcanzable
//!    5 timbre       hay canal de copia (L1d3) al que tocarle el timbre
//!    6 vblank       E2 armado: sin latido no hay radar, y sin radar no hay pase
//!    7 lienzo       un bloque del que pide, de la medida de la pantalla
//!    8 ocupado      UN pase a la vez: el volcador ARMADO o otro pase
//! ```
//!
//! # *** Por que "vblank" es una pregunta y no un detalle
//!
//! Porque el pase quita la burocracia A CAMBIO del radar. Un pase sin latido
//! seria el atajo sin nadie mirando: exactamente lo que la ruta prohibe.

/// Lo que el kernel sabe cuando llega la peticion. Lo junta el; esto solo juzga.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Hechos {
    pub propietario_pantalla: bool,
    pub gsp_despedido: bool,
    pub iommu_traducida: bool,
    pub bar1_fisica: bool,
    pub timbre_de_copia: bool,
    pub vblank_armado: bool,
    pub lienzo_suyo: bool,
    pub lienzo_mide: bool,
    pub ocupado: bool,
}

/// **Por que no.** Viaja en las banderas de `BmoStatus::negado`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum NoPase {
    NoEsTuPantalla = 1,
    GspDespedido = 2,
    SinIommu = 3,
    SinBar1 = 4,
    SinTimbre = 5,
    SinVblank = 6,
    LienzoAjeno = 7,
    LienzoNoMide = 8,
    Ocupado = 9,
    // ** Los de abajo NO los devuelve `juzgar`: los pone el kernel DESPUES del
    // si, cuando lo que falla es cumplirlo.
    /// El lienzo no se pudo prestar, o la MMU de la 3060 no invalido.
    NoSePresta = 10,
    /// El buzon no se pudo mapear en el proceso.
    NoSeMapea = 11,
}

impl NoPase {
    pub const fn codigo(self) -> u32 {
        self as u32
    }

    pub fn desde_codigo(c: u32) -> Option<NoPase> {
        Some(match c {
            1 => NoPase::NoEsTuPantalla,
            2 => NoPase::GspDespedido,
            3 => NoPase::SinIommu,
            4 => NoPase::SinBar1,
            5 => NoPase::SinTimbre,
            6 => NoPase::SinVblank,
            7 => NoPase::LienzoAjeno,
            8 => NoPase::LienzoNoMide,
            9 => NoPase::Ocupado,
            10 => NoPase::NoSePresta,
            11 => NoPase::NoSeMapea,
            _ => return None,
        })
    }

    pub fn texto(self) -> &'static str {
        match self {
            NoPase::NoEsTuPantalla => "este proceso no es el propietario de la pantalla",
            NoPase::GspDespedido => "el GSP ya se despidio: la 3060 no acepta trabajo",
            NoPase::SinIommu => "la 3060 no esta TRADUCIDA por la IOMMU (`gpu traducir`)",
            NoPase::SinBar1 => "BAR1 no esta en modo fisico (`gpu init`)",
            NoPase::SinTimbre => "no hay canal de copia (L1d3) al que tocarle el timbre",
            NoPase::SinVblank => "el VBLANK no esta armado (`gpu vblank`): sin latido no hay radar",
            NoPase::LienzoAjeno => "el lienzo no es un bloque de quien lo pide",
            NoPase::LienzoNoMide => "el lienzo no mide lo que la pantalla",
            NoPase::Ocupado => "ya hay un pase o un volcador armado",
            NoPase::NoSePresta => "el lienzo no se pudo prestar a la 3060",
            NoPase::NoSeMapea => "el buzon no se pudo mapear en este proceso",
        }
    }
}

/// **El juicio.** Una pregunta detras de otra; la primera que falla, contesta.
pub fn juzgar(h: &Hechos) -> Result<(), NoPase> {
    let preguntas = [
        (h.propietario_pantalla, NoPase::NoEsTuPantalla),
        (!h.gsp_despedido, NoPase::GspDespedido),
        (h.iommu_traducida, NoPase::SinIommu),
        (h.bar1_fisica, NoPase::SinBar1),
        (h.timbre_de_copia, NoPase::SinTimbre),
        (h.vblank_armado, NoPase::SinVblank),
        (h.lienzo_suyo, NoPase::LienzoAjeno),
        (h.lienzo_mide, NoPase::LienzoNoMide),
        (!h.ocupado, NoPase::Ocupado),
    ];
    match preguntas.iter().find(|(si, _)| !si) {
        Some(&(_, no)) => Err(no),
        None => Ok(()),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn todo_bien() -> Hechos {
        Hechos {
            propietario_pantalla: true,
            gsp_despedido: false,
            iommu_traducida: true,
            bar1_fisica: true,
            timbre_de_copia: true,
            vblank_armado: true,
            lienzo_suyo: true,
            lienzo_mide: true,
            ocupado: false,
        }
    }

    #[test]
    fn con_todo_en_orden_se_abre() {
        assert_eq!(juzgar(&todo_bien()), Ok(()));
    }

    #[test]
    fn cada_pregunta_tiene_su_no() {
        let casos: [(fn(&mut Hechos), NoPase); 9] = [
            (|h| h.propietario_pantalla = false, NoPase::NoEsTuPantalla),
            (|h| h.gsp_despedido = true, NoPase::GspDespedido),
            (|h| h.iommu_traducida = false, NoPase::SinIommu),
            (|h| h.bar1_fisica = false, NoPase::SinBar1),
            (|h| h.timbre_de_copia = false, NoPase::SinTimbre),
            (|h| h.vblank_armado = false, NoPase::SinVblank),
            (|h| h.lienzo_suyo = false, NoPase::LienzoAjeno),
            (|h| h.lienzo_mide = false, NoPase::LienzoNoMide),
            (|h| h.ocupado = true, NoPase::Ocupado),
        ];
        for (romper, no) in casos {
            let mut h = todo_bien();
            romper(&mut h);
            assert_eq!(juzgar(&h), Err(no));
        }
    }

    /// *** La pantalla va primero: quien no es su propietario no se entera ni
    /// de si la IOMMU esta puesta.
    #[test]
    fn la_pantalla_contesta_antes_que_el_hardware() {
        assert_eq!(juzgar(&Hechos::default()), Err(NoPase::NoEsTuPantalla));
        let mut h = Hechos::default();
        h.propietario_pantalla = true;
        assert_eq!(juzgar(&h), Err(NoPase::SinIommu));
    }

    #[test]
    fn los_codigos_van_y_vuelven() {
        for c in 1..=11 {
            let n = NoPase::desde_codigo(c).unwrap();
            assert_eq!(n.codigo(), c);
            assert!(!n.texto().is_empty());
        }
        assert_eq!(NoPase::desde_codigo(0), None);
        assert_eq!(NoPase::desde_codigo(12), None);
    }
}
