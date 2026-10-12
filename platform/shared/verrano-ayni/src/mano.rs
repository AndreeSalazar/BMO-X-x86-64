//! **LA 5D: la mano del jugador.**
//!
//! > El propietario (12-10): *"la CPU ya sabe que quiere el jugador ...
//! > cuando apunta con el puntero se crea una posibilidad propia, neuronal,
//! > que acepta o no, y eso ahorra el consumo en tiempo real"*.
//!
//! BMO-X ve el raton ANTES que el juego (el kernel lo recibe y luego se lo
//! da). Con eso, aqui se predice **donde va a parar el puntero** unos
//! fotogramas hacia delante, y con que **confianza** (la "posibilidad que
//! acepta o no"): la mano tiene inercia, y un puntero que frena se para a
//! la distancia que su frenada dice (`v^2 / 2a`).
//!
//! **La regla de oro: predecir para AHORRAR o PREPARAR, nunca para ACTUAR.**
//! Esta funcion recibe la historia PRESTADA y sin `mut`: no puede mover el
//! puntero, ni hacer un clic, ni apuntar por nadie. Si se equivoca, la
//! decision de `ayni` cuesta lo de siempre: nunca peor imagen, solo menos
//! ahorro.

/// Una muestra del puntero: el instante y la posicion, en pixeles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Muestra {
    pub t_us: u64,
    pub x: f32,
    pub y: f32,
}

/// Lo que se predice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediccion {
    /// Donde estara (o se parara) el puntero.
    pub x: f32,
    pub y: f32,
    /// En `[0, 1]`: por debajo de [`CONFIANZA_MIN`], no se usa.
    pub confianza: f32,
    /// La rapidez de la mano en `[0, 1]` (1 = dos anchos de pantalla por
    /// segundo o mas): una mano rapida emborrona la pantalla al ojo.
    pub rapidez: f32,
}

/// Por debajo de esto, la prediccion no se usa: no se hace nada.
pub const CONFIANZA_MIN: f32 = 0.6;
/// Por debajo de esto (pixeles por segundo), la mano esta quieta.
const QUIETA: f32 = 5.0;

/// Una raiz cuadrada sin `libm` (Newton desde arriba), para `no_std`.
pub fn raiz(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let mut r = if v > 1.0 { v } else { 1.0 };
    for _ in 0..40 {
        let s = 0.5 * (r + v / r);
        if s >= r {
            break;
        }
        r = s;
    }
    r
}

/// **Donde va a estar el puntero** dentro de `adelante_us`. Hacen falta 3
/// muestras (dos velocidades: la de antes y la de ahora).
pub fn predecir(h: &[Muestra], adelante_us: u64, ancho: f32) -> Option<Prediccion> {
    let n = h.len();
    if n < 3 {
        return None;
    }
    let (a, b, c) = (h[n - 3], h[n - 2], h[n - 1]);
    let dt0 = (b.t_us.checked_sub(a.t_us)? as f32) / 1e6;
    let dt1 = (c.t_us.checked_sub(b.t_us)? as f32) / 1e6;
    if dt0 <= 0.0 || dt1 <= 0.0 {
        return None;
    }
    let (v0x, v0y) = ((b.x - a.x) / dt0, (b.y - a.y) / dt0);
    let (v1x, v1y) = ((c.x - b.x) / dt1, (c.y - b.y) / dt1);
    let s0 = raiz(v0x * v0x + v0y * v0y);
    let s1 = raiz(v1x * v1x + v1y * v1y);
    let rapidez = (s1 / (2.0 * ancho)).clamp(0.0, 1.0);
    if s1 < QUIETA && s0 < QUIETA {
        // La mano quieta es lo MAS predecible.
        return Some(Prediccion { x: c.x, y: c.y, confianza: 1.0, rapidez: 0.0 });
    }
    if s1 < QUIETA {
        // Acaba de pararse.
        return Some(Prediccion { x: c.x, y: c.y, confianza: 0.8, rapidez });
    }
    // Que la mano siga en la misma direccion: el coseno de las dos.
    let coseno = if s0 > 0.0 { (v0x * v1x + v0y * v1y) / (s0 * s1) } else { 0.0 };
    let (ux, uy) = (v1x / s1, v1y / s1);
    let h_s = adelante_us as f32 / 1e6;
    // Cada velocidad es la del MEDIO de su tramo: la frenada es entre los
    // dos medios, y la parada se cuenta desde el medio del ultimo.
    let medio = 0.5 * (dt0 + dt1);
    let frenada = (s0 - s1) / medio;
    let (mx, my) = (0.5 * (b.x + c.x), 0.5 * (b.y + c.y));
    let desde_medio = 0.5 * dt1 + h_s;
    let (recorre, frena) = if frenada > 0.0 {
        let hasta_parar = s1 / frenada;
        let t = desde_medio.min(hasta_parar);
        (s1 * t - 0.5 * frenada * t * t, true)
    } else {
        (s1 * desde_medio, false)
    };
    let confianza = coseno.clamp(0.0, 1.0) * if frena { 1.0 } else { 0.75 };
    Some(Prediccion { x: mx + ux * recorre, y: my + uy * recorre, confianza, rapidez })
}
