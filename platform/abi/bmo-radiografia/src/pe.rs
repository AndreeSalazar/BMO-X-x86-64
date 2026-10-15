//! **POR DENTRO de un `.exe` de Windows** (LX1 de `PLAN_PROTON_X.md`, 15-10).
//!
//! > El propietario: *"quizas nos sorprenda que PROTON-X no deja claro aunque
//! > printf ayude [...] el binario PRECISO en 0 y 1 diga algo"*.
//!
//! Lo que PROTON-X tiene que saber de un juego ANTES de correrlo, leido del
//! fichero y sin pedir memoria (el director no tiene monton):
//!
//! ```text
//!    el veredicto     las mismas reglas y los mismos textos que el cargador
//!                     de PROTON-X (`bmo_proton_x::pe::leer`): x86-64 PE32+,
//!                     sin .NET, ninguna seccion que escriba y ejecute
//!    la forma         base, entrada, secciones (con sus permisos)
//!    lo que PIDE      las IMPORTACIONES: cada DLL y cada funcion (o su
//!                     ordinal), y la RANURA de la IAT donde Windows -- o
//!                     PROTON-X -- deja su direccion. El codigo llama con
//!                     `call [rip+x]` a esa ranura: asi la lente sabe A QUIEN
//!                     llama el juego
//!    el codigo        la lente de x86-64 desde la entrada y desde CADA
//!                     funcion que el compilador apunto en `.pdata` (las
//!                     RUNTIME_FUNCTION: MSVC las escribe todas), y que
//!                     extensiones de la CPU usa de verdad
//! ```
//!
//! El banco compara secciones e importaciones con las del cargador de
//! PROTON-X sobre los `.exe` de verdad de `proton-x/prueba/` (MSVC): si los
//! dos leen distinto, uno de los dos miente, y lo dice la prueba.

use core::cell::RefCell;

use bmo_lente_x86::recorre::Recorrido;

pub const MAQUINA_AMD64: u16 = 0x8664;
pub const MAGIA_PE32_MAS: u16 = 0x20B;
const SE_EJECUTA: u32 = 0x2000_0000;
const SE_ESCRIBE: u32 = 0x8000_0000;
const SE_LEE: u32 = 0x4000_0000;

/// Secciones que se guardan (un `.exe` normal trae de 4 a 10).
pub const SECCIONES_MAX: usize = 24;

/// Por que un fichero no es un `.exe` que PROTON-X corra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoPe {
    /// No empieza por `MZ` o no lleva la firma `PE\0\0`.
    NoEsPe,
    /// Es un PE, pero de lo que PROTON-X deja FUERA (el texto dice que).
    Fuera(&'static str),
    /// Se acaba antes de lo que su cabecera promete.
    Corto(&'static str),
    /// Una seccion que escribe Y ejecuta: no se carga (W^X).
    EscribeYEjecuta,
    /// Una seccion que no cabe en la imagen o en el fichero.
    Seccion(&'static str),
}

impl NoPe {
    pub fn nombre(self) -> &'static str {
        match self {
            NoPe::NoEsPe => "no es un PE (le falta `MZ` o la firma `PE`)",
            NoPe::Fuera(m) => m,
            NoPe::Corto(_) => "se acaba antes de lo que dice su cabecera",
            NoPe::EscribeYEjecuta => "una seccion escribe Y ejecuta: no se carga (W^X)",
            NoPe::Seccion(m) => m,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeccionPe {
    pub nombre: [u8; 8],
    pub rva: u32,
    pub tam_virtual: u32,
    pub desde: u32,
    pub tam_fichero: u32,
    pub caracteristicas: u32,
}

impl SeccionPe {
    pub fn nombre(&self) -> &[u8] {
        let n = self.nombre.iter().position(|&b| b == 0).unwrap_or(8);
        &self.nombre[..n]
    }
    pub fn ejecuta(&self) -> bool {
        self.caracteristicas & SE_EJECUTA != 0
    }
    pub fn escribe(&self) -> bool {
        self.caracteristicas & SE_ESCRIBE != 0
    }
    pub fn lee(&self) -> bool {
        self.caracteristicas & SE_LEE != 0
    }
    /// Lo que ocupa en la imagen (si `tam_virtual` es 0, lo del fichero).
    pub fn tam_en_imagen(&self) -> u32 {
        if self.tam_virtual == 0 { self.tam_fichero } else { self.tam_virtual }
    }
    /// Los bytes que trae del fichero (los de mas en la imagen son ceros).
    pub fn crudos<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let ini = self.desde as usize;
        let n = self.tam_fichero.min(self.tam_en_imagen()) as usize;
        bytes.get(ini..ini.saturating_add(n).min(bytes.len())).unwrap_or(&[])
    }
}

/// Un directorio de la cabecera opcional: RVA y medida (0 = no esta).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dir {
    pub rva: u32,
    pub tam: u32,
}

/// **La forma de un `.exe` que PROTON-X acepta.**
#[derive(Clone, Copy, Debug)]
pub struct Pe {
    pub base: u64,
    pub entrada: u32,
    pub tam_imagen: u32,
    pub tam_cabeceras: u32,
    pub secciones: [SeccionPe; SECCIONES_MAX],
    pub cuantas: usize,
    pub exportaciones: Dir,
    pub importaciones: Dir,
    pub excepciones: Dir,
    pub retrasadas: Dir,
    pub tls: Dir,
    pub es_dll: bool,
}

/// Una funcion que el `.exe` pide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Importada<'a> {
    pub dll: &'a [u8],
    /// El nombre, o `None` si se pide por ordinal.
    pub nombre: Option<&'a [u8]>,
    pub ordinal: u16,
    /// La RVA de su ranura en la IAT.
    pub ranura: u32,
}

fn u16_en(d: &[u8], o: usize, que: &'static str) -> Result<u16, NoPe> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(NoPe::Corto(que))
}

fn u32_en(d: &[u8], o: usize, que: &'static str) -> Result<u32, NoPe> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or(NoPe::Corto(que))
}

fn u64_en(d: &[u8], o: usize, que: &'static str) -> Result<u64, NoPe> {
    d.get(o..o + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8]))).ok_or(NoPe::Corto(que))
}

