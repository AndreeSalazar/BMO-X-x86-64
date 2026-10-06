//! **A9 (06-10): el .bsf vivo, de punta a punta** -- `bmox12.exe` (el BMOX-12
//! de EPICX, sin tocar) DOS veces por la puerta de la 3060, con los `.bsf`
//! en una carpeta del volumen (en BMO-X, la de ESTRATOS:
//! `proton-x/<juego>/bsf`). La primera vez su PSO se traduce, se comprueba
//! bit a bit contra la CPU y se guarda; la segunda (otra puerta: otro
//! arranque) sale del `.bsf`, sin traducir nada, y la receta que va al
//! kernel pega los MISMOS programas.

use super::*;

/// El recuerdo del banco: una carpeta del volumen, como ESTRATOS en BMO-X.
struct EnCarpeta(std::path::PathBuf);

impl bmo_proton_x_sm86::vivo::Recuerdo for EnCarpeta {
    fn leer(&mut self, nombre: &str) -> Option<Vec<u8>> {
        std::fs::read(self.0.join(nombre)).ok()
    }
    fn guardar(&mut self, nombre: &str, bsf: &[u8]) -> bool {
        std::fs::write(self.0.join(nombre), bsf).is_ok()
    }
}

/// Los `.bsf` de la carpeta, con sus bytes.
fn bsfs(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap()).map(|e| (e.file_name().into_string().unwrap(), std::fs::read(e.path()).unwrap())).collect();
    v.sort();
    v
}

#[test]
fn bmox12_exe_la_segunda_vez_su_pso_sale_del_bsf() {
    let uno = uno_a_la_vez();
    let dir = volumen().join("proton-x/bmox12/bsf");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esc = 1 << 8 | 1 << 9 | 0x01;
    let mut guion = vec![0u64; 20];
    guion.push(esc);
    let mut antes: Option<Vec<(String, Vec<u8>)>> = None;
    for vuelta in 0..2 {
        let mut puerta = bmo_proton_x_sm86::puerta::Puerta::nueva();
        puerta.recuerdo = Some(Box::new(EnCarpeta(dir.clone())));
        *LA3060.lock().unwrap() = Some(La3060 { almacen: bmo_proton_x_sm86::pso::Almacen::nuevo(), lotes: 0, vertices: 0, datos_max: 0, fallos: Vec::new(), puerta, recetas: 0, z_limpias: 0, rt_limpios: 0, z_mandadas: 0 });
        let (salio, texto) = correr_bmox12(&uno, true, "", &guion);
        let b = LA3060.lock().unwrap().take().unwrap();
        assert_eq!(salio, 0, "{texto}");
        // Lo de siempre: los programas que pega la puerta son los de
        // `pso::traducir`, y el de vertice da los bits de la casa.
        assert!(b.fallos.is_empty(), "vuelta {vuelta}: {:?}", &b.fallos[..b.fallos.len().min(5)]);
        assert!(b.lotes >= 10 && b.recetas == b.lotes, "vuelta {vuelta}: {} lotes, {} recetas", b.lotes, b.recetas);
        let ahora = bsfs(&dir);
        assert_eq!(ahora.len(), 1, "UN .bsf: el PSO de BMOX-12");
        let (nombre, bytes) = &ahora[0];
        assert!(nombre.len() == 68 && nombre.ends_with(".bsf"), "{nombre}");
        assert!(bmo_bsf::Bsf::parse(bytes).unwrap().module(0).es_mapa(), "su fuente es el mapa de la CPU");
        match vuelta {
            0 => assert_eq!((b.puerta.traducidos, b.puerta.recordados), (1, 0), "la primera vez se traduce (y se comprueba)"),
            _ => {
                assert_eq!((b.puerta.traducidos, b.puerta.recordados), (0, 1), "la segunda, NADA que traducir");
                assert_eq!(antes.as_ref(), Some(&ahora), "y el .bsf no se reescribe");
            }
        }
        antes = Some(ahora);
    }
}

/// Quitar la carpeta de los mapas al salir (tambien si la prueba cae): las
/// demas pruebas compilan como siempre.
struct SinMapas;
impl Drop for SinMapas {
    fn drop(&mut self) {
        bmo_proton_x_casa::enlaces::poner_carpeta(None);
    }
}

