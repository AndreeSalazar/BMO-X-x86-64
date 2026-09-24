//! **E2 -- EL VBLANK POR INTERRUPCION: que registros, y en que orden.**
//!
//! capa: puro -- decide el ORDEN de las escrituras; quien las hace es el kernel, por el trait [`Registros`] (L8)
//!
//! [eje]     CORRECCION -- la primera vez que BMO-X ESCRIBE en la 3060
//!
//! # Por que existe (2026-09-24)
//!
//! `docs/plan/PLAN_LA_3060.md`, E2: que la tarjeta AVISE cuando acaba de
//! barrer la pantalla, en vez de que el compositor le pregunte la linea. Es la
//! primera escritura en la grafica, y por eso el ORDEN vive aqui, probado en el
//! anfitrion contra un banco de registros de mentira, y no en el kernel.
//!
//! # De donde sale cada numero (nouveau, Linux v6.10, MIT)
//!
//! El GA106 es el `nv176` de `nvkm/engine/device/base.c`: su interrupcion va
//! por el arbol del VFN (`ga100_vfn`, en `0xB80000`), su pantalla es la de
//! Volta (`ga102_disp` usa `gv100_disp_intr` y `tu102_disp_init`), y el MSI se
//! rearma como en Pascal (`gp100_pci`).
//!
//! ```text
//!    el ARBOL (subdev/vfn/tu102.c, ga100.c)
//!      0xB81000 + 4*hoja   ESTADO de la hoja; escribir 1 lo limpia
//!      0xB81200 + 4*hoja   PERMITIR (EN_SET)
//!      0xB81400 + 4*hoja   BLOQUEAR (EN_CLEAR)
//!      0xB81600            la CIMA: bit hoja/2 = esa pareja tiene algo
//!      0xB81608 / 0xB81610 armar / desarmar la cima
//!      la pantalla: hoja 4, bit 26 (0x04000000)
//!    el MSI (subdev/pci/gp100.c)
//!      0x088704 = 0        rearmarlo tras cada aviso
//!    la PANTALLA (engine/disp/gv100.c, tu102.c)
//!      0x611EC0            QUIEN aviso: bits 0..7 = la cabeza n
//!      0x611800 + 4*c      el EVENTO de la cabeza: bit 2 VBLANK, 0..1 otros;
//!                          escribir 1 lo limpia
//!      0x611CC0 + 4*c      la MASCARA del aviso de la cabeza (nouveau: 4)
//!      0x611D80 + 4*c      el ENCENDIDO: bit 2 = avisar del VBLANK
//! ```
//!
//! # Lo que NO hace, a proposito
//!
//! No reclama la pantalla (`0x610078`), no toca canales ni el modo: el GOP la
//! dejo barriendo y asi sigue. Solo enciende UN aviso de UNA cabeza, y lo que no
//! reconoce lo limpia y lo cuenta -- no lo atiende.

use crate::es_error_pri;

/// El trait vive en la raiz del crate desde M0d3 (lo comparte `falcon`).
pub use crate::Registros;

// -- El arbol del VFN ---------------------------------------------------------

/// Donde empieza el VFN en BAR0 (`ga100_vfn_new`).
pub const VFN: u32 = 0x00B8_0000;
/// Cuantas hojas tiene el arbol (`tu102_vfn_intr_pending` recorre 8).
pub const HOJAS: u32 = 8;
/// La hoja y el bit de la PANTALLA (`ga100_vfn_intrs`: DISP, hoja 4).
pub const HOJA_PANTALLA: u32 = 4;
pub const BIT_PANTALLA: u32 = 0x0400_0000;
/// El bit de la cima que cubre esa hoja: uno por cada DOS hojas.
pub const CIMA_PANTALLA: u32 = 1 << (HOJA_PANTALLA / 2);

pub const fn hoja_estado(hoja: u32) -> u32 {
    VFN + 0x1000 + hoja * 4
}
pub const fn hoja_permitir(hoja: u32) -> u32 {
    VFN + 0x1200 + hoja * 4
}
pub const fn hoja_bloquear(hoja: u32) -> u32 {
    VFN + 0x1400 + hoja * 4
}
pub const CIMA_ESTADO: u32 = VFN + 0x1600;
pub const CIMA_ARMAR: u32 = VFN + 0x1608;
pub const CIMA_DESARMAR: u32 = VFN + 0x1610;

/// Rearmar el MSI: `nvkm_pci_wr32(pci, 0x0704, 0)` sobre `0x088000`.
pub const MSI_REARMAR: u32 = 0x0008_8704;

// -- La pantalla --------------------------------------------------------------

