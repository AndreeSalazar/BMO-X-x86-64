//! **FRAPS-X** -- cuantos fotogramas por segundo, contados con RELOJ, y un
//! banco de pruebas.
//!
//! generacion: hijo
//!
//! Pedido del propietario (25-09): *"FRAPS-X, ese fraps + OBS propio +
//! ACTION!4, los 3 fusionados, exclusivo en BMO-X"*, y despues *"dale al
//! FRAPS-X, el contador de FPS y las capturas"*.
//!
//! ## Que le falta a `bmo-ritmo`, y por que esto es otro crate
//!
//! `bmo-ritmo` cuenta MIRADAS a proposito --*"no sabe de tiempo"*-- y contesta
//! quien de los dos relojes espera al otro. Un contador de FPS es la pregunta
//! contraria: **cuantos por SEGUNDO**, y eso no se puede decir sin reloj. Lo que
//! FRAPS acostumbro a medir, y aqui se mide igual:
//!
//! ```text
//!    Medidor   una ventana de un segundo: cuantos fotogramas cayeron en ella
//!              y cual fue el PEOR (el tiron que se siente aunque la media
//!              diga 60)
//!    Banco     de que se pulsa hasta que se vuelve a pulsar: minimo, media y
//!              maximo por segundo, y los BAJOS del 1 % y del 0,1 %
//! ```
//!
//! ## Lo que se le da, y lo que no sabe
//!
//! Se le da **cuantos fotogramas nuevos hay** y **el reloj** (`ahora`, `hz`: el
//! TSC y su frecuencia). No sabe de donde salen: el DIRECTOR se lo dice mirando
//! la secuencia de una superficie (una app) o contando sus propias vueltas que
//! pintan (el escritorio).
//!
//! [!] Si llegan `n > 1` de golpe --la app publico varios entre dos miradas--,
//! el hueco se reparte entre los `n`: no se sabe cuando cayo cada uno, y
//! apuntar `n - 1` fotogramas de cero milisegundos inventaria un juego a mil
//! FPS.
//!
//! ## Sin memoria dinamica
//!
//! El histograma del banco son [`CUBOS`] contadores de una decima de
//! milisegundo (0 a 100 ms, y lo de mas cae en el ultimo): 4 KiB fijos, y con
//! ellos el 1 % bajo sale sin guardar un solo fotograma.

#![cfg_attr(not(test), no_std)]

/// Contadores del histograma: una decima de milisegundo cada uno, hasta 100 ms.
pub const CUBOS: usize = 1000;

/// Lo que dejo un segundo cerrado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Segundo {
    /// Fotogramas por segundo, en DECIMAS (`675` = 67,5).
    pub fps10: u32,
    /// El peor fotograma del segundo, en microsegundos. `0` si hubo menos de dos.
    pub peor_us: u32,
    /// El ultimo fotograma, en microsegundos.
    pub ultimo_us: u32,
}

/// **Una ventana de un segundo** que se cierra sola al pasar el segundo.
#[derive(Clone, Copy, Debug, Default)]
pub struct Medidor {
    desde: u64,
    cuenta: u32,
    /// El ultimo fotograma. `None` y no `0`: el reloj puede valer cero.
    ultimo: Option<u64>,
    peor: u64,
    hueco: u64,
    empezado: bool,
}

/// Microsegundos de `c` ciclos a `hz`.
fn us(c: u64, hz: u64) -> u32 {
    (c.saturating_mul(1_000_000) / hz.max(1)).min(u32::MAX as u64) as u32
}

impl Medidor {
    pub const fn nuevo() -> Self {
        Medidor { desde: 0, cuenta: 0, ultimo: None, peor: 0, hueco: 0, empezado: false }
    }

