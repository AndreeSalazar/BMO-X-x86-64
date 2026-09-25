//! **M6 V0: EL VIDEO POR LA 3060** -- un fotograma NV12 (el formato que da
//! `ffmpeg`, y el que dara el decodificador de la propia 3060, NVDEC) pasa a
//! RGB, se agranda a la pantalla y se escribe DIRECTAMENTE en el framebuffer
//! del GOP, por un programa de computo. La CPU solo lee el fichero: no
//! convierte ni escala ni mueve un pixel de la pantalla.
//!
//! capa: puro -- el mapa, el programa, el encaje en la pantalla y la cuenta
//! de referencia; la RAM, la IOMMU y los registros los toca el kernel (L8)
//!
//! [eje]     RENDIMIENTO -- la conversion de color y el escalado son lo que
//!           mas cuesta de mostrar video en una CPU (pl_mpeg: la mitad del
//!           tiempo por fotograma); aqui son trabajo de la 3060
//!
//! # Por que NV12 y no MP4
//!
//! Un `.mp4` es una CAJA; lo de dentro suele ser H.264, un codec que son
//! meses de trabajo en software. La 3060 trae NVDEC, que lo decodifica en
//! hardware y ENTREGA NV12. Asi que este es el tramo de despues del
//! decodificador: hoy lo alimenta un NV12 crudo del disco (`ffmpeg -i
//! video.mp4 -s 640x360 -pix_fmt nv12 -f rawvideo video.nv12`, en Windows o
//! en la antena), luego NVDEC o pl_mpeg, sin cambiar esto.
//!
//! # Donde vive cada cosa
//!
//! ```text
//!    ORIGEN   un bloque KIND_MEMORIA del escritorio con UN fotograma NV12:
//!             Y (ancho x alto) y detras UV intercalado (ancho x alto/2).
//!             Prestado a la 3060 SOLO LECTURA en la IOVA 0x5400_0000 durante
//!             UNA llamada, y visto en la VA 0x6_0000_0000 (la PD1 del tramo,
//!             entrada 48; tablas en VRAM 0x0460_0000), PTE de SISTEMA
//!    DESTINO  la pantalla del GOP en la VA de `pantalla`, centrado y
//!             agrandado por un entero (640x360 -> x3 en 1920x1080)
//!    PROGRAMA ptxas sm_86, 184 instrucciones, 24 registros: un hilo por
//!             bloque de 2x2 del origen (4 Y y un par UV, cada byte leido UNA
//!             vez), BT.601 de rango limitado en enteros, y los 4 cuadrados
//!             de s x s en la pantalla
//! ```
//!
//! # Como se sabe
//!
//! La CPU rehace [`MUESTRAS`] pixeles de la pantalla desde el MISMO NV12 y los
//! lee del framebuffer: iguales bit a bit, o la fila lo dice.

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::mmu::{indices, pde_vram, pte_sistema};
use crate::pantalla::Pantalla;
use crate::sombreador::{ordenes_con, qmd_rejilla, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::vram::{a_cero, escribir64, leer64};
use crate::Registros;

/// Donde ve la GPU el fotograma.
pub const VA: u64 = 0x6_0000_0000;
/// Donde lo ve la 3060 por la IOMMU.
pub const IOVA: u64 = 0x5400_0000;
/// La PD0 y las PT del fotograma, en VRAM (tras las del volcado, 0x0450_0000).
pub const TABLAS: u64 = 0x0460_0000;
/// Cuantas PT caben: 4 x 2 MiB (un 1920x1080 NV12 son 3 MiB).
pub const PTS: usize = 4;
pub const MAX_BYTES: u64 = PTS as u64 * (2 << 20);
const PAGINA: u64 = 0x1000;

pub const HILOS: u32 = 256;
pub const REGISTROS: u32 = 32;
pub const PAGA_QMD: u32 = 0x3060_5140;
pub const PAGA_FIN: u32 = 0x3060_51F0;
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x140;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x150;
/// Donde lee el programa sus 9 parametros (VA 0x2_0000_E580).
pub const PARAMETROS: u64 = SEMAFOROS + 0x580;
pub const N_PARAMETROS: usize = 9;
/// Pixeles de la pantalla que la CPU comprueba por fotograma (16 x 16).
pub const MUESTRAS: u32 = 256;

/// **El formato** de un fotograma NV12.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Formato {
    pub ancho: u32,
    pub alto: u32,
}

impl Formato {
    /// Y + UV: ancho x alto x 3/2.
    pub const fn bytes(&self) -> u64 {
        self.ancho as u64 * self.alto as u64 * 3 / 2
    }

    pub const fn paginas(&self) -> u64 {
        self.bytes().div_ceil(PAGINA)
    }

    /// Par en las dos medidas (el UV va de 2 en 2) y que quepa en el mapa.
    pub const fn valido(&self) -> bool {
        self.ancho >= 2 && self.alto >= 2 && self.ancho % 2 == 0 && self.alto % 2 == 0 && self.ancho <= 4096 && self.bytes() <= MAX_BYTES
    }
}