/// *** A9b: los MAPAS de la CPU. `bmox12.exe` dos veces con la carpeta de
/// mapas puesta (en BMO-X, `proton-x/<juego>/mapas` de ESTRATOS): la
/// primera compila su PSO y lo guarda cifrado; la segunda no lee el DXIL --
/// lo descifra -- y dibuja los MISMOS fotogramas (las huellas de la 3060).
#[test]
fn bmox12_exe_la_segunda_vez_no_compila_su_dxil() {
    let uno = uno_a_la_vez();
    let _limpio = SinMapas;
    let dir = volumen().join("proton-x/bmox12/mapas");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    bmo_proton_x_casa::enlaces::poner_carpeta(Some(String::from("proton-x/bmox12/mapas")));
    let esc = 1 << 8 | 1 << 9 | 0x01;
    let mut guion = vec![0u64; 70];
    guion.push(esc);
    let mut antes: Option<Vec<(String, Vec<u8>)>> = None;
    for vuelta in 0..2 {
        let (salio, texto) = correr_bmox12(&uno, true, "", &guion);
        assert_eq!(salio, 0, "{texto}");
        assert!(!texto.contains("PROTON-X:"), "ni un aviso de la casa: {texto}");
        let (compilados, recordados) = bmo_proton_x_casa::enlaces::cuentas();
        let vistas = VISTAS.lock().unwrap().clone();
        for (f, esperada) in bmo_cubo::referencia::HUELLAS {
            assert_eq!(vistas[f as usize], esperada, "vuelta {vuelta}, fotograma {f}");
        }
        let ahora = bsfs(&dir);
        assert!(!ahora.is_empty() && ahora.iter().all(|(n, _)| n.ends_with(".mapa")), "{:?}", ahora.iter().map(|x| &x.0).collect::<Vec<_>>());
        assert!(ahora.iter().all(|(_, b)| bmo_proton_x::cifra::descifrar_compilado(b).is_some()), "cada mapa se descifra");
        match vuelta {
            0 => assert_eq!((compilados, recordados), (ahora.len(), 0), "la primera vez se compila (y se guarda)"),
            _ => {
                assert_eq!((compilados, recordados), (0, ahora.len()), "la segunda, NADA que compilar");
                assert_eq!(antes.as_ref(), Some(&ahora), "y los mapas no se reescriben");
            }
        }
        antes = Some(ahora);
    }
}

