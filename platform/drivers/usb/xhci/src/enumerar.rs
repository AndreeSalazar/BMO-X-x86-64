//! **DE UN APARATO ENCHUFADO A UN APARATO CON DIRECCION.**
//!
//! [carril]  ROJO      escribe la fisica del anillo EP0 dentro del contexto de
//!                     la ranura, que es lo que el xHC usara para siempre
//! [cuesta]  APARATO   una vez por enchufe; si falla, ese aparato no existe
//! [riesgo]  SILENCIO  `Address Device` contesta que si a un contexto que
//!                     apunta mal: el fallo llega en la primera transferencia
//!
//!
//! Puertos, `Enable Slot`, el anillo EP0 de cada ranura y `Address Device`.
//!
//! ## Por que soy un fichero (L6b)
//!
//! Porque contesto UNA pregunta y se puede decir en una linea: **como pasa un
//! aparato de estar enchufado a poder recibir ordenes.** Son cuatro pasos que
//! solo tienen sentido en ese orden y que nadie ejecuta por separado:
//!
//! ```text
//!    1. el puerto dice que hay algo, y a que velocidad
//!    2. el controlador da una RANURA para ese algo
//!    3. la ranura recibe su anillo EP0 -- por donde entran las ordenes
//!    4. `Address Device`, y a partir de ahi el aparato tiene nombre
//! ```
//!
//! *** Y el paso 3 es la trampa que este fichero deja junta a proposito: el
//! anillo EP0 se guarda POR RANURA, y un aparato que se desenchufa y vuelve
//! recibe otra ranura. Tener el almacenamiento al lado de quien lo llena es lo
//! que impide que uno de los dos cambie sin el otro.
//!
//! ** El reparto es MOVER TEXTO (L6d): ni una linea cambia de contenido.

use super::*;

// ===================================================================
//  Port ops
// ===================================================================

pub unsafe fn port_speed(port: u8) -> u8 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    ((r32(c.mmio + pb + PORTSC as u64) >> 10) & 0x0F) as u8
}

pub unsafe fn port_peek(port: u8) -> u32 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    r32(c.mmio + pb + PORTSC as u64)
}

/// Enciende la corriente del puerto **y espera** la estabilizacion de VBUS.
/// Para encender UNO. Quien encienda varios debe usar [`port_power_solo`] y
/// esperar una sola vez al final -- ver ahi por que.
pub unsafe fn port_power_on(port: u8) {
    port_power_solo(port);
    // Spec: >=20 ms de estabilizacion de VBUS antes de confiar en CCS.
    hal().delay_ms(20);
}

/// Enciende la corriente y **no espera**.
///
/// * La espera de VBUS es un tiempo FISICO del puerto, y los puertos se
/// estabilizan **en paralelo**: encender ocho y esperar 20 ms una vez es tan
/// correcto como esperar 20 ms ocho veces, y tarda 160 ms menos. Con dos
/// controladores en esta placa, eso es un tercio de segundo de arranque que no
/// compraba nada.
pub unsafe fn port_power_solo(port: u8) {
    let c = match CTRL.as_mut() { Some(c) => c, None => return };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    w32(c.mmio + pb + PORTSC as u64, r32(c.mmio + pb + PORTSC as u64) | PORTSC_PP);
}

/// **Corta la corriente del puerto** (PP = 0). Con `port_power_on` despues es
/// lo mismo que desenchufar y enchufar: el aparato arranca de cero, sin
/// direccion ni estado que arrastre de antes.
///
/// Existe desde el 2026-09-17 para el segundo intento sobre un puerto que no
/// contesta: un teclado que se quedo a medias en un reinicio en caliente no
/// vuelve por resetearlo mas, vuelve por quitarle la corriente. Es lo que
/// hace el propietario con la mano cuando "no prende".
///
/// # Safety
/// Toca MMIO del xHC.
pub unsafe fn port_power_off(port: u8) {
    let c = match CTRL.as_mut() { Some(c) => c, None => return };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    let sc = r32(c.mmio + pb + PORTSC as u64);
    // Solo PP fuera; los bits de cambio (CSC/PRC...) se escriben con 1 para
    // borrarlos, asi que se enmascaran para NO tocarlos aqui.
    w32(c.mmio + pb + PORTSC as u64, sc & !PORTSC_PP & !(PORTSC_CSC | PORTSC_PRC));
}

/// Reset del puerto con TIEMPOS REALES. Un reset USB2 tarda ~10-50 ms; el
/// firmware/PHY latchea PED solo cuando termina. Poll a 1 ms, hasta 120 ms.
pub unsafe fn port_reset(port: u8) -> bool {
    if !port_reset_lanzar(port) {
        return false;
    }
    for _ in 0..120 {
        hal().delay_ms(1);
        if port_reset_acabo(port) {
            // Recovery post-reset (spec: 10 ms) y comprobar habilitacion.
            hal().delay_ms(10);
            return port_habilitado(port);
        }
    }
    false
}

