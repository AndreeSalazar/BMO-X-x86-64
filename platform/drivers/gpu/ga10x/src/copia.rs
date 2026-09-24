//! **L1d2d y L1d3: EL PRIMER TRABAJO DE LA 3060** -- el motor de copia COPY2
//! copia una pagina de VRAM a otra, por direcciones VIRTUALES de NUESTRO
//! espacio, porque se lo pedimos por el canal de L1d2b.
//!
//! capa: puro -- los bytes y las escrituras sobre `Registros`; el kernel pone
//! BAR0 y el reloj (L8)
//!
//! [eje]     CORRECCION -- es la primera vez que la GPU EJECUTA algo nuestro;
//!           todo lo que toca esta en el tramo de L1d1 y en ningun otro sitio
//!
//! # Los dos pasos
//!
//! ```text
//!    copiador  GSP_RM_ALLOC de AMPERE_DMA_COPY_B (0xC7B5) colgado del CANAL,
//!              con NVC0B5_ALLOCATION_PARAMETERS { version 1, engineType
//!              COPY2 } (`r535_ce_alloc` de nouveau). Sin el, SET_OBJECT en
//!              el canal no tiene clase que atar
//!    copia     por PRAMIN: el ORIGEN con el patron de L1c2, el DESTINO y el
//!              SEMAFORO a cero, el bufer de ordenes (17 palabras) y la
//!              entrada 0 del GPFIFO; despues GP_PUT = 1 en el USERD y la
//!              FICHA en el timbre (BAR0 0xBB0090: `ga100_vfn` 0xB80000 +
//!              `user` 0x30000 + 0x90, `tu102_chan_start`), tras invalidar
//!              la MMU de la GPU para nuestra raiz (`tu102_vmm_flush`). La
//!              3060 lee la entrada, ejecuta las ordenes, copia y escribe el
//!              semaforo
//! ```
//!
//! # El tramo (L1d1: VA 0x2_0000_0000 = VRAM 0x420_0000, 16 paginas)
//!
//! ```text
//!    pagina 0      instancia y RAMFC     (L1d2b)
//!    pagina 1      USERD, hueco 1        (L1d2b)  GP_GET +0x88, GP_PUT +0x8C
//!    pagina 2      GPFIFO, 512 entradas  (L1d2b)
//!    pagina 3      EMPUJE: las ordenes
//!    pagina 4      SEMAFORO: la palabra que escribe la 3060 al acabar
//!    pagina 8      ORIGEN
//!    pagina 12     DESTINO
//! ```
//!
//! # Como se sabe
//!
//! El semaforo dice [`PAGA`], GP_GET llego a 1, y las 1024 palabras del
//! DESTINO son el patron. Nada de eso lo escribe la CPU: solo la 3060.
//!
//! Las ordenes son las de `clc7b5.h` y `clc56f.h` (open-gpu-kernel-modules):
//! cabecera INC_METHOD (`SEC_OP` 1 en 31:29, cuenta en 28:16, subcanal en
//! 15:13, metodo >> 2 en 11:0).

use crate::canal::{CANAL, GPFIFO, MOTOR, USERD};
use crate::objeto::CLIENTE;
use crate::vram::{a_cero, patron, ventana, PALABRAS, TRAMO, TRAMO_VA, VENTANA, VENTANA_REG};
use crate::Registros;

/// `AMPERE_DMA_COPY_B`: la clase de copia de GA10x (`ga102_ce`).
pub const AMPERE_DMA_COPY_B: u32 = 0xC7B5;
/// Su asa, colgada del canal.
pub const COPIADOR: u32 = 0xCE00_0002;
/// `sizeof(NVC0B5_ALLOCATION_PARAMETERS)`.
pub const MEDIDA: usize = 8;
/// `NVC0B5_ALLOCATION_PARAMETERS_VERSION_1`: `engineType` es un
/// `NV2080_ENGINE_TYPE_*`.
pub const VERSION: u32 = 1;

