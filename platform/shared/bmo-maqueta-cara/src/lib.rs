//! # CARA -- el formato de una maquetacion que VIAJA
//!
//! generacion: abuelo -- no depende de nadie
//!
//! Lo que `PLAN_LA_CARA_VIAJA.md` pide en su escalon 1: *"el FORMATO en un crate
//! sin E/S (como `estratos` y `trim`: **un formato mal empaquetado no da un
//! fallo, da algo peor -- se lee mal y nadie se entera**)"*.
//!
//! ## Que es una cara
//!
//! Una maquetacion **ya resuelta**: rectangulos con sus colores y sus letras, en
//! orden de pintado. Las cinco generaciones de MAQUETA corrieron en el
//! anfitrion y lo caro se quedo alli; lo que viaja es el resultado.
//!
//! ```text
//!    un navegador   manda el documento Y trae el motor que lo maqueta
//!    una CARA       manda solo el resultado. El aparato no maqueta
//! ```
//!
//! Por eso cabe en menos de un kilobyte: la cara entera de la calculadora --28
//! cajas, 17 textos, 1 isla-- son ~950 bytes.
//!
//! ## ** POR QUE EL LECTOR DESCONFIA, Y NO ES PARANOIA
//!
//! El veredicto de MAQUETA --sus diez comprobaciones-- corre en el anfitrion
//! sobre el `.maqueta`. **Y no viaja con la cara.** Un recurso que llega de un
//! fichero editado a mano, del `.bex` de otro o de la red **no ha pasado por
//! ningun juez**, y el que lo lee es el compositor.
//!
//! El precedente esta escrito en `PLAN_DIRECTOR.md`, palabra por palabra:
//!
//! > `Cabecera::leer` valida ancho/alto/stride **contra los bytes que dijo el
//! > kernel**, en `u64` (en 32 bits el producto se desborda y da un total
//! > chico). Sin eso, una app que declare 4000x4000 en 1 MiB hace que el
//! > compositor lea fuera del prestamo.
//!
//! *** Y LAS DOS LISTAS SON DISTINTAS, que es lo que hay que tener claro:
//!
//! ```text
//!    el VEREDICTO   juzga si la maquetacion es BUENA (el texto cabe, la caja
//!                   no se sale de su padre). Se puede quedar en el anfitrion
//!    el LECTOR      comprueba si el fichero es SEGURO DE LEER. Siempre, y
//!                   tambien cuando el fichero lo hizo uno mismo
//! ```
//!
//! **Un recurso corrupto no debe dar un `#PF` en el compositor. Una app rota no
//! se lleva el escritorio** -- la misma ley que ya rige las superficies.
//!
//! ## El plano, y por que en este orden
//!
//! ```text
//!    cabecera    20 B     magico, version, lienzo, y las tres cuentas
//!    trazos      20 B c/u lo que se pinta, en orden de pintado
//!    golpes      12 B c/u donde se puede pulsar, y como se llama
//!    cadenas     N B      los textos y los nombres, uno detras de otro
//! ```
//!
//! Las cuentas van **todas en la cabecera** y los bloques son de medida fijo, asi
//! que **donde empieza cada uno se sabe sin recorrer nada**. Un formato que
//! obligue a recorrer para localizar es un formato que hay que recorrer con
//! datos que todavia no se han comprobado.

#![no_std]
#![forbid(unsafe_code)]

/// `CARA` en little-endian. Cuatro bytes que no son texto por casualidad.
pub const MAGICO: u32 = u32::from_le_bytes(*b"CARA");

/// La unica version que este lector entiende.
///
/// [!] Se compara por IGUALDAD y no por "mayor o igual". Un lector que acepte
/// versiones futuras esta prometiendo entender algo que todavia no existe.
///
/// ** La 2 (04-10, MAQUETA 2): las piezas SUAVES -- caja redonda, borde,
/// resplandor, degradado, la letra de la casa y los caminos de SVG. El plano
/// es el mismo; el campo reservado del trazo pasa a llevar lo de cada clase
/// (`trazo::EXTRA`), y una cara de la 1 ya no se abre: se vuelve a emitir.
pub const VERSION: u16 = 2;

/// Bytes de la cabecera.
pub const CABECERA: usize = 20;
/// Bytes de un trazo.
pub const TRAZO: usize = 20;
/// Bytes de un golpe.
pub const GOLPE: usize = 12;

