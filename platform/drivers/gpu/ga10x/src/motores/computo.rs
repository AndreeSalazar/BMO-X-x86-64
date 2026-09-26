//! **M5d S1..S3: EL PRIMER TRABAJO EN EL MOTOR GRAFICO** -- tras el contexto
//! de oro (G3 y G4, VISTOS en el metal el 24-09 a las 15:51), la clase de
//! computo en el canal de GR0 y un trabajo que SOLO puede pagar el GR.
//!
//! capa: puro -- arma las preguntas, las ordenes y lo que se mira; los
//! registros los toca el kernel con un `Registros` (L8)
//!
//! [eje]     CORRECCION -- la receta de la copia (L1d3, VISTA en el metal) con
//!           otra clase y otra lista; los metodos, de `clc7c0.h` de NVIDIA
//!
//! # El camino
//!
//! ```text
//!    S1  computo  AMPERE_COMPUTE_B (0xC7C0) colgado del canal de GR0, sin
//!                 parametros, como AMPERE_B                        (aqui)
//!    S2  fichagr  GET_WORK_SUBMIT_TOKEN del canal de GR0 (`control::FichaGr`);
//!                 el timbre, con la lista de GR0 de la tabla de aparatos
//!    S3  vacio    SET_OBJECT del computo en el subcanal 1 y un SEMAFORO DE
//!                 INFORME (`SET_REPORT_SEMAPHORE_*`), nada mas     (aqui)
//! ```
//!
//! # Por que el semaforo de INFORME y no el del canal
//!
//! El semaforo del canal (`NVC56F_SEM_*`) lo paga el PBDMA, antes de llegar
//! al motor: diria que el canal corre, no que el GR corre. El de informe es un
//! metodo de la clase de computo: lo ejecuta el FE del motor grafico, CON el
//! contexto de oro cargado. Si se paga, el GR corrio NUESTRO trabajo.
//!
//! # Donde vive (el tramo, 16 paginas)
//!
//! ```text
//!    0 instancia copia   1 USERD   2 GPFIFO copia   3 ordenes copia
//!    4 semaforo copia    5 instancia GR   6 GPFIFO GR
//!    7 ORDENES GR        9 SEMAFORO GR    8 y 12 origen y destino de la copia
//! ```

