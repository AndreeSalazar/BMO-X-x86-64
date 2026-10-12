//! El banco de AYNI: sin GPU, con imagenes hechas aqui.

use alloc::vec;
use alloc::vec::Vec;

use crate::maestra::{self, error_entre, sombrear};
use crate::mano::{self, Muestra, Prediccion};
use crate::pacha::{rasgos, Baldosa, Extra, Imagen, LADO, PIX};
use crate::semilla::{self, Clase};
use crate::uma::{Dado, Uma};
use crate::{Ayni, Config, Tasa};

fn plana(v: f32) -> Baldosa {
    [v; PIX]
}

/// Cambia a lo alto (cada fila un poco mas clara).
fn degradado_vertical() -> Baldosa {
    let mut b = [0.0; PIX];
    for y in 0..LADO {
        for x in 0..LADO {
            b[y * LADO + x] = 0.2 + 0.02 * y as f32;
        }
    }
    b
}

#[test]
fn las_tasas_cuentan_sus_invocaciones() {
    assert_eq!(Tasa::X1.invocaciones(), 256);
    assert_eq!(Tasa::X2x1.invocaciones(), 128);
    assert_eq!(Tasa::X1x2.invocaciones(), 128);
    assert_eq!(Tasa::X2x2.invocaciones(), 64);
    assert_eq!(Tasa::X4x4.invocaciones(), 16);
    assert_eq!(Tasa::Cero.invocaciones(), 0);
}

#[test]
fn la_maestra_mide_lo_que_el_silicio_haria() {
    // Lo liso no pierde nada con ninguna tasa.
    assert_eq!(maestra::errores(&plana(0.5)), [0.0; 4]);
    // Lo que cambia a lo alto se puede engrosar a lo ANCHO sin error, y a lo
    // alto no.
    let d = degradado_vertical();
    let e = maestra::errores(&d);
    assert_eq!(e[0], 0.0, "X2x1 en un degradado vertical");
    assert!(e[1] > 0.0 && e[3] > e[1], "{e:?}");
    // Un pixel repetido por cuadro.
    let s = sombrear(&d, Tasa::X4x4);
    assert_eq!(s[5 * LADO + 7], d[4 * LADO + 4]);
}

#[test]
fn el_error_pesa_menos_en_lo_claro() {
    let mut oscura = plana(0.1);
    let mut clara = plana(0.8);
    oscura[0] += 0.05;
    clara[0] += 0.05;
    assert!(error_entre(&plana(0.1), &oscura) > error_entre(&plana(0.8), &clara));
}

#[test]
fn uma_nace_sabiendo_lo_basico() {
    let u = Uma::nacer();
    let mut d = Dado(7);
    let x = |b: &Baldosa| rasgos(b, Some(b), &Extra::default()).entrada;
    // Lo liso: 4x4 con error casi nulo.
    let p = u.predecir(&x(&plana(0.4)));
    assert!(p[3] < 0.02, "plana {p:?}");
    // La textura fuerte: ninguna tasa gruesa cabe en un umbral suave.
    let mut alto = 0.0f32;
    for _ in 0..20 {
        let b = semilla::baldosa(&mut d, Clase::Textura, true);
        if maestra::errores(&b)[0] > 0.2 {
            alto = alto.max(u.predecir(&x(&b))[0]);
        }
    }
    assert!(alto > Config::SUAVE.umbral, "textura {alto}");
}

#[test]
fn uma_aprende_con_cada_leccion() {
    let mut u = Uma::en_blanco(1);
    let b = semilla::baldosa(&mut Dado(3), Clase::Rayas, true);
    let x = rasgos(&b, Some(&b), &Extra::default()).entrada;
    let objetivo = maestra::errores(&b);
    let primera = u.aprender(&x, &objetivo);
    let mut ultima = primera;
    for _ in 0..500 {
        ultima = u.aprender(&x, &objetivo);
    }
    assert!(ultima < primera * 0.01, "{primera} -> {ultima}");
    assert_eq!(u.lecciones, 501);
}

