//! **Las tiendas del riel**: la primera es "toda la coleccion", y despues las
//! doce de `bmo_ludoteca::Tienda`, cada una con su INSIGNIA (01-10, la
//! maqueta `Insignias`): su color oficial, su fondo y un gesto que es solo
//! suyo. Las insignias son dibujos PROPIOS, no los logos de las tiendas.
//!
//! ** EL HUECO DEL LOGO OFICIAL: `ludoteca/logos/<id>.svg` en ESTRATOS. Las
//! tiendas publican su logo en su kit de prensa; el propietario lo baja y lo
//! deja ahi, y la LUDOTECA lo dira (`logo`). Pintarlo pide un lector de SVG
//! que todavia no hay: mientras, la insignia.

use crate::iconos::Gesto;
use bmo_ludoteca::Tienda;

pub struct Puesto {
    pub tienda: Option<Tienda>,
    pub gesto: Gesto,
    /// El color oficial de la tienda y el fondo de su insignia.
    pub color: u32,
    pub fondo: u32,
    /// Su nombre en `ludoteca/logos/<id>.svg`.
    pub id: &'static str,
}

pub const PUESTOS: [Puesto; 13] = [
    Puesto { tienda: None, gesto: Gesto::Todas, color: 0x0039_FF88, fondo: 0x0008_241A, id: "todas" },
    Puesto { tienda: Some(Tienda::Gog), gesto: Gesto::Gog, color: 0x00A5_5DFF, fondo: 0x001A_0F2E, id: "gog" },
    Puesto { tienda: Some(Tienda::Steam), gesto: Gesto::Steam, color: 0x0066_C0F4, fondo: 0x0017_1A21, id: "steam" },
    Puesto { tienda: Some(Tienda::Epic), gesto: Gesto::Epic, color: 0x00FF_FFFF, fondo: 0x0020_2020, id: "epic" },
    Puesto { tienda: Some(Tienda::Ubisoft), gesto: Gesto::Ubisoft, color: 0x0000_70FF, fondo: 0x0006_142E, id: "ubisoft" },
    Puesto { tienda: Some(Tienda::Ea), gesto: Gesto::Ea, color: 0x00FF_4747, fondo: 0x0020_0A0C, id: "ea" },
    Puesto { tienda: Some(Tienda::Battlenet), gesto: Gesto::Battlenet, color: 0x0014_8EFF, fondo: 0x0006_1A33, id: "battlenet" },
    Puesto { tienda: Some(Tienda::Microsoft), gesto: Gesto::Microsoft, color: 0x0000_A4EF, fondo: 0x000C_1724, id: "microsoft" },
    Puesto { tienda: Some(Tienda::Rockstar), gesto: Gesto::Rockstar, color: 0x00FC_AF17, fondo: 0x001C_1404, id: "rockstar" },
    Puesto { tienda: Some(Tienda::Amazon), gesto: Gesto::Amazon, color: 0x00FF_9900, fondo: 0x001E_1406, id: "amazon" },
    Puesto { tienda: Some(Tienda::Itch), gesto: Gesto::Itch, color: 0x00FA_5C5C, fondo: 0x0022_0C0E, id: "itch" },
    Puesto { tienda: Some(Tienda::Humble), gesto: Gesto::Humble, color: 0x00CC_2929, fondo: 0x001E_0808, id: "humble" },
    Puesto { tienda: Some(Tienda::Libre), gesto: Gesto::Libre, color: 0x0039_FF88, fondo: 0x0008_241A, id: "libre" },
];

impl Puesto {
    pub fn nombre(&self) -> &'static str {
        self.tienda.map_or("Toda la coleccion", |t| t.nombre())
    }
}

impl Puesto {
    /// **El logo oficial de esta tienda, si el propietario lo dejo** en
    /// `ludoteca/logos/<id>.svg` (sin traerlo: el kernel lo refleja).
    pub fn logo(&self) -> bool {
        let mut ruta = [0u8; 40];
        let pre = b"ludoteca/logos/";
        let n = pre.len() + self.id.len() + 4;
        ruta[..pre.len()].copy_from_slice(pre);
        ruta[pre.len()..pre.len() + self.id.len()].copy_from_slice(self.id.as_bytes());
        ruta[n - 4..n].copy_from_slice(b".svg");
        match bmo_userland::Archivo::reflejar(&ruta[..n]) {
            Ok(a) => {
                a.close();
                true
            }
            Err(_) => false,
        }
    }
}
