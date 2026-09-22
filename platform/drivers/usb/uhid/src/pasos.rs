//! **LA ENUMERACION POR PASOS: un paso por bombeo, y el raton se sigue
//! leyendo** (EX4, 2026-09-21).
//!
//! [carril]  VERDE     decide QUE paso toca; el MMIO lo hace `Metal`
//! [cuesta]  APARATO   ~15 pasos por enchufe, cada uno microsegundos
//! [riesgo]  SILENCIO  un plazo que no se cumple es un aparato que no entra,
//!                     y se dice por `papeles` como siempre; lo que NO puede
//!                     pasar ya es que un intento se lleve la vuelta entera
//!
//! ## El fallo que esto arregla, con su numero
//!
//! El `save` de las 12:48 (21-09) puso `peor trabajo bombeo 932898 us`: UN
//! `pump_bus` de 933 ms. Era un intento de enumerar el puerto 1 del Ryzen, que
//! tiene algo que acepta direccion y no da descriptores: encender, 100 ms de
//! debounce, reset, `Address Device`, y tres `GET_DESCRIPTOR` que agotan sus
//! 100 ms cada uno. Todo eso se hacia **sin soltar el hilo del bus**, y en ese
//! hilo es donde se lee el raton. El propietario lo sintio como *"tirones como que
//! esta verificando mi mouse y teclado"*: doce intentos en el primer minuto,
//! doce tirones de casi un segundo.
//!
//! ## Lo que cambia
//!
//! La misma secuencia, con las mismas esperas y los mismos plazos, escrita
//! como una maquina de estados que **avanza UN paso por bombeo**: cada paso
//! lanza algo (encender, reset, un comando, una transferencia) o mira si lo
//! lanzado ya llego, y se VA. Las esperas son *"vuelve dentro de N ms"*
//! contra un reloj, no giros; los plazos de los descriptores son plazos, no
//! vueltas de un bucle. Entre paso y paso, el bombeo lee el raton y el teclado
//! como en cualquier otra vuelta.
//!
//! Es el mismo hilo: no hay un segundo escritor del xHC (el guardian
//! `escritores` lo exige). Lo que llega al anillo de eventos mientras se
//! espera lo caza el propio bombeo y lo guarda (`bmo_xhci::vigilar_*`).
//!
//! ## Por que esto se prueba sin xHC
//!
//! `Metal` es la unica puerta al hardware y tiene once verbos. Un `Metal`
//! fingido con un reloj que avanza a mano permite comprobar lo que importa
//! --que ningun paso ESPERA, que los plazos se cumplen en tiempo y no en
//! vueltas, que un aparato mudo cuesta tres lecturas y devuelve su ranura--
//! sin encender la maquina. Ver las pruebas al final.

use crate::enumera::{
    detalle_sin_descriptores, le_u16, MAX_CFG, PASO_CFG_CORTA, PASO_CFG_MENOR_DE_9, PASO_CFG_NO_CABE,
    PASO_SIN_APARATO, PASO_SIN_CABECERA,
};
use bmo_xhci::{mps0_declarado, mps0_supuesto};

/// Sin corriente antes de un reintento: lo que tarda un firmware en darse por
/// apagado (2026-09-17).
pub const PLAZO_SIN_CORRIENTE_MS: u64 = 200;
/// VBUS estable antes de fiarse de CCS (`port_power_on` esperaba lo mismo).
pub const PLAZO_VBUS_MS: u64 = 20;
/// Debounce antes del reset (USB 2.0, 7.1.7.3), solo en el primer intento.
pub const PLAZO_DEBOUNCE_MS: u64 = 100;
/// Un reset USB2 tarda 10-50 ms; se le da lo mismo que a `port_reset`.
pub const PLAZO_RESET_MS: u64 = 120;
/// Recuperacion tras el reset (USB 2.0, 7.1.7.5).
pub const PLAZO_RECUPERACION_MS: u64 = 10;
/// Un comando del controlador o una transferencia de control: lo que
/// `evt_poll_block` daba (~100 respiros de 1 ms).
pub const PLAZO_RESPUESTA_MS: u64 = 100;
/// Entre dos lecturas de un descriptor que no llego.
pub const ENTRE_LECTURAS_MS: u64 = 10;
/// Lecturas de cada descriptor antes de rendirse: las de `leer_descriptores`.
pub const LECTURAS: u8 = 3;
/// Tras el `SET_ADDRESS` de verdad, lo que se le da al aparato para asentar
/// (`bmo_xhci::PLAZO_ASENTAR_MS`).
pub const PLAZO_ASENTAR_MS: u64 = bmo_xhci::PLAZO_ASENTAR_MS;

/// Que descriptor se pide.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Descriptor {
    Dispositivo,
    Configuracion,
}

/// **Los once verbos con los que la enumeracion toca el bus.** Ninguno
/// espera: los que lanzan algo devuelven si se pudo lanzar, y los `*_llego`
/// contestan `None` mientras no haya llegado.
pub trait Metal {
    /// Milisegundos desde un origen cualquiera. Todo plazo es una resta.
    fn ahora_ms(&self) -> u64;
    fn corriente(&mut self, port: u8, encender: bool);
    fn reset_lanzar(&mut self, port: u8) -> bool;
    fn reset_acabo(&mut self, port: u8) -> bool;
    fn habilitado(&mut self, port: u8) -> bool;
    fn velocidad(&mut self, port: u8) -> u8;
    /// `Enable Slot` lanzado y vigilado.
    fn pedir_ranura(&mut self) -> bool;
    /// `Some(Some(slot))`, `Some(None)` = contesto que no, `None` = aun no.
    fn ranura_llego(&mut self) -> Option<Option<u8>>;
    /// `Address Device` lanzado y vigilado. `bsr` = sin `SET_ADDRESS`: la
    /// ranura queda en `Default` y el aparato en la direccion 0.
    fn direccionar(&mut self, port: u8, velocidad: u8, slot: u8, bsr: bool) -> bool;
    fn direccion_llego(&mut self, slot: u8) -> Option<bool>;
    /// `Reset Device` lanzado y vigilado: el xHC se entera del segundo reset.
    fn reset_device(&mut self, slot: u8) -> bool;
    fn reset_device_llego(&mut self) -> Option<bool>;
    fn devolver_ranura(&mut self, slot: u8);
    /// `GET_DESCRIPTOR` lanzado y vigilado.
    fn pedir_descriptor(&mut self, slot: u8, cual: Descriptor, largo: usize) -> bool;
    /// Los bytes que llegaron (0 = el aparato contesto con error).
    fn descriptor_llego(&mut self, buf: &mut [u8]) -> Option<usize>;
    /// El codigo de complecion del ultimo `descriptor_llego` (xHCI 6.4.2;
    /// 1 = bien, 3 = Babble, 4 = error de transaccion). Para la ficha.
    fn ultimo_cc(&self) -> u8;
    /// `Evaluate Context`: el EP0 pasa a `mps` bytes de paquete. Lanzado y
    /// vigilado.
    fn evaluar_mps0(&mut self, slot: u8, mps: u16) -> bool;
    fn evaluacion_llego(&mut self) -> Option<bool>;
    /// El plazo se agoto: lo lanzado ya no lo espera nadie.
    fn dejar_de_esperar(&mut self);
    fn log(&self, msg: &str);
    fn log_u64(&self, msg: &str, v: u64);
}

