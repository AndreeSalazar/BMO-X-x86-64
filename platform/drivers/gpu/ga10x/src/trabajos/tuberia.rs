//! **VERRANO V0: LA TUBERIA FIJA DE LA 3060** -- el cubo con UN programa de
//! vertice y UN programa de pixel que no cambian nunca, y los datos (la
//! posicion y el color de cada vertice) en un BUFFER. Es lo que pide el BSF:
//! el codigo se hace una vez y viaja ya traducido (`kind` SM86); de un
//! fotograma a otro solo cambian los datos. En X5 (`cubo`) cada triangulo
//! llevaba su programa con los numeros dentro: eso era compilar en marcha.
//!
//! capa: puro -- los programas, su ABI y las ordenes; el buffer y los
//! registros los toca el kernel (L8)
//!
//! [eje]     CORRECCION -- las instrucciones, codificadas con las MISMAS
//!           funciones que dan, bit a bit, las que ya corrieron en el metal
//!           (las pruebas lo comparan); el estado, el de X5 entero
//!
//! # La fuente
//!
//! `sombreadores/cubo.vert` y `cubo.frag` (GLSL 450), y su SPIR-V, que es lo
//! que el BSF guarda como origen. El SASS de abajo hace lo MISMO, escrito a
//! mano mientras no hay emisor de SPIR-V a SM86.
//!
//! # El ABI `SM86_V1` (el que el BSF nombra)
//!
//! ```text
//!    la tabla de buffers  la ranura k, su direccion (u64) en la VA TABLA + 8k
//!    vertice   entra      el numero de vertice en a[0x2fc]
//!              sale       la posicion en a[0x70], el generico 0 en a[0x80]
//!    pixel     entra      el generico 0 por IPA (ScreenLinear)
//!              sale       el color en R0..R3
//!    codigo               la cabecera SPH (128 B) y detras las instrucciones
//! ```
//!
//! # Los programas
//!
//! ```text
//!    vertice (21)  ALD R0 <- el vertice; R12:R13 <- 0x2_0000_0000
//!                  LDG R2, R3 <- la ranura 0 de la tabla (la direccion)
//!                  R1 = vertice * 32; R2:R3 += R1 (IADD3 + IMAD.X)
//!                  LDG R4..R7 <- la posicion, R8..R11 <- el color
//!                  AST.128 a[0x70], R4 ; AST.128 a[0x80], R8 ; EXIT
//!    pixel (7)     IPA R0..R3 <- el color ; EXIT
//! ```
//!
//! **El color, sin `flat`.** La fuente dice `flat`; el SASS lo interpola en
//! pantalla (el IPA de T2a, que ya corrio) con los TRES vertices del mismo
//! color. Con la ecuacion del plano de la 3060 sus pendientes son cero y el
//! valor sale exacto; si no fuera exacto, el juez lo diria (un canal con una
//! unidad de diferencia en caras enteras).
//!
//! # Las barreras
//!
//! ```text
//!    0  el ALD (el numero de vertice)       la espera el IMAD.SHL
//!    2  los dos LDG de la tabla             la espera el IADD3
//!    3  los ocho LDG del vertice            la espera el primer AST
//!    1  lectura de los AST                  la espera EXIT (como T1c)
//! ```

use crate::canal::GR;
use crate::copia::{entrada, escribir};
use crate::cubo::{self as cu, con_control, mov, Ventana, ALU, SEMAFORO_FIN};
use crate::lienzo::sombreador_va;
use crate::raster::{programa, ESCALONES, N_ESCALONES, SPH, SUSTITUTO};
use crate::sombreador::{EMPUJE, SEMAFOROS};
use crate::vram::a_cero;
use crate::Registros;

/// La tabla de buffers (`SM86_V1`): la ranura 0, la de los vertices.
pub const TABLA: u64 = SEMAFOROS + 0x600;
/// Los DATOS (P3b4b, 28-09): 64 KiB de VRAM propios (`vram::DATOS`), que
/// la GPU ve en `vram::DATOS_VA`. Antes, 1 KiB en la pagina de los semaforos.
pub const DATOS: u64 = crate::vram::DATOS;
/// Un vertice de V0: 8 floats (la posicion y el color), 32 B.
pub const BYTES_VERTICE: usize = 32;
/// Lo mas que se dibuja de una vez (los datos lo acotan antes: V0, 2048).
pub const MAX_VERTICES: usize = 3 * 4096;
/// Lo mas que miden los DATOS de un paquete (E5): los vertices de V0 o,
/// con un programa emitido, el cbuffer y los vertices como el programa los
/// lee. 64 KiB (P3b4b); el anillo, lo de su ranura (`anillo::cabe`).
pub const DATOS_MAX: usize = crate::vram::DATOS_PAGINAS * 4096;
/// Donde van los dos programas. Hasta LI2b, el principio de las paginas de
/// X5 (una cada uno: 128 instrucciones). ** LI2b (12-10): su PROPIA region
/// de la VRAM (`vram::CODIGO`), 17 paginas cada uno: hasta 4096
/// instrucciones, lo que pidio el censo de Cyberpunk en el metal.
pub const VS: u64 = crate::vram::CODIGO;
pub const PS: u64 = crate::vram::CODIGO + crate::vram::CODIGO_PAGINAS_UNO as u64 * 0x1000;
/// Donde los ve la GPU (las ordenes: `SET_PIPELINE_SHADER` + 0x14).
pub const VS_VA: u64 = crate::vram::CODIGO_VA;
pub const PS_VA: u64 = crate::vram::CODIGO_VA + (PS - VS);
/// Las dos direcciones, como las piden las ordenes.
pub const PROGRAMAS_VA: (u64, u64) = (VS_VA, PS_VA);
/// Los registros que se le dan a cada programa de VERRANO
/// (`REGISTER_COUNT`; la 3060 se queda DOS, `juez::RESERVADOS`). 16 hasta
/// E5, como X5; 64 desde E5 (28-09): un programa EMITIDO lleva sus
/// entradas y su cbuffer precargados en registros (el de vertice de BMOX-12,
/// ~52, y el pegamento). A un programa que usa menos no le cambia nada.
pub const REGISTROS: u32 = 64;
/// ** LI2b, los registros (12-10): lo mas que se le da a un programa de una
/// APP. El censo de Cyberpunk: "mas registros de los que da la tuberia" (el
/// cuerpo y su pegamento no cabian en 64). `REGISTER_COUNT` es de 8 bits y
/// R255 es RZ: 248, multiplo de 8, por debajo. No se da a todos: cada dibujo
/// pide lo que usan sus dos programas ([`registros_para`], `Dibujo::registros`)
/// -- dar 248 a un programa de 40 son cuatro veces menos hilos por SM --, y
/// el juez juzga con ESE numero.
pub const MAX_REGISTROS: u32 = 248;

/// Los registros que se le dan a un dibujo cuyos programas usan `usados`
/// (con su pegamento, sin los `RESERVADOS`): los `RESERVADOS` encima, a
/// multiplo de 8, y nunca menos de [`REGISTROS`] (lo de siempre).
pub const fn registros_para(usados: u32) -> u32 {
    let r = (usados + crate::sass::juez::RESERVADOS + 7) & !7;
    if r < REGISTROS {
        REGISTROS
    } else {
        r
    }
}
/// Lo mas que mide cada uno: su SPH y su codigo (`juez::MAX_INSTRUCCIONES`:
/// 128 desde E5, 4096 desde LI2b). Lo que queda de su ultima pagina se pone
/// a cero ([`escribir_programa`]).
pub const HUECO: usize = 4 * SPH + 16 * crate::sass::juez::MAX_INSTRUCCIONES;
/// Lo mas que mide un paquete entero (cabecera, dos programas y vertices):
/// la caja que el escritorio tiene que reservar.
pub const MAX_PAQUETE: usize = CABECERA + 2 * HUECO + DATOS_MAX;

/// Un vertice: la posicion en coordenadas de recorte y el color, en bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Vertice {
    pub posicion: [u32; 4],
    pub color: [u32; 4],
}

impl Vertice {
    pub const fn palabras(&self) -> [u32; 8] {
        let (p, c) = (self.posicion, self.color);
        [p[0], p[1], p[2], p[3], c[0], c[1], c[2], c[3]]
    }
}

// == Las instrucciones =======================================================

/// Control para una carga: 2 ciclos, el bit 4, escribe la barrera `b`.
pub const fn carga(b: u64) -> u64 {
    2 | 1 << 4 | b << 5 | 7 << 8
}

/// Un control de ALU que espera las barreras de `mascara`.
pub const fn espera(mascara: u64) -> u64 {
    ALU | mascara << 11
}

/// `LDG.E.STRONG.SYS Rd, [Ra.64 + imm]` (sin descriptor, 32 bits).
pub const fn ldg(rd: u64, ra: u64, imm: u32, control: u64) -> (u64, u64) {
    ((imm as u64 & 0xFF_FFFF) << 40 | 0x06 << 32 | ra << 24 | rd << 16 | 0x7981, con_control(0x0000_000c_1f59_00, control))
}

/// `IMAD.SHL.U32 Rd, Ra, imm, RZ`.
pub const fn imad_shl(rd: u64, ra: u64, imm: u32, control: u64) -> (u64, u64) {
    ((imm as u64) << 32 | ra << 24 | rd << 16 | 0x7824, con_control(0x0000_0000_078E_00FF, control))
}

/// `IADD3 Rd, Pp, Ra, Rb, RZ` (con el acarreo en Pp).
pub const fn iadd3_acarreo(rd: u64, p: u64, ra: u64, rb: u64, control: u64) -> (u64, u64) {
    (rb << 32 | ra << 24 | rd << 16 | 0x7210, con_control(0x0000_0000_07F1_E0FF | p << 17, control))
}

/// `IMAD.X Rd, Ra, 0x1, Rc, Pp` (la parte alta, con el acarreo de Pp).
pub const fn imad_x(rd: u64, ra: u64, rc: u64, p: u64, control: u64) -> (u64, u64) {
    (1 << 32 | ra << 24 | rd << 16 | 0x7824, con_control(0x0000_0000_000E_0600 | p << 23 | rc, control))
}

/// `IPA.PASS Rd, a[4 * atributo]` (ScreenLinear, sin predicado de salida).
pub const fn ipa(rd: u64, atributo: u64, control: u64) -> (u64, u64) {
    (0x0000_00ff_ff00_7326 | rd << 16, con_control(0x000E_0000 | atributo, control))
}

/// El control de los IPA de T2a: 1 ciclo (4 el ultimo), escriben la barrera 0.
pub const IPA_CONTROL: u64 = 1 | 1 << 4 | 7 << 8;
pub const IPA_ULTIMO: u64 = 4 | 1 << 4 | 7 << 8;

/// `AST.128 a[0x70], R4` esperando la barrera 3 (los LDG del vertice).
pub const AST_POSICION: (u64, u64) = (cu::AST_POSICION.0, con_control(cu::AST_POSICION.1, 1 | 1 << 4 | 7 << 5 | 1 << 8 | 1 << 3 << 11));
/// `AST.128 a[0x80], R8`: el de T2a, tal cual.
pub const AST_COLOR: (u64, u64) = crate::color3d::CODIGO_VS[19];

/// El registro del desplazamiento del vertice (vertice * 32).
pub const DESPLAZAMIENTO: u64 = 1;

pub const INSTR_VS: usize = 21;
pub const INSTR_PS: usize = 6;
pub const PALABRAS_VS: usize = SPH + INSTR_VS * 4;
pub const PALABRAS_PS: usize = SPH + INSTR_PS * 4;

