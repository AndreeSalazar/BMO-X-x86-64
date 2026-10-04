//! **Una pila de medida fija**: lo que hace falta para rasterizar una letra
//! SIN `alloc`. El escritorio (Ring 3) no tiene monton, y una letra no
//! necesita uno: sus trazos caben en unos cientos.

/// Hasta `N` elementos; lo que no cabe se cuenta en `perdidos` y no se
/// guarda (una letra a la que le faltan trazos se ve; una escritura fuera de
/// un bufer, no).
pub struct Pila<T: Copy, const N: usize> {
    datos: [T; N],
    n: usize,
    pub perdidos: usize,
}

impl<T: Copy, const N: usize> Pila<T, N> {
    pub const fn nueva(relleno: T) -> Self {
        Pila { datos: [relleno; N], n: 0, perdidos: 0 }
    }

    pub fn push(&mut self, v: T) {
        if self.n < N {
            self.datos[self.n] = v;
            self.n += 1;
        } else {
            self.perdidos += 1;
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.datos[..self.n]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.datos[..self.n]
    }

    pub fn clear(&mut self) {
        self.n = 0;
        self.perdidos = 0;
    }
}