const PAGINA: u64 = 0x1000;
/// Las paginas del tramo que usa la copia.
pub const EMPUJE: u64 = TRAMO + 3 * PAGINA;
pub const SEMAFORO: u64 = TRAMO + 4 * PAGINA;
pub const ORIGEN: u64 = TRAMO + 8 * PAGINA;
pub const DESTINO: u64 = TRAMO + 12 * PAGINA;

/// La misma pagina, vista por la GPU.
pub const fn va(vram: u64) -> u64 {
    TRAMO_VA + (vram - TRAMO)
}

/// Lo que escribe la 3060 en el semaforo al acabar.
pub const PAGA: u32 = 0x3060_C0DE;
/// El subcanal donde se ata la clase de copia.
pub const SUBCANAL: u32 = 4;
/// Palabras del bufer de ordenes.
pub const ORDENES: usize = 17;

/// El timbre: `NV_USERMODE_NOTIFY_CHANNEL_PENDING` en GA10x.
pub const TIMBRE: u32 = 0x00BB_0090;
/// En el USERD (`Nvc56fControl`).
pub const GP_GET: u64 = 0x88;
pub const GP_PUT: u64 = 0x8C;

/// Los metodos de `clc7b5.h`.
const SET_OBJECT: u32 = 0x000;
const SET_SEMAPHORE_A: u32 = 0x240;
const LAUNCH_DMA: u32 = 0x300;
const OFFSET_IN_UPPER: u32 = 0x400;

/// `LAUNCH_DMA`: NON_PIPELINED (2 en 1:0), FLUSH (2), semaforo RELEASE de
/// una palabra (1 en 4:3), origen y destino PITCH (7 y 8), una linea, sin
/// remapeo, los dos VIRTUALES (12 y 13 a 0).
pub const LANZAR: u32 = 2 | 1 << 2 | 1 << 3 | 1 << 7 | 1 << 8;

/// Cabecera `INC_METHOD` en el subcanal de la copia.
pub const fn cabecera(metodo: u32, cuenta: u32) -> u32 {
    1 << 29 | cuenta << 16 | SUBCANAL << 13 | metodo >> 2
}

/// `(hClient, hParent, hObject, hClass, medida)`, como `canal::forma`.
pub const fn forma() -> (u32, u32, u32, u32, usize) {
    (CLIENTE, CANAL, COPIADOR, AMPERE_DMA_COPY_B, MEDIDA)
}

/// **Los 8 B**, exactos: los que se mandan y los unicos que el contrato deja
/// pasar.
pub fn parametros(p: &mut [u8]) -> usize {
    p[..4].copy_from_slice(&VERSION.to_le_bytes());
    p[4..8].copy_from_slice(&MOTOR.to_le_bytes());
    MEDIDA
}

/// **La pregunta** (`GSP_RM_ALLOC`) en `hueco`.
pub fn pedir(hueco: &mut [u8], numero: u32) -> Option<usize> {
    let (cliente, padre, asa, clase, medida) = forma();
    crate::orden::componer(hueco, numero, crate::objeto::GSP_RM_ALLOC, crate::objeto::CABECERA_ALLOC + medida, |d| {
        d[0..4].copy_from_slice(&cliente.to_le_bytes());
        d[4..8].copy_from_slice(&padre.to_le_bytes());
        d[8..12].copy_from_slice(&asa.to_le_bytes());
        d[12..16].copy_from_slice(&clase.to_le_bytes());
        d[20..24].copy_from_slice(&(medida as u32).to_le_bytes());
        parametros(&mut d[crate::objeto::CABECERA_ALLOC..]);
    })
}

/// **Las ordenes**: atar la clase, origen y destino, el semaforo, y lanzar.
pub fn ordenes() -> [u32; ORDENES] {
    let (o, d, s) = (va(ORIGEN), va(DESTINO), va(SEMAFORO));
    let linea = (PALABRAS * 4) as u32;
    [
        cabecera(SET_OBJECT, 1),
        AMPERE_DMA_COPY_B,
        cabecera(OFFSET_IN_UPPER, 8),
        (o >> 32) as u32,
        o as u32,
        (d >> 32) as u32,
        d as u32,
        linea, // PITCH_IN
        linea, // PITCH_OUT
        linea, // LINE_LENGTH_IN
        1,     // LINE_COUNT
        cabecera(SET_SEMAPHORE_A, 3),
        (s >> 32) as u32,
        s as u32,
        PAGA,
        cabecera(LAUNCH_DMA, 1),
        LANZAR,
    ]
}

