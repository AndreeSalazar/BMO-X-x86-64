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
use crate::cubo::{self as cu, con_control, mov, Ventana, ALU, CABEN, SEMAFORO_FIN};
use crate::lienzo::sombreador_va;
use crate::raster::{programa, ESCALONES, N_ESCALONES, SPH, SUSTITUTO};
use crate::sombreador::{EMPUJE, PROGRAMA, SALIDA, SEMAFOROS};
use crate::vram::a_cero;
use crate::Registros;

/// La tabla de buffers (`SM86_V1`): la ranura 0, la de los vertices.
pub const TABLA: u64 = SEMAFOROS + 0x600;
/// Los vertices: 8 floats cada uno (la posicion y el color), 32 B.
pub const VERTICES: u64 = SEMAFOROS + 0x800;
pub const BYTES_VERTICE: usize = 32;
pub const MAX_VERTICES: usize = 3 * CABEN;
/// Donde van los dos programas: el principio de las paginas de X5.
pub const VS: u64 = SALIDA;
pub const PS: u64 = PROGRAMA;

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
/// ventana entera) y ceros hasta 32.
pub const CABECERA: usize = 32;

/// **El paquete**: los dos programas (tomados del BSF por la app, con sus
/// hashes comprobados) y los vertices, tal como llegan del escritorio.
#[derive(Clone, Copy, Debug)]
pub struct Paquete<'a> {
    pub ficha: u32,
    pub vs: &'a [u8],
    pub ps: &'a [u8],
    pub vertices: &'a [u8],
    /// V1c: lo que HACE FALTA limpiar, si quien dibuja lo sabe --
    /// `(xmin | xmax << 16, ymin | ymax << 16)` en pixeles de la ventana, el
    /// maximo fuera (`SET_CLEAR_RECT_*`). `None` = la ventana entera. Solo lo
    /// usa el anillo en modo `coopera`; los demas limpian siempre todo.
    pub limpiar: Option<(u32, u32)>,
}

/// Un recorte que cabe en la ventana y no es vacio.
pub const fn recorte_valido(h: u32, v: u32) -> bool {
    let (x0, x1, y0, y1) = (h & 0xFFFF, h >> 16, v & 0xFFFF, v >> 16);
    x0 < x1 && x1 <= cu::ANCHO && y0 < y1 && y1 <= cu::ALTO
}

fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Un programa que el kernel acepta subir: la SPH del tipo que toca y
/// cabe en su hueco.
fn programa_valido(p: &[u8], tipo: u32, hueco: u64) -> bool {
    p.len() >= 4 * SPH + 16 && p.len() % 16 == 0 && p.len() as u64 <= hueco && u32le(p, 0) & 0x1F == if tipo == crate::raster::VERTICE { 1 } else { 2 } && u32le(p, 0) >> 10 & 0xF == tipo
}

/// Cuanto mide el paquete que dice esta cabecera (o `None` si no lo es).
pub fn medida(cabecera: &[u8]) -> Option<usize> {
    if cabecera.len() < CABECERA || u32le(cabecera, 0) != MAGIA || cabecera[28..CABECERA].iter().any(|&b| b != 0) {
        return None;
    }
    let (h, v) = (u32le(cabecera, 20), u32le(cabecera, 24));
    if (h, v) != (0, 0) && !recorte_valido(h, v) {
        return None;
    }
    let (n, vs, ps) = (u32le(cabecera, 8) as usize, u32le(cabecera, 12) as usize, u32le(cabecera, 16) as usize);
    if n == 0 || n % 3 != 0 || n > MAX_VERTICES || vs > cu::PASO_VS as usize || ps > cu::PASO_PS as usize {
        return None;
    }
    Some(CABECERA + vs + ps + n * BYTES_VERTICE)
}

/// **Leer** un paquete entero (ya medido).
pub fn leer(b: &[u8]) -> Option<Paquete<'_>> {
    let total = medida(b)?;
    if b.len() != total {
        return None;
    }
    let (vs, ps) = (u32le(b, 12) as usize, u32le(b, 16) as usize);
    let (h, v) = (u32le(b, 20), u32le(b, 24));
    let limpiar = ((h, v) != (0, 0)).then_some((h, v));
    let p = Paquete { ficha: u32le(b, 4), vs: &b[CABECERA..CABECERA + vs], ps: &b[CABECERA + vs..CABECERA + vs + ps], vertices: &b[CABECERA + vs + ps..], limpiar };
    (programa_valido(p.vs, crate::raster::VERTICE, cu::PASO_VS) && programa_valido(p.ps, crate::raster::PIXEL, cu::PASO_PS)).then_some(p)
}