/// *** A9c: EL VIGIA, el modo dinamico, con un juego de verdad. Se deja en
/// el recuerdo un `.bsf` de BMOX-12 SANO por fuera -- su sobre sellado y su
/// mapa, el de ahora -- pero con el codigo de OTRO sombreador (lo que haria
/// un emisor con un fallo que los numeros de prueba no ven). El modo
/// estatico lo toma TAL CUAL; el vigia, en el PRIMER lote y con los datos
/// del juego, ve que no cuadra: ese lote y los demas van por la CPU (en la
/// 3060 no se dibuja nada mal), y queda la marca `.malo`. El arranque
/// siguiente ni lo intenta.
#[test]
fn bmox12_exe_el_vigia_caza_un_bsf_que_miente() {
    let uno = uno_a_la_vez();
    let dir = volumen().join("proton-x/bmox12/bsf-vigia");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esc = 1 << 8 | 1 << 9 | 0x01;
    let mut guion = vec![0u64; 20];
    guion.push(esc);
    let correr = |uno: &MutexGuard<'static, ()>| -> La3060 {
        let mut puerta = bmo_proton_x_sm86::puerta::Puerta::nueva();
        puerta.recuerdo = Some(Box::new(EnCarpeta(dir.clone())));
        *LA3060.lock().unwrap() = Some(La3060 { almacen: bmo_proton_x_sm86::pso::Almacen::nuevo(), lotes: 0, vertices: 0, datos_max: 0, fallos: Vec::new(), puerta, recetas: 0, z_limpias: 0, rt_limpios: 0, z_mandadas: 0 });
        let (salio, texto) = correr_bmox12(uno, true, "", &guion);
        assert_eq!(salio, 0, "{texto}");
        LA3060.lock().unwrap().take().unwrap()
    };
    // 1: el sano, traducido y revisado en su primer lote: cuadra.
    let b = correr(&uno);
    assert!(b.fallos.is_empty(), "{:?}", &b.fallos[..b.fallos.len().min(3)]);
    assert_eq!((b.puerta.traducidos, b.puerta.corregidos), (1, 0));
    assert!(b.puerta.revisados >= 1, "el primer lote, revisado");
    // El .bsf que MIENTE: el mismo mapa, el codigo de vertice de otro.
    let (nombre, bueno) = bsfs(&dir).remove(0);
    let sobre = bmo_bsf::Bsf::parse(&bueno).unwrap();
    let mapa = sobre.find(b"vs").unwrap().spirv().unwrap().to_vec();
    let (vs_bueno, ps_bueno) = bmo_proton_x_sm86::vivo::de_bsf(&bueno, &mapa).unwrap();
    let otro = dxil::programa::compilar(&dxil::leer(include_bytes!("../../../proton-x/prueba/textura_vs.dxil")).unwrap()).unwrap();
    let vs_malo = bmo_proton_x_sm86::emitir_con(&otro, 64, bmo_proton_x_sm86::Abi::Registros).unwrap();
    assert_ne!(vs_malo.codigo, vs_bueno.codigo);
    std::fs::write(dir.join(&nombre), bmo_proton_x_sm86::vivo::a_bsf(&mapa, &vs_malo, &ps_bueno)).unwrap();
    // 2: el estatico lo toma; el vigia lo caza en el PRIMER lote.
    let b = correr(&uno);
    assert_eq!((b.puerta.traducidos, b.puerta.recordados, b.puerta.corregidos), (0, 1, 1));
    assert_eq!(b.recetas, 0, "ni un lote a la 3060 con el codigo que miente");
    assert!(b.fallos.iter().all(|f| f.contains("el vigia, con los datos del juego") && f.contains("en la 3060 y")), "{:?}", &b.fallos[..b.fallos.len().min(3)]);
    let marca = dir.join(bmo_proton_x_sm86::vivo::malo(&nombre));
    let motivo = String::from_utf8(std::fs::read(&marca).expect("la marca .malo")).unwrap();
    assert!(motivo.starts_with("el de vertice, con el vertice ") && motivo.contains("en la CPU"), "{motivo}");
    // 3: el arranque siguiente ni lo intenta: ni lee el .bsf ni traduce.
    let b = correr(&uno);
    assert_eq!((b.puerta.traducidos, b.puerta.recordados, b.recetas), (0, 0, 0));
    assert!(b.fallos.iter().all(|f| f.contains("una revision con los datos del juego lo marco malo")), "{:?}", &b.fallos[..b.fallos.len().min(3)]);
}

/// **9d -- LO RARO de un render target** (06-10, `prueba/libreta.exe`,
/// nuestro): un dibujo normal y uno RARO (su de pixeles saca +inf, NaN y
/// -inf) a un RGBA8 de 1280 x 720; D3D los guarda 1, 0 y 0, y la imagen
/// entera es la que pide. En el banco va por la CPU (aqui no hay 3060 ni
/// libreta: el termometro y su apunte los juzgan `proton-x-sm86` y
/// `bmo-gpu-ga10x`); en el metal, el dibujo raro lo apunta la libreta.
#[test]
fn lo_raro_de_un_render_target_se_guarda_como_en_windows() {
    let uno = uno_a_la_vez();
    let (salio, dicho, _) = correr_exe(&uno, LIBRETA_EXE, true, &[]);
    let texto = format!("{}[salio {salio:#x}]", String::from_utf8(dicho).unwrap());
    assert!(!texto.contains("  MAL   "), "{texto}");
    assert!(!texto.lines().any(|l| l.starts_with("PROTON-X:")), "ni un aviso: {texto}");
    assert_eq!(texto.matches("  bien  ").count(), 4, "{texto}");
    assert!(texto.contains("  bien  D, la imagen entera: 0 pixeles distintos"), "{texto}");
    assert!(texto.ends_with("libreta.exe: lo raro de un render target se guarda como en Windows\r\n[salio 0x0]"), "{texto}");
}
