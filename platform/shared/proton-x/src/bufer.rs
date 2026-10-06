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
//!    textura       N5.3c (05-10), RWTexture2D: el texel (x, y), en el
//!                  formato de la vista (`paso` es el ancho)
//!    en floats     N5.16b (05-10): el formato con [`CUATRO_FLOATS`]: cada
//!                  elemento son cuatro f32 (como guarda la casa un RGBA16F,
//!                  un R11G11B10F...), leidos tal cual y escritos
//!                  cuantizados al formato de la vista
//! ```

/// **Como se direcciona** un bufer: lo dice el sombreador (su `ResKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    Tipado,
    Estructurado,
    Crudo,
    /// N5.3c (05-10): un UAV de TEXTURA de una o dos dimensiones
    /// (`RWTexture2D<float4>`): `i` es la x y `desp` la y; el ancho va en
    /// `paso` y los texels (ancho por alto) en `elementos`.
    Textura,
}

/// **Un bit del formato de una vista** (N5.16b, 05-10): sus elementos son
/// CUATRO f32 (16 bytes), ya cuantizados al formato de los bits bajos; es
/// como guarda la casa las texturas de float (`Almacen::Flotantes4`). Se
/// leen tal cual y se escriben cuantizados (`formato_ia::cuantizar`): lo que
/// el sombreador lee de vuelta es lo que leeria en la GPU.
pub const CUATRO_FLOATS: u32 = 0x1_0000;

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
            Modo::Textura => match self.texel(i, desp) {
                Some(t) => self.cargar(Modo::Tipado, t, 0),
                None => [0; 4],
            },
            Modo::Tipado if self.formato & CUATRO_FLOATS != 0 => {
                if i < self.elementos {
                    self.palabras(16 * i as u64, 16 * i as u64 + 16)
                } else {
                    [0; 4]
                }
            }
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

    /// El texel `(x, y)` de una textura, si cae dentro.
    fn texel(&self, x: u32, y: u32) -> Option<u32> {
        let ancho = self.paso.max(1);
        (x < self.paso && y < self.elementos / ancho).then(|| y * ancho + x)
    }

    /// **`GetDimensions`**: los elementos (en uno crudo, los bytes; en una
    /// textura, su ancho y su alto).
    pub fn medidas(&self, modo: Modo) -> [u32; 4] {
        match modo {
            Modo::Textura => [self.paso, self.elementos / self.paso.max(1), 0, 0],
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
    /// 06-10: un UAV de textura 3D o de ARRAY (`RWTexture3D`,
    /// `RWTexture2DArray`): donde va cada rebanada (o capa). `PLANA` en los
    /// demas: un bufer, o una textura de una o dos dimensiones.
    pub rebanadas: Rebanadas,
}

/// **Las rebanadas de un UAV de textura 3D o de array** (06-10): cada una,
/// una textura 2D de `paso` (el ancho) por `alto` texeles, la `z` empezando
/// `z * salto` texeles despues de la primera (en un array con mips, cada
/// capa lleva su cadena de mips detras: el salto es mas que ancho por alto).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rebanadas {
    pub alto: u32,
    /// Cuantas (WSize o ArraySize de la vista); 0, ninguna: `PLANA`.
    pub capas: u32,
    pub salto: u32,
}

impl Rebanadas {
    /// La de un bufer o una textura de una o dos dimensiones.
    pub const PLANA: Rebanadas = Rebanadas { alto: 0, capas: 0, salto: 0 };
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

/// **La operacion de un `Interlocked*`** (05-10: los UAV de un dibujo, y del
/// computo): la `atomicOp` de `AtomicBinOp` (0..8, en su orden) y, aparte,
/// `InterlockedCompareExchange` (`AtomicCompareExchange`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Atomo {
    Suma,
    Y,
    O,
    Xor,
    MinConSigno,
    MaxConSigno,
    MinSinSigno,
    MaxSinSigno,
    Cambia,
    /// Cambia solo si lo que hay es igual a lo que se compara.
    CambiaSiIgual,
}

