//! **LAS TABLAS DE PAGINA DE UN DOMINIO** -- lo que un aparato ve de la RAM
//! cuando su entrada es TRADUCIDA (M0d de `docs/plan/PLAN_LA_3060.md`).
//!
//! # Por que existe (2026-09-24)
//!
//! Hasta M0e una entrada podia ser DE PASO (el aparato ve toda la RAM) o
//! BLOQUEADA (no ve nada). Para arrancar el GSP de la 3060 hace falta la
//! tercera: que vea SOLO lo que BMO-X le presta -- su firmware, sus colas --
//! y nada mas. Eso es un dominio con sus tablas de pagina, y esas tablas las
//! lee la IOMMU de la RAM, asi que su formato no se inventa.
//!
//! # El formato (Linux, `drivers/iommu/amd/amd_iommu_types.h` y
//! `io_pgtable.c`, el "v1"), leido el 24-09
//!
//! ```text
//!    una tabla = una pagina de 4 KiB = 512 entradas de 64 bits
//!    indice en el nivel n (1..=6): (iova >> (12 + 9*(n-1))) & 0x1FF
//!
//!    DIRECTORIO (PM_LEVEL_PDE)   PR | IR | IW | nivel_siguiente << 9 | fisica
//!    HOJA de 4 KiB               PR | FC | IR? | IW? | fisica   (nivel 0)
//!
//!    PR bit 0   presente          FC bit 60  forzar coherencia
//!    IR bit 61  se puede leer     IW bit 62  se puede escribir
//! ```
//!
//! Con [`NIVELES`] = 3 el espacio del aparato mide 2^39 = 512 GiB: sobra para
//! el firmware del GSP (~69 MB). La raiz va en la entrada del dispositivo con
//! el mismo numero de niveles (`Dte::traducida`).
//!
//! # El ORACULO
//!
//! [`Dominio::traducir`] recorre las tablas COMO LA IOMMU: desde la raiz, nivel
//! a nivel, mirando PR, el nivel siguiente y los permisos de cada peldano. Las
//! pruebas no comparan con lo que el codigo cree haber escrito: comparan con lo
//! que el hardware leeria. Y el kernel lo usa igual, sobre la RAM de verdad,
//! para RELEER lo que presto.
//!
//! # Lo que NO hace, a proposito
//!
//! Solo paginas de 4 KiB (nada de paginas grandes ni saltos de nivel), y no
//! devuelve las tablas intermedias al quitar: un dominio de la 3060 crece hasta
//! lo que el GSP pida y se queda asi. Mas simple es mas facil de releer.

use crate::tablas::Dte;

/// Niveles del dominio: 3 = 39 bits de direccion del aparato.
pub const NIVELES: u8 = 3;
/// Lo mas alto que se puede prestar (exclusivo).
pub const IOVA_TOPE: u64 = 1 << (12 + 9 * NIVELES as u32);
pub const PAGINA: u64 = 4096;
pub const ENTRADAS: usize = 512;

const PR: u64 = 1 << 0;
const NIVEL_SHIFT: u32 = 9;
const NIVEL_MASK: u64 = 0x7 << NIVEL_SHIFT;
const FC: u64 = 1 << 60;
const IR: u64 = 1 << 61;
const IW: u64 = 1 << 62;
const FISICA_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// El indice de `iova` en una tabla de nivel `nivel` (1 = la de las hojas).
pub const fn indice(nivel: u8, iova: u64) -> usize {
    ((iova >> (12 + 9 * (nivel as u32 - 1))) & 0x1FF) as usize
}

/// Una entrada de DIRECTORIO: apunta a la tabla de `nivel_siguiente`.
pub const fn directorio(tabla: u64, nivel_siguiente: u8) -> u64 {
    PR | IR | IW | (nivel_siguiente as u64) << NIVEL_SHIFT | (tabla & FISICA_MASK)
}

/// Una HOJA de 4 KiB: la pagina fisica y lo que se puede hacer con ella.
pub const fn hoja(fisica: u64, lee: bool, escribe: bool) -> u64 {
    PR | FC | (fisica & FISICA_MASK) | if lee { IR } else { 0 } | if escribe { IW } else { 0 }
}

/// **La memoria donde viven las tablas.** El kernel la da por el physmap; las
/// pruebas, con un banco de paginas de mentira.
pub trait Memoria {
    /// Una pagina NUEVA a ceros para una tabla: su direccion fisica.
    fn nueva(&mut self) -> Option<u64>;
    fn leer(&self, tabla: u64, i: usize) -> u64;
    fn escribir(&mut self, tabla: u64, i: usize, v: u64);
}

