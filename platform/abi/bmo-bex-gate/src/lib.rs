//! **LA DECISION: es admisible este BEX?** Y nada mas.
//!
//! ## Por que existe este crate
//!
//! La respuesta a *"se puede ejecutar esto?"* estaba escrita **dos veces**:
//!
//! ```text
//!   bmo-abi/bef/validator.rs   1.281 lineas   con alloc   decide Y explica
//!   kernel/task/bex.rs           ~200 lineas   sin alloc   decide Y planifica
//! ```
//!
//! Y no estaban duplicadas por descuido. Estaban duplicadas porque **la decision
//! vivia incrustada en dos trabajos distintos**: una construye mensajes de error
//! con `String` para el que compila, la otra construye el plan de mapeo para el
//! que ejecuta. Compartirlas era imposible mientras la decision no fuera una cosa
//! por su cuenta.
//!
//! Aqui es una cosa por su cuenta. Los dos siguen haciendo lo suyo encima:
//!
//! ```text
//!                     bmo-bex-gate        <- la DECISION
//!                      /          \
//!       validator (alloc)          bex.rs (Ring 0)
//!       anade MENSAJES             anade el PLAN
//! ```
//!
//! **Ninguno de los dos es dueno de la decision, asi que ninguno puede desviarse
//! de ella.** Que es distinto de tener una prueba que compare los dos: eso caza
//! la divergencia despues de escribirla; esto la hace imposible.
//!
//! ## Y lo que esto NO hace
//!
//! No mapea, no reserva, no lee disco, no explica en prosa, y **no opina**. Una
//! imagen que pasa por aqui es una imagen **bien formada**, y eso no quiere decir
//! que sea buena, ni segura, ni tuya. Quien decide si se ejecuta es el sistema,
//! con esto y con lo demas -- la firma, los requisitos, quien la lanza.
//!
//! > Bien formado no es lo mismo que de fiar. Confundirlo es como creerse un
//! > documento porque la letra es bonita.

#![no_std]
#![forbid(unsafe_code)]

pub const MAX_SECCIONES: usize = 16;

// -- Tipos de seccion --------------------------------------------------------

pub const CODE: u8 = 0x01;
pub const RODATA: u8 = 0x02;
pub const DATA: u8 = 0x03;
pub const BSS: u8 = 0x04;
pub const RELOCS: u8 = 0x07;
pub const SIGNATURE: u8 = 0x0F;
pub const REQUISITOS: u8 = 0x15;

/// Se mapea en el espacio del programa?
///
/// **LA REGLA**: solo cuatro tipos son memoria del programa. Todo lo demas
/// --manifiesto, firma, simbolos, recursos, y **cualquier tipo desconocido**--
/// es data para otro, y se salta. Un tipo que no me incumbe no es un error: es
/// data que no voy a abrir. Es lo que ha mantenido vivo a ELF treinta anios.
pub fn se_carga(kind: u8) -> bool {
    matches!(kind, CODE | RODATA | DATA | BSS)
}

/// ** Los tipos que piden ENLAZADO DINAMICO o TLS: `Imports` 0x05, `Exports`
/// 0x06, `Tls` 0x0C (2026-09-19).
///
/// Un tipo desconocido se salta -- es data para otro. Estos NO: dicen "alguien
/// resolvera mis llamadas al cargar", y en BMO-X no hay nadie (enlaza estatico
/// desde el 17-09 y no tiene TLS). Saltarlos era cargar un programa con
/// llamadas a ninguna parte.
pub fn se_lee(kind: u8) -> bool {
    se_carga(kind) || matches!(kind, RELOCS | SIGNATURE | REQUISITOS)
}

// -- Banderas de la cabecera -------------------------------------------------

pub const SECCION_FLAG_EXEC: u32 = 1 << 2;

