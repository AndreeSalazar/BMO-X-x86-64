//! **LA DECISION: es admisible este `.bex`?** Y nada mas.
//!
//! ## Por que existe este crate
//!
//! La respuesta a *"se puede ejecutar esto?"* la necesitan dos que no se pueden
//! compartir codigo: el toolchain (con `alloc`, en el anfitrion) y Ring 0 (sin
//! asignador, en el metal). Este crate es la decision **por su cuenta**: cero
//! dependencias, cero `alloc`, y una prueba en `bmo-abi`
//! (`tests/gate_y_validador_no_se_separan.rs`) que le pregunta lo mismo a esta
//! puerta y al juez del contrato (`bmo_abi::bef2::lector`) y exige la misma
//! respuesta.
//!
//! ```text
//!                     bmo-bex-gate        <- la DECISION
//!                      /          \
//!       bef2::lector (alloc)       bex.rs (Ring 0)
//!       anade los HASHES           anade el PLAN
//! ```
//!
//! ## BEF2, y solo BEF2 (2026-09-19)
//!
//! BEF1 --la cabecera de 48 B con TABLA DE SECCIONES tipadas, la idea de ELF
//! con otro nombre-- murio en B6. Y en B8 murio tambien la PINTURA AL REVES:
//! hasta entonces esta puerta leia BEF2 y se lo **presentaba al kernel como
//! secciones** ("mismo tipo, mismo indice") para no tocar Ring 0. Eso dejaba
//! la propiedad central del formato --*el permiso lo da el HUECO de la
//! cabecera*-- viviendo en un adaptador, y el 19-09 se vio que el adaptador
//! mentia en los relocs. Ahora la puerta habla de lo que el fichero TIENE:
//!
//! ```text
//!    cuatro REGIONES   codigo, constantes, datos, ceros   (sitio fijo, permiso fijo)
//!    ANEXOS            relocs, firma, requisitos           (los abre el kernel)
//!                      recursos, manifiesto, katanas...    (data para otro: se saltan)
//! ```
//!
//! ## Y lo que esto NO hace
//!
//! No mapea, no reserva, no lee disco, no comprueba hashes (eso es de quien
//! COPIA: `task/landing.rs`), no explica en prosa, y **no opina**. Una imagen
//! que pasa por aqui es una imagen **bien formada**, y eso no quiere decir que
//! sea buena, ni segura, ni tuya. Quien decide si se ejecuta es el sistema, con
//! esto y con lo demas -- la firma, los requisitos, quien la lanza.
//!
//! > Bien formado no es lo mismo que de fiar. Confundirlo es como creerse un
//! > documento porque la letra es bonita.

#![no_std]
#![forbid(unsafe_code)]

// -- El contrato en el cable (espejo de `bmo_abi::bef2`) ----------------------
//
// *** POR QUE ESTOS NUMEROS ESTAN AQUI Y NO SE PIDEN A `bmo-abi`.
//
// Porque este crate tiene CERO DEPENDENCIAS y ese es su punto entero: lo
// consumen el toolchain y Ring 0. Depender de `bmo-abi` lo dejaria fuera del
// kernel, que es justo el consumidor por el que existe. Y la flecha tampoco se
// puede invertir: `bmo-abi` es el CONTRATO y esto es UNA PUERTA.
//
// [!] Asi que son dos copias de la misma decision, a sabiendas, **y atadas por
// una prueba**: `bmo-abi/tests/gate_y_validador_no_se_separan.rs`.

/// `"BEF2"` en little-endian.
pub const MAGIC: u32 = u32::from_le_bytes(*b"BEF2");
/// El ABI que habla la imagen. Uno solo.
pub const ABI: u8 = 2;
/// La cabecera: 64 B, una linea de cache.
pub const CABECERA: usize = 64;
/// Una entrada de la tabla de anexos.
pub const ANEXO: usize = 16;
/// Tope de anexos: la tabla entera cabe en el prologo.
pub const MAX_ANEXOS: usize = 16;

