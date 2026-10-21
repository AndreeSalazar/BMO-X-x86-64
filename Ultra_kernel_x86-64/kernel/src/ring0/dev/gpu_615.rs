//! GSP 615: GSP_INIT cerrado y recepcion MCTP/NVDM desde sus registros.
//! [carril] ROJO -- publica solo una orden construida aqui; consume respuesta validada.
//! [consumo] NADA -- un mensaje por llamada autorizada de MAQUINA.
//! [estado]  RAM  contadores, respuesta y corredor HS de esta sesion.
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use bmo_gpu_ga10x::{libos as lb, mctp615 as q, Registros};
use super::{gpu, gpu_gsp, gpu_libos, gpu_prestamo::{self, Bar0}};
use bmo_gpu_ga10x::{arranque::hs615, correr::{Contexto, Corredor, Tramo}, secuenciador::Orden};

pub const IOMMU_NO_615_LECTURA: u32 = 96;
pub const IOMMU_NO_615_FORMA: u32 = 97;
pub const IOMMU_NO_615_EVENTO: u32 = 98;
pub const IOMMU_NO_615_RESPUESTA: u32 = 99;
static ESTADO: AtomicU64 = AtomicU64::new(0);
static NO: AtomicU64 = AtomicU64::new(0);
static OCUPADO: AtomicBool = AtomicBool::new(false);
static ENLAZADA: AtomicBool = AtomicBool::new(false);
// La puerta MAQUINA y OCUPADO serializan este bufer de copia, no prestado.
static mut MENSAJE: [u8; q::MAX_BYTES] = [0; q::MAX_BYTES];
static mut ORDENES_HS: [Orden; hs615::MAX_ORDENES] = [Orden::Retraso { us: 0 }; hs615::MAX_ORDENES];
static mut CORREDOR_HS: Corredor = Corredor { i: 0, fase: 0, desde: None };
static HS_N: AtomicU64 = AtomicU64::new(0);
static EVENTOS: AtomicU64 = AtomicU64::new(0);
static HS_EVENTOS: AtomicU64 = AtomicU64::new(0);
static FOTO_RX: AtomicU64 = AtomicU64::new(u64::MAX);
// SAFETY: solo se toca mientras OCUPADO esta adquirido en paso().
static mut SEGUIMIENTO: q::observacion::Seguimiento = q::observacion::Seguimiento::nuevo();
static FOTOS_COLA: AtomicU64 = AtomicU64::new(0);
const SECUENCIA: u64 = 1;
const ENVIADO: u64 = 0x40 << 24;
const HECHO: u64 = 0x80 << 24;

fn ahora_us() -> u64 {
    crate::ring0::task::scheduler::rdtsc()
        / (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1)
}

struct Turno;
impl Drop for Turno { fn drop(&mut self) { OCUPADO.store(false, Ordering::Release); } }
pub fn info() -> u64 { ESTADO.load(Ordering::Acquire) }

fn fallar(m: u32, dato: u64) -> Result<u64, u32> {
    NO.store(m as u64, Ordering::Release);
    ESTADO.fetch_and(!HECHO, Ordering::AcqRel);
    ESTADO.fetch_or(1 << 24, Ordering::AcqRel);
    crate::ring0::cabina::warn("gpu", "F2b 615: NO del transporte", m as u64);
    crate::ring0::cabina::warn("gpu", "F2b 615: dato del NO (crudo)", dato);
    Err(m)
}

