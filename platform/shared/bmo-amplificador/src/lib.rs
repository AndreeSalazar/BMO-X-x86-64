//! **EL AMPLIFICADOR: la suma con ganancia, su limite y su medidor.**
//!
//! generacion: abuelo -- no depende de nadie
//!
//! capa: puro -- aritmetica entera sobre muestras; ni E/S ni `unsafe`
//!
//! # Por que ESTO es lo primero del sonido, y no el censo
//!
//! El propietario, el 2026-09-22: *"solo ponle amplificador de sonido porque es
//! personal, y para sorpresa el amplificador es lo que SUMA LA BASE... ahora
//! empezamos desde el inicio, genesis"*. Tiene razon, y corrige el orden que
//! este ayudante habia propuesto:
//!
//! ```text
//!    mezclar N fuentes  =  SUMAR con ganancia, y no pasarse
//!    amplificar         =  ganancia, y no pasarse
//!    bajar 5.1 a 2      =  SUMAR con ganancia (0,707), y no pasarse
//!    situar en el 3D    =  ganancia distinta por canal, y no pasarse
//! ```
//!
//! Los cuatro son **la misma pieza**. Sin ella, cada uno de los de arriba
//! tendria su propia suma, su propio recorte y su propia mentira sobre lo que
//! le paso a la onda. Con ella, son la misma cuenta con parametros distintos.
//! Por eso el amplificador es el GENESIS del sonido de BMO-X y no una etapa
//! mas: es la unica pieza que TODAS las demas necesitan.
//!
//! # Y por que es personal
//!
//! El propietario es duro de oido, y su audifono declara **-45,0 a 0,0 dB**: cero
//! es su techo y por encima el aparato no da mas. Subir mas solo se puede aqui,
//! en el software, y subir en el software sin red es exactamente como se rompe
//! el sonido. De ahi las tres piezas de este crate y no una:
//!
//! ```text
//!    GANANCIA    lo que se pide            (-inf .. +24 dB)
//!    LIMITE      lo que se permite salir   (un techo, con envolvente)
//!    MEDIDOR     lo que DE VERDAD salio    (pico, RMS, y cuanto se sujeto)
//! ```
//!
//! *** **Y la ley de esta pieza: EL AMPLIFICADOR NO MIENTE.** Un amplificador
//! que sube 20 dB y recorta un tercio de las muestras "suena mas alto" y suena
//! MAL, y el que escucha no tiene forma de saberlo. Aqui el medidor cuenta
//! cuantas muestras toco el limite y cuanto tuvo que bajar, y eso sale en el
//! `save`. Subir hasta oirlo bien deja de ser una ruleta.
//!
//! # Nada de coma flotante
//!
//! Todo es entero. La ganancia se lleva en **1/256 de dB** --la misma unidad
//! que usa el Feature Unit de USB Audio, o sea la misma que ya sale en el
//! `save`-- y se convierte a un factor lineal en Q16.16 con dos tablas y una
//! interpolacion. Sin `f32` no hay estado de FPU que salvar en un syscall, y el
//! resultado es **el mismo byte en todas las maquinas**, que es lo que permite
//! probar esto sin encender el Ryzen.
//!
//! # Lo que este crate NO es
//!
//! No es un ecualizador, ni una reverberacion, ni un compresor multibanda: eso
//! es un DAW y es otro programa (ver `docs/plan/PLAN_EL_SONIDO.md` seccion 5).
//! Aqui hay ganancia, limite y medida. Tampoco decide de donde salen las
//! muestras ni a donde van: entran enteros y salen enteros.

#![no_std]
#![forbid(unsafe_code)]

// ===================================================================
//  1. LA GANANCIA: de dB a un factor, sin coma flotante
// ===================================================================

/// Una ganancia, en **1/256 de dB**. `0` es dejarlo igual.
///
/// La unidad no es un capricho: es la del `wVolume` de USB Audio (clase 1.0,
/// 5.2.2.4.3.2), o sea la misma en la que el audifono declara su `-45,0 a 0,0`.
/// Que la ganancia del software y la del aparato se cuenten igual evita la
/// conversion silenciosa que acaba en "creia que estaba a la mitad".
pub type MilesimasDb = i32;

