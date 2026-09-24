//! **M5 T1b + T1c: EL TRIANGULO POR EL RASTERIZADOR DE HARDWARE** -- tres
//! vertices entran por el pipeline 3D de AMPERE_B: un programa de VERTICE los
//! coloca, el RASTERIZADOR de la 3060 decide que pixeles quedan dentro, un
//! programa de PIXEL les da color y el ROP los escribe. Ni computo ni
//! funciones de arista hechas a mano: el hardware de triangulos.
//!
//! capa: puro -- los dos programas, sus cabeceras, las ordenes y la cuenta de
//! referencia; la RAM y los registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- cada metodo de `clc797.h`; cada campo de la
//!           cabecera, de la especificacion SPH de NVIDIA (open-gpu-doc); cada
//!           instruccion, desensamblada con `nvdisasm -b SM86`
//!
//! # Los escalones
//!
//! ```text
//!    T1a  limpiar el destino con el ROP, sin programas         (tresde.rs)
//!    T1b  los programas de vertice y de pixel, con su SPH      (aqui)
//!    T1c  el triangulo por el rasterizador, con juez           (aqui)
//! ```
//!
//! # Los programas (SASS de SM86, comprobados con `nvdisasm`)
//!
//! `ptxas` solo compila computo, asi que el de VERTICE sale de un programa de
//! computo con la MISMA logica (elegir el vertice por su numero) y se cambian
//! dos instrucciones, con los campos encontrados probando contra `nvdisasm`:
//!
//! ```text
//!    S2R R0, SR_TID.X          ->  ALD R0, a[0x2fc]          el numero de vertice
//!    STG.E.128 [R2.64], R4     ->  AST.128 a[0x70], RZ, R4   la posicion (x,y,z,w)
//!
//!    ALD  0x321  destino 16..24, vertice 32..40, atributo 40..50, cuantos-1 74..76
//!    AST  0x322  dato 64..72, vertice 32..40, atributo 40..50, cuantos-1 74..76
//! ```
//!
//! (y los NOP de siempre en las lecturas de la cb0 de CUDA; el control de
//! cada instruccion, el que puso `ptxas`). El de PIXEL son cuatro MOV: en un
//! programa de pixel el color SALE en R0..R3 al terminar (lo dice su SPH).
//!
//! # Los vertices y el juez
//!
//! Los del triangulo por computo: v0 (256, 40), v1 (472, 440), v2 (40, 440),
//! en coordenadas normalizadas exactas (x/256 - 1). El rasterizador toma el
//! centro de cada pixel (x + 0.5, y + 0.5): con vertices enteros, NINGUN
//! centro cae justo en una arista (las cuentas dobles salen siempre impares:
//! ver la prueba), asi que la regla de desempate no importa y la CPU sabe
//! EXACTAMENTE que pixeles tienen que salir verdes.

