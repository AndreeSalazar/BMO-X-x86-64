//! **EL CENSO** -- cada clase de bloque que un lado PRESTA a otro, con su magia.
//! El mapa de guerra de la RAM compartida: de cada uno se puede decir quien
//! escribe, quien lee, que herramienta de LA_RAM es y quien lo juzga.
//!
//! [carril]  VERDE     una tabla constante y una busqueda
//! [consumo] NADA      se consulta al TOMAR un bloque, una vez
//!
//! # Contado el 14-10 (A1 de `docs/plan/EL_FOCO.md`)
//!
//! La seccion 0a del foco decia SEIS. Al contar las magias del arbol salieron
//! OCHO: el canal de Ring 0 <-> Ring 3 (`bmo-channel`) y el bloque de computo
//! de una app (VE5C, la puerta de las apps de la 3060) tambien son bloques
//! prestados, y nadie los habia puesto al lado de los otros. Y uno de los ocho
//! (CAB2) esta declarado y ningun bloque lo usa hoy: se dice, no se esconde.
//!
//! # El juez de que este al dia
//!
//! `toolchain/tools/laminas/laminas.py`: toda constante de magia del arbol es
//! una clase de AQUI o un formato que NO se presta (su lista, con su motivo).
//! Una magia nueva sin decir cual de las dos es para el build.

/// Que herramienta de LA_RAM (Parte IX, *"que herramienta para que sitio"*)
/// es el transporte de este bloque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Herramienta {
    /// (4) PRESTAR: los dos apuntan al mismo sitio; se paga UNA vez al mapear.
    Prestar,
    /// (3) SIN ESCALA: el productor escribe donde el consumidor lee.
    SinEscala,
}

/// Que trato sigue el bloque por dentro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trato {
    /// Las dos ranuras con secuencia y SELLO por ranura ([`crate::trato`]):
    /// un escritor, un lector, nunca medio dato.
    DosRanuras,
    /// Un buzon: cada lado es propietario de SUS palabras (indices, numeros) y solo
    /// lee las del otro.
    Buzon,
    /// Texto por lineas, que el lector juzga una a una.
    Texto,
    /// Una peticion en la cabecera, que se lee una vez.
    Peticion,
    /// La superficie de una ventana: los pixeles y el buzon de entrada.
    Superficie,
}

/// Quien lee lo que otro escribio, y por tanto como se juzga.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Juez {
    /// Lo lee Ring 3: un fallo deja a ese programa sin su dato, y ya.
    Ring3,
    /// Lo lee Ring 0: las palabras se COPIAN una vez y se juzga la copia (una
    /// palabra leida dos veces puede valer dos cosas, y la segunda es la que
    /// ataca).
    Ring0,
}

/// **Una clase de bloque prestado.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clase {
    /// La primera palabra del bloque (u32, little-endian).
    pub magia: u32,
    /// Como la escribe su fichero (las letras de su comentario).
    pub nombre: &'static str,
    /// Donde vive su forma.
    pub donde: &'static str,
    pub escribe: &'static str,
    pub lee: &'static str,
    pub herramienta: Herramienta,
    pub trato: Trato,
    pub juez: Juez,
    /// `false`: declarada, y ningun bloque la usa hoy.
    pub en_uso: bool,
}

const fn letras(b: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*b)
}

