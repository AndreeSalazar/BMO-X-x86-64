//! **Los buferes que lee un sombreador** (03-10, N5.3): `Buffer<T>`,
//! `StructuredBuffer<S>` y `ByteAddressBuffer`, vistos por un SRV.
//!
//! [carril]  VERDE     lee bytes que ya le dan; no toca la maquina
//! [cuesta]  DATO      un elemento mal contado da los datos del de al lado
//! [riesgo]  ESPEJO    las reglas son las de D3D12 (fuera de la vista, 0);
//!                     el banco las prueba con un sombreador de `dxc`
//! [consumo] NADA      solo cuando un sombreador lee un bufer
//!
//! Hasta el 03-10 un SRV de bufer "se leia como nulo" y un `bufferLoad` no
//! compilaba: los sombreadores de vertices de Cyberpunk leen sus instancias
//! y sus huesos de buferes, y no corria ninguno.
//!
//! ```text
//!    con tipo      Buffer<float4>: el elemento i, en el formato de la vista
//!                  (como un vertice: `formato_ia`)
//!    estructurado  StructuredBuffer<S>: 4 palabras desde i * paso + desp,
//!                  dentro de SU elemento
//!    crudo         ByteAddressBuffer: 4 palabras desde el byte i
//!    fuera         lo que cae fuera de la vista se lee como 0
//! ```

/// **Como se direcciona** un bufer: lo dice el sombreador (su `ResKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    Tipado,
    Estructurado,
    Crudo,
}

/// **Un bufer, visto por un SRV**: sus bytes desde el primer elemento de la
/// vista, y lo que dice la vista.
#[derive(Clone, Copy, Debug)]
pub struct Bufer<'a> {
    pub bytes: &'a [u8],
    /// El DXGI_FORMAT de una vista con tipo (0 si no tiene).
    pub formato: u32,
    /// El paso de una vista estructurada (0 si no lo es).
    pub paso: u32,
    /// Los elementos de la vista (en una cruda, palabras de 4 bytes).
    pub elementos: u32,
}

impl Bufer<'_> {
    /// Las 4 palabras de `[desde, desde + 16)`, con 0 fuera de `[0, hasta)`.
    fn palabras(&self, desde: u64, hasta: u64) -> [u32; 4] {
        let hasta = hasta.min(self.bytes.len() as u64);
        core::array::from_fn(|k| {
            let o = desde + 4 * k as u64;
            match self.bytes.get(o as usize..o as usize + 4) {
                Some(b) if o + 4 <= hasta => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                _ => 0,
            }
        })
    }

    /// **`Load`**: el elemento `i` (en uno crudo, el byte `i`), y `desp`
    /// bytes dentro de el en uno estructurado. Como bits (un float, sus
    /// bits; un entero, el).
    pub fn cargar(&self, modo: Modo, i: u32, desp: u32) -> [u32; 4] {
        match modo {
            Modo::Tipado => {
                let Some(f) = crate::formato_ia::forma(self.formato) else { return [0; 4] };
                let (n, o) = (f.bytes as usize, i as usize * f.bytes as usize);
                match self.bytes.get(o..o + n) {
                    Some(b) if i < self.elementos => crate::formato_ia::leer(self.formato, b).map(f32::to_bits),
                    _ => [0; 4],
                }
            }
            Modo::Estructurado if self.paso == 0 || i >= self.elementos => [0; 4],
            Modo::Estructurado => {
                let base = i as u64 * self.paso as u64;
                self.palabras(base + desp as u64, base + self.paso as u64)
            }
            Modo::Crudo => self.palabras(i as u64 + desp as u64, self.elementos as u64 * 4),
        }
    }

    /// **`GetDimensions`**: los elementos (en uno crudo, los bytes).
    pub fn medidas(&self, modo: Modo) -> [u32; 4] {
        match modo {
            Modo::Crudo => [self.elementos.saturating_mul(4), 0, 0, 0],
            _ => [self.elementos, 0, 0, 0],
        }
    }
}

