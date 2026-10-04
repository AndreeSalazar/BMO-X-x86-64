//! **LA BARRA TACTICA** -- la columna de la DERECHA del MODO FASE (04-10).
//!
//! [consumo] LATE      4 muestras por segundo en FASE; ~1 s de animacion al
//!                     transformarse; fuera de FASE, NADA (L6h)
//!
//! El propietario: *"mi BMO-X puede tener sus 2 barras laterales, con
//! interfaz elegante pero transformers e intimidante"*, y despues: *"con color
//! de acuerdo al gato que tengo, pero con glitch"*. Es su MODO CABINA: el
//! escritorio se transforma y a la derecha se ARMA una columna de placas
//! blindadas con lo que el hierro dice de verdad.
//!
//! ```text
//!    la espina      una linea cian que baja por el borde (lo primero)
//!    las placas     llegan volando desde la derecha, una tras otra, con
//!                   rebote; mientras vuelan, dos fantasmas (magenta y cian)
//!                   corridos: el GLITCH. Al aterrizar, se encienden
//!    lo que dicen   el nucleo (GHz medidos con APERF/MPERF propios), la
//!                   potencia, la memoria, los programas vivos, la red y el
//!                   tiempo encendido
//! ```
//!
//! Los colores son los del gato del fondo (`tema.maqueta`, `.fase`): el CIAN
//! de su holograma y el MAGENTA del neon. La columna es RESERVADA como la del
//! panel izquierdo (`margen`, que lee `chrome::area_util`): ninguna ventana
//! entra, y repintar a 4 Hz no puede pintar encima de una.

use bmo_dibujo::Recorte;
use bmo_userland as bmo;

use super::estilo;
use super::fino;
use super::tema_gen::{FASE_BORDE, FASE_CIAN, FASE_FONDO, FASE_NEON, FASE_TENUE, FASE_TINTA};
use super::vida::{self, Paso, REBOTE};

/// El ancho de la columna (el del panel izquierdo, y un poco mas: los
/// numeros grandes).
pub(crate) const ANCHO: u32 = 168;

const PLACAS: usize = 6;
const PLACA_H: u32 = 72;
const ENTRE: u32 = 10;
const CABEZA: u32 = 54;
const DENTRO: u32 = 10;

/// Cuando entra cada placa (ms desde que empieza) y cuanto vuela.
const ESCALON_MS: u64 = 80;
const VUELO_MS: u64 = 420;
const ESPINA_MS: u64 = 220;
const ENCENDIDO_MS: u64 = 160;
const TODO_MS: u32 = (ESPINA_MS + ESCALON_MS * PLACAS as u64 + VUELO_MS + ENCENDIDO_MS) as u32;

struct Estado {
    inicio: u64,
    paso: Paso,
    movia: bool,
    forzar: bool,
    proximo: u64,
    visto: bool,
    // Lo medido, para la proxima diferencia.
    aperf: u64,
    mperf: u64,
    ghz_cent: u32,
    mw: u64,
}