/// **Pide el reset y se va.** La mitad de arriba de `port_reset` (EX4): el
/// que enumera por pasos pregunta despues con `port_reset_acabo`, a su ritmo.
/// `false` = no hay nada en el puerto.
///
/// # Safety
/// MMIO del xHC: con el CR3 del kernel puesto.
pub unsafe fn port_reset_lanzar(port: u8) -> bool {
    let c = match CTRL.as_mut() { Some(c) => c, None => return false };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    let sc = r32(c.mmio + pb + PORTSC as u64);
    if sc & PORTSC_CCS == 0 { return false; }
    // Escribir PR preservando bits RW1C (no re-limpiar cambios por error):
    // solo PP + PR, el resto a 0 (los bits de estado son RO/RW1C).
    w32(c.mmio + pb + PORTSC as u64, (sc & PORTSC_PP) | PORTSC_PR);
    true
}

/// **Termino el reset?** PR se auto-limpia al acabar; entonces se reconoce
/// PRC y se contesta que si. No espera. Tras el si, el aparato necesita sus
/// 10 ms de recuperacion (USB 2.0, 7.1.7.5) antes de `port_habilitado`.
///
/// # Safety
/// MMIO del xHC: con el CR3 del kernel puesto.
pub unsafe fn port_reset_acabo(port: u8) -> bool {
    let c = match CTRL.as_mut() { Some(c) => c, None => return false };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    let s = r32(c.mmio + pb + PORTSC as u64);
    if s & PORTSC_PR != 0 {
        return false;
    }
    if s & PORTSC_PRC != 0 {
        w32(c.mmio + pb + PORTSC as u64, (s & PORTSC_PP) | PORTSC_PRC);
    }
    true
}

/// PED: el puerto quedo habilitado tras el reset.
///
/// # Safety
/// MMIO del xHC: con el CR3 del kernel puesto.
pub unsafe fn port_habilitado(port: u8) -> bool {
    let c = match CTRL.as_ref() { Some(c) => c, None => return false };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    r32(c.mmio + pb + PORTSC as u64) & PORTSC_PED != 0
}

// ===================================================================
//  Enable Slot
// ===================================================================

pub unsafe fn enable_slot() -> Option<u8> {
    hal().log("[xhci] enable_slot\n");
    let ev = send_cmd(Trb { dw0: 0, dw1: 0, dw2: 0, dw3: TRB_ENABLE << 10 })?;
    slot_de_complecion(&ev)
}

/// **Pide una ranura y se va** (EX4): devuelve la fisica del TRB del comando,
/// que es lo que `vigilar_comando` necesita para reconocer su complecion.
///
/// # Safety
/// MMIO del xHC: con el CR3 del kernel puesto.
pub unsafe fn enable_slot_lanzar() -> Option<u64> {
    let ctrl = CTRL.as_mut()?;
    hal().log("[xhci] enable_slot (por pasos)\n");
    let mio = ctrl.cmd_ring.enqueue(&Trb { dw0: 0, dw1: 0, dw2: 0, dw3: TRB_ENABLE << 10 });
    ring_doorbell(0, 0);
    Some(mio)
}

/// La ranura que trae la complecion de un `Enable Slot`, si salio bien.
pub fn slot_de_complecion(ev: &Evento) -> Option<u8> {
    let cc = (ev.2 >> 24) & 0xFF;
    let slot = ((ev.3 >> 24) & 0xFF) as u8;
    hal().log_u64(" cc=", cc as u64);
    hal().log_u64(" slot=", slot as u64);
    if cc != CC_SUCCESS || slot == 0 { None } else { Some(slot) }
}

