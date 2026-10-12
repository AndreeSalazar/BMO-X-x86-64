//! **EL PEGAMENTO DE E5** -- lo que rodea a un programa EMITIDO (el cuerpo
//! que sale de `bmo-proton-x-sm86` con `Abi::Registros`) para que corra en
//! la tuberia de VERRANO: carga lo que el cuerpo espera en registros, y
//! saca lo que deja en R0.. por donde la 3060 lo recoge.
//!
//! capa: puro -- instrucciones y cabeceras en bytes; nada toca la tarjeta
//!
//! [eje]     CORRECCION -- solo instrucciones que ya corrieron en el metal
//!           (ALD, MOV, LDG, IMAD.SHL, IADD3, IMAD.X, IPA, AST, EXIT, BRA:
//!           las de V0 y T2a) y el juez lo juzga todo, pegamento y cuerpo
//!
//! # Los DATOS (`tuberia::Paquete::vertices`, la ranura 0 de la tabla)
//!
//! ```text
//!    byte 0                 el cbuffer: `filas` filas de 16 B
//!    byte 16 * filas        los vertices TAL CUAL los da el juego: `paso`
//!                           bytes cada uno, y cada elemento de entrada en su
//!                           `desde`, con sus `componentes` float
//! ```
//!
//! Un componente que el elemento no trae es el de D3D: 0 en x, y, z y 1 en w
//! (un MOV, no una carga). Asi la CPU solo COPIA el bufer del juego: no
//! reempaqueta nada (P3b4a, 28-09).
//!
//! # El de vertice
//!
//! ```text
//!    ALD   vid <- a[0x2fc]                      barrera 0
//!    MOV   A, A+1 <- la pagina de la tabla
//!    LDG   D, D+1 <- la ranura 0 (los DATOS)    barrera 2
//!    V = vid * paso (IMAD.SHL por cada bit del paso, IADD3)
//!    V:V+1 = D:D+1 + 16 * filas + V (IADD3 con acarreo, IMAD.X)
//!    LDG   cada entrada  <- [V + desde + 4 * componente]   barrera 3
//!          (o MOV 0 / 1 si el elemento no trae ese componente)
//!    LDG   cada fila     <- [D + 16 * fila + 4 * k]                barrera 3
//!    el cuerpo (su primera instruccion espera la 3; sin su EXIT)
//!    AST.128 de cada salida: la posicion a a[0x70], las demas a los
//!          genericos a[0x80 + 16 g], en orden                       lectura 1
//!    EXIT esperando la 1 ; BRA .
//! ```
//!
//! Los registros del pegamento (A, D, V, vid) van DETRAS de los del cuerpo:
//! el cuerpo no los pisa, y las cargas no pisan lo que el cuerpo usa.
//!
//! # El de pixel
//!
//! ```text
//!    IPA   cada entrada <- el generico que la lleva (ScreenLinear)  barrera 0
//!          (`genericos`: el de cada entrada; [`generico`] da el que le
//!          puso el de vertice a cada salida)
//!    y si lee el cbuffer: MOV A, LDG D (barrera 2), LDG cada fila  barrera 3
//!    el cuerpo (su primera instruccion espera la 0 y la 3), con su EXIT ; BRA .
//! ```
//!
//! **ScreenLinear, no con perspectiva**: exacto cuando los tres vertices dan
//! el mismo valor (las caras del cubo: su normal y su color). Un valor que
//! cambia dentro del triangulo pide la interpolacion con perspectiva: eso es
//! otro paso, y aqui se dice, no se esconde.

use crate::cubo::{self as cu, con_control, mov, ALU};
use crate::lienzo::sombreador_va;
use crate::raster::{bit, SPH};
use crate::sass::juez::{MAX_INSTRUCCIONES, RESERVADOS};
use crate::tuberia::{carga, espera, iadd3_acarreo, imad_shl, imad_x, ipa, ldg, sph_pixel as sph_pixel_v0, sph_vertice as sph_vertice_v0, IPA_CONTROL, IPA_ULTIMO, MAX_REGISTROS, TABLA};

/// Lo que el pegamento carga antes del cuerpo (lo que `bmo-proton-x-sm86`
/// llama `Precarga`, dicho aqui sin nombrarlo: el driver no depende de
/// PROTON-X).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carga {
    /// El componente `componente` de la entrada `elemento`, en `reg`.
    Entrada { elemento: u8, componente: u8, reg: u8 },
    /// La fila `fila` del cbuffer, en `reg`..`reg + 3`.
    Fila { fila: u16, reg: u8 },
    /// P3b4c.8 T2: el ASA de la textura `textura` (`texturas::asa(k, k)`),
    /// en `reg`. La pone el pegamento con un MOV: la app no elige que TIC ni
    /// que TSC lee la 3060. Solo en el de pixel.
    Asa { textura: u8, reg: u8 },
    /// ** LI2a (12-10): `SV_Position` en el de PIXEL, su componente
    /// `componente` (0 = x, 1 = y) en `reg`: un IPA de `a[0x70 + 4c]` y su
    /// bit en la SPH (`ImapPositionX/Y`, bits 188 y 189: la tabla 9 de
    /// `Shader-Program-Header` de NVIDIA). Con el origen arriba y el centro del
    /// pixel en el medio (`raster.rs`), x e y son `columna + 0.5` y `fila +
    /// 0.5`: los MISMOS bits que la CPU. La z y la w, todavia no (la w es 1/w
    /// en la 3060 y pide la division exacta de la casa).
    Posicion { componente: u8, reg: u8 },
}

/// `a[0x70]`, la posicion en el de pixel, en palabras (`IPA` lleva el
/// atributo en palabras de 4 bytes: `a[4 * atributo]`).
pub const ATRIBUTO_POSICION: u64 = 0x70 / 4;
/// `ImapPositionX` en la SPH (tras CommonWord0..4, 160 bits, e
/// `ImapSystemValuesA`, 24: `ImapSystemValuesB` empieza en el 184 con
/// PrimitiveId, RtArrayIndex, ViewportIndex, PointSize, PositionX..W).
pub const BIT_IMAP_POSICION_X: u32 = 188;

/// **Lo que el pegamento hace con un canal de la salida 0** antes del EXIT
/// (LI2d-HDR, 12-10). Un render target de float lo guarda la casa en floats
/// de 32 bits YA redondeados a su formato (`formato_ia::cuantizar`: un
/// RGBA16F redondea al half, el canal que el formato no trae se lee 0, y el
/// alfa 1); la 3060 escribe floats de 32 (`destino::Flotante`) y el redondeo
/// lo pone esto:
///
/// ```text
///    Igual   el float de 32 tal cual (un canal de 32 bits, o 8 bits por
///            canal: ahi convierte el ROP)
///    Half    al half y de vuelta: F2FP.PACK_AB y HADD2.F32, las dos de
///            `ptxas` para `cvt.rn.f16.f32` y `cvt.f32.f16` (al par mas
///            cercano, como `formato_ia::a_half`)
///    Cero    0.0 (un canal que el formato no trae)
///    Uno     1.0 (el alfa que el formato no trae)
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Canal {
    #[default]
    Igual,
    Half,
    Cero,
    Uno,
}

