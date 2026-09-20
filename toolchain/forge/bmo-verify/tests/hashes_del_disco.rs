//! **Comprueba los hashes de seccion de los `.bex` que se van a desplegar.**
//!
//! === Por que esto es una prueba y no un script suelto ===
//!
//! El 2026-08-11, `doom.bex` no pasaba la admision en el Ryzen y el kernel no
//! decia por que --diez caminos de fallo hablaban por el klog, que con el
//! compositor en pantalla no se lee--. La pregunta *"es la firma o no?"* se
//! podia contestar **en el anfitrion, en un segundo**, y en vez de eso costo
//! varios arranques.
//!
//! > **Si una pregunta sobre un fichero se puede contestar sin encender la
//! > maquina, encender la maquina para contestarla es tiempo tirado.**
//!
//! Lo que hace: por cada `.bex` de `staging`, abre la imagen con el juez de
//! BEF2, que comprueba **cada digest contra los bytes reales de su region o
//! anexo** -- exactamente lo que hace `aterrizaje` en Ring 0 al cargar. Si aqui cuadra y
//! en metal no, el problema es el transporte; si aqui no cuadra, el fichero
//! sale mal del build y no hay que mirar el kernel.
//!
//! [!] No falla si no hay `staging`: es un artefacto del build, no del
//! repositorio, y una prueba que exige artefactos rompe el `cargo test` de
//! quien acaba de clonar.

use std::path::{Path, PathBuf};

/// `Err(motivo)` en el primer digest que no cuadre; `Ok(cuantos)` si cuadran.
///
/// ** BEF2 (2026-09-19): el juez del contrato comprueba cada hash contra sus
/// bytes al abrir la imagen; si abre, cuadran. Lo que se cuenta es lo que
/// declara el anexo de firma.
fn comprobar(d: &[u8]) -> Result<usize, String> {
    let v = bmo_abi::bef2::leer(d).map_err(|f| format!("BEF2: {}", f.nombre()))?;
    let firma = v.anexo(bmo_abi::bef2::ANEXO_FIRMA).ok_or("BEF2 sin anexo de firma")?;
    Ok(u32::from_le_bytes(firma[..4].try_into().unwrap()) as usize)
}

fn recoger(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            recoger(&p, out);
        } else if p.extension().map(|x| x == "bex").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// **Todos los `.bex` de `staging`, contra sus propios hashes.**
#[test]
fn los_bex_desplegables_cuadran_con_su_firma() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../Ultra_kernel_x86-64/staging/BMO-DATA");
    if !raiz.exists() {
        eprintln!("sin staging: no hay nada que comprobar (build.ps1 no ha corrido)");
        return;
    }
    let mut ficheros = Vec::new();
    recoger(&raiz, &mut ficheros);
    ficheros.sort();
    assert!(!ficheros.is_empty(), "hay staging pero ni un .bex dentro");

    let mut malos = Vec::new();
    for f in &ficheros {
        let d = std::fs::read(f).expect("se puede leer");
        let nombre = f.file_name().unwrap().to_string_lossy().to_string();
        match comprobar(&d) {
            Ok(0) => eprintln!("  {nombre:16} sin seccion Signature"),
            Ok(n) => eprintln!("  {nombre:16} {n} hashes OK"),
            Err(e) => {
                eprintln!("  {nombre:16} ** {e}");
                malos.push(format!("{nombre}: {e}"));
            }
        }
    }
    assert!(
        malos.is_empty(),
        "hay .bex desplegables cuya firma NO cuadra con su contenido:\n{}",
        malos.join("\n")
    );
}