/// **Devolver un slot al controlador.** La pareja de `enable_slot`, y sin ella
/// el controlador se queda sin slots.
///
/// * Esto faltaba, y se vio en la primera foto: los slots subian `0x30`,
/// `0x31`, ... `0x40` en el registro de arranque. Cada intento de adopcion que
/// no acababa en un aparato instalado se llevaba un slot **para siempre**;
/// al llegar a los 64 que declara este xHC, el `Address Device` empezo a
/// contestar `cc=0x9` -- *No Slots Available* -- y a partir de ahi no se pudo
/// enumerar nada mas en toda la sesion.
///
/// Un recurso que se pide en un camino que puede fallar necesita su
/// devolucion **en el mismo sitio**, no en el camino feliz.
///
/// Lo que NO devuelve: las paginas DMA del anillo EP0 y de los contextos. Desde
/// el 2026-09-14 no hace falta: son de la RANURA (`paginas.rs`) y el siguiente
/// aparato que la reciba las reutiliza. Es un techo, no una fuga.
pub unsafe fn disable_slot(slot: u8) -> bool {
    if slot == 0 { return false; }
    let ok = send_cmd(Trb {
        dw0: 0, dw1: 0, dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_DISABLE << 10),
    })
    .is_some();
    hal().log_u64("[xhci] disable_slot ", slot as u64);
    hal().log(if ok { " ok\n" } else { " FALLO\n" });
    // El puntero del contexto de dispositivo se retira SIEMPRE, salga bien el
    // comando o no: dejarlo puesto apuntando a un slot que el xHC ya no cree
    // suyo es peor que retirarlo de mas.
    if let Some(c) = CTRL.as_ref() {
        let dcbaa = hal().phys_to_virt(c.dcbaa_phys) as *mut u64;
        dcbaa.add(slot as usize).write_volatile(0);
    }
    if (slot as usize) < MAX_SLOTS {
        EP0_RINGS[slot as usize].valid = false;
    }
    // Y sus endpoints dejan de estar VIVOS: sin esto, el siguiente aparato en esta
    // ranura veria anillos "en uso" que ya no son de nadie y pediria paginas nuevas.
    crate::transferencia::olvidar_endpoints(slot);
    ok
}

// ===================================================================
//  Per-slot EP0 ring storage
// ===================================================================

// [!] `pub(crate)` y no privados: son las DOS unicas cosas que cruzan a
// `transferencia.rs`, y ese cruce es real -- una transferencia de control
// necesita el anillo EP0 de la ranura, que es lo que este fichero llena.
//
// ** Se dicen en vez de cambiarlas callando: L6d exige que un reparto sea texto
// movido, y lo que no lo es tiene que verse. Que sean DOS y no veinte es ademas
// la medida de que el corte estaba en el sitio correcto.
pub(crate) const MAX_SLOTS: usize = 255;
#[derive(Clone, Copy)]
#[allow(dead_code)]
// ** Los campos van abiertos al crate por lo mismo que `ep0_mut`: quien hace
// una transferencia de control ESCRIBE en este anillo, y ese es todo el punto
// de que la ranura lo guarde. Antes del reparto eran privados porque "privado"
// significaba "de este fichero" y el fichero era uno solo.
pub(crate) struct Ep0Info {
    pub(crate) valid: bool,
    pub(crate) ring_phys: u64,
    pub(crate) ring_virt: *mut u32,
    pub(crate) pcs: bool,
    pub(crate) enqueue: usize,
}
// ** `pcs: false` Y NO `true`, y no cambia el comportamiento.
//
// El PCS de un anillo nuevo vale 1 por el xHCI spec, y por eso estaba escrito
// asi. Pero esta ranura NO es un anillo: es una ranura vacia (`valid: false`),
// y nadie la lee sin pasar antes por esa bandera. Al registrarla de verdad,
// `ep0_reg` escribe la estructura ENTERA con `pcs: true`.
//
// Lo que si hacia era caro: un unico campo distinto de cero manda el array
// entero a `.data` en vez de a `.bss`, o sea que **8 KiB de ceros viajaban
// dentro de la imagen del kernel** para llevar un bit puesto que nadie mira.
// Ver `EP_RINGS`, que es este mismo caso multiplicado por treinta y dos.
static mut EP0_RINGS: [Ep0Info; MAX_SLOTS] = [Ep0Info {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: false, enqueue: 0
}; MAX_SLOTS];
unsafe fn ep0_reg(slot: u8, phys: u64, virt: *mut u32) {
    EP0_RINGS[slot as usize] = Ep0Info { valid: true, ring_phys: phys, ring_virt: virt, pcs: true, enqueue: 0 };
}
pub(crate) fn ep0_mut(slot: u8) -> Option<&'static mut Ep0Info> {
    if (slot as usize) < MAX_SLOTS { unsafe { let p = &mut EP0_RINGS[slot as usize]; if p.valid { Some(p) } else { None } } }
    else { None }
}

// ===================================================================
//  Address Device
// ===================================================================

