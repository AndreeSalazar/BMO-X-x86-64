//! **Una serie**: las ultimas `N` muestras de un numero, para una grafica
//! chica (la CPU, la memoria, los vatios del administrador de tareas).

/// Las ultimas `N` muestras, de la mas vieja a la mas nueva.
#[derive(Clone, Copy)]
pub struct Serie<const N: usize> {
    muestras: [u64; N],
    /// Cuantas hay (hasta `N`).
    cuantas: usize,
    /// Donde va la siguiente.
    siguiente: usize,
}

impl<const N: usize> Serie<N> {
    pub const fn nueva() -> Self {
        Serie { muestras: [0; N], cuantas: 0, siguiente: 0 }
    }

    pub fn poner(&mut self, v: u64) {
        if N == 0 {
            return;
        }
        self.muestras[self.siguiente] = v;
        self.siguiente = (self.siguiente + 1) % N;
        self.cuantas = (self.cuantas + 1).min(N);
    }

    pub fn cuantas(&self) -> usize {
        self.cuantas
    }

    /// La muestra `i`, contando desde la mas vieja que queda.
    pub fn muestra(&self, i: usize) -> Option<u64> {
        (i < self.cuantas).then(|| self.muestras[(self.siguiente + N - self.cuantas + i) % N])
    }

    pub fn ultima(&self) -> Option<u64> {
        self.cuantas.checked_sub(1).and_then(|i| self.muestra(i))
    }

    /// La mayor de las que hay (0 sin ninguna).
    pub fn maximo(&self) -> u64 {
        (0..self.cuantas).filter_map(|i| self.muestra(i)).max().unwrap_or(0)
    }

    /// **La altura de cada columna** de una grafica de `alto` pixeles con
    /// `techo` arriba (`0`: el maximo de la serie): de la mas vieja a la mas
    /// nueva, una por muestra. Lo de encima del techo se sujeta al techo.
    pub fn alturas(&self, alto: u32, techo: u64, dst: &mut [u32]) -> usize {
        let techo = if techo == 0 { self.maximo().max(1) } else { techo };
        let n = self.cuantas.min(dst.len());
        let desde = self.cuantas - n;
        for (k, d) in dst.iter_mut().take(n).enumerate() {
            let v = self.muestra(desde + k).unwrap_or(0).min(techo);
            *d = ((v as u128 * alto as u128 + techo as u128 / 2) / techo as u128) as u32;
        }
        n
    }
}

impl<const N: usize> Default for Serie<N> {
    fn default() -> Self {
        Self::nueva()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn guarda_las_ultimas_y_las_escala() {
        let mut s = Serie::<4>::nueva();
        assert_eq!((s.cuantas(), s.ultima(), s.maximo()), (0, None, 0));
        for v in [10, 20, 30, 40, 50, 60] {
            s.poner(v);
        }
        assert_eq!((0..4).map(|i| s.muestra(i).unwrap()).collect::<Vec<_>>(), [30, 40, 50, 60]);
        assert_eq!((s.ultima(), s.maximo()), (Some(60), 60));
        let mut a = [0u32; 8];
        // Techo 100 en 10 pixeles: 3, 4, 5, 6.
        assert_eq!(s.alturas(10, 100, &mut a), 4);
        assert_eq!(&a[..4], &[3, 4, 5, 6]);
        // Sin techo: el maximo arriba; con menos sitio, las MAS NUEVAS.
        let mut b = [0u32; 2];
        assert_eq!(s.alturas(6, 0, &mut b), 2);
        assert_eq!(b, [5, 6]);
        // Por encima del techo, sujeta.
        s.poner(500);
        s.alturas(10, 100, &mut b);
        assert_eq!(b[1], 10);
    }
}
