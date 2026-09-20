//! **El juez de un BEF2**: valida la imagen entera y devuelve una vista, o una
//! falta con nombre. Sin `alloc` y sin copiar un byte.
//!
//! [carril]  ROJO     decide si una imagen se ejecuta
//! [cuesta]  MAQUINA  lo que aqui pase por bueno lo mapea el kernel
//! [riesgo]  ESPEJO   la puerta (`bmo-bex-gate`) repite estas reglas sin `alloc`
//!                    ni dependencias; una prueba las ata
//!
//! ** Nada de "aviso": aqui se dice SI o se dice NO con su motivo. La
//! tolerancia del formato es una sola y esta escrita: un anexo de tipo
//! desconocido se SALTA, porque es data para otro. Todo lo demas se rechaza.

use super::*;

/// Por que una imagen no vale. Cada variante manda a mirar un sitio distinto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falta {
    /// No llega ni a la cabecera de 64 B.
    Corta,
    /// El magic no es `BEF2`.
    OtroFormato,
    /// Dice hablar otro ABI.
    OtroAbi,
    /// Una bandera que este sistema no conoce: cambia el significado de lo que
    /// viene detras.
    BanderaDesconocida,
    /// Ni ejecutable ni objeto, o las dos cosas a la vez.
    NiEjecutableNiObjeto,
    /// Un campo reservado que no es cero: o es basura, o es de una version que
    /// este sistema no entiende.
    ReservadoNoEsCero,
    /// Pide un estado de CPU que el kernel no sabe preservar (`xcr0`).
    EstadoDeCpuQueNoSePreserva,
    /// `total` no es lo que mide el fichero.
    TotalNoCuadra,
    /// Un ejecutable sin codigo.
    SinCodigo,
    /// Una region o un anexo se sale del fichero.
    SeSaleDelFichero,
    /// Dos trozos se pelean por los mismos bytes.
    SeSolapan,
    /// El punto de entrada no cae dentro del codigo.
    EntradaFueraDelCodigo,
    /// Mas anexos de los que caben.
    DemasiadosAnexos,
    /// Dos anexos del mismo tipo: cual de los dos vale?
    AnexoRepetido,
    /// Un anexo con el tipo 0, o VACIO (no existe, y un cero suele ser una
    /// tabla sin rellenar); o un ENLACE en un ejecutable (una unidad sin
    /// enlazar disfrazada). La puerta del kernel lo rechaza igual.
    AnexoQueNoVaAqui,
    /// El anexo de relocs esta mal formado, o un reloc apunta fuera.
    RelocMal,
    /// Un ejecutable sin firma: no hay con que comprobar que llego entero.
    SinFirma,
    /// La firma esta mal formada.
    FirmaMal,
    /// La firma no cubre algo que el kernel carga o lee.
    FirmaIncompleta,
    /// Un hash no cuadra con los bytes que dice cubrir.
    NoCuadraElHash,
}

impl Falta {
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::Corta => "no llega ni a la cabecera",
            Falta::OtroFormato => "no es un BEF2",
            Falta::OtroAbi => "habla otro ABI",
            Falta::BanderaDesconocida => "trae una bandera que este sistema no conoce",
            Falta::NiEjecutableNiObjeto => "ni ejecutable ni objeto, o las dos a la vez",
            Falta::ReservadoNoEsCero => "un campo reservado no es cero",
            Falta::EstadoDeCpuQueNoSePreserva => "pide un estado de CPU que el kernel no preserva",
            Falta::TotalNoCuadra => "el total no es lo que mide el fichero",
            Falta::SinCodigo => "un ejecutable sin codigo",
            Falta::SeSaleDelFichero => "una region o un anexo se sale del fichero",
            Falta::SeSolapan => "dos trozos se pelean por los mismos bytes",
            Falta::EntradaFueraDelCodigo => "la entrada cae fuera del codigo",
            Falta::DemasiadosAnexos => "demasiados anexos",
            Falta::AnexoRepetido => "dos anexos del mismo tipo",
            Falta::AnexoQueNoVaAqui => "un anexo que no va en esta clase de imagen",
            Falta::RelocMal => "un reloc mal formado o que apunta fuera",
            Falta::SinFirma => "un ejecutable sin firma",
            Falta::FirmaMal => "la firma esta mal formada",
            Falta::FirmaIncompleta => "la firma no cubre todo lo que el kernel toca",
            Falta::NoCuadraElHash => "un hash no cuadra con sus bytes",
        }
    }
}

/// Un trozo del fichero: donde empieza y cuanto mide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tramo {
    pub offset: u32,
    pub bytes: u32,
}

impl Tramo {
    fn fin(&self) -> u64 {
        self.offset as u64 + self.bytes as u64
    }
}

/// Una entrada de la tabla de anexos, ya comprobada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anexo {
    pub tipo: u8,
    pub tramo: Tramo,
}

