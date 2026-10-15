//! **LA LAMINA DE VERRANO** (2026-09-26) -- el bloque que un programa de OTRO
//! proceso (hoy INTI, que cuenta la tanda del cubo con SSE) escribe y VERRANO
//! lee, sin copias por el kernel, sin cerrojos y sin que ninguno espere al otro.
//!
//! [carril]  VERDE     aritmetica sobre palabras atomicas que da quien llama
//! [consumo] NADA      se lee cuando VERRANO dibuja
//!
//! # Como llega
//!
//! Igual que una superficie (`director/src/scene/surface.rs`): la app pide
//! memoria, escribe la cabecera, y la OFRECE al escritorio
//! (`MEM_OP_OFRECER`). El escritorio la toma UNA vez y a partir de ahi lee con
//! un `mov`. Lo que distingue una lamina de una superficie es la MAGIA de la
//! primera palabra (`BVER` y no `BSUP`), y el escritorio la reconoce en la
//! MISMA puerta donde toma las superficies: nadie se queda con lo del otro.
//!
//! # La forma: 64 bytes de cabecera y DOS ranuras
//!
//! ```text
//!    palabra  campo           quien la escribe
//!    0        MAGIA "BVER"    la app, al crearla
//!    1        VERSION (1)     la app, al crearla
//!    2        CABECERA (64)   la app, al crearla
//!    3        capacidad       la app, al crearla: vertices por ranura
//!    4        secuencia       la app, AL ACABAR cada fotograma (y solo ahi)
//!    5, 6     vertices de la ranura 0 y de la 1
//!    7, 8     fotograma de la ranura 0 y de la 1
//!    9, 10    SELLO de la ranura 0 y de la 1: impar mientras se escribe
//!    11       COMO SE DIBUJA (V2, 11-10): bit 0 con profundidad, bit 1 sin
//!             las caras de detras. La app, al crearla; 0 = como V0 (las
//!             laminas de antes dicen 0: nada cambia para ellas)
//!    12       VUELTA (C1, 15-10): 0, o k + 1 si la ranura k esta EN VUELO
//!             -- la lee la 3060 sin copia; la escribe el escritorio --.
//!             Un escritor que la respeta lo dice con el bit 2 de la palabra
//!             11 (`ESTADO_VUELTA`): solo entonces el escritorio presta la
//!             ranura sin copiarla; si no, la copia como siempre
//!    13       DUERME (C2, 15-10): 1 mientras el escritorio DUERME esperando
//!             una secuencia nueva -- la escribe el escritorio --. Un escritor
//!             con TIMBRE (bit 3 de la palabra 11, `ESTADO_TIMBRE`) la mira
//!             DESPUES de publicar y, si es 1, lo despierta con el futex
//!             COMPARTIDO en la palabra 4 (la secuencia). Sin timbre, el
//!             escritorio mira cada milisegundo, como antes
//!    14, 15   el TSC de la ultima publicacion (C2): lo escribe la app
//!             justo ANTES de la secuencia; 0 = no lo dice. El banco mide
//!             con el de publicado a pagado (la valla)
//!    +64      ranura 0: `capacidad` x `Vertex` (32 bytes cada uno)
//!    +64+C    ranura 1: lo mismo
//! ```
//!
//! # El trato, sin cerrojo (y por que no hay que pelear)
//!
//! Hay UN escritor (la app) y UN lector (VERRANO), y ninguno para al otro:
//!
//! ```text
//!    escribir   s = secuencia; ranura k = (s + 1) % 2 -- la que NO es la
//!               publicada
//!               sello k + 1 (IMPAR: "estoy escribiendo aqui")
//!               los vertices, su numero y su fotograma en la ranura k
//!               sello k + 1 (PAR otra vez: "acabe")
//!               y AL FINAL secuencia = s + 1: esa es la publicacion
//!    leer       s = secuencia (0 = aun nada); ranura k = s % 2
//!               e1 = sello k; si es impar, la app esta dentro: se descarta
//!               copiar; e2 = sello k
//!               e1 = e2: nadie toco la ranura mientras se copiaba, lo
//!               copiado es un fotograma ENTERO
//!               e1 != e2: la pisaron -- se descarta (`Leido::Rota`) y
//!               VERRANO dibuja el anterior. Nunca se ve medio fotograma
//! ```
//!
//! [!] La primera version (26-09) miraba solo la SECUENCIA (s2 - s1 < 2) y la
//! prueba de dos hilos la cazo leyendo medio fotograma: justo despues de
//! publicar, la app ya empieza a escribir la ranura que el lector esta
//! copiando, y la secuencia no ha cambiado todavia. El sello por ranura es el
//! que lo ve, siempre.
//!
//! En x86-64 las escrituras se ven en orden y las lecturas tambien (TSO): la
//! app en INTI solo tiene que ESCRIBIR en este orden, sin barreras. Aqui, en
//! Rust, las barreras se dicen para que valga en cualquier maquina.
//!
//! ** No es un cerrojo, y NO DEBE serlo, por lo mismo que la superficie: un
//! cerrojo entre dos procesos deja al escritorio esperando a una app colgada.
//! Aqui una app que se para deja el ultimo fotograma publicado, y ya.
//!
//! ** EL TRATO UNO (14-10, A2 de `docs/plan/EL_FOCO.md`): la cabecera comun,
//! el sello y las dos ranuras viven ahora en `bmo_lamina::trato`, y BVER es una
//! clase de su censo (`bmo_lamina::censo`). Sin mover un byte: los campos de
//! abajo son los mismos y el emisor de TITAN++ los sigue importando de aqui. Y
//! el trato trajo un arreglo: una ranura ENTERA pero POSTERIOR a la secuencia
//! leida ya no se da por buena (`trato::vigente`); antes, una de cada muchas,
//! el fotograma iba hacia atras.
//!
//! ** LA LAMINA QUIROFANO (10-10, 1e de `docs/plan/EL_FOCO.md`): un dato, un
//! sitio. El que espera un fotograma mira UNA palabra -- la secuencia,
//! [`Lamina::secuencia`] -- y solo con una publicacion nueva copia, y una vez
//! ([`Lamina::leer_si_nueva`]); lo que se dibuja es siempre un fotograma
//! entero. La CPU orquesta (cuando y que); la GPU ejecuta lo que se ve.
//!
//! *** Y el lector NO SE CREE la cabecera: la capacidad tiene que caber en los
//! bytes prestados, y el numero de vertices de una ranura en su capacidad y en
//! triangulos enteros. Una app que mienta no saca al escritorio de su memoria:
//! se queda sin dibujar.

