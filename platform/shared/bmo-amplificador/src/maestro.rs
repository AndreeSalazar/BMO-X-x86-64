//! **EL MAESTRO: la ultima etapa, la que va pegada al cable.**
//!
//! El propietario, el 2026-09-22, con DOOM sonando: *"funcionan pero no tengo
//! control de audio... un control que viva en mi escritorio, no como app...
//! para amplificar sonido maestro"*. Y eligio, con su coste delante, que esta
//! etapa la corra **el kernel**, justo antes del tubo, y que la mueva **solo el
//! escritorio** (`PLAN_EL_SONIDO.md`, S4c).
//!
//! # Por que es una pieza y no `Amplificador` con otro nombre
//!
//! `Amplificador` pone una ganancia y la deja puesta. Esto recibe un mando que
//! una mano mueve **mientras suena**, y eso trae tres obligaciones que un
//! amplificador fijo no tiene:
//!
//! ```text
//!    1. LA RAMPA     la ganancia nunca salta: un salto de ganancia es un
//!                    chasquido, y con +12 dB puestos, un golpe en el oido
//!    2. EL REPOSO    a 0 dB y sin nada que soltar, NO SE TOCA NI UN BIT:
//!                    el maestro sin mover tiene que ser un cable
//!    3. LA LECTURA   el medidor se lee por VENTANAS desde otro sitio (el
//!                    escritorio, 20 veces por segundo); lo que sale es un
//!                    numero cerrado, no un medidor a medio contar
//! ```
//!
//! # Por que mide IZQUIERDO y DERECHO por separado
//!
//! Porque es lo primero que muestra un mezclador de verdad y lo que contesta la
//! pregunta de *"por que oigo mas por un lado"*. Con mas de dos canales (un
//! 5.1 el dia que haya), los pares van a la izquierda y los impares a la
//! derecha: el medidor sigue diciendo si algo se pasa, que es para lo que esta.
//!
//! # El limite es UNO para todos los canales, a proposito
//!
//! Un limite por canal sujeta la izquierda y deja la derecha, y la imagen
//! estereo se mueve hacia el lado que no se sujeto. Un limite compartido baja
//! los dos a la vez y la imagen se queda donde estaba. Es lo que hace un
//! limitador de mezcla, y aqui sale gratis porque las muestras llegan
//! intercaladas.

use crate::{q16_a_db, Ganancia, Limite, Medidor, MilesimasDb, DB, MAX_DB, MIN_DB};

/// **Lo que la rampa se mueve por bloque** en la zona donde se oye: 1 dB. Un
/// bloque del tubo es 1 ms, asi que subir 12 dB tarda 12 ms -- mas rapido de
/// lo que se aparta una mano de la rueda, y bastante lento para no chasquear.
pub const PASO_RAMPA: MilesimasDb = DB;

/// **Lo que se mueve por bloque por debajo de -40 dB**: 8 dB. Callar desde 0 dB
/// tarda 12 ms en vez de 96. Ahi abajo un paso grande no se oye --la onda ya
/// casi no esta-- y un mudo que tarda una decima de segundo parece roto.
pub const PASO_HONDO: MilesimasDb = 8 * DB;

/// Por debajo de aqui la rampa usa [`PASO_HONDO`].
pub const HONDO: MilesimasDb = -40 * DB;

/// **Lo que tarda el limite del maestro en devolver la ganancia**: 250 ms.
///
/// El de mezcla devuelve en 100 ms (y en estereo eran 50, ver
/// `Limite::con_relajo_ms`), y con el fader muy arriba eso es bombeo: entre
/// disparo y disparo el fondo sube, y el siguiente disparo lo vuelve a hundir.
/// Un cuarto de segundo es lo de un limitador de seguridad: la cola de un
/// golpe baja y sube de una vez, no a tirones. La prueba
/// `con_el_fader_arriba_el_fondo_no_bombea` fija la diferencia.
pub const RELAJO_MS: u32 = 250;

/// **Lo que el medidor dijo en una ventana**, ya cerrado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lectura {
    /// Pico de la ventana, izquierdo y derecho, en dBFS (1/256). [`MIN_DB`] = nada.
    pub pico: [MilesimasDb; 2],
    /// RMS de la ventana: la fuerza que se OYE, no la punta.
    pub rms: [MilesimasDb; 2],
    /// Muestras que el limite tuvo que doblegar a pelo **desde el principio**.
    /// Es la luz de RECORTE: si sube, lo que sale ya no es la onda.
    pub dobladas: u64,
    /// **Lo MAS que el limite bajo en la ventana** (0 = nada, negativo =
    /// sujetando). No lo de este instante: al cerrar la ventana suele pillar
    /// la cola, y lo que dice si se esta aplastando es el fondo del pozo. Por
    /// encima de 6 dB, subir el fader ya no suena mas fuerte: suena APLASTADO.
    pub reduccion: MilesimasDb,
    /// La ganancia que de verdad esta puesta, por donde va la rampa.
    pub actual: MilesimasDb,
}

