//! Supervisor NVIDIA: observaciones tipadas; ninguna operacion de aparato.
//! capa: puro -- recibe observaciones y devuelve estados; no accede al aparato
//! generacion: abuelo
//! [carril] VERDE
//! [consumo] NADA
#![no_std]
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State { Unprepared, Prepared, Starting, Ready, Failed, RecoveryBlocked }

/// Huellas e identificadores suministrados por el propietario, nunca por ticks.
/// None impide abrir una sesion con historia, pero permite diagnosticar una foto.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity {
    pub device: Option<u64>,
    pub firmware: Option<u64>,
    pub build: Option<u64>,
    pub attempt: Option<u64>,
}
impl Identity {
    pub fn complete(self) -> bool {
        self.device.is_some() && self.firmware.is_some()
            && self.build.is_some() && self.attempt.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure { Reported(u32), Timeout }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Prepared,
    /// Plazo absoluto en la misma unidad monotona que Observation::time.
    /// Lo calcula el adaptador desde el protocolo; el modelo no inventa esperas.
    Start { deadline: u64 },
    InitDone,
    Fault(u32),
    Tick,
    /// Todas las condiciones deben ser conocidas y verdaderas. Solo recomienda.
    Recovery { engines_stopped: Option<bool>, dma_isolated: Option<bool>,
               memory_released: Option<bool> },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    pub identity: Identity,
    pub sequence: u64,
    pub time: u64,
    pub event: Event,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection { MissingIdentity, OtherSession, Stale, Order, Deadline }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    Recorded,
    Duplicate,
    Wait,
    PreserveFailure,
    /// No ejecuta ni permite saltarse al propietario: requiere una sesion nueva.
    ReviewRecovery,
    BlockRecovery,
}

pub struct Supervisor {
    identity: Identity,
    state: State,
    last: Option<Observation>,
    deadline: Option<u64>,
    first_failure: Option<Failure>,
}
impl Supervisor {
    pub fn new(identity: Identity) -> Result<Self, Rejection> {
        if !identity.complete() { return Err(Rejection::MissingIdentity); }
        Ok(Self { identity, state: State::Unprepared, last: None,
            deadline: None, first_failure: None })
    }
    pub fn state(&self) -> State { self.state }
    pub fn first_failure(&self) -> Option<Failure> { self.first_failure }
    fn fail(&mut self, failure: Failure) {
        if self.first_failure.is_none() { self.first_failure = Some(failure); }
        self.state = State::Failed;
    }
    pub fn observe(&mut self, o: Observation) -> Result<Decision, Rejection> {
        if o.identity != self.identity { return Err(Rejection::OtherSession); }
        if let Some(last) = self.last {
            if last == o { return Ok(Decision::Duplicate); }
            if o.sequence <= last.sequence || o.time < last.time {
                return Err(Rejection::Stale);
            }
        }
        // Un INIT_DONE tardio tampoco convierte un timeout en exito.
        if self.state == State::Starting && self.deadline.is_some_and(|d| o.time >= d) {
            self.fail(Failure::Timeout);
        }
        let decision = match o.event {
            Event::Fault(code) => {
                let blocked = self.state == State::RecoveryBlocked;
                self.fail(Failure::Reported(code));
                if blocked { self.state = State::RecoveryBlocked; }
                Decision::PreserveFailure
            }
            Event::Recovery { engines_stopped, dma_isolated, memory_released }
                if self.first_failure.is_some() => {
                if engines_stopped == Some(true) && dma_isolated == Some(true)
                    && memory_released == Some(true) {
                    // La foto favorable no borra el fallo ni desbloquea esta sesion.
                    Decision::ReviewRecovery
                } else {
                    self.state = State::RecoveryBlocked;
                    Decision::BlockRecovery
                }
            }
            _ if self.first_failure.is_some() => Decision::PreserveFailure,
            Event::Prepared if self.state == State::Unprepared => {
                self.state = State::Prepared; Decision::Recorded
            }
            Event::Start { deadline } if self.state == State::Prepared => {
                if deadline <= o.time { return Err(Rejection::Deadline); }
                self.deadline = Some(deadline);
                self.state = State::Starting; Decision::Wait
            }
            Event::InitDone if self.state == State::Starting => {
                self.state = State::Ready; Decision::Recorded
            }
            Event::Tick if self.state == State::Starting => Decision::Wait,
            _ => return Err(Rejection::Order),
        };
        self.last = Some(o);
        Ok(decision)
    }
}

/// Foto derivada: no establece transiciones ni salud actual a partir de INIT_DONE.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub prepared: Option<bool>,
    pub started: Option<bool>,
    pub init_done: Option<bool>,
    pub fault: Option<u32>,
    pub stable: bool,
}
pub fn diagnose(s: Snapshot) -> Option<State> {
    if !s.stable { return None; }
    if s.fault.is_some() { return Some(State::Failed); }
    match (s.prepared, s.started, s.init_done) {
        (Some(true), Some(true), Some(true)) => Some(State::Ready),
        (Some(true), Some(true), Some(false)) => Some(State::Starting),
        (Some(true), Some(false), Some(false)) => Some(State::Prepared),
        (Some(false), Some(false), Some(false)) => Some(State::Unprepared),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
