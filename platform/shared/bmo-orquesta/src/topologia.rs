//! **LOS DIRECTORES SALEN DE LA CPU** (H4.0 de `PLAN_LOS_DOCE_DIRECTORES`,
//! 07-10): quien es hermano SMT de quien, que nucleos comparten L3 y cuales
//! son grandes o chicos -- leido de lo que la CPU dice de si, sin un `if`
//! por modelo y sin un doce escrito.
//!
//! [carril]  VERDE     numeros: identificadores APIC y desplazamientos
//! [cuesta]  DATO -- dos partes en hermanos SMT cuando habia nucleos libres
//!           van a la mitad: no falla nada, va lento sin decirlo.
//!
//! El propietario (07-10): *"que ese core y hilo esten basado que SEAN
//! inteligente es DEPENDIENDO de CPU"*. Todo sale de tres numeros que el
//! kernel lee al arrancar (`plat/smp/topologia.rs`):
//!
//! ```text
//!    los APIC          la MADT: un identificador por hilo logico
//!    smt_bits          cuantos bits bajos del APIC son "que hilo del nucleo"
//!                      (AMD 0x8000_001E, Intel 0x0B nivel 0)
//!    l3_bits           cuantos bits bajos comparten la misma L3
//!                      (AMD 0x8000_001D, Intel 0x04: la cache de nivel 3)
//!    tipo (si hay)     0x1A en un Intel hibrido: 0x40 grande, 0x20 chico
//! ```
//!
//! Con eso: `nucleo = apic >> smt_bits`, `l3 = apic >> l3_bits`. Y el ORDEN
//! en el que se usan los hilos para repartir ([`Fila::orden`]):
//!
//! 1. un hilo por NUCLEO distinto del de quien reparte: los grandes antes
//!    que los chicos, y los de SU L3 antes que los de otra (hablar entre
//!    CCX cuesta un viaje por el Infinity Fabric);
//! 2. despues los hermanos SMT de esos nucleos (comparten L1 y L2: dan algo
//!    mas, no el doble);
//! 3. al final el hermano del nucleo de quien reparte (comparte con el que
//!    ya trabaja).

/// Hasta cuantos hilos logicos (el `MAX` de la MADT del kernel).
pub const HILOS_MAXIMOS: usize = 64;

/// El tipo de nucleo de un Intel hibrido (CPUID 0x1A, EAX[31:24]).
pub const TIPO_GRANDE: u8 = 0x40;
pub const TIPO_CHICO: u8 = 0x20;

/// **Lo que se lee de la CPU.** `tipos[k]` es el del `apics[k]` (0: no se
/// sabe, o todos iguales).
#[derive(Clone, Copy, Debug)]
pub struct Medida<'a> {
    pub apics: &'a [u32],
    pub smt_bits: u8,
    pub l3_bits: u8,
    pub tipos: &'a [u8],
}

/// Un hilo logico, ya situado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hilo {
    pub apic: u32,
    /// El nucleo fisico (`apic >> smt_bits`).
    pub nucleo: u32,
    /// El grupo que comparte L3 (`apic >> l3_bits`).
    pub l3: u32,
    /// Que hilo del nucleo es (0 el primero).
    pub hermano: u32,
    /// Un nucleo chico de un hibrido.
    pub chico: bool,
}

/// **La fila de esta CPU** (la de la tabla de `SMP_MAESTRO.md` seccion 6).
#[derive(Clone, Copy, Debug)]
pub struct Fila {
    hilos: [Hilo; HILOS_MAXIMOS],
    n: usize,
}

/// Los bits que hacen falta para numerar `n` cosas (`n` redondeado a la
/// potencia de dos de arriba): lo que la CPU da en "cuantos comparten".
pub fn bits(n: u32) -> u8 {
    if n <= 1 {
        0
    } else {
        (32 - (n - 1).leading_zeros()) as u8
    }
}

/// `smt_bits` de AMD: CPUID 0x8000_001E, EBX[15:8] + 1 hilos por nucleo.
pub fn smt_bits_amd(ebx_8000_001e: u32) -> u8 {
    bits(((ebx_8000_001e >> 8) & 0xFF) + 1)
}

