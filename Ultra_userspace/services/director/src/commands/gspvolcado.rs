//! **`gpu volcado`: EL COMPOSITOR POR GPU, PASO 1** (2026-09-25) -- el motor
//! de copia de la 3060 lleva el lienzo del escritorio a la pantalla, lo que
//! hoy hace la CPU con `rep movsb` en cada fotograma. Aqui se pide UNO, se
//! comprueba (1024 muestras) y se cronometra contra la CPU.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     una copia de la pantalla entera y una de la CPU para
//!                     comparar
//!
//! Lo que pasa dentro esta en el kernel (`dev/gpu_trabajo/volcado.rs`) y en
//! `bmo_gpu_ga10x::volcado`. El escritorio solo dice DONDE esta su lienzo:
//! el kernel comprueba que es un bloque suyo antes de prestarselo a la 3060.
//!
//! **Y 1b: EN CADA FOTOGRAMA.** Con la copia verificada, [`activar`] le dice a
//! la `Pantalla` que vuelque por la 3060 (`Volcador::Gpu`): las cajas sucias
//! van al motor de copia en UNA tanda por fotograma y la ultima vuelve con la
//! valla pagada. Lo hace solo `save mode` al acabar, y `gpu volcado`; `gpu
//! volcado off` vuelve a la CPU; y si una tanda falla, vuelve sola.
//!
//! # Como "lo lleva gratis" Windows, y por que aqui tambien
//!
//! No es gratis: es que la CPU NO COPIA. Deja el trabajo en una cola (el
//! GPFIFO), toca un timbre y sigue; la GPU copia sola. Lo unico que se espera
//! es la VALLA -- el semaforo que la GPU escribe al acabar --, y solo porque
//! la CPU va a pintar otra vez en el mismo lienzo que la GPU esta leyendo.
//! Con dos lienzos (uno se pinta mientras el otro se copia) ni eso: es el page
//! flip, el paso 3 del plan.

use bmo_gpu_ga10x::volcado as vl;
use bmo_userland as bmo;

use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Motivo del escritorio: la 3060 copio, pero no todas las muestras salieron.
pub(crate) const NO_VOLCADO_MAL: u32 = 0x14A;
/// Motivo del escritorio: el escritorio pinta directo al panel (sin lienzo).
pub(crate) const NO_VOLCADO_SIN_LIENZO: u32 = 0x14B;

#[derive(Clone, Copy)]
struct Resumen {
    r: Result<u64, u32>,
    /// Lo que tardo la CPU en el mismo volcado, si se midio (`gpu volcado`).
    cpu_us: Option<u64>,
}

static mut LIENZO: u64 = 0;
static mut RESUMEN: Option<Resumen> = None;

fn resumen() -> Option<Resumen> {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(RESUMEN) }
}

/// **Donde pinta el escritorio**: lo apunta el arranque al tener doble bufer.
pub(crate) fn apuntar(p: &bmo::Pantalla) {
    if p.tiene_doble_bufer() {
        // SAFETY: como `resumen`.
        unsafe { *core::ptr::addr_of_mut!(LIENZO) = p.lienzo as u64 };
    }
}

/// El lienzo del escritorio (0: pinta directo al panel). Lo usa `gpu pase`.
pub(crate) fn lienzo() -> u64 {
    // SAFETY: como `resumen`.
    unsafe { *core::ptr::addr_of!(LIENZO) }
}

fn pedir(cpu_us: Option<u64>) -> Result<u64, u32> {
    // SAFETY: como `resumen`.
    let lienzo = unsafe { *core::ptr::addr_of!(LIENZO) };
    let r = if lienzo == 0 { Err(NO_VOLCADO_SIN_LIENZO) } else { bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_VOLCADO, lienzo) };
    // SAFETY: como `resumen`.
    unsafe { *core::ptr::addr_of_mut!(RESUMEN) = Some(Resumen { r, cpu_us }) };
    match r {
        Ok(v) if vl::sano(v) => Ok(v),
        Ok(_) => Err(NO_VOLCADO_MAL),
        Err(m) => Err(m),
    }
}

