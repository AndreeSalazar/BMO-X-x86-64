//! Las pruebas de P3c3: el SM5 de FXC (los `.cso` del cubo de BMOX-12, que
//! `sombras.exe` compilo en el Windows del propietario) por el MISMO lote.

use alloc::vec::Vec;

use crate::dxil::programa::{compilar, NoPrograma, Op};
use crate::dxil::{self, Etapa};
use crate::pruebas::{cb_de, cubo_con};

const SM5_VS: &[u8] = include_bytes!("../prueba/sombras/f3ef42a0.cso");
const SM5_PS: &[u8] = include_bytes!("../prueba/sombras/4d67f5e4.cso");

/// Los `.cso` son los que la casa BUSCA: su nombre es la huella del pedido del
/// cubo (lo comprueba `pruebas_windows`), y dentro va un SM 5.0 de FXC.
#[test]
fn los_cso_del_cubo_se_leen_como_sm5() {
    let vs = dxil::leer(SM5_VS).unwrap();
    assert_eq!((vs.etapa, vs.modelo), (Etapa::Vertice, (5, 0)));
    assert_eq!(vs.partes, [*b"RDEF", *b"ISGN", *b"OSGN", *b"SHEX", *b"STAT"]);
    let nombres = |v: &[dxil::Elemento]| v.iter().map(|e| (e.semantica.clone(), e.registro, e.mascara)).collect::<Vec<_>>();
    assert_eq!(nombres(&vs.entradas), [("POSITION".into(), 0, 7), ("NORMAL".into(), 1, 7), ("COLOR".into(), 2, 15)]);
    assert_eq!(nombres(&vs.salidas), [("SV_POSITION".into(), 0, 15), ("NORMAL".into(), 1, 7), ("COLOR".into(), 2, 15)]);
    assert_eq!(vs.salidas[0].sistema, 1);
    let ps = dxil::leer(SM5_PS).unwrap();
    assert_eq!((ps.etapa, ps.modelo), (Etapa::Pixel, (5, 0)));
    assert_eq!(ps.salidas.len(), 1);
    assert_eq!(ps.sm5.as_ref().map(|t| t.len()), Some(308 / 4));
}

/// **El de vertices de FXC, corrido, da BIT A BIT las cuentas del juez** --
/// como el DXIL de dxc: FXC ordena distinto (`y*c1` primero, luego `x*c0 +`)
/// pero una suma de dos es la misma en los dos ordenes.
#[test]
fn el_sm5_de_vertices_da_las_cuentas_del_juez() {
    let p = compilar(&dxil::leer(SM5_VS).unwrap()).unwrap();
    assert_eq!((p.entradas, p.salidas, p.filas_cb), (3, 3, 7));
    // 9 instrucciones de FXC (STAT): 4 mul, 4 mad, 1 add sobre vectores.
    assert_eq!(p.ops.iter().filter(|o| matches!(o, Op::Mad { .. })).count(), 4 * 2 + 3 * 2);
    let mut regs = Vec::new();
    for f in [0u32, 30, 60, 123] {
        let cb = cb_de(f);
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        for v in bmo_cubo::vertices() {
            let e = [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color];
            let mut s = [[0.0f32; 4]; 3];
            p.correr(&e, &cb, &mut s, &mut regs);
            let bits = |x: &[f32]| x.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
            assert_eq!(bits(&s[0]), bits(&bmo_cubo::mat::transformar(&c.wvp, [v.pos[0], v.pos[1], v.pos[2], 1.0])), "SV_Position, fotograma {f}");
            assert_eq!(bits(&s[1][..3]), bits(&bmo_cubo::mat::transformar_dir(&c.world, v.normal)), "NORMAL, fotograma {f}");
            assert_eq!(s[2], v.color);
        }
    }
}

/// **El de pixeles de FXC ilumina como el juez**: `dp3`, `rsq`, el `dp3_sat`,
/// y el `1 - Luz.w` que FXC escribe con una fuente NEGADA (`-cb0[8].w`).
#[test]
fn el_sm5_de_pixeles_ilumina_como_el_juez() {
    let p = compilar(&dxil::leer(SM5_PS).unwrap()).unwrap();
    assert_eq!((p.salidas, p.filas_cb, p.lee), (1, 9, 0b110), "NORMAL y COLOR; SV_Position no");
    assert_eq!(p.ops.iter().filter(|o| matches!(o, Op::Saturate { .. })).count(), 1, "el _sat del dp3");
    let mut regs = Vec::new();
    for f in [0u32, 30, 60] {
        let cb = cb_de(f);
        let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
        for v in bmo_cubo::vertices() {
            let n = bmo_cubo::mat::transformar_dir(&c.world, v.normal);
            let e = [[0.0; 4], [n[0], n[1], n[2], 0.0], v.color];
            let mut s = [[0.0f32; 4]; 1];
            p.correr(&e, &cb, &mut s, &mut regs);
            let juez = bmo_cubo::iluminar(v.color, n, c.luz);
            assert_eq!(bmo_cubo::empaquetar(s[0]), bmo_cubo::empaquetar(juez), "fotograma {f}");
        }
    }
}

