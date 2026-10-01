//! **HOSTILE LUDOTECA** -- every line comes from whoever talks to the store
//! (the antenna today, the network tomorrow). Garbage and mutations of good
//! lines, one at a time and as a whole file.
//!
//! Checked: nothing panics, and what loads keeps its rules. See `bmo-hostile`
//! for what is not checked.

use bmo_hostile::{attack, DEFAULT_SEED};
use bmo_ludoteca::{leer, Ludoteca, LINEA_MAX};

const CASES: u32 = 20_000;

const GOOD_LINES: &[&[u8]] = &[
    b"JUEGO doom2 gog DOOM II\n",
    b"FICHERO doom2 14604584 ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad DOOM II/base/DOOM2.WAD\n",
    b"MOTOR doom2 doom\n",
    b"JUEGO cyberpunk2077 gog Cyberpunk 2077\r\n",
    b"FICHERO cyberpunk2077 59945608 ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad bin/x64/Cyberpunk2077.exe\n",
    b"# comentario\n",
];

#[test]
fn a_single_line_never_panics() {
    attack("leer", DEFAULT_SEED, CASES, GOOD_LINES, 300, |b| {
        if let Ok(_) = leer(b) {
            assert!(b.len() <= LINEA_MAX);
        }
    });
}

#[test]
fn a_whole_file_never_panics_and_keeps_its_rules() {
    let stream = GOOD_LINES.concat();
    let samples: &[&[u8]] = &[&stream];
    attack("Ludoteca::cargar", DEFAULT_SEED ^ 1, CASES, samples, 900, |b| {
        if let Ok(l) = Ludoteca::cargar(b) {
            for (i, j) in l.juegos.iter().enumerate() {
                assert!(l.juegos[..i].iter().all(|o| o.id != j.id), "un id repetido entro");
                assert!(l.camino(&j.id).is_some());
            }
            for f in &l.ficheros {
                assert!(l.juego(&f.id).is_some(), "un fichero huerfano entro");
                assert!(!f.nombre.starts_with('/') && !f.nombre.contains(".."));
            }
        }
    });
}
