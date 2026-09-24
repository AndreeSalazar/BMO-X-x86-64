//! **PREGUNTARLE AL RM POR UN OBJETO NUESTRO** -- `GSP_RM_CONTROL`: una orden
//! de control (`NV2080_CTRL_CMD_*`) sobre nuestro subdispositivo de L1b. Hoy
//! una sola: el P-state, lo publico que dice cuanto corre la 3060.
//!
//! capa: puro -- arma la pregunta y lee la respuesta; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- los parametros van en la medida exacta del struct:
//!           otra es `NV_ERR_INVALID_PARAM_STRUCT`
//!
//! # La pregunta (r570.144, `g_rpc-structures.h`)
//!
//! ```text
//!    rpc_gsp_rm_control_v03_00   24 B: hClient, hObject, cmd, status,
//!                                paramsSize, flags; y los parametros
//!    PERF_GET_CURRENT_PSTATE     0x20802068, 4 B: currPstate, una mascara
//!                                NV2080_CTRL_PERF_PSTATES_P0 = 1 .. P15 = 0x8000
//!                                (sobre el SUBDISPOSITIVO)
//!    DMA_SET_PAGE_DIRECTORY      0x00801813, 32 B (L1c3, sobre el DISPOSITIVO):
//!                                physAddress +0, numEntries +8, flags +12
//!                                (APERTURE 0 = VIDMEM), hVASpace +16, chId
//!                                +20, subDeviceId +24, pasid +28
//!    BIND                        0xA06F0104, 4 B (L1d2c, sobre el CANAL):
//!                                engineType -- COPY2
//!    GPFIFO_SCHEDULE             0xA06F0103, 2 B (L1d2c, sobre el CANAL):
//!                                bEnable 1, bSkipSubmit 0 (dos NvBool)
//!    GET_WORK_SUBMIT_TOKEN       0xC36F0108, 4 B (L1d2d, sobre el CANAL):
//!                                workSubmitToken, lo que se escribe en el
//!                                timbre
//! ```
//!
//! # El directorio de paginas (L1c3)
//!
//! El espacio de L1c1 es "de fuera": su raiz la pone quien hace de RM de la
//! CPU. La raiz del formato de Ampere (el de Pascal, `gp100_vmm_desc_*` de
//! nouveau, `page[0]` de 47 bits) es la PD3: 4 entradas de 8 B, que es el
//! `numEntries = 1 << 2` de `r535/vmm.c`. Va en una pagina NUESTRA de VRAM
//! ([`crate::vram::DIRECTORIO`]), a cero: todo sin mapear. Lo que el RM se
//! reserve lo escribe el mismo ahi.
//!
//! Los parametros son FIJOS -- la direccion, el espacio -- y el contrato los
//! compara byte a byte con estos: ni el kernel puede mandar otro directorio.

use crate::canal::{CANAL, MOTOR};
use crate::objeto::{CLIENTE, DISPOSITIVO, ESPACIO, SUBDISPOSITIVO};
use crate::orden;

/// `NV_VGPU_MSG_FUNCTION_GSP_RM_CONTROL`.
pub const GSP_RM_CONTROL: u32 = 76;
pub const CABECERA_CONTROL: usize = 24;

/// Las ordenes de control que se piden. Solo las de la lista: el kernel no
/// arma otra (ver `contrato`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    Pstate,
    /// L1c3: la raiz de NUESTRO espacio de direcciones, en nuestra VRAM.
    Directorio,
    /// L1d2: que motores tiene la 3060 (`GET_ENGINES_V2`): de aqui sale el de
    /// COPIA sobre el que ira el canal.
    Motores,
    /// L1d2: cuanto mide el bufer de metodos de un canal de copia
    /// (`CE_GET_FAULT_METHOD_BUFFER_SIZE`).
    Metodos,
    /// L1d2c: atar el canal a su motor de copia (`BIND`). Cambia estado: no
    /// es pregunta, tiene su puerta (`IOMMU_OP_GPU_CANAL_ORDEN`).
    Atar,
    /// L1d2c: meter el canal en su lista de ejecucion (`GPFIFO_SCHEDULE`).
    Programar,
    /// L1d2d: la FICHA del timbre (`GET_WORK_SUBMIT_TOKEN`). Pregunta.
    Ficha,
    /// L1d3, el diagnostico: la tabla de aparatos del FIFO
    /// (`FIFO_GET_DEVICE_INFO_TABLE`): de que lista de ejecucion es cada
    /// motor, y donde estan los registros de esa lista. Pregunta.
    Dispositivos,
}

