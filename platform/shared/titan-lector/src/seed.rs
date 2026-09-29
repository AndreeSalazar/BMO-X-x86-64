//! **THE SEED** -- the `asteroids` package as FILES, the ones F1 writes into
//! ESTRATOS the first time it finds no library there.
//!
//! Only headers: the body of a `.titan` is the grammar's (T0), and writing code
//! here would present as decided a syntax nobody has decided. The test
//! `the_seed_files_are_the_hand_written_sample` proves that reading these
//! files gives exactly `bmo_titan_contrato::sample::asteroids()`.

/// Where the library index lives.
pub const LIBRARY: &str = "titan/biblioteca.toml";

/// The folders to create, parents first.
pub const FOLDERS: &[&str] = &["titan", "titan/asteroids", "titan/asteroids/src", "titan/asteroids/src/physics"];

/// Path and contents, the index LAST: while it is missing, F1 seeds again,
/// so a seed cut halfway is finished the next time instead of being taken as
/// a library.
pub const FILES: &[(&str, &str)] = &[
    (
        "titan/asteroids/Titan.toml",
        "[package]\n\
         name = \"asteroids\"\n\
         version = \"0.1.0\"\n\
         edition = \"2026\"\n\
         \n\
         [permissions]\n\
         screen = true\n\
         input = true\n\
         sound = true\n\
         gpu = \"compute\"\n\
         disk = false\n\
         net = false\n\
         \n\
         # Where each node sits in F1: the positions live in the main node.\n\
         [layout]\n\
         asteroids = [380, 20]\n\
         main = [380, 150]\n\
         ship = [60, 300]\n\
         rock = [380, 300]\n\
         physics = [700, 300]\n\
         collide = [700, 460]\n\
         gpu = [1020, 460]\n\
         director = [60, 460]\n",
    ),
    (
        "titan/asteroids/src/main.titan",
        "mod main \"arranca el juego y el bucle del fotograma\"\n\
         use director\n\
         mod ship, rock, physics\n",
    ),
    ("titan/asteroids/src/ship.titan", "mod ship \"la nave: se mueve y dispara\"\n"),
    ("titan/asteroids/src/rock.titan", "mod rock \"las rocas: se parten al chocar\"\n"),
    (
        "titan/asteroids/src/physics.titan",
        "mod physics \"mueve los cuerpos y resuelve los choques\"\n\
         use ship, gpu\n\
         mod collide\n",
    ),
    ("titan/asteroids/src/physics/collide.titan", "mod collide \"quien toca a quien\"\n"),
    (LIBRARY, "[packages]\nasteroids = \"titan/asteroids\"\n"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_goes_last_and_every_folder_comes_before_its_files() {
        assert_eq!(FILES.last().map(|f| f.0), Some(LIBRARY));
        for (path, _) in FILES {
            let dir = &path[..path.rfind('/').unwrap()];
            assert!(FOLDERS.contains(&dir), "{path}");
        }
    }

    #[test]
    fn the_library_lists_the_seeded_package() {
        let (_, text) = FILES.iter().find(|f| f.0 == LIBRARY).unwrap();
        let lib = crate::library::parse(text.as_bytes()).unwrap();
        assert_eq!(lib.packages()[0].1.as_bytes(), b"titan/asteroids");
    }
}
