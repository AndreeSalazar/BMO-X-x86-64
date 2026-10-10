//! **El PROGRAMA de la casa** (P3b3, 27-09; en PROMETEO desde el 08-10): el
//! formato comun de lo que corre una GPU o la CPU. Un programa de registros
//! `f32`, una operacion tras otra, que corre el interprete (`interprete.rs`)
//! y que cada tarjeta traduce a SU codigo (la 3060: SM86). Lo escriben el
//! traductor de DXIL y el de SM5 de PROTON-X y el emisor de GPU de TITAN++.
//!
//! LB3b de `docs/plan/PLAN_LAS_LIBRERIAS.md`: su codigo salio de PROTON-X
//! (`platform/shared/proton-x/src/dxil/programa.rs`), que ahora lo toma de
//! aqui y lo re-exporta en la misma ruta. Lo que se quedo alli es el
//! TRADUCTOR de DXIL: leer el bitcode y escribir este Programa.
//!
//! # Los numeros, a mano (y por que asi)
//!
//! - **FMad NO se funde**: `a * b` redondeado y luego `+ c` redondeado. DXIL
//!   deja al driver fundirla o no; asi es como el juez de `bmo-cubo` hace
//!   `wvp * pos`, y el juez da las huellas de D3D12 en la 3060.
//! - **Dot** suma de izquierda a derecha, sin fundir (lo mismo).
//! - **Rsqrt** es `1 / raiz`, con la raiz EXACTA (redondeada al mas cercano,
//!   en enteros: `raiz`), igual en el anfitrion y en Ring 3 con soft-float.
//! - **Saturate** de un NaN es 0, como pide D3D.
//!
//! Cada decision se paga contra las huellas: el banco de la casa dibuja
//! `cubo.exe` con esto y compara con lo que la 3060 dibujo bajo Windows.

use alloc::string::String;
use alloc::vec::Vec;

pub use super::ranuras::{Lugar, Mapa, Ranuras};

/// Por que un sombreador no se deja correr. El texto dice CUAL cosa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoPrograma {
    /// La forma del modulo no es la esperada.
    Forma(&'static str),
    /// Una instruccion de LLVM que no se sabe correr (su codigo de registro).
    Instruccion(u64),
    /// Una operacion de D3D que no se sabe correr (su numero de `dx.op`).
    OperacionD3d(i64),
    /// Una llamada a algo que no es `dx.op.*`.
    Llamada(String),
    /// Una instruccion de SM5 (`SHEX`) que no se sabe correr (su codigo).
    Sm5(u32),
}

/// Un registro del programa: un `f32`.
pub type Reg = u16;

/// N5.4: la `t` de una lectura que no es una ranura sino la textura ELEGIDA
/// por el ultimo [`Op::EligeTextura`] (un array de texturas, o bindless).
pub const DINAMICA: u8 = 255;

