//! **`ID3D12GraphicsCommandList1` a `10`** (tanda 48, 02-10).
//!
//! Un motor pide la lista mas nueva que haya (`QueryInterface`) y usa lo de
//! cada version que necesita. Es el MISMO objeto con una vtabla mas larga (86
//! huecos, los de `d3d12.idl`; la 8, la 9 y la 10 de vkd3d-proton). Lo que
//! la casa hace con cada uno:
//!
//! ```text
//!    1  AtomicCopyBufferUINT(64)      la copia (la cola es sincrona: ya es
//!                                     atomica respecto a la GPU)
//!       OMSetDepthBounds, SetSamplePositions, SetViewInstanceMask
//!                                     nada (la casa anuncia que no los tiene)
//!       ResolveSubresourceRegion      una muestra: copiar el recurso
//!    2  WriteBufferImmediate          los valores, en su sitio, al ejecutarse
//!    3  SetProtectedResourceSession   nada (no hay sesiones protegidas)
//!    4  BeginRenderPass/EndRenderPass los destinos de OMSetRenderTargets y las
//!                                     limpiezas de su BeginningAccess CLEAR
//!    5  RSSetShadingRate(Image)       nada (VRS anunciado como NO)
//!    7  Barrier                       nada: la cola de la casa es sincrona
//!    8  OMSetFrontAndBackStencilRef   nada (sin stencil todavia)
//!    9  RSSetDepthBias, IASetIndexBufferStripCutValue   nada
//! ```
//!
//! Lo de rayos, malla, meta-ordenes y work graphs son fallas documentadas
//! (`fallas.rs`): la casa anuncia que no los tiene.
//!
//! [!] Los `float` llegan por xmm y la casa es soft-float en Ring 3: los
//! metodos que no hacen nada son UNA funcion que no mira sus argumentos
//! (`nada`), la forma segura de ignorarlos.

use crate::com::de;
use crate::d3d12::{self, Lista, Orden};
use crate::{aviso, dir};

/// Los huecos de ID3D12GraphicsCommandList1 a 10 que pone esto.
pub(crate) fn lista() -> [(usize, u64); 16] {
    [
        (60, dir!(atomic_copy_uint)),
        (61, dir!(atomic_copy_uint64)),
        (62, dir!(nada)),
        (63, dir!(nada)),
        (64, dir!(resolve_subresource_region)),
        (65, dir!(nada)),
        (66, dir!(write_buffer_immediate)),
        (67, dir!(nada)),
        (68, dir!(begin_render_pass)),
        (69, dir!(nada)),
        (77, dir!(nada)),
        (78, dir!(nada)),
        (80, dir!(nada)),
        (81, dir!(nada)),
        (82, dir!(nada)),
        (83, dir!(nada)),
    ]
}

/// Un metodo `void` que la casa no necesita hacer: no lee sus argumentos.
extern "win64" fn nada(_this: u64) {}

fn l<'a>(this: u64) -> &'a mut Lista {
    // SAFETY: `this` es una Lista de la casa (lo dice su vtabla).
    unsafe { de::<Lista>(this) }
}

/// Copiar `n` bytes de un bufer a otro, comprobado (las dos atomicas).
fn copia(this: u64, dst: u64, desde_dst: u64, src: u64, desde_src: u64, n: u64) {
    match (d3d12::base_de_bufer(dst), d3d12::base_de_bufer(src)) {
        (Some(d), Some(s)) if crate::tuberia::dentro_de_bufer(d + desde_dst, n as usize) && crate::tuberia::dentro_de_bufer(s + desde_src, n as usize) => {
            l(this).ordenes.push(Orden::Atomica { dst: d + desde_dst, src: s + desde_src, n });
        }
        _ => aviso("AtomicCopyBuffer: fuera de un bufer de la casa"),
    }
}