/// **Escribir** un paquete en `out`; devuelve cuanto mide.
pub fn escribir_paquete(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], vertices: &[Vertice]) -> Option<usize> {
    escribir_paquete_con(out, ficha, vs, ps, vertices, None)
}

/// Lo mismo, con el recorte de la limpieza (V1c).
pub fn escribir_paquete_con(out: &mut [u8], ficha: u32, vs: &[u8], ps: &[u8], vertices: &[Vertice], limpiar: Option<(u32, u32)>) -> Option<usize> {
    let total = CABECERA + vs.len() + ps.len() + vertices.len() * BYTES_VERTICE;
    if out.len() < total {
        return None;
    }
    out[..CABECERA].fill(0);
    let (h, v) = limpiar.unwrap_or((0, 0));
    for (k, v) in [MAGIA, ficha, vertices.len() as u32, vs.len() as u32, ps.len() as u32, h, v].iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
    }
    out[CABECERA..CABECERA + vs.len()].copy_from_slice(vs);
    out[CABECERA + vs.len()..CABECERA + vs.len() + ps.len()].copy_from_slice(ps);
    let mut i = CABECERA + vs.len() + ps.len();
    for v in vertices {
        for w in v.palabras() {
            out[i..i + 4].copy_from_slice(&w.to_le_bytes());
            i += 4;
        }
    }
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

// == Las ordenes y el preparar ===============================================

/// **Las ordenes**: las de X5 hasta el dibujo, y UN dibujo de `3n` vertices.
pub fn ordenes(v: &Ventana, n: usize) -> cu::Ordenes {
    ordenes_con(v, n, false)
}

/// Las mismas, `ligero` = SIN la escalera de T1c (ver `cubo::Ordenes`).
pub fn ordenes_con(v: &Ventana, n: usize, ligero: bool) -> cu::Ordenes {
    let mut e = cu::hasta_el_dibujo_con(v, !ligero);
    e.dibujo_de(3 * n as u32);
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
    let n = p.vertices.len() / BYTES_VERTICE;
    if !crate::blur::entrada_valida(e) || n == 0 || n % 3 != 0 || n > MAX_VERTICES {
        return false;
    }
    let o = ordenes_con(v, n / 3, ligero);
    let en = entrada(sombreador_va(EMPUJE), o.n as u32);
    let va = sombreador_va(VERTICES);
    escribir(r, SEMAFORO_FIN, &[0; 4]) == 4
        && escribir(r, ESCALONES, &[0; N_ESCALONES as usize]) == N_ESCALONES as usize
        && a_cero(r, SALIDA) as usize == crate::vram::PALABRAS
        && a_cero(r, PROGRAMA) as usize == crate::vram::PALABRAS
        && a_cero(r, SUSTITUTO) as usize == crate::vram::PALABRAS
        && escribir_bytes(r, VS, p.vs)
        && escribir_bytes(r, PS, p.ps)
        && escribir(r, TABLA, &[va as u32, (va >> 32) as u32]) == 2
        && escribir_bytes(r, VERTICES, p.vertices)
        && escribir(r, EMPUJE, &o.o[..o.n]) == o.n && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
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

/// **La huella de lo FIJO** de un dibujo de VERRANO: los dos programas, los
/// triangulos, la ventana y si va `ligero` -- todo lo que `preparar` escribe
/// y el caliente NO vuelve a escribir. FNV-1a de 64 bits.
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
    for x in [p.vertices.len() as u64, v.x0 as u64, v.y0 as u64, v.va, v.fila as u64, v.rgb as u64, ligero as u64] {
        mezclar(&x.to_le_bytes());
    }
    h
}