/// *** P3c3: EL SM5 QUE FXC COMPILO PARA BMOX-12, CORRIDO EN LA CPU POR EL
/// MISMO LOTE, DIBUJA LO QUE D3D12 DIBUJO EN LA 3060: las huellas de los
/// fotogramas 0, 30 y 60, bit a bit.
#[test]
fn el_sm5_de_fxc_y_la_trama_dan_las_huellas_de_d3d12() {
    for (f, esperada) in bmo_cubo::referencia::HUELLAS {
        let (px, cuenta) = cubo_con(SM5_VS, SM5_PS, f, false, 3);
        assert_eq!(bmo_cubo::referencia::huella(&px), esperada, "fotograma {f}: {cuenta:?}");
        assert_eq!((cuenta.dibujados + cuenta.descartados, cuenta.sin_recortar), (12, 0));
    }
}

/// Lo que no se sabe se DICE, con su numero: una instruccion cambiada por un
/// `exp` (25) no se salta callada. (Hasta E6 era un `movc`, que ya se sabe.)
#[test]
fn una_instruccion_sm5_que_no_se_sabe_se_dice() {
    let mut d = SM5_PS.to_vec();
    // El `rsq` (0x44, 5 palabras) -> `exp` (0x19).
    let o = d.windows(4).position(|w| w == [0x44, 0, 0, 0x05]).unwrap();
    d[o] = 0x19;
    assert_eq!(compilar(&dxil::leer(&d).unwrap()), Err(NoPrograma::Sm5(25)));
    // Un SHEX cortado tampoco se lee.
    let mut d = SM5_VS.to_vec();
    let o = d.windows(4).position(|w| w == b"SHEX").unwrap();
    d[o + 12..o + 16].copy_from_slice(&0xFFFFu32.to_le_bytes());
    assert!(dxil::leer(&d).is_err());
}

// -- P3c4: la profundidad de BMOX-12 ----------------------------------------

/// *** El cubo SIN descartar caras y CON la profundidad de BMOX-12 (D32
/// borrado a 1.0, LESS, escrita): las caras de detras se tapan por z, no por
/// su sentido, y sale la imagen de la 3060 -- salvo algun pixel de la
/// SILUETA, donde una cara de delante y una de detras comparten arista y
/// tienen la MISMA z: ahi decide el redondeo (tambien en una GPU; por eso el
/// cubo descarta). Se cuentan: como mucho 2 por fotograma, y ni uno sin la
/// arista compartida. Sin la profundidad, las de detras pintan encima.
#[test]
fn sin_descarte_la_profundidad_tapa_las_caras_de_detras() {
    for (f, esperada) in bmo_cubo::referencia::HUELLAS {
        let (px, cuenta) = cubo_con(SM5_VS, SM5_PS, f, true, 1);
        let (juez, _) = cubo_con(SM5_VS, SM5_PS, f, false, 3);
        assert_eq!(bmo_cubo::referencia::huella(&juez), esperada);
        assert_eq!((cuenta.dibujados, cuenta.descartados), (12, 0), "sin descarte se dibujan los 12");
        assert!(cuenta.tapados > 10_000, "{cuenta:?}");
        let distintos = (0..px.len()).filter(|&i| px[i] != juez[i]).count();
        assert!(distintos <= 2, "fotograma {f}: {distintos} pixeles distintos");
        // Con descarte y profundidad: la huella exacta, y nada se tapa (convexo).
        let (px, cuenta) = cubo_con(SM5_VS, SM5_PS, f, true, 3);
        assert_eq!(bmo_cubo::referencia::huella(&px), esperada);
        assert_eq!(cuenta.tapados, 0);
    }
    // Sin la profundidad ni el descarte, las de detras pintan encima: miles.
    let f = bmo_cubo::referencia::HUELLAS[0].0;
    let (a, b) = (cubo_con(SM5_VS, SM5_PS, f, false, 1).0, cubo_con(SM5_VS, SM5_PS, f, false, 3).0);
    assert!((0..a.len()).filter(|&i| a[i] != b[i]).count() > 10_000);
}

/// Las funciones de comparacion, una a una.
#[test]
fn la_prueba_de_profundidad_compara_como_d3d12() {
    use crate::trama::Profundidad;
    let p = |funcion| Profundidad { funcion, escribir: true };
    let casos = [(1, [false, false, false]), (2, [true, false, false]), (3, [false, true, false]), (4, [true, true, false]), (5, [false, false, true]), (6, [true, false, true]), (7, [false, true, true]), (8, [true, true, true])];
    for (f, esperado) in casos {
        assert_eq!([p(f).pasa(0.25, 0.5), p(f).pasa(0.5, 0.5), p(f).pasa(0.75, 0.5)], esperado, "funcion {f}");
    }
}


// -- El registro de lo que se dibuja ------------------------------------------

