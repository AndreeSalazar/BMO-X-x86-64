//! **LA LENTE AL ADMITIR** (B1 de `docs/plan/EL_FOCO.md`: LB7 + LB8 de
//! `docs/plan/PLAN_LA_LENTE_BINARIA.md`). Verificar una vez, y apartarse.
//!
//! [carril]  AMARILLO  lee las paginas de codigo que el cargador acaba de mapear
//! [consumo] UNA VEZ   por `.bex` (por el hash de su codigo); luego, nada
//!
//! > 14-10, el propietario: *"que hable con mi Kernel que primero verifica con
//! > respeto, luego se aparta y le habla a mi CPU (AMD) en binarios precisos"*.
//!
//! Al admitir, la lente (`bmo_lente_x86::recorre`) lee el codigo como lo
//! correra la CPU: desde la ENTRADA, por los saltos y las llamadas, y desde
//! cada sitio del codigo cuya direccion guarda el programa (las relocaciones
//! hacia la region de codigo). Y juzga:
//!
//! ```text
//!    una instruccion de una extension que el .bex NO declara
//!      y que sin ella da `#UD`                                 NO, con su sitio
//!      y que sin ella se ejecuta como otra (tzcnt -> bsf)      entra, y se cuenta
//!    una que no sabe leer, o un salto que sale del codigo      se dice: NO DEMOSTRADO
//!    todo lo alcanzable leido, declarado y dentro              DEMOSTRADO
//! ```
//!
//! ** Lo "no demostrado" ENTRA. La lente lee lo que emiten los compiladores de
//! BMO-X (el `metro` lo juzga cada build, y DOOM entero se recorre limpio el
//! 14-10), pero un `.bex` de fuera puede llevar lo que su tabla no tiene: eso
//! es un limite de la lente, no una falta del programa, y dejar de arrancar
//! por el seria castigar al que no tiene la culpa. Se dice en la bitacora con
//! su direccion. Lo que SI es una falta --usar lo que no declara-- es un NO.
//!
//! ** El caso de en medio lo trajo el DIRECTOR (14-10): LLVM emite `rep bsf`
//! (`F3 0F BC`, que es `tzcnt`) en x86-64 BASE, porque una CPU sin BMI1
//! ignora el `F3` y ejecuta `bsf`. La primera version de B1 lo daba por NO y
//! habria dejado la maquina sin escritorio. Ver `Fila::sin_ella_falla`.
//!
//! ** Y SE APARTA (LB8): el veredicto se apunta con el HASH de la region de
//! codigo (el que el cargador acaba de comprobar), la entrada y lo declarado.
//! El segundo `run` del mismo `.bex` no paga la lente. Una imagen sin hashes
//! (las embebidas) no tiene con que recordarse: se lee cada vez.

use bmo_lente_x86::recorre::{recorre, Recorrido};
use bmo_lente_x86::LARGO_MAX;

use crate::ring0::mm::{self, vmm};

/// Un bit por byte de codigo: 128 KiB cubren 1 MiB de codigo (DOOM: 509 KB).
/// Un codigo mayor no se recorre (`desborda`) y entra sin demostrar.
const VISTO_MAX: usize = 128 * 1024;
/// Lo pendiente: cada destino de un salto y cada semilla, hasta que se lee.
const PILA_MAX: usize = 32 * 1024;

// La admision va de una en una (como `REQUISITOS_BUF`): estos dos son suyos.
static mut VISTO: [u8; VISTO_MAX] = [0; VISTO_MAX];
static mut PILA: [u32; PILA_MAX] = [0; PILA_MAX];

/// Lo demostrado, por el hash de su codigo. 32 `.bex` distintos; el 33 pisa
/// al mas viejo (y, si vuelve, se lee otra vez: cuesta, no falla).
#[derive(Clone, Copy)]
struct Recuerdo {
    clave: [u8; 32],
    entrada: u64,
    declara: u64,
    demostrado: bool,
    vivo: bool,
}
const RECUERDOS: usize = 32;
static mut MEMORIA: [Recuerdo; RECUERDOS] =
    [Recuerdo { clave: [0; 32], entrada: 0, declara: 0, demostrado: false, vivo: false }; RECUERDOS];
static mut TURNO: usize = 0;
/// Cuantas veces se ahorro la lente: lo que el metal mira para saber que el
/// segundo `run` no la pago.
pub static mut AHORRADAS: u64 = 0;

/// **El juicio de un `.bex`.**
pub(crate) enum Juicio {
    /// Ya se habia leido este mismo codigo: no se vuelve a leer.
    Recordado { demostrado: bool },
    /// Se leyo ahora.
    Leido(Recorrido),
}

/// **Juzga el codigo recien mapeado** en `aspace`, de `largo` bytes desde
/// `code_va`. `declara` es la mascara de `CLASE_INSTRUCCIONES`; `clave`, el
/// hash comprobado de la region de codigo (si lo hay); `semillas`, los
/// destinos en el codigo de las relocaciones.
pub(crate) fn juzgar(
    aspace: u64,
    code_va: u64,
    largo: usize,
    entrada: u64,
    declara: u64,
    clave: Option<[u8; 32]>,
    semillas: impl IntoIterator<Item = u64>,
) -> Juicio {
    let memoria = unsafe { &mut *core::ptr::addr_of_mut!(MEMORIA) };
    if let Some(k) = clave {
        if let Some(r) = memoria.iter().find(|r| r.vivo && r.clave == k && r.entrada == entrada && r.declara == declara) {
            unsafe { AHORRADAS += 1 };
            return Juicio::Recordado { demostrado: r.demostrado };
        }
    }
    let visto = unsafe { &mut *core::ptr::addr_of_mut!(VISTO) };
    let pila = unsafe { &mut *core::ptr::addr_of_mut!(PILA) };
    // La ultima pagina traducida: casi todas las instrucciones caen en la misma.
    let mut cache: (u64, u64) = (u64::MAX, 0);
    let ventana = |en: usize, buf: &mut [u8; LARGO_MAX]| -> usize {
        let quiere = LARGO_MAX.min(largo - en);
        let mut n = 0;
        while n < quiere {
            let va = code_va + (en + n) as u64;
            let pagina = va & !(mm::PAGE - 1);
            if cache.0 != pagina {
                let Some(fisica) = vmm::translate(aspace, pagina) else { break };
                cache = (pagina, mm::phys_to_virt(fisica));
            }
            let dentro = (va - pagina) as usize;
            let cuantos = (quiere - n).min(mm::PAGE as usize - dentro);
            // El marco es de esta imagen, recien escrito y ya mapeado: se lee
            // por el physmap del kernel, como lo escribio el cargador.
            let src = unsafe { core::slice::from_raw_parts((cache.1 as *const u8).add(dentro), cuantos) };
            buf[n..n + cuantos].copy_from_slice(src);
            n += cuantos;
        }
        n
    };
    let r = recorre(
        largo,
        entrada as usize,
        semillas,
        ventana,
        |n| bmo_carga_juicio::instrucciones::por_nombre(n).is_some_and(|e| declara & e.bit != 0),
        visto,
        pila,
    );
    // Se recuerda lo que se va a admitir; un NO no hace falta recordarlo.
    if let (Some(k), None) = (clave, r.no_declarada) {
        let t = unsafe { TURNO } % RECUERDOS;
        memoria[t] = Recuerdo { clave: k, entrada, declara, demostrado: r.demostrado(), vivo: true };
        unsafe { TURNO = t + 1 };
    }
    Juicio::Leido(r)
}