/// Un dB entero, en la unidad de [`MilesimasDb`].
pub const DB: MilesimasDb = 256;

/// El techo que este crate acepta: **+24 dB**, o sea multiplicar por ~15,8.
///
/// No es un limite del algoritmo sino una decision: por encima de +24 dB, lo
/// que hay en la mayoria de las grabaciones no es onda, es el ruido de fondo
/// amplificado. Pedir mas se recorta AQUI y [`Ganancia::se_recorto`] lo dice,
/// en vez de fingir que se puso.
pub const MAX_DB: MilesimasDb = 24 * DB;

/// El suelo: por debajo de -96 dB una muestra de 16 bits ya es cero.
pub const MIN_DB: MilesimasDb = -96 * DB;

/// `10^(n/20)` en Q16.16, para `n` de 0 a 24 dB. Generada con la formula y
/// comprobada por la prueba `la_tabla_de_db_cuadra_con_la_formula`.
const ENTEROS: [u32; 25] = [
65536, 73533, 82505, 92572, 103868, 116541,
    130762, 146717, 164619, 184706, 207243, 232531,
    260904, 292739, 328458, 368536, 413504, 463959,
    520571, 584090, 655360, 735326, 825049, 925721,
    1038676,
];

/// `10^(k/(16*20))` en Q16.16, para `k` de 0 a 15: la parte de dB que no es
/// entera, en dieciseisavos. El resto (hasta 1/256 de dB) se interpola.
const DIECISEISAVOS: [u32; 16] = [
65536, 66009, 66486, 66966, 67450, 67937,
    68427, 68922, 69419, 69921, 70425, 70934,
    71446, 71962, 72482, 73005,
];

/// **Una ganancia, ya resuelta a su factor lineal.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ganancia {
    db: MilesimasDb,
    /// El factor en Q16.16: `65536` es x1.
    factor: u32,
    /// `true` si se pidio mas de [`MAX_DB`] (o menos de [`MIN_DB`]).
    recortada: bool,
    /// Silencio de verdad: ni un LSB.
    muda: bool,
}

impl Ganancia {
    /// Ni sube ni baja.
    pub const UNIDAD: Ganancia = Ganancia { db: 0, factor: 1 << 16, recortada: false, muda: false };

    /// Callar del todo. No es `-96 dB`: es **cero**, y se nota en que no deja
    /// ni el bit de ruido que deja una ganancia muy chica.
    pub const SILENCIO: Ganancia = Ganancia { db: MIN_DB, factor: 0, recortada: false, muda: true };

    /// Desde dB (en 1/256). Lo que no cabe se recorta y se DICE.
    pub fn db(db: MilesimasDb) -> Ganancia {
        if db <= MIN_DB {
            let mut g = Ganancia::SILENCIO;
            g.recortada = db < MIN_DB;
            return g;
        }
        let (db_usado, recortada) = if db > MAX_DB { (MAX_DB, true) } else { (db, false) };
        Ganancia { db: db_usado, factor: factor_q16(db_usado), recortada, muda: false }
    }

    /// Desde dB enteros, que es como se escriben a mano.
    pub fn db_entero(db: i32) -> Ganancia {
        Ganancia::db(db.saturating_mul(DB))
    }

    /// **Desde un porcentaje al estilo de un mando de volumen**, 0..400.
    ///
    /// 100 es ganancia unidad; 200 es **+6 dB** (el doble de amplitud), 400 es
    /// +12 dB. No es lineal a proposito: el oido no lo es, y un mando lineal
    /// deja todo lo util en el ultimo cuarto del recorrido.
    pub fn porcentaje(pct: u32) -> Ganancia {
        match pct {
            0 => Ganancia::SILENCIO,
            p => {
                // 20*log10(p/100), en 1/256 de dB, sin coma flotante:
                // log10(p/100) = (log2(p) - log2(100)) / log2(10).
                let l2 = log2_q16(p.max(1)) - log2_q16(100);
                // 20/log2(10) = 6,0206; en 1/256 son 1541,3. Y `l2` ya es
                // Q16.16, asi que se divide por 65536 al final.
                let db = ((l2 as i64 * 1541) >> 16) as i32;
                Ganancia::db(db)
            }
        }
    }

