//! **`gpu aguante [minutos]`: D1, LA 3060 BAJO CARGA LARGA.** Repite, vuelta
//! tras vuelta, TODOS los trabajos de la 3060 que ya salieron en este
//! arranque (fractal, triangulo, escena, raster, color, giro, pantalla), cada
//! uno con su juez de siempre, hasta que se acabe el tiempo o uno falle. Y
//! mira la temperatura mientras.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: de 1 a 30
//!                     minutos de 3060 a fondo; en reposo, nada
//!
//! # Por que (D1 de `PLAN_LA_3060_AFINADA.md`)
//!
//! `save mode` prueba cada cosa UNA vez. Lo que sale una vez puede fallar a
//! la 500: el anillo del GPFIFO dando la vuelta (ya paso: 24-09), una
//! carrera entre dos trabajos, la tarjeta caliente. Esto es lo mismo que
//! `save mode`, cientos de veces y seguido. Se para en el PRIMER fallo, para
//! que la fila y los avisos del GSP digan el de verdad y no su eco.

use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

type Trabajo = fn() -> Result<u64, u32>;

/// Los trabajos, en el orden de `save mode`, con lo que dice si ya salieron.
const TRABAJOS: [(&[u8], Trabajo, fn() -> bool); 7] = [
    (b"fractal", super::gspcomputo::calcular_fractal, super::gspcomputo::fractal_hecho),
    (b"triangulo", super::gspcomputo::dibujar_triangulo, super::gspcomputo::triangulo_hecho),
    (b"escena", super::gspcomputo::dibujar_escena, super::gspcomputo::escena_hecha),
    (b"raster", super::gspcomputo::dibujar_raster, super::gspcomputo::raster_hecho),
    (b"color", super::gspcomputo::dibujar_color3d, super::gspcomputo::color3d_hecho),
    (b"giro", super::gspcomputo::dibujar_giro, super::gspcomputo::giro_hecho),
    (b"pantalla", super::gspcomputo::dibujar_pantalla, super::gspcomputo::pantalla_hecha),
];
const N: usize = TRABAJOS.len();

/// Minutos por defecto y lo mas.
const MINUTOS: u64 = 2;
const MAX_MINUTOS: u64 = 30;

#[derive(Clone, Copy, Default)]
struct Cuenta {
    /// Si entra en la prueba (ya salio en este arranque).
    entra: bool,
    bien: u32,
    us: u64,
    peor_us: u64,
}

#[derive(Clone, Copy, Default)]
struct Aguante {
    cuentas: [Cuenta; N],
    vueltas: u32,
    segundos: u64,
    /// El que fallo: su indice, la vuelta y su NO.
    fallo: Option<(usize, u32, u32)>,
    /// Grados al empezar, lo mas alto y al acabar (0 = sin lectura).
    temp: (u32, u32, u32),
}

static mut ULTIMO: Option<Aguante> = None;

fn ultimo() -> Option<Aguante> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ULTIMO) }
}

fn grados() -> u32 {
    bmo_gpu_ga10x::salud::lectura(bmo::info(bmo::INFO_GPU_SALUD) as u32).map_or(0, |(g, _)| g)
}

fn aguantar(minutos: u64) -> Aguante {
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let mut a = Aguante::default();
    for (k, t) in TRABAJOS.iter().enumerate() {
        a.cuentas[k].entra = (t.2)();
    }
    let t0 = grados();
    a.temp = (t0, t0, t0);
    let desde = bmo::ciclos();
    let fin = desde + hz * 60 * minutos;
    'vueltas: while bmo::ciclos() < fin {
        for (k, t) in TRABAJOS.iter().enumerate() {
            if !a.cuentas[k].entra {
                continue;
            }
            let c0 = bmo::ciclos();
            let r = (t.1)();
            let us = (bmo::ciclos() - c0) * 1_000_000 / hz;
            let c = &mut a.cuentas[k];
            c.us += us;
            c.peor_us = c.peor_us.max(us);
            match r {
                Ok(_) => c.bien += 1,
                Err(m) => {
                    a.fallo = Some((k, a.vueltas, m));
                    break 'vueltas;
                }
            }
            // Que el bus USB (teclado y raton) corra entre trabajo y trabajo.
            bmo::yield_screen();
        }
        a.vueltas += 1;
        let g = grados();
        a.temp.1 = a.temp.1.max(g);
        a.temp.2 = g;
    }
    a.segundos = (bmo::ciclos() - desde) / hz;
    a
}