/// **Por que no se admite.** Cada una manda a mirar un sitio distinto, que es la
/// razon de que sean variantes y no un booleano.
///
/// Es `Copy` y sin datos prestados a proposito: cruza la frontera de Ring 0 sin
/// reservar nada, y quien quiera contarlo con numeros los saca del fichero, que
/// sigue teniendo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falta {
    NoLlegaNiALaCabecera,
    /// Magic, version mayor, o cero secciones.
    CabeceraInvalida,
    OtraArquitectura,
    OtroOrdenDeBytes,
    /// Declara una extension de CPU cuyo estado el sistema no sabe preservar.
    ExtensionDeCpuQueNoSePreserva,
    OtraVersionDelAbi,
    NoEsEjecutable,
    /// An object that nobody linked: the fix is `bmo-enlazar`, not a flag.
    EsUnObjetoSinEnlazar,
    /// Ver [`FLAGS_NO_IMPLEMENTADAS`].
    PideAlgoQueNadieImplementa,
    /// Trae una seccion de imports, exports o TLS: pide que alguien la
    /// enlace al cargar, y BMO-X enlaza estatico. Ver [`PIDEN_ENLAZADO_DINAMICO`].
    EnlazadoDinamico,
    /// Dice `FIRMADO` y no trae seccion de firma.
    CabeceraQueSeDesmiente,
    DemasiadasSecciones,
    /// La tabla no cabe en lo que se paso. Quien llama puede leer mas y volver.
    TablaFueraDeLoLeido,
    TablaFueraDelFichero,
    SeccionInvalida,
    /// Dos secciones se pelean por los mismos bytes del fichero.
    SeccionesSeSolapan,
    /// Una seccion declara bytes que caen fuera del fichero.
    SeccionFueraDelFichero,
    SinCodigo,
    LaCodigoNoEsEjecutable,
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
            Falta::CabeceraInvalida => "cabecera invalida (magic, version o 0 secciones)",
            Falta::OtraArquitectura => "otra arquitectura",
            Falta::OtroOrdenDeBytes => "otro orden de bytes",
            Falta::ExtensionDeCpuQueNoSePreserva => "pide una extension de CPU que no se preserva",
            Falta::OtraVersionDelAbi => "otra version del ABI",
            Falta::NoEsEjecutable => "no esta marcado como ejecutable",
            Falta::EsUnObjetoSinEnlazar => "es un OBJETO sin enlazar (.bo): pasalo por bmo-enlazar",
            Falta::PideAlgoQueNadieImplementa => "la cabecera pide algo que este sistema no hace",
            Falta::EnlazadoDinamico => "pide enlazado dinamico o TLS: BMO-X enlaza estatico (pasalo por bmo-enlazar)",
            Falta::CabeceraQueSeDesmiente => "dice venir firmado y no trae firma",
            Falta::DemasiadasSecciones => "demasiadas secciones",
            Falta::TablaFueraDeLoLeido => "la tabla de secciones no cabe en lo leido",
            Falta::TablaFueraDelFichero => "la tabla de secciones cae fuera del fichero",
            Falta::SeccionInvalida => "una seccion esta mal formada",
            Falta::SeccionesSeSolapan => "dos secciones se pelean por los mismos bytes",
            Falta::SeccionFueraDelFichero => "una seccion cae fuera del fichero",
            Falta::SinCodigo => "no hay seccion de codigo",
            Falta::LaCodigoNoEsEjecutable => "la seccion de codigo no es ejecutable",
            Falta::EntryFueraDelCodigo => "el punto de entrada cae fuera del codigo",
            Falta::ImagenIncompleta => "llegaron menos bytes de los que la imagen dice medir",
        }
    }
}

/// **BEF2**: el formato propio (`bmo_abi::bef2`). La puerta lo lee aqui y se
/// lo presenta al kernel como secciones, para que Ring 0 no cambie.
pub mod bef2;

/// Una seccion, ya comprobada. Los numeros son los del fichero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seccion {
    /// Su indice **en la tabla del fichero**. Es con lo que la firma la nombra.
    pub indice: usize,
    pub kind: u8,
    pub flags: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub alignment: u16,
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
    cuantas: usize,
    entry_offset: u64,
    /// Donde acaba el prologo: la cabecera mas su tabla. Los dos formatos la
    /// tienen en sitios distintos y todo lo demas empieza detras.
    fin_tabla: usize,
}

