//! **`gpu revivir`: E7b, EL CANAL DE GR OTRA VEZ, SIN REINICIAR** (13-10).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: tres RPC de
//!                     hasta 5 s cada una y los pasos de `canalgr` a `lienzo`
//!
//! Tras un Xid el GSP-RM deja el canal de GR MUERTO. Hasta hoy, el boton de
//! reinicio. Ahora, como un driver de verdad: el `FREE` de AMPERE_B, de
//! AMPERE_COMPUTE_B y del canal (en ese orden, cada uno esperando su
//! respuesta), el kernel lo olvida (`IOMMU_OP_GPU_REVIVIR_GR`), el escritorio
//! olvida sus pasos, y `canalgr` .. `lienzo` se dan otra vez.
//!
//! [!] NADA DE ESTO HA CORRIDO EN EL METAL: lo dira el primer `gpu revivir`.

use bmo_gpu_ga10x::liberar::{self, Liberable};
use bmo_gpu_ga10x::objeto;
use bmo_userland as bmo;

use super::gsprpc::{esperar, Otros};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

const NOMBRES: [&[u8]; 3] = [b"AMPERE_B (el 3D)", b"AMPERE_COMPUTE_B (el computo)", b"el canal de GR0"];

/// El `FREE` del `k`-esimo y su respuesta: `(estado del RM, us, numero)`.
fn liberar_uno(k: u64) -> Result<(u32, u64, u32), u32> {
    let numero = (bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIBERAR_GR, k)? >> 32) as u32;
    let mut d = [0u8; liberar::DATOS];
    let (m, us) = esperar(liberar::FREE, &mut d, &mut Otros::default())?;
    // `NVOS00_PARAMETERS`: el estado en +12; si el RPC entero fallo, su
    // `rpc_result`.
    let estado = u32::from_le_bytes([d[12], d[13], d[14], d[15]]);
    Ok((if m.resultado != 0 { m.resultado } else { estado }, us, numero))
}

/// **`gpu revivir`.**
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "reviviendo el canal de GR: FREE de sus tres objetos", INK_DIM);
    let mut canal_libre = false;
    for (k, l) in Liberable::TODOS.iter().enumerate() {
        let r = liberar_uno(k as u64);
        let g = &mut dsk.out.grid;
        g.text(b"  FREE ");
        g.text(NOMBRES[k]);
        g.text(b": ");
        match r {
            Ok((estado, us, numero)) => {
                g.with_ink(if estado == 0 { INK_GOOD } else { INK_ERR });
                g.text(objeto::estado(estado));
                if estado != 0 {
                    g.text(b" (0x");
                    g.hex(estado as u64, 8);
                    g.byte(b')');
                }
                g.with_ink(INK_ECHO);
                g.text(b"   en ");
                g.dec(us / 1000);
                g.text(b" ms (numero ");
                g.dec(numero as u64);
                g.text(b")\n");
                g.with_ink(INK_PLAIN);
                if *l == Liberable::CanalGr && estado == 0 {
                    canal_libre = true;
                }
            }
            Err(m) => {
                g.with_ink(INK_ERR);
                g.text(b"NO: ");
                g.text(super::iommu::motivo(m));
                g.text(b"\n");
                g.with_ink(INK_PLAIN);
                dsk.field.n = 0;
                return After::Settle;
            }
        }
    }
    let g = &mut dsk.out.grid;
    if !canal_libre {
        g.with_ink(INK_ERR);
        g.text(b"  el GSP-RM no libero el canal de GR0: no se revive (mira `gpu aviso`); queda el reinicio\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    if let Err(m) = bmo::iommu_orden(bmo::IOMMU_OP_GPU_REVIVIR_GR) {
        g.with_ink(INK_ERR);
        g.text(b"  NO al revivir: ");
        g.text(super::iommu::motivo(m));
        g.text(b"\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    super::gspcanalgr::olvidar();
    super::gspgr::olvidar_oro();
    super::gspcomputo::olvidar();
    paint_status(p, &dsk.run_box, "reviviendo el canal de GR: canalgr .. lienzo", INK_DIM);
    match super::verificar::preparar_hasta(dsk, p, b"lienzo") {
        Ok(_) => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_GOOD);
            g.text(b"  EL CANAL DE GR, VIVO OTRA VEZ, SIN REINICIAR: pedido, encendido, promovido, con su 3D y su computo, y el lienzo pintado (E7b)\n");
            g.with_ink(INK_PLAIN);
        }
        Err(_) => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            g.text(b"  liberado y olvidado, pero un paso no salio (arriba): queda el reinicio\n");
            g.with_ink(INK_PLAIN);
        }
    }
    dsk.field.n = 0;
    After::Settle
}
