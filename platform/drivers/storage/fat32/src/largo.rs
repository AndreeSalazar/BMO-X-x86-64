//! **NOMBRES LARGOS, CARPETAS Y AÑADIR AL FINAL** (12-10).
//!
//! El propietario: *"¿puedes hacer que mi kernel escriba sin final? pero que
//! sepa escribir y bueno también organizar carpetas"*, y la carpeta `datos`
//! pasa a llamarse `datos-componentes`. Hasta hoy este driver:
//!
//! ```text
//!    nombres      solo 8.3: `datos-componentes` (17 letras) no se podia ni
//!                 buscar ni crear; el kernel se negaba a recortar, con razon
//!    carpetas     ninguna: `informe/` la creaba el build en Windows
//!    directorio   con todas sus entradas ocupadas: `DirFull`, y se acabo
//!    escribir     un fichero ENTERO de una vez: lo escrito vivia en la RAM
//!                 (contigua) hasta cerrar, y no se podia seguir uno que ya
//!                 estaba en el disco
//! ```
//!
//! Aqui esta lo que faltaba, con las mismas reglas que `escribir.rs`:
//!
//! * **Nombres largos (VFAT)**, como los escribe Windows: las entradas de
//!   nombre largo (atributo `0x0F`, 13 letras cada una, en orden inverso) y,
//!   detras, la entrada 8.3 de siempre con un alias `DATOS-~1` y la suma que
//!   las ata. Un nombre que YA es 8.3 se escribe como hasta hoy, sin entradas
//!   largas: lo de antes no cambia ni un byte.
//! * **Buscar por cualquiera de los dos nombres**, sin distinguir mayusculas
//!   (como FAT): `datos-componentes`, `DATOS-COMPONENTES` y `DATOS-~1`.
//! * **Carpetas**, con su `.` y su `..`, y **directorios que crecen**: sin
//!   hueco, se encadena un cluster nuevo (a ceros) y se sigue.
//! * **Añadir al final** de un fichero que ya esta en el disco: se llena la
//!   cola de su ultimo cluster y lo que sobra va a una cadena nueva que se
//!   engancha detras. Es lo que deja al kernel escribir SIN FINAL: lo escrito
//!   baja al disco a trozos, y la RAM ya no es el techo (solo lo es FAT32:
//!   4 GiB - 1 por fichero, y se dice con [`WriteError::Grande`]).
//!
//! ## El orden, que es lo unico que importa (el de `escribir.rs`)
//!
//! * Crear: los datos primero, las entradas largas despues y **la entrada 8.3
//!   la ULTIMA**. Un corte antes deja como mucho entradas largas huerfanas,
//!   que la suma delata y todos (Windows incluido) ignoran.
//! * Añadir: los bytes nuevos de la cola del ultimo cluster estan por DETRAS
//!   de la medida apuntada, asi que no se ven; la cadena nueva se escribe
//!   entera antes de engancharla; y **la medida se apunta la ULTIMA**, en un
//!   solo sector. Un corte en cualquier punto deja el fichero de antes entero
//!   (y como mucho unos clusters perdidos, la fuga recuperable de siempre).
//!
//! Solo ASCII en los nombres largos: el kernel no tiene tablas de mayusculas
//! de UTF-16, y comparar sin ellas seria decir que dos nombres son distintos
//! cuando Windows dice que son el mismo.

use super::*;

/// Lo mas largo que puede ser un nombre en FAT.
pub const NOMBRE_MAX: usize = 255;

/// **Una entrada encontrada por su nombre**: donde vive y si es carpeta.
#[derive(Clone, Copy, Debug)]
pub struct Hallada {
    pub entrada: EntradaDir,
    pub es_dir: bool,
}

const LIBRE: u8 = 0xE5;
const ATTR_LARGO: u8 = 0x0F;
const ATTR_DIR: u8 = 0x10;
const ATTR_ARCHIVO: u8 = 0x20;
/// Letras de un nombre largo por entrada, y donde va cada una (UTF-16).
const LETRAS: usize = 13;
const POS: [usize; LETRAS] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
/// 20 entradas x 13 = 260: cabe cualquier nombre de 255.
const LARGO_MAX: usize = 20 * LETRAS;

