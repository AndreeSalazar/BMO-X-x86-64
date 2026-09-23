//! **El aviso del disco.** Que llegue, que se limpie, y que se cuente.
//!
//! [carril]  ROJO      el aviso del disco: que llegue y que se limpie
//! [consumo] NADA      corre cuando alguien lee o escribe el disco
//!
//! Paso 3 de `docs/plan/PLAN_ALMACENAMIENTO.md`. La septima pregunta de las que
//! respondia `dev/disk/mod.rs`:
//!
//! ```text
//!   avisame cuando acabe   ->  IRQ   (plumbing)
//! ```
//!
//! No es politica ni es formato: es cableado. Sale por eso -- no porque sea
//! grande, sino porque **es lo unico de este fichero que corre en contexto de
//! interrupcion**, y mezclar eso con codigo que puede tomar candados es como se
//! cuelga una maquina sin dejar rastro.

/// El disco avisa por MSI, o hay que seguir preguntandole?
static mut ARMADA: bool = false;

/// Avisos atendidos.
///
/// Es lo que dice si la interrupcion **llega de verdad**: si `ARMADA` queda en
/// cierto y esto no sube, la placa acepto la programacion de MSI y no la esta
/// enrutando -- que es un caso real, y por eso se cuenta por separado en vez de
/// fiarse de que *"quedo armado"* signifique *"funciona"*.
static mut CUENTA: u64 = 0;

/// Avisa el disco por su cuenta, y cuantas veces lo ha hecho.
pub fn estado() -> (bool, u64) {
    unsafe { (ARMADA, CUENTA) }
}

/// La marca el arranque cuando MSI queda programado y el HBA acepta avisar.
pub fn marcar_armada() {
    unsafe { ARMADA = true };
}

/// **Lo llama el manejador del vector del disco.** Ver `plat/irq.rs`.
///
/// Corre en contexto de interrupcion: lo minimo y nada mas. Limpiar el aviso
/// del aparato es obligatorio --si no, lo vuelve a pedir en el acto y no deja
/// correr a nadie-- y contar es lo que permite saber despues si esto funciono.
pub fn atender(puerto: u8) {
    if puerto == 0xFF {
        return;
    }
    if unsafe { bmo_ahci::atender(puerto) } {
        unsafe { CUENTA += 1 };
        // ** Y DESDE EL 2026-09-23 HAY QUIEN DUERMA: el HILO DEL DISCO, con una
        // orden en vuelo (`hilo.rs`). Aqui se le despierta y, si tiene mas
        // rango que quien estaba corriendo, entra ya: el stub del vector 49
        // devuelve el contexto que elija el planificador.
        //
        // Durante un syscall esto no llega: corren con `IF=0`. El aviso espera
        // al `sysretq` y entra entonces -- que es exactamente cuando el hilo
        // puede correr.
        super::hilo::despertado_por_irq();
        crate::ring0::task::scheduler::despertar_desde_irq(CLAVE_ESPERA);
    }
}

/// La clave sobre la que dormira quien espere al disco.
///
/// Un numero que no choca con las de los canales, que son indices chicos.
/// Vive aqui --y no en el planificador-- porque **el planificador no tiene por
/// que saber que existe un disco**: solo reparte turnos sobre claves que le dan.
/// Duerme sobre ella el HILO DEL DISCO (`hilo.rs`), y solo el.
pub const CLAVE_ESPERA: u64 = 0xD15C_0000_0000_0001;
