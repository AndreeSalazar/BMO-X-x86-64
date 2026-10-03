//! **EL JUEZ DE LA RAIZ** -- una carpeta como capacidad: un proceso que nace
//! con raiz `hermes` no puede nombrar nada fuera de `hermes/`.
//!
//! generacion: nieto -- recibe dos textos y contesta; no sabe de discos, de pids ni de quien lo usa
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan el kernel y el banco
//!
//! [carril]  AMARILLO  lee la ruta que escribe un proceso de Ring 3
//! [cuesta]  DATO      una ruta mal juzgada abre lo que no es de quien la pide
//! [riesgo]  AJENO     la ruta la escribe el proceso, y puede estar comprometido
//!
//! # H3 de `docs/plan/PLAN_HERMES.md` (2026-10-03)
//!
//! Hasta hoy, cualquier proceso abre cualquier ruta: `sys/director.bex`, el
//! disco `d:` entero, lo que sea. Para HERMES eso no vale: la app de F3 lee
//! fotos y paginas que manda otra maquina, y si un fichero hecho a mala idea
//! la tumba y toma el mando, lo que toma tiene que ser `F:/hermes/` y nada mas.
//!
//! # La regla, y por que es esta y no "empieza por"
//!
//! La forma ingenua es comprobar que la ruta pedida EMPIEZA por la raiz. Esta
//! rota de tres maneras, las tres medidas en el arbol el 03-10:
//!
//! ```text
//!    hermes/../sys/director.bex   FAT32 guarda `.` y `..` en cada carpeta, y
//!                                 su recorrido las sigue como a cualquiera
//!    hermes/.../sys               `to_8_3("...")` da el nombre `..`: el
//!                                 ultimo punto separa, y lo de delante es ".."
//!    c:sys/director.bex           FAT32 se come CUALQUIER letra de unidad, y
//!                                 `d:` es el disco Personal entero
//! ```
//!
//! *** Asi que aqui no se compara: **se construye.** El proceso encerrado ya no
//! escribe rutas del disco: escribe rutas DENTRO de su raiz, y el juez las
//! pega detras. Para que eso no se escape basta con que ningun tramo pueda
//! subir, y eso se garantiza rechazando:
//!
//! ```text
//!    un tramo de puntos y blancos    `.`, `...`, `.. .` -> Subir
//!    espacio al principio o al final `" .."`, `".. "`   -> Borde
//!    dos puntos en cualquier sitio   `d:`, `c:`, `x:y`  -> Letra
//!    un caracter de control          `\0` .. `\x1F`, 7F -> Control
//! ```
//!
//! y separando por `/` Y por `\`, igual que los tres sistemas de ficheros del
//! kernel (FAT32, ESTRATOS y NTFS). Un juez que partiera la ruta distinto que
//! el disco seria un juez de otra ruta.
//!
//! # Y la raiz de un hijo
//!
//! [`Raiz::para_hijo`]: la raiz de un hijo se escribe DENTRO de la de su
//! padre, por el mismo camino. Asi un hijo nunca nace con mas disco que quien
//! lo lanzo, sin tener que comparar nada.

#![no_std]
#![forbid(unsafe_code)]

/// Lo mas larga que puede ser una raiz.
pub const RAIZ_MAX: usize = 64;
/// Lo mas larga que puede ser una ruta ya resuelta: el renglon de rutas del
/// kernel (`RUTA_MAX` en `ring0/syscall/mod.rs`) mide lo mismo.
pub const RUTA_MAX: usize = 128;

/// **Por que no.** Uno por motivo, para CABINA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoRuta {
    /// Un tramo hecho solo de puntos: `.`, `..`, `...`. En FAT32 suben.
    Subir,
    /// Un tramo con espacio al principio o al final.
    Borde,
    /// Dos puntos: una letra de unidad, o algo que lo parece.
    Letra,
    /// Un caracter de control.
    Control,
    /// La ruta resuelta no cabe en el renglon del kernel.
    Larga,
    /// La raiz pedida para un hijo esta vacia, o no cabe.
    Raiz,
}

