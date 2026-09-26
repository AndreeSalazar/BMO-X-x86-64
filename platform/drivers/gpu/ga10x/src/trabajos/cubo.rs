//! **X5: EL CUBO DEL ESTUDIO D3D, POR LA 3060, SIN WINDOWS** -- el mismo
//! fotograma que D3D12 dibujo en la 3060 bajo Windows, dibujado ahora por el
//! pipeline 3D de AMPERE_B desde BMO-X, DIRECTO en la pantalla: una ventana
//! de 1280x720 en el framebuffer del GOP (el mapa de `pantalla`). Si su
//! huella es la de la captura de D3D12, el rasterizador de la 3060 hace bajo
//! BMO-X lo mismo que bajo Windows con el driver de NVIDIA.
//!
//! capa: puro -- los programas, las ordenes y la ventana; la tanda (que
//! triangulos, donde y de que color) la calcula `bmo-cubo` y la pasa el
//! kernel en bits, y la VRAM y los registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- las ordenes del estado son las de T1c (`raster`),
//!           metodo a metodo, en el mismo orden: su escalera y sus nombres
//!           sirven igual. Cambia el destino, el viewport y el dibujo
//!
//! # Lo unico nuevo contra T1c y T2a (lo que el metal tiene que confirmar)
//!
//! ```text
//!    el destino    la VA de la pantalla (`pantalla::VA`), fila = la del GOP,
//!                  1280x720, y el formato del GOP (BGR: A8R8G8B8, RGB: A8B8G8R8)
//!    el viewport   el de D3D: escala (640, -360, 1), desplazamiento (640, 360, 0)
//!    la limpieza   el FONDO del estudio (16, 16, 24) en vez del magenta
//!    el dibujo     UN par de programas por triangulo (hasta 8): entre dibujo y
//!                  dibujo se cambia la DIRECCION de los dos, como NVK
//! ```
//!
//! # Los programas: solo instrucciones que ya corrieron en el metal
//!
//! Vertice (19): el de T1c con las posiciones de la tanda --
//!
//! ```text
//!    NOP ; ALD R0, a[0x2fc]                      el numero de vertice
//!    MOV R4..R7, v0                              x, y, z, w del vertice 0
//!    ISETP.NE.U32.AND P0, R0, 1 ; P1, R0, 2
//!    SEL R4..R7, R4..R7, v1, P0                  si es el 1
//!    SEL R4..R7, R4..R7, v2, P1                  si es el 2
//!    AST.128 a[0x70], R4 ; EXIT ; BRA .
//! ```
//!
//! Pixel (6): el de T1c con el color de la cara -- `MOV R0..R3` y EXIT. Sin
//! IPA: la cara es de UN color, y asi el float llega al ROP exacto, sin
//! pasar por una interpolacion.
//!
//! El control de cada instruccion: el de T1c para ALD, AST, EXIT y BRA
//! (barreras 0 y 1); para MOV, ISETP y SEL, 6 ciclos de espera (T1c usa 1..5)
//! y el primer ISETP espera la barrera 0 del ALD, como en T1c.
//!
//! # Donde viven
//!
//! ```text
//!    pagina 10 del tramo (SALIDA, libre tras S4): 8 de vertice, 512 B cada uno
//!    pagina 11 (PROGRAMA):                        8 de pixel, 256 B cada uno
//! ```

use crate::canal::GR;
use crate::copia::{cabecera_en, entrada, escribir, leer32, GP_GET};
use crate::lienzo::sombreador_va;
use crate::pantalla::{Pantalla, VA as PANTALLA_VA};
use crate::raster::{
    self as ra, programa, set_pipeline_shader, ATRIBUTO_APAGADO, BEGIN, END, ESCALONES, ESTADO, ESTADO_PAGA, INVALIDAR_TODO, N_ESCALONES,
    PAGA_ESCALON, PIXEL, RECORTE_Z, SIN_CRUZAR, SPH, SUSTITUTO, TRIANGULOS, UNO, VERTICE, VERTICES, VERTICES_PAGA,
};
use crate::sombreador::{EMPUJE, PROGRAMA, SALIDA, SEMAFOROS};
use crate::tresde::{self as td, SUBCANAL};
use crate::vram::a_cero;
use crate::Registros;

/// La medida de las capturas de D3D12 (`bmo_cubo::referencia`).
pub const ANCHO: u32 = 1280;
pub const ALTO: u32 = 720;
/// Lo mas que se dibuja de una vez (`bmo_cubo::tanda::CABEN`).
pub const CABEN: usize = 8;