/// Entradas de la PD3 de Ampere: 2 bits de direccion (48..47).
pub const PD3_ENTRADAS: u32 = 4;

impl Control {
    pub const TODOS: [Control; 8] = [
        Control::Pstate,
        Control::Directorio,
        Control::Motores,
        Control::Metodos,
        Control::Atar,
        Control::Programar,
        Control::Ficha,
        Control::Dispositivos,
    ];

    pub fn de(n: u64) -> Option<Control> {
        Self::TODOS.get(n as usize).copied()
    }

    /// `(cmd, medida de los parametros, objeto sobre el que va)`.
    pub const fn forma(self) -> (u32, usize, u32) {
        match self {
            Control::Pstate => (0x2080_2068, 4, SUBDISPOSITIVO),
            Control::Directorio => (0x0080_1813, 32, DISPOSITIVO),
            Control::Motores => (0x2080_0170, 4 + 4 * MAX_MOTORES, SUBDISPOSITIVO),
            Control::Metodos => (0x2080_2A08, 4, SUBDISPOSITIVO),
            Control::Atar => (0xA06F_0104, 4, CANAL),
            Control::Programar => (0xA06F_0103, 2, CANAL),
            Control::Ficha => (0xC36F_0108, 4, CANAL),
            Control::Dispositivos => (0x2080_1112, DISPOSITIVOS_MEDIDA, SUBDISPOSITIVO),
        }
    }

    /// Una PREGUNTA, que se puede pedir sola (`IOMMU_OP_GSP_CONTROL`). El
    /// directorio no: va con su pagina a cero delante, y una vez
    /// (`IOMMU_OP_GPU_DIRECTORIO`).
    pub const fn pregunta(self) -> bool {
        matches!(self, Control::Pstate | Control::Motores | Control::Metodos | Control::Ficha | Control::Dispositivos)
    }

    /// Las que ENCIENDEN el canal (L1d2c): solo por su puerta, tras pedirlo.
    pub const fn del_canal(self) -> bool {
        matches!(self, Control::Atar | Control::Programar)
    }

    /// **Los parametros, exactos**: los que se mandan y los unicos que el
    /// contrato deja pasar. Devuelve cuantos bytes llenos.
    pub fn parametros(self, p: &mut [u8]) -> usize {
        let medida = self.forma().1;
        p[..medida].fill(0);
        if let Control::Directorio = self {
            p[0..8].copy_from_slice(&crate::vram::DIRECTORIO.to_le_bytes());
            poner(p, 8, PD3_ENTRADAS);
            // flags 0: APERTURE VIDMEM.
            poner(p, 16, ESPACIO);
        }
        match self {
            Control::Atar => poner(p, 0, MOTOR),
            // bEnable = 1; bSkipSubmit = 0.
            Control::Programar => p[0] = 1,
            _ => {}
        }
        medida
    }
}

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **La pregunta** en `hueco`, sobre nuestro subdispositivo.
pub fn pedir(hueco: &mut [u8], numero: u32, c: Control) -> Option<usize> {
    let (cmd, medida, objeto) = c.forma();
    orden::componer(hueco, numero, GSP_RM_CONTROL, CABECERA_CONTROL + medida, |d| {
        poner(d, 0, CLIENTE);
        poner(d, 4, objeto);
        poner(d, 8, cmd);
        poner(d, 16, medida as u32);
        c.parametros(&mut d[CABECERA_CONTROL..]);
    })
}

/// Lo que dice la respuesta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Respuesta {
    pub cmd: u32,
    pub estado: u32,
    /// El primer `u32` de los parametros (el P-state, en `Pstate`).
    pub valor: u32,
}

