//! Inicializacion 615 por GMCAPI, separada de INIT_DONE y RPC 570.
//! [consumo] NADA -- hasta 30 s cediendo pantalla entre tramos de Ring 0.
use bmo_userland as bmo;
use crate::scene::output::Output;
use super::tabla::campo;

static mut RESULTADO: Option<Result<u64, u32>> = None;
pub(crate) const NO_615_TIEMPO: u32 = 0x203;
pub(crate) fn es_615() -> bool { bmo::info(bmo::INFO_GPU_GSP) & bmo::GSP_ES_615 != 0 }
pub(crate) fn estado() -> u64 { bmo::info(bmo::INFO_GPU_DESPIERTO_BUZON | 2 << 8) }
pub(crate) fn listo() -> bool { estado() >> 24 & bmo::SEC_HECHO != 0 }
pub(crate) fn correr() -> Result<u64, u32> {
    if let Some(r) = unsafe { *core::ptr::addr_of!(RESULTADO) } { return r; }
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let r = loop {
        match bmo::iommu_orden(bmo::IOMMU_OP_GSP_SECUENCIAR) {
            Ok(v) if v >> 24 & bmo::SEC_HECHO != 0 => break Ok(v & 0xFFFF),
            Ok(_) => {}, Err(m) => break Err(m),
        }
        if bmo::ciclos().saturating_sub(desde) >= hz.saturating_mul(30) { break Err(NO_615_TIEMPO); }
        bmo::yield_screen();
    };
    unsafe { *core::ptr::addr_of_mut!(RESULTADO) = Some(r); }
    r
}
pub(crate) fn fila(s: &mut Output) {
    campo(s, b"gmcapi");
    if listo() {
        s.text(b"615: GSP_INIT respondido en esta sesion, secuencia correcta, estado cero y NVKV valido\n");
    } else if let Some(Err(m)) = unsafe { *core::ptr::addr_of!(RESULTADO) } {
        s.text(b"615: inicializacion detenida; motivo "); s.dec(m as u64);
        s.text(b"; "); s.text(super::iommu::motivo(m)); s.byte(b'\n');
    } else {
        s.text(b"615: GSP_INIT aun no confirmado; no se espera INIT_DONE de 570\n");
    }
    campo(s, b"615 raw"); s.hex(estado(), 16);
    s.text(b"; ver CABINA F2b y el volcado gspcola.bin; no acredita motores ni juegos\n");
}