/// **El programa de vertice**.
pub const fn codigo_vs() -> [(u64, u64); INSTR_VS] {
    let t = sombreador_va(TABLA);
    let base = t & !0xFF_FFFF;
    let off = (t - base) as u32;
    let mut o = [cu::NOP; INSTR_VS];
    o[1] = cu::ALD_VERTICE;
    o[2] = mov(12, base as u32);
    o[3] = mov(13, (base >> 32) as u32);
    o[4] = ldg(2, 12, off, carga(2));
    o[5] = ldg(3, 12, off + 4, carga(2));
    // ** R1 y no R14 (26-09, metal 06:33): con `REGISTROS = 16` la 3060 da
    // R0..R13 -- dos se gastan en el contador de programa --, y el R14 de V0
    // fue su Xid 13, "Out Of Range Register". R1 esta libre en este programa.
    o[6] = imad_shl(DESPLAZAMIENTO, 0, BYTES_VERTICE as u32, espera(1 << 0));
    o[7] = iadd3_acarreo(2, 0, 2, DESPLAZAMIENTO, espera(1 << 2));
    o[8] = imad_x(3, 3, 0xFF, 0, ALU);
    let mut k = 0;
    while k < 8 {
        o[9 + k] = ldg(4 + k as u64, 2, 4 * k as u32, carga(3));
        k += 1;
    }
    o[17] = AST_POSICION;
    o[18] = AST_COLOR;
    o[19] = cu::EXIT_TRAS_AST;
    o[20] = cu::BRA;
    o
}

/// **La prueba de UNA variable** (26-09, metal 05:07): el de vertice con el
/// bit 26 puesto SIGUE colgado en los VERTICES. Este es el mismo programa sin
/// NINGUN LDG: los dos de la tabla son NOP y los ocho del vertice son MOV de
/// constantes (posicion `(0, 0, 0.5, 1)`, color blanco). Los triangulos salen
/// de area cero -- no pintan nada --, pero el escalon de los VERTICES (el
/// dibujo con el rasterizador apagado) SI tiene que pagarse. Si se paga, el
/// que cuelga es el LDG (o la direccion que lee); si no, es otra cosa del
/// programa y los LDG quedan absueltos. `gpu verrano sinldg`.
pub const fn codigo_vs_sin_ldg() -> [(u64, u64); INSTR_VS] {
    let mut o = codigo_vs();
    o[4] = cu::NOP;
    o[5] = cu::NOP;
    let v = [0u32, 0, 0x3F00_0000, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000];
    let mut k = 0;
    while k < 8 {
        o[9 + k] = mov(4 + k as u64, v[k]);
        k += 1;
    }
    // ** Y sin esperar las barreras de los LDG que ya no estan (el juez del
    // SASS, R3: esperar una barrera que nadie enciende). La 3060 no se
    // colgaria por eso -- una barrera a cero no se espera --, pero la prueba
    // de UNA variable tiene que ser limpia: solo cambian las cargas.
    o[7] = (o[7].0, con_control(o[7].1, ALU));
    o[17] = (o[17].0, con_control(o[17].1, 1 | 1 << 4 | 7 << 5 | 1 << 8));
    o
}

/// El de vertice SIN LDG, con la SPH de siempre (ver `codigo_vs_sin_ldg`).
pub const fn vertice_sin_ldg() -> [u32; PALABRAS_VS] {
    programa(sph_vertice(), &codigo_vs_sin_ldg())
}

/// **El programa de pixel**: los cuatro canales por IPA (el de T2a, y el
/// alfa tambien), y el EXIT de T2a que espera la barrera 0.
pub const fn codigo_ps() -> [(u64, u64); INSTR_PS] {
    let c = crate::color3d::CODIGO_PS;
    [ipa(0, 0x20, IPA_CONTROL), ipa(1, 0x21, IPA_CONTROL), ipa(2, 0x22, IPA_CONTROL), ipa(3, 0x23, IPA_ULTIMO), c[4], c[5]]
}

/// La SPH del de pixel: la de T2a y tambien el W del generico 0.
pub const fn sph_pixel() -> [u32; SPH] {
    let mut h = crate::color3d::sph_pixel();
    h[6] |= crate::color3d::LINEAL << 6;
    h
}

/// **La SPH del de vertice**: la de T2a (posicion y generico 0) y ademas
/// `DoesLoadOrStore`, porque ESTE lee los vertices con LDG.
///
/// ** Metal 25-09 20:19: `gpu verrano` se paro en el escalon de los VERTICES
/// (el dibujo con el rasterizador apagado: solo el programa de vertice) sin
/// excepcion y con INTR/EXCEPTION/STATUS a 0. Lo unico nuevo de este programa
/// frente a X5 son los LDG, y su SPH no decia que los hacia: NAK y nvc0 ponen
/// el bit 26 en cuanto un programa toca memoria global.
pub const fn sph_vertice() -> [u32; SPH] {
    let mut h = crate::color3d::sph_vertice();
    h[0] |= crate::raster::LEE_O_ESCRIBE;
    h
}

pub const fn vertice() -> [u32; PALABRAS_VS] {
    programa(sph_vertice(), &codigo_vs())
}

pub const fn pixel() -> [u32; PALABRAS_PS] {
    programa(sph_pixel(), &codigo_ps())
}

/// Los bytes del codigo tal como van a memoria (y al BSF).
pub fn bytes<const N: usize>(palabras: &[u32; N], out: &mut [u8]) -> usize {
    for (k, w) in palabras.iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
    4 * N
}

// == El paquete: lo que la app le da al kernel ================================

/// `"VRN0"`: el paquete de VERRANO V0.
pub const MAGIA: u32 = u32::from_le_bytes(*b"VRN0");
/// La cabecera: magia, ficha de GR, vertices, bytes del de vertice, bytes
/// del de pixel, el RECORTE de la limpieza (V1c: dos palabras, 0 = la
/// ventana entera) y los bytes de los DATOS (E5; 0 = los de V0, 32 por
/// vertice).
pub const CABECERA: usize = 32;

/// `"VRN1"` (P3b4b, 28-09): la de V0 y, en 32 B mas, COMO se dibuja -- con
/// indices y con el descarte de caras por el hardware. La CPU deja de
/// escoger que triangulos miran a la camara (la tanda): se lo dice a la 3060.
///
/// ```text
///    +32  el byte de los INDICES en los datos (u32 cada uno), o SIN_INDICES
///    +36  bits 0..1 el descarte (0 ninguno, 1 las traseras, 2 las
///         delanteras); bit 2 delante es ANTIHORARIO (D3D: horario);
///         P3b4c: bit 3 hay Z, bits 4..7 su funcion (D3D, 1..8), bit 8 se
///         escribe, bit 9 se limpia antes (a 1.0: VRN1 no lleva el valor)
///    +40  cuantos vertices hay en los datos: cada indice, menos
///    +44  el DESTINO (P3b4b 3), si el bit 31 de +60 esta puesto: la VA de
///         la app (+44 baja, +48 alta), +52 bytes por fila, +56 ancho |
///         alto << 16, +60 bit 0 rgb; si no, todo a cero (la pantalla)
///    +20  ** Q0a2 (EL_FOCO, 10-10): 1 = el destino se LIMPIA antes con el
///         pixel de +24 (como en memoria), por la 3060 -- la misma marca que
///         la receta VRN2 --. Solo con destino. Un 1 en +20 nunca es un
///         recorte (x0 = 1, x1 = 0): no se confunden
/// ```
pub const MAGIA_1: u32 = u32::from_le_bytes(*b"VRN1");
pub const CABECERA_1: usize = 64;
/// Lo que el kernel lee para MEDIR un paquete (la mas grande de las dos).
pub const CABECERA_MAX: usize = CABECERA_1;
/// Sin indices: los vertices seguidos, como V0.
pub const SIN_INDICES: u32 = u32::MAX;

/// Que caras descarta el hardware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Descarte {
    #[default]
    Ninguna,
    Traseras,
    Delanteras,
}

/// **Como se dibuja** (VRN1). El de V0: sin indices, sin descarte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Dibujo {
    /// El byte de los indices (u32) en los datos; `n` indices.
    pub indices: Option<u32>,
    /// Cuantos vertices hay en los datos (cada indice, menos).
    pub vertices: u32,
    pub descarte: Descarte,
    /// Delante es antihorario (en D3D, `FrontCounterClockwise`).
    pub antihorario: bool,
    /// Donde dibuja: la RAM de la app (su VA y como es), o `None`: la
    /// ventana de VERRANO en la pantalla, limpiandola antes.
    pub destino: Option<(u64, crate::destino::Destino)>,
    /// P3b4c: la prueba de profundidad, por la 3060 (`profundidad`).
    pub z: Option<crate::profundidad::Z>,
    /// P3b4c: limpiar el DESTINO antes de dibujar, con este pixel tal como
    /// va en memoria (`B8G8R8A8` o, con `rgb`, `R8G8B8A8`): el
    /// `ClearRenderTargetView` de la app, hecho por la 3060 y no por la CPU.
    /// Solo con destino (la receta; VRN1 no lo lleva).
    pub color: Option<u32>,
    /// P3b4c.8 T2: cuantas texturas lee el de pixel (sus TIC y TSC, en las
    /// piscinas de `texturas`, los escribe el kernel). Solo la receta.
    pub texturas: u8,
    /// P3b4c.9 Z1: el destino es un BACK BUFFER de la cadena de intercambio
    /// (la receta, +88 bit 0): lo que la app muestra en `Present`, y nada mas
    /// lo lee. Solo eso puede ir directo a la pantalla.
    pub cadena: bool,
    /// 13-10: la app pide la ESCALERA de T1c (un semaforo por escalon:
    /// estado, vertices, dibujo) para saber DONDE se para la 3060 si no
    /// paga. Mas lento; para diagnosticar (`--3060-escalera`).
    pub escalera: bool,
    /// P3b4c.9 Z1: lo decide el KERNEL, nunca la app (la receta no lo
    /// lleva): el dibujo va a la ventana de la pantalla que el ESCRITORIO le
    /// dio (`gpu_trabajo::pantalla_para`), no a la RAM de la app; el destino
    /// queda para sus medidas y su formato (el de `color`), sin prestarlo.
    pub pantalla: bool,
    /// ** E8f (DL18, 09-10): los BYTES del cbuffer que se atan como el banco
    /// [`BANCO_APP`] de los dos programas (sus primeros bytes en los DATOS);
    /// 0 = ninguno. Lo pide la receta (+88 bit 1) cuando un cuerpo lee su
    /// cbuffer con un LDC (`ConstantesEn`), y el juez R7 lo mira contra esta
    /// misma medida.
    pub banco: u32,
    /// ** LI2d-HDR (12-10): el redondeo de la salida 0 al formato del render
    /// target de float (`pegamento::Redondeo`). Solo con un destino de float
    /// (`destino::Flotante`); la receta lo lleva en la bandera del destino.
    pub redondeo: crate::pegamento::Redondeo,
    /// ** 12-10: la MEZCLA del render target 0, en el sombreador
    /// (`mezcla`): el pegamento lee lo que ya esta en el destino. Solo con un
    /// destino en la RAM de la app, sin Z y sin limpieza en el lote; la
    /// receta la lleva en su bloque (+88 bit 2).
    pub mezcla: crate::mezcla::Mezcla,
    /// ** LI2b (12-10): los registros que pide este dibujo
    /// ([`registros_para`]); 0 = [`REGISTROS`], lo de siempre. Lo pone el
    /// KERNEL (`receta::paquete`, con lo que midio el pegamento), no la app.
    pub registros: u32,
    /// ** LI2d (12-10): los render targets 1.. (el 0 es [`Dibujo::destino`]),
    /// cada uno con la VA de sus pixeles en la app y su forma; `n_mas`
    /// cuantos. Todos de la medida del 0, sin Z (la sombra es UNA) y en la
    /// RAM de la app (`cabe`). Cada uno va a su ranura (`destino::va_de`).
    pub mas: [(u64, crate::destino::Destino); crate::destino::OBJETIVOS - 1],
    pub n_mas: u8,
}