/// `gpu aguante [minutos]`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, args: &[u8]) -> After {
    let t = args.trim_ascii();
    let minutos = if t.is_empty() {
        MINUTOS
    } else {
        match t.iter().try_fold(0u64, |v, &c| (c.is_ascii_digit() && v < 1000).then(|| v * 10 + (c - b'0') as u64)) {
            Some(m) if (1..=MAX_MINUTOS).contains(&m) => m,
            _ => 0,
        }
    };
    if minutos == 0 {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ERR);
        g.text(b"  gpu aguante [minutos]   (de 1 a 30; sin numero, 2)\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    if !TRABAJOS.iter().any(|t| (t.2)()) {
        let g = &mut dsk.out.grid;
        g.with_ink(INK_ERR);
        g.text(b"  aguante: ningun trabajo de la 3060 salio todavia en este arranque: primero `save mode`\n");
        g.with_ink(INK_PLAIN);
        dsk.field.n = 0;
        return After::Settle;
    }
    if !super::files::antes_de_arriesgar(dsk, p, b"gpu aguante") {
        dsk.field.n = 0;
        return After::Settle;
    }
    paint_status(p, &dsk.run_box, "la 3060 bajo carga larga: todos sus trabajos, vuelta tras vuelta", INK_DIM);
    let a = aguantar(minutos);
    // SAFETY: como `ultimo`.
    unsafe { *core::ptr::addr_of_mut!(ULTIMO) = Some(a) };
    // `pantalla` y `giro` pintan encima del escritorio.
    crate::repintar_escritorio(p, dsk, "aguante");
    let g = &mut dsk.out.grid;
    g.with_ink(if a.fallo.is_none() { INK_GOOD } else { INK_ERR });
    g.text(if a.fallo.is_none() {
        b"  LA 3060 AGUANTO: ningun trabajo fallo en toda la prueba (D1)\n" as &[u8]
    } else {
        b"  la 3060 NO aguanto: mira la fila `aguante` y los avisos del GSP\n"
    });
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    super::gspcola::avisos(&mut dsk.out.grid, 4);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila `aguante`**, si se hizo.
pub(crate) fn fila(s: &mut Output) {
    let Some(a) = ultimo() else { return };
    campo(s, b"aguante");
    s.with_ink(if a.fallo.is_none() { INK_GOOD } else { INK_ERR });
    s.dec(a.vueltas as u64);
    s.text(b" vueltas en ");
    s.dec(a.segundos);
    s.text(b" s");
    if let Some((k, v, m)) = a.fallo {
        s.text(b"; FALLO `");
        s.text(TRABAJOS[k].0);
        s.text(b"` en la vuelta ");
        s.dec(v as u64 + 1);
        s.text(b": ");
        s.text(super::iommu::motivo(m));
    } else {
        s.text(b", sin un fallo");
    }
    s.with_ink(INK_ECHO);
    if a.temp.0 != 0 {
        s.text(b"; temperatura ");
        s.dec(a.temp.0 as u64);
        s.text(b" -> ");
        s.dec(a.temp.2 as u64);
        s.text(b" grados (lo mas, ");
        s.dec(a.temp.1 as u64);
        s.byte(b')');
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    for (k, c) in a.cuentas.iter().enumerate() {
        if !c.entra {
            continue;
        }
        campo(s, b"  trabajo");
        s.text(TRABAJOS[k].0);
        s.text(b": ");
        s.dec(c.bien as u64);
        s.text(b" bien");
        s.with_ink(INK_ECHO);
        s.text(b"; media ");
        s.dec(c.us / (c.bien as u64 + a.fallo.map_or(0, |f| (f.0 == k) as u64)).max(1));
        s.text(b" us, la peor ");
        s.dec(c.peor_us);
        s.text(b" us");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }
    super::datos::anotar(b"gpu aguante vueltas", a.vueltas as u64, b"");
    super::datos::anotar(b"gpu aguante fallo", a.fallo.is_some() as u64, b"");
}