// --- la 5D ---------------------------------------------------------------

/// Un puntero que frena sin parar de golpe: x = x0 + v t - a t^2 / 2.
fn frenada(x0: f32, v: f32, a: f32, n: usize, cada_us: u64) -> Vec<Muestra> {
    (0..n)
        .map(|i| {
            let t = (i as u64 * cada_us) as f32 / 1e6;
            Muestra { t_us: i as u64 * cada_us, x: x0 + v * t - 0.5 * a * t * t, y: 300.0 }
        })
        .collect()
}

#[test]
fn la_mano_que_frena_dice_donde_para() {
    // De 100 px a 2000 px/s, frenando a 8000 px/s^2: para en 100 + 250.
    let h = frenada(100.0, 2000.0, 8000.0, 6, 8000);
    let p = mano::predecir(&h, 1_000_000, 1920.0).unwrap();
    assert!((p.x - 350.0).abs() < 2.0, "{p:?}");
    assert!((p.y - 300.0).abs() < 0.01);
    assert!(p.confianza > 0.9, "{p:?}");
    // Con un horizonte corto, donde estara entonces (no donde para).
    let corto = mano::predecir(&h, 8000, 1920.0).unwrap();
    assert!(corto.x < p.x);
}

#[test]
fn la_mano_que_tiembla_no_da_confianza() {
    let h = [
        Muestra { t_us: 0, x: 100.0, y: 100.0 },
        Muestra { t_us: 8000, x: 140.0, y: 100.0 },
        Muestra { t_us: 16000, x: 100.0, y: 130.0 },
    ];
    let p = mano::predecir(&h, 16000, 1920.0).unwrap();
    assert!(p.confianza < mano::CONFIANZA_MIN, "{p:?}");
    // Quieta: lo mas predecible.
    let quieta = [Muestra { t_us: 0, x: 5.0, y: 5.0 }, Muestra { t_us: 8000, x: 5.0, y: 5.0 }, Muestra { t_us: 16000, x: 5.0, y: 5.0 }];
    let q = mano::predecir(&quieta, 16000, 1920.0).unwrap();
    assert_eq!((q.x, q.y, q.confianza), (5.0, 5.0, 1.0));
    assert!(mano::predecir(&quieta[..2], 16000, 1920.0).is_none());
}

#[test]
fn la_prueba_del_no_de_la_5d_la_entrada_no_se_toca() {
    // Lo que llega al juego es identico, bit a bit, con la 5D o sin ella:
    // `predecir` la recibe prestada y sin `mut`, y aqui se comprueba.
    let h = frenada(100.0, 2000.0, 8000.0, 6, 8000);
    let copia = h.clone();
    let _ = mano::predecir(&h, 16000, 1920.0);
    assert_eq!(h, copia);
}

#[test]
fn la_raiz_sin_libm() {
    for v in [0.0f32, 0.25, 2.0, 9.0, 12345.0] {
        let r = mano::raiz(v);
        assert!((r * r - v).abs() <= v * 1e-5 + 1e-6, "{v} -> {r}");
    }
}

// --- el banco: una escena entera ----------------------------------------

const ANCHO: usize = 128;
const ALTO: usize = 64;
/// La baldosa del HUD: lisa, pero de INTERFAZ.
const HUD: usize = 0;

/// La escena: a la izquierda un cielo (degradado a lo alto), a la derecha
/// una textura con ruido fijo, y arriba a la izquierda un panel de HUD liso.
fn escena() -> Vec<f32> {
    let mut d = Dado(99);
    let mut l = vec![0.0; ANCHO * ALTO];
    for y in 0..ALTO {
        for x in 0..ANCHO {
            l[y * ANCHO + x] = if x < ANCHO / 2 { 0.3 + 0.012 * y as f32 } else { 0.3 + 0.5 * d.real() };
        }
    }
    for y in 0..LADO {
        for x in 0..LADO {
            l[y * ANCHO + x] = 0.15;
        }
    }
    l
}