/// Una operacion del programa ya compilado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// `d = entrada[elemento][componente]`.
    Entrada { d: Reg, elemento: u8, componente: u8 },
    /// `salida[elemento][componente] = s`.
    Salida { s: Reg, elemento: u8, componente: u8 },
    /// Los 4 floats del registro `fila` del cbuffer, en `d..d+4`. `cb` es
    /// su ranura ([`Ranuras::cbuffers`], 03-10, N5.2); tras el enlace,
    /// `fila` ya es la del bloque APLANADO (`lote::Enlace::constantes`), y
    /// quien corre el programa solo mira `fila`.
    Constantes { d: Reg, fila: u16, cb: u8 },
    Mul { d: Reg, a: Reg, b: Reg },
    Add { d: Reg, a: Reg, b: Reg },
    Sub { d: Reg, a: Reg, b: Reg },
    Div { d: Reg, a: Reg, b: Reg },
    /// `a * b + c`, sin fundir.
    Mad { d: Reg, a: Reg, b: Reg, c: Reg },
    /// El producto escalar de `n` componentes.
    Dot { d: Reg, n: u8, a: [Reg; 4], b: [Reg; 4] },
    Rsqrt { d: Reg, a: Reg },
    Sqrt { d: Reg, a: Reg },
    Saturate { d: Reg, a: Reg },
    Abs { d: Reg, a: Reg },
    /// N5.6: sin, cos, tan, exp2, log2, frac, los redondeos y los medios
    /// floats (`crate::mates`), sobre los BITS del registro.
    Mate { d: Reg, a: Reg, f: crate::mates::Mate },
    Min { d: Reg, a: Reg, b: Reg },
    Max { d: Reg, a: Reg, b: Reg },
    /// `Sample`: la textura `t` (el registro tN) con el muestreador `s` (sN)
    /// en `(u, v)`; los cuatro canales (R, G, B, A) en `d..d+4`. D4.4: con
    /// `g`, la mip de los gradientes de `g..g+4` (`olas::gradientes`); sin
    /// el, la de la vista (un programa hecho a mano).
    Muestra { d: Reg, t: u8, s: u8, u: Reg, v: Reg, g: Option<Reg> },
    /// N5.4 (05-10): ELEGIR la textura del rango dinamico `rango` cuyo
    /// registro (absoluto, la base incluida) calculo el sombreador en los
    /// bits de `i`. La lectura que viene detras, con `t` = [`DINAMICA`], lee
    /// esa. Va justo delante de cada una: dos arrays en el mismo sombreador
    /// no se pisan.
    EligeTextura { i: Reg, rango: u8 },
    /// N5.5 (05-10): un id del hilo de computo, en bits: `que` 0
    /// SV_DispatchThreadID, 1 SV_GroupID, 2 SV_GroupThreadID (su componente
    /// `c`), 3 SV_GroupIndex.
    IdHilo { d: Reg, que: u8, c: u8 },
    /// N5.5: `GroupMemoryBarrierWithGroupSync` y su familia: ningun hilo del
    /// grupo sigue hasta que todos llegan aqui (el interprete PARA el hilo y
    /// corre los demas).
    Barrera,
    /// N5.5: la memoria compartida del GRUPO (`groupshared`): la palabra
    /// `base + i` (de `n`; fuera, 0 al leer y nada al escribir).
    LeeCompartida { d: Reg, base: u32, n: u32, i: Reg },
    EscribeCompartida { base: u32, n: u32, i: Reg, s: Reg },
    /// 18 de la pila A (07-10): un `Interlocked*` sobre la memoria
    /// COMPARTIDA (`atomicrmw` de LLVM, la instruccion 38): la palabra
    /// `base + i` (de `n`) pasa a `como(antes, v)`; `d` la de antes. Fuera,
    /// 0 y nada.
    AtomicoCompartido { d: Reg, base: u32, n: u32, i: Reg, v: Reg, como: crate::bufer::Atomo },
    /// N5.5: `bufferStore` al UAV de la ranura `u`: el elemento `i`, `desp`
    /// bytes dentro de el (estructurado), los canales de `v` que dice
    /// `mascara`. En un UAV de textura, `i` la x, `desp` la y y (06-10) `z`
    /// la rebanada de un 3D o la capa de un array (en los demas, un 0).
    EscribeUav { u: u8, modo: crate::bufer::Modo, i: Reg, desp: Reg, z: Reg, v: [Reg; 4], mascara: u8 },
    /// N5.5: `bufferLoad` de un UAV (`RWStructuredBuffer`...): como
    /// `Lectura::Bufer`, pero del UAV `u`.
    LeeUav { d: Reg, u: u8, modo: crate::bufer::Modo, i: Reg, desp: Reg, z: Reg },
    /// N5.3c (05-10): `GetDimensions` de un UAV: sus elementos (de un bufer)
    /// o su ancho y su alto (de una textura), como enteros.
    MedidasUav { d: Reg, u: u8, modo: crate::bufer::Modo },
    /// E2.4 (05-10): `bufferUpdateCounter`: sube (`inc` 1) o baja (-1) el
    /// contador oculto del UAV `u`; `d` el de antes al subir, el de despues
    /// al bajar (lo de D3D). `Append` es esto y un `bufferStore` en ese
    /// indice.
    Contador { d: Reg, u: u8, inc: i8 },
    /// 05-10: un `Interlocked*` (`atomicBinOp`, `atomicCompareExchange`)
    /// sobre la palabra del elemento `i` (`desp` dentro) del UAV `u`: `d` la
    /// de antes ([`crate::bufer::Uav::atomico`]); `igual`, lo que se compara.
    Atomico { d: Reg, u: u8, modo: crate::bufer::Modo, i: Reg, desp: Reg, z: Reg, como: crate::bufer::Atomo, v: Reg, igual: Reg },
    /// E2.3b (05-10): una entrada de un sombreador de GEOMETRIA: el
    /// componente del elemento `elemento` del vertice `vertice` de su
    /// primitiva (en las entradas, cada vertice ocupa [`Programa::entradas`]
    /// elementos seguidos).
    EntradaDe { d: Reg, vertice: u8, elemento: u8, componente: u8 },
    /// E2.3b: `EmitStream`: un vertice, con las salidas de ahora, al flujo
    /// `flujo` (solo el 0 llega a la trama).
    Emite { flujo: u8 },
    /// E2.3b: `CutStream`: la tira de ahora se acaba.
    Corta { flujo: u8 },
    /// 02-10: leer una textura con lo que `Muestra` (2D) no dice: `Sample`
    /// con mas coordenadas (arrays, cubos, 3D) o desplazado, `SampleLevel`,
    /// `SampleBias` y `SampleGrad` (D4.4: con su sesgo y sus gradientes,
    /// [`Lectura::Gradientes`]), `Load` y `GetDimensions` (ver
    /// [`Lectura`]). `c`: las coordenadas (floats, o enteros en `Load`; las
    /// que no trae, un registro a 0); `nivel`: la mip (un float en
    /// SampleLevel, un entero en Load y GetDimensions); `desp`: el
    /// desplazamiento en texeles. Los cuatro canales en `d..d+4`.
    Lee { d: Reg, t: u8, s: u8, como: Lectura, c: [Reg; 4], nivel: Reg, desp: [i8; 3] },

    // -- E6 (02-10): comparar, elegir y saltar. Un registro guarda BITS: un
    // float, un entero (complemento a dos) o un booleano de D3D (0xFFFFFFFF
    // si, 0 no). Las de abajo los copian tal cual; las de arriba los leen
    // como float.
    /// `d = a <como> b`: 0xFFFFFFFF si se cumple, 0 si no (como `lt`, `ge`,
    /// `eq`, `ne` de SM5). Float: con un NaN solo se cumple `Distinto` (D3D:
    /// `ne` es desordenada). `entero`: los bits como `i32`.
    Compara { d: Reg, a: Reg, b: Reg, como: Comparacion, entero: bool },
    /// `d = c != 0 ? a : b`, los bits (`movc`).
    Elige { d: Reg, c: Reg, a: Reg, b: Reg },
    /// `d = a`, los bits: una VARIABLE (escrita mas de una vez) se escribe asi.
    Copia { d: Reg, a: Reg },
    /// `d = a + b` como enteros de 32 bits (modulo 2^32: `iadd`).
    SumaEntera { d: Reg, a: Reg, b: Reg },
    /// E6c (02-10): las demas de enteros de 32 bits (ver [`OpEntera`]).
    Entera { d: Reg, a: Reg, b: Reg, op: OpEntera },
    /// E6c: de entero a float y al reves (ver [`Conversion`]).
    Convierte { d: Reg, a: Reg, como: Conversion },
    /// Lo de hasta su `SiNo` o su `FinSi` corre si los bits de `c` no son 0.
    Si { c: Reg },
    SiNo,
    FinSi,
    /// Lo de hasta su `FinBucle` se repite hasta un `Romper`.
    Bucle,
    /// Sale del bucle mas interno si los bits de `c` no son 0 (`si_cero`:
    /// si SON 0): `breakc_nz` y `breakc_z`.
    RomperSi { c: Reg, si_cero: bool },
    /// Sale del bucle mas interno (`break`).
    Romper,
    /// Vuelve a la cabeza del bucle mas interno (`continue`; E6b, 02-10: lo
    /// pide el DXIL, que salta a la cabeza desde el medio del cuerpo).
    Continuar,
    FinBucle,
    /// N5.7 (03-10): `discard` y `clip()`: si los bits de `c` no son 0, el
    /// pixel se TIRA -- ni color ni profundidad -- y el programa acaba ahi.
    /// Solo tiene sentido en el de pixeles.
    Descarta { c: Reg },
    /// N5.10 (03-10): `d = array[i]`: el array son los `n` registros desde
    /// `base`; el indice, los bits de `i` (fuera, 0).
    LeeIndexado { d: Reg, base: Reg, n: u16, i: Reg },
    /// `array[i] = s` (fuera, nada).
    EscribeIndexado { base: Reg, n: u16, i: Reg, s: Reg },
    /// 03-10: [`Op::Constantes`] con la fila CALCULADA: la `fila + i` (los
    /// bits de `i`), si `i < filas` (las que se reservan para el cbuffer:
    /// sin saber hasta donde llega el indice, las 4096 de D3D); si no, 0.
    /// Los arrays de un cbuffer (luces, huesos) se leen asi.
    ConstantesEn { d: Reg, fila: u16, filas: u16, i: Reg, cb: u8 },
    /// E2.5 (05-10): una operacion de OLA (`Wave*`, `Quad*`): mira los
    /// carriles de la ola, asi que el interprete PARA el hilo aqui y la
    /// resuelve con los demas (`carriles.rs`). `a` el valor, `b` el
    /// segundo operando (el carril de `ReadLaneAt`); `d..d+4` en `Papeleta`.
    Ola { d: Reg, a: Reg, b: Reg, que: super::olas::Ola },
}