    /// Los dB que de verdad se pusieron (ya recortados a [`MAX_DB`]).
    pub fn en_db(&self) -> MilesimasDb {
        self.db
    }

    /// El factor lineal en Q16.16: `65536` es x1, `131072` es x2.
    pub fn factor_q16(&self) -> u32 {
        self.factor
    }

    /// Se pidio mas de lo que este crate acepta?
    pub fn se_recorto(&self) -> bool {
        self.recortada
    }

    /// Es silencio del todo?
    pub fn es_muda(&self) -> bool {
        self.muda
    }

    /// Aplicar a una muestra, en 32 bits y **sin desbordar**: el resultado
    /// puede salirse de 16 bits a proposito, y de eso se encarga el limite.
    pub fn aplicar(&self, x: i32) -> i32 {
        if self.muda {
            return 0;
        }
        (((x as i64) * (self.factor as i64)) >> 16) as i32
    }
}

/// `10^(db/20)` en Q16.16 para `db` en 1/256 de dB, positivo o negativo.
fn factor_q16(db: MilesimasDb) -> u32 {
    if db == 0 {
        return 1 << 16;
    }
    if db < 0 {
        // ** EL RECIPROCO, PERO POR TRAMOS DE 20 dB. La primera version hacia
        // `2^32 / factor_q16(-db)` a secas, y para -95 dB eso pedia a la tabla
        // el factor de +95 dB -- que no tiene, porque llega a 24. Saturaba en
        // 24 y devolvia un numero que no era: la prueba `la_ganancia_sube
        // _siempre_que_se_le_pide_mas` lo caza en -95,0 dB.
        //
        // 20 dB son EXACTAMENTE x10, asi que se sacan fuera y lo que queda
        // siempre cabe en la tabla.
        let d = -db;
        let decenas = d / (20 * DB);
        let resto = d % (20 * DB);
        let f = factor_positivo(resto) as u64;
        if f == 0 {
            return 0;
        }
        let mut inv = (1u64 << 32) / f;
        for _ in 0..decenas {
            inv /= 10;
        }
        return inv as u32;
    }
    factor_positivo(db)
}

/// `10^(db/20)` en Q16.16 para `db` positivo y **menor de 25 dB**, que es lo
/// que la tabla cubre. Los tramos de 20 dB los saca `factor_q16`.
fn factor_positivo(db: MilesimasDb) -> u32 {
    if db <= 0 {
        return 1 << 16;
    }
    // db = enteros + dieciseisavos + resto
    let enteros = (db / DB) as usize;
    let sobra = db % DB; // 0..255, o sea 0..1 dB
    let dieciseisavos = (sobra / 16) as usize; // 0..15
    let resto = sobra % 16; // 0..15, el trocito que se interpola

    let base = ENTEROS[enteros.min(24)] as u64;
    let fino = DIECISEISAVOS[dieciseisavos] as u64;
    // El trocito: entre este dieciseisavo y el siguiente, recto. El error de
    // interpolar recto dentro de 1/16 de dB es de unas partes por millon.
    let siguiente = if dieciseisavos == 15 {
        // El siguiente de 15/16 es 1 dB entero.
        ENTEROS[1] as u64
    } else {
        DIECISEISAVOS[dieciseisavos + 1] as u64
    };
    let fino = fino + ((siguiente - fino) * resto as u64) / 16;
    ((base * fino) >> 16) as u32
}

/// Un factor Q16.16 que solo BAJA (0..65536) a dB en 1/256: 0 o negativo.
fn q16_a_db(r: u32) -> MilesimasDb {
    if r >= (1 << 16) {
        return 0;
    }
    // 20*log10(r/65536), con r en Q16.16.
    let l2 = log2_q16(r.max(1)) - log2_q16(1 << 16);
    ((l2 as i64 * 1541) >> 16) as i32
}