/// Pide un slot y direcciona el aparato del puerto.
///
/// * **Si algo falla despues de tener el slot, el slot se DEVUELVE.** Antes se
/// salia por cinco sitios distintos con un `?` o un `return None` y el slot se
/// quedaba pedido para siempre; el bucle de adopcion del arranque los fue
/// gastando de uno en uno hasta agotar los 64 del controlador. La pareja
/// pedir/devolver tiene que estar en la misma funcion o no esta.
///
/// *** LA ENUMERACION EN DOS TIEMPOS (2026-09-21/22). Un aparato recien
/// reseteado esta en `Default`: contesta en la direccion 0 y solo sabe decir
/// quien es (USB 2.0, 9.1.1.3 y 9.4.3). Lo primero que un anfitrion necesita
/// de el no es su nombre: es COMO habla, el paquete maximo de su EP0, que va
/// en el byte 7 de su descriptor y que no se sabe hasta que contesta
/// (5.5.3). Por eso son dos tiempos y no uno:
///
/// ```text
///   PRIMER TIEMPO -- oirlo en la direccion 0
///   1. Enable Slot
///   2. Address Device con BSR = 1 (xHCI 4.3.4): el xHC monta la ranura y
///      el EP0 con el paquete SUPUESTO (`mps0_supuesto`: el maximo que su
///      velocidad permite) y la deja en `Default`, sin SET_ADDRESS
///   3. GET_DESCRIPTOR(aparato, 64) en la direccion 0: un aparato de 8
///      contesta 8 y para (paquete corto: cierra la transferencia, legal);
///      uno de 64, los 18. En los 8 primeros ya viene el byte 7
///   4. si el byte 7 no es lo supuesto: Evaluate Context (xHCI 4.6.7)
///
///   SEGUNDO TIEMPO -- darle su direccion, limpio
///   5. RESET del puerto otra vez: el aparato vuelve a `Default` con su EP0
///      en DATA0 (9.1.1.3), pase lo que haya pasado en el primer tiempo
///   6. Address Device con BSR = 0: SET_ADDRESS. El xHC copia el contexto de
///      entrada ENTERO (4.6.5), asi que lleva el paquete evaluado y el
///      anillo por donde va, no otra ranura nueva
///   7. `PLAZO_ASENTAR_MS` (9.2.6.3 da 2 ms al aparato; se le dan cinco
///      veces eso), y entonces los 18, la cabecera y la configuracion
/// ```
///
/// Es el orden en el que un aparato lo ha visto todo antes de llegar aqui
/// (los anfitriones contra los que se prueba en fabrica hacen estos dos
/// tiempos, y se leyeron para esto: `PLAN_AUDIO.md` 6.1). Aqui no se llama
/// como ellos: cada paso tiene su motivo en el protocolo, arriba, y es lo
/// que BMO-X hace. Lo de antes (un tiempo: reset, SET_ADDRESS, y los
/// descriptores con paquete supuesto 8) era correcto por el protocolo y
/// suficiente para teclados y ratones; se retiro el 21-09.
///
/// *** DOS PASOS QUE SOBRABAN, y que dejaron SIN TECLADO NI RATON dos
/// arranques del Ryzen (2026-09-22; el propietario: *"entre 2 veces en mi
/// BMO-X pero mi teclado y mouse no respondio"*). La primera version
/// (6d4a0457) metia un `Reset Device` entre el 5 y el 6, y en el 6 rehacia
/// los contextos enteros:
///
/// * `Reset Device` (xHCI 4.6.11) solo vale para una ranura en `Addressed`
///   o `Configured`. Tras el paso 2 esta en `Default`, y el xHC contesta
///   **Context State Error** (cc = 19). Aqui `cc != 1` era "NO acepta
///   direccion" y la ranura volvia: TODOS los aparatos, no solo el mudo.
///   El comando no aporta nada en estos dos tiempos y se quita, no se
///   ignora.
/// * El paso 6 volvia a suponer el paquete del EP0 (64 para Full Speed) y
///   pisaba el Evaluate Context del 4: un teclado Full Speed de paquete 8
///   volvia a un EP0 de 64, y sus 9 bytes de cabecera llegaban como 8
///   (paquete corto), tres veces. Ahora `address_lanzar` con `bsr = false`
///   no toca el anillo ni el contexto de salida: solo rehace el de entrada
///   con la verdad.
///
/// Las pruebas de `pasos.rs` estaban verdes con los dos fallos porque el
/// `Metal` fingido decia que si a todo: contestaba `Reset Device` con exito
/// en cualquier estado y no perdia el paquete al direccionar. Ahora modela
/// las dos cosas (ver `Fingido::direccionar`).
///
/// Devuelve la ranura y COMO entro (`como_entro`: su paquete de EP0, si hubo
/// que evaluarlo; los ms los pone el llamante, que tiene el reloj). Si no
/// entro, en que paso y con que `cc` (`Tropiezo`): es lo que la ficha del
/// portero muestra y lo que el `save` lee para decir que funciona y que no.
///
/// # Safety
/// MMIO del xHC: con el CR3 del kernel puesto.
pub unsafe fn address_device(port: u8, speed: u8) -> Result<(u8, u16), Tropiezo> {
    let slot = match enable_slot() {
        Some(s) => s,
        None => return Err(Tropiezo::Direccion { paso: PASO_DIR_RANURA, cc: 0 }),
    };
    match direccionar_en_dos_tiempos(port, speed, slot) {
        Ok(como) => Ok((slot, como)),
        Err(t) => {
            disable_slot(slot);
            Err(t)
        }
    }
}

