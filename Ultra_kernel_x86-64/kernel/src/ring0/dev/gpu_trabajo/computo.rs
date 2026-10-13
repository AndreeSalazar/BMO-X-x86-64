//! **LI2f: LA PUERTA DEL COMPUTO DE LAS APPS** (13-10) -- la TERCERA puerta
//! estrecha: una app (sin la autoridad MAQUINA) corre un `Dispatch` de
//! D3D12 en la 3060 con una receta de computo (VRC1,
//! `bmo_gpu_ga10x::receta_computo`). Lo que la hace segura, en orden:
//!
//! ```text
//!    la receta   es de QUIEN la manda (un bloque suyo o paginas hechas de su
//!                reserva) y se COPIA aqui antes de mirarla: lo que se juzga
//!                es lo que se sube (la app no la puede cambiar entre medias)
//!    el cuerpo   el JUEZ, R7 (`juzgar_cuerpo_con` con los permisos de la
//!                receta: cada LDG/STG/ATOMG sujeto a la base y el tope de SU
//!                bufer; un SRV, solo leer; la compartida, a sus bytes) y,
//!                ya pegado, R0..R6
//!    los datos   las precargas las pone el KERNEL (un MOV cada una): la VA de
//!                cada bufer en la 3060 la elige el kernel, no la app
//!    los buferes un bloque suyo o su reserva: un UAV ESCRIBIBLE en una
//!                ranura del destino, un SRV en una de las texturas, prestado
//!                SOLO LEER; todo DEVUELTO antes de volver, salga como salga
//!    el GR       con el cerrojo de todos (`gr_ocupado`), y el vigilante (E7)
//!                corta lo que no vuelve en su plazo
//! ```
//!
//! Cada NO apunta su porque (`porque::apuntar`: el lente) y devuelve el
//! `motivo 79` de siempre.
//!
//! [carril]  ROJO      la 3060 lee y escribe memoria de un proceso
//! [consumo] NADA      corre por orden (un `Dispatch` del juego); ~0,9 MiB
//!                fijos: la copia de la receta y el pegado
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL: lo dira el primer `Dispatch`.

use core::sync::atomic::{AtomicBool, Ordering};

use bmo_gpu_ga10x::computo_app as ca;
use bmo_gpu_ga10x::porque::Porque;
use bmo_gpu_ga10x::receta_computo as rc;

use super::porque::{apuntar, no};
use super::{esperando, gr_ocupado, BLUR_ENTRADA, BLUR_EN_MARCHA, FRACTAL_ESPERA_US, LIENZO_HECHO, PAGINA};
use crate::ring0::dev::gpu_prestamo::Bar0;

/// Uno en marcha (guarda la copia y el pegado).
static EN_USO: AtomicBool = AtomicBool::new(false);

struct Fijo<T>(core::cell::UnsafeCell<T>);
// SAFETY: solo se tocan con `EN_USO` tomado (ver `computo_de_app`).
unsafe impl<T> Sync for Fijo<T> {}

/// La copia de la receta.
static COPIA: Fijo<[u8; rc::MAX_RECETA]> = Fijo(core::cell::UnsafeCell::new([0; rc::MAX_RECETA]));
/// El pegado: los MOV de las precargas y el cuerpo, juntos.
static PEGADO: Fijo<[(u64, u64); rc::MAX_PEGADO]> = Fijo(core::cell::UnsafeCell::new([(0, 0); rc::MAX_PEGADO]));
/// Los valores de las precargas.
static CARGAS: Fijo<[(u8, u32); 4 * rc::MAX_PRECARGAS]> = Fijo(core::cell::UnsafeCell::new([(0, 0); 4 * rc::MAX_PRECARGAS]));

/// Lo prestado de cada bufer: `(iova, paginas)`.
type Prestados = [(u64, u64); rc::MAX_BUFERES];