/// La suma con la que una entrada 8.3 reconoce a sus entradas largas.
fn suma(corto: &[u8; 11]) -> u8 {
    corto.iter().fold(0u8, |s, &c| s.rotate_right(1).wrapping_add(c))
}

fn letra_corta(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || b"!#$%&'()-@^_`{}~".contains(&c)
}

/// Un nombre que FAT puede guardar: 1..=255 letras ASCII, sin las prohibidas,
/// y sin acabar en punto o espacio (Windows los quitaria y serian OTRO nombre).
fn nombre_valido(n: &[u8]) -> bool {
    !n.is_empty()
        && n.len() <= NOMBRE_MAX
        && n != b"."
        && n != b".."
        && n.iter().all(|&c| (0x20..0x7F).contains(&c) && !b"\\/:*?\"<>|".contains(&c))
        && !matches!(n.last(), Some(b'.') | Some(b' '))
}

/// **¿FAT puede guardar este nombre?** (largo o 8.3). Para validar ANTES de
/// aceptar bytes que luego no se podrian guardar.
pub fn nombre_escribible(n: &[u8]) -> bool {
    nombre_valido(n)
}

/// **El 8.3 EXACTO de `n`**, si lo tiene: sin recortar ni inventar. Las
/// minusculas valen (FAT las guarda en mayuscula, como hasta hoy).
pub(crate) fn corto_exacto(n: &[u8]) -> Option<[u8; 11]> {
    let (base, ext) = match n.iter().rposition(|&c| c == b'.') {
        Some(0) => return None,
        Some(i) => (&n[..i], &n[i + 1..]),
        None => (n, &n[n.len()..]),
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 {
        return None;
    }
    let mut c = [b' '; 11];
    for (k, &b) in base.iter().enumerate() {
        let b = b.to_ascii_uppercase();
        if !letra_corta(b) {
            return None;
        }
        c[k] = b;
    }
    for (k, &b) in ext.iter().enumerate() {
        let b = b.to_ascii_uppercase();
        if !letra_corta(b) {
            return None;
        }
        c[8 + k] = b;
    }
    Some(c)
}

/// La base del alias de un nombre largo: las letras validas en mayuscula (las
/// demas, `_`), sin espacios ni puntos; y la extension, igual. `(base, letras
/// de la base, extension)`.
fn base_de_alias(n: &[u8]) -> ([u8; 8], usize, [u8; 3]) {
    let (tallo, ext) = match n.iter().rposition(|&c| c == b'.') {
        Some(i) if i > 0 => (&n[..i], &n[i + 1..]),
        _ => (n, &n[n.len()..]),
    };
    let limpia = |c: u8| {
        let c = c.to_ascii_uppercase();
        if letra_corta(c) { c } else { b'_' }
    };
    let mut base = [b' '; 8];
    let mut k = 0;
    for &c in tallo.iter().filter(|&&c| c != b' ' && c != b'.') {
        if k == 8 {
            break;
        }
        base[k] = limpia(c);
        k += 1;
    }
    if k == 0 {
        base[0] = b'_';
        k = 1;
    }
    let mut e = [b' '; 3];
    for (j, &c) in ext.iter().filter(|&&c| c != b' ' && c != b'.').take(3).enumerate() {
        e[j] = limpia(c);
    }
    (base, k, e)
}

/// `BASE~N.EXT`: la base se acorta lo justo para que quepa la cola.
fn alias(base: &[u8; 8], letras: usize, ext: &[u8; 3], n: u32) -> [u8; 11] {
    let mut cola = [0u8; 8];
    let mut largo = 0;
    let mut x = n;
    let mut digitos = [0u8; 7];
    let mut d = 0;
    while x > 0 || d == 0 {
        digitos[d] = b'0' + (x % 10) as u8;
        x /= 10;
        d += 1;
    }
    cola[0] = b'~';
    largo += 1;
    for i in (0..d).rev() {
        cola[largo] = digitos[i];
        largo += 1;
    }
    let mut c = [b' '; 11];
    let quedan = letras.min(8 - largo);
    c[..quedan].copy_from_slice(&base[..quedan]);
    c[quedan..quedan + largo].copy_from_slice(&cola[..largo]);
    c[8..].copy_from_slice(ext);
    c
}

/// El 8.3 como se lee: `NOMBRE.EXT` sin el relleno. Devuelve cuantas letras.
fn corto_legible(c: &[u8; 11], out: &mut [u8]) -> usize {
    let mut n = 0;
    for &b in c[..8].iter().take_while(|&&b| b != b' ') {
        if n < out.len() {
            out[n] = b;
            n += 1;
        }
    }
    if c[8] != b' ' {
        if n < out.len() {
            out[n] = b'.';
            n += 1;
        }
        for &b in c[8..].iter().take_while(|&&b| b != b' ') {
            if n < out.len() {
                out[n] = b;
                n += 1;
            }
        }
    }
    n
}

fn igual_corto(c: &[u8; 11], nombre: &[u8]) -> bool {
    let mut l = [0u8; 12];
    let n = corto_legible(c, &mut l);
    l[..n].eq_ignore_ascii_case(nombre)
}

fn igual_largo(largo: &[u16], nombre: &[u8]) -> bool {
    let fin = largo.iter().position(|&u| u == 0 || u == 0xFFFF).unwrap_or(largo.len());
    fin == nombre.len()
        && largo[..fin].iter().zip(nombre).all(|(&u, &b)| u < 0x80 && (u as u8).eq_ignore_ascii_case(&b))
}

/// Lo que ve el recorrido en cada entrada que no es larga ni borrada.
struct Visto<'a> {
    e: [u8; 32],
    lba: u64,
    off: usize,
    /// El nombre largo que la precede, si su suma cuadra con este 8.3.
    largo: Option<&'a [u16]>,
}