impl<'a> Revisada<'a> {
    pub fn entry_offset(&self) -> u64 {
        self.entry_offset
    }
    pub fn cuantas(&self) -> usize {
        self.cuantas
    }
    /// La seccion `i`, con el indice con el que la firma la nombra.
    ///
    /// ** BEF2 no tiene tabla de secciones: lo que se devuelve son las cuatro
    /// REGIONES y los ANEXOS presentados como secciones, con el mismo tipo y el
    /// mismo indice de hash. Ver `bef2::seccion`.
    pub fn seccion(&self, i: usize) -> Option<Seccion> {
        if i >= self.cuantas {
            return None;
        }
        bef2::seccion(self.prologo, i)
    }
    /// Recorre las secciones en el orden del fichero.
    pub fn secciones(&self) -> impl Iterator<Item = Seccion> + '_ {
        (0..self.cuantas).filter_map(move |i| self.seccion(i))
    }
    /// La primera seccion de un tipo, si la hay.
    pub fn buscar(&self, kind: u8) -> Option<Seccion> {
        self.secciones().find(|s| s.kind == kind)
    }
    /// Hasta que byte del fichero hace falta leer para tener **todo lo que el
    /// cargador toca**: codigo, datos, relocations, hashes y requisitos.
    ///
    /// Los recursos van detras y no entran: se leen en ejecucion, por su puerta.
    pub fn hasta_donde_hace_falta(&self) -> u64 {
        let mut hasta = self.fin_tabla as u64;
        for s in self.secciones() {
            if s.kind == BSS || !se_lee(s.kind) {
                continue;
            }
            let fin = s.file_offset.saturating_add(s.file_size);
            if fin > hasta {
                hasta = fin;
            }
        }
        hasta
    }
}

/// **LA PUERTA.** Comprueba una imagen BEX y no hace nada mas.
///
/// - `prologo`: los primeros bytes del fichero. Tiene que llegar al menos a la
///   cabecera y a la tabla de anexos entera; con **2 KiB sobra para cualquier
///   `.bex` que pueda existir** (64 + 16*16 = 320 bytes).
/// - `tam_fichero`: lo que mide el archivo ENTERO en el disco.
///
/// == Los dos numeros no son el mismo, y confundirlos es el bug ==
///
/// Los limites de las secciones se comprueban contra `tam_fichero` --el fichero
/// completo-- y **no** contra lo que quepa en `prologo`. Desde que el cargador
/// trae las secciones una a una, "no esta en el prologo" es la situacion normal
/// de todas ellas. Medirlas contra el prologo rechazaria toda imagen que no
/// cupiera en dos kilos, o sea todas.
pub fn revisar(prologo: &[u8], tam_fichero: usize) -> Result<Revisada<'_>, Falta> {
    if prologo.len() < 4 {
        return Err(Falta::NoLlegaNiALaCabecera);
    }
    let magic = u32_en(prologo, 0).ok_or(Falta::NoLlegaNiALaCabecera)?;
    // ** EL FORMATO LO DICE EL MAGIC. Solo hay uno: BEF2 (2026-09-19, B6 de
    // `docs/plan/PLAN_BEF_NATIVO.md`). BEF1 --la cabecera con tabla de
    // secciones, ELF con otro nombre-- se rechaza por el primer numero, como
    // cualquier otro fichero que no sea de BMO-X.
    if magic != bef2::MAGIC {
        return Err(Falta::CabeceraInvalida);
    }
    bef2::revisar(prologo, tam_fichero)
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

// == ** LA DISPOSICION DEL FORMATO, EN UN SOLO SITIO =========================
//
// # El agujero que esto cierra
//
// El BEF tiene **DOS lectores y ningun compilador entre ellos**:
//
// ```text
//    bmo-abi/bef/*.rs        structs con `repr(C)`, para el toolchain
//    kernel/task/bex.rs      bytes a mano, porque el kernel NO importa bmo-abi
// ```
//
// Y no importarlo es una decision correcta --`bmo-abi` trae `alloc` y el kernel
// no puede-- pero tiene un precio que hasta hoy nadie pagaba: **los offsets
// estaban escritos dos veces**, una como campos de un struct y otra como
// literales dentro de `leer_reloc`. El propio comentario del kernel lo decia:
//
// > *"Tamano y disposicion fijados por `bmo_abi::bef::relocations::Relocation`,
// >  que este kernel no importa a proposito... si el struct cambiara de forma,
// >  estos offsets son el unico sitio a tocar."*
//
// "El unico sitio a tocar" **es la definicion de una duplicacion que se olvida**.
// Mover un campo del struct compila igual, pasa todos los tests del toolchain, y
// el cargador escribe la direccion equivocada dentro de un proceso.
//
// # La salida: no vigilar la copia, QUITARLA
//
// Este crate ya lo comparten los dos --el kernel lo importa para la puerta, y
// `bmo-verify` para no separarse de el-- y no tiene dependencias. Asi que los
// offsets viven aqui, los usa el kernel, y `bmo-abi` los CLAVA a su struct con
// `offset_of!` en una prueba.
//
// De dos verdades que hay que mantener a mano se pasa a una verdad y una prueba
// que la ata. Es el mismo movimiento que el guardian de `bmo.h`, salvo que alli
// los nombres no se podian unificar y aqui si.

