//! **NTFS, SOLO LECTURA** (N0, 29-09): el disco `Personal (D:)` de Windows,
//! donde vive Cyberpunk 2077, leido por BMO-X sin copiarlo a ningun sitio.
//!
//! Pedido por el propietario (29-09): *"el juego Cyberpunk 2077 lo tengo
//! instalado en PERSONAL: que mi BMO-X aprenda a LEER NTFS"*. Hasta hoy el
//! plan decia que no hacia falta (Ludoteca, seccion 8: copiar a ESTRATOS);
//! el propietario decidio que si, y que SOLO leer.
//!
//! ```text
//!    el sector de arranque  "NTFS    ", bytes por sector y por cluster, donde
//!                           empieza el MFT, cuanto mide un registro y un indice
//!    el MFT                 un registro por fichero ("FILE"), con sus arreglos
//!                           (update sequence) deshechos antes de leer nada
//!    los atributos          residentes (el valor dentro del registro) y no
//!                           residentes (una lista de tramos de clusteres)
//!    las carpetas           el indice $I30: $INDEX_ROOT y los bloques "INDX" de
//!                           $INDEX_ALLOCATION que su $BITMAP dice VIVOS
//!    los nombres            UTF-16, largos, sin 8.3 (el espacio DOS se salta),
//!                           comparados sin mayusculas (ASCII y Latin-1)
//!    los ficheros           $DATA sin nombre: residente o por tramos, con
//!                           huecos (sparse), mas de 4 GiB (todo en u64) y
//!                           partido en varios registros ($ATTRIBUTE_LIST)
//! ```
//!
//! Lo que NO hace, dicho: escribir (no hay ni una llamada a `write`), ni leer
//! un fichero COMPRIMIDO o CIFRADO (lo dice con su nombre, no devuelve basura),
//! ni un MFT tan partido que su propio $DATA necesite $ATTRIBUTE_LIST.
//!
//! Sin `alloc`: el volumen lleva sus buferes (unos 16 KiB), como FAT32.
//! El banco (`pruebas.rs`) lee un disco NTFS HECHO POR `mkntfs` y llenado por
//! `ntfs-3g` (`prueba/disco.ntfs`, ver `platform/drivers/storage/ntfs/prueba/COMO.md`): no un disco de
//! mentira armado a mano con lo que yo creo que es NTFS.

#![cfg_attr(not(test), no_std)]

use bmo_block::BlockDevice;

#[cfg(test)]
mod pruebas;

/// Lo mas que mide un registro del MFT o un bloque de indice que se acepta
/// (lo normal: 1 KiB y 4 KiB; en discos de 4 KiB por sector, 4 KiB los dos).
pub const MAX_REGISTRO: usize = 4096;
pub const MAX_INDICE: usize = 4096;
/// El registro de la carpeta raiz.
pub const RAIZ: u64 = 5;
/// El registro de `$Bitmap`: un bit por cluster.
const BITMAP: u64 = 6;
/// Los arreglos de NTFS van cada 512 bytes, sea cual sea el sector.
const PASO_ARREGLO: usize = 512;
/// Tipos de atributo.
const AT_LISTA: u32 = 0x20;
const AT_DATOS: u32 = 0x80;
const AT_RAIZ_INDICE: u32 = 0x90;
const AT_ASIGNACION_INDICE: u32 = 0xA0;
const AT_MAPA: u32 = 0xB0;
const AT_FIN: u32 = 0xFFFF_FFFF;
/// Banderas de un atributo.
const COMPRIMIDO: u16 = 0x0001;
const CIFRADO: u16 = 0x4000;

/// **Por que no se pudo.** Cada uno con su nombre: un lector de solo lectura
/// que se equivoca no rompe el disco, pero una respuesta sin motivo no se
/// puede arreglar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoNtfs {
    /// El disco no contesto.
    Leer,
    /// El sector de arranque no es de NTFS.
    NoEsNtfs,
    /// Algo del volumen no se sostiene (un registro sin "FILE", un arreglo
    /// que no cuadra, un atributo que se sale del registro...).
    Forma(&'static str),
    /// Mide mas de lo que este lector acepta (ver `MAX_REGISTRO`).
    Grande(&'static str),
    Comprimido,
    Cifrado,
    NoEsta,
    NoEsCarpeta,
    NoEsFichero,
}

impl NoNtfs {
    pub fn nombre(self) -> &'static str {
        match self {
            NoNtfs::Leer => "el disco no contesto",
            NoNtfs::NoEsNtfs => "no es un volumen NTFS",
            NoNtfs::Forma(q) | NoNtfs::Grande(q) => q,
            NoNtfs::Comprimido => "fichero COMPRIMIDO por NTFS: este lector no lo descomprime",
            NoNtfs::Cifrado => "fichero CIFRADO (EFS): no se lee",
            NoNtfs::NoEsta => "no esta",
            NoNtfs::NoEsCarpeta => "no es una carpeta",
            NoNtfs::NoEsFichero => "es una carpeta, no un fichero",
        }
    }
}

type R<T> = Result<T, NoNtfs>;

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

