//! **CARRIL VERDE** -- LA CUENTA del toque (EL0 de `PLAN_EL_TOQUE.md`).
//!
//! [carril]  VERDE     solo LEE tablas de paginas: no cambia un bit ni un mapeo
//! [consumo] NADA      corre al morir un programa (`enterrador`) y cuando el
//!                     informe pregunta por el que pregunta
//!
//! [cuesta]  NADA -- un numero mal contado sale mal en el informe y nada mas;
//!           nadie decide nada con el todavia.
//!
//! [riesgo]  -- ninguno declarado, con UNA condicion que se cumple fuera: solo
//!           se camina un espacio que NO puede desmontarse a la vez. Son dos:
//!           el de un muerto, justo ANTES de que el enterrador lo desmonte
//!           (nadie mas lo toca), y el del que pregunta, que esta corriendo.
//!           El de un tercero vivo NO se camina: podria morirse a mitad.
//!
//! # Que se cuenta
//!
//! Las hojas de 4 KiB de usuario, presentes, por region: la IMAGEN (el `.bex`
//! cargado y su pila, `USER_IMAGE_BASE..USER_STACK_TOP`), la MEMORIA pedida
//! (`MEMORIA_VA_BASE` en adelante: el monton y las laminas) y lo DEMAS (el
//! canal). La pantalla (`FRAMEBUFFER_VA_BASE..`) no se cuenta: no es RAM.
//! De cada una: cuantas estan, cuantas tienen el bit A (la CPU las LEYO o
//! escribio alguna vez desde que se mapearon) y cuantas el D (las ESCRIBIO).
//! Las paginas grandes (2 MiB y 1 GiB) se saltan: el usuario no las tiene.

use crate::ring0::mm::vmm::{FRAMEBUFFER_VA_BASE, MEMORIA_VA_BASE, PTE_HUGE, PTE_PRESENT, PTE_USER, USER_IMAGE_BASE, USER_STACK_TOP};
use crate::ring0::mm::{phys_to_virt, PHYSMAP_SIZE};

/// El bit A (accedida) y el D (sucia): los pone la CPU sola (manual de AMD,
/// tomo 2, 5.4.1).
const PTE_A: u64 = 1 << 5;
const PTE_D: u64 = 1 << 6;
const ADDR: u64 = 0x000F_FFFF_FFFF_F000;

/// Las tres regiones, por su indice en [`Cuenta`].
pub const IMAGEN: usize = 0;
pub const MEMORIA: usize = 1;
pub const DEMAS: usize = 2;

/// Paginas de 4 KiB, por region.
#[derive(Clone, Copy, Default)]
pub struct Cuenta {
    pub estan: [u32; 3],
    pub tocadas: [u32; 3],
    pub escritas: [u32; 3],
}

/// La tabla en `phys`, para leer; `None` si no cae en el espejo.
fn tabla(phys: u64) -> Option<&'static [u64; 512]> {
    if phys == 0 || phys >= PHYSMAP_SIZE || phys & 0xFFF != 0 {
        return None;
    }
    // SAFETY: dentro del physmap y alineada; solo se LEE, y el espacio no se
    // desmonta mientras (la condicion de la cabecera).
    Some(unsafe { &*(phys_to_virt(phys) as *const [u64; 512]) })
}

fn region(va: u64) -> Option<usize> {
    if (USER_IMAGE_BASE..USER_STACK_TOP).contains(&va) {
        Some(IMAGEN)
    } else if va >= MEMORIA_VA_BASE {
        Some(MEMORIA)
    } else if (FRAMEBUFFER_VA_BASE..MEMORIA_VA_BASE).contains(&va) {
        None
    } else {
        Some(DEMAS)
    }
}

/// **Cuenta el espacio `pml4`**: sus hojas de usuario del primer TiB (la
/// entrada 0 del PML4, donde vive todo lo de Ring 3).
pub fn contar(pml4: u64) -> Cuenta {
    let mut c = Cuenta::default();
    let Some(p4) = tabla(pml4 & ADDR) else { return c };
    let e4 = p4[0];
    if e4 & PTE_PRESENT == 0 {
        return c;
    }
    let Some(p3) = tabla(e4 & ADDR) else { return c };
    for (i3, &e3) in p3.iter().enumerate() {
        if e3 & PTE_PRESENT == 0 || e3 & PTE_HUGE != 0 || e3 & PTE_USER == 0 {
            continue;
        }
        let Some(p2) = tabla(e3 & ADDR) else { continue };
        for (i2, &e2) in p2.iter().enumerate() {
            if e2 & PTE_PRESENT == 0 || e2 & PTE_HUGE != 0 || e2 & PTE_USER == 0 {
                continue;
            }
            let Some(p1) = tabla(e2 & ADDR) else { continue };
            for (i1, &e1) in p1.iter().enumerate() {
                if e1 & PTE_PRESENT == 0 || e1 & PTE_USER == 0 {
                    continue;
                }
                let va = (i3 as u64) << 30 | (i2 as u64) << 21 | (i1 as u64) << 12;
                let Some(r) = region(va) else { continue };
                c.estan[r] += 1;
                if e1 & PTE_A != 0 {
                    c.tocadas[r] += 1;
                }
                if e1 & PTE_D != 0 {
                    c.escritas[r] += 1;
                }
            }
        }
    }
    c
}

// -- Lo que tocaron los que ya murieron --------------------------------------

/// Los ultimos ocho muertos, por tid (las fichas del registro son ocho).
const GUARDADOS: usize = 8;
static mut MUERTOS: [(u32, Cuenta); GUARDADOS] = [(0, Cuenta { estan: [0; 3], tocadas: [0; 3], escritas: [0; 3] }); GUARDADOS];
static mut SIGUIENTE: usize = 0;

/// **Al morir**, justo antes de desmontar su espacio: lo que toco en toda su
/// vida. Lo llama el enterrador, que es el unico que camina ese espacio.
pub fn al_morir(tid: u32, pml4: u64) {
    let c = contar(pml4);
    // SAFETY: solo el enterrador escribe (uno a la vez); un lector a medias
    // lee una cuenta vieja o nueva, nunca memoria de nadie.
    unsafe {
        let i = SIGUIENTE % GUARDADOS;
        MUERTOS[i] = (tid, c);
        SIGUIENTE = i + 1;
    }
}

/// Lo que toco el muerto `tid`, si se apunto.
pub fn de_muerto(tid: u32) -> Option<Cuenta> {
    if tid == 0 {
        return None;
    }
    // SAFETY: lectura de una tabla de copias (ver `al_morir`).
    unsafe { (*core::ptr::addr_of!(MUERTOS)).iter().find(|m| m.0 == tid).map(|m| m.1) }
}

/// `INFO_PROG_TOQUE` de una region: `estan | tocadas << 21 | escritas << 42`
/// y el bit 63 si hay cuenta (`1 << 62` ademas si es del que pregunta, VIVO).
pub fn empaquetar(c: &Cuenta, r: usize, vivo: bool) -> u64 {
    let m = 0x1F_FFFF;
    (c.estan[r] as u64 & m) | (c.tocadas[r] as u64 & m) << 21 | (c.escritas[r] as u64 & m) << 42 | 1 << 63 | if vivo { 1 << 62 } else { 0 }
}