/// Por que no se presto.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoPresta {
    /// La direccion del aparato o la fisica no van a pagina.
    Desalineada,
    /// Se sale de [`IOVA_TOPE`], o son cero paginas.
    FueraDelEspacio,
    /// No quedan paginas para tablas.
    SinTablas,
    /// Alguna de esas paginas del aparato ya estaba prestada. No se pisa.
    YaPrestada,
}

/// Lo que ve el aparato en una direccion, segun el oraculo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vista {
    pub fisica: u64,
    pub lee: bool,
    pub escribe: bool,
}

/// **Un dominio**: su raiz. Todo lo demas vive en la [`Memoria`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dominio {
    pub raiz: u64,
}

impl Dominio {
    /// Un dominio VACIO: la raiz a ceros. Un aparato con esta entrada no ve
    /// nada -- como BLOQUEADA, pero por el camino que despues prestara.
    pub fn nuevo(m: &mut impl Memoria) -> Option<Dominio> {
        Some(Dominio { raiz: m.nueva()? })
    }

    /// La entrada de dispositivo que traduce por este dominio.
    pub const fn entrada(&self, id: u16) -> Option<Dte> {
        Dte::traducida(id, self.raiz, NIVELES)
    }

    /// **PRESTAR** `paginas` paginas: el aparato vera `iova..` como `fisica..`.
    ///
    /// Todo o nada: si alguna ya estaba prestada o faltan tablas a mitad, lo
    /// que ESTA llamada habia puesto se quita antes de volver.
    pub fn prestar(
        &self,
        m: &mut impl Memoria,
        iova: u64,
        fisica: u64,
        paginas: u64,
        lee: bool,
        escribe: bool,
    ) -> Result<(), NoPresta> {
        if iova % PAGINA != 0 || fisica % PAGINA != 0 || fisica & !FISICA_MASK != 0 {
            return Err(NoPresta::Desalineada);
        }
        let fin = paginas.checked_mul(PAGINA).and_then(|b| iova.checked_add(b));
        if paginas == 0 || fin.map_or(true, |f| f > IOVA_TOPE) {
            return Err(NoPresta::FueraDelEspacio);
        }
        for k in 0..paginas {
            let a = iova + k * PAGINA;
            let r = match self.tabla_de_hojas(m, a) {
                None => Err(NoPresta::SinTablas),
                Some(t) if m.leer(t, indice(1, a)) & PR != 0 => Err(NoPresta::YaPrestada),
                Some(t) => {
                    m.escribir(t, indice(1, a), hoja(fisica + k * PAGINA, lee, escribe));
                    Ok(())
                }
            };
            if let Err(e) = r {
                self.quitar(m, iova, k);
                return Err(e);
            }
        }
        Ok(())
    }

    /// **QUITAR** `paginas` paginas desde `iova`. Devuelve cuantas estaban
    /// prestadas. Tras quitar, quien llama INVALIDA (la IOMMU guarda en cache).
    pub fn quitar(&self, m: &mut impl Memoria, iova: u64, paginas: u64) -> u64 {
        let mut n = 0;
        for k in 0..paginas {
            let a = iova + k * PAGINA;
            if a >= IOVA_TOPE {
                break;
            }
            if let Some(t) = self.buscar_hojas(m, a) {
                if m.leer(t, indice(1, a)) & PR != 0 {
                    m.escribir(t, indice(1, a), 0);
                    n += 1;
                }
            }
        }
        n
    }

    /// **EL ORACULO**: lo que la IOMMU daria para `iova`, recorriendo la
    /// tabla como ella. `None` = FALLO DE PAGINA (el aparato no ve eso).
    pub fn traducir(&self, m: &impl Memoria, iova: u64) -> Option<Vista> {
        if iova >= IOVA_TOPE {
            return None;
        }
        let mut tabla = self.raiz;
        let (mut lee, mut escribe) = (true, true);
        let mut nivel = NIVELES;
        loop {
            let e = m.leer(tabla, indice(nivel, iova));
            if e & PR == 0 {
                return None;
            }
            // Los permisos son el AND de todos los peldanos.
            lee &= e & IR != 0;
            escribe &= e & IW != 0;
            let siguiente = ((e & NIVEL_MASK) >> NIVEL_SHIFT) as u8;
            if nivel == 1 {
                // Una hoja de 4 KiB lleva nivel 0; otra cosa aqui no la sabe
                // leer este oraculo y no se da por buena.
                if siguiente != 0 {
                    return None;
                }
                return Some(Vista { fisica: (e & FISICA_MASK) | (iova & (PAGINA - 1)), lee, escribe });
            }
            if siguiente != nivel - 1 {
                return None;
            }
            tabla = e & FISICA_MASK;
            nivel -= 1;
        }
    }

