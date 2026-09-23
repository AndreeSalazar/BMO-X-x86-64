//! **BEF2** -- el formato de un programa de BMO-X, trazado desde lo que el
//! cargador HACE y no desde ELF.
//!
//! [carril]  ROJO     es la frontera: lo que el kernel lee antes de mapear nada
//! [cuesta]  MAQUINA  un campo mal leido mapea mal un programa
//! [riesgo]  ESPEJO   la puerta del kernel (`bmo-bex-gate`) lo lee por su cuenta
//!
//! Decidido por Eddi el 2026-09-19 (*"BEF reemplaza el ELF maestro"*). Plan y
//! motivos: `docs/plan/PLAN_BEF_NATIVO.md`. BEF1 (`crate::bef`) era la idea
//! central de ELF -- una cabecera y una tabla de secciones tipadas, con el
//! permiso de cada pagina sacado de una BANDERA --, y eso dejo `RoData`
//! escribible en el Ryzen hasta `8c3ac5c0`.
//!
//! # La forma
//!
//! ```text
//!    0   magic       "BEF2"
//!    4   abi         u8     2
//!    5   banderas    u8     EJECUTABLE | OBJETO | QUIERE_PANTALLA
//!    6   reservado   u16    0
//!    8   xcr0        u64    componentes XSAVE que el programa usa
//!   16   entrada     u32    offset dentro de CODIGO
//!   20   anexos      u32    cuantos (su tabla va en el byte 64)
//!   24   codigo      {offset u32, bytes u32}      R+X
//!   32   constantes  {offset u32, bytes u32}      R+NX
//!   40   datos       {offset u32, bytes u32}      R+W+NX
//!   48   ceros       u32 bytes                    R+W+NX, a cero
//!   52   total       u32    bytes del fichero
//!   56   reservado   u64    0
//!   64   anexos: {tipo u8, 0 u8 x3, offset u32, bytes u32, 0 u32} x N
//! ```
//!
//! ** EL PERMISO LO DA EL HUECO. Las cuatro regiones tienen sitio fijo en la
//! cabecera: no se puede escribir un BEF2 con codigo escribible o constantes
//! escribibles, porque no hay campo donde decirlo.
//!
//! ** Y todo es x86-64 sin disimulo: no hay byte de arquitectura ni de orden de
//! bytes (el magic ya dice BMO-X x86-64), y en vez de "extensiones de CPU" va
//! la mascara de XCR0, que es literalmente lo que `XSAVE` necesita.

mod escritor;
mod lector;
/// El objeto (`.bo`): lo que el enlazador junta.
pub mod objeto;
/// Una app es UN fichero: el .bex con sus recursos dentro.
pub mod paquete;

#[cfg(test)]
mod pruebas;

pub use escritor::{Escritor, Requisito};
pub use lector::{leer, Anexo, Falta, Tramo, Vista};
pub use paquete::{directorio, empaquetar, localizar_recursos};

/// `b"BEF2"` en little-endian.
pub const MAGIC: u32 = u32::from_le_bytes(*b"BEF2");
/// El ABI que habla la imagen. Uno solo: el 2.
pub const ABI: u8 = 2;
/// La cabecera: 64 B, una linea de cache.
pub const CABECERA: usize = 64;
/// Una entrada de la tabla de anexos.
pub const ANEXO: usize = 16;
/// Un reloc.
pub const RELOC: usize = 16;
/// Tope de anexos: la tabla entera cabe en una pantalla, y en 4 KiB de
/// prologo con la cabecera delante.
pub const MAX_ANEXOS: usize = 16;

// -- Banderas ---------------------------------------------------------------

/// Es un programa que se puede lanzar.
pub const EJECUTABLE: u8 = 1 << 0;
/// Es un objeto sin enlazar (`.bo`): lo consume `bmo-enlazar`, nunca el kernel.
pub const OBJETO: u8 = 1 << 1;
/// El programa reclama la pantalla. Lo pone el COMPILADOR al ver la operacion
/// (no puede mentir: dice lo que el programa hace); el DIRECTOR lo lee antes de
/// lanzar.
pub const QUIERE_PANTALLA: u8 = 1 << 2;
/// Las que existen. Una bandera fuera de aqui se RECHAZA: cambia el
/// significado de lo que viene detras, y adivinarlo es leer mal a proposito.
pub const BANDERAS: u8 = EJECUTABLE | OBJETO | QUIERE_PANTALLA;

// -- XCR0: el estado de CPU que el programa necesita preservado --------------

