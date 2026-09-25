//! **M5d P: LA 3060 PINTA TU PANTALLA ENTERA** -- sin la CPU de por medio. Un
//! programa de computo escribe CADA pixel de la pantalla, a su resolucion,
//! DIRECTAMENTE en la memoria que el escaner de la 3060 manda al monitor: el
//! framebuffer del GOP, que vive en su VRAM. Un fractal de Mandelbrot que se
//! acerca y se aleja del valle de los caballitos de mar, fotograma a
//! fotograma, con la paleta girando.
//!
//! capa: puro -- el mapa, el programa, los parametros de cada fotograma y la
//! cuenta de referencia; la VRAM y los registros los toca el kernel (L8)
//!
//! [eje]     RENDIMIENTO -- lo que antes era: la 3060 dibuja en la RAM del PC,
//!           la CPU lo lee y lo copia (y lo escala) a la pantalla. Ahora la
//!           CPU no mueve ni un pixel: la 3060 escribe donde mira el monitor
//!
//! # Donde esta la pantalla (el metal, 24-09 10:13)
//!
//! El GOP deja BAR1 en modo FISICO (`BAR1_BLOCK` = 0x002FFF00, el bit 31 a
//! 0), y `gpu init` se lo devuelve tras el GSP-RM: la direccion `d` de BAR1
//! es la `d` de la VRAM. Asi que el framebuffer del GOP esta en la VRAM en
//! `fb - BAR1`, en lo bajo (por debajo de los 49 MiB que el GSP-RM da como
//! usables). Se mapea en NUESTRO espacio, en su propia VA:
//!
//! ```text
//!    VA     0x4_0000_0000 (16 GiB: la PD1 del tramo, entrada 32)
//!    tablas 0x0440_0000: una PD0 y hasta 16 PT (32 MiB: hasta 3840x2160)
//!    PTE    VRAM, sin PRIV, kind 0 (PITCH: lineal, como la ve el escaner)
//! ```
//!
//! # El programa (PTX de origen, `ptxas -arch=sm_86`: 19 registros)
//!
//! ```text
//!    parametros  base (64 bits), pitch, ancho, alto, cr0, ci0, paso,
//!                desplaza, rgb -- en VA 0x2_0000_E500, leidos con STRONG.SYS
//!    rejilla     (ancho / 256) x alto bloques de 256 hilos: uno por pixel
//!    x, y        ctaid.x * 256 + tid.x (fuera si x >= ancho), ctaid.y
//!    c           (cr0 + x * paso, ci0 + y * paso), Q4.28 -- como `fractal`
//!    color       el de `fractal` con la paleta desplazada; negro si no escapa;
//!                rojo y azul cambiados si el GOP es RGB
//!    STG         base + (y * pitch + x) * 4
//! ```
//!
//! Como todos: `c[0x0][0x28]` y `ULDC.64 UR4, c[0x0][0x118]` en NOP, y los
//! LDG/STG sin descriptor (el bit 101).
//!
//! # Como se sabe
//!
//! La CPU rehace la cuenta de [`MUESTRAS`] pixeles repartidos por la pantalla
//! y los lee de vuelta del framebuffer: si salen iguales, la 3060 escribio
//! DONDE MIRA EL MONITOR (lo que la CPU lee por BAR1 es lo que se ve).

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::mmu::{indices, pde_vram, pte_vram};
use crate::sombreador::{ordenes_con, qmd_rejilla, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

/// Donde ve la GPU la pantalla.
pub const VA: u64 = 0x4_0000_0000;
/// La PD0 y las PT de la pantalla, en VRAM (tras las de GR, 0x0430_0000).
pub const TABLAS: u64 = 0x0440_0000;
/// Cuantas PT caben: 16 x 2 MiB.
pub const PTS: usize = 16;
pub const MAX_BYTES: u64 = PTS as u64 * (2 << 20);
/// Lo mas alto que puede estar el framebuffer en la VRAM: por debajo de la
/// prueba de L1c2 (64 MiB) y de todo lo nuestro.
pub const TECHO_VRAM: u64 = crate::vram::PRUEBA;

pub const HILOS: u32 = 256;
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_FA10;
pub const PAGA_FIN: u32 = 0x3060_FAF0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x120;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x130;
/// Donde lee el programa sus 10 parametros (VA 0x2_0000_E500).
pub const PARAMETROS: u64 = SEMAFOROS + 0x500;
pub const N_PARAMETROS: usize = 10;

/// Las vueltas de Mandelbrot, como `fractal`.
pub const VUELTAS: u32 = 256;
/// El valle de los caballitos de mar (-0.743643887, 0.131825904), Q4.28.
pub const CENTRO: (i32, i32) = (-199_620_386, 35_386_747);
/// Un ciclo: 100 fotogramas acercandose y 100 alejandose.
pub const CICLO: u32 = 200;
pub const ACERCA: u32 = CICLO / 2;
/// Pixeles que la CPU comprueba en cada fotograma (una rejilla de 32 x 32).
pub const MUESTRAS: u32 = 1024;

/// **La pantalla**, tal como la dejo el GOP.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pantalla {
    /// Donde empieza el framebuffer EN LA VRAM (`fb - BAR1`).
    pub vram: u64,
    /// Pixeles por linea en memoria (>= ancho).
    pub pitch: u32,
    pub ancho: u32,
    pub alto: u32,
    /// El GOP es RGB (el rojo en el byte 0), no BGR.
    pub rgb: bool,
}