/// Offsets dentro de la cabecera. Los declara este crate y **nadie mas**: si el
/// emisor tuviera los suyos, serian dos formatos con el mismo nombre.
pub mod cabecera {
    pub const MAGICO: usize = 0;
    pub const VERSION: usize = 4;
    pub const ANCHO: usize = 6;
    pub const ALTO: usize = 8;
    pub const N_TRAZOS: usize = 10;
    pub const N_GOLPES: usize = 12;
    pub const CADENAS: usize = 14;
    /// Tiene que ser CERO. Ver [`super::Falta::ReservadoSucio`].
    pub const RESERVADO: usize = 16;
}

/// Offsets dentro de un trazo.
pub mod trazo {
    pub const CLASE: usize = 0;
    pub const ESTADO: usize = 1;
    pub const X: usize = 2;
    pub const Y: usize = 4;
    pub const W: usize = 6;
    pub const H: usize = 8;
    pub const COLOR: usize = 10;
    pub const CAD_OFF: usize = 14;
    pub const CAD_LEN: usize = 16;
    /// Lo de cada clase (radio, grosor, estilo de la letra). En `RECT` y
    /// `TEXTO` tiene que ser CERO: era el reservado de la version 1.
    pub const EXTRA: usize = 18;
    pub const RESERVADO: usize = EXTRA;
}

/// Offsets dentro de un golpe.
pub mod golpe {
    pub const X: usize = 0;
    pub const Y: usize = 2;
    pub const W: usize = 4;
    pub const H: usize = 6;
    pub const CAD_OFF: usize = 8;
    pub const CAD_LEN: usize = 10;
}

/// Un rectangulo macizo.
pub const CLASE_RECT: u8 = 0;
/// Letras de PIXEL (8 x 16).
pub const CLASE_TEXTO: u8 = 1;
/// Caja de esquinas redondas. `EXTRA` = radio.
pub const CLASE_CAJA: u8 = 2;
/// Borde de una caja redonda. `EXTRA` = radio | grosor << 8.
pub const CLASE_BORDE: u8 = 3;
/// Resplandor alrededor de la caja. `COLOR` = `0xAARRGGBB` (el alfa es la
/// fuerza); `EXTRA` = radio | alcance << 8.
pub const CLASE_RESPLANDOR: u8 = 4;
/// Degradado. `COLOR` = el de un lado; los datos (4 bytes) = el del otro;
/// `EXTRA` = radio | vertical << 15.
pub const CLASE_DEGRADADO: u8 = 5;
/// La letra de la casa. Los datos = el texto; `H` = su `line-height`;
/// `EXTRA` = talla (7 bits) | peso << 7 (0..=3: 400, 500, 600, 700) |
/// mayusculas << 9 | espacio << 10 (centesimas de eme, 0..=63).
pub const CLASE_LETRA: u8 = 6;
/// Un camino con pluma. Los datos = sus puntos (ver [`puntos`]); `EXTRA` =
/// el grosor, en 1/16 de pixel.
pub const CLASE_LINEA: u8 = 7;
/// Un camino relleno (par-impar). Los datos = sus puntos.
pub const CLASE_RELLENO: u8 = 8;
/// **Una figura de SVG** (MAQUETA 3, 06-10): pluma redonda o relleno con
/// su regla, su TINTA y su opacidad. `EXTRA` = par-impar (bit 0) | alfa << 8
/// (los bits 1..=7, a cero). `COLOR` = la tinta lisa (0 con un degradado).
/// Los datos, ver [`figura`].
pub const CLASE_FIGURA: u8 = 9;

/// **Los datos de una `FIGURA`**, en este orden (todo multiplo de 4, asi
/// los puntos empiezan alineados como en una `LINEA`):
///
/// ```text
///    0   tinta      u8   0 lisa, 1 lineal, 2 radial
///    1   paradas    u8   0 en la lisa; 1..=8 en un degradado
///    2   pluma      u16  grosor en 1/16 px; 0 = relleno
///    4   geometria       lineal: de (i16 x, y), a (i16 x, y)       8 B
///                        radial: centro, eje_x, eje_y (i16 x, y)  12 B
///        paradas         cada una: en u16 (0..=1000, sin bajar),
///                        color u32 (0x00RRGGBB), alfa u8, cero u8   8 B
///        puntos          como los de una LINEA (ver [`puntos`])
/// ```
///
/// La geometria del degradado va en 1/16 px relativa al trazo, como los
/// puntos, pero PUEDE caer fuera de su caja: es donde el degradado vale 0 y
/// 1, no algo que se pinte.
pub mod figura {
    pub const LISA: u8 = 0;
    pub const LINEAL: u8 = 1;
    pub const RADIAL: u8 = 2;
    /// Las paradas que caben, como en el pintor.
    pub const PARADAS: usize = 8;
    /// Lo que mide una parada.
    pub const PARADA: usize = 8;
}