pub const PAGA_FIN: u32 = 0x3060_C0B0;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x160;

/// Donde van los programas: uno de vertice cada 512 B en la pagina 10, uno de
/// pixel cada 256 B en la 11.
pub const PASO_VS: u64 = 0x200;
pub const PASO_PS: u64 = 0x100;
pub const fn vs(t: usize) -> u64 {
    SALIDA + t as u64 * PASO_VS
}
pub const fn ps(t: usize) -> u64 {
    PROGRAMA + t as u64 * PASO_PS
}

/// `SET_COLOR_TARGET_FORMAT_V_A8B8G8R8`: el rojo en el byte 0 (GOP RGB).
pub const FORMATO_RGB: u32 = 0xD5;

/// El FONDO del estudio: R 16, G 16, B 24 (`bmo_cubo::FONDO_F`, en bits).
pub const FONDO: [u32; 4] = [
    f32_bits(16.0 / 255.0),
    f32_bits(16.0 / 255.0),
    f32_bits(24.0 / 255.0),
    f32_bits(1.0),
];
pub const PIXEL_FONDO: u32 = 0x0010_1018;

const fn f32_bits(x: f32) -> u32 {
    x.to_bits()
}

/// **Un triangulo**, en bits de `f32`: sus tres vertices en coordenadas de
/// RECORTE y el color de la cara (r, g, b, a).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangulo {
    pub clip: [[u32; 4]; 3],
    pub color: [u32; 4],
}

/// **La ventana** de 1280x720 en la pantalla: centrada, con la esquina en un
/// multiplo de 32 pixeles (128 B, lo que NVK exige a un destino lineal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ventana {
    pub x0: u32,
    pub y0: u32,
    /// Donde empieza, en la VA de la GPU.
    pub va: u64,
    /// Bytes por fila (la del GOP).
    pub fila: u32,
    pub rgb: bool,
}

/// La ventana en `p`, si cabe y el destino queda alineado.
pub const fn ventana(p: &Pantalla) -> Option<Ventana> {
    if p.ancho < ANCHO || p.alto < ALTO || (p.pitch * 4) % 128 != 0 {
        return None;
    }
    let x0 = ((p.ancho - ANCHO) / 2) & !31;
    let y0 = (p.alto - ALTO) / 2;
    let va = PANTALLA_VA + (y0 as u64 * p.pitch as u64 + x0 as u64) * 4;
    Some(Ventana { x0, y0, va, fila: p.pitch * 4, rgb: p.rgb })
}

/// Un pixel LEIDO de la pantalla (`0xAARRGGBB` si el GOP es BGR, `0xAABBGGRR`
/// si es RGB) como lo cuenta la huella: `0x00RRGGBB`.
pub const fn a_rrggbb(v: u32, rgb: bool) -> u32 {
    if rgb {
        (v & 0xFF) << 16 | (v & 0xFF00) | (v >> 16) & 0xFF
    } else {
        v & 0x00FF_FFFF
    }
}

// == Los programas ===========================================================

/// Pone el control (bits 105..125 de la instruccion: 41..61 de la palabra
/// alta) y conserva lo demas.
pub const fn con_control(hi: u64, control: u64) -> u64 {
    (hi & ((1 << 41) - 1)) | control << 41
}

/// 6 ciclos de espera, el bit 4 (como todas las de `ptxas`) y sin barreras
/// (7 = ninguna, la de escritura y la de lectura).
pub const ALU: u64 = 6 | 1 << 4 | 7 << 5 | 7 << 8;
/// Lo mismo esperando la barrera 0 (la del ALD).
pub const ALU_TRAS_ALD: u64 = ALU | 1 << 11;

/// `MOV Rd, imm32`.
pub const fn mov(rd: u64, imm: u32) -> (u64, u64) {
    ((imm as u64) << 32 | 0xFF << 24 | rd << 16 | 0x7424, con_control(0x0000_0000_078E_00FF, ALU))
}

/// `SEL Rd, Ra, imm32, [!]Pp`: Ra si el predicado vale, si no el inmediato.
pub const fn sel(rd: u64, ra: u64, imm: u32, p: u64, negado: bool) -> (u64, u64) {
    ((imm as u64) << 32 | ra << 24 | rd << 16 | 0x7807, con_control(p << 23 | (negado as u64) << 26, ALU))
}

/// `ISETP.NE.U32.AND Pp, PT, R0, imm32, PT`.
pub const fn isetp_ne_r0(p: u64, imm: u32, control: u64) -> (u64, u64) {
    ((imm as u64) << 32 | 0x780C, con_control(0x0003_F050_70 | p << 17, control))
}