impl Dibujo {
    /// Los render targets que escribe (el 0 y los `mas`).
    pub const fn objetivos(&self) -> u32 {
        if self.destino.is_some() {
            1 + self.n_mas as u32
        } else {
            1
        }
    }

    /// Los render targets 1.. de verdad.
    pub fn mas(&self) -> &[(u64, crate::destino::Destino)] {
        &self.mas[..(self.n_mas as usize).min(self.mas.len())]
    }

    /// Los registros que las ordenes dan a sus dos programas.
    pub const fn registros_dados(&self) -> u32 {
        if self.registros == 0 {
            REGISTROS
        } else {
            self.registros
        }
    }
}

/// ** E8f: el banco de constantes de una app: el 3, el mismo que el de E3 de
/// PROTON-X (`BANCO_CB`), asi el LDC se escribe igual en sus dos ABI.
pub const BANCO_APP: u8 = 3;

/// La medida con la que se ata un banco: a 256 bytes (lo que hace nouveau
/// con un cbuffer, `align(size, 0x100)`), sin pasar de los DATOS. Lo que
/// sobra detras son los vertices de la misma app; lo que el cuerpo puede
/// leer lo acota el juez (R7) con la medida SIN redondear.
pub const fn medida_del_banco(bytes: u32) -> u32 {
    let m = bytes.next_multiple_of(256);
    if m > DATOS_MAX as u32 { DATOS_MAX as u32 } else { m }
}

/// Los cuatro canales (R, G, B, A) de un pixel de 8 bits como floats de
/// `SET_COLOR_CLEAR_VALUE`: `byte / 255`, que la 3060 vuelve a redondear al
/// MISMO byte (`round(x * 255)`).
pub fn color_de_limpieza(pixel: u32, rgb: bool) -> [u32; 4] {
    let c = |k: u32| (((pixel >> (8 * k)) & 0xFF) as f32 / 255.0).to_bits();
    if rgb { [c(0), c(1), c(2), c(3)] } else { [c(2), c(1), c(0), c(3)] }
}

/// La Z de un `Dibujo` en los bits 3..9 del estado de VRN1.
const fn estado_z(z: Option<crate::profundidad::Z>) -> u32 {
    match z {
        None => 0,
        Some(z) => 1 << 3 | (z.funcion & 0xF) << 4 | (z.escribir as u32) << 8 | (z.limpiar.is_some() as u32) << 9,
    }
}

/// La Z de los bits 3..9 del estado (la limpieza, a 1.0), o `Err` si no se
/// sostiene.
fn z_de(estado: u32) -> Result<Option<crate::profundidad::Z>, ()> {
    if estado & 1 << 3 == 0 {
        return if estado >> 4 & 0x3F == 0 { Ok(None) } else { Err(()) };
    }
    let z = crate::profundidad::Z { funcion: estado >> 4 & 0xF, escribir: estado & 1 << 8 != 0, limpiar: (estado & 1 << 9 != 0).then_some(crate::profundidad::UNO) };
    if z.valida() { Ok(Some(z)) } else { Err(()) }
}

/// El bit de +60 que dice que hay destino.
const HAY_DESTINO: u32 = 1 << 31;
/// Q0a2: +20 de VRN1 dice que +24 es el pixel de la limpieza del destino.
const HAY_COLOR_1: u32 = 1;

/// VRN1 con la limpieza del destino (Q0a2): +20 es la marca, no un recorte.
fn color_1(cabecera: &[u8]) -> bool {
    u32le(cabecera, 0) == MAGIA_1 && u32le(cabecera, 20) == HAY_COLOR_1
}

/// `OGL_SET_FRONT_FACE` / `OGL_SET_CULL_FACE` (clc797.h de NVIDIA).
pub const OGL_SET_FRONT_FACE: u32 = 0x191c;
pub const OGL_SET_CULL_FACE: u32 = 0x1920;
pub const DELANTE_HORARIO: u32 = 0x900;
pub const DELANTE_ANTIHORARIO: u32 = 0x901;
pub const CULL_DELANTERAS: u32 = 0x404;
pub const CULL_TRASERAS: u32 = 0x405;
/// El bufer de indices: `SET_INDEX_BUFFER_A/B` (la direccion),
/// `SET_INDEX_BUFFER_SIZE_A/B` (sus bytes), `SET_INDEX_BUFFER_E` (2 = u32),
/// y el dibujo, `SET_INDEX_BUFFER_F` (el primero) y `DRAW_INDEX_BUFFER`
/// (cuantos), entre BEGIN y END (clc797.h; el orden de NVK).
pub const SET_INDEX_BUFFER_A: u32 = 0x17c8;
pub const SET_INDEX_BUFFER_SIZE_A: u32 = 0x0238;
pub const SET_INDEX_BUFFER_E: u32 = 0x17d8;
pub const SET_INDEX_BUFFER_F: u32 = 0x17dc;
pub const INDICES_U32: u32 = 2;

/// **El paquete**: los dos programas (tomados del BSF por la app, con sus
/// hashes comprobados) y los vertices, tal como llegan del escritorio.
#[derive(Clone, Copy, Debug)]
pub struct Paquete<'a> {
    pub ficha: u32,
    pub vs: &'a [u8],
    pub ps: &'a [u8],
    /// Los DATOS que lee el programa por la ranura 0 de la tabla: en V0 los
    /// vertices (`Vertice`, 32 B); con un programa emitido (E5), lo que su
    /// pegamento diga (el cbuffer y detras los vertices).
    pub vertices: &'a [u8],
    /// Cuantos vertices se dibujan (3 por triangulo).
    pub n: usize,
    /// V1c: lo que HACE FALTA limpiar, si quien dibuja lo sabe --
    /// `(xmin | xmax << 16, ymin | ymax << 16)` en pixeles de la ventana, el
    /// maximo fuera (`SET_CLEAR_RECT_*`). `None` = la ventana entera. Solo lo
    /// usa el anillo en modo `coopera`; los demas limpian siempre todo.
    pub limpiar: Option<(u32, u32)>,
    /// VRN1: con indices y descarte. En V0, el de siempre.
    pub dibujo: Dibujo,
}

/// Un recorte que cabe en la ventana y no es vacio.
pub const fn recorte_valido(h: u32, v: u32) -> bool {
    let (x0, x1, y0, y1) = (h & 0xFFFF, h >> 16, v & 0xFFFF, v >> 16);
    x0 < x1 && x1 <= cu::ANCHO && y0 < y1 && y1 <= cu::ALTO
}

pub(crate) fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Un programa que el kernel acepta subir: la SPH del tipo que toca y
/// cabe en su [`HUECO`].
fn programa_valido(p: &[u8], tipo: u32) -> bool {
    p.len() >= 4 * SPH + 16 && p.len() % 16 == 0 && p.len() <= HUECO && u32le(p, 0) & 0x1F == if tipo == crate::raster::VERTICE { 1 } else { 2 } && u32le(p, 0) >> 10 & 0xF == tipo
}

/// Cuanto mide la cabecera que empieza asi (V0 o VRN1), o `None`.
fn cabecera_de(cabecera: &[u8]) -> Option<usize> {
    let c = match u32le(cabecera.get(..4)?, 0) {
        MAGIA => CABECERA,
        MAGIA_1 => CABECERA_1,
        _ => return None,
    };
    (cabecera.len() >= c).then_some(c)
}

/// El `Dibujo` de una cabecera VRN1 ya medida (sin mirar los indices).
fn dibujo_de(cabecera: &[u8], n: usize, datos: usize) -> Option<Dibujo> {
    if cabecera_de(cabecera)? == CABECERA {
        return Some(Dibujo::default());
    }
    let mut d = dibujo_de_campos(cabecera, n, datos)?;
    // Q0a2: la limpieza del destino, por la 3060 (y sin destino, no hay que).
    if color_1(cabecera) {
        d.destino?;
        d.color = Some(u32le(cabecera, 24));
    }
    Some(d)
}

/// El `Dibujo` de los campos +28..+64 (los de VRN1, que la RECETA VRN2
/// repite en el mismo sitio), sin mirar la magia.
pub(crate) fn dibujo_de_campos(cabecera: &[u8], n: usize, datos: usize) -> Option<Dibujo> {
    let (desde, estado, vertices) = (u32le(cabecera, 32), u32le(cabecera, 36), u32le(cabecera, 40));
    if estado >> 10 != 0 || u32le(cabecera, 28) == 0 || vertices == 0 {
        return None;
    }
    let bandera = u32le(cabecera, 60);
    let destino = if bandera & HAY_DESTINO == 0 {
        if cabecera[44..CABECERA_1].iter().any(|&b| b != 0) {
            return None;
        }
        None
    } else {
        let va = u32le(cabecera, 44) as u64 | (u32le(cabecera, 48) as u64) << 32;
        let medidas = u32le(cabecera, 56);
        // La bandera: el 0 rgb, el 1..3 el float (LI2d-HDR), el 8..16 el
        // redondeo (abajo); nada mas.
        let flotante = crate::destino::Flotante::de_numero(bandera >> 1 & 3)?;
        let d = crate::destino::Destino { fila: u32le(cabecera, 52), ancho: medidas & 0xFFFF, alto: medidas >> 16, flotante, rgb: bandera & 1 != 0 };
        if bandera & !(HAY_DESTINO | 7 | 0xFF << 8) != 0 || !d.valido_en(va) || va == 0 {
            return None;
        }
        Some((va, d))
    };
    let descarte = match estado & 3 {
        0 => Descarte::Ninguna,
        1 => Descarte::Traseras,
        2 => Descarte::Delanteras,
        _ => return None,
    };
    let indices = if desde == SIN_INDICES {
        if n > vertices as usize {
            return None;
        }
        None
    } else {
        if desde % 4 != 0 || desde as usize + 4 * n > datos {
            return None;
        }
        Some(desde)
    };
    let z = z_de(estado).ok()?;
    let redondeo = crate::pegamento::Redondeo::de_numero(bandera >> 8 & 0xFF)?;
    let d = Dibujo { indices, vertices, descarte, antihorario: estado & 4 != 0, destino, z, color: None, texturas: 0, cadena: false, escalera: false, pantalla: false, banco: 0, redondeo, mezcla: crate::mezcla::Mezcla::NINGUNA, registros: 0, mas: Default::default(), n_mas: 0 };
    cabe(&d).then_some(d)
}