/// **Leer la respuesta**: los datos del mensaje (tras sus 80 B de cabecera).
pub fn leer(d: &[u8]) -> Option<Respuesta> {
    if d.len() < CABECERA_CONTROL + 4 {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    Some(Respuesta { cmd: u(8), estado: u(12), valor: u(CABECERA_CONTROL) })
}

impl Control {
    /// **Son estos los parametros de `self`?** Sin un bufer de la medida de los
    /// mas grandes: los de `Dispositivos` (3212 B) son todo ceros (`baseIndex`
    /// 0), y el resto caben en 512.
    pub fn iguales(self, p: &[u8]) -> bool {
        let medida = self.forma().1;
        if p.len() < medida {
            return false;
        }
        if let Control::Dispositivos = self {
            return p[..medida].iter().all(|&b| b == 0);
        }
        let mut esperados = [0u8; 512];
        let n = self.parametros(&mut esperados);
        p[..n] == esperados[..n]
    }
}

// == L1d3, EL DIAGNOSTICO: LA TABLA DE APARATOS DEL FIFO =====================
//
// `NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS` (r570, igual que r535):
// `baseIndex`, `numEntries`, `bMore` (y 3 de relleno), y 32 entradas de
// `NV2080_CTRL_FIFO_DEVICE_ENTRY`: `engineData[16]`, `pbdmaIds[2]`,
// `pbdmaFaultIds[2]`, `numPbdmas` y `engineName[16]` = 100 B. Es lo que lee
// nouveau (`r535_fifo_runl_ctor`) para saber de que lista es cada motor.

/// Entradas de la tabla.
pub const MAX_DISPOSITIVOS: usize = 32;
const ENTRADA: usize = 100;
/// Lo que miden los parametros.
pub const DISPOSITIVOS_MEDIDA: usize = 12 + MAX_DISPOSITIVOS * ENTRADA;
/// `ENGINE_INFO_TYPE_*`: el indice en `engineData`.
const TIPO_RM: usize = 2;
const LISTA: usize = 3;
const LISTA_BASE: usize = 11;
const CHRAM_BASE: usize = 14;

/// Un motor de la tabla.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Dispositivo {
    /// `RM_ENGINE_TYPE_*` (para GR y COPY vale lo mismo que `NV2080_ENGINE_TYPE`).
    pub tipo: u32,
    /// El numero de su lista de ejecucion: el `<< 16` de la ficha.
    pub lista: u32,
    /// Donde estan en BAR0 los registros de esa lista (`RUNLIST_PRI_BASE`).
    pub lista_base: u32,
    /// Y su CHRAM (`CHRAM_PRI_BASE`), si el RM la da.
    pub chram_base: u32,
}

/// **La tabla**, de la respuesta (`d` son los datos del mensaje).
pub fn dispositivos(d: &[u8]) -> ([Dispositivo; MAX_DISPOSITIVOS], usize) {
    let mut t = [Dispositivo::default(); MAX_DISPOSITIVOS];
    let p = &d[CABECERA_CONTROL.min(d.len())..];
    if p.len() < 12 {
        return (t, 0);
    }
    let u = |o: usize| u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
    let n = (u(4) as usize).min(MAX_DISPOSITIVOS).min((p.len() - 12) / ENTRADA);
    for (k, x) in t.iter_mut().enumerate().take(n) {
        let e = 12 + k * ENTRADA;
        *x = Dispositivo {
            tipo: u(e + 4 * TIPO_RM),
            lista: u(e + 4 * LISTA),
            lista_base: u(e + 4 * LISTA_BASE),
            chram_base: u(e + 4 * CHRAM_BASE),
        };
    }
    (t, n)
}

/// `NV2080_GPU_MAX_ENGINES_LIST_SIZE`.
pub const MAX_MOTORES: usize = 0x54;

