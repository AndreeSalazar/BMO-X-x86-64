//! **EL FORMATO DE LA MMU DE LA 3060** -- las entradas de las tablas de
//! paginas (el formato "v2" de Pascal, que usan Turing y Ampere), y como parte
//! una direccion virtual. Lo que L1d escribira en la VRAM para que la GPU vea
//! sus colas y sus datos.
//!
//! capa: puro -- bits y cuentas; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- un bit de apertura corrido y la GPU lee de la RAM
//!           del PC en vez de su VRAM (o al reves), en la direccion equivocada
//!
//! # Los niveles (nouveau `vmmgp100.c`, `gp100_vmm_desc_12`)
//!
//! ```text
//!    VA de 49 bits, paginas de 4 KiB:
//!    PD3  bits 48..47    4 entradas de 8 B    (la raiz de L1c3)
//!    PD2  bits 46..38  512 entradas de 8 B
//!    PD1  bits 37..29  512 entradas de 8 B
//!    PD0  bits 28..21  256 entradas de 16 B: [tabla de 64 KiB, tabla de 4 KiB]
//!    PT   bits 20..12  512 entradas de 8 B    (las paginas)
//! ```
//!
//! # Las entradas
//!
//! ```text
//!    PDE (PD3, PD2, PD1, y cada mitad de PD0):
//!         bits 2..1  apertura: 0 NADA, 1 VRAM, 2 sistema coherente (+ bit 3
//!                    VOL), 3 sistema no coherente
//!         bits 63..8 la direccion de la tabla de abajo >> 4 (alineada a 4 KiB)
//!    PTE:
//!         bit 0      VALIDA
//!         bits 2..1  apertura: 0 VRAM, 2 sistema coherente, 3 no coherente
//!         bit 3 VOL, bit 5 PRIV, bit 6 SOLO LECTURA, bits 63..56 kind
//!         bits 55..8 la direccion de la pagina >> 4
//! ```
//!
//! [!] La apertura de una PDE y la de una PTE NO cuentan igual: en la PDE la
//! VRAM es el 1 (el 0 es "no hay tabla"); en la PTE la VRAM es el 0 (y la
//! validez va en su propio bit). Mezclarlas es el fallo que esto evita.

/// Los indices de `va` en cada nivel: `[PD3, PD2, PD1, PD0, PT]`.
pub const fn indices(va: u64) -> [usize; 5] {
    [
        ((va >> 47) & 0x3) as usize,
        ((va >> 38) & 0x1FF) as usize,
        ((va >> 29) & 0x1FF) as usize,
        ((va >> 21) & 0xFF) as usize,
        ((va >> 12) & 0x1FF) as usize,
    ]
}

/// Una PDE que apunta a una tabla en VRAM.
pub const fn pde_vram(tabla: u64) -> u64 {
    1 << 1 | tabla >> 4
}

/// Una PTE valida de una pagina de 4 KiB en VRAM (kind 0, lectura y escritura).
pub const fn pte_vram(pagina: u64) -> u64 {
    1 | pagina >> 4
}

/// El bit PRIV de una PTE: solo la ven los privilegiados (el FECS y el
/// GPCCS que guardan el contexto), no un sombreador. nouveau mapea asi los
/// buferes de contexto (`gf100_vmm_map_v0 { .priv = 1 }`).
pub const PTE_PRIV: u64 = 1 << 5;

/// Lo que dice una PDE leida.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pde {
    Vacia,
    Vram(u64),
    Sistema(u64),
}

/// **Leer una PDE** (de PD3, PD2, PD1 o media PD0).
pub const fn pde(v: u64) -> Pde {
    let dir = (v >> 8) << 12;
    match (v >> 1) & 3 {
        0 => Pde::Vacia,
        1 => Pde::Vram(dir),
        _ => Pde::Sistema(dir),
    }
}

/// Una escritura en la VRAM: `(direccion, valor de 64 bits)`.
pub type Escritura = (u64, u64);

/// **Mapear UNA pagina** de 4 KiB, `va -> pagina` (en VRAM), con tablas
/// NUEVAS y nuestras bajo la raiz `pd3`: `tablas` son las cuatro paginas de
/// VRAM (a cero) para PD2, PD1, PD0 y PT. Devuelve las cinco escrituras, de
/// la hoja a la raiz -- asi la GPU no ve nunca un camino a medias.
///
/// Solo vale si la entrada de la raiz para `va` esta VACIA: si el RM ya colgo
/// ahi una PD2 suya, esto no la pisa (lo comprueba quien llama, leyendo).
pub const fn mapear(pd3: u64, tablas: [u64; 4], va: u64, pagina: u64) -> [Escritura; 5] {
    let i = indices(va);
    let [pd2, pd1, pd0, pt] = tablas;
    [
        (pt + 8 * i[4] as u64, pte_vram(pagina)),
        // PD0: la mitad de 4 KiB es la segunda (+8); la de 64 KiB, vacia.
        (pd0 + 16 * i[3] as u64 + 8, pde_vram(pt)),
        (pd1 + 8 * i[2] as u64, pde_vram(pd0)),
        (pd2 + 8 * i[1] as u64, pde_vram(pd1)),
        (pd3 + 8 * i[0] as u64, pde_vram(pd2)),
    ]
}