/// **La entrada del GPFIFO** que apunta a `palabras` ordenes en `va`
/// (`GP_ENTRY0_GET` 31:2; `GP_ENTRY1_GET_HI` 7:0, `LENGTH` 30:10, nivel MAIN).
pub const fn entrada(va: u64, palabras: u32) -> u64 {
    let e0 = va as u32 & !3;
    let e1 = (va >> 32) as u32 & 0xFF | palabras << 10;
    e0 as u64 | (e1 as u64) << 32
}

/// Escribir palabras seguidas en la VRAM por la ventana (misma ventana: no
/// cruzan 1 MiB), y devolver cuantas se RELEEN iguales.
fn escribir<R: Registros>(r: &mut R, dir: u64, p: &[u32]) -> usize {
    let (base, off) = ventana(dir);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    for (k, &v) in p.iter().enumerate() {
        r.escribir(VENTANA + off + 4 * k as u32, v);
    }
    let bien = p.iter().enumerate().filter(|&(k, &v)| r.leer(VENTANA + off + 4 * k as u32) == v).count();
    r.escribir(VENTANA_REG, antes);
    bien
}

/// Leer una palabra de la VRAM por la ventana, que queda como estaba.
pub fn leer32<R: Registros>(r: &mut R, dir: u64) -> u32 {
    let (base, off) = ventana(dir);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    let v = r.leer(VENTANA + off);
    r.escribir(VENTANA_REG, antes);
    v
}

/// **Preparar**: el origen con el patron, destino y semaforo a cero, las
/// ordenes y la entrada 0 del GPFIFO, todo releido. `true` si todo quedo.
pub fn preparar<R: Registros>(r: &mut R) -> bool {
    let mut origen = [0u32; PALABRAS];
    for (k, w) in origen.iter_mut().enumerate() {
        *w = patron(k);
    }
    let o = ordenes();
    let e = entrada(va(EMPUJE), ORDENES as u32);
    escribir(r, ORIGEN, &origen) == PALABRAS
        && a_cero(r, DESTINO) as usize == PALABRAS
        && a_cero(r, SEMAFORO) as usize == PALABRAS
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GPFIFO, &[e as u32, (e >> 32) as u32]) == 2
}

/// La invalidacion de la MMU de la GPU (`tu102_vmm_flush` de nouveau, la que
/// usa GA10x): la raiz (>> 8), su parte alta, y la orden con el bit 31, que la
/// GPU baja al acabar.
pub const INVALIDAR_PDB: u32 = 0x00B8_30A0;
pub const INVALIDAR_PDB_HI: u32 = 0x00B8_30A4;
pub const INVALIDAR: u32 = 0x00B8_30B0;
/// `0x80000000 | PAGE_ALL`.
pub const INVALIDAR_TODO: u32 = 0x8000_0001;
/// Cuantas lecturas se espera a que baje el bit 31 (nouveau: 2 s).
const INVALIDAR_VUELTAS: u32 = 2_000_000;

/// **Invalidar la MMU** para NUESTRA raiz: lo que la GPU tuviera de nuestras
/// tablas se tira y las vuelve a leer. nouveau lo hace tras CADA mapeo;
/// BMO-X no lo hizo tras L1d1 (metal 24-09 14:19: GP_GET se quedo en 0).
/// `true` si la GPU acabo.
pub fn invalidar<R: Registros>(r: &mut R) -> bool {
    r.escribir(INVALIDAR_PDB, (crate::vram::DIRECTORIO >> 8) as u32);
    r.escribir(INVALIDAR_PDB_HI, 0);
    r.escribir(INVALIDAR, INVALIDAR_TODO);
    (0..INVALIDAR_VUELTAS).any(|_| r.leer(INVALIDAR) & 0x8000_0000 == 0)
}