/// El redondeo de los cuatro canales de la salida 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Redondeo(pub [Canal; 4]);

impl Redondeo {
    /// Nada que hacer (8 bits por canal, o floats de 32).
    pub const NADA: Redondeo = Redondeo([Canal::Igual; 4]);

    /// En la receta: dos bits por canal (r en los bajos).
    pub const fn numero(self) -> u32 {
        let mut n = 0;
        let mut c = 0;
        while c < 4 {
            n |= (match self.0[c] {
                Canal::Igual => 0,
                Canal::Half => 1,
                Canal::Cero => 2,
                Canal::Uno => 3,
            }) << (2 * c);
            c += 1;
        }
        n
    }

    pub const fn de_numero(n: u32) -> Option<Redondeo> {
        if n > 0xFF {
            return None;
        }
        let mut r = [Canal::Igual; 4];
        let mut c = 0;
        while c < 4 {
            r[c] = match n >> (2 * c) & 3 {
                0 => Canal::Igual,
                1 => Canal::Half,
                2 => Canal::Cero,
                _ => Canal::Uno,
            };
            c += 1;
        }
        Some(Redondeo(r))
    }
}

/// `F2FP.PACK_AB Rd, RZ, Rb`: el half de Rb (al par mas cercano) en la mitad
/// baja de Rd, 0 en la alta. La palabra de `ptxas` (sm_86) para
/// `cvt.rn.f16.f32`, con el control que se pida.
pub const fn f2fp_half(rd: u64, rb: u64, control: u64) -> (u64, u64) {
    (rb << 32 | 0xFF << 24 | rd << 16 | 0x723E, con_control(0xFF, control))
}

/// `HADD2.F32 Rd, -RZ, Rb.H0_H0`: el half de la mitad baja de Rb, en float
/// de 32 (exacto). La palabra de `ptxas` (sm_86) para `cvt.f32.f16`.
pub const fn hadd2_f32(rd: u64, rb: u64, control: u64) -> (u64, u64) {
    (0x20 << 56 | rb << 32 | 0xFF << 24 | rd << 16 | 0x7230, con_control(0x4100, control))
}

/// **El redondeo de la salida 0** (R0..R3), detras del cuerpo y antes de su
/// EXIT. Cada una espera 6 ciclos (la del cuerpo de delante tambien:
/// `al_menos`), lo que pide la mas lenta de la tabla del juez.
fn redondear(o: &mut Poner, r: Redondeo) {
    for (c, canal) in r.0.iter().enumerate() {
        let c = c as u64;
        match canal {
            Canal::Igual => {}
            Canal::Half => {
                o.p(f2fp_half(c, c, ALU));
                o.p(hadd2_f32(c, c, ALU));
            }
            Canal::Cero => o.p(mov(c, 0)),
            Canal::Uno => o.p(mov(c, 0x3F80_0000)),
        }
    }
}

/// Los registros que llevan un asa (la mascara que pide el juez R7).
pub fn asas(cargas: &[Carga]) -> u64 {
    cargas.iter().fold(0, |m, c| match *c {
        Carga::Asa { reg, .. } if reg < 64 => m | 1 << reg,
        _ => m,
    })
}

/// Donde esta un elemento de entrada dentro de un vertice (del input layout
/// del juego), o de que valor de SISTEMA sale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elemento {
    /// Su byte dentro del vertice.
    pub desde: u32,
    /// Cuantos componentes trae (1..4).
    pub componentes: u8,
    /// De la memoria del vertice, o un valor de sistema.
    pub sistema: Sistema,
    /// ** LI2e (12-10): su `DXGI_FORMAT` (`formatos::forma`), o 0: floats de
    /// 32 bits (`componentes` de ellos), lo de siempre.
    pub formato: u8,
}

/// **Los valores de sistema del de vertice** (LI2e, 12-10: el censo de
/// Cyberpunk, 16 lotes de 34). Lo que valen lo dice la CPU
/// (`bmo_proton_x::lote::entrada`, la referencia); aqui, como los da la 3060:
///
/// ```text
///    Vertice     SV_VertexID: el `vid` que el ALD ya trae (el indice que
///                escribe la puerta). La puerta escribe el indice SIN el
///                vertice base y copia los vertices DESDE el base: el mismo
///                `vid` es el SV_VertexID de la CPU y la direccion de la
///                memoria no cambia. Un entero en x; y, z y w a 0
///    Instancia   SV_InstanceID: 0. La 3060 dibuja hoy UNA instancia (la
///                puerta de "varias instancias" sigue), y D3D12 lo cuenta
///                desde 0, sin StartInstanceLocation
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sistema {
    No,
    Vertice,
    Instancia,
}

impl Sistema {
    /// Su numero en la receta (los bits 8..10 de `componentes`).
    pub const fn numero(self) -> u32 {
        match self {
            Sistema::No => 0,
            Sistema::Vertice => 1,
            Sistema::Instancia => 2,
        }
    }

    pub const fn de_numero(n: u32) -> Option<Sistema> {
        match n {
            0 => Some(Sistema::No),
            1 => Some(Sistema::Vertice),
            2 => Some(Sistema::Instancia),
            _ => None,
        }
    }
}

impl Elemento {
    /// De la memoria del vertice.
    pub const fn memoria(desde: u32, componentes: u8) -> Self {
        Elemento { desde, componentes, sistema: Sistema::No, formato: 0 }
    }

    /// ** LI2e (12-10): de la memoria, en el `DXGI_FORMAT` `formato`, si la
    /// 3060 lo lee (`formatos::forma`); los floats de 32, como [`Self::memoria`].
    pub const fn con_formato(desde: u32, formato: u32) -> Option<Self> {
        match crate::formatos::forma(formato) {
            Some(f) if f.es_float32() => Some(Elemento::memoria(desde, f.componentes())),
            Some(f) => Some(Elemento { desde, componentes: f.componentes(), sistema: Sistema::No, formato: formato as u8 }),
            None => None,
        }
    }

    /// Un valor de sistema (un entero, en x).
    pub const fn de_sistema(sistema: Sistema) -> Self {
        Elemento { desde: 0, componentes: 1, sistema, formato: 0 }
    }

    /// Su forma (la de los floats de 32 si no dice otra).
    pub const fn forma(&self) -> Option<crate::formatos::Forma> {
        match self.formato {
            0 => crate::formatos::forma(match self.componentes {
                1 => 41,
                2 => 16,
                3 => 6,
                4 => 2,
                _ => return None,
            }),
            f => crate::formatos::forma(f as u32),
        }
    }

    /// Los bytes que lee de la memoria del vertice.
    pub const fn bytes(&self) -> u32 {
        match self.forma() {
            Some(f) => f.bytes(),
            None => 0,
        }
    }
}

/// float4 seguidos: el vertice de E5 (`bmo_cubo::tanda::datos`).
const FLOAT4: [Elemento; 8] = {
    let mut e = [Elemento::memoria(0, 4); 8];
    let mut k = 0;
    while k < 8 {
        e[k].desde = 16 * k as u32;
        k += 1;
    }
    e
};

