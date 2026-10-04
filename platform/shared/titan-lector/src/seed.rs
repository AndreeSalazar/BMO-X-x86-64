//! **THE SEED** -- the `asteroids` package as FILES, the ones F1 writes into
//! ESTRATOS the first time it finds no library there.
//!
//! The headers make the graph; the BODIES, since 04-10, are TITAN++ of the
//! levels already decided (0-2: `fn`, `let`, `mut`, `print`), so every node of
//! the first package shows something of its own in F1 (`traits.rs`). Before
//! that day the files were headers only, on purpose: code here would have
//! presented as decided a syntax nobody had decided. The test
//! `the_seed_files_are_the_hand_written_sample` proves that reading these
//! files still gives exactly `bmo_titan_contrato::sample::asteroids()`.

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
         mod ship, rock, physics\n\
         \n\
         fn main()\n\
         \x20   let titulo = \"ASTEROIDS\"\n\
         \x20   print(titulo, \" -- hecho en TITAN++\")\n",
    ),
    (
        "titan/asteroids/src/ship.titan",
        "mod ship \"la nave: se mueve y dispara\"\n\
         \n\
         fn avanza()\n\
         \x20   let mut x = 100\n\
         \x20   let mut combustible = 50\n\
         \x20   x = x + 3\n\
         \x20   combustible = combustible - 1\n\
         \x20   print(\"nave en \", x, \" con \", combustible)\n",
    ),
    (
        "titan/asteroids/src/rock.titan",
        "mod rock \"las rocas: se parten al chocar\"\n\
         \n\
         fn parte()\n\
         \x20   let grande = 64\n\
         \x20   let mitad = grande / 2\n\
         \x20   let cuarto = mitad / 2\n",
    ),
    (
        "titan/asteroids/src/physics.titan",
        "mod physics \"mueve los cuerpos y resuelve los choques\"\n\
         use ship, gpu\n\
         mod collide\n\
         \n\
         fn paso()\n\
         \x20   let mut t = 0\n\
         \x20   t = t + 16\n\
         \x20   print(\"fotograma: \", t, \" ms\")\n\
         \x20   choques()\n\
         \n\
         fn choques()\n\
         \x20   print(\"mira quien toca a quien\")\n",
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

    /// The bodies are TITAN++ of the levels done: each one, wrapped in a
    /// module of its own, is something the reader can count.
    #[test]
    fn the_bodies_give_each_node_traits_of_its_own() {
        let t = |p: &str| crate::traits::scan(FILES.iter().find(|f| f.0 == p).unwrap().1.as_bytes());
        let ship = t("titan/asteroids/src/ship.titan");
        assert_eq!((ship.muts, ship.changes, ship.writes), (2, 2, 1));
        let rock = t("titan/asteroids/src/rock.titan");
        assert_eq!((rock.lets, rock.muts, rock.writes), (3, 0, 0));
        let physics = t("titan/asteroids/src/physics.titan");
        assert_eq!((physics.fns, physics.writes, physics.calls), (2, 2, 1));
        assert_eq!(t("titan/asteroids/src/physics/collide.titan").lines, 0);
    }

    #[test]
    fn the_library_lists_the_seeded_package() {
        let (_, text) = FILES.iter().find(|f| f.0 == LIBRARY).unwrap();
        let lib = crate::library::parse(text.as_bytes()).unwrap();
        assert_eq!(lib.packages()[0].1.as_bytes(), b"titan/asteroids");
    }
}