/// Los motores de la respuesta de `Motores`: cuantos, y sus tipos
/// (`NV2080_ENGINE_TYPE_*`). `d` son los datos del mensaje.
pub fn motores(d: &[u8]) -> ([u32; MAX_MOTORES], usize) {
    let mut m = [0u32; MAX_MOTORES];
    let p = &d[CABECERA_CONTROL.min(d.len())..];
    if p.len() < 4 {
        return (m, 0);
    }
    let n = (u32::from_le_bytes([p[0], p[1], p[2], p[3]]) as usize).min(MAX_MOTORES).min((p.len() - 4) / 4);
    for (k, x) in m.iter_mut().enumerate().take(n) {
        let o = 4 + 4 * k;
        *x = u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
    }
    (m, n)
}

/// `NV2080_ENGINE_TYPE_COPY0`: los de copia son 0x09..=0x12.
pub const COPIA0: u32 = 0x09;

/// El nombre de un `NV2080_ENGINE_TYPE_*` (`cl2080_notification.h`), y su
/// numero dentro de su familia.
pub fn motor(t: u32) -> (&'static [u8], u32) {
    match t {
        0x01..=0x08 => (b"GR", t - 0x01),
        0x09..=0x12 => (b"COPY", t - 0x09),
        0x13 => (b"NVDEC", 0),
        0x14..=0x1A => (b"NVDEC", t - 0x13),
        0x1B => (b"NVENC", 0),
        0x1C..=0x1D => (b"NVENC", t - 0x1B),
        0x22 => (b"SW", 0),
        0x23 => (b"TSEC", 0),
        0x26 => (b"SEC2", 0),
        0x33 => (b"OFA", 0),
        _ => (b"motor", t),
    }
}