    /// **Llegaron `n` fotogramas en `ahora`.** Devuelve el segundo si se cerro.
    ///
    /// Con `n = 0` solo mira el reloj: una app congelada tiene que llegar a
    /// decir **0 FPS**, y eso solo pasa si alguien cierra el segundo sin que
    /// llegue ningun fotograma.
    pub fn fotogramas(&mut self, n: u32, ahora: u64, hz: u64) -> Option<Segundo> {
        if !self.empezado {
            self.empezado = true;
            self.desde = ahora;
            self.ultimo = if n > 0 { Some(ahora) } else { None };
            return None;
        }
        if n > 0 {
            if let Some(u) = self.ultimo {
                let paso = ahora.saturating_sub(u) / n as u64;
                self.peor = self.peor.max(paso);
                self.hueco = paso;
            }
            self.ultimo = Some(ahora);
            self.cuenta = self.cuenta.saturating_add(n);
        }
        let pasado = ahora.saturating_sub(self.desde);
        if pasado < hz.max(1) {
            return None;
        }
        let s = Segundo {
            // Redondeado y no truncado: 60 justos no pueden salir 59,9.
            fps10: ((self.cuenta as u64 * 10 * hz + pasado / 2) / pasado.max(1)).min(u32::MAX as u64) as u32,
            peor_us: us(self.peor, hz),
            ultimo_us: us(self.hueco, hz),
        };
        self.desde = ahora;
        self.cuenta = 0;
        self.peor = 0;
        Some(s)
    }

    /// Se deja de medir (la app se fue, o se cambio de a quien se mide): la
    /// proxima llamada empieza de cero en vez de contar el hueco como un tiron.
    pub fn reiniciar(&mut self) {
        *self = Medidor::nuevo();
    }
}

/// **Un banco de pruebas**: de que se empieza hasta que se para.
#[derive(Clone, Debug)]
pub struct Banco {
    desde: u64,
    ultimo: Option<u64>,
    fotogramas: u64,
    medidos: u64,
    hist: [u32; CUBOS],
    fps_min10: u32,
    fps_max10: u32,
    segundos: u32,
}

/// **Lo que dijo un banco.** Los FPS, en decimas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resumen {
    /// Lo que duro, en milisegundos.
    pub ms: u64,
    pub fotogramas: u64,
    /// El peor y el mejor SEGUNDO. `0` si no llego a cerrarse ninguno.
    pub min10: u32,
    pub max10: u32,
    /// Fotogramas entre tiempo: la media de verdad, no la media de las medias.
    pub media10: u32,
    /// El 1 % bajo: los FPS del fotograma que deja por DETRAS al 1 % mas lento
    /// (el percentil 99 del tiempo de fotograma, pasado a FPS).
    pub bajo1_10: u32,
    /// Lo mismo con el 0,1 %: los tirones sueltos que la media esconde.
    pub bajo01_10: u32,
}

impl Banco {
    pub const fn nuevo(ahora: u64) -> Self {
        Banco {
            desde: ahora,
            ultimo: None,
            fotogramas: 0,
            medidos: 0,
            hist: [0; CUBOS],
            fps_min10: u32::MAX,
            fps_max10: 0,
            segundos: 0,
        }
    }

    /// Llegaron `n` fotogramas en `ahora`: sus tiempos van al histograma.
    pub fn fotogramas(&mut self, n: u32, ahora: u64, hz: u64) {
        if n == 0 {
            return;
        }
        self.fotogramas += n as u64;
        if let Some(u) = self.ultimo {
            let paso = ahora.saturating_sub(u) / n as u64;
            // En decimas de milisegundo: 100 us por cubo.
            let cubo = ((us(paso, hz) / 100) as usize).min(CUBOS - 1);
            self.hist[cubo] = self.hist[cubo].saturating_add(n);
            self.medidos += n as u64;
        }
        self.ultimo = Some(ahora);
    }

    /// Un segundo que cerro el [`Medidor`]: el peor y el mejor.
    pub fn segundo(&mut self, s: Segundo) {
        self.segundos += 1;
        self.fps_min10 = self.fps_min10.min(s.fps10);
        self.fps_max10 = self.fps_max10.max(s.fps10);
    }

    /// Segundos cerrados desde que empezo.
    pub fn segundos(&self) -> u32 {
        self.segundos
    }

    /// Los FPS del fotograma que deja por detras la fraccion `de_mil` de mil
    /// (10 = el 1 %) de los mas lentos.
    fn bajo(&self, de_mil: u64) -> u32 {
        if self.medidos == 0 {
            return 0;
        }
        // Cuantos fotogramas son esa fraccion, redondeando hacia arriba: con
        // cincuenta fotogramas, el 1 % es el peor, no ninguno.
        let hace_falta = (self.medidos * de_mil).div_ceil(1000).max(1);
        let mut vistos = 0u64;
        for i in (0..CUBOS).rev() {
            vistos += self.hist[i] as u64;
            if vistos >= hace_falta {
                // El centro del cubo, en microsegundos.
                let us = i as u64 * 100 + 50;
                return (10_000_000 / us) as u32;
            }
        }
        0
    }