/// **Como cae en la pantalla**: la escala entera mas grande que cabe, y la
/// esquina para que quede centrado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Encaje {
    pub escala: u32,
    pub x0: u32,
    pub y0: u32,
}

/// `None` si el formato no vale o el video es MAS GRANDE que la pantalla.
pub const fn encaje(f: &Formato, p: &Pantalla) -> Option<Encaje> {
    if !f.valido() {
        return None;
    }
    let (sx, sy) = (p.ancho / f.ancho, p.alto / f.alto);
    let escala = if sx < sy { sx } else { sy };
    if escala == 0 {
        return None;
    }
    Some(Encaje { escala, x0: (p.ancho - f.ancho * escala) / 2, y0: (p.alto - f.alto * escala) / 2 })
}

/// Los 9 parametros, en el orden en que los lee el programa: el origen, sus
/// medidas, la esquina del destino (ya con `x0, y0`), el paso de la pantalla,
/// la escala y si el GOP es RGB.
pub fn parametros(f: &Formato, e: &Encaje, p: &Pantalla) -> [u32; N_PARAMETROS] {
    let d = crate::pantalla::VA + 4 * (e.y0 as u64 * p.pitch as u64 + e.x0 as u64);
    [VA as u32, (VA >> 32) as u32, f.ancho, f.alto, d as u32, (d >> 32) as u32, p.pitch, e.escala, p.rgb as u32]
}

/// **BT.601 de rango limitado**, en enteros: la MISMA cuenta que el programa.
/// `0x00RRGGBB` (rojo y azul cambiados si `rgb`).
pub const fn color(y: u8, u: u8, v: u8, rgb: bool) -> u32 {
    let c = 298 * (y as i32 - 16) + 128;
    let (d, e) = (u as i32 - 128, v as i32 - 128);
    let r = sat((c + 409 * e) >> 8);
    let g = sat((c - 100 * d - 208 * e) >> 8);
    let b = sat((c + 516 * d) >> 8);
    let (r, b) = if rgb { (b, r) } else { (r, b) };
    r << 16 | g << 8 | b
}

const fn sat(v: i32) -> u32 {
    (if v < 0 { 0 } else if v > 255 { 255 } else { v }) as u32
}

/// Lo que tiene que haber en `(dx, dy)` DENTRO del rectangulo del video
/// (sin `x0, y0`), desde el fotograma `nv12`.
pub fn pixel(nv12: &[u8], f: &Formato, e: &Encaje, rgb: bool, dx: u32, dy: u32) -> u32 {
    let (x, y) = ((dx / e.escala) as usize, (dy / e.escala) as usize);
    let w = f.ancho as usize;
    let uv = w * f.alto as usize + (y / 2) * w + (x & !1);
    color(nv12[y * w + x], nv12[uv], nv12[uv + 1], rgb)
}

/// La muestra `k` (de [`MUESTRAS`]): una rejilla de 16 x 16 dentro del
/// rectangulo del video, de borde a borde. `(dx, dy)` sin la esquina.
pub const fn muestra(f: &Formato, e: &Encaje, k: u32) -> (u32, u32) {
    let (w, h) = (f.ancho * e.escala, f.alto * e.escala);
    let (i, j) = (k % 16, k / 16);
    ((i * (w - 1)) / 15, (j * (h - 1)) / 15)
}

/// La entrada de la PD1 del tramo que cuelga [`VA`].
pub const fn entrada_pd1() -> u64 {
    crate::vram::TABLAS[1] + 8 * indices(VA)[2] as u64
}

/// **Mapear** [`MAX_BYTES`] en [`VA`] con PTE de SISTEMA hacia [`IOVA`]: como
/// el volcado, de la hoja a la raiz y RELEIDO, solo si la entrada de la PD1
/// esta vacia o ya es la nuestra. No depende de DONDE esta el fotograma (eso
/// lo pone la IOMMU al prestarlo), asi que se hace una vez. `(escrituras,
/// releidas)`.
pub fn mapear<R: Registros>(r: &mut R) -> Option<(u32, u32)> {
    if VA % (2 << 20) != 0 {
        return None;
    }
    let pd1 = leer64(r, entrada_pd1());
    if pd1 != 0 && pd1 != pde_vram(TABLAS) {
        return None;
    }
    let paginas = MAX_BYTES / PAGINA;
    let pt = |k: usize| TABLAS + PAGINA * (1 + k as u64);
    if pd1 == 0 {
        for k in 0..=PTS {
            a_cero(r, TABLAS + PAGINA * k as u64);
        }
    }
    let (mut n, mut bien) = (0u32, 0u32);
    let mut poner = |r: &mut R, dir: u64, v: u64| {
        escribir64(r, dir, v);
        n += 1;
        bien += (leer64(r, dir) == v) as u32;
    };
    for q in 0..paginas {
        poner(r, pt((q / 512) as usize) + 8 * (q % 512), pte_sistema(IOVA + q * PAGINA));
    }
    let i0 = indices(VA)[3] as u64;
    for k in 0..PTS {
        poner(r, TABLAS + 16 * (i0 + k as u64) + 8, pde_vram(pt(k)));
    }
    poner(r, entrada_pd1(), pde_vram(TABLAS));
    Some((n, bien))
}

