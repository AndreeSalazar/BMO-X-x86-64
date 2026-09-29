//! **P3b4c: EL LOTE A LA PUERTA DE LA 3060** -- lo que la app `proton-x`
//! manda al kernel por `IOMMU_OP_GPU_DIBUJAR`: la RECETA (VRN2) de un lote.
//!
//! capa: puro -- bytes; la llamada al kernel la hace la app
//!
//! ```text
//!    una vez por PSO   los CUERPOS emitidos (el emisor, ABI de registros),
//!                      sus cargas, el input layout y como van los genericos
//!    una vez por PSO   un PEGADO DE PRUEBA aqui, con el mismo pegamento y
//!    y paso            el mismo juez que el kernel: si no va a pasar, se
//!                      dice AQUI, con su porque, y no se llama a nadie
//!    cada lote         los DATOS (el cbuffer, los vertices TAL CUAL y los
//!                      indices), el dibujo (descarte, giro, Z) y el DESTINO:
//!                      el back buffer de la app
//! ```
//!
//! La CPU dirige: traduce una vez, copia bytes y llama. No dibuja.
//!
//! # La Z, coherente
//!
//! La profundidad de la 3060 vive en SU VRAM; la de la casa, en la RAM del
//! recurso D32. Mientras todos los dibujos de un fotograma van por la 3060,
//! la suya es LA profundidad. Si uno cae a la CPU, la de la 3060 deja de
//! tener lo ultimo: el siguiente con Z que no la limpie tambien va a la CPU
//! (y se dice). Lo lleva [`Puerta::z_viva`].

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bmo_gpu_ga10x::destino::Destino;
use bmo_gpu_ga10x::pegamento::{Carga, Elemento};
use bmo_gpu_ga10x::profundidad::Z;
use bmo_gpu_ga10x::texturas::{DeApp, Muestreo, MAX_TEXTURAS};
use bmo_proton_x::textura::{Direccion, Filtro};
use bmo_gpu_ga10x::receta::{self, Receta, Taller, MAX_CARGAS, MAX_ELEMENTOS, MAX_GENERICOS, NINGUNA, NINGUNO};
use bmo_gpu_ga10x::tuberia::{Descarte, Dibujo, DATOS_MAX};
use bmo_proton_x::lote::{ElementoIa, Enlace, Lote, Topologia};

use crate::pso::{cargas, elementos, NoVa};

/// Lo que contesta el kernel (`cubo::empaquetar`): `sano` = la 3060 pago el
/// dibujo entero; `desempaquetar` = `(us, triangulos, etapas, ..)`.
pub use bmo_gpu_ga10x::cubo::{a_pantalla, copia_us, desempaquetar, preparado, sano};
use crate::{emitir_con, Abi};

/// Los registros que se le dan al emisor (los de VERRANO, `tuberia::REGISTROS`).
const REGISTROS: u32 = 64;

/// **La Z por la 3060, en la SOMBRA del kernel** (P3b4c.6b). El metal
/// (28-09 20:31): `gpu verrano bmox12 30 z` dio Xid 69 (error de clase del
/// motor grafico) AL DIBUJAR -- el estado entero paso, los vertices tambien,
/// y el rasterizador rechazo la pareja ZT en bloque + color PITCH, y el canal
/// GR quedo muerto hasta reiniciar. Es del hardware: nouveau apaga la Z si
/// el color es lineal (`nvc0_validate_fb`) y NVK dibuja en una SOMBRA en
/// bloque y la copia (`nvk_rendering_linear`: "Depth and stencil are never
/// linear"). Desde el 29-09 el kernel hace lo mismo: con Z, el color va a su
/// sombra en VRAM (`bmo_gpu_ga10x::sombra`) y el motor de copia la lleva al
/// back buffer de la app. La receta con Z ya puede ir; apagar esto devuelve
/// cada lote con Z a la CPU (un kernel SIN sombra mataria el canal).
pub const Z_EN_LA_SOMBRA: bool = true;

/// **Lo de un PSO para la puerta**: los cuerpos y como se cargan.
#[derive(Debug, Clone)]
pub struct Cuerpos {
    pub vs: Vec<u8>,
    pub ps: Vec<u8>,
    pub registros_vs: u32,
    pub registros_ps: u32,
    pub salidas: u32,
    pub posicion: u32,
    pub filas: u32,
    pub elementos: Vec<Elemento>,
    pub cargas_vs: Vec<Carga>,
    pub cargas_ps: Vec<Carga>,
    pub genericos: Vec<Option<u8>>,
    /// P3b4c.8 T2b: las texturas del de pixel, en el orden de la receta: la
    /// k es la pareja (tN, sM) de su k-esima asa.
    pub texturas: Vec<(u8, u8)>,
}