    /// **Lo que dice el banco hasta `ahora`.**
    pub fn resumen(&self, ahora: u64, hz: u64) -> Resumen {
        let pasado = ahora.saturating_sub(self.desde);
        Resumen {
            ms: pasado.saturating_mul(1000) / hz.max(1),
            fotogramas: self.fotogramas,
            min10: if self.segundos == 0 { 0 } else { self.fps_min10 },
            max10: self.fps_max10,
            media10: (self.fotogramas.saturating_mul(10).saturating_mul(hz) / pasado.max(1))
                .min(u32::MAX as u64) as u32,
            bajo1_10: self.bajo(10),
            bajo01_10: self.bajo(1),
        }
    }
}

/// Escribe `v` decimas como `67.5`.
fn decimas(dst: &mut [u8], n: &mut usize, v: u32) {
    entero(dst, n, (v / 10) as u64);
    poner(dst, n, b".");
    poner(dst, n, &[b'0' + (v % 10) as u8]);
}

fn entero(dst: &mut [u8], n: &mut usize, v: u64) {
    let mut d = [0u8; 20];
    let mut k = 0;
    let mut x = v;
    loop {
        d[k] = b'0' + (x % 10) as u8;
        k += 1;
        x /= 10;
        if x == 0 {
            break;
        }
    }
    while k > 0 {
        k -= 1;
        poner(dst, n, &[d[k]]);
    }
}

fn poner(dst: &mut [u8], n: &mut usize, s: &[u8]) {
    for &c in s {
        if *n < dst.len() {
            dst[*n] = c;
            *n += 1;
        }
    }
}

/// La cabecera del CSV del banco.
pub const CSV_CABECERA: &[u8] = b"que,fotogramas,ms,min,media,max,bajo_1,bajo_01\n";