/// Las de T1c, tal cual (con su control).
pub const NOP: (u64, u64) = (0x0000000000007918, 0x000fe40000000000);
pub const ALD_VERTICE: (u64, u64) = (0x0002fcffff007321, 0x000e220000000000);
pub const AST_POSICION: (u64, u64) = (0x00007004ff007322, 0x0003e20000000cff);
pub const EXIT_TRAS_AST: (u64, u64) = (0x000000000000794d, 0x002fea0003800000);
pub const EXIT: (u64, u64) = (0x000000000000794d, 0x000fea0003800000);
pub const BRA: (u64, u64) = (0xfffffff000007947, 0x000fc0000383ffff);

pub const INSTR_VS: usize = 19;
pub const INSTR_PS: usize = 6;
pub const PALABRAS_VS: usize = SPH + INSTR_VS * 4;
pub const PALABRAS_PS: usize = SPH + INSTR_PS * 4;

/// **El programa de vertice** de un triangulo.
pub const fn codigo_vs(t: &Triangulo) -> [(u64, u64); INSTR_VS] {
    let [a, b, c] = t.clip;
    let mut o = [NOP; INSTR_VS];
    o[1] = ALD_VERTICE;
    let mut k = 0;
    while k < 4 {
        o[2 + k] = mov(4 + k as u64, a[k]);
        k += 1;
    }
    o[6] = isetp_ne_r0(0, 1, ALU_TRAS_ALD);
    o[7] = isetp_ne_r0(1, 2, ALU);
    k = 0;
    while k < 4 {
        o[8 + k] = sel(4 + k as u64, 4 + k as u64, b[k], 0, false);
        o[12 + k] = sel(4 + k as u64, 4 + k as u64, c[k], 1, false);
        k += 1;
    }
    o[16] = AST_POSICION;
    o[17] = EXIT_TRAS_AST;
    o[18] = BRA;
    o
}

/// **El programa de pixel** de un triangulo: el color de la cara.
pub const fn codigo_ps(t: &Triangulo) -> [(u64, u64); INSTR_PS] {
    [mov(0, t.color[0]), mov(1, t.color[1]), mov(2, t.color[2]), mov(3, t.color[3]), EXIT, BRA]
}

pub const fn vertice(t: &Triangulo) -> [u32; PALABRAS_VS] {
    programa(ra::sph_vertice(), &codigo_vs(t))
}

pub const fn pixel(t: &Triangulo) -> [u32; PALABRAS_PS] {
    programa(ra::sph_pixel(), &codigo_ps(t))
}

// == Las ordenes =============================================================

/// Lo que cabe en la pagina de ordenes.
pub const MAX_ORDENES: usize = 1024;

pub struct Ordenes {
    pub o: [u32; MAX_ORDENES],
    pub n: usize,
    k: u32,
    /// Con la ESCALERA de T1c (un WAIT_FOR_IDLE y un semaforo tras cada
    /// metodo, y el primer triangulo sin rasterizar): lo que dice DONDE se
    /// colgo. Sin ella (`ligero`), las mismas ordenes de estado sin esperar
    /// entre una y otra -- lo que hace un driver que ya sabe que funcionan.
    escalera: bool,
}

impl Ordenes {
    pub(crate) fn m(&mut self, metodo: u32, v: &[u32]) {
        self.o[self.n] = cabecera_en(SUBCANAL, metodo, v.len() as u32);
        let mut i = 0;
        while i < v.len() {
            self.o[self.n + 1 + i] = v[i];
            i += 1;
        }
        self.n += 1 + v.len();
    }

    pub(crate) fn semaforo(&mut self, donde: u64, paga: u32) {
        let s = sombreador_va(donde);
        self.m(td::SET_REPORT_SEMAPHORE_A, &[(s >> 32) as u32, s as u32, paga, td::INFORME]);
    }

    fn escalon(&mut self, k: u32) {
        if !self.escalera {
            return;
        }
        self.m(td::WAIT_FOR_IDLE, &[0]);
        self.semaforo(ESCALONES + 4 * k as u64, PAGA_ESCALON + k);
    }

    fn paso(&mut self, metodo: u32, v: &[u32]) {
        self.m(metodo, v);
        self.escalon(self.k);
        self.k += 1;
    }

