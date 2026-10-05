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

use bmo_proton_x::bufer::Uav;
use bmo_proton_x::dxil::programa::Lugar;
use bmo_proton_x::raiz::Firma;

use crate::com::de;
use crate::tuberia::{Estado, RootSignature};
use crate::{aviso, d3d12};

/// D3D12_UAV_DIMENSION_BUFFER.
const UAV_BUFER: u32 = 1;

/// **El UAV de bufer del lugar `l`**, buscado en las tablas de la raiz: la
/// memoria de su bufer desde su primer elemento, para escribirla; `None` (y
/// se lee como nulo) si no hay, o si es de una textura (todavia no).
fn uav_de(firma: &Firma, tablas: &[u64; 16], l: Lugar) -> Option<Uav<'static>> {
    use bmo_proton_x::donde::{self, RANGO_UAV};
    let r = donde::en_tabla(firma, RANGO_UAV, l).and_then(|(k, i)| crate::tuberia::descriptor_de(tablas, k, i)).filter(|r| r[1] == d3d12::DESC_UAV && r[0] != 0)?;
    if crate::d3d12_vistas::leer(r).0 .0 != UAV_BUFER {
        aviso("Dispatch: un UAV de TEXTURA (RWTexture2D...): todavia no; se ve nulo");
        return None;
    }
    let v = crate::d3d12_vistas::leer_bufer(r);
    let base = d3d12::base_de_bufer(r[0])?;
    let medida = match (v.crudo, v.paso, bmo_proton_x::formato_ia::forma(v.formato)) {
        (true, _, _) => 4,
        (false, p, _) if p != 0 => p as u64,
        (false, _, Some(f)) => f.bytes as u64,
        _ => {
            aviso("Dispatch: un UAV de bufer sin paso ni un formato que la casa sepa (se ve nulo)");
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
    Some(Uav { bytes, formato, paso: if v.crudo { 0 } else { v.paso }, elementos })
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
    let (texturas, muestreadores, buferes) = crate::tuberia::recursos_del_dibujo(firma, &e.tablas, &p.programa.ranuras);
    let mut uavs: Vec<Option<Uav>> = p.programa.ranuras.uavs.iter().map(|&l| uav_de(firma, &e.tablas, l)).collect();
    // E2.3b (05-10): con su traduccion a x86-64 si la hay (EXPRIMIR: 50
    // veces el interprete); si no, el interprete, que es su juez.
    if let Some(f) = pso.nativo.and_then(crate::nativo::computo) {
        // SAFETY: `f` es la traduccion de este programa (`registrar_computo`
        // al crear el PSO), en el bloque sellado de ahora: un Dispatch no
        // cede el turno, asi que nadie lo cambia mientras corre.
        unsafe { bmo_proton_x::nativo_computo::despachar(&p.programa, f, grupos, &cb, &buferes, &mut uavs) };
        return;
    }
    let rec = bmo_proton_x::textura::Recursos { texturas: &texturas, muestreadores: &muestreadores, buferes: &buferes, dinamicas: None };
    p.programa.despachar(grupos, &cb, &rec, &mut uavs);
}