/// **Una imagen que ya paso el juez.** Solo se construye con [`leer`], asi que
/// tenerla es la prueba de que las comprobaciones corrieron.
#[derive(Clone, Copy)]
pub struct Vista<'a> {
    bytes: &'a [u8],
    pub banderas: u8,
    pub xcr0: u64,
    pub entrada: u32,
    pub ceros: u32,
    codigo: Tramo,
    constantes: Tramo,
    datos: Tramo,
    cuantos_anexos: usize,
}

impl<'a> Vista<'a> {
    pub fn es_ejecutable(&self) -> bool {
        self.banderas & EJECUTABLE != 0
    }

    pub fn es_objeto(&self) -> bool {
        self.banderas & OBJETO != 0
    }

    pub fn quiere_pantalla(&self) -> bool {
        self.banderas & QUIERE_PANTALLA != 0
    }

    pub fn tramo(&self, r: Region) -> Tramo {
        match r {
            Region::Codigo => self.codigo,
            Region::Constantes => self.constantes,
            Region::Datos => self.datos,
            Region::Ceros => Tramo { offset: 0, bytes: self.ceros },
        }
    }

    /// Los bytes de una region. `Ceros` no tiene bytes en el fichero.
    pub fn region(&self, r: Region) -> &'a [u8] {
        if matches!(r, Region::Ceros) {
            return &[];
        }
        self.trozo(self.tramo(r))
    }

    /// Los bytes de un tramo que el juez ya dio por bueno.
    ///
    /// ** Un tramo VACIO no se rebana: la pasada hostil (`tests/hostile.rs`)
    /// encontro que una region de 0 bytes con un offset fuera del fichero
    /// pasaba el juez -- no ocupa sitio, no se compara con nadie -- y despues
    /// `bytes[65536..65536]` sobre 704 bytes panicaba. El juez ya lo rechaza
    /// (`SeSaleDelFichero`); esto es el cinturon por si algun dia deja de
    /// hacerlo: un `Vista` no puede panicar por construccion.
    fn trozo(&self, t: Tramo) -> &'a [u8] {
        if t.bytes == 0 {
            return &[];
        }
        &self.bytes[t.offset as usize..t.offset as usize + t.bytes as usize]
    }

    /// **Lo que ocupa en memoria**: las cuatro regiones.
    pub fn memoria(&self) -> u64 {
        self.codigo.bytes as u64
            + self.constantes.bytes as u64
            + self.datos.bytes as u64
            + self.ceros as u64
    }

    pub fn cuantos_anexos(&self) -> usize {
        self.cuantos_anexos
    }

    pub fn anexo_n(&self, i: usize) -> Option<Anexo> {
        if i >= self.cuantos_anexos {
            return None;
        }
        anexo_en(self.bytes, i)
    }

    pub fn anexos(&self) -> impl Iterator<Item = Anexo> + '_ {
        (0..self.cuantos_anexos).filter_map(move |i| self.anexo_n(i))
    }

    /// Los bytes de un anexo por su tipo.
    pub fn anexo(&self, tipo: u8) -> Option<&'a [u8]> {
        let a = self.anexos().find(|a| a.tipo == tipo)?;
        Some(self.trozo(a.tramo))
    }

    /// **La CADENA de hashes**: el BLAKE3 de todas las entradas de la firma,
    /// en orden. Es lo que firma `bmo-firmar` con Ed25519 -- una firma sobre un
    /// solo numero que ya resume la imagen entera, en vez de N firmas.
    pub fn cadena_de_hashes(&self) -> Option<[u8; 32]> {
        let firma = self.anexo(ANEXO_FIRMA)?;
        let cuantos = u32_en(firma, 0)? as usize;
        let fin = FIRMA_CABECERA + cuantos * FIRMA_HASH;
        Some(crate::bef::blake3::blake3_256(firma.get(FIRMA_CABECERA..fin)?))
    }

    /// Los relocs, ya comprobados al leer.
    pub fn relocs(&self) -> impl Iterator<Item = Reloc> + '_ {
        let datos = self.anexo(ANEXO_RELOCS).unwrap_or(&[]);
        (0..datos.len() / RELOC).filter_map(move |i| Reloc::de_bytes(&datos[i * RELOC..]))
    }
}

/// Lee una entrada de la tabla de anexos SIN comprobar nada mas que sus bytes.
fn anexo_en(bytes: &[u8], i: usize) -> Option<Anexo> {
    let e = CABECERA + i * ANEXO;
    let tipo = *bytes.get(e)?;
    if bytes.get(e + 1)? | bytes.get(e + 2)? | bytes.get(e + 3)? != 0 {
        return None;
    }
    if u32_en(bytes, e + 12)? != 0 {
        return None;
    }
    Some(Anexo {
        tipo,
        tramo: Tramo {
            offset: u32_en(bytes, e + 4)?,
            bytes: u32_en(bytes, e + 8)?,
        },
    })
}

