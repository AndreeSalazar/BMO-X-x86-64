//! `casa` -- THE HOUSE LIBRARY (18-10): TITAN++ modules every app may use,
//! instead of copying them. ECCE, RESOLUCION and TINTA carried the same
//! `medida.titan`; four apps carried the same `es_cifra`.
//!
//! ```text
//!    use casa.medida        in a header: the module comes from the house
//!    medida.ancho()         and is called by its own name, like any module
//! ```
//!
//! The modules travel INSIDE the compiler (`include_str!`, like the prelude
//! does with `Numero`): a package needs no copy and no path, and every
//! build of an app uses the house of the compiler that builds it.
//!
//! ** The rules, checked where the package is read and joined (`paquete.rs`):
//!
//! ```text
//!    use casa.nada          there is no such house module            T0051
//!    mod medida + casa      a module of the package with a house name T0052
//!    use medida             the house's, without `casa.`              T0051
//!    a house module that    uses a module of the APP: the house is   T0051
//!                           below every app, never above one (L8)
//! ```
//!
//! A house module may `use` other house modules and the nodes of BMO-X
//! (`director`...). Using a node still needs the app's manifest to ask for
//! it (U2): the house does not grant what the kernel concedes.

/// What a `use` says before a house module's name.
pub const PREFIX: &str = "casa.";

/// Where a house module is in the package's count of files: a path no
/// package can write (a `mod ... in` path never starts here by accident,
/// and the NO of a house module says which one).
pub const FOLDER: &str = "casa/";

/// Every house module, by its name, and its source.
const MODULES: [(&str, &str); 4] = [
    ("cifras", include_str!("../casa/cifras.titan")),
    ("colores", include_str!("../casa/colores.titan")),
    ("letrero", include_str!("../casa/letrero.titan")),
    ("medida", include_str!("../casa/medida.titan")),
];

/// The house module that `use casa.x` names: `x`, if it is one.
pub fn short(used: &str) -> Option<&str> {
    used.strip_prefix(PREFIX)
}

/// The source of the house module `name`.
pub fn source(name: &str) -> Option<&'static str> {
    MODULES.iter().find(|m| m.0 == name).map(|m| m.1)
}

/// The names of the house modules, to say which there are.
pub fn names() -> Vec<&'static str> {
    MODULES.iter().map(|m| m.0).collect()
}

/// Its path in the package: `casa/medida.titan`.
pub fn path(name: &str) -> String {
    format!("{}{}.titan", FOLDER, name)
}

/// Whether a file of the package is a house module.
pub fn is_house(path: &str) -> bool {
    path.starts_with(FOLDER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_house_module_says_its_own_name_in_its_header() {
        for (name, src) in MODULES {
            let first = src.lines().find(|l| !l.is_empty() && !l.starts_with('#')).unwrap();
            assert!(first.starts_with(&format!("mod {} \"", name)), "{name}: {first}");
        }
    }
}
