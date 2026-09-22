//! **LA GRABADORA DEL INFORME** -- los mismos numeros que `save` pinta para
//! una persona, escritos una vez mas para una MAQUINA: `informe/DATOS.TXT`.
//!
//! [carril]  VERDE     no pregunta nada a la maquina: recibe lo que `fila` ya
//!           tenia en la mano y lo apunta. Su modo de fallo es un dato que
//!           no cabe (se cuenta, no se pierde en silencio)
//! [consumo] NADA      no corre en reposo: graba solo entre `empezar` y
//!                     `parar`, que es lo que dura un `save` (L6h)
//!
//! [cuesta]  NADA -- una linea de mas o de menos en DATOS.TXT no rompe una
//!           app, ni un dato, ni la maquina. El informe para personas sigue
//!           siendo el mismo y se escribe antes.
//!
//! [riesgo]  ESPEJO -- copia lo que pasa por `fila`/`fila_de`. Un numero que
//!           un informe escriba a mano (sin `fila`) no sale aqui, y no hay
//!           error que lo diga: se ve comparando las dos hojas.
//!
//! # Por que existe (2026-09-21)
//!
//! El informe de `save` esta escrito para leerse: columnas, colores, notas al
//! lado de cada numero. Eso es lo correcto para el propietario y esta mal para
//! cualquier cosa que quiera DECIDIR sobre el: un programa en la antena, una
//! hoja de calculo, o el escalon 0 del asistente (`PLAN_EL_ASISTENTE.md`,
//! seccion 9: una capa que toma decisiones acotadas sobre el estado de la
//! maquina, no prosa). Para eso hace falta el mismo dato sin tipografia:
//!
//! ```text
//!    consumo.escritorio = 8 MiB
//!    consumo.latido_tarde = 929 ms
//!    memoria.marcos_libres = 3914112 de 4194304
//! ```
//!
//! ** Y se consigue SIN duplicar las 91 puertas de `reports.rs`: `fila` y
//! `fila_de`, por las que pasa cada numero del informe, le dicen a la
//! grabadora lo que acaban de pintar. Mientras no hay `save`, es un `if`
//! sobre un bool y nada mas. Una hoja de datos que se escribiera aparte, con
//! su propia lista de preguntas, seria la segunda version de la verdad, y
//! divergiria de la primera en el primer campo nuevo.
//!
//! [!] Lo que NO lleva, a proposito: la nota. La nota es la PROCEDENCIA
//! ("lo que costo la vuelta anterior; si es ~ el retraso, fue el BUS") y
//! vive en la hoja para personas. Quien lea DATOS.TXT y no sepa que es
//! `latido_tarde` tiene la respuesta en CONSUMO.TXT, en la misma linea.

use bmo_userland as bmo;

/// Cuantos datos caben en un informe. El maestro de hoy ronda los 200; el
/// resto es margen, y `perdidos` dice si un dia no basta.
const MAX_DATOS: usize = 320;
/// Largo de una clave (`fila` corta a 16 columnas) y de una unidad.
const CLAVE: usize = 24;
const UNIDAD: usize = 8;

#[derive(Clone, Copy)]
struct Dato {
    capitulo: u8,
    clave: [u8; CLAVE],
    clave_n: u8,
    unidad: [u8; UNIDAD],
    unidad_n: u8,
    valor: u64,
    /// `0` = una fila con unidad; otra cosa = una fila de `X de Y`.
    total: u64,
}

const VACIO: Dato = Dato {
    capitulo: 0,
    clave: [0; CLAVE],
    clave_n: 0,
    unidad: [0; UNIDAD],
    unidad_n: 0,
    valor: 0,
    total: 0,
};

/// Los nombres de los capitulos del maestro, en su orden. El `0` es "antes
/// de cualquier capitulo" (la cabecera) y se escribe como `informe`.
const CAPITULOS: [&[u8]; 8] = [
    b"informe", b"sesion", b"maquina", b"memoria", b"consumo", b"programa", b"disco", b"autopsia",
];

static mut DATOS: [Dato; MAX_DATOS] = [VACIO; MAX_DATOS];
static mut N: usize = 0;
static mut GRABANDO: bool = false;
static mut CAPITULO: u8 = 0;
static mut PERDIDOS: u32 = 0;

/// Empieza a grabar: lo anotado antes se olvida.
pub(crate) fn empezar() {
    unsafe {
        N = 0;
        PERDIDOS = 0;
        CAPITULO = 0;
        GRABANDO = true;
    }
}

/// Deja de grabar. Lo grabado se queda para `volcar`.
pub(crate) fn parar() {
    unsafe { GRABANDO = false };
}

