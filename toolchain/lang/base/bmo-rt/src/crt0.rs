//! **El arranque de un programa de C en BMO-X**: `_start` -> `main` -> `exit`.
//!
//! Lo que el kernel ya deja hecho al admitir un `.bex` (BEF2), y por eso aqui
//! NO se hace:
//!
//! ```text
//!    los ceros   la region CEROS de BEF2 llega a cero: el kernel da marcos
//!                limpios. No hay `__bss_start`/`__bss_end` que recorrer --
//!                eso era el `crt0` de un ELF de GNU, y el `link.ld` de BMO ni
//!                siquiera los define: con ellos, esto no enlazaba
//!    la pila     puesta y alineada, como a cualquier `_start` de la casa
//!    el entorno  no hay `envp` ni `auxv`: lo que un proceso sabe lo pregunta
//!                por su handle (`TASK_OP_INFO`, `OP_MI_PAQUETE`...)
//! ```
//!
//! Lo que SI hace: traer los argumentos (`TASK_OP_ARGUMENTOS`), llamar a
//! `main`, y al volver, `exit` -- que cierra los `FILE` abiertos, porque en
//! BMO-X lo escrito llega al disco AL CERRAR.
//!
//! `_start` se llama asi porque es la `ENTRY` del `link.ld` de la casa: el
//! mismo nombre que usan el escritorio y las apps de Rust. El kernel salta al
//! campo `entrada` de la cabecera BEF2, que `bex-link` saca de ahi.

use crate::argumentos::{partir, LINEA, MAX_ARGS};
use core::ptr::addr_of_mut;

extern "C" {
    fn main(argc: i32, argv: *const *const u8) -> i32;
}

static mut LINEA_ARGS: [u8; LINEA] = [0; LINEA];
static mut ARGV: [*const u8; MAX_ARGS + 2] = [core::ptr::null(); MAX_ARGS + 2];
static VACIA: [u8; 1] = [0];

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    let linea = &mut *addr_of_mut!(LINEA_ARGS);
    let argv = &mut *addr_of_mut!(ARGV);
    let n = crate::syscall::argumentos(&mut linea[..LINEA - 1]);
    let argc = partir(linea, n, argv, VACIA.as_ptr());
    exit(main(argc as i32, argv.as_ptr()))
}

/// **Salir**: cerrar los `FILE` (lo escrito baja al disco) y terminar.
#[no_mangle]
pub unsafe extern "C" fn exit(codigo: i32) -> ! {
    crate::ffi::cerrar_todos();
    crate::syscall::proc_exit(codigo as u32);
    loop {
        core::arch::asm!("pause");
    }
}

/// **Abortar**: terminar YA, sin cerrar nada (lo escrito a medias no llega:
/// mejor nada que un fichero a medias).
#[no_mangle]
pub unsafe extern "C" fn abort() -> ! {
    crate::syscall::proc_exit(134);
    loop {
        core::arch::asm!("pause");
    }
}
