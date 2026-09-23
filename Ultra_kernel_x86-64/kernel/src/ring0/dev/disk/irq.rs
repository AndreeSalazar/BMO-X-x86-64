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

/// Donde se armo: `(bus, dev, func)` del HBA y el APIC al que se le dijo que
/// mandara. Para volver a preguntarle al aparato con que se quedo.
static mut DONDE: (u8, u8, u8) = (0, 0, 0);
static mut DESTINO: u8 = 0;

/// Veces que entro el vector 49, fuera o no del disco. Contra `CUENTA` dice si
/// el mensaje llega y no es del puerto, o no llega nunca.
static ENTRADAS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// La marca el arranque cuando MSI queda programado y el HBA acepta avisar.
pub fn marcar_armada(bus: u8, dev: u8, func: u8, destino: u8) {
    unsafe {
        ARMADA = true;
        DONDE = (bus, dev, func);
        DESTINO = destino;
    }
}

// -- La escalera, espejo de `bmo_abi::...::informe::DISCO_AVISO_*` ----------

pub const DISCO_AVISO_ENTRADAS_MASK: u64 = 0xFFFF;
pub const DISCO_AVISO_MSI_ENABLE: u64 = 1 << 16;
pub const DISCO_AVISO_MSI_MASCARA: u64 = 1 << 17;
pub const DISCO_AVISO_MSIX: u64 = 1 << 18;
pub const DISCO_AVISO_GHC_IE: u64 = 1 << 20;
pub const DISCO_AVISO_PXIE: u64 = 1 << 21;
pub const DISCO_AVISO_IS_HBA: u64 = 1 << 22;
pub const DISCO_AVISO_PXIS: u64 = 1 << 23;
pub const DISCO_AVISO_IRR: u64 = 1 << 24;
pub const DISCO_AVISO_CI: u64 = 1 << 25;
pub const DISCO_AVISO_DIRECCION_OK: u64 = 1 << 26;
pub const DISCO_AVISO_DATO_OK: u64 = 1 << 27;
pub const DISCO_AVISO_DESTINO_SHIFT: u64 = 28;
pub const DISCO_AVISO_CPU_SHIFT: u64 = 36;
pub const DISCO_AVISO_ARMADA: u64 = 1 << 62;
pub const DISCO_AVISO_VALIDO: u64 = 1 << 63;

/// **`INFO_DISCO_AVISO`: la escalera del aviso, leida AHORA.** Cada peldano es
/// un sitio donde el aviso se puede perder, preguntado a quien lo tiene: el
/// aparato (su MSI), el HBA (sus registros), el LAPIC (su IRR) y el vector
/// (sus entradas). Ver el ABI.
pub fn escalera(puerto: u8) -> u64 {
    use core::sync::atomic::Ordering;
    let mut e = DISCO_AVISO_VALIDO
        | (ENTRADAS.load(Ordering::Relaxed) as u64).min(DISCO_AVISO_ENTRADAS_MASK)
        | ((crate::ring0::plat::smp::tramp::apic_id() as u64 & 0xFF) << DISCO_AVISO_CPU_SHIFT);
    let (armada, (b, d, f), destino) = unsafe { (ARMADA, DONDE, DESTINO) };
    if armada {
        e |= DISCO_AVISO_ARMADA | ((destino as u64) << DISCO_AVISO_DESTINO_SHIFT);
        let m = crate::ring0::dev::pci::msi_leer(b, d, f);
        let vector = crate::ring0::plat::irq::VECTOR_DISCO as u16;
        let direccion = 0xFEE0_0000u32 | ((destino as u32) << 12);
        for (si, bit) in [
            (m.enable, DISCO_AVISO_MSI_ENABLE),
            (m.enmascarado, DISCO_AVISO_MSI_MASCARA),
            (m.msix, DISCO_AVISO_MSIX),
            (m.direccion == direccion, DISCO_AVISO_DIRECCION_OK),
            (m.dato & 0xFF == vector, DISCO_AVISO_DATO_OK),
        ] {
            if si {
                e |= bit;
            }
        }
    }
    if let Some((ghc, is_hba, pxis, pxie, pxci)) = unsafe { bmo_ahci::aviso_crudo(puerto) } {
        for (si, bit) in [
            (ghc & (1 << 1) != 0, DISCO_AVISO_GHC_IE),
            (pxie & 1 != 0, DISCO_AVISO_PXIE),
            (is_hba & (1 << puerto) != 0, DISCO_AVISO_IS_HBA),
            (pxis != 0, DISCO_AVISO_PXIS),
            (pxci & 1 != 0, DISCO_AVISO_CI),
        ] {
            if si {
                e |= bit;
            }
        }
    }
    if crate::ring0::plat::timer::pendiente_en_lapic(crate::ring0::plat::irq::VECTOR_DISCO as u8) == Some(true) {
        e |= DISCO_AVISO_IRR;
    }
    e
}

/// **Lo llama el manejador del vector del disco.** Ver `plat/irq.rs`.
///
/// Corre en contexto de interrupcion: lo minimo y nada mas. Limpiar el aviso
/// del aparato es obligatorio --si no, lo vuelve a pedir en el acto y no deja
/// correr a nadie-- y contar es lo que permite saber despues si esto funciono.
pub fn atender(puerto: u8) {
    ENTRADAS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
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
