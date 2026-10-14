//! **EL COMPUTO DE UNA APP: su QMD y su pegado** (LI2f, 13-10) -- lo puro del
//! despacho de un cuerpo de computo que manda una app (el que emite
//! `bmo-proton-x-sm86` con `Abi::Registros`): el QMD de su rejilla y su
//! grupo, con su memoria compartida, y el PEGADO -- las precargas del cuerpo
//! puestas con un MOV cada una, con el valor que sabe el KERNEL al despachar
//! (la VA y el tope de cada UAV y SRV, sus medidas, las filas del cbuffer)
//! --, y detras el cuerpo, con su EXIT.
//!
//! capa: puro -- palabras y campos; la RAM, la IOMMU y el GPFIFO los toca el
//! kernel
//!
//! ```text
//!    el pegado   MOV Rk, valor   (una por precarga, cada una con 6 ciclos:
//!                                 el cuerpo empieza con todo escrito)
//!                el cuerpo       (juzgado: R0..R6, y R7 con sus permisos)
//!    el QMD      V03_00 (`motores::sombreador::qmd_rejilla`), y ademas:
//!                CTA_RASTER_* = los grupos (x, y, z)
//!                CTA_THREAD_DIMENSION* = los hilos del grupo
//!                SHARED_MEMORY_SIZE (561:544) = la compartida, a 256 B
//!                *_SM_CONFIG_SHARED_MEM_SIZE = la configuracion de la SM que
//!                la cabe, mas el KiB que Ampere reserva por grupo: 8, 16, 32
//!                o 64 KiB, en (KiB / 4) + 1 (la cuenta de NVK,
//!                `gv100_sm_config_smem_size`)
//! ```
//!
//! Los techos son los de D3D12 (y los de la 3060): hasta 1024 hilos por
//! grupo (1024 x 1024 x 64), 65535 grupos por eje, y 32 KiB de compartida.
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL: el primer despacho de una app lo
//! dira. El QMD de la compartida y de la rejilla en 3D es nuevo; el resto,
//! el de `blur` y `fractal`, que si corrieron.

use crate::canal::GR;
use crate::copia::{entrada, escribir, invalidar, leer32, GP_GET, GP_PUT, TIMBRE};
use crate::lienzo::sombreador_va;
use crate::motores::sombreador::{campo, ordenes_con, qmd_rejilla, EMPUJE, ORDENES, QMD, QMD_PALABRAS, SEMAFOROS};
use crate::trabajos::cubo::mov;
use crate::vram::{CODIGO_COMPUTO, CODIGO_COMPUTO_PAGINAS, CODIGO_COMPUTO_VA};
use crate::Registros;

/// Los semaforos del despacho, en la pagina de los semaforos (libre desde el
/// 0x870: `completa` acaba sus parametros en el 0x86B), y lo que pagan.
pub const SEMAFORO_QMD: u64 = SEMAFOROS + 0x880;
pub const SEMAFORO_FIN: u64 = SEMAFOROS + 0x890;
pub const PAGA_QMD: u32 = 0x3060_C510;
pub const PAGA_FIN: u32 = 0x3060_C5F0;
/// Las instrucciones del pegado, como mucho (las de su region).
pub const MAX_PEGADO: usize = CODIGO_COMPUTO_PAGINAS * 256;
const _: () = assert!(crate::receta_computo::MAX_PEGADO <= MAX_PEGADO);
/// Los registros del banco de una SM: los hilos de un grupo por los
/// registros de cada uno no pueden pasar (si no, el grupo no se lanza).
pub const BANCO_DE_REGISTROS: u32 = 64 * 1024;