/// **LA PUERTA**: `va` = la receta de computo de quien llama.
/// `Ok(computo_app::empaquetar(us, lanzado, pagado))`.
pub fn computo_de_app(va: u64) -> Result<u64, u32> {
    let bar0 = crate::ring0::dev::gpu::bar0();
    let Some(ficha) = super::ficha_del_gr() else {
        crate::ring0::cabina::warn("gpu", "LI2f: una app quiere computar y el escritorio no preparo la 3060 (`gpu lienzo`)", va);
        return Err(no(Porque::SinEscritorio, 0, 0));
    };
    if bar0 == 0 || crate::ring0::dev::gpu_apagar::despedido() || !crate::ring0::dev::gpu_despertar::bar1_fisica() || !LIENZO_HECHO.load(Ordering::Acquire) {
        return Err(no(Porque::SinEscritorio, 1, 0));
    }
    if EN_USO.swap(true, Ordering::AcqRel) {
        return Err(no(Porque::TallerOcupado, 1, 0));
    }
    // SAFETY: `EN_USO` tomado arriba: nadie mas toca los fijos hasta soltarlo.
    let (copia, pegado, cargas) = unsafe { (&mut *COPIA.0.get(), &mut *PEGADO.0.get(), &mut *CARGAS.0.get()) };
    let r = despachar(bar0, ficha, va, copia, pegado, cargas);
    EN_USO.store(false, Ordering::Release);
    r
}

/// **Copiar la receta de `pid` en `va`** a `copia`: de un bloque suyo (por el
/// physmap) o de su reserva (pagina a pagina). `Ok(bytes)`.
fn copiar_receta(pid: u32, va: u64, copia: &mut [u8]) -> Result<usize, u32> {
    use crate::ring0::obj::reserva;
    let dentro = |fisica: u64, bytes: u64| fisica.checked_add(bytes).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE);
    let cabecera = rc::CABECERA as u64;
    let aspace = crate::ring0::mm::vmm::read_cr3();
    let en_bloque = crate::ring0::obj::memory::fisica_de(pid, va, cabecera).filter(|&f| dentro(f, cabecera)).is_some();
    let leer = |hasta: usize, copia: &mut [u8]| -> bool {
        if en_bloque {
            match crate::ring0::obj::memory::fisica_de(pid, va, hasta as u64).filter(|&f| dentro(f, hasta as u64)) {
                Some(f) => {
                    // SAFETY: `fisica_de` dio `[va, va + hasta)` en UN bloque del
                    // proceso, dentro del physmap (comprobado); solo se lee.
                    let src = unsafe { core::slice::from_raw_parts(crate::ring0::mm::phys_to_virt(f) as *const u8, hasta) };
                    copia[..hasta].copy_from_slice(src);
                    true
                }
                None => false,
            }
        } else {
            reserva::en_la_ventana(va, hasta as u64) && super::cubo::copiar_de_la_reserva(aspace, va, &mut copia[..hasta])
        }
    };
    if !leer(rc::CABECERA, copia) {
        crate::ring0::cabina::warn("gpu", "LI2f: la receta de computo no es un bloque de quien la manda ni de su reserva", va);
        return Err(no(Porque::ComputoReceta, 1, va));
    }
    let total = u32::from_le_bytes([copia[4], copia[5], copia[6], copia[7]]) as usize;
    if total < rc::CABECERA || total > copia.len() {
        crate::ring0::cabina::warn("gpu", "LI2f: la receta de computo pasa de lo que cabe en la copia del kernel; bytes", total as u64);
        return Err(no(Porque::ComputoReceta, 8, total as u64));
    }
    if !leer(total, copia) {
        crate::ring0::cabina::warn("gpu", "LI2f: la receta de computo entera no es de quien la manda; bytes", total as u64);
        return Err(no(Porque::ComputoReceta, 1, total as u64));
    }
    Ok(total)
}

