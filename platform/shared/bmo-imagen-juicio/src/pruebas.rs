use super::*;

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

fn limites(libre: u64) -> Limites {
    Limites { max_partes: 128, libre, margen: 64 * MIB, ventana_base: 0x10_0000_0000, ventana_bytes: 16 * GIB, pagina: 4096, alineacion: 64 << 10 }
}

fn c(pe: u16, bytes: u64) -> Parte {
    Parte { pe, codigo: true, bytes }
}

fn d(pe: u16, bytes: u64) -> Parte {
    Parte { pe, codigo: false, bytes }
}

/// Como la de Cyberpunk: el .exe (43 + 37 MiB), libxess (70 MiB de datos) y
/// 25 DLL mas, de 3 + 2 MiB.
fn cyberpunk() -> std::vec::Vec<Parte> {
    let mut v = std::vec![c(0, 43 * MIB), d(0, 37 * MIB), c(1, 5 * MIB), d(1, 70 * MIB)];
    for pe in 2..27 {
        v.push(c(pe, 3 * MIB));
        v.push(d(pe, 2 * MIB + 123));
    }
    v
}

#[test]
fn la_de_cyberpunk_cabe_con_ram_de_sobra() {
    let v = cyberpunk();
    let Veredicto::Concedida { ram, va } = juzgar(&v, &limites(24 * GIB)) else { panic!("{:?}", juzgar(&v, &limites(24 * GIB))) };
    assert!(ram > 200 * MIB && ram < 300 * MIB, "{ram}");
    assert!(va >= ram);
}

#[test]
fn es_dinamico_sin_ram_dice_cuanta_pide_y_cuanta_hay() {
    let v = cyberpunk();
    let Veredicto::Concedida { ram, .. } = juzgar(&v, &limites(24 * GIB)) else { panic!() };
    // Justo lo que pide mas el margen: cabe. Una pagina menos: no.
    assert!(matches!(juzgar(&v, &limites(ram + 64 * MIB)), Veredicto::Concedida { .. }));
    assert_eq!(juzgar(&v, &limites(ram + 64 * MIB - 4096)), Veredicto::Negada(Motivo::SinRam { pide: ram, hay: ram - 4096 }));
    // Menos RAM libre que el margen: hay 0, no un numero que da la vuelta.
    assert_eq!(juzgar(&v, &limites(10 * MIB)), Veredicto::Negada(Motivo::SinRam { pide: ram, hay: 0 }));
}

#[test]
fn codigo_y_datos_seguidos_y_cada_pe_alineado() {
    let v = cyberpunk();
    let l = limites(24 * GIB);
    let (c0, n0) = donde(&v, &l, 0).unwrap();
    let (d0, _) = donde(&v, &l, 1).unwrap();
    assert_eq!(c0, l.ventana_base);
    assert_eq!(d0, c0 + n0, "los datos del .exe, justo detras de su codigo");
    for i in 0..v.len() {
        let (va, n) = donde(&v, &l, i).unwrap();
        assert_eq!(n % 4096, 0);
        assert!(n >= v[i].bytes && n - v[i].bytes < 4096);
        if v[i].codigo {
            assert_eq!(va % (64 << 10), 0, "el PE {} empieza alineado", v[i].pe);
        }
        if i > 0 {
            let (antes, m) = donde(&v, &l, i - 1).unwrap();
            assert!(va >= antes + m, "no se pisan");
        }
    }
    assert_eq!(donde(&v, &l, v.len()), None);
}

#[test]
fn un_pe_solo_de_datos_tambien_empieza_alineado() {
    let v = [c(0, 5000), d(1, 100)];
    let l = limites(GIB);
    assert_eq!(donde(&v, &l, 1).unwrap().0, l.ventana_base + (64 << 10));
}

#[test]
fn lo_que_no_vale() {
    let l = limites(24 * GIB);
    assert_eq!(juzgar(&[], &l), Veredicto::Negada(Motivo::Vacia));
    let muchas: std::vec::Vec<Parte> = (0..129).map(|i| c(i as u16, 4096)).collect();
    assert_eq!(juzgar(&muchas, &l), Veredicto::Negada(Motivo::DeMas { partes: 129, max: 128 }));
    assert_eq!(juzgar(&[c(0, 10), d(0, 0)], &l), Veredicto::Negada(Motivo::ParteVacia { i: 1 }));
    // Desordenadas: datos antes que codigo, dos codigos, un PE que salta o
    // vuelve, y uno que no empieza en 0.
    assert_eq!(juzgar(&[d(0, 10), c(0, 10)], &l), Veredicto::Negada(Motivo::Desordenada { i: 1 }));
    assert_eq!(juzgar(&[c(0, 10), c(0, 10)], &l), Veredicto::Negada(Motivo::Desordenada { i: 1 }));
    assert_eq!(juzgar(&[c(0, 10), c(2, 10)], &l), Veredicto::Negada(Motivo::Desordenada { i: 1 }));
    assert_eq!(juzgar(&[c(0, 10), d(0, 10), c(0, 10)], &l), Veredicto::Negada(Motivo::Desordenada { i: 2 }));
    assert_eq!(juzgar(&[c(1, 10)], &l), Veredicto::Negada(Motivo::Desordenada { i: 0 }));
}

#[test]
fn la_ventana_manda_aunque_sobre_ram() {
    let mut l = limites(64 * GIB);
    l.ventana_bytes = 100 * MIB;
    assert_eq!(juzgar(&cyberpunk(), &l), Veredicto::Negada(Motivo::SinVentana { pide: match juzgar(&cyberpunk(), &limites(64 * GIB)) {
        Veredicto::Concedida { va, .. } => va,
        _ => unreachable!(),
    }, hay: 100 * MIB }));
}

#[test]
fn medidas_absurdas_no_dan_la_vuelta() {
    let l = limites(u64::MAX);
    assert!(matches!(juzgar(&[c(0, u64::MAX)], &l), Veredicto::Negada(Motivo::SinVentana { .. })));
    assert!(matches!(juzgar(&[c(0, u64::MAX / 2), d(0, u64::MAX / 2)], &l), Veredicto::Negada(Motivo::SinVentana { .. })));
}

#[test]
fn limites_que_no_tienen_sentido() {
    let mut l = limites(GIB);
    l.pagina = 3000;
    assert_eq!(juzgar(&[c(0, 10)], &l), Veredicto::Negada(Motivo::LimitesMalos));
    let mut l = limites(GIB);
    l.alineacion = 1024;
    assert_eq!(juzgar(&[c(0, 10)], &l), Veredicto::Negada(Motivo::LimitesMalos));
}
