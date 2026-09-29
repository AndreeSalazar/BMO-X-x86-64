//! **P3b4c: LA RECETA (VRN2)** -- lo que una APP le da al kernel para que la
//! 3060 dibuje: los CUERPOS de sus dos sombreadores, como se cargan sus
//! entradas, y los DATOS. El pegamento --todas las lecturas de memoria-- lo
//! pone el KERNEL.
//!
//! capa: puro -- el formato, lo que se valida y el pegado; los registros y la
//! RAM los toca el kernel (L8)
//!
//! [eje]     SEGURIDAD -- la puerta de las apps no tiene la autoridad
//!           `MAQUINA`: una app no puede decir a que memoria de la 3060 va
//!
//! # Por que el cuerpo y no el programa
//!
//! VRN1 lleva los programas ENTEROS, y los fabrica el anfitrion: viajan
//! dentro de `d.bex`. Una app los fabrica ella (PROTON-X traduce los DXIL del
//! juego en marcha), y el juez de R0..R6 dice si un programa esta BIEN HECHO,
//! no a QUE MEMORIA va: un `STG` bien cronometrado al lienzo del escritorio
//! es PERFECTO Y PRECISO. Asi que:
//!
//! ```text
//!    la app      el CUERPO (lista blanca: juez R7), sus cargas (que entrada
//!                o fila va a que registro), el input layout, los DATOS
//!    el kernel   comprueba que cada lectura cae DENTRO de los datos, pega
//!                (`pegamento`: todas las direcciones son suyas), juzga el
//!                programa entero (R0..R6) y dibuja en la RAM de la app
//! ```
//!
//! # La cabecera (96 B)
//!
//! ```text
//!    +0   "VRN2"            +4  0 (la ficha del GR la pone el kernel)
//!    +8   n (indices, o vertices seguidos)
//!    +12  bytes del cuerpo de vertice   +16 los del de pixel (16 por
//!         instruccion, con su EXIT)
//!    +20  bit 0: limpiar el destino antes (el `ClearRenderTargetView` de la
//!         app, por la 3060)   +24  con que pixel, como va en memoria
//!    +28  bytes de los DATOS
//!    +32..+64  los campos de VRN1: los indices, el estado (descarte, giro,
//!         la Z), los vertices y el DESTINO -- OBLIGATORIO: una receta
//!         dibuja en la RAM de quien la manda, nunca en la pantalla
//!    +64  registros del de vertice | del de pixel << 8 | salidas << 16 |
//!         la posicion << 24
//!    +68  filas del cbuffer (16 bits) | elementos << 16 | genericos << 24
//!    +72  paso (bytes de un vertice)
//!    +76  cargas del de vertice (16 bits) | del de pixel << 16
//!    +80  con la limpieza de la Z (bit 9 del estado): su valor (f32)
//!    +84  cuantas TEXTURAS lee el de pixel (P3b4c.8 T2; hasta
//!         `texturas::MAX_TEXTURAS`)
//!    +88..+96  0
//! ```
//!
//! Y detras, seguidos: el cuerpo de vertice, el de pixel, los elementos (8 B:
//! su byte y cuantos float), las cargas (4 B: `0, elemento, componente, reg`,
//! `1, fila baja, fila alta, reg` o `2, textura, 0, reg` -- el ASA, solo en
//! el de pixel), los genericos (1 B cada uno, 0xFF la posicion), las
//! texturas (48 B cada una, `texturas::DeApp`: SU memoria, sus medidas y su
//! muestreador), relleno hasta 16, y los DATOS.

use crate::pegamento::{self, Carga, Datos, Elemento, NoPega};
use crate::sass::juez::{self, Bodrio, MAX_INSTRUCCIONES};
use crate::texturas::{DeApp, BYTES_DE_APP, MAX_TEXTURAS};
use crate::tuberia::{self as tu, u32le, Dibujo, Paquete, DATOS_MAX, HUECO, MAX_VERTICES, SIN_INDICES};

