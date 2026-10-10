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
use bmo_proton_x::textura::{Clase, Como, Direccion, Filtro, Textura};
use bmo_gpu_ga10x::receta::{self, Receta, Taller, MAX_CARGAS, MAX_ELEMENTOS, MAX_GENERICOS, NINGUNA, NINGUNO};
use bmo_gpu_ga10x::tuberia::{Descarte, Dibujo, DATOS_MAX};
use bmo_proton_x::lote::{ElementoIa, Enlace, Lote, Topologia};

use crate::pso::{cargas, elementos, NoVa};

/// Lo que contesta el kernel (`cubo::empaquetar`): `sano` = la 3060 pago el
/// dibujo entero; `desempaquetar` = `(us, triangulos, etapas, ..)`.
pub use bmo_gpu_ga10x::cubo::{a_pantalla, copia_us, desempaquetar, preparado, raro, sano};
use crate::vivo::{Origen, Recuerdo, Vivos};
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
    /// 9d: el registro del TERMOMETRO de cada cuerpo (con libreta).
    pub termometro_vs: Option<u8>,
    pub termometro_ps: Option<u8>,
    /// ** E8f: algun cuerpo lee su cbuffer con un LDC (`ConstantesEn`): la
    /// receta le pide al kernel el banco de la app (+88 bit 1).
    pub banco: bool,
}

fn bytes(codigo: &[(u64, u64)]) -> Vec<u8> {
    codigo.iter().flat_map(|&(lo, hi)| lo.to_le_bytes().into_iter().chain(hi.to_le_bytes())).collect()
}

/// **Los cuerpos de un PSO**: el emisor, con el ABI de registros (y A9,
/// comprobados bit a bit contra la CPU: [`crate::vivo`]).
pub fn cuerpos(en: &Enlace, ia: &[ElementoIa]) -> Result<Cuerpos, NoVa> {
    cuerpos_con(en, ia, None).map(|(c, _)| c)
}

/// 9d (06-10): los cuerpos se emiten con la LIBRETA (su termometro, que el
/// pegamento del kernel apunta; `crate::libreta`).
pub const CON_LIBRETA: bool = true;

/// [`cuerpos`] con un [`Recuerdo`] (A9, 06-10): del .bsf de antes si lo hay,
/// o traducidos, comprobados y guardados en el. Dice de donde salieron.
pub fn cuerpos_con(en: &Enlace, ia: &[ElementoIa], recuerdo: Option<&mut dyn Recuerdo>) -> Result<(Cuerpos, Vivos), NoVa> {
    cuerpos_con_libreta(en, ia, recuerdo, CON_LIBRETA)
}