/// Banderas de la cabecera (byte 5).
pub const EJECUTABLE: u8 = 1 << 0;
pub const OBJETO: u8 = 1 << 1;
pub const QUIERE_PANTALLA: u8 = 1 << 2;
const BANDERAS: u8 = EJECUTABLE | OBJETO | QUIERE_PANTALLA;

/// **Lo que este kernel PRESERVA en un cambio de contexto**: x87 y SSE. Un
/// programa que declare mas en `xcr0` se rechaza con nombre -- sin esto, usar
/// AVX corromperia sus ymm en silencio a la primera interrupcion.
pub const XCR0_PRESERVADO: u64 = (1 << 0) | (1 << 1);

/// Tipos de anexo.
pub const ANEXO_RELOCS: u8 = 0x01;
pub const ANEXO_FIRMA: u8 = 0x02;
pub const ANEXO_REQUISITOS: u8 = 0x03;
pub const ANEXO_RECURSOS: u8 = 0x04;
pub const ANEXO_MANIFIESTO: u8 = 0x05;
pub const ANEXO_KATANAS: u8 = 0x06;
pub const ANEXO_SIMBOLOS: u8 = 0x07;
/// Los enlaces de un OBJETO: en un ejecutable no pueden ir.
pub const ANEXO_ENLACE: u8 = 0x08;

/// **Los tres anexos que el kernel ABRE.** Todo otro anexo es data para OTRO
/// --el enlazador, el verificador, el DIRECTOR, el runtime de un lenguaje-- y
/// se SALTA: es la unica idea de la regla congelada que sobrevive, y sobrevive
/// porque es la que deja crecer el formato sin que crezca el kernel.
pub const fn lo_lee_el_kernel(tipo: u8) -> bool {
    matches!(tipo, ANEXO_RELOCS | ANEXO_FIRMA | ANEXO_REQUISITOS)
}

/// El `que` de un hash de la firma que cubre un ANEXO: `0x80 | indice`. Los
/// valores 0..=2 son las regiones con bytes.
pub const FIRMA_ANEXO: u8 = 0x80;

// -- Las cuatro regiones ------------------------------------------------------

/// Las cuatro regiones de un programa, en el orden de la cabecera. El numero es
/// el que usan los relocs y con el que la firma las nombra.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cual {
    Codigo = 0,
    Constantes = 1,
    Datos = 2,
    Ceros = 3,
}

impl Cual {
    pub const TODAS: [Cual; 4] = [Cual::Codigo, Cual::Constantes, Cual::Datos, Cual::Ceros];

    pub const fn de(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Codigo),
            1 => Some(Self::Constantes),
            2 => Some(Self::Datos),
            3 => Some(Self::Ceros),
            _ => None,
        }
    }

    /// El nombre, para decirlo en voz alta: un numero en una foto de pantalla
    /// obliga a abrir el fichero con otra herramienta.
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::Codigo => "codigo",
            Self::Constantes => "constantes",
            Self::Datos => "datos",
            Self::Ceros => "ceros",
        }
    }

    /// Donde esta su tramo en la cabecera. Los ceros solo tienen medida.
    const fn en_cabecera(self) -> usize {
        match self {
            Self::Codigo => 24,
            Self::Constantes => 32,
            Self::Datos => 40,
            Self::Ceros => 48,
        }
    }
}

/// Una region que EXISTE, ya comprobada. Los numeros son los del fichero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub cual: Cual,
    /// Donde empiezan sus bytes en el fichero. Los ceros no tienen.
    pub file_offset: u64,
    /// Cuantos bytes hay en el fichero. Los ceros: 0.
    pub file_size: u64,
    /// Cuanto ocupa en memoria. En las tres con bytes es `file_size`; en los
    /// ceros es lo que hay que poner a cero.
    pub mem_size: u64,
}

impl Region {
    /// El `que` con el que la firma la nombra.
    pub const fn que(&self) -> u8 {
        self.cual as u8
    }
}