// == Lo que dice el sector de arranque ========================================

/// **La forma del volumen**, del sector de arranque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Forma {
    pub bytes_por_sector: u32,
    pub bytes_por_cluster: u64,
    pub sectores: u64,
    pub mft_lcn: u64,
    pub registro: u32,
    pub indice: u32,
}

/// Un valor de "clusteres por registro" de NTFS: positivo = clusteres, negativo
/// = `2^-v` bytes.
fn medida_de(v: u8, bytes_por_cluster: u64) -> Option<u64> {
    let v = v as i8;
    if v > 0 {
        (v as u64).checked_mul(bytes_por_cluster)
    } else if v < 0 && v > -32 {
        Some(1u64 << (-(v as i32)))
    } else {
        None
    }
}

/// **Leer el sector de arranque.** `None` si no es NTFS o no se sostiene.
pub fn forma(s: &[u8]) -> Option<Forma> {
    if s.len() < 512 || &s[3..11] != b"NTFS    " || le16(s, 0x1FE) != 0xAA55 {
        return None;
    }
    let bps = le16(s, 0x0B) as u32;
    if !(512..=4096).contains(&bps) || !bps.is_power_of_two() {
        return None;
    }
    let spc = match s[0x0D] {
        0 => return None,
        v if v > 0x80 => 1u64.checked_shl(256 - v as u32)?,
        v => v as u64,
    };
    let bpc = spc * bps as u64;
    if bpc > 2 * 1024 * 1024 {
        return None;
    }
    let registro = medida_de(s[0x40], bpc)?;
    let indice = medida_de(s[0x44], bpc)?;
    if registro < 512 || indice < 512 || !registro.is_power_of_two() || !indice.is_power_of_two() {
        return None;
    }
    Some(Forma { bytes_por_sector: bps, bytes_por_cluster: bpc, sectores: le64(s, 0x28), mft_lcn: le64(s, 0x30), registro: registro as u32, indice: indice as u32 })
}

// == Los arreglos (update sequence) ===========================================

/// **Deshacer los arreglos** de un registro "FILE" o un bloque "INDX": el
/// ultimo `u16` de cada tramo de 512 bytes lleva el numero de secuencia (tiene
/// que cuadrar: si no, el registro se escribio a medias) y el valor de verdad
/// esta en la tabla. `magia` = "FILE" o "INDX".
pub fn arreglar(b: &mut [u8], magia: &[u8; 4]) -> R<()> {
    if b.len() < 8 || &b[..4] != magia {
        return Err(NoNtfs::Forma("un registro sin su firma (FILE / INDX)"));
    }
    let (ofs, n) = (le16(b, 4) as usize, le16(b, 6) as usize);
    if n < 1 || n - 1 != b.len() / PASO_ARREGLO || ofs + 2 * n > b.len() {
        return Err(NoNtfs::Forma("la tabla de arreglos no cuadra con la medida"));
    }
    let usn = [b[ofs], b[ofs + 1]];
    for i in 1..n {
        let fin = i * PASO_ARREGLO - 2;
        if b[fin..fin + 2] != usn {
            return Err(NoNtfs::Forma("un tramo del registro no lleva su numero de secuencia (escrito a medias)"));
        }
        b[fin] = b[ofs + 2 * i];
        b[fin + 1] = b[ofs + 2 * i + 1];
    }
    Ok(())
}

// == Los atributos de un registro =============================================

/// **Un atributo**, visto dentro de su registro (`desde` su cabecera).
#[derive(Debug, Clone, Copy)]
struct Atributo {
    tipo: u32,
    desde: usize,
    largo: usize,
    residente: bool,
    banderas: u16,
    /// Sin nombre (el `$DATA` de un fichero; `$I30` lleva nombre).
    sin_nombre: bool,
}

/// Los atributos de un registro ya arreglado, en orden.
fn atributos(reg: &[u8]) -> impl Iterator<Item = R<Atributo>> + '_ {
    let mut o = if reg.len() >= 0x18 { le16(reg, 0x14) as usize } else { usize::MAX };
    let uso = if reg.len() >= 0x1C { (le32(reg, 0x18) as usize).min(reg.len()) } else { 0 };
    let mut acabado = false;
    core::iter::from_fn(move || {
        if acabado {
            return None;
        }
        if o + 8 > uso {
            acabado = true;
            return Some(Err(NoNtfs::Forma("los atributos se salen del registro")));
        }
        let tipo = le32(reg, o);
        if tipo == AT_FIN {
            acabado = true;
            return None;
        }
        let largo = le32(reg, o + 4) as usize;
        if largo < 0x18 || o + largo > uso {
            acabado = true;
            return Some(Err(NoNtfs::Forma("un atributo con una medida que no cabe")));
        }
        let a = Atributo { tipo, desde: o, largo, residente: reg[o + 8] == 0, banderas: le16(reg, o + 0xC), sin_nombre: reg[o + 9] == 0 };
        if !a.residente && largo < 0x40 {
            acabado = true;
            return Some(Err(NoNtfs::Forma("un atributo no residente sin su cabecera entera")));
        }
        o += largo;
        Some(Ok(a))
    })
}