/// En que punto va la enumeracion. Cada variante es un paso que se da y se
/// vuelve; las `Esperando*` miran y, si no ha llegado, se vuelven igual.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Paso {
    Apagar,
    SinCorriente,
    Encender,
    Encendido,
    Reset,
    Reseteando,
    Recuperando,
    PedirRanura,
    EsperandoRanura,
    /// `Address Device` con BSR = 1: la ranura en `Default`, el aparato en la
    /// direccion 0. El paso 2 del esquema de Windows.
    Direccionar,
    EsperandoDireccion,
    /// 64 bytes del descriptor del aparato EN LA DIRECCION 0: un aparato de
    /// paquete 8 contesta 8 y para; uno de 64, los 18. Ver
    /// `bmo_xhci::mps0_supuesto`.
    PedirOcho,
    EsperandoOcho,
    PausaOcho,
    /// El paquete declarado no es el supuesto: `Evaluate Context`.
    Evaluar,
    EsperandoEvaluar,
    /// El SEGUNDO reset (lo que el aparato espera), y que el xHC lo sepa.
    Reset2,
    Reseteando2,
    Recuperando2,
    ResetDevice,
    EsperandoResetDevice,
    /// `Address Device` con BSR = 0: ahora si, `SET_ADDRESS`, y 10 ms.
    Direccionar2,
    EsperandoDireccion2,
    Asentando,
    PedirDispositivo,
    EsperandoDispositivo,
    PausaDispositivo,
    PedirCabecera,
    EsperandoCabecera,
    PausaCabecera,
    PedirEntera,
    EsperandoEntera,
    Terminada,
}

/// Lo que dijo un paso.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Marcha {
    /// Vuelve en el bombeo siguiente.
    Sigue,
    /// El aparato no acepto direccion (o el puerto no reseteo, o no hay
    /// ranuras): `VEREDICTO_SIN_DIRECCION`. La ranura, si la hubo, ya esta
    /// devuelta.
    SinDireccion,
    /// Con direccion y sin descriptores: `VEREDICTO_SIN_DESCRIPTORES`, con el
    /// detalle de en que paso (`enumera::detalle_sin_descriptores`). La
    /// ranura ya esta devuelta.
    SinDescriptores(u16),
    /// Descriptores en la mano: `slot()`, `cfg()`, `cfg_val()`, `vid_pid()`.
    Lista,
}

/// La enumeracion de UN puerto, a medias. Vive en `UsbHidHal::en_curso`
/// mientras dura.
pub struct Enumeracion {
    port: u8,
    reintento: bool,
    paso: Paso,
    /// Hasta cuando (ms del reloj de `Metal`) espera el paso actual.
    hasta: u64,
    lecturas: u8,
    slot: u8,
    velocidad: u8,
    /// El paquete del EP0 que el aparato declaro, si no es el supuesto.
    mps0: u16,
    empezo: u64,
    pasos: u32,
    dev_desc: [u8; 18],
    n_dev: usize,
    cfg_hdr: [u8; 9],
    total_len: usize,
    cfg: [u8; MAX_CFG],
}

impl Enumeracion {
    /// Empieza en `port`. `reintento` = del segundo intento en adelante: con
    /// corte de corriente y sin debounce (ver `direccionar_puerto`).
    pub fn nueva(port: u8, reintento: bool, ahora: u64) -> Self {
        Self {
            port,
            reintento,
            paso: if reintento { Paso::Apagar } else { Paso::Encender },
            hasta: 0,
            lecturas: 0,
            slot: 0,
            velocidad: 0,
            mps0: 0,
            empezo: ahora,
            pasos: 0,
            dev_desc: [0; 18],
            n_dev: 0,
            cfg_hdr: [0; 9],
            total_len: 0,
            cfg: [0; MAX_CFG],
        }
    }

    pub fn port(&self) -> u8 {
        self.port
    }
    pub fn slot(&self) -> u8 {
        self.slot
    }
    /// La configuracion entera, tal como la mando el aparato.
    pub fn cfg(&self) -> &[u8] {
        &self.cfg[..self.total_len.min(MAX_CFG)]
    }
    pub fn cfg_val(&self) -> u8 {
        self.cfg_hdr[5]
    }
    /// `(idVendor, idProduct)`, o ceros si el descriptor vino corto.
    pub fn vid_pid(&self) -> (u16, u16) {
        if self.n_dev >= 12 {
            (le_u16(&self.dev_desc, 8), le_u16(&self.dev_desc, 10))
        } else {
            (0, 0)
        }
    }
    /// Cuantos pasos ha dado.
    pub fn pasos(&self) -> u32 {
        self.pasos
    }
    /// Cuanto lleva, en ms del reloj de `Metal`.
    pub fn lleva_ms(&self, ahora: u64) -> u64 {
        ahora.saturating_sub(self.empezo)
    }