/// `"VRN2"`.
pub const MAGIA_2: u32 = u32::from_le_bytes(*b"VRN2");
pub const CABECERA_2: usize = 96;
/// Lo mas que trae una receta de cada cosa.
pub const MAX_ELEMENTOS: usize = 16;
pub const MAX_CARGAS: usize = 64;
pub const MAX_GENERICOS: usize = 16;
/// Un cuerpo: 128 instrucciones de 16 bytes.
pub const MAX_CUERPO: usize = 16 * MAX_INSTRUCCIONES;
/// Lo mas que mide una receta: la caja que la app reserva.
pub const MAX_RECETA: usize = CABECERA_2 + 2 * MAX_CUERPO + 8 * MAX_ELEMENTOS + 4 * 2 * MAX_CARGAS + MAX_GENERICOS + BYTES_DE_APP * MAX_TEXTURAS + 16 + DATOS_MAX;
/// Un generico que no hay (la entrada es la posicion).
const SIN_GENERICO: u8 = 0xFF;
/// Lo mas que se acepta de filas del cbuffer (64 KiB de DATOS / 16).
const MAX_FILAS: u32 = (DATOS_MAX / 16) as u32;

/// **Una receta leida** (o para escribir).
#[derive(Clone, Copy, Debug)]
pub struct Receta<'a> {
    pub n: usize,
    /// Los cuerpos, tal como viajan: 16 bytes por instruccion.
    pub vs: &'a [u8],
    pub ps: &'a [u8],
    pub registros_vs: u32,
    pub registros_ps: u32,
    /// Las salidas del de vertice, y cual es la posicion.
    pub salidas: u32,
    pub posicion: u32,
    pub filas: u32,
    pub paso: u32,
    pub elementos: [Elemento; MAX_ELEMENTOS],
    pub n_elementos: usize,
    pub cargas_vs: [Carga; MAX_CARGAS],
    pub n_cargas_vs: usize,
    pub cargas_ps: [Carga; MAX_CARGAS],
    pub n_cargas_ps: usize,
    pub genericos: [Option<u8>; MAX_GENERICOS],
    pub n_genericos: usize,
    pub datos: &'a [u8],
    pub dibujo: Dibujo,
    /// Las texturas del de pixel (`dibujo.texturas` de ellas).
    pub texturas: [DeApp; MAX_TEXTURAS],
}

/// Un hueco de carga (para los arreglos a medio llenar).
pub const NINGUNA: Carga = Carga::Fila { fila: 0, reg: 0 };
pub const NINGUNO: Elemento = Elemento { desde: 0, componentes: 0 };