/// **Lo que el KERNEL pone de cada UAV o SRV de bufer** en los registros que
/// el cuerpo pidio (`Precarga::Uav` de `bmo-proton-x-sm86`, su `uav.rs`):
///
/// ```text
///    Base       el par (lo, hi): la VA del bufer en la 3060, alineada
///    Tope       la mayor direccion de palabra que se puede tocar, desde la
///               base: lo prestado menos 4 (alineado). LA SEGURIDAD
///    Limite     crudo: las palabras que valen empiezan antes de esto
///    Elementos  estructurado: los elementos (0 si el paso es 0)
///    Paso       estructurado: los bytes de un elemento
///    Medida     lo que dice `GetDimensions`
/// ```
///
/// ** L3 (14-10): un UAV CON TIPO (`RWBuffer<T>`) o DE TEXTURA
/// (`RWTexture2D<T>`), en las palabras en que lo guarda la casa
/// (`bufer::Uav`): la [`Forma`] de la vista, por despacho, y de ella
///
/// ```text
///    Ancho, Alto   la textura (x < Ancho, y < Alto)
///    Texeles       los elementos que caben enteros en lo prestado
///    Salto         los bytes de un elemento (16 en cuatro floats, 4n si no)
///    Canales       los de 32 bits que guarda (1..4)
///    Alfa          el canal 3 que no se guarda: 1.0, o 1 en uno de enteros
///    Relleno       las palabras de un elemento en cuatro floats que no son
///                  canales (bit j): la casa las reescribe en cada escritura
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DelUav {
    Base = 0,
    Tope = 1,
    Limite = 2,
    Elementos = 3,
    Paso = 4,
    /// Lo que dice `GetDimensions`: los elementos de la vista (en un crudo,
    /// sus bytes), sin mirar lo prestado.
    Medida = 5,
    Ancho = 6,
    Alto = 7,
    Texeles = 8,
    Salto = 9,
    Canales = 10,
    Alfa = 11,
    Relleno = 12,
}

/// ** L3: **la forma de una vista con tipo**, en un byte (el 21 de un bufer
/// de VRC1): los canales de 32 bits (1..4) en 0..3, si va en CUATRO floats
/// por elemento (el 3), y si es de enteros (el 4). 0: sin tipo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Forma(pub u8);

impl Forma {
    pub const SIN_TIPO: Forma = Forma(0);

    pub const fn nueva(canales: u8, cuatro: bool, entero: bool) -> Forma {
        Forma(canales & 7 | (cuatro as u8) << 3 | (entero as u8) << 4)
    }
    pub const fn canales(self) -> u32 {
        (self.0 & 7) as u32
    }
    pub const fn cuatro(self) -> bool {
        self.0 & 8 != 0
    }
    pub const fn entero(self) -> bool {
        self.0 & 16 != 0
    }
    /// Una que se entiende: sin tipo, o de 1 a 4 canales y nada mas.
    pub const fn valida(self) -> bool {
        self.0 == 0 || (self.0 < 32 && self.canales() >= 1 && self.canales() <= 4)
    }
    pub const fn salto(self) -> u32 {
        if self.cuatro() { 16 } else { 4 * self.canales() }
    }
}

impl DelUav {
    pub const TODOS: [DelUav; 13] = [
        DelUav::Base,
        DelUav::Tope,
        DelUav::Limite,
        DelUav::Elementos,
        DelUav::Paso,
        DelUav::Medida,
        DelUav::Ancho,
        DelUav::Alto,
        DelUav::Texeles,
        DelUav::Salto,
        DelUav::Canales,
        DelUav::Alfa,
        DelUav::Relleno,
    ];

    /// El de un numero (el de la receta y el del .bsf vivo).
    pub fn de(n: u8) -> Option<DelUav> {
        DelUav::TODOS.into_iter().find(|q| *q as u8 == n)
    }

    /// **Lo que el KERNEL pone** en este registro (la base: el par entero)
    /// para un UAV de `bytes` prestados a la 3060 desde `base`, con su vista
    /// (`elementos`, en un crudo palabras; y `paso` si es estructurado): las
    /// reglas de la casa (`bufer::Uav`). Una vez, aqui: lo usan el kernel y
    /// las pruebas. `None`: no se presta (menos de una palabra, o la base sin
    /// alinear). [!] Un estructurado cuyo ultimo elemento no cabe entero en
    /// lo prestado lo pierde entero (la casa leeria sus palabras de dentro).
    pub fn valor(self, base: u64, bytes: u32, elementos: u32, paso: u32) -> Option<u64> {
        self.valor_con(base, bytes, elementos, paso, Forma::SIN_TIPO)
    }