/// Como estan los DATOS (ver el principio).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Datos<'a> {
    /// Filas del cbuffer delante de los vertices.
    pub filas: u32,
    /// Bytes de un vertice.
    pub paso: u32,
    /// Cada elemento de entrada del de vertice, por su numero.
    pub elementos: &'a [Elemento],
}

impl Datos<'static> {
    /// `entradas` float4 seguidos por vertice (el de E5).
    pub const fn float4(filas: u32, entradas: u32) -> Self {
        let n = if entradas > 8 { 8 } else { entradas as usize };
        Datos { filas, paso: 16 * entradas, elementos: FLOAT4.split_at(n).0 }
    }
}

impl Datos<'_> {
    /// Bytes de los datos con `n` vertices.
    pub const fn bytes(&self, n: usize) -> usize {
        16 * self.filas as usize + n * self.paso as usize
    }
}

/// El generico que el de VERTICE le da a su salida `salida` (la `posicion`
/// no es generico): el de pixel recibe asi cada entrada.
pub const fn generico(salida: u32, posicion: u32) -> Option<u8> {
    if salida == posicion {
        None
    } else if salida < posicion {
        Some(salida as u8)
    } else {
        Some(salida as u8 - 1)
    }
}

/// Por que no se pega.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoPega {
    /// No cabe en `juez::MAX_INSTRUCCIONES`.
    Instrucciones,
    /// El cuerpo y el pegamento no caben en `tuberia::MAX_REGISTROS`.
    Registros,
    /// Una carga fuera de los datos, o que el de pixel no puede recibir (la
    /// posicion, o un generico que no hay).
    Carga,
    /// El cuerpo no acaba en EXIT, o esta vacio.
    Cuerpo,
    /// Las salidas no se entienden (la posicion fuera, o mas de 8).
    Salidas,
}

/// Un programa pegado: su SPH, su codigo y cuantos registros usa.
#[derive(Clone, Copy, Debug)]
pub struct Pegado {
    pub sph: [u32; SPH],
    pub codigo: [(u64, u64); MAX_INSTRUCCIONES],
    pub n: usize,
    /// Los registros que usa, con los del pegamento (sin los `RESERVADOS`).
    pub registros: u32,
}

impl Pegado {
    /// Uno vacio, para pegar en el ([`vertice_en`], [`pixel_en`]).
    pub const VACIO: Pegado = Pegado { sph: [0; SPH], codigo: [(0, 0); MAX_INSTRUCCIONES], n: 0, registros: 0 };

    pub fn codigo(&self) -> &[(u64, u64)] {
        &self.codigo[..self.n]
    }

    /// Los bytes tal como viajan (SPH y codigo); devuelve cuantos.
    pub fn bytes(&self, out: &mut [u8]) -> usize {
        for (k, w) in self.sph.iter().enumerate() {
            out[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut i = 4 * SPH;
        for &(lo, hi) in self.codigo() {
            out[i..i + 8].copy_from_slice(&lo.to_le_bytes());
            out[i + 8..i + 16].copy_from_slice(&hi.to_le_bytes());
            i += 16;
        }
        i
    }
}

/// ** Pone las instrucciones DIRECTAMENTE en el `Pegado` de quien pega
/// (28-09): antes se armaban en un arreglo propio y se devolvia el `Pegado`
/// por valor -- dos copias de 2 KiB por programa, y el pegado de una receta
/// en el kernel se comia 13 KiB de la pila del syscall.
struct Poner<'a> {
    g: &'a mut Pegado,
    lleno: bool,
}

impl Poner<'_> {
    fn p(&mut self, w: (u64, u64)) {
        if self.g.n == MAX_INSTRUCCIONES {
            self.lleno = true;
            return;
        }
        self.g.codigo[self.g.n] = w;
        self.g.n += 1;
    }
}

/// `ALD Rd, a[0x2fc]` (el numero de vertice): el de V0 con otro destino.
pub const fn ald_vertice(rd: u64) -> (u64, u64) {
    ((cu::ALD_VERTICE.0 & !(0xFF << 16)) | rd << 16, cu::ALD_VERTICE.1)
}

/// `AST.128 a[atributo], Rs`: el de V0 (`cu::AST_POSICION`) con otra fuente
/// y otro atributo, y el control que se pida.
pub const fn ast(fuente: u64, atributo: u64, control: u64) -> (u64, u64) {
    let lo = cu::AST_POSICION.0 & !(0xFF << 32) & !(0x3FF << 40);
    (lo | fuente << 32 | atributo << 40, con_control(cu::AST_POSICION.1, control))
}

/// El control de un AST: 1 ciclo, el bit 4, sin barrera de escritura, la
/// de LECTURA 1 (la que espera el EXIT, como V0).
const AST_CONTROL: u64 = 1 | 1 << 4 | 7 << 5 | 1 << 8;

/// Las barreras del pegamento.
const B_ALD: u64 = 0;
const B_TABLA: u64 = 2;
const B_CARGAS: u64 = 3;

/// *** Que cada carga caiga en registros DEL CUERPO (P3b4c, 28-09): los del
/// pegamento (la tabla `A`, el puntero de los datos `D`, `V`, el vertice)
/// van detras de `registros` (`propios`). Una carga con su destino en `D`
/// cambiaria el puntero con un valor de los DATOS -- que da la app -- y el
/// LDG de las filas de detras leeria donde ella quisiera. Con programas de
/// la casa no pasaba (el emisor no lo hace); con la RECETA de una app, esto
/// es la valla.
fn cargas_propias(cargas: &[Carga], registros: u32) -> Result<(), NoPega> {
    for c in cargas {
        let (reg, n) = match *c {
            Carga::Entrada { reg, .. } => (reg as u32, 1),
            Carga::Posicion { componente, reg } => {
                if componente > 1 {
                    return Err(NoPega::Carga);
                }
                (reg as u32, 1)
            }
            Carga::Fila { reg, .. } => (reg as u32, 4),
            Carga::Asa { textura, reg } => {
                if textura as usize >= crate::texturas::MAX_TEXTURAS {
                    return Err(NoPega::Carga);
                }
                (reg as u32, 1)
            }
        };
        if reg + n > registros {
            return Err(NoPega::Carga);
        }
    }
    Ok(())
}

/// El cuerpo sin su EXIT final (y su EXIT, aparte).
fn partir(cuerpo: &[(u64, u64)]) -> Result<(&[(u64, u64)], (u64, u64)), NoPega> {
    match cuerpo.split_last() {
        Some((&exit, resto)) if exit.0 & 0x1FF == 0x14D && !resto.is_empty() => Ok((resto, exit)),
        _ => Err(NoPega::Cuerpo),
    }
}

/// Que la instruccion `w` espere tambien las barreras de `mascara`.
const fn esperando(w: (u64, u64), mascara: u64) -> (u64, u64) {
    (w.0, w.1 | mascara << (41 + 11))
}

/// Que la instruccion `w` espere al menos `ciclos` antes de la siguiente.
const fn al_menos(w: (u64, u64), ciclos: u64) -> (u64, u64) {
    let e = w.1 >> 41 & 0xF;
    if e >= ciclos {
        w
    } else {
        (w.0, (w.1 & !(0xF << 41)) | ciclos << 41)
    }
}