    /// Los programas del triangulo `t` en los huecos 1 y 5, como NVK.
    fn programas(&mut self, t: usize) {
        for (j, dir, grupo) in [(VERTICE, vs(t), 0), (PIXEL, ps(t), 4)] {
            let d = sombreador_va(dir);
            self.m(set_pipeline_shader(j), &[j << 4 | 1]);
            self.m(set_pipeline_shader(j) + 0x14, &[(d >> 32) as u32, d as u32]);
            self.m(set_pipeline_shader(j) + 0x0c, &[ra::REGISTROS, grupo]);
        }
    }

    fn dibujo(&mut self) {
        self.dibujo_de(3);
    }

    /// Un dibujo de `vertices` vertices seguidos (desde el 0): `vertices / 3`
    /// triangulos.
    pub(crate) fn dibujo_de(&mut self, vertices: u32) {
        self.m(BEGIN, &[TRIANGULOS]);
        self.m(ra::SET_VERTEX_ARRAY_START, &[0, vertices]);
        self.m(END, &[0]);
    }
}

const fn f(x: f32) -> u32 {
    x.to_bits()
}

/// **Las ordenes** para `n` triangulos (1..=8) en la ventana `v`, un par de
/// programas por triangulo.
pub fn ordenes(v: &Ventana, n: usize) -> Ordenes {
    let mut e = hasta_el_dibujo(v);
    for t in 0..n {
        if t > 0 {
            e.programas(t);
        }
        e.dibujo();
    }
    e.cerrar();
    e
}

impl Ordenes {
    /// Esperar al GR y pagar el semaforo del dibujo entero.
    pub(crate) fn cerrar(&mut self) {
        self.m(td::WAIT_FOR_IDLE, &[0]);
        self.semaforo(SEMAFORO_FIN, PAGA_FIN);
    }
}

/// **Todo menos los dibujos**: la limpieza de la ventana, el estado de T1c
/// con su escalera (los programas en los huecos 1 y 5 son los de `vs(0)` y
/// `ps(0)`), el primer triangulo SIN rasterizar y el rasterizador de vuelta.
/// Lo comparten X5 (un par de programas por triangulo) y la tuberia fija de
/// VERRANO (`tuberia`: un par para todos).
pub(crate) fn hasta_el_dibujo(v: &Ventana) -> Ordenes {
    hasta_el_dibujo_con(v, true)
}

