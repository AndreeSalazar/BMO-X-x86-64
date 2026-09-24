//! **M5 (G0): LO QUE PIDE EL MOTOR GRAFICO** -- la primera pieza del contexto
//! de GR0 bajo el GSP-RM: preguntarle al RM que buferes de contexto necesita
//! el motor grafico, y de que medida, y calcular como los reparte nouveau.
//!
//! capa: puro -- arma la pregunta y lee la respuesta; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- es la receta de `r570_gr_get_ctxbufs_and_zcull_info`
//!           y `r535_gr_get_ctxbuf_info` de nouveau; un bufer de menos o de
//!           otra medida y el RM no hace el contexto de oro
//!
//! # El camino (nouveau `r535_gr_oneinit`), y donde esta cada paso
//!
//! ```text
//!    G0  preguntar los buferes: INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO
//!        (0x20800A32) sobre el subdispositivo INTERNO del RM          (aqui)
//!    G1  un canal en GR0 (lista 0), como el de L1d2b
//!    G2  los buferes en VRAM, mapeados en nuestro espacio
//!    G3  PROMOTE_CTX (0x2080012B) con cada bufer
//!    G4  AMPERE_B (0xC797) en ese canal: el RM hace el contexto de ORO
//! ```
//!
//! # La respuesta (r570.144, `nvrm/gr.h` de nouveau)
//!
//! `engineContextBuffersInfo[8]` (uno por motor grafico), cada uno con
//! `engine[0x1A]` de `{ size, alignment }` (8 B): 8 x 26 x 8 = 1664 B. Solo
//! cuenta el motor 0. Los indices son `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_
//! PROPERTIES_ENGINE_ID_*`.
//!
//! # Por que el cliente INTERNO
//!
//! Es una orden `INTERNAL`: nouveau la manda sobre `gsp->internal.device.
//! subdevice`, las asas del RM que da `GET_GSP_STATIC_INFO` (L1a, fila
//! `asas`). El contrato la deja salir SOLO esta, con sus 1664 B a cero: es una
//! pregunta y no cambia nada.

use crate::control::{CABECERA_CONTROL, GSP_RM_CONTROL};
use crate::orden;

/// `NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO`.
pub const BUFERES: u32 = 0x2080_0A32;
/// `NV2080_CTRL_INTERNAL_GR_MAX_ENGINES`.
const MOTORES: usize = 8;
/// `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_PROPERTIES_ENGINE_ID_COUNT` en r570.
pub const IDS: usize = 0x1A;
/// Lo que miden los parametros.
pub const MEDIDA: usize = MOTORES * IDS * 8;

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// **La pregunta**, sobre el cliente y el subdispositivo INTERNOS del RM.
pub fn pedir(hueco: &mut [u8], numero: u32, cliente: u32, subdispositivo: u32) -> Option<usize> {
    orden::componer(hueco, numero, GSP_RM_CONTROL, CABECERA_CONTROL + MEDIDA, |d| {
        poner(d, 0, cliente);
        poner(d, 4, subdispositivo);
        poner(d, 8, BUFERES);
        poner(d, 16, MEDIDA as u32);
        // Los parametros, a cero (`componer` ya los dejo asi).
    })
}

/// **Puede salir?** Solo esta orden, con su medida y sus parametros a cero;
/// con cualquier asa distinta de 0 (las del RM, que da L1a). `d` son los
/// datos del mensaje (cabecera de control y parametros).
pub fn permitida(d: &[u8]) -> bool {
    if d.len() < CABECERA_CONTROL + MEDIDA {
        return false;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    u(0) != 0
        && u(4) != 0
        && u(8) == BUFERES
        && u(16) as usize == MEDIDA
        && d[CABECERA_CONTROL..CABECERA_CONTROL + MEDIDA].iter().all(|&b| b == 0)
}

/// Un bufer que pide el motor grafico, como lo prepara nouveau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bufer {
    /// `NV0080_CTRL_FIFO_GET_ENGINE_CONTEXT_PROPERTIES_ENGINE_ID_*`.
    pub id: u8,
    /// `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*`.
    pub promover: u8,
    pub nombre: &'static [u8],
    /// Lo que dijo el RM.
    pub medida_rm: u32,
    pub alineado_rm: u32,
    /// Lo que se reserva: la de MAIN lleva 64 cabeceras de subcontexto mas.
    pub medida: u64,
    /// Paginas de 2^`pagina` (12, 16 o 21) y alineada a 2^`alinear`.
    pub pagina: u8,
    pub alinear: u8,
    /// Uno para todos los canales (se hace una vez, en el de oro).
    pub global: bool,
    /// El RM lo rellena al promoverlo.
    pub iniciar: bool,
    /// Solo lectura para la GPU.
    pub ro: bool,
}

