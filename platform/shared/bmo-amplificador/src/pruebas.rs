//! Las pruebas del amplificador. Todo es entero, asi que **todo se puede
//! comprobar contra un numero exacto**: no hay "aproximadamente" salvo donde
//! se dice, y ahi se dice cuanto.

extern crate std;
use super::*;
use std::vec::Vec;

// ===================================================================
//  La ganancia
// ===================================================================

#[test]
fn cero_db_no_toca_nada() {
    let g = Ganancia::db(0);
    assert_eq!(g.factor_q16(), 1 << 16);
    for x in [-32768, -1000, 0, 1, 1000, 32767] {
        assert_eq!(g.aplicar(x), x, "x = {}", x);
    }
}

#[test]
fn mas_seis_db_es_el_doble_y_menos_seis_la_mitad() {
    // No es exacto: +6 dB son x1,9953. Lo que se comprueba es que el error
    // esta por debajo del 0,3 %, que es lo que dice la tabla.
    let doble = Ganancia::db_entero(6);
    assert_eq!(doble.aplicar(10_000), 19_952);
    let mitad = Ganancia::db_entero(-6);
    assert_eq!(mitad.aplicar(10_000), 5_011);
    // Y el exacto de verdad: +6,0206 dB.
    let exacto = Ganancia::db(6 * DB + 5);
    assert!((19_990..=20_010).contains(&exacto.aplicar(10_000)));
}

#[test]
fn la_tabla_de_db_cuadra_con_la_formula() {
    // 10^(n/20) x 65536, calculado aparte. Si alguien toca la tabla, esto cae.
    const ESPERADO: [u32; 25] = [
        65536, 73533, 82505, 92572, 103868, 116541, 130762, 146717, 164619, 184706,
        207243, 232531, 260904, 292739, 328458, 368536, 413504, 463959, 520571,
        584090, 655360, 735326, 825049, 925721, 1038676,
    ];
    for n in 0..=24 {
        let g = Ganancia::db_entero(n as i32);
        let e = ESPERADO[n];
        let d = g.factor_q16().abs_diff(e);
        // Una parte en mil, que es lo que deja la interpolacion.
        assert!(d * 1000 <= e, "n={} dB: {} contra {}", n, g.factor_q16(), e);
    }
}

#[test]
fn subir_y_bajar_lo_mismo_devuelve_lo_mismo() {
    // f(+x) * f(-x) tiene que ser 1, y el error acumulado de las dos
    // conversiones no puede pasar del 0,2 %.
    for db in [1, 3, 6, 10, 12, 17, 24] {
        let sube = Ganancia::db_entero(db).factor_q16() as u64;
        let baja = Ganancia::db_entero(-db).factor_q16() as u64;
        let ida_y_vuelta = (sube * baja) >> 16;
        let d = ida_y_vuelta.abs_diff(1 << 16);
        assert!(d * 500 <= (1 << 16), "{} dB: {} contra 65536", db, ida_y_vuelta);
    }
}

#[test]
fn la_ganancia_sube_siempre_que_se_le_pide_mas() {
    // Monotona en TODO el recorrido, de 1/256 en 1/256: una curva de volumen
    // que baja en algun punto es un mando que da saltos raros.
    let mut antes = 0;
    let mut db = MIN_DB + 1;
    while db <= MAX_DB {
        let f = Ganancia::db(db).factor_q16();
        assert!(f >= antes, "en {} (1/256 dB) bajo: {} tras {}", db, f, antes);
        antes = f;
        db += 1;
    }
}

#[test]
fn lo_que_no_cabe_se_recorta_y_se_dice() {
    let g = Ganancia::db_entero(40);
    assert!(g.se_recorto());
    assert_eq!(g.en_db(), MAX_DB);
    // Y lo que si cabe, no miente.
    let ok = Ganancia::db_entero(12);
    assert!(!ok.se_recorto());
    assert_eq!(ok.en_db(), 12 * DB);
}

#[test]
fn el_silencio_es_cero_y_no_un_susurro() {
    let g = Ganancia::SILENCIO;
    assert!(g.es_muda());
    assert_eq!(g.aplicar(32767), 0);
    assert_eq!(g.aplicar(-32768), 0);
    // -96 dB por la puerta normal tambien calla.
    assert!(Ganancia::db(MIN_DB).es_muda());
}