/// `log2(x)` en Q16.16 para `x >= 1`. Sin coma flotante: la parte entera son
/// los bits, y la fraccion sale de elevar al cuadrado ocho veces.
fn log2_q16(x: u32) -> i32 {
    if x == 0 {
        return i32::MIN / 2;
    }
    let entero = 31 - x.leading_zeros(); // floor(log2(x))
    // La mantisa, normalizada a [1,2) en Q16.16.
    let mut m = if entero >= 16 { (x >> (entero - 16)) as u64 } else { (x as u64) << (16 - entero) };
    let mut frac: u32 = 0;
    // Ocho vueltas dan 8 bits de fraccion: 1/256 de un bit de exponente, o sea
    // 0,02 dB. De sobra para un mando de volumen.
    for i in 0..8 {
        m = (m * m) >> 16;
        if m >= (2 << 16) {
            m >>= 1;
            frac |= 1 << (15 - i);
        }
    }
    ((entero << 16) | frac) as i32
}

/// **Un valor lineal a dBFS**, en 1/256 de dB, con `32.767` como cero.
///
/// Devuelve [`MIN_DB`] para el cero absoluto, que es lo honesto: "menos infinito"
/// no cabe en un entero y fingir `-0` seria mentir en la direccion peligrosa.
pub fn a_dbfs(valor: i32) -> MilesimasDb {
    let v = valor.unsigned_abs();
    if v == 0 {
        return MIN_DB;
    }
    // 20*log10(v/32767) = (log2(v) - log2(32767)) * 6,0206
    let l2 = log2_q16(v) - log2_q16(32767);
    let db = ((l2 as i64 * 1541) >> 16) as i32;
    db.max(MIN_DB)
}

// ===================================================================
//  2. LA SUMA: la base, y la unica que se usa
// ===================================================================

/// **Suma una fuente en el acumulador, con su ganancia.** ESTA es la pieza.
///
/// El acumulador es de 32 bits a proposito: ocho fuentes a todo volumen suman
/// 8 x 32.767 = 262.136, que no cabe en 16 bits pero sobra en 32. Los bits de
/// mas son la HABITACION donde la suma cabe **antes** de que el limite decida
/// que hacer con ella. Sumar directamente en 16 bits es el fallo clasico: la
/// suma da la vuelta y un pasaje fuerte suena como un chasquido.
///
/// Suma sobre lo que ya hay (no lo pisa), y se detiene en el mas corto de los
/// dos: mezclar es responsabilidad del llamante, no adivinar medidas.
pub fn sumar(acumulador: &mut [i32], fuente: &[i16], g: Ganancia) {
    if g.es_muda() {
        return;
    }
    let n = acumulador.len().min(fuente.len());
    for i in 0..n {
        acumulador[i] = acumulador[i].saturating_add(g.aplicar(fuente[i] as i32));
    }
}

/// Lo mismo desde un acumulador de 32 bits (una fuente ya mezclada).
pub fn sumar_32(acumulador: &mut [i32], fuente: &[i32], g: Ganancia) {
    if g.es_muda() {
        return;
    }
    let n = acumulador.len().min(fuente.len());
    for i in 0..n {
        acumulador[i] = acumulador[i].saturating_add(g.aplicar(fuente[i]));
    }
}

// ===================================================================
//  3. EL LIMITE: no es un recortador
// ===================================================================

/// Lo que cabe en una muestra de 16 bits.
pub const PLENO: i32 = 32767;

