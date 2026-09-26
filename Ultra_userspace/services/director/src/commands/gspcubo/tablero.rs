//! **EL TABLERO DEL BANCO** (VERRANO V1, 26-09) -- lo que se ve MIENTRAS la
//! tarjeta gira el cubo: los fps, lo que tarda, lo que cuesta
//! prepararle cada fotograma y por donde va, en una banda debajo de la
//! ventana del cubo.
//!
//! [carril]  VERDE     solo pinta en el lienzo, FUERA de la ventana del cubo
//! [consumo] NADA      corre con el banco (que se teclea): la banda cada 66 ms;
//!                     lo que cuesta se mide y se DESCUENTA de los fps
//!
//! # La regla de la banda
//!
//! El aparato dibuja el cubo directamente en la memoria que escanea el monitor.
//! El lienzo del escritorio, en esa ventana, tiene el FONDO: si una caja
//! sucia la tocara, el volcado taparia el cubo con el fondo. Por eso todo el
//! tablero cae en UNA banda que empieza debajo de la ventana, se pinta
//! empezando por un `rect` de la banda entera (lo demas se une a esa caja
//! sin desperdicio) y se vuelca en cuanto se acaba de pintar: nunca quedan
//! en la cola cajas de dos sitios que `sucio` pudiera juntar por encima del
//! cubo.
//!
//! # El estilo
//!
//! Sobrio: marfil para las cifras, gris perla para las etiquetas (espaciadas,
//! en mayusculas), un filete dorado de un pixel, y la firma: tres trazos de
//! dos pixeles, azul, blanco y rojo. Nada parpadea; lo unico que se mueve es
//! lo que cambia.

use bmo_userland as bmo;

use super::{Texto, FONDO, ROJO, VERDE};

const MARFIL: u32 = 0x00EC_E6D6;
const PERLA: u32 = 0x008A_8A94;
const ORO: u32 = 0x00C8_A45C;
const SOMBRA: u32 = 0x0030_3038;
const AZUL: u32 = 0x0024_3F8F;
const BLANCO: u32 = 0x00EE_EEEE;
const GRANATE: u32 = 0x00C0_2E2E;

/// Cuanto mide la banda como poco (sin eso, no hay tablero en vivo).
pub(super) const ALTO_MINIMO: u32 = 150;
/// Cada cuanto se repinta, en ms.
const CADA_MS: u64 = 66;
/// Los ultimos fotogramas que dibuja la grafica.
const MUESTRAS: usize = 160;

/// El tablero: lo que se lleva contado y donde se pinta.
pub(super) struct Tablero {
    x: u32,
    y: u32,
    ancho: u32,
    alto: u32,
    hz: u64,
    total: u32,
    /// El aparato, como lo dice su puerta (`LA 3060`).
    aparato: &'static [u8],
    modo: &'static [u8],
    juez: &'static [u8],
    /// Fotogramas dibujados, y sus ciclos de pared (sin el tablero).
    hechos: u32,
    ciclos: u64,
    /// El aparato: suma, mejor y peor, en us.
    tarjeta: u64,
    mejor: u32,
    peor: u32,
    /// Preparar: suma en us, y cuantos fueron en caliente.
    preparar: u64,
    calientes: u32,
    /// Lo que costo pintar el tablero, en ciclos.
    pintar: u64,
    /// La ventana del ultimo repintado: fotogramas y ciclos.
    v_hechos: u32,
    v_ciclos: u64,
    ultimo: u64,
    muestras: [u32; MUESTRAS],
}

impl Tablero {
    /// La banda debajo de una ventana que acaba en `fin_y`, o `None` si no
    /// cabe.
    pub(super) fn nuevo(p: &bmo::Pantalla, fin_y: u32, total: u32, aparato: &'static [u8], modo: &'static [u8], juez: &'static [u8]) -> Option<Self> {
        let y = fin_y + 12;
        let alto = p.alto.checked_sub(y)?;
        if alto < ALTO_MINIMO || p.ancho < 1024 {
            return None;
        }
        Some(Tablero {
            x: 40,
            y,
            ancho: p.ancho - 80,
            alto,
            hz: bmo::info(bmo::INFO_TSC_HZ).max(1000),
            total,
            aparato,
            modo,
            juez,
            hechos: 0,
            ciclos: 0,
            tarjeta: 0,
            mejor: u32::MAX,
            peor: 0,
            preparar: 0,
            calientes: 0,
            pintar: 0,
            v_hechos: 0,
            v_ciclos: 0,
            ultimo: bmo::ciclos(),
            muestras: [0; MUESTRAS],
        })
    }