/// El capitulo del maestro que se esta escribiendo (1..=7); `0` = ninguno.
pub(crate) fn capitulo(n: u8) {
    unsafe { CAPITULO = n.min((CAPITULOS.len() - 1) as u8) };
}

/// Una fila con unidad, tal como `fila` la acaba de pintar.
pub(crate) fn anotar(clave: &[u8], valor: u64, unidad: &[u8]) {
    anotar_crudo(clave, valor, unidad, 0);
}

/// Una fila de `X de Y`, tal como `fila_de` la acaba de pintar.
pub(crate) fn anotar_de(clave: &[u8], valor: u64, total: u64) {
    anotar_crudo(clave, valor, b"", total);
}

#[inline(never)]
fn anotar_crudo(clave: &[u8], valor: u64, unidad: &[u8], total: u64) {
    unsafe {
        if !GRABANDO {
            return;
        }
        if N >= MAX_DATOS {
            PERDIDOS = PERDIDOS.wrapping_add(1);
            return;
        }
        let d = &mut DATOS[N];
        *d = VACIO;
        d.capitulo = CAPITULO;
        // La clave, en minusculas y con `_` donde `fila` ponia espacio: una
        // clave con espacios no la lee nadie sin comillas.
        let mut k = 0usize;
        for &b in clave.iter().take(CLAVE) {
            d.clave[k] = match b {
                // Un separador al principio (las filas sangradas empiezan
                // por espacios) o repetido no dice nada: se salta.
                b' ' | b'/' | b'-' if k == 0 || d.clave[k - 1] == b'_' => continue,
                b' ' | b'/' | b'-' => b'_',
                b'A'..=b'Z' => b + 32,
                b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' => b,
                // Todo lo demas (parentesis, dos puntos, acentos caidos) se
                // omite: la clave es un nombre, no una frase.
                _ => continue,
            };
            k += 1;
        }
        while k > 0 && d.clave[k - 1] == b'_' {
            k -= 1;
        }
        d.clave_n = k as u8;
        let u = unidad.len().min(UNIDAD);
        d.unidad[..u].copy_from_slice(&unidad[..u]);
        d.unidad_n = u as u8;
        d.valor = valor;
        d.total = total;
        N += 1;
    }
}

/// **Escribe lo grabado a `a`**, una linea por dato: `capitulo.clave = valor
/// unidad`, o `= valor de total`. Devuelve `(bytes, lineas)`.
///
/// `\r\n` por lo mismo que las otras hojas: se abre en el bloc de notas.
#[inline(never)]
pub(crate) fn volcar(a: &bmo::Archivo) -> (usize, usize) {
    let mut bytes = 0usize;
    let mut lineas = 0usize;
    let mut num = [0u8; 20];
    bytes += a.write(b"# BMO-X: los numeros del informe, para una maquina. Una linea por dato:\r\n");
    bytes += a.write(b"#   capitulo.clave = valor unidad      (o: = valor de total)\r\n");
    bytes += a.write(b"# La nota de cada dato --que es y como se lee-- esta en la hoja del capitulo.\r\n");
    lineas += 3;
    let (n, perdidos) = unsafe { (N, PERDIDOS) };
    for i in 0..n {
        let d = unsafe { DATOS[i] };
        bytes += a.write(CAPITULOS[d.capitulo as usize]);
        bytes += a.write(b".");
        bytes += a.write(&d.clave[..d.clave_n as usize]);
        bytes += a.write(b" = ");
        let k = decimal(d.valor, &mut num);
        bytes += a.write(&num[..k]);
        if d.total != 0 {
            bytes += a.write(b" de ");
            let k = decimal(d.total, &mut num);
            bytes += a.write(&num[..k]);
        } else if d.unidad_n != 0 {
            bytes += a.write(b" ");
            bytes += a.write(&d.unidad[..d.unidad_n as usize]);
        }
        bytes += a.write(b"\r\n");
        lineas += 1;
    }
    if perdidos != 0 {
        // Se dice en la propia hoja: un tope que se toca en silencio es un
        // dato que falta sin que nadie lo sepa.
        bytes += a.write(b"# TOPE: se quedaron fuera ");
        let k = decimal(perdidos as u64, &mut num);
        bytes += a.write(&num[..k]);
        bytes += a.write(b" datos (subir MAX_DATOS en datos.rs)\r\n");
        lineas += 1;
    }
    (bytes, lineas)
}

/// Cuantos datos hay grabados.
pub(crate) fn cuantos() -> usize {
    unsafe { N }
}

/// `v` en decimal ASCII, sin ceros a la izquierda. Devuelve cuantos bytes.
fn decimal(mut v: u64, out: &mut [u8; 20]) -> usize {
    if v == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut n = 0;
    while v > 0 {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    for i in 0..n {
        out[i] = tmp[n - 1 - i];
    }
    n
}