/// [`cuerpos_con`] diciendo si con la libreta. Sin ella, lo de antes de 9d
/// (lo pide la puerta si un PSO con libreta no cabe en el pegado: cinco
/// instrucciones mas por programa, y las del termometro).
pub fn cuerpos_con_libreta(en: &Enlace, ia: &[ElementoIa], recuerdo: Option<&mut dyn Recuerdo>, libreta: bool) -> Result<(Cuerpos, Vivos), NoVa> {
    // E2.3b: el sombreador de geometria corre en la CPU (la 3060, todavia no).
    if en.gs.is_some() {
        return Err(NoVa::Entrada("un sombreador de geometria"));
    }
    // N5.9: el de pixeles que lee SV_Position, por la CPU todavia (la 3060
    // la da en un atributo de sistema que el pegamento no pone).
    if en.pos_ps.is_some() {
        return Err(NoVa::Entrada("SV_Position en el de pixeles"));
    }
    // N5.8: la 3060 pinta UN render target todavia: el G-buffer, por la CPU.
    if en.objetivos != [0] {
        return Err(NoVa::Entrada("varios render targets (o uno que no es el 0, o SV_Depth)"));
    }
    // Con libreta, y si su termometro no cabe en los registros, sin ella.
    let emitir = |p: &bmo_proton_x::dxil::programa::Programa, que: &'static str| {
        let sin = || emitir_con(p, REGISTROS, Abi::Registros).map_err(|e| NoVa::Emisor(que, e));
        if libreta {
            crate::emitir_libreta(p, REGISTROS, Abi::Registros, true).or_else(|_| sin())
        } else {
            sin()
        }
    };
    let vivos = crate::vivo::cuerpos_vivos(en, recuerdo, emitir)?;
    let (ev, ep) = (&vivos.vs, &vivos.ps);
    if ev.precargas.iter().any(|q| matches!(q, crate::Precarga::Asa { .. })) {
        return Err(NoVa::Emisor("vertice", crate::NoEmite::Operacion(0)));
    }
    let posicion = en.posicion as u32;
    let genericos = en.desde_vs.iter().map(|o| o.and_then(|o| bmo_gpu_ga10x::pegamento::generico(o as u32, posicion))).collect();
    let c = Cuerpos {
        vs: bytes(&ev.codigo),
        ps: bytes(&ep.codigo),
        registros_vs: ev.registros,
        registros_ps: ep.registros,
        salidas: en.vs.salidas as u32,
        posicion,
        filas: en.vs.filas_cb.max(en.ps.filas_cb) as u32,
        elementos: elementos(en, ia)?,
        cargas_vs: cargas(ev),
        cargas_ps: cargas(ep),
        genericos,
        texturas: crate::pso::texturas_de(ep),
        termometro_vs: ev.termometro,
        termometro_ps: ep.termometro,
        banco: ev.usa_banco() || ep.usa_banco(),
    };
    Ok((c, vivos))
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
        // N5.16b: la receta de la 3060 recorta siempre en z.
        d if d & bmo_proton_x::trama::SIN_RECORTE_Z != 0 => return Err(String::from("DepthClipEnable = FALSE: la receta de la 3060 recorta en z siempre (no lo apaga todavia)")),
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
        // 02-10: la 3060 lee aqui RGBA/BGRA de 8 bits, lineales y con el
        // mapeo de siempre; un BC, un float, un sRGB o un mapeo, en la CPU.
        if !matches!(t.como, Como::Rgba8 | Como::Bgra8) || t.srgb || t.mapeo & 0xFFF != Textura::MAPEO & 0xFFF || t.clase != Clase::Plana || (t.mip, t.capa) != (0, 0) {
            return Err(format!("la textura t{tn} es BC, float, sRGB, con otro mapeo o una vista que no es la mip 0 de una 2D: la 3060 todavia lee solo eso"));
        }
        let muestreo = Muestreo { lineal: m.filtro == Filtro::Lineal, u: d3d(m.u), v: d3d(m.v), borde: m.borde };
        let d = DeApp { va: t.texeles.as_ptr() as u64, ancho: t.ancho, alto: t.alto, fila: 4 * t.ancho, bgra: t.bgra(), muestreo };
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
        dibujo: Dibujo { indices: Some(desde as u32), vertices: vertices as u32, descarte, antihorario: r.antihorario, destino: Some((b.va, dst)), z, color: l.limpiar_rt, texturas: c.texturas.len() as u8, cadena: b.cadena, pantalla: false, banco: if c.banco { 16 * c.filas } else { 0 } },
        texturas,
        termometro_vs: c.termometro_vs,
        termometro_ps: c.termometro_ps,
    };
    rec.elementos[..c.elementos.len()].copy_from_slice(&c.elementos);
    rec.cargas_vs[..c.cargas_vs.len()].copy_from_slice(&c.cargas_vs);
    rec.cargas_ps[..c.cargas_ps.len()].copy_from_slice(&c.cargas_ps);
    rec.genericos[..c.genericos.len()].copy_from_slice(&c.genericos);
    receta::escribir(caja, &rec).ok_or_else(|| String::from("la receta no se sostiene (lo que lee el pegamento no cae en los DATOS)"))
}

/// A9c: de cada cuantos lotes de un PSO revisa uno el vigia (el primero,
/// siempre). Revisar son unos pocos vertices en el simulador: en 256 lotes,
/// menos que nada.
pub const VIGIA_CADA: u64 = 256;

/// **La puerta de la app**: los cuerpos por PSO, lo probado por PSO y paso,
/// la caja de la receta y la Z.
pub struct Puerta {
    /// Por PSO: sus cuerpos y lo vivo (A9c: lo que revisa el vigia), y
    /// cuantos lotes lleva.
    cuerpos: Vec<(usize, Result<(Cuerpos, Vivos), NoVa>, u64)>,
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
    /// A9 (06-10): donde se recuerdan los .bsf de cada PSO (en BMO-X,
    /// ESTRATOS); sin el, se traduce cada arranque.
    pub recuerdo: Option<Box<dyn Recuerdo + Send>>,
    /// A9: los PSO traducidos (y comprobados) y los que salieron del
    /// recuerdo, en la vida de esta puerta.
    pub traducidos: usize,
    pub recordados: usize,
    /// A9c (06-10): EL VIGIA, el modo dinamico -- el PRIMER lote de cada PSO
    /// y luego uno de cada `vigia_cada` se revisan con los datos del juego
    /// ([`crate::vivo::revisar`]); uno que no cuadra se dibuja por la CPU
    /// desde ese lote, y se marca malo en el recuerdo.
    pub vigia_cada: u64,
    pub revisados: usize,
    pub corregidos: usize,
    /// 9d (06-10): los dibujos en los que la LIBRETA de la 3060 apunto algo
    /// raro ([`Puerta::apunto`]), en la vida de esta puerta; y los PSO que no
    /// cabian con ella en el pegado y van sin libreta.
    pub apuntados: usize,
    pub sin_libreta: usize,
}