/// `AtomicCopyBufferUINT(this, dst, desde, src, desde, n_deps, deps, rangos)`.
#[allow(clippy::too_many_arguments)]
extern "win64" fn atomic_copy_uint(this: u64, dst: u64, desde_dst: u64, src: u64, desde_src: u64, _n: u32, _deps: *const u64, _rangos: *const u8) {
    copia(this, dst, desde_dst, src, desde_src, 4);
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn atomic_copy_uint64(this: u64, dst: u64, desde_dst: u64, src: u64, desde_src: u64, _n: u32, _deps: *const u64, _rangos: *const u8) {
    copia(this, dst, desde_dst, src, desde_src, 8);
}

/// `ResolveSubresourceRegion(this, dst, sub, x, y, src, sub, rect, formato,
/// modo)`: la casa no tiene MSAA (una muestra): el recurso entero, copiado.
#[allow(clippy::too_many_arguments)]
extern "win64" fn resolve_subresource_region(this: u64, dst: u64, _sd: u32, x: u32, y: u32, src: u64, _ss: u32, rect: *const u8, _f: u32, _m: u32) {
    if x != 0 || y != 0 || !rect.is_null() {
        aviso("ResolveSubresourceRegion con una region: la casa copia el recurso entero");
    }
    crate::d3d12_resto::copy_resource(this, dst, src);
}

/// `WriteBufferImmediate(this, n, parametros, modos)`:
/// D3D12_WRITEBUFFERIMMEDIATE_PARAMETER -- Dest +0 (una direccion de GPU,
/// que en la casa es la del bufer), Value +8; 16 bytes cada uno.
extern "win64" fn write_buffer_immediate(this: u64, n: u32, p: *const u8, _modos: *const u32) {
    if p.is_null() {
        return;
    }
    for i in 0..n as usize {
        // SAFETY: `n` parametros del `.exe`.
        let (dst, valor) = unsafe { ((p.add(16 * i) as *const u64).read_unaligned(), (p.add(16 * i + 8) as *const u32).read_unaligned()) };
        if crate::tuberia::dentro_de_bufer(dst, 4) {
            l(this).ordenes.push(Orden::Escribir { dst, valor });
        } else {
            aviso("WriteBufferImmediate a una direccion que no es de un bufer de la casa");
        }
    }
}

/// D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR.
const LIMPIAR: u32 = 2;
/// D3D12_RENDER_PASS_RENDER_TARGET_DESC (88 B): cpuDescriptor +0,
/// BeginningAccess +8 (Type +0, ClearValue +4: Format, Color[4]),
/// EndingAccess +32. D3D12_RENDER_PASS_DEPTH_STENCIL_DESC (168 B):
/// cpuDescriptor +0, DepthBeginningAccess +8 (Depth en +8 de ella).
const RT: usize = 88;

/// **`BeginRenderPass(this, n, destinos, profundidad, banderas)`**: lo que
/// es por debajo -- OMSetRenderTargets con esos descriptores y, si su acceso
/// de entrada es CLEAR, la limpieza con su valor (como Clear*View). Lo de la
/// salida (resolver, descartar) no hace falta: una muestra y memoria de CPU.
extern "win64" fn begin_render_pass(this: u64, n: u32, destinos: *const u8, profundidad: *const u8, _banderas: u32) {
    let mut handles = alloc::vec::Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let d = destinos.wrapping_add(RT * i);
        // SAFETY: `n` D3D12_RENDER_PASS_RENDER_TARGET_DESC del `.exe`.
        let (cpu, tipo) = unsafe { ((d as *const u64).read_unaligned(), (d.add(8) as *const u32).read_unaligned()) };
        handles.push(cpu);
        if tipo == LIMPIAR {
            // El color: cuatro floats en +16 (+8 del acceso, +4 del formato, +4).
            d3d12::clear_render_target_view(this, cpu, d.wrapping_add(16) as *const f32, 0, core::ptr::null());
        }
    }
    let dsv = if profundidad.is_null() {
        0
    } else {
        // SAFETY: un D3D12_RENDER_PASS_DEPTH_STENCIL_DESC del `.exe`.
        let (cpu, tipo, bits) = unsafe { ((profundidad as *const u64).read_unaligned(), (profundidad.add(8) as *const u32).read_unaligned(), (profundidad.add(16) as *const u32).read_unaligned()) };
        if tipo == LIMPIAR {
            // CLEAR_FLAG_DEPTH, y los BITS del float (la casa es soft-float).
            d3d12::clear_depth_stencil_view(this, cpu, 1, bits, 0, 0, core::ptr::null());
        }
        cpu
    };
    let dsv_ptr = if profundidad.is_null() { core::ptr::null() } else { &dsv as *const u64 };
    d3d12::om_set_render_targets(this, n, if n == 0 { core::ptr::null() } else { handles.as_ptr() }, 0, dsv_ptr);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_huecos_son_de_la_lista_1_a_10() {
        let nombres = crate::com::M_ID3D12GRAPHICSCOMMANDLIST;
        assert_eq!(nombres.len(), 86);
        for (k, _) in lista() {
            assert!((60..86).contains(&k), "hueco {k}");
        }
        assert_eq!(nombres[66], "WriteBufferImmediate");
        assert_eq!(nombres[68], "BeginRenderPass");
        // Los de rayos y malla no son de aqui: son fallas documentadas.
        for k in [72, 76, 79] {
            assert!(!lista().iter().any(|x| x.0 == k));
            assert!(crate::fallas::FALLAS.iter().any(|f| f.interfaz == crate::com::LIST && f.hueco == k));
        }
    }
}