impl Atributo {
    /// El valor de un residente.
    fn valor<'r>(&self, reg: &'r [u8]) -> R<&'r [u8]> {
        let (n, o) = (le32(reg, self.desde + 0x10) as usize, le16(reg, self.desde + 0x14) as usize);
        if !self.residente || o + n > self.largo {
            return Err(NoNtfs::Forma("un valor residente que se sale de su atributo"));
        }
        Ok(&reg[self.desde + o..self.desde + o + n])
    }
    /// El nombre (UTF-16) del atributo.
    fn nombre<'r>(&self, reg: &'r [u8]) -> &'r [u8] {
        let (n, o) = (reg[self.desde + 9] as usize, le16(reg, self.desde + 0xA) as usize);
        reg.get(self.desde + o..self.desde + o + 2 * n).unwrap_or(&[])
    }
    fn vcn_inicial(&self, reg: &[u8]) -> u64 {
        le64(reg, self.desde + 0x10)
    }
    fn vcn_final(&self, reg: &[u8]) -> u64 {
        le64(reg, self.desde + 0x18)
    }
    /// Los bytes de verdad del valor (no residente).
    fn medida(&self, reg: &[u8]) -> u64 {
        le64(reg, self.desde + 0x30)
    }
    /// Los tramos (no residente).
    fn tramos<'r>(&self, reg: &'r [u8]) -> R<&'r [u8]> {
        let o = le16(reg, self.desde + 0x20) as usize;
        if self.residente || o >= self.largo {
            return Err(NoNtfs::Forma("una lista de tramos fuera de su atributo"));
        }
        Ok(&reg[self.desde + o..self.desde + self.largo])
    }
}

/// `$I30` en UTF-16.
const I30: [u8; 8] = [b'$', 0, b'I', 0, b'3', 0, b'0', 0];

// == Los tramos (runlist) =====================================================

/// **Donde esta el cluster virtual `vcn`** segun una lista de tramos que
/// empieza en `vcn0`: `Some((Some(lcn), quedan))` en el disco, `Some((None,
/// quedan))` un HUECO (sparse: se lee como ceros), `None` si la lista no lo
/// cubre. `quedan` = clusteres seguidos desde ahi.
pub fn mapear(tramos: &[u8], vcn0: u64, vcn: u64) -> R<Option<(Option<u64>, u64)>> {
    let (mut i, mut v, mut lcn) = (0usize, vcn0, 0i64);
    while i < tramos.len() && tramos[i] != 0 {
        let h = tramos[i];
        let (nl, no) = ((h & 0xF) as usize, (h >> 4) as usize);
        if nl == 0 || nl > 8 || no > 8 || i + 1 + nl + no > tramos.len() {
            return Err(NoNtfs::Forma("un tramo que no se sostiene"));
        }
        let mut largo = 0u64;
        for k in 0..nl {
            largo |= (tramos[i + 1 + k] as u64) << (8 * k);
        }
        let hueco = no == 0;
        if !hueco {
            let mut d = 0i64;
            for k in 0..no {
                d |= (tramos[i + 1 + nl + k] as i64) << (8 * k);
            }
            // El signo del ultimo byte.
            if no < 8 && tramos[i + nl + no] & 0x80 != 0 {
                d |= -1i64 << (8 * no);
            }
            lcn = lcn.checked_add(d).ok_or(NoNtfs::Forma("un tramo que desborda"))?;
            if lcn < 0 {
                return Err(NoNtfs::Forma("un tramo antes del principio del volumen"));
            }
        }
        if largo == 0 {
            return Err(NoNtfs::Forma("un tramo de largo cero"));
        }
        let fin = v.checked_add(largo).ok_or(NoNtfs::Forma("un tramo que desborda"))?;
        if vcn >= v && vcn < fin {
            let dentro = vcn - v;
            return Ok(Some((if hueco { None } else { Some(lcn as u64 + dentro) }, fin - vcn)));
        }
        v = fin;
        i += 1 + nl + no;
    }
    Ok(None)
}

// == Las entradas de un indice ===============================================

/// Espacios de nombre de `$FILE_NAME`: el 2 es el alias DOS (8.3).
const DOS: u8 = 2;
/// En las banderas de `$FILE_NAME`: tiene indice (es carpeta).
const ES_CARPETA: u32 = 0x1000_0000;