/// Los bits que comparten una cache, de su descriptor (AMD 0x8000_001D o
/// Intel 0x04, EAX[25:14] + 1: cuantos identificadores la comparten).
pub fn bits_de_cache(eax: u32) -> u8 {
    bits(((eax >> 14) & 0xFFF) + 1)
}

/// El nivel de cache de ese descriptor (EAX[7:5]); 0: ya no hay mas.
pub fn nivel_de_cache(eax: u32) -> u32 {
    (eax >> 5) & 0x7
}

impl Fila {
    /// **Leer la fila** de lo medido. Los repetidos cuentan una vez.
    pub fn leer(m: &Medida) -> Fila {
        let mut f = Fila { hilos: [Hilo::default(); HILOS_MAXIMOS], n: 0 };
        let smt = m.smt_bits.min(31);
        let l3 = m.l3_bits.min(31).max(smt);
        for (k, &apic) in m.apics.iter().enumerate() {
            if f.n == HILOS_MAXIMOS || f.hilos[..f.n].iter().any(|h| h.apic == apic) {
                continue;
            }
            let tipo = m.tipos.get(k).copied().unwrap_or(0);
            f.hilos[f.n] = Hilo { apic, nucleo: apic >> smt, l3: apic >> l3, hermano: apic & ((1 << smt) - 1), chico: tipo == TIPO_CHICO };
            f.n += 1;
        }
        f
    }

    pub fn hilos(&self) -> &[Hilo] {
        &self.hilos[..self.n]
    }

    fn distintos(&self, clave: impl Fn(&Hilo) -> u32) -> u32 {
        let h = self.hilos();
        h.iter().enumerate().filter(|(i, x)| !h[..*i].iter().any(|y| clave(y) == clave(x))).count() as u32
    }

    /// Nucleos fisicos.
    pub fn nucleos(&self) -> u32 {
        self.distintos(|h| h.nucleo)
    }

    /// Grupos que comparten L3 (un 5600X: 1; un Zen 2 de dos CCX: 2).
    pub fn l3s(&self) -> u32 {
        self.distintos(|h| h.l3)
    }

    /// Nucleos chicos (de un hibrido).
    pub fn chicos(&self) -> u32 {
        let h = self.hilos();
        h.iter().enumerate().filter(|(i, x)| x.chico && !h[..*i].iter().any(|y| y.nucleo == x.nucleo)).count() as u32
    }

    fn de(&self, apic: u32) -> Option<Hilo> {
        self.hilos().iter().copied().find(|h| h.apic == apic)
    }