/// **Lanzar**: la MMU invalidada, GP_PUT = 1 en el USERD y la ficha en el
/// timbre. `false` (y el timbre sin tocar) si algo no quedo.
pub fn lanzar<R: Registros>(r: &mut R, ficha: u32) -> bool {
    let puesto = invalidar(r) && escribir(r, USERD + GP_PUT, &[1]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// Lo que se mira mientras se espera: `(GP_GET, el semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    (leer32(r, USERD + GP_GET), leer32(r, SEMAFORO))
}

/// **Comprobar**: cuantas palabras del DESTINO son el patron.
pub fn comprobar<R: Registros>(r: &mut R) -> u32 {
    let (base, off) = ventana(DESTINO);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, base);
    let n = (0..PALABRAS).filter(|&k| r.leer(VENTANA + off + 4 * k as u32) == patron(k)).count() as u32;
    r.escribir(VENTANA_REG, antes);
    n
}

/// **Lo que se mira si la copia no sale** (solo lectura, por PRAMIN): lo que
/// el RM escribio en la instancia del canal (RAMFC de `gv100_chan_ramfc_write`
/// de nouveau: USERD +0x008/+0x00C, GPFIFO +0x048/+0x04C, chid +0x0E8; y el
/// directorio de paginas en +0x200/+0x204), el USERD (GP_GET, GP_PUT), la
/// entrada 0 del GPFIFO y el semaforo. `(nombre, direccion de VRAM)`.
///
/// ** Metal 24-09 14:30: `userd` y `chid` salen a 0, y es lo NORMAL en GA10x:
/// ahi el USERD va en la entrada de la lista de ejecucion
/// (`NV_RAMRL_ENTRY_CHAN_USERD_PTR`, `kernel_channel_ga100.c` de OpenRM) y la
/// 3060 los copia a la RAMFC al CARGAR el canal. Siguen en la fila porque si
/// un dia dejan de ser 0, la 3060 lo cargo.
pub const DIAGNOSTICO: [(&[u8], u64); 13] = [
    (b"userd lo", crate::canal::INSTANCIA + 0x008),
    (b"userd hi", crate::canal::INSTANCIA + 0x00C),
    // nouveau pone 0xFACE aqui; lo que ponga el RM dice si escribio esa parte.
    (b"+010", crate::canal::INSTANCIA + 0x010),
    (b"config", crate::canal::INSTANCIA + 0x0F8),
    (b"gpfifo lo", crate::canal::INSTANCIA + 0x048),
    (b"gpfifo hi", crate::canal::INSTANCIA + 0x04C),
    (b"chid", crate::canal::INSTANCIA + 0x0E8),
    (b"pdb lo", crate::canal::INSTANCIA + 0x200),
    (b"pdb hi", crate::canal::INSTANCIA + 0x204),
    (b"gp_get", USERD + GP_GET),
    (b"gp_put", USERD + GP_PUT),
    (b"entrada0", GPFIFO),
    (b"semaforo", SEMAFORO),
];

/// El reloj de la ventana de usuario de la 3060 (`NVC361_TIME_0`, en la misma
/// region que el timbre: `ga100_vfn` 0xB80000 + 0x30000 + 0x80). Si al leerlo
/// dos veces CAMBIA, la region del timbre es la buena y el timbre llega; si no,
/// el timbre se escribe en el vacio.
pub const RELOJ_USUARIO: u32 = 0x00BB_0080;

/// Los registros de BAR0 que el kernel deja LEER para el diagnostico: solo el
/// reloj de la ventana de usuario (y su parte alta).
pub const fn registro_legible(reg: u32) -> bool {
    reg == RELOJ_USUARIO || reg == RELOJ_USUARIO + 4
}

// == La lista de ejecucion de nuestro canal (solo lectura) ===================
//
// De `ga100_runl_new` de nouveau y `dev_runlist.h` de OpenRM (ga100): en la
// base de la lista, +0x004 la config de la CHRAM (su direccion en BAR0 en los
// bits 31:4) y +0x008 la del timbre (el numero de timbre de la lista en 31:16);
// y en la CHRAM, una palabra por canal con su estado.