/// x87. `XSAVE` lo exige siempre.
pub const XCR0_X87: u64 = 1 << 0;
/// SSE (xmm0-15, MXCSR). BMO C e INTI lo usan para la coma flotante escalar.
pub const XCR0_SSE: u64 = 1 << 1;
/// AVX (mitad alta de los ymm).
pub const XCR0_AVX: u64 = 1 << 2;
/// **Lo que el kernel preserva HOY** en un cambio de contexto. Un programa que
/// pida mas se rechaza con nombre: sin esto, usar AVX corromperia sus ymm en
/// silencio a la primera interrupcion. Crece el dia que `trap.rs` guarde mas.
pub const XCR0_PRESERVADO: u64 = XCR0_X87 | XCR0_SSE;

// -- Regiones --------------------------------------------------------------

/// Las cuatro regiones de la imagen. El numero es el que usan los relocs.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Codigo = 0,
    Constantes = 1,
    Datos = 2,
    Ceros = 3,
}

impl Region {
    /// **La numeracion que usan los emisores por dentro** (`SEC_CODE = 0`,
    /// `SEC_DATA = 1`, `SEC_RODATA = 2`), que NO es la de las regiones.
    ///
    /// ** Existe aqui y no copiada en cada emisor a proposito: son dos
    /// numeraciones del mismo concepto, y dos traducciones acaban
    /// discrepando. Se ira el dia que los emisores hablen de regiones por
    /// dentro; mientras tanto, la conversion tiene un solo sitio.
    pub const fn de_seccion_de_emisor(sec: u8) -> Option<Self> {
        match sec {
            0 => Some(Self::Codigo),
            1 => Some(Self::Datos),
            2 => Some(Self::Constantes),
            _ => None,
        }
    }

    pub const fn de(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Codigo),
            1 => Some(Self::Constantes),
            2 => Some(Self::Datos),
            3 => Some(Self::Ceros),
            _ => None,
        }
    }
}

// -- Anexos ---------------------------------------------------------------

/// Los relocs de la imagen: [`Reloc`] x N. Los aplica el kernel.
pub const ANEXO_RELOCS: u8 = 0x01;
/// La firma: el hash BLAKE3 del INDICE, de cada region con bytes y de CADA
/// anexo, y Ed25519 opcional sobre la cadena de todos ellos.
pub const ANEXO_FIRMA: u8 = 0x02;
/// Lo que el programa necesita para arrancar (`bmo-carga-juicio`).
pub const ANEXO_REQUISITOS: u8 = 0x03;
/// Los recursos de la app (`bef::recursos`).
pub const ANEXO_RECURSOS: u8 = 0x04;
/// El manifiesto en texto, para humanos.
pub const ANEXO_MANIFIESTO: u8 = 0x05;
/// Las katanas (`bef::katanas`).
pub const ANEXO_KATANAS: u8 = 0x06;
/// **Que funcion vive en cada offset.** Los llevan los objetos (para enlazar)
/// y tambien los EJECUTABLES, porque el DIRECTOR los lee para anotar una
/// autopsia: `SHA1_Update+0x18` en vez de `rip 0x400815f2`
/// (`Ultra_userspace/services/director/src/simbolos.rs`).
///
/// ** El primer esquema los prohibia en un ejecutable -- *"se enlazo estatico,
/// no hay nada que resolver"* -- y eso habria roto esa anotacion sin que nadie
/// se enterara hasta el siguiente fallo en el Ryzen. Son data para OTRO: el
/// kernel los SALTA, que es lo que hace que puedan viajar.
pub const ANEXO_SIMBOLOS: u8 = 0x07;
/// **Los enlaces de un OBJETO** (`objeto::Enlace`): lo que `bmo-enlazar`
/// resuelve al juntar unidades. Solo va en un `.bo`: un ejecutable con esto
/// dentro es una unidad sin enlazar disfrazada, y se rechaza.
pub const ANEXO_ENLACE: u8 = 0x08;
/// **Los sombreadores de la app** (S6, el BSF): SPIR-V, su interfaz y el
/// codigo ya traducido, en el formato de `toolchain/lang/spirv/bsf`. Data para
/// la app que los ejecuta: el kernel lo salta, y la firma lo cubre como a todo
/// anexo.
pub const ANEXO_SOMBREADORES: u8 = 0x09;

/// **Los tres que el kernel abre.** Todo otro anexo es data para OTRO -- el
/// enlazador, el verificador, el runtime de un lenguaje -- y se SALTA: es la
/// unica idea de la regla congelada de BEF1 que sobrevive, y sobrevive porque
/// es la que deja crecer el formato sin que crezca el kernel.
///
/// [!] Saltarlo no es dejarlo sin firmar: **la firma cubre TODOS los anexos**
/// (2026-09-20). El kernel solo comprueba los que lee; el que lea los otros
/// --el DIRECTOR con los recursos, `bmo-verify` con las katanas-- tiene su
/// hash en la misma tabla, y la firma de autor responde por ellos.
pub const fn lo_lee_el_kernel(tipo: u8) -> bool {
    matches!(tipo, ANEXO_RELOCS | ANEXO_FIRMA | ANEXO_REQUISITOS)
}

// -- Firma ----------------------------------------------------------------