static mut ESTADO: Estado = Estado {
    inicio: 0,
    paso: Paso::QUIETO,
    movia: false,
    forzar: true,
    proximo: 0,
    visto: false,
    aperf: 0,
    mperf: 0,
    ghz_cent: 0,
    mw: 0,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo lo toca el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// Esta la barra? Solo en el MODO FASE.
pub(crate) fn visible() -> bool {
    estilo::fase()
}

/// **Lo que la columna le quita a las ventanas**, por la derecha.
pub(crate) fn margen() -> u32 {
    if visible() {
        super::lateral::hueco() + ANCHO
    } else {
        0
    }
}

/// `(x, y, ancho, alto)` de la barra en una pantalla de `ancho x alto`.
pub(crate) fn caja(ancho: u32, alto: u32) -> (u32, u32, u32, u32) {
    let h = super::lateral::hueco();
    (ancho.saturating_sub(h + ANCHO), h, ANCHO, alto.saturating_sub(2 * h))
}

/// **El escritorio se transforma**: la barra se arma desde cero.
pub(crate) fn transformar() {
    let e = estado();
    e.inicio = vida::ahora_ms();
    e.paso = Paso::empezar(TODO_MS);
    e.movia = true;
    e.forzar = true;
}

/// Lo pintado ya no esta (se repinto el escritorio): la proxima vuelta, entera.
pub(crate) fn olvidar() {
    estado().forzar = true;
}

/// **Un fotograma del compositor.** `mw` son los milivatios del paquete que
/// ya leyo el escritorio para el panel (preguntarlos otra vez se los robaria).
pub(crate) fn latido(p: &bmo::Pantalla, mw: Option<u64>) {
    if !visible() {
        estado().visto = false;
        return;
    }
    let e = estado();
    if let Some(m) = mw {
        e.mw = m;
    }
    // La primera vez que se ve (al arrancar en FASE), tambien se ARMA.
    if !e.visto {
        e.visto = true;
        transformar();
    }
    let ahora = vida::ahora_ms();
    let vivo = e.paso.vivo();
    if vivo || e.movia {
        e.movia = vivo;
        if !vivo {
            medir(e);
        }
        pintar(p, if vivo { Some(ahora.saturating_sub(e.inicio)) } else { None });
        return;
    }
    if e.forzar || ahora >= e.proximo {
        e.forzar = false;
        e.proximo = ahora + 250;
        medir(e);
        pintar(p, None);
    }
}

/// Lo que cambia solo: la frecuencia, por diferencia de APERF/MPERF (los
/// PROPIOS: `INFO_CPU_HZ_REAL` es "desde que alguien pregunto", y preguntar
/// aqui descuadraria a VITALES).
fn medir(e: &mut Estado) {
    let (a, m) = (bmo::info(bmo::INFO_CPU_APERF), bmo::info(bmo::INFO_CPU_MPERF));
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    let (da, dm) = (a.wrapping_sub(e.aperf), m.wrapping_sub(e.mperf));
    if e.mperf != 0 && dm > 0 && hz > 0 {
        e.ghz_cent = (hz as u128 * da as u128 / dm as u128 / 10_000_000) as u32;
    }
    e.aperf = a;
    e.mperf = m;
}

// -- lo que dicen las placas --------------------------------------------------

/// Un texto chico sin `alloc`.
struct Texto {
    b: [u8; 24],
    n: usize,
}

impl Texto {
    fn nuevo() -> Texto {
        Texto { b: [0; 24], n: 0 }
    }
    fn s(mut self, s: &[u8]) -> Texto {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }
    fn num(self, v: u64) -> Texto {
        let mut d = [0u8; 20];
        let (mut k, mut v) = (0, v);
        loop {
            d[k] = b'0' + (v % 10) as u8;
            k += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        d[..k].reverse();
        self.s(&d[..k])
    }
    /// `v` centesimas como `1.23`.
    fn cent(self, v: u64) -> Texto {
        let t = self.num(v / 100).s(b".");
        let c = v % 100;
        t.num(c / 10).num(c % 10)
    }
    fn dos(self, v: u64) -> Texto {
        self.num(v / 10).num(v % 10)
    }
    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

/// Una placa: su rotulo, lo grande, lo chico y cuanto se llena (milesimas,
/// o nada si no es una medida con tope).
struct Placa {
    rotulo: &'static [u8],
    valor: Texto,
    nota: Texto,
    lleno: Option<u32>,
}

fn placas() -> [Placa; PLACAS] {
    let e = estado();
    let ghz = if e.ghz_cent > 0 { Texto::nuevo().cent(e.ghz_cent as u64).s(b" GHz") } else { Texto::nuevo().s(b"-- GHz") };
    let l3 = bmo::info(bmo::INFO_CPU_CACHE_L3);
    let nota_l3 = if l3 >> 63 != 0 { Texto::nuevo().s(b"L3 ").num((l3 & 0xFF_FFFF) / 1024).s(b" MiB") } else { Texto::nuevo().s(b"medido, no dicho") };
    let vatios = if e.mw > 0 { Texto::nuevo().cent(e.mw / 10).s(b" W") } else { Texto::nuevo().s(b"-- W") };
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let usada = total.saturating_sub(bmo::info(bmo::INFO_RAM_LIBRE));
    let mib = |b: u64| b / (1024 * 1024);
    let mut vivos = 0u64;
    for k in 0..64u64 {
        if bmo::info(bmo::INFO_PROG_QUIEN | (k << 8)) == 0 {
            break;
        }
        vivos += 1;
    }
    let red = bmo::info(bmo::INFO_NET_PRESENTE) != 0;
    let (valor_red, nota_red) = if red {
        (Texto::nuevo().num(bmo::info(bmo::INFO_NET_MEGABITS)).s(b" Mbps"), Texto::nuevo().s(b"tramas ").num(bmo::info(bmo::INFO_NET_RX_TRAMAS)))
    } else {
        (Texto::nuevo().s(b"sin red"), Texto::nuevo().s(b"la tarjeta no esta"))
    };
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
    let s = bmo::ciclos() / hz;
    [
        Placa { rotulo: b"nucleo", valor: ghz, nota: nota_l3, lleno: Some((e.ghz_cent * 1000 / 470).min(1000)) },
        Placa { rotulo: b"potencia", valor: vatios, nota: Texto::nuevo().s(b"el paquete entero"), lleno: Some(((e.mw / 76) as u32).min(1000)) },
        Placa {
            rotulo: b"memoria",
            valor: Texto::nuevo().num(mib(usada)).s(b" MiB"),
            nota: Texto::nuevo().s(b"de ").num(mib(total)).s(b" MiB"),
            lleno: Some(if total > 0 { (usada * 1000 / total) as u32 } else { 0 }),
        },
        Placa { rotulo: b"programas", valor: Texto::nuevo().num(vivos), nota: Texto::nuevo().s(b"vivos en el kernel"), lleno: None },
        Placa { rotulo: b"red", valor: valor_red, nota: nota_red, lleno: None },
        Placa {
            rotulo: b"encendido",
            valor: Texto::nuevo().num(s / 3600).s(b":").dos(s / 60 % 60).s(b":").dos(s % 60),
            nota: Texto::nuevo().s(b"desde el arranque"),
            lleno: None,
        },
    ]
}

// -- el dibujo -----------------------------------------------------------------

/// **Pinta la barra** a los `ms` de transformarse (o quieta, con `None`).
fn pintar(p: &bmo::Pantalla, ms: Option<u64>) {
    let (x, y, w, h) = caja(p.ancho, p.alto);
    if w == 0 || h < CABEZA + PLACA_H {
        return;
    }
    let lim = Recorte::nuevo(x as i32, y as i32, w as i32, h as i32);
    let (xi, yi, wi, hi) = (x as i32, y as i32, w as i32, h as i32);
    p.pieza(&bmo::Pieza::Caja { x: xi, y: yi, w: wi, h: hi, r: 10, c: FASE_FONDO }, 0, 0, None);
    p.pieza(&bmo::Pieza::Borde { x: xi, y: yi, w: wi, h: hi, r: 10, grosor: 1, c: FASE_BORDE }, 0, 0, None);
    let t = ms.unwrap_or(u64::MAX);

    // La espina: baja por el borde izquierdo de la barra.
    let espina = if t >= ESPINA_MS { hi - 20 } else { (hi - 20) * t as i32 / ESPINA_MS as i32 };
    p.pieza(&bmo::Pieza::Caja { x: xi + 3, y: yi + 10, w: 2, h: espina.max(0), r: 1, c: FASE_CIAN }, 0, 0, Some(lim));

    // La cabeza: el nombre, con glitch mientras se arma.
    let glitch = ms.is_some_and(|t| t < ESPINA_MS + 300 && (t / 45) % 3 != 0);
    let rot = |c: u32, dx: i32| {
        p.letra(xi + DENTRO as i32 + 4 + dx, yi + 26, b"MODO FASE", c, fino::ROTULO);
    };
    if glitch {
        rot(FASE_NEON, 2);
        rot(FASE_CIAN, -2);
    }
    rot(FASE_CIAN, 0);
    p.letra(xi + DENTRO as i32 + 4, yi + 42, b"estado tactico", FASE_TENUE, bmo::Estilo::normal(11));

    // Las placas.
    let datos = placas();
    for (k, placa) in datos.iter().enumerate() {
        let py = yi + CABEZA as i32 + k as i32 * (PLACA_H + ENTRE) as i32;
        if py + PLACA_H as i32 > yi + hi - 6 {
            break;
        }
        let empieza = ESPINA_MS + k as u64 * ESCALON_MS;
        let vuelo = t.saturating_sub(empieza);
        if t < empieza {
            continue;
        }
        let kk = if vuelo >= VUELO_MS { 1000 } else { bmo::avance(vuelo as u32, 0, VUELO_MS as u32, REBOTE) };
        let px = xi + 8 + vida::entre(wi + 24, 0, kk);
        let (pw, ph) = (wi - 16, PLACA_H as i32);
        let volando = vuelo < VUELO_MS;
        // El glitch: dos fantasmas corridos mientras vuela.
        if volando && (vuelo / 40 + k as u64) % 3 != 0 {
            p.pieza(&bmo::Pieza::Borde { x: px + 3, y: py - 1, w: pw, h: ph, r: 6, grosor: 1, c: FASE_NEON }, 0, 0, Some(lim));
            p.pieza(&bmo::Pieza::Borde { x: px - 3, y: py + 1, w: pw, h: ph, r: 6, grosor: 1, c: FASE_CIAN }, 0, 0, Some(lim));
        }
        placa_marco(p, px, py, pw, ph, lim, vuelo < VUELO_MS + ENCENDIDO_MS && !volando);
        if !volando {
            placa_texto(p, px, py, pw, placa, vuelo.saturating_sub(VUELO_MS));
        }
    }
}

/// El cuerpo de una placa: su caja, su filo y las escuadras cian en dos
/// esquinas (el chasis). Recien aterrizada, el filo ARDE un instante.
fn placa_marco(p: &bmo::Pantalla, x: i32, y: i32, w: i32, h: i32, lim: Recorte, arde: bool) {
    let caja = |x: i32, y: i32, w: i32, h: i32, r: i32, c: u32| p.pieza(&bmo::Pieza::Caja { x, y, w, h, r, c }, 0, 0, Some(lim));
    caja(x, y, w, h, 6, bmo::entre_color(FASE_FONDO, FASE_BORDE, 90));
    p.pieza(&bmo::Pieza::Borde { x, y, w, h, r: 6, grosor: 1, c: if arde { FASE_CIAN } else { FASE_BORDE } }, 0, 0, Some(lim));
    // Las escuadras: arriba a la izquierda y abajo a la derecha.
    caja(x - 1, y - 1, 12, 2, 0, FASE_CIAN);
    caja(x - 1, y - 1, 2, 12, 0, FASE_CIAN);
    caja(x + w - 11, y + h - 1, 12, 2, 0, FASE_CIAN);
    caja(x + w - 1, y + h - 11, 2, 12, 0, FASE_CIAN);
}

/// Lo de dentro de una placa. `encendida` ms desde que aterrizo: los primeros,
/// el valor parpadea (arranca, como un instrumento).
fn placa_texto(p: &bmo::Pantalla, x: i32, y: i32, w: i32, placa: &Placa, encendida: u64) {
    let d = DENTRO as i32;
    let arranca = encendida < ENCENDIDO_MS && (encendida / 40) % 2 == 0;
    p.letra_en_caja(x + d, y + 8, 14, placa.rotulo, FASE_CIAN, bmo::Estilo::media(10).espaciado(160).mayusculas());
    let valor = if arranca { FASE_NEON } else { FASE_TINTA };
    p.letra_en_caja(x + d, y + 24, 22, placa.valor.bytes(), valor, bmo::Estilo::media(18));
    p.letra_en_caja(x + d, y + 48, 14, placa.nota.bytes(), FASE_TENUE, bmo::Estilo::normal(11));
    if let Some(lleno) = placa.lleno {
        let (bx, by, bw) = (x + d, y + 64, w - 2 * d);
        p.caja_redonda(bx, by, bw, 3, 1, FASE_BORDE);
        let n = bw * lleno.min(1000) as i32 / 1000;
        if n > 0 {
            p.caja_redonda(bx, by, n, 3, 1, FASE_CIAN);
            // El filo del glitch: un pelo de magenta donde acaba.
            p.caja_redonda(bx + n - 1, by - 1, 2, 5, 1, FASE_NEON);
        }
    }
}