use crate::canal::GR;
use crate::copia::{cabecera_en, entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::fractal::{LADO, PIXELES};
use crate::lienzo::sombreador_va;
use crate::sombreador::{EMPUJE, PROGRAMA, SEMAFOROS};
use crate::triangulo::{arista, V0, V1, V2};
use crate::tresde::{self as td, SUBCANAL};
use crate::vram::a_cero;
use crate::Registros;

// Los metodos de `clc797.h` que T1a no usaba.
pub const INVALIDATE_SHADER_CACHES: u32 = 0x021c;
pub const SET_RASTER_ENABLE: u32 = 0x037c;
pub const SET_STREAM_OUTPUT: u32 = 0x0744;
pub const SET_SHADER_LOCAL_MEMORY_WINDOW: u32 = 0x077c;
pub const SET_VIEWPORT_SCALE_X0: u32 = 0x0a00;
pub const SET_VIEWPORT_CLIP_HORIZONTAL0: u32 = 0x0c00;
pub const SET_VERTEX_ARRAY_START: u32 = 0x0d74;
pub const SET_VERTEX_STREAM_SUBSTITUTE_A: u32 = 0x0f84;
pub const SET_SAMPLE_MASK_X0_Y0: u32 = 0x0fbc;
pub const SET_VERTEX_ID_BASE: u32 = 0x1118;
pub const SET_VERTEX_ATTRIBUTE_A0: u32 = 0x1160;
pub const SET_DEPTH_TEST: u32 = 0x12cc;
pub const SET_ALPHA_TEST: u32 = 0x12ec;
pub const SET_BLEND0: u32 = 0x1360;
pub const SET_STENCIL_TEST: u32 = 0x1380;
pub const SET_WINDOW_ORIGIN: u32 = 0x13ac;
pub const SET_ANTI_ALIAS_ENABLE: u32 = 0x1534;
pub const SET_ZT_SELECT: u32 = 0x1538;
pub const SET_ANTI_ALIAS: u32 = 0x15d0;
pub const END: u32 = 0x1614;
pub const BEGIN: u32 = 0x1618;
pub const SET_SPH_VERSION: u32 = 0x16a4;
pub const OGL_SET_CULL: u32 = 0x1918;
pub const SET_VIEWPORT_PIXEL: u32 = 0x1924;
pub const SET_VIEWPORT_SCALE_OFFSET: u32 = 0x192c;
pub const SET_VIEWPORT_CLIP_CONTROL: u32 = 0x193c;
pub const SET_RENDER_ENABLE_OVERRIDE: u32 = 0x1944;
pub const SET_DEPTH_BOUNDS_TEST: u32 = 0x19bc;
pub const SET_VERTEX_STREAM_A_FORMAT0: u32 = 0x1c00;
pub const fn set_pipeline_shader(j: u32) -> u32 {
    0x2000 + j * 64
}

/// `SET_PIPELINE_SHADER_TYPE`: el hueco `j` lleva el tipo `j`.
pub const VERTICE: u32 = 1;
pub const PIXEL: u32 = 5;
/// `BEGIN_OP_TRIANGLES`.
pub const TRIANGULOS: u32 = 4;
/// `INVALIDATE_SHADER_CACHES`: INSTRUCTION, DATA y CONSTANT (el mismo sitio
/// lo ocupo antes un programa de computo).
pub const INVALIDAR_TODO: u32 = 1 | 1 << 4 | 1 << 12;
/// `SET_VERTEX_ATTRIBUTE_A_SOURCE_INACTIVE`: ningun atributo sale de memoria.
pub const ATRIBUTO_APAGADO: u32 = 1 << 6;
/// `SET_VIEWPORT_CLIP_CONTROL`: z sin recorte (-inf..+inf, 17:16 = 3) y
/// sujeta en los pixeles (bits 3 y 4); el resto, lo de fabrica (0).
pub const RECORTE_Z: u32 = 3 << 16 | 1 << 4 | 1 << 3;
/// `SET_VIEWPORT_COORDINATE_SWIZZLE`: X, Y, Z y W tal cual.
pub const SIN_CRUZAR: u32 = 6 << 12 | 4 << 8 | 2 << 4;

pub const UNO: u32 = 0x3F80_0000;
pub const MEDIO: u32 = 0x3F00_0000;
pub const F256: u32 = 0x4380_0000;
/// El color del programa de pixel: R = 0, G = 1, B = 0, A = 1.
pub const PIXEL_VERDE: u32 = 0xFF00_FF00;

/// Donde van: los dos en la pagina del programa de computo, alineados.
pub const VS: u64 = PROGRAMA;
pub const PS: u64 = PROGRAMA + 0x400;
pub const REGISTROS: u32 = 16;

/// `SET_SPH_VERSION`: la version de las cabeceras que se dan (3, la de
/// nouveau y NVK), en `CURRENT` (15:0) y `OLDEST_SUPPORTED` (31:16).
pub const VERSION_SPH: u32 = 3 << 16 | 3;
/// La ventana de memoria local de la clase 3D (>> 8): la del computo,
/// 0xFF_0000_0000, lejos de todo lo mapeado. Como NVK.
pub const VENTANA_LOCAL: u32 = 0xFF00_0000;
/// Una pagina a cero para los atributos sin flujo (la del QMD, que el
/// rasterizador no usa): nunca se lee de la direccion 0.
pub const SUSTITUTO: u64 = crate::sombreador::QMD;

pub const PAGA_FIN: u32 = 0x3060_7A1C;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0xD0;

/// **El programa de VERTICE** (17 instrucciones): el numero de vertice elige
/// la posicion, que sale por `a[0x70]`.
pub const CODIGO_VS: [(u64, u64); 17] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP
    (0x0002fcffff007321, 0x000e220000000000), // ALD R0, a[0x2fc]
    (0x3f380000ff057424, 0x000fe200078e00ff), // MOV R5, 0.71875
    (0x0000000000007918, 0x000fe20000000000), // NOP
    (0x0000000000007918, 0x000fe40000000000), // NOP
    (0x0000000000007918, 0x000fe40000000000), // NOP
    (0x3f800000ff077424, 0x000fe400078e00ff), // MOV R7, 1.0 (w)
    (0x000000ffff067224, 0x000fe200078e00ff), // MOV R6, RZ (z)
    (0x000000010000780c, 0x001fc40003f05070), // ISETP.NE.U32.AND P0, PT, R0, 0x1, PT
    (0x000000020000780c, 0x000fe40003f25070), // ISETP.NE.U32.AND P1, PT, R0, 0x2, PT
    (0xbf58000005057807, 0x000fe40004000000), // SEL R5, R5, -0.84375, !P0
    (0x3f580000ff007807, 0x000fe40000000000), // SEL R0, RZ, 0.84375, P0
    (0x3f38000005057807, 0x000fe40000800000), // SEL R5, R5, 0.71875, P1
    (0xbf58000000047807, 0x000fca0000800000), // SEL R4, R0, -0.84375, P1
    (0x000070ffff007322, 0x000fe20000000c04), // AST.128 a[0x70], RZ, R4
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA . (el relleno de ptxas)
];