fn despachar(bar0: u64, ficha: u32, va: u64, copia: &mut [u8], pegado: &mut [(u64, u64)], cargas: &mut [(u8, u32)]) -> Result<u64, u32> {
    use bmo_gpu_ga10x::sass::juez;
    let pid = crate::ring0::task::scheduler::current_pid();
    let total = copiar_receta(pid, va, copia)?;
    let r = match rc::leer(&copia[..total]) {
        Ok(r) => r,
        Err(e) => {
            let (sub, detalle) = rc::sub_porque(e);
            crate::ring0::cabina::warn("gpu", e.texto(), detalle as u64);
            return Err(no(Porque::ComputoReceta, sub, detalle as u64));
        }
    };
    // Donde vera la 3060 cada bufer (lo elige el kernel), y los valores de
    // las precargas con esas VA.
    let es_uav = |k: usize| !r.bufer(k).solo_lee();
    let va_3060 = |k: usize| ca::ranura_de(es_uav, k).map_or(0, |(j, uav)| ca::va_iova_de(j, uav).0 + r.bufer(k).va % PAGINA);
    if let Some(k) = (0..r.n_buferes()).find(|&k| ca::ranura_de(es_uav, k).is_none()) {
        crate::ring0::cabina::warn("gpu", "LI2f: mas de 4 UAV (o de 4 SRV) en un computo: no hay ranura; bufer", k as u64);
        return Err(no(Porque::BuferNoSePresta, k as u8, r.bufer(k).va));
    }
    let Some(m) = rc::valores(&r, va_3060, cargas) else {
        return Err(no(Porque::ComputoReceta, 5, 0));
    };
    // El cuerpo, copiado detras de donde iran los MOV; R7 sobre el.
    let n = r.instrucciones();
    if m + n > pegado.len() {
        return Err(no(Porque::ComputoReceta, 4, (m + n) as u64));
    }
    for k in 0..n {
        pegado[m + k] = r.instruccion(k);
    }
    let Some(permisos) = rc::permisos(&r) else {
        return Err(no(Porque::ComputoReceta, 6, 0));
    };
    if let Err(b) = juez::juzgar_cuerpo_con(&pegado[m..m + n], r.despacho.registros, permisos) {
        apuntar(Porque::Juez, 3, b.instruccion as u64);
        crate::ring0::cabina::warn("gpu", b.regla.nombre(), b.que as u64);
        crate::ring0::cabina::warn("gpu", "[BMO-X Juez V3b]: TOMA TU BODRIO! el computo de una app no sube a la 3060; instruccion", b.instruccion as u64);
        return Err(super::porque::motivo());
    }
    // Pegado: los MOV delante, y R0..R6 sobre el programa entero.
    ca::poner_cargas(&mut pegado[..m], &cargas[..m]);
    let programa = &pegado[..m + n];
    let ctx = juez::Contexto { registros: r.despacho.registros + juez::RESERVADOS, sph: None };
    if let Err(b) = juez::juzgar_drenado(programa, &ctx) {
        apuntar(Porque::Juez, 4, b.instruccion as u64);
        crate::ring0::cabina::warn("gpu", b.regla.nombre(), b.que as u64);
        crate::ring0::cabina::warn("gpu", "[BMO-X Juez V3b]: TOMA TU BODRIO! el computo pegado no sube a la 3060; instruccion", b.instruccion as u64);
        return Err(super::porque::motivo());
    }
    // El GR, con el cerrojo de todos; desde aqui se suelta en TODA salida.
    if gr_ocupado() {
        return Err(no(if super::gr_muerto().is_some() { Porque::CanalMuerto } else { Porque::GrOcupado }, 0, 0));
    }
    let hecho = con_el_gr(bar0, ficha, pid, &r, programa);
    BLUR_EN_MARCHA.store(false, Ordering::Release);
    hecho
}

/// Con el GR tomado: prestar, preparar, lanzar, esperar, devolver.
fn con_el_gr(bar0: u64, ficha: u32, pid: u32, r: &rc::Receta, programa: &[(u64, u64)]) -> Result<u64, u32> {
    let usa_uav = (0..r.n_buferes()).any(|k| !r.bufer(k).solo_lee());
    let usa_srv = (0..r.n_buferes()).any(|k| r.bufer(k).solo_lee());
    if usa_uav {
        super::cubo::destino_mapeado(bar0)?;
    }
    if usa_srv {
        super::cubo::texturas_mapeadas(bar0)?;
    }
    let mut prestados: Prestados = [(0, 0); rc::MAX_BUFERES];
    let prestado = prestar_buferes(pid, r, &mut prestados);
    let hecho = prestado.and_then(|()| lanzar_y_esperar(bar0, ficha, r, programa));
    // Devuelto SIEMPRE, salga como salga.
    devolver(&prestados);
    hecho
}