/// **Un bufer visto por un UAV** (N5.5, 05-10): sus bytes, que el computo
/// LEE y ESCRIBE (`RWStructuredBuffer`, `RWByteAddressBuffer`, `RWBuffer`).
#[derive(Debug)]
pub struct Uav<'a> {
    pub bytes: &'a mut [u8],
    /// El DXGI_FORMAT de una vista con tipo (0 si no tiene).
    pub formato: u32,
    /// El paso de una vista estructurada (0 si no lo es).
    pub paso: u32,
    /// Los elementos de la vista (en una cruda, palabras de 4 bytes).
    pub elementos: u32,
    /// E2.4 (05-10): su CONTADOR oculto (`CreateUnorderedAccessView` con
    /// un `pCounterResource`): lo que mueven `Append`, `Consume`,
    /// `IncrementCounter` y `DecrementCounter`.
    pub contador: Option<&'a mut u32>,
}

/// Los formatos con 32 bits por canal (float, uint y sint de 4, 3, 2 y 1
/// canales): los que un `RWBuffer` con tipo escribe aqui tal cual, palabra a
/// palabra. Los demas (UNORM, 16 bits...) piden convertir: todavia no.
fn canales_de_32(formato: u32) -> Option<usize> {
    match formato {
        2..=4 => Some(4),
        6..=8 => Some(3),
        16..=18 => Some(2),
        41..=43 => Some(1),
        _ => None,
    }
}