/// Lo que pregunta [`Op::Compara`]. Las `SinSigno` (E6c), solo con
/// `entero`: los bits como `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparacion {
    Menor,
    MenorIgual,
    Mayor,
    MayorIgual,
    Igual,
    Distinto,
    MenorSinSigno,
    MenorIgualSinSigno,
    MayorSinSigno,
    MayorIgualSinSigno,
}

/// Lo que hace [`Op::Entera`], sobre los bits (modulo 2^32). Los
/// desplazamientos usan los 5 bits de abajo de la cuenta, como D3D (`ishl`,
/// `ushr`, `ishr`; `dxc` pone ese `& 31` el mismo).
///
/// La division y el resto (E6d): por 0 dan 0xFFFFFFFF, cociente y resto, con
/// signo o sin el (lo de D3D en `udiv`; la 3060 hace eso mismo, `~b`). Con
/// signo, hacia cero y el resto con el signo de `a`; `i32::MIN / -1` es
/// `i32::MIN` y su resto 0 (lo que no cabe da la vuelta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpEntera {
    Resta,
    Mul,
    Shl,
    /// `>>` logico (`ushr`, `lshr`).
    ShrL,
    /// `>>` aritmetico (`ishr`, `ashr`).
    ShrA,
    Y,
    O,
    OX,
    MinS,
    MaxS,
    MinU,
    MaxU,
    DivU,
    RemU,
    DivS,
    RemS,
}

