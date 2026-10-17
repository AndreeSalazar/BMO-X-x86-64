//! Booter SEC2: distinguir retorno, lectura ausente y foto de otro intento.
//! capa: puro -- recibe hechos del adaptador, nunca consulta el aparato
//! [carril] VERDE
//! [consumo] NADA
//! [cuesta] NADA -- diagnostico; no autoriza recuperar ni cambia el arranque

/// La disponibilidad la determina el lector del registro, no el codigo bruto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mailbox { Readable(u32), Unreadable(u32) }

#[derive(Clone, Copy, Debug)]
pub struct Evidence {
    /// Consultas delimitadoras iguales; tampoco equivale a atomicidad.
    pub stable: bool,
    pub started: Option<bool>,
    pub halted: Option<bool>,
    pub mailbox0: Option<Mailbox>,
    /// El kernel registro IOMMU_NO_BOOTER_MAL. Es otro dominio que MAILBOX0.
    pub host_rejected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Changing,
    Unknown,
    NotStarted,
    InFlight,
    ReadUnavailable(Option<u32>),
    Contradictory,
    /// Cero no acredita INIT_DONE ni salud actual del GSP.
    MailboxZero,
    /// No traduce el valor a un motivo interno del firmware.
    MailboxNonZero(u32),
}

pub fn diagnose(e: Evidence) -> Completion {
    if !e.stable { return Completion::Changing; }
    match (e.started, e.halted) {
        (Some(false), Some(true)) => return Completion::Contradictory,
        (Some(false), _) => return if e.host_rejected {
            Completion::Contradictory
        } else { Completion::NotStarted },
        (None, _) => return Completion::Unknown,
        (Some(true), None) => return Completion::Unknown,
        (Some(true), Some(false)) => return if e.host_rejected {
            Completion::Contradictory
        } else { Completion::InFlight },
        (Some(true), Some(true)) => {}
    }
    match e.mailbox0 {
        None => Completion::ReadUnavailable(None),
        Some(Mailbox::Unreadable(raw)) => Completion::ReadUnavailable(Some(raw)),
        Some(Mailbox::Readable(0)) if e.host_rejected => Completion::Contradictory,
        Some(Mailbox::Readable(0)) => Completion::MailboxZero,
        Some(Mailbox::Readable(code)) => Completion::MailboxNonZero(code),
    }
}

/// El kernel actual conserva la primera foto de parada del arranque, sin
/// sellarla con INTENTOS. Solo el intento 1 puede atribuirla sin ambiguedad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureLink { Changing, Absent, ThisAttempt, Unattributed }

pub fn link_capture(stable: bool, attempt: Option<u64>, present: bool) -> CaptureLink {
    if !stable { CaptureLink::Changing }
    else if !present { CaptureLink::Absent }
    else if attempt == Some(1) { CaptureLink::ThisAttempt }
    else { CaptureLink::Unattributed }
}

impl Completion {
    pub fn explanation(self) -> &'static [u8] {
        match self {
            Self::Changing => b"consulta cambio; repetir lectura, sin diagnostico estable",
            Self::Unknown => b"faltan hechos del intento; no asumir exito ni fallo del booter",
            Self::NotStarted => b"no consta STARTCPU del booter en SEC2",
            Self::InFlight => b"STARTCPU registrado; no se observo SEC2 parado; no es un timeout demostrado",
            Self::ReadUnavailable(_) => b"buzon no interpretable; conservar crudo, no convertir lectura fallida en cero",
            Self::Contradictory => b"hechos no compatibles; conservar motivo del kernel y lecturas por separado",
            Self::MailboxZero => b"SEC2 parado con MAILBOX0 cero; no demuestra por si solo INIT_DONE ni salud del GSP",
            Self::MailboxNonZero(0x15) => b"SEC2 parado con retorno 0x15; significado interno no verificado; no prueba fallo de shader ni de firma",
            Self::MailboxNonZero(_) => b"SEC2 parado con retorno no cero; significado interno no verificado",
        }
    }
}

#[cfg(test)]
mod tests;
