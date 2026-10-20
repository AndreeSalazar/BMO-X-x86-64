//! PP3: lectura de los MSR permitidos en el Ryzen 5 5600X.
//!
//! [carril]  ROJO      ejecuta rdmsr en Ring 0, solo tras comprobar el perfil
//! [consumo] NADA      una lectura, sin memoria ni escritura de registros

/// Recibe la fila que concedio `bmo_msr_lista::decidir`, nunca el numero
/// sin comprobar de la tarea. Quien llama ya comprobo la CPU del perfil.
pub fn leer(fila: &bmo_msr_lista::Fila) -> u64 {
    let bajo: u32;
    let alto: u32;
    // SAFETY: el despachador solo entrega filas de la lista blanca despues
    // de comprobar que esta es la CPU del perfil donde existen estos MSR.
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") fila.msr, out("eax") bajo,
            out("edx") alto, options(nomem, nostack));
    }
    (u64::from(alto) << 32) | u64::from(bajo)
}