    /// [`valor`](Self::valor) de una vista con su [`Forma`] (L3): con tipo,
    /// `paso` es el ancho de una textura (0 en un bufer) y `elementos` sus
    /// texeles, como en la casa.
    pub fn valor_con(self, base: u64, bytes: u32, elementos: u32, paso: u32, forma: Forma) -> Option<u64> {
        if bytes < 4 || base % 4 != 0 || !forma.valida() {
            return None;
        }
        let salto = forma.salto();
        let n = forma.canales();
        Some(match self {
            DelUav::Base => base,
            DelUav::Tope => (bytes & !3) as u64 - 4,
            DelUav::Limite => (4 * elementos as u64).min(bytes as u64).saturating_sub(3),
            DelUav::Elementos if paso == 0 => 0,
            DelUav::Elementos => elementos.min(bytes / paso) as u64,
            DelUav::Paso => paso as u64,
            // La de la casa (`Bufer::medidas`): un tipado, sus elementos;
            // un crudo, en bytes.
            DelUav::Medida if forma.0 != 0 => elementos as u64,
            DelUav::Medida if paso == 0 => elementos.saturating_mul(4) as u64,
            DelUav::Medida => elementos as u64,
            DelUav::Ancho => paso as u64,
            DelUav::Alto => elementos.checked_div(paso).unwrap_or(0) as u64,
            DelUav::Texeles => bytes.checked_div(salto).map_or(0, |c| elementos.min(c)) as u64,
            DelUav::Salto => salto as u64,
            DelUav::Canales => n as u64,
            DelUav::Alfa if forma.entero() => 1,
            DelUav::Alfa => 1.0f32.to_bits() as u64,
            DelUav::Relleno => ((1u32 << (salto / 4)) - 1) as u64 & !((1u64 << n) - 1),
        })
    }
}


/// Hilos por grupo, como mucho (D3D12 y la 3060).
pub const MAX_HILOS: u32 = 1024;
/// Grupos por eje, como mucho (D3D12).
pub const MAX_GRUPOS: u32 = 65535;
/// La compartida de un grupo, como mucho (`groupshared` de D3D12).
pub const MAX_COMPARTIDA: u32 = 32 * 1024;
/// Lo que Ampere reserva de la compartida por grupo (sm_8x: 1 KiB).
pub const RESERVA_POR_GRUPO: u32 = 1024;

/// Lo que se despacha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Despacho {
    pub grupos: [u32; 3],
    pub hilos: [u32; 3],
    /// Bytes de compartida del grupo.
    pub compartida: u32,
    /// Registros por hilo (los del cuerpo).
    pub registros: u32,
}

/// Por que un despacho no se puede.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoDespacha {
    /// Los hilos del grupo: 0, o mas de los que caben.
    Hilos,
    /// Los grupos: 0, o mas de 65535 en un eje.
    Grupos,
    /// La compartida pasa de 32 KiB.
    Compartida,
    /// Mas registros de los que da la 3060.
    Registros,
    /// El pegado no cabe donde va.
    NoCabe,
    /// El cuerpo no acaba en EXIT, o esta vacio.
    Cuerpo,
}

impl NoDespacha {
    /// La frase (el lente: cada NO, dicho).
    pub fn texto(self) -> &'static str {
        match self {
            NoDespacha::Hilos => "los hilos de un grupo: 0, o mas de 1024 (1024 x 1024 x 64)",
            NoDespacha::Grupos => "los grupos: 0, o mas de 65535 en un eje",
            NoDespacha::Compartida => "la compartida de un grupo pasa de 32 KiB",
            NoDespacha::Registros => "mas registros por hilo de los que da la 3060 (o los hilos del grupo por sus registros pasan de los 64 Ki de la SM)",
            NoDespacha::NoCabe => "el programa pegado no cabe en su hueco",
            NoDespacha::Cuerpo => "el cuerpo esta vacio o no acaba en EXIT",
        }
    }
}

/// La configuracion de la SM para `bytes` de compartida por grupo (con su
/// reserva): (KiB / 4) + 1 de 8, 16, 32 o 64 KiB (la de NVK).
pub fn config_compartida(bytes: u32) -> u64 {
    let total = bytes + RESERVA_POR_GRUPO;
    let kib = [8u32, 16, 32, 64].into_iter().find(|&k| total <= k * 1024).unwrap_or(64);
    (kib / 4 + 1) as u64
}