/// **El programa de PIXEL** (6 instrucciones): el color en R0..R3.
pub const CODIGO_PS: [(u64, u64); 6] = [
    (0x00000000ff007424, 0x000fe200078e00ff), // MOV R0, 0 (rojo)
    (0x3f800000ff017424, 0x000fe200078e00ff), // MOV R1, 1.0 (verde)
    (0x00000000ff027424, 0x000fe200078e00ff), // MOV R2, 0 (azul)
    (0x3f800000ff037424, 0x000fca00078e00ff), // MOV R3, 1.0 (alfa)
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA .
];

/// Las 20 palabras de la cabecera (SPH).
pub const SPH: usize = 20;

/// Pone a 1 el bit `b` de la cabecera.
pub const fn bit(mut h: [u32; SPH], b: usize) -> [u32; SPH] {
    h[b / 32] |= 1 << (b % 32);
    h
}

/// `CommonWord0`: SphType (4:0), Version (9:5) = 3, ShaderType (13:10) y
/// SassVersion (20:17) = 1 -- los de nouveau (`0x20061 | 1 << 10`).
pub const fn palabra0(tipo_sph: u32, tipo: u32) -> u32 {
    tipo_sph | 3 << 5 | tipo << 10 | 1 << 17
}

/// **La SPH del de vertice** (tipo 1, VTG): lee `ImapVertexId` (bit 351) y
/// escribe `OmapPositionX..W` (bits 428..431). StoreReqStart (19:12 de la
/// palabra 4) mayor que StoreReqEnd: ninguno se relee.
pub const fn sph_vertice() -> [u32; SPH] {
    let mut h = [0u32; SPH];
    h[0] = palabra0(1, VERTICE);
    h[4] = 0xFF << 12;
    h = bit(h, 351);
    let mut k = 428;
    while k < 432 {
        h = bit(h, k);
        k += 1;
    }
    h
}