/// **Preparar EN CALIENTE**: lo que cambia, sin releer. Los programas, la
/// tabla y las ordenes tienen que estar ya donde los dejo un `preparar_con`
/// con la misma [`huella_fija`] (eso lo comprueba el kernel, no esto).
pub fn preparar_caliente<R: Registros>(r: &mut R, e: u32, v: &Ventana, p: &Paquete, ligero: bool) -> bool {
    let n = p.vertices.len() / BYTES_VERTICE;
    if !crate::blur::entrada_valida(e) || n == 0 || n % 3 != 0 || n > MAX_VERTICES {
        return false;
    }
    let o = ordenes_con(v, n / 3, ligero);
    let en = entrada(sombreador_va(EMPUJE), o.n as u32);
    let vol = crate::volcado::escribir_sin_releer;
    vol(r, SEMAFORO_FIN, &[0; 4]);
    if !ligero {
        vol(r, ESCALONES, &[0; N_ESCALONES as usize]);
    }
    let mut w = [0u32; 64];
    for (k, trozo) in p.vertices.chunks(256).enumerate() {
        let m = trozo.len() / 4;
        for (i, q) in trozo.chunks_exact(4).enumerate() {
            w[i] = u32le(q, 0);
        }
        vol(r, VERTICES + 256 * k as u64, &w[..m]);
    }
    vol(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]);
    true
}

const _: () = assert!(PALABRAS_VS * 4 <= cu::PASO_VS as usize && PALABRAS_PS * 4 <= cu::PASO_PS as usize);
const _: () = assert!(VS == cu::vs(0) && PS == cu::ps(0));
const _: () = assert!(TABLA >= crate::video::PARAMETROS + 4 * crate::video::N_PARAMETROS as u64);
const _: () = assert!(VERTICES >= TABLA + 8 && VERTICES + (MAX_VERTICES * BYTES_VERTICE) as u64 <= SEMAFOROS + 0x1000);

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

    #[test]
    fn las_ordenes() {
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let o = ordenes(&v, 6);
        let w = &o.o[..o.n];
        // UN dibujo con el rasterizador, de 18 vertices, y el de 3 sin el.
        let starts: std::vec::Vec<u32> = w.windows(3).filter(|q| q[0] == crate::copia::cabecera_en(0, crate::raster::SET_VERTEX_ARRAY_START, 2)).map(|q| q[2]).collect();
        assert_eq!(starts, [3, 18]);
        // Con UN triangulo las ordenes son EXACTAMENTE las de X5: lo unico
        // que cambia son los programas que hay en las dos paginas.
        let (x5, uno) = (crate::cubo::ordenes(&v, 1), ordenes(&v, 1));
        assert_eq!(&uno.o[..uno.n], &x5.o[..x5.n]);
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
    /// con los vertices (lo unico que el caliente vuelve a escribir).
    #[test]
    fn la_huella_de_lo_fijo() {
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        let v = crate::cubo::ventana(&gop).unwrap();
        let mut vs = [0u8; 4 * PALABRAS_VS];
        let mut ps = [0u8; 4 * PALABRAS_PS];
        bytes(&vertice(), &mut vs);
        bytes(&pixel(), &mut ps);
        let (a, b) = ([7u8; 6 * BYTES_VERTICE], [9u8; 6 * BYTES_VERTICE]);
        let p = |x: &'static [u8], vv: &'static [u8], pp: &'static [u8]| Paquete { ficha: 1, vs: vv, ps: pp, vertices: x, limpiar: None };
        let (vs, ps): (&'static [u8], &'static [u8]) = (std::boxed::Box::leak(std::boxed::Box::new(vs)), std::boxed::Box::leak(std::boxed::Box::new(ps)));
        let (a, b): (&'static [u8], &'static [u8]) = (std::boxed::Box::leak(std::boxed::Box::new(a)), std::boxed::Box::leak(std::boxed::Box::new(b)));
        let h = huella_fija(&v, &p(a, vs, ps), false);
        assert_eq!(h, huella_fija(&v, &p(b, vs, ps), false), "los vertices no son lo fijo");
        assert_ne!(h, huella_fija(&v, &p(a, vs, ps), true), "ligero cambia las ordenes");
        assert_ne!(h, huella_fija(&v, &p(a, ps, vs), false), "otros programas");
        assert_ne!(h, huella_fija(&v, &p(&a[..3 * BYTES_VERTICE], vs, ps), false), "otros triangulos, otras ordenes");
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