    /// La tabla de nivel 1 de `iova`, creando los directorios que falten.
    fn tabla_de_hojas(&self, m: &mut impl Memoria, iova: u64) -> Option<u64> {
        let mut tabla = self.raiz;
        let mut nivel = NIVELES;
        while nivel > 1 {
            let i = indice(nivel, iova);
            let e = m.leer(tabla, i);
            tabla = if e & PR != 0 {
                e & FISICA_MASK
            } else {
                let nueva = m.nueva()?;
                m.escribir(tabla, i, directorio(nueva, nivel - 1));
                nueva
            };
            nivel -= 1;
        }
        Some(tabla)
    }

    /// La tabla de nivel 1 de `iova`, SIN crear nada.
    fn buscar_hojas(&self, m: &impl Memoria, iova: u64) -> Option<u64> {
        let mut tabla = self.raiz;
        let mut nivel = NIVELES;
        while nivel > 1 {
            let e = m.leer(tabla, indice(nivel, iova));
            if e & PR == 0 {
                return None;
            }
            tabla = e & FISICA_MASK;
            nivel -= 1;
        }
        Some(tabla)
    }
}

// ===================================================================
//  PRUEBAS
// ===================================================================

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un banco de paginas: la "fisica" de la pagina k es `BASE + k * 4096`.
    struct Banco {
        paginas: Vec<[u64; ENTRADAS]>,
        tope: usize,
    }
    const BASE: u64 = 0x0100_0000;

    impl Banco {
        fn nuevo(tope: usize) -> Self {
            Banco { paginas: Vec::new(), tope }
        }
        fn k(t: u64) -> usize {
            ((t - BASE) / PAGINA) as usize
        }
    }

    impl Memoria for Banco {
        fn nueva(&mut self) -> Option<u64> {
            if self.paginas.len() >= self.tope {
                return None;
            }
            self.paginas.push([0; ENTRADAS]);
            Some(BASE + (self.paginas.len() as u64 - 1) * PAGINA)
        }
        fn leer(&self, t: u64, i: usize) -> u64 {
            self.paginas[Self::k(t)][i]
        }
        fn escribir(&mut self, t: u64, i: usize, v: u64) {
            self.paginas[Self::k(t)][i] = v;
        }
    }

    const IOVA: u64 = 0x1000_0000;
    const FIS: u64 = 0x7654_3000;

    #[test]
    fn un_dominio_nuevo_no_ve_nada() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        for a in [0, IOVA, IOVA_TOPE - PAGINA, IOVA_TOPE] {
            assert_eq!(d.traducir(&b, a), None, "{a:#x}");
        }
        assert_eq!(b.paginas.len(), 1, "vacio es UNA pagina: la raiz");
    }

    #[test]
    fn prestar_una_pagina_y_nada_mas() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        d.prestar(&mut b, IOVA, FIS, 1, true, true).unwrap();
        assert_eq!(d.traducir(&b, IOVA), Some(Vista { fisica: FIS, lee: true, escribe: true }));
        assert_eq!(d.traducir(&b, IOVA + 0x123).unwrap().fisica, FIS + 0x123);
        assert_eq!(d.traducir(&b, IOVA + PAGINA), None, "la de al lado NO se presto");
        assert_eq!(d.traducir(&b, IOVA - PAGINA), None);
        assert_eq!(b.paginas.len(), 3, "raiz + un directorio + una tabla de hojas");
    }

    #[test]
    fn solo_lectura_es_solo_lectura() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        d.prestar(&mut b, IOVA, FIS, 1, true, false).unwrap();
        assert_eq!(d.traducir(&b, IOVA), Some(Vista { fisica: FIS, lee: true, escribe: false }));
    }

    #[test]
    fn un_prestamo_que_cruza_2_mib_y_1_gib() {
        let mut b = Banco::nuevo(16);
        let d = Dominio::nuevo(&mut b).unwrap();
        let iova = (1 << 30) - 3 * PAGINA; // cruza el GiB y con el los 2 MiB
        d.prestar(&mut b, iova, FIS, 6, true, true).unwrap();
        for k in 0..6 {
            assert_eq!(d.traducir(&b, iova + k * PAGINA).unwrap().fisica, FIS + k * PAGINA);
        }
        assert_eq!(d.traducir(&b, iova + 6 * PAGINA), None);
        assert_eq!(b.paginas.len(), 5, "raiz + 2 directorios + 2 tablas de hojas");
    }

    #[test]
    fn no_se_pisa_un_prestamo_y_lo_de_la_llamada_fallida_se_quita() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        d.prestar(&mut b, IOVA + 2 * PAGINA, 0xAAAA_0000, 1, true, true).unwrap();
        assert_eq!(d.prestar(&mut b, IOVA, FIS, 4, true, true), Err(NoPresta::YaPrestada));
        assert_eq!(d.traducir(&b, IOVA), None, "la primera de la llamada fallida sigue puesta");
        assert_eq!(d.traducir(&b, IOVA + PAGINA), None);
        assert_eq!(d.traducir(&b, IOVA + 2 * PAGINA).unwrap().fisica, 0xAAAA_0000, "lo de antes no se toca");
    }

    #[test]
    fn sin_tablas_se_dice_y_no_queda_nada_a_medias() {
        let mut b = Banco::nuevo(4);
        let d = Dominio::nuevo(&mut b).unwrap();
        // 1 GiB - 1 pagina: la segunda necesita un directorio y una tabla mas
        // de las que caben.
        let iova = (1 << 30) - PAGINA;
        assert_eq!(d.prestar(&mut b, iova, FIS, 2, true, true), Err(NoPresta::SinTablas));
        assert_eq!(d.traducir(&b, iova), None);
    }

    #[test]
    fn lo_que_no_va_a_pagina_o_se_sale_no_se_presta() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        assert_eq!(d.prestar(&mut b, IOVA + 1, FIS, 1, true, true), Err(NoPresta::Desalineada));
        assert_eq!(d.prestar(&mut b, IOVA, FIS + 8, 1, true, true), Err(NoPresta::Desalineada));
        assert_eq!(d.prestar(&mut b, IOVA_TOPE - PAGINA, FIS, 2, true, true), Err(NoPresta::FueraDelEspacio));
        assert_eq!(d.prestar(&mut b, IOVA, FIS, 0, true, true), Err(NoPresta::FueraDelEspacio));
        assert_eq!(d.prestar(&mut b, IOVA, FIS, u64::MAX, true, true), Err(NoPresta::FueraDelEspacio));
    }

    #[test]
    fn quitar_devuelve_la_ceguera() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        d.prestar(&mut b, IOVA, FIS, 3, true, true).unwrap();
        assert_eq!(d.quitar(&mut b, IOVA, 5), 3, "de cinco, tres estaban prestadas");
        for k in 0..3 {
            assert_eq!(d.traducir(&b, IOVA + k * PAGINA), None);
        }
        d.prestar(&mut b, IOVA, FIS, 3, true, true).expect("quitado, se puede volver a prestar");
    }

    #[test]
    fn el_formato_es_el_de_linux() {
        // PM_LEVEL_PDE(2, t): PR | IR | IW | 2 << 9 | t
        assert_eq!(directorio(0x1234_5000, 2), 1 | 1 << 61 | 1 << 62 | 2 << 9 | 0x1234_5000);
        // una hoja de 4 KiB: PR | FC | IR | IW | fisica, nivel 0
        assert_eq!(hoja(0x8000, true, true), 1 | 1 << 60 | 1 << 61 | 1 << 62 | 0x8000);
        assert_eq!(hoja(0x8000, false, false), 1 | 1 << 60 | 0x8000);
        assert_eq!(indice(1, 0x1000_0000), 0);
        assert_eq!(indice(2, 0x1000_0000), 0x80);
        assert_eq!(indice(3, 0x1_0000_0000), 4);
        let mut b = Banco::nuevo(2);
        let d = Dominio::nuevo(&mut b).unwrap();
        let e = d.entrada(3).unwrap();
        assert!(e.valida() && e.traduce() && e.lee() && e.escribe());
        assert_eq!((e.niveles(), e.raiz(), e.dominio()), (3, BASE, 3));
    }

    /// El oraculo no se cree una hoja donde el hardware no la veria.
    #[test]
    fn el_oraculo_no_acepta_lo_que_no_sabe_leer() {
        let mut b = Banco::nuevo(8);
        let d = Dominio::nuevo(&mut b).unwrap();
        d.prestar(&mut b, IOVA, FIS, 1, true, true).unwrap();
        // Un directorio que dice apuntar al nivel equivocado.
        let i = indice(3, IOVA);
        let e = b.leer(d.raiz, i);
        b.escribir(d.raiz, i, (e & !NIVEL_MASK) | 1 << NIVEL_SHIFT);
        assert_eq!(d.traducir(&b, IOVA), None);
    }
}