/// **En que paso de los dos tiempos se quedo un aparato que no entro**, y
/// con que `cc` (0 = no hubo comando que contestara; 254 = no contesto en
/// plazo). Va en el detalle de la ficha del portero como `paso | cc << 4`,
/// la misma forma que el "sin papeles".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tropiezo {
    /// Sin direccion: `PASO_DIR_*`.
    Direccion { paso: u8, cc: u8 },
    /// Con la ranura en la direccion 0 y sin los 8 primeros bytes: es un
    /// "sin papeles" en el paso 1, con el `cc` de la ultima lectura.
    Papeles { cc: u8 },
}

/// El reset del puerto no se pudo lanzar, no acabo, o no dejo el puerto
/// habilitado: no hay con quien hablar.
pub const PASO_DIR_RESET: u8 = 1;
/// `Enable Slot`: el controlador no dio ranura.
pub const PASO_DIR_RANURA: u8 = 2;
/// `Address Device` con BSR = 1 (la direccion 0).
pub const PASO_DIR_DIRECCION0: u8 = 3;
/// `Evaluate Context`: el paquete del EP0 declarado no se pudo poner.
pub const PASO_DIR_EVALUAR: u8 = 4;
/// El segundo reset: el puerto se vacio o no acabo.
pub const PASO_DIR_RESET2: u8 = 5;
/// `Address Device` con BSR = 0: el `SET_ADDRESS`.
pub const PASO_DIR_DIRECCION: u8 = 6;

/// El detalle de una ficha "sin direccion": `paso | cc << 4`.
pub fn detalle_sin_direccion(paso: u8, cc: u8) -> u16 {
    (paso as u16 & 0xF) | ((cc as u16) << 4)
}

/// **COMO entro un aparato que si entro**, para el detalle de su ficha:
/// bits 0..8 su paquete de EP0 declarado (0 = no se supo; 255 = 255 o mas),
/// bit 8 = hubo que evaluarlo (el supuesto no era el suyo), bits 9..16 los
/// ms que costaron los dos tiempos en octavos (tope 1016). Con esto el
/// `save` dice, aparato por aparato, que hizo falta y cuanto tardo.
pub fn como_entro(mps0_declarado: u16, evaluado: bool, ms: u64) -> u16 {
    let paquete = mps0_declarado.min(255);
    let octavos = (ms / 8).min(127) as u16;
    paquete | ((evaluado as u16) << 8) | (octavos << 9)
}

unsafe fn direccionar_en_dos_tiempos(port: u8, speed: u8, slot: u8) -> Result<u16, Tropiezo> {
    let h = hal();
    // PRIMER TIEMPO. 2. En la direccion 0.
    if let Err(cc) = direccionar_en_slot(port, speed, slot, true, 0) {
        h.log("[xhci] address (BSR=1) FALLO\n");
        return Err(Tropiezo::Direccion { paso: PASO_DIR_DIRECCION0, cc });
    }
    // 3. Los 64 bytes en la direccion 0: solo hacen falta los 8 primeros.
    let mut cabeza = [0u8; 64];
    let mut n = 0usize;
    for _ in 0..3 {
        n = get_device_descriptor(slot, &mut cabeza);
        if n >= 8 { break; }
        h.delay_ms(10);
    }
    if n < 8 {
        h.log("[xhci] ni los 8 primeros bytes en la direccion 0\n");
        return Err(Tropiezo::Papeles { cc: crate::transferencia::last_event().2 });
    }
    // 4. El paquete de verdad.
    let declarado = mps0_declarado(cabeza[7], speed);
    let mut mps0 = 0u16;
    if declarado != mps0_supuesto(speed) && declarado != 0 {
        h.log_u64("[xhci] mps0 declarado=", declarado as u64);
        if let Err(cc) = evaluar_mps0(slot, declarado) {
            h.log("[xhci] evaluate context FALLO\n");
            return Err(Tropiezo::Direccion { paso: PASO_DIR_EVALUAR, cc });
        }
        mps0 = declarado;
    }
    // SEGUNDO TIEMPO. 5. El reset que lo deja limpio.
    if !port_reset(port) {
        h.log("[xhci] el segundo reset FALLO\n");
        return Err(Tropiezo::Direccion { paso: PASO_DIR_RESET2, cc: 0 });
    }
    // 6. SET_ADDRESS de verdad (con el paquete evaluado), y 7. que asiente.
    if let Err(cc) = direccionar_en_slot(port, speed, slot, false, mps0) {
        return Err(Tropiezo::Direccion { paso: PASO_DIR_DIRECCION, cc });
    }
    h.delay_ms(PLAZO_ASENTAR_MS);
    Ok(como_entro(declarado, mps0 != 0, 0))
}