impl Default for Puerta {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Puerta {
    pub fn nueva() -> Self {
        Puerta { cuerpos: Vec::new(), probados: Vec::new(), caja: alloc::vec![0; receta::MAX_RECETA], datos: Vec::new(), taller: Box::new(Taller::nuevo()), z_viva: false, z_a_la_3060: Z_EN_LA_SOMBRA, recuerdo: None, traducidos: 0, recordados: 0, vigia_cada: VIGIA_CADA, revisados: 0, corregidos: 0, apuntados: 0, sin_libreta: 0 }
    }

    /// **La receta de este lote**, en `self.caja[..n]`: `Ok(n)`, o por que
    /// no va a la 3060.
    pub fn preparar(&mut self, l: &Lote, b: Blanco) -> Result<usize, String> {
        // E2.7: con una consulta de oclusion abierta hay que CONTAR los
        // pixeles que pasan, y la 3060 aun no los cuenta: por la CPU.
        if l.oclusion {
            return Err(String::from("hay una consulta de oclusion abierta: la 3060 no cuenta los pixeles que pasan todavia"));
        }
        // 05-10: lo que un dibujo escribe en sus UAV queda en la memoria de la
        // CPU; la receta no lleva UAV (ni la 3060 emite sus operaciones).
        if l.uavs.is_some() {
            return Err(String::from("sus sombreadores leen o escriben UAV (RWTexture, RWBuffer, Interlocked): la 3060 no los lleva todavia"));
        }
        // E2.5 (05-10): las olas (Wave*, Quad*) tampoco: el emisor no las
        // sabe (`vote`, `shfl`); dicho con su nombre, no con un numero.
        if l.enlace.vs.olas_propias() || l.enlace.ps.olas_propias() {
            return Err(String::from("sus sombreadores usan las olas (Wave*, Quad*): la 3060 no las lleva todavia"));
        }
        // D4.4 (05-10): las derivadas (ddx, ddy, fwidth) tampoco; y la mip de
        // un `Sample` la saca su TEX, pero de la mip 0 de una textura de una
        // mip: si la vista tiene mas, o el muestreador filtra distinto de
        // lejos, la CPU (que elige la mip) daria otra cosa.
        if l.enlace.vs.deriva() || l.enlace.ps.deriva() {
            return Err(String::from("sus sombreadores usan las derivadas (ddx, ddy, fwidth): la 3060 no las lleva todavia"));
        }
        if l.enlace.ps.mip_por_derivadas() && l.recursos.mip_importa() {
            return Err(String::from("muestrea con la mip de sus derivadas una textura de varias mips (o con MIN y MAG distintos): la 3060 lee la mip 0 todavia"));
        }
        // N5.13: la receta lleva UNA instancia y los elementos por vertice.
        if l.instancias != 1 || l.entradas.iter().any(|e| e.por_instancia.is_some()) {
            return Err(String::from("el lote dibuja varias instancias o lee datos por instancia: la 3060 no lo sabe todavia"));
        }
        // N5.11: la 3060 aun no mezcla (ni enmascara): ese lote, por la CPU.
        if !l.reglas.mezcla.trivial() {
            return Err(String::from("el lote mezcla (o escribe solo algunos canales): la 3060 no lo sabe todavia"));
        }
        // 05-10: el stencil vive en la RAM de la casa (un byte por texel del
        // DSV) y la receta no lo lleva: ese lote, por la CPU (y `despues`
        // da por muerta la Z de la 3060 si el lote tambien la usa).
        if l.reglas.stencil.is_some() {
            return Err(String::from("el lote usa STENCIL: la 3060 no lo prueba ni lo escribe todavia (su plano vive en la RAM de la casa)"));
        }
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
                // A9: vivos -- del .bsf de antes, o traducidos, comprobados
                // contra la CPU y guardados.
                let r = cuerpos_con(l.enlace, l.entradas, self.recuerdo.as_mut().map(|r| r.as_mut() as &mut dyn Recuerdo));
                if let Ok((_, v)) = &r {
                    match v.origen {
                        Origen::Traducido => self.traducidos += 1,
                        Origen::Recordado => self.recordados += 1,
                    }
                }
                self.cuerpos.push((clave, r, 0));
                self.cuerpos.len() - 1
            }
        };
        // A9c: EL VIGIA -- el primer lote de este PSO, y uno de cada
        // `vigia_cada`, con los datos del juego. Si no cuadra: por la CPU
        // desde YA (este lote no llega mal a la pantalla), marcado malo en el
        // recuerdo (el arranque siguiente ni lo intenta), y se dice.
        let (_, r, lotes) = &mut self.cuerpos[i];
        *lotes += 1;
        if let Ok((c, v)) = r {
            if (*lotes - 1) % self.vigia_cada.max(1) == 0 {
                self.revisados += 1;
                if let Err(m) = crate::vivo::revisar(l.enlace, &c.elementos, &v.vs, &v.ps, l) {
                    if let (Some(rec), Some(nombre)) = (self.recuerdo.as_mut(), &v.nombre) {
                        rec.guardar(&crate::vivo::malo(nombre), m.as_bytes());
                    }
                    self.corregidos += 1;
                    *r = Err(NoVa::Juez("el vigia, con los datos del juego", m));
                }
            }
        }
        let c = match &self.cuerpos[i].1 {
            Ok((c, _)) => c,
            Err(e) => return Err(format!("el PSO no va a la 3060: {e:?}")),
        };
        let mut n = escribir(c, l, b, l.limpiar_z, &mut self.datos, &mut self.caja)?;
        let con_libreta = c.termometro_vs.is_some() || c.termometro_ps.is_some();
        // UNA vez por PSO y paso: el pegado de prueba, como lo hara el kernel.
        if !self.probados.iter().any(|p| p.0 == clave && p.1 == l.paso) {
            let mut prueba = self.pegar_de_prueba(n);
            // 9d: si con la libreta no cabe (cinco instrucciones mas por
            // programa, y las del termometro), el PSO sigue SIN ella: lo de
            // antes, que si cabia. Sin recuerdo: es un caso raro.
            if prueba.is_err() && con_libreta {
                if let Ok((c2, v2)) = cuerpos_con_libreta(l.enlace, l.entradas, None, false) {
                    self.sin_libreta += 1;
                    self.cuerpos[i].1 = Ok((c2, v2));
                    if let Ok((c2, _)) = &self.cuerpos[i].1 {
                        n = escribir(c2, l, b, l.limpiar_z, &mut self.datos, &mut self.caja)?;
                        prueba = self.pegar_de_prueba(n);
                    }
                }
            }
            self.probados.push((clave, l.paso, prueba));
        }
        match &self.probados.iter().find(|p| p.0 == clave && p.1 == l.paso).expect("recien puesto").2 {
            Ok(()) => Ok(n),
            Err(e) => Err(e.clone()),
        }
    }

    /// El pegado de prueba de la receta de `self.caja[..n]`, como el kernel.
    fn pegar_de_prueba(&mut self, n: usize) -> Result<(), String> {
        match receta::leer(&self.caja[..n]) {
            None => Err(String::from("la receta no se relee")),
            Some(r) => receta::pegar(&r, &mut self.taller).map_err(|e| format!("el kernel no la pegaria: {e:?}")),
        }
    }

    /// **LA LIBRETA DIJO RARO en el dibujo de `l`** (9d, 06-10): el kernel lo
    /// dice en el `Ok` de SU receta (`cubo::raro`). El siguiente lote de ese
    /// PSO lo revisa el vigia con los datos del juego YA, sin esperar a su
    /// turno de `vigia_cada`: la 3060 dice donde mirar, y la CPU decide.
    /// `false` si el PSO no es de esta puerta (o ya va por la CPU).
    pub fn apunto(&mut self, l: &Lote) -> bool {
        let clave = l.enlace as *const Enlace as usize;
        match self.cuerpos.iter_mut().find(|x| x.0 == clave) {
            Some((_, Ok(_), lotes)) => {
                *lotes = 0;
                self.apuntados += 1;
                true
            }
            _ => false,
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