/// ** Q0a1 (EL_FOCO, 10-10): un destino de CUALQUIER medida que quepa
/// (`Destino::valido`), salvo con Z: la sombra en bloque (`sombra`) y el
/// bufer de Z (`profundidad`) miden 1280x720.
pub const fn cabe(d: &Dibujo) -> bool {
    // LI2d-HDR: un destino de float, hoy sin Z (la sombra en bloque es de 8
    // bits por canal), sin limpieza (`color` es un pixel de 8 bits) y fuera
    // de la pantalla; y el redondeo, solo con el.
    let flotante = match d.destino {
        Some((_, dst)) => !matches!(dst.flotante, crate::destino::Flotante::No),
        None => false,
    };
    if flotante && (d.z.is_some() || d.color.is_some() || d.cadena || d.pantalla) {
        return false;
    }
    if !flotante && d.redondeo.numero() != 0 {
        return false;
    }
    // La mezcla lee el DESTINO de la app: sin Z (el color iria a la sombra),
    // sin limpieza en el lote y nunca a la pantalla.
    if !d.mezcla.trivial() && (d.destino.is_none() || d.z.is_some() || d.color.is_some() || d.pantalla || d.mezcla.valida().is_err()) {
        return false;
    }
    // 13-10: un render target de ENTEROS (`Flotante::Uint8`) no mezcla ni
    // enmascara (D3D12 no mezcla enteros; la mascara va por la mezcla).
    if !d.mezcla.trivial() && matches!(d.destino, Some((_, crate::destino::Destino { flotante: crate::destino::Flotante::Uint8, .. }))) {
        return false;
    }
    // LI2d: los render targets 1.., con destino, sin Z (la sombra es UNA),
    // sin limpieza (la de la app es del 0), fuera de la pantalla, de la
    // medida del 0 y cada uno valido donde esta.
    if d.n_mas > 0 {
        let Some((_, d0)) = d.destino else { return false };
        if d.n_mas as usize >= crate::destino::OBJETIVOS || d.z.is_some() || d.color.is_some() || d.cadena || d.pantalla {
            return false;
        }
        let mut k = 0;
        while k < d.n_mas as usize {
            let (va, x) = d.mas[k];
            if !x.valido_en(va) || x.ancho != d0.ancho || x.alto != d0.alto {
                return false;
            }
            k += 1;
        }
    }
    match (d.z, d.destino) {
        (Some(_), Some((_, dst))) => dst.es_la_de_verrano(),
        _ => true,
    }
}

/// Cuanto mide el paquete que dice esta cabecera (o `None` si no lo es).
pub fn medida(cabecera: &[u8]) -> Option<usize> {
    let c = cabecera_de(cabecera)?;
    let (h, v) = (u32le(cabecera, 20), u32le(cabecera, 24));
    if (h, v) != (0, 0) && !recorte_valido(h, v) && !color_1(cabecera) {
        return None;
    }
    let (n, vs, ps) = (u32le(cabecera, 8) as usize, u32le(cabecera, 12) as usize, u32le(cabecera, 16) as usize);
    if n == 0 || n % 3 != 0 || n > MAX_VERTICES || vs > HUECO || ps > HUECO {
        return None;
    }
    let d = datos(cabecera, n)?;
    dibujo_de(cabecera, n, d)?;
    Some(c + vs + ps + d)
}

/// Los bytes de los datos que dice la cabecera: los de V0 (32 por vertice)
/// o los que diga (E5), enteros de 16 y hasta [`DATOS_MAX`].
fn datos(cabecera: &[u8], n: usize) -> Option<usize> {
    match u32le(cabecera, 28) as usize {
        0 => (n * BYTES_VERTICE <= DATOS_MAX).then_some(n * BYTES_VERTICE),
        d => (d % 16 == 0 && d <= DATOS_MAX).then_some(d),
    }
}

/// **Leer** un paquete entero (ya medido).
pub fn leer(b: &[u8]) -> Option<Paquete<'_>> {
    let total = medida(b)?;
    if b.len() != total {
        return None;
    }
    let c = cabecera_de(b)?;
    let (vs, ps) = (u32le(b, 12) as usize, u32le(b, 16) as usize);
    let (h, v) = (u32le(b, 20), u32le(b, 24));
    let limpiar = ((h, v) != (0, 0) && !color_1(b)).then_some((h, v));
    let n = u32le(b, 8) as usize;
    let vertices = &b[c + vs + ps..];
    let dibujo = dibujo_de(b, n, vertices.len())?;
    // Cada indice, de un vertice que ESTA en los datos: la 3060 no lee fuera.
    if let Some(desde) = dibujo.indices {
        if (0..n).any(|k| u32le(vertices, desde as usize + 4 * k) >= dibujo.vertices) {
            return None;
        }
    }
    let p = Paquete { ficha: u32le(b, 4), vs: &b[c..c + vs], ps: &b[c + vs..c + vs + ps], vertices, n, limpiar, dibujo };
    (programa_valido(p.vs, crate::raster::VERTICE) && programa_valido(p.ps, crate::raster::PIXEL)).then_some(p)
}

/// **Escribir** un paquete en `out`; devuelve cuanto mide.
pub fn escribir_paquete(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], vertices: &[Vertice]) -> Option<usize> {
    escribir_paquete_con(out, ficha, vs, ps, vertices, None)
}

/// Lo mismo, con el recorte de la limpieza (V1c).
pub fn escribir_paquete_con(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], vertices: &[Vertice], limpiar: Option<(u32, u32)>) -> Option<usize> {
    escribir_paquete_de(out, ficha, vs, ps, vertices.iter().copied(), limpiar)
}

/// **Lo mismo, con los vertices segun se sacan**: cada uno va derecho al
/// paquete, sin un arreglo en medio (2026-09-28).
///
/// El director armaba un `[Vertice; MAX_VERTICES]` en SU pila para llamar a
/// [`escribir_paquete_con`], y cuando `MAX_VERTICES` subio a 3 * 4096 ese
/// arreglo paso a medir 384 KiB contra una pila de Ring 3 de 64 KiB. Con esto
/// quien tiene los vertices en otra forma (`bmo_verrano::Vertex`) los
/// convierte al vuelo y lo que ocupa su pila no depende de ningun tope.
pub fn escribir_paquete_de<I>(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], vertices: I, limpiar: Option<(u32, u32)>) -> Option<usize>
where
    I: IntoIterator<Item = Vertice>,
    I::IntoIter: ExactSizeIterator,
{
    let vertices = vertices.into_iter();
    let n = vertices.len();
    let d = CABECERA + vs.len() + ps.len();
    if out.len() < d + n * BYTES_VERTICE {
        return None;
    }
    let mut i = d;
    for v in vertices.take(n) {
        for w in v.palabras() {
            out[i..i + 4].copy_from_slice(&w.to_le_bytes());
            i += 4;
        }
    }
    cerrar_paquete(out, ficha, vs, ps, n, 0, n * BYTES_VERTICE, limpiar)
}

/// **E5**: un paquete con `n` vertices y los DATOS tal cual (el cbuffer y
/// los vertices como los lee el pegamento de los programas emitidos).
pub fn escribir_paquete_datos(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], n: usize, datos: &[u8]) -> Option<usize> {
    let d = CABECERA + vs.len() + ps.len();
    if datos.is_empty() || out.len() < d + datos.len() {
        return None;
    }
    out[d..d + datos.len()].copy_from_slice(datos);
    cerrar_paquete(out, ficha, vs, ps, n, datos.len() as u32, datos.len(), None)
}

/// **P3b4b**: un paquete VRN1 -- `n` vertices (o `n` indices), los DATOS
/// tal cual y COMO se dibujan. `None` si no se sostiene (un indice de un
/// vertice que no esta, tambien).
pub fn escribir_paquete_dibujo(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], n: usize, datos: &[u8], dibujo: Dibujo) -> Option<usize> {
    let d = CABECERA_1 + vs.len() + ps.len();
    let total = d + datos.len();
    if datos.is_empty() || out.len() < total {
        return None;
    }
    out[d..total].copy_from_slice(datos);
    cerrar_paquete_1(out, ficha, vs, ps, n, datos.len(), dibujo)
}

/// ** Q0a2 (EL_FOCO, 10-10): un VRN1 con los vertices de V0 (32 B, sin
/// indices) escritos DIRECTO en su sitio del paquete: la lamina de una app
/// pasa al paquete en UNA copia, sin un bufer de por medio (la LAMINA
/// quirofano: un dato, un sitio).
pub fn escribir_paquete_dibujo_de<I>(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], vertices: I, dibujo: Dibujo) -> Option<usize>
where
    I: IntoIterator<Item = Vertice>,
    I::IntoIter: ExactSizeIterator,
{
    let vertices = vertices.into_iter();
    let n = vertices.len();
    let d = CABECERA_1 + vs.len() + ps.len();
    if n == 0 || out.len() < d + n * BYTES_VERTICE || dibujo.indices.is_some() || dibujo.vertices as usize != n {
        return None;
    }
    let mut i = d;
    for v in vertices.take(n) {
        for w in v.palabras() {
            out[i..i + 4].copy_from_slice(&w.to_le_bytes());
            i += 4;
        }
    }
    cerrar_paquete_1(out, ficha, vs, ps, n, n * BYTES_VERTICE, dibujo)
}

/// La cabecera VRN1 y los dos programas, con los `datos` bytes ya en su
/// sitio (detras); `None` si el paquete que queda no se sostiene.
#[allow(clippy::too_many_arguments)]
fn cerrar_paquete_1(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], n: usize, datos: usize, dibujo: Dibujo) -> Option<usize> {
    let d = CABECERA_1 + vs.len() + ps.len();
    let total = d + datos;
    out[..CABECERA_1].fill(0);
    let estado = match dibujo.descarte {
        Descarte::Ninguna => 0,
        Descarte::Traseras => 1,
        Descarte::Delanteras => 2,
    } | (dibujo.antihorario as u32) << 2
        | estado_z(dibujo.z);
    // VRN1 limpia la Z a 1.0 y no lleva otro valor. El COLOR, desde Q0a2
    // (10-10), si: el del destino, por la 3060 -- y la CPU deja de llenar el
    // bloque a mano en cada fotograma --.
    if dibujo.color.is_some() && dibujo.destino.is_none() || dibujo.texturas != 0 || dibujo.z.is_some_and(|z| z.limpiar.is_some_and(|v| v != crate::profundidad::UNO)) {
        return None;
    }
    let (va, dst) = dibujo.destino.unwrap_or_default();
    let bandera = if dibujo.destino.is_some() { HAY_DESTINO | dst.rgb as u32 } else { 0 };
    let (marca, pixel) = dibujo.color.map_or((0, 0), |c| (HAY_COLOR_1, c));
    let palabras = [MAGIA_1, ficha, n as u32, vs.len() as u32, ps.len() as u32, marca, pixel, datos as u32, dibujo.indices.unwrap_or(SIN_INDICES), estado, dibujo.vertices, va as u32, (va >> 32) as u32, dst.fila, dst.ancho | dst.alto << 16, bandera];
    for (k, w) in palabras.iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
    out[CABECERA_1..CABECERA_1 + vs.len()].copy_from_slice(vs);
    out[CABECERA_1 + vs.len()..d].copy_from_slice(ps);
    leer(&out[..total]).map(|_| total)
}

/// La cabecera y los dos programas (los datos ya estan detras); `None` si el
/// paquete que queda no se sostiene.
#[allow(clippy::too_many_arguments)]
fn cerrar_paquete(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], n: usize, palabra_datos: u32, datos: usize, limpiar: Option<(u32, u32)>) -> Option<usize> {
    let total = CABECERA + vs.len() + ps.len() + datos;
    out[..CABECERA].fill(0);
    let (h, v) = limpiar.unwrap_or((0, 0));
    for (k, v) in [MAGIA, ficha, n as u32, vs.len() as u32, ps.len() as u32, h, v, palabra_datos].iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
    }
    out[CABECERA..CABECERA + vs.len()].copy_from_slice(vs);
    out[CABECERA + vs.len()..CABECERA + vs.len() + ps.len()].copy_from_slice(ps);
    (medida(&out[..total]) == Some(total)).then_some(total)
}