/// Cuantos buferes: los ocho de la tabla de nouveau y, detras de
/// PRIV_ACCESS_MAP, su copia UNRESTRICTED.
pub const N: usize = 9;

/// La tabla de `r535_gr_get_ctxbuf_info`: `(id, promover, nombre, global,
/// iniciar, ro)`. Los `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ID_*` de la r570
/// (`nvrm/gpu.h` de nouveau): MAIN 0, PATCH 2, BUNDLE_CB 3, PAGEPOOL 4,
/// ATTRIBUTE_CB 5, RTV_CB_GLOBAL 6, FECS_EVENT 9, PRIV_ACCESS_MAP 10 y
/// UNRESTRICTED_PRIV_ACCESS_MAP 11.
///
/// ** La novena (24-09, estudiando G3 antes de probar G2): nouveau, tras
/// PRIV_ACCESS_MAP, anade una copia con el id 11 y la misma medida; en el
/// contexto de ORO cada una lleva SU memoria, y PRIV_ACCESS_MAP va sin mapear
/// (`bNonmapped`). La primera version de G2 contaba ocho.
pub const TABLA: [(u8, u8, &[u8], bool, bool, bool); N] = [
    (0x00, 0, b"MAIN", false, true, false),
    (0x10, 2, b"PATCH", false, true, false),
    (0x11, 3, b"BUNDLE_CB", true, false, false),
    (0x0D, 4, b"PAGEPOOL", true, false, false),
    (0x13, 5, b"ATTRIBUTE_CB", true, false, false),
    (0x14, 6, b"RTV_CB_GLOBAL", true, false, false),
    (0x17, 9, b"FECS_EVENT", true, true, false),
    (0x18, 10, b"PRIV_ACCESS_MAP", true, true, true),
    (0x18, 11, b"UNRESTRICTED_PAM", true, true, true),
];

/// El que va SIN mapear en el contexto de oro (`bNonmapped`).
pub const fn sin_mapear(promover: u8) -> bool {
    promover == 10
}

/// `order_base_2`: el menor `k` con `2^k >= v`.
const fn orden_base_2(v: u64) -> u8 {
    if v <= 1 {
        0
    } else {
        (64 - (v - 1).leading_zeros()) as u8
    }
}

/// **Los buferes de la respuesta** (`d` son los datos del mensaje), en el
/// orden de [`TABLA`], con la medida, la pagina y la alineacion que calcula
/// nouveau. `None` si la respuesta no llega entera.
pub fn buferes(d: &[u8]) -> Option<[Bufer; N]> {
    let p = d.get(CABECERA_CONTROL..CABECERA_CONTROL + MEDIDA)?;
    let u = |o: usize| u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
    let mut m = [(0u32, 0u32); DISTINTOS];
    for (k, x) in m.iter_mut().enumerate() {
        // Motor 0, entrada `id`.
        let id = TABLA[k].0 as usize;
        *x = (u(8 * id), u(8 * id + 4));
    }
    Some(desde_medidas(&m))
}

/// Cuantas medidas distintas: la novena es la copia de la octava.
pub const DISTINTOS: usize = N - 1;

/// **La tabla desde las medidas** `(medida, alineacion)` del RM, una por fila
/// de [`TABLA`] (la novena usa la de la octava). Es lo que hace `buferes` con
/// la respuesta, y lo que rehace el kernel para G3 con las ocho medidas que le
/// pasa el escritorio: la MISMA cuenta a los dos lados.
pub fn desde_medidas(m: &[(u32, u32); DISTINTOS]) -> [Bufer; N] {
    let mut t = [Bufer::default(); N];
    for (k, &(id, promover, nombre, global, iniciar, ro)) in TABLA.iter().enumerate() {
        let (medida_rm, alineado_rm) = m[k.min(DISTINTOS - 1)];
        let mut medida = medida_rm as u64;
        if id == 0x00 {
            // MAIN: las cabeceras de 64 subcontextos detras.
            medida = (medida + 0xFFF) & !0xFFF;
            medida += 64 * 0x1000;
        }
        let pagina = if medida >= 1 << 21 {
            21
        } else if medida >= 1 << 16 {
            16
        } else {
            12
        };
        let alinear = if id == 0x13 { orden_base_2(medida) } else { pagina };
        t[k] = Bufer { id, promover, nombre, medida_rm, alineado_rm, medida, pagina, alinear, global, iniciar, ro };
    }
    t
}

