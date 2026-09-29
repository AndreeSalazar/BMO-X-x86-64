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
use crate::tuberia::{carga, espera, iadd3_acarreo, imad_shl, imad_x, ipa, ldg, sph_pixel as sph_pixel_v0, sph_vertice as sph_vertice_v0, IPA_CONTROL, IPA_ULTIMO, REGISTROS, TABLA};

/// Lo que el pegamento carga antes del cuerpo (lo que `bmo-proton-x-sm86`
/// llama `Precarga`, dicho aqui sin nombrarlo: el driver no depende de
/// PROTON-X).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carga {
    /// El componente `componente` de la entrada `elemento`, en `reg`.
    Entrada { elemento: u8, componente: u8, reg: u8 },
    /// La fila `fila` del cbuffer, en `reg`..`reg + 3`.
    Fila { fila: u16, reg: u8 },
}

/// Donde esta un elemento de entrada dentro de un vertice (del input layout
/// del juego).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elemento {
    /// Su byte dentro del vertice.
    pub desde: u32,
    /// Cuantos float trae (1..4).
    pub componentes: u8,
}

/// float4 seguidos: el vertice de E5 (`bmo_cubo::tanda::datos`).
const FLOAT4: [Elemento; 8] = {
    let mut e = [Elemento { desde: 0, componentes: 4 }; 8];
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
    /// El cuerpo y el pegamento no caben en `tuberia::REGISTROS`.
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
            Carga::Fila { reg, .. } => (reg as u32, 4),
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
    if total + RESERVADOS > REGISTROS {
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
    g.n = 0;
    let (resto, _) = partir(cuerpo)?;
    cargas_propias(cargas, registros)?;
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
    if paso == 0 {
        return Err(NoPega::Carga);
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
    for c in cargas {
        if let Carga::Entrada { elemento, componente, reg } = *c {
            let Some(el) = datos.elementos.get(elemento as usize) else { return Err(NoPega::Carga) };
            if componente > 3 || el.componentes == 0 || el.componentes > 4 || el.desde + 4 * el.componentes as u32 > paso {
                return Err(NoPega::Carga);
            }
            if componente < el.componentes {
                o.p(ldg(reg as u64, v, el.desde + 4 * componente as u32, carga(B_CARGAS)));
            } else {
                // El que no trae: 0, y 1 en w (D3D).
                o.p(mov(reg as u64, if componente == 3 { 0x3F80_0000 } else { 0 }));
            }
        }
    }
    las_filas(&mut o, cargas, datos, d, false)?;
    // El cuerpo: su primera espera las cargas; la ultima, lo que tarde en
    // estar lo que escribio (el planificador la calculo para el EXIT, y
    // detras vienen los AST: 6 ciclos, lo mas largo de la tabla del juez).
    for (k, &w) in resto.iter().enumerate() {
        let w = if k == 0 { esperando(w, 1 << B_CARGAS) } else { w };
        o.p(if k + 1 == resto.len() { al_menos(w, 6) } else { w });
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
    g.n = 0;
    partir(cuerpo)?;
    cargas_propias(cargas, registros)?;
    let (a, d, _, _, total) = propios(registros)?;
    let mut o = Poner { g, lleno: false };
    let mut sph = sph_pixel_v0_sin_generico();
    // Los IPA; el ultimo con el control largo de T2a.
    let entradas = cargas.iter().filter(|c| matches!(c, Carga::Entrada { .. })).count();
    let mut k = 0;
    for c in cargas {
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
    for (k, &w) in cuerpo.iter().enumerate() {
        o.p(if k == 0 { esperando(w, mascara) } else { w });
    }
    o.p(cu::BRA);
    cerrar(o, sph, total)
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
        const JUEGO: [Elemento; 3] = [Elemento { desde: 0, componentes: 3 }, Elemento { desde: 12, componentes: 3 }, Elemento { desde: 24, componentes: 4 }];
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
        let malo = [Elemento { desde: 0, componentes: 3 }, Elemento { desde: 12, componentes: 3 }, Elemento { desde: 32, componentes: 4 }];
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
        assert_eq!(vertice(&ev.codigo, 60, &cs, DATOS, pv.salidas as u32, 0).unwrap_err(), NoPega::Registros);
        assert_eq!(vertice(&ev.codigo[..3], ev.registros, &cs, DATOS, 3, 0).unwrap_err(), NoPega::Cuerpo);
        assert_eq!(vertice(&ev.codigo, ev.registros, &cs, DATOS, 3, 3).unwrap_err(), NoPega::Salidas);
        let (_, ep) = emitido("sombras/4d67f5e4.cso");
        assert_eq!(pixel(&ep.codigo, ep.registros, &cargas(&ep), DATOS, &[None, None, Some(1)]).unwrap_err(), NoPega::Carga);
    }
}