/// Escribe bytes (multiplo de 4) en la VRAM, de 64 palabras en 64.
pub(crate) fn escribir_bytes<R: Registros>(r: &mut R, dir: u64, b: &[u8]) -> bool {
    let mut w = [0u32; 64];
    for (k, trozo) in b.chunks(256).enumerate() {
        let n = trozo.len() / 4;
        for (i, p) in trozo.chunks_exact(4).enumerate() {
            w[i] = u32le(p, 0);
        }
        if escribir(r, dir + 256 * k as u64, &w[..n]) != n {
            return false;
        }
    }
    true
}

/// **Un programa en su sitio** (LI2b): sus bytes, RELEIDOS, y lo que queda
/// de su ultima pagina a cero (lo que hubiera de un programa anterior mas
/// largo no queda detras del EXIT). Sin tocar las paginas que no usa: un
/// programa de 40 instrucciones no paga las 17.
pub(crate) fn escribir_programa<R: Registros>(r: &mut R, dir: u64, b: &[u8]) -> bool {
    if !escribir_bytes(r, dir, b) {
        return false;
    }
    let fin = dir + b.len() as u64;
    let resto = (fin.next_multiple_of(0x1000) - fin) as usize / 4;
    let ceros = [0u32; 64];
    let mut k = 0;
    while k < resto {
        let m = (resto - k).min(64);
        if escribir(r, fin + 4 * k as u64, &ceros[..m]) != m {
            return false;
        }
        k += m;
    }
    true
}

// == Las ordenes y el preparar ===============================================

/// **Las ordenes**: las de X5 hasta el dibujo, y UN dibujo de `3n` vertices.
pub fn ordenes(v: &Ventana, n: usize) -> cu::Ordenes {
    ordenes_con(v, n, false)
}

/// Las mismas, `ligero` = SIN la escalera de T1c (ver `cubo::Ordenes`).
pub fn ordenes_con(v: &Ventana, n: usize, ligero: bool) -> cu::Ordenes {
    ordenes_dibujo(v, n, ligero, Dibujo::default())
}

/// **P3b4b**: las mismas, y el `Dibujo` -- el descarte de caras encendido
/// en el hardware, y el dibujo CON INDICES (los de los datos, u32).
pub fn ordenes_dibujo(v: &Ventana, n: usize, ligero: bool, d: Dibujo) -> cu::Ordenes {
    // Con destino, la RAM de la app y SIN limpiar: el juego limpia su back
    // buffer (ClearRenderTargetView) antes de dibujar en el.
    let (v, limpiar) = match d.destino {
        // Z1: en la pantalla, y SIN la limpieza de VERRANO: limpia la app.
        Some(_) if d.pantalla => (*v, false),
        Some((va, dst)) => (dst.ventana_en(va), false),
        None => (*v, true),
    };
    // P3b4c.6b: con Z, el color va a la SOMBRA en bloque (la 3060 no dibuja
    // con Z sobre un color pitch: Xid 69 en el metal). La limpieza de la
    // ventana, entonces, en la sombra; y el motor de copia la lleva despues
    // a `v` (el kernel, `sombra::copia`).
    let mut e = cu::hasta_el_dibujo_de(&v, !ligero, limpiar && d.z.is_none(), None, d.registros_dados(), PROGRAMAS_VA);
    // ** LI2d (12-10): los render targets 1.., cada uno en SU ranura, y el
    // CT_SELECT con todos (el de `hasta_el_dibujo_de` dice uno). Sin Z ni
    // pantalla (`cabe`): el 0 es el destino de la app, como los demas.
    if d.n_mas > 0 && !d.pantalla && d.z.is_none() {
        for (j, &(va, dst)) in d.mas().iter().enumerate() {
            let k = j as u32 + 1;
            let w = dst.ventana_de(k as usize, va);
            e.m(crate::tresde::set_color_target_a(k), &[(w.va >> 32) as u32, w.va as u32, w.fila, w.alto, w.flotante.formato(w.rgb), crate::tresde::MEMORIA_PITCH, 1, 0]);
            e.m(crate::raster::set_color_target_layer(k), &[0]);
            e.m(crate::raster::set_color_compression(k), &[0]);
            e.m(crate::raster::set_blend(k), &[0]);
            e.m(crate::tresde::set_ct_write(k), &[crate::tresde::ESCRIBIR_RGBA]);
        }
        e.m(crate::tresde::SET_CT_SELECT, &[crate::tresde::ct_select(d.objetivos())]);
    }
    if d.z.is_some() {
        crate::sombra::al_color(&mut e, &v);
        if limpiar {
            e.m(crate::tresde::SET_CLEAR_SURFACE_CONTROL, &[0]);
            e.m(crate::tresde::CLEAR_SURFACE, &[crate::tresde::LIMPIAR_RGBA]);
            e.m(crate::tresde::WAIT_FOR_IDLE, &[0]);
        }
    }
    if d.descarte != Descarte::Ninguna {
        e.m(OGL_SET_FRONT_FACE, &[if d.antihorario { DELANTE_ANTIHORARIO } else { DELANTE_HORARIO }]);
        e.m(OGL_SET_CULL_FACE, &[if d.descarte == Descarte::Traseras { CULL_TRASERAS } else { CULL_DELANTERAS }]);
        e.m(crate::raster::OGL_SET_CULL, &[1]);
    }
    // P3b4c: la limpieza del destino que pidio la app, por la 3060.
    if let (Some(pixel), Some((_, dst))) = (d.color, d.destino) {
        e.m(crate::tresde::SET_COLOR_CLEAR_VALUE0, &color_de_limpieza(pixel, dst.rgb));
        e.m(crate::tresde::SET_CLEAR_SURFACE_CONTROL, &[0]);
        e.m(crate::tresde::CLEAR_SURFACE, &[crate::tresde::LIMPIAR_RGBA]);
        e.m(crate::tresde::WAIT_FOR_IDLE, &[0]);
    }
    if let Some(z) = d.z {
        crate::profundidad::ordenes(&mut e, &z);
    }
    // ** E8f: el banco de la app -- su cbuffer, al principio de los DATOS --,
    // atado a los dos programas.
    if d.banco > 0 {
        use crate::tresde::{bind_group_constant_buffer, GRUPO_PIXEL, GRUPO_VERTICE, SET_CONSTANT_BUFFER_SELECTOR_A};
        let va = crate::vram::DATOS_VA;
        e.m(SET_CONSTANT_BUFFER_SELECTOR_A, &[medida_del_banco(d.banco), (va >> 32) as u32, va as u32]);
        for g in [GRUPO_VERTICE, GRUPO_PIXEL] {
            e.m(bind_group_constant_buffer(g), &[1 | (BANCO_APP as u32) << 4]);
        }
    }
    // P3b4c.8 T2: las piscinas de las texturas (G5: los 16 TIC de la
    // textura k, `mipmap`, y su TSC k), con sus caches invalidadas: el kernel
    // acaba de escribirlas.
    if d.texturas > 0 {
        let n = d.texturas as u32;
        let t = crate::texturas::PISCINAS_VA;
        crate::texturas::ordenes(&mut e, t, crate::mipmap::HUECOS * n, t + crate::texturas::TSC_DESDE, n);
    }
    match d.indices {
        None => e.dibujo_de(3 * n as u32),
        Some(desde) => {
            let va = crate::vram::DATOS_VA + desde as u64;
            let bytes = 4 * 3 * n as u32;
            e.m(SET_INDEX_BUFFER_A, &[(va >> 32) as u32, va as u32]);
            e.m(SET_INDEX_BUFFER_SIZE_A, &[0, bytes]);
            e.m(SET_INDEX_BUFFER_E, &[INDICES_U32]);
            e.m(crate::raster::BEGIN, &[crate::raster::TRIANGULOS]);
            e.m(SET_INDEX_BUFFER_F, &[0, 3 * n as u32]);
            e.m(crate::raster::END, &[0]);
        }
    }
    e.cerrar();
    e
}

/// **Preparar** un paquete: los semaforos a cero, los DOS programas (tal
/// como llegan: el kernel no traduce nada), la tabla, los vertices, las
/// ordenes y la entrada.
pub fn preparar<R: Registros>(r: &mut R, e: u32, v: &Ventana, p: &Paquete) -> bool {
    preparar_con(r, e, v, p, false)
}

/// `preparar`, con las ordenes `ligero` o con su escalera.
pub fn preparar_con<R: Registros>(r: &mut R, e: u32, v: &Ventana, p: &Paquete, ligero: bool) -> bool {
    preparar_paso(r, e, v, p, ligero).is_ok()
}

/// **`preparar_con`, diciendo QUE PASO fallo** (12-10): `Err(k)` con `k` el
/// indice de `porque::PASOS`. El kernel lo apunta como
/// `Porque::Preparar` (el metal daba un 79 sin mas).
pub fn preparar_paso<R: Registros>(r: &mut R, e: u32, v: &Ventana, p: &Paquete, ligero: bool) -> Result<(), u8> {
    let n = p.n;
    if !crate::blur::entrada_valida(e) || n == 0 || n % 3 != 0 || n > MAX_VERTICES {
        return Err(0);
    }
    let o = ordenes_dibujo(v, n / 3, ligero, p.dibujo);
    let en = entrada(sombreador_va(EMPUJE), o.n as u32);
    let va = crate::vram::DATOS_VA;
    // En orden, y el primero que falla corta (como el `&&` de antes): la
    // entrada del GPFIFO, la ultima, nunca apunta a algo a medias.
    let mut paso = 0u8;
    let mut hecho = |bien: bool| {
        paso += 1;
        if bien { Ok(()) } else { Err(paso) }
    };
    hecho(escribir(r, SEMAFORO_FIN, &[0; 4]) == 4)?;
    hecho(escribir(r, ESCALONES, &[0; N_ESCALONES as usize]) == N_ESCALONES as usize)?;
    hecho(a_cero(r, SUSTITUTO) as usize == crate::vram::PALABRAS)?;
    hecho(escribir_programa(r, VS, p.vs))?;
    hecho(escribir_programa(r, PS, p.ps))?;
    hecho(escribir(r, TABLA, &[va as u32, (va >> 32) as u32]) == 2)?;
    hecho(escribir(r, crate::libreta::LIBRETA, &[0]) == 1)?;
    hecho(escribir_bytes(r, DATOS, p.vertices))?;
    hecho(escribir(r, EMPUJE, &o.o[..o.n]) == o.n)?;
    hecho(escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2)?;
    Ok(())
}

pub use crate::cubo::{empaquetar, lanzar, mirar, sano};

// == EN CALIENTE (V1, 26-09) =================================================
//
// `preparar` pone a cero TRES paginas de VRAM y RELEE cada palabra: 3072
// lecturas por PCIe (~1 us cada una, lo medido en `volcado`), y relee
// tambien los programas, la tabla, los vertices y las ordenes. Para UN
// fotograma es lo correcto. Para 360 seguidos con los MISMOS programas es
// volver a escribir lo que ya esta escrito y preguntar 4.000 veces si llego.
//
// En caliente solo va lo que CAMBIA de un fotograma a otro, y sin releer
// (como `volcado`): los semaforos a cero, los vertices y la entrada del
// GPFIFO. La prueba de que llego no es releer: es que la 3060 pague el
// semaforo del dibujo -- y el banco, al acabar, juzga un fotograma contra
// D3D12 hecho TAMBIEN en caliente.
//
// [!] Quien decide si se puede es el KERNEL (`gpu_trabajo/cubo.rs`), no el
// que pide: los mismos programas (esta huella), nadie mas lanzo nada por el
// GR desde el ultimo dibujo de VERRANO, y ese dibujo se pago entero.