#[test]
fn el_mando_de_porcentaje_dobla_en_doscientos() {
    assert!(Ganancia::porcentaje(0).es_muda());
    assert_eq!(Ganancia::porcentaje(100).factor_q16(), 1 << 16);
    // 200 % = +6,02 dB = x2, con el margen de la conversion.
    let doble = Ganancia::porcentaje(200).aplicar(10_000);
    assert!((19_900..=20_100).contains(&doble), "200 % dio {}", doble);
    let mitad = Ganancia::porcentaje(50).aplicar(10_000);
    assert!((4_950..=5_050).contains(&mitad), "50 % dio {}", mitad);
    let cuadruple = Ganancia::porcentaje(400).aplicar(5_000);
    assert!((19_800..=20_200).contains(&cuadruple), "400 % dio {}", cuadruple);
}

#[test]
fn una_ganancia_absurda_no_desborda() {
    // El caso hostil: la ganancia mas alta sobre la muestra mas negativa.
    let g = Ganancia::db(MAX_DB);
    let y = g.aplicar(i16::MIN as i32);
    assert!(y < 0 && y > -600_000, "dio {}", y);
    // Y sobre un acumulador que ya venia grande.
    let y = g.aplicar(i32::MAX / 32);
    assert!(y > 0, "dio {}", y);
}

// ===================================================================
//  dBFS
// ===================================================================

#[test]
fn el_pleno_es_cero_dbfs_y_la_mitad_son_seis_menos() {
    assert_eq!(a_dbfs(32767), 0);
    let mitad = a_dbfs(16384);
    // -6,02 dB = -1541 en 1/256. Medio dB de margen por el log entero.
    assert!((-1_700..=-1_400).contains(&mitad), "la mitad dio {}", mitad);
    let decima = a_dbfs(3277);
    assert!((-5_300..=-4_900).contains(&decima), "un decimo dio {}", decima);
    // El cero es el suelo, no "casi cero".
    assert_eq!(a_dbfs(0), MIN_DB);
    // Y el signo no cuenta: -32767 suena igual de fuerte que +32767.
    assert_eq!(a_dbfs(-32767), 0);
}

// ===================================================================
//  La suma: LA BASE
// ===================================================================

#[test]
fn sumar_dos_fuentes_es_sumar_sus_muestras() {
    let mut acc = [0i32; 4];
    sumar(&mut acc, &[1000, -2000, 3000, 0], Ganancia::UNIDAD);
    sumar(&mut acc, &[500, 500, 500, 500], Ganancia::UNIDAD);
    assert_eq!(acc, [1500, -1500, 3500, 500]);
}

#[test]
fn sumar_ocho_fuentes_a_tope_no_da_la_vuelta() {
    // El fallo clasico: sumar en 16 bits. Aqui el acumulador es de 32 y la
    // suma de ocho plenos tiene que estar ENTERA, para que el limite decida.
    let mut acc = [0i32; 2];
    for _ in 0..8 {
        sumar(&mut acc, &[32767, -32768], Ganancia::UNIDAD);
    }
    assert_eq!(acc[0], 32767 * 8);
    assert_eq!(acc[1], -32768 * 8);
}

#[test]
fn sumar_con_silencio_no_toca_el_acumulador() {
    let mut acc = [7i32; 3];
    sumar(&mut acc, &[1000, 1000, 1000], Ganancia::SILENCIO);
    assert_eq!(acc, [7, 7, 7]);
}

#[test]
fn sumar_se_para_en_el_mas_corto_de_los_dos() {
    let mut acc = [0i32; 2];
    sumar(&mut acc, &[100, 200, 300, 400], Ganancia::UNIDAD);
    assert_eq!(acc, [100, 200]);
    let mut acc = [0i32; 4];
    sumar(&mut acc, &[100], Ganancia::UNIDAD);
    assert_eq!(acc, [100, 0, 0, 0]);
    // Y con listas vacias no explota.
    sumar(&mut [], &[1, 2, 3], Ganancia::UNIDAD);
    sumar(&mut [0; 3], &[], Ganancia::UNIDAD);
}

#[test]
fn la_mezcla_de_cinco_uno_a_estereo_es_la_misma_pieza() {
    // La prueba de que el amplificador ES la base: bajar 5.1 a estereo no
    // necesita codigo nuevo, solo ganancias (0,707 = -3,01 dB) y sumas.
    let tres_db = Ganancia::db(-3 * DB - 3); // -3,01 dB
    let (l, r, c, lfe, ls, rs) = (10_000i16, 8_000, 6_000, 4_000, 2_000, 1_000);
    let mut izq = [0i32; 1];
    let mut der = [0i32; 1];
    sumar(&mut izq, &[l], Ganancia::UNIDAD);
    sumar(&mut der, &[r], Ganancia::UNIDAD);
    for x in [c, lfe] {
        sumar(&mut izq, &[x], tres_db);
        sumar(&mut der, &[x], tres_db);
    }
    sumar(&mut izq, &[ls], tres_db);
    sumar(&mut der, &[rs], tres_db);
    // L + 0,707*(C + LFE + LS) = 10.000 + 0,707*12.000 = 18.485
    assert!((18_400..=18_560).contains(&izq[0]), "izquierda dio {}", izq[0]);
    // R + 0,707*(C + LFE + RS) = 8.000 + 0,707*11.000 = 15.777
    assert!((15_700..=15_860).contains(&der[0]), "derecha dio {}", der[0]);
}

