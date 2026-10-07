//! **El COMPUTO de D3D12 en la casa** (N5.5, 05-10): un `Dispatch` apuntado
//! en una lista (`Orden::Despachar`), corrido al ejecutarla.
//!
//! ```text
//!    el PSO de computo        su CS ya compilado y con los cbuffers
//!                             aplanados (al CREARLO: d3d12_resto)
//!    su root signature        la de SetComputeRootSignature: las tablas, los
//!                             CBV y las constantes de la raiz de COMPUTO
//!    los cbuffers             como los de un dibujo (`cbuffers::de_ranuras`)
//!    los SRV                  como los de un dibujo (`recursos_del_dibujo`)
//!    los UAV de bufer         de su tabla: la memoria del bufer, que el
//!                             computo ESCRIBE (`bmo_proton_x::bufer::Uav`)
//!    correr                   `Programa::despachar`: grupo a grupo, con su
//!                             memoria compartida y sus barreras, en la CPU
//! ```
//!
//! La cola es sincrona: cuando ExecuteCommandLists vuelve, el computo ya
//! escribio (lo que lea despues, una copia o un dibujo, lo ve).

use alloc::vec::Vec;

use bmo_proton_x::bufer::{Rebanadas, Uav};
use bmo_proton_x::dxil::programa::Lugar;
use bmo_proton_x::raiz::Firma;

use crate::com::de;
use crate::tuberia::{Estado, RootSignature};
use crate::{aviso, d3d12};

/// D3D12_UAV_DIMENSION_BUFFER.
const UAV_BUFER: u32 = 1;

/// **El UAV de bufer del lugar `l`**, buscado en las tablas de la raiz: la
/// memoria de su bufer desde su primer elemento, para escribirla (o la de su
/// textura: `uav_de_textura`); `None` (y se lee como nulo) si no hay. Lo
/// usan el Dispatch y, desde el 05-10, el Draw (`tuberia::pintar`).
pub(crate) fn uav_de(firma: &Firma, tablas: &[u64; 16], raiz: &[u64; 16], ranuras: &bmo_proton_x::dxil::programa::Ranuras, l: Lugar) -> Option<Uav<'static>> {
    use bmo_proton_x::donde::{self, RANGO_UAV};
    // N5.3b (05-10): un UAV en la RAIZ: crudo o estructurado, sin contador
    // (D3D12 no deja otros ahi).
    if let Some(k) = donde::en_raiz(firma, bmo_proton_x::raiz::UAV, l) {
        let (bytes, paso, elementos) = crate::tuberia::bufer_de_raiz(raiz[k], ranuras.paso(true, l))?;
        // SAFETY: la memoria de un bufer de la casa (`resolver_hasta` comprobo
        // que es suya y cuanto mide); el computo es el unico que la toca
        // mientras corre (la cola es sincrona y de un hilo).
        let bytes = unsafe { core::slice::from_raw_parts_mut(bytes.as_ptr() as *mut u8, bytes.len()) };
        return Some(Uav { bytes, formato: 0, paso, elementos, contador: None, rebanadas: Rebanadas::PLANA });
    }
    let r = donde::en_tabla(firma, RANGO_UAV, l).and_then(|(k, i)| crate::tuberia::descriptor_de(tablas, k, i)).filter(|r| r[1] == d3d12::DESC_UAV && r[0] != 0)?;
    let ((dimension, formato_vista, _), _) = crate::d3d12_vistas::leer(r);
    if dimension != UAV_BUFER {
        return uav_de_textura(r, dimension, formato_vista);
    }
    let v = crate::d3d12_vistas::leer_bufer(r);
    let base = d3d12::base_de_bufer(r[0])?;
    let medida = match (v.crudo, v.paso, bmo_proton_x::formato_ia::forma(v.formato)) {
        (true, _, _) => 4,
        (false, p, _) if p != 0 => p as u64,
        (false, _, Some(f)) => f.bytes as u64,
        _ => {
            aviso("Dispatch o Draw: un UAV de bufer sin paso ni un formato que la casa sepa (se ve nulo)");
            return None;
        }
    };
    let bytes = crate::tuberia::resolver_hasta(base + v.primero * medida, v.elementos as usize * medida as usize)?;
    // SAFETY: la memoria de un bufer de la casa (`resolver_hasta` comprobo
    // que es suya y cuanto mide). El computo es el unico que la toca mientras
    // corre (la cola es sincrona y de un hilo).
    let bytes = unsafe { core::slice::from_raw_parts_mut(bytes.as_ptr() as *mut u8, bytes.len()) };
    let elementos = (bytes.len() as u64 / medida) as u32;
    let formato = if v.crudo || v.paso != 0 { 0 } else { v.formato };
    let contador = contador_de(crate::d3d12_vistas::leer(r).0 .2, bytes);
    Some(Uav { bytes, formato, paso: if v.crudo { 0 } else { v.paso }, elementos, contador, rebanadas: Rebanadas::PLANA })
}