impl OpEntera {
    pub fn hacer(self, a: u32, b: u32) -> u32 {
        match self {
            OpEntera::Resta => a.wrapping_sub(b),
            OpEntera::Mul => a.wrapping_mul(b),
            OpEntera::Shl => a << (b & 31),
            OpEntera::ShrL => a >> (b & 31),
            OpEntera::ShrA => ((a as i32) >> (b & 31)) as u32,
            OpEntera::Y => a & b,
            OpEntera::O => a | b,
            OpEntera::OX => a ^ b,
            OpEntera::MinS => (a as i32).min(b as i32) as u32,
            OpEntera::MaxS => (a as i32).max(b as i32) as u32,
            OpEntera::MinU => a.min(b),
            OpEntera::MaxU => a.max(b),
            _ if b == 0 => u32::MAX,
            OpEntera::DivU => a / b,
            OpEntera::RemU => a % b,
            OpEntera::DivS => (a as i32).wrapping_div(b as i32) as u32,
            OpEntera::RemS => (a as i32).wrapping_rem(b as i32) as u32,
        }
    }
}

/// Lo que hace [`Op::Convierte`]. De float a entero: hacia cero, y lo que no
/// cabe se queda en el limite (un NaN, 0) -- lo de D3D (`ftoi`, `ftou`) y de
/// PTX (`cvt.rzi`); `as` de Rust hace eso mismo. De entero a float: al mas
/// cercano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conversion {
    EnteroAFloat,
    SinSignoAFloat,
    FloatAEntero,
    FloatASinSigno,
}

impl Conversion {
    pub fn hacer(self, x: u32) -> u32 {
        match self {
            Conversion::EnteroAFloat => (x as i32 as f32).to_bits(),
            Conversion::SinSignoAFloat => (x as f32).to_bits(),
            Conversion::FloatAEntero => f32::from_bits(x) as i32 as u32,
            Conversion::FloatASinSigno => f32::from_bits(x) as u32,
        }
    }
}

