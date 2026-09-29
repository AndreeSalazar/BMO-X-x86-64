//! **TITAN++ READER** -- a package on disk into the contract's graph.
//!
//! [layer]  PURE   no disk, no allocator, no `unsafe`: it reads through a
//!                 `Source` and writes into fixed arrays
//!
//! ```text
//!    manifest   Titan.toml: name, [permissions], [layout]
//!    header     the first lines of a .titan: `mod x "..."`, `use`, `mod a, b`
//!    package    follow `mod` from src/main.titan, like cargo: the Graph,
//!               the files it read and the PROBLEMS, each with its names
//!    library    titan/biblioteca.toml: which packages exist
//!    seed       `asteroids` as files, for F1 to write the first time
//! ```
//!
//! It is the smallest piece of the future front (`titan-front`): it knows the
//! header the owner chose on 29-09 and nothing of the body, which is the
//! grammar's (T0). See `docs/plan/PLAN_TALLER.md` 8.6.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
extern crate std;

pub mod header;
pub mod library;
pub mod manifest;
pub mod package;
pub mod seed;
pub mod text;

pub use package::{read_package, Fetch, FileEntry, Loaded, Problem, ProblemKind, Source};
pub use text::Path;