/// D3D12_UAV_DIMENSION_TEXTURE1D, TEXTURE2D y (06-10) TEXTURE2DARRAY y
/// TEXTURE3D.
const UAV_TEXTURA_1D: u32 = 2;
const UAV_TEXTURA_2D: u32 = 4;
const UAV_TEXTURA_2D_ARRAY: u32 = 5;
const UAV_TEXTURA_3D: u32 = 8;

/// **Un UAV de TEXTURA** (N5.3c, 05-10: `RWTexture2D`, el post-proceso): la
/// memoria de su subresource, texel a texel, en el formato en que la casa
/// la GUARDA (8 bits por canal, un float de 32 o, N5.16b, los cuatro floats
/// de un RGBA16F, R11G11B10F...: se escriben cuantizados al formato de la
/// vista, `bufer::CUATRO_FLOATS`).
///
/// 06-10: y los de un 3D (`RWTexture3D`, sus rebanadas desde FirstWSlice) o
/// un array de 2D (`RWTexture2DArray`, sus capas desde FirstArraySlice):
/// la memoria de la primera a la ultima, y donde cae cada una
/// (`bufer::Rebanadas`; ver [`rebanadas_de`]).
fn uav_de_textura(r: &[u64], dimension: u32, formato_vista: u32) -> Option<Uav<'static>> {
    use crate::subrecursos::Almacen;
    if ![UAV_TEXTURA_1D, UAV_TEXTURA_2D, UAV_TEXTURA_2D_ARRAY, UAV_TEXTURA_3D].contains(&dimension) {
        aviso("Dispatch o Draw: un UAV de textura de array de una dimension o multimuestra: todavia no; se ve nulo");
        return None;
    }
    // SAFETY: un Recurso de la casa (lo dice su ranura).
    let formato = unsafe { crate::com::de::<d3d12::Recurso>(r[0]) }.formato;
    let entero = bmo_proton_x::formato_ia::forma(formato_vista).is_some_and(|f| matches!(f.clase, bmo_proton_x::formato_ia::Clase::Uint | bmo_proton_x::formato_ia::Clase::Sint));
    let vista = if formato_vista != 0 { formato_vista } else { formato };
    let mide = |f: u32| bmo_proton_x::formato_ia::forma(f).map(|x| x.bytes);
    let efectivo = match Almacen::de(formato) {
        // D2.7 (06-10): lo que la casa guarda TAL CUAL (RGBA8, BGRA8, R32):
        // sus bytes son los de la memoria, y cualquier vista de 4 bytes los
        // lee en su formato (un R32_UINT sobre un RGBA8, un RGBA8_SNORM...).
        _ if crate::subrecursos::interno_es_nativo(formato) && mide(vista) == Some(4) => vista,
        Almacen::Rgba8 if entero => 30,
        Almacen::Rgba8 => 28,
        Almacen::Bgra8 => 87,
        Almacen::Flotante if (41..=43).contains(&formato_vista) => formato_vista,
        Almacen::Flotante => 41,
        Almacen::Bloques(_) => {
            aviso("Dispatch o Draw: un UAV de una textura de bloques: en Windows es un error; se ve nulo");
            return None;
        }
        // N5.16b: la vista de float de su formato (o la de su TYPELESS). D2.7
        // (06-10): otra del mismo tamanio (un R32_UINT sobre un R11G11B10F,
        // el truco de leer con tipo) lee y escribe los bytes del elemento.
        Almacen::Flotantes4 => {
            let guardado = Almacen::nativo(formato);
            if Almacen::nativo(vista) == guardado {
                guardado | bmo_proton_x::bufer::CUATRO_FLOATS
            } else if mide(vista).is_some() && mide(vista) == mide(guardado) {
                bmo_proton_x::bufer::con_vista(guardado, vista)
            } else {
                aviso("Dispatch o Draw: un UAV de textura con una vista de otro tamanio de elemento: en Windows es un error; se ve nulo");
                return None;
            }
        }
    };
    if dimension == UAV_TEXTURA_2D_ARRAY || dimension == UAV_TEXTURA_3D {
        crate::tuberia::aplicar_limpieza(r[0]);
        return rebanadas_de(r, dimension == UAV_TEXTURA_3D, efectivo);
    }
    if r[3] == 0 {
        crate::tuberia::aplicar_limpieza(r[0]);
    }
    let Some((px, ancho, alto)) = crate::tuberia::destino(r[0], r[3]) else {
        aviso("Dispatch o Draw: un UAV de un subrecurso que la textura no tiene; se ve nulo");
        return None;
    };
    // SAFETY: las palabras de un subrecurso de la casa, vistas como bytes;
    // la cola es sincrona: nadie mas las toca mientras corre el Dispatch.
    let bytes = unsafe { core::slice::from_raw_parts_mut(px.as_mut_ptr() as *mut u8, px.len() * 4) };
    Some(Uav { bytes, formato: efectivo, paso: ancho, elementos: ancho * alto, contador: None, rebanadas: Rebanadas::PLANA })
}