/// Que parte de la pantalla aviso: bits 0..7 = la cabeza de ese numero.
pub const PANTALLA_QUIEN: u32 = 0x0061_1EC0;
/// Los eventos de temporizacion de la cabeza.
pub const fn cabeza_evento(c: u32) -> u32 {
    0x0061_1800 + c * 4
}
/// La mascara de su aviso.
pub const fn cabeza_mascara(c: u32) -> u32 {
    0x0061_1CC0 + c * 4
}
/// Que eventos de la cabeza avisan.
pub const fn cabeza_encendido(c: u32) -> u32 {
    0x0061_1D80 + c * 4
}
/// El bit del VBLANK, en el evento, la mascara y el encendido.
pub const VBLANK: u32 = 0x0000_0004;
/// LAST_DATA y LOADV: se limpian y no se cuentan (`gv100_disp_intr_head_timing`).
pub const OTROS_DE_LA_CABEZA: u32 = 0x0000_0003;

/// Por que no se pudo armar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoArma {
    /// La cabeza no existe (fuera de 0..8).
    Cabeza,
    /// Un registro contesto con un error del anillo PRIV: la tarjeta no esta.
    NoContesta,
    /// El encendido se escribio y al releer no estaba: se deshizo.
    NoSeQuedo,
}

/// Lo que habia antes de armar, para devolverlo al desarmar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Armado {
    pub cabeza: u32,
    pub mascara_antes: u32,
}

/// **ARMAR el aviso del VBLANK de `cabeza`.** El kernel lo llama con el MSI
/// ya programado y el Bus Master encendido: primero se dice A DONDE, luego se
/// dice QUE avise, y por ultimo se abre la puerta de arriba.
///
/// ```text
///    1. todas las hojas BLOQUEADAS        como `nvkm_intr_rearm`: nadie
///                                         avisa de algo que no se atiende
///    2. el evento viejo de la cabeza, limpio
///    3. mascara y encendido del VBLANK    y se RELEE el encendido
///    4. la hoja de la pantalla, limpia y PERMITIDA
///    5. el MSI rearmado, y la cima ARMADA
/// ```
pub fn armar(r: &mut impl Registros, cabeza: u32) -> Result<Armado, NoArma> {
    if cabeza >= 8 {
        return Err(NoArma::Cabeza);
    }
    let encendido = r.leer(cabeza_encendido(cabeza));
    let mascara_antes = r.leer(cabeza_mascara(cabeza));
    if es_error_pri(encendido) || es_error_pri(mascara_antes) {
        return Err(NoArma::NoContesta);
    }
    for hoja in 0..HOJAS {
        r.escribir(hoja_bloquear(hoja), 0xFFFF_FFFF);
    }
    let viejo = r.leer(cabeza_evento(cabeza));
    if !es_error_pri(viejo) && viejo & (VBLANK | OTROS_DE_LA_CABEZA) != 0 {
        r.escribir(cabeza_evento(cabeza), viejo & (VBLANK | OTROS_DE_LA_CABEZA));
    }
    r.escribir(cabeza_mascara(cabeza), VBLANK);
    r.escribir(cabeza_encendido(cabeza), encendido | VBLANK);
    let releido = r.leer(cabeza_encendido(cabeza));
    if es_error_pri(releido) || releido & VBLANK == 0 {
        r.escribir(cabeza_encendido(cabeza), encendido & !VBLANK);
        r.escribir(cabeza_mascara(cabeza), mascara_antes);
        return Err(NoArma::NoSeQuedo);
    }
    r.escribir(hoja_estado(HOJA_PANTALLA), BIT_PANTALLA);
    r.escribir(hoja_permitir(HOJA_PANTALLA), BIT_PANTALLA);
    r.escribir(MSI_REARMAR, 0);
    r.escribir(CIMA_ARMAR, CIMA_PANTALLA);
    Ok(Armado { cabeza, mascara_antes })
}

/// **DESARMAR**: el reves, y de arriba abajo -- primero se calla la cima,
/// despues la hoja, y por ultimo la cabeza vuelve a como estaba.
pub fn desarmar(r: &mut impl Registros, a: Armado) {
    r.escribir(CIMA_DESARMAR, CIMA_PANTALLA);
    r.escribir(hoja_bloquear(HOJA_PANTALLA), BIT_PANTALLA);
    let e = r.leer(cabeza_encendido(a.cabeza));
    if !es_error_pri(e) {
        r.escribir(cabeza_encendido(a.cabeza), e & !VBLANK);
    }
    r.escribir(cabeza_mascara(a.cabeza), a.mascara_antes);
}