/// **Prestar cada bufer** a la 3060 en su ranura: de un bloque suyo (un UAV,
/// escribible) o de paginas hechas de su reserva.
fn prestar_buferes(pid: u32, r: &rc::Receta, prestados: &mut Prestados) -> Result<(), u32> {
    use crate::ring0::obj::reserva;
    let aspace = crate::ring0::mm::vmm::read_cr3();
    let es_uav = |k: usize| !r.bufer(k).solo_lee();
    for k in 0..r.n_buferes() {
        let b = r.bufer(k);
        let Some((j, uav)) = ca::ranura_de(es_uav, k) else { return Err(no(Porque::BuferNoSePresta, k as u8, b.va)) };
        let iova = ca::va_iova_de(j, uav).1;
        let base = b.va & !(PAGINA - 1);
        let paginas = (b.va % PAGINA + b.bytes as u64).div_ceil(PAGINA);
        if paginas * PAGINA > ca::RANURA_BYTES {
            crate::ring0::cabina::warn("gpu", "LI2f: un bufer del computo pasa de los 16 MiB de su ranura; bytes", b.bytes as u64);
            return Err(no(Porque::BuferNoSePresta, k as u8, b.va));
        }
        let fisica = if uav { crate::ring0::obj::memory::fisica_para_escribir(pid, b.va, b.bytes as u64) } else { crate::ring0::obj::memory::fisica_de(pid, b.va, b.bytes as u64) };
        let fisica = fisica.map(|f| f - b.va % PAGINA).filter(|&f| f % PAGINA == 0 && f.checked_add(paginas * PAGINA).is_some_and(|fin| fin <= crate::ring0::mm::PHYSMAP_SIZE));
        let marco = |i: u64| reserva::marco(aspace, base + i * PAGINA, uav);
        let hecho = match fisica {
            Some(f) => crate::ring0::plat::iommu::prestar_gpu(iova, f, paginas, uav),
            None if reserva::en_la_ventana(b.va, b.bytes as u64) && (0..paginas).all(|i| marco(i).is_some()) => crate::ring0::plat::iommu::prestar_gpu_marcos(iova, paginas, uav, marco),
            None => {
                crate::ring0::cabina::warn("gpu", "LI2f: un bufer del computo no es de quien lo manda (o un UAV no es escribible); VA", b.va);
                return Err(no(Porque::BuferNoEsSuyo, k as u8, b.va));
            }
        };
        if let Err(m) = hecho {
            crate::ring0::cabina::warn("gpu", "LI2f: un bufer del computo no se pudo prestar a la 3060; motivo de la IOMMU", m as u64);
            return Err(no(Porque::BuferNoSePresta, k as u8, b.va));
        }
        prestados[k] = (iova, paginas);
    }
    Ok(())
}

/// Devolver lo que [`prestar_buferes`] presto.
fn devolver(prestados: &Prestados) {
    for &(iova, paginas) in prestados {
        if paginas != 0 && crate::ring0::plat::iommu::devolver_gpu(iova, paginas).is_err() {
            crate::ring0::cabina::warn("gpu", "LI2f: un bufer del computo NO se pudo devolver (la invalidacion no contesto); paginas", paginas);
        }
    }
}

/// Preparar en el tramo, tocar el timbre y esperar al semaforo (o al plazo:
/// el vigilante corta).
fn lanzar_y_esperar(bar0: u64, ficha: u32, r: &rc::Receta, programa: &[(u64, u64)]) -> Result<u64, u32> {
    let mut g = Bar0(bar0);
    let en = BLUR_ENTRADA.load(Ordering::Acquire);
    if !bmo_gpu_ga10x::blur::entrada_valida(en) || !ca::preparar(&mut g, en, programa, &r.despacho) {
        crate::ring0::cabina::warn("gpu", "LI2f: el despacho no quedo preparado en el tramo; no se toca el timbre", en as u64);
        return Err(no(Porque::ComputoPreparar, 0, en as u64));
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    let hz = (crate::ring0::task::scheduler::tsc_freq() / 1_000_000).max(1);
    let desde = crate::ring0::task::scheduler::rdtsc();
    let lanzado = ca::lanzar(&mut g, ficha, en);
    if lanzado {
        BLUR_ENTRADA.store(bmo_gpu_ga10x::blur::siguiente(en), Ordering::Release);
    }
    let (mut qmd, mut fin, mut us) = (0, 0, 0);
    while lanzado && us < FRACTAL_ESPERA_US {
        (_, qmd, fin) = ca::mirar(&mut g);
        us = (crate::ring0::task::scheduler::rdtsc() - desde) / hz;
        if qmd == ca::PAGA_QMD && fin == ca::PAGA_FIN {
            break;
        }
        esperando(us);
    }
    let pagado = qmd == ca::PAGA_QMD && fin == ca::PAGA_FIN;
    super::vigilante::vigilar("LI2f: un COMPUTO de una app no volvio en su plazo; us", lanzado, pagado, us, FRACTAL_ESPERA_US);
    core::sync::atomic::fence(Ordering::SeqCst);
    if !pagado {
        return Err(no(Porque::ComputoSinPagar, lanzado as u8, us));
    }
    Ok(ca::empaquetar(us as u32, lanzado, pagado))
}