/// Bytes que ocupa un reloc de BEF2. Espejo de `bmo_abi::bef2::RELOC`.
///
/// *** ERA 24, Y ERA MENTIRA DESDE B3 (2026-09-19). El paso B3 decia "Ring 0
/// no cambia una linea", y para los relocs era falso: el kernel descodificaba
/// el registro de 24 bytes de BEF1 (`offset u64, symbol_idx u32, kind,
/// target_section, addend i64`, secciones 0 code / 1 data / 2 rodata) sobre un
/// anexo de registros de 16 bytes de BEF2. En el Ryzen, cualquier programa con
/// un puntero en sus datos --DOOM, INTI con su monton-- habria caido en
/// "relocation fuera de su seccion". Lo cazo la lectura de B6, no el metal.
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
/// La numeracion es la de `bmo_abi::bef2::Region`, la MISMA con la que la
/// firma nombra las regiones y con la que `bef2.rs` las presenta como
/// secciones: una sola numeracion, y por eso ya no hay tabla que cruzar.
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

/// **CABE ESTA RELOCATION DENTRO DE LA SECCION QUE DICE PARCHEAR?**
///
/// # Por que esta regla vive AQUI y no en el cargador (2026-08-25)
///
/// El toolchain juzga un `.bex` con DOS capas --`revisar()` y
/// `bmo_abi::bef::validator`-- y el cargador del kernel solo con la primera.
/// Esta comprobacion vivia unicamente en la segunda, o sea que **un `.bex`
/// copiado a mano al FAT32 entraba sin que nadie mirara sus relocations**.
///
/// La respuesta obvia era anadirle la comprobacion al kernel. Es la
/// equivocada: serian **dos copias de la misma decision**, que es exactamente
/// el problema que `bmo-bex-gate` se creo el 2026-08-10 para terminar.
///
/// ```text
///    la REGLA        vive aqui, una vez, sin alloc y sin dependencias
///    los DATOS       los pone cada llamante, porque cada uno tiene otros
/// ```
///
/// [!] Y hacen falta los dos, porque `revisar()` **no puede** hacerlo: en el
/// kernel recibe solo el PROLOGO del fichero, y la tabla de relocations vive
/// mucho mas alla --la de DOOM son 30.840 bytes al final--. No es que no se
/// quisiera: es que ahi todavia no estan esos bytes. Por eso la regla es una
/// funcion suelta y no una linea mas dentro de `revisar`.
///
/// # Que pasa si no se comprueba
///
/// Las secciones se colocan **seguidas** desde `USER_IMAGE_BASE`, asi que un
/// `offset` mas grande que su seccion no se sale de la imagen: **cae en la
/// SIGUIENTE**. El cargador comprueba que el destino este dentro de la pagina
/// que esta parcheando --lo esta-- y escribe.
///
/// > Una reloc que dice `.data + 0x9000` en una `.data` de 0x400 no falla:
/// > **acierta en otra seccion.** Y como el hash de cada seccion se cierra
/// > ANTES de parchear, tampoco lo caza el hash.
///
/// No es una fuga fuera del proceso --el marco es suyo-- pero si es un
/// programa que se corrompe a si mismo en silencio, que es la clase de fallo
/// que tarda semanas en atribuirse.
///
/// # Los parametros, y por que `mem` y `fichero` son dos
///
/// Una `.bss` ocupa en memoria y no en el fichero, y una `.data` con relleno
/// tiene `mem_size > file_size`. Se parchea sobre lo que hay **en memoria**,
/// asi que manda `mem`; se pasa `fichero` porque una seccion cuyo `mem` fuera
/// menor ya seria invalida y aqui se ve gratis.
///
/// `parche` son los bytes que la relocation escribe: 8 para `SeccionAbs64`.
pub fn reloc_cabe(offset: u64, parche: u64, fichero: u64, mem: u64) -> bool {
    // El tope es el mayor de los dos: `validator` lo hace asi desde el
    // principio y aqui se conserva el mismo criterio A PROPOSITO -- dos jueces
    // que dan veredictos distintos sobre el mismo fichero son peor que uno.
    let tope = if mem > fichero { mem } else { fichero };
    match offset.checked_add(parche) {
        // ** El desbordamiento es un NO, no un panico. `offset` viene del
        // fichero, o sea de fuera: `u64::MAX` es un valor que alguien puede
        // escribir, y en `release` un `+` normal daria la vuelta y diria que si.
        None => false,
        Some(fin) => fin <= tope,
    }
}