/// **Un UAV de textura 3D o de array** (06-10), del subrecurso de su ranura
/// (la mip de la vista; en un array, su primera capa): en un 3D las
/// rebanadas van seguidas en su subrecurso (`hondo` de ellas); en un array
/// cada capa es otro subrecurso, `mips` mas alla, y entre una y la
/// siguiente va el resto de la cadena de mips (el salto). Cuantas: las de
/// la vista (WSize, ArraySize), o todas las que quedan.
pub(crate) fn rebanadas_de(r: &[u64], es_3d: bool, efectivo: u32) -> Option<Uav<'static>> {
    let t = crate::d3d12_vistas::tex(r[0])?;
    let (sub, primera) = (r[3] as u32, (r[3] >> 32) as u32);
    let s = *t.subs.get(sub as usize)?;
    let (bytes_texel, lado) = t.almacen.elemento();
    // Lo que el UAV direcciona: texeles de 4 o 16 bytes, filas sin relleno.
    if lado != 1 || s.fila != s.ancho as u64 * bytes_texel {
        aviso("Dispatch o Draw: un UAV de textura 3D o de array con filas de otra medida: todavia no; se ve nulo");
        return None;
    }
    let por_rebanada = s.ancho as u64 * s.alto as u64 * bytes_texel;
    let mips = t.forma.mips.max(1);
    let (desde, salto, quedan) = if es_3d {
        (s.desde + primera as u64 * por_rebanada, por_rebanada, s.hondo.saturating_sub(primera))
    } else {
        let capa = sub / mips;
        let salto = t.subs.get((sub + mips) as usize).map_or(por_rebanada, |sig| sig.desde - s.desde);
        (s.desde, salto, t.forma.capas().saturating_sub(capa))
    };
    let pedidas = crate::d3d12_vistas::cuantas_de(r);
    let capas = if pedidas == 0 { quedan } else { pedidas.min(quedan) };
    if capas == 0 {
        aviso("Dispatch o Draw: un UAV de textura 3D o de array sin rebanadas dentro de la textura; se ve nulo");
        return None;
    }
    let n = (capas as u64 - 1) * salto + por_rebanada;
    // SAFETY: de `desde` a `desde + n` cae dentro de la memoria de la
    // textura (sus subrecursos estan dentro de `datos`); la cola es
    // sincrona: nadie mas la toca mientras corre el Dispatch o el Draw.
    let bytes = unsafe { core::slice::from_raw_parts_mut((t.datos + desde) as *mut u8, n as usize) };
    let rebanadas = Rebanadas { alto: s.alto, capas, salto: (salto / bytes_texel) as u32 };
    Some(Uav { bytes, formato: efectivo, paso: s.ancho, elementos: (n / bytes_texel) as u32, contador: None, rebanadas })
}