    /// Un fotograma hecho: sus ciclos de pared, los us del aparato y los de
    /// preparar, y si fue en caliente.
    pub(super) fn apuntar(&mut self, ciclos: u64, tarjeta_us: u32, preparar_us: u32, caliente: bool) {
        self.muestras[self.hechos as usize % MUESTRAS] = tarjeta_us;
        self.hechos += 1;
        self.ciclos += ciclos;
        self.v_hechos += 1;
        self.v_ciclos += ciclos;
        self.tarjeta += tarjeta_us as u64;
        self.mejor = self.mejor.min(tarjeta_us);
        self.peor = self.peor.max(tarjeta_us);
        self.preparar += preparar_us as u64;
        self.calientes += caliente as u32;
    }

    /// Repinta si toca (cada [`CADA_MS`]), y apunta lo que costo.
    pub(super) fn quizas(&mut self, p: &bmo::Pantalla) {
        let ahora = bmo::ciclos();
        if (ahora - self.ultimo) * 1000 < CADA_MS * self.hz {
            return;
        }
        let fps = self.por_segundo(self.v_hechos, self.v_ciclos);
        self.pintar_banda(p, fps, None);
        self.v_hechos = 0;
        self.v_ciclos = 0;
        let fin = bmo::ciclos();
        self.pintar += fin - ahora;
        self.ultimo = fin;
    }

    /// El final: las cifras de todo el banco y el veredicto.
    pub(super) fn final_(&mut self, p: &bmo::Pantalla, veredicto: &[u8], igual: bool) {
        let fps = self.fps();
        self.pintar_banda(p, fps, Some((veredicto, igual)));
    }

    /// fps de todo el banco, sin lo que costo el tablero.
    pub(super) fn fps(&self) -> u64 {
        self.por_segundo(self.hechos, self.ciclos)
    }

    /// El aparato, de media, en us.
    pub(super) fn tarjeta_media(&self) -> u64 {
        self.tarjeta / self.hechos.max(1) as u64
    }

    pub(super) fn preparar_medio(&self) -> u64 {
        self.preparar / self.hechos.max(1) as u64
    }

    /// Las cuentas para el panel del escritorio (y para `datos`).
    pub(super) fn resumen(&self, a: &mut Texto, b: &mut Texto) {
        a.t(b"banco: ").d(self.hechos as u64).t(b" fotogramas en ").d(self.ciclos * 1000 / self.hz).t(b" ms = ").d(self.fps()).t(b" fps de pared (").t(self.modo).t(b")");
        b.t(self.aparato).t(b" ").d(self.tarjeta_media()).t(b" us (").d(self.mejor.min(self.peor) as u64).t(b"..").d(self.peor as u64).t(b"), preparar ").d(self.preparar_medio()).t(b" us, ").d(self.calientes as u64).t(b" en caliente; tablero ").d(self.pintar * 1000 / self.hz).t(b" ms aparte");
    }

    fn por_segundo(&self, n: u32, ciclos: u64) -> u64 {
        n as u64 * self.hz / ciclos.max(1)
    }

