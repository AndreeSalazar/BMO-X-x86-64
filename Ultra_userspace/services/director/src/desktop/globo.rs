//! **EL GLOBO DEL PUNTERO: CUANDO SALE Y QUE DICE** (2026-09-25). Peticion
//! del propietario: *"mi puntero siempre aparezca animacion de comentario
//! simple o recomendacion ... como 20 segundos ... GPU que falta prender o
//! curiosidad"*.
//!
//! [consumo] LATE      mientras vive un globo pide ~30 fotogramas por segundo
//!                     ([`anima`], que solo mira el reloj: `rdtsc`, sin
//!                     puerta); al NACER, unas pocas preguntas al kernel --
//!                     una vez por minuto, no por fotograma. Sin globo, nada
//!                     (L6h)
//!
//! La cara la pinta `scene::globo`; esto la mueve. Nace cuando el raton se
//! MUEVE (asi sale donde se esta mirando) y ya paso la pausa; vive
//! [`DURA_MS`] siguiendo al puntero, y se va. Los temas se turnan: lo que pasa
//! con la 3060 ahora mismo, y un dato o un atajo de la casa.

use bmo_userland as bmo;

use crate::desktop::Desktop;
use crate::scene::globo::{self, Cara, LETRAS};
pub(crate) use crate::scene::globo::Tono;

/// Lo que vive un globo.
pub(crate) const DURA_MS: u64 = 20_000;
/// Entre uno y el siguiente.
const PAUSA_MS: u64 = 45_000;
/// El primero, tras arrancar el escritorio.
const PRIMERO_MS: u64 = 3_000;
/// Un fotograma de la animacion: ~30 por segundo.
const FOTOGRAMA_MS: u64 = 33;

/// Los datos y atajos, por turno. Cada uno tiene que ser VERDAD en esta casa:
/// una orden que cambie de nombre se cambia aqui.
const SABIAS: &[(&[u8], &[u8])] = &[
    (b"sabias", b"el fractal en la 3060 sale unas 220 veces mas rapido que en la CPU"),
    (b"atajo", b"Ctrl+F busca en la salida; cada Enter, la coincidencia anterior"),
    (b"sabias", b"`gpu pantalla`: la 3060 pinta tu monitor entero, 400 fotogramas"),
    (b"atajo", b"TAB completa la orden; las flechas eligen entre las sugerencias"),
    (b"sabias", b"el GSP es un RISC-V dentro de la 3060: ahi corre su firmware 570.144"),
    (b"atajo", b"`save` escribe el INFORME MAESTRO en datos/salida.txt"),
    (b"sabias", b"`reboot` apaga el GSP en orden: el arranque siguiente sale limpio"),
    (b"atajo", b"Ctrl+Alt: el consejero dice el siguiente paso"),
    (b"sabias", b"la IOMMU: la 3060 solo ve la RAM que BMO-X le presta"),
    (b"atajo", b"Ctrl+Shift+S recorta un trozo de pantalla con el raton"),
    (b"sabias", b"`gpu giro`: una esfera que gira y bota, dibujada por la 3060"),
    (b"atajo", b"`gpu` a secas: cada fila de la verificacion de la 3060"),
];

struct Estado {
    /// Ciclos del TSC por ms; 0 hasta la primera vez.
    por_ms: u64,
    /// Cuando nacio el que vive, o 0.
    desde: u64,
    /// Antes de esto no nace otro.
    siguiente: u64,
    /// El ultimo fotograma pintado con el globo, para [`anima`].
    pintado: u64,
    turno: u32,
    /// Donde estaba el raton la ultima vez que se miro.
    raton: (u32, u32),
    /// Un AVISO por nacer (`avisar`): pasa delante del turno y de la pausa.
    aviso: bool,
    tono: Tono,
    /// Si el volcado lo hacia la 3060 la ultima vez que se miro.
    gpu_antes: bool,
    titulo: [u8; 16],
    tn: usize,
    texto: [u8; LETRAS],
    n: usize,
}

static mut ESTADO: Estado = Estado {
    por_ms: 0,
    desde: 0,
    siguiente: 0,
    pintado: 0,
    aviso: false,
    tono: Tono::Consejo,
    gpu_antes: false,
    turno: 0,
    raton: (u32::MAX, u32::MAX),
    titulo: [0; 16],
    tn: 0,
    texto: [0; LETRAS],
    n: 0,
};

fn estado() -> &'static mut Estado {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde el compositor.
    unsafe { &mut *core::ptr::addr_of_mut!(ESTADO) }
}

/// Un texto que se escribe por trozos, cortado a lo que cabe.
struct Linea<'a> {
    b: &'a mut [u8],
    n: usize,
}

impl Linea<'_> {
    fn t(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }

    fn d(&mut self, v: u64) -> &mut Self {
        let mut dig = [0u8; 20];
        let (mut v, mut k) = (v, dig.len());
        loop {
            k -= 1;
            dig[k] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.t(&dig[k..])
    }
}