/// **Una figura descodificada** (los datos ya comprobados por [`leer`]).
#[derive(Clone, Copy, Debug)]
pub struct Figura<'a> {
    pub tinta: u8,
    /// Grosor de la pluma en 1/16 px; 0 = relleno.
    pub pluma16: u16,
    /// La geometria: lineal `[de.x, de.y, a.x, a.y, 0, 0]`; radial
    /// `[centro.x, centro.y, eje_x.x, eje_x.y, eje_y.x, eje_y.y]`. En 1/16 px.
    pub geo: [i16; 6],
    /// Las paradas, sin descodificar: `n * 8` bytes.
    pub paradas: &'a [u8],
    /// Los puntos, como los de una `LINEA`.
    pub puntos: &'a [u8],
}

/// **Parte los datos de una `FIGURA`** en sus trozos. `None` si no tienen
/// la forma (el lector ya lo comprobo; esto lo repite sin coste para quien
/// pinte una figura que no paso por el).
pub fn figura_de(d: &[u8]) -> Option<Figura<'_>> {
    let (&tinta, &n) = (d.first()?, d.get(1)?);
    let pluma16 = u16::from_le_bytes([*d.get(2)?, *d.get(3)?]);
    let geo_len = match (tinta, n) {
        (figura::LISA, 0) => 0,
        (figura::LINEAL, 1..=8) => 8,
        (figura::RADIAL, 1..=8) => 12,
        _ => return None,
    };
    let mut geo = [0i16; 6];
    for (k, g) in geo.iter_mut().enumerate().take(geo_len / 2) {
        *g = i16::from_le_bytes([*d.get(4 + 2 * k)?, *d.get(5 + 2 * k)?]);
    }
    let ini_p = 4 + geo_len;
    let fin_p = ini_p + n as usize * figura::PARADA;
    Some(Figura { tinta, pluma16, geo, paradas: d.get(ini_p..fin_p)?, puntos: d.get(fin_p..)? })
}

/// La parada `k` de unas paradas sin descodificar: `(en, color, alfa)`.
pub fn parada(paradas: &[u8], k: usize) -> Option<(u16, u32, u8)> {
    let p = paradas.get(k * figura::PARADA..(k + 1) * figura::PARADA)?;
    Some((u16::from_le_bytes([p[0], p[1]]), u32::from_le_bytes([p[2], p[3], p[4], p[5]]), p[6]))
}

/// Los datos de una figura: su forma, sus paradas en orden y sus puntos
/// dentro de la caja.
fn figura_valida(d: &[u8], w: u16, h: u16, color: u32, extra: u16) -> Result<(), Falta> {
    if extra & 0x00FE != 0 {
        return Err(Falta::ReservadoSucio);
    }
    let f = figura_de(d).ok_or(Falta::DatosMal)?;
    if f.tinta != figura::LISA && color != 0 {
        return Err(Falta::DatosMal);
    }
    let mut antes = 0u16;
    for k in 0..f.paradas.len() / figura::PARADA {
        let (en, c, _) = parada(f.paradas, k).ok_or(Falta::DatosMal)?;
        if en > 1000 || en < antes || c >> 24 != 0 || f.paradas[k * figura::PARADA + 7] != 0 {
            return Err(Falta::DatosMal);
        }
        antes = en;
    }
    puntos_validos(f.puntos, w, h)
}

/// **Los puntos de un camino**, en los datos de una `LINEA` o un `RELLENO`:
/// pares `(i16 x, i16 y)` en 1/16 de pixel, RELATIVOS a la esquina del
/// trazo, y antes de cada subcamino un separador `(i16::MIN, cerrado)`.
pub mod puntos {
    /// El primer campo de un separador.
    pub const SEPARA: i16 = i16::MIN;
}