/// Lo mismo, con la escalera o sin ella (VERRANO `ligero`). Sin escalera
/// no hay semaforos de ESTADO ni de VERTICES: de la escalera de etapas solo
/// se paga el bit 2 (el dibujo entero), que es el que pide `sano`.
pub(crate) fn hasta_el_dibujo_con(v: &Ventana, escalera: bool) -> Ordenes {
    let mut e = Ordenes { o: [0; MAX_ORDENES], n: 0, k: 1, escalera };
    // T1a con este destino: la ventana, del FONDO.
    e.m(td::SET_OBJECT, &[crate::gr::AMPERE_B]);
    let formato = if v.rgb { FORMATO_RGB } else { td::FORMATO };
    e.m(td::SET_COLOR_TARGET_A0, &[(v.va >> 32) as u32, v.va as u32, v.fila, ALTO, formato, td::MEMORIA_PITCH, 1, 0]);
    e.m(td::SET_CT_SELECT, &[1]);
    e.m(td::SET_SURFACE_CLIP_HORIZONTAL, &[ANCHO << 16, ALTO << 16]);
    e.m(td::SET_WINDOW_OFFSET_X, &[0, 0]);
    e.m(td::SET_SCISSOR_ENABLE0, &[0]);
    e.m(td::SET_CT_WRITE0, &[td::ESCRIBIR_RGBA]);
    e.m(td::SET_COLOR_CLEAR_VALUE0, &FONDO);
    e.m(td::SET_CLEAR_SURFACE_CONTROL, &[0]);
    e.m(td::CLEAR_SURFACE, &[td::LIMPIAR_RGBA]);
    e.m(td::WAIT_FOR_IDLE, &[0]);
    // El estado de T1c, metodo a metodo y con su escalera.
    e.escalon(0);
    e.paso(ra::INVALIDATE_SHADER_CACHES, &[INVALIDAR_TODO]);
    let z = sombreador_va(SUSTITUTO);
    e.paso(ra::SET_VERTEX_STREAM_SUBSTITUTE_A, &[(z >> 32) as u32, z as u32]);
    e.paso(ra::SET_COLOR_TARGET_LAYER0, &[0]);
    e.paso(ra::SET_COLOR_COMPRESSION0, &[0]);
    // El viewport de D3D: x * 640 + 640, y * -360 + 360, z * 1 + 0.
    let (mw, mh) = ((ANCHO / 2) as f32, (ALTO / 2) as f32);
    e.paso(ra::SET_VIEWPORT_SCALE_X0, &[f(mw), f(-mh), f(1.0), f(mw), f(mh), f(0.0), SIN_CRUZAR]);
    e.paso(ra::SET_VIEWPORT_SCALE_OFFSET, &[1]);
    e.paso(ra::SET_VIEWPORT_CLIP_HORIZONTAL0, &[ANCHO << 16, ALTO << 16, 0, UNO]);
    e.paso(ra::SET_VIEWPORT_CLIP_CONTROL, &[RECORTE_Z]);
    e.paso(ra::SET_WINDOW_ORIGIN, &[0]);
    e.paso(ra::SET_VIEWPORT_PIXEL, &[0]);
    e.paso(ra::OGL_SET_CULL, &[0]);
    e.paso(ra::SET_ZT_SELECT, &[0]);
    e.paso(ra::SET_DEPTH_TEST, &[0]);
    e.paso(ra::SET_STENCIL_TEST, &[0]);
    e.paso(ra::SET_DEPTH_BOUNDS_TEST, &[0]);
    e.paso(ra::SET_ALPHA_TEST, &[0]);
    e.paso(ra::SET_BLEND0, &[0]);
    e.paso(ra::SET_ANTI_ALIAS_ENABLE, &[0]);
    e.paso(ra::SET_ANTI_ALIAS, &[0]);
    e.paso(ra::SET_SAMPLE_MASK_X0_Y0, &[0xFFFF; 4]);
    e.paso(ra::SET_STREAM_OUTPUT, &[0]);
    e.paso(ra::SET_RASTER_ENABLE, &[1]);
    e.paso(ra::SET_RENDER_ENABLE_C, &[1]);
    e.paso(ra::SET_CT_MRT_ENABLE, &[1]);
    e.paso(ra::SET_VERTEX_ATTRIBUTE_A0, &[ATRIBUTO_APAGADO; 32]);
    e.paso(ra::SET_VERTEX_STREAM_A_FORMAT0, &[0]);
    e.paso(ra::SET_VERTEX_ID_BASE, &[0]);
    let mut j = 0;
    while j < 6 {
        if j == VERTICE || j == PIXEL {
            let (dir, grupo) = if j == VERTICE { (vs(0), 0) } else { (ps(0), 4) };
            let d = sombreador_va(dir);
            e.m(set_pipeline_shader(j), &[j << 4 | 1]);
            e.m(set_pipeline_shader(j) + 0x14, &[(d >> 32) as u32, d as u32]);
            e.paso(set_pipeline_shader(j) + 0x0c, &[ra::REGISTROS, grupo]);
        } else {
            e.paso(set_pipeline_shader(j), &[j << 4]);
        }
        j += 1;
    }
    // La escalera de T1c: el estado, el primer triangulo sin rasterizar, y
    // todos.
    if !escalera {
        return e;
    }
    e.m(td::WAIT_FOR_IDLE, &[0]);
    e.semaforo(SEMAFORO_FIN + ESTADO, PAGA_FIN ^ ESTADO_PAGA);
    e.m(ra::SET_RASTER_ENABLE, &[0]);
    e.dibujo();
    e.m(td::WAIT_FOR_IDLE, &[0]);
    e.semaforo(SEMAFORO_FIN + VERTICES, PAGA_FIN ^ VERTICES_PAGA);
    e.m(ra::SET_RASTER_ENABLE, &[1]);
    e
}

/// **Preparar** con la entrada `e` del GPFIFO de GR: los semaforos a cero,
/// los programas de cada triangulo, las ordenes y la entrada.
pub fn preparar<R: Registros>(r: &mut R, e: u32, v: &Ventana, tris: &[Triangulo]) -> bool {
    if !crate::blur::entrada_valida(e) || tris.is_empty() || tris.len() > CABEN {
        return false;
    }
    let o = ordenes(v, tris.len());
    let en = entrada(sombreador_va(EMPUJE), o.n as u32);
    let mut bien = escribir(r, SEMAFORO_FIN, &[0; 4]) == 4
        && escribir(r, ESCALONES, &[0; N_ESCALONES as usize]) == N_ESCALONES as usize
        && a_cero(r, SALIDA) as usize == crate::vram::PALABRAS
        && a_cero(r, PROGRAMA) as usize == crate::vram::PALABRAS
        && a_cero(r, SUSTITUTO) as usize == crate::vram::PALABRAS;
    for (t, tri) in tris.iter().enumerate() {
        bien = bien && escribir(r, vs(t), &vertice(tri)) == PALABRAS_VS && escribir(r, ps(t), &pixel(tri)) == PALABRAS_PS;
    }
    bien && escribir(r, EMPUJE, &o.o[..o.n]) == o.n && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

pub use crate::raster::lanzar;

/// `(GP_GET, semaforo)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_FIN))
}