use crate::canal::GR;
use crate::copia::{cabecera_en, entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::vram::{a_cero, TRAMO, TRAMO_VA};
use crate::Registros;

/// `AMPERE_COMPUTE_B`: la clase de computo de GA10x.
pub const AMPERE_COMPUTE_B: u32 = 0xC7C0;
/// Su asa, colgada del canal de GR0.
pub const COMPUTO: u32 = 0xC7C0_0002;

const PAGINA: u64 = 0x1000;
/// Las paginas del tramo que usa el primer trabajo del GR.
pub const EMPUJE: u64 = TRAMO + 7 * PAGINA;
pub const SEMAFORO: u64 = TRAMO + 9 * PAGINA;

/// La misma pagina, vista por la GPU.
pub const fn va(vram: u64) -> u64 {
    TRAMO_VA + (vram - TRAMO)
}

/// Lo que escribe el GR en el semaforo: "GR0 corrio".
pub const PAGA: u32 = 0x3060_06C0;
/// El subcanal del computo (el de NVK: 0 3D, 1 computo, 4 copia).
pub const SUBCANAL: u32 = 1;
/// Palabras de ordenes.
pub const ORDENES: usize = 7;

/// Los metodos de `clc7c0.h`.
const SET_OBJECT: u32 = 0x0000;
const SET_REPORT_SEMAPHORE_A: u32 = 0x1B00;
/// `SET_REPORT_SEMAPHORE_D`: OPERATION RELEASE (0 en 1:0), FLUSH_DISABLE
/// FALSE (2), sin reduccion, STRUCTURE_SIZE ONE_WORD (1 en 28): solo la
/// carga, sin la marca de tiempo.
pub const INFORME: u32 = 1 << 28;

/// `(hClient, hParent, hObject, hClass, medida)`, como `gr::forma_tresde`.
pub const fn forma() -> (u32, u32, u32, u32, usize) {
    (crate::objeto::CLIENTE, GR.asa, COMPUTO, AMPERE_COMPUTE_B, 0)
}

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **S1: la pregunta** (`GSP_RM_ALLOC`), sin parametros.
pub fn pedir(hueco: &mut [u8], numero: u32) -> Option<usize> {
    let (cliente, padre, asa, clase, _) = forma();
    crate::orden::componer(hueco, numero, crate::objeto::GSP_RM_ALLOC, crate::objeto::CABECERA_ALLOC, |d| {
        poner(d, 0, cliente);
        poner(d, 4, padre);
        poner(d, 8, asa);
        poner(d, 12, clase);
    })
}

/// **S3: las ordenes**: atar el computo y el semaforo de informe.
pub fn ordenes() -> [u32; ORDENES] {
    let s = va(SEMAFORO);
    [
        cabecera_en(SUBCANAL, SET_OBJECT, 1),
        AMPERE_COMPUTE_B,
        cabecera_en(SUBCANAL, SET_REPORT_SEMAPHORE_A, 4),
        (s >> 32) as u32,
        s as u32,
        PAGA,
        INFORME,
    ]
}

/// **Preparar**: el semaforo a cero, las ordenes y la entrada 0 del GPFIFO del
/// canal de GR0, todo RELEIDO.
pub fn preparar<R: Registros>(r: &mut R) -> bool {
    let o = ordenes();
    let e = entrada(va(EMPUJE), ORDENES as u32);
    a_cero(r, SEMAFORO) as usize == crate::vram::PALABRAS
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GR.gpfifo, &[e as u32, (e >> 32) as u32]) == 2
}

/// **Lanzar**: la MMU invalidada, GP_PUT = 1 en el USERD del canal de GR0 y la
/// ficha en el timbre.
pub fn lanzar<R: Registros>(r: &mut R, ficha: u32) -> bool {
    let puesto = invalidar(r) && escribir(r, GR.userd + GP_PUT, &[1]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, el semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO))
}

/// Una ficha que es de NUESTRO canal de GR0: chid 2 y una lista en rango.
pub const fn ficha_valida(ficha: u64) -> bool {
    ficha & 0xFFFF == GR.chid as u64 && ficha >> 16 < 64
}

/// `semaforo | GP_GET << 32 | pagado << 40 | lanzado << 41 | us << 42`.
pub const fn empaquetar(semaforo: u32, gp_get: u32, lanzado: bool, us: u32) -> u64 {
    semaforo as u64 | (gp_get as u64 & 0xFF) << 32 | ((semaforo == PAGA) as u64) << 40 | (lanzado as u64) << 41 | (us as u64 & 0x3F_FFFF) << 42
}

/// `(semaforo, GP_GET, pagado, lanzado, us)`.
pub const fn desempaquetar(v: u64) -> (u32, u32, bool, bool, u32) {
    (v as u32, (v >> 32 & 0xFF) as u32, v >> 40 & 1 != 0, v >> 41 & 1 != 0, (v >> 42) as u32)
}