use core::sync::atomic::{AtomicU32, Ordering};

use bmo_lamina::trato;

use crate::{Cull, Vertex, VERTEX_BYTES};

/// `"BVER"` en little-endian, en la primera palabra.
pub const MAGIA: u32 = 0x5245_5642;
const _: () = assert!(MAGIA == u32::from_le_bytes(*b"BVER"));
const _: () = assert!(bmo_lamina::censo::misma_version(bmo_lamina::censo::clase(MAGIA).version, bmo_lamina::censo::Version::Palabra1(VERSION)));
pub const VERSION: u32 = 1;
/// Lo que ocupa la cabecera antes de la ranura 0.
pub const CABECERA: usize = 64;
/// Las palabras de la cabecera (indices de `u32`).
pub const CAMPO_MAGIA: usize = trato::CAMPO_MAGIA;
pub const CAMPO_VERSION: usize = trato::CAMPO_VERSION;
pub const CAMPO_CABECERA: usize = trato::CAMPO_CABECERA;
pub const CAMPO_CAPACIDAD: usize = 3;
pub const CAMPO_SECUENCIA: usize = 4;
/// Vertices de la ranura `k`: `CAMPO_VERTICES + k`.
pub const CAMPO_VERTICES: usize = 5;
/// Fotograma de la ranura `k`: `CAMPO_FOTOGRAMA + k`.
pub const CAMPO_FOTOGRAMA: usize = 7;
/// Sello de la ranura `k`: `CAMPO_SELLO + k`. Impar = se esta escribiendo.
pub const CAMPO_SELLO: usize = 9;
/// Como se dibuja (V2): [`ESTADO_PROFUNDIDAD`] y [`ESTADO_DESCARTE`].
pub const CAMPO_ESTADO: usize = 11;
/// Bit 0 de [`CAMPO_ESTADO`]: con z-buffer (`Frame::depth`).
pub const ESTADO_PROFUNDIDAD: u32 = 1;
/// Bit 1 de [`CAMPO_ESTADO`]: sin las caras de detras (`Cull::Back`).
pub const ESTADO_DESCARTE: u32 = 2;
/// Bit 2 de [`CAMPO_ESTADO`] (C1): el escritor RESPETA la palabra de vuelta
/// (`trato::empezar_si_libre`). Sin el, el escritorio no presta la ranura sin
/// copia: un escritor de antes la pisaria mientras la 3060 la lee.
pub const ESTADO_VUELTA: u32 = 4;
/// La palabra de VUELTA (C1): `k + 1` si la ranura `k` esta en vuelo.
pub const CAMPO_VUELTA: usize = 12;
/// Bit 3 de [`CAMPO_ESTADO`] (C2): el escritor TOCA EL TIMBRE -- despues de
/// publicar mira [`CAMPO_DUERME`] y, si alguien duerme, lo despierta con el
/// futex compartido en [`CAMPO_SECUENCIA`] --. Solo entonces el escritorio
/// se atreve a dormir en el sello en vez de mirar cada milisegundo.
pub const ESTADO_TIMBRE: u32 = 8;
/// DUERME (C2): 1 mientras el escritorio espera dormido una secuencia nueva.
pub const CAMPO_DUERME: usize = 13;
/// El TSC de la ultima publicacion (C2): dos palabras, la baja primero
/// (byte 56: alineado a 8, el emisor lo escribe de una vez).
pub const CAMPO_PUBLICADA: usize = 14;

const PALABRAS_VERTICE: usize = VERTEX_BYTES / 4;
const _: () = assert!(CAMPO_SELLO + 2 <= CABECERA / 4 && CAMPO_VUELTA < CABECERA / 4 && CAMPO_VUELTA > CAMPO_ESTADO);
const _: () = assert!(CAMPO_DUERME == CAMPO_VUELTA + 1 && CAMPO_PUBLICADA == CAMPO_DUERME + 1 && CAMPO_PUBLICADA + 2 <= CABECERA / 4 && (4 * CAMPO_PUBLICADA) % 8 == 0);
// Sin mover un byte (A2): las palabras de BVER son las de antes del trato.
const _: () = assert!(CAMPO_MAGIA == 0 && CAMPO_VERSION == 1 && CAMPO_CABECERA == 2 && CAMPO_CAPACIDAD == trato::PALABRAS_COMUNES);

