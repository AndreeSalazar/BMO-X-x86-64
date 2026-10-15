//! **LO QUE DECIDIO EL PROPIETARIO** (B4 de `docs/plan/EL_FOCO.md`):
//! las columnas `mano` y `llvm` de la tabla de extensiones de
//! `PERFIL/CPU.txt`, como mascaras de [`crate::instrucciones`].
//!
//! ** GENERADO por `toolchain/tools/perfil-campos/perfil_campos.py --apply`.
//! No se escribe a mano: se cambia el perfil y se regenera. El guardian
//! falla si este fichero y el perfil no dicen lo mismo.

/// Lo que el codigo de la CASA puede usar: `intrinsics.toml` y el `asm!` de
/// los crates. `sem-asm` no carga una fila fuera de esto y `bex-link` para
/// el build de un programa de Rust que lo lleve.
pub const MANO: u64 = 1 << 2 | 1 << 4 | 1 << 5 | 1 << 6 | 1 << 8 | 1 << 10 | 1 << 11 | 1 << 12 | 1 << 13 | 1 << 14 | 1 << 15 /* SSE4.1, POPCNT, LZCNT, BMI1, AVX, FMA, RDRAND, RDSEED, RDTSCP, CR4.OSXSAVE, MONITOR/MWAIT */;

/// Lo que el COMPILADOR (LLVM) puede elegir por su cuenta. Lo exige el
/// guardian `llvm-obedece` en cada objetivo de Rust.
pub const LLVM: u64 = 0 /* ninguna */;

const _: () = assert!(LLVM & !MANO == 0, "LLVM no elige lo que la casa no puede");
