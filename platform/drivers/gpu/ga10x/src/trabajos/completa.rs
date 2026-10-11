//! **VE5: COMPLETAR POR LA 3060** -- una pasada del filtro "a trous" de
//! ESTUDIO VIVO (el que quita el grano guiado por la superficie de cada
//! pixel, la idea de SVGF) como programa de COMPUTO: un hilo por pixel. La
//! cuenta es la de VERRANO (`bmo_verrano::completa::alisa_pixel`), la misma
//! que hace la CPU, y su JUEZ (`bmo_verrano::completa::del_bloque`) lo pasa
//! quien presta el bloque (el DIRECTOR): este crate no tiene dependencias.
//!
//! capa: puro -- el programa, el mapa y los parametros; la RAM, la IOMMU y
//! los registros los toca el kernel (`gpu_trabajo/completa.rs`)
//!
//! [eje]     RENDIMIENTO -- lo que en la CPU cuesta mas que la luz (25
//!           vecinos por pixel y pasada: 26 ms en 4 nucleos a 320x240), en
//!           la 3060 es un despacho por pasada
//! [riesgo]  ESPEJO    lo que escribe la 3060 lo juzga la cuenta de VERRANO
//!           en la CPU (`bmo_verrano::completa::del_bloque`), pixel a pixel
//!
//! Lo NUEVO: las cuentas de coma flotante (FADD, FMUL, FMNMX, FSETP)
//! corrieron en el metal en los cuerpos de PROTON-X (E5), no en un programa
//! de computo: este es el primero. Las mismas palabras; lo que cambia es el
//! motor que las lanza.
//!
//! # El bloque
//!
//! Un bloque de quien lo pide, prestado ENTERO a la 3060 (para escribir) en
//! el mapa de `video` ([`crate::video::VA`]), con las tres zonas del bloque
//! de VERRANO (`bmo_verrano::completa::BYTES_POR_PIXEL`; la prueba mira que
//! sean las mismas):
//!
//! ```text
//!    0          la luz A    16 B por pixel (r, g, b, nada)
//!    16 n       la guia     32 B por pixel (normal, distancia, color del
//!                           material, nada)
//!    48 n       la luz B    16 B por pixel
//! ```
//!
//! Una pasada lee una luz y escribe la otra ([`parametros`]): tres pasadas,
//! A -> B -> A -> B. 64 B por pixel: 320 x 240 son 4,9 MB, y lo mas que cabe
//! en el mapa (8 MiB) son 131 072 pixeles.
//!
//! # El programa (`ptxas -arch=sm_86`, CUDA 12.9: 40 registros, 159 instrucciones)
//!
//! Fabricado en la nube el 11-10 con el `ptxas` 12.9 y el `nvdisasm` 13.4 de
//! PyPI (los de `imagen`). Como todos: `c[0x0][0x28]` y los `ULDC.64 ..,
//! c[0x0][0x118]` en NOP con sus bits de planificacion, y los LDG/STG sin
//! descriptor. El PTX esta escrito para quedarse en lo que el juez conoce:
//!
//! - cada operacion con `.rn` (`mul.rn`, `add.rn`): ptxas NO las junta en
//!   FFMA, que en el computo aun no corrio en el metal, y cada una redondea
//!   como la de la CPU;
//! - los contadores de los bucles empiezan en un cero que ptxas no puede
//!   probar (`x >> 31`): son de cada hilo y no van al camino uniforme (UR,
//!   UIMAD, PLOP3: nada de eso ha corrido aqui);
//! - el nucleo (1/16, 1/4, 3/8) se elige con SEL sobre sus BITS (no FSEL);
//! - fuera de la imagen o el centro: UNA comparacion sin signo;
//! - las dos divisiones, `rcp.approx.ftz` (MUFU.RCP) y un producto: los
//!   divisores nunca son subnormales (el de cada vecino, >= 1e-6; el de la
//!   suma de pesos, >= 0,14).
//!
//! Por eso la 3060 no da los MISMOS bits que la CPU (MUFU.RCP no es la
//! division de IEEE): el juez acepta [`TOLERANCIA`] relativa.
//!
//! ```text
//! .version 7.1
//! .target sm_86
//! .address_size 64
//! .visible .entry completa()
//! {
//!   .reg .b32 %r<40>;
//!   .reg .f32 %f<80>;
//!   .reg .b64 %rd<20>;
//!   .reg .pred %p<12>;
//!   // los 11 parametros, en VA 0x2_0000_E840
//!   mov.b64 %rd10, {59456, 2};
//!   ld.volatile.global.u32 %r1, [%rd10];
//!   ld.volatile.global.u32 %r2, [%rd10+4];
//!   ld.volatile.global.u32 %r3, [%rd10+8];
//!   ld.volatile.global.u32 %r4, [%rd10+12];
//!   ld.volatile.global.u32 %r5, [%rd10+16];
//!   ld.volatile.global.u32 %r6, [%rd10+20];
//!   ld.volatile.global.u32 %r7, [%rd10+24];
//!   ld.volatile.global.u32 %r8, [%rd10+28];
//!   ld.volatile.global.u32 %r9, [%rd10+32];
//!   ld.volatile.global.f32 %f1, [%rd10+36];
//!   ld.volatile.global.f32 %f2, [%rd10+40];
//!   // un hilo por pixel: x = bloque.x 256 + hilo, y = bloque.y
//!   mov.u32 %r10, %tid.x;
//!   mov.u32 %r11, %ctaid.x;
//!   mad.lo.u32 %r12, %r11, 256, %r10;
//!   setp.ge.u32 %p1, %r12, %r7;
//!   @%p1 bra FIN;
//!   mov.u32 %r13, %ctaid.y;
//!   setp.ge.u32 %p1, %r13, %r8;
//!   @%p1 bra FIN;
//!   mad.lo.u32 %r14, %r13, %r7, %r12;
//!   mov.b64 %rd1, {%r1, %r2};
//!   mov.b64 %rd2, {%r3, %r4};
//!   mov.b64 %rd3, {%r5, %r6};
//!   // la guia del centro (8 f32 por pixel) y su luz (4 f32)
//!   mul.wide.u32 %rd4, %r14, 32;
//!   add.s64 %rd5, %rd2, %rd4;
//!   ld.global.f32 %f3, [%rd5];
//!   ld.global.f32 %f4, [%rd5+4];
//!   ld.global.f32 %f5, [%rd5+8];
//!   ld.global.f32 %f6, [%rd5+12];
//!   ld.global.f32 %f7, [%rd5+16];
//!   ld.global.f32 %f8, [%rd5+20];
//!   ld.global.f32 %f9, [%rd5+24];
//!   mul.wide.u32 %rd6, %r14, 16;
//!   add.s64 %rd7, %rd1, %rd6;
//!   ld.global.f32 %f10, [%rd7];
//!   ld.global.f32 %f11, [%rd7+4];
//!   ld.global.f32 %f12, [%rd7+8];
//!   // lp
//!   mul.rn.f32 %f13, %f10, 0f3E59B3D0;
//!   mul.rn.f32 %f14, %f11, 0f3F371759;
//!   add.rn.f32 %f15, %f13, %f14;
//!   mul.rn.f32 %f16, %f12, 0f3D93DD98;
//!   add.rn.f32 %f17, %f15, %f16;
//!   // el centro, con su peso entero (3/8 * 3/8)
//!   mov.f32 %f24, 0f3E100000;
//!   mul.rn.f32 %f19, %f10, %f24;
//!   mul.rn.f32 %f20, %f11, %f24;
//!   mul.rn.f32 %f21, %f12, %f24;
//!   // un cero que ptxas no puede probar (x < 2^31 siempre: x < ancho): los
//!   // contadores son de CADA hilo y no van al camino uniforme (UR)
//!   shr.u32 %r30, %r12, 31;
//!   mov.u32 %r20, %r30;
//! FILA:
//!   .pragma "nounroll";
//!   add.s32 %r21, %r20, -2;
//!   mad.lo.s32 %r22, %r21, %r9, %r13;
//!   // fuera de la imagen (un negativo, sin signo, es enorme): una comparacion
//!   setp.ge.u32 %p2, %r22, %r8;
//!   @%p2 bra SIG_FILA;
//!   abs.s32 %r23, %r21;
//!   setp.eq.u32 %p4, %r23, 0;
//!   setp.eq.u32 %p5, %r23, 1;
//!   selp.b32 %r31, 1048576000, 1031798784, %p5;
//!   selp.b32 %r31, 1052770304, %r31, %p4;
//!   mov.b32 %f22, %r31;
//!   mov.u32 %r24, %r30;
//! COL:
//!   .pragma "nounroll";
//!   add.s32 %r25, %r24, -2;
//!   mad.lo.s32 %r26, %r25, %r9, %r12;
//!   // el centro ya se conto: como si estuviera fuera
//!   or.b32 %r27, %r21, %r25;
//!   setp.eq.u32 %p8, %r27, 0;
//!   selp.b32 %r33, -1, %r26, %p8;
//!   setp.ge.u32 %p6, %r33, %r7;
//!   @%p6 bra SIG_COL;
//!   mad.lo.u32 %r28, %r22, %r7, %r26;
//!   mul.wide.u32 %rd8, %r28, 32;
//!   add.s64 %rd9, %rd2, %rd8;
//!   // otro material: fuera
//!   ld.global.f32 %f30, [%rd9+16];
//!   ld.global.f32 %f31, [%rd9+20];
//!   ld.global.f32 %f32, [%rd9+24];
//!   sub.rn.f32 %f33, %f7, %f30;
//!   abs.f32 %f33, %f33;
//!   setp.ge.f32 %p9, %f33, 0f3D4CCCCD;
//!   sub.rn.f32 %f34, %f8, %f31;
//!   abs.f32 %f34, %f34;
//!   setp.ge.f32 %p10, %f34, 0f3D4CCCCD;
//!   or.pred %p9, %p9, %p10;
//!   sub.rn.f32 %f35, %f9, %f32;
//!   abs.f32 %f35, %f35;
//!   setp.ge.f32 %p10, %f35, 0f3D4CCCCD;
//!   or.pred %p9, %p9, %p10;
//!   @%p9 bra SIG_COL;
//!   // la normal: coseno^32; la distancia: 1 + (|dz| lejos)^2
//!   ld.global.f32 %f40, [%rd9];
//!   ld.global.f32 %f41, [%rd9+4];
//!   ld.global.f32 %f42, [%rd9+8];
//!   ld.global.f32 %f43, [%rd9+12];
//!   mul.rn.f32 %f44, %f3, %f40;
//!   mul.rn.f32 %f45, %f4, %f41;
//!   add.rn.f32 %f46, %f44, %f45;
//!   mul.rn.f32 %f47, %f5, %f42;
//!   add.rn.f32 %f48, %f46, %f47;
//!   max.f32 %f48, %f48, 0f00000000;
//!   mul.rn.f32 %f48, %f48, %f48;
//!   mul.rn.f32 %f48, %f48, %f48;
//!   mul.rn.f32 %f48, %f48, %f48;
//!   mul.rn.f32 %f48, %f48, %f48;
//!   mul.rn.f32 %f48, %f48, %f48;
//!   setp.eq.f32 %p10, %f48, 0f00000000;
//!   @%p10 bra SIG_COL;
//!   sub.rn.f32 %f49, %f6, %f43;
//!   abs.f32 %f49, %f49;
//!   mul.rn.f32 %f49, %f49, %f1;
//!   mul.rn.f32 %f50, %f49, %f49;
//!   add.rn.f32 %f50, %f50, 0f3F800000;
//!   // la luz del vecino y su freno
//!   mul.wide.u32 %rd11, %r28, 16;
//!   add.s64 %rd12, %rd1, %rd11;
//!   ld.global.f32 %f51, [%rd12];
//!   ld.global.f32 %f52, [%rd12+4];
//!   ld.global.f32 %f53, [%rd12+8];
//!   mul.rn.f32 %f54, %f51, 0f3E59B3D0;
//!   mul.rn.f32 %f55, %f52, 0f3F371759;
//!   add.rn.f32 %f56, %f54, %f55;
//!   mul.rn.f32 %f57, %f53, 0f3D93DD98;
//!   add.rn.f32 %f57, %f56, %f57;
//!   add.rn.f32 %f60, %f17, %f57;
//!   mul.rn.f32 %f60, %f2, %f60;
//!   mul.rn.f32 %f60, %f60, 0f3F000000;
//!   add.rn.f32 %f60, %f60, 0f3A83126F;
//!   mul.rn.f32 %f61, %f60, %f60;
//!   sub.rn.f32 %f62, %f17, %f57;
//!   // k del nucleo de esta columna
//!   abs.s32 %r29, %r25;
//!   setp.eq.u32 %p4, %r29, 0;
//!   setp.eq.u32 %p5, %r29, 1;
//!   selp.b32 %r32, 1048576000, 1031798784, %p5;
//!   selp.b32 %r32, 1052770304, %r32, %p4;
//!   mov.b32 %f23, %r32;
//!   // wq = ((kj kk) arriba m2) / (abajo (m2 + d^2))
//!   mul.rn.f32 %f63, %f22, %f23;
//!   mul.rn.f32 %f63, %f63, %f48;
//!   mul.rn.f32 %f63, %f63, %f61;
//!   mul.rn.f32 %f64, %f62, %f62;
//!   add.rn.f32 %f64, %f61, %f64;
//!   mul.rn.f32 %f64, %f50, %f64;
//!   rcp.approx.ftz.f32 %f65, %f64;
//!   mul.rn.f32 %f66, %f63, %f65;
//!   mul.rn.f32 %f67, %f51, %f66;
//!   add.rn.f32 %f19, %f19, %f67;
//!   mul.rn.f32 %f68, %f52, %f66;
//!   add.rn.f32 %f20, %f20, %f68;
//!   mul.rn.f32 %f69, %f53, %f66;
//!   add.rn.f32 %f21, %f21, %f69;
//!   add.rn.f32 %f24, %f24, %f66;
//! SIG_COL:
//!   add.u32 %r24, %r24, 1;
//!   setp.lt.u32 %p11, %r24, 5;
//!   @%p11 bra COL;
//! SIG_FILA:
//!   add.u32 %r20, %r20, 1;
//!   setp.lt.u32 %p11, %r20, 5;
//!   @%p11 bra FILA;
//!   // lo que sale: la suma entre los pesos
//!   rcp.approx.ftz.f32 %f70, %f24;
//!   mul.rn.f32 %f71, %f19, %f70;
//!   mul.rn.f32 %f72, %f20, %f70;
//!   mul.rn.f32 %f73, %f21, %f70;
//!   add.s64 %rd13, %rd3, %rd6;
//!   st.global.f32 [%rd13], %f71;
//!   st.global.f32 [%rd13+4], %f72;
//!   st.global.f32 [%rd13+8], %f73;
//! FIN:
//!   ret;
//! }
//! ```
//!
//! # Como se sabe
//!
//! Aqui: el juez del SASS lo da por bueno, el programa lee sus 11 parametros
//! donde van, y el mapa es el de VERRANO. En el metal (`gpu completa` en el
//! DIRECTOR): la CPU rehace TODOS los pixeles de cada pasada con la cuenta de
//! VERRANO y los compara ([`igual`]).

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::sombreador::{ordenes_con, qmd_rejilla, EMPUJE, ORDENES, PROGRAMA, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::Registros;

pub use crate::video::{IOVA, VA};

pub const HILOS: u32 = 256;
/// 40 usados y los 2 del contador de programa, redondeado a 8.
pub const REGISTROS: u32 = 48;
pub const PAGA_QMD: u32 = 0x3060_5E10;
pub const PAGA_FIN: u32 = 0x3060_5EF0;
/// Detras de la libreta (+0x700): la pagina de semaforos, de aqui arriba,
/// estaba libre.
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x800;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x810;
/// Donde lee el programa sus 11 parametros (VA 0x2_0000_E840).
pub const PARAMETROS: u64 = SEMAFOROS + 0x840;
pub const N_PARAMETROS: usize = 11;
/// Lo que se perdona entre la 3060 y la CPU, relativo: MUFU.RCP se aparta
/// ~1 ulp (1,2e-7) por division, y hay hasta 25 por pixel.
pub const TOLERANCIA: f32 = 2e-5;
/// El paso mas largo (los vecinos a `2^paso`).
pub const MAX_PASO: u32 = 7;

/// **El formato**: las medidas de la imagen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Formato {
    pub ancho: u32,
    pub alto: u32,
}