/// Lo que mide una lamina de `capacidad` vertices por ranura.
pub const fn bytes_para(capacidad: usize) -> usize {
    CABECERA + 2 * capacidad * VERTEX_BYTES
}

/// Por que un bloque no es una lamina.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoEs {
    /// Menos bytes que una cabecera, o no es multiplo de 4.
    Corta,
    /// La primera palabra no es `BVER`: no es una lamina (quiza una superficie).
    Magia,
    /// Otra version, u otra medida de cabecera.
    Version,
    /// La capacidad que declara no cabe en lo que se presto (o es cero).
    NoCabe,
}

/// Lo que dio una publicacion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publicacion {
    Hecha,
    /// No cabe en la capacidad, o no son triangulos enteros.
    NoCabe,
    /// La ranura que tocaba esta EN VUELO (la 3060 la lee): este fotograma no
    /// se escribe -- gana el siguiente --.
    Ocupada,
}

/// **Una ranura RESERVADA** para quien la lee sin copia (C1): su secuencia,
/// el fotograma, cuantos vertices y donde empiezan, en bytes desde el
/// principio de la lamina. Vale hasta [`Lamina::soltar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reserva {
    pub secuencia: u32,
    pub ranura: usize,
    pub fotograma: u32,
    pub vertices: usize,
    pub desde_byte: usize,
}

/// Lo que dio una lectura.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leido {
    /// La app aun no publico ningun fotograma.
    Nada,
    /// La app volvio a esta ranura mientras se copiaba: se descarta.
    Rota,
    /// La ranura publicada dice un numero de vertices imposible: la app miente.
    Mentira,
    /// Un fotograma ENTERO: su numero y cuantos vertices.
    Fotograma { fotograma: u32, vertices: usize },
}

/// **Una lamina**: el bloque entero, visto como palabras atomicas.
pub struct Lamina<'a> {
    w: &'a [AtomicU32],
    capacidad: usize,
}

impl<'a> Lamina<'a> {
    /// **Crearla** (la app, una vez): la cabecera y la secuencia a cero.
    /// `None` si el bloque no es de al menos `bytes_para(capacidad)`.
    pub fn crear(w: &'a [AtomicU32], capacidad: usize) -> Option<Self> {
        if capacidad == 0 || w.len() * 4 < bytes_para(capacidad) || !trato::crear(w, MAGIA, VERSION, CABECERA) {
            return None;
        }
        w[CAMPO_CAPACIDAD].store(capacidad as u32, Ordering::Release);
        // Este escritor respeta la vuelta (C1): lo dice desde que nace.
        w[CAMPO_ESTADO].store(ESTADO_VUELTA, Ordering::Release);
        Some(Lamina { w, capacidad })
    }

    /// **Abrirla** (VERRANO): comprueba la cabecera contra lo prestado.
    pub fn abrir(w: &'a [AtomicU32]) -> Result<Self, NoEs> {
        trato::abrir(w, MAGIA, VERSION, CABECERA).map_err(|e| match e {
            trato::NoEs::Corta => NoEs::Corta,
            trato::NoEs::Magia => NoEs::Magia,
            trato::NoEs::Version => NoEs::Version,
        })?;
        let capacidad = w[CAMPO_CAPACIDAD].load(Ordering::Relaxed) as usize;
        if capacidad == 0 || capacidad > (w.len() * 4 - CABECERA) / (2 * VERTEX_BYTES) {
            return Err(NoEs::NoCabe);
        }
        Ok(Lamina { w, capacidad })
    }

    pub fn capacidad(&self) -> usize {
        self.capacidad
    }

    /// **Como se dibuja** (la app, V2): con profundidad, y que caras fuera.
    pub fn poner_estado(&self, depth: bool, cull: Cull) {
        // El timbre (C2) no es de como se dibuja: se queda como estaba.
        let timbre = self.w[CAMPO_ESTADO].load(Ordering::Relaxed) & ESTADO_TIMBRE;
        let e = if depth { ESTADO_PROFUNDIDAD } else { 0 } | if cull == Cull::Back { ESTADO_DESCARTE } else { 0 } | ESTADO_VUELTA | timbre;
        self.w[CAMPO_ESTADO].store(e, Ordering::Release);
    }

    /// **Como pide que se dibuje** (VERRANO): `(depth, cull)` para el
    /// `Frame`. Los bits que no se conocen no cuentan.
    pub fn estado(&self) -> (bool, Cull) {
        let e = self.w[CAMPO_ESTADO].load(Ordering::Acquire);
        (e & ESTADO_PROFUNDIDAD != 0, if e & ESTADO_DESCARTE != 0 { Cull::Back } else { Cull::None })
    }

    /// Donde empieza la ranura `k`, en palabras.
    fn ranura(&self, k: usize) -> usize {
        CABECERA / 4 + k * self.capacidad * PALABRAS_VERTICE
    }

    /// **Publicar un fotograma** (la app): en la ranura que no se lee, y la
    /// secuencia AL FINAL. `false` si no se publico (ver [`Lamina::publica`]).
    pub fn publicar(&self, fotograma: u32, v: &[Vertex]) -> bool {
        self.publica(fotograma, v) == Publicacion::Hecha
    }