/// Por que una receta no dibuja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoReceta {
    /// La cabecera o las medidas no se sostienen (lo dice `leer`).
    Forma,
    /// Un cuerpo toca lo que no es suyo (juez R7).
    Cuerpo(&'static str, Bodrio),
    /// El pegamento no puede.
    Pegamento(&'static str, NoPega),
    /// El programa pegado es un BODRIO (R0..R6): no deberia pasar nunca.
    Juez(&'static str, Bodrio),
}

impl<'a> Receta<'a> {
    pub fn cargas_vs(&self) -> &[Carga] {
        &self.cargas_vs[..self.n_cargas_vs]
    }
    pub fn cargas_ps(&self) -> &[Carga] {
        &self.cargas_ps[..self.n_cargas_ps]
    }
    pub fn elementos(&self) -> &[Elemento] {
        &self.elementos[..self.n_elementos]
    }
    pub fn genericos(&self) -> &[Option<u8>] {
        &self.genericos[..self.n_genericos]
    }
    pub fn texturas(&self) -> &[DeApp] {
        &self.texturas[..(self.dibujo.texturas as usize).min(MAX_TEXTURAS)]
    }
    /// Lo que ocupa detras de la cabecera, hasta los DATOS (con el relleno).
    fn medio(&self) -> usize {
        (self.vs.len() + self.ps.len() + 8 * self.n_elementos + 4 * (self.n_cargas_vs + self.n_cargas_ps) + self.n_genericos + BYTES_DE_APP * self.texturas().len()).next_multiple_of(16)
    }
    /// Lo que mide la receta entera.
    pub fn medida(&self) -> usize {
        CABECERA_2 + self.medio() + self.datos.len()
    }
}

fn carga_de(b: &[u8]) -> Option<Carga> {
    match b[0] {
        0 => Some(Carga::Entrada { elemento: b[1], componente: b[2], reg: b[3] }),
        1 => Some(Carga::Fila { fila: b[1] as u16 | (b[2] as u16) << 8, reg: b[3] }),
        2 if b[2] == 0 => Some(Carga::Asa { textura: b[1], reg: b[3] }),
        _ => None,
    }
}

fn carga_a(c: Carga) -> [u8; 4] {
    match c {
        Carga::Entrada { elemento, componente, reg } => [0, elemento, componente, reg],
        Carga::Fila { fila, reg } => [1, fila as u8, (fila >> 8) as u8, reg],
        Carga::Asa { textura, reg } => [2, textura, 0, reg],
    }
}

/// Un cuerpo: entero de instrucciones, al menos una, y cabe.
const fn cuerpo_valido(bytes: usize) -> bool {
    bytes >= 16 && bytes % 16 == 0 && bytes <= MAX_CUERPO
}

/// **Cuanto mide la receta** que empieza con esta cabecera (lo que el kernel
/// lee de la RAM de la app antes de mirar el resto), o `None` si la
/// cabecera ya no se sostiene. El resto lo comprueba [`leer`].
pub fn medida(cabecera: &[u8]) -> Option<usize> {
    if cabecera.len() < CABECERA_2 || u32le(cabecera, 0) != MAGIA_2 {
        return None;
    }
    let (bvs, bps, bdatos) = (u32le(cabecera, 12) as usize, u32le(cabecera, 16) as usize, u32le(cabecera, 28) as usize);
    let (medidas, cargas) = (u32le(cabecera, 68), u32le(cabecera, 76));
    let (n_elementos, n_genericos) = ((medidas >> 16 & 0xFF) as usize, (medidas >> 24) as usize);
    let (cv, cp) = ((cargas & 0xFFFF) as usize, (cargas >> 16) as usize);
    let n_texturas = u32le(cabecera, 84) as usize;
    if !cuerpo_valido(bvs) || !cuerpo_valido(bps) || bdatos > DATOS_MAX || n_elementos > MAX_ELEMENTOS || n_genericos > MAX_GENERICOS || cv > MAX_CARGAS || cp > MAX_CARGAS || n_texturas > MAX_TEXTURAS {
        return None;
    }
    Some(CABECERA_2 + (bvs + bps + 8 * n_elementos + 4 * (cv + cp) + n_genericos + BYTES_DE_APP * n_texturas).next_multiple_of(16) + bdatos)
}

/// **Leer una receta**, comprobandolo todo lo que se puede sin pegar: las
/// medidas, el destino (obligatorio), cada elemento dentro de su vertice,
/// cada carga dentro de lo que hay, los DATOS con todos los vertices dentro
/// y cada indice de un vertice que esta. `None` si algo no se sostiene.
pub fn leer(b: &[u8]) -> Option<Receta<'_>> {
    if b.len() < CABECERA_2 || u32le(b, 0) != MAGIA_2 || u32le(b, 4) != 0 || u32le(b, 20) > 1 || (u32le(b, 20) == 0 && u32le(b, 24) != 0) || u32le(b, 84) as usize > MAX_TEXTURAS || b[88..CABECERA_2].iter().any(|&x| x != 0) {
        return None;
    }
    let n_texturas = u32le(b, 84) as usize;
    let n = u32le(b, 8) as usize;
    let (bvs, bps, bdatos) = (u32le(b, 12) as usize, u32le(b, 16) as usize, u32le(b, 28) as usize);
    if n == 0 || n % 3 != 0 || n > MAX_VERTICES || !cuerpo_valido(bvs) || !cuerpo_valido(bps) || bdatos == 0 || bdatos % 16 != 0 || bdatos > DATOS_MAX {
        return None;
    }
    let (regs, medidas, paso, cargas) = (u32le(b, 64), u32le(b, 68), u32le(b, 72), u32le(b, 76));
    let (registros_vs, registros_ps, salidas, posicion) = (regs & 0xFF, regs >> 8 & 0xFF, regs >> 16 & 0xFF, regs >> 24);
    let (filas, n_elementos, n_genericos) = (medidas & 0xFFFF, (medidas >> 16 & 0xFF) as usize, (medidas >> 24) as usize);
    let (n_cargas_vs, n_cargas_ps) = ((cargas & 0xFFFF) as usize, (cargas >> 16) as usize);
    if registros_vs == 0 || registros_ps == 0 || registros_vs >= tu::REGISTROS || registros_ps >= tu::REGISTROS || salidas == 0 || salidas > 8 || posicion >= salidas {
        return None;
    }
    if filas > MAX_FILAS || paso == 0 || paso % 4 != 0 || n_elementos > MAX_ELEMENTOS || n_genericos > MAX_GENERICOS || n_cargas_vs > MAX_CARGAS || n_cargas_ps > MAX_CARGAS {
        return None;
    }
    // Las piezas de detras de la cabecera.
    let mut i = CABECERA_2;
    let vs = b.get(i..i + bvs)?;
    i += bvs;
    let ps = b.get(i..i + bps)?;
    i += bps;
    let mut elementos = [NINGUNO; MAX_ELEMENTOS];
    for e in elementos.iter_mut().take(n_elementos) {
        let x = b.get(i..i + 8)?;
        *e = Elemento { desde: u32le(x, 0), componentes: u32le(x, 4) as u8 };
        if u32le(x, 4) == 0 || u32le(x, 4) > 4 || e.desde % 4 != 0 || e.desde as u64 + 4 * u32le(x, 4) as u64 > paso as u64 {
            return None;
        }
        i += 8;
    }
    let mut leer_cargas = |cuantas: usize| -> Option<[Carga; MAX_CARGAS]> {
        let mut c = [NINGUNA; MAX_CARGAS];
        for x in c.iter_mut().take(cuantas) {
            *x = carga_de(b.get(i..i + 4)?)?;
            i += 4;
        }
        Some(c)
    };
    let cargas_vs = leer_cargas(n_cargas_vs)?;
    let cargas_ps = leer_cargas(n_cargas_ps)?;
    let mut genericos = [None; MAX_GENERICOS];
    for g in genericos.iter_mut().take(n_genericos) {
        let x = *b.get(i)?;
        *g = if x == SIN_GENERICO { None } else if x < 8 { Some(x) } else { return None };
        i += 1;
    }
    let mut texturas = [DeApp::NINGUNA; MAX_TEXTURAS];
    for t in texturas.iter_mut().take(n_texturas) {
        *t = DeApp::leer(b.get(i..i + BYTES_DE_APP)?)?;
        i += BYTES_DE_APP;
    }
    // El relleno, a cero, hasta 16; y los DATOS, justo hasta el final.
    let desde_datos = i.next_multiple_of(16);
    if b.get(i..desde_datos)?.iter().any(|&x| x != 0) || b.len() != desde_datos + bdatos {
        return None;
    }
    let datos = &b[desde_datos..];
    // Como se dibuja: los campos de VRN1, con el DESTINO obligatorio.
    let mut dibujo = tu::dibujo_de_campos(b, n, datos.len())?;
    if dibujo.destino.is_none() {
        return None;
    }
    dibujo.color = (u32le(b, 20) == 1).then(|| u32le(b, 24));
    dibujo.texturas = n_texturas as u8;
    if let Some(z) = dibujo.z.as_mut() {
        if z.limpiar.is_some() {
            z.limpiar = Some(u32le(b, 80));
            if !z.valida() {
                return None;
            }
        }
    } else if u32le(b, 80) != 0 {
        return None;
    }
    // *** TODO VERTICE QUE SE PUEDA PEDIR, DENTRO DE LOS DATOS. El pegamento
    // lee `16 * filas + vid * paso + desde + 4 * c`, y `vid` es un indice
    // (comprobado abajo) o `0..n` (menos que `vertices`, lo dice VRN1).
    if 16 * filas as u64 + dibujo.vertices as u64 * paso as u64 > datos.len() as u64 {
        return None;
    }
    if let Some(desde) = dibujo.indices {
        if (0..n).any(|k| u32le(datos, desde as usize + 4 * k) >= dibujo.vertices) {
            return None;
        }
    }
    // Las cargas: dentro de lo que hay (el pegamento lo vuelve a mirar, y
    // que sus registros sean del cuerpo).
    let r = Receta { n, vs, ps, registros_vs, registros_ps, salidas, posicion, filas, paso, elementos, n_elementos, cargas_vs, n_cargas_vs, cargas_ps, n_cargas_ps, genericos, n_genericos, datos, dibujo, texturas };
    let fila_ok = |fila: u16| (fila as u32) < filas;
    for c in r.cargas_vs() {
        match *c {
            Carga::Entrada { elemento, componente, .. } if (elemento as usize) < n_elementos && componente < 4 => {}
            Carga::Fila { fila, .. } if fila_ok(fila) => {}
            _ => return None,
        }
    }
    for c in r.cargas_ps() {
        match *c {
            Carga::Entrada { elemento, componente, .. } if componente < 4 && matches!(r.genericos().get(elemento as usize), Some(Some(_))) => {}
            Carga::Fila { fila, .. } if fila_ok(fila) => {}
            // El asa de una textura que la receta TRAE.
            Carga::Asa { textura, .. } if (textura as usize) < n_texturas => {}
            _ => return None,
        }
    }
    Some(r)
}

/// **Escribir una receta** en `out`; devuelve cuanto mide, o `None` si no
/// cabe o no se sostiene (se relee con [`leer`] antes de darla por buena).
pub fn escribir(out: &mut [u8], r: &Receta) -> Option<usize> {
    let total = r.medida();
    if out.len() < total || r.n_elementos > MAX_ELEMENTOS || r.n_cargas_vs > MAX_CARGAS || r.n_cargas_ps > MAX_CARGAS || r.n_genericos > MAX_GENERICOS || r.dibujo.texturas as usize > MAX_TEXTURAS {
        return None;
    }
    // La cabecera de VRN1 de la misma forma, y se le cambia la magia.
    let d = r.dibujo;
    let (dva, dst) = d.destino?;
    let estado = match d.descarte {
        tu::Descarte::Ninguna => 0,
        tu::Descarte::Traseras => 1,
        tu::Descarte::Delanteras => 2,
    } | (d.antihorario as u32) << 2
        | match d.z {
            None => 0,
            Some(z) => 1 << 3 | (z.funcion & 0xF) << 4 | (z.escribir as u32) << 8 | (z.limpiar.is_some() as u32) << 9,
        };
    let medidas = r.filas | (r.n_elementos as u32) << 16 | (r.n_genericos as u32) << 24;
    let cabecera = [
        MAGIA_2,
        0,
        r.n as u32,
        r.vs.len() as u32,
        r.ps.len() as u32,
        d.color.is_some() as u32,
        d.color.unwrap_or(0),
        r.datos.len() as u32,
        d.indices.unwrap_or(SIN_INDICES),
        estado,
        d.vertices,
        dva as u32,
        (dva >> 32) as u32,
        dst.fila,
        dst.ancho | dst.alto << 16,
        1 << 31 | dst.rgb as u32,
        r.registros_vs | r.registros_ps << 8 | r.salidas << 16 | r.posicion << 24,
        medidas,
        r.paso,
        r.n_cargas_vs as u32 | (r.n_cargas_ps as u32) << 16,
        d.z.and_then(|z| z.limpiar).unwrap_or(0),
        d.texturas as u32,
    ];
    out[..total].fill(0);
    for (k, w) in cabecera.iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
    let mut i = CABECERA_2;
    let mut poner = |b: &[u8]| {
        out[i..i + b.len()].copy_from_slice(b);
        i += b.len();
    };
    poner(r.vs);
    poner(r.ps);
    for e in r.elementos() {
        poner(&e.desde.to_le_bytes());
        poner(&(e.componentes as u32).to_le_bytes());
    }
    for c in r.cargas_vs().iter().chain(r.cargas_ps()) {
        poner(&carga_a(*c));
    }
    for g in r.genericos() {
        poner(&[g.unwrap_or(SIN_GENERICO)]);
    }
    for t in r.texturas() {
        let mut b = [0u8; BYTES_DE_APP];
        t.escribir(&mut b);
        poner(&b);
    }
    let desde_datos = i.next_multiple_of(16);
    out[desde_datos..total].copy_from_slice(r.datos);
    leer(&out[..total]).map(|_| total)
}

/// **El taller**: donde el kernel pega los dos programas. ~8 KiB: el kernel
/// lo tiene ESTATICO (bajo el cerrojo del GR), no en su pila.
pub struct Taller {
    cuerpo: [(u64, u64); MAX_INSTRUCCIONES],
    /// Donde pega el pegamento, un programa y luego el otro.
    pegado: pegamento::Pegado,
    pub vs: [u8; HUECO],
    pub ps: [u8; HUECO],
    pub bytes_vs: usize,
    pub bytes_ps: usize,
    /// Instrucciones que el juez miro, las dos (con el pegamento).
    pub instrucciones: usize,
}

impl Taller {
    pub const fn nuevo() -> Self {
        Taller { cuerpo: [(0, 0); MAX_INSTRUCCIONES], pegado: pegamento::Pegado::VACIO, vs: [0; HUECO], ps: [0; HUECO], bytes_vs: 0, bytes_ps: 0, instrucciones: 0 }
    }
}

/// Los bytes de un cuerpo a sus instrucciones, en el taller.
fn instrucciones<'t>(cuerpo: &'t mut [(u64, u64); MAX_INSTRUCCIONES], b: &[u8]) -> &'t [(u64, u64)] {
    let n = b.len() / 16;
    for (k, x) in cuerpo.iter_mut().take(n).enumerate() {
        let w = &b[16 * k..16 * k + 16];
        *x = (u64::from_le_bytes(w[..8].try_into().unwrap()), u64::from_le_bytes(w[8..].try_into().unwrap()));
    }
    &cuerpo[..n]
}

