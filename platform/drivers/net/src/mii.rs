//! **EL PHY POR DENTRO: por que el enlace va a 10 Mbit, y como volver a 1000**
//! (2026-10-03, `PLAN_LA_RED_SOLA.md` RS0 y RS5b).
//!
//! El propietario: *"mi Internet es de 100, el router no es el cuello de
//! botella: algo lo limita"*. El metal lo dijo desde el 24-08: `PHYstatus`
//! 0x87 = enlace arriba, **10 Mbit**, full duplex; el 12-08 leia 0x0B (100).
//! Y BMO-X **nunca habia escrito en el PHY**: se quedaba con la velocidad que
//! encontraba. La sospecha, que este modulo deja comprobar con numeros:
//!
//! ```text
//!    Windows apaga con el driver de Realtek en "WOL & Shutdown Link Speed =
//!    10 Mbps First" (lo de fabrica): para despertar por la red gastando poco,
//!    deja el PHY ANUNCIANDO SOLO 10 Mbit. Windows, al volver, renegocia; el
//!    firmware de la placa no; BMO-X tampoco -- y se queda en 10.
//! ```
//!
//! Aqui, puro y con banco: los registros MII (IEEE 802.3, clausula 22), como
//! se le PIDEN a la tarjeta (dos caminos segun la generacion del chip), el
//! DIAGNOSTICO de lo que se lee, y lo que hay que escribir para anunciar
//! 10/100/1000 y renegociar -- lo que hace el driver de cualquier sistema al
//! arrancar.

/// Los registros MII de la clausula 22.
pub mod reg {
    /// Control: autonegociacion, reinicio, apagado.
    pub const BMCR: u8 = 0;
    /// Estado: enlace, autonegociacion acabada.
    pub const BMSR: u8 = 1;
    /// El fabricante del PHY (Realtek: 0x001C).
    pub const PHYID1: u8 = 2;
    pub const PHYID2: u8 = 3;
    /// Lo que ANUNCIAMOS (10/100 y pausa).
    pub const ANAR: u8 = 4;
    /// Lo que anuncia EL OTRO (el router o el switch).
    pub const ANLPAR: u8 = 5;
    /// Lo que anunciamos de gigabit.
    pub const GBCR: u8 = 9;
    /// Lo que el otro anuncia de gigabit.
    pub const GBSR: u8 = 10;
}

/// Bits, del estandar.
pub mod bit {
    pub const BMCR_ANRESTART: u16 = 0x0200;
    pub const BMCR_ISOLATE: u16 = 0x0400;
    pub const BMCR_PDOWN: u16 = 0x0800;
    pub const BMCR_ANENABLE: u16 = 0x1000;
    pub const BMSR_LINK: u16 = 0x0004;
    pub const BMSR_ANEG_HECHA: u16 = 0x0020;
    pub const ANAR_SELECTOR_8023: u16 = 0x0001;
    pub const ANAR_10H: u16 = 0x0020;
    pub const ANAR_10F: u16 = 0x0040;
    pub const ANAR_100H: u16 = 0x0080;
    pub const ANAR_100F: u16 = 0x0100;
    pub const ANAR_PAUSA: u16 = 0x0400;
    pub const ANAR_PAUSA_ASIM: u16 = 0x0800;
    /// Las cuatro velocidades de ANAR y ANLPAR, juntas.
    pub const ANAR_VELOCIDADES: u16 = ANAR_10H | ANAR_10F | ANAR_100H | ANAR_100F;
    pub const GBCR_1000H: u16 = 0x0100;
    pub const GBCR_1000F: u16 = 0x0200;
    /// En GBSR, lo del otro va dos bits mas arriba que en GBCR.
    pub const GBSR_1000H: u16 = 0x0400;
    pub const GBSR_1000F: u16 = 0x0800;
}

/// El fabricante que contesta Realtek en `PHYID1`. Es la prueba de que el
/// camino elegido llega al PHY de verdad.
pub const REALTEK_PHYID1: u16 = 0x001C;

// -- LOS DOS CAMINOS AL PHY ----------------------------------------------------

/// **Como se le habla al PHY.** Depende de la generacion del chip, y no se
/// adivina: se prueban los dos y vale el que contesta [`REALTEK_PHYID1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camino {
    /// `PHYAR` (0x60): el de los 8168 hasta la "f". Bit 31 = la orden.
    Phyar,
    /// `GPHY_OCP` (0xB8): el de los 8168g/h (y la mayoria de placas AM4): el
    /// registro estandar `n` vive en la direccion OCP `0xA400 + 2n`.
    Ocp,
}