    /// **Un paso.** Lanza o mira, y se vuelve. Nunca espera.
    pub fn avanzar(&mut self, m: &mut dyn Metal) -> Marcha {
        self.pasos = self.pasos.wrapping_add(1);
        let ahora = m.ahora_ms();
        let port = self.port;
        match self.paso {
            Paso::Apagar => {
                // ** SEGUNDO INTENTO: SE LE QUITA LA CORRIENTE (2026-09-17). Un
                // aparato que se quedo a medias no vuelve por resetearlo otra
                // vez, vuelve por apagarlo. Es lo que hace la mano al sacar y
                // meter el cable.
                m.log_u64("[uhid] reintento: corto la corriente del puerto ", port as u64);
                m.corriente(port, false);
                self.esperar(ahora, PLAZO_SIN_CORRIENTE_MS, Paso::SinCorriente)
            }
            Paso::SinCorriente => self.si_cumplio(ahora, Paso::Encender),
            Paso::Encender => {
                m.corriente(port, true);
                // VBUS estable, y el DEBOUNCE (USB 2.0, 7.1.7.3) solo la
                // primera vez: en un reintento el aparato lleva segundos
                // enchufado, o acaba de recibir su corte de corriente.
                let plazo = PLAZO_VBUS_MS + if self.reintento { 0 } else { PLAZO_DEBOUNCE_MS };
                self.esperar(ahora, plazo, Paso::Encendido)
            }
            Paso::Encendido => self.si_cumplio(ahora, Paso::Reset),
            Paso::Reset => {
                if !m.reset_lanzar(port) {
                    m.log_u64("[uhid] puerto sin reset: ", port as u64);
                    return self.acabar(Marcha::SinDireccion);
                }
                self.esperar(ahora, PLAZO_RESET_MS, Paso::Reseteando)
            }
            Paso::Reseteando => {
                if m.reset_acabo(port) {
                    return self.esperar(ahora, PLAZO_RECUPERACION_MS, Paso::Recuperando);
                }
                if ahora >= self.hasta {
                    m.log_u64("[uhid] puerto sin reset: ", port as u64);
                    return self.acabar(Marcha::SinDireccion);
                }
                Marcha::Sigue
            }
            Paso::Recuperando => {
                if ahora < self.hasta {
                    return Marcha::Sigue;
                }
                if !m.habilitado(port) {
                    m.log_u64("[uhid] puerto sin reset: ", port as u64);
                    return self.acabar(Marcha::SinDireccion);
                }
                self.velocidad = m.velocidad(port);
                if self.velocidad == 0 {
                    return self.acabar(Marcha::SinDireccion);
                }
                m.log_u64("[uhid] puerto con algo: ", port as u64);
                self.paso = Paso::PedirRanura;
                Marcha::Sigue
            }
            Paso::PedirRanura => {
                if !m.pedir_ranura() {
                    return self.no_acepta(m);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoRanura)
            }
            Paso::EsperandoRanura => match m.ranura_llego() {
                Some(Some(s)) => {
                    self.slot = s;
                    self.paso = Paso::Direccionar;
                    Marcha::Sigue
                }
                Some(None) => self.no_acepta(m),
                None => self.o_plazo(m, ahora),
            },
            Paso::Direccionar => {
                if !m.direccionar(port, self.velocidad, self.slot, true) {
                    return self.no_acepta(m);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoDireccion)
            }
            Paso::EsperandoDireccion => match m.direccion_llego(self.slot) {
                Some(true) => {
                    m.log_u64("[uhid] slot (direccion 0)=", self.slot as u64);
                    self.lecturas = 0;
                    self.paso = Paso::PedirOcho;
                    Marcha::Sigue
                }
                Some(false) => self.no_acepta(m),
                None => self.o_plazo(m, ahora),
            },
            Paso::PedirOcho => {
                self.lecturas += 1;
                if !m.pedir_descriptor(self.slot, Descriptor::Dispositivo, 64) {
                    return self.sin_descriptores(m, "[uhid] no dev desc\n", PASO_SIN_APARATO);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoOcho)
            }
            Paso::EsperandoOcho => {
                let mut buf = [0u8; 64];
                match m.descriptor_llego(&mut buf) {
                    Some(n) if n >= 8 => {
                        self.lecturas = 0;
                        // El byte 7 es el paquete de verdad del EP0.
                        let declarado = mps0_declarado(buf[7], self.velocidad);
                        if declarado != mps0_supuesto(self.velocidad) && declarado != 0 {
                            m.log_u64("[uhid] mps0 declarado=", declarado as u64);
                            self.mps0 = declarado;
                            self.paso = Paso::Evaluar;
                        } else {
                            self.paso = Paso::Reset2;
                        }
                        Marcha::Sigue
                    }
                    Some(_) => self.otra_lectura(m, ahora, Paso::PausaOcho, "[uhid] no dev desc\n", PASO_SIN_APARATO),
                    None => {
                        if ahora < self.hasta {
                            return Marcha::Sigue;
                        }
                        m.dejar_de_esperar();
                        self.otra_lectura(m, ahora, Paso::PausaOcho, "[uhid] no dev desc\n", PASO_SIN_APARATO)
                    }
                }
            }
            Paso::PausaOcho => self.si_cumplio(ahora, Paso::PedirOcho),
            Paso::Evaluar => {
                if !m.evaluar_mps0(self.slot, self.mps0) {
                    return self.no_acepta(m);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoEvaluar)
            }
            Paso::EsperandoEvaluar => match m.evaluacion_llego() {
                Some(true) => {
                    self.paso = Paso::Reset2;
                    Marcha::Sigue
                }
                Some(false) => {
                    m.log("[uhid] evaluate context FALLO\n");
                    self.no_acepta(m)
                }
                None => self.o_plazo(m, ahora),
            },
            Paso::Reset2 => {
                if !m.reset_lanzar(port) {
                    m.log_u64("[uhid] segundo reset: puerto vacio ", port as u64);
                    return self.no_acepta(m);
                }
                self.esperar(ahora, PLAZO_RESET_MS, Paso::Reseteando2)
            }
            Paso::Reseteando2 => {
                if m.reset_acabo(port) {
                    return self.esperar(ahora, PLAZO_RECUPERACION_MS, Paso::Recuperando2);
                }
                if ahora >= self.hasta {
                    m.log_u64("[uhid] segundo reset sin acabar: ", port as u64);
                    return self.no_acepta(m);
                }
                Marcha::Sigue
            }
            Paso::Recuperando2 => {
                if ahora < self.hasta {
                    return Marcha::Sigue;
                }
                if !m.habilitado(port) {
                    m.log_u64("[uhid] tras el segundo reset, sin habilitar: ", port as u64);
                    return self.no_acepta(m);
                }
                self.paso = Paso::ResetDevice;
                Marcha::Sigue
            }
            Paso::ResetDevice => {
                if !m.reset_device(self.slot) {
                    return self.no_acepta(m);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoResetDevice)
            }
            Paso::EsperandoResetDevice => match m.reset_device_llego() {
                Some(true) => {
                    self.paso = Paso::Direccionar2;
                    Marcha::Sigue
                }
                Some(false) => {
                    m.log("[uhid] reset device FALLO\n");
                    self.no_acepta(m)
                }
                None => self.o_plazo(m, ahora),
            },
            Paso::Direccionar2 => {
                if !m.direccionar(port, self.velocidad, self.slot, false) {
                    return self.no_acepta(m);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoDireccion2)
            }
            Paso::EsperandoDireccion2 => match m.direccion_llego(self.slot) {
                Some(true) => {
                    m.log_u64("[uhid] slot=", self.slot as u64);
                    self.lecturas = 0;
                    self.esperar(ahora, PLAZO_ASENTAR_MS, Paso::Asentando)
                }
                Some(false) => self.no_acepta(m),
                None => self.o_plazo(m, ahora),
            },
            Paso::Asentando => self.si_cumplio(ahora, Paso::PedirDispositivo),
            Paso::PedirDispositivo => {
                self.lecturas += 1;
                if !m.pedir_descriptor(self.slot, Descriptor::Dispositivo, 18) {
                    return self.sin_descriptores(m, "[uhid] no dev desc\n", PASO_SIN_APARATO);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoDispositivo)
            }
            Paso::EsperandoDispositivo => {
                let mut buf = [0u8; 18];
                match m.descriptor_llego(&mut buf) {
                    Some(n) if n >= 8 => {
                        self.dev_desc = buf;
                        self.n_dev = n;
                        m.log_u64(" class=", buf[4] as u64);
                        self.lecturas = 0;
                        self.paso = Paso::PedirCabecera;
                        Marcha::Sigue
                    }
                    Some(_) => self.otra_lectura(m, ahora, Paso::PausaDispositivo, "[uhid] no dev desc\n", PASO_SIN_APARATO),
                    None => {
                        if ahora < self.hasta {
                            return Marcha::Sigue;
                        }
                        m.dejar_de_esperar();
                        self.otra_lectura(m, ahora, Paso::PausaDispositivo, "[uhid] no dev desc\n", PASO_SIN_APARATO)
                    }
                }
            }
            Paso::PausaDispositivo => self.si_cumplio(ahora, Paso::PedirDispositivo),
            Paso::PedirCabecera => {
                self.lecturas += 1;
                if !m.pedir_descriptor(self.slot, Descriptor::Configuracion, 9) {
                    return self.sin_descriptores(m, "[uhid] no cfg hdr\n", PASO_SIN_CABECERA);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoCabecera)
            }
            Paso::EsperandoCabecera => {
                let mut buf = [0u8; 9];
                match m.descriptor_llego(&mut buf) {
                    Some(n) if n >= 9 => {
                        self.cfg_hdr = buf;
                        self.total_len = le_u16(&buf, 2) as usize;
                        m.log_u64(" cfg_val=", buf[5] as u64);
                        m.log_u64(" total_len=", self.total_len as u64);
                        // ** Una configuracion es al menos su propia cabecera
                        // de nueve bytes; menos es un aparato mintiendo sobre
                        // si mismo. Y mas que `MAX_CFG` no cabe.
                        if self.total_len < 9 {
                            return self.sin_descriptores(m, "[uhid] cfg too small\n", PASO_CFG_MENOR_DE_9);
                        }
                        if self.total_len > MAX_CFG {
                            m.log_u64("[uhid] cfg too big: ", self.total_len as u64);
                            return self.sin_descriptores(m, "[uhid] cfg too big\n", PASO_CFG_NO_CABE);
                        }
                        self.paso = Paso::PedirEntera;
                        Marcha::Sigue
                    }
                    Some(_) => self.otra_lectura(m, ahora, Paso::PausaCabecera, "[uhid] no cfg hdr\n", PASO_SIN_CABECERA),
                    None => {
                        if ahora < self.hasta {
                            return Marcha::Sigue;
                        }
                        m.dejar_de_esperar();
                        self.otra_lectura(m, ahora, Paso::PausaCabecera, "[uhid] no cfg hdr\n", PASO_SIN_CABECERA)
                    }
                }
            }
            Paso::PausaCabecera => self.si_cumplio(ahora, Paso::PedirCabecera),
            Paso::PedirEntera => {
                if !m.pedir_descriptor(self.slot, Descriptor::Configuracion, self.total_len) {
                    return self.sin_descriptores(m, "[uhid] cfg short\n", PASO_CFG_CORTA);
                }
                self.esperar(ahora, PLAZO_RESPUESTA_MS, Paso::EsperandoEntera)
            }
            Paso::EsperandoEntera => {
                let total = self.total_len;
                match m.descriptor_llego(&mut self.cfg[..total]) {
                    Some(n) if n >= total => {
                        self.paso = Paso::Terminada;
                        Marcha::Lista
                    }
                    // La entera se pide UNA vez, como en `leer_descriptores`:
                    // un aparato que dio la cabecera y no da el resto no va a
                    // darlo por insistir.
                    Some(_) => self.sin_descriptores(m, "[uhid] cfg short\n", PASO_CFG_CORTA),
                    None => {
                        if ahora < self.hasta {
                            return Marcha::Sigue;
                        }
                        m.dejar_de_esperar();
                        self.sin_descriptores(m, "[uhid] cfg short\n", PASO_CFG_CORTA)
                    }
                }
            }
            Paso::Terminada => Marcha::Lista,
        }
    }