/// Lo que se le da a un aparato para asentar su direccion nueva antes de
/// pedirle nada: USB 2.0 (9.2.6.3) le concede 2 ms para hacer caso al
/// SET_ADDRESS; se le dan cinco veces eso, porque el aparato que los
/// necesita no avisa y 8 ms una vez por enchufe no se notan.
pub const PLAZO_ASENTAR_MS: u64 = 10;

// ** AQUI HUBO un `Reset Device` (xHCI 4.6.11) durante un dia (6d4a0457 ->
// 2026-09-22). No va: la ranura esta en `Default` y el xHC lo rechaza con
// Context State Error. Ver `address_device`. Si algun dia hace falta
// resetear un aparato YA direccionado sin devolver la ranura, es el sitio;
// hoy un aparato que se atasca se suelta entero (`soltar_puerto`).

/// `Err(cc)`: 0 = no se pudo ni lanzar, 254 = no contesto en plazo.
unsafe fn direccionar_en_slot(port: u8, speed: u8, slot: u8, bsr: bool, mps0: u16) -> Result<(), u8> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return Err(0) };
    let mio = address_lanzar(port, speed, slot, bsr, mps0).ok_or(0u8)?;
    // * Esto tomaba el primer evento SIN MIRAR EL TIPO y le leia el `cc`. Un
    // Transfer Event correcto tambien trae `cc=1`, asi que un informe del
    // raton se leia como "el Address Device salio bien" -- y de paso ese
    // informe desaparecia. Y despues tomaba CUALQUIER complecion: la de este
    // comando, o la tardia del anterior. Ver `Espera::Comando`.
    let ev = evt_poll_block(ctrl, Espera::Comando { trb: mio }).ok_or(254u8)?;
    if address_rematar(slot, &ev) { Ok(()) } else { Err(cc_de(&ev)) }
}

/// **El medida de paquete del EP0 que se SUPONE al direccionar**, por
/// velocidad del puerto: 8 para Low Speed (no puede ser otro), **64 para
/// Full Speed**, 64 para High, 512 para Super. El aparato declara el suyo en
/// el byte 7 de su descriptor, y si no coincide se le dice al xHC
/// (`evaluar_mps0`) antes de pedirle nada mas.
///
/// *** 64 Y NO 8 PARA FULL SPEED: se supone el MAXIMO que la velocidad
/// permite (USB 2.0, 5.5.3: Full Speed 8, 16, 32 o 64; ver
/// `address_device`). Suponer menos de lo que el aparato tiene rompe: pide
/// 18 bytes a uno de 64 y el xHC ve un paquete mayor que el contexto,
/// Babble. Suponer el maximo NO rompe con ninguno: un aparato de 8 contesta
/// un paquete de 8 --corto, y un paquete corto cierra la transferencia sin
/// error (8.5.3.2)-- y en esos 8 ya viene el byte 7. Es la asimetria del
/// protocolo sobre la que se apoya el primer tiempo.
pub fn mps0_supuesto(speed: u8) -> u16 {
    match speed { 2 => 8, 1 | 3 => 64, 4 | 5 => 512, _ => 64 }
}

/// Lo que el aparato DECLARA en `bMaxPacketSize0` (byte 7 del descriptor
/// del aparato): bytes en USB 2, y un EXPONENTE en Super Speed (9 = 512).
pub fn mps0_declarado(byte7: u8, speed: u8) -> u16 {
    if speed >= 4 { 1u16 << byte7.min(12) } else { byte7 as u16 }
}