/// Se pinta siempre.
pub const ESTADO_REPOSO: u8 = 0;
/// Solo mientras el puntero esta encima.
pub const ESTADO_ENCIMA: u8 = 1;

/// **Por que esta cara no se puede leer.**
///
/// Una variante por motivo y no un `bool`, por lo mismo que en `bmo-bex-gate`:
/// *"no se pudo"* manda a mirar el fichero entero, y el nombre manda al campo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Falta {
    /// No hay ni cabecera.
    NoLlegaNiALaCabecera,
    /// Los cuatro bytes del principio no son `CARA`.
    NoEsUnaCara,
    /// Es una cara de otra version.
    OtraVersion,
    /// Un campo que tiene que ser cero no lo es. **Es la signal mas barata de que
    /// el fichero viene de otro sitio**: nadie escribe basura ahi por accidente,
    /// y un emisor futuro que use ese hueco tendra que subir la version.
    ReservadoSucio,
    /// Las cuentas declaradas piden mas bytes de los que hay. **La comprobacion
    /// de la que penden todas las demas.**
    LasCuentasNoCaben,
    /// Una cadena se sale del bloque de cadenas.
    CadenaFuera,
    /// Un rectangulo se sale del lienzo que la propia cara declara.
    TrazoFueraDelLienzo,
    /// El lienzo declarado no cabe en la pantalla que hay.
    LienzoMasGrandeQueLaPantalla,
    /// El lienzo es de ancho o alto cero: no se puede pintar nada y **todo rect
    /// se saldria**, o sea que el error de verdad seria el de al lado.
    LienzoVacio,
    /// Una clase de trazo que esta version no tiene.
    ClaseDesconocida,
    /// Los datos de un trazo no tienen la forma de su clase (un degradado sin
    /// su segundo color, un camino partido o con un punto fuera de su caja).
    DatosMal,
}

/// Una cara ya comprobada. **Solo se construye pasando por [`leer`]**, asi que
/// tener una es la prueba de que las cinco comprobaciones se hicieron.
///
/// * Ese es el punto entero del tipo, y es el mismo truco que `Revisada` en
/// `bmo-bex-gate`: si el lector devolviera `&[u8]` y una lista de avisos, nada
/// impediria pintar sin mirarlos.
#[derive(Clone, Copy)]
pub struct Cara<'a> {
    bytes: &'a [u8],
    ancho: u16,
    alto: u16,
    n_trazos: usize,
    n_golpes: usize,
    cadenas_off: usize,
    cadenas_len: usize,
}

/// **A mano, y no `derive`.** Un `derive` volcaria el buffer entero -- cientos
/// de bytes de ruido en el mensaje de un test que fallo por un campo. Lo que
/// hace falta ver cuando esto sale impreso es **que decia la cabecera**.
impl core::fmt::Debug for Cara<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Cara {}x{} ({} trazos, {} golpes, {} B de cadenas)",
            self.ancho, self.alto, self.n_trazos, self.n_golpes, self.cadenas_len
        )
    }
}

/// Un trazo ya descodificado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pincelada<'a> {
    pub clase: u8,
    pub estado: u8,
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    pub color: u32,
    /// Las letras (`TEXTO` y `LETRA`). Vacio en las demas.
    pub texto: &'a [u8],
    /// Lo de cada clase (ver las `CLASE_*`).
    pub extra: u16,
    /// Los datos del trazo en el bloque de cadenas, YA COMPROBADOS contra su
    /// clase: el texto, el segundo color, o los puntos.
    pub datos: &'a [u8],
}

/// Una region que se puede pulsar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pulsable<'a> {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    /// Como se llama. Es lo que el programa recibe cuando alguien pulsa aqui.
    pub nombre: &'a [u8],
}

fn u16_en(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}
fn i16_en(b: &[u8], i: usize) -> Option<i16> {
    u16_en(b, i).map(|v| v as i16)
}
fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