/// Las entradas de un nodo de indice (`nodo` empieza en su cabecera: el
/// desplazamiento de la primera entrada es relativo a ella). `Ok(true)` si
/// `f` paro.
fn entradas(nodo: &[u8], f: &mut dyn FnMut(&Entrada) -> bool) -> R<bool> {
    if nodo.len() < 16 {
        return Err(NoNtfs::Forma("un nodo de indice corto"));
    }
    let (primero, uso) = (le32(nodo, 0) as usize, (le32(nodo, 4) as usize).min(nodo.len()));
    let mut e = primero;
    loop {
        if e + 16 > uso {
            return Err(NoNtfs::Forma("una entrada de indice que se sale del nodo"));
        }
        let (largo, clave, banderas) = (le16(nodo, e + 8) as usize, le16(nodo, e + 10) as usize, le16(nodo, e + 12));
        if banderas & 2 != 0 {
            // La ultima: sin clave.
            return Ok(false);
        }
        if largo < 16 || e + largo > uso || clave < 0x42 || 16 + clave > largo {
            return Err(NoNtfs::Forma("una entrada de indice que no se sostiene"));
        }
        let k = &nodo[e + 16..e + 16 + clave];
        let (n, espacio) = (k[0x40] as usize, k[0x41]);
        if 0x42 + 2 * n > clave {
            return Err(NoNtfs::Forma("un nombre que se sale de su clave"));
        }
        if espacio != DOS {
            let ent = Entrada { registro: le64(nodo, e) & 0xFFFF_FFFF_FFFF, nombre16: &k[0x42..0x42 + 2 * n], carpeta: le32(k, 0x38) & ES_CARPETA != 0, medida: le64(k, 0x30) };
            if f(&ent) {
                return Ok(true);
            }
        }
        e += largo;
    }
}

// == El volumen ===============================================================

/// **Un fichero o carpeta** del volumen: su registro y lo que mide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nodo {
    pub registro: u64,
    pub carpeta: bool,
    /// Los bytes del `$DATA` (0 en una carpeta).
    pub medida: u64,
}

/// Una entrada de una carpeta, para listar.
pub struct Entrada<'a> {
    pub registro: u64,
    /// El nombre tal cual, en UTF-16LE (ver `nombre_utf8`).
    pub nombre16: &'a [u8],
    pub carpeta: bool,
    /// La medida que apunta el INDICE (la de Windows al listar; la de
    /// verdad la da `abrir`).
    pub medida: u64,
}

impl Entrada<'_> {
    /// El nombre en UTF-8 dentro de `out`; devuelve cuanto ocupa (se corta en
    /// un caracter entero si no cabe).
    pub fn nombre_utf8(&self, out: &mut [u8]) -> usize {
        utf16_a_utf8(self.nombre16, out)
    }
}

/// UTF-16LE a UTF-8 (un sustituto U+FFFD por cada mitad suelta).
pub fn utf16_a_utf8(b: &[u8], out: &mut [u8]) -> usize {
    let unidades = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut n = 0;
    for c in core::char::decode_utf16(unidades) {
        let c = c.unwrap_or('\u{FFFD}');
        let mut t = [0u8; 4];
        let s = c.encode_utf8(&mut t);
        if n + s.len() > out.len() {
            break;
        }
        out[n..n + s.len()].copy_from_slice(s.as_bytes());
        n += s.len();
    }
    n
}

/// Una unidad UTF-16 en mayusculas (ASCII y Latin-1: lo que llevan los
/// nombres de un juego). NTFS compara con su tabla `$UpCase` entera; para lo
/// que no es Latin-1 aqui se compara tal cual.
fn mayuscula(u: u16) -> u16 {
    match u {
        0x61..=0x7A => u - 0x20,
        0xE0..=0xFE if u != 0xF7 => u - 0x20,
        _ => u,
    }
}

/// Si el nombre UTF-16 `n16` es `nombre` (UTF-8), sin mayusculas.
fn mismo_nombre(n16: &[u8], nombre: &str) -> bool {
    let mut a = n16.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut b = nombre.encode_utf16();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) if mayuscula(x) == mayuscula(y) => {}
            _ => return false,
        }
    }
}

/// El disco, la particion y el bloque: lo que hace falta para leer.
#[derive(Clone, Copy)]
struct Disco<'d> {
    dev: &'d dyn BlockDevice,
    /// Donde empieza la particion, en bloques del dispositivo.
    part_lba: u64,
    /// Bytes por bloque del dispositivo.
    bloque: u64,
}

/// Lee `dst.len()` bytes desde el byte `off` del VOLUMEN (`puente`: un
/// bloque para lo que no va alineado).
fn leer_disco(d: &Disco, puente: &mut [u8; MAX_REGISTRO], off: u64, dst: &mut [u8]) -> R<()> {
    let b = d.bloque;
    let (mut off, mut hecho) = (off, 0usize);
    while hecho < dst.len() {
        let lba = d.part_lba + off / b;
        let dentro = (off % b) as usize;
        let quedan = dst.len() - hecho;
        if dentro == 0 && quedan >= b as usize {
            // Alineado: directo al destino, hasta 128 bloques de una vez.
            let n = (quedan / b as usize).min(128);
            match d.dev.read(lba, n as u16, &mut dst[hecho..hecho + n * b as usize]) {
                Ok(k) if k as usize == n => {}
                _ => return Err(NoNtfs::Leer),
            }
            hecho += n * b as usize;
            off += n as u64 * b;
        } else {
            let p = &mut puente[..b as usize];
            match d.dev.read(lba, 1, p) {
                Ok(1) => {}
                _ => return Err(NoNtfs::Leer),
            }
            let n = (b as usize - dentro).min(quedan);
            dst[hecho..hecho + n].copy_from_slice(&p[dentro..dentro + n]);
            hecho += n;
            off += n as u64;
        }
    }
    Ok(())
}