/// **La SPH del de pixel** (tipo 2, PS): escribe el destino 0 entero
/// (`OmapTarget[0]`, bits 576..579) y declara `ImapPositionW` (bit 191),
/// que nouveau pone siempre ("trap si FRAG_COORD.w = 0").
pub const fn sph_pixel() -> [u32; SPH] {
    let mut h = [0u32; SPH];
    h[0] = palabra0(2, PIXEL);
    h = bit(h, 191);
    let mut k = 576;
    while k < 580 {
        h = bit(h, k);
        k += 1;
    }
    h
}

pub const PALABRAS_VS: usize = SPH + CODIGO_VS.len() * 4;
pub const PALABRAS_PS: usize = SPH + CODIGO_PS.len() * 4;

/// Cabecera y codigo, en palabras, tal como van a memoria.
pub const fn programa<const N: usize>(sph: [u32; SPH], codigo: &[(u64, u64)]) -> [u32; N] {
    let mut w = [0u32; N];
    let mut k = 0;
    while k < SPH {
        w[k] = sph[k];
        k += 1;
    }
    let mut i = 0;
    while i < codigo.len() {
        let (lo, hi) = codigo[i];
        w[SPH + 4 * i] = lo as u32;
        w[SPH + 4 * i + 1] = (lo >> 32) as u32;
        w[SPH + 4 * i + 2] = hi as u32;
        w[SPH + 4 * i + 3] = (hi >> 32) as u32;
        i += 1;
    }
    w
}

pub const fn vertice() -> [u32; PALABRAS_VS] {
    programa(sph_vertice(), &CODIGO_VS)
}

pub const fn pixel() -> [u32; PALABRAS_PS] {
    programa(sph_pixel(), &CODIGO_PS)
}

// == Las ordenes =============================================================

/// T1a sin su semaforo: el destino, el recorte, la limpieza a magenta.
pub const PREFIJO: usize = td::ORDENES - 5;
pub const ORDENES: usize = 193;

struct Empuje {
    o: [u32; ORDENES],
    n: usize,
}

impl Empuje {
    /// Un metodo con `v.len()` valores seguidos (metodo, metodo + 4, ...).
    fn m(&mut self, metodo: u32, v: &[u32]) {
        self.o[self.n] = cabecera_en(SUBCANAL, metodo, v.len() as u32);
        self.o[self.n + 1..self.n + 1 + v.len()].copy_from_slice(v);
        self.n += 1 + v.len();
    }

    /// Un semaforo de informe tras todas las escrituras.
    fn semaforo(&mut self, donde: u64, paga: u32) {
        let s = sombreador_va(donde);
        self.m(td::SET_REPORT_SEMAPHORE_A, &[(s >> 32) as u32, s as u32, paga, td::INFORME]);
    }

    /// EL DIBUJO: tres vertices, un triangulo.
    fn dibujo(&mut self) {
        self.m(BEGIN, &[TRIANGULOS]);
        self.m(SET_VERTEX_ARRAY_START, &[0, 3]);
        self.m(END, &[0]);
    }
}

/// **Las ordenes**: limpiar (T1a), el estado 3D, los dos programas, el
/// dibujo de TRES vertices y el semaforo tras todas las escrituras.
pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(SEMAFORO_FIN, PAGA_FIN)
}