/// Un anexo, ya comprobado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anexo {
    pub tipo: u8,
    pub file_offset: u64,
    pub file_size: u64,
    /// El `que` con el que la firma lo nombra: `0x80 | indice en la tabla`.
    pub que: u8,
}

/// **Por que no se admite.** Cada una manda a mirar un sitio distinto, que es la
/// razon de que sean variantes y no un booleano.
///
/// Es `Copy` y sin datos prestados a proposito: cruza la frontera de Ring 0 sin
/// reservar nada, y quien quiera contarlo con numeros los saca del fichero, que
/// sigue teniendo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falta {
    NoLlegaNiALaCabecera,
    /// No es un BEF2: otro magic (un BEF1, un ELF, un PE, basura).
    OtroFormato,
    /// Un campo reservado que no es cero: basura, o una version que este
    /// sistema no entiende.
    CabeceraInvalida,
    /// Declara en `xcr0` un estado de CPU que el sistema no sabe preservar.
    ExtensionDeCpuQueNoSePreserva,
    OtraVersionDelAbi,
    /// Ni ejecutable ni objeto, o las dos a la vez.
    NoEsEjecutable,
    /// Un objeto que nadie enlazo (o un ejecutable con un anexo ENLACE): la
    /// respuesta es `bmo-enlazar`, no una bandera.
    EsUnObjetoSinEnlazar,
    /// Una bandera que este sistema no conoce: cambia el significado de lo
    /// que viene detras.
    PideAlgoQueNadieImplementa,
    /// Un ejecutable sin anexo de firma: no hay con que comprobar que llego
    /// entero.
    SinFirma,
    DemasiadosAnexos,
    /// La tabla no cabe en lo que se paso. Quien llama puede leer mas y volver.
    TablaFueraDeLoLeido,
    TablaFueraDelFichero,
    /// Un anexo de tipo 0, vacio, repetido, o con el relleno sucio.
    AnexoInvalido,
    /// Dos trozos (regiones, anexos, la cabecera) se pelean por los mismos
    /// bytes del fichero.
    TramosSeSolapan,
    /// Una region o un anexo declara bytes que caen fuera del fichero.
    TramoFueraDelFichero,
    /// Un ejecutable sin codigo.
    SinCodigo,
    EntryFueraDelCodigo,
    /// La cabecera dice medir mas de lo que el fichero mide.
    ImagenIncompleta,
}

impl Falta {
    /// Una linea corta, en el idioma del sistema.
    ///
    /// `&'static str` y no `String`: esto lo imprime CABINA en Ring 0, donde no
    /// hay a quien pedirle memoria. Quien quiera una frase larga con numeros
    /// dentro --el toolchain-- la construye encima, que para eso tiene `alloc`.
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::NoLlegaNiALaCabecera => "la imagen no llega ni a la cabecera",
            Falta::OtroFormato => "no es un BEF2 (otro magic: BEF1, ELF, PE o basura)",
            Falta::CabeceraInvalida => "cabecera invalida: un campo reservado no es cero",
            Falta::ExtensionDeCpuQueNoSePreserva => "pide un estado de CPU (xcr0) que no se preserva",
            Falta::OtraVersionDelAbi => "otra version del ABI",
            Falta::NoEsEjecutable => "ni ejecutable ni objeto, o las dos a la vez",
            Falta::EsUnObjetoSinEnlazar => "es un OBJETO sin enlazar (.bo): pasalo por bmo-enlazar",
            Falta::PideAlgoQueNadieImplementa => "trae una bandera que este sistema no conoce",
            Falta::SinFirma => "un ejecutable sin firma: no hay con que comprobar que llego entero",
            Falta::DemasiadosAnexos => "demasiados anexos",
            Falta::TablaFueraDeLoLeido => "la tabla de anexos no cabe en lo leido",
            Falta::TablaFueraDelFichero => "la tabla de anexos cae fuera del fichero",
            Falta::AnexoInvalido => "un anexo esta mal formado (tipo 0, vacio, repetido o relleno sucio)",
            Falta::TramosSeSolapan => "dos trozos se pelean por los mismos bytes",
            Falta::TramoFueraDelFichero => "una region o un anexo cae fuera del fichero",
            Falta::SinCodigo => "un ejecutable sin codigo",
            Falta::EntryFueraDelCodigo => "el punto de entrada cae fuera del codigo",
            Falta::ImagenIncompleta => "llegaron menos bytes de los que la imagen dice medir",
        }
    }
}