impl Visto<'_> {
    fn corto(&self) -> [u8; 11] {
        let mut c = [0u8; 11];
        c.copy_from_slice(&self.e[..11]);
        c
    }
    fn attr(&self) -> u8 {
        self.e[11]
    }
    fn cluster(&self) -> u32 {
        (u16::from_le_bytes([self.e[20], self.e[21]]) as u32) << 16 | u16::from_le_bytes([self.e[26], self.e[27]]) as u32
    }
    fn medida(&self) -> u32 {
        u32::from_le_bytes([self.e[28], self.e[29], self.e[30], self.e[31]])
    }
}

/// Como acabo un recorrido.
enum Fin {
    /// `visita` dijo que basta.
    Parado,
    /// Se llego al final del directorio (la entrada 0x00 o la cadena).
    Acabo,
    /// El disco no contesto, o la cadena del directorio esta rota.
    Roto,
}

impl FatVolume {
    /// **La hora de lo que se escriba desde ahora** (la de la placa): el
    /// informe de un arranque dice CUANDO, tambien desde Windows. `None`
    /// borra el sello.
    pub fn poner_reloj(&mut self, cuando: Option<(u16, u8, u8, u8, u8, u8)>) {
        self.sello = match cuando {
            Some((a, m, d, h, mi, s)) if (1980..2108).contains(&a) && (1..=12).contains(&m) && (1..=31).contains(&d) => (
                (a - 1980) << 9 | (m as u16) << 5 | d as u16,
                (h.min(23) as u16) << 11 | (mi.min(59) as u16) << 5 | (s.min(59) / 2) as u16,
            ),
            _ => (0, 0),
        };
    }

    /// El sello en los 32 bytes de una entrada: creada y escrita.
    fn sellar(&self, e: &mut [u8]) {
        let (f, h) = self.sello;
        if f == 0 {
            return;
        }
        e[14..16].copy_from_slice(&h.to_le_bytes());
        e[16..18].copy_from_slice(&f.to_le_bytes());
        e[18..20].copy_from_slice(&f.to_le_bytes());
        e[22..24].copy_from_slice(&h.to_le_bytes());
        e[24..26].copy_from_slice(&f.to_le_bytes());
    }