/// Las mismas, con otro semaforo y otra paga (T2 las usa enteras: solo
/// cambian los programas que hay en `VS` y `PS`).
pub fn ordenes_con(semaforo: u64, paga: u32) -> [u32; ORDENES] {
    let mut e = Empuje { o: [0; ORDENES], n: PREFIJO };
    e.o[..PREFIJO].copy_from_slice(&td::ordenes()[..PREFIJO]);
    e.m(INVALIDATE_SHADER_CACHES, &[INVALIDAR_TODO]);
    // ** Metal 24-09 17:56: sin esto el dibujo colgo el canal (el semaforo sin
    // pagar). Lo que NVK pone SIEMPRE al empezar un contexto 3D y aqui
    // faltaba: la version de SPH, la ventana local, el sustituto de los
    // atributos, y dibujar pase lo que pase con el render condicional.
    e.m(SET_SPH_VERSION, &[VERSION_SPH]);
    e.m(SET_SHADER_LOCAL_MEMORY_WINDOW, &[VENTANA_LOCAL]);
    let z = sombreador_va(SUSTITUTO);
    e.m(SET_VERTEX_STREAM_SUBSTITUTE_A, &[(z >> 32) as u32, z as u32]);
    e.m(SET_RENDER_ENABLE_OVERRIDE, &[1]);
    // El viewport 0: escala y desplazamiento de 256 (de -1..1 a 0..512), z
    // de 0 a 1, sin cruzar ejes; y su recorte, el destino entero.
    e.m(SET_VIEWPORT_SCALE_X0, &[F256, F256, MEDIO, F256, F256, MEDIO, SIN_CRUZAR]);
    e.m(SET_VIEWPORT_SCALE_OFFSET, &[1]);
    e.m(SET_VIEWPORT_CLIP_HORIZONTAL0, &[LADO << 16, LADO << 16, 0, UNO]);
    e.m(SET_VIEWPORT_CLIP_CONTROL, &[RECORTE_Z]);
    // Origen arriba a la izquierda (y crece hacia abajo, como el escritorio)
    // y el centro del pixel en el medio.
    e.m(SET_WINDOW_ORIGIN, &[0]);
    e.m(SET_VIEWPORT_PIXEL, &[0]);
    // Nada entre el rasterizador y el ROP: sin caras ocultas, sin
    // profundidad, sin plantilla, sin mezcla, sin multimuestreo.
    e.m(OGL_SET_CULL, &[0]);
    e.m(SET_ZT_SELECT, &[0]);
    e.m(SET_DEPTH_TEST, &[0]);
    e.m(SET_STENCIL_TEST, &[0]);
    e.m(SET_DEPTH_BOUNDS_TEST, &[0]);
    e.m(SET_ALPHA_TEST, &[0]);
    e.m(SET_BLEND0, &[0]);
    e.m(SET_ANTI_ALIAS_ENABLE, &[0]);
    e.m(SET_ANTI_ALIAS, &[0]);
    e.m(SET_SAMPLE_MASK_X0_Y0, &[0xFFFF; 4]);
    e.m(SET_STREAM_OUTPUT, &[0]);
    e.m(SET_RASTER_ENABLE, &[1]);
    // Ningun atributo ni flujo de vertices en memoria: el programa saca la
    // posicion del NUMERO de vertice.
    e.m(SET_VERTEX_ATTRIBUTE_A0, &[ATRIBUTO_APAGADO; 32]);
    e.m(SET_VERTEX_STREAM_A_FORMAT0, &[0]);
    e.m(SET_VERTEX_ID_BASE, &[0]);
    // Los seis huecos del pipeline: solo el de vertice (1) y el de pixel (5).
    // SHADER, RESERVED_B, RESERVED_A, REGISTER_COUNT, BINDING, ADDRESS_A/B.
    let mut j = 0;
    while j < 6 {
        let (vivo, dir, grupo) = match j {
            VERTICE => (1, sombreador_va(VS), 0),
            PIXEL => (1, sombreador_va(PS), 4),
            _ => (0, 0, 0),
        };
        if vivo == 1 {
            e.m(set_pipeline_shader(j), &[j << 4 | 1, 0, 0, REGISTROS, grupo, (dir >> 32) as u32, dir as u32]);
        } else {
            e.m(set_pipeline_shader(j), &[j << 4]);
        }
        j += 1;
    }
    // ** Metal 24-09 18:06: el dibujo se quedo esperando SIN excepcion (el
    // GSP no conto ningun Xid). Una ESCALERA de semaforos dice hasta donde
    // llego en UN arranque: (1) el estado aceptado, (2) el dibujo con el
    // rasterizador APAGADO -- solo corre el programa de vertice --, (3) el
    // dibujo entero. El primero sin pagar es la etapa que se cuelga.
    e.m(td::WAIT_FOR_IDLE, &[0]);
    e.semaforo(semaforo + ESTADO, paga ^ ESTADO_PAGA);
    e.m(SET_RASTER_ENABLE, &[0]);
    e.dibujo();
    e.m(td::WAIT_FOR_IDLE, &[0]);
    e.semaforo(semaforo + VERTICES, paga ^ VERTICES_PAGA);
    e.m(SET_RASTER_ENABLE, &[1]);
    e.dibujo();
    e.m(td::WAIT_FOR_IDLE, &[0]);
    e.semaforo(semaforo, paga);
    debug_assert!(e.n == ORDENES);
    e.o
}