/// **El limite, con envolvente: baja la ganancia ANTES de recortar.**
///
/// La diferencia con recortar, dicha en una frase: un recortador corta la punta
/// de la onda y eso mete armonicos que no estaban --suena *sucio*--; un limite
/// con envolvente **baja el volumen de todo el pasaje** mientras dura la punta
/// y lo devuelve despues, que es lo que hace un limitador de verdad y lo que
/// permite subir +12 dB sin que la musica se rompa.
///
/// ```text
///    |x| por encima del techo  ->  la reduccion BAJA deprisa   (ataque)
///    |x| por debajo            ->  la reduccion vuelve despacio (relajo)
/// ```
///
/// [!] Sin ventana de anticipacion (*look-ahead*): un golpe de un solo ciclo
/// puede pasar antes de que el ataque reaccione, y por eso el recorte duro
/// sigue estando detras, contado aparte. Dicho para no venderlo como lo que no
/// es; la anticipacion pide retrasar la onda y eso es otra casilla.
#[derive(Clone, Copy, Debug)]
pub struct Limite {
    /// El techo, lineal (0..[`PLENO`]).
    techo: i32,
    /// La reduccion que hay puesta ahora, Q16.16 (65536 = ninguna).
    reduccion: u32,
    /// Cuanto se acerca a la reduccion que toca, por muestra, subiendo y
    /// bajando. Q16.16.
    ataque: u32,
    relajo: u32,
    /// Muestras que el limite tuvo que sujetar (la reduccion actuo).
    sujetadas: u64,
    /// Muestras que AUN ASI se salieron y hubo que doblegar a pelo. Esta es la
    /// cifra que tiene que ser chica; si crece, el ataque es lento para esta
    /// onda o la ganancia es absurda.
    dobladas: u64,
}

impl Limite {
    /// Un limite al pleno, con ataque de ~1 ms y relajo de ~100 ms a la
    /// frecuencia dada. Los dos valores son los de un limitador de mezcla
    /// corriente: bastante rapido para sujetar una silaba, bastante lento
    /// para no "bombear".
    pub fn nuevo(hz: u32) -> Limite {
        Limite::con_techo(PLENO, hz)
    }

    /// Igual, con el techo puesto a mano (por ejemplo a -1 dBFS).
    pub fn con_techo(techo: i32, hz: u32) -> Limite {
        let hz = hz.max(1);
        // Muestras en el tiempo de ataque y de relajo; el paso es 1/n de la
        // distancia que falta, con minimo 1 para que siempre avance.
        let muestras_ataque = (hz / 1000).max(1);
        let muestras_relajo = (hz / 10).max(1);
        Limite {
            techo: techo.clamp(1, PLENO),
            reduccion: 1 << 16,
            ataque: ((1u64 << 16) / muestras_ataque as u64).max(1) as u32,
            relajo: ((1u64 << 16) / muestras_relajo as u64).max(1) as u32,
            sujetadas: 0,
            dobladas: 0,
        }
    }

    /// **Un limite SIN ataque: el de la etapa pegada al oido.**
    ///
    /// El de [`Limite::nuevo`] tarda ~1 ms en bajar, y en ese milisegundo lo
    /// que se pasa se doblega a pelo. Para una mezcla vale; para la ULTIMA
    /// etapa no: el 2026-09-22 la prueba del maestro lo conto --una cuadrada
    /// fuerte con +12 dB, **982 muestras doblegadas** en 200 ms-- y cada una es
    /// una punta cortada que suena sucia justo cuando mas alto esta.
    ///
    /// Este baja la reduccion **en la misma muestra** que se pasaria y la
    /// devuelve con el mismo relajo de ~100 ms. Es lo que hace un limitador de
    /// seguridad: el arranque de un golpe se sujeta de una vez (y el golpe
    /// mismo tapa el cambio), y lo que viene detras baja y sube despacio.
    pub fn inmediato(hz: u32) -> Limite {
        let mut l = Limite::nuevo(hz);
        l.ataque = 1 << 16;
        l
    }

    /// **El relajo, puesto a mano**: cuanto tarda en devolver la ganancia,
    /// en ms, para `muestras_por_segundo` muestras que PASAN por aqui (en
    /// estereo, el doble de la frecuencia: el limite ve las dos intercaladas).
    ///
    /// *** LO QUE ESTO ARREGLA (2026-09-22). El maestro sonaba *"como si
    /// peleara, a tirones"* con +24 dB. Con 12 dB o mas de reduccion, un
    /// relajo corto sube la ganancia entre golpe y golpe y la vuelve a bajar
    /// en el siguiente: el volumen del FONDO sube y baja con cada disparo --el
    /// bombeo--. Y el de antes era mas corto de lo que decia: los "100 ms" se
    /// contaban por muestra intercalada, o sea 50 ms reales en estereo.
    pub fn con_relajo_ms(mut self, muestras_por_segundo: u32, ms: u32) -> Limite {
        let n = (muestras_por_segundo as u64 * ms as u64 / 1000).max(1);
        self.relajo = ((1u64 << 16) / n).max(1) as u32;
        self
    }