/// Lo que la GPU daria con el mapa: cada baldosa a su tasa.
fn dibujar(llena: &[f32], mapa: &[Tasa], antes: &[f32]) -> Vec<f32> {
    let img = Imagen { l: llena, ancho: ANCHO, alto: ALTO };
    let mut out = antes.to_vec();
    let cols = img.columnas();
    for (i, t) in mapa.iter().enumerate() {
        if *t == Tasa::Cero {
            continue;
        }
        let (bx, by) = (i % cols, i / cols);
        let s = sombrear(&img.baldosa(bx, by), *t);
        for y in 0..LADO {
            for x in 0..LADO {
                out[(by * LADO + y) * ANCHO + bx * LADO + x] = s[y * LADO + x];
            }
        }
    }
    out
}

fn extras(cols: usize, filas: usize) -> Vec<Extra> {
    let mut e = vec![Extra::default(); cols * filas];
    e[HUD].interfaz = true;
    e
}

/// Corre `n` fotogramas y devuelve AYNI y la ultima imagen dibujada.
fn correr(cfg: Config, n: usize, uma: &Uma, mano: Option<&Prediccion>) -> (Ayni, Vec<f32>) {
    let llena = escena();
    let mut a = Ayni::con_uma(ANCHO, ALTO, cfg, uma.clone());
    let ex = extras(a.columnas(), a.filas());
    let mut antes = vec![0.0; ANCHO * ALTO];
    let mut dibujada = antes.clone();
    for _ in 0..n {
        dibujada = dibujar(&llena, a.mapa(), &antes);
        let ahora = Imagen { l: &dibujada, ancho: ANCHO, alto: ALTO };
        let previa = Imagen { l: &antes, ancho: ANCHO, alto: ALTO };
        a.mirar(&ahora, Some(&previa), &ex);
        a.decidir(mano);
        antes = dibujada.clone();
    }
    (a, dibujada)
}

#[test]
fn el_banco_ahorra_donde_no_se_nota() {
    let uma = Uma::nacer();
    let (a, dibujada) = correr(Config::SUAVE, 40, &uma, None);
    let cols = a.columnas();
    let mapa = a.mapa();
    // El cielo (izquierda, sin el HUD) va grueso; la textura (derecha), entera.
    for i in 0..mapa.len() {
        let x = i % cols;
        if i == HUD {
            assert_eq!(mapa[i], Tasa::X1, "el HUD");
        } else if x < cols / 2 {
            assert!(mapa[i].invocaciones() <= 128, "cielo {i}: {:?}", mapa[i]);
        } else {
            assert_eq!(mapa[i], Tasa::X1, "textura {i}");
        }
    }
    let c = a.cuentas;
    assert!(c.invocaciones * 100 < c.llenas * 75, "{c:?}");
    // Y lo dibujado se parece a lo de verdad: el error medio en el cielo
    // cabe en el umbral.
    let llena = escena();
    let (img, dib) = (Imagen { l: &llena, ancho: ANCHO, alto: ALTO }, Imagen { l: &dibujada, ancho: ANCHO, alto: ALTO });
    let mut peor = 0.0f32;
    for by in 0..a.filas() {
        for bx in 0..cols / 2 {
            peor = peor.max(error_entre(&img.baldosa(bx, by), &dib.baldosa(bx, by)));
        }
    }
    assert!(peor <= Config::SUAVE.umbral, "peor {peor}");
}

#[test]
fn la_prueba_del_no_sin_la_regla_de_interfaz_el_hud_engruesa() {
    let uma = Uma::nacer();
    let cfg = Config { regla_interfaz: false, ..Config::SUAVE };
    let (a, _) = correr(cfg, 40, &uma, None);
    assert_ne!(a.mapa()[HUD], Tasa::X1, "sin la regla, el HUD liso se engruesa: la regla es la que lo salva");
}