    /// **Se abandona a medias** (el puerto se desenchufo): lo lanzado deja de
    /// esperarse y la ranura, si la habia, se devuelve.
    pub fn abandonar(&mut self, m: &mut dyn Metal) {
        m.dejar_de_esperar();
        if self.slot != 0 {
            m.devolver_ranura(self.slot);
            self.slot = 0;
        }
        self.paso = Paso::Terminada;
    }

    fn esperar(&mut self, ahora: u64, ms: u64, luego: Paso) -> Marcha {
        self.hasta = ahora.saturating_add(ms);
        self.paso = luego;
        Marcha::Sigue
    }

    fn si_cumplio(&mut self, ahora: u64, luego: Paso) -> Marcha {
        if ahora >= self.hasta {
            self.paso = luego;
        }
        Marcha::Sigue
    }

    /// Esperando un comando: si el plazo paso, el aparato no acepta direccion.
    fn o_plazo(&mut self, m: &mut dyn Metal, ahora: u64) -> Marcha {
        if ahora < self.hasta {
            return Marcha::Sigue;
        }
        m.dejar_de_esperar();
        self.no_acepta(m)
    }

    fn no_acepta(&mut self, m: &mut dyn Metal) -> Marcha {
        m.log_u64("[uhid] NO acepta direccion, puerto ", self.port as u64);
        self.devolver(m);
        self.acabar(Marcha::SinDireccion)
    }