/// **Pegar**: cada cuerpo pasa el juez R7 (lista blanca), el pegamento del
/// KERNEL lo rodea con las lecturas de los DATOS y las salidas, y el
/// programa entero pasa R0..R6. Los dos programas quedan en el taller.
pub fn pegar(r: &Receta, t: &mut Taller) -> Result<(), NoReceta> {
    let datos = Datos { filas: r.filas, paso: r.paso, elementos: r.elementos() };
    let cuerpo = instrucciones(&mut t.cuerpo, r.vs);
    juez::juzgar_cuerpo_de_app(cuerpo, r.registros_vs).map_err(|b| NoReceta::Cuerpo("vertice", b))?;
    pegamento::vertice_en(&mut t.pegado, cuerpo, r.registros_vs, r.cargas_vs(), datos, r.salidas, r.posicion).map_err(|e| NoReceta::Pegamento("vertice", e))?;
    t.bytes_vs = t.pegado.bytes(&mut t.vs);
    let cuerpo = instrucciones(&mut t.cuerpo, r.ps);
    juez::juzgar_cuerpo_con_asas(cuerpo, r.registros_ps, pegamento::asas(r.cargas_ps())).map_err(|b| NoReceta::Cuerpo("pixel", b))?;
    pegamento::pixel_en(&mut t.pegado, cuerpo, r.registros_ps, r.cargas_ps(), datos, r.genericos()).map_err(|e| NoReceta::Pegamento("pixel", e))?;
    t.bytes_ps = t.pegado.bytes(&mut t.ps);
    let jv = juez::juzgar_programa(&t.vs[..t.bytes_vs], tu::REGISTROS).map_err(|b| NoReceta::Juez("vertice", b))?;
    let jp = juez::juzgar_programa(&t.ps[..t.bytes_ps], tu::REGISTROS).map_err(|b| NoReceta::Juez("pixel", b))?;
    t.instrucciones = jv.instrucciones + jp.instrucciones;
    Ok(())
}