/// Lee `dst` desde el byte `off` (del valor entero) de lo que describe una
/// lista de tramos que empieza en `vcn0`; los huecos, a cero.
fn leer_tramos(d: &Disco, puente: &mut [u8; MAX_REGISTRO], bpc: u64, tramos: &[u8], vcn0: u64, off: u64, dst: &mut [u8]) -> R<()> {
    let mut hecho = 0usize;
    while hecho < dst.len() {
        let p = off + hecho as u64;
        let (vcn, dentro) = (p / bpc, p % bpc);
        let Some((lcn, quedan)) = mapear(tramos, vcn0, vcn)? else { return Err(NoNtfs::Forma("un tramo que no cubre lo que se pide")) };
        let n = ((quedan * bpc - dentro).min((dst.len() - hecho) as u64)) as usize;
        match lcn {
            Some(l) => leer_disco(d, puente, l * bpc + dentro, &mut dst[hecho..hecho + n])?,
            None => dst[hecho..hecho + n].fill(0),
        }
        hecho += n;
    }
    Ok(())
}

/// **Un volumen NTFS montado, para LEER.**
///
/// Todos sus buferes van DENTRO (unos 28 KiB): nada grande en la pila, que
/// en el kernel va justa (`toolchain/tools/pila`). Para el kernel: un
/// `static` hecho con [`Volumen::vacio`] y montado en su sitio con
/// [`Volumen::montar_aqui`].
pub struct Volumen<'d> {
    d: Disco<'d>,
    pub forma: Forma,
    /// El registro 0 ($MFT), arreglado: sus tramos dicen donde esta cada
    /// registro.
    mft: [u8; MAX_REGISTRO],
    /// El registro que se esta mirando.
    reg: [u8; MAX_REGISTRO],
    /// Un segundo registro, para las extensiones de `$ATTRIBUTE_LIST`.
    ext: [u8; MAX_REGISTRO],
    /// La `$ATTRIBUTE_LIST` del registro que se mira.
    lista: [u8; MAX_INDICE],
    /// Un bloque INDX.
    blq: [u8; MAX_INDICE],
    /// El mapa de bloques de indice en uso.
    bits: [u8; 512],
    puente: [u8; MAX_REGISTRO],
    montado: bool,
}

impl<'d> Volumen<'d> {
    /// Un volumen SIN montar sobre `dev` (para un `static`).
    pub const fn vacio(dev: &'d dyn BlockDevice) -> Self {
        Volumen {
            d: Disco { dev, part_lba: 0, bloque: 512 },
            forma: Forma { bytes_por_sector: 512, bytes_por_cluster: 4096, sectores: 0, mft_lcn: 0, registro: 1024, indice: 4096 },
            mft: [0; MAX_REGISTRO],
            reg: [0; MAX_REGISTRO],
            ext: [0; MAX_REGISTRO],
            lista: [0; MAX_INDICE],
            blq: [0; MAX_INDICE],
            bits: [0; 512],
            puente: [0; MAX_REGISTRO],
            montado: false,
        }
    }

    /// **Montar**: el sector de arranque en `part_lba` y el registro 0.
    pub fn montar(dev: &'d dyn BlockDevice, part_lba: u64) -> R<Self> {
        let mut v = Self::vacio(dev);
        v.montar_aqui(part_lba)?;
        Ok(v)
    }

    /// **Montar EN SU SITIO** (sin mover los 28 KiB por la pila).
    pub fn montar_aqui(&mut self, part_lba: u64) -> R<()> {
        self.montado = false;
        let bloque = self.d.dev.block_size() as u64;
        if !(512..=MAX_REGISTRO as u64).contains(&bloque) || !bloque.is_power_of_two() {
            return Err(NoNtfs::Grande("bloques del dispositivo de mas de 4 KiB"));
        }
        self.d.part_lba = part_lba;
        self.d.bloque = bloque;
        let Volumen { d, puente, mft, .. } = self;
        leer_disco(d, puente, 0, &mut mft[..512])?;
        let f = forma(&mft[..512]).ok_or(NoNtfs::NoEsNtfs)?;
        if f.registro as usize > MAX_REGISTRO {
            return Err(NoNtfs::Grande("registros del MFT de mas de 4 KiB"));
        }
        if f.indice as usize > MAX_INDICE {
            return Err(NoNtfs::Grande("bloques de indice de mas de 4 KiB"));
        }
        let n = f.registro as usize;
        leer_disco(d, puente, f.mft_lcn * f.bytes_por_cluster, &mut mft[..n])?;
        arreglar(&mut mft[..n], b"FILE")?;
        self.forma = f;
        self.montado = true;
        Ok(())
    }

    /// Si esta montado.
    pub fn montado(&self) -> bool {
        self.montado
    }

    fn rm(&self) -> usize {
        self.forma.registro as usize
    }