/// **La ultima etapa**: mando con rampa, limite compartido y medidor por lado.
#[derive(Clone, Copy, Debug)]
pub struct Maestro {
    /// Lo que pidio el mando, ya recortado a [`MIN_DB`]..[`MAX_DB`].
    objetivo: MilesimasDb,
    mudo: bool,
    /// Por donde va la rampa. [`MIN_DB`] es callado del todo.
    actual: MilesimasDb,
    limite: Limite,
    medidores: [Medidor; 2],
    /// La reduccion mas honda de la ventana, en Q16.16 (65536 = ninguna).
    pozo: u32,
}

impl Maestro {
    /// Un maestro en reposo --0 dB, sin mudo-- para un aparato de `hz` y
    /// `canales`.
    ///
    /// ** LOS CANALES HACEN FALTA, y no por el medidor: el limite ve las
    /// muestras intercaladas, asi que en estereo pasan `2 x hz` por segundo.
    /// Construirlo con `hz` a secas hacia sus tiempos la MITAD de lo que
    /// decian.
    pub fn nuevo(hz: u32, canales: u32) -> Maestro {
        let por_segundo = hz.saturating_mul(canales.max(1));
        Maestro {
            objetivo: 0,
            mudo: false,
            actual: 0,
            // Sin ataque (ver `Limite::inmediato`) y con el relajo del maestro.
            limite: Limite::inmediato(por_segundo).con_relajo_ms(por_segundo, RELAJO_MS),
            medidores: [Medidor::nuevo(); 2],
            pozo: 1 << 16,
        }
    }

    /// **El mando**: a donde tiene que ir la ganancia y si calla. No se aplica
    /// de golpe: lo lleva la rampa en los bloques siguientes.
    pub fn pedir(&mut self, db: MilesimasDb, mudo: bool) {
        self.objetivo = db.clamp(MIN_DB, MAX_DB);
        self.mudo = mudo;
    }

    /// Lo pedido (recortado), para quien lo quiera mostrar.
    pub fn objetivo(&self) -> MilesimasDb {
        self.objetivo
    }

    /// La ganancia que esta puesta ahora mismo.
    pub fn actual(&self) -> MilesimasDb {
        self.actual
    }

    /// **En reposo = un cable.** A 0 dB, sin rampa por delante y sin nada que
    /// el limite tenga que soltar. En reposo, [`Maestro::pasar`] mide y no toca.
    pub fn en_reposo(&self) -> bool {
        !self.mudo && self.objetivo == 0 && self.actual == 0 && self.limite.reduccion_db() == 0
    }

    /// Un paso de la rampa: devuelve la ganancia de antes y la de despues.
    fn avanzar(&mut self) -> (MilesimasDb, MilesimasDb) {
        let destino = if self.mudo { MIN_DB } else { self.objetivo };
        let antes = self.actual;
        let paso = if antes < HONDO || destino < HONDO { PASO_HONDO } else { PASO_RAMPA };
        self.actual = if antes < destino {
            (antes + paso).min(destino)
        } else {
            (antes - paso).max(destino)
        };
        (antes, self.actual)
    }

    /// **Un bloque por la etapa, en el sitio.** `muestras` intercaladas, de
    /// `canales` en `canales`.
    ///
    /// La ganancia del bloque va RECTA de la de antes a la de despues, muestra
    /// a muestra: la rampa no es una escalera de un paso por bloque sino una
    /// cuesta, y por eso no deja el "zumbido de cremallera" que dejan los
    /// mandos que saltan una vez por bloque.
    pub fn pasar(&mut self, muestras: &mut [i16], canales: usize) {
        let canales = canales.max(1);
        if self.en_reposo() {
            // ** EL CABLE. Ni la multiplicacion (que a x1 es exacta) ni el
            // limite (que tocaria el -32.768, el unico valor de 16 bits cuyo
            // modulo no cabe): solo se mira.
            self.mirar(muestras, canales);
            return;
        }
        let (antes, despues) = self.avanzar();
        let f0 = Ganancia::db(antes).factor_q16() as i64;
        let f1 = Ganancia::db(despues).factor_q16() as i64;
        let tramas = (muestras.len() / canales).max(1) as i64;
        for (i, m) in muestras.iter_mut().enumerate() {
            let k = (i / canales) as i64;
            // La cuesta: de f0 a f1 a lo largo del bloque. En la ultima trama
            // ya se esta en f1, que es desde donde sale el bloque siguiente.
            let f = f0 + (f1 - f0) * (k + 1) / tramas;
            let x = ((*m as i64 * f) >> 16) as i32;
            let y = self.limite.muestra(x);
            *m = y;
            self.medidores[(i % canales) & 1].mirar_uno(y as i32);
            if self.limite.reduccion < self.pozo {
                self.pozo = self.limite.reduccion;
            }
        }
    }