/// El registro de la tarjeta por el que va cada camino.
pub const PHYAR: usize = 0x60;
pub const GPHY_OCP: usize = 0xB8;
/// La bandera de "orden en marcha / hecha" de los dos.
pub const BANDERA: u32 = 0x8000_0000;
/// Donde empiezan los registros estandar por OCP.
pub const OCP_BASE_ESTANDAR: u16 = 0xA400;

impl Camino {
    /// El registro de la tarjeta.
    pub fn registro(self) -> usize {
        match self {
            Camino::Phyar => PHYAR,
            Camino::Ocp => GPHY_OCP,
        }
    }

    /// Lo que se escribe para LEER el registro MII `r`. La lectura acaba
    /// cuando la tarjeta PONE la bandera; el dato son los 16 de abajo.
    pub fn pedir_lectura(self, r: u8) -> u32 {
        match self {
            Camino::Phyar => ((r as u32) & 0x1F) << 16,
            Camino::Ocp => (ocp_de(r) as u32) << 15,
        }
    }

    /// Lo que se escribe para ESCRIBIR `v` en el registro MII `r`. La
    /// escritura acaba cuando la tarjeta QUITA la bandera.
    pub fn pedir_escritura(self, r: u8, v: u16) -> u32 {
        match self {
            Camino::Phyar => BANDERA | (((r as u32) & 0x1F) << 16) | v as u32,
            Camino::Ocp => BANDERA | ((ocp_de(r) as u32) << 15) | v as u32,
        }
    }
}

/// La direccion OCP del registro estandar `r` (0..=15).
pub fn ocp_de(r: u8) -> u16 {
    OCP_BASE_ESTANDAR + 2 * (r & 0x0F) as u16
}

// -- EL DIAGNOSTICO ------------------------------------------------------------

/// Lo que se leyo del PHY.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Mii {
    pub bmcr: u16,
    pub bmsr: u16,
    pub anar: u16,
    pub anlpar: u16,
    pub gbcr: u16,
    pub gbsr: u16,
}

/// Las velocidades de un lado, en Mbit (la mas alta que ofrece).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lado {
    pub m1000: bool,
    pub m100: bool,
    pub m10: bool,
}

impl Lado {
    pub fn tope(&self) -> u32 {
        if self.m1000 {
            1000
        } else if self.m100 {
            100
        } else if self.m10 {
            10
        } else {
            0
        }
    }
}

/// **Por que va a la velocidad que va.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Causa {
    /// A lo mas que dan los dos: no hay nada que arreglar.
    Bien,
    /// NOSOTROS solo anunciamos 10 (o 10 y 100 sin 1000): lo dejo asi quien
    /// apago antes (el driver de Windows al apagar con Wake-on-LAN). Se
    /// arregla RENEGOCIANDO: [`anuncio_completo`].
    AnunciamosPoco,
    /// El OTRO (el puerto del router o del switch) solo ofrece eso: el puerto,
    /// o el router lo limita. No es BMO-X.
    ElOtroDaPoco,
    /// Los dos ofrecen 1000 y salio 100: el CABLE solo tiene dos pares buenos
    /// (el PHY "baja" solo). Cambiar el cable.
    CableDeDosPares,
    /// La autonegociacion esta APAGADA: velocidad forzada.
    SinAutonegociar,
    /// El PHY esta APAGADO o aislado.
    Apagado,
    /// Sin enlace: no hay nada al otro lado (o no se ha acabado de negociar).
    SinEnlace,
}

impl Causa {
    /// El numero que viaja por el ABI (`RED_OP_MII` con `0xFF`).
    pub fn codigo(self) -> u8 {
        match self {
            Causa::Bien => 1,
            Causa::AnunciamosPoco => 2,
            Causa::ElOtroDaPoco => 3,
            Causa::CableDeDosPares => 4,
            Causa::SinAutonegociar => 5,
            Causa::Apagado => 6,
            Causa::SinEnlace => 7,
        }
    }
}

/// El registro "de mentira" que pide el VEREDICTO en vez de un registro MII.
pub const PIDE_VEREDICTO: u8 = 0xFF;

/// El diagnostico entero: lo que ofrecemos, lo que ofrece el otro, lo que
/// sale, y por que.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Veredicto {
    pub nosotros: Lado,
    pub el_otro: Lado,
    /// La velocidad que deberia salir: la mas alta comun.
    pub comun: u32,
    pub causa: Causa,
}

impl Veredicto {
    /// Empaquetado para el ABI: `causa | nosotros << 8 | el_otro << 24 |
    /// comun << 40` (las velocidades en Mbit, 16 bits cada una).
    pub fn empaquetar(&self) -> u64 {
        self.causa.codigo() as u64
            | ((self.nosotros.tope() as u64) << 8)
            | ((self.el_otro.tope() as u64) << 24)
            | ((self.comun as u64) << 40)
    }
}