/// Lo mas que mapea [`mapear_tramo`] de una vez.
pub const MAX_TRAMO: usize = 64;

/// **Mapear un TRAMO**: `n` paginas seguidas, `va.. -> pagina0..`, dentro de
/// una sola PT (no cruza un limite de 2 MiB). Las `n` PTE primero y las
/// cuatro PDE despues, de la hoja a la raiz. Devuelve las escrituras y cuantas
/// son; `None` si el tramo no cabe o no esta alineado a 4 KiB.
pub fn mapear_tramo(pd3: u64, tablas: [u64; 4], va: u64, pagina0: u64, n: usize) -> Option<([Escritura; MAX_TRAMO + 4], usize)> {
    let pt_i = indices(va)[4];
    if n == 0 || n > MAX_TRAMO || pt_i + n > 512 || va & 0xFFF != 0 || pagina0 & 0xFFF != 0 {
        return None;
    }
    let mut e = [(0u64, 0u64); MAX_TRAMO + 4];
    let pt = tablas[3];
    for k in 0..n {
        e[k] = (pt + 8 * (pt_i + k) as u64, pte_vram(pagina0 + 4096 * k as u64));
    }
    let una = mapear(pd3, tablas, va, pagina0);
    // Las cuatro PDE de `mapear` (la PTE ya va arriba).
    e[n..n + 4].copy_from_slice(&una[1..5]);
    Some((e, n + 4))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_indices_de_una_va() {
        assert_eq!(indices(0), [0; 5]);
        // 4 GiB: donde el RM se reserva su zona (`SPLIT_VAS_SERVER_RM_MANAGED_VA_START`).
        assert_eq!(indices(0x1_0000_0000), [0, 0, 8, 0, 0]);
        assert_eq!(indices(0x1000), [0, 0, 0, 0, 1]);
        assert_eq!(indices(0x20_0000), [0, 0, 0, 1, 0]);
        assert_eq!(indices(1 << 47), [1, 0, 0, 0, 0]);
        assert_eq!(indices(u64::MAX >> 15), [3, 511, 511, 255, 511]);
    }

    #[test]
    fn pde_y_pte_no_cuentan_igual() {
        // En la PDE la VRAM es el 1; en la PTE el 0, y la validez va aparte.
        assert_eq!(pde_vram(0x0410_1000), 0x2 | 0x0041_0100);
        assert_eq!(pte_vram(0x0420_0000), 0x1 | 0x0042_0000);
        assert_eq!(pde(pde_vram(0x2F3C_2D000)), Pde::Vram(0x2F3C_2D000));
        assert_eq!(pde(0), Pde::Vacia);
        assert_eq!(pde(2 << 1 | 1 << 3 | 0x1234_0000 >> 4), Pde::Sistema(0x1234_0000));
    }

    #[test]
    fn una_pagina_se_mapea_de_la_hoja_a_la_raiz() {
        let raiz = 0x0410_0000;
        let t = [0x0410_1000, 0x0410_2000, 0x0410_3000, 0x0410_4000];
        let e = mapear(raiz, t, 0x2_0000_3000, 0x0420_0000);
        let i = indices(0x2_0000_3000);
        assert_eq!(i, [0, 0, 16, 0, 3]);
        assert_eq!(e[0], (t[3] + 8 * 3, pte_vram(0x0420_0000)), "la PTE, primero");
        assert_eq!(e[1], (t[2] + 8, pde_vram(t[3])), "PD0: la mitad de 4 KiB");
        assert_eq!(e[2], (t[1] + 8 * 16, pde_vram(t[2])));
        assert_eq!(e[3], (t[0], pde_vram(t[1])));
        assert_eq!(e[4], (raiz, pde_vram(t[0])), "la raiz, la ULTIMA");
    }

    #[test]
    fn un_tramo_de_dieciseis_paginas() {
        let raiz = 0x0410_0000;
        let t = [0x0410_1000, 0x0410_2000, 0x0410_3000, 0x0410_4000];
        let (e, n) = mapear_tramo(raiz, t, 0x2_0000_0000, 0x0420_0000, 16).unwrap();
        assert_eq!(n, 20);
        for k in 0..16 {
            assert_eq!(e[k], (t[3] + 8 * k as u64, pte_vram(0x0420_0000 + 4096 * k as u64)));
        }
        assert_eq!(e[19], (raiz, pde_vram(t[0])), "la raiz, la ULTIMA");
        assert!(mapear_tramo(raiz, t, 0x2_001F_F000, 0x0420_0000, 2).is_none(), "cruza 2 MiB");
        assert!(mapear_tramo(raiz, t, 0x2_0000_0800, 0x0420_0000, 1).is_none(), "sin alinear");
    }
}
