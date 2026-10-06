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