fn reg(r: &mut Bar0, dir: u32) -> Result<u32, u32> {
    let v = r.leer(dir);
    if bmo_gpu_ga10x::es_error_pri(v) {
        crate::ring0::cabina::warn("gpu", "F2b 615: PRI registro | valor << 32", dir as u64 | (v as u64) << 32);
        Err(IOMMU_NO_615_LECTURA)
    } else { Ok(v) }
}
fn palabra(f: u64, o: u64) -> u64 {
    // SAFETY: el llamador acota offsets al GspMem contiguo de gpu_libos.
    unsafe { (crate::ring0::mm::phys_to_virt(f + o) as *const u64).read_volatile() }
}
fn copiar(f: u64, tail: u32, destino: &mut [u8]) {
    for (i, trozo) in destino.chunks_mut(8).enumerate() {
        let o = (tail as u64 % q::HUECOS as u64 * 4096 + i as u64 * 8) % (q::HUECOS as u64 * 4096);
        let w = palabra(f, lb::COLA_GSP + 4096 + o).to_le_bytes();
        trozo.copy_from_slice(&w[..trozo.len()]);
    }
}
/// Lista cerrada de lecturas; no publica, reconoce ni consume mensajes.
fn foto_cola(r: &mut Bar0, f: u64, fase: u64, transcurrido: u64) {
    let id = FOTOS_COLA.fetch_add(1, Ordering::AcqRel) + 1;
    // Una previa, seis plazos y una final. Defensa adicional de volumen.
    if id > 8 { return; }
    let inicio = crate::ring0::task::scheduler::rdtsc();
    let foto = q::observacion::Foto::leer(r);
    let mut cabeceras = [0u64; 8];
    for i in 0..4 {
        cabeceras[i] = palabra(f, lb::COLA_CPU + i as u64 * 8);
        cabeceras[i + 4] = palabra(f, lb::COLA_GSP + i as u64 * 8);
    }
    let fin = crate::ring0::task::scheduler::rdtsc();
    use crate::ring0::cabina::info;
    info("gpu", "F2b foto: id | fase << 32 (0 previa,1 espera,2 respuesta)", id | fase << 32);
    info("gpu", "F2b foto: us desde publicacion observada", transcurrido);
    info("gpu", "F2b foto: TSC antes", inicio);
    info("gpu", "F2b foto: TSC despues", fin);
    info("gpu", "F2b foto: mascara PRI de ocho registros", foto.pri as u64);
    for (dir, v) in q::observacion::REGISTROS.iter().zip(foto.crudos) {
        info("gpu", "F2b foto: dir[31:0] raw[63:32]", *dir as u64 | (v as u64) << 32);
    }
    let (tx, rx) = foto.pendientes();
    info("gpu", "F2b foto: pendientes TX (MAX invalido; no prueba ejecucion)", tx.map(u64::from).unwrap_or(u64::MAX));
    info("gpu", "F2b foto: disponibles RX (MAX invalido)", rx.map(u64::from).unwrap_or(u64::MAX));
    for (i, w) in cabeceras.iter().enumerate() {
        info("gpu", "F2b foto: cabecera indice (0..3 TX,4..7 RX)", i as u64);
        info("gpu", "F2b foto: palabra RAM cruda de cabecera", *w);
    }
    info("gpu", "F2b foto: cierre id; lecturas secuenciales", id);
}