/// Los registros del pegamento, a partir del primero PAR tras el cuerpo:
/// `(A, D, V, vid)` y cuantos registros usa todo.
fn propios(registros: u32) -> Result<(u64, u64, u64, u64, u32), NoPega> {
    let a = (registros as u64 + 1) & !1;
    let total = a as u32 + 7;
    // LI2b (12-10): hasta lo mas que da la tuberia; las ordenes piden lo que
    // de verdad se usa (`tuberia::registros_para`).
    if total + RESERVADOS > MAX_REGISTROS {
        return Err(NoPega::Registros);
    }
    Ok((a, a + 2, a + 4, a + 6, total))
}

/// MOV de la pagina de la tabla a `a`:`a+1` y LDG de la ranura 0 a `d`:`d+1`.
fn la_tabla(o: &mut Poner, a: u64, d: u64) {
    let t = sombreador_va(TABLA);
    let base = t & !0xFF_FFFF;
    let off = (t - base) as u32;
    o.p(mov(a, base as u32));
    o.p(mov(a + 1, (base >> 32) as u32));
    o.p(ldg(d, a, off, carga(B_TABLA)));
    o.p(ldg(d + 1, a, off + 4, carga(B_TABLA)));
}

/// LDG de cada fila del cbuffer desde `d`; la primera espera la tabla si
/// `esperar_tabla`.
fn las_filas(o: &mut Poner, cargas: &[Carga], datos: Datos, d: u64, mut esperar_tabla: bool) -> Result<(), NoPega> {
    for c in cargas {
        if let Carga::Fila { fila, reg } = *c {
            if fila as u32 >= datos.filas {
                return Err(NoPega::Carga);
            }
            for k in 0..4u32 {
                let control = if esperar_tabla { carga(B_CARGAS) | 1 << B_TABLA << 11 } else { carga(B_CARGAS) };
                esperar_tabla = false;
                o.p(ldg(reg as u64 + k as u64, d, 16 * fila as u32 + 4 * k, control));
            }
        }
    }
    Ok(())
}

fn cerrar(o: Poner, sph: [u32; SPH], registros: u32) -> Result<(), NoPega> {
    if o.lleno {
        return Err(NoPega::Instrucciones);
    }
    o.g.sph = sph;
    o.g.registros = registros;
    Ok(())
}

/// **El de VERTICE**: `cuerpo` (con su EXIT al final) usa `registros` y
/// espera `cargas`; deja `salidas` float4 en R0.. (la `posicion`-esima es
/// la posicion; las demas, los genericos 0, 1... en orden).
pub fn vertice(cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, salidas: u32, posicion: u32) -> Result<Pegado, NoPega> {
    let mut g = Pegado::VACIO;
    vertice_en(&mut g, cuerpo, registros, cargas, datos, salidas, posicion)?;
    Ok(g)
}

/// [`vertice`], pegando en `g` (sin copias: el kernel pega asi).
pub fn vertice_en(g: &mut Pegado, cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, salidas: u32, posicion: u32) -> Result<(), NoPega> {
    vertice_con_libreta_en(g, cuerpo, registros, cargas, datos, salidas, posicion, None)
}

/// [`vertice_en`] con la LIBRETA (9d): `termometro`, el registro del cuerpo
/// donde deja el suyo; detras del cuerpo, el apunte (`libreta::apunte`).
#[allow(clippy::too_many_arguments)]
pub fn vertice_con_libreta_en(g: &mut Pegado, cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, salidas: u32, posicion: u32, termometro: Option<u8>) -> Result<(), NoPega> {
    g.n = 0;
    let (resto, _) = partir(cuerpo)?;
    cargas_propias(cargas, registros)?;
    termometro_propio(termometro, registros)?;
    // Las texturas, hoy, solo en el de pixel.
    if asas(cargas) != 0 {
        return Err(NoPega::Carga);
    }
    if posicion >= salidas || salidas > 8 {
        return Err(NoPega::Salidas);
    }
    let (a, d, v, vid, total) = propios(registros)?;
    let mut o = Poner { g, lleno: false };
    o.p(ald_vertice(vid));
    la_tabla(&mut o, a, d);
    // V = vid * paso: un IMAD.SHL por cada bit del paso (48 = 32 + 16), en
    // V y V+1, y sumados. El primero espera el ALD.
    let paso = datos.paso;
    // Sin paso no hay vertices en la memoria: solo valores de sistema (un
    // cuadro de pantalla completa con SV_VertexID). V vale 0 y no se lee.
    let de_memoria = cargas.iter().any(|c| match *c {
        Carga::Entrada { elemento, .. } => datos.elementos.get(elemento as usize).is_none_or(|e| e.sistema == Sistema::No),
        _ => false,
    });
    if paso == 0 {
        if de_memoria {
            return Err(NoPega::Carga);
        }
        o.p(mov(v, 0));
    }
    let mut primero = true;
    for b in 0..32u32 {
        if paso & 1 << b == 0 {
            continue;
        }
        let control = if primero { espera(1 << B_ALD) } else { ALU };
        if primero {
            o.p(imad_shl(v, vid, 1 << b, control));
        } else {
            o.p(imad_shl(v + 1, vid, 1 << b, control));
            o.p(iadd3_acarreo(v, 0, v, v + 1, ALU));
        }
        primero = false;
    }
    // Y las filas del cbuffer van delante: V += 16 * filas.
    if datos.filas > 0 {
        o.p(mov(v + 1, 16 * datos.filas));
        o.p(iadd3_acarreo(v, 0, v, v + 1, ALU));
    }
    // V:V+1 = D:D+1 + V (espera la tabla).
    o.p(iadd3_acarreo(v, 0, d, v, espera(1 << B_TABLA)));
    o.p(imad_x(v + 1, d + 1, 0xFF, 0, ALU));
    // LI2e: los componentes que hay que convertir, tras sus cargas.
    let mut convertir = [(crate::formatos::forma(41).unwrap(), 0u64, 0u32, 0u32); 64];
    let mut convertir_n = 0;
    for c in cargas {
        if let Carga::Entrada { elemento, componente, reg } = *c {
            let Some(el) = datos.elementos.get(elemento as usize) else { return Err(NoPega::Carga) };
            if componente > 3 {
                return Err(NoPega::Carga);
            }
            match el.sistema {
                // SV_VertexID: el `vid` en x (un IMAD.SHL por 1, que espera
                // al ALD como el primero de la direccion); lo demas, 0.
                Sistema::Vertice if componente == 0 => {
                    o.p(imad_shl(reg as u64, vid, 1, espera(1 << B_ALD)));
                    continue;
                }
                Sistema::Vertice | Sistema::Instancia => {
                    o.p(mov(reg as u64, 0));
                    continue;
                }
                Sistema::No => {}
            }
            let Some(f) = el.forma() else { return Err(NoPega::Carga) };
            if el.componentes == 0 || el.componentes > 4 || el.desde % 4 != 0 || el.desde + el.bytes() > paso {
                return Err(NoPega::Carga);
            }
            match f.lugar(componente) {
                // LI2e: la palabra que lo lleva; si no es un float o un
                // entero de 32, se convierte detras (`formatos`).
                Some((palabra, desplazamiento, bits)) => {
                    o.p(ldg(reg as u64, v, el.desde + 4 * palabra, carga(B_CARGAS)));
                    if bits < 32 || !matches!(f.clase, crate::formatos::Clase::Float | crate::formatos::Clase::Uint | crate::formatos::Clase::Sint) {
                        if convertir_n == convertir.len() {
                            return Err(NoPega::Carga);
                        }
                        convertir[convertir_n] = (f, reg as u64, desplazamiento, bits);
                        convertir_n += 1;
                    }
                }
                // El que no trae: 0, y 1 en w (D3D; entero en UINT/SINT).
                None => o.p(mov(reg as u64, f.falta(componente))),
            }
        }
    }
    las_filas(&mut o, cargas, datos, d, false)?;
    // ** LI2e: las conversiones, con TODAS las cargas ya en sus registros (la
    // primera espera la barrera); V y V+1, que ya no hacen falta, de trabajo.
    let mut primera = true;
    for &(f, r, desplazamiento, bits) in &convertir[..convertir_n] {
        let mut w = [(0, 0); crate::formatos::MAX_CONVERSION];
        let n = crate::formatos::conversion(&f, r, v, desplazamiento, bits, &mut w);
        for &x in &w[..n] {
            o.p(if primera { esperando(x, 1 << B_CARGAS) } else { x });
            primera = false;
        }
    }
    // Si hubo cargas de la memoria (LDG): las de un elemento del vertice y
    // las filas del cbuffer. Un cuadro de SV_VertexID sin cbuffer no lee
    // nada, y esperar su barrera seria esperar una que nadie enciende (el
    // juez, R3: lo cazo el banco de LI2e el 12-10). Si ya la espero una
    // conversion, el cuerpo no la vuelve a esperar.
    let leyo = primera && cargas.iter().any(|c| match *c {
        Carga::Fila { .. } => true,
        Carga::Entrada { elemento, componente, .. } => datos.elementos.get(elemento as usize).is_some_and(|e| e.sistema == Sistema::No && e.forma().is_some_and(|f| f.lugar(componente).is_some())),
        _ => false,
    });
    // El cuerpo: su primera espera las cargas; la ultima, lo que tarde en
    // estar lo que escribio (el planificador la calculo para el EXIT, y
    // detras vienen los AST: 6 ciclos, lo mas largo de la tabla del juez).
    for (k, &w) in resto.iter().enumerate() {
        let w = if k == 0 && leyo { esperando(w, 1 << B_CARGAS) } else { w };
        o.p(if k + 1 == resto.len() { al_menos(w, 6) } else { w });
    }
    // 9d: el apunte de la libreta (A y V ya no los usa nadie).
    if let Some(t) = termometro {
        for w in crate::libreta::apunte(t as u64, a, v) {
            o.p(w);
        }
    }
    // Las salidas.
    let mut g = 0u64;
    for e in 0..salidas as u64 {
        let atributo = if e == posicion as u64 {
            0x70
        } else {
            g += 1;
            0x80 + 16 * (g - 1)
        };
        o.p(ast(4 * e, atributo, AST_CONTROL));
    }
    o.p(cu::EXIT_TRAS_AST);
    o.p(cu::BRA);
    cerrar(o, sph_vertice(salidas - 1), total)
}

