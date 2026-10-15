//! ** Q2 DE `docs/plan/EL_FOCO.md` (15-10): EL REPRODUCTOR EN TITAN++
//! (`Ultra_userspace/apps/musica`), el relevo de `musica.inti`. Las mismas
//! partituras que su banco (`toolchain/lang/inti/emisor-x86_64/tests/
//! musica.rs`) dan las mismas notas, los mismos accidentes, y las mismas rotas
//! se paran en el mismo byte. Y lo que INTI no podia probar: las MUESTRAS que
//! se le piden al orquestador, contra una onda cuadrada contada aqui.

use bmo_lower::emu::{cargar_bex, run, Machine};
use std::path::Path;

fn bex() -> Vec<u8> {
    let pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../Ultra_userspace/apps/musica");
    let src = std::fs::read_to_string(pkg.join("src").join("main.titan")).expect("la app");
    bmo_titan_x86_64::build_package("src/main.titan", &src, &mut |p| std::fs::read_to_string(pkg.join(p)).ok()).unwrap_or_else(|e| panic!("{:?}", e))
}

fn toca(tema: Option<&[u8]>) -> Machine {
    let mut m = cargar_bex(&bex()).expect("carga");
    m.poner_tubo(48000, 192);
    if let Some(t) = tema {
        m.poner_archivo("datos/tema.mus", t);
    }
    let m = run(m, 200_000_000);
    assert!(m.exited, "el reproductor tiene que acabar: {}", m.console);
    m
}

/// Las notas que dijo por consola, en hercios (0: pausa).
fn notas(m: &Machine) -> Vec<u64> {
    m.console
        .lines()
        .filter_map(|l| {
            if l.starts_with("  pausa") {
                Some(0)
            } else {
                l.strip_prefix("  nota  ").and_then(|r| r.split(' ').next()).map(|x| x.parse().unwrap())
            }
        })
        .collect()
}

/// La onda que se espera, contada aqui: a 24000 muestras por segundo, el 85 %
/// de cada nota suena a +-3000 y el resto calla.
fn onda(notas: &[(u64, u64)]) -> Vec<i16> {
    let r = 24000u64;
    let mut v = Vec::new();
    for &(hz, ms) in notas {
        let n = r * ms / 1000;
        let suenan = n * 85 / 100;
        for k in 0..n {
            v.push(if hz > 0 && k < suenan { if (k * hz * 2 / r) % 2 == 0 { 3000 } else { -3000 } } else { 0 });
        }
    }
    v
}

/// ** LO QUE SUENA, muestra a muestra: LA4 negra a 120 (500 ms), un silencio
/// de corchea y DO5 corchea, en UNA voz del canal 0 a 24000 Hz, por las voces
/// del orquestador (el banco prestado, el tubo armado por el y soltado al
/// acabar).
#[test]
fn titan_plays_the_score_through_the_orchestrator_voices() {
    let m = toca(Some(b"T120 LA4 -:2 DO5:2\n"));
    assert!(m.console.contains("voces   ") && m.console.contains("fin     0"), "{}", m.console);
    assert_eq!(notas(&m), vec![440, 0, 523]);
    assert_eq!(m.voces.tocados.len(), 1, "un tramo: cabe en el banco");
    let (canal, hz, muestras) = &m.voces.tocados[0];
    assert_eq!((*canal, *hz), (0, 24000));
    assert_eq!(muestras, &onda(&[(440, 500), (0, 250), (523, 250)]), "las muestras, una a una");
    // El banco, prestado una vez; al soltar, callar todo y soltar el banco.
    let subs: Vec<u64> = m.voces.llamadas.iter().map(|v| v.0).collect();
    assert_eq!((subs.first(), subs.iter().filter(|&&s| s == 2).count()), (Some(&1), 1));
    assert!(subs.ends_with(&[4, 6]), "callar todo y soltar el banco: {:?}", subs);
}

/// Sostenido, bemol y el salto de octava: lo mismo que el de INTI.
#[test]
fn the_accidentals_cross_the_octave_as_in_inti() {
    let m = toca(Some(b"SI#4:1 DOB5:1 SOL#5:1 SIB3:1"));
    assert_eq!(notas(&m), vec![523, 494, 831, 233]);
}

/// Sin fichero, dice 1 y ni reclama el sonido.
#[test]
fn without_a_file_it_says_1_and_plays_nothing() {
    let m = toca(None);
    assert!(m.console.contains("fin     1"), "{}", m.console);
    assert!(m.voces.llamadas.is_empty() && m.voces.tocados.is_empty());
}

/// Una partitura rota se para, y dice EN QUE BYTE -- el mismo que INTI --; lo
/// que ya estaba en el banco suena.
#[test]
fn a_broken_score_stops_and_says_where_as_in_inti() {
    let m = toca(Some(b"T120 LA4 XX4 DO5"));
    assert!(m.console.contains("mal en  9"), "{}", m.console);
    assert!(m.console.contains("fin     5"), "{}", m.console);
    assert_eq!(notas(&m), vec![440]);
    assert_eq!(m.voces.tocados.len(), 1);
    assert_eq!(m.voces.tocados[0].2, onda(&[(440, 500)]));
}

/// Cada forma de escribirlo mal (las del banco de INTI), y ninguna suena.
#[test]
fn every_way_to_write_it_wrong_and_none_plays() {
    for mal in ["LA9", "LA4:0", "LA4:65", "LA4X", "T", "DOB0", "SI#8", "LA", "LA4:"] {
        let m = toca(Some(mal.as_bytes()));
        assert!(m.console.contains("fin     5"), "`{}` tenia que estar mal: {}", mal, m.console);
        assert!(m.voces.tocados.is_empty(), "`{}` no puede sonar", mal);
    }
}