/// Lo que se puede leer de una lista: la config de la CHRAM, la del timbre y
/// la entrada de NUESTRO canal en la CHRAM.
pub const LISTA_CHRAM: u32 = 0x004;
pub const LISTA_TIMBRE: u32 = 0x008;

/// Una base de lista que se deja mirar: alineada a 64 B, dentro de los 16 MiB
/// de BAR0 y no cero (la da el RM en la tabla de aparatos). ** Pedia 4 KiB y
/// el metal (24-09 14:47) trajo la de COPY2 en 0x00C00400: la fila decia
/// "sin leer".
pub const fn lista_legible(base: u32) -> bool {
    base != 0 && base % 0x40 == 0 && base < 0x0100_0000
}

/// **Lo que se escribe en el timbre** para `chid` en la lista `lista`:
/// `lista << 16 | chid`, como `tu102_chan_doorbell_handle` de nouveau, que es
/// el que usa GA1xx sobre el GSP-RM (`rm/ga1xx.c`), con la lista de la TABLA
/// de aparatos (`ENGINE_INFO_TYPE_RUNLIST`). ** El metal (24-09 14:47): la
/// tabla pone COPY2 en la lista 1 y la ficha del RM decia 0x1 (lista 0, la
/// de GR0): el timbre llamaba a otra lista.
pub const fn timbre_de(lista: u32, chid: u32) -> u32 {
    (lista & 0x7F) << 16 | (chid & 0xFFF)
}

/// La direccion en BAR0 de la entrada de `chid` en la CHRAM, de la config.
pub const fn chram_de(config: u32, chid: u32) -> Option<u32> {
    let base = config & 0xFFFF_FFF0;
    if base == 0 || base >= 0x0100_0000 {
        None
    } else {
        Some(base + 4 * chid)
    }
}

/// Los bits de una entrada de la CHRAM (`NV_CHRAM_CHANNEL_*`), con su nombre.
pub const CHRAM_BITS: [(u32, &[u8]); 12] = [
    (1 << 1, b"ENABLE"),
    (1 << 2, b"NEXT"),
    (1 << 3, b"BUSY"),
    (1 << 4, b"PBDMA_FAULTED"),
    (1 << 5, b"ENG_FAULTED"),
    (1 << 6, b"ON_PBDMA"),
    (1 << 7, b"ON_ENG"),
    (1 << 8, b"PENDING"),
    (1 << 9, b"CTX_RELOAD"),
    (1 << 10, b"PBDMA_BUSY"),
    (1 << 11, b"ENG_BUSY"),
    (1 << 12, b"ACQUIRE_FAIL"),
];

/// La direccion de VRAM que el kernel deja LEER (una palabra): dentro del
/// tramo y alineada a 4.
pub const fn legible(dir: u64) -> bool {
    dir >= TRAMO && dir + 4 <= TRAMO + crate::vram::TRAMO_PAGINAS as u64 * PAGINA && dir % 4 == 0
}

/// Lo que devuelve el kernel en un `u64`: `buenas | GP_GET << 16 | semaforo
/// pagado << 24 | lanzada << 25 | us esperados << 32`.
pub const fn empaquetar(buenas: u32, gp_get: u32, pagado: bool, lanzada: bool, us: u32) -> u64 {
    (buenas as u64 & 0xFFFF) | (gp_get as u64 & 0xFF) << 16 | (pagado as u64) << 24 | (lanzada as u64) << 25 | (us as u64) << 32
}

/// `(buenas, GP_GET, pagado, lanzada, us)`.
pub const fn desempaquetar(v: u64) -> (u32, u32, bool, bool, u32) {
    ((v & 0xFFFF) as u32, (v >> 16 & 0xFF) as u32, v >> 24 & 1 != 0, v >> 25 & 1 != 0, (v >> 32) as u32)
}