impl Mii {
    /// Lo que anunciamos.
    pub fn nosotros(&self) -> Lado {
        Lado {
            m1000: self.gbcr & (bit::GBCR_1000F | bit::GBCR_1000H) != 0,
            m100: self.anar & (bit::ANAR_100F | bit::ANAR_100H) != 0,
            m10: self.anar & (bit::ANAR_10F | bit::ANAR_10H) != 0,
        }
    }

    /// Lo que anuncia el otro.
    pub fn el_otro(&self) -> Lado {
        Lado {
            m1000: self.gbsr & (bit::GBSR_1000F | bit::GBSR_1000H) != 0,
            m100: self.anlpar & (bit::ANAR_100F | bit::ANAR_100H) != 0,
            m10: self.anlpar & (bit::ANAR_10F | bit::ANAR_10H) != 0,
        }
    }

    /// **El veredicto.** `mbit` es lo que el chip dice que salio (`PHYstatus`).
    pub fn veredicto(&self, mbit: u32) -> Veredicto {
        let (n, o) = (self.nosotros(), self.el_otro());
        let comun = if n.m1000 && o.m1000 {
            1000
        } else if n.m100 && o.m100 {
            100
        } else if n.m10 && o.m10 {
            10
        } else {
            0
        };
        let causa = if self.bmcr & (bit::BMCR_PDOWN | bit::BMCR_ISOLATE) != 0 {
            Causa::Apagado
        } else if self.bmcr & bit::BMCR_ANENABLE == 0 {
            Causa::SinAutonegociar
        } else if self.bmsr & bit::BMSR_LINK == 0 && mbit == 0 {
            Causa::SinEnlace
        } else if comun == 1000 && mbit == 100 {
            Causa::CableDeDosPares
        } else if n.tope() < 1000 && o.tope() > n.tope() {
            // Lo que limita es lo que NOSOTROS ofrecemos.
            Causa::AnunciamosPoco
        } else if o.tope() < 1000 && mbit <= o.tope() {
            Causa::ElOtroDaPoco
        } else {
            Causa::Bien
        };
        Veredicto { nosotros: n, el_otro: o, comun, causa }
    }
}