/// **Una imagen que ya paso la puerta.** Solo se puede construir con
/// [`revisar`], asi que tener una es la prueba de que las comprobaciones
/// corrieron.
///
/// Es el patron de "estado ilegal irrepresentable": quien recibe esto no tiene
/// que acordarse de validar, porque no existe una forma de llegar aqui sin haber
/// validado.
#[derive(Clone, Copy)]
pub struct Revisada<'a> {
    prologo: &'a [u8],
    entrada: u32,
    xcr0: u64,
    banderas: u8,
    cuantos_anexos: usize,
    /// Donde acaba el prologo: la cabecera mas su tabla.
    fin_tabla: usize,
}

impl<'a> Revisada<'a> {
    /// Offset del punto de entrada DENTRO del codigo.
    pub fn entrada(&self) -> u64 {
        self.entrada as u64
    }
    /// Los componentes XSAVE que el programa declara usar.
    pub fn xcr0(&self) -> u64 {
        self.xcr0
    }
    pub fn quiere_pantalla(&self) -> bool {
        self.banderas & QUIERE_PANTALLA != 0
    }

    /// Una region, si existe (tiene bytes, o ceros que poner).
    pub fn region(&self, cual: Cual) -> Option<Region> {
        let o = cual.en_cabecera();
        let (file_offset, file_size, mem_size) = if matches!(cual, Cual::Ceros) {
            let ceros = u32_en(self.prologo, o)? as u64;
            (0, 0, ceros)
        } else {
            let off = u32_en(self.prologo, o)? as u64;
            let len = u32_en(self.prologo, o + 4)? as u64;
            (off, len, len)
        };
        if mem_size == 0 {
            return None;
        }
        Some(Region { cual, file_offset, file_size, mem_size })
    }

    /// Las regiones que existen, en el orden de la cabecera (que es tambien el
    /// orden del fichero: el escritor las coloca asi, y el cargador las pide al
    /// disco hacia adelante).
    pub fn regiones(&self) -> impl Iterator<Item = Region> + '_ {
        Cual::TODAS.iter().filter_map(move |c| self.region(*c))
    }

    /// Cuantas regiones existen.
    pub fn cuantas_regiones(&self) -> usize {
        self.regiones().count()
    }

    /// El anexo `i` de la tabla, si lo hay.
    pub fn anexo_n(&self, i: usize) -> Option<Anexo> {
        if i >= self.cuantos_anexos {
            return None;
        }
        let (tipo, off, len) = entrada_de_anexo(self.prologo, i)?;
        Some(Anexo { tipo, file_offset: off, file_size: len, que: FIRMA_ANEXO | i as u8 })
    }

    /// Los anexos, en el orden de su tabla.
    pub fn anexos(&self) -> impl Iterator<Item = Anexo> + '_ {
        (0..self.cuantos_anexos).filter_map(move |i| self.anexo_n(i))
    }

    /// Cuantos anexos trae.
    pub fn cuantos_anexos(&self) -> usize {
        self.cuantos_anexos
    }

    /// El anexo de un tipo, si lo hay. (`revisar` ya exigio que no se repita.)
    pub fn anexo(&self, tipo: u8) -> Option<Anexo> {
        self.anexos().find(|a| a.tipo == tipo)
    }

    /// Hasta que byte del fichero hace falta leer para tener **todo lo que el
    /// cargador toca**: las tres regiones con bytes y los anexos que el kernel
    /// abre. Los recursos, el manifiesto y los simbolos van detras y no entran:
    /// se leen en ejecucion, por su puerta.
    pub fn hasta_donde_hace_falta(&self) -> u64 {
        let mut hasta = self.fin_tabla as u64;
        for r in self.regiones() {
            let fin = r.file_offset.saturating_add(r.file_size);
            if fin > hasta {
                hasta = fin;
            }
        }
        for a in self.anexos() {
            if !lo_lee_el_kernel(a.tipo) {
                continue;
            }
            let fin = a.file_offset.saturating_add(a.file_size);
            if fin > hasta {
                hasta = fin;
            }
        }
        hasta
    }
}