    /// **El registro `n` del MFT**, arreglado, en `self.reg` (o en `self.ext`
    /// con `en_ext`).
    fn registro(&mut self, n: u64, en_ext: bool) -> R<()> {
        if !self.montado {
            return Err(NoNtfs::NoEsNtfs);
        }
        let rm = self.forma.registro as u64;
        let bpc = self.forma.bytes_por_cluster;
        let byte = n.checked_mul(rm).ok_or(NoNtfs::Forma("un registro fuera del MFT"))?;
        let Volumen { d, mft, reg, ext, puente, .. } = self;
        let destino = if en_ext { &mut ext[..rm as usize] } else { &mut reg[..rm as usize] };
        let mft = &mft[..rm as usize];
        let mut hecho = 0u64;
        while hecho < rm {
            let vcn = (byte + hecho) / bpc;
            let dentro = (byte + hecho) % bpc;
            // El $DATA del propio MFT (sin $ATTRIBUTE_LIST: ver la cabecera).
            let mut sitio = None;
            for a in atributos(mft) {
                let a = a?;
                if a.tipo == AT_DATOS && a.sin_nombre && !a.residente && vcn >= a.vcn_inicial(mft) && vcn <= a.vcn_final(mft) {
                    sitio = mapear(a.tramos(mft)?, a.vcn_inicial(mft), vcn)?;
                    break;
                }
            }
            let Some((Some(lcn), quedan)) = sitio else { return Err(NoNtfs::Forma("un registro que el $DATA del MFT no cubre")) };
            let k = (quedan * bpc - dentro).min(rm - hecho);
            leer_disco(d, puente, lcn * bpc + dentro, &mut destino[hecho as usize..(hecho + k) as usize])?;
            hecho += k;
        }
        arreglar(destino, b"FILE")?;
        if le16(destino, 0x16) & 1 == 0 {
            return Err(NoNtfs::NoEsta);
        }
        Ok(())
    }

    /// **Abrir por su registro**: si es carpeta y lo que mide su `$DATA`.
    pub fn nodo(&mut self, n: u64) -> R<Nodo> {
        self.registro(n, false)?;
        let carpeta = le16(&self.reg, 0x16) & 2 != 0;
        let medida = if carpeta { 0 } else { self.datos_base()?.1 };
        Ok(Nodo { registro: n, carpeta, medida })
    }

    /// El `$DATA` sin nombre de VCN 0 del registro en `self.reg` (el que
    /// lleva la medida): `(banderas, medida)`, en el o donde diga la lista.
    fn datos_base(&mut self) -> R<(u16, u64)> {
        let rm = self.rm();
        let mut lista = false;
        {
            let reg = &self.reg[..rm];
            for a in atributos(reg) {
                let a = a?;
                if a.tipo == AT_DATOS && a.sin_nombre {
                    if a.residente {
                        return Ok((a.banderas, a.valor(reg)?.len() as u64));
                    }
                    if a.vcn_inicial(reg) == 0 {
                        return Ok((a.banderas, a.medida(reg)));
                    }
                }
                lista |= a.tipo == AT_LISTA;
            }
        }
        if lista {
            if let Some(r) = self.extension(0)? {
                self.registro(r, true)?;
                let ext = &self.ext[..rm];
                for a in atributos(ext) {
                    let a = a?;
                    if a.tipo == AT_DATOS && a.sin_nombre && !a.residente && a.vcn_inicial(ext) == 0 {
                        return Ok((a.banderas, a.medida(ext)));
                    }
                }
            }
        }
        Err(NoNtfs::Forma("un fichero sin $DATA"))
    }

    /// **`$ATTRIBUTE_LIST`** del registro en `self.reg`: el registro que tiene
    /// el `$DATA` sin nombre que cubre `vcn` (el de mayor VCN inicial que no
    /// lo pasa). `None` sin lista.
    fn extension(&mut self, vcn: u64) -> R<Option<u64>> {
        let rm = self.rm();
        let bpc = self.forma.bytes_por_cluster;
        let Volumen { d, reg, lista, puente, .. } = self;
        let reg = &reg[..rm];
        let mut hallada = None;
        for a in atributos(reg) {
            let a = a?;
            if a.tipo == AT_LISTA {
                hallada = Some(a);
                break;
            }
        }
        let Some(a) = hallada else { return Ok(None) };
        let largo = if a.residente {
            let v = a.valor(reg)?;
            if v.len() > MAX_INDICE {
                return Err(NoNtfs::Grande("una $ATTRIBUTE_LIST de mas de 4 KiB"));
            }
            lista[..v.len()].copy_from_slice(v);
            v.len()
        } else {
            let m = a.medida(reg);
            if m > MAX_INDICE as u64 {
                return Err(NoNtfs::Grande("una $ATTRIBUTE_LIST de mas de 4 KiB"));
            }
            leer_tramos(d, puente, bpc, a.tramos(reg)?, a.vcn_inicial(reg), 0, &mut lista[..m as usize])?;
            m as usize
        };
        let (mut o, mut mejor) = (0usize, None);
        while o + 0x1A <= largo {
            let l = le16(lista, o + 4) as usize;
            if l < 0x1A || o + l > largo {
                return Err(NoNtfs::Forma("una entrada de $ATTRIBUTE_LIST que no cabe"));
            }
            let (tipo, nombre, inicio, r) = (le32(lista, o), lista[o + 6], le64(lista, o + 8), le64(lista, o + 0x10) & 0xFFFF_FFFF_FFFF);
            if tipo == AT_DATOS && nombre == 0 && inicio <= vcn && mejor.is_none_or(|(i, _)| inicio >= i) {
                mejor = Some((inicio, r));
            }
            o += l;
        }
        Ok(mejor.map(|(_, r)| r))
    }