// == El resultado ============================================================

/// `Ok` del dibujo: los us de la 3060, cuantos triangulos, la escalera
/// (`raster::etapas`) y si se lanzo.
pub const fn empaquetar(us: u32, n: u32, etapas: u32, lanzado: bool) -> u64 {
    us as u64 | (n as u64 & 0xFF) << 32 | (etapas as u64 & 0x7) << 40 | (lanzado as u64) << 43
}

/// `(us, triangulos, etapas, lanzado)`.
pub const fn desempaquetar(v: u64) -> (u32, u32, u32, bool) {
    (v as u32, (v >> 32) as u32 & 0xFF, (v >> 40) as u32 & 0x7, v >> 43 & 1 != 0)
}

/// VERRANO (V1) le pone al `Ok` lo que costo PREPARAR: el bit 44 si fue EN
/// CALIENTE (`tuberia::preparar_caliente`) y los us de preparar en 45..63
/// (hasta ~0,26 s; mas, satura). X5 los deja a cero.
pub const fn con_preparar(v: u64, caliente: bool, us: u64) -> u64 {
    let us = if us > 0x3_FFFF { 0x3_FFFF } else { us };
    v & ((1 << 44) - 1 | EN_VUELO) | (caliente as u64) << 44 | us << 45
}

/// `(en caliente, us de preparar)` de un `Ok` de VERRANO.
pub const fn preparado(v: u64) -> (bool, u32) {
    (v >> 44 & 1 != 0, (v >> 45) as u32 & 0x3_FFFF)
}

/// **V1b, EL ANILLO (`anillo`)**: el bit 63 del `Ok` dice que el fotograma
/// quedo EN VUELO -- enviado, sin esperar a que se pague. Sus us (0..31) no
/// son entonces lo que tardo la 3060, sino lo que la CPU espero a que su
/// ranura quedara libre (0 si la 3060 va por delante). Que se pago lo dice
/// la valla despues: al reusar la ranura, o al vaciar el anillo.
pub const EN_VUELO: u64 = 1 << 63;

/// El `Ok` de un fotograma que queda en vuelo: se lanzo, `n` triangulos, y
/// `espera_us` la CPU esperando su ranura.
pub const fn en_vuelo(espera_us: u32, n: u32) -> u64 {
    empaquetar(espera_us, n, 0, true) | EN_VUELO
}

/// Si el `Ok` es de un fotograma en vuelo.
pub const fn es_en_vuelo(v: u64) -> bool {
    v & EN_VUELO != 0
}

/// Pago el dibujo entero (el bit 2 de la escalera), o quedo en vuelo en
/// el anillo (lo que la 3060 pague se ve despues, en la valla).
pub const fn sano(v: u64) -> bool {
    let (_, n, etapas, lanzado) = desempaquetar(v);
    lanzado && n > 0 && (etapas & 0b100 != 0 || es_en_vuelo(v))
}