    fn sin_descriptores(&mut self, m: &mut dyn Metal, motivo: &str, paso: u16) -> Marcha {
        m.log(motivo);
        self.devolver(m);
        // En los pasos sin largo declarado (1 y 2) el sitio del largo lleva
        // el `cc` de la ultima respuesta: 3 = Babble (el paquete era mas
        // grande de lo supuesto), 4 = error de transaccion, 254 = no
        // contesto. Es lo que separa "mudo" de "hablamos distinto".
        let extra = if paso == PASO_SIN_APARATO || paso == PASO_SIN_CABECERA {
            m.ultimo_cc() as usize
        } else {
            self.total_len
        };
        let detalle = detalle_sin_descriptores(paso, extra);
        self.acabar(Marcha::SinDescriptores(detalle))
    }

    /// Otra lectura del mismo descriptor tras `ENTRE_LECTURAS_MS`, o rendirse
    /// si ya fueron `LECTURAS`.
    fn otra_lectura(&mut self, m: &mut dyn Metal, ahora: u64, pausa: Paso, motivo: &str, paso: u16) -> Marcha {
        if self.lecturas >= LECTURAS {
            return self.sin_descriptores(m, motivo, paso);
        }
        self.esperar(ahora, ENTRE_LECTURAS_MS, pausa)
    }

    /// * **Si algo falla despues de tener la ranura, la ranura se DEVUELVE.**
    /// La misma regla que `address_device`: dejarla pedida es lo que agoto
    /// las 64 del controlador el 2026-07-31.
    fn devolver(&mut self, m: &mut dyn Metal) {
        if self.slot != 0 {
            m.devolver_ranura(self.slot);
            self.slot = 0;
        }
    }

    fn acabar(&mut self, como: Marcha) -> Marcha {
        self.paso = Paso::Terminada;
        como
    }
}

// ===================================================================
//  El Metal de verdad: bmo_xhci
// ===================================================================

/// Los once verbos sobre el xHC. Guarda la transferencia en vuelo entre un
/// bombeo y el siguiente, que es lo unico que la maquina no puede guardar
/// sin conocer el controlador.
pub struct Xhc {
    vuelo: Option<bmo_xhci::EnVuelo>,
    /// El `cc` de la ultima transferencia de control que llego; 254 si la
    /// ultima espera se agoto sin respuesta.
    ultimo_cc: u8,
}

impl Xhc {
    pub const fn nuevo() -> Self {
        Self { vuelo: None, ultimo_cc: 0 }
    }
}

impl Metal for Xhc {
    fn ahora_ms(&self) -> u64 {
        bmo_xhci::hal().ahora_ms()
    }
    fn corriente(&mut self, port: u8, encender: bool) {
        // `port_power_solo`, no `port_power_on`: los 20 ms de VBUS los pone
        // la maquina como plazo, no como espera.
        unsafe {
            if encender {
                bmo_xhci::port_power_solo(port);
            } else {
                bmo_xhci::port_power_off(port);
            }
        }
    }
    fn reset_lanzar(&mut self, port: u8) -> bool {
        unsafe { bmo_xhci::port_reset_lanzar(port) }
    }
    fn reset_acabo(&mut self, port: u8) -> bool {
        unsafe { bmo_xhci::port_reset_acabo(port) }
    }
    fn habilitado(&mut self, port: u8) -> bool {
        unsafe { bmo_xhci::port_habilitado(port) }
    }
    fn velocidad(&mut self, port: u8) -> u8 {
        unsafe { bmo_xhci::port_speed(port) }
    }
    fn pedir_ranura(&mut self) -> bool {
        match unsafe { bmo_xhci::enable_slot_lanzar() } {
            Some(trb) => {
                bmo_xhci::vigilar_comando(trb);
                true
            }
            None => false,
        }
    }
    fn ranura_llego(&mut self) -> Option<Option<u8>> {
        let ev = unsafe { bmo_xhci::vigilado_llego()? };
        Some(bmo_xhci::slot_de_complecion(&ev))
    }
    fn direccionar(&mut self, port: u8, velocidad: u8, slot: u8, bsr: bool) -> bool {
        match unsafe { bmo_xhci::address_lanzar(port, velocidad, slot, bsr) } {
            Some(trb) => {
                bmo_xhci::vigilar_comando(trb);
                true
            }
            None => false,
        }
    }
    fn direccion_llego(&mut self, slot: u8) -> Option<bool> {
        let ev = unsafe { bmo_xhci::vigilado_llego()? };
        Some(unsafe { bmo_xhci::address_rematar(slot, &ev) })
    }
    fn reset_device(&mut self, slot: u8) -> bool {
        match unsafe { bmo_xhci::reset_device_lanzar(slot) } {
            Some(trb) => {
                bmo_xhci::vigilar_comando(trb);
                true
            }
            None => false,
        }
    }
    fn reset_device_llego(&mut self) -> Option<bool> {
        let ev = unsafe { bmo_xhci::vigilado_llego()? };
        Some(bmo_xhci::cc_de(&ev) == 1)
    }
    fn devolver_ranura(&mut self, slot: u8) {
        unsafe {
            bmo_xhci::disable_slot(slot);
        }
    }
    fn pedir_descriptor(&mut self, slot: u8, cual: Descriptor, largo: usize) -> bool {
        let tipo = match cual {
            Descriptor::Dispositivo => bmo_xhci::DESC_DEVICE,
            Descriptor::Configuracion => bmo_xhci::DESC_CONFIG,
        };
        self.vuelo = unsafe { bmo_xhci::get_descriptor_lanzar(slot, tipo, 0, largo) };
        if self.vuelo.is_some() {
            bmo_xhci::vigilar_transferencia(slot);
            true
        } else {
            false
        }
    }
    fn descriptor_llego(&mut self, buf: &mut [u8]) -> Option<usize> {
        let ev = unsafe { bmo_xhci::vigilado_llego()? };
        let vuelo = self.vuelo.take()?;
        self.ultimo_cc = bmo_xhci::cc_de(&ev);
        Some(unsafe { bmo_xhci::control_rematar(&vuelo, &ev, buf) })
    }
    fn ultimo_cc(&self) -> u8 {
        self.ultimo_cc
    }
    fn evaluar_mps0(&mut self, slot: u8, mps: u16) -> bool {
        match unsafe { bmo_xhci::evaluar_mps0_lanzar(slot, mps) } {
            Some(trb) => {
                bmo_xhci::vigilar_comando(trb);
                true
            }
            None => false,
        }
    }
    fn evaluacion_llego(&mut self) -> Option<bool> {
        let ev = unsafe { bmo_xhci::vigilado_llego()? };
        Some(bmo_xhci::cc_de(&ev) == 1)
    }
    fn dejar_de_esperar(&mut self) {
        bmo_xhci::dejar_de_vigilar();
        self.vuelo = None;
        // Un plazo agotado es "no contesto", y asi lo dira la ficha.
        self.ultimo_cc = 254;
    }
    fn log(&self, msg: &str) {
        bmo_xhci::hal().log(msg);
    }
    fn log_u64(&self, msg: &str, v: u64) {
        bmo_xhci::hal().log_u64(msg, v);
    }
}