/// Lo que ocupan todos, alineados: lo que G2 tendra que buscar en VRAM.
pub fn total(t: &[Bufer; N]) -> u64 {
    t.iter().fold(0u64, |acc, b| {
        let a = 1u64 << b.alinear;
        ((acc + a - 1) & !(a - 1)) + b.medida
    })
}

// == G2: LOS BUFERES EN VRAM, MAPEADOS ======================================
//
// Donde van (el metal, 24-09 15:22: 26048 KiB, con ATTRIBUTE_CB de 8517 KiB
// alineado a 16 MiB):
//
// ```text
//    VRAM  0x0800_0000 ..  (128 MiB: lo usable empieza en 49; el tramo, 66)
//    VA    0x3_0000_0000 .. (12 GiB: la PD1 del tramo, entrada 24; el tramo
//                           es la 16)
//    tablas 0x0430_0000:  una PD0 y hasta 16 PT (32 MiB de 4 KiB)
// ```
//
// El MISMO desplazamiento en VRAM y en VA, y las dos bases alineadas a 128
// MiB: la alineacion de cada bufer vale en las dos. Paginas de 4 KiB para
// todo (el formato ya VISTO en el metal con el tramo y la copia): las de 64
// KiB y 2 MiB de nouveau son rendimiento, no correccion.
//
// Los que el RM LLENA (`iniciar`) van PRIMERO: asi el kernel los pone a cero
// de un tramo seguido, `[0, cero_hasta)`, sin saber de buferes.

/// Donde empiezan en VRAM.
pub const VRAM: u64 = 0x0800_0000;
/// Donde los ve la GPU.
pub const VA: u64 = 0x3_0000_0000;
/// La PD0 y las PT, en VRAM.
pub const TABLAS: u64 = 0x0430_0000;
/// Cuantas PT caben: 16 x 2 MiB.
pub const PTS: usize = 16;
/// Lo mas que se mapea.
pub const MAX_BYTES: u64 = PTS as u64 * (2 << 20);

/// Un bufer ya colocado: su desplazamiento desde [`VRAM`] y desde [`VA`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Colocado {
    pub b: Bufer,
    pub off: u64,
}

impl Colocado {
    pub const fn vram(&self) -> u64 {
        VRAM + self.off
    }
    pub const fn va(&self) -> u64 {
        VA + self.off
    }
}

/// **El reparto**: primero los que el RM llena, despues los demas, cada uno
/// alineado. `(colocados, total, cero_hasta)`, o `None` si no cabe.
pub fn repartir(t: &[Bufer; N]) -> Option<([Colocado; N], u64, u64)> {
    let mut c = [Colocado::default(); N];
    let mut off = 0u64;
    let mut k = 0;
    let mut cero_hasta = 0;
    for primero in [true, false] {
        for b in t.iter().filter(|b| b.iniciar == primero) {
            let a = 1u64 << b.alinear;
            off = (off + a - 1) & !(a - 1);
            c[k] = Colocado { b: *b, off };
            off += (b.medida + 0xFFF) & !0xFFF;
            k += 1;
        }
        if primero {
            cero_hasta = off;
        }
    }
    (off <= MAX_BYTES).then_some((c, off, cero_hasta))
}

/// La entrada de la PD1 del tramo que cuelga [`VA`] (el tramo usa la 16).
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * crate::mmu::indices(VA)[2] as u64
}