/// Lo que deja fuera PROTON-X, con SUS textos (`bmo_proton_x::pe`).
fn maquina_fuera(m: u16) -> Option<&'static str> {
    Some(match m {
        0x14C => "x86 de 32 bits (pediria WOW64: un segundo mundo entero)",
        0xAA64 => "ARM64 (otras instrucciones: pediria traducirlas, como FEX o Rosetta)",
        0xA641 => "ARM64EC (ARM con llamadas de x64: sigue siendo ARM)",
        0xA64E => "ARM64X (dos binarios en uno, ARM64 y ARM64EC)",
        0x1C4 => "ARM de 32 bits",
        _ => return None,
    })
}

/// Empieza como un `.exe` de Windows (`MZ`).
pub fn es_pe(bytes: &[u8]) -> bool {
    bytes.get(..2) == Some(b"MZ")
}

/// **Lee la forma de un `.exe`**, con el veredicto de PROTON-X primero.
pub fn leer(d: &[u8]) -> Result<Pe, NoPe> {
    if !es_pe(d) {
        return Err(NoPe::NoEsPe);
    }
    let e = u32_en(d, 0x3C, "la firma PE")? as usize;
    if d.get(e..e + 4) != Some(b"PE\0\0") {
        return Err(NoPe::NoEsPe);
    }
    let maquina = u16_en(d, e + 4, "la maquina")?;
    let nsec = u16_en(d, e + 6, "el numero de secciones")? as usize;
    let tam_opcional = u16_en(d, e + 20, "la medida de la cabecera opcional")? as usize;
    let coff = u16_en(d, e + 22, "las caracteristicas COFF")?;
    let opc = e + 24;
    let magia = u16_en(d, opc, "la magia")?;
    if let Some(m) = maquina_fuera(maquina) {
        return Err(NoPe::Fuera(m));
    }
    if maquina != MAQUINA_AMD64 {
        return Err(NoPe::Fuera("maquina desconocida: no es x86-64"));
    }
    if magia != MAGIA_PE32_MAS {
        return Err(NoPe::Fuera("cabecera PE32 (de 32 bits) con maquina AMD64: no es un PE32+"));
    }
    let ndirs = u32_en(d, opc + 108, "el numero de directorios")? as usize;
    let dir = |i: usize| -> Result<Dir, NoPe> {
        if i >= ndirs {
            return Ok(Dir::default());
        }
        Ok(Dir { rva: u32_en(d, opc + 112 + 8 * i, "un directorio")?, tam: u32_en(d, opc + 116 + 8 * i, "un directorio")? })
    };
    if dir(14)?.rva != 0 {
        return Err(NoPe::Fuera(".NET: lleva IL, no instrucciones x86-64 (pediria una CLR)"));
    }
    let tam_imagen = u32_en(d, opc + 56, "la medida de la imagen")?;
    let mut pe = Pe {
        base: u64_en(d, opc + 24, "la base")?,
        entrada: u32_en(d, opc + 16, "la entrada")?,
        tam_imagen,
        tam_cabeceras: u32_en(d, opc + 60, "la medida de las cabeceras")?,
        secciones: [SeccionPe::default(); SECCIONES_MAX],
        cuantas: 0,
        exportaciones: dir(0)?,
        importaciones: dir(1)?,
        excepciones: dir(3)?,
        tls: dir(9)?,
        retrasadas: dir(13)?,
        es_dll: coff & 0x2000 != 0,
    };
    for k in 0..nsec {
        let s = opc + tam_opcional + 40 * k;
        let crudo = d.get(s..s + 8).ok_or(NoPe::Corto("el nombre de una seccion"))?;
        let mut nombre = [0u8; 8];
        nombre.copy_from_slice(crudo);
        let sec = SeccionPe {
            nombre,
            tam_virtual: u32_en(d, s + 8, "una seccion")?,
            rva: u32_en(d, s + 12, "una seccion")?,
            tam_fichero: u32_en(d, s + 16, "una seccion")?,
            desde: u32_en(d, s + 20, "una seccion")?,
            caracteristicas: u32_en(d, s + 36, "una seccion")?,
        };
        if sec.ejecuta() && sec.escribe() {
            return Err(NoPe::EscribeYEjecuta);
        }
        if sec.rva as u64 + sec.tam_en_imagen() as u64 > tam_imagen as u64 {
            return Err(NoPe::Seccion("una seccion no cabe en la imagen"));
        }
        if sec.desde as u64 + sec.tam_fichero.min(sec.tam_en_imagen()) as u64 > d.len() as u64 {
            return Err(NoPe::Seccion("los datos de una seccion pasan del final del fichero"));
        }
        if pe.cuantas < SECCIONES_MAX {
            pe.secciones[pe.cuantas] = sec;
            pe.cuantas += 1;
        }
    }
    Ok(pe)
}