/// La entrada `i` de la tabla de anexos: `(tipo, offset, bytes)`. `None` si no
/// esta en el prologo o su relleno no es cero.
fn entrada_de_anexo(prologo: &[u8], i: usize) -> Option<(u8, u64, u64)> {
    let e = CABECERA + i * ANEXO;
    let tipo = *prologo.get(e)?;
    if *prologo.get(e + 1)? | *prologo.get(e + 2)? | *prologo.get(e + 3)? != 0 {
        return None;
    }
    if u32_en(prologo, e + 12)? != 0 {
        return None;
    }
    Some((tipo, u32_en(prologo, e + 4)? as u64, u32_en(prologo, e + 8)? as u64))
}

/// Cuantos trozos se comparan entre si: tres regiones, la cabecera con su
/// tabla, y los anexos.
const MAX_TROZOS: usize = 4 + MAX_ANEXOS;

/// **LA PUERTA.** Comprueba una imagen y no hace nada mas.
///
/// - `prologo`: los primeros bytes del fichero. Tiene que llegar al menos a la
///   cabecera y a la tabla de anexos entera; con **2 KiB sobra para cualquier
///   `.bex` que pueda existir** (64 + 16*16 = 320 bytes).
/// - `tam_fichero`: lo que mide el archivo ENTERO en el disco.
///
/// == Los dos numeros no son el mismo, y confundirlos es el bug ==
///
/// Los limites de regiones y anexos se comprueban contra `tam_fichero` --el
/// fichero completo-- y **no** contra lo que quepa en `prologo`. Desde que el
/// cargador trae las regiones una a una, "no esta en el prologo" es la
/// situacion normal de todas ellas.
///
/// Las mismas reglas que `bmo_abi::bef2::lector::leer`, sin `alloc` y sin
/// hashes (los hashes los comprueba quien COPIA, al aterrizar cada trozo).
pub fn revisar(prologo: &[u8], tam_fichero: usize) -> Result<Revisada<'_>, Falta> {
    if prologo.len() < CABECERA {
        return Err(Falta::NoLlegaNiALaCabecera);
    }
    // ** EL FORMATO LO DICE EL MAGIC. Solo hay uno. BEF1 --la cabecera con
    // tabla de secciones, ELF con otro nombre-- se rechaza por el primer
    // numero, como cualquier otro fichero que no sea de BMO-X.
    if u32_en(prologo, 0) != Some(MAGIC) {
        return Err(Falta::OtroFormato);
    }
    if prologo[4] != ABI {
        return Err(Falta::OtraVersionDelAbi);
    }
    let banderas = prologo[5];
    if banderas & !BANDERAS != 0 {
        return Err(Falta::PideAlgoQueNadieImplementa);
    }
    let ejecutable = banderas & EJECUTABLE != 0;
    let objeto = banderas & OBJETO != 0;
    if ejecutable && objeto {
        return Err(Falta::NoEsEjecutable);
    }
    if objeto {
        return Err(Falta::EsUnObjetoSinEnlazar);
    }
    if !ejecutable {
        return Err(Falta::NoEsEjecutable);
    }
    if u16_en(prologo, 6).ok_or(Falta::NoLlegaNiALaCabecera)? != 0
        || u64_en(prologo, 56).ok_or(Falta::NoLlegaNiALaCabecera)? != 0
    {
        return Err(Falta::CabeceraInvalida);
    }
    // ** Un bit de estado que el kernel no guarda es una corrupcion silenciosa
    // en la primera interrupcion, no una limitacion. Se dice y no se carga.
    let xcr0 = u64_en(prologo, 8).ok_or(Falta::NoLlegaNiALaCabecera)?;
    if xcr0 & !XCR0_PRESERVADO != 0 {
        return Err(Falta::ExtensionDeCpuQueNoSePreserva);
    }
    let entrada = u32_en(prologo, 16).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let cuantos_anexos = u32_en(prologo, 20).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    if cuantos_anexos > MAX_ANEXOS {
        return Err(Falta::DemasiadosAnexos);
    }
    let total = u32_en(prologo, 52).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    if total > tam_fichero {
        return Err(Falta::ImagenIncompleta);
    }
    let fin_tabla = CABECERA + cuantos_anexos * ANEXO;
    if fin_tabla > prologo.len() {
        return Err(Falta::TablaFueraDeLoLeido);
    }
    if fin_tabla > total {
        return Err(Falta::TablaFueraDelFichero);
    }

    let rev = Revisada { prologo, entrada, xcr0, banderas, cuantos_anexos, fin_tabla };

    // -- Cada trozo dentro del fichero, y ninguno pisando a otro -------------
    //
    // Un tramo VACIO no ocupa sitio y no se pelea con nadie, pero su offset
    // tiene que caer DENTRO del fichero igual: uno que apunte fuera es basura,
    // y la basura no pasa aunque no haga dano (lo encontro la pasada hostil).
    let mut trozos = [(0u64, 0u64); MAX_TROZOS];
    let mut n = 0usize;
    let mut apunta = |off: u64, len: u64| -> Result<(), Falta> {
        let fin = off.checked_add(len).ok_or(Falta::TramoFueraDelFichero)?;
        if fin > total as u64 {
            return Err(Falta::TramoFueraDelFichero);
        }
        if len == 0 {
            return Ok(());
        }
        trozos[n] = (off, fin);
        n += 1;
        Ok(())
    };
    // La cabecera y su tabla ocupan sitio como cualquier otra cosa.
    apunta(0, fin_tabla as u64)?;
    for c in [Cual::Codigo, Cual::Constantes, Cual::Datos] {
        let o = c.en_cabecera();
        let off = u32_en(prologo, o).ok_or(Falta::NoLlegaNiALaCabecera)? as u64;
        let len = u32_en(prologo, o + 4).ok_or(Falta::NoLlegaNiALaCabecera)? as u64;
        apunta(off, len)?;
    }
    let mut vistos = [0u8; MAX_ANEXOS];
    let mut hay_firma = false;
    for i in 0..cuantos_anexos {
        let (tipo, off, len) = entrada_de_anexo(prologo, i).ok_or(Falta::AnexoInvalido)?;
        if tipo == 0 || len == 0 || vistos[..i].contains(&tipo) {
            return Err(Falta::AnexoInvalido);
        }
        // Un anexo ENLACE es de un OBJETO: lo que llega aqui es un ejecutable,
        // y uno que lo traiga es una unidad sin enlazar disfrazada.
        if tipo == ANEXO_ENLACE {
            return Err(Falta::EsUnObjetoSinEnlazar);
        }
        if tipo == ANEXO_FIRMA {
            hay_firma = true;
        }
        vistos[i] = tipo;
        apunta(off, len)?;
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (trozos[i], trozos[j]);
            if a.0 < b.1 && b.0 < a.1 {
                return Err(Falta::TramosSeSolapan);
            }
        }
    }

    let Some(codigo) = rev.region(Cual::Codigo) else {
        return Err(Falta::SinCodigo);
    };
    if entrada as u64 >= codigo.file_size {
        return Err(Falta::EntryFueraDelCodigo);
    }
    // ** En BEF2 la firma NO es opcional: sin ella no hay con que comprobar que
    // lo que aterrizo es lo que se escribio, y el kernel aplica relocs que
    // vienen del mismo fichero.
    if !hay_firma {
        return Err(Falta::SinFirma);
    }
    Ok(rev)
}

