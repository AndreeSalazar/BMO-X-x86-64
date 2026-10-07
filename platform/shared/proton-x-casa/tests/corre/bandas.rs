//! **Los sub-directores en la casa** (H4.3 de `PLAN_LOS_DOCE_DIRECTORES`,
//! 07-10): el `cubo.exe` y el `cubo12.exe` (con profundidad y sus
//! limpiezas) enteros, con el ejecutor nativo de la casa partiendo cada
//! dibujo en FRANJAS por hilos de verdad -- y una franja que "falla" en
//! cada reparto, que rehace la casa. Lo que se ve en cada Present sigue
//! siendo lo que dibujo la 3060 bajo Windows.

use super::*;
use bmo_proton_x_casa::Obreros;
use std::sync::atomic::AtomicU32;

/// En cuantas partes reparte el banco (0: no reparte; lo de siempre).
static PARTES: AtomicU32 = AtomicU32::new(0);
/// Franjas que pinto OTRO hilo, y las que "fallaron" (las rehizo la casa).
static FUERA: AtomicU32 = AtomicU32::new(0);
static FALLADAS: AtomicU32 = AtomicU32::new(0);

fn cuantos() -> u32 {
    PARTES.load(Ordering::SeqCst)
}

/// La parte 0 aqui, las demas en hilos; la ULTIMA no corre (un obrero que
/// fallo): se devuelve en la mascara.
std::thread_local! {
    static EN_PARTE: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

fn en_parte() -> bool {
    EN_PARTE.with(|e| e.get())
}

fn repartir(n: u32, f: &(dyn Fn(u32) + Sync)) -> u64 {
    std::thread::scope(|s| {
        for k in 1..n.saturating_sub(1) {
            s.spawn(move || {
                EN_PARTE.with(|e| e.set(true));
                f(k);
                FUERA.fetch_add(1, Ordering::SeqCst);
            });
        }
        f(0);
    });
    FALLADAS.fetch_add(1, Ordering::SeqCst);
    1 << (n - 1)
}

pub static OBREROS_DEL_BANCO: Obreros = Obreros { cuantos, repartir, en_parte };

/// Vuelve a "sin repartir" aunque la prueba falle (las demas del binario
/// no reparten).
struct Volver;
impl Drop for Volver {
    fn drop(&mut self) {
        PARTES.store(0, Ordering::SeqCst);
    }
}

#[test]
fn el_cubo_en_franjas_por_hilos_se_ve_como_en_la_3060() {
    let letra = |c: u8| 1 << 62 | 1 << 8 | 1 << 9 | c as u64;
    let uno = uno_a_la_vez();
    let _volver = Volver;
    PARTES.store(6, Ordering::SeqCst);
    FUERA.store(0, Ordering::SeqCst);
    FALLADAS.store(0, Ordering::SeqCst);
    let (salio, dicho, _) = correr_exe(&uno, CUBO, true, &[letra(b'b'), 0, letra(b'b'), 0, letra(b'q')]);
    assert_eq!(String::from_utf8_lossy(&dicho), "", "ni un aviso");
    assert_eq!(salio, 3);
    let huellas: Vec<u64> = bmo_cubo::referencia::HUELLAS.iter().map(|&(_, h)| h).collect();
    assert_eq!(*VISTAS.lock().unwrap(), huellas, "cubo.exe en franjas: lo que dibujo la 3060");
    let (fuera, falladas) = (FUERA.load(Ordering::SeqCst), FALLADAS.load(Ordering::SeqCst));
    assert_eq!(falladas, 3, "un reparto por dibujo, y en cada uno una franja rehecha");
    assert_eq!(fuera, 3 * 4, "cuatro franjas de cada dibujo, en otros hilos");

    // cubo12.exe: con profundidad (D32, LESS) y su ClearDepthStencilView.
    let dir = volumen().join("window/sombras");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for h in ["f3ef42a0", "4d67f5e4"] {
        let cso = std::fs::read(format!("../proton-x/prueba/sombras/{h}.cso")).unwrap();
        std::fs::write(dir.join(format!("{h}.cso")), cso).unwrap();
    }
    *NOMBRE.lock().unwrap() = ("window/cubo12.exe", "");
    let (salio, dicho, _) = correr_exe(&uno, CUBO12, true, &[letra(b'b'), 0, letra(b'b'), 0, letra(b'q')]);
    *NOMBRE.lock().unwrap() = ("window/prueba.exe", "");
    assert_eq!(String::from_utf8_lossy(&dicho), "", "ni un aviso");
    assert_eq!(*VISTAS.lock().unwrap(), huellas, "cubo12.exe en franjas, con profundidad: lo de la 3060");
    assert_eq!(salio, 3, "y las tres huellas que LEE el .exe (CopyTextureRegion) cuadran");
    assert!(FALLADAS.load(Ordering::SeqCst) > falladas, "cubo12 tambien se repartio");
}