/// **El QMD** de un despacho: el programa en `programa`, RELEASE0 con `paga`
/// en `sem` al acabar la rejilla.
pub fn qmd(programa: u64, d: &Despacho, sem: u64, paga: u32) -> Result<[u32; QMD_PALABRAS], NoDespacha> {
    let [hx, hy, hz] = d.hilos;
    if hx == 0 || hy == 0 || hz == 0 || hx > 1024 || hy > 1024 || hz > 64 || hx * hy * hz > MAX_HILOS {
        return Err(NoDespacha::Hilos);
    }
    if d.grupos.iter().any(|&g| g == 0 || g > MAX_GRUPOS) {
        return Err(NoDespacha::Grupos);
    }
    if d.compartida > MAX_COMPARTIDA {
        return Err(NoDespacha::Compartida);
    }
    let registros = crate::tuberia::registros_para(d.registros);
    if d.registros == 0 || registros > crate::tuberia::MAX_REGISTROS || hx * hy * hz * registros > BANCO_DE_REGISTROS {
        return Err(NoDespacha::Registros);
    }
    let mut q = qmd_rejilla(programa, hx, d.grupos[0], d.grupos[1], sem, paga, registros);
    campo(&mut q, 463, 448, d.grupos[2] as u64); // CTA_RASTER_DEPTH
    campo(&mut q, 623, 608, hy as u64); // CTA_THREAD_DIMENSION1
    campo(&mut q, 639, 624, hz as u64); // CTA_THREAD_DIMENSION2
    campo(&mut q, 561, 544, d.compartida.next_multiple_of(256) as u64); // SHARED_MEMORY_SIZE
    let c = config_compartida(d.compartida);
    campo(&mut q, 567, 562, c); // MIN_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 574, 569, c); // MAX_SM_CONFIG_SHARED_MEM_SIZE
    campo(&mut q, 662, 657, c); // TARGET_SM_CONFIG_SHARED_MEM_SIZE
    Ok(q)
}

/// **El pegado**: en `out`, un MOV por cada `(registro, valor)` de `cargas`
/// y detras el `cuerpo`. Devuelve cuantas instrucciones.
pub fn pegar(out: &mut [(u64, u64)], cargas: &[(u8, u32)], cuerpo: &[(u64, u64)]) -> Result<usize, NoDespacha> {
    if cuerpo.last().is_none_or(|&(lo, _)| lo & 0x1FF != 0x14D) {
        return Err(NoDespacha::Cuerpo);
    }
    let n = cargas.len() + cuerpo.len();
    if n > out.len() {
        return Err(NoDespacha::NoCabe);
    }
    poner_cargas(out, cargas);
    out[cargas.len()..n].copy_from_slice(cuerpo);
    Ok(n)
}

/// Los MOV de las `cargas`, al principio de `out` (el kernel pone el cuerpo
/// detras por su cuenta: lo copia de la receta ahi mismo).
pub fn poner_cargas(out: &mut [(u64, u64)], cargas: &[(u8, u32)]) {
    for (w, &(r, v)) in out.iter_mut().zip(cargas) {
        *w = mov(r as u64, v);
    }
}

/// Lo que contesta un despacho: los microsegundos, y si se lanzo y se pago.
pub const fn empaquetar(us: u32, lanzado: bool, pagado: bool) -> u64 {
    us as u64 | (lanzado as u64) << 32 | (pagado as u64) << 33
}

/// `(us, lanzado, pagado)`.
pub const fn desempaquetar(v: u64) -> (u32, bool, bool) {
    (v as u32, v >> 32 & 1 != 0, v >> 33 & 1 != 0)
}

/// Donde ve la 3060 cada bufer de un computo: los UAV, por orden, en las
/// ranuras del DESTINO (escribibles); los SRV, en las de las TEXTURAS (solo
/// leer). `Some((ranura, es_uav))` del bufer `k` de `es_uav` (la lista de si
/// cada bufer es UAV), o `None` si no hay ranura (mas de 4 de una clase).
pub fn ranura_de(es_uav: impl Fn(usize) -> bool, k: usize) -> Option<(usize, bool)> {
    let uav = es_uav(k);
    let j = (0..k).filter(|&i| es_uav(i) == uav).count();
    let max = if uav { crate::destino::OBJETIVOS } else { crate::texturas::MAX_TEXTURAS };
    (j < max).then_some((j, uav))
}