/// **ABRIR UNA CARA, DESCONFIANDO.** Las cinco comprobaciones de
/// `PLAN_LA_CARA_VIAJA.md` seccion 6, en el orden en que se sostienen.
///
/// `pantalla_*` es lo que hay de verdad. Se pasa y no se supone: el mismo
/// recurso es legal en una pantalla y absurdo en otra, y **el fichero no puede
/// ser quien diga cual tiene delante**.
///
/// # El orden no es estetico
///
/// ```text
///    1. el magico y la version     sin esto lo demas ni siquiera son campos
///    2. las CUENTAS caben          <- de esta penden todas las siguientes
///    3. cada cadena cae dentro
///    4. ningun rect se sale del lienzo declarado
///    5. el lienzo cabe en la pantalla
/// ```
///
/// *** La 2 va antes que la 3 y la 4 **porque las otras dos leen usando esas
/// cuentas**. Comprobar que un rect cabe leyendo el rect de una tabla cuyo
/// medida no se ha comprobado es hacer la pregunta con la respuesta ya perdida.
pub fn leer(bytes: &[u8], pantalla_ancho: u16, pantalla_alto: u16) -> Result<Cara<'_>, Falta> {
    // -- 1. Que esto sea una cara, y de esta version ------------------------
    if bytes.len() < CABECERA {
        return Err(Falta::NoLlegaNiALaCabecera);
    }
    if u32_en(bytes, cabecera::MAGICO) != Some(MAGICO) {
        return Err(Falta::NoEsUnaCara);
    }
    if u16_en(bytes, cabecera::VERSION) != Some(VERSION) {
        return Err(Falta::OtraVersion);
    }
    if u32_en(bytes, cabecera::RESERVADO) != Some(0) {
        return Err(Falta::ReservadoSucio);
    }

    let ancho = u16_en(bytes, cabecera::ANCHO).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let alto = u16_en(bytes, cabecera::ALTO).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let n_trazos = u16_en(bytes, cabecera::N_TRAZOS).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    let n_golpes = u16_en(bytes, cabecera::N_GOLPES).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    let cadenas_len = u16_en(bytes, cabecera::CADENAS).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;

    // -- 2. *** QUE LAS CUENTAS QUEPAN, Y LA CUENTA SE HACE EN u64 ----------
    //
    // Es la comprobacion de la que penden las otras tres, y es la que el
    // precedente del compositor dice como hacer: **en `u64`**. Con `usize` de 32
    // bits, `n_trazos * TRAZO` de una cabecera hostil da la vuelta y contesta un
    // total chico -- que pasa la comprobacion y luego lee fuera.
    //
    // Aqui `usize` son 64 en las dos maquinas de hoy, y aun asi se hace en `u64`
    // explicito: **la correccion no puede depender de en que maquina se compila.**
    let falta = CABECERA as u64
        + (n_trazos as u64) * (TRAZO as u64)
        + (n_golpes as u64) * (GOLPE as u64)
        + cadenas_len as u64;
    if falta > bytes.len() as u64 {
        return Err(Falta::LasCuentasNoCaben);
    }

    let trazos_off = CABECERA;
    let golpes_off = trazos_off + n_trazos * TRAZO;
    let cadenas_off = golpes_off + n_golpes * GOLPE;

    // -- 5. El lienzo, antes de mirar ningun rect --------------------------
    //
    // Se adelanta a la 3 y la 4 porque un lienzo de cero hace que **todo** rect
    // se salga, y entonces el error que saldria seria `TrazoFueraDelLienzo` --
    // que manda a mirar los trazos cuando el roto es el lienzo.
    if ancho == 0 || alto == 0 {
        return Err(Falta::LienzoVacio);
    }
    if ancho > pantalla_ancho || alto > pantalla_alto {
        return Err(Falta::LienzoMasGrandeQueLaPantalla);
    }

    let cara = Cara {
        bytes,
        ancho,
        alto,
        n_trazos,
        n_golpes,
        cadenas_off,
        cadenas_len,
    };

    // -- 3 y 4. Cada trazo y cada golpe, uno por uno ------------------------
    //
    // ** SE COMPRUEBAN TODOS AL ABRIR y no al pintar. Un lector que validara
    // sobre la marcha dejaria la mitad del dibujo hecho antes de descubrir que
    // el fichero estaba roto -- y entonces "una app rota no se lleva el
    // escritorio" seria falso a medias: no lo tumba, pero lo ensucia.
    for i in 0..n_trazos {
        let b = trazos_off + i * TRAZO;
        let clase = *bytes.get(b + trazo::CLASE).ok_or(Falta::LasCuentasNoCaben)?;
        if clase > CLASE_FIGURA {
            return Err(Falta::ClaseDesconocida);
        }
        if (clase == CLASE_RECT || clase == CLASE_TEXTO) && u16_en(bytes, b + trazo::EXTRA) != Some(0) {
            return Err(Falta::ReservadoSucio);
        }
        let off = u16_en(bytes, b + trazo::CAD_OFF).ok_or(Falta::LasCuentasNoCaben)? as usize;
        let len = u16_en(bytes, b + trazo::CAD_LEN).ok_or(Falta::LasCuentasNoCaben)? as usize;
        cara.cadena_valida(off, len)?;
        let x = i16_en(bytes, b + trazo::X).ok_or(Falta::LasCuentasNoCaben)?;
        let y = i16_en(bytes, b + trazo::Y).ok_or(Falta::LasCuentasNoCaben)?;
        let w = u16_en(bytes, b + trazo::W).ok_or(Falta::LasCuentasNoCaben)?;
        let h = u16_en(bytes, b + trazo::H).ok_or(Falta::LasCuentasNoCaben)?;
        cara.rect_dentro(x, y, w, h)?;
        // -- La forma de los datos, por clase ---------------------------------
        let datos = cara.cadena(off, len);
        match clase {
            CLASE_DEGRADADO if len != 4 => return Err(Falta::DatosMal),
            CLASE_LINEA | CLASE_RELLENO => puntos_validos(datos, w, h)?,
            CLASE_FIGURA => {
                let color = u32_en(bytes, b + trazo::COLOR).ok_or(Falta::LasCuentasNoCaben)?;
                let extra = u16_en(bytes, b + trazo::EXTRA).ok_or(Falta::LasCuentasNoCaben)?;
                figura_valida(datos, w, h, color, extra)?
            }
            _ => {}
        }
    }
    for i in 0..n_golpes {
        let b = golpes_off + i * GOLPE;
        let off = u16_en(bytes, b + golpe::CAD_OFF).ok_or(Falta::LasCuentasNoCaben)? as usize;
        let len = u16_en(bytes, b + golpe::CAD_LEN).ok_or(Falta::LasCuentasNoCaben)? as usize;
        cara.cadena_valida(off, len)?;
        let x = i16_en(bytes, b + golpe::X).ok_or(Falta::LasCuentasNoCaben)?;
        let y = i16_en(bytes, b + golpe::Y).ok_or(Falta::LasCuentasNoCaben)?;
        let w = u16_en(bytes, b + golpe::W).ok_or(Falta::LasCuentasNoCaben)?;
        let h = u16_en(bytes, b + golpe::H).ok_or(Falta::LasCuentasNoCaben)?;
        cara.rect_dentro(x, y, w, h)?;
    }

    Ok(cara)
}