    /// **Publicar, y por que no** (C1): si la ranura que toca esta en vuelo,
    /// [`Publicacion::Ocupada`] sin tocar un byte.
    pub fn publica(&self, fotograma: u32, v: &[Vertex]) -> Publicacion {
        self.publica_con_hora(fotograma, v, 0)
    }

    /// [`Lamina::publica`], diciendo CUANDO (C2): `tsc` es el reloj de la
    /// CPU de quien publica (en BMO-X, `bmo::ciclos()`), y el banco mide con
    /// el de publicado a pagado. Esta crate no lee el reloj: no tiene
    /// `unsafe`. 0 = no lo dice.
    pub fn publica_con_hora(&self, fotograma: u32, v: &[Vertex], tsc: u64) -> Publicacion {
        if v.len() > self.capacidad || v.len() % 3 != 0 {
            return Publicacion::NoCabe;
        }
        let s = trato::secuencia_propia(self.w, CAMPO_SECUENCIA);
        let k = trato::ranura_para_escribir(s);
        let r = self.ranura(k);
        // El sello, IMPAR antes de tocar nada de la ranura; y si la ranura
        // esta en vuelo, ni eso.
        let Some(sello) = trato::empezar_si_libre(self.w, CAMPO_SELLO, CAMPO_VUELTA, k) else {
            return Publicacion::Ocupada;
        };
        for (i, x) in v.iter().enumerate() {
            let p = r + i * PALABRAS_VERTICE;
            for (j, b) in x.position.iter().chain(x.color.iter()).enumerate() {
                self.w[p + j].store(b.to_bits(), Ordering::Relaxed);
            }
        }
        self.w[CAMPO_VERTICES + k].store(v.len() as u32, Ordering::Relaxed);
        self.w[CAMPO_FOTOGRAMA + k].store(fotograma, Ordering::Relaxed);
        // PAR otra vez: la ranura esta entera.
        trato::acabar(self.w, CAMPO_SELLO + k, sello);
        // C2: cuando se publico, para el banco (antes que la secuencia).
        let t = tsc;
        self.w[CAMPO_PUBLICADA].store(t as u32, Ordering::Relaxed);
        self.w[CAMPO_PUBLICADA + 1].store((t >> 32) as u32, Ordering::Relaxed);
        // La publicacion: todo lo de arriba se ve antes que este numero.
        trato::publicar(self.w, CAMPO_SECUENCIA, s);
        Publicacion::Hecha
    }

    /// ** C2 -- EL TIMBRE, del lado de la app. Quien toca el timbre lo dice
    /// UNA vez (al crear) y, despues de cada [`Lamina::publica`] hecha,
    /// pregunta [`Lamina::hay_quien_duerme`]: si si, despierta con el futex
    /// COMPARTIDO en [`Lamina::palabra_secuencia`] (en BMO-X,
    /// `bmo::futex::despertar_compartido`). Esta crate no llama al kernel:
    /// lo hace quien la usa.
    pub fn con_timbre(&self) {
        self.w[CAMPO_ESTADO].fetch_or(ESTADO_TIMBRE, Ordering::Release);
    }

    /// Si el escritor dice que toca el timbre (C2): solo entonces el
    /// escritorio duerme en el sello.
    pub fn tiene_timbre(&self) -> bool {
        self.w[CAMPO_ESTADO].load(Ordering::Acquire) & ESTADO_TIMBRE != 0
    }

    /// **Hay quien duerme** (la app, DESPUES de publicar). La valla va entre
    /// la secuencia (ya escrita) y esta mirada, y el escritorio pone la suya
    /// entre DUERME y su mirada a la secuencia ([`Lamina::me_duermo`]): o la
    /// app ve el 1 y despierta, o el escritorio ve la secuencia nueva y no
    /// duerme. Las dos cosas a la vez no pueden faltar (Dekker otra vez).
    pub fn hay_quien_duerme(&self) -> bool {
        core::sync::atomic::fence(Ordering::SeqCst);
        self.w[CAMPO_DUERME].load(Ordering::Relaxed) != 0
    }

    /// La palabra en la que se duerme y se despierta: la SECUENCIA.
    pub fn palabra_secuencia(&self) -> &AtomicU32 {
        &self.w[CAMPO_SECUENCIA]
    }

    /// **Me duermo** (el escritorio, C2): DUERME = 1, la valla, y la
    /// secuencia que hay AHORA. Si es la ya vista, a dormir en el futex con
    /// ella como `visto` (el kernel la vuelve a mirar bajo su cerrojo); si
    /// no, hay fotograma y no se duerme. Despues, [`Lamina::despierto`].
    pub fn me_duermo(&self) -> u32 {
        self.w[CAMPO_DUERME].store(1, Ordering::Relaxed);
        core::sync::atomic::fence(Ordering::SeqCst);
        self.secuencia()
    }

    /// Ya no duerme: la app deja de llamar al kernel para despertarlo.
    pub fn despierto(&self) {
        self.w[CAMPO_DUERME].store(0, Ordering::Relaxed);
    }