/// **El de PIXEL**: `cuerpo` (con su EXIT, que se queda: el color sale en
/// R0..R3) usa `registros` y espera `cargas`. `genericos[e]` es el generico
/// que lleva su entrada `e` (`None`: la posicion, que hoy no se recibe).
pub fn pixel(cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, genericos: &[Option<u8>]) -> Result<Pegado, NoPega> {
    let mut g = Pegado::VACIO;
    pixel_en(&mut g, cuerpo, registros, cargas, datos, genericos)?;
    Ok(g)
}

/// [`pixel`], pegando en `g`.
pub fn pixel_en(g: &mut Pegado, cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, genericos: &[Option<u8>]) -> Result<(), NoPega> {
    pixel_con_libreta_en(g, cuerpo, registros, cargas, datos, genericos, None)
}

/// [`pixel_en`] con la LIBRETA (9d): el apunte va antes del EXIT del
/// cuerpo, que espera ademas su lectura.
pub fn pixel_con_libreta_en(g: &mut Pegado, cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, genericos: &[Option<u8>], termometro: Option<u8>) -> Result<(), NoPega> {
    pixel_entero_en(g, cuerpo, registros, cargas, datos, genericos, termometro, Redondeo::NADA, None, 1)
}

/// [`pixel_con_libreta_en`] con el REDONDEO de la salida 0 al formato del
/// render target (LI2d-HDR): detras del cuerpo, antes de su EXIT.
#[allow(clippy::too_many_arguments)]
///
/// ** 12-10: con `mezcla` (no trivial), tras el cuerpo, lo que ya esta en
/// ESE destino mezclado con el pixel nuevo (`mezcla`), antes del redondeo.
pub fn pixel_entero_en(g: &mut Pegado, cuerpo: &[(u64, u64)], registros: u32, cargas: &[Carga], datos: Datos, genericos: &[Option<u8>], termometro: Option<u8>, redondeo: Redondeo, mezcla: Option<(crate::mezcla::Mezcla, crate::destino::Destino, u32)>, objetivos: u32) -> Result<(), NoPega> {
    // LI2d (12-10): los render targets que escribe (1..=8): el k sale de
    // R4k..R4k+3 (lo deja asi el emisor, `4 * salidas` desde R0).
    if objetivos == 0 || objetivos > 8 {
        return Err(NoPega::Salidas);
    }
    g.n = 0;
    let (resto, exit) = partir(cuerpo)?;
    cargas_propias(cargas, registros)?;
    termometro_propio(termometro, registros)?;
    let (a, d, v, _, total) = propios(registros)?;
    let mut o = Poner { g, lleno: false };
    let mut sph = sph_pixel_v0_sin_generico();
    // LI2d: OmapTarget, 4 bits (R G B A) por target desde el 576 (la SPH de
    // V0 trae los del 0).
    sph[18] |= if objetivos == 8 { u32::MAX } else { (1u32 << (4 * objetivos)) - 1 };
    // ** DL12: un cuerpo que tira pixeles (KILL) lo dice en su SPH; si no,
    // su KILL seria un NOP y una excepcion (y el juez lo para: R5).
    if cuerpo.iter().any(|w| w.0 & 0xFFF == 0x95B) {
        sph[0] |= crate::raster::MATA_PIXELES;
    }
    // Las asas de las texturas: un MOV cada una (acoplado: su latencia la
    // cubre su propia espera de 6 ciclos, `mov`).
    for c in cargas {
        if let Carga::Asa { textura, reg } = *c {
            o.p(mov(reg as u64, crate::texturas::asa(textura as u32, textura as u32)));
        }
    }
    // Los IPA; el ultimo con el control largo de T2a.
    let entradas = cargas.iter().filter(|c| matches!(c, Carga::Entrada { .. } | Carga::Posicion { .. })).count();
    let mut k = 0;
    for c in cargas {
        // LI2a: la posicion de la pantalla, del atributo de sistema.
        if let Carga::Posicion { componente, reg } = *c {
            k += 1;
            let control = if k == entradas { IPA_ULTIMO } else { IPA_CONTROL };
            o.p(ipa(reg as u64, ATRIBUTO_POSICION + componente as u64, control));
            sph[5] |= 1 << (BIT_IMAP_POSICION_X - 160 + componente as u32);
            continue;
        }
        if let Carga::Entrada { elemento, componente, reg } = *c {
            let Some(&Some(g)) = genericos.get(elemento as usize) else { return Err(NoPega::Carga) };
            let g = g as u32;
            if componente > 3 || g >= 8 {
                return Err(NoPega::Carga);
            }
            k += 1;
            let control = if k == entradas { IPA_ULTIMO } else { IPA_CONTROL };
            o.p(ipa(reg as u64, 0x20 + 4 * g as u64 + componente as u64, control));
            sph[6 + g as usize / 4] |= crate::color3d::LINEAL << (8 * (g % 4) + 2 * componente as u32);
        }
    }
    let filas = cargas.iter().any(|c| matches!(c, Carga::Fila { .. }));
    if filas {
        la_tabla(&mut o, a, d);
        las_filas(&mut o, cargas, datos, d, true)?;
        sph[0] |= crate::raster::LEE_O_ESCRIBE;
    }
    let mascara = if entradas > 0 { 1 << 0 } else { 0 } | if filas { 1 << B_CARGAS } else { 0 };
    let mezcla = mezcla.filter(|(m, _, _)| !m.trivial());
    if let Some((m, _, _)) = mezcla {
        if m.valida().is_err() {
            return Err(NoPega::Carga);
        }
    }
    // La mezcla (12-10): tras el cuerpo, con a..a+8 de trabajo (las filas ya
    // se leyeron; el apunte de la libreta pone los suyos).
    let mezclar = |o: &mut Poner, sph: &mut [u32; SPH]| {
        let Some((m, dst, desplazamiento)) = mezcla else { return };
        o.p(ipa(a, ATRIBUTO_POSICION, IPA_CONTROL));
        o.p(ipa(a + 1, ATRIBUTO_POSICION + 1, IPA_ULTIMO));
        sph[5] |= 0b11 << (BIT_IMAP_POSICION_X - 160);
        sph[0] |= crate::raster::LEE_O_ESCRIBE;
        let mut primera = true;
        crate::mezcla::leer_destino(&dst, desplazamiento, a, &mut |w| {
            o.p(if primera { esperando(w, 1 << 0) } else { w });
            primera = false;
        });
        let mut primera = true;
        crate::mezcla::mezclar(&m, &dst, a, &mut |w| {
            o.p(if primera { esperando(w, 1 << crate::mezcla::B_LEER) } else { w });
            primera = false;
        });
    };
    match termometro {
        None if redondeo == Redondeo::NADA && mezcla.is_none() => {
            for (k, &w) in cuerpo.iter().enumerate() {
                o.p(if k == 0 { esperando(w, mascara) } else { w });
            }
        }
        // LI2d-HDR: el cuerpo sin su EXIT, la mezcla, el redondeo y el EXIT.
        None => {
            for (k, &w) in resto.iter().enumerate() {
                let w = if k == 0 { esperando(w, mascara) } else { w };
                o.p(if k + 1 == resto.len() { al_menos(w, 6) } else { w });
            }
            mezclar(&mut o, &mut sph);
            redondear(&mut o, redondeo);
            o.p(exit);
        }
        // 9d: el cuerpo sin su EXIT, el apunte, y el EXIT esperando su
        // lectura. La ultima del cuerpo, con la espera de los AST (6).
        Some(t) => {
            for (k, &w) in resto.iter().enumerate() {
                let w = if k == 0 { esperando(w, mascara) } else { w };
                o.p(if k + 1 == resto.len() { al_menos(w, 6) } else { w });
            }
            mezclar(&mut o, &mut sph);
            redondear(&mut o, redondeo);
            for w in crate::libreta::apunte(t as u64, a, v) {
                o.p(w);
            }
            o.p(esperando(exit, 1 << crate::libreta::B_LECTURA));
            sph[0] |= crate::raster::LEE_O_ESCRIBE;
        }
    }
    o.p(cu::BRA);
    cerrar(o, sph, total)
}