// El argumento de `IOMMU_OP_GPU_VOLCADOR` se escribe en `userland` y se lee
// en el kernel con el crate: que digan lo mismo se comprueba AL COMPILAR.
const _: () = {
    assert!(bmo::volcador_caja(1919, 1079, 7, 3, true) == vl::caja(1919, 1079, 7, 3, true));
    assert!(bmo::volcador_caja(0, 5, 1920, 1, false) == vl::caja(0, 5, 1920, 1, false));
    assert!(bmo::VOLCADOR_ARMAR == vl::ARMAR && bmo::VOLCADOR_SOLTAR == vl::SOLTAR && bmo::VOLCADOR_COMO_VA == vl::COMO_VA);
    assert!(bmo::VOLCADOR_ESPERAR == vl::ESPERAR);
};

/// **Que la 3060 vuelque CADA fotograma**, si la copia verificada salio. Lo
/// llama `save mode` al acabar. Dice en una linea lo que paso.
pub(crate) fn activar(s: &mut Output, p: &bmo::Pantalla) {
    if !hecho() || p.volcando_por_gpu() {
        return;
    }
    match p.volcar_por_gpu() {
        Ok(()) => {
            s.with_ink(INK_GOOD);
            s.text(b"  LA 3060 VUELCA TU ESCRITORIO EN CADA FOTOGRAMA desde ahora (`gpu volcado off` lo devuelve a la CPU)\n");
            crate::desktop::globo::avisar(b"LA 3060", b"vuelca tu escritorio en cada fotograma: la CPU queda libre", crate::desktop::globo::Tono::Bien);
        }
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"  el volcado en cada fotograma no se armo: ");
            s.text(super::iommu::motivo(m));
            s.byte(b'\n');
        }
    }
    s.with_ink(INK_PLAIN);
}

/// `gpu volcado off`: vuelve a la CPU.
pub(crate) fn orden_off(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    p.volcar_por_cpu();
    dsk.out.grid.text(b"  el volcado vuelve a la CPU; el lienzo, devuelto por la 3060\n");
    fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// El paso `volcado` de `save mode` (sin la medida de la CPU: no tiene la
/// pantalla a mano).
pub(crate) fn volcar() -> Result<u64, u32> {
    pedir(resumen().and_then(|r| r.cpu_us))
}

/// Lo pregunta `save mode`.
pub(crate) fn hecho() -> bool {
    matches!(resumen(), Some(Resumen { r: Ok(v), .. }) if vl::sano(v))
}

/// `gpu volcado`: primero la CPU vuelca la pantalla ENTERA (lo que cuesta
/// hoy), despues la 3060 lo mismo.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la CPU vuelca la pantalla entera, y luego la 3060", INK_DIM);
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    p.marcar(0, 0, p.ancho, p.alto);
    let desde = bmo::ciclos();
    p.vaciar();
    let cpu_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    let r = pedir(Some(cpu_us));
    // Lo de la 3060 ya es el lienzo; si no salio, la CPU lo deja bien.
    p.marcar(0, 0, p.ancho, p.alto);
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  LA 3060 LLEVO TU ESCRITORIO A LA PANTALLA: el motor de copia, en una orden\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el volcado por la 3060 no salio: mira la fila `volcado`\n");
    }
    g.with_ink(INK_PLAIN);
    // Verificado: desde ya, cada fotograma.
    activar(&mut dsk.out.grid, p);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "volcado", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La fila `volcado`**, si se pidio.