// ===================================================================
//  El limite
// ===================================================================

#[test]
fn nada_sale_nunca_del_techo() {
    let mut lim = Limite::nuevo(48_000);
    let mut peor = 0i32;
    // Una onda que se pasa cuatro veces del pleno, mil muestras.
    for i in 0..1000 {
        let x = if i % 2 == 0 { 130_000 } else { -130_000 };
        let y = lim.muestra(x);
        peor = peor.max((y as i32).abs());
    }
    assert!(peor <= PLENO, "salio {}", peor);
    assert!(lim.sujetadas() > 0, "no sujeto nada");
}

#[test]
fn el_limite_baja_la_ganancia_en_vez_de_recortar() {
    // Esta es la diferencia con un recortador, y se mide: tras el ataque, las
    // muestras que hay que doblegar a pelo tienen que ser MUY POCAS.
    let mut lim = Limite::nuevo(48_000);
    let mut dobladas_al_final = 0;
    for i in 0..48_000 {
        let x = if i % 100 < 50 { 100_000 } else { -100_000 };
        lim.muestra(x);
        if i == 47_000 {
            dobladas_al_final = lim.dobladas();
        }
    }
    // En el ultimo segundo, con la envolvente ya asentada, casi ninguna.
    let en_el_ultimo_tramo = lim.dobladas() - dobladas_al_final;
    assert!(en_el_ultimo_tramo <= 2, "doblego {} en el ultimo tramo", en_el_ultimo_tramo);
    // Y la reduccion que hay puesta se puede decir en dB, que es lo que ve
    // el propietario en el save.
    assert!(lim.reduccion_db() < 0, "no hay reduccion que contar");
}

#[test]
fn una_onda_que_cabe_sale_intacta() {
    // Un limitador que toca lo que no hace falta es un limitador que colorea.
    let mut lim = Limite::nuevo(48_000);
    let entrada: Vec<i32> = (0..500).map(|i| ((i * 37) % 20_000) - 10_000).collect();
    let mut salida = [0i16; 500];
    lim.bloque(&entrada, &mut salida);
    for i in 0..500 {
        assert_eq!(salida[i] as i32, entrada[i], "muestra {}", i);
    }
    assert_eq!(lim.sujetadas(), 0);
    assert_eq!(lim.dobladas(), 0);
    assert_eq!(lim.reduccion_db(), 0);
}

#[test]
fn el_techo_se_puede_bajar_a_menos_un_dbfs() {
    // -1 dBFS = 29.204. Es lo que se pone cuando detras hay algo que no
    // quiere recibir plenos.
    let mut lim = Limite::con_techo(29_204, 48_000);
    let mut peor = 0i32;
    for _ in 0..2000 {
        peor = peor.max((lim.muestra(60_000) as i32).abs());
    }
    assert!(peor <= 29_204, "salio {}", peor);
}

#[test]
fn tras_el_golpe_la_reduccion_vuelve() {
    // Si no volviera, el primer golpe dejaria toda la cancion baja.
    let mut lim = Limite::nuevo(48_000);
    for _ in 0..200 {
        lim.muestra(200_000);
    }
    let durante = lim.reduccion_db();
    assert!(durante < -1000, "no bajo bastante: {}", durante);
    // Medio segundo de silencio.
    for _ in 0..24_000 {
        lim.muestra(0);
    }
    assert_eq!(lim.reduccion_db(), 0, "no volvio del todo");
}

// ===================================================================
//  El medidor
// ===================================================================

#[test]
fn el_medidor_dice_el_pico_y_el_rms() {
    let mut m = Medidor::nuevo();
    m.mirar(&[0, 100, -32767, 50]);
    assert_eq!(m.pico(), 32767);
    assert_eq!(m.pico_dbfs(), 0);
    assert_eq!(m.muestras(), 4);
    // Una onda cuadrada a pleno: RMS = pico, o sea 0 dBFS.
    let mut m = Medidor::nuevo();
    let cuadrada: Vec<i16> = (0..1000).map(|i| if i % 2 == 0 { 32767 } else { -32767 }).collect();
    m.mirar(&cuadrada);
    assert_eq!(m.rms_dbfs(), 0);
    // El silencio es el suelo, no un numero cualquiera.
    let mut m = Medidor::nuevo();
    m.mirar(&[0; 100]);
    assert_eq!(m.pico_dbfs(), MIN_DB);
    assert_eq!(m.rms_dbfs(), MIN_DB);
    // Y sin nada que mirar, tampoco inventa.
    assert_eq!(Medidor::nuevo().rms_dbfs(), MIN_DB);
}