    /// **Leer un fichero**: `dst` desde el byte `off`. Devuelve cuantos se
    /// leyeron (menos si se acaba). Comprimido o cifrado: lo dice.
    pub fn leer(&mut self, n: &Nodo, off: u64, dst: &mut [u8]) -> R<usize> {
        if n.carpeta {
            return Err(NoNtfs::NoEsFichero);
        }
        self.registro(n.registro, false)?;
        let (banderas, medida) = self.datos_base()?;
        if banderas & COMPRIMIDO != 0 {
            return Err(NoNtfs::Comprimido);
        }
        if banderas & CIFRADO != 0 {
            return Err(NoNtfs::Cifrado);
        }
        if off >= medida {
            return Ok(0);
        }
        let total = ((medida - off).min(dst.len() as u64)) as usize;
        let bpc = self.forma.bytes_por_cluster;
        let mut hecho = 0usize;
        while hecho < total {
            let p = off + hecho as u64;
            let vcn = p / bpc;
            // El $DATA que cubre `vcn`: en el registro base, o en la
            // extension que diga la lista (`self.reg` sigue siendo el base).
            let mut k = self.tramo_de(false, vcn, p, &mut dst[hecho..total])?;
            if k.is_none() {
                if let Some(r) = self.extension(vcn)? {
                    self.registro(r, true)?;
                    k = self.tramo_de(true, vcn, p, &mut dst[hecho..total])?;
                }
            }
            match k {
                Some(0) | None => return Err(NoNtfs::Forma("un trozo del fichero que ningun $DATA cubre")),
                Some(k) => hecho += k,
            }
        }
        Ok(total)
    }

    /// En el registro base (o la extension): si un `$DATA` sin nombre cubre
    /// `vcn`, lee de el (hasta el final de ese atributo) y dice cuantos bytes.
    fn tramo_de(&mut self, en_ext: bool, vcn: u64, p: u64, dst: &mut [u8]) -> R<Option<usize>> {
        let rm = self.rm();
        let bpc = self.forma.bytes_por_cluster;
        let Volumen { d, reg, ext, puente, .. } = self;
        let r = if en_ext { &ext[..rm] } else { &reg[..rm] };
        for a in atributos(r) {
            let a = a?;
            if a.tipo != AT_DATOS || !a.sin_nombre {
                continue;
            }
            if a.residente {
                let v = a.valor(r)?;
                let desde = p as usize;
                let n = v.len().saturating_sub(desde).min(dst.len());
                dst[..n].copy_from_slice(&v[desde..desde + n]);
                return Ok(Some(n));
            }
            let (i, f) = (a.vcn_inicial(r), a.vcn_final(r));
            if vcn < i || vcn > f {
                continue;
            }
            let hasta = (f + 1) * bpc;
            let n = ((hasta - p).min(dst.len() as u64)) as usize;
            leer_tramos(d, puente, bpc, a.tramos(r)?, i, p, &mut dst[..n])?;
            return Ok(Some(n));
        }
        Ok(None)
    }

    // == El espacio ==============================================================

    /// **Los clusteres del volumen**: los sectores del arranque en clusteres.
    pub fn clusteres(&self) -> u64 {
        self.forma.sectores * self.forma.bytes_por_sector as u64 / self.forma.bytes_por_cluster
    }

    /// **Los clusteres LIBRES**, contados en `$Bitmap` (el registro 6): un bit
    /// por cluster, 1 = en uso. Solo lee: de 4 KiB en 4 KiB por `blq`, sin
    /// nada en la pila. Los bits de despues del ultimo cluster no cuentan (el
    /// mapa se redondea a 8 bytes y ahi NTFS pone lo que quiere).
    pub fn libres(&mut self) -> R<u64> {
        self.registro(BITMAP, false)?;
        let total = self.clusteres();
        let bytes = total.div_ceil(8);
        let rm = self.rm();
        let bpc = self.forma.bytes_por_cluster;
        let Volumen { d, reg, blq, puente, .. } = self;
        let reg = &reg[..rm];
        let mut dato = None;
        for a in atributos(reg) {
            let a = a?;
            if a.tipo == AT_DATOS && a.sin_nombre {
                dato = Some(a);
                break;
            }
        }
        let Some(a) = dato else { return Err(NoNtfs::Forma("un $Bitmap sin $DATA en su registro")) };
        if !a.residente && a.vcn_inicial(reg) != 0 {
            return Err(NoNtfs::Forma("un $Bitmap repartido en extensiones"));
        }
        let medida = if a.residente { a.valor(reg)?.len() as u64 } else { a.medida(reg) };
        if medida < bytes {
            return Err(NoNtfs::Forma("un $Bitmap mas corto que el volumen"));
        }
        let (mut off, mut usados) = (0u64, 0u64);
        while off < bytes {
            let n = ((bytes - off) as usize).min(MAX_INDICE);
            let b = &mut blq[..n];
            if a.residente {
                b.copy_from_slice(&a.valor(reg)?[off as usize..off as usize + n]);
            } else {
                leer_tramos(d, puente, bpc, a.tramos(reg)?, 0, off, b)?;
            }
            // El ultimo byte puede tener bits de mas: fuera.
            let fin = off + n as u64 == bytes && total % 8 != 0;
            if fin {
                b[n - 1] &= (1u8 << (total % 8)) - 1;
            }
            usados += b.iter().map(|x| x.count_ones() as u64).sum::<u64>();
            off += n as u64;
        }
        Ok(total - usados)
    }