impl Uav<'_> {
    /// **`IncrementCounter` (`inc` 1) y `DecrementCounter` (-1)**: suben o
    /// bajan el contador y devuelven, como D3D, el de ANTES al subir y el de
    /// DESPUES al bajar. Sin contador, 0 (y nada se mueve).
    pub fn contar(&mut self, inc: i8) -> u32 {
        match self.contador.as_deref_mut() {
            Some(c) => {
                let antes = *c;
                *c = c.wrapping_add(inc as i32 as u32);
                if inc >= 0 {
                    antes
                } else {
                    *c
                }
            }
            None => 0,
        }
    }

    /// Lo que se lee de el, con las reglas de un SRV ([`Bufer::cargar`]).
    pub fn cargar(&self, modo: Modo, i: u32, desp: u32) -> [u32; 4] {
        Bufer { bytes: self.bytes, formato: self.formato, paso: self.paso, elementos: self.elementos }.cargar(modo, i, desp)
    }

    /// **`Store`**: los canales de `v` que dice `mascara` (bit 0 el primero),
    /// en el elemento `i` (en uno crudo, el byte `i`) y `desp` bytes dentro de
    /// el en uno estructurado. Fuera de la vista no se escribe nada (D3D12:
    /// una escritura fuera de un UAV se pierde).
    pub fn escribir(&mut self, modo: Modo, i: u32, desp: u32, v: [u32; 4], mascara: u8) {
        let (desde, hasta, canales) = match modo {
            Modo::Estructurado if self.paso == 0 || i >= self.elementos => return,
            Modo::Estructurado => {
                let base = i as u64 * self.paso as u64;
                (base + desp as u64, base + self.paso as u64, 4)
            }
            Modo::Crudo => (i as u64 + desp as u64, self.elementos as u64 * 4, 4),
            Modo::Tipado => {
                let Some(n) = canales_de_32(self.formato) else { return };
                if i >= self.elementos {
                    return;
                }
                let base = i as u64 * 4 * n as u64;
                (base, base + 4 * n as u64, n)
            }
        };
        let hasta = hasta.min(self.bytes.len() as u64);
        for (k, palabra) in v.iter().enumerate().take(canales) {
            let o = desde + 4 * k as u64;
            if mascara & (1 << k) != 0 && o + 4 <= hasta {
                self.bytes[o as usize..o as usize + 4].copy_from_slice(&palabra.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use alloc::vec::Vec;

    fn bytes(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn un_uav_escribe_lo_de_su_mascara_y_nada_fuera_de_su_vista() {
        let mut b = bytes(&[0; 8]);
        let mut u = Uav { bytes: &mut b, formato: 0, paso: 16, elementos: 2, contador: None };
        u.escribir(Modo::Estructurado, 1, 0, [1, 2, 3, 4], 0b0101);
        u.escribir(Modo::Estructurado, 2, 0, [9; 4], 0xF); // fuera: se pierde
        u.escribir(Modo::Estructurado, 0, 8, [7, 8, 9, 9], 0xF); // del 8 al 16: dos
        assert_eq!(u.cargar(Modo::Estructurado, 1, 0), [1, 0, 3, 0]);
        assert_eq!(u.cargar(Modo::Estructurado, 0, 0), [0, 0, 7, 8], "no pisa el elemento de al lado");
        let mut c = bytes(&[0; 4]);
        let mut t = Uav { bytes: &mut c, formato: 41, paso: 0, elementos: 4, contador: None }; // R32_FLOAT
        t.escribir(Modo::Tipado, 2, 0, [5, 6, 7, 8], 0xF);
        assert_eq!(t.cargar(Modo::Crudo, 8, 0), [5, 0, 0, 0], "un R32: una palabra por elemento");
    }

    #[test]
    fn estructurado_dentro_de_su_elemento() {
        // Dos elementos de 12 bytes: (1, 2, 3) y (4, 5, 6).
        let b = bytes(&[1, 2, 3, 4, 5, 6]);
        let v = Bufer { bytes: &b, formato: 0, paso: 12, elementos: 2 };
        assert_eq!(v.cargar(Modo::Estructurado, 1, 0), [4, 5, 6, 0], "la cuarta palabra ya es de fuera");
        assert_eq!(v.cargar(Modo::Estructurado, 0, 4), [2, 3, 0, 0], "no se lee el elemento de al lado");
        assert_eq!(v.cargar(Modo::Estructurado, 2, 0), [0; 4], "fuera de la vista");
        assert_eq!(v.medidas(Modo::Estructurado), [2, 0, 0, 0]);
    }

    #[test]
    fn crudo_por_bytes() {
        let b = bytes(&[10, 20, 30, 40, 50]);
        let v = Bufer { bytes: &b, formato: 0, paso: 0, elementos: 5 };
        assert_eq!(v.cargar(Modo::Crudo, 8, 0), [30, 40, 50, 0]);
        assert_eq!(v.medidas(Modo::Crudo), [20, 0, 0, 0]);
        // Una vista mas corta que el bufer: lo de detras no se ve.
        let corta = Bufer { elementos: 3, ..v };
        assert_eq!(corta.cargar(Modo::Crudo, 4, 0), [20, 30, 0, 0]);
    }

    #[test]
    fn con_tipo_en_su_formato() {
        // R32G32B32A32_FLOAT (2) y R8G8B8A8_UNORM (28).
        let f = [1.5f32, -2.0, 0.25, 8.0];
        let b: Vec<u8> = f.iter().flat_map(|x| x.to_le_bytes()).collect();
        let v = Bufer { bytes: &b, formato: 2, paso: 0, elementos: 1 };
        assert_eq!(v.cargar(Modo::Tipado, 0, 0), f.map(f32::to_bits));
        assert_eq!(v.cargar(Modo::Tipado, 1, 0), [0; 4]);
        let c = [0u8, 255, 0, 255];
        let u = Bufer { bytes: &c, formato: 28, paso: 0, elementos: 1 };
        assert_eq!(u.cargar(Modo::Tipado, 0, 0), [0.0f32, 1.0, 0.0, 1.0].map(f32::to_bits));
    }

    /// E2.4: el contador sube devolviendo el de antes, baja devolviendo el
    /// de despues; sin contador, 0.
    #[test]
    fn el_contador_de_un_uav_sube_y_baja_como_en_d3d() {
        let mut b = bytes(&[0; 4]);
        let mut c = 5u32;
        let mut u = Uav { bytes: &mut b, formato: 0, paso: 16, elementos: 1, contador: Some(&mut c) };
        assert_eq!((u.contar(1), u.contar(1)), (5, 6));
        assert_eq!(u.contar(-1), 6, "bajar: el de despues");
        drop(u);
        assert_eq!(c, 6);
        let mut sin = Uav { bytes: &mut b, formato: 0, paso: 16, elementos: 1, contador: None };
        assert_eq!(sin.contar(1), 0);
    }
}
