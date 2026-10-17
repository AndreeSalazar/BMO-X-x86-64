//! Compatibilidad publica tras extraer el lector LIBOS.
//! [carril] VERDE
//! [consumo] NADA
use bmo_gpu_ga10x::{elf::Fuente, inspeccion::libos};

// Una implementacion del trait historico debe ser aceptada por el crate
// independiente: no son dos contratos con el mismo nombre.
struct Copia([u8; 16]);
impl Fuente for Copia {
    fn leer(&mut self, desde: u64, destino: &mut [u8]) -> bool {
        self.0.as_slice().leer(desde, destino)
    }
}
fn acepta_fuente<F: bmo_nvidia_libos::Fuente>(mut fuente: F) {
    let mut byte = [1];
    assert!(fuente.leer(0, &mut byte));
    assert_eq!(byte, [0]);
}

#[test]
fn las_rutas_historicas_comparten_fuente_y_lector() {
    acepta_fuente(Copia([0; 16]));
    let copia = [0; 24];
    assert_eq!(libos::resumen(&copia), bmo_nvidia_libos::resumen(&copia));
    let _: Result<libos::diccionario::Elf<&[u8]>, _> =
        bmo_nvidia_libos::diccionario::Elf::nuevo(copia.as_slice(), 24);
}