/// 9d: el termometro, un registro DEL CUERPO (los del pegamento son del
/// kernel).
fn termometro_propio(termometro: Option<u8>, registros: u32) -> Result<(), NoPega> {
    match termometro {
        Some(t) if t as u32 >= registros => Err(NoPega::Registros),
        _ => Ok(()),
    }
}

/// La SPH de pixel de V0 sin su generico 0 (lo pone [`pixel`] segun lo que
/// lea).
const fn sph_pixel_v0_sin_generico() -> [u32; SPH] {
    let mut h = sph_pixel_v0();
    h[6] = 0;
    h[7] = 0;
    h
}

/// La SPH de vertice: la de V0 (posicion, generico 0, `DoesLoadOrStore`) y
/// los genericos 1..`genericos` enteros (4 bits cada uno desde el 432).
pub const fn sph_vertice(genericos: u32) -> [u32; SPH] {
    let mut h = sph_vertice_v0();
    let mut b = 436;
    while b < 432 + 4 * genericos as usize {
        h = bit(h, b);
        b += 1;
    }
    h
}

#[cfg(test)]
mod pruebas {
    #[allow(unused_imports)]
    use crate::tuberia::REGISTROS;
    extern crate std;

    use super::*;
    use crate::sass::juez::{juzgar, Contexto};
    use bmo_proton_x::dxil::{self, programa::compilar};
    use bmo_proton_x_sm86::{emitir_con, Abi, Precarga};

    const RAIZ: &str = "../../../shared/proton-x/prueba/";

    fn cargas(e: &bmo_proton_x_sm86::Emitido) -> std::vec::Vec<Carga> {
        e.precargas
            .iter()
            .map(|p| match *p {
                Precarga::Entrada { elemento, componente, reg } => Carga::Entrada { elemento, componente, reg },
                Precarga::Fila { fila, reg } => Carga::Fila { fila, reg },
                // Los cubos no muestrean: la unica textura seria la 0.
                Precarga::Asa { reg, .. } | Precarga::AsaPar { reg, .. } => Carga::Asa { textura: 0, reg },
            })
            .collect()
    }

    fn emitido(f: &str) -> (bmo_proton_x::dxil::programa::Programa, bmo_proton_x_sm86::Emitido) {
        let d = std::fs::read(std::format!("{RAIZ}{f}")).unwrap();
        let p = compilar(&dxil::leer(&d).unwrap()).unwrap();
        let e = emitir_con(&p, 64, Abi::Registros).unwrap();
        (p, e)
    }