    /// **El TSC de la ultima publicacion** (C2): 0 si la app no lo dice.
    /// Puede ser de la siguiente a la leida si la app publico entre medias:
    /// es una medida, no un sello.
    pub fn publicada_tsc(&self) -> u64 {
        self.w[CAMPO_PUBLICADA].load(Ordering::Relaxed) as u64 | (self.w[CAMPO_PUBLICADA + 1].load(Ordering::Relaxed) as u64) << 32
    }

    /// Si su escritor respeta la vuelta (C1): solo entonces se puede
    /// [`Lamina::reservar`].
    pub fn respeta_vuelta(&self) -> bool {
        self.w[CAMPO_ESTADO].load(Ordering::Acquire) & ESTADO_VUELTA != 0
    }

    /// **Reservar la ranura publicada** para leerla sin copia (C1, el
    /// escritorio que se la da a la 3060). `None` si no hay nada, si el
    /// escritor estaba dentro, si no respeta la vuelta, o si la ranura dice
    /// un numero de vertices imposible (miente: no se presta). Hasta
    /// [`Lamina::soltar`] el escritor no toca esa ranura.
    pub fn reservar(&self) -> Option<Reserva> {
        if !self.respeta_vuelta() {
            return None;
        }
        let (s, k) = trato::reservar(self.w, CAMPO_VUELTA, CAMPO_SECUENCIA, CAMPO_SELLO)?;
        let n = self.w[CAMPO_VERTICES + k].load(Ordering::Relaxed) as usize;
        if n > self.capacidad || n % 3 != 0 {
            trato::soltar(self.w, CAMPO_VUELTA);
            return None;
        }
        Some(Reserva {
            secuencia: s,
            ranura: k,
            fotograma: self.w[CAMPO_FOTOGRAMA + k].load(Ordering::Relaxed),
            vertices: n,
            desde_byte: self.ranura(k) * 4,
        })
    }

    /// **Soltar la reserva** (tras la valla de la 3060).
    pub fn soltar(&self) {
        trato::soltar(self.w, CAMPO_VUELTA);
    }

    /// **Leer el ultimo fotograma publicado** (VERRANO), a `out`.
    pub fn leer(&self, out: &mut [Vertex]) -> Leido {
        self.leer_si_nueva(0, out).1
    }

    /// La SECUENCIA publicada (0 = aun nada): mirarla cuesta una palabra, sin
    /// copiar nada.
    pub fn secuencia(&self) -> u32 {
        trato::secuencia(self.w, CAMPO_SECUENCIA)
    }

    /// ** LA LAMINA QUIROFANO (10-10): `leer`, pero SOLO si la app publico
    /// algo despues de `vista` (la secuencia de lo ultimo que se leyo). Si
    /// no, `Leido::Nada` sin tocar un byte de `out`: el que espera un
    /// fotograma nuevo mira UNA palabra por vuelta, no copia la ranura entera
    /// cada vez. Da la secuencia que leyo (la que se pasa la proxima vez).
    pub fn leer_si_nueva(&self, vista: u32, out: &mut [Vertex]) -> (u32, Leido) {
        let s = self.secuencia();
        if s == 0 || s == vista {
            return (vista, Leido::Nada);
        }
        match self.leer_ranura(s, out) {
            // Pillada a medio escribir: NO se da por vista, la proxima mirada
            // la vuelve a intentar.
            Leido::Rota => (vista, Leido::Rota),
            leido => (s, leido),
        }
    }