impl Pantalla {
    pub const fn bytes(&self) -> u64 {
        self.pitch as u64 * self.alto as u64 * 4
    }

    /// Si se puede mapear sin pisar nada nuestro.
    pub const fn valida(&self) -> bool {
        self.ancho > 0
            && self.alto > 0
            && self.alto <= 0xFFFF
            && self.ancho <= self.pitch
            && self.vram % 4096 == 0
            && self.bytes() <= MAX_BYTES
            && self.vram + self.bytes() <= TECHO_VRAM
    }

    /// Bloques por linea.
    pub const fn bloques(&self) -> u32 {
        self.ancho.div_ceil(HILOS)
    }
}

/// La pantalla en la VRAM desde lo que dice el GOP y BAR1 (`None` si el
/// framebuffer no esta dentro de BAR1, o no se puede mapear).
pub fn desde_gop(fb: u64, bar1: u64, pitch: u32, ancho: u32, alto: u32, rgb: bool) -> Option<Pantalla> {
    let vram = fb.checked_sub(bar1)?;
    let p = Pantalla { vram, pitch, ancho, alto, rgb };
    p.valida().then_some(p)
}

/// La entrada de la PD1 del tramo que cuelga [`VA`] (el tramo usa la 16; GR, la 24).
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// **Mapear la pantalla** en [`VA`]: la PD0 y las PT a cero, las PTE, las PDE
/// de la PD0 y al final la de la PD1 (de la hoja a la raiz), todo RELEIDO.
/// Solo si esa entrada de la PD1 esta VACIA o ya es la nuestra (volver a
/// mapear la MISMA pantalla no cambia nada). `(escrituras, releidas iguales)`.
pub fn mapear<R: Registros>(r: &mut R, p: &Pantalla) -> Option<(u32, u32)> {
    if !p.valida() || VA % (2 << 20) != 0 {
        return None;
    }
    let pd1 = leer64(r, entrada_pd1());
    if pd1 != 0 && pd1 != pde_vram(TABLAS) {
        return None;
    }
    let paginas = p.bytes().div_ceil(0x1000);
    let pts = paginas.div_ceil(512) as usize;
    let pt = |k: usize| TABLAS + 0x1000 * (1 + k as u64);
    if pd1 == 0 {
        for k in 0..=pts {
            a_cero(r, TABLAS + 0x1000 * k as u64);
        }
    }
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    for q in 0..paginas {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_vram(p.vram + q * 0x1000));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..pts {
        // La mitad de 4 KiB de la PD0 es la segunda (+8), como el tramo.
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

/// Lo que cambia de un fotograma a otro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Marco {
    pub cr0: i32,
    pub ci0: i32,
    pub paso: i32,
    pub desplaza: u32,
}

/// **El fotograma `f`** de una pantalla de `ancho` x `alto`: 3,5 de ancho al
/// empezar, y cada fotograma el paso x 243/256 (hasta ~180 veces mas cerca
/// en la mitad del ciclo); la paleta avanza 2 por fotograma. Todo entero.
pub fn marco(f: u32, ancho: u32, alto: u32) -> Marco {
    let k = f % CICLO;
    let k = if k < ACERCA { k } else { CICLO - k };
    let mut paso = (939_524_096 / ancho.max(1) as i64).max(1);
    for _ in 0..k {
        paso = (paso * 243 / 256).max(1);
    }
    let cr0 = CENTRO.0 as i64 - paso * (ancho as i64 / 2);
    let ci0 = CENTRO.1 as i64 - paso * (alto as i64 / 2);
    Marco { cr0: cr0 as i32, ci0: ci0 as i32, paso: paso as i32, desplaza: (f.wrapping_mul(2)) & 0xFF }
}

/// Los 10 parametros, en el orden en que los lee el programa.
pub fn parametros(p: &Pantalla, m: &Marco) -> [u32; N_PARAMETROS] {
    [VA as u32, (VA >> 32) as u32, p.pitch, p.ancho, p.alto, m.cr0 as u32, m.ci0 as u32, m.paso as u32, m.desplaza, p.rgb as u32]
}

/// Las vueltas de `c`: la MISMA cuenta que el programa (y que `fractal`).
pub fn vueltas(cr: i32, ci: i32) -> u32 {
    let (mut zr, mut zi, mut n) = (0i32, 0i32, 0u32);
    loop {
        let (r2, i2) = (zr as i64 * zr as i64, zi as i64 * zi as i64);
        if r2 + i2 > crate::fractal::CUATRO {
            return n;
        }
        let nzi = ((zr as i64 * zi as i64) >> 27) as i32;
        zi = nzi.wrapping_add(ci);
        zr = ((r2 >> 28) as i32).wrapping_sub((i2 >> 28) as i32).wrapping_add(cr);
        n += 1;
        if n >= VUELTAS {
            return n;
        }
    }
}

/// El color de `n` vueltas con la paleta desplazada `d`, tal como lo escribe
/// el programa en la pantalla (`rgb`: rojo y azul cambiados).
pub const fn color(n: u32, d: u32, rgb: bool) -> u32 {
    if n == VUELTAS {
        return 0;
    }
    let m = n.wrapping_add(d);
    let r = (m << 1) & 0xFF;
    let g = if (m << 3) & 0x1FF > 0xFF { 0xFF } else { (m << 3) & 0x1FF };
    let b = (m << 4) & 0xFF;
    let (r, b) = if rgb { (b, r) } else { (r, b) };
    r << 16 | g << 8 | b
}

/// Lo que tiene que haber en `(x, y)`.
pub fn pixel(p: &Pantalla, m: &Marco, x: u32, y: u32) -> u32 {
    let cr = m.cr0.wrapping_add((x as i32).wrapping_mul(m.paso));
    let ci = m.ci0.wrapping_add((y as i32).wrapping_mul(m.paso));
    color(vueltas(cr, ci), m.desplaza, p.rgb)
}

/// La muestra `k` (de [`MUESTRAS`]): una rejilla de 32 x 32 que toca los
/// cuatro bordes.
pub const fn muestra(p: &Pantalla, k: u32) -> (u32, u32) {
    let (i, j) = (k % 32, k / 32);
    ((i * (p.ancho - 1)) / 31, (j * (p.alto - 1)) / 31)
}

/// **El programa**, como lo leyo `nvdisasm` (ver arriba).
pub const CODIGO: [(u64, u64); 62] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x00000000ff067424, 0x000fe200078e00ff), // IMAD.MOV.U32 R6, RZ, RZ, 0x0
    (0x0000000000007918, 0x000fe20000000000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x00000002ff077424, 0x000fca00078e00ff), // IMAD.MOV.U32 R7, RZ, RZ, 0x2
    (0x00e5000406027981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R2, [R6.64+0xe500]
    (0x00e5040406037981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R3, [R6.64+0xe504]
    (0x00e5080406007981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R0, [R6.64+0xe508]
    (0x00e50c0406047981, 0x000ea8000c1f5900), // LDG.E.STRONG.SYS R4, [R6.64+0xe50c]
    (0x00e5140406087981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R8, [R6.64+0xe514]
    (0x00e5180406097981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R9, [R6.64+0xe518]
    (0x00e51c04060a7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R10, [R6.64+0xe51c]
    (0x00e52004060c7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R12, [R6.64+0xe520]
    (0x00e52404060d7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R13, [R6.64+0xe524]
    (0x0000000000057919, 0x000e680000002100), // S2R R5, SR_TID.X
    (0x00000000000e7919, 0x000e640000002500), // S2R R14, SR_CTAID.X
    (0x000001000e057824, 0x002fca00078e0205), // IMAD R5, R14, 0x100, R5
    (0x000000040500720c, 0x004fda0003f06070), // ISETP.GE.U32.AND P0, PT, R5, R4, PT
    (0x000000000000094d, 0x000fea0003800000), // @P0 EXIT
    (0x00000000000679c3, 0x001e220000002600), // S2UR UR6, SR_CTAID.Y
    (0x0000015000007945, 0x000fe20003800000), // BSSY B0, 0x290
    (0x000000050a047224, 0x060fe200078e0208), // IMAD R4, R10.reuse, R5, R8
    (0x0000000000067805, 0x000fe2000001ff00), // CS2R R6, SRZ
    (0x000000ffff0f7224, 0x000fe400078e00ff), // IMAD.MOV.U32 R15, RZ, RZ, RZ
    (0x000000060a0e7c24, 0x001fc6000f8e0209), // IMAD R14, R10, UR6, R9
    (0x0000000606087225, 0x000fc800078e02ff), // IMAD.WIDE R8, R6, R6, RZ
    (0x00000007070a7225, 0x000fca00078e02ff), // IMAD.WIDE R10, R7, R7, RZ
    (0x0000000a08107210, 0x000fc80007f3e0ff), // IADD3 R16, P1, R8, R10, RZ
    (0x000000ff1000720c, 0x000fe20003f04070), // ISETP.GT.U32.AND P0, PT, R16, RZ, PT
    (0x0000000109107824, 0x000fca00008e060b), // IMAD.X R16, R9, 0x1, R11, P1
    (0x040000001000780c, 0x000fda0003f04300), // ISETP.GT.AND.EX P0, PT, R16, 0x4000000, PT, P0
    (0x0000009000000947, 0x000fea0003800000), // @P0 BRA 0x280
    (0x000000010f0f7810, 0x000fe20007ffe0ff), // IADD3 R15, R15, 0x1, RZ
    (0x0000000706067225, 0x000fe200078e02ff), // IMAD.WIDE R6, R6, R7, RZ
    (0x0000001c08087819, 0x000fe40000001009), // SHF.R.S64 R8, R8, 0x1c, R9
    (0x000001000f00780c, 0x000fe40003f06070), // ISETP.GE.U32.AND P0, PT, R15, 0x100, PT
    (0x0000001b06077819, 0x000fe40000001007), // SHF.R.S64 R7, R6, 0x1b, R7
    (0x0000001c0a0b7819, 0x000fc6000000100b), // SHF.R.S64 R11, R10, 0x1c, R11
    (0x000000010e077824, 0x000fe200078e0207), // IMAD.IADD R7, R14, 0x1, R7
    (0x0000000804067210, 0x000fca0007ffe80b), // IADD3 R6, R4, R8, -R11
    (0xffffff0000008947, 0x000fea000383ffff), // @!P0 BRA 0x180
    (0x0000000000007941, 0x000fea0003800000), // BSYNC B0
    (0x000000010c0c7824, 0x000fe200078e020f), // IMAD.IADD R12, R12, 0x1, R15
    (0x000000ff0d00720c, 0x000fe20003f05070), // ISETP.NE.U32.AND P0, PT, R13, RZ, PT
    (0x0000000600057c24, 0x000fe4000f8e0205), // IMAD R5, R0, UR6, R5
    (0x000000020c047824, 0x040fe400078e00ff), // IMAD.SHL.U32 R4, R12.reuse, 0x2, RZ
    (0x000000100c067824, 0x040fe400078e00ff), // IMAD.SHL.U32 R6, R12.reuse, 0x10, RZ
    (0x000000080c0c7824, 0x000fe200078e00ff), // IMAD.SHL.U32 R12, R12, 0x8, RZ
    (0x000000ff04047812, 0x000fe200078ec0ff), // LOP3.LUT R4, R4, 0xff, RZ, 0xc0, !PT
    (0x0000000405027825, 0x000fe200078e0002), // IMAD.WIDE.U32 R2, R5, 0x4, R2
    (0x000000ff06077812, 0x000fc400078ec0ff), // LOP3.LUT R7, R6, 0xff, RZ, 0xc0, !PT
    (0x000001ff0c0c7812, 0x000fe400078ec0ff), // LOP3.LUT R12, R12, 0x1ff, RZ, 0xc0, !PT
    (0x0000000407067207, 0x000fe40000000000), // SEL R6, R7, R4, P0
    (0x000000ff0c0c7817, 0x000fe40003800000), // IMNMX.U32 R12, R12, 0xff, PT
    (0x0000000704047207, 0x000fe20000000000), // SEL R4, R4, R7, P0
    (0x0001000006077824, 0x000fe200078e00ff), // IMAD.U32 R7, R6, 0x10000, RZ
    (0x000001000f00780c, 0x000fe20003f05070), // ISETP.NE.U32.AND P0, PT, R15, 0x100, PT
    (0x000001000c0c7824, 0x000fca00078e00ff), // IMAD.SHL.U32 R12, R12, 0x100, RZ
    (0x0000000704047212, 0x000fc800078efe0c), // LOP3.LUT R4, R4, R7, R12, 0xfe, !PT
    (0x000000ff04057207, 0x000fca0000000000), // SEL R5, R4, RZ, P0
    (0x0000000502007986, 0x000fe2000c101904), // STG.E [R2.64], R5
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA 0x3d0
];

pub const PALABRAS_CODIGO: usize = CODIGO.len() * 4;

pub fn codigo() -> [u32; PALABRAS_CODIGO] {
    let mut w = [0u32; PALABRAS_CODIGO];
    for (k, &(lo, hi)) in CODIGO.iter().enumerate() {
        w[4 * k] = lo as u32;
        w[4 * k + 1] = (lo >> 32) as u32;
        w[4 * k + 2] = hi as u32;
        w[4 * k + 3] = (hi >> 32) as u32;
    }
    w
}

/// `bloques` x `alto` bloques de 256 hilos.
pub fn qmd(p: &Pantalla) -> [u32; QMD_PALABRAS] {
    qmd_rejilla(sombreador_va(PROGRAMA), HILOS, p.bloques(), p.alto, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

/// **Preparar el fotograma** con la entrada `e` del GPFIFO de GR. `todo`: el
/// programa, el QMD y las ordenes tambien (el primero, o si otro trabajo
/// uso sus paginas); si no, solo los semaforos, los parametros y la entrada:
/// asi un fotograma cuesta unas 20 escrituras por PRAMIN, no 400.
pub fn preparar<R: Registros>(r: &mut R, e: u32, p: &Pantalla, m: &Marco, todo: bool) -> bool {
    if !crate::blur::entrada_valida(e) || !p.valida() {
        return false;
    }
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let base = escribir(r, SEMAFORO_QMD, &[0; 8]) == 8 && escribir(r, PARAMETROS, &parametros(p, m)) == N_PARAMETROS;
    let cargado = !todo || {
        let (c, q, o) = (codigo(), qmd(p), ordenes());
        escribir(r, PROGRAMA, &c) == PALABRAS_CODIGO && escribir(r, QMD, &q) == QMD_PALABRAS && escribir(r, EMPUJE, &o) == ORDENES
    };
    base && cargado && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = crate::blur::entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[crate::blur::siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

pub use crate::fractal::{desempaquetar, empaquetar};

/// Sano: los dos semaforos pagados y las muestras iguales.
pub const fn sano(v: u64) -> bool {
    let (buenos, qmd, fin, lanzado, _, _) = desempaquetar(v);
    buenos == MUESTRAS && qmd && fin && lanzado
}

const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);
const _: () = assert!(PARAMETROS >= crate::giro::PARAMETROS + 20 && PARAMETROS + 4 * N_PARAMETROS as u64 <= SEMAFOROS + 4096);
const _: () = assert!(SEMAFORO_QMD > crate::giro::SEMAFORO_FIN && SEMAFORO_FIN + 16 <= crate::raster::ESCALONES);
const _: () = assert!(crate::lienzo::sombreador_va(PARAMETROS) == 0x2_0000_E500);
const _: () = assert!(TABLAS >= crate::gr::TABLAS + 0x1000 * (crate::gr::PTS as u64 + 1));
const _: () = assert!(TABLAS + 0x1000 * (PTS as u64 + 1) <= crate::gr::VRAM);

#[cfg(test)]
mod pruebas {
    use super::*;

    const FHD: Pantalla = Pantalla { vram: 0, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };

    #[test]
    fn la_pantalla_se_cuelga_de_su_propia_entrada() {
        assert_eq!(indices(VA), [0, 0, 32, 0, 0]);
        assert_ne!(entrada_pd1(), crate::gr::entrada_pd1());
        assert_ne!(indices(VA)[2], indices(crate::vram::TRAMO_VA)[2]);
    }

    #[test]
    fn que_pantallas_valen() {
        assert!(FHD.valida());
        assert_eq!(FHD.bloques(), 8);
        let cuatro_k = Pantalla { pitch: 3840, ancho: 3840, alto: 2160, ..FHD };
        assert!(cuatro_k.valida(), "3840x2160 cabe en 16 PT");
        assert!(!Pantalla { vram: 0x0300_0000, ..cuatro_k }.valida(), "no pasa de los 64 MiB");
        assert!(!Pantalla { vram: 0x800, ..FHD }.valida(), "sin alinear");
        assert!(!Pantalla { ancho: 2000, ..FHD }.valida(), "mas ancha que su pitch");
        assert_eq!(desde_gop(0xD000_0000, 0xD000_0000, 1920, 1920, 1080, false), Some(FHD));
        assert_eq!(desde_gop(0xC000_0000, 0xD000_0000, 1920, 1920, 1080, false), None);
    }

    #[test]
    fn el_mapa_de_la_hoja_a_la_raiz() {
        struct Vram(std::collections::BTreeMap<u64, u32>, u32);
        impl Registros for Vram {
            fn leer(&mut self, reg: u32) -> u32 {
                if reg == crate::vram::VENTANA_REG {
                    return self.1;
                }
                let d = ((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64;
                *self.0.get(&d).unwrap_or(&0)
            }
            fn escribir(&mut self, reg: u32, v: u32) {
                if reg == crate::vram::VENTANA_REG {
                    self.1 = v;
                    return;
                }
                let d = ((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64;
                self.0.insert(d, v);
            }
        }
        extern crate std;
        let mut r = Vram(Default::default(), 0);
        let p = Pantalla { vram: 0x0010_0000, ..FHD };
        let (n, bien) = mapear(&mut r, &p).unwrap();
        let paginas = p.bytes().div_ceil(4096);
        assert_eq!(n as u64, paginas + 4 + 1, "2025 PTE, 4 PDE de la PD0 y la de la PD1");
        assert_eq!(bien, n);
        assert_eq!(leer64(&mut r, TABLAS + 0x1000), pte_vram(0x0010_0000));
        assert_eq!(leer64(&mut r, TABLAS + 8), pde_vram(TABLAS + 0x1000));
        assert_eq!(leer64(&mut r, entrada_pd1()), pde_vram(TABLAS));
        // Otra vez, la misma: vale. Otra cosa colgada ahi: no se pisa.
        assert!(mapear(&mut r, &p).is_some());
        escribir64(&mut r, entrada_pd1(), pde_vram(0x0999_0000));
        assert!(mapear(&mut r, &p).is_none());
    }

    #[test]
    fn el_programa_lee_sus_parametros_y_escribe_una_vez() {
        let ldg: std::vec::Vec<u64> = CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x981).map(|c| c.0 >> 40 & 0xFFFF).collect();
        extern crate std;
        assert_eq!(ldg, [0xE500, 0xE504, 0xE508, 0xE50C, 0xE514, 0xE518, 0xE51C, 0xE520, 0xE524]);
        assert_eq!(CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x986).count(), 1, "un STG");
        // Ningun LDG/STG con descriptor (el bit 101 = 37 de la palabra alta).
        assert!(CODIGO.iter().filter(|c| matches!(c.0 & 0xFFF, 0x981 | 0x986)).all(|c| c.1 >> 37 & 1 == 0));
        assert_eq!(CODIGO[0].0, 0x7918, "el c[0x0][0x28], en NOP");
    }

    #[test]
    fn el_ciclo_se_acerca_y_vuelve() {
        let a = marco(0, 1920, 1080);
        assert_eq!(a.paso, 939_524_096 / 1920);
        assert_eq!(a, Marco { desplaza: 0, ..marco(CICLO, 1920, 1080) });
        let medio = marco(ACERCA, 1920, 1080);
        assert!(medio.paso * 100 < a.paso && medio.paso > 1000, "unas 180 veces mas cerca, sin perder la cuenta");
        assert_eq!(marco(ACERCA - 7, 1920, 1080).paso, marco(ACERCA + 7, 1920, 1080).paso);
        // El centro de la pantalla es el valle, en todos.
        for f in [0, 37, ACERCA, 150] {
            let m = marco(f, 1920, 1080);
            assert!((m.cr0 + 960 * m.paso - CENTRO.0).abs() < 2 && (m.ci0 + 540 * m.paso - CENTRO.1).abs() < 2);
        }
    }

    #[test]
    fn el_color_es_el_del_fractal_sin_desplazar() {
        for n in [0, 1, 17, 31, 32, 100, 255, 256] {
            assert_eq!(color(n, 0, false), crate::fractal::color(n), "n = {n}");
        }
        let c = color(5, 0, false);
        assert_eq!(color(5, 0, true), (c & 0xFF) << 16 | c & 0xFF00 | c >> 16);
        assert_eq!(color(VUELTAS, 77, true), 0);
    }

    #[test]
    fn las_vueltas_son_las_del_fractal() {
        use crate::fractal::{CI0, CR0, PASO};
        for (x, y) in [(0, 0), (256, 256), (100, 300), (511, 17), (340, 200)] {
            let v = vueltas(CR0 + x as i32 * PASO, CI0 + y as i32 * PASO);
            assert_eq!(v, crate::fractal::vueltas(x, y));
        }
    }

    #[test]
    fn las_muestras_tocan_las_esquinas() {
        assert_eq!(muestra(&FHD, 0), (0, 0));
        assert_eq!(muestra(&FHD, 31), (1919, 0));
        assert_eq!(muestra(&FHD, MUESTRAS - 1), (1919, 1079));
    }
}