#[test]
fn ninguna_baldosa_se_queda_gruesa_para_siempre() {
    let uma = Uma::nacer();
    let cfg = Config::SUAVE;
    let llena = escena();
    let mut a = Ayni::con_uma(ANCHO, ALTO, cfg, uma);
    let ex = extras(a.columnas(), a.filas());
    let mut antes = vec![0.0; ANCHO * ALTO];
    let cielo = a.columnas() + 1; // fila 1, columna 1: cielo
    let mut desde_x1 = 0u32;
    let mut peor = 0u32;
    for f in 0..120 {
        let d = dibujar(&llena, a.mapa(), &antes);
        a.mirar(&Imagen { l: &d, ancho: ANCHO, alto: ALTO }, Some(&Imagen { l: &antes, ancho: ANCHO, alto: ALTO }), &ex);
        a.decidir(None);
        if a.mapa()[cielo] == Tasa::X1 {
            if f > 40 {
                peor = peor.max(desde_x1);
            }
            desde_x1 = 0;
        } else {
            desde_x1 += 1;
        }
        antes = d;
    }
    assert!(peor > 0 && peor <= cfg.refresco, "fotogramas seguidos gruesa: {peor}");
}

#[test]
fn lo_que_no_cambio_no_se_vuelve_a_sombrear() {
    let mut a = Ayni::con_uma(32, 16, Config::SUAVE, Uma::nacer());
    let l = vec![0.5; 32 * 16];
    let img = Imagen { l: &l, ancho: 32, alto: 16 };
    let ex = [Extra { sin_cambios: true, ..Extra::default() }, Extra::default()];
    a.mirar(&img, None, &ex);
    a.decidir(None);
    assert_eq!(a.mapa()[0], Tasa::Cero);
    assert_eq!(a.cuentas.invocaciones, Tasa::X1.invocaciones() as u64, "solo la otra, entera (aprendiz)");
}

#[test]
fn el_borde_de_geometria_solo_engruesa_a_lo_largo() {
    let uma = Uma::nacer();
    let mut a = Ayni::con_uma(16, 16, Config::FUERTE, uma);
    a.uma.lecciones = Config::FUERTE.aprendiz;
    // Lisa (UMA querria 4x4), pero la cruza un borde vertical de geometria.
    let mut l = vec![0.5; 256];
    l[8] = 0.52; // la luz cambia a lo ancho
    let img = Imagen { l: &l, ancho: 16, alto: 16 };
    let ex = [Extra { borde_geometria: true, ..Extra::default() }];
    a.mirar(&img, None, &ex);
    a.decidir(None);
    assert_eq!(a.mapa()[0], Tasa::X1x2);
}

#[test]
fn la_5d_ahorra_lejos_de_la_mano_y_cuida_donde_va() {
    let uma = Uma::nacer();
    // La mano corre y va a parar en el centro del cielo.
    let mano = Prediccion { x: 24.0, y: 40.0, confianza: 0.95, rapidez: 1.0 };
    let (sin, _) = correr(Config::SUAVE, 40, &uma, None);
    let (con, _) = correr(Config::SUAVE, 40, &uma, Some(&mano));
    assert!(con.cuentas.invocaciones < sin.cuentas.invocaciones, "{:?} vs {:?}", con.cuentas, sin.cuentas);
    // Donde va la mano, nunca mas grueso que sin la 5D.
    let foco = (40 / LADO) * con.columnas() + 24 / LADO;
    assert!(con.mapa()[foco].invocaciones() >= sin.mapa()[foco].invocaciones());
    // Sin confianza, la 5D no cambia nada.
    let dudosa = Prediccion { confianza: 0.3, ..mano };
    let (nada, _) = correr(Config::SUAVE, 40, &uma, Some(&dudosa));
    assert_eq!(nada.mapa(), sin.mapa());
}
