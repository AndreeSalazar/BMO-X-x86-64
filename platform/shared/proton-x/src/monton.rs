//! **El monton de Windows de un `.exe`** (P4e, 27-09): `HeapAlloc`,
//! `HeapFree`, `HeapReAlloc`, `HeapSize` -- y debajo de `VirtualAlloc`.
//!
//! Un CRT de verdad (`malloc`, `new`, el `Vec` de Rust) pide y suelta
//! MILLONES de veces. El monton del cargador (`apps/proton-x/src/monton.rs`)
//! solo avanza: servido por el, un juego se comeria la memoria en segundos.
//! Este es el otro, el suyo, con su politica, y SIN pedir memoria para
//! apuntar: todo lo que sabe vive DENTRO de los bloques, como en cualquier
//! monton de C.
//!
//! ```text
//!    un bloque              h+0   medida | USADO | PREVIO_USADO   (u64)
//!    (medida: multiplo       h+8   usado: lo pedido | propietario << 48
//!     de 16, cabecera              libre: el siguiente libre
//!     incluida)             h+16  usado: lo del `.exe` (alineado a 16)
//!                                 libre: el anterior libre
//!                           fin-8 libre: la medida otra vez (el PIE)
//!
//!    una arena   [bloque][bloque]...[centinela: medida 0, USADO]
//! ```
//!
//! Pedir es el primero que cabe en la lista de libres (con la alineacion que
//! se pida: 16 para `HeapAlloc`, 64 KiB para `VirtualAlloc`); lo que sobra
//! vuelve a la lista. Soltar FUSIONA con el vecino de delante (su cabecera) y
//! con el de detras (su pie, que solo existe si esta libre: eso dice el bit
//! PREVIO_USADO). Un `.exe` que suelta todo deja la arena como estaba: un
//! bloque libre y el centinela ([`Monton::comprobar`] lo mira entero).
//!
//! El PROPIETARIO (16 bits) dice de que monton de Windows es cada bloque: el del
//! proceso, uno de `HeapCreate` (y `HeapDestroy` suelta todos los suyos de una
//! pasada, [`Monton::soltar_de`]) o `VirtualAlloc`.
//!
//! Aqui no hay punteros: las direcciones son numeros y la memoria se lee y se
//! escribe por [`Palabras`]. La casa la pone sobre la memoria de verdad; el
//! banco, sobre un `Vec`.
//!
//! Lo que no sabe, dicho: un puntero que apunta DENTRO de un bloque (no a su
//! principio) no siempre se distingue de uno bueno -- un monton de C tampoco
//! puede; se rechaza lo que cae fuera de las arenas, lo desalineado y lo que
//! no esta USADO (el doble `HeapFree`). La busqueda recorre la lista: con
//! millones de bloques libres chicos, sera la medida a mirar.

/// La memoria donde vive el monton, de 8 en 8 bytes.
pub trait Palabras {
    fn leer(&self, dir: u64) -> u64;
    fn poner(&mut self, dir: u64, v: u64);
}

const USADO: u64 = 1;
const PREVIO_USADO: u64 = 2;
const BANDERAS: u64 = 15;
/// La cabecera de un bloque: lo que va antes de lo del `.exe`.
pub const CABECERA: u64 = 16;
/// El bloque mas chico: cabecera, dos enlaces... y el pie, que cabe en el
/// segundo enlace (a +24) porque un bloque libre de 32 no necesita mas.
const MINIMO: u64 = 32;
/// Lo pedido ocupa 48 bits; el propietario, los 16 de arriba.
const PEDIDO: u64 = (1 << 48) - 1;

/// El propietario del monton del proceso (`GetProcessHeap`) y el de `VirtualAlloc`.
pub const PROPIETARIO_PROCESO: u16 = 1;
pub const PROPIETARIO_VIRTUAL: u16 = 0xFFFF;

/// **El HANDLE de un monton de Windows**: lo que `GetProcessHeap` y
/// `HeapCreate` devuelven y el PEB guarda en `ProcessHeap`. Un numero que no
/// es un puntero (nadie lo lee por dentro).
pub const fn asa(propietario: u16) -> u64 {
    0x5A1D_0000_0000 | (propietario as u64) << 16
}