/// **El P-state** de la mascara: `Some(0)` es P0 (lo mas rapido), `Some(8)` P8
/// (reposo). `None` si no dice ninguno o dice mas de uno.
pub fn pstate(mascara: u32) -> Option<u8> {
    (mascara.count_ones() == 1).then(|| mascara.trailing_zeros() as u8)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn la_pregunta_del_pstate() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 7, Control::Pstate).unwrap();
        assert_eq!(n, CABECERA + 24 + 4);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.datos()), (GSP_RM_CONTROL, 28));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        let d = &h[CABECERA..];
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        assert_eq!((u(0), u(4), u(8), u(12), u(16), u(20), u(24)), (CLIENTE, SUBDISPOSITIVO, 0x2080_2068, 0, 4, 0, 0));
    }

    #[test]
    fn la_respuesta_y_el_pstate() {
        let mut d = [0u8; 28];
        d[8..12].copy_from_slice(&0x2080_2068u32.to_le_bytes());
        d[24..28].copy_from_slice(&0x100u32.to_le_bytes());
        let r = leer(&d).unwrap();
        assert_eq!((r.cmd, r.estado, pstate(r.valor)), (0x2080_2068, 0, Some(8)));
        assert_eq!(pstate(1), Some(0));
        assert_eq!(pstate(0), None);
        assert_eq!(pstate(0x101), None);
        assert_eq!(Control::de(1), Some(Control::Directorio));
        assert_eq!(Control::de(2), Some(Control::Motores));
        assert_eq!(Control::de(6), Some(Control::Ficha));
        assert_eq!(Control::de(7), Some(Control::Dispositivos));
        assert_eq!(Control::de(8), None);
        assert!(Control::Dispositivos.pregunta() && !Control::Dispositivos.del_canal());
        assert!(Control::Motores.pregunta() && Control::Metodos.pregunta() && !Control::Directorio.pregunta());
        assert!(Control::Ficha.pregunta() && !Control::Atar.pregunta() && !Control::Programar.pregunta());
        assert!(Control::Atar.del_canal() && Control::Programar.del_canal() && !Control::Ficha.del_canal());
        // El contrato compara los parametros en un bufer de 512 B.
        assert!(Control::TODOS.iter().filter(|&&c| c != Control::Dispositivos).all(|c| c.forma().1 <= 512));
        assert_eq!(Control::Dispositivos.forma().1, 3212);
    }

    #[test]
    fn el_directorio_va_al_dispositivo_con_lo_fijo() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 9, Control::Directorio).unwrap();
        assert_eq!(n, CABECERA + 24 + 32);
        let d = &h[CABECERA..];
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        assert_eq!((u(0), u(4), u(8), u(16)), (CLIENTE, DISPOSITIVO, 0x0080_1813, 32));
        let p = &d[24..56];
        assert_eq!(u64::from_le_bytes(p[0..8].try_into().unwrap()), crate::vram::DIRECTORIO);
        assert_eq!(u(24 + 8), 4, "la PD3: 4 entradas");
        assert_eq!(u(24 + 12), 0, "VIDMEM");
        assert_eq!(u(24 + 16), ESPACIO);
        assert!(p[20..32].iter().all(|&b| b == 0), "chId, subDeviceId y pasid a cero");
    }

    #[test]
    fn las_del_canal_van_al_canal_con_lo_fijo() {
        for (c, cmd, medida, primero) in [
            (Control::Atar, 0xA06F_0104u32, 4usize, 0x0Bu32),
            (Control::Programar, 0xA06F_0103, 2, 1),
            (Control::Ficha, 0xC36F_0108, 4, 0),
        ] {
            let mut h = [0xAAu8; 4096];
            let n = pedir(&mut h, 12, c).unwrap();
            assert_eq!(n, CABECERA + 24 + medida);
            let d = &h[CABECERA..];
            let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
            assert_eq!((u(0), u(4), u(8), u(16)), (CLIENTE, CANAL, cmd, medida as u32));
            let mut p = [0u8; 4];
            p[..medida].copy_from_slice(&d[24..24 + medida]);
            assert_eq!(u32::from_le_bytes(p), primero);
        }
    }

    #[test]
    fn la_tabla_de_aparatos() {
        let mut d = [0u8; CABECERA_CONTROL + DISPOSITIVOS_MEDIDA];
        let p = CABECERA_CONTROL;
        d[p + 4..p + 8].copy_from_slice(&2u32.to_le_bytes());
        // GR0 en la lista 0; COPY2 en la lista 3, sus registros en 0x00B40000.
        let pon = |d: &mut [u8], k: usize, i: usize, v: u32| {
            let o = p + 12 + k * 100 + 4 * i;
            d[o..o + 4].copy_from_slice(&v.to_le_bytes());
        };
        pon(&mut d, 0, 2, 1);
        pon(&mut d, 1, 2, 0x0B);
        pon(&mut d, 1, 3, 3);
        pon(&mut d, 1, 11, 0x00B4_0000);
        pon(&mut d, 1, 14, 0x00B4_1000);
        let (t, n) = dispositivos(&d);
        assert_eq!(n, 2);
        assert_eq!((t[0].tipo, t[0].lista), (1, 0));
        assert_eq!(t[1], Dispositivo { tipo: 0x0B, lista: 3, lista_base: 0x00B4_0000, chram_base: 0x00B4_1000 });
        // Sus parametros: ceros, y el contrato los compara sin un bufer de 3 KiB.
        assert!(Control::Dispositivos.iguales(&[0u8; DISPOSITIVOS_MEDIDA]));
        let mut malos = [0u8; DISPOSITIVOS_MEDIDA];
        malos[0] = 1;
        assert!(!Control::Dispositivos.iguales(&malos));
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 20, Control::Dispositivos).unwrap();
        assert_eq!(n, CABECERA + 24 + 3212);
    }

    #[test]
    fn los_motores_de_la_respuesta() {
        let mut d = [0u8; CABECERA_CONTROL + 4 + 4 * MAX_MOTORES];
        let lista = [0x01u32, 0x09, 0x0A, 0x0B, 0x13, 0x1B, 0x22];
        d[CABECERA_CONTROL..CABECERA_CONTROL + 4].copy_from_slice(&(lista.len() as u32).to_le_bytes());
        for (k, t) in lista.iter().enumerate() {
            let o = CABECERA_CONTROL + 4 + 4 * k;
            d[o..o + 4].copy_from_slice(&t.to_le_bytes());
        }
        let (m, n) = motores(&d);
        assert_eq!(&m[..n], &lista);
        assert_eq!(motor(0x0B), (b"COPY" as &[u8], 2));
        assert_eq!(motor(0x01), (b"GR" as &[u8], 0));
        assert_eq!(motor(0x1B), (b"NVENC" as &[u8], 0));
        assert_eq!(motores(&d[..10]).1, 0);
    }
}