/// **Una raiz ya juzgada.** Solo se construye por [`Raiz::para_hijo`], asi que
/// tener una es haber pasado el juez.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Raiz {
    b: [u8; RAIZ_MAX],
    n: u8,
}

impl core::fmt::Debug for Raiz {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Raiz({:?})", self.texto())
    }
}

/// Recorre los tramos de una ruta relativa, ya juzgados. Los separadores de
/// mas (al principio, al final o repetidos) no cuentan: no nombran nada.
fn tramos(ruta: &str) -> impl Iterator<Item = Result<&str, NoRuta>> {
    ruta.split(['/', '\\']).filter(|t| !t.is_empty()).map(juzgar_tramo)
}

fn juzgar_tramo(t: &str) -> Result<&str, NoRuta> {
    if t.bytes().any(|c| c < 0x20 || c == 0x7F) {
        return Err(NoRuta::Control);
    }
    if t.contains(':') {
        return Err(NoRuta::Letra);
    }
    // Puntos Y espacios, no solo puntos: `to_8_3(".. .")` corta por el ultimo
    // punto, se queda con `".. "` delante y eso es la entrada `..` de FAT32.
    // Lo encontro la prueba de las cien mil rutas el mismo dia, con la regla de
    // "solo puntos" ya escrita. Cualquier tramo que FAT32 pueda leer como `.`
    // o `..` esta hecho solo de puntos y blancos, y por eso basta con esto.
    if t.chars().all(|c| c == '.' || c.is_whitespace()) {
        return Err(NoRuta::Subir);
    }
    let primero = t.chars().next().is_some_and(char::is_whitespace);
    let ultimo = t.chars().next_back().is_some_and(char::is_whitespace);
    if primero || ultimo {
        return Err(NoRuta::Borde);
    }
    Ok(t)
}

/// Escribe `tramos` detras de `dst[..n]`, separados por `/`.
fn pegar(dst: &mut [u8], mut n: usize, ruta: &str) -> Result<usize, NoRuta> {
    for t in tramos(ruta) {
        let t = t?;
        let sep = usize::from(n > 0);
        let fin = n + sep + t.len();
        if fin > dst.len() {
            return Err(NoRuta::Larga);
        }
        if sep == 1 {
            dst[n] = b'/';
        }
        dst[n + sep..fin].copy_from_slice(t.as_bytes());
        n = fin;
    }
    Ok(n)
}

impl Raiz {
    /// El texto de la raiz, sin barras de mas: `hermes/entrantes`.
    pub fn texto(&self) -> &str {
        // Solo se escriben tramos que eran `&str` enteros y `/`: no puede fallar.
        core::str::from_utf8(&self.b[..self.n as usize]).unwrap_or("")
    }

    /// **La raiz con la que nace un hijo.** `pedida` se lee DENTRO de la raiz
    /// del padre si la tiene (un hijo de `hermes` que pide `entrantes` nace en
    /// `hermes/entrantes`), y desde la raiz del volumen si no. Una raiz vacia
    /// no encierra nada y por eso no se acepta: quien no quiere encerrar a su
    /// hijo, no pide raiz.
    pub fn para_hijo(padre: Option<&Raiz>, pedida: &str) -> Result<Raiz, NoRuta> {
        let mut b = [0u8; RAIZ_MAX];
        let mut n = 0;
        if let Some(p) = padre {
            n = p.n as usize;
            b[..n].copy_from_slice(&p.b[..n]);
        }
        let antes = n;
        n = pegar(&mut b, n, pedida).map_err(|e| if e == NoRuta::Larga { NoRuta::Raiz } else { e })?;
        if n == 0 || (padre.is_none() && n == antes) {
            return Err(NoRuta::Raiz);
        }
        Ok(Raiz { b, n: n as u8 })
    }

    /// **Donde cae `pedida` para un proceso con esta raiz**, escrita en `dst`.
    /// Devuelve cuanto mide. `""` (o solo barras) es la raiz misma.
    pub fn resolver(&self, pedida: &str, dst: &mut [u8; RUTA_MAX]) -> Result<usize, NoRuta> {
        let n = self.n as usize;
        dst[..n].copy_from_slice(&self.b[..n]);
        pegar(dst, n, pedida)
    }
}

#[cfg(test)]
mod pruebas;
