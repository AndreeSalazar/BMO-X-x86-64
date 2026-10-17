//! Que paquetes y metodos hay en una copia FIFO_DMA NVIDIA.
//! [carril] VERDE -- no implementa ni recibe Registros
//! [consumo] NADA -- recorrido lineal sin monton, no ejecuta controles
//!
//! Formatos de NV_FIFO_DMA en dev_ram.ref de NVIDIA; contrastados con la
//! documentacion de ordenes NVK. Nunca empezar a decodificar en mitad del
//! recurso: una palabra de datos puede parecer una cabecera.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Nop, Increment, NonIncrement, Immediate, OneIncrement }
impl Mode {
    pub const fn label(self) -> &'static str {
        match self { Self::Nop => "NOP", Self::Increment => "INC", Self::NonIncrement => "NOINC", Self::Immediate => "IMMD", Self::OneIncrement => "ONEINC" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason { PartialWord, UnsupportedControl, ReservedBit, Truncated, MethodRange }
impl Reason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PartialWord => "cola que no completa una palabra de 4 bytes",
            Self::UnsupportedControl => "control no interpretado: no se sigue ni resincroniza",
            Self::ReservedBit => "bit 12 reservado: no se interpreta este paquete",
            Self::Truncated => "faltan datos declarados por la cabecera",
            Self::MethodRange => "la secuencia sale del campo de direccion de metodo",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fault { pub word: usize, pub raw: u32, pub reason: Reason, pub declared: usize, pub available: usize }

#[derive(Clone, Copy)]
enum Input<'a> { Bytes(&'a [u8]), Words(&'a [u32]) }
impl Input<'_> {
    fn len(self) -> usize { match self { Self::Bytes(b) => b.len() / 4, Self::Words(w) => w.len() } }
    fn get(self, k: usize) -> Option<u32> {
        match self {
            Self::Bytes(b) => {
                let start = k.checked_mul(4)?;
                Some(u32::from_le_bytes(b.get(start..start.checked_add(4)?)?.try_into().ok()?))
            }
            Self::Words(w) => w.get(k).copied(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Packet<'a> {
    input: Input<'a>,
    pub word: usize,
    pub raw: u32,
    pub mode: Mode,
    pub subchannel: u8,
    pub method: u16,
    pub count: usize,
    pub words: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// Ubicacion en la copia: el inmediato esta en la propia cabecera.
    pub word: usize,
    pub subchannel: u8,
    /// Direccion de metodo en BYTES, no la direccion de memoria GPU.
    pub method: u16,
    pub value: u32,
}

impl Packet<'_> {
    pub fn command(self, k: usize) -> Option<Command> {
        if k >= self.count { return None; }
        let step = match self.mode { Mode::Increment => k, Mode::OneIncrement => usize::from(k > 0), _ => 0 };
        let method = self.method.checked_add(u16::try_from(step.checked_mul(4)?).ok()?)?;
        if method > 0x3ffc { return None; }
        let (word, value) = if self.mode == Mode::Immediate { (self.word, self.raw >> 16 & 0x1fff) }
                          else { let w = self.word.checked_add(1)?.checked_add(k)?; (w, self.input.get(w)?) };
        Some(Command { word, subchannel: self.subchannel, method, value })
    }
}

/// Solo un segmento contiguo completo. Los controles de flujo o de
/// subdispositivo no se ejecutan: se devuelve un error y termina el lector.
pub struct Push<'a> { input: Input<'a>, at: usize, stopped: bool }
impl<'a> Push<'a> {
    pub fn from_words(words: &'a [u32]) -> Self { Self { input: Input::Words(words), at: 0, stopped: false } }
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Fault> {
        if bytes.len() % 4 != 0 {
            return Err(Fault { word: bytes.len()/4, raw: 0, reason: Reason::PartialWord, declared: 4, available: bytes.len()%4 });
        }
        Ok(Self { input: Input::Bytes(bytes), at: 0, stopped: false })
    }
}

impl<'a> Iterator for Push<'a> {
    type Item = Result<Packet<'a>, Fault>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped || self.at == self.input.len() { return None; }
        let word = self.at;
        let raw = self.input.get(word)?;
        let mode = match raw >> 29 {
            _ if raw == 0 => Mode::Nop,
            1 => Mode::Increment, 3 => Mode::NonIncrement, 4 => Mode::Immediate, 5 => Mode::OneIncrement,
            _ => { self.stopped = true; return Some(Err(Fault { word, raw, reason: Reason::UnsupportedControl, declared: 0, available: self.input.len()-word-1 })); }
        };
        let count = match mode { Mode::Nop => 0, Mode::Immediate => 1, _ => (raw >> 16 & 0x1fff) as usize };
        let words = if mode == Mode::Immediate { 1 } else { 1 + count };
        let method = ((raw & 0xfff) * 4) as u16;
        let available = self.input.len() - word - 1;
        let last_step = match mode { Mode::Increment => count.saturating_sub(1), Mode::OneIncrement => usize::from(count > 1), _ => 0 };
        // COUNT=0 ignora los otros campos, segun NV_FIFO_DMA. El bit
        // reservado se rechaza conservadoramente solo si hay un metodo.
        let reason = if count > 0 && raw & (1 << 12) != 0 { Some(Reason::ReservedBit) }
            else if words - 1 > available { Some(Reason::Truncated) }
            else if method as usize + last_step * 4 > 0x3ffc { Some(Reason::MethodRange) }
            else { None };
        if let Some(reason) = reason {
            self.stopped = true;
            return Some(Err(Fault { word, raw, reason, declared: count, available }));
        }
        self.at += words;
        Some(Ok(Packet { input: self.input, word, raw, mode, subchannel: (raw >> 13 & 7) as u8, method, count, words }))
    }
}

impl core::iter::FusedIterator for Push<'_> {}