    /// El techo puesto.
    pub fn techo(&self) -> i32 {
        self.techo
    }

    /// La reduccion que hay puesta ahora, en 1/256 de dB (negativa o cero).
    pub fn reduccion_db(&self) -> MilesimasDb {
        q16_a_db(self.reduccion)
    }

    /// Muestras que la envolvente sujeto.
    pub fn sujetadas(&self) -> u64 {
        self.sujetadas
    }

    /// Muestras que aun asi hubo que doblegar a pelo. **Tiene que ser chica.**
    pub fn dobladas(&self) -> u64 {
        self.dobladas
    }

    /// Una muestra por el limite: entra de 32 bits, sale de 16.
    pub fn muestra(&mut self, x: i32) -> i16 {
        // A donde deberia estar la reduccion para que ESTA muestra quepa.
        let pico = x.unsigned_abs().max(1);
        let objetivo: u32 = if pico > self.techo as u32 {
            (((self.techo as u64) << 16) / pico as u64) as u32
        } else {
            1 << 16
        };
        // *** EL RELAJO VA HACIA `objetivo`, NO HACIA "ninguna reduccion".
        // La primera version subia siempre hacia 65536 en cuanto no tocaba
        // bajar, y con una onda que se pasa de forma sostenida eso es un
        // sube-y-baja: la reduccion se asienta en su sitio, el relajo la
        // empuja 8 arriba, el ataque la baja 1, y a partir de ahi **cada
        // muestra se sale otra vez**. La prueba `el_limite_BAJA_la_ganancia
        // _en_vez_de_recortar` lo conto: 888 muestras doblegadas a pelo
        // cuando tenian que ser ninguna.
        if objetivo < self.reduccion {
            // Hay que bajar: deprisa.
            let paso = ((self.reduccion - objetivo) as u64 * self.ataque as u64 >> 16) as u32;
            self.reduccion -= paso.max(1).min(self.reduccion - objetivo);
            self.sujetadas += 1;
        } else if objetivo > self.reduccion {
            // Se puede volver: despacio, y nunca por encima de lo que esta
            // muestra permite.
            let falta = objetivo - self.reduccion;
            let paso = (falta as u64 * self.relajo as u64 >> 16) as u32;
            self.reduccion += paso.max(1).min(falta);
            if self.reduccion < (1 << 16) {
                self.sujetadas += 1;
            }
        } else if self.reduccion < (1 << 16) {
            // Asentada donde toca: no se toca, pero sigue contando.
            self.sujetadas += 1;
        }
        let y = (((x as i64) * (self.reduccion as i64)) >> 16) as i32;
        if y > self.techo || y < -self.techo {
            self.dobladas += 1;
        }
        y.clamp(-self.techo, self.techo) as i16
    }

    /// Todo un bloque, del acumulador a las muestras que salen al aparato.
    pub fn bloque(&mut self, acumulador: &[i32], salida: &mut [i16]) {
        let n = acumulador.len().min(salida.len());
        for i in 0..n {
            salida[i] = self.muestra(acumulador[i]);
        }
    }
}

// ===================================================================
//  4. EL MEDIDOR: lo que DE VERDAD salio
// ===================================================================

/// **Pico y RMS de lo que paso por aqui.** Sin esto, amplificar es a ciegas.
#[derive(Clone, Copy, Debug, Default)]
pub struct Medidor {
    pico: i32,
    suma_cuadrados: u64,
    muestras: u64,
}

impl Medidor {
    pub const fn nuevo() -> Medidor {
        Medidor { pico: 0, suma_cuadrados: 0, muestras: 0 }
    }