    // == Las carpetas ==========================================================

    /// **Recorrer las entradas VIVAS de la carpeta `dir`**: las de
    /// `$INDEX_ROOT` y las de los bloques "INDX" que su `$BITMAP` marca en
    /// uso (un bloque libre puede guardar entradas viejas: no se miran). Los
    /// nombres DOS (8.3) se saltan. `f` devuelve `true` para parar; `Ok(true)`
    /// si paro.
    pub fn recorrer(&mut self, dir: u64, f: &mut dyn FnMut(&Entrada) -> bool) -> R<bool> {
        self.registro(dir, false)?;
        if le16(&self.reg, 0x16) & 2 == 0 {
            return Err(NoNtfs::NoEsCarpeta);
        }
        let rm = self.rm();
        let bpc = self.forma.bytes_por_cluster;
        let ib = self.forma.indice as usize;
        let Volumen { d, reg, blq, bits, puente, .. } = self;
        let base = &reg[..rm];
        let (mut raiz, mut asig, mut mapa) = (None, None, None);
        for a in atributos(base) {
            let a = a?;
            if a.nombre(base) != I30 {
                continue;
            }
            match a.tipo {
                AT_RAIZ_INDICE => raiz = Some(a),
                AT_ASIGNACION_INDICE => asig = Some(a),
                AT_MAPA => mapa = Some(a),
                _ => {}
            }
        }
        let raiz = raiz.ok_or(NoNtfs::Forma("una carpeta sin $INDEX_ROOT"))?;
        let v = raiz.valor(base)?;
        if v.len() < 0x20 {
            return Err(NoNtfs::Forma("un $INDEX_ROOT corto"));
        }
        if entradas(&v[0x10..], f)? {
            return Ok(true);
        }
        let Some(asig) = asig else { return Ok(false) };
        // El mapa de bloques en uso.
        let nbits = match mapa {
            Some(m) if m.residente => {
                let mv = m.valor(base)?;
                let n = mv.len().min(bits.len());
                bits[..n].copy_from_slice(&mv[..n]);
                n
            }
            Some(m) => {
                let n = (m.medida(base) as usize).min(bits.len());
                leer_tramos(d, puente, bpc, m.tramos(base)?, m.vcn_inicial(base), 0, &mut bits[..n])?;
                n
            }
            None => return Err(NoNtfs::Forma("una carpeta con $INDEX_ALLOCATION y sin $BITMAP")),
        };
        let bloques = (asig.medida(base) / ib as u64).min(8 * nbits as u64);
        for k in 0..bloques {
            if bits[(k / 8) as usize] >> (k % 8) & 1 == 0 {
                continue;
            }
            leer_tramos(d, puente, bpc, asig.tramos(base)?, asig.vcn_inicial(base), k * ib as u64, &mut blq[..ib])?;
            arreglar(&mut blq[..ib], b"INDX")?;
            if entradas(&blq[0x18..ib], f)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// **Buscar `nombre` en la carpeta `dir`** (sin mayusculas).
    pub fn en_carpeta(&mut self, dir: u64, nombre: &str) -> R<Nodo> {
        let mut hallado = None;
        self.recorrer(dir, &mut |e| {
            if mismo_nombre(e.nombre16, nombre) {
                hallado = Some(e.registro);
                true
            } else {
                false
            }
        })?;
        self.nodo(hallado.ok_or(NoNtfs::NoEsta)?)
    }

    /// **Abrir por su ruta** desde la raiz: `/` o `\` separan, sin
    /// mayusculas (`bin/x64/Cyberpunk2077.exe`).
    pub fn abrir(&mut self, ruta: &str) -> R<Nodo> {
        let mut n = self.nodo(RAIZ)?;
        for parte in ruta.split(['/', '\\']).filter(|p| !p.is_empty()) {
            if !n.carpeta {
                return Err(NoNtfs::NoEsCarpeta);
            }
            n = self.en_carpeta(n.registro, parte)?;
        }
        Ok(n)
    }
}