pub(crate) fn fila(s: &mut Output) {
    let Some(u) = resumen() else { return };
    campo(s, b"volcado");
    match u.r {
        Err(m) => {
            s.with_ink(INK_ERR);
            s.text(b"NO: ");
            s.text(super::iommu::motivo(m));
        }
        Ok(v) => {
            let (buenas, pagado, lanzado, us) = vl::desempaquetar(v);
            s.with_ink(if vl::sano(v) { INK_GOOD } else { INK_ERR });
            if vl::sano(v) {
                s.text(b"LA 3060 LLEVO TU ESCRITORIO A LA PANTALLA: ");
            }
            s.dec(buenas as u64);
            s.text(b" de 1024 muestras; ");
            s.text(if pagado { b"semaforo PAGADO" as &[u8] } else if lanzado { b"semaforo SIN PAGAR" } else { b"sin lanzar" });
            s.with_ink(INK_PLAIN);
            s.text(b"; la 3060 en ");
            s.dec(us as u64);
            s.text(b" us");
            // B2 (25-09): a cuanto fue, contra lo que da el cable. Si va cerca
            // del techo del bus, lo que queda por ganar no es de la 3060.
            let modo = bmo::info(bmo::INFO_GPU_MODO);
            let bytes = (modo & 0xFFFF) * (modo >> 16 & 0xFFFF) * 4;
            if us > 0 && bytes > 0 {
                let mbs = bytes / us as u64;
                s.with_ink(INK_ECHO);
                s.text(b"; ");
                s.dec(bytes / 1024);
                s.text(b" KiB a ");
                s.dec(mbs);
                s.text(b" MB/s");
                let l = bmo::info(bmo::INFO_GPU_SALUD | 1 << 8);
                // ** Contra el TECHO del enlace (LNKCAP), no contra la marcha de
                // ahora: el RM lo baja a Gen1 en reposo y lo sube al trabajar
                // (metal 25-09 06:56: "12721 MB/s, el 318% de Gen1" -- la copia
                // fue en Gen3 y la fila miro despues, ya en reposo).
                if let Some(e) = bmo_gpu_ga10x::salud::enlace(l as u16, (l >> 32) as u32) {
                    let techo = bmo_gpu_ga10x::salud::mb_por_segundo(e.gen_max, e.ancho_max) as u64;
                    if techo > 0 {
                        s.text(b", el ");
                        s.dec(mbs * 100 / techo);
                        s.text(b"% del techo del bus (Gen");
                        s.dec(e.gen_max as u64);
                        s.text(b" x");
                        s.dec(e.ancho_max as u64);
                        s.text(b": ");
                        s.dec(techo);
                        s.text(b" MB/s; al mirar iba en Gen");
                        s.dec(e.gen as u64);
                        s.text(b": el RM lo sube y baja solo)");
                    }
                }
                s.with_ink(INK_PLAIN);
            }
            if let Some(c) = u.cpu_us {
                s.text(b", la CPU en ");
                s.dec(c);
                s.text(b" us");
                if us > 0 {
                    s.with_ink(INK_ECHO);
                    s.text(b" (x");
                    s.dec(c / us as u64);
                    s.byte(b')');
                }
            }
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    // Y cada fotograma: cuantas tandas lleva la 3060.
    if let Ok(v) = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_VOLCADOR, bmo::VOLCADOR_COMO_VA << 60) {
        if v >> 32 & 1 != 0 {
            campo(s, b"cada foto");
            s.with_ink(INK_GOOD);
            s.text(b"POR LA 3060: ");
            s.dec(v & 0xFFFF_FFFF);
            s.text(b" tandas enviadas (una por fotograma; la CPU no espera: la valla, al volver a pintar)");
            // B2: cuanto copia cada una, y cuantas veces la CPU SI espero.
            let tandas = (v & 0xFFFF_FFFF).max(1);
            let como = |sel: u64| bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_VOLCADOR, bmo::VOLCADOR_COMO_VA << 60 | sel).unwrap_or(0);
            let bytes = como(vl::COMO_VA_BYTES);
            let esperas = como(vl::COMO_VA_ESPERAS);
            s.with_ink(INK_ECHO);
            s.text(b"; ");
            s.dec(bytes / tandas / 1024);
            s.text(b" KiB por tanda, ");
            s.dec(bytes >> 20);
            s.text(b" MiB en total; la CPU espero ");
            s.dec(esperas);
            s.text(b" veces");
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
    }
}
