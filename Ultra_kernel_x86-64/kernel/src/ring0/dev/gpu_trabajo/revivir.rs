//! **E7b: REVIVIR EL CANAL DE GR SIN REINICIAR** (13-10).
//!
//! capa: kernel -- las dos ordenes; el `FREE` lo arma `bmo_gpu_ga10x::liberar`
//! y lo juzga el contrato
//!
//! [carril]  ROJO      libera objetos del GSP-RM y vuelve a poner a cero
//!                     memoria de la 3060
//! [consumo] NADA      corre por orden (`gpu revivir` en el escritorio)
//!
//! # Por que existe
//!
//! Tras un Xid el canal de GR quedaba MUERTO hasta reiniciar, y el
//! propietario dependia del boton (13-10: *"tengo que depender del
//! reinicio"*). Un driver de verdad libera el canal y lo pide otra vez; el
//! resto de la tarjeta sigue viva. Aqui, en dos ordenes que el escritorio da
//! en este orden y esperando la respuesta del GSP-RM a cada `FREE`:
//!
//! ```text
//!    LIBERAR_GR 0, 1, 2   el FREE de AMPERE_B, AMPERE_COMPUTE_B y el canal
//!                         (cada uno solo con los anteriores ya pedidos, y
//!                         solo si el canal esta MUERTO)
//!    REVIVIR_GR           el kernel lo olvida todo del canal: lo pedido, lo
//!                         promovido, el 3D, el computo, la ficha, la primera
//!                         entrada del GPFIFO, lo caliente, el anillo y el
//!                         parte; su USERD y los buferes que llena el RM, a
//!                         cero. Despues `gpu verrano` lo levanta como en el
//!                         arranque, desde `canalgr`.
//! ```
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL: lo dira el primer canal revivido.

use core::sync::atomic::{AtomicU64, Ordering};

use bmo_gpu_ga10x::liberar::Liberable;

/// E7b: el canal no esta muerto, falta un `FREE` antes, el GR esta en uso, o
/// el RC viejo sigue en la cola del GSP.
pub const IOMMU_NO_REVIVIR: u32 = 93;

/// Los `FREE` ya pedidos (bit k = `Liberable::TODOS[k]`).
static LIBERADOS: AtomicU64 = AtomicU64::new(0);
/// Cuantas veces se revivio en este arranque.
static REVIVIDOS: AtomicU64 = AtomicU64::new(0);

/// **`IOMMU_OP_GPU_LIBERAR_GR`**: el `FREE` del `k`-esimo (0 el 3D, 1 el
/// computo, 2 el canal). `Ok` como cualquier RPC: la respuesta la espera el
/// escritorio.
pub fn liberar(k: u64) -> Result<u64, u32> {
    let Some(que) = Liberable::de(k) else {
        return Err(IOMMU_NO_REVIVIR);
    };
    if super::gr_muerto().is_none() {
        crate::ring0::cabina::warn("gpu", "E7b: el canal de GR esta VIVO: no se libera (solo uno muerto)", k);
        return Err(IOMMU_NO_REVIVIR);
    }
    let antes = (1u64 << k) - 1;
    if LIBERADOS.load(Ordering::Acquire) & antes != antes {
        crate::ring0::cabina::warn("gpu", "E7b: un FREE antes de los anteriores (el 3D, el computo, el canal: en ese orden); pedido", k);
        return Err(IOMMU_NO_REVIVIR);
    }
    let r = super::super::gpu_libos::liberar_gr(que)?;
    LIBERADOS.fetch_or(1 << k, Ordering::AcqRel);
    crate::ring0::cabina::count("gpu", "E7b: FREE pedido al GSP-RM (0 el 3D, 1 el computo, 2 el canal)", k);
    Ok(r)
}

/// **`IOMMU_OP_GPU_REVIVIR_GR`**: con los tres `FREE` pedidos, el kernel
/// olvida el canal. `Ok(paginas puestas a cero | veces << 32)`.
pub fn revivir() -> Result<u64, u32> {
    use core::sync::atomic::Ordering as O;
    if super::gr_muerto().is_none() || LIBERADOS.load(O::Acquire) != 0b111 {
        crate::ring0::cabina::warn("gpu", "E7b: revivir sin el canal muerto o sin sus tres FREE; liberados", LIBERADOS.load(O::Acquire));
        return Err(IOMMU_NO_REVIVIR);
    }
    // El RC viejo, aun en la cola, lo volveria a dar por muerto al instante.
    if super::super::gpu_libos::rc_del_canal(bmo_gpu_ga10x::canal::GR.chid).is_some() {
        crate::ring0::cabina::warn("gpu", "E7b: el RC_TRIGGERED viejo sigue en la cola del GSP: el escritorio la lee esperando los FREE; no se revive", 0);
        return Err(IOMMU_NO_REVIVIR);
    }
    // Nadie dibujando: el cerrojo del GR, tomado mientras se olvida.
    if super::BLUR_EN_MARCHA.swap(true, O::AcqRel) {
        return Err(IOMMU_NO_REVIVIR);
    }
    let ceros = super::super::gpu_libos::olvidar_gr();
    super::olvidar_trabajos();
    super::cubo::olvidar();
    super::parte::olvidar();
    LIBERADOS.store(0, O::Release);
    super::revivido();
    super::BLUR_EN_MARCHA.store(false, O::Release);
    let n = REVIVIDOS.fetch_add(1, O::AcqRel) + 1;
    crate::ring0::cabina::count("gpu", "E7b: el canal de GR, OLVIDADO y listo para pedirse otra vez (`gpu verrano` desde canalgr); veces en este arranque", n);
    Ok(ceros | n << 32)
}