impl Pe {
    pub fn secciones(&self) -> &[SeccionPe] {
        &self.secciones[..self.cuantas]
    }

    /// La seccion donde cae una RVA.
    pub fn seccion_de(&self, rva: u32) -> Option<&SeccionPe> {
        self.secciones().iter().find(|s| rva >= s.rva && (rva - s.rva) < s.tam_en_imagen())
    }

    /// **De una RVA a su byte en el fichero** (`None` si cae en lo que la
    /// imagen rellena con ceros, o fuera de todo).
    pub fn en_fichero(&self, rva: u32) -> Option<usize> {
        if rva < self.tam_cabeceras {
            return Some(rva as usize);
        }
        let s = self.seccion_de(rva)?;
        let dentro = rva - s.rva;
        (dentro < s.tam_fichero).then(|| s.desde as usize + dentro as usize)
    }

    /// Una cadena terminada en cero, en la RVA `rva`.
    fn cadena<'a>(&self, d: &'a [u8], rva: u32) -> Result<&'a [u8], NoPe> {
        let o = self.en_fichero(rva).ok_or(NoPe::Corto("un nombre importado"))?;
        let resto = d.get(o..).ok_or(NoPe::Corto("un nombre importado"))?;
        let n = resto.iter().position(|&b| b == 0).ok_or(NoPe::Corto("el final de un nombre importado"))?;
        Ok(&resto[..n])
    }

    /// **Lo que pide**, una funcion por llamada a `f`, en el orden del
    /// fichero (DLL a DLL). Devuelve cuantas.
    pub fn importaciones<'a>(&self, d: &'a [u8], mut f: impl FnMut(Importada<'a>)) -> Result<usize, NoPe> {
        let mut cuantas = 0usize;
        if self.importaciones.rva == 0 {
            return Ok(0);
        }
        let leer32 = |rva: u32, que| self.en_fichero(rva).ok_or(NoPe::Corto(que)).and_then(|o| u32_en(d, o, que));
        let mut p = self.importaciones.rva;
        // Un tope: un descriptor corrupto no da vueltas sin fin.
        for _ in 0..4096 {
            let nombres = leer32(p, "un descriptor de importacion")?;
            let nombre_dll = leer32(p + 12, "un descriptor de importacion")?;
            let iat = leer32(p + 16, "un descriptor de importacion")?;
            if nombres == 0 && nombre_dll == 0 && iat == 0 {
                break;
            }
            let dll = self.cadena(d, nombre_dll)?;
            let mut lista = if nombres != 0 { nombres } else { iat };
            let mut ranura = iat;
            for _ in 0..65536 {
                let o = self.en_fichero(lista).ok_or(NoPe::Corto("una entrada importada"))?;
                let e = u64_en(d, o, "una entrada importada")?;
                if e == 0 {
                    break;
                }
                let (nombre, ordinal) = if e >> 63 != 0 {
                    (None, e as u16)
                } else {
                    // +2: el `hint` va delante del nombre.
                    (Some(self.cadena(d, (e & 0x7FFF_FFFF) as u32 + 2)?), 0)
                };
                f(Importada { dll, nombre, ordinal, ranura });
                cuantas += 1;
                lista += 8;
                ranura += 8;
            }
            p += 20;
        }
        Ok(cuantas)
    }

    /// **La importacion cuya ranura es `rva`**: a quien llama un
    /// `call [rip+x]` que cae en la IAT.
    pub fn importada_en<'a>(&self, d: &'a [u8], rva: u32) -> Option<Importada<'a>> {
        let mut hallada = None;
        let _ = self.importaciones(d, |i| {
            if i.ranura == rva && hallada.is_none() {
                hallada = Some(i);
            }
        });
        hallada
    }

    /// La seccion de CODIGO: la que ejecuta y lleva la entrada.
    pub fn codigo(&self) -> Option<&SeccionPe> {
        self.seccion_de(self.entrada).filter(|s| s.ejecuta())
    }

    /// **La lente sobre el codigo**: desde la entrada y desde cada funcion de
    /// `.pdata`. En un `.exe` nada se "declara", asi que toda extension
    /// vale; lo que se cuenta es CUALES usa (`usadas`). `visto` y `pila`,
    /// como en [`crate::radiografiar`].
    pub fn lente(&self, d: &[u8], visto: &mut [u8], pila: &mut [u32], usadas: &mut Extensiones) -> Option<Recorrido> {
        let c = *self.codigo()?;
        let codigo = c.crudos(d);
        if codigo.is_empty() || visto.len() < codigo.len().div_ceil(8) {
            return None;
        }
        // `.pdata`: RUNTIME_FUNCTION de 12 bytes (inicio, fin, desenrollo).
        let pdata: &[u8] = match self.en_fichero(self.excepciones.rva) {
            Some(o) if self.excepciones.rva != 0 => d.get(o..o.saturating_add(self.excepciones.tam as usize).min(d.len())).unwrap_or(&[]),
            _ => &[],
        };
        let semillas = pdata.chunks_exact(12).filter_map(|e| {
            let inicio = u32::from_le_bytes([e[0], e[1], e[2], e[3]]);
            inicio.checked_sub(c.rva).filter(|&o| (o as usize) < codigo.len()).map(|o| o as u64)
        });
        let ventana = |en: usize, buf: &mut [u8; bmo_lente_x86::LARGO_MAX]| -> usize {
            let n = bmo_lente_x86::LARGO_MAX.min(codigo.len() - en);
            buf[..n].copy_from_slice(&codigo[en..en + n]);
            n
        };
        let vistas = RefCell::new(&mut *usadas);
        let r = bmo_lente_x86::recorre::recorre(
            codigo.len(),
            (self.entrada - c.rva) as usize,
            semillas,
            ventana,
            |n| {
                vistas.borrow_mut().apunta(n);
                true
            },
            visto,
            pila,
        );
        Some(r)
    }
}

/// **Las extensiones de la CPU que usa el codigo** (las que la lente vio).
#[derive(Clone, Copy, Debug)]
pub struct Extensiones {
    nombres: [&'static str; 32],
    n: usize,
}

impl Default for Extensiones {
    fn default() -> Self {
        Extensiones { nombres: [""; 32], n: 0 }
    }
}

impl Extensiones {
    fn apunta(&mut self, n: &'static str) {
        if !self.nombres[..self.n].contains(&n) && self.n < self.nombres.len() {
            self.nombres[self.n] = n;
            self.n += 1;
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.nombres[..self.n].iter().copied()
    }
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
}