/// **El programa**, como lo leyo `nvdisasm` (el PTX de origen, en
/// `docs/plan/PLAN_LA_3060.md`, M6 V0).
pub const CODIGO: [(u64, u64); 184] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP (era IMAD.MOV.U32 R1, RZ, RZ, c[0x0][0x28])
    (0x00000000ff087424, 0x000fe200078e00ff), // IMAD.MOV.U32 R8, RZ, RZ, 0x0
    (0x0000000000007918, 0x000fe20000000000), // NOP (era ULDC.64 UR6, c[0x0][0x118])
    (0x00000002ff097424, 0x000fca00078e00ff), // IMAD.MOV.U32 R9, RZ, RZ, 0x2
    (0x00e58006080e7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R14, [R8.64+0xe580]
    (0x00e5840608067981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R6, [R8.64+0xe584]
    (0x00e58806080c7981, 0x000ea8000c1f5900), // LDG.E.STRONG.SYS R12, [R8.64+0xe588]
    (0x0000000000057919, 0x000e680000002100), // S2R R5, SR_TID.X
    (0x0000000000007919, 0x000e680000002500), // S2R R0, SR_CTAID.X
    (0x00e58c06080b7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R11, [R8.64+0xe58c]
    (0x00e5900608027981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R2, [R8.64+0xe590]
    (0x00e5940608037981, 0x000162000c1f5900), // LDG.E.STRONG.SYS R3, [R8.64+0xe594]
    (0x0000010000057824, 0x002fe200078e0205), // IMAD R5, R0, 0x100, R5
    (0x00000001ff007819, 0x004fc8000001160c), // SHF.R.U32.HI R0, RZ, 0x1, R12
    (0x000000000500720c, 0x000fe40003f06070), // ISETP.GE.U32.AND P0, PT, R5, R0, PT
    (0x00e5980608007981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R0, [R8.64+0xe598]
    (0x00e59c0608047981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R4, [R8.64+0xe59c]
    (0x00e5a00608077981, 0x000166000c1f5900), // LDG.E.STRONG.SYS R7, [R8.64+0xe5a0]
    (0x000000000000094d, 0x000fea0003800000), // @P0 EXIT
    (0x00000000000579c3, 0x000e620000002600), // S2UR UR5, SR_CTAID.Y
    (0x0000000205057824, 0x000fe200078e00ff), // IMAD.SHL.U32 R5, R5, 0x2, RZ
    (0x0000000105047899, 0x002fc6000800063f), // USHF.L.U32 UR4, UR5, 0x1, URZ
    (0x000000050c087c24, 0x001fc8000f8e0205), // IMAD R8, R12, UR5, R5
    (0x0000000b0c0b7224, 0x060fe400078e0208), // IMAD R11, R12.reuse, R11, R8
    (0x000000040c097c24, 0x000fca000f8e0205), // IMAD R9, R12, UR4, R5
    (0x000000090e0a7210, 0x040fe40007f1e0ff), // IADD3 R10, P0, R14.reuse, R9, RZ
    (0x0000000b0e0e7210, 0x000fc60007f3e0ff), // IADD3 R14, P1, R14, R11, RZ
    (0x000000ffff0b7224, 0x100fe400000e0606), // IMAD.X R11, RZ, RZ, R6.reuse, P0
    (0x000000ffff0f7224, 0x000fc600008e0606), // IMAD.X R15, RZ, RZ, R6, P1
    (0x000000060a067981, 0x000ea8000c1e1500), // LDG.E.U16 R6, [R10.64]
    (0x000000060e0e7981, 0x000ee2000c1e1500), // LDG.E.U16 R14, [R14.64]
    (0x0000012aff127424, 0x000fe200078e00ff), // IMAD.MOV.U32 R18, RZ, RZ, 0x12a
    (0x0000000a0c0c7210, 0x000fe20007f1e0ff), // IADD3 R12, P0, R12, R10, RZ
    (0xffffff9cff137424, 0x000fe400078e00ff), // IMAD.MOV.U32 R19, RZ, RZ, -0x64
    (0x00000204ff157424, 0x000fe400078e00ff), // IMAD.MOV.U32 R21, RZ, RZ, 0x204
    (0x000000ffff0d7224, 0x000fca00000e060b), // IMAD.X R13, RZ, RZ, R11, P0
    (0x000000060c087981, 0x000162000c1e1500), // LDG.E.U16 R8, [R12.64]
    (0x000000ff0700720c, 0x000fe20003f05070), // ISETP.NE.U32.AND P0, PT, R7, RZ, PT
    (0x0000000504057224, 0x000fe200078e02ff), // IMAD R5, R4, R5, RZ
    (0x000000ff06097812, 0x004fe400078ec0ff), // LOP3.LUT R9, R6, 0xff, RZ, 0xc0, !PT
    (0x000000ff0e107812, 0x008fc600078ec0ff), // LOP3.LUT R16, R14, 0xff, RZ, 0xc0, !PT
    (0xffffed6009117424, 0x000fe200078e0212), // IMAD R17, R9, R18, -0x12a0
    (0xffffff800e097811, 0x000fe200078fc0ff), // LEA.HI R9, R14, 0xffffff80, RZ, 0x18
    (0x00003200100e7424, 0x000fc600078e0213), // IMAD R14, R16, R19, 0x3200
    (0x0000008011127810, 0x000fe20007ffe0ff), // IADD3 R18, R17, 0x80, RZ
    (0xfffefe00100a7424, 0x000fe400078e0215), // IMAD R10, R16, R21, -0x10200
    (0xffffff30090b7824, 0x040fe400078e020e), // IMAD R11, R9.reuse, -0xd0, R14
    (0x00000199090c7824, 0x101fe400078e0212), // IMAD R12, R9, 0x199, R18.reuse
    (0x000000010a0d7824, 0x100fe400078e0212), // IMAD.IADD R13, R10, 0x1, R18.reuse
    (0x000000010b127824, 0x000fe200078e0212), // IMAD.IADD R18, R11, 0x1, R18
    (0x00000008ff0c7819, 0x000fc4000001140c), // SHF.R.S32.HI R12, RZ, 0x8, R12
    (0x00000008ff0d7819, 0x000fe4000001140d), // SHF.R.S32.HI R13, RZ, 0x8, R13
    (0x00000008ff127819, 0x000fe40000011412), // SHF.R.S32.HI R18, RZ, 0x8, R18
    (0x0000000cff0c7217, 0x000fe40007800200), // IMNMX R12, RZ, R12, !PT
    (0x0000000dff0d7217, 0x000fe40007800200), // IMNMX R13, RZ, R13, !PT
    (0x00000012ff127217, 0x000fe40007800200), // IMNMX R18, RZ, R18, !PT
    (0x000000ff0c0c7817, 0x000fc40003800200), // IMNMX R12, R12, 0xff, PT
    (0x000000ff0d0d7817, 0x000fe40003800200), // IMNMX R13, R13, 0xff, PT
    (0x000000ff12127817, 0x000fe40003800200), // IMNMX R18, R18, 0xff, PT
    (0x0000000c0d077207, 0x000fe40000000000), // SEL R7, R13, R12, P0
    (0x0000000d0c0c7207, 0x000fe20000000000), // SEL R12, R12, R13, P0
    (0x0000010012127824, 0x000fe400078e00ff), // IMAD.SHL.U32 R18, R18, 0x100, RZ
    (0x0001000007077824, 0x000fca00078e00ff), // IMAD.U32 R7, R7, 0x10000, RZ
    (0x000000070c0f7212, 0x000fe200078efe12), // LOP3.LUT R15, R12, R7, R18, 0xfe, !PT
    (0x0000000404077c24, 0x000fe2000f8e02ff), // IMAD R7, R4, UR4, RZ
    (0x0000003f00047c82, 0x000fc80008000000), // UMOV UR4, URZ
    (0x00000004070c7c10, 0x001fe2000fffe0ff), // IADD3 R12, R7, UR4, RZ
    (0x0000000104047890, 0x000fe4000fffe03f), // UIADD3 UR4, UR4, 0x1, URZ
    (0x0000003f00057c82, 0x000fe40008000000), // UMOV UR5, URZ
    (0x0000000c000e7224, 0x000fe400078e0205), // IMAD R14, R0, R12, R5
    (0x0000000404007c0c, 0x000fc6000bf43070), // ISETP.LE.U32.AND P2, PT, R4, UR4, PT
    (0x000000050e0d7c10, 0x001fe2000fffe0ff), // IADD3 R13, R14, UR5, RZ
    (0x0000000105057890, 0x000fc8000fffe03f), // UIADD3 UR5, UR5, 0x1, URZ
    (0x000000040d0c7825, 0x000fe400078e0002), // IMAD.WIDE.U32 R12, R13, 0x4, R2
    (0x0000000504007c0c, 0x000fc6000bf23070), // ISETP.LE.U32.AND P1, PT, R4, UR5, PT
    (0x0000000f0c007986, 0x0001f4000c101906), // STG.E [R12.64], R15
    (0xffffffa000009947, 0x000fea000383ffff), // @!P1 BRA 0x470
    (0xffffff400000a947, 0x000fea000383ffff), // @!P2 BRA 0x420
    (0x00000008ff067819, 0x000fe20000011606), // SHF.R.U32.HI R6, RZ, 0x8, R6
    (0x0000012aff0d7424, 0x001fe200078e00ff), // IMAD.MOV.U32 R13, RZ, RZ, 0x12a
    (0x0000003f00047c82, 0x000fc40008000000), // UMOV UR4, URZ
    (0x000000ff06067812, 0x000fca00078ec0ff), // LOP3.LUT R6, R6, 0xff, RZ, 0xc0, !PT
    (0xffffed6006067424, 0x000fca00078e020d), // IMAD R6, R6, R13, -0x12a0
    (0x0000008006067810, 0x000fca0007ffe0ff), // IADD3 R6, R6, 0x80, RZ
    (0x00000199090c7824, 0x100fe400078e0206), // IMAD R12, R9, 0x199, R6.reuse
    (0x000000010a0d7824, 0x100fe400078e0206), // IMAD.IADD R13, R10, 0x1, R6.reuse
    (0x000000010b067824, 0x000fe200078e0206), // IMAD.IADD R6, R11, 0x1, R6
    (0x00000008ff0c7819, 0x000fe4000001140c), // SHF.R.S32.HI R12, RZ, 0x8, R12
    (0x00000008ff0d7819, 0x000fe4000001140d), // SHF.R.S32.HI R13, RZ, 0x8, R13
    (0x00000008ff067819, 0x000fe40000011406), // SHF.R.S32.HI R6, RZ, 0x8, R6
    (0x0000000cff0c7217, 0x000fc40007800200), // IMNMX R12, RZ, R12, !PT
    (0x0000000dff0d7217, 0x000fe40007800200), // IMNMX R13, RZ, R13, !PT
    (0x00000006ff067217, 0x000fe40007800200), // IMNMX R6, RZ, R6, !PT
    (0x000000ff0c0c7817, 0x000fe40003800200), // IMNMX R12, R12, 0xff, PT
    (0x000000ff0d0d7817, 0x000fe40003800200), // IMNMX R13, R13, 0xff, PT
    (0x000000ff06067817, 0x000fe40003800200), // IMNMX R6, R6, 0xff, PT
    (0x0000000c0d0e7207, 0x000fc40000000000), // SEL R14, R13, R12, P0
    (0x0000000d0c0c7207, 0x000fe20000000000), // SEL R12, R12, R13, P0
    (0x0000010006067824, 0x000fe400078e00ff), // IMAD.SHL.U32 R6, R6, 0x100, RZ
    (0x000100000e0d7824, 0x000fca00078e00ff), // IMAD.U32 R13, R14, 0x10000, RZ
    (0x0000000d0c0f7212, 0x000fe400078efe06), // LOP3.LUT R15, R12, R13, R6, 0xfe, !PT
    (0x0000000407067c10, 0x000fe2000fffe0ff), // IADD3 R6, R7, UR4, RZ
    (0x0000000104047890, 0x000fe4000fffe03f), // UIADD3 UR4, UR4, 0x1, URZ
    (0x0000003f00057c82, 0x000fe40008000000), // UMOV UR5, URZ
    (0x0000000600117224, 0x000fe400078e0205), // IMAD R17, R0, R6, R5
    (0x0000000404007c0c, 0x001fc6000bf43070), // ISETP.LE.U32.AND P2, PT, R4, UR4, PT
    (0x00000005040d7c10, 0x001fe2000fffe011), // IADD3 R13, R4, UR5, R17
    (0x0000000105057890, 0x000fc8000fffe03f), // UIADD3 UR5, UR5, 0x1, URZ
    (0x000000040d0c7825, 0x000fe400078e0002), // IMAD.WIDE.U32 R12, R13, 0x4, R2
    (0x0000000504007c0c, 0x000fc6000bf23070), // ISETP.LE.U32.AND P1, PT, R4, UR5, PT
    (0x0000000f0c007986, 0x0001f4000c101906), // STG.E [R12.64], R15
    (0xffffffa000009947, 0x000fea000383ffff), // @!P1 BRA 0x6a0
    (0xffffff400000a947, 0x000fea000383ffff), // @!P2 BRA 0x650
    (0x000000ff08067812, 0x020fe200078ec0ff), // LOP3.LUT R6, R8, 0xff, RZ, 0xc0, !PT
    (0x0000012aff0d7424, 0x001fe200078e00ff), // IMAD.MOV.U32 R13, RZ, RZ, 0x12a
    (0x0000003f00047c82, 0x000fc60008000000), // UMOV UR4, URZ
    (0xffffed6006067424, 0x000fca00078e020d), // IMAD R6, R6, R13, -0x12a0
    (0x0000008006067810, 0x000fca0007ffe0ff), // IADD3 R6, R6, 0x80, RZ
    (0x00000199090c7824, 0x100fe400078e0206), // IMAD R12, R9, 0x199, R6.reuse
    (0x000000010a0d7824, 0x100fe400078e0206), // IMAD.IADD R13, R10, 0x1, R6.reuse
    (0x000000010b067824, 0x000fe200078e0206), // IMAD.IADD R6, R11, 0x1, R6
    (0x00000008ff0c7819, 0x000fe4000001140c), // SHF.R.S32.HI R12, RZ, 0x8, R12
    (0x00000008ff0d7819, 0x000fe4000001140d), // SHF.R.S32.HI R13, RZ, 0x8, R13
    (0x00000008ff067819, 0x000fe40000011406), // SHF.R.S32.HI R6, RZ, 0x8, R6
    (0x0000000cff0c7217, 0x000fc40007800200), // IMNMX R12, RZ, R12, !PT
    (0x0000000dff0d7217, 0x000fe40007800200), // IMNMX R13, RZ, R13, !PT
    (0x00000006ff067217, 0x000fe40007800200), // IMNMX R6, RZ, R6, !PT
    (0x000000ff0c0c7817, 0x000fe40003800200), // IMNMX R12, R12, 0xff, PT
    (0x000000ff0d0d7817, 0x000fe40003800200), // IMNMX R13, R13, 0xff, PT
    (0x000000ff06067817, 0x000fe40003800200), // IMNMX R6, R6, 0xff, PT
    (0x0000000c0d0e7207, 0x000fc40000000000), // SEL R14, R13, R12, P0
    (0x0000000d0c0c7207, 0x000fe20000000000), // SEL R12, R12, R13, P0
    (0x0000010006067824, 0x000fe400078e00ff), // IMAD.SHL.U32 R6, R6, 0x100, RZ
    (0x000100000e0d7824, 0x000fca00078e00ff), // IMAD.U32 R13, R14, 0x10000, RZ
    (0x0000000d0c0f7212, 0x000fe400078efe06), // LOP3.LUT R15, R12, R13, R6, 0xfe, !PT
    (0x0000000407067c10, 0x000fe2000fffe004), // IADD3 R6, R7, UR4, R4
    (0x0000000104047890, 0x000fe4000fffe03f), // UIADD3 UR4, UR4, 0x1, URZ
    (0x0000003f00057c82, 0x000fe40008000000), // UMOV UR5, URZ
    (0x0000000600067224, 0x000fe400078e0205), // IMAD R6, R0, R6, R5
    (0x0000000404007c0c, 0x001fc6000bf43070), // ISETP.LE.U32.AND P2, PT, R4, UR4, PT
    (0x00000005060d7c10, 0x001fe2000fffe0ff), // IADD3 R13, R6, UR5, RZ
    (0x0000000105057890, 0x000fc8000fffe03f), // UIADD3 UR5, UR5, 0x1, URZ
    (0x000000040d0c7825, 0x000fe400078e0002), // IMAD.WIDE.U32 R12, R13, 0x4, R2
    (0x0000000504007c0c, 0x000fc6000bf23070), // ISETP.LE.U32.AND P1, PT, R4, UR5, PT
    (0x0000000f0c007986, 0x0001f4000c101906), // STG.E [R12.64], R15
    (0xffffffa000009947, 0x000fea000383ffff), // @!P1 BRA 0x8c0
    (0xffffff400000a947, 0x000fea000383ffff), // @!P2 BRA 0x870
    (0x00000008ff087819, 0x000fe20000011608), // SHF.R.U32.HI R8, RZ, 0x8, R8
    (0x0000012aff0d7424, 0x001fe200078e00ff), // IMAD.MOV.U32 R13, RZ, RZ, 0x12a
    (0x0000003f00047c82, 0x000fc40008000000), // UMOV UR4, URZ
    (0x000000ff08087812, 0x000fca00078ec0ff), // LOP3.LUT R8, R8, 0xff, RZ, 0xc0, !PT
    (0xffffed6008087424, 0x000fca00078e020d), // IMAD R8, R8, R13, -0x12a0
    (0x0000008008087810, 0x000fca0007ffe0ff), // IADD3 R8, R8, 0x80, RZ
    (0x0000019909097824, 0x100fe400078e0208), // IMAD R9, R9, 0x199, R8.reuse
    (0x000000010a0a7824, 0x100fe400078e0208), // IMAD.IADD R10, R10, 0x1, R8.reuse
    (0x000000010b087824, 0x000fe200078e0208), // IMAD.IADD R8, R11, 0x1, R8
    (0x00000008ff097819, 0x000fe40000011409), // SHF.R.S32.HI R9, RZ, 0x8, R9
    (0x00000008ff0a7819, 0x000fe4000001140a), // SHF.R.S32.HI R10, RZ, 0x8, R10
    (0x00000008ff087819, 0x000fe40000011408), // SHF.R.S32.HI R8, RZ, 0x8, R8
    (0x00000009ff097217, 0x000fc40007800200), // IMNMX R9, RZ, R9, !PT
    (0x0000000aff0a7217, 0x000fe40007800200), // IMNMX R10, RZ, R10, !PT
    (0x00000008ff087217, 0x000fe40007800200), // IMNMX R8, RZ, R8, !PT
    (0x000000ff09097817, 0x000fe40003800200), // IMNMX R9, R9, 0xff, PT
    (0x000000ff0a0a7817, 0x000fe40003800200), // IMNMX R10, R10, 0xff, PT
    (0x000000ff08087817, 0x000fe40003800200), // IMNMX R8, R8, 0xff, PT
    (0x000000090a067207, 0x000fc40000000000), // SEL R6, R10, R9, P0
    (0x0000000a09097207, 0x000fe20000000000), // SEL R9, R9, R10, P0
    (0x0000010008087824, 0x000fe400078e00ff), // IMAD.SHL.U32 R8, R8, 0x100, RZ
    (0x0001000006067824, 0x000fca00078e00ff), // IMAD.U32 R6, R6, 0x10000, RZ
    (0x00000006090b7212, 0x000fe400078efe08), // LOP3.LUT R11, R9, R6, R8, 0xfe, !PT
    (0x0000000407067c10, 0x000fe2000fffe004), // IADD3 R6, R7, UR4, R4
    (0x0000000104047890, 0x000fe4000fffe03f), // UIADD3 UR4, UR4, 0x1, URZ
    (0x0000003f00057c82, 0x000fe40008000000), // UMOV UR5, URZ
    (0x00000006000d7224, 0x000fe400078e0205), // IMAD R13, R0, R6, R5
    (0x0000000404007c0c, 0x001fc6000bf23070), // ISETP.LE.U32.AND P1, PT, R4, UR4, PT
    (0x0000000504097c10, 0x001fe2000fffe00d), // IADD3 R9, R4, UR5, R13
    (0x0000000105057890, 0x000fc8000fffe03f), // UIADD3 UR5, UR5, 0x1, URZ
    (0x0000000409087825, 0x000fe400078e0002), // IMAD.WIDE.U32 R8, R9, 0x4, R2
    (0x0000000504007c0c, 0x000fc6000bf03070), // ISETP.LE.U32.AND P0, PT, R4, UR5, PT
    (0x0000000b08007986, 0x0001f4000c101906), // STG.E [R8.64], R11
    (0xffffffa000008947, 0x000fea000383ffff), // @!P0 BRA 0xaf0
    (0xffffff4000009947, 0x000fea000383ffff), // @!P1 BRA 0xaa0
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA 0xb70
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

/// Un hilo por bloque de 2x2: (ancho/2 / 256) x alto/2 bloques de 256.
pub fn qmd(f: &Formato) -> [u32; QMD_PALABRAS] {
    qmd_rejilla(sombreador_va(PROGRAMA), HILOS, (f.ancho / 2).div_ceil(HILOS), f.alto / 2, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

/// **Preparar el fotograma** con la entrada `e` del GPFIFO de GR. `todo`: el
/// programa, el QMD y las ordenes tambien (el primero de cada tanda, o si
/// otro trabajo uso sus paginas); si no, los semaforos, los parametros y la
/// entrada.
pub fn preparar<R: Registros>(r: &mut R, e: u32, f: &Formato, parametros: &[u32; N_PARAMETROS], todo: bool) -> bool {
    if !crate::blur::entrada_valida(e) || !f.valido() {
        return false;
    }
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    let base = escribir(r, SEMAFORO_QMD, &[0; 8]) == 8 && escribir(r, PARAMETROS, parametros) == N_PARAMETROS;
    let cargado = !todo || {
        let (c, q, o) = (codigo(), qmd(f), ordenes());
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
const _: () = assert!(PARAMETROS >= crate::pantalla::PARAMETROS + 4 * crate::pantalla::N_PARAMETROS as u64);
const _: () = assert!(PARAMETROS + 4 * N_PARAMETROS as u64 <= SEMAFOROS + 4096);
const _: () = assert!(SEMAFORO_QMD > crate::pantalla::SEMAFORO_FIN && SEMAFORO_FIN + 16 <= SEMAFOROS + 0x200);
const _: () = assert!(crate::lienzo::sombreador_va(PARAMETROS) == 0x2_0000_E580);
const _: () = assert!(TABLAS >= crate::volcado::TABLAS + PAGINA * (crate::volcado::PTS as u64 + 1));
const _: () = assert!(TABLAS + PAGINA * (PTS as u64 + 1) <= crate::gr::VRAM);
const _: () = assert!(IOVA >= crate::volcado::IOVA + crate::volcado::MAX_BYTES);

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    const FHD: Pantalla = Pantalla { vram: 0, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
    const V360: Formato = Formato { ancho: 640, alto: 360 };

    #[test]
    fn el_video_se_cuelga_de_su_propia_entrada() {
        assert_eq!(indices(VA), [0, 0, 48, 0, 0]);
        for otra in [crate::pantalla::entrada_pd1(), crate::volcado::entrada_pd1(), crate::gr::entrada_pd1()] {
            assert_ne!(entrada_pd1(), otra);
        }
    }

    #[test]
    fn el_encaje_centra_y_agranda_por_un_entero() {
        assert_eq!(encaje(&V360, &FHD), Some(Encaje { escala: 3, x0: 0, y0: 0 }));
        assert_eq!(encaje(&Formato { ancho: 640, alto: 480 }, &FHD), Some(Encaje { escala: 2, x0: 320, y0: 60 }));
        assert_eq!(encaje(&Formato { ancho: 1920, alto: 1080 }, &FHD), Some(Encaje { escala: 1, x0: 0, y0: 0 }));
        assert_eq!(encaje(&Formato { ancho: 3840, alto: 2160 }, &FHD), None, "mas grande que la pantalla");
        assert_eq!(encaje(&Formato { ancho: 641, alto: 360 }, &FHD), None, "impar");
        assert!(Formato { ancho: 1920, alto: 1080 }.bytes() <= MAX_BYTES);
    }

    #[test]
    fn el_color_de_bt601() {
        // Negro, blanco y los primarios de las barras de color de 75 %.
        assert_eq!(color(16, 128, 128, false), 0x00_0000);
        assert_eq!(color(235, 128, 128, false), 0xFF_FFFF);
        assert_eq!(color(81, 90, 240, false), 0xFF_0000);
        // El verde de 100 % en enteros sale con el azul a 1: el redondeo de
        // la cuenta (la misma en el programa), no un error.
        assert_eq!(color(145, 54, 34, false), 0x00_FF01);
        assert_eq!(color(41, 240, 110, false), 0x00_00FF);
        assert_eq!(color(81, 90, 240, true), 0x00_00FF, "en un GOP RGB, rojo y azul cambiados");
    }

    #[test]
    fn el_pixel_toma_su_bloque() {
        // Un 4x2: Y distintas, un solo par UV por bloque de 2x2.
        let f = Formato { ancho: 4, alto: 2 };
        let nv12: Vec<u8> = [16, 235, 81, 145, 41, 16, 235, 81, 128, 128, 90, 240].to_vec();
        let e = Encaje { escala: 2, x0: 0, y0: 0 };
        assert_eq!(pixel(&nv12, &f, &e, false, 0, 0), color(16, 128, 128, false));
        assert_eq!(pixel(&nv12, &f, &e, false, 3, 1), color(235, 128, 128, false), "el (1, 0) del origen, agrandado x2");
        assert_eq!(pixel(&nv12, &f, &e, false, 5, 0), color(81, 90, 240, false), "el segundo bloque usa su UV");
        assert_eq!(pixel(&nv12, &f, &e, false, 0, 3), color(41, 128, 128, false));
    }

    #[test]
    fn las_muestras_tocan_las_esquinas_del_video() {
        let e = encaje(&V360, &FHD).unwrap();
        assert_eq!(muestra(&V360, &e, 0), (0, 0));
        assert_eq!(muestra(&V360, &e, MUESTRAS - 1), (1919, 1079));
    }

    #[test]
    fn los_parametros_llevan_la_esquina() {
        let f = Formato { ancho: 640, alto: 480 };
        let e = encaje(&f, &FHD).unwrap();
        let p = parametros(&f, &e, &FHD);
        let d = (p[4] as u64) | (p[5] as u64) << 32;
        assert_eq!(d, crate::pantalla::VA + 4 * (60 * 1920 + 320));
        assert_eq!((p[0] as u64) | (p[1] as u64) << 32, VA);
        assert_eq!(&p[6..], &[1920, 2, 0]);
    }

    #[test]
    fn el_programa_lee_sus_parametros_y_el_nv12() {
        let ldg: Vec<u64> = CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x981 && c.1 & 0xFFFF == 0x5900).map(|c| c.0 >> 40 & 0xFFFF).collect();
        assert_eq!(ldg, (0..9).map(|k| 0xE580 + 4 * k).collect::<Vec<u64>>());
        assert_eq!(CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x981).count(), 12, "9 parametros y 3 lecturas del NV12");
        assert_eq!(CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x986).count(), 4, "un STG por pixel del bloque");
        assert!(CODIGO.iter().filter(|c| matches!(c.0 & 0xFFF, 0x981 | 0x986)).all(|c| c.1 >> 37 & 1 == 0), "sin descriptor");
        assert_eq!(CODIGO[0].0, 0x7918, "el c[0x0][0x28], en NOP");
    }

    #[test]
    fn el_mapa_de_la_hoja_a_la_raiz() {
        struct Vram(std::collections::BTreeMap<u64, u32>, u32);
        impl Registros for Vram {
            fn leer(&mut self, reg: u32) -> u32 {
                if reg == crate::vram::VENTANA_REG {
                    return self.1;
                }
                *self.0.get(&(((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64)).unwrap_or(&0)
            }
            fn escribir(&mut self, reg: u32, v: u32) {
                if reg == crate::vram::VENTANA_REG {
                    self.1 = v;
                    return;
                }
                self.0.insert(((self.1 as u64) << 16) + (reg - crate::vram::VENTANA) as u64, v);
            }
        }
        let mut r = Vram(Default::default(), 0);
        let (n, bien) = mapear(&mut r).unwrap();
        assert_eq!(n as u64, MAX_BYTES / PAGINA + PTS as u64 + 1);
        assert_eq!(bien, n);
        assert_eq!(leer64(&mut r, TABLAS + PAGINA + 8), pte_sistema(IOVA + PAGINA));
        assert_eq!(leer64(&mut r, entrada_pd1()), pde_vram(TABLAS));
        assert!(mapear(&mut r).is_some(), "otra vez, la misma: vale");
        escribir64(&mut r, entrada_pd1(), pde_vram(0x0999_0000));
        assert!(mapear(&mut r).is_none(), "otra cosa colgada ahi: no se pisa");
    }
}
