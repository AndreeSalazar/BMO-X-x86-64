//! **`sonidos`: el censo de los sonidos del disco entero.**
//!
//! [consumo] NADA      no corre en reposo: solo cuando se teclea. Entonces
//!                     recorre DATOS (FAT32) y ESTRATOS y lee los 64 primeros
//!                     bytes de cada fichero, por la ventana: un fichero de
//!                     GiB cuesta lo mismo que uno de 16 bytes (L6h)
//!
//! El propietario (2026-10-03): *"que encuentren TODO el disco en FAT32 y
//! ESTRATOS [...] encuentran formatos de sonidos y te dice que no son
//! oficiales y otras si"*.
//!
//! Por cada fichero que por dentro es sonido --o que por su nombre DICE serlo--
//! una linea con su veredicto, en color:
//!
//! ```text
//!    SUENA        PCM en un sobre; sonara cuando el tubo suene (A1)
//!    OFICIAL      BMO-X lo tocara; dice que falta (el MP3 es M2)
//!    NO OFICIAL   se reconoce y no se toca, con el motivo (WMA, MIDI)
//!    MIENTE       la extension dice una cosa y los bytes otra
//! ```
//!
//! y al final la cuenta. La ficha la hace `bmo_sonido::censo`, puro y probado
//! en el anfitrion; aqui solo se camina y se pinta.
//!
//! [!] El recorrido tiene techos (carpetas, hondura, ficheros) y si llega a
//! uno lo DICE: un censo recortado en silencio diria "no hay mas" sin saberlo.

use bmo_sonido::censo::{self, Cuenta, Veredicto};
use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

const RUTA: usize = 96;
const CARPETAS: usize = 256;
const HONDO: u8 = 8;
/// Cuantos ficheros se miran como mucho en una pasada.
const FICHEROS: u32 = 4000;
/// Cuantas lineas de ficha se pintan: la cuenta de abajo sigue contando.
const LINEAS: u32 = 60;

static mut COLA: [[u8; RUTA]; CARPETAS] = [[0; RUTA]; CARPETAS];
static mut COLA_LARGO: [u8; CARPETAS] = [0; CARPETAS];
static mut COLA_HONDO: [u8; CARPETAS] = [0; CARPETAS];

/// Lo que Windows deja en una particion y no es del propietario.
fn es_de_windows(nom: &[u8]) -> bool {
    nom.first() == Some(&b'$') || nom.eq_ignore_ascii_case(b"system~1") || nom.eq_ignore_ascii_case(b"system volume information")
}