/// **Las ocho**, de la mas suelta (Ring 3 con Ring 3) a la mas atada (la lee
/// el kernel).
pub const CENSO: &[Clase] = &[
    Clase {
        magia: 0x5055_5342,
        nombre: "BSUP",
        donde: "platform/abi/bmo-abi/src/syscalls/surface/superficie.rs",
        escribe: "la app (sus pixeles); el DIRECTOR (el buzon de entrada)",
        lee: "el DIRECTOR (compone); la app (sus teclas y su raton)",
        herramienta: Herramienta::Prestar,
        trato: Trato::Superficie,
        juez: Juez::Ring3,
        en_uso: true,
    },
    Clase {
        magia: 0x5245_5642,
        nombre: "BVER",
        donde: "platform/shared/verrano/src/lamina.rs",
        escribe: "la app (TITAN++ o INTI): los vertices de cada fotograma",
        lee: "VERRANO, en el escritorio: lo dibuja la 3060 o la CPU",
        herramienta: Herramienta::Prestar,
        trato: Trato::DosRanuras,
        juez: Juez::Ring3,
        en_uso: true,
    },
    Clase {
        magia: 0x494D_414C,
        nombre: "LAMI",
        donde: "platform/shared/bmo-antena/src/lamina.rs",
        escribe: "el DIRECTOR, una vez: la pagina que trajo la antena, ya juzgada",
        lee: "NAVEGAR (TITAN++), de solo lectura",
        herramienta: Herramienta::Prestar,
        trato: Trato::Texto,
        juez: Juez::Ring3,
        en_uso: true,
    },
    Clase {
        magia: 0x5550_4742,
        nombre: "BGPU",
        donde: "platform/shared/bmo-pase-gpu/src/buzon.rs",
        escribe: "el escritorio (las cajas sucias); el kernel (enviado, pagado)",
        lee: "el kernel, en cada VBLANK; el escritorio",
        herramienta: Herramienta::Prestar,
        trato: Trato::Buzon,
        juez: Juez::Ring0,
        en_uso: true,
    },
    Clase {
        magia: 0x4445_5242,
        nombre: "BRED",
        donde: "platform/shared/bmo-puerta-red/src/buzon.rs",
        escribe: "el proceso (las tramas de salida); el kernel (las de entrada)",
        lee: "el kernel; el proceso",
        herramienta: Herramienta::Prestar,
        trato: Trato::Buzon,
        juez: Juez::Ring0,
        en_uso: true,
    },
    Clase {
        magia: 0x424D_4F43,
        nombre: "BMOC",
        donde: "platform/shared/bmo-channel/src/lib.rs",
        escribe: "Ring 3 (las peticiones); Ring 0 (las respuestas)",
        lee: "Ring 0 en su tic; Ring 3",
        herramienta: Herramienta::Prestar,
        trato: Trato::Buzon,
        juez: Juez::Ring0,
        en_uso: true,
    },
    Clase {
        magia: 0x4335_4556,
        nombre: "VE5C",
        donde: "platform/drivers/gpu/ga10x/src/trabajos/completa.rs",
        escribe: "la app: lo que pide (medidas, pasadas, tolerancia) y sus datos",
        lee: "el kernel, que se lo da a la 3060 con SU programa",
        herramienta: Herramienta::SinEscala,
        trato: Trato::Peticion,
        juez: Juez::Ring0,
        en_uso: true,
    },
    Clase {
        magia: 0x4341_4232,
        nombre: "CAB2",
        donde: "platform/services/cabina-core/src/lib.rs",
        escribe: "el kernel (los eventos)",
        lee: "CABINA",
        herramienta: Herramienta::Prestar,
        trato: Trato::Buzon,
        juez: Juez::Ring3,
        en_uso: false,
    },
];

// Las letras de las que se escriben con letras en su fichero.
const _: () = assert!(CENSO[1].magia == letras(b"BVER"));
const _: () = assert!(CENSO[2].magia == letras(b"LAMI"));
const _: () = assert!(CENSO[3].magia == letras(b"BGPU"));
const _: () = assert!(CENSO[4].magia == letras(b"BRED"));
const _: () = assert!(CENSO[0].magia == letras(b"BSUP"));
const _: () = assert!(CENSO[6].magia == letras(b"VE5C"));

/// **La palabra 0 de un bloque -> su clase.** Lo que hace el que TOMA un
/// bloque antes de nada: si no esta aqui, no es una lamina.
pub const fn por_magia(palabra: u32) -> Option<&'static Clase> {
    let mut i = 0;
    while i < CENSO.len() {
        if CENSO[i].magia == palabra {
            return Some(&CENSO[i]);
        }
        i += 1;
    }
    None
}

/// Esta magia es una clase del censo? Para un `const _: () = assert!(...)`
/// en el fichero de cada clase.
pub const fn conoce(magia: u32) -> bool {
    por_magia(magia).is_some()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ni_dos_clases_con_la_misma_magia() {
        for (i, a) in CENSO.iter().enumerate() {
            for b in &CENSO[i + 1..] {
                assert_ne!(a.magia, b.magia, "{} y {} se confunden", a.nombre, b.nombre);
                assert_ne!(a.nombre, b.nombre);
            }
        }
    }

    #[test]
    fn la_palabra_cero_dice_la_clase() {
        assert_eq!(por_magia(letras(b"BVER")).map(|c| c.trato), Some(Trato::DosRanuras));
        assert_eq!(por_magia(letras(b"BGPU")).map(|c| c.juez), Some(Juez::Ring0));
        assert_eq!(por_magia(letras(b"BEF2")), None, "un ejecutable no es una lamina");
        assert_eq!(por_magia(0), None, "un bloque a cero no es nada");
    }

    /// La lamina de la antena no tiene magia escrita: es TEXTO, y su primera
    /// linea es la cabecera `LAMINA <ancho> <alto> <n>`. Sus cuatro primeras
    /// letras son su magia.
    #[test]
    fn la_de_la_antena_empieza_por_su_cabecera() {
        let p = b"LAMINA 640 100 2\n";
        assert_eq!(por_magia(u32::from_le_bytes([p[0], p[1], p[2], p[3]])).map(|c| c.nombre), Some("LAMI"));
    }

    /// Las que lee el kernel son de buzon o de peticion: nunca de dos ranuras
    /// con sello, porque el kernel no reintenta -- copia una vez y juzga --.
    #[test]
    fn el_kernel_no_lee_dos_ranuras() {
        for c in CENSO.iter().filter(|c| c.juez == Juez::Ring0) {
            assert_ne!(c.trato, Trato::DosRanuras, "{}", c.nombre);
        }
    }

    #[test]
    fn el_nombre_son_las_letras_de_la_magia() {
        for c in CENSO {
            let le = c.magia.to_le_bytes();
            let be = c.magia.to_be_bytes();
            assert!(c.nombre.as_bytes() == le || c.nombre.as_bytes() == be, "{}: {:#x}", c.nombre, c.magia);
        }
    }
}