fn u16_en(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o.checked_add(2)?)?.try_into().ok()?))
}
fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))
}
fn u64_en(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o.checked_add(8)?)?.try_into().ok()?))
}

#[cfg(test)]
mod tests;

// -- Lo que el CARGADOR necesita ademas de la decision -------------------------
//
// Son numeros del contrato que Ring 0 usa al aplicar relocs. Viven aqui y no en
// el kernel por lo mismo de siempre: una sola copia, atada por prueba.

/// Bytes que ocupa un reloc de BEF2. Espejo de `bmo_abi::bef2::RELOC`.
///
/// *** ERA 24, Y ERA MENTIRA DESDE B3 (2026-09-19). El paso B3 decia "Ring 0
/// no cambia una linea", y para los relocs era falso: el kernel descodificaba
/// el registro de 24 bytes de BEF1 sobre un anexo de registros de 16 bytes de
/// BEF2. En el Ryzen, cualquier programa con un puntero en sus datos --DOOM,
/// INTI con su monton-- habria caido en "relocation fuera de su seccion". Lo
/// cazo la lectura de B6, no el metal.
pub const RELOC_SIZE: usize = 16;

/// Offsets dentro de un reloc de BEF2, en el orden en que estan.
///
/// ```text
///    0  donde    u8    REGION donde se escribe (0 codigo, 1 constantes, 2 datos)
///    1  destino  u8    REGION a la que apunta (las cuatro; 3 = ceros)
///    2  0        u16
///    4  offset   u32   dentro de `donde`
///    8  addend   u64   dentro de `destino`
/// ```
///
/// La numeracion es la de [`Cual`], la MISMA con la que la firma nombra las
/// regiones: una sola numeracion, y por eso ya no hay tabla que cruzar.
pub mod reloc {
    /// `donde`: la region que se parchea. `u8`.
    pub const DONDE: usize = 0;
    /// `destino`: la region a la que apunta. `u8`.
    pub const DESTINO: usize = 1;
    /// Dos bytes a cero. Un reloc con relleno sucio no es un reloc.
    pub const RELLENO: usize = 2;
    /// `offset`: donde se escribe, dentro de `donde`. `u32`.
    pub const OFFSET: usize = 4;
    /// `addend`: offset del destino dentro de `destino`. `u64`.
    pub const ADDEND: usize = 8;
}

/// **CABE ESTE RELOC DENTRO DE LA REGION QUE DICE PARCHEAR?**
///
/// # Por que esta regla vive AQUI y no en el cargador (2026-08-25)
///
/// El toolchain la tenia y el cargador no, asi que un `.bex` copiado a mano al
/// FAT32 entraba con sus relocs sin mirar. Las regiones van seguidas en
/// memoria, o sea que un offset pasado de rosca CAE EN LA SIGUIENTE: no se sale
/// de la imagen, se mete en la region de al lado.
///
/// `parche` son los bytes que escribe (8). El tope es el mayor de `fichero` y
/// `mem` --el mismo criterio que el juez del contrato-- porque los ceros no
/// tienen bytes en el fichero y si se pueden parchear.
pub fn reloc_cabe(offset: u64, parche: u64, fichero: u64, mem: u64) -> bool {
    let tope = if mem > fichero { mem } else { fichero };
    match offset.checked_add(parche) {
        // ** El desbordamiento es un NO, no un panico. `offset` viene del
        // fichero, o sea de fuera: `u64::MAX` es un valor que alguien puede
        // escribir, y en `release` un `+` normal daria la vuelta y diria que si.
        None => false,
        Some(fin) => fin <= tope,
    }
}