impl<'a> Cara<'a> {
    /// El lienzo que esta cara declara.
    pub fn lienzo(&self) -> (u16, u16) {
        (self.ancho, self.alto)
    }
    /// Cuantos trazos trae.
    pub fn trazos(&self) -> usize {
        self.n_trazos
    }
    /// Cuantas regiones pulsables trae.
    pub fn golpes(&self) -> usize {
        self.n_golpes
    }

    /// **Cabe esta cadena en el bloque de cadenas?** En `u64`, por lo mismo que
    /// las cuentas: `off + len` con dos `u16` no desborda hoy, pero la regla no
    /// puede depender de que los campos sigan siendo de 16 bits.
    fn cadena_valida(&self, off: usize, len: usize) -> Result<(), Falta> {
        if off as u64 + len as u64 > self.cadenas_len as u64 {
            return Err(Falta::CadenaFuera);
        }
        Ok(())
    }

    /// **Se sale este rect del lienzo declarado?**
    ///
    /// [!] En `i64` y no en `i32`: `x` es `i16` y `w` es `u16`, asi que la suma
    /// cabe de sobra -- pero el dia que alguien suba los campos a 32 bits, esta
    /// linea sigue siendo correcta en vez de empezar a mentir en silencio.
    fn rect_dentro(&self, x: i16, y: i16, w: u16, h: u16) -> Result<(), Falta> {
        if x < 0 || y < 0 {
            return Err(Falta::TrazoFueraDelLienzo);
        }
        if x as i64 + w as i64 > self.ancho as i64 || y as i64 + h as i64 > self.alto as i64 {
            return Err(Falta::TrazoFueraDelLienzo);
        }
        Ok(())
    }