/// **La huella de lo FIJO** de un dibujo de VERRANO: los dos programas, la
/// ventana, como se dibuja y si va `ligero` -- todo lo que `preparar` escribe
/// y el caliente NO vuelve a escribir. FNV-1a de 64 bits.
///
/// ** Sin el NUMERO de triangulos (10-10, el cuello 6 de EL_FOCO): el cubo
/// que gira descarta caras de espaldas y su cuenta cambia de un fotograma a
/// otro; con la cuenta en la huella, cada cambio era un `preparar` en frio
/// (tres paginas releidas por el PCIe). Las ORDENES, que es donde va la
/// cuenta, las reescribe ahora el caliente ([`preparar_caliente`]).
///
/// **El destino entra por sus MEDIDAS, no por su direccion** (29-09): las
/// ordenes apuntan a `destino::VA` (fija) y lo que cambia de un back buffer
/// a otro es el prestamo de la IOMMU detras de esa VA, que el kernel hace en
/// cada dibujo. Con la direccion dentro, los dos back buffers de un juego
/// daban dos huellas alternas y el caliente no acertaba nunca: el metal del
/// 29-09 07:02 (BMOX-12) dio `preparar` 5,4 ms de 10,3 por lote, siempre en
/// frio, releyendo tres paginas por el PCIe.
pub fn huella_fija(v: &Ventana, p: &Paquete, ligero: bool) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut mezclar = |b: &[u8]| {
        for &x in b {
            h = (h ^ x as u64).wrapping_mul(0x0100_0000_01b3);
        }
    };
    mezclar(p.vs);
    mezclar(&[0xA5]);
    mezclar(p.ps);
    let d = p.dibujo;
    let dibujo = d.indices.unwrap_or(SIN_INDICES) as u64 | (d.descarte as u64) << 32 | (d.antihorario as u64) << 34;
    let z = estado_z(d.z) as u64 | (d.z.and_then(|z| z.limpiar).unwrap_or(0) as u64) << 32;
    let color = d.color.map_or(0, |c| 1 << 32 | c as u64) | (d.texturas as u64) << 40;
    let dst = d.destino.map_or(Default::default(), |(_, dst)| dst);
    let hay_destino = d.destino.is_some() as u64 | (d.cadena as u64) << 1 | (d.pantalla as u64) << 2;
    // LI2d: los render targets 1.. (su forma y donde empiezan en su pagina).
    for &(va, x) in d.mas() {
        for y in [va % 4096, x.fila as u64 | (x.ancho as u64) << 32, x.alto as u64 | (x.rgb as u64) << 32 | (x.flotante.numero() as u64) << 33] {
            mezclar(&y.to_le_bytes());
        }
    }
    for x in [dibujo, z, color, hay_destino, dst.fila as u64 | (dst.ancho as u64) << 32, dst.alto as u64 | (dst.rgb as u64) << 32 | (dst.flotante.numero() as u64) << 33 | (d.redondeo.numero() as u64) << 40, v.x0 as u64, v.y0 as u64, v.va, v.fila as u64, v.rgb as u64, ligero as u64, d.destino.map_or(0, |(va, _)| va % 4096), d.registros_dados() as u64] {
        mezclar(&x.to_le_bytes());
    }
    h
}

/// **Preparar EN CALIENTE**: lo que cambia, sin releer. Los programas y la
/// tabla tienen que estar ya donde los dejo un `preparar_con` con la misma
/// [`huella_fija`] (eso lo comprueba el kernel, no esto).
///
/// ** Las ORDENES se reescriben aqui tambien (10-10): llevan la cuenta de
/// triangulos, que cambia al girar. Son unos cientos de bytes por el BAR0 sin
/// releer -- lo mismo que un lote de `volcado` --, contra las 3072 lecturas
/// del frio que costaban.
pub fn preparar_caliente<R: Registros>(r: &mut R, e: u32, v: &Ventana, p: &Paquete, ligero: bool) -> bool {
    let n = p.n;
    if !crate::blur::entrada_valida(e) || n == 0 || n % 3 != 0 || n > MAX_VERTICES {
        return false;
    }
    let o = ordenes_dibujo(v, n / 3, ligero, p.dibujo);
    let en = entrada(sombreador_va(EMPUJE), o.n as u32);
    let vol = crate::volcado::escribir_sin_releer;
    vol(r, SEMAFORO_FIN, &[0; 4]);
    // 9d: la libreta, a 0 en CADA dibujo (lo de otro no se arrastra).
    vol(r, crate::libreta::LIBRETA, &[0]);
    if !ligero {
        vol(r, ESCALONES, &[0; N_ESCALONES as usize]);
    }
    let mut w = [0u32; 64];
    for (k, trozo) in p.vertices.chunks(256).enumerate() {
        let m = trozo.len() / 4;
        for (i, q) in trozo.chunks_exact(4).enumerate() {
            w[i] = u32le(q, 0);
        }
        vol(r, DATOS + 256 * k as u64, &w[..m]);
    }
    vol(r, EMPUJE, &o.o[..o.n]);
    vol(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]);
    true
}