/// **`Evaluate Context`: el EP0 pasa a tener el medida de paquete que el
/// aparato declaro** (2026-09-21). Lanzado y sin esperar; devuelve la fisica
/// del TRB para `vigilar_comando`.
///
/// *** ESTO FALTABA, y es la causa mas probable del "sin papeles" del
/// puerto 1 del Ryzen. `address_lanzar` supone 8 bytes de paquete para un
/// aparato Full Speed, y `leer_descriptores` pedia los 18 bytes del
/// descriptor de golpe. Un teclado o un raton (mps0 = 8) contestan en tres
/// paquetes de 8 y todo cuadra. Un audifono USB Audio suele declarar
/// mps0 = 64: manda los 18 bytes en UN paquete, el xHC ve un paquete mas
/// grande que el maximo del contexto y contesta Babble (cc = 3): "acepta
/// direccion y no da descriptores", exactamente lo que dijo la ficha
/// (`ni el descriptor del aparato`). La regla: primero lo que cabe en
/// cualquier paquete, leer el byte 7, y si no coincide con lo supuesto,
/// `Evaluate Context`; despues el resto. Hoy es el paso 4 del primer tiempo
/// (`address_device`).
///
/// El contexto de entrada se rellena copiando el EP0 del Device Context de
/// SALIDA (lo que el xHC tiene ahora, con su dequeue actual) y cambiando
/// solo el Max Packet Size, con `A1` puesto: de un EP0 el `Evaluate
/// Context` solo evalua ese campo (xHCI 6.2.3.1), y todo lo demas tiene
/// que ser lo que el xHC ya tiene o lo pisaria con ceros.
///
/// # Safety
/// MMIO del xHC y paginas DMA de la ranura: con el CR3 del kernel puesto.
pub unsafe fn evaluar_mps0_lanzar(slot: u8, mps: u16) -> Option<u64> {
    let ctrl = CTRL.as_mut()?;
    let h = hal();
    let cs = ctx_sz(ctrl);
    let in_phys = crate::paginas::de_ranura(slot, crate::paginas::Uso::Entrada)?;
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);
    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop: nada
    in32.add(1).write_volatile(1 << 1); // Add: solo el EP0 (A1)
    let dev_phys = dcbaa_get(slot)?;
    let dev_ep0 = (h.phys_to_virt(dev_phys) as *const u32).add(cs / 4);
    let in_ep0 = in_virt.add(2 * cs) as *mut u32;
    for i in 0..cs / 4 {
        in_ep0.add(i).write_volatile(dev_ep0.add(i).read_volatile());
    }
    let dw1 = in_ep0.add(1).read_volatile();
    in_ep0.add(1).write_volatile((dw1 & 0xFFFF) | ((mps as u32) << 16));
    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_EVAL_CTX << 10),
    };
    let mio = ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    h.log_u64("[xhci] evaluate context, mps0=", mps as u64);
    Some(mio)
}

/// `evaluar_mps0_lanzar` + la espera bloqueante. Para el arranque.
/// `Err(cc)`: 0 = no se pudo lanzar, 254 = no contesto en plazo.
///
/// # Safety
/// MMIO del xHC: con el CR3 del kernel puesto.
pub unsafe fn evaluar_mps0(slot: u8, mps: u16) -> Result<(), u8> {
    let mio = evaluar_mps0_lanzar(slot, mps).ok_or(0u8)?;
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return Err(0) };
    match evt_poll_block(ctrl, Espera::Comando { trb: mio }) {
        Some(ev) => {
            let cc = (ev.2 >> 24) & 0xFF;
            hal().log_u64(" eval_ctx cc=", cc as u64);
            if cc == CC_SUCCESS { Ok(()) } else { Err(cc as u8) }
        }
        None => Err(254),
    }
}