/// **Una fila de CSV** con el resumen: `que` es lo que se midio (el nombre de
/// la app, o `escritorio`). Devuelve los bytes escritos. Lo abre cualquier hoja
/// de calculo, como el `minmaxavg.csv` de FRAPS.
pub fn fila_csv(que: &[u8], r: &Resumen, dst: &mut [u8]) -> usize {
    let mut n = 0;
    // Una coma en el nombre partiria la fila: se cambia por un espacio.
    for &c in que {
        poner(dst, &mut n, &[if c == b',' || c == b'\n' { b' ' } else { c }]);
    }
    poner(dst, &mut n, b",");
    entero(dst, &mut n, r.fotogramas);
    poner(dst, &mut n, b",");
    entero(dst, &mut n, r.ms);
    for v in [r.min10, r.media10, r.max10, r.bajo1_10, r.bajo01_10] {
        poner(dst, &mut n, b",");
        decimas(dst, &mut n, v);
    }
    poner(dst, &mut n, b"\n");
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un reloj de mentira a 1 GHz: un ciclo es un nanosegundo.
    const HZ: u64 = 1_000_000_000;
    const MS: u64 = 1_000_000;

    /// A 60 por segundo: 60 FPS y 16,6 ms. El segundo se cierra con el primer
    /// fotograma que cae pasado el segundo, y cuenta lo que duro de verdad.
    #[test]
    fn sesenta_en_un_segundo_son_sesenta() {
        let mut m = Medidor::nuevo();
        let paso = HZ / 60;
        let mut t = 1000;
        assert!(m.fotogramas(1, t, HZ).is_none(), "el primero solo toma referencia");
        let mut s = None;
        for _ in 0..61 {
            t += paso;
            s = s.or(m.fotogramas(1, t, HZ));
        }
        let s = s.expect("se cerro el segundo");
        assert_eq!(s.fps10, 600);
        assert_eq!(s.peor_us, 16_666);
        assert_eq!(s.ultimo_us, 16_666);
    }

    /// Una app CONGELADA tiene que llegar a decir 0 FPS: el segundo se cierra
    /// mirando el reloj aunque no llegue nada.
    #[test]
    fn una_app_congelada_dice_cero() {
        let mut m = Medidor::nuevo();
        m.fotogramas(1, 0, HZ);
        assert!(m.fotogramas(0, HZ / 2, HZ).is_none());
        let s = m.fotogramas(0, HZ + 1, HZ).expect("paso el segundo");
        assert_eq!(s.fps10, 0);
    }

    /// El tiron que la media esconde: 59 fotogramas rapidos y uno de 100 ms.
    #[test]
    fn el_peor_fotograma_se_ve_aunque_la_media_no() {
        let mut m = Medidor::nuevo();
        let mut t = 0;
        m.fotogramas(1, t, HZ);
        let mut s = None;
        for i in 0..70 {
            t += if i == 30 { 100 * MS } else { 15 * MS };
            s = s.or(m.fotogramas(1, t, HZ));
        }
        let s = s.unwrap();
        assert_eq!(s.peor_us, 100_000);
        assert!(s.fps10 > 550, "la media sigue alta: {}", s.fps10);
    }

    /// Tres de golpe tras 30 ms: se reparten, no son dos de cero.
    #[test]
    fn varios_de_golpe_se_reparten_el_hueco() {
        let mut b = Banco::nuevo(0);
        b.fotogramas(1, 0, HZ);
        b.fotogramas(3, 30 * MS, HZ);
        let r = b.resumen(30 * MS, HZ);
        assert_eq!(r.fotogramas, 4);
        // Los tres en el cubo de 10 ms: el 1 % bajo es 10 ms = 100 FPS (el
        // centro del cubo, 10,05 ms, da 99,5).
        assert_eq!(r.bajo1_10, 995);
    }

    /// El banco entero: un minuto a 60 con un tiron de 50 ms cada segundo.
    #[test]
    fn un_banco_con_tirones_dice_sus_bajos() {
        let mut m = Medidor::nuevo();
        let mut b = Banco::nuevo(0);
        let mut t = 0;
        m.fotogramas(1, t, HZ);
        b.fotogramas(1, t, HZ);
        for i in 0..3600u32 {
            t += if i % 60 == 59 { 50 * MS } else { 16 * MS };
            if let Some(s) = m.fotogramas(1, t, HZ) {
                b.segundo(s);
            }
            b.fotogramas(1, t, HZ);
        }
        let r = b.resumen(t, HZ);
        assert_eq!(r.fotogramas, 3601);
        assert!(b.segundos() >= 59);
        // Uno de cada 60 es de 50 ms: son el 1,7 %, asi que el 1 % bajo cae
        // en ellos (20 FPS) y la media casi no se entera.
        assert_eq!(r.bajo1_10, 10_000_000 / 50_050);
        // 3601 fotogramas en 60 x (59 x 16 + 50) = 59.640 ms: 60,38 FPS.
        assert_eq!(r.media10, 603);
        assert!(r.min10 >= 590 && r.max10 <= 620, "min {} max {}", r.min10, r.max10);
    }

    /// Lo que pasa de 100 ms cae en el ultimo cubo y no se sale.
    #[test]
    fn un_fotograma_eterno_no_se_sale_del_histograma() {
        let mut b = Banco::nuevo(0);
        b.fotogramas(1, 0, HZ);
        b.fotogramas(1, 5 * HZ, HZ);
        assert_eq!(b.resumen(5 * HZ, HZ).bajo01_10, 10_000_000 / 99_950);
    }

    /// Un banco sin fotogramas no inventa numeros.
    #[test]
    fn un_banco_vacio_dice_ceros() {
        let b = Banco::nuevo(0);
        let r = b.resumen(HZ, HZ);
        assert_eq!(r, Resumen { ms: 1000, ..Resumen::default() });
    }

    #[test]
    fn la_fila_de_csv_se_lee_en_una_hoja() {
        let r = Resumen { ms: 60_000, fotogramas: 4051, min10: 552, max10: 701, media10: 675, bajo1_10: 480, bajo01_10: 212 };
        let mut b = [0u8; 128];
        let n = fila_csv(b"DOOM,2", &r, &mut b);
        assert_eq!(&b[..n], b"DOOM 2,4051,60000,55.2,67.5,70.1,48.0,21.2\n");
        assert_eq!(CSV_CABECERA.iter().filter(|&&c| c == b',').count(), 7);
    }
}