/// Donde van los escalones, tras el semaforo final, y lo que pagan.
pub const ESTADO: u64 = 4;
pub const VERTICES: u64 = 8;
pub const ESTADO_PAGA: u32 = 0xE500;
pub const VERTICES_PAGA: u32 = 0x7E00;

/// **Hasta donde llego**: bit 0 el estado, bit 1 los vertices, bit 2 el
/// dibujo entero.
pub fn etapas<R: Registros>(r: &mut R, semaforo: u64, paga: u32) -> u32 {
    (leer32(r, semaforo + ESTADO) == paga ^ ESTADO_PAGA) as u32
        | ((leer32(r, semaforo + VERTICES) == paga ^ VERTICES_PAGA) as u32) << 1
        | ((leer32(r, semaforo) == paga) as u32) << 2
}

/// **Preparar** con la entrada `e` del GPFIFO de GR: el semaforo a cero, los
/// dos programas en su pagina, las ordenes y la entrada.
pub fn preparar<R: Registros>(r: &mut R, e: u32) -> bool {
    preparar_con(r, e, &vertice(), &pixel(), SEMAFORO_FIN, PAGA_FIN)
}

/// Lo mismo con otros dos programas (cabecera y codigo), otro semaforo y
/// otra paga.
pub fn preparar_con<R: Registros>(r: &mut R, e: u32, vs: &[u32], ps: &[u32], semaforo: u64, paga: u32) -> bool {
    if !crate::blur::entrada_valida(e) || vs.len() * 4 > (PS - VS) as usize || ps.len() * 4 > 4096 - (PS - VS) as usize {
        return false;
    }
    let o = ordenes_con(semaforo, paga);
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    escribir(r, semaforo, &[0; 4]) == 4
        && a_cero(r, PROGRAMA) as usize == crate::vram::PALABRAS
        && a_cero(r, SUSTITUTO) as usize == crate::vram::PALABRAS
        && escribir(r, VS, vs) == vs.len()
        && escribir(r, PS, ps) == ps.len()
        && escribir(r, EMPUJE, &o) == ORDENES
        && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = crate::blur::entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[e + 1]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    mirar_en(r, SEMAFORO_FIN)
}

pub fn mirar_en<R: Registros>(r: &mut R, semaforo: u64) -> (u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, semaforo))
}

// == El juez =================================================================

/// La arista en el CENTRO del pixel, con todo al doble para seguir entero.
pub const fn arista_centro(a: (i32, i32), b: (i32, i32), x: u32, y: u32) -> i32 {
    arista((2 * a.0, 2 * a.1), (2 * b.0, 2 * b.1), 2 * x as i32 + 1, 2 * y as i32 + 1)
}

/// Si el centro de `(x, y)` queda dentro del triangulo.
pub const fn dentro(x: u32, y: u32) -> bool {
    arista_centro(V1, V2, x, y) > 0 && arista_centro(V2, V0, x, y) > 0 && arista_centro(V0, V1, x, y) > 0
}