/// Cuantos trozos se comparan entre si: tres regiones, la cabecera con su
/// tabla, y los anexos.
const MAX_TROZOS: usize = 4 + MAX_ANEXOS;

/// **El juez.** Ver [`Falta`] para lo que puede contestar.
pub fn leer(bytes: &[u8]) -> Result<Vista<'_>, Falta> {
    if bytes.len() < CABECERA {
        return Err(Falta::Corta);
    }
    if u32_en(bytes, 0) != Some(MAGIC) {
        return Err(Falta::OtroFormato);
    }
    if bytes[4] != ABI {
        return Err(Falta::OtroAbi);
    }
    let banderas = bytes[5];
    if banderas & !BANDERAS != 0 {
        return Err(Falta::BanderaDesconocida);
    }
    let ejecutable = banderas & EJECUTABLE != 0;
    let objeto = banderas & OBJETO != 0;
    if ejecutable == objeto {
        return Err(Falta::NiEjecutableNiObjeto);
    }
    if u16::from_le_bytes([bytes[6], bytes[7]]) != 0 || u64_en(bytes, 56) != Some(0) {
        return Err(Falta::ReservadoNoEsCero);
    }
    let xcr0 = u64_en(bytes, 8).ok_or(Falta::Corta)?;
    // ** Un bit de estado que el kernel no guarda es una corrupcion silenciosa
    // en la primera interrupcion, no una limitacion. Se dice y no se carga.
    if xcr0 & !XCR0_PRESERVADO != 0 {
        return Err(Falta::EstadoDeCpuQueNoSePreserva);
    }
    let entrada = u32_en(bytes, 16).ok_or(Falta::Corta)?;
    let cuantos = u32_en(bytes, 20).ok_or(Falta::Corta)? as usize;
    let tramo_en = |o: usize| -> Tramo {
        Tramo {
            offset: u32_en(bytes, o).unwrap_or(0),
            bytes: u32_en(bytes, o + 4).unwrap_or(0),
        }
    };
    let codigo = tramo_en(24);
    let constantes = tramo_en(32);
    let datos = tramo_en(40);
    let ceros = u32_en(bytes, 48).ok_or(Falta::Corta)?;
    let total = u32_en(bytes, 52).ok_or(Falta::Corta)?;
    if total as usize != bytes.len() {
        return Err(Falta::TotalNoCuadra);
    }
    if cuantos > MAX_ANEXOS {
        return Err(Falta::DemasiadosAnexos);
    }
    let fin_tabla = CABECERA + cuantos * ANEXO;
    if fin_tabla > bytes.len() {
        return Err(Falta::SeSaleDelFichero);
    }

    // -- Los trozos, cada uno dentro del fichero y sin pisarse ---------------
    let mut trozos = [(0u64, 0u64); MAX_TROZOS];
    let mut n = 0usize;
    let apunta = |t: Tramo, trozos: &mut [(u64, u64); MAX_TROZOS], n: &mut usize| -> Result<(), Falta> {
        // Un tramo vacio no ocupa sitio y no se pelea con nadie, pero su
        // offset tiene que caer DENTRO del fichero igual: uno que apunte fuera
        // es basura, y la basura no pasa aunque no haga dano.
        if t.fin() > total as u64 {
            return Err(Falta::SeSaleDelFichero);
        }
        if t.bytes == 0 {
            return Ok(());
        }
        trozos[*n] = (t.offset as u64, t.fin());
        *n += 1;
        Ok(())
    };
    // La cabecera y su tabla ocupan sitio como cualquier otra cosa.
    apunta(Tramo { offset: 0, bytes: fin_tabla as u32 }, &mut trozos, &mut n)?;
    apunta(codigo, &mut trozos, &mut n)?;
    apunta(constantes, &mut trozos, &mut n)?;
    apunta(datos, &mut trozos, &mut n)?;

    let mut vistos = [0u8; MAX_ANEXOS];
    for i in 0..cuantos {
        let a = anexo_en(bytes, i).ok_or(Falta::ReservadoNoEsCero)?;
        if a.tipo == 0 || a.tramo.bytes == 0 || (ejecutable && a.tipo == ANEXO_ENLACE) {
            return Err(Falta::AnexoQueNoVaAqui);
        }
        if vistos[..i].contains(&a.tipo) {
            return Err(Falta::AnexoRepetido);
        }
        vistos[i] = a.tipo;
        apunta(a.tramo, &mut trozos, &mut n)?;
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (trozos[i], trozos[j]);
            if a.0 < b.1 && b.0 < a.1 {
                return Err(Falta::SeSolapan);
            }
        }
    }

    if ejecutable && codigo.bytes == 0 {
        return Err(Falta::SinCodigo);
    }
    if codigo.bytes > 0 && entrada >= codigo.bytes {
        return Err(Falta::EntradaFueraDelCodigo);
    }

    let v = Vista {
        bytes,
        banderas,
        xcr0,
        entrada,
        ceros,
        codigo,
        constantes,
        datos,
        cuantos_anexos: cuantos,
    };

    comprobar_relocs(&v)?;
    if ejecutable {
        comprobar_firma(&v)?;
    }
    Ok(v)
}