#[test]
fn el_rms_distingue_una_cancion_floja_de_una_fuerte() {
    // Dos ondas con el MISMO pico y distinta fuerza: es justo lo que el pico
    // no sabe decir y por lo que hace falta el RMS.
    let mut fuerte = Medidor::nuevo();
    fuerte.mirar(&(0..1000).map(|i| if i % 2 == 0 { 30_000 } else { -30_000 }).collect::<Vec<_>>());
    let mut floja = Medidor::nuevo();
    floja.mirar(&(0..1000).map(|i| if i == 0 { 30_000 } else { 300 }).collect::<Vec<_>>());
    assert_eq!(fuerte.pico(), floja.pico());
    assert!(fuerte.rms_dbfs() > floja.rms_dbfs() + 25 * DB,
            "fuerte {} contra floja {}", fuerte.rms_dbfs(), floja.rms_dbfs());
}

// ===================================================================
//  Las tres juntas
// ===================================================================

#[test]
fn subir_doce_db_una_cancion_floja_la_deja_alta_y_entera() {
    // El caso del propietario, entero: una grabacion floja (-20 dBFS), +12 dB, y
    // el resultado tiene que oirse mucho mas alto SIN que el limite tenga que
    // doblegar nada, porque -20 + 12 = -8 dBFS y eso cabe de sobra.
    let mut amp = Amplificador::nuevo(48_000);
    amp.subir(12 * DB);
    let floja: Vec<i32> = (0..4800)
        .map(|i| if (i / 24) % 2 == 0 { 3_276 } else { -3_276 })
        .collect();
    let mut salida = [0i16; 4800];
    amp.bloque(&floja, &mut salida);
    let pico = amp.medidor.pico_dbfs();
    // -20 dBFS + 12 dB = -8 dBFS, o sea unas -2.050 milesimas.
    assert!((-2_200..=-1_900).contains(&pico), "quedo en {} (1/256 dB)", pico);
    assert_eq!(amp.limite.dobladas(), 0, "doblego sin necesidad");
    assert_eq!(amp.limite.sujetadas(), 0, "sujeto sin necesidad");
}

#[test]
fn subir_veinte_db_una_cancion_que_ya_estaba_alta_lo_confiesa() {
    // La ley del crate: si hay que sujetar, se cuenta. Aqui se pide un
    // imposible (-3 dBFS + 20 dB) y el amplificador no finge.
    let mut amp = Amplificador::nuevo(48_000);
    amp.subir(20 * DB);
    let alta: Vec<i32> = (0..4800)
        .map(|i| if (i / 24) % 2 == 0 { 23_000 } else { -23_000 })
        .collect();
    let mut salida = [0i16; 4800];
    amp.bloque(&alta, &mut salida);
    assert!(amp.limite.sujetadas() > 0, "no conto nada, y tuvo que sujetar");
    assert!(amp.limite.reduccion_db() < 0);
    // Pero lo que sale sigue cabiendo, que es lo que el aparato necesita.
    assert!(amp.medidor.pico() <= PLENO);
}

#[test]
fn en_el_sitio_hace_lo_mismo_que_por_bloque() {
    let entrada: Vec<i16> = (0..256).map(|i| ((i * 211) % 12_000) as i16 - 6_000).collect();
    let mut a = Amplificador::nuevo(48_000);
    a.subir(6 * DB);
    let mut por_bloque = [0i16; 256];
    let como_32: Vec<i32> = entrada.iter().map(|&x| x as i32).collect();
    a.bloque(&como_32, &mut por_bloque);

    let mut b = Amplificador::nuevo(48_000);
    b.subir(6 * DB);
    let mut en_sitio = entrada.clone();
    b.en_el_sitio(&mut en_sitio);

    assert_eq!(&por_bloque[..], &en_sitio[..]);
    assert_eq!(a.medidor.pico(), b.medidor.pico());
}

#[test]
fn con_listas_vacias_no_hace_nada_y_no_explota() {
    let mut amp = Amplificador::nuevo(48_000);
    amp.bloque(&[], &mut []);
    amp.en_el_sitio(&mut []);
    assert_eq!(amp.medidor.muestras(), 0);
    // Y una frecuencia absurda tampoco lo rompe: el limite se protege.
    let mut raro = Amplificador::nuevo(0);
    raro.bloque(&[100_000], &mut [0i16; 1]);
    assert!(raro.medidor.pico() <= PLENO);
}