    /// **Recorre un directorio** entrada a entrada, juntando los nombres
    /// largos. `visita` devuelve `true` para parar.
    fn recorrer(&mut self, dir: u32, mut visita: impl FnMut(&Visto) -> bool) -> Fin {
        let mut largo = [0u16; LARGO_MAX];
        let mut largo_n = 0usize;
        let mut suma_l = 0u8;
        let mut falta = 0u8;
        let mut vivo = false;
        let spc = self.sectors_per_cluster as u64;
        let mut cluster = dir;
        let mut vueltas = 0u32;
        loop {
            let Some(lba) = self.lba_valido(cluster) else { return Fin::Roto };
            for s in 0..spc {
                if !self.read_sector(lba + s, Buf::buf) {
                    return Fin::Roto;
                }
                for i in 0..16 {
                    let mut e = [0u8; 32];
                    e.copy_from_slice(&self.buf[i * 32..i * 32 + 32]);
                    if e[0] == 0 {
                        return Fin::Acabo;
                    }
                    if e[0] == LIBRE {
                        vivo = false;
                        continue;
                    }
                    if e[11] & 0x3F == ATTR_LARGO {
                        let ord = (e[0] & 0x1F) as usize;
                        if e[0] & 0x40 != 0 {
                            if ord == 0 || ord > LARGO_MAX / LETRAS {
                                vivo = false;
                                continue;
                            }
                            largo_n = ord * LETRAS;
                            largo[..largo_n].fill(0xFFFF);
                            suma_l = e[13];
                            falta = ord as u8;
                            vivo = true;
                        }
                        if vivo && ord == falta as usize && ord > 0 && e[13] == suma_l {
                            for (j, &p) in POS.iter().enumerate() {
                                largo[(ord - 1) * LETRAS + j] = u16::from_le_bytes([e[p], e[p + 1]]);
                            }
                            falta -= 1;
                        } else {
                            vivo = false;
                        }
                        continue;
                    }
                    let con_largo = vivo && falta == 0;
                    vivo = false;
                    if e[11] & 0x08 != 0 {
                        continue; // la etiqueta del volumen
                    }
                    let mut corto = [0u8; 11];
                    corto.copy_from_slice(&e[..11]);
                    let v = Visto {
                        e,
                        lba: lba + s,
                        off: i * 32,
                        largo: (con_largo && suma(&corto) == suma_l).then_some(&largo[..largo_n]),
                    };
                    if visita(&v) {
                        return Fin::Parado;
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => return Fin::Acabo,
            };
            // Una cadena con un ciclo no se recorre para siempre.
            vueltas += 1;
            if vueltas > self.max_cluster {
                return Fin::Roto;
            }
        }
    }

    /// **Busca `nombre` en el directorio `dir`**, por su nombre largo o por su
    /// 8.3, sin distinguir mayusculas. Ficheros y carpetas.
    pub fn hallar(&mut self, dir: u32, nombre: &[u8]) -> Option<Hallada> {
        if self.fs_type != FsType::Fat32 || nombre.is_empty() {
            return None;
        }
        let mut hallada = None;
        self.recorrer(dir, |v| {
            let c = v.corto();
            if v.largo.is_some_and(|l| igual_largo(l, nombre)) || igual_corto(&c, nombre) {
                hallada = Some((v.lba, v.off, v.cluster(), v.medida(), v.attr() & ATTR_DIR != 0));
                return true;
            }
            false
        });
        let (lba, offset, first_cluster, size, es_dir) = hallada?;
        // El cluster viene del disco: uno fuera de rango es basura y se corta
        // (el cero es un fichero vacio, o el `..` que apunta a la raiz).
        if first_cluster != 0 && !self.cluster_valido(first_cluster) {
            return None;
        }
        Some(Hallada { entrada: EntradaDir { lba, offset, first_cluster, size }, es_dir })
    }

    /// **La entrada `n` de un directorio, con su nombre largo** si lo tiene
    /// (si no, el 8.3 legible: `NOMBRE.EXT`). `(letras, es_dir, medida)`.
    /// Cuenta como [`FatVolume::entry_at`]: los mismos numeros, los mismos
    /// ficheros, otro nombre.
    pub fn entrada_larga(&mut self, dir: u32, n: usize, out: &mut [u8]) -> Option<(usize, bool, u32)> {
        if self.fs_type != FsType::Fat32 {
            return None;
        }
        let mut k = 0usize;
        let mut r = None;
        self.recorrer(dir, |v| {
            if k < n {
                k += 1;
                return false;
            }
            let largo = match v.largo {
                Some(l) => {
                    let fin = l.iter().position(|&u| u == 0 || u == 0xFFFF).unwrap_or(l.len());
                    let m = fin.min(out.len());
                    for (o, &u) in out.iter_mut().zip(&l[..m]) {
                        *o = if (0x20..0x7F).contains(&u) { u as u8 } else { b'?' };
                    }
                    m
                }
                None => corto_legible(&v.corto(), out),
            };
            r = Some((largo, v.attr() & ATTR_DIR != 0, v.medida()));
            true
        });
        r
    }

    fn se_puede_escribir(&self) -> Result<(), WriteError> {
        if !self.escribible {
            return Err(WriteError::ReadOnly);
        }
        if self.fs_type != FsType::Fat32 {
            return Err(WriteError::Unsupported);
        }
        Ok(())
    }

    /// Donde vive la ranura `k` del directorio `dir`: `(sector, byte)`.
    fn ranura(&mut self, dir: u32, k: usize) -> Option<(u64, usize)> {
        let por_cluster = self.sectors_per_cluster as usize * 16;
        let mut c = dir;
        for _ in 0..k / por_cluster {
            c = self.read_fat_entry(c)?;
        }
        let lba = self.lba_valido(c)?;
        let r = k % por_cluster;
        Some((lba + (r / 16) as u64, (r % 16) * 32))
    }

    /// Escribe los 32 bytes de la ranura `k`, sin pisar a las vecinas.
    fn poner_ranura(&mut self, dir: u32, k: usize, e: &[u8; 32]) -> Result<(u64, usize), WriteError> {
        let (lba, off) = self.ranura(dir, k).ok_or(WriteError::Io)?;
        if !self.read_sector(lba, Buf::buf) {
            return Err(WriteError::Io);
        }
        self.buf[off..off + 32].copy_from_slice(e);
        if !self.write_sector(lba, Buf::buf) {
            return Err(WriteError::Io);
        }
        Ok((lba, off))
    }

    /// **`n` ranuras libres seguidas** en `dir`; si no hay, el directorio
    /// CRECE un cluster (a ceros) y se sigue. Devuelve la primera.
    fn hueco(&mut self, dir: u32, n: usize) -> Result<usize, WriteError> {
        let spc = self.sectors_per_cluster as u64;
        let mut cluster = dir;
        let mut k = 0usize;
        let mut desde = 0usize;
        let mut seguidas = 0usize;
        let mut vueltas = 0u32;
        loop {
            let lba = self.lba_valido(cluster).ok_or(WriteError::Io)?;
            for s in 0..spc {
                if !self.read_sector(lba + s, Buf::buf) {
                    return Err(WriteError::Io);
                }
                for i in 0..16 {
                    let b = self.buf[i * 32];
                    if b == 0 {
                        // Del fin del directorio en adelante, todo esta libre:
                        // si hacen falta mas de las que quedan en ESTE
                        // cluster, las da el que se encadene.
                        if seguidas == 0 {
                            desde = k;
                        }
                        let quedan_aqui = (spc - s) as usize * 16 - i;
                        if seguidas + quedan_aqui >= n {
                            return Ok(desde);
                        }
                        seguidas += quedan_aqui;
                        k += quedan_aqui;
                        return self.crecer_y_seguir(cluster, desde, seguidas, n);
                    }
                    if b == LIBRE {
                        if seguidas == 0 {
                            desde = k;
                        }
                        seguidas += 1;
                        if seguidas == n {
                            return Ok(desde);
                        }
                    } else {
                        seguidas = 0;
                    }
                    k += 1;
                }
            }
            match self.raw_fat_entry(cluster) {
                Some(c) if c >= 0x0FFF_FFF8 => {
                    if seguidas == 0 {
                        desde = k;
                    }
                    return self.crecer_y_seguir(cluster, desde, seguidas, n);
                }
                Some(c) if self.cluster_valido(c) => cluster = c,
                _ => return Err(WriteError::Io),
            }
            vueltas += 1;
            if vueltas > self.max_cluster {
                return Err(WriteError::Io);
            }
        }
    }

    /// El directorio se acabo en `ultimo` con `seguidas` libres al final:
    /// se encadenan clusters a ceros hasta que quepan `n`.
    fn crecer_y_seguir(&mut self, mut ultimo: u32, desde: usize, mut seguidas: usize, n: usize) -> Result<usize, WriteError> {
        let por_cluster = self.sectors_per_cluster as usize * 16;
        // Un directorio FAT no pasa de 65.536 entradas.
        if desde + n > 65_536 {
            return Err(WriteError::DirFull);
        }
        // El ultimo de la cadena de verdad (el de `hueco` puede no serlo si
        // se paro en el 0x00).
        while let Some(c) = self.read_fat_entry(ultimo) {
            ultimo = c;
        }
        while seguidas < n {
            let nuevo = self.escribir_cadena(&[])?;
            if !self.set_fat_entry(ultimo, nuevo) {
                self.free_chain(nuevo);
                return Err(WriteError::Io);
            }
            ultimo = nuevo;
            seguidas += por_cluster;
        }
        Ok(desde)
    }

    /// Un alias `BASE~N` que no este en `dir`.
    fn alias_libre(&mut self, dir: u32, nombre: &[u8]) -> Result<[u8; 11], WriteError> {
        let (base, letras, ext) = base_de_alias(nombre);
        for n in 1..1_000_000u32 {
            let c = alias(&base, letras, &ext, n);
            let mut esta = false;
            let fin = self.recorrer(dir, |v| {
                esta = v.corto() == c;
                esta
            });
            if matches!(fin, Fin::Roto) {
                return Err(WriteError::Io);
            }
            if !esta {
                return Ok(c);
            }
        }
        Err(WriteError::DirFull)
    }

    /// **Apunta `nombre` en `dir`**: las entradas largas (si hacen falta) y la
    /// 8.3 la ULTIMA. Los datos ya tienen que estar en el disco.
    fn apuntar(&mut self, dir: u32, nombre: &[u8], attr: u8, primero: u32, medida: u32) -> Result<EntradaDir, WriteError> {
        let (corto, largas) = match corto_exacto(nombre) {
            Some(c) => (c, 0),
            None => (self.alias_libre(dir, nombre)?, nombre.len().div_ceil(LETRAS)),
        };
        let k = self.hueco(dir, largas + 1)?;
        let s = suma(&corto);
        for j in 0..largas {
            let ord = largas - j;
            let mut e = [0u8; 32];
            e[0] = ord as u8 | if j == 0 { 0x40 } else { 0 };
            e[11] = ATTR_LARGO;
            e[13] = s;
            for (t, &p) in POS.iter().enumerate() {
                let i = (ord - 1) * LETRAS + t;
                let u: u16 = match i.cmp(&nombre.len()) {
                    core::cmp::Ordering::Less => nombre[i] as u16,
                    core::cmp::Ordering::Equal => 0,
                    core::cmp::Ordering::Greater => 0xFFFF,
                };
                e[p..p + 2].copy_from_slice(&u.to_le_bytes());
            }
            self.poner_ranura(dir, k + j, &e)?;
        }
        let mut e = [0u8; 32];
        e[..11].copy_from_slice(&corto);
        e[11] = attr;
        e[20..22].copy_from_slice(&((primero >> 16) as u16).to_le_bytes());
        e[26..28].copy_from_slice(&(primero as u16).to_le_bytes());
        e[28..32].copy_from_slice(&medida.to_le_bytes());
        self.sellar(&mut e);
        let (lba, offset) = self.poner_ranura(dir, k + largas, &e)?;
        Ok(EntradaDir { lba, offset, first_cluster: primero, size: medida })
    }

    /// **Crea la carpeta `nombre` en `dir`** y devuelve su cluster. Si ya
    /// existe como carpeta, devuelve la que hay (crear dos veces no es un
    /// error: es lo que pide un `mkdir -p`); si existe como fichero, `Exists`.
    pub fn crear_carpeta(&mut self, dir: u32, nombre: &[u8]) -> Result<u32, WriteError> {
        self.se_puede_escribir()?;
        if !nombre_valido(nombre) {
            return Err(WriteError::Nombre);
        }
        if let Some(h) = self.hallar(dir, nombre) {
            return if h.es_dir && h.entrada.first_cluster >= 2 { Ok(h.entrada.first_cluster) } else { Err(WriteError::Exists) };
        }
        // El cluster, a ceros (`escribir_cadena` rellena), y en su primer
        // sector `.` (ella) y `..` (la madre; 0 si la madre es la raiz).
        let c = self.escribir_cadena(&[])?;
        let lba = self.cluster_to_lba(c);
        let madre = if dir == self.root_cluster { 0 } else { dir };
        self.buf = [0; 512];
        for (k, (nom, apunta)) in [(b".          ", c), (b"..         ", madre)].into_iter().enumerate() {
            let mut e = [0u8; 32];
            self.sellar(&mut e);
            e[..11].copy_from_slice(nom);
            e[11] = ATTR_DIR;
            e[20..22].copy_from_slice(&((apunta >> 16) as u16).to_le_bytes());
            e[26..28].copy_from_slice(&(apunta as u16).to_le_bytes());
            self.buf[k * 32..k * 32 + 32].copy_from_slice(&e);
        }
        if !self.write_sector(lba, Buf::buf) {
            self.free_chain(c);
            return Err(WriteError::Io);
        }
        match self.apuntar(dir, nombre, ATTR_DIR, c, 0) {
            Ok(_) => Ok(c),
            Err(e) => {
                self.free_chain(c);
                Err(e)
            }
        }
    }

    /// **Guarda `data` como `nombre` en `dir`**, exista o no (crea o
    /// reemplaza, con el orden seguro de `replace_file_fat32`). Acepta
    /// nombres largos.
    pub fn guardar_como(&mut self, dir: u32, nombre: &[u8], data: &[u8]) -> Result<(), WriteError> {
        self.se_puede_escribir()?;
        if !nombre_valido(nombre) {
            return Err(WriteError::Nombre);
        }
        if data.len() > u32::MAX as usize {
            return Err(WriteError::Grande);
        }
        match self.hallar(dir, nombre) {
            Some(h) if h.es_dir => Err(WriteError::Exists),
            Some(h) => self.replace_file_fat32(h.entrada, data),
            None => {
                let primero = self.escribir_cadena(data)?;
                match self.apuntar(dir, nombre, ATTR_ARCHIVO, primero, data.len() as u32) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        self.free_chain(primero);
                        Err(e)
                    }
                }
            }
        }
    }