impl Comparacion {
    /// Con floats (`Distinto` se cumple con un NaN; las demas, no).
    pub fn floats(self, x: f32, y: f32) -> bool {
        match self {
            Comparacion::Menor => x < y,
            Comparacion::MenorIgual => x <= y,
            Comparacion::Mayor => x > y,
            Comparacion::MayorIgual => x >= y,
            Comparacion::Igual => x == y,
            Comparacion::Distinto => x != y,
            // Sin sentido con floats (los lectores no las dan): las ordenadas.
            Comparacion::MenorSinSigno => x < y,
            Comparacion::MenorIgualSinSigno => x <= y,
            Comparacion::MayorSinSigno => x > y,
            Comparacion::MayorIgualSinSigno => x >= y,
        }
    }

    /// Con enteros: con signo, o sin el las `SinSigno`.
    pub fn enteros(self, x: i32, y: i32) -> bool {
        let (u, v) = (x as u32, y as u32);
        match self {
            Comparacion::Menor => x < y,
            Comparacion::MenorIgual => x <= y,
            Comparacion::Mayor => x > y,
            Comparacion::MayorIgual => x >= y,
            Comparacion::Igual => x == y,
            Comparacion::Distinto => x != y,
            Comparacion::MenorSinSigno => u < v,
            Comparacion::MenorIgualSinSigno => u <= v,
            Comparacion::MayorSinSigno => u > v,
            Comparacion::MayorIgualSinSigno => u >= v,
        }
    }
}

/// Por que la forma de un programa no vale (su indice): un `SiNo` o un
/// `FinSi` sin su `Si`, un `Romper` fuera de un bucle, algo sin cerrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalaForma(pub usize);

/// Lo que se puede anidar (bucles dentro de `si` dentro de bucles...).
pub const ANIDADO_MAXIMO: usize = 32;

/// **Un sombreador listo para correr.**
#[derive(Debug, Clone, PartialEq)]
pub struct Programa {
    pub ops: Vec<Op>,
    /// Los registros al empezar: las constantes float del modulo ya puestas.
    pub iniciales: Vec<f32>,
    /// Cuantos elementos de entrada y de salida toca (por su id en la firma).
    pub entradas: usize,
    pub salidas: usize,
    /// Que elementos de entrada LEE de verdad (bit = id en la firma).
    pub lee: u32,
    /// El mayor registro del cbuffer que lee, mas uno (en filas de 16 bytes).
    pub filas_cb: u16,
    /// Las texturas y los muestreadores que lee, con su espacio y su registro
    /// (03-10, N5.1): el `t` y el `s` de [`Op::Lee`] y [`Op::Muestra`] son
    /// su POSICION aqui, no un registro.
    pub ranuras: Ranuras,
    /// N5.5: lo de un sombreador de computo.
    pub computo: Computo,
}

/// **Lo de un sombreador de computo** (N5.5, 05-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Computo {
    /// Los hilos de un grupo (`[numthreads(x, y, z)]`); [0; 3] si no es de
    /// computo.
    pub hilos: [u32; 3],
    /// Las palabras de 4 bytes de su memoria compartida (`groupshared`).
    pub compartida: u32,
    /// 05-10, de uno de PIXELES: `[earlydepthstencil]` (la profundidad se
    /// prueba ANTES de correrlo aunque escriba UAV). Va aqui, con lo de
    /// la etapa que no son operaciones, y no en `Programa`: asi no cambian
    /// los que lo construyen a mano.
    pub temprana: bool,
}

/// **Como lee una textura** [`Op::Lee`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lectura {
    /// `Sample` en la mip mas detallada de la vista (D4.4: el DXIL ya no la
    /// pide: sus `Sample` van por [`Lectura::Gradientes`]).
    Muestra,
    /// `SampleLevel`: la mip de `nivel` (float).
    Nivel,
    /// `Load`: el texel de coordenadas enteras; `enteros`, del formato
    /// entero (`.i32`), los canales como enteros.
    Carga { enteros: bool },
    /// `GetDimensions`: ancho, alto, profundidad o capas, mips (enteros).
    Medidas,
    /// N5.3: `Load` de un BUFER: `c[0]` el elemento (o el byte, crudo) y
    /// `c[1]` el desplazamiento dentro de el (estructurado). Como bits.
    Bufer(crate::bufer::Modo),
    /// N5.3: `GetDimensions` de un bufer: sus elementos (o bytes).
    MedidasBufer(crate::bufer::Modo),
    /// 03-10: `Gather` (el canal de los cuatro texeles del cuadro de 2x2).
    Junta { canal: u8 },
    /// 03-10: `SampleCmp` / `SampleCmpLevelZero`: la referencia va en
    /// `nivel`; el resultado (0..1), en los cuatro.
    Compara,
    /// 03-10: `GatherCmp`: los cuatro texeles, cada uno comparado.
    JuntaCompara { canal: u8 },
    /// D4.4: `Sample`, `SampleBias`, `SampleGrad` (y con `compara`,
    /// `SampleCmp`) con la mip de sus GRADIENTES. Desde `nivel`, seguidos:
    /// ddx(u), ddx(v), ddy(u), ddy(v), el sesgo, el clamp del sombreador (la
    /// mip menor que deja) y, si compara, la referencia.
    Gradientes { compara: bool },
    /// D4.4: `CalculateLevelOfDetail` (`sujeta`) o su `Unclamped`: el LOD
    /// de los gradientes de `nivel..nivel+4`, en x.
    Lod { sujeta: bool },
}