/// Cada reloc escribe ocho bytes DENTRO de una region que existe, y apunta a
/// un sitio DENTRO de otra. Un reloc fuera es memoria de otro pisada.
fn comprobar_relocs(v: &Vista<'_>) -> Result<(), Falta> {
    let Some(datos) = v.anexo(ANEXO_RELOCS) else {
        return Ok(());
    };
    if datos.len() % RELOC != 0 {
        return Err(Falta::RelocMal);
    }
    for i in 0..datos.len() / RELOC {
        let r = Reloc::de_bytes(&datos[i * RELOC..]).ok_or(Falta::RelocMal)?;
        // En los CEROS no se escribe: no tienen bytes en el fichero.
        if matches!(r.donde, Region::Ceros) {
            return Err(Falta::RelocMal);
        }
        let donde = v.tramo(r.donde);
        if r.offset as u64 + 8 > donde.bytes as u64 {
            return Err(Falta::RelocMal);
        }
        let destino = v.tramo(r.destino);
        if r.addend > destino.bytes as u64 {
            return Err(Falta::RelocMal);
        }
    }
    Ok(())
}

/// La firma cubre TODO lo que el kernel carga o lee: las regiones con bytes,
/// los relocs y los requisitos. Lo que no cubre puede cambiar sin que nadie se
/// entere, y el kernel lo aplica igual.
fn comprobar_firma(v: &Vista<'_>) -> Result<(), Falta> {
    let firma = v.anexo(ANEXO_FIRMA).ok_or(Falta::SinFirma)?;
    if firma.len() < FIRMA_CABECERA {
        return Err(Falta::FirmaMal);
    }
    let cuantos = u32_en(firma, 0).ok_or(Falta::FirmaMal)? as usize;
    let algo = u32_en(firma, 4).ok_or(Falta::FirmaMal)?;
    let fin = FIRMA_CABECERA + cuantos * FIRMA_HASH;
    if fin > firma.len() {
        return Err(Falta::FirmaMal);
    }
    // Lo que va DETRAS de los hashes lo dice el algoritmo, y no se admite un
    // sobrante sin nombre: un anexo con bytes de mas es un sitio donde
    // esconder algo que nadie mira.
    let esperado = match algo {
        ALGO_NINGUNO => fin,
        ALGO_ED25519 => fin + FIRMA_ED25519,
        _ => return Err(Falta::FirmaMal),
    };
    if esperado != firma.len() {
        return Err(Falta::FirmaMal);
    }

    let mut cubre_region = [false; 3];
    let mut cubiertos = [false; MAX_ANEXOS];
    for i in 0..cuantos {
        let e = FIRMA_CABECERA + i * FIRMA_HASH;
        let que = firma[e];
        if firma[e + 1..e + 8].iter().any(|b| *b != 0) {
            return Err(Falta::FirmaMal);
        }
        let digest = &firma[e + 8..e + FIRMA_HASH];
        let trozo: &[u8] = if que & FIRMA_ANEXO != 0 {
            let idx = (que & !FIRMA_ANEXO) as usize;
            let a = v.anexo_n(idx).ok_or(Falta::FirmaMal)?;
            if a.tipo == ANEXO_FIRMA {
                return Err(Falta::FirmaMal);
            }
            cubiertos[idx] = true;
            v.trozo(a.tramo)
        } else {
            let r = Region::de(que).ok_or(Falta::FirmaMal)?;
            if matches!(r, Region::Ceros) {
                return Err(Falta::FirmaMal);
            }
            cubre_region[que as usize] = true;
            v.region(r)
        };
        if crate::bef::blake3::blake3_256(trozo)[..] != *digest {
            return Err(Falta::NoCuadraElHash);
        }
    }

    for (i, r) in [Region::Codigo, Region::Constantes, Region::Datos].iter().enumerate() {
        if v.tramo(*r).bytes > 0 && !cubre_region[i] {
            return Err(Falta::FirmaIncompleta);
        }
    }
    for i in 0..v.cuantos_anexos() {
        let a = v.anexo_n(i).ok_or(Falta::FirmaMal)?;
        if lo_lee_el_kernel(a.tipo) && a.tipo != ANEXO_FIRMA && !cubiertos[i] {
            return Err(Falta::FirmaIncompleta);
        }
    }
    Ok(())
}