    /// **Añade `data` al final de `nombre`** (lo crea si no esta). Devuelve
    /// la medida nueva. Ver la cabecera: la medida se apunta la ultima.
    pub fn anadir(&mut self, dir: u32, nombre: &[u8], data: &[u8]) -> Result<u32, WriteError> {
        self.se_puede_escribir()?;
        if !nombre_valido(nombre) {
            return Err(WriteError::Nombre);
        }
        let h = match self.hallar(dir, nombre) {
            None => {
                self.guardar_como(dir, nombre, data)?;
                return Ok(data.len() as u32);
            }
            Some(h) if h.es_dir => return Err(WriteError::Exists),
            Some(h) => h.entrada,
        };
        let viejo = h.size as u64;
        let nuevo = viejo + data.len() as u64;
        if nuevo > u32::MAX as u64 {
            return Err(WriteError::Grande);
        }
        if data.is_empty() {
            return Ok(viejo as u32);
        }
        let cb = self.sectors_per_cluster as u64 * 512;
        let mut primero = h.first_cluster;
        if primero < 2 {
            primero = self.escribir_cadena(data)?;
        } else {
            // El cluster del ultimo byte (o el primero, si esta vacio).
            let pasos = if viejo == 0 { 0 } else { (viejo - 1) / cb };
            let mut ultimo = primero;
            for _ in 0..pasos {
                ultimo = self.read_fat_entry(ultimo).ok_or(WriteError::Io)?;
            }
            let usado = viejo - pasos * cb;
            let cabe = ((cb - usado) as usize).min(data.len());
            if cabe > 0 && !self.escribir_dentro(ultimo, usado as usize, &data[..cabe]) {
                return Err(WriteError::Io);
            }
            let resto = &data[cabe..];
            if !resto.is_empty() {
                // Lo que colgaba detras del ultimo (no deberia haber nada:
                // clusters por detras de la medida) se suelta al final.
                let sobra = self.raw_fat_entry(ultimo).filter(|&c| self.cluster_valido(c));
                let nueva = self.escribir_cadena(resto)?;
                if !self.set_fat_entry(ultimo, nueva) {
                    self.free_chain(nueva);
                    return Err(WriteError::Io);
                }
                if let Some(c) = sobra {
                    self.free_chain(c);
                }
            }
        }
        // Y la medida, LA ULTIMA, en un solo sector.
        if !self.read_sector(h.lba, Buf::buf) {
            return Err(WriteError::Io);
        }
        let (f, hora) = self.sello;
        let e = &mut self.buf[h.offset..h.offset + 32];
        if f != 0 {
            e[22..24].copy_from_slice(&hora.to_le_bytes());
            e[24..26].copy_from_slice(&f.to_le_bytes());
        }
        e[20..22].copy_from_slice(&((primero >> 16) as u16).to_le_bytes());
        e[26..28].copy_from_slice(&(primero as u16).to_le_bytes());
        e[28..32].copy_from_slice(&(nuevo as u32).to_le_bytes());
        if !self.write_sector(h.lba, Buf::buf) {
            return Err(WriteError::Io);
        }
        Ok(nuevo as u32)
    }