/// **El paquete** de una receta pegada: los programas del taller, los DATOS
/// de la receta y su dibujo; la ficha, la del GR que ponga el kernel.
pub fn paquete<'a>(r: &Receta<'a>, t: &'a Taller, ficha: u32) -> Paquete<'a> {
    Paquete { ficha, vs: &t.vs[..t.bytes_vs], ps: &t.ps[..t.bytes_ps], vertices: r.datos, n: r.n, limpiar: None, dibujo: r.dibujo }
}

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;
    use crate::destino::Destino;
    use crate::texturas::{tex, Muestreo};
    use bmo_sm86::codifica as c;

    const ALU_1: u64 = 1 | 1 << 4 | 7 << 5 | 7 << 8;

    /// P3b4c.8 T2: una receta con UNA textura, de punta a punta: se escribe,
    /// se relee igual, se pega (el MOV del asa lo pone el pegamento, el TEX
    /// es del cuerpo) y el juez dice PERFECTO Y PRECISO.
    #[test]
    fn una_receta_con_una_textura() {
        let bytes = |w: &[(u64, u64)]| w.iter().flat_map(|&(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect::<std::vec::Vec<u8>>();
        // Vertice: la posicion y (u, v, 0, 0), tal cual.
        let vs = bytes(&[c::mov(0, c::r(0), 1), c::exit(0)]);
        // Pixel: TEX R0..R3 <- (R4, R5) con el asa en R6; EXIT espera su barrera.
        let ps = bytes(&[tex(0, 4, 6, crate::tuberia::carga(0)), c::exit(ALU_1 | 1 << 11)]);
        let datos = std::vec![0u8; 96];
        let dst = Destino { fila: 5120, ancho: 1280, alto: 720, rgb: false };
        let t = DeApp { va: 0x4000_0000, ancho: 256, alto: 256, fila: 1024, bgra: false, muestreo: Muestreo { lineal: true, u: 1, v: 3, borde: [0.0, 0.0, 0.0, 1.0] } };
        let mut r = Receta {
            n: 3,
            vs: &vs,
            ps: &ps,
            registros_vs: 8,
            registros_ps: 8,
            salidas: 2,
            posicion: 0,
            filas: 0,
            paso: 32,
            elementos: [NINGUNO; MAX_ELEMENTOS],
            n_elementos: 2,
            cargas_vs: [NINGUNA; MAX_CARGAS],
            n_cargas_vs: 8,
            cargas_ps: [NINGUNA; MAX_CARGAS],
            n_cargas_ps: 3,
            genericos: [None; MAX_GENERICOS],
            n_genericos: 1,
            datos: &datos,
            dibujo: Dibujo { indices: None, vertices: 3, destino: Some((0x1000_0000, dst)), texturas: 1, ..Dibujo::default() },
            texturas: [DeApp::NINGUNA; MAX_TEXTURAS],
        };
        r.elementos[0] = Elemento { desde: 0, componentes: 4 };
        r.elementos[1] = Elemento { desde: 16, componentes: 4 };
        for k in 0..8u8 {
            r.cargas_vs[k as usize] = Carga::Entrada { elemento: k / 4, componente: k % 4, reg: k };
        }
        r.cargas_ps[0] = Carga::Entrada { elemento: 0, componente: 0, reg: 4 };
        r.cargas_ps[1] = Carga::Entrada { elemento: 0, componente: 1, reg: 5 };
        r.cargas_ps[2] = Carga::Asa { textura: 0, reg: 6 };
        r.genericos[0] = Some(0);
        r.texturas[0] = t;
        let mut caja = std::vec![0u8; MAX_RECETA];
        let n = escribir(&mut caja, &r).expect("la receta con textura se sostiene");
        assert_eq!(medida(&caja[..CABECERA_2]), Some(n));
        let l = leer(&caja[..n]).unwrap();
        assert_eq!((l.texturas(), l.dibujo.texturas, l.cargas_ps()[2]), (&[t][..], 1, Carga::Asa { textura: 0, reg: 6 }));
        let mut taller = std::boxed::Box::new(Taller::nuevo());
        pegar(&l, &mut taller).expect("se pega y el juez la aprueba");
        // En el de pixel: el MOV del asa (0 = el TIC 0 y el TSC 0) y el TEX.
        let codigo: std::vec::Vec<(u64, u64)> = taller.ps[4 * crate::raster::SPH..taller.bytes_ps].chunks(16).map(|w| (u64::from_le_bytes(w[..8].try_into().unwrap()), u64::from_le_bytes(w[8..].try_into().unwrap()))).collect();
        assert!(codigo.contains(&crate::cubo::mov(6, crate::texturas::asa(0, 0))), "el asa la pone el pegamento");
        assert!(codigo.iter().any(|w| w.0 & 0xFFF == 0x361), "y el TEX es el del cuerpo");
        // Y las ordenes del dibujo apuntan las piscinas.
        let o = crate::tuberia::ordenes_dibujo(&dst.ventana(), 1, true, l.dibujo);
        assert!(o.o[..o.n].contains(&crate::copia::cabecera_en(0, crate::texturas::SET_TEX_HEADER_POOL_A, 3)));

        // Lo que no pasa: un asa de una textura que no viene, una textura
        // que no cabe en su ranura, y un TEX con un asa que no puso el kernel.
        let mut mala = r;
        mala.cargas_ps[2] = Carga::Asa { textura: 1, reg: 6 };
        assert!(escribir(&mut caja, &mala).is_none());
        let mut mala = r;
        mala.texturas[0].alto = 2048;
        assert!(escribir(&mut caja, &mala).is_none(), "2 MiB no caben en la ranura de 1 MiB");
        let ps_malo = bytes(&[tex(0, 4, 7, crate::tuberia::carga(0)), c::exit(ALU_1 | 1 << 11)]);
        let mala = Receta { ps: &ps_malo, ..r };
        let n = escribir(&mut caja, &mala).unwrap();
        assert!(matches!(pegar(&leer(&caja[..n]).unwrap(), &mut taller), Err(NoReceta::Cuerpo("pixel", b)) if b.regla == juez::Regla::R7CuerpoAjeno));
        // Sin texturas, la de siempre: +84 a cero.
        let sin = Receta { dibujo: Dibujo { texturas: 0, ..r.dibujo }, n_cargas_ps: 2, ..r };
        let n = escribir(&mut caja, &sin).unwrap();
        assert_eq!(u32le(&caja, 84), 0);
        assert!(leer(&caja[..n]).unwrap().texturas().is_empty());
    }
}