/// **Callar la cima**, y nada mas: lo que se puede hacer desde la
/// interrupcion si algo va mal. Sin lecturas, sin la cabeza.
pub fn callar(r: &mut impl Registros) {
    r.escribir(CIMA_DESARMAR, CIMA_PANTALLA);
    r.escribir(hoja_bloquear(HOJA_PANTALLA), BIT_PANTALLA);
}

/// Lo que traia un aviso.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Aviso {
    /// El VBLANK de NUESTRA cabeza.
    pub vblank: bool,
    /// VBLANKs de otras cabezas (limpiados, no pedidos).
    pub otras_cabezas: u32,
    /// Bits que no se pidieron: de la pantalla fuera de las cabezas
    /// (`0x611EC0` por encima de 0xFF), o de la cabeza fuera de 0..2. Se
    /// limpian los que se pueden limpiar y se DICEN.
    pub ajeno: u32,
    /// Otros bits de la hoja de la pantalla (estan bloqueados: no deberian).
    pub hoja_ajena: u32,
    /// La tarjeta contesto con un error del anillo: no esta.
    pub no_contesta: bool,
}

/// **ATENDER un aviso.** Es `nvkm_intr` + `gv100_disp_intr` para UN aviso de
/// UNA cabeza: desarmar la cima y rearmar el MSI, preguntar quien, limpiar lo
/// que haya, y contar. **La cima NO se vuelve a armar aqui**: lo decide quien
/// llama ([`rearmar`]), que es quien sabe si esto se ha vuelto una tormenta.
pub fn atender(r: &mut impl Registros, cabeza: u32) -> Aviso {
    let mut a = Aviso::default();
    r.escribir(CIMA_DESARMAR, CIMA_PANTALLA);
    r.escribir(MSI_REARMAR, 0);
    let cima = r.leer(CIMA_ESTADO);
    if es_error_pri(cima) {
        a.no_contesta = true;
        return a;
    }
    if cima & CIMA_PANTALLA == 0 {
        return a;
    }
    let hoja = r.leer(hoja_estado(HOJA_PANTALLA));
    if es_error_pri(hoja) {
        a.no_contesta = true;
        return a;
    }
    a.hoja_ajena = hoja & !BIT_PANTALLA;
    if hoja & BIT_PANTALLA == 0 {
        return a;
    }
    // `nvkm_intr` limpia la hoja ANTES de llamar al manejador: lo que la
    // pantalla levante mientras se atiende vuelve a subirla.
    r.escribir(hoja_estado(HOJA_PANTALLA), BIT_PANTALLA);
    let quien = r.leer(PANTALLA_QUIEN);
    if es_error_pri(quien) {
        a.no_contesta = true;
        return a;
    }
    a.ajeno |= quien & !0xFF;
    for c in 0..8u32 {
        if quien & (1 << c) == 0 {
            continue;
        }
        let ev = r.leer(cabeza_evento(c));
        if es_error_pri(ev) {
            a.no_contesta = true;
            return a;
        }
        if ev & OTROS_DE_LA_CABEZA != 0 {
            r.escribir(cabeza_evento(c), ev & OTROS_DE_LA_CABEZA);
        }
        if ev & VBLANK != 0 {
            r.escribir(cabeza_evento(c), VBLANK);
            if c == cabeza {
                a.vblank = true;
            } else {
                a.otras_cabezas += 1;
            }
        }
        let resto = ev & !(VBLANK | OTROS_DE_LA_CABEZA);
        if resto != 0 {
            r.escribir(cabeza_evento(c), resto);
            a.ajeno |= resto;
        }
    }
    a
}