    /// **Un bloque de silencio que no se ha escrito**: cuando no hay muestras,
    /// el tubo manda ceros sin pasar por aqui, pero el medidor tiene que CAER
    /// y la rampa tiene que seguir andando -- un mudo pulsado en silencio que
    /// no avanzara saltaria de golpe al volver el sonido.
    ///
    /// *** Y EL LIMITE SUELTA, que no lo hacia (2026-09-22). El silencio no
    /// pasaba por el limite, asi que la reduccion se quedaba CONGELADA donde
    /// la dejo el ultimo golpe: el `save` de las 14:26 decia `limite -5,2 dB`
    /// con DOOM ya cerrado, un numero que parecia de la partida y era del
    /// ultimo disparo. Y con la reduccion puesta, `en_reposo` no volvia nunca.
    /// Ahora los ceros pasan por el limite, que suelta con su relajo como lo
    /// haria con onda: el cero no se sale, asi que no cambia ni una muestra.
    pub fn silencio(&mut self, muestras: usize, canales: usize) {
        let canales = canales.max(1);
        if !self.en_reposo() {
            let _ = self.avanzar();
            for _ in 0..muestras {
                let _ = self.limite.muestra(0);
            }
        }
        for i in 0..muestras {
            self.medidores[(i % canales) & 1].mirar_uno(0);
        }
    }

    /// **Solo mirar**: lo que usa el kernel en reposo, cuando el aparato lee
    /// del bloque de la app y aqui no hay nada que escribir.
    pub fn mirar(&mut self, muestras: &[i16], canales: usize) {
        let canales = canales.max(1);
        for (i, &m) in muestras.iter().enumerate() {
            self.medidores[(i % canales) & 1].mirar_uno(m as i32);
        }
    }

    /// **Cierra la ventana**: lo que se midio desde la ultima lectura, y el
    /// medidor a cero para la siguiente. Las cuentas del limite no se ponen a
    /// cero: `dobladas` es desde el principio, que es lo que dice si algo se
    /// rompio alguna vez aunque nadie estuviera mirando.
    pub fn lectura(&mut self) -> Lectura {
        let l = Lectura {
            pico: [self.medidores[0].pico_dbfs(), self.medidores[1].pico_dbfs()],
            rms: [self.medidores[0].rms_dbfs(), self.medidores[1].rms_dbfs()],
            dobladas: self.limite.dobladas(),
            reduccion: q16_a_db(self.pozo),
            actual: self.actual,
        };
        self.medidores = [Medidor::nuevo(); 2];
        // El pozo vuelve a donde esta el limite AHORA, no a "nada": si sigue
        // sujetando, la ventana siguiente tiene que decirlo.
        self.pozo = self.limite.reduccion;
        l
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una onda cuadrada de `amplitud`, estereo, `tramas` tramas.
    fn cuadrada(amplitud: i16, tramas: usize) -> [i16; 96] {
        let mut b = [0i16; 96];
        for t in 0..tramas.min(48) {
            let v = if (t / 6) % 2 == 0 { amplitud } else { -amplitud };
            b[2 * t] = v;
            b[2 * t + 1] = v;
        }
        b
    }

    #[test]
    fn en_reposo_es_un_cable_bit_a_bit() {
        let mut m = Maestro::nuevo(48_000, 2);
        let mut b = [0i16; 96];
        for (i, v) in b.iter_mut().enumerate() {
            *v = (i as i16).wrapping_mul(700).wrapping_sub(i16::MAX);
        }
        // Incluido el -32.768, que es el que el limite tocaria.
        b[0] = i16::MIN;
        let antes = b;
        m.pasar(&mut b, 2);
        assert_eq!(b, antes);
        assert!(m.en_reposo());
    }

    #[test]
    fn la_rampa_no_salta_de_golpe() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(12 * DB, false);
        let mut b = cuadrada(1000, 48);
        m.pasar(&mut b, 2);
        // Tras un bloque, 1 dB y no 12.
        assert_eq!(m.actual(), DB);
        // Y dentro del bloque la ganancia sube POCO a POCO: la primera trama
        // casi igual que la entrada, la ultima ya a +1 dB.
        assert!(b[0] >= 1000 && b[0] <= 1003, "primera {}", b[0]);
        assert!(b[94] <= -1115 && b[94] >= -1125, "ultima {}", b[94]);
        for _ in 0..11 {
            let mut b = cuadrada(1000, 48);
            m.pasar(&mut b, 2);
        }
        assert_eq!(m.actual(), 12 * DB);
    }