pub(crate) fn sonidos(dsk: &mut Desktop, _p: &bmo::Pantalla) -> After {
    let s = &mut dsk.out.grid;
    s.text(b"  el censo de los sonidos: DATOS (FAT32) y ESTRATOS enteros\n");
    let mut cuenta = Cuenta::default();
    let mut pintadas = 0u32;
    let mut carpetas = 0u32;
    let mut recortado = false;
    // SAFETY: el hilo del escritorio, dentro de una orden; nadie mas toca la cola.
    let (cola, largo, hondo) = unsafe {
        (
            &mut *core::ptr::addr_of_mut!(COLA),
            &mut *core::ptr::addr_of_mut!(COLA_LARGO),
            &mut *core::ptr::addr_of_mut!(COLA_HONDO),
        )
    };
    largo[0] = 0;
    hondo[0] = 0;
    let mut en_cola = 1usize;
    let mut i = 0usize;
    'carpetas: while i < en_cola {
        let base = cola[i];
        let bl = largo[i] as usize;
        let d = hondo[i];
        i += 1;
        let Ok(dir) = bmo::Directorio::open(&base[..bl]) else { continue };
        carpetas += 1;
        loop {
            let mut nom = [0u8; 64];
            let Some((n, es_dir, _bytes, _estratos)) = dir.siguiente_con_origen(&mut nom) else { break };
            let nom = &nom[..n];
            if nom == b"." || nom == b".." || es_de_windows(nom) {
                continue;
            }
            let hueco = usize::from(bl > 0);
            if bl + hueco + n > RUTA {
                recortado = true;
                continue;
            }
            let mut r = [0u8; RUTA];
            r[..bl].copy_from_slice(&base[..bl]);
            if hueco == 1 {
                r[bl] = b'/';
            }
            r[bl + hueco..bl + hueco + n].copy_from_slice(nom);
            let rl = bl + hueco + n;
            if es_dir {
                if d + 1 >= HONDO || en_cola == CARPETAS {
                    recortado = true;
                    continue;
                }
                cola[en_cola] = r;
                largo[en_cola] = rl as u8;
                hondo[en_cola] = d + 1;
                en_cola += 1;
                continue;
            }
            if cuenta.ficheros >= FICHEROS {
                recortado = true;
                break 'carpetas;
            }
            // Los primeros bytes, por la ventana: sin traer el fichero entero.
            let mut cab = [0u8; censo::CABECERA];
            let leidos = match bmo::Archivo::reflejar(&r[..rl]) {
                Ok(a) => {
                    let k = a.read(&mut cab);
                    a.close();
                    k
                }
                Err(_) => continue,
            };
            let nombre = core::str::from_utf8(nom).unwrap_or("");
            let f = censo::ficha(&cab[..leidos], nombre);
            cuenta.meter(&f);
            if f.veredicto == Veredicto::NoEsSonido && !f.nombre_miente {
                continue;
            }
            if pintadas >= LINEAS {
                continue;
            }
            pintadas += 1;
            let tinta = match f.veredicto {
                Veredicto::Suena => INK_GOOD,
                Veredicto::Oficial(_) => INK_PLAIN,
                _ => INK_ERR,
            };
            s.with_ink(tinta);
            s.text(b"  ");
            s.text(f.veredicto.palabra().as_bytes());
            for _ in f.veredicto.palabra().len()..13 {
                s.byte(b' ');
            }
            s.with_ink(INK_PLAIN);
            s.text(&r[..rl]);
            s.text(b"  ");
            s.with_ink(INK_PLAIN);
            s.text(f.tipo.nombre().as_bytes());
            if f.dudoso {
                s.text(b" (por su sincronismo: podria ser casualidad)");
            }
            if f.nombre_miente {
                s.with_ink(INK_ERR);
                s.text(b"  MIENTE: su nombre dice otra cosa");
            }
            s.with_ink(INK_PLAIN);
            s.text(b"\n               ");
            s.text(f.veredicto.motivo().as_bytes());
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
    s.with_ink(INK_PLAIN);
    s.text(b"  -- ");
    s.dec(cuenta.ficheros as u64);
    s.text(b" ficheros en ");
    s.dec(carpetas as u64);
    s.text(b" carpetas: ");
    s.dec(cuenta.sonidos() as u64);
    s.text(b" sonidos\n  ");
    s.with_ink(INK_GOOD);
    s.dec(cuenta.suenan as u64);
    s.text(b" suenan");
    s.with_ink(INK_PLAIN);
    s.text(b"   ");
    s.dec(cuenta.oficiales as u64);
    s.text(b" oficiales   ");
    s.with_ink(INK_ERR);
    s.dec(cuenta.no_oficiales as u64);
    s.text(b" no oficiales   ");
    s.dec(cuenta.mienten as u64);
    s.text(b" nombres que mienten\n");
    s.with_ink(INK_PLAIN);
    if pintadas >= LINEAS {
        s.text(b"  (se pintan las 60 primeras fichas; la cuenta es de todas)\n");
    }
    if recortado {
        s.with_ink(INK_ERR);
        s.text(b"  [!] el censo llego a un techo (hondura, carpetas, ficheros o una ruta larga): hay mas disco sin mirar\n");
        s.with_ink(INK_PLAIN);
    }
    After::Settle
}