/// Volver a armar la cima tras [`atender`].
pub fn rearmar(r: &mut impl Registros) {
    r.escribir(CIMA_ARMAR, CIMA_PANTALLA);
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// Un banco de registros de mentira: lo que se lee sale de `val`, y cada
    /// escritura se APUNTA en orden. Los registros "escribir 1 limpia" se
    /// imitan para que la prueba vea lo mismo que veria la tarjeta.
    struct Banco {
        val: Vec<(u32, u32)>,
        escrito: Vec<(u32, u32)>,
        /// Si esta, el encendido no acepta el bit (una tarjeta terca).
        terca: bool,
    }

    impl Banco {
        fn nuevo() -> Self {
            Banco { val: Vec::new(), escrito: Vec::new(), terca: false }
        }
        fn poner(&mut self, reg: u32, v: u32) {
            match self.val.iter_mut().find(|(r, _)| *r == reg) {
                Some(e) => e.1 = v,
                None => self.val.push((reg, v)),
            }
        }
        fn valor(&self, reg: u32) -> u32 {
            self.val.iter().find(|(r, _)| *r == reg).map_or(0, |e| e.1)
        }
        fn escribio(&self, reg: u32) -> Vec<u32> {
            self.escrito.iter().filter(|(r, _)| *r == reg).map(|e| e.1).collect()
        }
        fn orden(&self, reg: u32) -> Option<usize> {
            self.escrito.iter().position(|(r, _)| *r == reg)
        }
        fn es_w1c(reg: u32) -> bool {
            (0x0061_1800..0x0061_1820).contains(&reg) || (hoja_estado(0)..=hoja_estado(7)).contains(&reg)
        }
    }

    impl Registros for Banco {
        fn leer(&mut self, reg: u32) -> u32 {
            self.valor(reg)
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            self.escrito.push((reg, v));
            if Self::es_w1c(reg) {
                let viejo = self.valor(reg);
                self.poner(reg, viejo & !v);
            } else if reg == cabeza_encendido(0) && self.terca {
                // no se queda
            } else {
                self.poner(reg, v);
            }
        }
    }

    #[test]
    fn armar_bloquea_todo_antes_de_permitir_la_pantalla() {
        let mut b = Banco::nuevo();
        let a = armar(&mut b, 0).expect("arma");
        assert_eq!(a.cabeza, 0);
        for h in 0..HOJAS {
            assert_eq!(b.escribio(hoja_bloquear(h)), [0xFFFF_FFFF], "la hoja {h} no se bloqueo");
        }
        let ultimo_bloqueo = b.orden(hoja_bloquear(HOJAS - 1)).unwrap();
        let permitir = b.orden(hoja_permitir(HOJA_PANTALLA)).unwrap();
        let cima = b.orden(CIMA_ARMAR).unwrap();
        let encendido = b.orden(cabeza_encendido(0)).unwrap();
        assert!(ultimo_bloqueo < encendido, "se encendio el aviso antes de bloquear las hojas");
        assert!(encendido < permitir, "se permitio la hoja antes de encender la cabeza");
        assert!(permitir < cima, "se armo la cima antes de permitir la hoja");
        assert_eq!(b.escribio(hoja_permitir(HOJA_PANTALLA)), [BIT_PANTALLA]);
        assert_eq!(b.escribio(CIMA_ARMAR), [CIMA_PANTALLA], "la cima: solo la pareja de la hoja 4");
        assert_eq!(b.escribio(MSI_REARMAR), [0]);
        assert_eq!(b.valor(cabeza_encendido(0)) & VBLANK, VBLANK);
        assert_eq!(b.valor(cabeza_mascara(0)), VBLANK);
    }

    #[test]
    fn armar_respeta_lo_que_ya_habia_en_el_encendido_y_devuelve_la_mascara() {
        let mut b = Banco::nuevo();
        b.poner(cabeza_encendido(1), 0x10);
        b.poner(cabeza_mascara(1), 0x55);
        let a = armar(&mut b, 1).unwrap();
        assert_eq!(b.valor(cabeza_encendido(1)), 0x14);
        desarmar(&mut b, a);
        assert_eq!(b.valor(cabeza_encendido(1)), 0x10, "desarmar quita SOLO el bit del VBLANK");
        assert_eq!(b.valor(cabeza_mascara(1)), 0x55, "la mascara vuelve a como la dejo el GOP");
        assert_eq!(b.escribio(CIMA_DESARMAR), [CIMA_PANTALLA]);
        assert!(b.orden(CIMA_DESARMAR).unwrap() > b.orden(CIMA_ARMAR).unwrap());
    }

    #[test]
    fn un_evento_viejo_se_limpia_al_armar() {
        let mut b = Banco::nuevo();
        b.poner(cabeza_evento(0), VBLANK | 1);
        armar(&mut b, 0).unwrap();
        assert_eq!(b.valor(cabeza_evento(0)), 0);
    }

    #[test]
    fn una_tarjeta_que_no_contesta_no_se_arma() {
        let mut b = Banco::nuevo();
        b.poner(cabeza_encendido(0), 0xBADF_5620);
        assert_eq!(armar(&mut b, 0), Err(NoArma::NoContesta));
        assert!(b.escrito.is_empty(), "sin tarjeta no se escribe NADA");
        assert_eq!(armar(&mut Banco::nuevo(), 8), Err(NoArma::Cabeza));
    }

    #[test]
    fn un_encendido_que_no_se_queda_se_deshace_y_no_abre_la_puerta() {
        let mut b = Banco::nuevo();
        b.terca = true;
        b.poner(cabeza_mascara(0), 0x7);
        assert_eq!(armar(&mut b, 0), Err(NoArma::NoSeQuedo));
        assert_eq!(b.valor(cabeza_mascara(0)), 0x7);
        assert!(b.escribio(hoja_permitir(HOJA_PANTALLA)).is_empty());
        assert!(b.escribio(CIMA_ARMAR).is_empty());
    }

    /// El aviso de verdad: cima -> hoja -> quien -> la cabeza.
    fn con_vblank(b: &mut Banco, cabeza: u32) {
        b.poner(CIMA_ESTADO, CIMA_PANTALLA);
        b.poner(hoja_estado(HOJA_PANTALLA), BIT_PANTALLA);
        b.poner(PANTALLA_QUIEN, 1 << cabeza);
        b.poner(cabeza_evento(cabeza), VBLANK);
    }

    #[test]
    fn atender_cuenta_el_vblank_y_lo_limpia_todo() {
        let mut b = Banco::nuevo();
        con_vblank(&mut b, 0);
        let a = atender(&mut b, 0);
        assert!(a.vblank);
        assert_eq!(a, Aviso { vblank: true, ..Aviso::default() });
        assert_eq!(b.valor(cabeza_evento(0)), 0, "el evento sigue puesto: volveria a avisar");
        assert_eq!(b.valor(hoja_estado(HOJA_PANTALLA)), 0);
        // El orden de nouveau: desarmar y rearmar el MSI ANTES de leer.
        assert!(b.orden(CIMA_DESARMAR).unwrap() < b.orden(hoja_estado(HOJA_PANTALLA)).unwrap());
        assert!(b.orden(MSI_REARMAR).unwrap() < b.orden(hoja_estado(HOJA_PANTALLA)).unwrap());
        assert!(b.escribio(CIMA_ARMAR).is_empty(), "atender no rearma: eso lo decide quien llama");
        rearmar(&mut b);
        assert_eq!(b.escribio(CIMA_ARMAR), [CIMA_PANTALLA]);
    }

    #[test]
    fn otra_cabeza_y_lo_ajeno_se_limpian_y_se_dicen() {
        let mut b = Banco::nuevo();
        con_vblank(&mut b, 1);
        b.poner(PANTALLA_QUIEN, 0x2 | 0x1000);
        b.poner(cabeza_evento(1), VBLANK | 0x2 | 0x100);
        let a = atender(&mut b, 0);
        assert!(!a.vblank, "el VBLANK de la cabeza 1 no es el nuestro");
        assert_eq!(a.otras_cabezas, 1);
        assert_eq!(a.ajeno, 0x1000 | 0x100);
        assert_eq!(b.valor(cabeza_evento(1)), 0);
    }

    #[test]
    fn un_aviso_que_no_es_de_la_pantalla_no_toca_la_pantalla() {
        let mut b = Banco::nuevo();
        b.poner(CIMA_ESTADO, CIMA_PANTALLA);
        b.poner(hoja_estado(HOJA_PANTALLA), 0x0020_0000);
        let a = atender(&mut b, 0);
        assert!(!a.vblank);
        assert_eq!(a.hoja_ajena, 0x0020_0000);
        assert!(b.escribio(PANTALLA_QUIEN).is_empty());
        let mut b = Banco::nuevo();
        let a = atender(&mut b, 0);
        assert_eq!(a, Aviso::default(), "sin nada en la cima no se pregunta mas");
    }

    #[test]
    fn una_tarjeta_que_se_cae_se_dice_y_no_se_sigue() {
        let mut b = Banco::nuevo();
        b.poner(CIMA_ESTADO, 0xFFFF_FFFF);
        assert!(atender(&mut b, 0).no_contesta);
        let mut b = Banco::nuevo();
        con_vblank(&mut b, 0);
        b.poner(PANTALLA_QUIEN, 0xBAD0_0100);
        assert!(atender(&mut b, 0).no_contesta);
    }

    #[test]
    fn los_numeros_son_los_de_nouveau() {
        assert_eq!(hoja_estado(4), 0x00B8_1010);
        assert_eq!(hoja_permitir(4), 0x00B8_1210);
        assert_eq!(hoja_bloquear(4), 0x00B8_1410);
        assert_eq!(CIMA_PANTALLA, 0x4);
        assert_eq!(cabeza_evento(1), 0x0061_1804);
        assert_eq!(cabeza_mascara(0), 0x0061_1CC0);
        assert_eq!(cabeza_encendido(2), 0x0061_1D88);
    }
}
