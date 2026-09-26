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
//!    11..16   ceros
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
//! *** Y el lector NO SE CREE la cabecera: la capacidad tiene que caber en los
//! bytes prestados, y el numero de vertices de una ranura en su capacidad y en
//! triangulos enteros. Una app que mienta no saca al escritorio de su memoria:
//! se queda sin dibujar.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{Vertex, VERTEX_BYTES};

/// `"BVER"` en little-endian, en la primera palabra.
pub const MAGIA: u32 = 0x5245_5642;
const _: () = assert!(MAGIA == u32::from_le_bytes(*b"BVER"));
pub const VERSION: u32 = 1;
/// Lo que ocupa la cabecera antes de la ranura 0.
pub const CABECERA: usize = 64;
/// Las palabras de la cabecera (indices de `u32`).
pub const CAMPO_MAGIA: usize = 0;
pub const CAMPO_VERSION: usize = 1;
pub const CAMPO_CABECERA: usize = 2;
pub const CAMPO_CAPACIDAD: usize = 3;
pub const CAMPO_SECUENCIA: usize = 4;
/// Vertices de la ranura `k`: `CAMPO_VERTICES + k`.
pub const CAMPO_VERTICES: usize = 5;
/// Fotograma de la ranura `k`: `CAMPO_FOTOGRAMA + k`.
pub const CAMPO_FOTOGRAMA: usize = 7;
/// Sello de la ranura `k`: `CAMPO_SELLO + k`. Impar = se esta escribiendo.
pub const CAMPO_SELLO: usize = 9;

const PALABRAS_VERTICE: usize = VERTEX_BYTES / 4;
const _: () = assert!(CAMPO_SELLO + 2 <= CABECERA / 4);

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
        if capacidad == 0 || w.len() * 4 < bytes_para(capacidad) {
            return None;
        }
        for x in &w[..CABECERA / 4] {
            x.store(0, Ordering::Relaxed);
        }
        w[CAMPO_MAGIA].store(MAGIA, Ordering::Relaxed);
        w[CAMPO_VERSION].store(VERSION, Ordering::Relaxed);
        w[CAMPO_CABECERA].store(CABECERA as u32, Ordering::Relaxed);
        w[CAMPO_CAPACIDAD].store(capacidad as u32, Ordering::Release);
        Some(Lamina { w, capacidad })
    }

    /// **Abrirla** (VERRANO): comprueba la cabecera contra lo prestado.
    pub fn abrir(w: &'a [AtomicU32]) -> Result<Self, NoEs> {
        if w.len() * 4 < CABECERA {
            return Err(NoEs::Corta);
        }
        if w[CAMPO_MAGIA].load(Ordering::Acquire) != MAGIA {
            return Err(NoEs::Magia);
        }
        if w[CAMPO_VERSION].load(Ordering::Relaxed) != VERSION || w[CAMPO_CABECERA].load(Ordering::Relaxed) != CABECERA as u32 {
            return Err(NoEs::Version);
        }
        let capacidad = w[CAMPO_CAPACIDAD].load(Ordering::Relaxed) as usize;
        if capacidad == 0 || capacidad > (w.len() * 4 - CABECERA) / (2 * VERTEX_BYTES) {
            return Err(NoEs::NoCabe);
        }
        Ok(Lamina { w, capacidad })
    }

    pub fn capacidad(&self) -> usize {
        self.capacidad
    }

    /// Donde empieza la ranura `k`, en palabras.
    fn ranura(&self, k: usize) -> usize {
        CABECERA / 4 + k * self.capacidad * PALABRAS_VERTICE
    }

    /// **Publicar un fotograma** (la app): en la ranura que no se lee, y la
    /// secuencia AL FINAL. `false` si no cabe o no son triangulos enteros.
    pub fn publicar(&self, fotograma: u32, v: &[Vertex]) -> bool {
        if v.len() > self.capacidad || v.len() % 3 != 0 {
            return false;
        }
        let s = self.w[CAMPO_SECUENCIA].load(Ordering::Relaxed);
        let k = (s.wrapping_add(1) % 2) as usize;
        let r = self.ranura(k);
        // El sello, IMPAR antes de tocar nada de la ranura.
        let sello = self.w[CAMPO_SELLO + k].load(Ordering::Relaxed);
        self.w[CAMPO_SELLO + k].store(sello.wrapping_add(1), Ordering::Relaxed);
        core::sync::atomic::fence(Ordering::Release);
        for (i, x) in v.iter().enumerate() {
            let p = r + i * PALABRAS_VERTICE;
            for (j, b) in x.position.iter().chain(x.color.iter()).enumerate() {
                self.w[p + j].store(b.to_bits(), Ordering::Relaxed);
            }
        }
        self.w[CAMPO_VERTICES + k].store(v.len() as u32, Ordering::Relaxed);
        self.w[CAMPO_FOTOGRAMA + k].store(fotograma, Ordering::Relaxed);
        // PAR otra vez: la ranura esta entera.
        self.w[CAMPO_SELLO + k].store(sello.wrapping_add(2), Ordering::Release);
        // La publicacion: todo lo de arriba se ve antes que este numero.
        self.w[CAMPO_SECUENCIA].store(s.wrapping_add(1), Ordering::Release);
        true
    }

    /// **Leer el ultimo fotograma publicado** (VERRANO), a `out`.
    pub fn leer(&self, out: &mut [Vertex]) -> Leido {
        let s = self.w[CAMPO_SECUENCIA].load(Ordering::Acquire);
        if s == 0 {
            return Leido::Nada;
        }
        let k = (s % 2) as usize;
        let e1 = self.w[CAMPO_SELLO + k].load(Ordering::Acquire);
        if e1 % 2 != 0 {
            // La app ya esta escribiendo aqui (va por el siguiente).
            return Leido::Rota;
        }
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
        // Lo leido vale si NADIE toco la ranura mientras tanto.
        core::sync::atomic::fence(Ordering::Acquire);
        if self.w[CAMPO_SELLO + k].load(Ordering::Relaxed) != e1 {
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
}