    /// **El orden en el que se usan los hilos** para repartir desde `bsp`
    /// (que ya trabaja: no sale). Ver la cabecera. Escribe los APIC en
    /// `sale` y devuelve cuantos.
    pub fn orden(&self, bsp: u32, sale: &mut [u32]) -> usize {
        let yo = self.de(bsp);
        let mut puestos = 0usize;
        // La clave: (ronda, chico, otra L3, nucleo, hermano). Ronda 0: el
        // primer hilo de cada nucleo ajeno; 1: los demas de nucleos ajenos;
        // 2: los del nucleo de `bsp`.
        let clave = |h: &Hilo, primero: bool| -> (u32, bool, bool, u32, u32) {
            let mio = yo.is_some_and(|y| y.nucleo == h.nucleo);
            let ronda = if mio { 2 } else if primero { 0 } else { 1 };
            (ronda, h.chico, yo.is_some_and(|y| y.l3 != h.l3), h.nucleo, h.hermano)
        };
        // El "primero" de cada nucleo: el de menor `hermano` de los que hay.
        let es_primero = |h: &Hilo| !self.hilos().iter().any(|o| o.nucleo == h.nucleo && o.hermano < h.hermano);
        let mut usados = [false; HILOS_MAXIMOS];
        while puestos < sale.len() {
            let mut mejor: Option<(usize, (u32, bool, bool, u32, u32))> = None;
            for (i, h) in self.hilos().iter().enumerate() {
                if usados[i] || h.apic == bsp {
                    continue;
                }
                let c = clave(h, es_primero(h));
                if mejor.is_none_or(|(_, m)| c < m) {
                    mejor = Some((i, c));
                }
            }
            let Some((i, _)) = mejor else { break };
            usados[i] = true;
            sale[puestos] = self.hilos[i].apic;
            puestos += 1;
        }
        puestos
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn orden(f: &Fila, bsp: u32) -> Vec<u32> {
        let mut v = [0u32; HILOS_MAXIMOS];
        let n = f.orden(bsp, &mut v);
        v[..n].to_vec()
    }

    /// *** EL 5600X DE HOY: 6 nucleos x 2, una L3, y APIC con HUECO (el CCD
    /// tiene sitio para 8: los 6 vivos no son seguidos).
    #[test]
    fn el_5600x_seis_nucleos_doce_hilos_una_l3() {
        let apics = [0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13];
        let f = Fila::leer(&Medida { apics: &apics, smt_bits: smt_bits_amd(1 << 8), l3_bits: 4, tipos: &[] });
        assert_eq!((f.hilos().len(), f.nucleos(), f.l3s(), f.chicos()), (12, 6, 1, 0));
        let o = orden(&f, 0);
        assert_eq!(o.len(), 11, "todos menos el BSP");
        assert_eq!(&o[..5], &[2, 4, 8, 10, 12], "primero un hilo por cada uno de los otros 5 nucleos");
        assert_eq!(&o[5..10], &[3, 5, 9, 11, 13], "despues sus hermanos SMT");
        assert_eq!(o[10], 1, "y al final el hermano del BSP");
    }

    /// Un Zen 2 de dos CCX (3+3, una L3 cada uno): primero los nucleos de la
    /// L3 del BSP, despues los de la otra.
    #[test]
    fn un_zen2_de_dos_ccx_usa_primero_su_l3() {
        let apics = [0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13];
        // 8 identificadores comparten cada L3 (0x8000_001D: EAX[25:14] = 7).
        let f = Fila::leer(&Medida { apics: &apics, smt_bits: 1, l3_bits: bits_de_cache(7 << 14), tipos: &[] });
        assert_eq!((f.nucleos(), f.l3s()), (6, 2));
        let o = orden(&f, 0);
        assert_eq!(&o[..5], &[2, 4, 8, 10, 12], "los dos de su CCX (2, 4) antes que los tres del otro");
        let o = orden(&f, 10);
        assert_eq!(&o[..5], &[8, 12, 0, 2, 4], "desde el otro CCX, al reves");
    }

    /// Un Intel hibrido: 6 grandes con HT y 4 chicos sin HT. Primero los
    /// grandes, luego los chicos, luego los hermanos de los grandes.
    #[test]
    fn un_hibrido_usa_primero_los_grandes() {
        let apics = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 16, 18, 20, 22];
        let mut tipos = [TIPO_GRANDE; 16];
        for t in &mut tipos[12..] {
            *t = TIPO_CHICO;
        }
        let f = Fila::leer(&Medida { apics: &apics, smt_bits: 1, l3_bits: 6, tipos: &tipos });
        assert_eq!((f.hilos().len(), f.nucleos(), f.chicos(), f.l3s()), (16, 10, 4, 1));
        let o = orden(&f, 0);
        assert_eq!(&o[..5], &[2, 4, 6, 8, 10], "los cinco grandes ajenos");
        assert_eq!(&o[5..9], &[16, 18, 20, 22], "los cuatro chicos");
        assert_eq!(&o[9..14], &[3, 5, 7, 9, 11], "los hermanos de los grandes");
        assert_eq!(o[14], 1);
    }

    /// Sin SMT y sin datos de cache: cada hilo es su nucleo, todos una L3.
    #[test]
    fn sin_smt_cada_hilo_es_un_nucleo() {
        let f = Fila::leer(&Medida { apics: &[0, 1, 2, 3, 3], smt_bits: 0, l3_bits: 0, tipos: &[] });
        assert_eq!((f.hilos().len(), f.nucleos()), (4, 4), "el repetido cuenta una vez");
        assert_eq!(orden(&f, 2), vec![0, 1, 3]);
    }

    #[test]
    fn los_bits_de_lo_que_dice_cpuid() {
        assert_eq!((bits(1), bits(2), bits(3), bits(8), bits(12), bits(16)), (0, 1, 2, 3, 4, 4));
        assert_eq!(smt_bits_amd(0x0000_0100), 1, "EBX[15:8] = 1: dos hilos por nucleo");
        assert_eq!(smt_bits_amd(0), 0);
        let l3 = 3 << 5 | 15 << 14;
        assert_eq!((nivel_de_cache(l3), bits_de_cache(l3)), (3, 4));
    }
}
