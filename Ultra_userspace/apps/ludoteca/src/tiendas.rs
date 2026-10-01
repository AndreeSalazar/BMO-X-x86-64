//! **Las tiendas del riel**: la primera es "toda la coleccion", y despues las
//! doce de `bmo_ludoteca::Tienda`, cada una con sus dos colores y su gesto.

use crate::iconos::Gesto;
use bmo_ludoteca::Tienda;

pub struct Puesto {
    pub tienda: Option<Tienda>,
    pub gesto: Gesto,
    /// El color de la tienda y el segundo, para los degradados.
    pub color: u32,
    pub color2: u32,
}

pub const PUESTOS: [Puesto; 13] = [
    Puesto { tienda: None, gesto: Gesto::Todas, color: 0x00EE_F1F7, color2: 0x009A_A2B4 },
    Puesto { tienda: Some(Tienda::Gog), gesto: Gesto::Gog, color: 0x00B4_8CFF, color2: 0x006E_4BDB },
    Puesto { tienda: Some(Tienda::Steam), gesto: Gesto::Steam, color: 0x0066_C0F4, color2: 0x002A_6FB0 },
    Puesto { tienda: Some(Tienda::Epic), gesto: Gesto::Epic, color: 0x00E8_E8F0, color2: 0x007A_7F95 },
    Puesto { tienda: Some(Tienda::Ubisoft), gesto: Gesto::Ubisoft, color: 0x004F_A3FF, color2: 0x007B_5CFF },
    Puesto { tienda: Some(Tienda::Ea), gesto: Gesto::Ea, color: 0x00FF_5C6C, color2: 0x00FF_9A5C },
    Puesto { tienda: Some(Tienda::Battlenet), gesto: Gesto::Battlenet, color: 0x0038_C8FF, color2: 0x001E_6BFF },
    Puesto { tienda: Some(Tienda::Microsoft), gesto: Gesto::Microsoft, color: 0x007C_E38B, color2: 0x003F_B8FF },
    Puesto { tienda: Some(Tienda::Rockstar), gesto: Gesto::Rockstar, color: 0x00FF_C24D, color2: 0x00FF_7A3D },
    Puesto { tienda: Some(Tienda::Amazon), gesto: Gesto::Amazon, color: 0x00FF_AE3D, color2: 0x00FF_6E3D },
    Puesto { tienda: Some(Tienda::Itch), gesto: Gesto::Itch, color: 0x00FF_4F8B, color2: 0x00FF_8A5C },
    Puesto { tienda: Some(Tienda::Humble), gesto: Gesto::Humble, color: 0x00FF_6B6B, color2: 0x00C4_4DFF },
    Puesto { tienda: Some(Tienda::Libre), gesto: Gesto::Libre, color: 0x004D_E38F, color2: 0x002A_B8A0 },
];

impl Puesto {
    pub fn nombre(&self) -> &'static str {
        self.tienda.map_or("Toda la coleccion", |t| t.nombre())
    }
}

/// El color de una tienda (el del primero si no esta).
pub fn color_de(t: Tienda) -> u32 {
    PUESTOS.iter().find(|p| p.tienda == Some(t)).map_or(PUESTOS[0].color, |p| p.color)
}