/// **G2: mapear `bytes` de VRAM desde [`VRAM`] en [`VA`]**: la PD0 y las PT a
/// cero, las PTE, las PDE de la PD0 y al final la de la PD1 (de la hoja a la
/// raiz), todo RELEIDO. Solo si esa entrada de la PD1 esta VACIA. Devuelve
/// `(escrituras, releidas iguales)`.
pub fn mapear<R: crate::Registros>(r: &mut R, bytes: u64) -> Option<(u32, u32)> {
    use crate::mmu::{indices, pde_vram, pte_vram};
    use crate::vram::{a_cero, escribir64, leer64};
    if bytes == 0 || bytes > MAX_BYTES || VA % (2 << 20) != 0 {
        return None;
    }
    if leer64(r, entrada_pd1()) != 0 {
        return None;
    }
    let paginas = bytes.div_ceil(0x1000);
    let pts = paginas.div_ceil(512) as usize;
    let pd0 = TABLAS;
    let pt = |k: usize| TABLAS + 0x1000 * (1 + k as u64);
    for k in 0..=pts {
        a_cero(r, TABLAS + 0x1000 * k as u64);
    }
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    for p in 0..paginas {
        // PRIV, como nouveau (`.priv = 1`): son buferes del FECS, no nuestros.
        poner(r, pt((p / 512) as usize) + 8 * (p % 512), pte_vram(VRAM + p * 0x1000) | crate::mmu::PTE_PRIV);
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..pts {
        // La mitad de 4 KiB de la PD0 es la segunda (+8), como el tramo.
        poner(r, pd0 + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(pd0));
    Some((n, bien))
}

// == G3: PROMOTE_CTX -- DARLE AL RM LOS BUFERES ==============================
//
// `r535_gr_promote_ctx(gr, golden = true, ...)` de nouveau, linea a linea:
//
// ```text
//    orden     NV2080_CTRL_CMD_GPU_PROMOTE_CTX (0x2080012B), sobre NUESTRO
//              subdispositivo (el de `vmm->rm.device`, no el interno)
//    cabecera  engineType 1 (GR0), hChanClient = nuestro cliente, hObject =
//              el canal de GR0; hClient, ChID, hVirtMemory, virtAddress y
//              size a CERO
//    entradas  una por bufer, EN EL ORDEN DE LA TABLA (MAIN, PATCH, ...):
//              bufferId; gpuVirtAddr si va mapeado (todos menos
//              PRIV_ACCESS_MAP: `bNonmapped`); y si el RM lo llena
//              (`bInitialize`): gpuPhysAddr, size y physAttr 4
// ```
//
// En el de ORO todos llevan SU memoria (`alloc = golden || !global`), y
// UNRESTRICTED_PRIV_ACCESS_MAP SI va mapeado: solo el id 10 lleva
// `bNonmapped`. Con UN solo canal de GR, el nuestro es a la vez el de oro.

/// `NV2080_CTRL_CMD_GPU_PROMOTE_CTX`.
pub const PROMOVER: u32 = 0x2080_012B;
/// `NV2080_CTRL_GPU_PROMOTE_CTX_MAX_ENTRIES`.
pub const MAX_ENTRADAS: usize = 16;
/// Cabecera de los parametros hasta `promoteEntry` (con el relleno a 8).
pub const PROMOVER_CABECERA: usize = 48;
/// `NV2080_CTRL_GPU_PROMOTE_CTX_BUFFER_ENTRY`: 3 x u64, u32, u16, u8, u8.
pub const ENTRADA: usize = 32;
/// `sizeof(NV2080_CTRL_GPU_PROMOTE_CTX_PARAMS)`: 48 + 16 x 32 = 560.
pub const PROMOVER_MEDIDA: usize = PROMOVER_CABECERA + MAX_ENTRADAS * ENTRADA;
/// `NV2080_ENGINE_TYPE_GR0`.
pub const MOTOR_GR0: u32 = 1;
/// El `physAttr` de nouveau en los que el RM llena.
pub const ATRIBUTO: u32 = 4;

fn poner64(d: &mut [u8], o: usize, v: u64) {
    d[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

/// Una entrada, como la escribe nouveau: `(fisica, virtual, medida, attr,
/// id, iniciar, sin_mapear)`.
pub const fn entrada(c: &Colocado) -> (u64, u64, u64, u32, u16, bool, bool) {
    let nm = sin_mapear(c.b.promover);
    let va = if nm { 0 } else { c.va() };
    if c.b.iniciar {
        (c.vram(), va, c.b.medida, ATRIBUTO, c.b.promover as u16, true, nm)
    } else {
        (0, va, 0, 0, c.b.promover as u16, false, nm)
    }
}

/// **Los 560 B**: la cabecera y las nueve entradas en el orden de [`TABLA`]
/// (el reparto las coloca en otro orden: se buscan por su id).
pub fn parametros_promover(c: &[Colocado; N], p: &mut [u8]) -> Option<usize> {
    let p = p.get_mut(..PROMOVER_MEDIDA)?;
    p.fill(0);
    poner(p, 0, MOTOR_GR0);
    poner(p, 12, crate::objeto::CLIENTE);
    poner(p, 16, crate::canal::GR.asa);
    poner(p, 40, N as u32);
    for (k, &(_, id, ..)) in TABLA.iter().enumerate() {
        let x = c.iter().find(|x| x.b.promover == id)?;
        let (fisica, va, medida, attr, bid, ini, nm) = entrada(x);
        let o = PROMOVER_CABECERA + k * ENTRADA;
        poner64(p, o, fisica);
        poner64(p, o + 8, va);
        poner64(p, o + 16, medida);
        poner(p, o + 24, attr);
        p[o + 28..o + 30].copy_from_slice(&bid.to_le_bytes());
        p[o + 30] = ini as u8;
        p[o + 31] = nm as u8;
    }
    Some(PROMOVER_MEDIDA)
}

/// **La orden** (`GSP_RM_CONTROL`) en `hueco`, sobre nuestro subdispositivo.
pub fn promover(hueco: &mut [u8], numero: u32, c: &[Colocado; N]) -> Option<usize> {
    let mut p = [0u8; PROMOVER_MEDIDA];
    parametros_promover(c, &mut p)?;
    orden::componer(hueco, numero, GSP_RM_CONTROL, CABECERA_CONTROL + PROMOVER_MEDIDA, |d| {
        poner(d, 0, crate::objeto::CLIENTE);
        poner(d, 4, crate::objeto::SUBDISPOSITIVO);
        poner(d, 8, PROMOVER);
        poner(d, 16, PROMOVER_MEDIDA as u32);
        d[CABECERA_CONTROL..CABECERA_CONTROL + PROMOVER_MEDIDA].copy_from_slice(&p);
    })
}

/// **Puede salir?** El contrato no sabe que respondio G0, asi que mira la
/// FORMA: nuestras asas, la cabecera de nouveau, las nueve entradas con los
/// ids, `bInitialize` y `bNonmapped` de la tabla y en su orden, y cada
/// direccion DENTRO de la region de G2 -- la misma distancia de [`VRAM`] que
/// de [`VA`] y sin pasar de [`MAX_BYTES`]. Asi una promocion no puede darle
/// al RM memoria que no es la de G2.
pub fn promover_permitida(d: &[u8]) -> bool {
    if d.len() < CABECERA_CONTROL + PROMOVER_MEDIDA {
        return false;
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let q = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap_or([0xFF; 8]));
    if (u(0), u(4), u(8), u(16) as usize) != (crate::objeto::CLIENTE, crate::objeto::SUBDISPOSITIVO, PROMOVER, PROMOVER_MEDIDA)
        || u(12) != 0
    {
        return false;
    }
    let p = CABECERA_CONTROL;
    let cabecera_bien = u(p) == MOTOR_GR0
        && u(p + 4) == 0
        && u(p + 8) == 0
        && u(p + 12) == crate::objeto::CLIENTE
        && u(p + 16) == crate::canal::GR.asa
        && u(p + 20) == 0
        && q(p + 24) == 0
        && q(p + 32) == 0
        && u(p + 40) == N as u32
        && u(p + 44) == 0;
    if !cabecera_bien {
        return false;
    }
    let dentro = |base: u64, dir: u64, medida: u64| {
        dir >= base && dir % 0x1000 == 0 && dir - base + medida <= MAX_BYTES
    };
    for (k, &(_, id, _, _, iniciar, _)) in TABLA.iter().enumerate() {
        let o = p + PROMOVER_CABECERA + k * ENTRADA;
        let (fisica, va, medida, attr) = (q(o), q(o + 8), q(o + 16), u(o + 24));
        let bid = u16::from_le_bytes([d[o + 28], d[o + 29]]);
        let (ini, nm) = (d[o + 30], d[o + 31]);
        if bid != id as u16 || ini != iniciar as u8 || nm != sin_mapear(id) as u8 {
            return false;
        }
        let va_bien = if sin_mapear(id) { va == 0 } else { dentro(VA, va, medida.max(0x1000)) };
        let fisica_bien = if iniciar {
            attr == ATRIBUTO && medida != 0 && dentro(VRAM, fisica, medida) && (sin_mapear(id) || fisica - VRAM == va - VA)
        } else {
            fisica == 0 && medida == 0 && attr == 0
        };
        if !va_bien || !fisica_bien {
            return false;
        }
    }
    // Las siete entradas que sobran, a cero.
    d[p + PROMOVER_CABECERA + N * ENTRADA..p + PROMOVER_MEDIDA].iter().all(|&b| b == 0)
}

// == G4: AMPERE_B EN EL CANAL -- EL CONTEXTO DE ORO ==========================
//
// `nvkm_gsp_rm_alloc(&golden.chan, NVKM_RM_THREED, threed, 0, &threed)`: la
// clase 3D del GA10x (`rm->gpu->gr.class.threed`, AMPERE_B 0xC797) colgada
// del canal, SIN parametros. Al crearla, el RM corre el contexto de ORO con
// los buferes de G3 y lo guarda; nouveau la suelta y suelta el canal (el RM
// ya lo tiene). Nosotros NO: el canal y el objeto son los del triangulo.

/// `AMPERE_B`.
pub const AMPERE_B: u32 = 0xC797;
/// Nuestra asa del objeto 3D.
pub const TRESDE: u32 = 0xC797_0002;

/// `(hClient, hParent, hObject, hClass, medida)`, como `copia::forma`.
pub const fn forma_tresde() -> (u32, u32, u32, u32, usize) {
    (crate::objeto::CLIENTE, crate::canal::GR.asa, TRESDE, AMPERE_B, 0)
}

/// **La pregunta** (`GSP_RM_ALLOC`), sin parametros.
pub fn pedir_tresde(hueco: &mut [u8], numero: u32) -> Option<usize> {
    let (cliente, padre, asa, clase, _) = forma_tresde();
    orden::componer(hueco, numero, crate::objeto::GSP_RM_ALLOC, crate::objeto::CABECERA_ALLOC, |d| {
        poner(d, 0, cliente);
        poner(d, 4, padre);
        poner(d, 8, asa);
        poner(d, 12, clase);
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn la_pregunta_va_al_cliente_interno_y_suma_cero() {
        let mut h = [0xAAu8; 4096];
        let n = pedir(&mut h, 21, 0xC200_0006, 0xABCD_2080).unwrap();
        assert_eq!(n, CABECERA + CABECERA_CONTROL + 1664);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero), (GSP_RM_CONTROL, 21));
        let mut s = Suma::default();
        s.mas(&h[..m.bytes_sumados()]);
        assert_eq!(s.valor(), 0);
        assert!(permitida(&h[CABECERA..n]));
        // Con un parametro distinto de cero, u otra orden, no.
        let mut otra = h;
        otra[CABECERA + CABECERA_CONTROL + 7] = 1;
        assert!(!permitida(&otra[CABECERA..n]));
        let mut otra = h;
        otra[CABECERA + 8] = 0x33;
        assert!(!permitida(&otra[CABECERA..n]));
    }

    /// Una VRAM de mentira de 8 MiB desde el DIRECTORIO (las tablas del tramo
    /// y las de G2), por la ventana PRAMIN.
    struct Falsa {
        ventana: u32,
        vram: std::vec::Vec<u32>,
    }
    extern crate std;

    impl crate::Registros for Falsa {
        fn leer(&mut self, reg: u32) -> u32 {
            if reg == crate::vram::VENTANA_REG {
                return self.ventana;
            }
            let dir = ((self.ventana as u64) << 16) + (reg - crate::vram::VENTANA) as u64;
            self.vram.get(((dir - crate::vram::DIRECTORIO) / 4) as usize).copied().unwrap_or(0)
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            if reg == crate::vram::VENTANA_REG {
                self.ventana = v;
                return;
            }
            let dir = ((self.ventana as u64) << 16) + (reg - crate::vram::VENTANA) as u64;
            if let Some(c) = self.vram.get_mut(((dir - crate::vram::DIRECTORIO) / 4) as usize) {
                *c = v;
            }
        }
    }

    /// Las medidas que dijo el RM en el metal (24-09 15:22).
    fn del_metal() -> [Bufer; N] {
        let mut d = [0u8; CABECERA_CONTROL + MEDIDA];
        for (id, m) in [(0x00, 694016u32), (0x10, 16384), (0x11, 12288), (0x0D, 131072), (0x13, 8720896), (0x14, 524288), (0x17, 65536), (0x18, 524288)] {
            let o = CABECERA_CONTROL + 8 * id;
            d[o..o + 4].copy_from_slice(&m.to_le_bytes());
        }
        buferes(&d).unwrap()
    }

    #[test]
    fn el_reparto_del_metal_cabe() {
        let t = del_metal();
        assert_eq!(total(&t), 26048 * 1024 + 512 * 1024, "la fila `gr` del metal (sin la novena) mas los 512 KiB de UNRESTRICTED");
        assert_eq!((t[8].promover, t[8].medida), (11, t[7].medida));
        assert!(sin_mapear(t[7].promover) && !sin_mapear(t[8].promover));
        let (c, bytes, cero) = repartir(&t).unwrap();
        // Los que el RM llena, primero y seguidos.
        assert!(c[..5].iter().all(|x| x.b.iniciar) && c[5..].iter().all(|x| !x.b.iniciar));
        assert!(c[..5].iter().all(|x| x.off + x.b.medida <= cero));
        for x in &c {
            assert_eq!(x.off % (1 << x.b.alinear), 0, "{:?} alineado", core::str::from_utf8(x.b.nombre));
            assert_eq!(x.vram() % (1 << x.b.alinear), 0);
            assert_eq!(x.va() % (1 << x.b.alinear), 0);
        }
        // Sin solapes.
        for i in 0..N {
            for j in i + 1..N {
                let (a, b) = (c[i], c[j]);
                assert!(a.off + a.b.medida <= b.off || b.off + b.b.medida <= a.off);
            }
        }
        assert!(bytes <= MAX_BYTES && cero < bytes);
        // Y todo cae en lo usable del metal (0x003110000..0x2F06DFFFF), lejos
        // del tramo (0x4200000) y de las tablas.
        assert!(VRAM >= 0x0311_0000 && VRAM + bytes < 0x2_F06D_FFFF);
        assert!(TABLAS + 0x1000 * (PTS as u64 + 1) <= VRAM && TABLAS >= crate::vram::TRAMO + 0x10000);
    }

    #[test]
    fn el_mapeo_de_g2_se_relee() {
        let mut f = Falsa { ventana: 0xFFF0, vram: std::vec![0u32; 2 << 20] };
        let (_, bytes, _) = repartir(&del_metal()).unwrap();
        let (n, bien) = mapear(&mut f, bytes).unwrap();
        assert_eq!(n, bien);
        let paginas = bytes.div_ceil(0x1000);
        assert_eq!(n as u64, paginas + paginas.div_ceil(512) + 1);
        assert_eq!(f.ventana, 0xFFF0);
        // La PD1 del tramo apunta a nuestra PD0, y la primera PTE a VRAM.
        assert_eq!(crate::vram::leer64(&mut f, entrada_pd1()), crate::mmu::pde_vram(TABLAS));
        assert_eq!(crate::vram::leer64(&mut f, TABLAS + 0x1000), crate::mmu::pte_vram(VRAM) | crate::mmu::PTE_PRIV);
        // No toca la entrada del tramo.
        assert_ne!(entrada_pd1(), crate::vram::TABLAS[1] + 8 * crate::mmu::indices(crate::vram::TRAMO_VA)[2] as u64);
        // Otra vez: la entrada ya esta ocupada, no se pisa.
        assert_eq!(mapear(&mut f, bytes), None);
    }

    #[test]
    fn los_buferes_como_nouveau() {
        let mut d = [0u8; CABECERA_CONTROL + MEDIDA];
        let pon = |d: &mut [u8], id: usize, medida: u32| {
            let o = CABECERA_CONTROL + 8 * id;
            d[o..o + 4].copy_from_slice(&medida.to_le_bytes());
        };
        pon(&mut d, 0x00, 0x0003_1234); // MAIN
        pon(&mut d, 0x13, 0x0030_0000); // ATTRIBUTE_CB, 3 MiB
        pon(&mut d, 0x18, 0x1000); // PRIV_ACCESS_MAP
        // El motor 1 no cuenta.
        pon(&mut d, IDS, 0xFFFF_FFFF);
        let t = buferes(&d).unwrap();
        assert_eq!((t[0].nombre, t[0].promover), (b"MAIN" as &[u8], 0));
        assert_eq!((t[7].promover, t[6].promover, t[1].promover), (10, 9, 2));
        assert_eq!(t[0].medida, 0x0003_2000 + 64 * 0x1000, "alineada a 4 KiB y 64 cabeceras");
        assert_eq!((t[0].pagina, t[0].alinear), (16, 16));
        assert_eq!(t[4].nombre, b"ATTRIBUTE_CB");
        assert_eq!((t[4].pagina, t[4].alinear), (21, 22), "order_base_2 de 3 MiB");
        assert_eq!((t[7].pagina, t[7].ro, t[7].iniciar), (12, true, true));
        assert!(total(&t) >= t[0].medida + t[4].medida + t[7].medida);
        assert_eq!(orden_base_2(1 << 21), 21);
        assert_eq!(orden_base_2((1 << 21) + 1), 22);
    }

    #[test]
    fn la_promocion_como_nouveau() {
        let t = del_metal();
        let (c, ..) = repartir(&t).unwrap();
        let mut h = [0u8; 4096];
        let n = promover(&mut h, 30, &c).unwrap();
        assert_eq!(n, CABECERA + CABECERA_CONTROL + PROMOVER_MEDIDA);
        let d = &h[CABECERA..n];
        assert!(promover_permitida(d));
        let p = &d[CABECERA_CONTROL..];
        let u = |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        let q = |o: usize| u64::from_le_bytes(p[o..o + 8].try_into().unwrap());
        assert_eq!((u(0), u(12), u(16), u(40)), (1, crate::objeto::CLIENTE, crate::canal::GR.asa, 9));
        for (k, &(_, id, _, _, ini, _)) in TABLA.iter().enumerate() {
            let o = PROMOVER_CABECERA + k * ENTRADA;
            let x = c.iter().find(|x| x.b.promover == id).unwrap();
            assert_eq!(u16::from_le_bytes([p[o + 28], p[o + 29]]), id as u16);
            assert_eq!(p[o + 30], ini as u8);
            // Solo PRIV_ACCESS_MAP va sin mapear; su copia UNRESTRICTED, si.
            assert_eq!(p[o + 31], (id == 10) as u8);
            assert_eq!(q(o + 8), if id == 10 { 0 } else { x.va() });
            if ini {
                assert_eq!((q(o), q(o + 16), u(o + 24)), (x.vram(), x.b.medida, 4));
            } else {
                assert_eq!((q(o), q(o + 16), u(o + 24)), (0, 0, 0));
            }
        }
    }

    #[test]
    fn la_promocion_no_sale_de_g2() {
        let (c, ..) = repartir(&del_metal()).unwrap();
        let mut h = [0u8; 4096];
        let n = promover(&mut h, 30, &c).unwrap();
        let base = CABECERA + CABECERA_CONTROL + PROMOVER_CABECERA;
        let q = |h: &[u8], o: usize| u64::from_le_bytes(h[o..o + 8].try_into().unwrap());
        let cambia = |o: usize, v: u64| {
            let mut m = h;
            m[o..o + 8].copy_from_slice(&v.to_le_bytes());
            promover_permitida(&m[CABECERA..n])
        };
        // MAIN (entrada 0) fuera de la VRAM de G2, o con fisica y virtual
        // desparejadas: no sale.
        assert!(!cambia(base, 0x0100_0000));
        assert!(!cambia(base, q(&h, base) + 0x1000));
        // Una medida que pasa del final.
        assert!(!cambia(base + 16, MAX_BYTES + 0x1000));
        // Una VA en otro sitio.
        assert!(!cambia(base + ENTRADA * 3 + 8, 0x1_0000_0000));
        // Otro canal o una decima entrada.
        let mut m = h;
        m[CABECERA + CABECERA_CONTROL + 16] ^= 1;
        assert!(!promover_permitida(&m[CABECERA..n]));
        let mut m = h;
        m[base + N * ENTRADA + 28] = 1;
        assert!(!promover_permitida(&m[CABECERA..n]));
        // Y el contrato entero la deja pasar tal cual.
        assert!(crate::contrato::permitido(&h[..n]).is_ok());
    }

    #[test]
    fn las_medidas_dan_la_misma_tabla_que_la_respuesta() {
        let t = del_metal();
        let mut m = [(0u32, 0u32); DISTINTOS];
        for (k, x) in m.iter_mut().enumerate() {
            *x = (t[k].medida_rm, t[k].alineado_rm);
        }
        assert_eq!(desde_medidas(&m), t);
    }

    #[test]
    fn el_tresde_va_sin_parametros_y_el_contrato_lo_deja() {
        let mut h = [0u8; 256];
        let n = pedir_tresde(&mut h, 31).unwrap();
        assert_eq!(n, CABECERA + crate::objeto::CABECERA_ALLOC);
        let u = |o: usize| u32::from_le_bytes(h[CABECERA + o..CABECERA + o + 4].try_into().unwrap());
        assert_eq!((u(4), u(8), u(12), u(20)), (crate::canal::GR.asa, TRESDE, 0xC797, 0));
        assert!(crate::contrato::permitido(&h[..n]).is_ok());
    }

}