    fn cadena(&self, off: usize, len: usize) -> &'a [u8] {
        let a = self.cadenas_off + off;
        self.bytes.get(a..a + len).unwrap_or(&[])
    }

    /// El trazo `i`, ya descodificado. `None` fuera de rango.
    ///
    /// * No entra en panico y no devuelve basura: quien pinte en un bucle no
    /// tiene por que volver a comprobar el limite que este tipo ya conoce.
    pub fn trazo(&self, i: usize) -> Option<Pincelada<'a>> {
        if i >= self.n_trazos {
            return None;
        }
        let b = CABECERA + i * TRAZO;
        let clase = *self.bytes.get(b + trazo::CLASE)?;
        let off = u16_en(self.bytes, b + trazo::CAD_OFF)? as usize;
        let len = u16_en(self.bytes, b + trazo::CAD_LEN)? as usize;
        Some(Pincelada {
            clase,
            estado: *self.bytes.get(b + trazo::ESTADO)?,
            x: i16_en(self.bytes, b + trazo::X)?,
            y: i16_en(self.bytes, b + trazo::Y)?,
            w: u16_en(self.bytes, b + trazo::W)?,
            h: u16_en(self.bytes, b + trazo::H)?,
            color: u32_en(self.bytes, b + trazo::COLOR)?,
            texto: if clase == CLASE_TEXTO || clase == CLASE_LETRA {
                self.cadena(off, len)
            } else {
                &[]
            },
            extra: u16_en(self.bytes, b + trazo::EXTRA)?,
            datos: self.cadena(off, len),
        })
    }

    /// La region pulsable `i`. `None` fuera de rango.
    pub fn golpe(&self, i: usize) -> Option<Pulsable<'a>> {
        if i >= self.n_golpes {
            return None;
        }
        let b = CABECERA + self.n_trazos * TRAZO + i * GOLPE;
        let off = u16_en(self.bytes, b + golpe::CAD_OFF)? as usize;
        let len = u16_en(self.bytes, b + golpe::CAD_LEN)? as usize;
        Some(Pulsable {
            x: i16_en(self.bytes, b + golpe::X)?,
            y: i16_en(self.bytes, b + golpe::Y)?,
            w: u16_en(self.bytes, b + golpe::W)?,
            h: u16_en(self.bytes, b + golpe::H)?,
            nombre: self.cadena(off, len),
        })
    }
}

/// Los puntos de un camino: pares enteros, el primero un separador, y cada
/// punto DENTRO de la caja de su trazo (en 1/16 de pixel).
fn puntos_validos(d: &[u8], w: u16, h: u16) -> Result<(), Falta> {
    if d.is_empty() || d.len() % 4 != 0 {
        return Err(Falta::DatosMal);
    }
    let (wm, hm) = (w as i32 * 16, h as i32 * 16);
    for (k, par) in d.chunks_exact(4).enumerate() {
        let x = i16::from_le_bytes([par[0], par[1]]);
        let y = i16::from_le_bytes([par[2], par[3]]);
        if x == puntos::SEPARA {
            if y != 0 && y != 1 {
                return Err(Falta::DatosMal);
            }
            continue;
        }
        if k == 0 || x < 0 || y < 0 || x as i32 > wm || y as i32 > hm {
            return Err(Falta::DatosMal);
        }
    }
    Ok(())
}

/// **Recorre los subcaminos** de los datos (ya comprobados) de una `LINEA`
/// o un `RELLENO`: `f(cerrado, puntos)`, con los puntos en 1/16 de pixel
/// relativos al trazo, como pares de `i16` sin decodificar.
pub fn subcaminos<'a>(datos: &'a [u8], mut f: impl FnMut(bool, &'a [u8])) {
    let mut ini = 0;
    let mut cerrado = false;
    let pares = datos.len() / 4;
    for k in 0..=pares {
        let separa = k == pares || i16::from_le_bytes([datos[k * 4], datos[k * 4 + 1]]) == puntos::SEPARA;
        if separa {
            if k > ini {
                f(cerrado, &datos[ini * 4..k * 4]);
            }
            if k < pares {
                cerrado = datos[k * 4 + 2] == 1;
            }
            ini = k + 1;
        }
    }
}

#[cfg(test)]
mod tests;