/// La copia salio entera.
///
/// ** GP_GET NO cuenta. El metal (24-09 14:55): 1024 de 1024 y el semaforo
/// PAGADO al instante, con GP_GET aun en 0 -- y un segundo despues, 1. La
/// 3060 escribe GP_GET en el USERD cuando le toca, no cuando acaba: lo que
/// dice que copio es el destino y el semaforo, que solo escribe ella.
pub const fn sana(v: u64) -> bool {
    let (buenas, _, pagado, lanzada, _) = desempaquetar(v);
    buenas as usize == PALABRAS && pagado && lanzada
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::objeto::{CABECERA_ALLOC, GSP_RM_ALLOC};
    use crate::rpc::{Mensaje, Suma, CABECERA};
    extern crate std;

    /// Una 3060 de mentira: la ventana PRAMIN sobre los 64 KiB del tramo, y
    /// el timbre apuntado.
    struct Falsa {
        ventana: u32,
        tramo: std::vec::Vec<u32>,
        timbre: Option<u32>,
        /// Lo que se pidio invalidar: `(raiz, orden)`.
        invalidado: Option<(u32, u32)>,
        pdb: u32,
    }

    impl Falsa {
        fn nueva() -> Falsa {
            Falsa { ventana: 0xFFF0, tramo: std::vec![0xDEAD_BEEF; 16 * 1024], timbre: None, invalidado: None, pdb: 0 }
        }
        fn celda(&mut self, reg: u32) -> Option<&mut u32> {
            let dir = ((self.ventana as u64) << 16) + (reg - VENTANA) as u64;
            let k = dir.checked_sub(TRAMO)? / 4;
            self.tramo.get_mut(k as usize)
        }
        fn en(&mut self, dir: u64) -> &mut u32 {
            &mut self.tramo[((dir - TRAMO) / 4) as usize]
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
                INVALIDAR_PDB => self.pdb = v,
                INVALIDAR_PDB_HI => {}
                // La GPU de mentira acaba al momento: el bit 31, bajado.
                INVALIDAR => self.invalidado = Some((self.pdb, v)),
                _ => {
                    if let Some(c) = self.celda(reg) {
                        *c = v;
                    }
                }
            }
        }
    }

    #[test]
    fn la_pregunta_suma_cero_y_va_al_canal() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 15).unwrap();
        assert_eq!(n, CABECERA + CABECERA_ALLOC + 8);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero), (GSP_RM_ALLOC, 15));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        let d = &h[CABECERA..];
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        assert_eq!((u(0), u(4), u(8), u(12), u(20)), (CLIENTE, CANAL, COPIADOR, 0xC7B5, 8));
        assert_eq!((u(CABECERA_ALLOC), u(CABECERA_ALLOC + 4)), (1, 0x0B), "version 1, COPY2");
    }

    #[test]
    fn las_ordenes_son_las_de_clc7b5() {
        let o = ordenes();
        // SET_OBJECT en el subcanal 4: 0x2001_8000.
        assert_eq!((o[0], o[1]), (0x2001_8000, 0xC7B5));
        // OFFSET_IN_UPPER (0x400 >> 2 = 0x100), 8 seguidas.
        assert_eq!(o[2], 0x2008_8100);
        assert_eq!((o[3], o[4]), (2, 0x8000), "el origen, VA 0x2_0000_8000");
        assert_eq!((o[5], o[6]), (2, 0xC000), "el destino, VA 0x2_0000_C000");
        assert_eq!(&o[7..11], &[4096, 4096, 4096, 1]);
        assert_eq!(o[11], 0x2003_8090, "SET_SEMAPHORE_A (0x240 >> 2), 3 seguidas");
        assert_eq!((o[12], o[13], o[14]), (2, 0x4000, PAGA));
        assert_eq!((o[15], o[16]), (0x2001_80C0, 0x18E), "LAUNCH_DMA");
    }

    #[test]
    fn la_entrada_del_gpfifo() {
        let e = entrada(va(EMPUJE), 17);
        assert_eq!(e as u32, 0x3000, "GET 31:2 de la VA baja");
        assert_eq!((e >> 32) as u32, 2 | 17 << 10, "GET_HI 2, LENGTH 17, MAIN");
    }

    #[test]
    fn la_lista_de_ejecucion_solo_se_mira() {
        assert!(lista_legible(0x0080_4000) && !lista_legible(0) && !lista_legible(0x0080_4004));
        assert!(lista_legible(0x00C0_0400), "la de COPY2 en el metal (24-09 14:47)");
        assert_eq!(timbre_de(1, 1), 0x0001_0001);
        assert!(!lista_legible(0x0100_0000));
        assert_eq!(chram_de(0x0080_6007, 1), Some(0x0080_6004), "los 4 bits bajos son el numero de canales");
        assert_eq!(chram_de(0x0000_0007, 1), None);
        assert_eq!(chram_de(0x0200_0000, 1), None);
        assert_eq!(CHRAM_BITS[0], (2, b"ENABLE" as &[u8]));
    }

    #[test]
    fn el_diagnostico_solo_lee_el_tramo() {
        assert!(DIAGNOSTICO.iter().all(|&(_, d)| legible(d)));
        assert!(!legible(TRAMO - 4) && !legible(TRAMO + 16 * PAGINA) && !legible(TRAMO + 2));
        assert!(registro_legible(0xBB_0080) && registro_legible(0xBB_0084));
        assert!(!registro_legible(TIMBRE) && !registro_legible(INVALIDAR));
        assert_eq!(RELOJ_USUARIO + 0x10, TIMBRE, "el reloj y el timbre, en la misma ventana");
        // Lo que nouveau escribe en la RAMFC: el USERD de L1d2b y el GPFIFO.
        assert_eq!(DIAGNOSTICO[0].1, crate::canal::INSTANCIA + 8);
    }

    #[test]
    fn todo_en_el_tramo_y_sin_pisarse() {
        let fin = TRAMO + crate::vram::TRAMO_PAGINAS as u64 * PAGINA;
        let usadas = [crate::canal::INSTANCIA, crate::canal::USERD_PAGINA, GPFIFO, EMPUJE, SEMAFORO, ORIGEN, DESTINO];
        for (k, a) in usadas.iter().enumerate() {
            assert!(*a >= TRAMO && *a + PAGINA <= fin && *a % PAGINA == 0);
            assert!(usadas[k + 1..].iter().all(|b| b != a), "cada una en su pagina");
        }
    }

    #[test]
    fn preparar_lanzar_y_una_copia_de_mentira() {
        let mut f = Falsa::nueva();
        // Lo que dejo L1d2b: sus tres paginas a cero.
        for p in crate::canal::PAGINAS_A_CERO {
            a_cero(&mut f, p);
        }
        assert!(preparar(&mut f));
        assert_eq!(f.ventana, 0xFFF0, "la ventana, como estaba");
        assert_eq!(comprobar(&mut f), 0, "el destino, a cero");
        assert_eq!(mirar(&mut f), (0, 0));
        assert!(lanzar(&mut f, 1));
        assert_eq!((f.timbre, *f.en(USERD + GP_PUT)), (Some(1), 1));
        assert_eq!(f.invalidado, Some((0x41000, 0x8000_0001)), "la raiz de L1c3 >> 8, PAGE_ALL");
        // Lo que hara la 3060: copiar, pagar el semaforo y avanzar GP_GET.
        for k in 0..PALABRAS as u64 {
            let v = *f.en(ORIGEN + 4 * k);
            *f.en(DESTINO + 4 * k) = v;
        }
        *f.en(SEMAFORO) = PAGA;
        *f.en(USERD + GP_GET) = 1;
        assert_eq!(mirar(&mut f), (1, PAGA));
        assert_eq!(comprobar(&mut f), 1024);
        let v = empaquetar(1024, 1, true, true, 37);
        assert!(sana(v));
        assert_eq!(desempaquetar(v), (1024, 1, true, true, 37));
        assert!(!sana(empaquetar(1023, 1, true, true, 0)));
        assert!(sana(empaquetar(1024, 0, true, true, 0)), "GP_GET tarde no es fallo (metal 24-09 14:55)");
        assert!(!sana(empaquetar(1024, 1, false, true, 0)), "sin semaforo, no");
    }
}