const _: () = assert!(PALABRAS_VS * 4 <= cu::PASO_VS as usize && PALABRAS_PS * 4 <= cu::PASO_PS as usize);
// LI2b: cada uno cabe en sus paginas de `vram::CODIGO`, uno detras del otro.
const _: () = assert!(HUECO <= crate::vram::CODIGO_PAGINAS_UNO * 0x1000 && PS + (crate::vram::CODIGO_PAGINAS_UNO as u64 * 0x1000) == crate::vram::CODIGO + crate::vram::CODIGO_PAGINAS as u64 * 0x1000);
const _: () = assert!(TABLA >= crate::video::PARAMETROS + 4 * crate::video::N_PARAMETROS as u64);
const _: () = assert!(TABLA + 8 <= SEMAFOROS + 0x1000 && DATOS_MAX == 64 * 1024);

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;

    const fn sin_control((lo, hi): (u64, u64)) -> (u64, u64) {
        (lo, hi & ((1 << 41) - 1))
    }

    /// Cada codificador da, bit a bit, una instruccion que YA corrio en el
    /// metal (sin contar el control, que aqui se pone a proposito).
    #[test]
    fn codifican_lo_que_ya_corrio() {
        use crate::{blur, color3d, fractal, giro};
        let esta = |c: &[(u64, u64)], x: (u64, u64)| c.iter().any(|&i| sin_control(i) == sin_control(x));
        assert!(esta(&giro::CODIGO, ldg(0x0b, 0x08, 0xe400, 0)), "LDG.E.STRONG.SYS R11, [R8.64+0xe400]");
        assert!(esta(&giro::CODIGO, ldg(0x03, 0x08, 0xe404, 0)), "LDG.E.STRONG.SYS R3, [R8.64+0xe404]");
        assert!(esta(&blur::CODIGO, imad_shl(0x0d, 0x02, 0x80, 0)), "IMAD.SHL.U32 R13, R2, 0x80, RZ");
        assert!(esta(&fractal::CODIGO, iadd3_acarreo(0x0c, 1, 0x04, 0x06, 0)), "IADD3 R12, P1, R4, R6, RZ");
        assert!(esta(&fractal::CODIGO, imad_x(0x0c, 0x05, 0x07, 1, 0)), "IMAD.X R12, R5, 0x1, R7, P1");
        // Los IPA de T2a, con SU control incluido.
        let c = color3d::CODIGO_PS;
        assert_eq!((ipa(0, 0x20, IPA_CONTROL), ipa(1, 0x21, IPA_CONTROL), ipa(2, 0x22, IPA_ULTIMO)), (c[1], c[2], c[3]));
    }

    #[test]
    fn el_vertice() {
        let c = codigo_vs();
        // La tabla: 0x2_0000_E600 = R12:R13 (0x2_0000_0000) + 0xE600.
        assert_eq!((c[2].0 >> 32, c[3].0 >> 32), (0, 2));
        assert_eq!((c[4].0 >> 40, c[5].0 >> 40), (0xE600, 0xE604));
        // Ocho cargas del vertice: R4..R11 desde [R2.64 + 4k].
        for k in 0..8u64 {
            assert_eq!((c[9 + k as usize].0 >> 16 & 0xFF, c[9 + k as usize].0 >> 24 & 0xFF), (4 + k, 2));
            assert_eq!(c[9 + k as usize].0 >> 40, 4 * k);
        }
        // Las barreras: el ALD escribe la 0 y la espera el IMAD.SHL; la tabla
        // la 2 y la espera el IADD3; el vertice la 3 y la espera el AST.
        let esc = |i: usize| c[i].1 >> 46 & 7;
        let esp = |i: usize| c[i].1 >> 52 & 0x3F;
        assert_eq!((esc(1), esp(6)), (0, 1));
        assert_eq!((esc(4), esc(5), esp(7)), (2, 2, 1 << 2));
        assert!((9..17).all(|i| esc(i) == 3));
        assert_eq!(esp(17), 1 << 3);
        // Y el AST de la posicion es el de T1c salvo el control.
        assert_eq!(sin_control(AST_POSICION), sin_control(crate::raster::CODIGO_VS[14]));
    }

    /// El de vertice dice que lee memoria (bit 26) y el resto de su SPH es
    /// la de T2a; el de pixel no lee memoria y no lo dice.
    #[test]
    fn sin_ldg_es_el_mismo_menos_las_cargas() {
        let (a, b) = (codigo_vs(), codigo_vs_sin_ldg());
        for i in 0..INSTR_VS {
            let carga = i == 4 || i == 5 || (9..17).contains(&i);
            // 7 y 17: la misma instruccion, sin esperar las barreras de las cargas.
            let espera = i == 7 || i == 17;
            assert_eq!(a[i] == b[i], !carga && !espera, "instruccion {i}");
            if espera {
                assert_eq!(sin_control(a[i]), sin_control(b[i]), "instruccion {i}");
                assert_eq!(b[i].1 >> 52 & 0x3F, 0, "instruccion {i} no espera nada");
            }
            // Ni un LDG (0x981 en los 12 bits bajos) en la variante.
            assert_ne!(b[i].0 & 0xFFF, 0x981, "instruccion {i}");
        }
        assert_eq!(b[9 + 3], mov(7, 0x3F80_0000));
    }

    #[test]
    fn la_sph_dice_que_lee_memoria() {
        let (h, t2a) = (sph_vertice(), crate::color3d::sph_vertice());
        assert_eq!(h[0], t2a[0] | 1 << 26);
        assert_eq!(&h[1..], &t2a[1..]);
        assert_eq!(vertice()[0] & 1 << 26, 1 << 26);
        assert_eq!(pixel()[0] & 1 << 26, 0);
    }

    #[test]
    fn el_pixel() {
        let c = codigo_ps();
        // IPA R0..R3 desde a[0x80..0x8c].
        for k in 0..4u64 {
            assert_eq!(c[k as usize].0 >> 16 & 0xFF, k, "el destino R{k}");
            assert_eq!(c[k as usize].1 & 0x3FF, 0x20 + k, "el atributo a[0x{:x}]", 0x80 + 4 * k);
            assert_eq!(c[k as usize].0 & 0xFFF, 0x326, "IPA");
        }
        assert_eq!(c[4], crate::color3d::CODIGO_PS[4], "EXIT, esperando la barrera 0");
        assert_eq!(sph_pixel()[6], 0b1111_1111, "X, Y, Z y W del generico 0, lineales");
    }

    /// P3b4b: un paquete VRN1 -- 4 vertices, 6 indices, descarte de las
    /// traseras -- va y vuelve; un indice de un vertice que no esta, o unos
    /// indices que se salen de los datos, NO; y sus ordenes encienden el
    /// descarte y dibujan CON INDICES desde los datos.
    #[test]
    fn vrn1_indices_y_descarte() {
        let (vs, ps) = (programa_de(&vertice()), programa_de(&pixel()));
        let mut datos = std::vec![0u8; 64 + 32]; // enteros de 16: 6 indices y relleno
        for (k, i) in [0u32, 1, 2, 0, 2, 3].iter().enumerate() {
            datos[64 + 4 * k..68 + 4 * k].copy_from_slice(&i.to_le_bytes());
        }
        let d = Dibujo { indices: Some(64), vertices: 4, descarte: Descarte::Traseras, antihorario: false, destino: None, z: None, color: None, texturas: 0, cadena: false, escalera: false, pantalla: false, banco: 0, redondeo: crate::pegamento::Redondeo::NADA, mezcla: crate::mezcla::Mezcla::NINGUNA, registros: 0, mas: Default::default(), n_mas: 0 };
        let mut caja = std::vec![0u8; MAX_PAQUETE];
        let n = escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 6, &datos, d).unwrap();
        assert_eq!(medida(&caja[..CABECERA_MAX]), Some(n));
        let p = leer(&caja[..n]).unwrap();
        assert_eq!((p.n, p.dibujo, p.vertices.len()), (6, d, datos.len()));
        // Un indice fuera (el 4 con 4 vertices), no.
        let mut malo = datos.clone();
        malo[64..68].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 6, &malo, d), None);
        // Indices que se salen de los datos, o desalineados, no.
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 9, &datos, d), None);
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 6, &datos, Dibujo { indices: Some(62), ..d }), None);
        // Sin indices, no mas vertices de los que hay.
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 6, &datos, Dibujo { indices: None, ..d }), None);
        // Las ordenes.
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let o = ordenes_dibujo(&v, 2, true, d);
        let w = &o.o[..o.n];
        let metodo = |m: u32, k: u32| crate::copia::cabecera_en(0, m, k);
        let tras = |m: u32, k: u32| -> std::vec::Vec<u32> {
            let i = w.iter().rposition(|&x| x == metodo(m, k)).unwrap_or_else(|| panic!("falta el metodo 0x{m:04x}"));
            w[i + 1..i + 1 + k as usize].to_vec()
        };
        assert_eq!(tras(OGL_SET_FRONT_FACE, 1), [DELANTE_HORARIO]);
        assert_eq!(tras(OGL_SET_CULL_FACE, 1), [CULL_TRASERAS]);
        assert_eq!(tras(crate::raster::OGL_SET_CULL, 1), [1], "el ultimo: encendido (el estado lo pone a 0 antes)");
        let va = crate::vram::DATOS_VA + 64;
        assert_eq!(tras(SET_INDEX_BUFFER_A, 2), [(va >> 32) as u32, va as u32]);
        assert_eq!(tras(SET_INDEX_BUFFER_SIZE_A, 2), [0, 24]);
        assert_eq!(tras(SET_INDEX_BUFFER_E, 1), [INDICES_U32]);
        assert_eq!(tras(SET_INDEX_BUFFER_F, 2), [0, 6], "desde el 0, 6 indices");
        // Sin descarte ni indices, las de siempre.
        let (a, b) = (ordenes_con(&v, 2, true), ordenes_dibujo(&v, 2, true, Dibujo::default()));
        assert_eq!(&a.o[..a.n], &b.o[..b.n]);
    }

    /// ** Q0a1 (EL_FOCO, 10-10): un destino de 640x360 da ordenes de 640x360
    /// -- el alto del destino de color, el recorte de la superficie, el
    /// viewport (x * 320 + 320, y * -180 + 180) y su recorte --; y la
    /// pantalla, las de siempre (1280x720).
    #[test]
    fn q0a1_las_ordenes_tienen_la_medida_del_destino() {
        use crate::destino::Destino;
        use crate::tresde as td;
        let tras = |o: &cu::Ordenes, m: u32, n: u32| -> std::vec::Vec<u32> {
            let w = &o.o[..o.n];
            let i = w.iter().position(|&x| x == crate::copia::cabecera_en(0, m, n)).unwrap_or_else(|| panic!("el metodo {:#x}", m));
            w[i + 1..i + 1 + n as usize].to_vec()
        };
        let f = |x: f32| x.to_bits();
        let dst = Destino { fila: 640 * 4, ancho: 640, alto: 360, flotante: crate::destino::Flotante::No, rgb: false };
        let d = Dibujo { destino: Some((0x1234_5000, dst)), vertices: 3, ..Dibujo::default() };
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let o = ordenes_dibujo(&v, 1, false, d);
        assert_eq!(tras(&o, td::SET_COLOR_TARGET_A0, 8)[2..4], [2560, 360]);
        assert_eq!(tras(&o, td::SET_SURFACE_CLIP_HORIZONTAL, 2), [640 << 16, 360 << 16]);
        assert_eq!(tras(&o, crate::raster::SET_VIEWPORT_SCALE_X0, 7)[..6], [f(320.0), f(-180.0), f(1.0), f(320.0), f(180.0), f(0.0)]);
        assert_eq!(tras(&o, crate::raster::SET_VIEWPORT_CLIP_HORIZONTAL0, 4)[..2], [640 << 16, 360 << 16]);
        let o = ordenes_dibujo(&v, 1, false, Dibujo::default());
        assert_eq!(tras(&o, td::SET_SURFACE_CLIP_HORIZONTAL, 2), [1280 << 16, 720 << 16]);
        assert_eq!(tras(&o, crate::raster::SET_VIEWPORT_SCALE_X0, 7)[..2], [f(640.0), f(-360.0)]);
    }

    /// ** Q0a2 (EL_FOCO, 10-10): VRN1 con destino LLEVA la limpieza -- el
    /// pixel, tal como va en memoria -- y sus ordenes la mandan a la 3060
    /// (`SET_COLOR_CLEAR_VALUE0` y `CLEAR_SURFACE`) antes del dibujo. Sin
    /// destino, no; y un V0 con +20 = 1 sigue siendo un recorte que no vale.
    #[test]
    fn q0a2_vrn1_lleva_el_color_del_destino() {
        use crate::destino::Destino;
        use crate::tresde as td;
        let (vs, ps) = (programa_de(&vertice()), programa_de(&pixel()));
        let datos = std::vec![0u8; 96];
        let dst = Destino { fila: 640 * 4, ancho: 640, alto: 360, flotante: crate::destino::Flotante::No, rgb: false };
        let d = Dibujo { vertices: 3, destino: Some((0x1234_5000, dst)), color: Some(0xFF14_1E3C), ..Dibujo::default() };
        let mut caja = std::vec![0u8; MAX_PAQUETE];
        let n = escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, d).unwrap();
        let p = leer(&caja[..n]).unwrap();
        assert_eq!((p.dibujo, p.limpiar), (d, None), "el color vuelve, y no es un recorte");
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let o = ordenes_dibujo(&v, 1, false, p.dibujo);
        let w = &o.o[..o.n];
        // La ULTIMA: la del estudio la pone antes `hasta_el_dibujo_de`.
        let i = w.iter().rposition(|&x| x == crate::copia::cabecera_en(0, td::SET_COLOR_CLEAR_VALUE0, 4)).expect("la limpieza del destino");
        assert_eq!(&w[i + 1..i + 5], &color_de_limpieza(0xFF14_1E3C, false));
        assert!(w.contains(&crate::copia::cabecera_en(0, td::CLEAR_SURFACE, 1)));
        // Sin color, ni una limpieza del destino: lo de siempre.
        let o = ordenes_dibujo(&v, 1, false, Dibujo { color: None, ..d });
        assert!(!o.o[..o.n].contains(&crate::copia::cabecera_en(0, td::CLEAR_SURFACE, 1)));
        // Sin destino no hay que limpiar: no se escribe.
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, Dibujo { destino: None, ..d }), None);
        // Y a mano: la marca sin destino no se lee.
        let n = escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, Dibujo { destino: None, color: None, ..d }).unwrap();
        caja[20..24].copy_from_slice(&1u32.to_le_bytes());
        assert!(leer(&caja[..n]).is_none());
        // V0: +20 = 1 no es la marca (solo VRN1): es un recorte que no vale.
        let mut v0 = std::vec![0u8; MAX_PAQUETE];
        let m = escribir_paquete_datos(&mut v0, 7, &vs, &ps, 3, &datos).unwrap();
        v0[20..24].copy_from_slice(&1u32.to_le_bytes());
        assert!(leer(&v0[..m]).is_none());
    }

    /// ** Q0a2: los vertices de la LAMINA, directo al VRN1 (sin bufer): los
    /// mismos bytes que el VRN1 de unos datos escritos aparte, y con el
    /// destino y su color.
    #[test]
    fn q0a2_los_vertices_de_v0_directo_al_vrn1() {
        use crate::destino::Destino;
        let (vs, ps) = (programa_de(&vertice()), programa_de(&pixel()));
        let vertices: std::vec::Vec<Vertice> = (0..6u32).map(|k| Vertice { posicion: [k, k + 1, k + 2, 0x3F80_0000], color: [k * 3, 0, 0, 0x3F80_0000] }).collect();
        let mut datos = std::vec::Vec::new();
        for v in &vertices {
            for w in v.palabras() {
                datos.extend_from_slice(&w.to_le_bytes());
            }
        }
        let dst = Destino { fila: 640 * 4, ancho: 640, alto: 360, flotante: crate::destino::Flotante::No, rgb: false };
        let d = Dibujo { vertices: 6, destino: Some((0x7_0000_0000, dst)), color: Some(0x0010_1018), ..Dibujo::default() };
        let (mut a, mut b) = (std::vec![0u8; MAX_PAQUETE], std::vec![0u8; MAX_PAQUETE]);
        let n = escribir_paquete_dibujo_de(&mut a, 7, &vs, &ps, vertices.iter().copied(), d).unwrap();
        let m = escribir_paquete_dibujo(&mut b, 7, &vs, &ps, 6, &datos, d).unwrap();
        assert_eq!(a[..n], b[..m]);
        assert_eq!(leer(&a[..n]).unwrap().dibujo, d);
        // Con indices, o con otra cuenta de vertices, no: son los de V0.
        assert_eq!(escribir_paquete_dibujo_de(&mut a, 7, &vs, &ps, vertices.iter().copied(), Dibujo { indices: Some(0), ..d }), None);
        assert_eq!(escribir_paquete_dibujo_de(&mut a, 7, &vs, &ps, vertices.iter().copied(), Dibujo { vertices: 5, ..d }), None);
    }

    /// P3b4b (3): con DESTINO, el paquete lleva la RAM de la app; sus ordenes
    /// ponen el destino de color en `destino::VA` con su fila, y NO limpian
    /// (el juego limpia su back buffer). Un destino que no vale, no pasa.
    #[test]
    fn vrn1_con_destino_en_la_ram_de_la_app() {
        use crate::destino::{Destino, VA as DESTINO_VA};
        use crate::tresde as td;
        let (vs, ps) = (programa_de(&vertice()), programa_de(&pixel()));
        let datos = std::vec![0u8; 96];
        let dst = Destino { fila: 1280 * 4, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: false };
        let d = Dibujo { indices: None, vertices: 3, descarte: Descarte::Ninguna, antihorario: false, destino: Some((0x1234_5000, dst)), z: None, color: None, texturas: 0, cadena: false, escalera: false, pantalla: false, banco: 0, redondeo: crate::pegamento::Redondeo::NADA, mezcla: crate::mezcla::Mezcla::NINGUNA, registros: 0, mas: Default::default(), n_mas: 0 };
        let mut caja = std::vec![0u8; MAX_PAQUETE];
        let n = escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, d).unwrap();
        assert_eq!(leer(&caja[..n]).unwrap().dibujo, d);
        for malo in [Destino { ancho: 0, ..dst }, Destino { fila: 5124, ..dst }] {
            assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, Dibujo { destino: Some((0x1234_5000, malo)), ..d }), None);
        }
        // ** Q0a1: otra medida, SI (sin Z); con Z, solo la de VERRANO.
        let otra = Destino { ancho: 640, alto: 360, ..dst };
        let con_otra = Dibujo { destino: Some((0x1234_5000, otra)), ..d };
        let n2 = escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, con_otra).unwrap();
        assert_eq!(leer(&caja[..n2]).unwrap().dibujo, con_otra);
        let z = Some(crate::profundidad::Z { funcion: 2, escribir: true, limpiar: Some(crate::profundidad::UNO) });
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, Dibujo { z, ..con_otra }), None, "Z en otra medida, no");
        assert!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, Dibujo { z, ..d }).is_some(), "Z en la de VERRANO, si");
        assert_eq!(escribir_paquete_dibujo(&mut caja, 7, &vs, &ps, 3, &datos, Dibujo { destino: Some((0x1234_5010, dst)), ..d }), None, "sin alinear a pagina");
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let o = ordenes_dibujo(&v, 1, true, d);
        let w = &o.o[..o.n];
        let i = w.iter().position(|&x| x == crate::copia::cabecera_en(0, td::SET_COLOR_TARGET_A0, 8)).unwrap();
        assert_eq!(&w[i + 1..i + 4], &[(DESTINO_VA >> 32) as u32, DESTINO_VA as u32, 1280 * 4]);
        assert!(!w.contains(&crate::copia::cabecera_en(0, td::CLEAR_SURFACE, 1)), "no limpia");
        // Sin destino, la pantalla y limpiando, como siempre.
        let o = ordenes_dibujo(&v, 1, true, Dibujo::default());
        assert!(o.o[..o.n].contains(&crate::copia::cabecera_en(0, td::CLEAR_SURFACE, 1)));
    }

    fn programa_de<const N: usize>(p: &[u32; N]) -> std::vec::Vec<u8> {
        p.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn las_ordenes() {
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let o = ordenes(&v, 6);
        let w = &o.o[..o.n];
        // UN dibujo con el rasterizador, de 18 vertices, y el de 3 sin el.
        let starts: std::vec::Vec<u32> = w.windows(3).filter(|q| q[0] == crate::copia::cabecera_en(0, crate::raster::SET_VERTEX_ARRAY_START, 2)).map(|q| q[2]).collect();
        assert_eq!(starts, [3, 18]);
        // Con UN triangulo las ordenes son EXACTAMENTE las de X5 salvo los
        // REGISTROS de cada programa (E5: 64, no 16) y, desde LI2b, DONDE
        // esta cada programa (la palabra baja de su VA: `vram::CODIGO`).
        let (x5, uno) = (crate::cubo::ordenes(&v, 1), ordenes(&v, 1));
        assert_eq!(uno.n, x5.n);
        let distintas: std::vec::Vec<usize> = (0..x5.n).filter(|&i| uno.o[i] != x5.o[i]).collect();
        assert_eq!(distintas.len(), 4, "los dos REGISTER_COUNT y las dos direcciones");
        let mut vistas = std::vec::Vec::new();
        for i in distintas {
            vistas.push((x5.o[i], uno.o[i]));
        }
        let va = |d: u64| crate::lienzo::sombreador_va(d) as u32;
        assert_eq!(vistas, [(va(crate::cubo::vs(0)), VS_VA as u32), (crate::raster::REGISTROS, REGISTROS), (va(crate::cubo::ps(0)), PS_VA as u32), (crate::raster::REGISTROS, REGISTROS)]);
        assert_eq!((VS_VA >> 32, PS_VA >> 32), (crate::lienzo::sombreador_va(crate::cubo::vs(0)) >> 32, 2), "la palabra alta, la misma");
    }

    /// V1 `ligero`: el MISMO estado y el MISMO dibujo, sin la escalera --
    /// dos WAIT_FOR_IDLE (tras limpiar y al cerrar), UN semaforo (el del
    /// final) y un solo dibujo, el de todos los vertices.
    #[test]
    fn ligero_es_la_escalera_sin_escalones() {
        use crate::copia::cabecera_en;
        use crate::tresde as td;
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let (con, sin) = (ordenes_con(&v, 12, false), ordenes_con(&v, 12, true));
        let (con, sin) = (&con.o[..con.n], &sin.o[..sin.n]);
        let cuenta = |w: &[u32], m: u32, n: u32| w.iter().filter(|&&x| x == cabecera_en(0, m, n)).count();
        assert_eq!(cuenta(sin, td::WAIT_FOR_IDLE, 1), 2);
        assert_eq!(cuenta(sin, td::SET_REPORT_SEMAPHORE_A, 4), 1);
        assert!(cuenta(con, td::WAIT_FOR_IDLE, 1) > 20, "la escalera espera tras cada metodo");
        let starts: std::vec::Vec<u32> = sin.windows(3).filter(|q| q[0] == cabecera_en(0, crate::raster::SET_VERTEX_ARRAY_START, 2)).map(|q| q[2]).collect();
        assert_eq!(starts, [36]);
        // Lo de `ligero` es lo de la escalera en el mismo orden: quitando de
        // la escalera sus escalones, sale una secuencia que CONTIENE a ligero.
        let mut i = 0;
        for &x in sin {
            while i < con.len() && con[i] != x {
                i += 1;
            }
            assert!(i < con.len(), "ligero trae una palabra que la escalera no: {x:#x}");
            i += 1;
        }
        // Y el numero de registros que se le da a cada programa, el mismo.
        let regs = |w: &[u32]| w.windows(2).filter(|q| q[0] == cabecera_en(0, crate::raster::set_pipeline_shader(1) + 0x0c, 2)).map(|q| q[1]).collect::<std::vec::Vec<u32>>();
        assert_eq!(regs(sin), regs(con));
    }

    /// La huella de lo fijo: cambia con los programas y con `ligero`, y NO
    /// con los vertices ni con cuantos son (lo que el caliente vuelve a
    /// escribir: los datos y las ordenes).
    #[test]
    fn la_huella_de_lo_fijo() {
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let mut vs = [0u8; 4 * PALABRAS_VS];
        let mut ps = [0u8; 4 * PALABRAS_PS];
        bytes(&vertice(), &mut vs);
        bytes(&pixel(), &mut ps);
        let (a, b) = ([7u8; 6 * BYTES_VERTICE], [9u8; 6 * BYTES_VERTICE]);
        let p = |x: &'static [u8], vv: &'static [u8], pp: &'static [u8]| Paquete { ficha: 1, vs: vv, ps: pp, vertices: x, n: x.len() / BYTES_VERTICE, limpiar: None, dibujo: crate::tuberia::Dibujo::default() };
        let (vs, ps): (&'static [u8], &'static [u8]) = (std::boxed::Box::leak(std::boxed::Box::new(vs)), std::boxed::Box::leak(std::boxed::Box::new(ps)));
        let (a, b): (&'static [u8], &'static [u8]) = (std::boxed::Box::leak(std::boxed::Box::new(a)), std::boxed::Box::leak(std::boxed::Box::new(b)));
        let h = huella_fija(&v, &p(a, vs, ps), false);
        assert_eq!(h, huella_fija(&v, &p(b, vs, ps), false), "los vertices no son lo fijo");
        assert_ne!(h, huella_fija(&v, &p(a, vs, ps), true), "ligero cambia las ordenes");
        assert_ne!(h, huella_fija(&v, &p(a, ps, vs), false), "otros programas");
        assert_eq!(h, huella_fija(&v, &p(&a[..3 * BYTES_VERTICE], vs, ps), false), "otra cuenta de triangulos: el caliente reescribe las ordenes");
        // El destino: sus medidas SI (otras ordenes), su direccion NO (los
        // dos back buffers de un juego son la misma receta caliente).
        let dst = crate::destino::Destino { fila: 5120, ancho: 1280, alto: 720, flotante: crate::destino::Flotante::No, rgb: true };
        let con = |va: u64, d: crate::destino::Destino| Paquete { dibujo: crate::tuberia::Dibujo { destino: Some((va, d)), ..Default::default() }, ..p(a, vs, ps) };
        let hd = huella_fija(&v, &con(0x1000_0000, dst), false);
        assert_ne!(h, hd, "con destino no es sin destino");
        assert_eq!(hd, huella_fija(&v, &con(0x2000_0000, dst), false), "el otro back buffer, la misma huella");
        assert_ne!(hd, huella_fija(&v, &con(0x1000_0000, crate::destino::Destino { alto: 719, ..dst }), false), "otras medidas, otras ordenes");
    }

    #[test]
    fn el_paquete() {
        let mut vs = [0u8; 4 * PALABRAS_VS];
        let mut ps = [0u8; 4 * PALABRAS_PS];
        bytes(&vertice(), &mut vs);
        bytes(&pixel(), &mut ps);
        let v = [Vertice { posicion: [1, 2, 3, 4], color: [5, 6, 7, 8] }; 6];
        let mut b = [0u8; 2048];
        let n = escribir_paquete(&mut b, 0xF1C4, &vs, &ps, &v).unwrap();
        assert_eq!(medida(&b[..CABECERA]), Some(n));
        let p = leer(&b[..n]).unwrap();
        assert_eq!((p.ficha, p.vs, p.ps, p.vertices.len()), (0xF1C4, &vs[..], &ps[..], 6 * BYTES_VERTICE));
        assert_eq!(p.limpiar, None, "sin recorte: la ventana entera");
        // El de vertice en el hueco del de pixel no pasa, ni 4 vertices.
        let n2 = escribir_paquete(&mut b, 1, &ps, &vs, &v).unwrap_or(0);
        assert!(n2 == 0 || leer(&b[..n2]).is_none());
        assert!(escribir_paquete(&mut b, 1, &vs, &ps, &v[..4]).is_none());
        // V1c: el recorte viaja en la cabecera, y uno que no cabe no pasa.
        let r = (100 | 500 << 16, 20 | 400 << 16);
        let n = escribir_paquete_con(&mut b, 1, &vs, &ps, &v, Some(r)).unwrap();
        assert_eq!(leer(&b[..n]).unwrap().limpiar, Some(r));
        assert!(escribir_paquete_con(&mut b, 1, &vs, &ps, &v, Some((500 | 100 << 16, 20 | 400 << 16))).is_none(), "al reves");
        assert!(escribir_paquete_con(&mut b, 1, &vs, &ps, &v, Some((0 | 1281 << 16, 0 | 720 << 16))).is_none(), "fuera");
    }

    #[test]
    fn el_vertice_en_memoria() {
        let x = Vertice { posicion: [1, 2, 3, 4], color: [5, 6, 7, 8] };
        assert_eq!(x.palabras(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
