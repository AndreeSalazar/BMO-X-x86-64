//! **Las tiendas del riel**: la primera es "toda la coleccion", y despues las
//! doce de `bmo_ludoteca::Tienda`, cada una con su gesto (01-10: todas en el
//! verde de la LUDOTECA; cada una se distingue por como se MUEVE).

use crate::iconos::Gesto;
use bmo_ludoteca::Tienda;

pub struct Puesto {
    pub tienda: Option<Tienda>,
    pub gesto: Gesto,
}

pub const PUESTOS: [Puesto; 13] = [
    Puesto { tienda: None, gesto: Gesto::Todas },
    Puesto { tienda: Some(Tienda::Gog), gesto: Gesto::Gog },
    Puesto { tienda: Some(Tienda::Steam), gesto: Gesto::Steam },
    Puesto { tienda: Some(Tienda::Epic), gesto: Gesto::Epic },
    Puesto { tienda: Some(Tienda::Ubisoft), gesto: Gesto::Ubisoft },
    Puesto { tienda: Some(Tienda::Ea), gesto: Gesto::Ea },
    Puesto { tienda: Some(Tienda::Battlenet), gesto: Gesto::Battlenet },
    Puesto { tienda: Some(Tienda::Microsoft), gesto: Gesto::Microsoft },
    Puesto { tienda: Some(Tienda::Rockstar), gesto: Gesto::Rockstar },
    Puesto { tienda: Some(Tienda::Amazon), gesto: Gesto::Amazon },
    Puesto { tienda: Some(Tienda::Itch), gesto: Gesto::Itch },
    Puesto { tienda: Some(Tienda::Humble), gesto: Gesto::Humble },
    Puesto { tienda: Some(Tienda::Libre), gesto: Gesto::Libre },
];

impl Puesto {
    pub fn nombre(&self) -> &'static str {
        self.tienda.map_or("Toda la coleccion", |t| t.nombre())
    }
}
