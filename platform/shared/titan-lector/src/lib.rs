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
//!    edit       one `mod` line in or out of a header, the rest untouched
//!    hang       a file under another parent: two headers, no file moves
//!    seed       `asteroids` as files, for F1 to write the first time
//!    explorer   the package as it IS on disk, in the order the owner chose
//!    organize   the EXPLORER's gestures, carrying each `.titan`'s module along
//!    traits     what each module's BODY does: how F1 draws its node, live
//!    wire       a cable pulled in F1 is a `use` written (and a cycle, refused)
//!    maestros   the TAB: the bench's programs as master nodes, and placing one
//! ```
//!
//! It is the smallest piece of the future front (`titan-front`): it knows the
//! header the owner chose on 29-09 and nothing of the body, which is the
//! grammar's (T0). See `docs/plan/PLAN_TALLER.md` 8.6.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
extern crate std;

pub mod edit;
pub mod explorer;
pub mod hang;
pub mod header;
pub mod library;
pub mod maestros;
mod maestros_gen;
pub mod manifest;
pub mod organize;
pub mod package;
pub mod seed;
pub mod text;
pub mod traits;
pub mod wire;

pub use hang::{HangError, Plan, Sink};
pub use package::{read_package, read_package_into, Fetch, FileEntry, Loaded, Problem, ProblemKind, Say, Source};
pub use text::Path;
pub use traits::{Said, Traits};

/// ** THE CUT BETWEEN LOGIC AND LOOK (the owner, 04-10: "divide bien en
/// apariencia y la logica de TITAN++"), as a test anyone can run: this crate
/// says what a package IS -- its tree, its traits, the gestures on it -- and
/// never how it is drawn. A colour, a pixel or a painter named here is the
/// look leaking into the logic; it belongs in F1's `aspecto.rs`. Comments
/// may talk about drawing (they say who reads what); the CODE may not.
#[cfg(test)]
mod the_logic_names_no_look {
    // `maestros_gen.rs` is not here: it is DATA, the bench's programs as text
    // (one of them says `let mut color`), written by a tool and checked by
    // its own guardian -- not logic that could leak the look.
    const SOURCES: [(&str, &str); 15] = [
        ("edit.rs", include_str!("edit.rs")),
        ("explorer.rs", include_str!("explorer.rs")),
        ("hang.rs", include_str!("hang.rs")),
        ("header.rs", include_str!("header.rs")),
        ("library.rs", include_str!("library.rs")),
        ("maestros.rs", include_str!("maestros.rs")),
        ("manifest.rs", include_str!("manifest.rs")),
        ("organize.rs", include_str!("organize.rs")),
        ("package.rs", include_str!("package.rs")),
        ("seed.rs", include_str!("seed.rs")),
        ("text.rs", include_str!("text.rs")),
        ("traits.rs", include_str!("traits.rs")),
        ("wire.rs", include_str!("wire.rs")),
        ("lib.rs", include_str!("lib.rs")),
        ("../Cargo.toml", include_str!("../Cargo.toml")),
    ];

    #[test]
    fn no_colour_no_pixel_no_painter() {
        for (name, src) in SOURCES {
            let code = src.split("#[cfg(test)]\nmod ").next().unwrap_or("");
            for line in code.lines().filter(|l| {
                let t = l.trim_start();
                !(t.starts_with("//") || t.starts_with('#'))
            }) {
                let low = line.to_ascii_lowercase();
                for word in ["color", "colour", "0x00", "rgb", "pixel", "bmo_dibujo", "bmo-dibujo", "bmo_pinta", "bmo-pinta"] {
                    assert!(!low.contains(word), "{} names the look (`{}`): {}", name, word, line.trim());
                }
            }
        }
    }
}