    fn pintar_banda(&self, p: &bmo::Pantalla, fps: u64, fin: Option<(&[u8], bool)>) {
        let (x, y, w) = (self.x, self.y, self.ancho);
        // La banda entera primero: todo lo demas cae dentro de esta caja.
        p.rect(0, y, p.ancho, self.alto, FONDO);
        // El filete, y la firma encima de su arranque.
        p.rect(x, y, w, 1, ORO);
        for (k, c) in [AZUL, BLANCO, GRANATE].into_iter().enumerate() {
            p.rect(x + 12 * k as u32, y - 1, 12, 3, c);
        }
        let col = (w / 4).min(300);
        let ty = y + 14;
        let ny = y + 34;
        etiqueta(p, x, ty, b"FOTOGRAMAS POR SEGUNDO");
        etiqueta(p, x + col, ty, self.aparato);
        etiqueta(p, x + 2 * col, ty, b"PREPARAR");
        etiqueta(p, x + 3 * col, ty, b"FOTOGRAMA");

        let mut t = Texto::nuevo();
        t.d(fps);
        cifra(p, x, ny, t.s(), b"fps");
        let hechos = self.hechos.max(1) as u64;
        let mut t = Texto::nuevo();
        t.d(self.tarjeta / hechos);
        cifra(p, x + col, ny, t.s(), b"us");
        let mut t = Texto::nuevo();
        t.d(self.preparar / hechos);
        cifra(p, x + 2 * col, ny, t.s(), b"us");
        let mut t = Texto::nuevo();
        t.d(self.hechos as u64).t(b"/").d(self.total as u64);
        cifra(p, x + 3 * col, ny, t.s(), b"");

        // Debajo de las cifras: el modo y el juez, en perla.
        let my = ny + 54;
        let mut t = Texto::nuevo();
        t.t(self.modo).t(b"  -  ").d(self.calientes as u64).t(b" en caliente  -  juez: ").t(self.juez);
        p.texto_bytes(x, my, t.s(), PERLA);

        // La grafica del aparato (a la derecha, si cabe): una barra por
        // fotograma, el ultimo en oro.
        let gx = x + 4 * col + 24;
        let gw = (x + w).saturating_sub(gx);
        let gh = 56;
        if gw >= 3 * 40 {
            let caben = ((gw / 3) as usize).min(MUESTRAS).min(self.hechos as usize);
            let tope = (0..caben).map(|k| self.muestra(k)).max().unwrap_or(1).max(1);
            let mut t = Texto::nuevo();
            t.t(self.aparato).t(b", FOTOGRAMA A FOTOGRAMA");
            etiqueta(p, gx, ty, t.s());
            p.rect(gx, ny + gh, gw, 1, SOMBRA);
            for k in 0..caben {
                let v = self.muestra(k);
                let h = ((v as u64 * gh as u64) / tope as u64).max(1) as u32;
                let bx = gx + gw - 3 * (k as u32 + 1);
                p.rect(bx, ny + gh - h, 2, h, if k == 0 { ORO } else { MARFIL });
            }
        }

        // El avance: un filete perla y, encima, lo hecho en oro.
        let ay = my + 24;
        p.rect(x, ay, w, 1, SOMBRA);
        let hecho = (w as u64 * self.hechos as u64 / self.total.max(1) as u64) as u32;
        p.rect(x, ay, hecho.min(w), 1, ORO);

        if let Some((v, igual)) = fin {
            p.texto_bytes(x, ay + 10, v, if igual { VERDE } else { ROJO });
            let pulsa = b"pulsa cualquier tecla";
            p.texto_bytes((x + w).saturating_sub(8 * pulsa.len() as u32), ay + 10, pulsa, PERLA);
        }
        p.vaciar();
    }

    /// La muestra `k` desde la ultima (0 = la ultima).
    fn muestra(&self, k: usize) -> u32 {
        let i = (self.hechos as usize + MUESTRAS - 1 - k) % MUESTRAS;
        self.muestras[i]
    }
}

/// Una etiqueta: mayusculas espaciadas, en perla.
fn etiqueta(p: &bmo::Pantalla, x: u32, y: u32, s: &[u8]) {
    for (k, &c) in s.iter().enumerate() {
        p.glifo(x + 10 * k as u32, y, c, PERLA);
    }
}

/// Una cifra grande en marfil y su unidad chica, en perla, al pie.
fn cifra(p: &bmo::Pantalla, x: u32, y: u32, s: &[u8], unidad: &[u8]) {
    let fin = p.texto_escala(x, y, core::str::from_utf8(s).unwrap_or("?"), MARFIL, 3);
    p.texto_bytes(fin + 8, y + 30, unidad, PERLA);
}