impl Atomo {
    /// La de `AtomicBinOp` con su numero de DXIL (`None` si no es ninguna).
    pub fn de_dxil(n: i64) -> Option<Atomo> {
        use Atomo::*;
        [Suma, Y, O, Xor, MinConSigno, MaxConSigno, MinSinSigno, MaxSinSigno, Cambia].get(usize::try_from(n).ok()?).copied()
    }

    /// Lo que queda en la palabra que tenia `antes` (con el valor `v` y, en
    /// la de comparar, `igual`).
    pub fn hacer(self, antes: u32, v: u32, igual: u32) -> u32 {
        match self {
            Atomo::Suma => antes.wrapping_add(v),
            Atomo::Y => antes & v,
            Atomo::O => antes | v,
            Atomo::Xor => antes ^ v,
            Atomo::MinConSigno => (antes as i32).min(v as i32) as u32,
            Atomo::MaxConSigno => (antes as i32).max(v as i32) as u32,
            Atomo::MinSinSigno => antes.min(v),
            Atomo::MaxSinSigno => antes.max(v),
            Atomo::Cambia => v,
            Atomo::CambiaSiIgual if antes == igual => v,
            Atomo::CambiaSiIgual => antes,
        }
    }
}

impl Uav<'_> {
    /// **Un `Interlocked*`** sobre la palabra de 32 bits del elemento `i`
    /// (`desp` bytes dentro, en uno estructurado; en uno crudo, el byte `i`;
    /// en una textura, el texel `(i, desp)`): la deja como dice `como` y
    /// devuelve la de ANTES. Fuera de la vista, ni se lee ni se escribe (0).
    /// Atomico de verdad porque quien corre los hilos es UNO: los pixeles de
    /// un dibujo y los hilos de un Dispatch van de uno en uno.
    pub fn atomico(&mut self, modo: Modo, i: u32, desp: u32, como: Atomo, v: u32, igual: u32) -> u32 {
        let antes = self.cargar(modo, i, desp)[0];
        self.escribir(modo, i, desp, [como.hacer(antes, v, igual), 0, 0, 0], 1);
        antes
    }

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

    /// **La rebanada `z` de un UAV de textura** (06-10), vista como una
    /// textura 2D: la de un 3D o un array, o la textura entera si es plana y
    /// `z` es 0. `None` fuera (D3D12: se lee 0 y no se escribe).
    pub fn rebanada(&mut self, z: u32) -> Option<Uav<'_>> {
        let r = self.rebanadas;
        if r.capas == 0 {
            return (z == 0).then(|| Uav { bytes: &mut *self.bytes, formato: self.formato, paso: self.paso, elementos: self.elementos, contador: None, rebanadas: Rebanadas::PLANA });
        }
        if z >= r.capas {
            return None;
        }
        let por_texel = if self.formato & CUATRO_FLOATS != 0 { 16 } else { crate::formato_ia::forma(self.formato)?.bytes as usize };
        let texeles = self.paso as usize * r.alto as usize;
        let desde = z as usize * r.salto as usize * por_texel;
        let bytes = self.bytes.get_mut(desde..desde + texeles * por_texel)?;
        Some(Uav { bytes, formato: self.formato, paso: self.paso, elementos: texeles as u32, contador: None, rebanadas: Rebanadas::PLANA })
    }

    /// **`GetDimensions` de un UAV de textura** (06-10): ancho, alto y, en
    /// un 3D o un array, cuantas rebanadas (o capas).
    pub fn medidas_textura(&self) -> [u32; 4] {
        match self.rebanadas.capas {
            0 => [self.paso, self.elementos / self.paso.max(1), 0, 0],
            n => [self.paso, self.rebanadas.alto, n, 0],
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
        let vista = Bufer { bytes: self.bytes, formato: self.formato, paso: self.paso, elementos: self.elementos };
        // N5.3c: una textura es el tipado de su texel; un tipado que no es de
        // 32 bits por canal, su elemento ENTERO en su formato (`empaquetar`,
        // con lo que la mascara deja de antes).
        let (modo, i) = match modo {
            Modo::Textura => match vista.texel(i, desp) {
                Some(t) => (Modo::Tipado, t),
                None => return,
            },
            m => (m, i),
        };
        // N5.16b: en cuatro floats, lo de la mascara sobre lo de antes, y
        // cuantizado al formato de la vista.
        if modo == Modo::Tipado && self.formato & CUATRO_FLOATS != 0 {
            let o = 16 * i as usize;
            if i >= self.elementos || o + 16 > self.bytes.len() {
                return;
            }
            let antes = vista.cargar(Modo::Tipado, i, 0);
            let w: [f32; 4] = core::array::from_fn(|k| f32::from_bits(if mascara & (1 << k) != 0 { v[k] } else { antes[k] }));
            // 05-10: una textura de ENTEROS (RGBA8_UINT, R16_SINT...) se guarda
            // asi tambien: sus bits bajos, como un RWBuffer de enteros (abajo).
            let f = self.formato & !CUATRO_FLOATS;
            let q = match crate::formato_ia::es_entero(f) {
                true => crate::formato_ia::empaquetar(f, w.map(f32::to_bits), true).map_or(w, |b| crate::formato_ia::leer(f, &b)),
                false => crate::formato_ia::cuantizar(f, w),
            };
            for (k, x) in q.iter().enumerate() {
                self.bytes[o + 4 * k..o + 4 * k + 4].copy_from_slice(&x.to_bits().to_le_bytes());
            }
            return;
        }
        if modo == Modo::Tipado && canales_de_32(self.formato).is_none() {
            let Some(f) = crate::formato_ia::forma(self.formato) else { return };
            let n = f.bytes as usize;
            let o = i as usize * n;
            if i >= self.elementos || o + n > self.bytes.len() {
                return;
            }
            let antes = vista.cargar(Modo::Tipado, i, 0);
            let w: [u32; 4] = core::array::from_fn(|k| if mascara & (1 << k) != 0 { v[k] } else { antes[k] });
            let enteros = matches!(f.clase, crate::formato_ia::Clase::Uint | crate::formato_ia::Clase::Sint);
            if let Some(e) = crate::formato_ia::empaquetar(self.formato, w, enteros) {
                self.bytes[o..o + n].copy_from_slice(&e);
            }
            return;
        }
        let (desde, hasta, canales) = match modo {
            Modo::Textura => return,
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
        let mut u = Uav { bytes: &mut b, formato: 0, paso: 16, elementos: 2, contador: None, rebanadas: Rebanadas::PLANA };
        u.escribir(Modo::Estructurado, 1, 0, [1, 2, 3, 4], 0b0101);
        u.escribir(Modo::Estructurado, 2, 0, [9; 4], 0xF); // fuera: se pierde
        u.escribir(Modo::Estructurado, 0, 8, [7, 8, 9, 9], 0xF); // del 8 al 16: dos
        assert_eq!(u.cargar(Modo::Estructurado, 1, 0), [1, 0, 3, 0]);
        assert_eq!(u.cargar(Modo::Estructurado, 0, 0), [0, 0, 7, 8], "no pisa el elemento de al lado");
        let mut c = bytes(&[0; 4]);
        let mut t = Uav { bytes: &mut c, formato: 41, paso: 0, elementos: 4, contador: None, rebanadas: Rebanadas::PLANA }; // R32_FLOAT
        t.escribir(Modo::Tipado, 2, 0, [5, 6, 7, 8], 0xF);
        assert_eq!(t.cargar(Modo::Crudo, 8, 0), [5, 0, 0, 0], "un R32: una palabra por elemento");
    }

    /// N5.16b: una textura RGBA16F guardada en cuatro f32 por texel: se lee
    /// tal cual y se escribe cuantizada a half (1 + 2^-12 es 1: no cabe), con
    /// lo que la mascara deja de antes.
    #[test]
    fn un_uav_en_cuatro_floats_escribe_cuantizado_a_su_formato() {
        let mut b = bytes(&[1.5f32, -2.0, 0.25, 1.0, 0.0, 0.0, 0.0, 0.0].map(f32::to_bits));
        let mut u = Uav { bytes: &mut b, formato: 10 | CUATRO_FLOATS, paso: 2, elementos: 2, contador: None, rebanadas: Rebanadas::PLANA };
        assert_eq!(u.cargar(Modo::Textura, 0, 0), [1.5f32, -2.0, 0.25, 1.0].map(f32::to_bits));
        u.escribir(Modo::Textura, 1, 0, [1.0 + 1.0 / 4096.0, 70000.0, -3.5, 9.0].map(f32::to_bits), 0b0111);
        assert_eq!(u.cargar(Modo::Textura, 1, 0), [1.0, f32::INFINITY, -3.5, 0.0].map(f32::to_bits), "a half, y el alfa de antes");
        u.escribir(Modo::Textura, 2, 0, [0; 4], 0xF); // fuera: se pierde
        assert_eq!(u.cargar(Modo::Textura, 0, 1), [0; 4]);
        // 06-10: un 3D (o un array) de rebanadas de 2 x 1 texeles de R32_UINT,
        // una cada TRES texeles (el de en medio, de otra mip: no se toca).
        let mut b = [0u8; 4 * 8];
        let mut u = Uav { bytes: &mut b, formato: 42, paso: 2, elementos: 8, contador: None, rebanadas: Rebanadas { alto: 1, capas: 3, salto: 3 } };
        for z in 0..3 {
            u.rebanada(z).unwrap().escribir(Modo::Textura, 1, 0, [10 + z, 0, 0, 0], 1);
        }
        assert!(u.rebanada(3).is_none(), "fuera de las rebanadas");
        assert_eq!(u.rebanada(2).unwrap().cargar(Modo::Textura, 1, 0)[0], 12);
        assert_eq!(u.medidas_textura(), [2, 1, 3, 0]);
        let palabras: alloc::vec::Vec<u32> = b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        assert_eq!(palabras, [0, 10, 0, 0, 11, 0, 0, 12], "cada una en su sitio, y lo de en medio intacto");
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

    /// Los `Interlocked*` (05-10): la palabra nueva, la de antes devuelta, y
    /// fuera de la vista nada.
    #[test]
    fn un_atomico_devuelve_lo_de_antes_y_deja_lo_nuevo() {
        let mut b = bytes(&[5, 0xFFFF_FFFE, 0, 0]);
        let mut u = Uav { bytes: &mut b, formato: 0, paso: 0, elementos: 4, contador: None, rebanadas: Rebanadas::PLANA };
        assert_eq!(u.atomico(Modo::Crudo, 0, 0, Atomo::Suma, 3, 0), 5);
        assert_eq!(u.atomico(Modo::Crudo, 4, 0, Atomo::MinConSigno, 1, 0), 0xFFFF_FFFE, "-2 con signo es menor que 1");
        assert_eq!(u.atomico(Modo::Crudo, 4, 0, Atomo::MaxSinSigno, 1, 0), 0xFFFF_FFFE);
        assert_eq!(u.atomico(Modo::Crudo, 8, 0, Atomo::CambiaSiIgual, 9, 1), 0, "0 no es 1: no cambia");
        assert_eq!(u.atomico(Modo::Crudo, 8, 0, Atomo::CambiaSiIgual, 9, 0), 0);
        assert_eq!(u.atomico(Modo::Crudo, 16, 0, Atomo::Suma, 1, 0), 0, "fuera de la vista");
        assert_eq!(u.cargar(Modo::Crudo, 0, 0), [8, 0xFFFF_FFFE, 9, 0]);
        assert_eq!(Atomo::de_dxil(8), Some(Atomo::Cambia));
        assert_eq!(Atomo::de_dxil(9), None);
    }

    /// E2.4: el contador sube devolviendo el de antes, baja devolviendo el
    /// de despues; sin contador, 0.
    #[test]
    fn el_contador_de_un_uav_sube_y_baja_como_en_d3d() {
        let mut b = bytes(&[0; 4]);
        let mut c = 5u32;
        let mut u = Uav { bytes: &mut b, formato: 0, paso: 16, elementos: 1, contador: Some(&mut c), rebanadas: Rebanadas::PLANA };
        assert_eq!((u.contar(1), u.contar(1)), (5, 6));
        assert_eq!(u.contar(-1), 6, "bajar: el de despues");
        drop(u);
        assert_eq!(c, 6);
        let mut sin = Uav { bytes: &mut b, formato: 0, paso: 16, elementos: 1, contador: None, rebanadas: Rebanadas::PLANA };
        assert_eq!(sin.contar(1), 0);
    }
}