/// **Lo que tiene que haber en `(x, y)`**: verde dentro, el magenta de la
/// limpieza fuera.
pub const fn esperado(x: u32, y: u32) -> u32 {
    if dentro(x, y) {
        PIXEL_VERDE
    } else {
        td::PIXEL_LIMPIO
    }
}

/// Cuantos pixeles son lo que tienen que ser.
pub fn comprobar(salida: &[u32]) -> u32 {
    salida.iter().take(PIXELES).enumerate().filter(|&(k, &p)| p == esperado(k as u32 % LADO, k as u32 / LADO)).count() as u32
}

/// Cuantos pixeles salieron del verde del programa de pixel (lo pinto el
/// rasterizador, bien o mal).
pub fn verdes(salida: &[u32]) -> u32 {
    salida.iter().take(PIXELES).filter(|&&p| p == PIXEL_VERDE).count() as u32
}

pub use crate::fractal::{desempaquetar, empaquetar, sano};

const _: () = assert!(PALABRAS_VS * 4 <= (PS - VS) as usize && PALABRAS_PS * 4 <= 4096 - (PS - VS) as usize);
const _: () = assert!(ORDENES * 4 <= 4096);
const _: () = assert!(SEMAFORO_FIN > crate::escena::SEMAFORO_FIN && SEMAFORO_FIN + 16 <= SEMAFOROS + 4096);

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;

    #[test]
    fn las_ordenes_cuadran() {
        let o = ordenes();
        assert_eq!(o[..PREFIJO], td::ordenes()[..PREFIJO]);
        // Lo ultimo: el semaforo, con la paga y el informe de T1a.
        assert_eq!(o[ORDENES - 5], cabecera_en(0, td::SET_REPORT_SEMAPHORE_A, 4));
        assert_eq!((o[ORDENES - 2], o[ORDENES - 1]), (PAGA_FIN, td::INFORME));
        // Lo que NVK pone siempre, antes del dibujo.
        let v = o.iter().position(|&w| w == cabecera_en(0, SET_SPH_VERSION, 1)).unwrap();
        assert_eq!(o[v + 1], 0x0003_0003);
        let z = o.iter().position(|&w| w == cabecera_en(0, SET_VERTEX_STREAM_SUBSTITUTE_A, 2)).unwrap();
        assert_eq!(((o[z + 1] as u64) << 32) | o[z + 2] as u64, sombreador_va(SUSTITUTO));
        // El dibujo: BEGIN(TRIANGLES), START 0 y 3 vertices, END.
        let b = o.iter().position(|&w| w == cabecera_en(0, BEGIN, 1)).unwrap();
        assert_eq!(o[b + 1], TRIANGULOS);
        assert_eq!(&o[b + 2..b + 5], &[cabecera_en(0, SET_VERTEX_ARRAY_START, 2), 0, 3]);
        assert_eq!(o[b + 5], cabecera_en(0, END, 1));
    }

    #[test]
    fn la_escalera() {
        let o = ordenes();
        let sem = cabecera_en(0, td::SET_REPORT_SEMAPHORE_A, 4);
        // Tres semaforos en orden: estado, vertices, el final.
        let k: std::vec::Vec<usize> = (0..ORDENES).filter(|&i| o[i] == sem).collect();
        assert_eq!(k.len(), 3);
        let donde = |i: usize| ((o[i + 1] as u64) << 32) | o[i + 2] as u64;
        assert_eq!(donde(k[0]), sombreador_va(SEMAFORO_FIN + ESTADO));
        assert_eq!(donde(k[1]), sombreador_va(SEMAFORO_FIN + VERTICES));
        assert_eq!(donde(k[2]), sombreador_va(SEMAFORO_FIN));
        assert_eq!((o[k[0] + 3], o[k[1] + 3]), (PAGA_FIN ^ ESTADO_PAGA, PAGA_FIN ^ VERTICES_PAGA));
        // Dos dibujos: el primero con el rasterizador APAGADO.
        let b: std::vec::Vec<usize> = (0..ORDENES).filter(|&i| o[i] == cabecera_en(0, BEGIN, 1)).collect();
        assert_eq!(b.len(), 2);
        assert!(k[0] < b[0] && b[0] < k[1] && k[1] < b[1] && b[1] < k[2]);
        assert_eq!(&o[b[0] - 2..b[0]], &[cabecera_en(0, SET_RASTER_ENABLE, 1), 0]);
        assert_eq!(&o[b[1] - 2..b[1]], &[cabecera_en(0, SET_RASTER_ENABLE, 1), 1]);
    }

    #[test]
    fn los_huecos_del_pipeline() {
        let o = ordenes();
        for j in 0..6 {
            let vivo = j == VERTICE || j == PIXEL;
            let c = cabecera_en(0, set_pipeline_shader(j), if vivo { 7 } else { 1 });
            let k = PREFIJO + o[PREFIJO..].iter().position(|&w| w == c).unwrap();
            assert_eq!(o[k + 1], j << 4 | vivo as u32, "hueco {j}");
        }
        assert_eq!(set_pipeline_shader(PIXEL), 0x2140);
    }

    #[test]
    fn las_cabeceras() {
        let v = sph_vertice();
        assert_eq!(v[0], 0x20461);
        assert_eq!(v[10], 1 << 31); // ImapVertexId
        assert_eq!(v[13], 0xF << 12); // OmapPosition
        let p = sph_pixel();
        assert_eq!(p[0], 0x21462);
        assert_eq!(p[5], 1 << 31);
        assert_eq!(p[18], 0xF);
    }

    #[test]
    fn los_programas_en_memoria() {
        let v = vertice();
        assert_eq!(v[SPH + 4], 0xFF00_7321); // ALD R0, tras la cabecera
        assert_eq!(v[SPH + 4 * 15], 0x0000_794d); // EXIT
        assert_eq!(pixel()[SPH + 4 * 4], 0x0000_794d);
    }

    #[test]
    fn ningun_centro_cae_en_una_arista() {
        // Si alguno fuera 0, la regla de desempate del rasterizador decidiria.
        for y in 0..LADO {
            for x in 0..LADO {
                for (a, b) in [(V1, V2), (V2, V0), (V0, V1)] {
                    assert_ne!(arista_centro(a, b, x, y), 0);
                }
            }
        }
    }

    #[test]
    fn el_juez() {
        assert!(dentro(256, 240) && !dentro(256, 20) && !dentro(10, 500));
        // El vertice de arriba: el pixel (256, 40) tiene el centro por debajo.
        assert!(!dentro(256, 39));
        let n = (0..PIXELES as u32).filter(|&k| dentro(k % LADO, k / LADO)).count();
        // Area 86400: el recuento de centros anda por ahi.
        assert!((86_000..86_800).contains(&n), "{n}");
        // 1 MiB en la pila de la prueba (sin `alloc` en este crate).
        let mut img = [0u32; PIXELES];
        for (k, p) in img.iter_mut().enumerate() {
            *p = esperado(k as u32 % LADO, k as u32 / LADO);
        }
        assert_eq!(comprobar(&img), PIXELES as u32);
        assert_eq!(verdes(&img) as usize, n);
        img[0] = PIXEL_VERDE;
        assert_eq!(comprobar(&img), PIXELES as u32 - 1);
    }

    #[test]
    fn los_vertices_en_normalizadas() {
        // x/256 - 1 de cada vertice, tal como estan en el programa.
        let n = |p: i32| f32::to_bits(p as f32 / 256.0 - 1.0);
        assert_eq!((n(V0.0), n(V0.1)), (0, 0xBF58_0000));
        assert_eq!((n(V1.0), n(V1.1)), (0x3F58_0000, 0x3F38_0000));
        assert_eq!((n(V2.0), n(V2.1)), (0xBF58_0000, 0x3F38_0000));
    }
}