fn enviar(r: &mut Bar0, f: u64) -> Result<(), u32> {
    let Some((bus, dev, func)) = gpu::bdf() else { return Err(IOMMU_NO_615_LECTURA) };
    let pci = |off| super::pci::cfg_read32(bus, dev, func, off);
    let barra = |off| {
        let lo = pci(off);
        if lo & 1 != 0 { return 0; }
        (lo & !15) as u64 | if (lo >> 1) & 3 == 2 { (pci(off+4) as u64) << 32 } else { 0 }
    };
    let s = bmo_gpu_ga10x::orden::Sistema { bar0: barra(0x10), bar1: barra(0x14), bar3: barra(0x1C),
        bdf: (bus as u16) << 8 | (dev as u16) << 3 | func as u16, id: pci(0), subid: pci(0x2C), revision: pci(8) as u8 };
    // SAFETY: una pagina CPU->GSP de GspMem, asignada y comprobada; unica orden.
    let dst = unsafe { core::slice::from_raw_parts_mut(crate::ring0::mm::phys_to_virt(f + lb::COLA_CPU + 4096) as *mut u8, 4096) };
    let n = q::iniciar(dst, SECUENCIA, &s).map_err(|_| IOMMU_NO_615_FORMA)?;
    // Segunda llave: identidad, secuencia, comando y bytes EXACTOS construidos.
    if bmo_gpu_ga10x::contrato::permitido_615(&dst[..n], SECUENCIA, &s).is_err() {
        return Err(IOMMU_NO_615_FORMA);
    }
    foto_cola(r, f, 0, 0);
    let huella = bmo_hash::hash(&dst[..n]);
    for (i, w) in huella.chunks_exact(8).enumerate() {
        crate::ring0::cabina::info("gpu", "F2b TX: BLAKE3 indice de palabra LE", i as u64);
        crate::ring0::cabina::info("gpu", "F2b TX: BLAKE3 copia CPU; no prueba consumo GPU", u64::from_le_bytes(w.try_into().unwrap_or([0; 8])));
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    r.escribir(q::TX_HEAD, 1);
    ESTADO.store(ENVIADO, Ordering::Release);
    crate::ring0::cabina::info("gpu", "F2b 615: GSP_INIT publicado por GMCAPI; bytes", n as u64);
    Ok(())
}

fn tramo_hs(r: &mut Bar0) -> Result<u64, u32> {
    let n = HS_N.load(Ordering::Acquire) as usize;
    let ctx = Contexto { boot0: r.leer(bmo_gpu_ga10x::BOOT_0), libos: gpu_libos::IOVA_LIBOS,
        os: gpu_gsp::app_version().ok_or(IOMMU_NO_615_EVENTO)? };
    // SAFETY: solo desde paso con OCUPADO adquirido; ningun alias escapa.
    let resultado = unsafe {
        (&mut *core::ptr::addr_of_mut!(CORREDOR_HS)).correr(
            &(&*core::ptr::addr_of!(ORDENES_HS))[..n], r, &mut gpu_prestamo::reloj(), &ctx, 1000)
    };
    match resultado {
        Tramo::Sigue => Ok(info()),
        Tramo::Hecho => {
            super::gpu_hs::foto_actual(r, bmo_gpu_ga10x::arranque::hs_observacion::Motor::Gsp,
                "HS foto externa tras corredor 615 y reanudacion; id");
            HS_N.store(0, Ordering::Release);
            crate::ring0::cabina::info("gpu", "F2b 615: HS completado y RISC-V reanudado", n as u64);
            Ok(info())
        }
        Tramo::Falla(f) => {
            if let bmo_gpu_ga10x::correr::Falla::Falcon(_, e) = &f {
                let (texto, valor) = bmo_gpu_ga10x::arranque::hs_observacion::fallo(*e);
                crate::ring0::cabina::warn("gpu", texto, valor as u64);
            }
            super::gpu_hs::foto_actual(r, bmo_gpu_ga10x::arranque::hs_observacion::Motor::Gsp,
                "HS foto externa al fallar corredor 615; id");
            crate::ring0::cabina::warn("gpu", "F2b 615: fallo al ejecutar HS, orden", unsafe { (*core::ptr::addr_of!(CORREDOR_HS)).i as u64 });
            let dato = match f {
                bmo_gpu_ga10x::correr::Falla::Fuera(_, r) | bmo_gpu_ga10x::correr::Falla::NoContesta(_, r)
                | bmo_gpu_ga10x::correr::Falla::Plazo(_, r) | bmo_gpu_ga10x::correr::Falla::Sec2(r) => r as u64,
                bmo_gpu_ga10x::correr::Falla::Falcon(_, e) => gpu_prestamo::motivo(e),
            };
            fallar(IOMMU_NO_615_EVENTO, dato)
        }
    }
}

/// Tramo no bloqueante de inicializacion. Un mensaje completo como maximo.
/// Estado reutiliza SEC_CARGADO/SEC_HECHO; HECHO exige respuesta de GSP_INIT
/// de esta secuencia, estado cero y NVKV valido, nunca INIT_DONE de 570.
pub fn paso() -> Result<u64, u32> {
    if gpu_gsp::version() != Some(gpu_gsp::Version::V615) { return Err(gpu_gsp::IOMMU_NO_IDIOMA_615); }
    if OCUPADO.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return Err(gpu_libos::IOMMU_NO_RPC_LLENA);
    }
    let _turno = Turno;
    if NO.load(Ordering::Acquire) != 0 { return Err(NO.load(Ordering::Acquire) as u32); }
    if info() & HECHO != 0 { return Ok(info()); }
    let bar = gpu_prestamo::candado_dma()?;
    if super::gpu_despertar::info_despierto() & super::gpu_despertar::DESPIERTO_VISTO == 0 {
        return Err(gpu_libos::IOMMU_NO_RPC_ANTES);
    }
    let f = gpu_libos::gspmem();
    if f == 0 { return Err(gpu_libos::IOMMU_NO_RPC_ANTES); }
    let mut r = Bar0(bar);
    if info() != 0 {
        let ahora = ahora_us();
        // SAFETY: _turno mantiene OCUPADO hasta salir de paso.
        if let Some(us) = unsafe { (&mut *core::ptr::addr_of_mut!(SEGUIMIENTO)).toca(ahora) } {
            foto_cola(&mut r, f, 1, us);
        }
    }
    if HS_N.load(Ordering::Acquire) != 0 { return tramo_hs(&mut r); }
    if info() == 0 {
        // No se reutilizan punteros de una sesion anterior. La cabecera TX
        // ya fue creada/comprobada por gpu_libos antes de despertar.
        if !super::gpu_despertar::guardar_bar1() { return fallar(IOMMU_NO_615_LECTURA, 0); }
        if let Err(m) = enviar(&mut r, f) { return fallar(m, 0); }
        let ahora = ahora_us();
        // SAFETY: OCUPADO adquirido; empieza al observar la publicacion.
        let us = unsafe { (&mut *core::ptr::addr_of_mut!(SEGUIMIENTO)).toca(ahora) };
        if let Some(us) = us { foto_cola(&mut r, f, 1, us); }
        crate::ring0::cabina::info("gpu", "F2b 615: TX head | tail << 32, crudos", r.leer(q::TX_HEAD) as u64 | (r.leer(q::TX_TAIL) as u64) << 32);
    }
    let head = match reg(&mut r, q::RX_HEAD) { Ok(v) => v, Err(m) => return fallar(m, q::RX_HEAD as u64) };
    if !ENLAZADA.load(Ordering::Acquire) {
        let mut hdr = [0; 32];
        for k in 0..4 { hdr[k*8..k*8+8].copy_from_slice(&palabra(f, lb::COLA_GSP + k as u64*8).to_le_bytes()); }
        if hdr.iter().all(|b| *b == 0) && head == 0 { return Ok(info()); }
        if q::cabecera_cola(&hdr).is_err() { return fallar(IOMMU_NO_615_FORMA, 0); }
        core::sync::atomic::fence(Ordering::SeqCst);
        r.escribir(q::RX_TAIL, 0);
        ENLAZADA.store(true, Ordering::Release);
        crate::ring0::cabina::info("gpu", "F2b 615: cola RX v2 enlazada", head as u64);
    }
    let tail = match reg(&mut r, q::RX_TAIL) { Ok(v) => v, Err(m) => return fallar(m, q::RX_TAIL as u64) };
    let foto = head as u64 | (tail as u64) << 32;
    if FOTO_RX.swap(foto, Ordering::AcqRel) != foto {
        crate::ring0::cabina::info("gpu", "F2b 615: RX head | tail << 32", foto);
    }
    let disponibles = match q::disponibles(head, tail) { Ok(n) => n, Err(_) => return fallar(IOMMU_NO_615_LECTURA, head as u64 | (tail as u64) << 32) };
    if disponibles == 0 { return Ok(info()); }
    core::sync::atomic::fence(Ordering::SeqCst);
    // SAFETY: OCUPADO excluye otro paso y ninguna referencia escapa.
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(MENSAJE) };
    copiar(f, tail, &mut buf[..q::ENVUELTA]);
    let marco = match q::Marco::leer(buf) { Ok(v) => v, Err(_) => return fallar(IOMMU_NO_615_FORMA, palabra(f, lb::COLA_GSP + 4096)) };
    if marco.paginas > disponibles { return fallar(IOMMU_NO_615_FORMA, disponibles as u64); }
    copiar(f, tail, &mut buf[..marco.bytes]);
    if marco.tipo == q::RPC {
        let codigo = match q::aviso_rpc(&buf[..marco.bytes]) {
            Ok(c) => c, Err(_) => return fallar(IOMMU_NO_615_EVENTO, q::RPC as u64),
        };
        if EVENTOS.fetch_add(1, Ordering::AcqRel) >= 256 { return fallar(IOMMU_NO_615_EVENTO, codigo as u64); }
        crate::ring0::cabina::info("gpu", "F2b 615: RPC informativa encapsulada", codigo as u64);
        core::sync::atomic::fence(Ordering::SeqCst);
        r.escribir(q::RX_TAIL, tail.wrapping_add(marco.paginas));
        return Ok(info());
    }
    let m = match q::Gmc::leer(&buf[..marco.bytes]) { Ok(v) => v, Err(_) => return fallar(IOMMU_NO_615_FORMA, marco.tipo as u64) };
    crate::ring0::cabina::info("gpu", "F2b 615: GMCAPI recibido, comando", m.comando as u64);
    if !m.respuesta {
        if m.comando != q::HS || HS_EVENTOS.load(Ordering::Acquire) >= 32 {
            return fallar(IOMMU_NO_615_EVENTO, m.comando as u64);
        }
        let mib = gpu::info_fb() as u32;
        if !bmo_gpu_ga10x::identidad::vram_es_la_suya(mib) { return fallar(IOMMU_NO_615_LECTURA, mib as u64); }
        let vram = (mib as u64) << 20;
        // Limite conservador de TCM del perfil. Una peticion mayor se rechaza
        // antes de reset/DMA; no se cree una capacidad enviada en el mensaje.
        let ordenes = unsafe { &mut *core::ptr::addr_of_mut!(ORDENES_HS) };
        let n = match hs615::ordenes(m.datos, vram, 65536, 65536, ordenes) {
            Ok(n) => n, Err(_) => return fallar(IOMMU_NO_615_EVENTO, m.comando as u64),
        };
        if bmo_gpu_ga10x::correr::validar(&ordenes[..n]).is_err() { return fallar(IOMMU_NO_615_EVENTO, n as u64); }
        let hs = super::gpu_hs::Tramo::runtime(m.datos);
        hs.foto(&mut r, "HS foto externa previa al corredor 615; id");
        unsafe { *core::ptr::addr_of_mut!(CORREDOR_HS) = Corredor::default(); }
        HS_N.store(n as u64, Ordering::Release);
        HS_EVENTOS.fetch_add(1, Ordering::AcqRel);
        // Devuelve los huecos despues de copiar/validar todo y antes de la
        // reanudacion. La orden es FIRE_AND_FORGET en la tabla GMCAPI 615.
        core::sync::atomic::fence(Ordering::SeqCst);
        r.escribir(q::RX_TAIL, tail.wrapping_add(marco.paginas));
        return tramo_hs(&mut r);
    }
    let cfg = match m.respuesta_init(SECUENCIA) { Ok(c) => c, Err(_) => return fallar(IOMMU_NO_615_RESPUESTA, m.estado_o_maximo as u64) };
    core::sync::atomic::fence(Ordering::SeqCst);
    r.escribir(q::RX_TAIL, tail.wrapping_add(marco.paginas));
    ESTADO.store(HECHO | 1, Ordering::Release);
    // SAFETY: _turno mantiene OCUPADO; no crea nuevas fotos de espera.
    let us = unsafe { (&*core::ptr::addr_of!(SEGUIMIENTO)).transcurrido(ahora_us()) };
    foto_cola(&mut r, f, 2, us);
    crate::ring0::cabina::info("gpu", "F2b 615: GSP_INIT respondido, estado cero; claves NVKV", cfg.claves as u64);
    crate::ring0::cabina::info("gpu", "F2b 615: asas internas cliente/dispositivo", cfg.cliente as u64 | (cfg.dispositivo as u64) << 32);
    if super::gpu_despertar::devolver_bar1().is_err() { return fallar(IOMMU_NO_615_RESPUESTA, 0); }
    Ok(info())
}