/// **El contador oculto de un UAV** (E2.4, 05-10): el numero `n` de su ranura
/// (ver `d3d12_vistas::contador_de`), en la memoria de su recurso. Si cae
/// dentro de los datos de la vista (dos `&mut` a lo mismo), no: se dice.
fn contador_de(n: u32, datos: &[u8]) -> Option<&'static mut u32> {
    let (recurso, desplazamiento) = crate::d3d12_vistas::contador(n)?;
    let base = d3d12::base_de_bufer(recurso)?;
    let va = base + desplazamiento;
    let c = crate::tuberia::resolver_hasta(va, 4).filter(|c| c.len() == 4 && va % 4 == 0)?;
    let (desde, hasta) = (datos.as_ptr() as u64, datos.as_ptr() as u64 + datos.len() as u64);
    if va < hasta && va + 4 > desde {
        aviso("Dispatch o Draw: el contador de un UAV cae dentro de sus datos (en Windows es un error): se ve sin contador");
        return None;
    }
    // SAFETY: cuatro bytes alineados de un bufer de la casa, fuera de los
    // datos de la vista: nadie mas los toca mientras corre el Dispatch.
    Some(unsafe { &mut *(c.as_ptr() as *mut u32) })
}

/// **Correr un `Dispatch(grupos)`** con el estado de computo `e`.
pub(crate) fn despachar(e: &Estado, grupos: [u32; 3]) {
    crate::pulso::contar(crate::pulso::Cosa::Lista, 0);
    if e.pso == 0 || !crate::d3d12_resto::es_computo(e.pso) {
        aviso("Dispatch sin un PSO de computo: en Windows es un error, y no se hace");
        return;
    }
    if e.raiz == 0 {
        aviso("Dispatch sin SetComputeRootSignature: en Windows es un error, y no se hace");
        return;
    }
    // SAFETY: un PSO de computo de la casa (`es_computo`) y una RootSignature
    // de la casa (los Set* solo guardan de esos).
    let (pso, firma) = unsafe { (de::<crate::d3d12_resto::Computo>(e.pso), &de::<RootSignature>(e.raiz).firma) };
    // Lo que no compilo ya lo dijo CreateComputePipelineState.
    let Ok(p) = &pso.preparado else { return };
    // SAFETY: como arriba.
    if firma != unsafe { &de::<RootSignature>(pso.raiz).firma } {
        aviso("Dispatch con una root signature distinta de la del PSO: en Windows es un error");
        return;
    }
    let cb = match crate::cbuffers::de_ranuras(firma, e, &p.programa.ranuras.cbuffers, &p.constantes) {
        Ok(c) => c,
        Err(m) => {
            aviso(&alloc::format!("Dispatch: {m}: no se hace"));
            return;
        }
    };
    let (texturas, muestreadores, buferes) = crate::tuberia::recursos_del_dibujo(firma, &e.tablas, &e.cbv, &p.programa.ranuras);
    let mut uavs: Vec<Option<Uav>> = p.programa.ranuras.uavs.iter().map(|&l| uav_de(firma, &e.tablas, &e.cbv, &p.programa.ranuras, l)).collect();
    // 06-10: y el indice DINAMICO (el bindless: `textures[i]`), como en un
    // dibujo; antes un CS que elegia su textura la leia como nula. Buscadas
    // cuando un hilo las pide y GUARDADAS: una vez por textura distinta del
    // Dispatch, no por hilo (lo de `tuberia::pintar`).
    let guardadas: core::cell::RefCell<alloc::collections::BTreeMap<(u8, u32), Option<bmo_proton_x::textura::Textura<'static>>>> = Default::default();
    let buscar = |rango: u8, registro: u32| {
        if let Some(t) = guardadas.borrow().get(&(rango, registro)) {
            return *t;
        }
        let t = crate::tuberia::textura_dinamica(firma, &e.tablas, &p.programa.ranuras, rango, registro);
        guardadas.borrow_mut().insert((rango, registro), t);
        t
    };
    let rec = bmo_proton_x::textura::Recursos { texturas: &texturas, muestreadores: &muestreadores, buferes: &buferes, dinamicas: Some(bmo_proton_x::textura::Dinamicas(&buscar)) };
    // E2.3b (05-10): con su traduccion a x86-64 si la hay (EXPRIMIR: 50
    // veces el interprete); si no, el interprete, que es su juez.
    if let Some(f) = pso.nativo.and_then(crate::nativo::computo) {
        crate::nativo::despachar_computo(&p.programa, f, grupos, &cb, &rec, &buferes, &mut uavs);
        return;
    }
    p.programa.despachar(grupos, &cb, &rec, &mut uavs);
}