const _: () = assert!(PALABRAS_VS * 4 <= PASO_VS as usize && PALABRAS_PS * 4 <= PASO_PS as usize);
const _: () = assert!(vs(CABEN) <= SALIDA + 0x1000 && ps(CABEN) <= PROGRAMA + 0x1000);
const _: () = assert!(SALIDA + 0x1000 == PROGRAMA);
const _: () = assert!(SEMAFORO_FIN > crate::video::SEMAFORO_FIN && SEMAFORO_FIN + 16 <= SEMAFOROS + 0x200);
const _: () = assert!(MAX_ORDENES * 4 <= 4096);

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;
    use crate::raster::CODIGO_VS as T1C;

    /// El `Ok` de VERRANO: caliente, preparar y en vuelo no se pisan.
    #[test]
    fn el_ok_de_verrano() {
        let v = con_preparar(empaquetar(281, 12, 0b100, true), true, 168);
        assert_eq!(desempaquetar(v), (281, 12, 0b100, true));
        assert_eq!(preparado(v), (true, 168));
        assert!(sano(v) && !es_en_vuelo(v));
        let w = con_preparar(en_vuelo(3, 12), true, 1 << 30);
        assert!(es_en_vuelo(w) && sano(w));
        assert_eq!(preparado(w), (true, 0x3_FFFF), "satura sin tocar el bit 63");
        assert_eq!(desempaquetar(w), (3, 12, 0, true));
        assert!(!sano(empaquetar(3, 12, 0, true)), "ni pagado ni en vuelo");
    }

    /// Los codificadores dan las instrucciones de T1c (salvo el control, que
    /// aqui es mas lento a proposito).
    #[test]
    fn codifican_lo_que_corrio_en_t1c() {
        let sin_control = |(lo, hi): (u64, u64)| (lo, hi & ((1 << 41) - 1));
        assert_eq!(sin_control(mov(5, 0x3f38_0000)), sin_control(T1C[2]));
        assert_eq!(sin_control(mov(7, 0x3f80_0000)), sin_control(T1C[6]));
        assert_eq!(sin_control(isetp_ne_r0(0, 1, ALU)), sin_control(T1C[8]));
        assert_eq!(sin_control(isetp_ne_r0(1, 2, ALU)), sin_control(T1C[9]));
        assert_eq!(sin_control(sel(5, 5, 0xbf58_0000, 0, true)), sin_control(T1C[10]));
        assert_eq!(sin_control(sel(0, 0xFF, 0x3f58_0000, 0, false)), sin_control(T1C[11]));
        assert_eq!(sin_control(sel(5, 5, 0x3f38_0000, 1, false)), sin_control(T1C[12]));
        assert_eq!(ALD_VERTICE, T1C[1]);
        assert_eq!(AST_POSICION, T1C[14]);
        assert_eq!(EXIT_TRAS_AST, T1C[15]);
        assert_eq!(BRA, T1C[16]);
        assert_eq!(EXIT, crate::raster::CODIGO_PS[4]);
    }

    #[test]
    fn el_control() {
        // Palabra alta >> 41: 6 ciclos, bit 4, barreras 7 y 7.
        let (_, hi) = mov(0, 0);
        assert_eq!(hi >> 41, 0x7F6);
        let (_, hi) = isetp_ne_r0(0, 1, ALU_TRAS_ALD);
        assert_eq!(hi >> 41, 0xFF6, "espera la barrera 0 del ALD");
        // El ALD escribe la barrera 0; el AST lee con la 1 y EXIT la espera.
        assert_eq!(ALD_VERTICE.1 >> 46 & 7, 0);
        assert_eq!(AST_POSICION.1 >> 49 & 7, 1);
        assert_eq!(EXIT_TRAS_AST.1 >> 52 & 0x3F, 0b10);
    }

    fn uno() -> Triangulo {
        Triangulo {
            clip: [[0x3f00_0000, 0x3e80_0000, 0x3f70_0000, 0x40a0_0000], [1, 2, 3, 4], [5, 6, 7, 8]],
            color: [0x3e00_0000, 0x3f00_0000, 0x3e40_0000, 0x3f80_0000],
        }
    }

    #[test]
    fn el_vertice_lleva_la_tanda() {
        let c = codigo_vs(&uno());
        // v0 en los MOV, v1 en los SEL con P0 y v2 en los SEL con P1.
        for k in 0..4 {
            assert_eq!((c[2 + k].0 >> 32) as u32, uno().clip[0][k]);
            assert_eq!((c[8 + k].0 >> 32) as u32, uno().clip[1][k]);
            assert_eq!((c[12 + k].0 >> 32) as u32, uno().clip[2][k]);
            assert_eq!(c[2 + k].0 >> 16 & 0xFF, 4 + k as u64, "R4..R7: los del AST.128");
            assert_eq!(c[12 + k].1 >> 23 & 7, 1, "P1");
        }
        let p = codigo_ps(&uno());
        for k in 0..4 {
            assert_eq!((p[k].0 >> 32) as u32, uno().color[k]);
            assert_eq!(p[k].0 >> 16 & 0xFF, k as u64, "el color sale en R0..R3");
        }
        assert_eq!(vertice(&uno())[..SPH], ra::sph_vertice());
        assert_eq!(pixel(&uno())[..SPH], ra::sph_pixel());
    }

    const GOP: Pantalla = Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };

    #[test]
    fn la_ventana() {
        let v = ventana(&GOP).unwrap();
        assert_eq!((v.x0, v.y0, v.fila), (320, 180, 7680));
        assert_eq!(v.va, PANTALLA_VA + (180 * 1920 + 320) * 4);
        assert_eq!(v.va % 128, 0);
        assert!(ventana(&Pantalla { ancho: 1024, alto: 768, pitch: 1024, ..GOP }).is_none());
        assert_eq!(a_rrggbb(0xFF10_1018, false), PIXEL_FONDO);
        assert_eq!(a_rrggbb(0xFF18_1010, true), PIXEL_FONDO);
    }

    #[test]
    fn el_estado_es_el_de_t1c() {
        // Quitando el prefijo (su destino es otro), el estado, metodo a
        // metodo, es el de T1c salvo los valores del viewport y su recorte.
        let v = ventana(&GOP).unwrap();
        let o = ordenes(&v, 6);
        let t = ra::ordenes();
        let desde = |w: &[u32]| w.iter().position(|&x| x == cabecera_en(0, ra::INVALIDATE_SHADER_CACHES, 1)).unwrap();
        let (a, b) = (desde(&o.o[..o.n]), desde(&t));
        let cabeceras = |w: &[u32]| {
            let mut c = std::vec::Vec::new();
            let mut i = 0;
            while i < w.len() {
                c.push(w[i]);
                i += 1 + (w[i] >> 16 & 0x1FFF) as usize;
            }
            c
        };
        // Hasta el semaforo del ESTADO, las mismas cabeceras en el mismo orden.
        let estado = |w: &[u32], d: usize, s: u64| {
            let va = sombreador_va(s + ESTADO) as u32;
            d + w[d..].windows(2).position(|p| p[0] == 2 && p[1] == va).unwrap()
        };
        let (fa, fb) = (estado(&o.o[..o.n], a, SEMAFORO_FIN), estado(&t, b, ra::SEMAFORO_FIN));
        assert_eq!(cabeceras(&o.o[a..fa]), cabeceras(&t[b..fb]));
        // Y los mismos valores, salvo el viewport (6 de sus 7 palabras: el
        // cruce de ejes es el mismo), su recorte (2) y donde estan los dos
        // programas (la parte baja de las dos direcciones).
        let distintos = o.o[a..fa].iter().zip(&t[b..fb]).filter(|(x, y)| x != y).count();
        assert_eq!(distintos, 6 + 2 + 2);
    }

    #[test]
    fn las_ordenes() {
        let v = ventana(&GOP).unwrap();
        let o = ordenes(&v, 6);
        let w = &o.o[..o.n];
        assert!(o.n < MAX_ORDENES);
        // El destino: la ventana en la pantalla, 1280x720, BGR, lineal.
        let d = w.iter().position(|&x| x == cabecera_en(0, td::SET_COLOR_TARGET_A0, 8)).unwrap();
        assert_eq!(&w[d + 1..d + 9], &[(v.va >> 32) as u32, v.va as u32, 7680, 720, td::FORMATO, td::MEMORIA_PITCH, 1, 0]);
        // El viewport de D3D.
        let p = w.iter().position(|&x| x == cabecera_en(0, ra::SET_VIEWPORT_SCALE_X0, 7)).unwrap();
        let fl = |k: usize| f32::from_bits(w[p + 1 + k]);
        assert_eq!([fl(0), fl(1), fl(2), fl(3), fl(4), fl(5)], [640.0, -360.0, 1.0, 640.0, 360.0, 0.0]);
        // Seis dibujos con el rasterizador y uno sin: 7 BEGIN.
        assert_eq!(w.iter().filter(|&&x| x == cabecera_en(0, BEGIN, 1)).count(), 7);
        // El triangulo 5 con sus programas.
        let a = sombreador_va(vs(5));
        assert!(w.windows(3).any(|q| q[0] == cabecera_en(0, set_pipeline_shader(VERTICE) + 0x14, 2) && q[1] == (a >> 32) as u32 && q[2] == a as u32));
        // Lo ultimo, el semaforo del dibujo entero.
        assert_eq!((w[o.n - 2], w[o.n - 1]), (PAGA_FIN, td::INFORME));
        // Un GOP RGB: el otro formato.
        let rgb = ordenes(&ventana(&Pantalla { rgb: true, ..GOP }).unwrap(), 1);
        assert_eq!(rgb.o[d + 5], FORMATO_RGB);
    }

    #[test]
    fn el_resultado() {
        let v = empaquetar(1234, 6, 0b111, true);
        assert_eq!(desempaquetar(v), (1234, 6, 0b111, true));
        assert!(sano(v));
        assert!(!sano(empaquetar(1234, 6, 0b011, true)));
    }
}