    /// **Una sola muestra, de 32 bits.** La usa la mesa: una pista mide lo que
    /// APORTA --ya con su ganancia y antes de sumarse-- y eso no existe como
    /// bloque en ningun sitio. Guardar un bufer intermedio solo para poder
    /// medirlo seria pagar una copia por pista y por vuelta.
    pub fn mirar_uno(&mut self, muestra: i32) {
        let v = muestra.abs();
        if v > self.pico {
            self.pico = v;
        }
        self.suma_cuadrados += (v as u64) * (v as u64);
        self.muestras += 1;
    }

    /// Mira un bloque (no lo cambia).
    pub fn mirar(&mut self, muestras: &[i16]) {
        for &m in muestras {
            let v = (m as i32).abs();
            if v > self.pico {
                self.pico = v;
            }
            self.suma_cuadrados += (v as u64) * (v as u64);
            self.muestras += 1;
        }
    }

    /// El pico, lineal.
    pub fn pico(&self) -> i32 {
        self.pico
    }

    /// El pico en dBFS (1/256 de dB). `0` es pleno.
    pub fn pico_dbfs(&self) -> MilesimasDb {
        a_dbfs(self.pico)
    }

    /// El RMS en dBFS: la fuerza PERCIBIDA, que es la que dice si se oye, no
    /// el pico. Una grabacion con pico a 0 dBFS y RMS a -30 suena floja.
    pub fn rms_dbfs(&self) -> MilesimasDb {
        if self.muestras == 0 {
            return MIN_DB;
        }
        let medio = self.suma_cuadrados / self.muestras;
        // sqrt sin coma flotante: la raiz entera de un u64.
        a_dbfs(raiz(medio) as i32)
    }

    /// Cuantas muestras ha visto.
    pub fn muestras(&self) -> u64 {
        self.muestras
    }

    /// A cero, para el siguiente tramo.
    pub fn olvidar(&mut self) {
        *self = Medidor::nuevo();
    }
}

/// Raiz cuadrada entera (Newton sobre enteros).
fn raiz(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = 1u64 << ((64 - n.leading_zeros()).div_ceil(2));
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

// ===================================================================
//  5. LAS TRES JUNTAS
// ===================================================================

/// **Ganancia + limite + medidor**, que es como se usa de verdad.
#[derive(Clone, Copy, Debug)]
pub struct Amplificador {
    pub ganancia: Ganancia,
    pub limite: Limite,
    pub medidor: Medidor,
}

impl Amplificador {
    /// Uno para un aparato de `hz`, sin subir nada todavia.
    pub fn nuevo(hz: u32) -> Amplificador {
        Amplificador { ganancia: Ganancia::UNIDAD, limite: Limite::nuevo(hz), medidor: Medidor::nuevo() }
    }

    /// Poner la ganancia. Devuelve la que quedo (puede venir recortada).
    pub fn subir(&mut self, db: MilesimasDb) -> Ganancia {
        self.ganancia = Ganancia::db(db);
        self.ganancia
    }

    /// **Un bloque entero**: acumulador de 32 bits -> muestras de 16, con la
    /// ganancia puesta, el limite detras y el medidor mirando lo que SALE
    /// (que es lo unico que se oye).
    pub fn bloque(&mut self, acumulador: &[i32], salida: &mut [i16]) {
        let n = acumulador.len().min(salida.len());
        for i in 0..n {
            let con_ganancia = self.ganancia.aplicar(acumulador[i]);
            salida[i] = self.limite.muestra(con_ganancia);
        }
        self.medidor.mirar(&salida[..n]);
    }

    /// Sobre muestras que ya estan en 16 bits, en el sitio.
    pub fn en_el_sitio(&mut self, muestras: &mut [i16]) {
        for m in muestras.iter_mut() {
            let con_ganancia = self.ganancia.aplicar(*m as i32);
            *m = self.limite.muestra(con_ganancia);
        }
        self.medidor.mirar(muestras);
    }
}

/// **LA MESA**: N pistas con nombre y un maestro. Ver [`mesa`].
pub mod mesa;

/// **EL MAESTRO**: la ultima etapa, pegada al cable, con rampa y medidor por
/// lado. La corre el kernel antes del tubo. Ver [`maestro`].
pub mod maestro;

#[cfg(test)]
mod pruebas;