impl Formato {
    /// La de ESTUDIO VIVO en BMO-X.
    pub const ESTUDIO: Formato = Formato { ancho: 320, alto: 240 };

    pub const fn pixeles(&self) -> u64 {
        self.ancho as u64 * self.alto as u64
    }

    /// El bloque entero: 64 B por pixel.
    pub const fn bytes(&self) -> u64 {
        64 * self.pixeles()
    }

    pub const fn paginas(&self) -> u64 {
        self.bytes().div_ceil(4096)
    }

    /// Que no este vacio y quepa en el mapa de `video`.
    pub const fn valido(&self) -> bool {
        self.ancho >= 1 && self.alto >= 1 && self.ancho <= 4096 && self.alto <= 4096 && self.paginas() * 4096 <= crate::video::MAX_BYTES
    }

    /// Bloques de [`HILOS`] por fila.
    pub const fn bloques(&self) -> u32 {
        self.ancho.div_ceil(HILOS)
    }

    /// Donde empieza cada zona, en bytes desde el bloque.
    pub const fn luz_a(&self) -> u64 {
        0
    }
    pub const fn guia(&self) -> u64 {
        16 * self.pixeles()
    }
    pub const fn luz_b(&self) -> u64 {
        48 * self.pixeles()
    }
}

/// **La tolerancia de la luz**, como viaja en la orden: los 16 bits de
/// arriba de su f32 (3 cifras: es un freno, no una medida). La CPU usa la
/// MISMA, ya cortada.
pub const fn tolerancia_corta(t: f32) -> u16 {
    (t.to_bits() >> 16) as u16
}
pub const fn tolerancia_de(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

/// Los 11 parametros de una pasada: la luz que se lee, la guia, la luz que
/// se escribe, las medidas, el paso (`2^paso`), lo lejos (24 / paso, como la
/// CPU) y la tolerancia. `de_b`: se lee la B y se escribe la A.
pub fn parametros(f: &Formato, paso: u32, de_b: bool, tolerancia: f32) -> [u32; N_PARAMETROS] {
    let (lee, escribe) = if de_b { (f.luz_b(), f.luz_a()) } else { (f.luz_a(), f.luz_b()) };
    let (l, g, e) = (VA + lee, VA + f.guia(), VA + escribe);
    let s = 1u32 << paso;
    let lejos = 24.0 / s as f32;
    [l as u32, (l >> 32) as u32, g as u32, (g >> 32) as u32, e as u32, (e >> 32) as u32, f.ancho, f.alto, s, lejos.to_bits(), tolerancia.to_bits()]
}

/// Si lo de la 3060 vale por lo del juez: cada canal, a [`TOLERANCIA`]
/// relativa (o casi cero los dos).
pub fn igual(gpu: [f32; 3], cpu: [f32; 3]) -> bool {
    (0..3).all(|k| {
        let (a, b) = (gpu[k], cpu[k]);
        a.is_finite() && (a - b).abs() <= TOLERANCIA * b.abs().max(1e-6)
    })
}

/// **El programa**, como lo leyo `nvdisasm` (el PTX de origen, arriba).
pub const CODIGO: [(u64, u64); 159] = [
    (0x0000000000007918, 0x000fe40000000000), // NOP (era MOV R1, c[0x0][0x28])
    (0x0000000000087802, 0x000fe20000000f00), // MOV R8, 0x0
    (0x0000000000007918, 0x000fe20000000000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x0000000200097802, 0x000fca0000000f00), // MOV R9, 0x2
    (0x00e8400408067981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R6, [R8.64+0xe840]
    (0x00e8440408077981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R7, [R8.64+0xe844]
    (0x00e8480408047981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R4, [R8.64+0xe848]
    (0x00e84c0408057981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R5, [R8.64+0xe84c]
    (0x00e8500408027981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R2, [R8.64+0xe850]
    (0x00e8540408037981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R3, [R8.64+0xe854]
    (0x00e8580408007981, 0x000ea8000c1f5900), // LDG.E.STRONG.SYS R0, [R8.64+0xe858]
    (0x00e85c04080c7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R12, [R8.64+0xe85c]
    (0x00e86004080e7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R14, [R8.64+0xe860]
    (0x00e86404080f7981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R15, [R8.64+0xe864]
    (0x00e8680408107981, 0x000168000c1f5900), // LDG.E.STRONG.SYS R16, [R8.64+0xe868]
    (0x00000000000d7919, 0x000e680000002100), // S2R R13, SR_TID.X
    (0x00000000000a7919, 0x000e640000002500), // S2R R10, SR_CTAID.X
    (0x0000000d0a0d7211, 0x002fc800078e40ff), // LEA R13, R10, R13, 0x8
    (0x000000000d00720c, 0x004fda0003f06070), // ISETP.GE.U32.AND P0, PT, R13, R0, PT
    (0x000000000000094d, 0x000fea0003800000), // @P0 EXIT
    (0x00000000000679c3, 0x001e240000002600), // S2UR UR6, SR_CTAID.Y
    (0x000000060c007c0c, 0x021fda000bf03070), // ISETP.LE.U32.AND P0, PT, R12, UR6, PT
    (0x000000000000094d, 0x000fea0003800000), // @P0 EXIT
    (0x0000000600117c24, 0x000fc8000f8e020d), // IMAD R17, R0, UR6, R13
    (0x00000010110a7825, 0x000fca00078e0006), // IMAD.WIDE.U32 R10, R17, 0x10, R6
    (0x000000040a1f7981, 0x000ea8000c1e1900), // LDG.E R31, [R10.64]
    (0x000004040a1e7981, 0x000ee2000c1e1900), // LDG.E R30, [R10.64+0x4]
    (0x0000002011087825, 0x000fc600078e0004), // IMAD.WIDE.U32 R8, R17, 0x20, R4
    (0x000008040a1c7981, 0x000f28000c1e1900), // LDG.E R28, [R10.64+0x8]
    (0x0000000408127981, 0x000168000c1e1900), // LDG.E R18, [R8.64]
    (0x0000040408137981, 0x000168000c1e1900), // LDG.E R19, [R8.64+0x4]
    (0x0000080408147981, 0x000168000c1e1900), // LDG.E R20, [R8.64+0x8]
    (0x00000c0408157981, 0x000168000c1e1900), // LDG.E R21, [R8.64+0xc]
    (0x0000100408167981, 0x000168000c1e1900), // LDG.E R22, [R8.64+0x10]
    (0x0000140408177981, 0x000168000c1e1900), // LDG.E R23, [R8.64+0x14]
    (0x0000180408187981, 0x000162000c1e1900), // LDG.E R24, [R8.64+0x18]
    (0x000006f000007945, 0x000fe20003800000), // BSSY B0, `(.L_x_0)
    (0x3e10000000207802, 0x000fe20000000f00), // MOV R32, 0x3e100000
    (0x3e59b3d01f197820, 0x044fe20000400000), // FMUL R25, R31.reuse, 0.21259999275207519531
    (0x3e1000001f1f7820, 0x000fe20000400000), // FMUL R31, R31, 0.140625
    (0x3f3717591e1a7820, 0x048fe20000400000), // FMUL R26, R30.reuse, 0.71520000696182250977
    (0x3e1000001e1e7820, 0x000fc60000400000), // FMUL R30, R30, 0.140625
    (0x0000001a19197221, 0x000fe20000000000), // FADD R25, R25, R26
    (0x3d93dd981c0a7820, 0x050fe20000400000), // FMUL R10, R28.reuse, 0.072200000286102294922
    (0x0000001fff1a7819, 0x000fe2000001160d), // SHF.R.U32.HI R26, RZ, 0x1f, R13
    (0x3e1000001c1c7820, 0x000fe40000400000), // FMUL R28, R28, 0.140625
    (0x0000000a19197221, 0x001fe20000000000), // FADD R25, R25, R10
    (0xfffffffe1a1d7810, 0x040fe20007ffe0ff), // IADD3 R29, R26.reuse, -0x2, RZ
    (0x0000061000017945, 0x000fe20003800000), // BSSY B1, `(.L_x_1)
    (0x000000011a1a7810, 0x000fc60007ffe0ff), // IADD3 R26, R26, 0x1, RZ
    (0x000000060e097e24, 0x000fe2000f8e021d), // IMAD R9, R14, R29, UR6
    (0x000000051a00780c, 0x000fc80003f26070), // ISETP.GE.U32.AND P1, PT, R26, 0x5, PT
    (0x0000000c0900720c, 0x000fda0003f06070), // ISETP.GE.U32.AND P0, PT, R9, R12, PT
    (0x000005b000000947, 0x000fea0003800000), // @P0 BRA `(.L_x_2)
    (0x0000001d00087213, 0x000fe40000000000), // IABS R8, R29
    (0x3e800000001b7802, 0x000fe40000000f00), // MOV R27, 0x3e800000
    (0x000000010800780c, 0x040fe40003f45070), // ISETP.NE.U32.AND P2, PT, R8.reuse, 0x1, PT
    (0x000000ff0800720c, 0x000fe40003f05070), // ISETP.NE.U32.AND P0, PT, R8, RZ, PT
    (0x3d8000001b1b7807, 0x000fe40005000000), // SEL R27, R27, 0x3d800000, !P2
    (0x0000001fff247819, 0x000fc4000001160d), // SHF.R.U32.HI R36, RZ, 0x1f, R13
    (0x3ec000001b1b7807, 0x000fe40000000000), // SEL R27, R27, 0x3ec00000, P0
    (0xfffffffe240a7810, 0x040fe20007ffe0ff), // IADD3 R10, R36.reuse, -0x2, RZ
    (0x0000051000027945, 0x000fe20003800000), // BSSY B2, `(.L_x_3)
    (0x0000000124247810, 0x000fe40007ffe0ff), // IADD3 R36, R36, 0x1, RZ
    (0x0000000a1dff7212, 0x080fe2000780fcff), // LOP3.LUT P0, RZ, R29, R10.reuse, RZ, 0xfc, !PT
    (0x0000000a0e0b7224, 0x000fe200078e020d), // IMAD R11, R14, R10, R13
    (0x000000052400780c, 0x000fc80003f46070), // ISETP.GE.U32.AND P2, PT, R36, 0x5, PT
    (0xffffffff0b097807, 0x000fc80000000000), // SEL R9, R11, 0xffffffff, P0
    (0x000000000900720c, 0x000fda0003f06070), // ISETP.GE.U32.AND P0, PT, R9, R0, PT
    (0x0000049000000947, 0x000fea0003800000), // @P0 BRA `(.L_x_4)
    (0x000000060e087e24, 0x000fe2000f8e021d), // IMAD R8, R14, R29, UR6
    (0x0000000000007918, 0x000fc60000000000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x00000008000b7224, 0x000fc800078e020b), // IMAD R11, R0, R8, R11
    (0x000000200b087825, 0x000fca00078e0004), // IMAD.WIDE.U32 R8, R11, 0x20, R4
    (0x0000140408227981, 0x000ea8000c1e1900), // LDG.E R34, [R8.64+0x14]
    (0x0000100408237981, 0x000ee8000c1e1900), // LDG.E R35, [R8.64+0x10]
    (0x0000180408217981, 0x000f22000c1e1900), // LDG.E R33, [R8.64+0x18]
    (0x8000002217227221, 0x024fe20000000000), // FADD R34, R23, -R34
    (0x8000002316237221, 0x008fc80000000000), // FADD R35, R22, -R35
    (0x3d4ccccd2200780b, 0x000fe20003f06200), // FSETP.GE.AND P0, PT, |R34|, 0.050000000745058059692, PT
    (0x8000002118217221, 0x010fc60000000000), // FADD R33, R24, -R33
    (0x3d4ccccd2300780b, 0x000fc80000706600), // FSETP.GE.OR P0, PT, |R35|, 0.050000000745058059692, P0
    (0x3d4ccccd2100780b, 0x000fda0000706600), // FSETP.GE.OR P0, PT, |R33|, 0.050000000745058059692, P0
    (0x000003b000000947, 0x000fea0003800000), // @P0 BRA `(.L_x_4)
    (0x0000000000007918, 0x000fe40000000000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x0000000408217981, 0x000ea8000c1e1900), // LDG.E R33, [R8.64]
    (0x0000040408227981, 0x000ee2000c1e1900), // LDG.E R34, [R8.64+0x4]
    (0x0000002112217220, 0x004fe20000400000), // FMUL R33, R18, R33
    (0x0000002213227220, 0x008fc80000400000), // FMUL R34, R19, R34
    (0x0000002221227221, 0x000fe40000000000), // FADD R34, R33, R34
    (0x0000080408217981, 0x000ea4000c1e1900), // LDG.E R33, [R8.64+0x8]
    (0x0000002114217220, 0x004fc80000400000), // FMUL R33, R20, R33
    (0x0000002122217221, 0x000fca0000000000), // FADD R33, R34, R33
    (0x00000021ff217209, 0x000fca0007800000), // FMNMX R33, RZ, R33, !PT
    (0x0000002121217220, 0x000fc80000400000), // FMUL R33, R33, R33
    (0x0000002121217220, 0x000fc80000400000), // FMUL R33, R33, R33
    (0x0000002121217220, 0x000fc80000400000), // FMUL R33, R33, R33
    (0x0000002121217220, 0x000fc80000400000), // FMUL R33, R33, R33
    (0x0000002121217220, 0x000fca0000400000), // FMUL R33, R33, R33
    (0x000000ff2100720b, 0x000fda0003f0d000), // FSETP.NEU.AND P0, PT, R33, RZ, PT
    (0x000002a000008947, 0x000fea0003800000), // @!P0 BRA `(.L_x_4)
    (0x0000000a000a7213, 0x000fe20000000000), // IABS R10, R10
    (0x0000000000007918, 0x000fe40000000000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x00000c0408087981, 0x000ea2000c1e1900), // LDG.E R8, [R8.64+0xc]
    (0x000000ff0a00720c, 0x040fe40003f05070), // ISETP.NE.U32.AND P0, PT, R10.reuse, RZ, PT
    (0x000000010a00780c, 0x000fe20003f65070), // ISETP.NE.U32.AND P3, PT, R10, 0x1, PT
    (0x000000100b0a7825, 0x000fca00078e0006), // IMAD.WIDE.U32 R10, R11, 0x10, R6
    (0x000000040a237981, 0x000ee8000c1e1900), // LDG.E R35, [R10.64]
    (0x000004040a227981, 0x000f28000c1e1900), // LDG.E R34, [R10.64+0x4]
    (0x000008040a257981, 0x000962000c1e1900), // LDG.E R37, [R10.64+0x8]
    (0x8000000815087221, 0x004fc80000000000), // FADD R8, R21, -R8
    (0x400000080f087220, 0x000fc80000400000), // FMUL R8, R15, |R8|
    (0x0000000808087220, 0x000fe20000400000), // FMUL R8, R8, R8
    (0x3e59b3d023097820, 0x008fc60000400000), // FMUL R9, R35, 0.21259999275207519531
    (0x3f80000008087421, 0x000fe20000000000), // FADD R8, R8, 1
    (0x3f371759220a7820, 0x010fc80000400000), // FMUL R10, R34, 0.71520000696182250977
    (0x0000000a09097221, 0x000fe20000000000), // FADD R9, R9, R10
    (0x3d93dd98250a7820, 0x020fc80000400000), // FMUL R10, R37, 0.072200000286102294922
    (0x0000000a09097221, 0x000fe20000000000), // FADD R9, R9, R10
    (0x3e800000000a7802, 0x000fc60000000f00), // MOV R10, 0x3e800000
    (0x00000009190b7221, 0x140fe20000000000), // FADD R11, R25.reuse, R9.reuse
    (0x3d8000000a0a7807, 0x000fe20005800000), // SEL R10, R10, 0x3d800000, !P3
    (0x8000000919097221, 0x000fe40000000000), // FADD R9, R25, -R9
    (0x0000000b100b7220, 0x000fe20000400000), // FMUL R11, R16, R11
    (0x3ec000000a0a7807, 0x000fe20000000000), // SEL R10, R10, 0x3ec00000, P0
    (0x0000000909097220, 0x000fc80000400000), // FMUL R9, R9, R9
    (0x0000000a1b0a7220, 0x000fc80000400000), // FMUL R10, R27, R10
    (0x0000000a21217220, 0x000fe20000400000), // FMUL R33, R33, R10
    (0x3f0000000b0a7820, 0x000fc80000400000), // FMUL R10, R11, 0.5
    (0x3a83126f0a0a7421, 0x000fc80000000000), // FADD R10, R10, 0.0010000000474974513054
    (0x0000000a0a0a7220, 0x000fc80000400000), // FMUL R10, R10, R10
    (0x000000090a097221, 0x040fe20000000000), // FADD R9, R10.reuse, R9
    (0x000000210a217220, 0x000fc60000400000), // FMUL R33, R10, R33
    (0x0000000908087220, 0x000fcc0000400000), // FMUL R8, R8, R9
    (0x0000000800087308, 0x000e240000001000), // MUFU.RCP R8, R8
    (0x0000000821217220, 0x001fc80000400000), // FMUL R33, R33, R8
    (0x00000021230a7220, 0x080fe20000400000), // FMUL R10, R35, R33.reuse
    (0x0000002122097220, 0x080fe20000400000), // FMUL R9, R34, R33.reuse
    (0x0000002125257220, 0x000fe20000400000), // FMUL R37, R37, R33
    (0x0000002120207221, 0x000fe20000000000), // FADD R32, R32, R33
    (0x0000000a1f1f7221, 0x000fe20000000000), // FADD R31, R31, R10
    (0x000000091e1e7221, 0x000fe20000000000), // FADD R30, R30, R9
    (0x000000251c1c7221, 0x000fe40000000000), // FADD R28, R28, R37
    (0x0000000000027941, 0x000fea0003800000), // BSYNC B2
    (0xfffffac00000a947, 0x000fea000383ffff), // @!P2 BRA `(.L_x_5)
    (0x0000000000017941, 0x000fea0003800000), // BSYNC B1
    (0xfffff9c000009947, 0x000fea000383ffff), // @!P1 BRA `(.L_x_6)
    (0x0000000000007941, 0x000fea0003800000), // BSYNC B0
    (0x0000002000057308, 0x000e220000001000), // MUFU.RCP R5, R32
    (0x0000001011027825, 0x000fe200078e0002), // IMAD.WIDE.U32 R2, R17, 0x10, R2
    (0x0000000000007918, 0x000fe20000000000), // NOP (era ULDC.64 UR4, c[0x0][0x118])
    (0x000000051f1f7220, 0x081fe20000400000), // FMUL R31, R31, R5.reuse
    (0x000000051e077220, 0x080fe20000400000), // FMUL R7, R30, R5.reuse
    (0x000000051c057220, 0x000fc60000400000), // FMUL R5, R28, R5
    (0x0000001f02007986, 0x000fe8000c101904), // STG.E [R2.64], R31
    (0x0000040702007986, 0x000fe8000c101904), // STG.E [R2.64+0x4], R7
    (0x0000080502007986, 0x000fe2000c101904), // STG.E [R2.64+0x8], R5
    (0x000000000000794d, 0x000fea0003800000), // EXIT
    (0xfffffff000007947, 0x000fc0000383ffff), // BRA `(.L_x_7)
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

/// Un hilo por pixel: `bloques` x `alto` bloques de 256.
pub fn qmd(f: &Formato) -> [u32; QMD_PALABRAS] {
    qmd_rejilla(sombreador_va(PROGRAMA), HILOS, f.bloques(), f.alto, sombreador_va(SEMAFORO_QMD), PAGA_QMD, REGISTROS)
}

pub fn ordenes() -> [u32; ORDENES] {
    ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN)
}

/// **Preparar una pasada** con la entrada `e` del GPFIFO de GR, como
/// `imagen::preparar`: `todo` carga tambien el programa, el QMD y las
/// ordenes (la pagina del programa es de quien la uso el ultimo).
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

/// Sano: la pasada se lanzo y los dos semaforos se pagaron (lo que escribio
/// lo juzga quien presto el bloque).
pub const fn sano(v: u64) -> bool {
    let (_, qmd, fin, lanzado, _, _) = desempaquetar(v);
    qmd && fin && lanzado
}

const _: () = assert!(PALABRAS_CODIGO * 4 <= 4096);
const _: () = assert!(SEMAFORO_QMD > crate::libreta::LIBRETA && PARAMETROS >= SEMAFORO_FIN + 16);
const _: () = assert!(PARAMETROS + 4 * N_PARAMETROS as u64 <= SEMAFOROS + 0x1000);
const _: () = assert!(crate::lienzo::sombreador_va(PARAMETROS) == 0x2_0000_E840);
const _: () = assert!(Formato::ESTUDIO.valido());

#[cfg(test)]
mod pruebas {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    /// El mapa es el bloque de VERRANO: las mismas zonas, los mismos bytes.
    #[test]
    fn el_mapa_es_el_bloque_de_verrano() {
        use bmo_verrano::completa as v;
        let f = Formato::ESTUDIO;
        let n = f.pixeles() as usize;
        assert_eq!(f.bytes(), v::BYTES_POR_PIXEL * f.pixeles());
        assert_eq!((f.luz_a(), f.guia(), f.luz_b()), (4 * v::luz_a(n) as u64, 4 * v::guia(n) as u64, 4 * v::luz_b(n) as u64));
    }

    /// *** Lo que hace la pasada lo dice la cuenta de VERRANO: con la guia de
    /// una pared y un suelo y luz con grano, el borde se queda.
    #[test]
    fn la_cuenta_de_verrano_guarda_el_borde() {
        use bmo_verrano::completa as v;
        let f = Formato { ancho: 16, alto: 8 };
        let n = f.pixeles() as usize;
        let mut b = std::vec![0.0f32; 16 * n];
        for i in 0..n {
            let pared = (i as u32 % f.ancho) < 8;
            let albedo = if pared { [0.65, 0.05, 0.05] } else { [0.73; 3] };
            v::pon_guia(&mut b, n, i, &v::Guia { n: if pared { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }, z: 2.0, albedo });
            v::pon_luz(&mut b, n, false, i, albedo.map(|a| a * if i % 3 == 0 { 2.0 } else { 0.8 }));
        }
        let pared = v::del_bloque(&b, f.ancho, f.alto, 1, false, 2.0, 7, 4);
        assert!(pared[0] > 5.0 * pared[1], "{pared:?}");
    }

    #[test]
    fn el_mapa_y_los_parametros() {
        let f = Formato::ESTUDIO;
        assert_eq!(f.bytes(), 4_915_200);
        assert_eq!(f.paginas(), 1200);
        assert!(!Formato { ancho: 640, alto: 480 }.valido(), "640x480 no cabe en los 8 MiB");
        assert!(!Formato { ancho: 0, alto: 10 }.valido());
        let p = parametros(&f, 2, false, 0.75);
        assert_eq!((p[0] as u64) | (p[1] as u64) << 32, VA);
        assert_eq!((p[2] as u64) | (p[3] as u64) << 32, VA + 16 * 76_800);
        assert_eq!((p[4] as u64) | (p[5] as u64) << 32, VA + 48 * 76_800);
        assert_eq!(&p[6..9], &[320, 240, 4]);
        assert_eq!(f32::from_bits(p[9]), 6.0);
        assert_eq!(f32::from_bits(p[10]), 0.75);
        let q = parametros(&f, 0, true, 0.75);
        assert_eq!(((q[0] as u64) | (q[1] as u64) << 32, (q[4] as u64) | (q[5] as u64) << 32), (VA + 48 * 76_800, VA), "de B a A");
        assert_eq!(tolerancia_de(tolerancia_corta(0.75)), 0.75);
        assert!((tolerancia_de(tolerancia_corta(1.414_213_5)) - 1.414_213_5).abs() < 0.01);
    }

    #[test]
    fn igual_perdona_lo_de_mufu_y_nada_mas() {
        assert!(igual([1.0, 0.5, 0.25], [1.0 + 1e-6, 0.5, 0.25]));
        assert!(!igual([1.0, 0.5, 0.25], [1.001, 0.5, 0.25]));
        assert!(!igual([f32::NAN, 0.5, 0.25], [1.0, 0.5, 0.25]));
        assert!(igual([0.0; 3], [0.0; 3]));
    }

    #[test]
    fn el_programa_lee_sus_parametros_y_escribe_tres() {
        let fuertes: Vec<u64> = CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x981 && c.1 & 0xFFFF == 0x5900).map(|c| c.0 >> 40 & 0xFFFF).collect();
        assert_eq!(fuertes, (0..11).map(|k| 0xE840 + 4 * k).collect::<Vec<u64>>(), "los 11 parametros, en su sitio");
        assert_eq!(CODIGO.iter().filter(|c| c.0 & 0xFFF == 0x986).count(), 3, "r, g, b");
        assert!(CODIGO.iter().filter(|c| matches!(c.0 & 0xFFF, 0x981 | 0x986)).all(|c| c.1 >> 37 & 1 == 0), "sin descriptor");
        let nops: Vec<usize> = CODIGO.iter().enumerate().filter(|(_, c)| c.0 == 0x7918).map(|(k, _)| k).collect();
        assert_eq!(nops.len(), 6, "las seis lecturas de CUDA (c[0x0][0x28] y cinco ULDC), en NOP");
        assert!(nops.iter().all(|&k| CODIGO[k].1 & ((1 << 41) - 1) == 0), "con solo sus bits de planificacion");
        assert_eq!(CODIGO[CODIGO.len() - 2].0 & 0xFFF, 0x94D, "acaba en EXIT");
        assert!(CODIGO.iter().all(|c| c.0 & 0xFFF != 0x223 && c.0 & 0xFFF != 0x823), "sin FFMA");
    }

    #[test]
    fn el_qmd_es_un_hilo_por_pixel() {
        let q = qmd(&Formato::ESTUDIO);
        let r = qmd_rejilla(sombreador_va(PROGRAMA), 256, 2, 240, sombreador_va(SEMAFORO_QMD), PAGA_QMD, 48);
        assert_eq!(q, r);
    }

    #[test]
    fn el_juez_del_sass_lo_da_por_bueno() {
        let ctx = crate::sass::juez::Contexto { registros: REGISTROS, sph: None };
        let v = crate::sass::juez::juzgar(&CODIGO, &ctx).unwrap_or_else(|b| panic!("TOMA TU BODRIO: {b:?}"));
        assert_eq!(v.instrucciones, CODIGO.len() - 1);
    }
}
