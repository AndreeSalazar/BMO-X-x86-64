//! **EL ESTILO que se lee de `sys/director.cfg`** (2026-09-13).
//!
//! [consumo] NADA      lee el fichero UNA vez, al arrancar (L6h)
//!
//! Eddi: *"puede haber su propia configuracion?"*. Los valores de partida son los
//! de siempre (`tema.maqueta`); el fichero pisa los que diga, y lo que no se
//! entienda sale en la consola de Ejecutar con su linea. La lectura y sus reglas
//! viven en `bmo-config`, probadas contra ficheros rotos.
//!
//! ** El acento lo leen TODAS las ventanas desde el 2026-09-13 (`scene::acento`):
//! la barra, Ejecutar, las apps, la seleccion del F12 y el conmutador. El fondo,
//! la barra y sus widgets tambien. Lo que NO lee de aqui: los colores propios de
//! cada ventana (el verde de ESTRATOS, el ambar de CABINA), que dicen QUE ventana
//! es y no son un tema.

use bmo_config::{Estilo, Informe};
use bmo_userland as bmo;
use core::ptr::{addr_of, addr_of_mut};

use super::output::Output;
use super::{ACCENT_BASE, BG_BOTTOM, BG_TOP, TASKBAR, TASKBAR_LINE};

pub(crate) const POR_DEFECTO: Estilo = Estilo {
    acento: ACCENT_BASE,
    fondo_arriba: BG_TOP,
    fondo_abajo: BG_BOTTOM,
    barra_fondo: TASKBAR,
    barra_borde: TASKBAR_LINE,
    barra_flotante: true,
    barra_hueco: 6,
    reloj: true,
    vatios: true,
    memoria: true,
    cpu: true,
    fondo_imagen: bmo_config::Ruta::VACIA,
    bienvenida: true,
    // ** El de 04-10, "la elegancia de Francia". El hacker de 25-09 sigue a
    // una linea: `marco = hacker` en `sys/director.cfg`.
    marco: bmo_config::Marco::Fino,
    // ** El de mision (06-10, HM3): la estrella gato y su cielo. El degradado
    // de siempre sigue a una linea: `fondo = degradado`.
    fondo: bmo_config::FONDO_POR_DEFECTO,
};

pub(crate) const RUTA: &[u8] = b"sys/director.cfg";
const TOPE: usize = 4096;

/// Lo CONFIGURADO: lo que dice `sys/director.cfg` y lo que el editor de
/// aspecto cambia y guarda.
static mut ESTILO: Estilo = POR_DEFECTO;
/// Lo VISTO: lo configurado, con los colores del MODO FASE encima si toca. Es
/// lo que lee todo el escritorio; asi FASE no se cuela en el `.cfg`.
static mut VISTO: Estilo = POR_DEFECTO;
static mut INFORME: Informe = Informe::VACIO;
static mut LEIDO: bool = false;

/// **El estilo que se ve**: el configurado, con el MODO FASE aplicado.
pub(crate) fn estilo() -> &'static Estilo {
    unsafe { &*addr_of!(VISTO) }
}

/// **El estilo configurado**, sin los colores de FASE: el que se edita y se
/// guarda.
pub(crate) fn configurado() -> &'static Estilo {
    unsafe { &*addr_of!(ESTILO) }
}

/// Cambia el estilo en vivo. Lo usa el editor de aspecto; quien llame repinta.
pub(crate) fn poner(e: Estilo) {
    unsafe {
        *addr_of_mut!(ESTILO) = e;
        *addr_of_mut!(VISTO) = visto(e);
    }
}

/// ** EL MODO FASE (04-10): el acento, el panel y su filo pasan a los colores
/// del gato de `tema.maqueta` (`.fase`: su cian, y el magenta para el glitch). Las ventanas leen el acento, asi que sus marcos
/// tambien. El fondo del escritorio y los colores propios de cada ventana no
/// cambian: dicen QUE es cada cosa, no son tema.
fn visto(e: Estilo) -> Estilo {
    use super::tema_gen::{FASE_BORDE, FASE_CIAN, FASE_FONDO};
    if e.marco != bmo_config::Marco::Fase {
        return e;
    }
    Estilo { acento: FASE_CIAN, barra_fondo: FASE_FONDO, barra_borde: FASE_BORDE, ..e }
}

/// Esta puesto el MODO FASE?
pub(crate) fn fase() -> bool {
    estilo().marco == bmo_config::Marco::Fase
}

/// Lee `sys/director.cfg`. Si no esta, se queda lo de siempre: no tener fichero
/// no es un fallo, es no haber cambiado nada.
pub(crate) fn cargar() {
    let Ok(a) = bmo::Archivo::leer_de(RUTA) else { return };
    let mide = (a.size() as usize).min(TOPE);
    let mut buf = [0u8; TOPE];
    let n = a.read(&mut buf[..mide]);
    let mut e = POR_DEFECTO;
    let inf = e.aplicar(&buf[..n]);
    unsafe {
        *addr_of_mut!(ESTILO) = e;
        *addr_of_mut!(VISTO) = visto(e);
        *addr_of_mut!(INFORME) = inf;
        LEIDO = true;
    }
}

/// **Cuenta en la consola lo que dijo el fichero**: cuantos ajustes, y cada
/// linea que no se entendio con su motivo.
pub(crate) fn contar_en(grid: &mut Output) {
    if !unsafe { LEIDO } {
        return;
    }
    let inf = unsafe { &*addr_of!(INFORME) };
    let mut b = [0u8; 10];
    let n = crate::text::decimal(inf.aplicadas as u64, &mut b);
    grid.text(b"director.cfg: ");
    grid.text(&b[..n]);
    grid.text(b" ajuste(s)\n");
    for f in inf.fallos() {
        let n = crate::text::decimal(f.linea as u64, &mut b);
        grid.text(b"  director.cfg linea ");
        grid.text(&b[..n]);
        grid.text(b": ");
        grid.text(f.motivo.texto().as_bytes());
        grid.text(b"\n");
    }
    if inf.recortado {
        grid.text(b"  (y mas lineas mal: solo se muestran ocho)\n");
    }
    // La foto de fondo se pidio y no salio: se dice por que, y queda el degradado.
    if let Some(m) = super::fondo::motivo() {
        grid.text(b"  ");
        grid.text(m.as_bytes());
        grid.text(b" -- queda el degradado\n");
    }
}