/// La VA de la 3060 de la ranura `(j, es_uav)`, y su IOVA.
pub const fn va_iova_de(j: usize, uav: bool) -> (u64, u64) {
    if uav {
        (crate::destino::va_de(j), crate::destino::iova_de(j))
    } else {
        (crate::texturas::VA + j as u64 * crate::texturas::RANURA, crate::texturas::IOVA + j as u64 * crate::texturas::RANURA)
    }
}

/// Lo que mide una ranura (las dos clases, 16 MiB).
pub const RANURA_BYTES: u64 = crate::destino::MAX_BYTES;
const _: () = assert!(crate::destino::MAX_BYTES == crate::texturas::RANURA);

/// **Preparar un despacho** con la entrada `e` del GPFIFO de GR: los
/// semaforos a cero, el PEGADO en su region (lo que sobra de su ultima pagina
/// a cero), el QMD, las ordenes y la entrada, todo escrito por la ventana de
/// la VRAM.
pub fn preparar<R: Registros>(r: &mut R, e: u32, pegado: &[(u64, u64)], d: &Despacho) -> bool {
    if !crate::blur::entrada_valida(e) || pegado.is_empty() || pegado.len() > MAX_PEGADO {
        return false;
    }
    let Ok(mut q) = qmd(CODIGO_COMPUTO_VA, d, sombreador_va(SEMAFORO_QMD), PAGA_QMD) else { return false };
    // ** 13-10 (metal 13:35): nBody por la 3060 NO VOLVIO en 1 s y el
    // vigilante corto el canal. Lo que si corrio antes en el metal
    // (`sombreo`, `lienzo`) no tiene barreras; nBody si (`BAR.SYNC` con la
    // compartida), y el QMD decia CERO barreras para el grupo.
    // `NVC6C0_QMDV03_00_BARRIER_COUNT` = MW(767:763) (`clc6c0qmd.h` de
    // open-gpu-doc): una, la 0, si el programa la usa.
    if crate::sass::juez_computo::usa_barrera(pegado) {
        campo(&mut q, 767, 763, 1);
    }
    if escribir(r, SEMAFORO_QMD, &[0; 8]) != 8 {
        return false;
    }
    // El pegado, de 16 en 16 instrucciones (64 palabras), y su ultima pagina
    // con ceros detras.
    let mut w = [0u32; 64];
    let paginas = (pegado.len() * 16).div_ceil(0x1000);
    for k in 0..paginas * 256 / 16 {
        for j in 0..16 {
            let (lo, hi) = pegado.get(16 * k + j).copied().unwrap_or((0, 0));
            w[4 * j..4 * j + 4].copy_from_slice(&[lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32]);
        }
        if escribir(r, CODIGO_COMPUTO + 256 * k as u64, &w) != 64 {
            return false;
        }
    }
    let o = ordenes_con(sombreador_va(QMD), sombreador_va(SEMAFORO_FIN), PAGA_FIN);
    let en = entrada(sombreador_va(EMPUJE), ORDENES as u32);
    escribir(r, QMD, &q) == QMD_PALABRAS && escribir(r, EMPUJE, &o) == ORDENES && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

/// Tocar el timbre (como `completa::lanzar`).
pub fn lanzar<R: Registros>(r: &mut R, ficha: u32, e: u32) -> bool {
    let puesto = crate::blur::entrada_valida(e) && invalidar(r) && escribir(r, GR.userd + GP_PUT, &[crate::blur::siguiente(e)]) == 1;
    if puesto {
        r.escribir(TIMBRE, ficha);
    }
    puesto
}

/// `(GP_GET, el semaforo del QMD, el del fin)`.
pub fn mirar<R: Registros>(r: &mut R) -> (u32, u32, u32) {
    (leer32(r, GR.userd + GP_GET), leer32(r, SEMAFORO_QMD), leer32(r, SEMAFORO_FIN))
}

// Los semaforos, en lo libre de su pagina (detras de los de `completa`).
const _: () = assert!(SEMAFORO_QMD >= crate::completa::SEMAFORO_FIN + 0x70 && SEMAFORO_FIN + 8 <= SEMAFOROS + 0x1000);

#[cfg(test)]
mod pruebas {
    use super::*;

    /// L3: lo de una vista con tipo. Un R32_UINT de textura de 5 x 3 en
    /// cuatro floats (16 B por texel) con 200 B prestados: caben 12 texeles
    /// enteros; sus palabras 1..3 son relleno y su alfa, 1 (de enteros). Un
    /// RWBuffer<float2> de 32 bits: 8 B por elemento, sin relleno, alfa 1.0.
    #[test]
    fn lo_de_una_vista_con_tipo() {
        let tex = Forma::nueva(1, true, true);
        let v = |q: DelUav, f, bytes, elementos, paso| q.valor_con(0x1000, bytes, elementos, paso, f).unwrap();
        assert_eq!([DelUav::Ancho, DelUav::Alto, DelUav::Texeles, DelUav::Salto, DelUav::Canales, DelUav::Alfa, DelUav::Relleno].map(|q| v(q, tex, 200, 15, 5)), [5, 3, 12, 16, 1, 1, 0b1110]);
        let buf = Forma::nueva(2, false, false);
        assert_eq!([DelUav::Texeles, DelUav::Salto, DelUav::Canales, DelUav::Alfa, DelUav::Relleno, DelUav::Medida].map(|q| v(q, buf, 4096, 10, 0)), [10, 8, 2, 0x3F80_0000, 0, 10]);
        assert_eq!(DelUav::Medida.valor(0x1000, 4096, 10, 0), Some(40), "sin tipo, un crudo: sus bytes");
        assert_eq!(DelUav::Base.valor_con(0x1000, 64, 1, 0, Forma(0x28)), None, "una forma que no se entiende");
        assert!(DelUav::TODOS.iter().all(|q| DelUav::de(*q as u8) == Some(*q)));
    }
    use crate::motores::sombreador::leer_campo;
    use crate::sass::juez::{juzgar_drenado, Contexto, RESERVADOS};

    fn d() -> Despacho {
        Despacho { grupos: [4, 3, 2], hilos: [16, 4, 2], compartida: 2048, registros: 40 }
    }

    #[test]
    fn el_qmd_dice_la_rejilla_el_grupo_y_la_compartida() {
        let q = qmd(0x2_0000_0000, &d(), 0x2_0000_1040, 0x3060_C0C0).unwrap();
        let f = |hi, lo| leer_campo(&q, hi, lo);
        assert_eq!((f(415, 384), f(431, 416), f(463, 448)), (4, 3, 2));
        assert_eq!((f(607, 592), f(623, 608), f(639, 624)), (16, 4, 2));
        assert_eq!(f(561, 544), 2048);
        assert_eq!((f(567, 562), f(574, 569), f(662, 657)), (3, 3, 3), "2 KiB + 1: la de 8 KiB");
        assert_eq!(f(656, 648), 64, "los de `registros_para`: 40 + 2, a 8, nunca menos de 64");
        assert_eq!(f(1567, 1536) | f(1584, 1568) << 32, 0x2_0000_0000);
    }

    #[test]
    fn la_compartida_elige_su_configuracion() {
        assert_eq!(config_compartida(0), 3);
        assert_eq!(config_compartida(7 * 1024), 3);
        assert_eq!(config_compartida(7 * 1024 + 1), 5, "con su KiB de reserva ya no cabe en 8");
        assert_eq!(config_compartida(32 * 1024), 17, "32 KiB + 1: la de 64");
        let mut x = d();
        x.compartida = 300;
        assert_eq!(leer_campo(&qmd(0, &x, 0, 0).unwrap(), 561, 544), 512, "a 256 B");
    }

    #[test]
    fn los_techos_dicen_que_no() {
        let no = |f: &dyn Fn(&mut Despacho)| {
            let mut x = d();
            f(&mut x);
            qmd(0, &x, 0, 0).unwrap_err()
        };
        assert_eq!(no(&|x| x.hilos = [1024, 2, 1]), NoDespacha::Hilos);
        assert_eq!(no(&|x| x.hilos = [1, 1, 65]), NoDespacha::Hilos);
        assert_eq!(no(&|x| x.grupos = [0, 1, 1]), NoDespacha::Grupos);
        assert_eq!(no(&|x| x.grupos = [1, 65536, 1]), NoDespacha::Grupos);
        assert_eq!(no(&|x| x.compartida = 32 * 1024 + 4), NoDespacha::Compartida);
        assert_eq!(no(&|x| x.registros = 0), NoDespacha::Registros);
        // 1024 hilos de 72 registros (80 con los reservados): 80 Ki > 64 Ki.
        assert_eq!(no(&|x| { x.hilos = [1024, 1, 1]; x.registros = 72 }), NoDespacha::Registros);
        assert!(qmd(0, &Despacho { hilos: [1024, 1, 1], registros: 62, ..d() }, 0, 0).is_ok(), "1024 x 64 justo");
        assert!(!NoDespacha::Grupos.texto().is_empty());
    }

    /// Preparar escribe el pegado en su region (con ceros detras, hasta su
    /// pagina), el QMD que lo apunta, las ordenes y la entrada del GPFIFO.
    #[test]
    fn preparar_lo_deja_todo_en_su_sitio() {
        extern crate std;
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
        let pegado: std::vec::Vec<(u64, u64)> = (0..300u64).map(|k| (0x1000_0000_0000 | k, 0x2000_0000_0000 | k)).collect();
        assert!(preparar(&mut r, 7, &pegado, &d()));
        assert_eq!((leer32(&mut r, CODIGO_COMPUTO + 16 * 299), leer32(&mut r, CODIGO_COMPUTO + 16 * 299 + 8)), (299, 299));
        assert_eq!(leer32(&mut r, CODIGO_COMPUTO + 16 * 299 + 4), 0x1000);
        assert_eq!(leer32(&mut r, CODIGO_COMPUTO + 16 * 300 + 8), 0, "detras, a cero");
        let mut q = [0u32; QMD_PALABRAS];
        for (k, x) in q.iter_mut().enumerate() {
            *x = leer32(&mut r, QMD + 4 * k as u64);
        }
        assert_eq!(crate::motores::sombreador::leer_campo(&q, 1567, 1536) | crate::motores::sombreador::leer_campo(&q, 1584, 1568) << 32, CODIGO_COMPUTO_VA);
        assert_eq!(leer32(&mut r, GR.gpfifo + 8 * 7), entrada(sombreador_va(EMPUJE), ORDENES as u32) as u32);
        assert_eq!(crate::motores::sombreador::leer_campo(&q, 767, 763), 0, "sin BAR.SYNC, cero barreras");
        // 13-10: con un BAR.SYNC 0x0 en el programa, el QMD dice UNA barrera
        // (`NVC6C0_QMDV03_00_BARRIER_COUNT`, MW(767:763)).
        let mut con_barrera = pegado.clone();
        con_barrera[5] = (0x7b1d, 1 << 16);
        assert!(preparar(&mut r, 7, &con_barrera, &d()));
        for (k, x) in q.iter_mut().enumerate() {
            *x = leer32(&mut r, QMD + 4 * k as u64);
        }
        assert_eq!(crate::motores::sombreador::leer_campo(&q, 767, 763), 1);
        assert!(!preparar(&mut r, 7, &[], &d()), "sin pegado, nada");
    }

    /// El pegado: los MOV y el cuerpo, y el juez lo da por bueno (el cuerpo
    /// lee lo cargado justo despues del ultimo MOV: sus 6 ciclos bastan).
    #[test]
    fn el_pegado_carga_y_el_juez_lo_deja() {
        use bmo_sm86::codifica as c;
        // R4 = R2 + R3, y EXIT (con su control, el de ALU).
        let cuerpo = [c::iadd3(4, 2, c::r(3), c::ALU), c::exit(c::ALU)];
        let mut out = [(0, 0); 8];
        let n = pegar(&mut out, &[(2, 7), (3, 0x1000)], &cuerpo).unwrap();
        assert_eq!(n, 4);
        assert_eq!(out[0].0 >> 32, 7);
        juzgar_drenado(&out[..n], &Contexto { registros: 8 + RESERVADOS, sph: None }).unwrap_or_else(|b| panic!("{b}"));
        assert_eq!(pegar(&mut out[..3], &[(2, 7), (3, 1)], &cuerpo), Err(NoDespacha::NoCabe));
        assert_eq!(pegar(&mut out, &[], &cuerpo[..1]), Err(NoDespacha::Cuerpo));
    }
}
