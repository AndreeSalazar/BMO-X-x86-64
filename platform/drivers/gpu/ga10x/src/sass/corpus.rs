//! **EL CORPUS DE ORO** (J0 de `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`) --
//! los programas que YA corrieron en la 3060, con lo que el juez necesita de
//! cada uno.
//!
//! [carril]  VERDE     solo listas de programas que ya existen en el crate
//!
//! Un juez se calibra con lo que se SABE bueno: todo lo de `ORO` dibujo o
//! calculo en el metal, comparado bit a bit por la CPU. Si el juez le dice que
//! no a uno de ellos, el que esta mal es el juez. Y aparte, lo que NO se sabe
//! bueno: la variante `sinldg` de VERRANO (nunca corrio: la orden no la
//! reconocia hasta el 26-09).
//! Los de VERRANO V0 pasaron al oro el 26-09 06:46: IGUAL a D3D12.

use super::juez::Contexto;
use crate::raster::SPH;
use crate::{blur, color3d, cubo, escena, fractal, giro, lienzo, pantalla, raster, sombreador, triangulo, tuberia, video};

/// Un programa del corpus.
pub struct Programa {
    pub nombre: &'static str,
    /// Donde corrio (o por que esta aqui).
    pub origen: &'static str,
    pub codigo: &'static [(u64, u64)],
    pub registros: u32,
    pub sph: Option<&'static [u32; SPH]>,
}

impl Programa {
    pub fn contexto(&self) -> Contexto<'static> {
        Contexto { registros: self.registros, sph: self.sph }
    }
}

/// Un triangulo cualquiera para los programas de X5 (llevan los numeros
/// dentro; el juez mira la forma, no los numeros).
const T: cubo::Triangulo = cubo::Triangulo {
    clip: [[0x3f00_0000, 0x3e80_0000, 0x3f70_0000, 0x3f80_0000], [0; 4], [0x3f80_0000; 4]],
    color: [0x3f80_0000; 4],
};

static X5_VS: [(u64, u64); cubo::INSTR_VS] = cubo::codigo_vs(&T);
static X5_PS: [(u64, u64); cubo::INSTR_PS] = cubo::codigo_ps(&T);
static VERRANO_VS: [(u64, u64); tuberia::INSTR_VS] = tuberia::codigo_vs();
static VERRANO_VS_SIN_LDG: [(u64, u64); tuberia::INSTR_VS] = tuberia::codigo_vs_sin_ldg();
static VERRANO_PS: [(u64, u64); tuberia::INSTR_PS] = tuberia::codigo_ps();

static SPH_RASTER_VS: [u32; SPH] = raster::sph_vertice();
static SPH_RASTER_PS: [u32; SPH] = raster::sph_pixel();
static SPH_COLOR_VS: [u32; SPH] = color3d::sph_vertice();
static SPH_COLOR_PS: [u32; SPH] = color3d::sph_pixel();
static SPH_VERRANO_VS: [u32; SPH] = tuberia::sph_vertice();
static SPH_VERRANO_PS: [u32; SPH] = tuberia::sph_pixel();

const fn computo(nombre: &'static str, origen: &'static str, codigo: &'static [(u64, u64)], registros: u32) -> Programa {
    Programa { nombre, origen, codigo, registros, sph: None }
}

const fn grafico(nombre: &'static str, origen: &'static str, codigo: &'static [(u64, u64)], sph: &'static [u32; SPH]) -> Programa {
    Programa { nombre, origen, codigo, registros: raster::REGISTROS, sph: Some(sph) }
}

/// **El oro**: todo lo de aqui corrio en la 3060 y la CPU lo dio por bueno.
pub static ORO: [Programa; 17] = [
    computo("sombreador", "M5d S4: 32 de 32 hilos", &sombreador::CODIGO, sombreador::REGISTROS),
    computo("lienzo", "M5d L: 16384 de 16384 pixeles", &lienzo::CODIGO, sombreador::REGISTROS),
    computo("blur", "M5d B: igual a la CPU", &blur::CODIGO, blur::REGISTROS),
    computo("fractal", "M5d F: 262144 hilos, bit a bit", &fractal::CODIGO, fractal::REGISTROS),
    computo("triangulo", "M5d T0: bit a bit", &triangulo::CODIGO, triangulo::REGISTROS),
    computo("escena", "M5d E: bit a bit", &escena::CODIGO, escena::REGISTROS),
    computo("giro", "M5d G: 32 fotogramas", &giro::CODIGO, giro::REGISTROS),
    computo("pantalla", "M5d P: la pantalla entera, ~466 fps", &pantalla::CODIGO, pantalla::REGISTROS),
    computo("video", "M6 V0", &video::CODIGO, video::REGISTROS),
    grafico("raster vertice", "T1c: 50 de 50", &raster::CODIGO_VS, &SPH_RASTER_VS),
    grafico("raster pixel", "T1c: 50 de 50", &raster::CODIGO_PS, &SPH_RASTER_PS),
    grafico("color3d vertice", "T2a: tres colores", &color3d::CODIGO_VS, &SPH_COLOR_VS),
    grafico("color3d pixel", "T2a: tres colores", &color3d::CODIGO_PS, &SPH_COLOR_PS),
    grafico("X5 vertice", "X5: el cubo IGUAL a D3D12", &X5_VS, &SPH_RASTER_VS),
    grafico("X5 pixel", "X5: el cubo IGUAL a D3D12", &X5_PS, &SPH_RASTER_PS),
    grafico("VERRANO vertice", "VERRANO V0: IGUAL a D3D12 (metal 26-09 06:46), con R1", &VERRANO_VS, &SPH_VERRANO_VS),
    grafico("VERRANO pixel", "VERRANO V0: IGUAL a D3D12 (metal 26-09 06:46)", &VERRANO_PS, &SPH_VERRANO_PS),
];

/// **Los sospechosos**: lo que NO se sabe bueno.
pub static SOSPECHOSOS: [Programa; 1] = [
    grafico("VERRANO vertice sin LDG", "la prueba de una variable (gpu verrano sinldg)", &VERRANO_VS_SIN_LDG, &SPH_VERRANO_VS),
];