/// Cabecera del anexo de firma: `cuantos u32`, `algoritmo u32`.
pub const FIRMA_CABECERA: usize = 8;
/// Un hash: `que u8`, 7 de relleno, BLAKE3 de 32 bytes.
///
/// ** LOS 40 BYTES SON LOS MISMOS QUE LOS DE BEF1, Y ES A PROPOSITO. El kernel
/// lee esa entrada como `section_index: u16` + 6 de relleno + digest
/// (`task/landing.rs::Firmas`). Con `que` en el primer byte y el segundo a
/// cero, los dos formatos escriben bytes IDENTICOS mientras el indice quepa en
/// un byte -- y aqui cabe siempre: tres regiones y hasta dieciseis anexos.
///
/// Asi Ring 0 encuentra los digests de un BEF2 **sin cambiar una linea**, y eso
/// no es una casualidad que haya que descubrir: es la razon de que el campo
/// mida un byte y no dos. Una prueba de `gate_y_validador_no_se_separan` lee
/// la firma con el codigo del kernel copiado a mano.
pub const FIRMA_HASH: usize = 40;
/// `que` de un hash que cubre un ANEXO: `0x80 | indice en la tabla`. Los
/// valores 0..=2 son las regiones con bytes.
pub const FIRMA_ANEXO: u8 = 0x80;
/// **`que` del hash del INDICE: la cabecera (64 B) y la tabla de anexos.**
///
/// *** LA FIRMA ES DEL INDICE (`docs/identidad/EL_CONTRATO_DE_CARGA.md`,
/// parte 2b, apuntado el 2026-08-10 y HECHO el 2026-09-20). Hasta hoy los
/// hashes cubrian las regiones y los anexos y NADIE cubria lo que dice donde
/// esta cada cosa: un `.bex` firmado admitia que le cambiaran la ENTRADA, el
/// `xcr0`, los `ceros` o el medida de un anexo sin que ninguna comprobacion
/// se quejara -- el hash de la region cuadraba igual, porque la region no
/// habia cambiado; habia cambiado a DONDE saltaba el kernel.
///
/// Es la PRIMERA entrada de la firma, y el kernel la comprueba con el
/// prologo que ya tiene en la mano, antes de reservar un solo marco: si el
/// indice no cuadra, no se lee ni el 0,03 % del fichero. Con ella, la cadena
/// que firma Ed25519 resume la imagen ENTERA, cabecera incluida.
pub const FIRMA_INDICE: u8 = 0x7F;
/// Sin firma de autor: solo integridad (los hashes dicen "llego lo que se
/// escribio", no "esto lo escribi YO").
pub const ALGO_NINGUNO: u32 = 0;
/// Ed25519: firma de AUTOR sobre la cadena de hashes. Detras de los hashes van
/// 64 bytes de firma y 32 de clave publica.
pub const ALGO_ED25519: u32 = 1;
/// Lo que ocupa una firma Ed25519 detras de los hashes.
pub const FIRMA_ED25519: usize = 96;

// -- Reloc -----------------------------------------------------------------

/// **El unico reloc de un ejecutable**: en `donde`+`offset` se escriben 64
/// bits con la direccion de `destino`+`addend`.
///
/// ```text
///    0  donde    u8    region donde se escribe (Codigo, Constantes, Datos)
///    1  destino  u8    region a la que apunta (las cuatro)
///    2  0        u16
///    4  offset   u32   dentro de `donde`
///    8  addend   u64   dentro de `destino`
/// ```
///
/// Un `Rel32` a un simbolo lo resuelve `bmo-enlazar` al enlazar y no llega
/// aqui; un GOT es enlazado dinamico y no existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reloc {
    pub donde: Region,
    pub destino: Region,
    pub offset: u32,
    pub addend: u64,
}

impl Reloc {
    pub fn a_bytes(&self) -> [u8; RELOC] {
        let mut b = [0u8; RELOC];
        b[0] = self.donde as u8;
        b[1] = self.destino as u8;
        b[4..8].copy_from_slice(&self.offset.to_le_bytes());
        b[8..16].copy_from_slice(&self.addend.to_le_bytes());
        b
    }

    /// `None` si una region no existe o el relleno no es cero.
    pub fn de_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < RELOC || b[2] != 0 || b[3] != 0 {
            return None;
        }
        Some(Self {
            donde: Region::de(b[0])?,
            destino: Region::de(b[1])?,
            offset: u32::from_le_bytes(b[4..8].try_into().ok()?),
            addend: u64::from_le_bytes(b[8..16].try_into().ok()?),
        })
    }
}

/// Lee un `u32` little-endian. `None` si no hay bytes.
pub(crate) fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))
}

/// Lee un `u64` little-endian. `None` si no hay bytes.
pub(crate) fn u64_en(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o.checked_add(8)?)?.try_into().ok()?))
}