impl Programa {
    /// N5.12: el que no hace nada (el de pixeles de un PSO sin el: solo
    /// profundidad, las sombras).
    pub fn vacio() -> Programa {
        Programa { ops: Vec::new(), iniciales: Vec::new(), entradas: 0, salidas: 0, lee: 0, filas_cb: 0, ranuras: Ranuras::default(), computo: Computo::default() }
    }

    /// **Correr el sombreador una vez.** `entradas` y `salidas` por el id del
    /// elemento en su firma; `cb`, los bytes del cbuffer (lo que falte se lee
    /// como 0). `regs` es memoria de trabajo (se reusa entre llamadas).
    pub fn correr(&self, entradas: &[[f32; 4]], cb: &[u8], salidas: &mut [[f32; 4]], regs: &mut Vec<f32>) -> bool {
        self.correr_con(entradas, cb, &crate::textura::Recursos::NINGUNO, salidas, regs)
    }

    /// Si el programa lee alguna textura (`Sample`).
    pub fn muestrea(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Muestra { .. } | Op::Lee { .. } | Op::EligeTextura { .. }))
    }

    /// 05-10: si LEE o ESCRIBE un UAV (lo de un dibujo con efectos: la trama
    /// no puede saltarse ni repetir un pixel de estos).
    pub fn toca_uav(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::EscribeUav { .. } | Op::LeeUav { .. } | Op::MedidasUav { .. } | Op::Contador { .. } | Op::Atomico { .. }))
    }

    /// E2.5: si usa las olas (un pixel asi va en cuadros y olas, `cuadros`).
    /// D4.4: las derivadas son olas (de su cuadro), y un muestreo con la mip
    /// por derivadas las pide: tambien va en cuadros.
    pub fn usa_olas(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Ola { .. }))
    }

    /// D4.4: si usa olas que NO son derivadas (`Wave*`, `Quad*`).
    pub fn olas_propias(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Ola { que, .. } if !matches!(que, super::olas::Ola::Derivada { .. })))
    }

    /// D4.4: si el sombreador pide derivadas (`ddx`, `ddy`, `fwidth`); las de
    /// la mip de un muestreo no cuentan (ver [`Programa::mip_por_derivadas`]).
    pub fn deriva(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Ola { que: super::olas::Ola::Derivada { muestra: false, .. }, .. }))
    }

    /// D4.4: si muestrea con la mip de sus derivadas (`Sample`, no
    /// `SampleLevel`).
    pub fn mip_por_derivadas(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Ola { que: super::olas::Ola::Derivada { muestra: true, .. }, .. }))
    }

    /// ** E8g (10-10): si lee una textura en la mip que le DICEN
    /// (`SampleLevel`, `Load`): la 3060 tiene en su TIC la mip 0 de cada una
    /// todavia; la puerta lo pregunta.
    pub fn lee_con_nivel(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Lee { como: Lectura::Nivel | Lectura::Carga { .. }, .. }))
    }

    /// Si el programa salta (E6): `si`, bucles, o lo que lee bits como
    /// enteros o booleanos. Lo que no sabe de esto (el traductor a x86-64,
    /// `nativo`) lo mira aqui y se aparta.
    pub fn salta(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Compara { .. } | Op::Elige { .. } | Op::Copia { .. } | Op::SumaEntera { .. } | Op::Entera { .. } | Op::Convierte { .. } | Op::Si { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::RomperSi { .. } | Op::Romper | Op::Continuar | Op::FinBucle))
    }

    /// **La forma**: cada `Si` con su `FinSi` (y a lo sumo un `SiNo`), cada
    /// `Bucle` con su `FinBucle`, cada `Romper` dentro de un bucle, y no mas
    /// de [`ANIDADO_MAXIMO`] por dentro. Quien lee un sombreador la comprueba
    /// antes de darlo; el interprete y el emisor cuentan con ella.
    pub fn forma(&self) -> Result<(), MalaForma> {
        // Lo abierto: `true` un bucle, `false` un si (y si ya vio su SiNo).
        let mut abierto: Vec<(bool, bool)> = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            match op {
                Op::Si { .. } => abierto.push((false, false)),
                Op::Bucle => abierto.push((true, false)),
                Op::SiNo => match abierto.last_mut() {
                    Some((false, visto)) if !*visto => *visto = true,
                    _ => return Err(MalaForma(i)),
                },
                Op::FinSi => {
                    if !matches!(abierto.pop(), Some((false, _))) {
                        return Err(MalaForma(i));
                    }
                }
                Op::FinBucle => {
                    if !matches!(abierto.pop(), Some((true, _))) {
                        return Err(MalaForma(i));
                    }
                }
                Op::RomperSi { .. } | Op::Romper | Op::Continuar => {
                    if !abierto.iter().any(|x| x.0) {
                        return Err(MalaForma(i));
                    }
                }
                _ => {}
            }
            if abierto.len() > ANIDADO_MAXIMO {
                return Err(MalaForma(i));
            }
        }
        if abierto.is_empty() {
            Ok(())
        } else {
            Err(MalaForma(self.ops.len()))
        }
    }
}