    fn leer_ranura(&self, s: u32, out: &mut [Vertex]) -> Leido {
        let k = trato::ranura_publicada(s);
        let Some(e1) = trato::mirar(self.w, CAMPO_SELLO + k) else {
            // La app ya esta escribiendo aqui (va por el siguiente).
            return Leido::Rota;
        };
        let n = self.w[CAMPO_VERTICES + k].load(Ordering::Relaxed) as usize;
        let fotograma = self.w[CAMPO_FOTOGRAMA + k].load(Ordering::Relaxed);
        let entero = n <= self.capacidad && n <= out.len() && n % 3 == 0;
        if entero {
            let r = self.ranura(k);
            for (i, x) in out[..n].iter_mut().enumerate() {
                let p = r + i * PALABRAS_VERTICE;
                let b = |j: usize| f32::from_bits(self.w[p + j].load(Ordering::Relaxed));
                *x = Vertex { position: [b(0), b(1), b(2), b(3)], color: [b(4), b(5), b(6), b(7)] };
            }
        }
        // Lo leido vale si NADIE toco la ranura mientras tanto, y si es el de
        // `s` y no uno que la app dejo despues en la misma ranura (14-10).
        if !trato::intacto(self.w, CAMPO_SELLO + k, e1) || !trato::vigente(self.w, CAMPO_SECUENCIA, s) {
            return Leido::Rota;
        }
        if !entero {
            // La ranura estaba quieta y aun asi dice un numero imposible.
            return Leido::Mentira;
        }
        Leido::Fotograma { fotograma, vertices: n }
    }
}

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;
    use std::sync::Arc;
    use std::vec::Vec;

    fn bloque(capacidad: usize) -> Vec<AtomicU32> {
        (0..bytes_para(capacidad) / 4).map(|_| AtomicU32::new(0)).collect()
    }

    /// ** V2: como se dibuja viaja en la cabecera. Una lamina nueva dice V0
    /// (las de antes escribian 0 ahi: para ellas nada cambia), y lo que la app
    /// pone es lo que VERRANO lee.
    #[test]
    fn how_it_draws_travels_in_the_header() {
        let b = bloque(6);
        let l = Lamina::crear(&b, 6).unwrap();
        assert_eq!(Lamina::abrir(&b).unwrap().estado(), (false, Cull::None));
        l.poner_estado(true, Cull::Back);
        assert_eq!(Lamina::abrir(&b).unwrap().estado(), (true, Cull::Back));
        // (y el escritor de Rust dice ademas que respeta la vuelta: C1)
        assert_eq!(b[CAMPO_ESTADO].load(Ordering::Relaxed), ESTADO_PROFUNDIDAD | ESTADO_DESCARTE | ESTADO_VUELTA);
        l.poner_estado(true, Cull::None);
        assert_eq!(Lamina::abrir(&b).unwrap().estado(), (true, Cull::None));
        // un bit que no se conoce no cuenta
        b[CAMPO_ESTADO].store(0x80 | ESTADO_DESCARTE, Ordering::Relaxed);
        assert_eq!(Lamina::abrir(&b).unwrap().estado(), (false, Cull::Back));
    }

    fn marca(f: u32, n: usize) -> Vec<Vertex> {
        // Cada vertice lleva su fotograma DENTRO: si alguna vez se lee uno
        // mezclado con otro, se ve.
        (0..n).map(|i| Vertex { position: [f as f32, i as f32, 0.0, 1.0], color: [f as f32; 4] }).collect()
    }

    #[test]
    fn se_publica_y_se_lee_entero() {
        let b = bloque(36);
        let l = Lamina::crear(&b, 36).unwrap();
        let mut out = [Vertex::default(); 36];
        assert_eq!(Lamina::abrir(&b).unwrap().leer(&mut out), Leido::Nada);
        assert!(l.publicar(7, &marca(7, 12)));
        assert_eq!(l.leer(&mut out), Leido::Fotograma { fotograma: 7, vertices: 12 });
        assert_eq!(&out[..12], &marca(7, 12)[..]);
        assert!(l.publicar(8, &marca(8, 18)));
        assert_eq!(l.leer(&mut out), Leido::Fotograma { fotograma: 8, vertices: 18 });
        assert!(!l.publicar(9, &marca(9, 39)), "no cabe");
        assert!(!l.publicar(9, &marca(9, 4)), "no son triangulos enteros");
    }

    /// ** Quirofano: sin publicacion nueva, NADA se copia (`out` intacto); con
    /// una, se lee entera y se apunta su secuencia.
    #[test]
    fn sin_nada_nuevo_no_se_copia_nada() {
        let b = bloque(12);
        let l = Lamina::crear(&b, 12).unwrap();
        let centinela = Vertex { position: [9.0; 4], color: [9.0; 4] };
        let mut out = [centinela; 12];
        assert_eq!(l.leer_si_nueva(0, &mut out), (0, Leido::Nada));
        assert!(l.publicar(1, &marca(1, 6)));
        let (s, leido) = l.leer_si_nueva(0, &mut out);
        assert_eq!((s, leido), (l.secuencia(), Leido::Fotograma { fotograma: 1, vertices: 6 }));
        out = [centinela; 12];
        assert_eq!(l.leer_si_nueva(s, &mut out), (s, Leido::Nada));
        assert!(out.iter().all(|v| *v == centinela), "lo ya visto no se vuelve a copiar");
        assert!(l.publicar(2, &marca(2, 3)));
        let (s2, leido) = l.leer_si_nueva(s, &mut out);
        assert_eq!((s2, leido), (s + 1, Leido::Fotograma { fotograma: 2, vertices: 3 }));
        assert_eq!(&out[..3], &marca(2, 3)[..]);
        // A medio escribir (sello impar): Rota, y la secuencia vista NO avanza.
        assert!(l.publicar(3, &marca(3, 3)));
        let k = (l.secuencia() % 2) as usize;
        b[CAMPO_SELLO + k].fetch_add(1, Ordering::Relaxed);
        assert_eq!(l.leer_si_nueva(s2, &mut out), (s2, Leido::Rota));
        b[CAMPO_SELLO + k].fetch_add(1, Ordering::Relaxed);
        assert_eq!(l.leer_si_nueva(s2, &mut out).1, Leido::Fotograma { fotograma: 3, vertices: 3 });
    }

    /// ** 14-10: el lector lee la secuencia, la app publica el siguiente y
    /// deja ENTERO el de despues en la ranura que el lector va a copiar. Lo
    /// copiado no esta roto, pero es POSTERIOR: darlo por el publicado hace
    /// que la vuelta siguiente lea uno mas viejo (el fotograma va hacia
    /// atras). Ahora es `Rota`, y se vuelve a mirar.
    #[test]
    fn uno_posterior_en_la_misma_ranura_es_rota() {
        let b = bloque(6);
        let l = Lamina::crear(&b, 6).unwrap();
        assert!(l.publicar(1, &marca(1, 6)));
        let s = l.secuencia();
        // La app, entre la mirada del lector y su copia: el 2 y el 3.
        assert!(l.publicar(2, &marca(2, 6)));
        assert!(l.publicar(3, &marca(3, 6)));
        // El 3 esta publicado (secuencia s + 2) y vive en la ranura de s:
        // se deshace SOLO la publicacion, como si el lector llegara antes.
        b[CAMPO_SECUENCIA].store(s + 1, Ordering::Relaxed);
        let mut out = [Vertex::default(); 6];
        assert_eq!(l.leer_ranura(s, &mut out), Leido::Rota, "el 3 no es el de la secuencia s");
        b[CAMPO_SECUENCIA].store(s + 2, Ordering::Relaxed);
        assert_eq!(l.leer_ranura(s + 2, &mut out), Leido::Fotograma { fotograma: 3, vertices: 6 });
    }

    /// El lector no se cree la cabecera.
    #[test]
    fn una_cabecera_que_miente_no_saca_a_nadie_de_su_memoria() {
        let b = bloque(12);
        Lamina::crear(&b, 12).unwrap();
        assert!(Lamina::abrir(&b[..8]).is_err_and(|e| e == NoEs::Corta));
        b[CAMPO_CAPACIDAD].store(13, Ordering::Relaxed);
        assert_eq!(Lamina::abrir(&b).err(), Some(NoEs::NoCabe));
        b[CAMPO_CAPACIDAD].store(12, Ordering::Relaxed);
        b[CAMPO_MAGIA].store(u32::from_le_bytes(*b"BSUP"), Ordering::Relaxed);
        assert_eq!(Lamina::abrir(&b).err(), Some(NoEs::Magia), "una superficie no es una lamina");
        b[CAMPO_MAGIA].store(MAGIA, Ordering::Relaxed);
        let l = Lamina::abrir(&b).unwrap();
        l.publicar(1, &marca(1, 6));
        // La app dice que la ranura publicada tiene 1000 vertices.
        let k = (b[CAMPO_SECUENCIA].load(Ordering::Relaxed) % 2) as usize;
        b[CAMPO_VERTICES + k].store(1000, Ordering::Relaxed);
        let mut out = [Vertex::default(); 12];
        assert_eq!(l.leer(&mut out), Leido::Mentira);
    }

    /// ***Sin pelea, castigado***: una app publicando sin parar en un hilo y
    /// VERRANO leyendo sin parar en otro. Todo lo que VERRANO da por bueno es
    /// un fotograma ENTERO -- todos sus vertices del mismo fotograma, y ese
    /// fotograma es el que dice la cabecera --, y ademas ve fotogramas que
    /// avanzan.
    #[test]
    fn un_escritor_y_un_lector_sin_cerrojo_nunca_ven_medio_fotograma() {
        let b: Arc<Vec<AtomicU32>> = Arc::new(bloque(24));
        Lamina::crear(&b, 24).unwrap();
        let escritor = {
            let b = Arc::clone(&b);
            std::thread::spawn(move || {
                let l = Lamina::abrir(&b).unwrap();
                for f in 1..=200_000u32 {
                    l.publicar(f, &marca(f, 24));
                }
            })
        };
        let l = Lamina::abrir(&b).unwrap();
        let mut out = [Vertex::default(); 24];
        let (mut buenos, mut rotos, mut ultimo) = (0u32, 0u32, 0u32);
        while !escritor.is_finished() || buenos == 0 {
            match l.leer(&mut out) {
                Leido::Fotograma { fotograma, vertices } => {
                    assert_eq!(vertices, 24);
                    for (i, v) in out.iter().enumerate() {
                        assert_eq!(v.position[0], fotograma as f32, "vertice {i} de OTRO fotograma: medio fotograma leido");
                        assert_eq!(v.color[3], fotograma as f32);
                    }
                    assert!(fotograma >= ultimo, "los fotogramas no van hacia atras");
                    ultimo = fotograma;
                    buenos += 1;
                }
                Leido::Rota => rotos += 1,
                Leido::Nada => {}
                Leido::Mentira => panic!("una app honrada no miente"),
            }
        }
        escritor.join().unwrap();
        assert!(buenos > 0);
        std::eprintln!("lamina: {buenos} fotogramas enteros leidos, {rotos} descartados, el ultimo {ultimo}");
    }

    /// ** C1: la ranura RESERVADA para la 3060 no la pisa el escritor. La
    /// reserva dice donde esta (para prestarla por la IOMMU); mientras dura,
    /// el fotograma que tocaba en esa ranura sale `Ocupada`; soltada, vuelve.
    #[test]
    fn a_reserved_slot_is_not_written_until_released() {
        let b = bloque(6);
        let l = Lamina::crear(&b, 6).unwrap();
        assert!(l.respeta_vuelta());
        assert_eq!(l.reservar(), None, "nada publicado");
        assert_eq!(l.publica(1, &marca(1, 6)), Publicacion::Hecha);
        let r = l.reservar().expect("la publicada");
        assert_eq!((r.secuencia, r.fotograma, r.vertices), (1, 1, 6));
        assert_eq!(r.desde_byte, CABECERA + r.ranura * 6 * VERTEX_BYTES);
        let antes: Vec<u32> = b.iter().map(|x| x.load(Ordering::Relaxed)).collect();
        // la otra ranura esta libre: se publica el 2
        assert_eq!(l.publica(2, &marca(2, 3)), Publicacion::Hecha);
        // el 3 tocaria la reservada: no se escribe, y sus bytes no cambian
        assert_eq!(l.publica(3, &marca(3, 6)), Publicacion::Ocupada);
        let ranura = |v: &[u32]| v[r.desde_byte / 4..r.desde_byte / 4 + 6 * VERTEX_BYTES / 4].to_vec();
        let ahora: Vec<u32> = b.iter().map(|x| x.load(Ordering::Relaxed)).collect();
        assert_eq!(ranura(&ahora), ranura(&antes), "la ranura en vuelo no se toco");
        assert_eq!(b[CAMPO_VERTICES + r.ranura].load(Ordering::Relaxed), 6);
        l.soltar();
        assert_eq!(l.publica(3, &marca(3, 6)), Publicacion::Hecha, "soltada, se escribe");
        assert_eq!(l.reservar().map(|r| r.fotograma), Some(3));
    }

    /// ** C2: EL TIMBRE no pierde un despertar. Un "kernel" de mentira con
    /// un cerrojo y una condicion hace lo que `wait_current_checked`: mira la
    /// palabra BAJO el cerrojo antes de dormir. El escritorio duerme con un
    /// plazo largo (1 s); un despertar perdido seria un plazo cumplido. 3000
    /// fotogramas, ninguno; y el escritor que NO toca el timbre (el sabotaje)
    /// deja al escritorio dormido hasta el plazo: la prueba muerde.
    #[test]
    fn the_bell_never_loses_a_wakeup() {
        use std::sync::{Condvar, Mutex};
        use std::time::Duration;
        struct Kernel {
            m: Mutex<()>,
            c: Condvar,
        }
        fn corre(toca: bool, fotogramas: u32) -> (u32, u32) {
            let b: Arc<Vec<AtomicU32>> = Arc::new(bloque(3));
            let k = Arc::new(Kernel { m: Mutex::new(()), c: Condvar::new() });
            Lamina::crear(&b, 3).unwrap().con_timbre();
            let (b2, k2) = (b.clone(), k.clone());
            let app = std::thread::spawn(move || {
                let l = Lamina::abrir(&b2).unwrap();
                for f in 1..=fotogramas {
                    while l.publica(f, &marca(f, 3)) != Publicacion::Hecha {}
                    if toca && l.hay_quien_duerme() {
                        let _g = k2.m.lock().unwrap();
                        k2.c.notify_all();
                    }
                    if f % 64 == 0 {
                        std::thread::yield_now();
                    }
                    // El saboteador va despacio: el escritorio llega a dormirse.
                    if !toca {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            });
            let l = Lamina::abrir(&b).unwrap();
            assert!(l.tiene_timbre());
            let (mut visto, mut plazos) = (0u32, 0u32);
            while visto < fotogramas {
                let s = l.me_duermo();
                if s == visto {
                    let g = k.m.lock().unwrap();
                    // Bajo el cerrojo, otra vez: lo que hace el kernel.
                    if l.secuencia() == visto {
                        let (_g, r) = k.c.wait_timeout(g, Duration::from_millis(if toca { 1000 } else { 20 })).unwrap();
                        // Desperto el PLAZO, no el timbre.
                        if r.timed_out() {
                            plazos += 1;
                            if !toca && plazos > 2 {
                                l.despierto();
                                break;
                            }
                        }
                    }
                }
                l.despierto();
                visto = l.secuencia();
            }
            app.join().unwrap();
            (visto, plazos)
        }
        let (visto, plazos) = corre(true, 3000);
        assert_eq!((visto, plazos), (3000, 0), "con timbre, ningun despertar perdido");
        let (_, plazos) = corre(false, 5);
        assert!(plazos > 0, "sin timbre el escritorio se queda dormido: la prueba muerde");
    }

    /// La publicacion dice cuando fue, y el estado de dibujo no borra el timbre.
    #[test]
    fn the_publication_says_when_and_drawing_state_keeps_the_bell() {
        let b = bloque(3);
        let l = Lamina::crear(&b, 3).unwrap();
        assert_eq!(l.publicada_tsc(), 0);
        assert!(!l.tiene_timbre(), "una lamina nueva no lo promete");
        l.con_timbre();
        l.poner_estado(true, Cull::Back);
        assert!(l.tiene_timbre() && l.respeta_vuelta());
        assert!(l.publicar(1, &marca(1, 3)));
        assert_eq!(l.publicada_tsc(), 0, "sin hora, no la dice");
        assert_eq!(l.publica_con_hora(2, &marca(2, 3), 0x1_2345_6789_ABCD), Publicacion::Hecha);
        assert_eq!(l.publicada_tsc(), 0x1_2345_6789_ABCD);
        assert!(!l.hay_quien_duerme());
        assert_eq!(l.me_duermo(), 2);
        assert!(l.hay_quien_duerme());
        l.despierto();
        assert!(!l.hay_quien_duerme());
    }

    /// Una lamina de un escritor que NO respeta la vuelta (las de antes del
    /// 15-10, sin el bit) no se reserva: se sigue copiando.
    #[test]
    fn an_old_writer_is_never_lent_without_a_copy() {
        let b = bloque(6);
        let l = Lamina::crear(&b, 6).unwrap();
        assert!(l.publicar(1, &marca(1, 6)));
        b[CAMPO_ESTADO].store(0, Ordering::Relaxed);
        assert!(!l.respeta_vuelta());
        assert_eq!(l.reservar(), None);
        let mut out = [Vertex { position: [0.0; 4], color: [0.0; 4] }; 6];
        assert!(matches!(l.leer(&mut out), Leido::Fotograma { fotograma: 1, vertices: 6 }), "copiar, si");
    }
}