/// Un segundo a 10 fps (cada fotograma 100 ms: 80 dibujando, 5 presentando)
/// da UNA linea, con lo que paso; el siguiente segundo empieza de cero.
#[test]
fn el_registro_da_una_linea_por_segundo_con_lo_que_paso() {
    use crate::registro::Registro;
    let mut r = Registro::default();
    let ms = 1_000_000u64;
    assert_eq!(r.presente(0, 5 * ms), None, "el primero abre la cuenta");
    let mut lineas = Vec::new();
    for k in 1..=25u64 {
        r.dibujo(80 * ms);
        // el 5 tarda el doble: el maximo lo dice, y el medio (9 en 1000 ms) es 111
        let t = k * 100 * ms + if k >= 5 { 100 * ms } else { 0 };
        if let Some(l) = r.presente(t, 5 * ms) {
            lineas.push(l);
        }
    }
    assert_eq!(lineas.len(), 2, "{lineas:?}");
    assert_eq!(lineas[0], "[registro] 9 fps  fotograma 100/111/200 ms  dibujar 80 ms  presentar 5 ms  (fotogramas 0..9)\n");
    assert_eq!(lineas[1], "[registro] 10 fps  fotograma 100/100/100 ms  dibujar 80 ms  presentar 5 ms  (fotogramas 10..19)\n");
}

/// *** SM5 con TEXTURA (29-09): `sample o0.xyzw, v1.xyxx, t0.xyzw, s0`, como
/// lo pone FXC para `imagen.Sample(muestreo, uv)` en `ps_5_0`, armado token
/// a token: una `Muestra` de t0 con s0 en (v1.x, v1.y), y el swizzle del
/// recurso reparte los canales (`t0.zyxw`: R y B cambiados).
#[test]
fn sample_de_sm5_muestrea() {
    use alloc::vec;
    use crate::dxil::Elemento;
    use crate::textura::{Direccion, Filtro, Muestreador, Recursos, Textura};
    let el = |registro: u32, mascara: u8| Elemento { semantica: alloc::string::String::new(), indice: 0, sistema: 0, tipo: 3, registro, mascara };
    let entradas = [el(0, 0xF), el(1, 0x3)];
    let salidas = [el(0, 0xF)];
    // Un operando de 4 componentes, de tipo `tipo`, registro `r`.
    let mascara = |tipo: u32| 2 | 0xF << 4 | tipo << 12 | 1 << 20;
    let swz = |tipo: u32, s: [u32; 4]| 2 | 1 << 2 | s[0] << 4 | s[1] << 6 | s[2] << 8 | s[3] << 10 | tipo << 12 | 1 << 20;
    let programa = |sel_t0: [u32; 4]| -> Vec<u32> {
        let mut t = vec![0x50, 0];
        t.extend([88 | 3 << 11 | 4 << 24, 7 << 12 | 1 << 20, 0, 0x5555]); // dcl_resource_texture2d t0
        t.extend([90 | 3 << 24, 6 << 12 | 1 << 20, 0]); // dcl_sampler s0
        t.extend([69 | 9 << 24, mascara(2), 0, swz(1, [0, 1, 0, 0]), 1, swz(7, sel_t0), 0, 6 << 12 | 1 << 20, 0]);
        t.push(62 | 1 << 24); // ret
        t[1] = t.len() as u32;
        t
    };
    let tx: Vec<u32> = vec![0xFF30_2010, 0xFF60_5040, 0xFF90_8070, 0xFFC0_B0A0];
    let tex = [Some(Textura::rgba(&tx, 2, 2, false))];
    let m = [Some(Muestreador { filtro: Filtro::Punto, u: Direccion::Repetir, v: Direccion::Repetir, borde: [0.0; 4] })];
    let rec = Recursos { texturas: &tex, muestreadores: &m };
    let p = crate::sm5::compilar(&programa([0, 1, 2, 3]), &entradas, &salidas).unwrap();
    assert_eq!(p.ops.iter().filter(|o| matches!(o, Op::Muestra { t: 0, s: 0, .. })).count(), 1);
    let (mut sal, mut regs) = (vec![[0f32; 4]; 1], Vec::new());
    p.correr_con(&[[0.0; 4], [0.75, 0.25, 0.0, 0.0]], &[], &rec, &mut sal, &mut regs);
    let esperado = tex[0].unwrap().muestrear(&m[0].unwrap(), 0.75, 0.25);
    assert_eq!(sal[0], esperado);
    let q = crate::sm5::compilar(&programa([2, 1, 0, 3]), &entradas, &salidas).unwrap();
    q.correr_con(&[[0.0; 4], [0.75, 0.25, 0.0, 0.0]], &[], &rec, &mut sal, &mut regs);
    assert_eq!(sal[0], [esperado[2], esperado[1], esperado[0], esperado[3]], "t0.zyxw");
    // Una textura 3D, o un muestreador de comparacion: se dice.
    let mut malo = programa([0, 1, 2, 3]);
    malo[2] = 88 | 4 << 11 | 4 << 24;
    assert!(matches!(crate::sm5::compilar(&malo, &entradas, &salidas), Err(NoPrograma::Forma(_))));
    let mut comp = programa([0, 1, 2, 3]);
    comp[6] = 90 | 1 << 11 | 3 << 24;
    assert!(matches!(crate::sm5::compilar(&comp, &entradas, &salidas), Err(NoPrograma::Forma(_))));
}