/// **Lo que dice la 3060 ahora**, si hay algo que decir.
fn de_la_3060(e: &mut Estado, turno: u32) -> bool {
    if bmo::info(bmo::INFO_GPU_CHIP) & bmo::GPU_HALLADA == 0 {
        return false;
    }
    let mut l = Linea { b: &mut e.texto, n: 0 };
    let titulo: &[u8] = if !crate::commands::gsp::despierto() {
        l.t(b"esta dormida: `save mode` la despierta y la prueba entera");
        b"LA 3060"
    } else if turno % 4 == 0 {
        // El consejero, en una linea: el paso que falta, o que esta todo.
        let mut t = [0u8; 160];
        let (etiqueta, n) = crate::commands::verificar::pista(&mut t);
        l.t(&t[..n]);
        etiqueta
    } else {
        match bmo_gpu_ga10x::salud::lectura(bmo::info(bmo::INFO_GPU_SALUD) as u32) {
            Some((g, _)) => l.t(b"despierta, a ").d(g as u64).t(b" grados; `gpu salud` dice el resto"),
            None => l.t(b"despierta; `gpu` muestra cada fila de la verificacion"),
        };
        b"LA 3060"
    };
    e.n = l.n;
    let mut l = Linea { b: &mut e.titulo, n: 0 };
    l.t(titulo);
    e.tn = l.n;
    true
}

/// **Un AVISO**: algo acaba de pasar y se dice YA, junto al puntero, aunque
/// haya otro globo vivo o se este en la pausa. Lo llaman las ordenes (save
/// mode al acabar, el volcado por la 3060...).
pub(crate) fn avisar(titulo: &[u8], texto: &[u8], tono: Tono) {
    let e = estado();
    let mut l = Linea { b: &mut e.titulo, n: 0 };
    l.t(titulo);
    e.tn = l.n;
    let mut l = Linea { b: &mut e.texto, n: 0 };
    l.t(texto);
    e.n = l.n;
    e.tono = tono;
    e.aviso = true;
}

fn nacer(e: &mut Estado, ahora: u64) {
    e.tono = Tono::Consejo;
    let turno = e.turno;
    e.turno = e.turno.wrapping_add(1);
    if turno % 2 == 1 || !de_la_3060(e, turno / 2) {
        let (titulo, texto) = SABIAS[(turno / 2) as usize % SABIAS.len()];
        let mut l = Linea { b: &mut e.titulo, n: 0 };
        l.t(titulo);
        e.tn = l.n;
        let mut l = Linea { b: &mut e.texto, n: 0 };
        l.t(texto);
        e.n = l.n;
    }
    e.desde = ahora;
}

/// **Quita las capas de encima**, al PRINCIPIO del fotograma y despues del
/// cursor: la del recorte y el globo, al reves de como se pusieron.
pub(crate) fn quitar_capas(p: &bmo::Pantalla) {
    crate::desktop::captura::capa_quitar(p);
    globo::quitar(p);
    crate::scene::brillo::quitar(p);
}

/// **El globo de este fotograma**: nace, sigue al raton o se va. Al FINAL del
/// fotograma que pinta, ANTES de la capa del recorte y del cursor. `tapado`:
/// una ventana a pantalla completa (ahi no se habla).
pub(crate) fn poner(dsk: &Desktop, p: &bmo::Pantalla, tapado: bool) {
    // Si la 3060 dejo de volcar (una tanda fallo, o `gpu volcado off`), se dice.
    let gpu = p.volcando_por_gpu();
    if estado().gpu_antes && !gpu {
        avisar(b"volcado", b"vuelve a la CPU: la 3060 ya no lleva el escritorio", Tono::Mal);
    }
    let e = estado();
    e.gpu_antes = gpu;
    let ahora = bmo::ciclos();
    if e.por_ms == 0 {
        e.por_ms = (bmo::info(bmo::INFO_TSC_HZ) / 1000).max(1);
        e.siguiente = ahora + PRIMERO_MS * e.por_ms;
    }
    if e.aviso {
        e.aviso = false;
        e.desde = ahora;
    }
    let (ax, ay) = (dsk.tick.ax, dsk.tick.ay);
    let movido = (ax, ay) != e.raton;
    e.raton = (ax, ay);
    if ax == u32::MAX || tapado {
        return;
    }
    if e.desde == 0 {
        if !movido || ahora < e.siguiente {
            return;
        }
        nacer(e, ahora);
    }
    let ms = ahora.wrapping_sub(e.desde) / e.por_ms;
    if ms >= DURA_MS {
        e.desde = 0;
        e.siguiente = ahora + PAUSA_MS * e.por_ms;
        return;
    }
    e.pintado = ahora;
    let cara = Cara { titulo: &e.titulo[..e.tn], texto: &e.texto[..e.n], edad_ms: ms, vida_ms: DURA_MS, tono: e.tono };
    globo::poner(p, ax, ay, &cara);
}

/// **Pide fotograma** mientras vive un globo: uno cada [`FOTOGRAMA_MS`], para
/// que la animacion no vaya a saltos de cuarto de segundo. Lo pregunta el
/// bucle en cada vuelta: solo lee el reloj.
pub(crate) fn anima() -> bool {
    let e = estado();
    e.aviso || e.desde != 0 && bmo::ciclos().wrapping_sub(e.pintado) >= FOTOGRAMA_MS * e.por_ms
}