/// **Prepara los contextos, encola el `Address Device` y se va** (EX4). La
/// mitad de arriba de `direccionar_en_slot`; devuelve la fisica del TRB para
/// `vigilar_comando`. Si devuelve `None`, la ranura sigue pedida y es del
/// llamante devolverla (`disable_slot`), como en `address_device`.
///
/// # Safety
/// MMIO del xHC y paginas DMA de la ranura: con el CR3 del kernel puesto.
///
/// `bsr` = Block Set Address Request (xHCI 4.6.5): con `true` el xHC monta
/// los contextos y deja la ranura en `Default` SIN mandar `SET_ADDRESS`; el
/// aparato sigue en la direccion 0 y se le puede hablar por su EP0. Es el
/// paso 2 del primer tiempo (`address_device`), y la ranura se monta
/// ENTERA: anillo del EP0 nuevo, contexto de salida a cero, DCBAA.
///
/// Con `false` es el paso 6, sobre una ranura que YA esta montada y en
/// `Default`: **no se toca ni el anillo ni el contexto de salida** (son del
/// xHC mientras la ranura viva), solo se rehace el contexto de ENTRADA con
/// el paquete del EP0 de verdad (`mps0`; 0 = el supuesto por velocidad) y
/// el dequeue por donde va el anillo. El xHC copia ese contexto entero al
/// de salida (4.6.5): lo que no vaya en el, se pierde. Es lo que no se
/// hacia el 2026-09-21 (ver `address_device`).
pub unsafe fn address_lanzar(port: u8, speed: u8, slot: u8, bsr: bool, mps0: u16) -> Option<u64> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    let h = hal();
    let cs = ctx_sz(ctrl);

    let (dq, mps) = if bsr {
        // ** Las tres paginas son de la RANURA: pedidas la primera vez que
        // se usa este numero, reutilizadas en cada enchufe despues
        // (`paginas.rs`).
        let ep0_phys = crate::paginas::de_ranura(slot, crate::paginas::Uso::AnilloEp0)?;
        let ep0_virt = h.phys_to_virt(ep0_phys) as *mut u32;
        core::ptr::write_bytes(ep0_virt as *mut u8, 0, 4096);
        let mut ring = TransferRing::new(ep0_virt, ep0_phys);
        // El productor (control_transfer) alterna su cycle state al dar la
        // vuelta -- el Link TRB necesita Toggle Cycle para que el xHC haga
        // lo mismo, o el anillo se desincroniza tras el primer wrap.
        ring.enable_toggle_cycle();
        ep0_reg(slot, ep0_phys & !0xF, ep0_virt);

        let dev_phys = crate::paginas::de_ranura(slot, crate::paginas::Uso::Dispositivo)?;
        let dev_virt = h.phys_to_virt(dev_phys) as *mut u8;
        core::ptr::write_bytes(dev_virt, 0, 4096);
        // DCBAA[slot]
        let dcbaa = h.phys_to_virt(ctrl.dcbaa_phys) as *mut u64;
        dcbaa.add(slot as usize).write_volatile(dev_phys & !0x3F);

        ((ep0_phys & !0xF) | 1, mps0_supuesto(speed) as u32)
    } else {
        // La ranura ya tiene su anillo: el dequeue es por donde va el
        // productor, con su cycle state (`ep0_dequeue`).
        let ep0 = ep0_mut(slot)?;
        let mps = if mps0 != 0 { mps0 as u32 } else { mps0_supuesto(speed) as u32 };
        (ep0_dequeue(ep0), mps)
    };

    let in_phys = crate::paginas::de_ranura(slot, crate::paginas::Uso::Entrada)?;
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);

    // Input Control Context
    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile(3); // Add Slot+EP0

    // Slot Context
    let sc = in_virt.add(cs) as *mut u32;
    sc.add(0).write_volatile(((speed as u32) & 0xF) << 20 | (1 << 27));
    sc.add(1).write_volatile((port as u32 + 1) << 16);

    // EP0 Context
    let ep0 = in_virt.add(2 * cs) as *mut u32;
    ep0.add(0).write_volatile(0);
    ep0.add(1).write_volatile((mps << 16) | (4 << 3) | (3 << 1));
    ep0.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep0.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    ep0.add(4).write_volatile(8);

    // Address Device TRB
    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_ADDRESS_DEV << 10) | ((bsr as u32) << 9),
    };
    let mio = ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    Some(mio)
}

/// La complecion del `Address Device` ya esta en la mano: **salio bien?** Si
/// si, deja el dequeue del EP0 escrito en el Device Context, que es lo que el
/// timbre recarga en cada transferencia. La mitad de abajo de
/// `direccionar_en_slot`.
///
/// # Safety
/// Escribe el Device Context de la ranura: con el CR3 del kernel puesto.
pub unsafe fn address_rematar(slot: u8, ev: &Evento) -> bool {
    let ctrl = match CTRL.as_ref() { Some(c) => c, None => return false };
    let h = hal();
    let cs = ctx_sz(ctrl);
    let cc = (ev.2 >> 24) & 0xFF;
    h.log_u64(" addr_dev cc=", cc as u64);
    if cc != CC_SUCCESS { return false; }
    // Las dos paginas son las de la RANURA: las mismas que `address_lanzar`
    // pidio, porque `de_ranura` devuelve siempre la misma para el mismo uso.
    // El dequeue que se escribe es POR DONDE VA el anillo (tras el segundo
    // `Address Device` ya lleva una transferencia hecha), no su principio.
    let dq = match ep0_mut(slot) { Some(e) => ep0_dequeue(e), None => return false };
    let dev_phys = match crate::paginas::de_ranura(slot, crate::paginas::Uso::Dispositivo) {
        Some(p) => p,
        None => return false,
    };
    let dev_virt = h.phys_to_virt(dev_phys);
    // Write EP0 dequeue into Device Context EP0 for future doorbell reloads
    let d_ep0 = dev_virt.add(cs) as *mut u32;
    d_ep0.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    d_ep0.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    true
}

/// El TR Dequeue Pointer que describe el anillo del EP0 tal como va: la
/// fisica del proximo TRB que el productor va a escribir, con su cycle
/// state en el bit 0 (DCS). Sobre un anillo recien montado es su principio
/// con DCS = 1.
fn ep0_dequeue(e: &Ep0Info) -> u64 {
    ((e.ring_phys & !0xF) + (e.enqueue as u64) * 16) | if e.pcs { 1 } else { 0 }
}