// ===================================================================
//  Pruebas: un Metal fingido con reloj a mano
// ===================================================================

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// Como se porta el aparato fingido.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Aparato {
        /// Contesta a todo: un teclado sano.
        Sano,
        /// Acepta direccion y no da descriptores: el del puerto 1 del Ryzen.
        Mudo,
        /// No hay nada: el reset no se lanza.
        Vacio,
        /// Contesta, y su configuracion declara mas de `MAX_CFG`: el audifono
        /// 7.1 que se salia de los 512 de antes.
        Grande,
        /// Full Speed con paquete de EP0 de 64: contesta los 8 primeros
        /// bytes, y los 18 de golpe SOLO despues del `Evaluate Context`
        /// (antes, Babble). El audifono del puerto 1, si es lo que parece.
        Paquete64,
    }

    struct Fingido {
        aparato: Aparato,
        ahora: u64,
        /// Cuando termina el reset lanzado (el PR tarda en limpiarse).
        reset_hasta: Option<u64>,
        /// Lo que hay en vuelo: (cuando llega, que es).
        vuelo: Option<(u64, Vuelo)>,
        ranuras_pedidas: u32,
        ranuras_devueltas: u32,
        descriptores_pedidos: u32,
        cortes: u32,
        encendidos: u32,
        dejo_de_esperar: u32,
        /// Cuanto tardo cada llamada a `avanzar` (la prueba lo mide fuera).
        eventos: Vec<&'static str>,
        /// El paquete que el xHC cree que tiene el EP0 (64 al direccionar un
        /// Full Speed: el esquema de Windows).
        mps0_xhc: u16,
        ultimo_cc: u8,
        /// Los `Address Device` que se mandaron, con su BSR.
        direcciones: Vec<bool>,
        resets_device: u32,
    }

    #[derive(Clone, Copy)]
    enum Vuelo {
        Ranura,
        Direccion,
        Descriptor(Descriptor, usize),
        Evaluacion(u16),
        ResetDevice,
    }

    impl Fingido {
        fn nuevo(aparato: Aparato) -> Self {
            Self {
                aparato,
                ahora: 1_000,
                reset_hasta: None,
                vuelo: None,
                ranuras_pedidas: 0,
                ranuras_devueltas: 0,
                descriptores_pedidos: 0,
                cortes: 0,
                encendidos: 0,
                dejo_de_esperar: 0,
                eventos: Vec::new(),
                mps0_xhc: 64,
                ultimo_cc: 0,
                direcciones: Vec::new(),
                resets_device: 0,
            }
        }
        /// El paquete de EP0 del aparato fingido: 8 (teclado, raton) o 64
        /// (el audifono).
        fn mps0_real(&self) -> usize {
            if self.aparato == Aparato::Paquete64 { 64 } else { 8 }
        }
        /// El bombeo: cada 4 ms un paso, hasta que acabe o pasen `tope` ms.
        fn bombear(&mut self, e: &mut Enumeracion, tope: u64) -> Marcha {
            let fin = self.ahora + tope;
            loop {
                let m = e.avanzar(self);
                if m != Marcha::Sigue {
                    return m;
                }
                self.ahora += 4;
                assert!(self.ahora < fin, "la enumeracion no termina en {} ms", tope);
            }
        }
        fn llego(&mut self) -> Option<Vuelo> {
            let (cuando, que) = self.vuelo?;
            if self.ahora >= cuando {
                self.vuelo = None;
                Some(que)
            } else {
                None
            }
        }
    }

    impl Metal for Fingido {
        fn ahora_ms(&self) -> u64 {
            self.ahora
        }
        fn corriente(&mut self, _port: u8, encender: bool) {
            if encender {
                self.encendidos += 1;
                self.eventos.push("encender");
            } else {
                self.cortes += 1;
                self.eventos.push("cortar");
            }
        }
        fn reset_lanzar(&mut self, _port: u8) -> bool {
            if self.aparato == Aparato::Vacio {
                return false;
            }
            self.eventos.push("reset");
            // Un reset USB2 de 30 ms.
            self.reset_hasta = Some(self.ahora + 30);
            true
        }
        fn reset_acabo(&mut self, _port: u8) -> bool {
            self.reset_hasta.is_some_and(|h| self.ahora >= h)
        }
        fn habilitado(&mut self, _port: u8) -> bool {
            true
        }
        fn velocidad(&mut self, _port: u8) -> u8 {
            // Full Speed: el paquete supuesto es 64, y un teclado de 8 tiene
            // que pasar por el evaluate. Es el caso que importa.
            1
        }
        fn pedir_ranura(&mut self) -> bool {
            self.ranuras_pedidas += 1;
            self.eventos.push("enable_slot");
            self.vuelo = Some((self.ahora + 1, Vuelo::Ranura));
            true
        }
        fn ranura_llego(&mut self) -> Option<Option<u8>> {
            match self.llego()? {
                Vuelo::Ranura => Some(Some(7)),
                _ => panic!("se esperaba la ranura"),
            }
        }
        fn direccionar(&mut self, _port: u8, _v: u8, slot: u8, bsr: bool) -> bool {
            assert_eq!(slot, 7);
            self.eventos.push(if bsr { "address0" } else { "address" });
            self.direcciones.push(bsr);
            self.vuelo = Some((self.ahora + 1, Vuelo::Direccion));
            true
        }
        fn reset_device(&mut self, _slot: u8) -> bool {
            self.eventos.push("reset_device");
            self.resets_device += 1;
            self.vuelo = Some((self.ahora + 1, Vuelo::ResetDevice));
            true
        }
        fn reset_device_llego(&mut self) -> Option<bool> {
            match self.llego()? {
                Vuelo::ResetDevice => Some(true),
                _ => panic!("se esperaba el reset device"),
            }
        }
        fn direccion_llego(&mut self, _slot: u8) -> Option<bool> {
            match self.llego()? {
                Vuelo::Direccion => Some(true),
                _ => panic!("se esperaba la direccion"),
            }
        }
        fn devolver_ranura(&mut self, slot: u8) {
            assert_eq!(slot, 7);
            self.ranuras_devueltas += 1;
            self.eventos.push("disable_slot");
        }
        fn pedir_descriptor(&mut self, _slot: u8, cual: Descriptor, largo: usize) -> bool {
            self.descriptores_pedidos += 1;
            self.eventos.push(match cual {
                Descriptor::Dispositivo => "get_dev",
                Descriptor::Configuracion => "get_cfg",
            });
            // Un mudo no contesta NUNCA: la maquina tiene que rendirse por
            // plazo. Un sano contesta en 1 ms.
            let cuando = if self.aparato == Aparato::Mudo { u64::MAX } else { self.ahora + 1 };
            self.vuelo = Some((cuando, Vuelo::Descriptor(cual, largo)));
            true
        }
        fn descriptor_llego(&mut self, buf: &mut [u8]) -> Option<usize> {
            match self.llego()? {
                Vuelo::Descriptor(Descriptor::Dispositivo, n) => {
                    let mut d = [18u8, 1, 0, 2, 0, 0, 0, 8, 0x6D, 0x04, 0x77, 0xC0, 0, 0, 0, 0, 0, 1];
                    let real = self.mps0_real();
                    d[7] = real as u8;
                    // El aparato contesta en paquetes de SU medida. Si el
                    // suyo es mayor que el que el xHC cree, el primer paquete
                    // ya se pasa: Babble (cc = 3), sin datos. Si es menor, su
                    // primer paquete es CORTO y cierra la transferencia: solo
                    // llegan esos bytes, y es legal (cc = 13).
                    if real > self.mps0_xhc as usize && n > self.mps0_xhc as usize {
                        self.ultimo_cc = 3;
                        return Some(0);
                    }
                    let llegan = if real < self.mps0_xhc as usize { n.min(real) } else { n.min(18) };
                    self.ultimo_cc = if llegan < n { 13 } else { 1 };
                    buf[..llegan].copy_from_slice(&d[..llegan]);
                    Some(llegan)
                }
                Vuelo::Descriptor(Descriptor::Configuracion, n) => {
                    // Cabecera de 9 con total 34, y el resto relleno. El
                    // Grande declara 1500: mas de lo que cabe.
                    let mut c = [0xAAu8; 64];
                    c[0] = 9;
                    c[1] = 2;
                    let total: u16 = if self.aparato == Aparato::Grande { 1500 } else { 34 };
                    c[2] = (total & 0xFF) as u8;
                    c[3] = (total >> 8) as u8;
                    c[5] = 1;
                    buf[..n].copy_from_slice(&c[..n]);
                    Some(n)
                }
                _ => panic!("se esperaba un descriptor"),
            }
        }
        fn ultimo_cc(&self) -> u8 {
            self.ultimo_cc
        }
        fn evaluar_mps0(&mut self, _slot: u8, mps: u16) -> bool {
            self.eventos.push("evaluate");
            self.vuelo = Some((self.ahora + 1, Vuelo::Evaluacion(mps)));
            true
        }
        fn evaluacion_llego(&mut self) -> Option<bool> {
            match self.llego()? {
                Vuelo::Evaluacion(mps) => {
                    self.mps0_xhc = mps;
                    Some(true)
                }
                _ => panic!("se esperaba la evaluacion"),
            }
        }
        fn dejar_de_esperar(&mut self) {
            self.dejo_de_esperar += 1;
            self.vuelo = None;
            self.ultimo_cc = 254;
        }
        fn log(&self, _m: &str) {}
        fn log_u64(&self, _m: &str, _v: u64) {}
    }

    #[test]
    fn un_aparato_sano_llega_a_lista_con_su_configuracion() {
        let mut m = Fingido::nuevo(Aparato::Sano);
        let mut e = Enumeracion::nueva(0, false, m.ahora);
        let r = m.bombear(&mut e, 2_000);
        assert_eq!(r, Marcha::Lista);
        assert_eq!(e.slot(), 7);
        assert_eq!(e.cfg_val(), 1);
        assert_eq!(e.cfg().len(), 34);
        assert_eq!(e.vid_pid(), (0x046D, 0xC077));
        // El esquema de Windows, entero: direccion 0, 64 bytes (un teclado
        // de paquete 8 contesta 8: hay que decirselo al xHC), segundo reset,
        // Reset Device, la direccion de verdad, y entonces los descriptores.
        assert_eq!(
            m.eventos,
            [
                "encender", "reset", "enable_slot", "address0", "get_dev", "evaluate", "reset",
                "reset_device", "address", "get_dev", "get_cfg", "get_cfg"
            ]
        );
        assert_eq!(m.direcciones, [true, false]);
        assert_eq!(m.mps0_xhc, 8);
        // La ranura NO se devuelve: es del aparato que se va a instalar.
        assert_eq!(m.ranuras_devueltas, 0);
        // Y tardo lo que tardan sus plazos, no mas: 20 VBUS + 100 debounce +
        // dos resets de 30 + 10 + 10 de asentar + respuestas de 1 ms, en
        // pasos de 4.
        let ms = e.lleva_ms(m.ahora);
        assert!((260..340).contains(&ms), "tardo {} ms", ms);
    }

    #[test]
    fn el_mudo_cuesta_tres_lecturas_de_100_ms_y_devuelve_la_ranura() {
        let mut m = Fingido::nuevo(Aparato::Mudo);
        let mut e = Enumeracion::nueva(1, false, m.ahora);
        let r = m.bombear(&mut e, 2_000);
        // Y el detalle dice DONDE y COMO: ni el descriptor del aparato (paso
        // 1), y el `cc` 254 = el plazo se agoto sin respuesta.
        assert_eq!(r, Marcha::SinDescriptores(detalle_sin_descriptores(PASO_SIN_APARATO, 254)));
        assert_eq!(m.descriptores_pedidos, LECTURAS as u32);
        assert_eq!(m.dejo_de_esperar, LECTURAS as u32);
        assert_eq!(m.ranuras_pedidas, 1);
        assert_eq!(m.ranuras_devueltas, 1);
        // Tres plazos de 100 ms con 10 ms entre ellos, mas lo de antes: es
        // el mismo tiempo de pared que `leer_descriptores`...
        let ms = e.lleva_ms(m.ahora);
        assert!((460..560).contains(&ms), "tardo {} ms", ms);
        // ...repartido en pasos de 4 ms: mas de cien vueltas en las que el
        // bombeo leyo el raton. Antes era UNA vuelta de 933 ms.
        assert!(e.pasos() > 100, "solo {} pasos", e.pasos());
    }

    #[test]
    fn el_reintento_corta_la_corriente_y_no_hace_debounce() {
        let mut m = Fingido::nuevo(Aparato::Sano);
        let mut e = Enumeracion::nueva(1, true, m.ahora);
        assert_eq!(m.bombear(&mut e, 2_000), Marcha::Lista);
        assert_eq!(m.cortes, 1);
        assert_eq!(m.encendidos, 1);
        assert_eq!(&m.eventos[..3], ["cortar", "encender", "reset"]);
        // 200 sin corriente + 20 VBUS (sin los 100 de debounce) + los dos
        // resets + asentar, en pasos de 4.
        let ms = e.lleva_ms(m.ahora);
        assert!((360..440).contains(&ms), "tardo {} ms", ms);
    }

    #[test]
    fn una_configuracion_que_no_cabe_lo_dice_con_su_largo() {
        let mut m = Fingido::nuevo(Aparato::Grande);
        let mut e = Enumeracion::nueva(1, false, m.ahora);
        let r = m.bombear(&mut e, 2_000);
        assert_eq!(r, Marcha::SinDescriptores(detalle_sin_descriptores(PASO_CFG_NO_CABE, 1500)));
        // Se supo en la cabecera: no se pidio la entera, y la ranura volvio.
        // Tres lecturas: los 64 en la direccion 0, los 18, y la cabecera.
        assert_eq!(m.descriptores_pedidos, 3);
        assert_eq!(m.ranuras_devueltas, 1);
    }

    #[test]
    fn un_paquete_de_64_entra_sin_evaluate_y_sin_babble() {
        // El caso del audifono: con el paquete supuesto de 8 y 18 bytes de
        // golpe contestaba Babble. Con el esquema de Windows (64 supuesto,
        // 64 pedidos en la direccion 0) entra a la primera y sin evaluate.
        let mut m = Fingido::nuevo(Aparato::Paquete64);
        let mut e = Enumeracion::nueva(1, false, m.ahora);
        assert_eq!(m.bombear(&mut e, 2_000), Marcha::Lista);
        assert_eq!(e.vid_pid(), (0x046D, 0xC077));
        assert_eq!(
            &m.eventos[3..],
            ["address0", "get_dev", "reset", "reset_device", "address", "get_dev", "get_cfg", "get_cfg"]
        );
        assert_eq!(m.mps0_xhc, 64);
        assert_eq!(m.resets_device, 1);
    }

    #[test]
    fn un_puerto_vacio_se_rinde_sin_pedir_ranura() {
        let mut m = Fingido::nuevo(Aparato::Vacio);
        let mut e = Enumeracion::nueva(3, false, m.ahora);
        assert_eq!(m.bombear(&mut e, 2_000), Marcha::SinDireccion);
        assert_eq!(m.ranuras_pedidas, 0);
    }

    #[test]
    fn ningun_paso_avanza_el_reloj() {
        // La prueba de fondo de EX4: `avanzar` no espera. El reloj lo mueve
        // el bombeo, y aqui se comprueba que entre dos pasos consecutivos
        // sin bombeo la enumeracion no se mueve sola ni bloquea.
        let mut m = Fingido::nuevo(Aparato::Mudo);
        let mut e = Enumeracion::nueva(1, false, m.ahora);
        for _ in 0..1_000 {
            assert_eq!(e.avanzar(&mut m), Marcha::Sigue);
        }
        // Mil pasos y sigue en el primer plazo (los 120 ms tras encender),
        // porque el reloj no se ha movido.
        assert_eq!(m.eventos, ["encender"]);
    }

    #[test]
    fn abandonar_a_medias_devuelve_la_ranura() {
        let mut m = Fingido::nuevo(Aparato::Mudo);
        let mut e = Enumeracion::nueva(1, false, m.ahora);
        // Hasta que tenga ranura y este esperando un descriptor.
        while m.descriptores_pedidos == 0 {
            assert_eq!(e.avanzar(&mut m), Marcha::Sigue);
            m.ahora += 4;
        }
        e.abandonar(&mut m);
        assert_eq!(m.ranuras_devueltas, 1);
        assert_eq!(m.dejo_de_esperar, 1);
    }
}