/// El propietario de un HANDLE de monton, si lo es.
pub fn propietario_de(asa: u64) -> Option<u16> {
    let d = (asa >> 16) as u16;
    (asa & !0xFFFF_0000 == 0x5A1D_0000_0000 && d != 0 && d != PROPIETARIO_VIRTUAL).then_some(d)
}

/// Las arenas que caben: una por bloque del kernel, que da ocho por proceso.
pub const MAX_ARENAS: usize = 8;

/// Lo que se sabia de un bloque al soltarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloque {
    /// Lo que el `.exe` pidio (lo que `HeapSize` dice).
    pub pedido: u64,
    pub propietario: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monton {
    /// El primero de la lista de libres; 0, ninguno.
    libres: u64,
    arenas: [(u64, u64); MAX_ARENAS],
    n_arenas: usize,
    /// Lo libre, cabeceras incluidas.
    bytes_libres: u64,
}

fn arriba(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

/// Lo que ocupa un bloque para `tam` bytes del `.exe`.
fn ocupa(tam: u64) -> u64 {
    (arriba(tam, 16) + CABECERA).max(MINIMO)
}

fn medida<M: Palabras>(m: &M, h: u64) -> u64 {
    m.leer(h) & !BANDERAS
}

fn usado<M: Palabras>(m: &M, h: u64) -> bool {
    m.leer(h) & USADO != 0
}

fn poner_previo<M: Palabras>(m: &mut M, h: u64, u: bool) {
    let w = m.leer(h);
    m.poner(h, if u { w | PREVIO_USADO } else { w & !PREVIO_USADO });
}

impl Default for Monton {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Monton {
    pub const fn nuevo() -> Self {
        Monton { libres: 0, arenas: [(0, 0); MAX_ARENAS], n_arenas: 0, bytes_libres: 0 }
    }

    pub fn bytes_libres(&self) -> u64 {
        self.bytes_libres
    }

    pub fn arenas(&self) -> &[(u64, u64)] {
        &self.arenas[..self.n_arenas]
    }

    fn quitar<M: Palabras>(&mut self, m: &mut M, h: u64) {
        let (sig, ant) = (m.leer(h + 8), m.leer(h + 16));
        if ant == 0 {
            self.libres = sig;
        } else {
            m.poner(ant + 8, sig);
        }
        if sig != 0 {
            m.poner(sig + 16, ant);
        }
    }

    fn meter<M: Palabras>(&mut self, m: &mut M, h: u64) {
        let sig = self.libres;
        m.poner(h + 8, sig);
        m.poner(h + 16, 0);
        if sig != 0 {
            m.poner(sig + 16, h);
        }
        self.libres = h;
    }

    /// Un bloque libre de `t` en `h`: cabecera, pie, y a la lista.
    fn libre<M: Palabras>(&mut self, m: &mut M, h: u64, t: u64, previo: u64) {
        m.poner(h, t | previo);
        m.poner(h + t - 8, t);
        self.meter(m, h);
    }

    /// **Una arena nueva**: `[base, base + bytes)`, memoria de este proceso
    /// que nadie mas usa. `false` si no caben mas o es demasiado chica.
    pub fn agregar<M: Palabras>(&mut self, m: &mut M, base: u64, bytes: u64) -> bool {
        let b = arriba(base, 16);
        let fin = (base + bytes) & !15;
        if self.n_arenas == MAX_ARENAS || fin < b + MINIMO + CABECERA {
            return false;
        }
        let t = fin - CABECERA - b;
        // El centinela: medida 0, USADO; lo de antes, libre.
        m.poner(fin - CABECERA, USADO);
        m.poner(fin - 8, 0);
        // Antes del primero no hay nada que fusionar: "previo usado".
        self.libre(m, b, t, PREVIO_USADO);
        self.arenas[self.n_arenas] = (b, fin);
        self.n_arenas += 1;
        self.bytes_libres += t;
        true
    }

    /// **Pedir** `tam` bytes alineados a `alin` (potencia de dos; menos de 16
    /// cuenta como 16), para `propietario`. La direccion, o `None` si no cabe.
    pub fn pedir<M: Palabras>(&mut self, m: &mut M, tam: u64, alin: u64, propietario: u16) -> Option<u64> {
        if tam > PEDIDO || !alin.is_power_of_two() {
            return None;
        }
        let alin = alin.max(16);
        let hace_falta = ocupa(tam);
        let mut h = self.libres;
        while h != 0 {
            let t = medida(m, h);
            let mut p = arriba(h + CABECERA, alin);
            // El hueco de delante, si lo hay, tiene que ser un bloque libre.
            if p - CABECERA != h && p - CABECERA - h < MINIMO {
                p = arriba(h + CABECERA + MINIMO, alin);
            }
            let nh = p - CABECERA;
            if nh + hace_falta <= h + t {
                self.cortar(m, h, t, nh, hace_falta);
                m.poner(nh + 8, tam | (propietario as u64) << 48);
                return Some(p);
            }
            h = m.leer(h + 8);
        }
        None
    }

    /// Sacar de la libre `h` (de `t`) un usado en `nh` de `hace_falta`.
    fn cortar<M: Palabras>(&mut self, m: &mut M, h: u64, t: u64, nh: u64, hace_falta: u64) {
        self.quitar(m, h);
        let mut previo = m.leer(h) & PREVIO_USADO;
        if nh != h {
            self.libre(m, h, nh - h, previo);
            previo = 0;
        }
        let hay = h + t - nh;
        let usa = if hay - hace_falta >= MINIMO {
            self.libre(m, nh + hace_falta, hay - hace_falta, PREVIO_USADO);
            hace_falta
        } else {
            poner_previo(m, nh + hay, true);
            hay
        };
        m.poner(nh, usa | USADO | previo);
        self.bytes_libres -= usa;
    }

    /// La cabecera de `p` si `p` es el principio de un bloque usado.
    fn cabecera<M: Palabras>(&self, m: &M, p: u64) -> Option<u64> {
        if p % 16 != 0 || !self.arenas().iter().any(|&(b, fin)| p >= b + CABECERA && p < fin) {
            return None;
        }
        let h = p - CABECERA;
        let t = medida(m, h);
        let dentro = self.arenas().iter().any(|&(b, fin)| h >= b && h + t <= fin - CABECERA);
        (usado(m, h) && t >= MINIMO && dentro).then_some(h)
    }

    /// Lo que se sabe de `p`, si es un bloque usado.
    pub fn bloque<M: Palabras>(&self, m: &M, p: u64) -> Option<Bloque> {
        let w = m.leer(self.cabecera(m, p)? + 8);
        Some(Bloque { pedido: w & PEDIDO, propietario: (w >> 48) as u16 })
    }

    /// **Soltar** `p`. `None` si no es un bloque usado (fuera de las arenas,
    /// desalineado, o ya suelto).
    pub fn soltar<M: Palabras>(&mut self, m: &mut M, p: u64) -> Option<Bloque> {
        let b = self.bloque(m, p)?;
        self.liberar(m, p - CABECERA);
        Some(b)
    }

    /// Liberar el usado `h`, fusionado con sus vecinos libres. Devuelve el
    /// bloque libre que queda (principio y medida).
    fn liberar<M: Palabras>(&mut self, m: &mut M, h: u64) -> (u64, u64) {
        let mut t = medida(m, h);
        let mut previo = m.leer(h) & PREVIO_USADO;
        self.bytes_libres += t;
        let mut h = h;
        let sig = h + t;
        // El centinela es USADO: nunca se fusiona con el.
        if !usado(m, sig) {
            t += medida(m, sig);
            self.quitar(m, sig);
        }
        if previo == 0 {
            let tp = m.leer(h - 8);
            h -= tp;
            t += tp;
            self.quitar(m, h);
            previo = m.leer(h) & PREVIO_USADO;
        }
        self.libre(m, h, t, previo);
        poner_previo(m, h + t, false);
        (h, t)
    }

    /// **Cambiar la medida EN SU SITIO** (`HeapReAlloc`): encoger siempre se
    /// puede; crecer, si el de delante esta libre y basta. `false` y nada
    /// tocado si no.
    pub fn cambiar_en_sitio<M: Palabras>(&mut self, m: &mut M, p: u64, tam: u64) -> bool {
        let Some(h) = self.cabecera(m, p) else { return false };
        if tam > PEDIDO {
            return false;
        }
        let hace_falta = ocupa(tam);
        let t = medida(m, h);
        let propietario = m.leer(h + 8) >> 48;
        let banderas = m.leer(h) & BANDERAS;
        let tiene = if hace_falta <= t {
            t
        } else {
            let sig = h + t;
            if usado(m, sig) || t + medida(m, sig) < hace_falta {
                return false;
            }
            let ts = medida(m, sig);
            self.quitar(m, sig);
            self.bytes_libres -= ts;
            poner_previo(m, sig + ts, true);
            t + ts
        };
        if tiene - hace_falta >= MINIMO {
            // Lo que sobra se marca usado un momento y se libera: asi se
            // fusiona con lo de delante como cualquier otro.
            m.poner(h, hace_falta | banderas);
            let f = h + hace_falta;
            m.poner(f, (tiene - hace_falta) | USADO | PREVIO_USADO);
            self.liberar(m, f);
        } else {
            m.poner(h, tiene | banderas);
        }
        m.poner(h + 8, tam | propietario << 48);
        true
    }

    /// **Soltar todo lo de `propietario`** (`HeapDestroy`). Cuantos bloques.
    pub fn soltar_de<M: Palabras>(&mut self, m: &mut M, propietario: u16) -> u32 {
        let mut n = 0;
        for i in 0..self.n_arenas {
            let (b, fin) = self.arenas[i];
            let mut h = b;
            while h < fin - CABECERA {
                let w = m.leer(h);
                if w & USADO != 0 && (m.leer(h + 8) >> 48) as u16 == propietario {
                    let (nh, nt) = self.liberar(m, h);
                    n += 1;
                    h = nh + nt;
                } else {
                    h += w & !BANDERAS;
                }
            }
        }
        n
    }

    /// **Mirarlo entero** (el banco): cada arena se recorre de punta a punta
    /// y cuadra con la lista de libres y con `bytes_libres`.
    pub fn comprobar<M: Palabras>(&self, m: &M) -> Result<(), &'static str> {
        let mut libres_en_arenas = 0u64;
        let mut n_libres = 0u64;
        for &(b, fin) in self.arenas() {
            let mut h = b;
            let mut previo_usado = true;
            while h < fin - CABECERA {
                let w = m.leer(h);
                let t = w & !BANDERAS;
                if t < MINIMO || t % 16 != 0 || h + t > fin - CABECERA {
                    return Err("una medida que no es");
                }
                if (w & PREVIO_USADO != 0) != previo_usado {
                    return Err("PREVIO_USADO no dice la verdad");
                }
                let u = w & USADO != 0;
                if !u {
                    if !previo_usado {
                        return Err("dos libres seguidos sin fusionar");
                    }
                    if m.leer(h + t - 8) != t {
                        return Err("el pie de un libre no es su medida");
                    }
                    libres_en_arenas += t;
                    n_libres += 1;
                }
                previo_usado = u;
                h += t;
            }
            if h != fin - CABECERA || m.leer(h) & !PREVIO_USADO != USADO {
                return Err("el centinela no esta donde va");
            }
            if (m.leer(h) & PREVIO_USADO != 0) != previo_usado {
                return Err("el centinela no sabe si lo de antes esta libre");
            }
        }
        let (mut h, mut ant, mut en_lista, mut n) = (self.libres, 0, 0u64, 0u64);
        while h != 0 {
            if usado(m, h) || m.leer(h + 16) != ant {
                return Err("la lista de libres esta rota");
            }
            en_lista += medida(m, h);
            n += 1;
            if n > n_libres {
                return Err("la lista de libres tiene de mas");
            }
            ant = h;
            h = m.leer(h + 8);
        }
        if n != n_libres || en_lista != libres_en_arenas || en_lista != self.bytes_libres {
            return Err("lo libre no cuadra");
        }
        Ok(())
    }
}
