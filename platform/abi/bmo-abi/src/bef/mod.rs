//! **Lo que un BEF2 lleva DENTRO de sus anexos**: las tablas que viajan con un
//! programa y que ni el formato ni el kernel abren -- las lee quien las pidio.
//!
//! [carril]  AMARILLO  lo que aqui se lea mal lo nota una app, no el kernel
//! [cuesta]  TAREA     una app sin su icono, un requisito que no se ve
//! [riesgo]  ESPEJO    algunas tablas tienen un lector en Ring 0
//!                     (`bmo-carga-juicio` los requisitos) y otro aqui
//!
//! ** BEF1 --la cabecera de 48 B con tabla de secciones, "ELF con otro
//! nombre"-- MURIO el 2026-09-19 (B6 de `docs/plan/PLAN_BEF_NATIVO.md`). El
//! formato es `crate::bef2`. Lo que queda en esta carpeta son los CONTENIDOS
//! de anexos, que no cambiaron de bytes al cambiar el contenedor:
//!
//! ```text
//!    katanas     anexo KATANAS      lo que un .ibx de INTI se compromete a no hacer
//!    recursos    anexo RECURSOS     una app es UN fichero: sus datos dentro
//!    requisitos  anexo REQUISITOS   lo que hace falta para arrancar (lo lee Ring 0)
//!    symbols     anexo SIMBOLOS     que funcion vive en cada offset (objetos y autopsias)
//!    blake3      el hash de todo lo anterior, reexportado de bmo-hash
//! ```

pub mod blake3;
pub mod katanas;
pub mod recursos;
pub mod requisitos;
pub mod symbols;

pub use recursos::{Directorio, Entrada as EntradaRecurso, RECURSOS_MAGIC};
pub use requisitos::{
    Declaracion as DeclaracionRequisito, Requisito, Tabla as TablaRequisitos, REQUISITOS_MAGIC,
};
pub use symbols::{Symbol, TablaCadenas};