    const DATOS: Datos = Datos::float4(9, 3);
    /// Las entradas del de pixel del cubo: la posicion, la normal (generico
    /// 0) y el color (1).
    const GENERICOS: [Option<u8>; 3] = [None, Some(0), Some(1)];

    /// Los de BMOX-12 (SM5 de FXC) y los del cubo de dxc, pegados: PERFECTO
    /// Y PRECISO para el juez, con los registros de VERRANO y su SPH.
    #[test]
    fn lo_pegado_es_perfecto() {
        for (vs, ps) in [("sombras/f3ef42a0.cso", "sombras/4d67f5e4.cso"), ("cubo_vs.dxil", "cubo_ps.dxil")] {
            let (pv, ev) = emitido(vs);
            let v = vertice(&ev.codigo, ev.registros, &cargas(&ev), DATOS, pv.salidas as u32, 0).unwrap();
            let r = juzgar(v.codigo(), &Contexto { registros: REGISTROS, sph: Some(&v.sph) });
            assert!(r.is_ok(), "{vs}: {}", r.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
            let (_, ep) = emitido(ps);
            let p = pixel(&ep.codigo, ep.registros, &cargas(&ep), DATOS, &GENERICOS).unwrap();
            let r = juzgar(p.codigo(), &Contexto { registros: REGISTROS, sph: Some(&p.sph) });
            assert!(r.is_ok(), "{ps}: {}", r.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
            std::eprintln!("{vs}: {} instrucciones, {} registros; {ps}: {} y {}", v.n, v.registros, p.n, p.registros);
        }
    }

    /// *** 9d: con la LIBRETA (el cuerpo emitido con su termometro), lo
    /// pegado sigue siendo PERFECTO Y PRECISO; lleva el apunte una vez --
    /// el FSETP sobre SU termometro, la direccion del kernel y el STG con
    /// guarda --, el de pixel declara que escribe memoria, y su EXIT espera
    /// la lectura del STG. Un termometro fuera del cuerpo, no se pega.
    #[test]
    fn lo_pegado_con_libreta_es_perfecto() {
        use bmo_proton_x_sm86::emitir_libreta;
        let libreta = |f: &str| {
            let d = std::fs::read(std::format!("{RAIZ}{f}")).unwrap();
            let p = compilar(&dxil::leer(&d).unwrap()).unwrap();
            let e = emitir_libreta(&p, 64, Abi::Registros, true).unwrap();
            (p, e)
        };
        for (vs, ps) in [("sombras/f3ef42a0.cso", "sombras/4d67f5e4.cso"), ("cubo_vs.dxil", "cubo_ps.dxil")] {
            let (pv, ev) = libreta(vs);
            let mut v = Pegado::VACIO;
            vertice_con_libreta_en(&mut v, &ev.codigo, ev.registros, &cargas(&ev), DATOS, pv.salidas as u32, 0, ev.termometro).unwrap();
            let r = juzgar(v.codigo(), &Contexto { registros: REGISTROS, sph: Some(&v.sph) });
            assert!(r.is_ok(), "{vs}: {}", r.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
            let (_, ep) = libreta(ps);
            let mut p = Pegado::VACIO;
            pixel_con_libreta_en(&mut p, &ep.codigo, ep.registros, &cargas(&ep), DATOS, &GENERICOS, ep.termometro).unwrap();
            let r = juzgar(p.codigo(), &Contexto { registros: REGISTROS, sph: Some(&p.sph) });
            assert!(r.is_ok(), "{ps}: {}", r.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
            for (g, e) in [(&v, &ev), (&p, &ep)] {
                let t = e.termometro.unwrap() as u64;
                assert_eq!(g.codigo().iter().filter(|w| w.0 & 0xFFF == 0x986).count(), 1, "un STG");
                let stg = g.codigo().iter().position(|w| w.0 & 0xFFF == 0x986).unwrap();
                assert_eq!(g.codigo()[stg].0 >> 12 & 0xF, 0, "con guarda P0");
                assert_eq!(g.codigo()[stg - 4], crate::libreta::fsetp_nan(t, crate::cubo::ALU), "el FSETP sobre su termometro");
                assert_ne!(g.sph[0] & crate::raster::LEE_O_ESCRIBE, 0);
            }
            assert_eq!(p.codigo()[p.n - 2].1 >> (41 + 11) & 0x3F & 1 << crate::libreta::B_LECTURA, 1 << crate::libreta::B_LECTURA, "el EXIT espera el STG");
            // Fuera del cuerpo: el de pixel con su termometro en un registro del pegamento.
            let mut x = Pegado::VACIO;
            assert_eq!(pixel_con_libreta_en(&mut x, &ep.codigo, ep.registros, &cargas(&ep), DATOS, &GENERICOS, Some(ep.registros as u8)).unwrap_err(), NoPega::Registros);
        }
    }

    /// Cada carga lee de donde tiene que leer: el registro, la base (V para
    /// una entrada, D para una fila) y el desplazamiento.
    #[test]
    fn cada_carga_lee_su_sitio() {
        let (pv, ev) = emitido("sombras/f3ef42a0.cso");
        let cs = cargas(&ev);
        let v = vertice(&ev.codigo, ev.registros, &cs, DATOS, pv.salidas as u32, 0).unwrap();
        let (_, d, vv, _, _) = propios(ev.registros).unwrap();
        let ldgs: std::vec::Vec<(u64, u64, u64)> = v.codigo().iter().filter(|w| w.0 & 0xFFFF == 0x7981).map(|w| (w.0 >> 16 & 0xFF, w.0 >> 24 & 0xFF, w.0 >> 40 & 0xFF_FFFF)).collect();
        let mut esperado = std::vec::Vec::new();
        for c in &cs {
            if let Carga::Entrada { elemento, componente, reg } = *c {
                esperado.push((reg as u64, vv, 16 * elemento as u64 + 4 * componente as u64));
            }
        }
        for c in &cs {
            if let Carga::Fila { fila, reg } = *c {
                for k in 0..4 {
                    esperado.push((reg as u64 + k, d, 16 * fila as u64 + 4 * k));
                }
            }
        }
        // Las dos primeras son las de la tabla.
        assert_eq!(&ldgs[2..], &esperado[..]);
        assert_eq!(ldgs.len(), 2 + 10 + 7 * 4);
        // Las salidas: la posicion a 0x70, la normal a 0x80, el color a 0x90.
        let asts: std::vec::Vec<(u64, u64)> = v.codigo().iter().filter(|w| w.0 & 0xFFFF == 0x7322).map(|w| (w.0 >> 32 & 0xFF, w.0 >> 40 & 0x3FF)).collect();
        assert_eq!(asts, [(0, 0x70), (4, 0x80), (8, 0x90)]);
    }

    /// El de pixel: cada entrada por el IPA de su generico (la normal, el
    /// 0; el color, el 1), y la SPH los declara.
    #[test]
    fn el_pixel_recibe_sus_genericos() {
        let (_, ep) = emitido("sombras/4d67f5e4.cso");
        let cs = cargas(&ep);
        let p = pixel(&ep.codigo, ep.registros, &cs, DATOS, &GENERICOS).unwrap();
        let ipas: std::vec::Vec<(u64, u64)> = p.codigo().iter().filter(|w| w.0 & 0xFFFF == 0x7326).map(|w| (w.0 >> 16 & 0xFF, w.1 & 0x3FF)).collect();
        let mut esperado = std::vec::Vec::new();
        for c in &cs {
            if let Carga::Entrada { elemento, componente, reg } = *c {
                esperado.push((reg as u64, 0x20 + 4 * (elemento as u64 - 1) + componente as u64));
            }
        }
        assert_eq!(ipas, esperado);
        // Normal: X, Y, Z del generico 0; color: los cuatro del 1.
        assert_eq!(p.sph[6], 0b11_11_11 | 0xFF << 8);
        assert_ne!(p.sph[0] & crate::raster::LEE_O_ESCRIBE, 0, "lee la luz con LDG");
    }

    /// P3b4a: el vertice TAL CUAL lo da un juego -- la posicion float3 en
    /// el 0, la normal float3 en el 12, el color float4 en el 24, 40 B cada
    /// uno --: cada componente que trae se carga de su byte, y el que no
    /// trae es el de D3D (w de la posicion = 1, un MOV). Y el juez: PERFECTO.
    #[test]
    fn el_vertice_del_juego_tal_cual() {
        const JUEGO: [Elemento; 3] = [Elemento::memoria(0, 3), Elemento::memoria(12, 3), Elemento::memoria(24, 4)];
        let datos = Datos { filas: 9, paso: 40, elementos: &JUEGO };
        let (pv, ev) = emitido("sombras/f3ef42a0.cso");
        let cs = cargas(&ev);
        let v = vertice(&ev.codigo, ev.registros, &cs, datos, pv.salidas as u32, 0).unwrap();
        let r = juzgar(v.codigo(), &Contexto { registros: REGISTROS, sph: Some(&v.sph) });
        assert!(r.is_ok(), "{}", r.map(|_| std::string::String::new()).unwrap_or_else(|b| std::format!("{b}")));
        let (_, _, vv, _, _) = propios(ev.registros).unwrap();
        for c in &cs {
            if let Carga::Entrada { elemento, componente, reg } = *c {
                let el = JUEGO[elemento as usize];
                let w = v.codigo().iter().find(|w| w.0 >> 16 & 0xFF == reg as u64 && matches!(w.0 & 0xFFFF, 0x7981 | 0x7424)).unwrap();
                if componente < el.componentes {
                    assert_eq!((w.0 & 0xFFFF, w.0 >> 24 & 0xFF, w.0 >> 40 & 0xFF_FFFF), (0x7981, vv, (el.desde + 4 * componente as u32) as u64), "{c:?}");
                } else {
                    assert_eq!((w.0 & 0xFFFF, w.0 >> 32), (0x7424, if componente == 3 { 0x3F80_0000 } else { 0 }), "{c:?}: el de D3D");
                }
            }
        }
        // El paso 40 = 32 + 8: dos IMAD.SHL.
        assert_eq!(v.codigo().iter().filter(|w| w.0 & 0xFFFF == 0x7824 && w.1 & 0xFFFF_FFFF == 0x078E_00FF).count(), 2);
        // Un elemento que se sale del vertice, no.
        let malo = [Elemento::memoria(0, 3), Elemento::memoria(12, 3), Elemento::memoria(32, 4)];
        assert_eq!(vertice(&ev.codigo, ev.registros, &cs, Datos { filas: 9, paso: 40, elementos: &malo }, pv.salidas as u32, 0).unwrap_err(), NoPega::Carga);
    }

    #[test]
    fn el_generico_de_cada_salida() {
        assert_eq!([generico(0, 0), generico(1, 0), generico(2, 0)], [None, Some(0), Some(1)]);
        assert_eq!([generico(0, 1), generico(1, 1), generico(2, 1)], [Some(0), None, Some(1)]);
    }

    /// Lo que no se puede, se dice.
    #[test]
    fn lo_que_no_se_pega() {
        let (pv, ev) = emitido("sombras/f3ef42a0.cso");
        let cs = cargas(&ev);
        assert_eq!(vertice(&ev.codigo, ev.registros, &cs, Datos::float4(3, 3), pv.salidas as u32, 0).unwrap_err(), NoPega::Carga);
        // LI2b (12-10): 60 ya caben (hasta `tuberia::MAX_REGISTROS`); 241 no.
        assert!(vertice(&ev.codigo, 60, &cs, DATOS, pv.salidas as u32, 0).is_ok());
        assert_eq!(vertice(&ev.codigo, 241, &cs, DATOS, pv.salidas as u32, 0).unwrap_err(), NoPega::Registros);
        assert_eq!(vertice(&ev.codigo[..3], ev.registros, &cs, DATOS, 3, 0).unwrap_err(), NoPega::Cuerpo);
        assert_eq!(vertice(&ev.codigo, ev.registros, &cs, DATOS, 3, 3).unwrap_err(), NoPega::Salidas);
        let (_, ep) = emitido("sombras/4d67f5e4.cso");
        assert_eq!(pixel(&ep.codigo, ep.registros, &cargas(&ep), DATOS, &[None, None, Some(1)]).unwrap_err(), NoPega::Carga);
    }

    /// ** LI2e (12-10): SV_VertexID es el `vid` del ALD (un IMAD.SHL por 1
    /// que espera su barrera) y lo demas del elemento, 0; sin paso, ni una
    /// carga de la memoria -- y el cuerpo no espera una barrera que nadie
    /// enciende (el juez lo aprueba). La prueba del NO: un elemento de la
    /// memoria sin paso no se pega.
    #[test]
    fn sv_vertexid_sale_del_vid() {
        let cuerpo = [mov(0, 0x3F80_0000), mov(1, 0), mov(2, 0), mov(3, 0x3F80_0000), cu::EXIT];
        let cargas = [Carga::Entrada { elemento: 0, componente: 0, reg: 4 }, Carga::Entrada { elemento: 0, componente: 1, reg: 5 }];
        let els = [Elemento::de_sistema(Sistema::Vertice)];
        let v = vertice(&cuerpo, 8, &cargas, Datos { filas: 0, paso: 0, elementos: &els }, 1, 0).expect("se pega");
        let c = v.codigo();
        assert_eq!(c.iter().filter(|w| w.0 & 0xFFFF == 0x7981).count(), 2, "solo los dos LDG de la tabla: ni un vertice de la memoria");
        let (_, _, _, vid, _) = propios(8).unwrap();
        assert!(c.iter().any(|w| w.0 & 0xFFFF == 0x7824 && w.0 >> 16 & 0xFF == 4 && w.0 >> 24 & 0xFF == vid && w.0 >> 32 == 1), "IMAD.SHL R4, vid, 1");
        assert!(c.iter().any(|w| w.0 & 0xFFFF == 0x7424 && w.0 >> 16 & 0xFF == 5 && w.0 >> 32 == 0), "MOV R5, 0");
        let malo = [Elemento::memoria(0, 4)];
        assert_eq!(vertice(&cuerpo, 8, &cargas, Datos { filas: 0, paso: 0, elementos: &malo }, 1, 0).unwrap_err(), NoPega::Carga);
    }
}