/// El GR pago: corrio nuestro trabajo.
pub const fn sano(v: u64) -> bool {
    let (_, _, pagado, lanzado, _) = desempaquetar(v);
    pagado && lanzado
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::CABECERA;
    use crate::vram::{VENTANA, VENTANA_REG};
    extern crate std;

    struct Falsa {
        ventana: u32,
        tramo: std::vec::Vec<u32>,
        timbre: Option<u32>,
    }

    impl Falsa {
        fn celda(&mut self, reg: u32) -> Option<&mut u32> {
            let dir = ((self.ventana as u64) << 16) + (reg - VENTANA) as u64;
            let k = dir.checked_sub(TRAMO)? / 4;
            self.tramo.get_mut(k as usize)
        }
        fn en(&self, dir: u64) -> u32 {
            self.tramo[((dir - TRAMO) / 4) as usize]
        }
    }

    impl Registros for Falsa {
        fn leer(&mut self, reg: u32) -> u32 {
            match reg {
                VENTANA_REG => self.ventana,
                _ => self.celda(reg).map_or(0, |c| *c),
            }
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            match reg {
                VENTANA_REG => self.ventana = v,
                TIMBRE => self.timbre = Some(v),
                crate::copia::INVALIDAR_PDB | crate::copia::INVALIDAR_PDB_HI | crate::copia::INVALIDAR => {}
                _ => {
                    if let Some(c) = self.celda(reg) {
                        *c = v;
                    }
                }
            }
        }
    }

    #[test]
    fn el_computo_va_sin_parametros_y_el_contrato_lo_deja() {
        let mut h = [0u8; 256];
        let n = pedir(&mut h, 23).unwrap();
        assert_eq!(n, CABECERA + crate::objeto::CABECERA_ALLOC);
        let u = |o: usize| u32::from_le_bytes(h[CABECERA + o..CABECERA + o + 4].try_into().unwrap());
        assert_eq!((u(4), u(8), u(12), u(20)), (GR.asa, COMPUTO, 0xC7C0, 0));
        assert!(crate::contrato::permitido(&h[..n]).is_ok());
    }

    #[test]
    fn las_ordenes_son_las_de_clc7c0() {
        let o = ordenes();
        // INC_METHOD (1 en 31:29), 1 metodo, subcanal 1, SET_OBJECT.
        assert_eq!(o[0], 0x2001_2000);
        assert_eq!(o[1], 0xC7C0);
        // 4 metodos desde 0x1B00 (>> 2 = 0x6C0).
        assert_eq!(o[2], 0x2004_26C0);
        assert_eq!(((o[3] as u64) << 32) | o[4] as u64, TRAMO_VA + 9 * 0x1000);
        assert_eq!((o[5], o[6]), (PAGA, 0x1000_0000));
    }

    #[test]
    fn preparar_y_lanzar_en_una_3060_de_mentira() {
        let mut f = Falsa { ventana: 0xFFF0, tramo: std::vec![0xDEAD_BEEF; 16 * 1024], timbre: None };
        assert!(preparar(&mut f));
        assert_eq!(f.en(SEMAFORO), 0);
        assert_eq!(f.en(EMPUJE + 4), 0xC7C0);
        let e = entrada(va(EMPUJE), ORDENES as u32);
        assert_eq!((f.en(GR.gpfifo), f.en(GR.gpfifo + 4)), (e as u32, (e >> 32) as u32));
        // Lo de la copia, sin tocar.
        assert_eq!(f.en(crate::copia::SEMAFORO), 0xDEAD_BEEF);
        assert!(lanzar(&mut f, 0x0000_0002));
        assert_eq!(f.en(GR.userd + GP_PUT), 1);
        assert_eq!(f.timbre, Some(2));
        assert_eq!(f.ventana, 0xFFF0);
    }

    #[test]
    fn la_ficha_es_la_del_canal_de_gr0() {
        assert!(ficha_valida(0x0000_0002));
        assert!(!ficha_valida(0x0000_0001));
        assert!(!ficha_valida(0x0040_0002));
    }

    #[test]
    fn el_resultado_se_empaqueta_y_se_lee() {
        let v = empaquetar(PAGA, 1, true, 12);
        assert_eq!(desempaquetar(v), (PAGA, 1, true, true, 12));
        assert!(sano(v));
        assert!(!sano(empaquetar(0, 0, true, 100_000)));
        assert!(!sano(empaquetar(PAGA, 0, false, 0)));
    }
}