/// `saturate` de D3D: a [0, 1], y un NaN es 0.
pub fn saturar(x: f32) -> f32 {
    if x > 0.0 {
        if x < 1.0 {
            x
        } else {
            1.0
        }
    } else {
        0.0
    }
}

/// **La raiz cuadrada EXACTA de un `f32`**, redondeada al mas cercano con
/// empates al par (lo que pide IEEE-754), con enteros: sale igual en cualquier
/// CPU y con soft-float, sin `libm`.
pub fn raiz(x: f32) -> f32 {
    let b = x.to_bits();
    if x.is_nan() || x == 0.0 || x == f32::INFINITY {
        return x; // raiz(-0) = -0
    }
    if b >> 31 != 0 {
        return f32::NAN;
    }
    let (mut e, mut m) = (((b >> 23) & 0xFF) as i32, (b & 0x7F_FFFF) as u64);
    if e == 0 {
        // Subnormal: normalizar la mantisa.
        e = 1;
        while m & 0x80_0000 == 0 {
            m <<= 1;
            e -= 1;
        }
    } else {
        m |= 0x80_0000;
    }
    // x = m * 2^(e - 150); con el exponente par, raiz(x) = raiz(m') * 2^(k/2).
    let mut k = e - 150;
    if k & 1 != 0 {
        m <<= 1;
        k -= 1;
    }
    // raiz(m << 48) tiene 36..37 bits: de sobra para 24 y el redondeo.
    let n = (m as u128) << 48;
    let mut r: u128 = 0;
    let mut bit: u128 = 1 << 72;
    let mut resto = n;
    while bit > n {
        bit >>= 2;
    }
    while bit != 0 {
        if resto >= r + bit {
            resto -= r + bit;
            r = (r >> 1) + bit;
        } else {
            r >>= 1;
        }
        bit >>= 2;
    }
    // raiz(x) = (r + fraccion) * 2^(k/2 - 24), con `resto != 0` si no es exacta.
    let largo = 128 - r.leading_zeros() as i32;
    let sobra = largo - 24;
    let mut mant = (r >> sobra) as u64;
    let caidos = r & ((1u128 << sobra) - 1);
    let mitad = 1u128 << (sobra - 1);
    if caidos > mitad || (caidos == mitad && (resto != 0 || mant & 1 == 1)) {
        mant += 1;
    }
    let mut exp = k / 2 - 24 + sobra + 23; // del bit alto de `mant`
    if mant == 1 << 24 {
        mant >>= 1;
        exp += 1;
    }
    f32::from_bits((((exp + 127) as u32) << 23) | (mant as u32 & 0x7F_FFFF))
}