/// **Lo que hay que escribir para anunciar 10/100/1000 y RENEGOCIAR**: lo que
/// hace el driver de cualquier sistema al arrancar. Devuelve `(anar, gbcr,
/// bmcr)` a partir de lo que hay, tocando SOLO lo suyo: las cuatro
/// velocidades y la pausa en ANAR (el selector se queda), el 1000 full en
/// GBCR (como el r8169: sin 1000 half), y en BMCR autonegociacion encendida,
/// el reinicio, y fuera el apagado y el aislamiento.
pub fn anuncio_completo(m: &Mii) -> (u16, u16, u16) {
    let anar = (m.anar & !(bit::ANAR_VELOCIDADES | bit::ANAR_PAUSA | bit::ANAR_PAUSA_ASIM) & !0x001F)
        | bit::ANAR_SELECTOR_8023
        | bit::ANAR_VELOCIDADES
        | bit::ANAR_PAUSA
        | bit::ANAR_PAUSA_ASIM;
    let gbcr = (m.gbcr & !(bit::GBCR_1000F | bit::GBCR_1000H)) | bit::GBCR_1000F;
    let bmcr = (m.bmcr & !(bit::BMCR_PDOWN | bit::BMCR_ISOLATE)) | bit::BMCR_ANENABLE | bit::BMCR_ANRESTART;
    (anar, gbcr, bmcr)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El Ryzen desde el 24-08: `PHYstatus` 0x87 (10 Mbit). Si lo que
    /// ANUNCIAMOS es solo 10 (lo que deja el apagado con WOL de Windows) y el
    /// router ofrece hasta 100, la causa es nuestra y se arregla renegociando.
    #[test]
    fn diez_por_el_apagado_de_windows() {
        let m = Mii {
            bmcr: bit::BMCR_ANENABLE,
            bmsr: bit::BMSR_LINK | bit::BMSR_ANEG_HECHA,
            anar: bit::ANAR_SELECTOR_8023 | bit::ANAR_10H | bit::ANAR_10F,
            anlpar: bit::ANAR_SELECTOR_8023 | bit::ANAR_VELOCIDADES,
            gbcr: 0,
            gbsr: 0,
        };
        let v = m.veredicto(10);
        assert_eq!(v.causa, Causa::AnunciamosPoco);
        assert_eq!(v.nosotros.tope(), 10);
        assert_eq!(v.el_otro.tope(), 100);
        // Y lo que se escribe anuncia todo y renegocia.
        let (anar, gbcr, bmcr) = anuncio_completo(&m);
        assert_eq!(anar & bit::ANAR_VELOCIDADES, bit::ANAR_VELOCIDADES);
        assert_eq!(anar & 0x001F, bit::ANAR_SELECTOR_8023, "el selector 802.3 se queda");
        assert_eq!(gbcr, bit::GBCR_1000F);
        assert_eq!(bmcr & (bit::BMCR_ANENABLE | bit::BMCR_ANRESTART), bit::BMCR_ANENABLE | bit::BMCR_ANRESTART);
        // Con eso, contra el mismo router, saldria 100.
        let despues = Mii { anar, gbcr, ..m };
        assert_eq!(despues.veredicto(100).causa, Causa::ElOtroDaPoco, "100 es lo que da el router: no es BMO-X");
        assert_eq!(despues.veredicto(100).comun, 100);
    }

    /// Con todo anunciado y el router en gigabit, si sale 100 es el CABLE.
    #[test]
    fn mil_de_los_dos_y_cien_es_el_cable() {
        let m = Mii {
            bmcr: bit::BMCR_ANENABLE,
            bmsr: bit::BMSR_LINK,
            anar: bit::ANAR_SELECTOR_8023 | bit::ANAR_VELOCIDADES,
            anlpar: bit::ANAR_SELECTOR_8023 | bit::ANAR_VELOCIDADES,
            gbcr: bit::GBCR_1000F,
            gbsr: bit::GBSR_1000F,
        };
        assert_eq!(m.veredicto(100).causa, Causa::CableDeDosPares);
        assert_eq!(m.veredicto(1000).causa, Causa::Bien);
    }

    /// El PHY apagado o sin autonegociar se dice antes que nada.
    #[test]
    fn apagado_y_forzado_se_dicen() {
        let mut m = Mii { bmcr: bit::BMCR_PDOWN | bit::BMCR_ANENABLE, ..Mii::default() };
        assert_eq!(m.veredicto(0).causa, Causa::Apagado);
        m.bmcr = 0;
        assert_eq!(m.veredicto(10).causa, Causa::SinAutonegociar);
        // Y anunciar todo lo enciende y quita el apagado.
        m.bmcr = bit::BMCR_PDOWN;
        let (_, _, bmcr) = anuncio_completo(&m);
        assert_eq!(bmcr & bit::BMCR_PDOWN, 0);
        assert_ne!(bmcr & bit::BMCR_ANENABLE, 0);
    }

    /// Los dos caminos al PHY: lo que se escribe en la tarjeta.
    #[test]
    fn los_dos_caminos() {
        // PHYAR: el registro en 20:16, el dato abajo, la bandera arriba.
        assert_eq!(Camino::Phyar.pedir_lectura(reg::PHYID1), 0x0002_0000);
        assert_eq!(Camino::Phyar.pedir_escritura(reg::BMCR, 0x1200), 0x8000_1200);
        // OCP: el registro estandar 2 es la direccion 0xA404, desplazada 15.
        assert_eq!(ocp_de(reg::PHYID1), 0xA404);
        assert_eq!(Camino::Ocp.pedir_lectura(reg::PHYID1), 0xA404u32 << 15);
        assert_eq!(Camino::Ocp.pedir_escritura(reg::ANAR, 0x0DE1), BANDERA | (0xA408u32 << 15) | 0x0DE1);
        assert_eq!(Camino::Phyar.registro(), 0x60);
        assert_eq!(Camino::Ocp.registro(), 0xB8);
    }

    /// El paquete del ABI se lee como se escribio.
    #[test]
    fn el_paquete() {
        let v = Veredicto {
            nosotros: Lado { m1000: false, m100: false, m10: true },
            el_otro: Lado { m1000: false, m100: true, m10: true },
            comun: 10,
            causa: Causa::AnunciamosPoco,
        };
        let p = v.empaquetar();
        assert_eq!(p & 0xFF, 2);
        assert_eq!((p >> 8) & 0xFFFF, 10);
        assert_eq!((p >> 24) & 0xFFFF, 100);
        assert_eq!((p >> 40) & 0xFFFF, 10);
    }

    /// Sin nada al otro lado se dice SIN ENLACE, no "anunciamos poco".
    #[test]
    fn sin_enlace() {
        let m = Mii { bmcr: bit::BMCR_ANENABLE, anar: bit::ANAR_VELOCIDADES, gbcr: bit::GBCR_1000F, ..Mii::default() };
        assert_eq!(m.veredicto(0).causa, Causa::SinEnlace);
    }
}
