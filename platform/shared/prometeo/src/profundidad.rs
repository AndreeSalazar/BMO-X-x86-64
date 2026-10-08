//! **La prueba de profundidad** (de `trama.rs` de PROTON-X; en PROMETEO
//! desde el 08-10, LB3b de `docs/plan/PLAN_LAS_LIBRERIAS.md`): la comparacion
//! de D3D12 que usan el interprete y las texturas al COMPARAR (`SampleCmp`,
//! `GatherCmp`) y la trama de PROTON-X al probar cada pixel. Una cuenta, un
//! sitio.
//!
//! [carril]  VERDE     una comparacion de floats; no toca la maquina

/// La prueba de profundidad: `D3D12_COMPARISON_FUNC` (1 nunca, 2 menor,
/// 3 igual, 4 menor o igual, 5 mayor, 6 distinto, 7 mayor o igual, 8
/// siempre) y si se escribe (`DepthWriteMask` ALL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profundidad {
    pub funcion: u32,
    pub escribir: bool,
}

impl Profundidad {
    /// Si `z` (el del pixel) pasa contra `guardado` (el del bufer).
    pub fn pasa(&self, z: f32, guardado: f32) -> bool {
        match self.funcion {
            1 => false,
            2 => z < guardado,
            3 => z == guardado,
            4 => z <= guardado,
            5 => z > guardado,
            6 => z != guardado,
            7 => z >= guardado,
            _ => true,
        }
    }
}