fn bytes(codigo: &[(u64, u64)]) -> Vec<u8> {
    codigo.iter().flat_map(|&(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect()
}

/// **Los cuerpos de un PSO**: el emisor, con el ABI de registros.
pub fn cuerpos(en: &Enlace, ia: &[ElementoIa]) -> Result<Cuerpos, NoVa> {
    let ev = emitir_con(&en.vs, REGISTROS, Abi::Registros).map_err(|e| NoVa::Emisor("vertice", e))?;
    if ev.precargas.iter().any(|q| matches!(q, crate::Precarga::Asa { .. })) {
        return Err(NoVa::Emisor("vertice", crate::NoEmite::Operacion(0)));
    }
    let ep = emitir_con(&en.ps, REGISTROS, Abi::Registros).map_err(|e| NoVa::Emisor("pixel", e))?;
    let posicion = en.posicion as u32;
    let genericos = en.desde_vs.iter().map(|o| o.and_then(|o| bmo_gpu_ga10x::pegamento::generico(o as u32, posicion))).collect();
    Ok(Cuerpos {
        vs: bytes(&ev.codigo),
        ps: bytes(&ep.codigo),
        registros_vs: ev.registros,
        registros_ps: ep.registros,
        salidas: en.vs.salidas as u32,
        posicion,
        filas: en.vs.filas_cb.max(en.ps.filas_cb) as u32,
        elementos: elementos(en, ia)?,
        cargas_vs: cargas(&ev),
        cargas_ps: cargas(&ep),
        genericos,
        texturas: crate::pso::texturas_de(&ep),
    })
}

/// El modo de la casa con su numero de D3D12.
const fn d3d(d: Direccion) -> u32 {
    match d {
        Direccion::Repetir => 1,
        Direccion::Espejo => 2,
        Direccion::Sujetar => 3,
        Direccion::Borde => 4,
        Direccion::EspejoUnaVez => 5,
    }
}

/// Lo que se sabe del destino (el render target de la casa).
#[derive(Clone, Copy, Debug)]
pub struct Blanco {
    /// La VA de sus pixeles en el proceso (tiene que empezar en pagina).
    pub va: u64,
    pub ancho: u32,
    pub alto: u32,
    /// `B8G8R8A8` (si no, `R8G8B8A8`).
    pub bgra: bool,
    /// P3b4c.9 Z1: es un back buffer de la cadena de intercambio (lo que se
    /// muestra en `Present`): el kernel puede ponerlo directo en la pantalla
    /// si el escritorio se la dio a esta app.
    pub cadena: bool,
}

/// **Escribir la receta de un lote** en `caja` (con `datos` de apoyo).
/// `Err` = por que este lote NO va a la 3060 (va a la CPU, y se dice).
pub fn escribir(c: &Cuerpos, l: &Lote, b: Blanco, limpiar_z: Option<u32>, datos: &mut Vec<u8>, caja: &mut [u8]) -> Result<usize, String> {
    if l.topologia != Topologia::Lista {
        return Err(String::from("una TIRA de triangulos: la 3060 dibuja listas todavia"));
    }
    let n = l.ids.len();
    if n == 0 || n % 3 != 0 {
        return Err(format!("{n} indices: no son triangulos enteros"));
    }
    let dst = Destino { fila: 4 * b.ancho, ancho: b.ancho, alto: b.alto, rgb: !b.bgra };
    if !dst.valido() || b.va % 4096 != 0 {
        return Err(format!("el back buffer de {}x{} (VA {:#x}): la puerta dibuja 1280x720 desde una pagina, todavia", b.ancho, b.alto, b.va));
    }
    let r = &l.reglas;
    if r.viewport != [0.0, 0.0, b.ancho as f32, b.alto as f32, 0.0, 1.0] {
        return Err(format!("un viewport que no es el back buffer entero ({:?})", r.viewport));
    }
    if r.tijera[0] > 0 || r.tijera[1] > 0 || r.tijera[2] < b.ancho as i32 || r.tijera[3] < b.alto as i32 {
        return Err(format!("una tijera que corta ({:?}): la 3060 no la pone todavia", r.tijera));
    }
    let descarte = match r.descarte {
        1 => Descarte::Ninguna,
        2 => Descarte::Delanteras,
        3 => Descarte::Traseras,
        d => return Err(format!("un modo de descarte que no es de D3D12 ({d})")),
    };
    let z = r.profundidad.map(|p| Z { funcion: p.funcion, escribir: p.escribir, limpiar: limpiar_z });
    if z.is_some_and(|z| !z.valida()) {
        return Err(String::from("una regla de profundidad fuera de D3D12"));
    }
    // Los DATOS: el cbuffer (a ceros lo que no traiga), los vertices que se
    // piden TAL CUAL (hasta el mayor indice) y detras los indices, en u32.
    let paso = l.paso;
    if paso == 0 || paso % 4 != 0 {
        return Err(format!("un paso de vertice de {paso} bytes"));
    }
    let hay = l.vertices.len() / paso;
    let mayor = *l.ids.iter().max().unwrap_or(&0) as usize;
    if mayor >= hay {
        return Err(format!("el indice {mayor} pasa de los {hay} vertices del bufer"));
    }
    let cb = 16 * c.filas as usize;
    let vertices = mayor + 1;
    let desde = (cb + vertices * paso).next_multiple_of(4);
    let total = (desde + 4 * n).next_multiple_of(16);
    if total > DATOS_MAX {
        return Err(format!("{total} bytes de DATOS: no caben en los {DATOS_MAX} de la 3060"));
    }
    datos.clear();
    datos.resize(total, 0);
    let k = l.cb.len().min(cb);
    datos[..k].copy_from_slice(&l.cb[..k]);
    datos[cb..cb + vertices * paso].copy_from_slice(&l.vertices[..vertices * paso]);
    for (i, &id) in l.ids.iter().enumerate() {
        datos[desde + 4 * i..desde + 4 * i + 4].copy_from_slice(&id.to_le_bytes());
    }
    if c.elementos.len() > MAX_ELEMENTOS || c.cargas_vs.len() > MAX_CARGAS || c.cargas_ps.len() > MAX_CARGAS || c.genericos.len() > MAX_GENERICOS {
        return Err(String::from("el PSO trae mas entradas o cargas de las que caben en una receta"));
    }
    // P3b4c.8 T2b: cada textura del de pixel -- la imagen tN con el
    // muestreador sM de este lote --, tal como esta en la RAM de la app: el
    // kernel la presta SOLO LECTURA mientras dibuja.
    if c.texturas.len() > MAX_TEXTURAS {
        return Err(format!("{} texturas: la receta lleva {MAX_TEXTURAS}", c.texturas.len()));
    }
    let mut texturas = [DeApp::NINGUNA; MAX_TEXTURAS];
    for (k, &(tn, sm)) in c.texturas.iter().enumerate() {
        let (Some(Some(t)), Some(Some(m))) = (l.recursos.texturas.get(tn as usize), l.recursos.muestreadores.get(sm as usize)) else {
            return Err(format!("el sombreador muestrea t{tn} con s{sm} y el lote no los tiene atados"));
        };
        let muestreo = Muestreo { lineal: m.filtro == Filtro::Lineal, u: d3d(m.u), v: d3d(m.v), borde: m.borde };
        let d = DeApp { va: t.texeles.as_ptr() as u64, ancho: t.ancho, alto: t.alto, fila: 4 * t.ancho, bgra: t.bgra, muestreo };
        if t.texeles.len() < (t.ancho * t.alto) as usize || !d.valida() {
            return Err(format!(
                "la textura t{tn} ({}x{}, VA {:#x}): la 3060 la lee con filas de 32 B, desde 32 B y en 1 MiB; esta no",
                t.ancho, t.alto, d.va
            ));
        }
        texturas[k] = d;
    }
    let mut rec = Receta {
        n,
        vs: &c.vs,
        ps: &c.ps,
        registros_vs: c.registros_vs,
        registros_ps: c.registros_ps,
        salidas: c.salidas,
        posicion: c.posicion,
        filas: c.filas,
        paso: paso as u32,
        elementos: [NINGUNO; MAX_ELEMENTOS],
        n_elementos: c.elementos.len(),
        cargas_vs: [NINGUNA; MAX_CARGAS],
        n_cargas_vs: c.cargas_vs.len(),
        cargas_ps: [NINGUNA; MAX_CARGAS],
        n_cargas_ps: c.cargas_ps.len(),
        genericos: [None; MAX_GENERICOS],
        n_genericos: c.genericos.len(),
        datos,
        dibujo: Dibujo { indices: Some(desde as u32), vertices: vertices as u32, descarte, antihorario: r.antihorario, destino: Some((b.va, dst)), z, color: l.limpiar_rt, texturas: c.texturas.len() as u8, cadena: b.cadena, pantalla: false },
        texturas,
    };
    rec.elementos[..c.elementos.len()].copy_from_slice(&c.elementos);
    rec.cargas_vs[..c.cargas_vs.len()].copy_from_slice(&c.cargas_vs);
    rec.cargas_ps[..c.cargas_ps.len()].copy_from_slice(&c.cargas_ps);
    rec.genericos[..c.genericos.len()].copy_from_slice(&c.genericos);
    receta::escribir(caja, &rec).ok_or_else(|| String::from("la receta no se sostiene (lo que lee el pegamento no cae en los DATOS)"))
}

/// **La puerta de la app**: los cuerpos por PSO, lo probado por PSO y paso,
/// la caja de la receta y la Z.
pub struct Puerta {
    cuerpos: Vec<(usize, Result<Cuerpos, NoVa>)>,
    probados: Vec<(usize, usize, Result<(), String>)>,
    /// La caja de la receta: en el MONTON de la app (un bloque suyo), que es
    /// lo que el kernel exige.
    pub caja: Vec<u8>,
    datos: Vec<u8>,
    taller: Box<Taller>,
    /// La Z de la 3060 tiene lo ultimo (ver la cabecera).
    pub z_viva: bool,
    /// Mandar lotes con Z a la 3060 ([`Z_EN_LA_SOMBRA`]): el kernel los
    /// dibuja en su sombra en bloque y los copia al back buffer.
    pub z_a_la_3060: bool,
}

impl Default for Puerta {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Puerta {
    pub fn nueva() -> Self {
        Puerta { cuerpos: Vec::new(), probados: Vec::new(), caja: alloc::vec![0; receta::MAX_RECETA], datos: Vec::new(), taller: Box::new(Taller::nuevo()), z_viva: false, z_a_la_3060: Z_EN_LA_SOMBRA }
    }

    /// **La receta de este lote**, en `self.caja[..n]`: `Ok(n)`, o por que
    /// no va a la 3060.
    pub fn preparar(&mut self, l: &Lote, b: Blanco) -> Result<usize, String> {
        if l.reglas.profundidad.is_some() && !self.z_a_la_3060 {
            return Err(String::from(
                "el lote usa Z y la 3060 no dibuja con Z sobre un color PITCH (el back buffer en tu RAM): sin la sombra en bloque del kernel fue Xid 69 y el canal GR MUERTO hasta reiniciar",
            ));
        }
        if l.reglas.profundidad.is_some() && l.limpiar_z.is_none() && !self.z_viva {
            return Err(String::from("la Z de la 3060 no tiene lo ultimo (un dibujo anterior fue por la CPU) y este no la limpia"));
        }
        let clave = l.enlace as *const Enlace as usize;
        let i = match self.cuerpos.iter().position(|x| x.0 == clave) {
            Some(i) => i,
            None => {
                self.cuerpos.push((clave, cuerpos(l.enlace, l.entradas)));
                self.cuerpos.len() - 1
            }
        };
        let c = match &self.cuerpos[i].1 {
            Ok(c) => c,
            Err(e) => return Err(format!("el PSO no va a la 3060: {e:?}")),
        };
        let n = escribir(c, l, b, l.limpiar_z, &mut self.datos, &mut self.caja)?;
        // UNA vez por PSO y paso: el pegado de prueba, como lo hara el kernel.
        if !self.probados.iter().any(|p| p.0 == clave && p.1 == l.paso) {
            let prueba = match receta::leer(&self.caja[..n]) {
                None => Err(String::from("la receta no se relee")),
                Some(r) => receta::pegar(&r, &mut self.taller).map_err(|e| format!("el kernel no la pegaria: {e:?}")),
            };
            self.probados.push((clave, l.paso, prueba));
        }
        match &self.probados.iter().find(|p| p.0 == clave && p.1 == l.paso).expect("recien puesto").2 {
            Ok(()) => Ok(n),
            Err(e) => Err(e.clone()),
        }
    }

    /// Lo que paso con el lote: `a_la_3060` = lo dibujo la 3060. La Z de la
    /// 3060 queda viva si la uso; muerta si el lote se dibujo en la CPU.
    pub fn despues(&mut self, l: &Lote, a_la_3060: bool) {
        if l.reglas.profundidad.is_some() {
            self.z_viva = a_la_3060;
        }
    }
}