    /// Escribe `bytes` dentro del cluster `c` a partir del byte `desde`
    /// (leyendo y reescribiendo los sectores a medias).
    fn escribir_dentro(&mut self, c: u32, desde: usize, bytes: &[u8]) -> bool {
        let Some(lba0) = self.lba_valido(c) else { return false };
        let mut puesto = 0usize;
        while puesto < bytes.len() {
            let pos = desde + puesto;
            let lba = lba0 + (pos / 512) as u64;
            let off = pos % 512;
            let n = (512 - off).min(bytes.len() - puesto);
            if n == 512 {
                if !self.write_from(lba, &bytes[puesto..puesto + 512]) {
                    return false;
                }
            } else {
                if !self.read_sector(lba, Buf::buf) {
                    return false;
                }
                self.buf[off..off + n].copy_from_slice(&bytes[puesto..puesto + n]);
                if !self.write_sector(lba, Buf::buf) {
                    return false;
                }
            }
            puesto += n;
        }
        true
    }
}

#[cfg(test)]
mod reglas {
    use super::*;

    #[test]
    fn el_8_3_exacto_no_recorta() {
        assert_eq!(&corto_exacto(b"salida.txt").unwrap(), b"SALIDA  TXT");
        assert_eq!(&corto_exacto(b"informe").unwrap(), b"INFORME    ");
        assert!(corto_exacto(b"datos-componentes").is_none());
        assert!(corto_exacto(b"a.b.c").is_none());
        assert!(corto_exacto(b"nueve.texto").is_none());
    }

    #[test]
    fn el_alias_es_el_de_windows() {
        let (b, n, e) = base_de_alias(b"datos-componentes");
        assert_eq!(&alias(&b, n, &e, 1), b"DATOS-~1   ");
        let (b, n, e) = base_de_alias(b"mi informe largo.texto");
        assert_eq!(&alias(&b, n, &e, 12), b"MIINF~12TEX");
    }

    #[test]
    fn la_suma_es_la_de_la_especificacion() {
        // La de `FOO     BAR`, calculada a mano con la regla de Microsoft.
        let mut s = 0u8;
        for &c in b"FOO     BAR" {
            s = (if s & 1 != 0 { 0x80 } else { 0 }) + (s >> 1);
            s = s.wrapping_add(c);
        }
        assert_eq!(suma(b"FOO     BAR"), s);
    }
}