    #[test]
    fn el_mudo_calla_del_todo_y_rapido() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(0, true);
        let mut bloques = 0;
        loop {
            let mut b = cuadrada(20_000, 48);
            m.pasar(&mut b, 2);
            bloques += 1;
            if b.iter().all(|&x| x == 0) {
                break;
            }
            assert!(bloques < 20, "el mudo no llega a cero");
        }
        // De 0 a -40 a 1 dB por bloque y de -40 a -96 a 8: 40 + 7, y el ultimo
        // bloque ya sale entero en cero.
        assert!(bloques <= 48, "{} bloques", bloques);
        assert_eq!(m.actual(), MIN_DB);
        // Y al quitarlo vuelve por la misma cuesta, no de golpe.
        m.pedir(0, false);
        let mut b = cuadrada(20_000, 48);
        m.pasar(&mut b, 2);
        assert!(b.iter().all(|&x| x.unsigned_abs() < 200), "volvio de golpe");
    }

    #[test]
    fn subir_doce_db_a_una_onda_floja_la_cuadruplica_sin_doblegar() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(12 * DB, false);
        for _ in 0..12 {
            let mut b = cuadrada(3000, 48);
            m.pasar(&mut b, 2);
        }
        let _ = m.lectura();
        let mut b = cuadrada(3000, 48);
        m.pasar(&mut b, 2);
        // x3,98: 3.000 -> ~11.940, lejos del techo.
        assert!(b[0] > 11_800 && b[0] < 12_050, "{}", b[0]);
        let l = m.lectura();
        assert_eq!(l.dobladas, 0);
        assert_eq!(l.reduccion, 0);
        // Pico de ~11.940 son unos -8,8 dBFS.
        assert!(l.pico[0] < -8 * DB && l.pico[0] > -10 * DB, "{}", l.pico[0]);
    }

    #[test]
    fn una_onda_fuerte_con_ganancia_la_sujeta_el_limite_y_no_se_sale() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(12 * DB, false);
        for _ in 0..200 {
            let mut b = cuadrada(20_000, 48);
            m.pasar(&mut b, 2);
            assert!(b.iter().all(|&x| x.unsigned_abs() <= 32_767));
        }
        let l = m.lectura();
        assert!(l.reduccion < 0, "el limite no esta sujetando");
        // ** CERO doblegadas. Con el limite de mezcla (ataque de 1 ms) esta
        // misma prueba contaba 982: por eso el maestro lleva el INMEDIATO.
        assert_eq!(l.dobladas, 0);
    }

    #[test]
    fn el_medidor_separa_izquierda_y_derecha() {
        let mut m = Maestro::nuevo(48_000, 2);
        let mut b = [0i16; 96];
        for t in 0..48 {
            b[2 * t] = 16_000; // izquierda fuerte
            b[2 * t + 1] = 1_000; // derecha floja
        }
        m.pasar(&mut b, 2);
        let l = m.lectura();
        assert!(l.pico[0] > -7 * DB && l.pico[0] < -5 * DB, "izq {}", l.pico[0]);
        assert!(l.pico[1] < -29 * DB && l.pico[1] > -31 * DB, "der {}", l.pico[1]);
    }

    #[test]
    fn el_silencio_hace_caer_el_medidor_y_la_lectura_cierra_la_ventana() {
        let mut m = Maestro::nuevo(48_000, 2);
        let mut b = cuadrada(10_000, 48);
        m.pasar(&mut b, 2);
        assert!(m.lectura().pico[0] > -11 * DB);
        m.silencio(96, 2);
        let l = m.lectura();
        assert_eq!(l.pico, [MIN_DB, MIN_DB]);
        // Y sin nada entre medias, la ventana vacia tambien dice "nada".
        assert_eq!(m.lectura().rms, [MIN_DB, MIN_DB]);
    }

    #[test]
    fn el_mudo_en_silencio_sigue_andando() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(0, true);
        for _ in 0..60 {
            m.silencio(96, 2);
        }
        assert_eq!(m.actual(), MIN_DB);
    }

    /// **El caso del propietario**: disparos fuertes y colas flojas, con el fader
    /// a +24 dB. Se mide cuanto CAMBIA el volumen de la cola entre su principio
    /// y su final: eso es el bombeo que se oia como "pelea, a tirones".
    fn bombeo(relajo_ms: u32) -> i64 {
        // 96.000 muestras por segundo: estereo a 48 kHz, intercaladas.
        let mut l = Limite::inmediato(96_000).con_relajo_ms(96_000, relajo_ms);
        let g = Ganancia::db(24 * DB);
        let mut peor = 0i64;
        for _ in 0..10 {
            // 10 ms de disparo...
            for t in 0..960 {
                let v = if (t / 40) % 2 == 0 { 20_000 } else { -20_000 };
                let _ = l.muestra(g.aplicar(v));
            }
            // ...y 90 ms de cola floja: lo que sube y baja es ESTO.
            let mut primera = 0i64;
            let mut ultima = 0i64;
            for t in 0..8_640 {
                let v = if (t / 40) % 2 == 0 { 1_000 } else { -1_000 };
                let y = l.muestra(g.aplicar(v)).unsigned_abs() as i64;
                if t < 80 {
                    primera = primera.max(y);
                }
                if t >= 8_560 {
                    ultima = ultima.max(y);
                }
            }
            peor = peor.max(ultima - primera);
        }
        peor
    }

    #[test]
    fn con_el_fader_arriba_el_fondo_no_bombea() {
        // El de antes: los "100 ms" que en estereo eran 50.
        let antes = bombeo(50);
        let ahora = bombeo(RELAJO_MS);
        // La cola sube MENOS de la mitad que antes entre disparo y disparo.
        assert!(ahora * 2 < antes, "antes {} ahora {}", antes, ahora);
    }

    #[test]
    fn la_lectura_dice_el_fondo_del_pozo_y_no_la_cola() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(24 * DB, false);
        for _ in 0..24 {
            let mut b = cuadrada(3000, 48);
            m.pasar(&mut b, 2);
        }
        let _ = m.lectura();
        // Un golpe fuerte y luego onda floja: al cerrar la ventana el limite
        // ya ha soltado algo, pero el pozo fue hondo.
        let mut b = cuadrada(20_000, 48);
        m.pasar(&mut b, 2);
        for _ in 0..20 {
            let mut b = cuadrada(100, 48);
            m.pasar(&mut b, 2);
        }
        let ahora = m.limite.reduccion_db();
        let l = m.lectura();
        // 20.000 x 15,85 contra 32.767: el pozo tiene que ser de -19,7 dB...
        assert!(l.reduccion < -19 * DB, "pozo {}", l.reduccion);
        // ...y MAS hondo que lo que el limite baja al cerrar la ventana, que
        // es lo que antes se contaba.
        assert!(l.reduccion < ahora - 2 * DB, "pozo {} ahora {}", l.reduccion, ahora);
    }

    #[test]
    fn en_el_silencio_el_limite_suelta_y_el_maestro_vuelve_al_reposo() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(24 * DB, false);
        for _ in 0..30 {
            let mut b = cuadrada(20_000, 48);
            m.pasar(&mut b, 2);
        }
        assert!(m.limite.reduccion_db() < -10 * DB);
        // Un segundo y medio de silencio, bloque a bloque como lo da el tubo.
        for _ in 0..1500 {
            m.silencio(96, 2);
        }
        let _ = m.lectura();
        assert_eq!(m.lectura().reduccion, 0, "el limite se quedo congelado");
        // Y bajando el fader a 0, vuelve a ser un cable.
        m.pedir(0, false);
        for _ in 0..30 {
            m.silencio(96, 2);
        }
        assert!(m.en_reposo());
    }

    #[test]
    fn lo_que_se_pide_de_mas_se_recorta_al_techo_del_crate() {
        let mut m = Maestro::nuevo(48_000, 2);
        m.pedir(100 * DB, false);
        assert_eq!(m.objetivo(), MAX_DB);
        m.pedir(-500 * DB, false);
        assert_eq!(m.objetivo(), MIN_DB);
    }
}
