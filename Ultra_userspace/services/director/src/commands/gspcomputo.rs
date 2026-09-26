//! **`gpu computo`: M5d S1..S3, EL PRIMER TRABAJO DEL MOTOR GRAFICO.** Tras el
//! contexto de oro (VISTO en el metal el 24-09 a las 15:51): AMPERE_COMPUTE_B
//! en el canal de GR0, su ficha, y un trabajo que solo el GR puede pagar
//! (`bmo_gpu_ga10x::computo`).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea, o en `save mode`:
//!                     dos RPC de hasta 5 s y una espera de hasta 100 ms
//!
//! El timbre sale de la TABLA de aparatos (la lista de GR0 << 16 | chid 2),
//! como la copia; la ficha del RM queda en la fila y es el respaldo.

use bmo_gpu_ga10x::canal::GR;
use bmo_gpu_ga10x::computo;
use bmo_gpu_ga10x::control::{self, Control, CABECERA_CONTROL};
use bmo_gpu_ga10x::copia;
use bmo_gpu_ga10x::objeto::{self, CABECERA_ALLOC};
use bmo_gpu_ga10x::blur;
use bmo_gpu_ga10x::fractal;
use bmo_gpu_ga10x::lienzo;
use bmo_gpu_ga10x::escena;
use bmo_gpu_ga10x::tresde;
use bmo_gpu_ga10x::triangulo;
use bmo_gpu_ga10x::sombreador;
use bmo_userland as bmo;

/// T1c y T2a: el triangulo por el pipeline 3D.
mod pipeline3d;
/// G: la esfera que gira y bota.
mod giro;
pub(crate) use giro::{dibujar_giro, giro_hecho, orden_giro};
/// P: la 3060 pinta la pantalla entera.
mod pantalla;
pub(crate) use pantalla::{dibujar_pantalla, fotograma, medir_pantalla, orden_pantalla, pantalla_hecha};
/// M6 V0: el video NV12, pasado a color y agrandado por la 3060.
mod video;
pub(crate) use video::{dibujar_video, orden_video, video_hecho};
/// D2b: una imagen de 32 bits (DOOM), agrandada por la 3060.
mod imagen;
pub(crate) use imagen::orden_imagen;
pub(crate) use pipeline3d::{color3d_hecho, dibujar_color3d, dibujar_raster, escalera, orden_color3d, orden_raster, raster_hecho};

use super::gsprpc::{esperar, Otros};
use super::gspsalud::{controlar, Contestada};
use super::tabla::campo;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

#[derive(Clone, Copy)]
struct Pedido {
    r: objeto::Respuesta,
    resultado: u32,
    espera_us: u64,
    numero: u32,
}

#[derive(Clone, Copy, Default)]
struct Computo {
    pedido: Option<Result<Pedido, u32>>,
    ficha: Option<Result<Contestada, u32>>,
    /// La lista de GR0 segun la tabla de aparatos, si contesto.
    lista: Option<u32>,
    trabajo: Option<Result<u64, u32>>,
    /// Lo que se escribio en el timbre, y si salio de la tabla.
    timbre: Option<(u32, bool)>,
    /// S4..S6: el primer sombreador.
    sombreo: Option<Result<u64, u32>>,
    /// L: el lienzo.
    lienzo: Option<Result<u64, u32>>,
    /// B: el ultimo blur, y de donde salio (`true`: de la pantalla).
    blur: Option<Result<u64, u32>>,
    blur_de_pantalla: bool,
    /// F: el fractal.
    fractal: Option<Result<u64, u32>>,
    /// T0: el triangulo por computo.
    triangulo: Option<Result<u64, u32>>,
    /// T1a: la clase 3D limpia un destino.
    limpio3d: Option<Result<u64, u32>>,
    /// E: la escena 3D con luz.
    escena: Option<Result<u64, u32>>,
    /// T1c: el triangulo por el rasterizador.
    raster: Option<Result<u64, u32>>,
    /// T2a: el triangulo con color, mezclado por el rasterizador.
    color3d: Option<Result<u64, u32>>,
    /// G: la esfera que gira, sus 32 fotogramas.
    giro: Option<Result<giro::Vuelta, u32>>,
    /// P: la ultima tanda a pantalla completa.
    pantalla: Option<Result<pantalla::Tanda, u32>>,
    /// M6 V0: la ultima reproduccion de `gpu video`.
    video: Option<Result<video::Repro, u32>>,
    /// D2b: la ultima tanda de `gpu imagen`.
    imagen: Option<Result<imagen::Tanda, u32>>,
}

static mut ESTADO: Option<Computo> = None;

fn estado() -> Computo {
    // SAFETY: el escritorio es un solo hilo; esto solo se toca desde sus ordenes.
    unsafe { *core::ptr::addr_of!(ESTADO) }.unwrap_or_default()
}

fn con(f: impl FnOnce(&mut Computo)) {
    // SAFETY: como `estado`.
    f(unsafe { (*core::ptr::addr_of_mut!(ESTADO)).get_or_insert_with(Computo::default) })
}

/// El RM contesto, pero NO creo AMPERE_COMPUTE_B.
pub(crate) const NO_COMPUTO_NEGADO: u32 = 0x137;
/// Sin ficha del canal de GR0 ni lista de GR0 en la tabla: no se toca el timbre.
pub(crate) const NO_TRABAJO_SIN_FICHA: u32 = 0x138;
/// El timbre sono pero el GR no pago el semaforo (la fila `gr trabajo`).
pub(crate) const NO_TRABAJO_MAL: u32 = 0x139;
/// El sombreador se lanzo pero no escribio sus 32 palabras (la fila `sombreo`).
pub(crate) const NO_SOMBREO_MAL: u32 = 0x13A;
/// El lienzo se lanzo pero no salio entero (la fila `lienzo`).
pub(crate) const NO_LIENZO_MAL: u32 = 0x13B;
/// El blur se lanzo pero no salio igual que la CPU (la fila `blur`).
pub(crate) const NO_BLUR_MAL: u32 = 0x13C;
/// Un trozo de la pantalla no se pudo subir al lienzo.
pub(crate) const NO_BLUR_SUBIR: u32 = 0x13D;
/// El fractal se lanzo pero no salio igual que la CPU (la fila `fractal`).
pub(crate) const NO_FRACTAL_MAL: u32 = 0x13E;
/// El triangulo se lanzo pero no salio igual que la CPU (la fila `triangulo`).
pub(crate) const NO_TRIANGULO_MAL: u32 = 0x13F;
/// La clase 3D se lanzo pero el destino no salio del color de limpieza.
pub(crate) const NO_LIMPIO3D_MAL: u32 = 0x140;
/// La escena se lanzo pero no salio igual que la CPU (la fila `escena`).
pub(crate) const NO_ESCENA_MAL: u32 = 0x141;
/// El triangulo 3D se lanzo pero no salio igual que el juez (la fila `raster`).
pub(crate) const NO_RASTER_MAL: u32 = 0x142;
/// El triangulo con color se lanzo pero no salio como dice el juez (la fila `color`).
pub(crate) const NO_COLOR3D_MAL: u32 = 0x143;
/// Un fotograma de la esfera que gira no salio igual que la CPU (la fila `giro`).
pub(crate) const NO_GIRO_MAL: u32 = 0x144;
/// Un fotograma a pantalla completa no salio igual que la CPU (la fila `pantalla`).
pub(crate) const NO_PANTALLA_MAL: u32 = 0x149;
/// Un fotograma del video no salio igual que la CPU (la fila `video`).
pub(crate) const NO_VIDEO_MAL: u32 = 0x14C;
/// No hubo un bloque de memoria para un fotograma del video.
pub(crate) const NO_VIDEO_SIN_MEMORIA: u32 = 0x14D;
/// Un fotograma de `gpu imagen` no salio igual que la CPU, o el fichero se
/// acabo a medias (la fila `imagen`).
pub(crate) const NO_IMAGEN_MAL: u32 = 0x150;
/// No hubo un bloque de memoria para un fotograma de `gpu imagen`.
pub(crate) const NO_IMAGEN_SIN_MEMORIA: u32 = 0x151;

fn pedido_bien(p: &Option<Result<Pedido, u32>>) -> bool {
    matches!(p, Some(Ok(p)) if p.r.estado == 0 && p.resultado == 0)
}

/// **S1: AMPERE_COMPUTE_B en el canal de GR0.** `Ok(su asa)`.
pub(crate) fn pedir() -> Result<u64, u32> {
    let r = (|| {
        let numero = (bmo::iommu_orden(bmo::IOMMU_OP_GSP_COMPUTO)? >> 32) as u32;
        let mut d = [0u8; CABECERA_ALLOC];
        let (m, espera_us) = esperar(objeto::GSP_RM_ALLOC, &mut d, &mut Otros::default())?;
        let r = objeto::leer(&d).ok_or(NO_COMPUTO_NEGADO)?;
        Ok(Pedido { r, resultado: m.resultado, espera_us, numero })
    })();
    con(|c| {
        c.pedido = Some(r);
        c.trabajo = None;
    });
    if pedido_bien(&Some(r)) {
        Ok(computo::COMPUTO as u64)
    } else {
        Err(r.err().unwrap_or(NO_COMPUTO_NEGADO))
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn pedido() -> bool {
    pedido_bien(&estado().pedido)
}

/// **S2: la ficha del canal de GR0**, y la lista de GR0 de la tabla.
pub(crate) fn ficha() -> Result<u64, u32> {
    let f = controlar(Control::FichaGr, &mut [0u8; CABECERA_CONTROL + 4]);
    let lista = (|| {
        let mut d = [0u8; CABECERA_CONTROL + control::DISPOSITIVOS_MEDIDA];
        let c = controlar(Control::Dispositivos, &mut d).ok()?;
        if !c.bien() {
            return None;
        }
        let (tabla, n) = control::dispositivos(&d);
        tabla[..n].iter().find(|x| x.tipo == GR.motor).map(|x| x.lista)
    })();
    con(|c| {
        c.ficha = Some(f);
        c.lista = lista;
    });
    let f = f?;
    if !f.bien() {
        return Err(super::gspsalud::NO_CONTROL_NEGADO);
    }
    Ok(f.r.valor as u64)
}

/// Lo pregunta `save mode`.
pub(crate) fn ficha_leida() -> bool {
    matches!(estado().ficha, Some(Ok(f)) if f.bien())
}

/// **S3: el primer trabajo del GR**: el kernel prepara el tramo, pone GP_PUT
/// del canal de GR0, toca el timbre y espera el semaforo de informe.
pub(crate) fn trabajar() -> Result<u64, u32> {
    let e = estado();
    let de_la_ficha = match e.ficha {
        Some(Ok(f)) if f.bien() => Some(f.r.valor),
        _ => None,
    };
    let de_la_tabla = e.lista.map(|l| copia::timbre_de(l, GR.chid));
    let valor = de_la_tabla.or(de_la_ficha);
    let r = match valor {
        Some(v) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_TRABAJO_GR, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    };
    con(|c| {
        c.trabajo = Some(r);
        c.timbre = valor.map(|v| (v, de_la_tabla.is_some()));
    });
    match r {
        Ok(v) if computo::sano(v) => Ok(v),
        Ok(_) => Err(NO_TRABAJO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn trabajado() -> bool {
    matches!(estado().trabajo, Some(Ok(v)) if computo::sano(v))
}

/// **S4..S6: el primer sombreador**, con el MISMO timbre que S3.
pub(crate) fn sombrear() -> Result<u64, u32> {
    let e = estado();
    let r = match e.timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_SOMBREO, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    };
    con(|c| c.sombreo = Some(r));
    match r {
        Ok(v) if sombreador::sano(v) => Ok(v),
        Ok(_) => Err(NO_SOMBREO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn sombreado() -> bool {
    matches!(estado().sombreo, Some(Ok(v)) if sombreador::sano(v))
}

/// **L: el lienzo** -- la 3060 pinta 128 x 128 pixeles en la RAM del PC.
pub(crate) fn pintar() -> Result<u64, u32> {
    let e = estado();
    let r = match e.timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIENZO, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    };
    con(|c| c.lienzo = Some(r));
    match r {
        Ok(v) if lienzo::sano(v) => Ok(v),
        Ok(_) => Err(NO_LIENZO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn pintado() -> bool {
    matches!(estado().lienzo, Some(Ok(v)) if lienzo::sano(v))
}

/// Los pixeles, leidos del kernel de dos en dos.
static mut PIXELES: [u32; lienzo::PIXELES] = [0; lienzo::PIXELES];

/// **Leer** el lienzo (o, con `salida`, lo que dejo el blur) en `PIXELES`.
fn leer(salida: bool) -> bool {
    // SAFETY: el escritorio es un solo hilo; solo se toca desde aqui.
    let px = unsafe { &mut *core::ptr::addr_of_mut!(PIXELES) };
    for k in 0..lienzo::PIXELES / 2 {
        match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIENZO_LEER, k as u64 | (salida as u64) << 32) {
            Ok(v) => {
                px[2 * k] = v as u32;
                px[2 * k + 1] = (v >> 32) as u32;
            }
            Err(_) => return false,
        }
    }
    true
}

/// **Pintar `PIXELES`** con cada pixel en un cuadro de `escala` x `escala`,
/// desde `(x0, y0)`, con un marco.
fn pintar_en(p: &bmo::Pantalla, x0: u32, y0: u32, escala: u32) {
    // SAFETY: como `leer`.
    let px = unsafe { &*core::ptr::addr_of!(PIXELES) };
    let lado = lienzo::LADO * escala;
    p.rect(x0.saturating_sub(3), y0.saturating_sub(3), lado + 6, lado + 6, 0x0076_B900);
    for (k, &c) in px.iter().enumerate() {
        let (x, y) = (k as u32 % lienzo::LADO, k as u32 / lienzo::LADO);
        p.rect(x0 + x * escala, y0 + y * escala, escala, escala, c & 0x00FF_FFFF);
    }
}

/// **Mostrar el lienzo** arriba a la derecha. `false` si el kernel no lo dio.
fn mostrar(p: &bmo::Pantalla, escala: u32) -> bool {
    if !leer(false) {
        return false;
    }
    pintar_en(p, p.ancho.saturating_sub(lienzo::LADO * escala + 32), 96, escala);
    true
}

/// Todo lo que falta hasta el lienzo, en orden.
fn hasta_el_lienzo() -> Result<u64, u32> {
    let mut r = if pedido() { Ok(0) } else { pedir() };
    if r.is_ok() && !ficha_leida() {
        r = ficha();
    }
    if r.is_ok() && !trabajado() {
        r = trabajar();
    }
    if r.is_ok() && !sombreado() {
        r = sombrear();
    }
    if r.is_ok() && !pintado() {
        r = pintar();
    }
    r
}

/// **X5 (`gpu cubo 3060`)**: todo hasta el lienzo, y la ficha del timbre de
/// GR0 que los trabajos de la 3060 llevan en su orden.
pub(crate) fn ficha_del_gr() -> Result<u64, u32> {
    hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => Ok(v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    })
}

/// **B: el blur** de lo que haya en el lienzo (en `save mode`, el degradado).
pub(crate) fn desenfocar() -> Result<u64, u32> {
    let e = estado();
    let r = match e.timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_BLUR, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    };
    con(|c| c.blur = Some(r));
    match r {
        Ok(v) if blur::sano(v) => Ok(v),
        Ok(_) => Err(NO_BLUR_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn desenfocado() -> bool {
    matches!(estado().blur, Some(Ok(v)) if blur::sano(v))
}

/// **Subir un trozo de la pantalla al lienzo**, dos pixeles por llamada: el
/// de 128 x 128 al 37 % del ancho y a 60 del borde de arriba -- en 1920 x
/// 1080, el centro del fondo, donde hay color.
///
/// ** Metal 24-09 16:49: la primera version subia (32, 200), que es el panel
/// de la izquierda, casi negro: la 3060 desenfoco bien (16384 de 16384) un
/// cuadro NEGRO, y los dos cuadros salieron negros.
fn subir_trozo(p: &bmo::Pantalla) -> bool {
    let x0 = (p.ancho * 37 / 100).min(p.ancho.saturating_sub(lienzo::LADO));
    let y0 = 60u32.min(p.alto.saturating_sub(lienzo::LADO));
    let px = |k: u32| {
        let (x, y) = (x0 + k % lienzo::LADO, y0 + k / lienzo::LADO);
        // SAFETY: `(x, y)` esta dentro de la pantalla (recortado arriba) y
        // `lienzo` es su bufer de dibujo, de `alto` filas de `stride` pixeles.
        unsafe { p.lienzo.add((y * p.stride + x) as usize).read_volatile() & 0x00FF_FFFF }
    };
    (0..lienzo::PIXELES as u32 / 2).all(|k| bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIENZO_ESCRIBIR, blur::subir(k, px(2 * k), px(2 * k + 1))).is_ok())
}

/// `gpu blur`: lo que falte, un trozo de tu pantalla al lienzo, el blur, y el
/// antes y el despues, arriba a la derecha.
pub(crate) fn orden_blur(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 desenfoca un trozo de tu pantalla", INK_DIM);
    let mut r = hasta_el_lienzo();
    if r.is_ok() {
        r = if subir_trozo(p) { Ok(0) } else { Err(NO_BLUR_SUBIR) };
    }
    if r.is_ok() {
        r = desenfocar();
        con(|c| c.blur_de_pantalla = true);
    }
    let escala = 3;
    let lado = lienzo::LADO * escala;
    let visto = r.is_ok() && leer(false) && {
        pintar_en(p, p.ancho.saturating_sub(2 * lado + 64), 96, escala);
        leer(true)
    } && {
        pintar_en(p, p.ancho.saturating_sub(lado + 32), 96, escala);
        true
    };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  ARRIBA A LA DERECHA: un trozo de tu pantalla, y al lado, DESENFOCADO POR TU 3060 (M5d B)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el blur no salio: mira la fila `blur`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "blur", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **F: el fractal** -- Mandelbrot de 512 x 512 en la 3060, cronometrado
/// contra la CPU haciendo lo mismo.
pub(crate) fn calcular_fractal() -> Result<u64, u32> {
    let r = hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_FRACTAL, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    });
    con(|c| c.fractal = Some(r));
    match r {
        Ok(v) if fractal::sano(v) => Ok(v),
        Ok(_) => Err(NO_FRACTAL_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn fractal_hecho() -> bool {
    matches!(estado().fractal, Some(Ok(v)) if fractal::sano(v))
}

/// **T0: el triangulo** por computo, cronometrado contra la CPU.
pub(crate) fn dibujar_triangulo() -> Result<u64, u32> {
    let r = hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_TRIANGULO, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    });
    con(|c| c.triangulo = Some(r));
    match r {
        Ok(v) if triangulo::sano(v) => Ok(v),
        Ok(_) => Err(NO_TRIANGULO_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn triangulo_hecho() -> bool {
    matches!(estado().triangulo, Some(Ok(v)) if triangulo::sano(v))
}

/// **T1a: la clase 3D limpia un destino** con su ROP, sin programas.
pub(crate) fn limpiar_3d() -> Result<u64, u32> {
    let r = hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIMPIAR_3D, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    });
    con(|c| c.limpio3d = Some(r));
    match r {
        Ok(v) if tresde::sano(v) => Ok(v),
        Ok(_) => Err(NO_LIMPIO3D_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn limpio_3d() -> bool {
    matches!(estado().limpio3d, Some(Ok(v)) if tresde::sano(v))
}

/// **E: la escena 3D con luz**, dibujada por la 3060.
pub(crate) fn dibujar_escena() -> Result<u64, u32> {
    let r = hasta_el_lienzo().and_then(|_| match estado().timbre {
        Some((v, _)) => bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_ESCENA, v as u64),
        None => Err(NO_TRABAJO_SIN_FICHA),
    });
    con(|c| c.escena = Some(r));
    match r {
        Ok(v) if escena::sano(v) => Ok(v),
        Ok(_) => Err(NO_ESCENA_MAL),
        Err(m) => Err(m),
    }
}

/// Lo pregunta `save mode`.
pub(crate) fn escena_hecha() -> bool {
    matches!(estado().escena, Some(Ok(v)) if escena::sano(v))
}

/// `gpu escena`: la escena 3D con luz, a pantalla completa.
pub(crate) fn orden_escena(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 dibuja una escena 3D con luz", INK_DIM);
    let r = dibujar_escena();
    let visto = matches!(r, Ok(v) if panel(p, v, Vista::Escena));
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = visto };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  LA ESCENA 3D CON LUZ, DIBUJADA POR TU 3060 (M5d E): mira la fila `escena`\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  la escena no salio: mira la fila `escena`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu 3d`: la clase 3D limpia el destino, y el panel.
pub(crate) fn orden_3d(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la clase 3D de la 3060 limpia un destino con su ROP", INK_DIM);
    let r = limpiar_3d();
    let visto = matches!(r, Ok(v) if panel(p, v, Vista::Limpieza3d));
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = visto };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  LA CLASE 3D DE TU 3060 ESCRIBIO PIXELES CON SU ROP (M5 T1a): mira la fila `3d`\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  la clase 3D no limpio el destino: mira la fila `3d`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// Que muestra el panel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Vista {
    Fractal,
    Triangulo,
    Limpieza3d,
    Escena,
    Raster,
    Color3d,
}

/// Una linea de texto sin reservar memoria.
struct Linea {
    b: [u8; 96],
    n: usize,
}

impl Linea {
    fn nueva() -> Linea {
        Linea { b: [0; 96], n: 0 }
    }
    fn t(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }
    fn d(&mut self, mut v: u64) -> &mut Self {
        let mut tmp = [0u8; 20];
        let mut k = tmp.len();
        loop {
            k -= 1;
            tmp[k] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.t(&tmp[k..])
    }
}

/// El panel esta encima: el escritorio no pinta su mobiliario (como con una
/// app a pantalla completa) y la primera tecla lo cierra.
static mut PANEL_ABIERTO: bool = false;

/// **Abrir el panel** desde fuera de `gspcomputo` (`gpu cubo`): la primera
/// tecla devuelve el escritorio.
pub(crate) fn abrir_panel() {
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = true };
}

pub(crate) fn panel_abierto() -> bool {
    // SAFETY: el escritorio es un solo hilo.
    unsafe { *core::ptr::addr_of!(PANEL_ABIERTO) }
}

/// **Cerrar el panel**: devolver el escritorio ENTERO, como al salir de una
/// app a pantalla completa.
pub(crate) fn cerrar_panel(dsk: &mut Desktop, p: &bmo::Pantalla) {
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = false };
    crate::repintar_escritorio(p, dsk, "panel de la 3060: fuera");
}

/// **El panel de la 3060, a pantalla completa**: el fractal al doble a la
/// derecha, y a la izquierda lo que dijo.
fn panel(p: &bmo::Pantalla, v: u64, que: Vista) -> bool {
    const FONDO: u32 = 0x000B_0D12;
    const VERDE: u32 = 0x0076_B900;
    const CLARO: u32 = 0x00E6_EDF6;
    const TENUE: u32 = 0x008A_94A6;
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    p.rect(0, 0, p.ancho, 4, VERDE);
    let escala = if p.alto >= 2 * fractal::LADO + 40 { 2 } else { 1 };
    let lado = fractal::LADO * escala;
    let x0 = p.ancho.saturating_sub(lado + 40);
    let y0 = (p.alto.saturating_sub(lado)) / 2;
    p.rect(x0.saturating_sub(3), y0.saturating_sub(3), lado + 6, lado + 6, VERDE);
    // Fila a fila, de dos en dos pixeles: sin un bufer de 1 MiB en el escritorio.
    for y in 0..fractal::LADO {
        for par in 0..fractal::LADO / 2 {
            let k = (y * fractal::LADO / 2 + par) as u64;
            let Ok(dos) = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_LIENZO_LEER, k | 1 << 33) else {
                return false;
            };
            for (i, c) in [dos as u32, (dos >> 32) as u32].into_iter().enumerate() {
                let x = 2 * par + i as u32;
                p.rect(x0 + x * escala, y0 + y * escala, escala, escala, c & 0x00FF_FFFF);
            }
        }
    }
    let (buenos, _, _, _, gpu_us, cpu_us) = fractal::desempaquetar(v);
    let mut y = 60;
    p.texto_escala(40, y, "BMO-X  |  RTX 3060", VERDE, 3);
    y += 60;
    let fila = |y: u32, l: &mut Linea, c: u32| {
        p.texto_bytes(40, y, &l.b[..l.n], c);
    };
    if que == Vista::Color3d {
        p.texto_escala(40, y, "TRES COLORES, MEZCLADOS POR EL RASTERIZADOR", CLARO, 2);
        y += 60;
        fila(y, Linea::nueva().t(b"cada vertice con su color; la 3060 los mezcla en cada pixel (IPA)"), CLARO);
        y += 28;
        fila(y, Linea::nueva().t(b"el triangulo de T0, ahora por el hardware de triangulos"), CLARO);
    } else if que == Vista::Raster {
        p.texto_escala(40, y, "EL TRIANGULO POR EL RASTERIZADOR", CLARO, 2);
        y += 60;
        fila(y, Linea::nueva().t(b"vertice -> rasterizador -> pixel -> ROP: el pipeline 3D"), CLARO);
        y += 28;
        fila(y, Linea::nueva().t(b"3 vertices; la 3060 decide que pixeles quedan dentro"), CLARO);
    } else if que == Vista::Escena {
        p.texto_escala(40, y, "UNA ESCENA 3D CON LUZ, POR TU 3060", CLARO, 2);
        y += 60;
        fila(y, Linea::nueva().t(b"esfera: rayo, normal, luz difusa y brillo especular"), CLARO);
        y += 28;
        fila(y, Linea::nueva().t(b"suelo en perspectiva con cuadros, sombra y cielo"), CLARO);
    } else if que == Vista::Limpieza3d {
        p.texto_escala(40, y, "LA CLASE 3D DE TU 3060: EL ROP", CLARO, 2);
        y += 60;
        fila(y, Linea::nueva().t(b"AMPERE_B limpio un destino de render de 512 x 512"), CLARO);
        y += 28;
        fila(y, Linea::nueva().t(b"sin programas: su hardware de pixeles, en tu RAM"), CLARO);
    } else if que == Vista::Triangulo {
        p.texto_escala(40, y, "EL PRIMER TRIANGULO DE TU 3060", CLARO, 2);
        y += 60;
        fila(y, Linea::nueva().t(b"las tres funciones de arista, en cada pixel de 512 x 512"), CLARO);
        y += 28;
        fila(y, Linea::nueva().t(b"el color: los tres vertices mezclados por su peso"), CLARO);
    } else {
        p.texto_escala(40, y, "EL FRACTAL, CALCULADO POR TU 3060", CLARO, 2);
        y += 60;
        fila(y, Linea::nueva().t(b"Mandelbrot 512 x 512, hasta ").d(fractal::VUELTAS as u64).t(b" vueltas por pixel"), CLARO);
    }
    y += 28;
    if !matches!(que, Vista::Limpieza3d | Vista::Raster | Vista::Color3d) {
        fila(y, Linea::nueva().d(fractal::PIXELES as u64).t(b" hilos a la vez: 512 bloques de 512"), CLARO);
    }
    y += 48;
    p.texto_escala(40, y, "LA 3060", VERDE, 2);
    y += 36;
    fila(y, Linea::nueva().d(gpu_us as u64).t(b" us, del timbre al semaforo"), CLARO);
    y += 48;
    if matches!(que, Vista::Limpieza3d | Vista::Raster | Vista::Color3d) {
        // Aqui la CPU no hace la misma cuenta: solo CUENTA los pixeles.
        let bien = buenos as usize == fractal::PIXELES;
        let (dice, si, no) = if que == Vista::Color3d {
            (b" de 262144 pixeles como dice el juez (1 de margen por canal: el ROP redondea)" as &[u8], "EL RASTERIZADOR MEZCLA: SI", "EL RASTERIZADOR MEZCLA: NO")
        } else if que == Vista::Raster {
            (b" de 262144 pixeles donde el juez dice (la CPU no dibuja: sabe cuales van dentro)" as &[u8], "EL RASTERIZADOR DIBUJA: SI", "EL RASTERIZADOR DIBUJA: NO")
        } else {
            (b" de 262144 pixeles del color de limpieza (la CPU solo los cuenta)" as &[u8], "EL PIPELINE 3D ESCRIBE: SI", "EL PIPELINE 3D ESCRIBE: NO")
        };
        fila(y, Linea::nueva().d(buenos as u64).t(dice), if bien { VERDE } else { 0x00FF_5555 });
        y += 48;
        p.texto_escala(40, y, if bien { si } else { no }, if bien { VERDE } else { 0x00FF_5555 }, 3);
        fila(p.alto.saturating_sub(48), Linea::nueva().t(b"pulsa cualquier tecla para volver al escritorio"), TENUE);
        p.vaciar();
        return true;
    }
    p.texto_escala(40, y, "LA CPU (Ryzen 5 5600X, un nucleo)", TENUE, 2);
    y += 36;
    fila(y, Linea::nueva().d(cpu_us as u64).t(b" us, haciendo la MISMA cuenta"), CLARO);
    y += 48;
    if gpu_us > 0 && cpu_us >= gpu_us {
        let veces = cpu_us as u64 / gpu_us as u64;
        let mut l = Linea::nueva();
        l.t(b"LA 3060 FUE ").d(veces).t(b" VECES MAS RAPIDA");
        p.texto_escala(40, y, core::str::from_utf8(&l.b[..l.n]).unwrap_or(""), VERDE, 2);
        y += 48;
    } else if gpu_us > 0 {
        // ** Metal 24-09 17:08: el triangulo salio `x0` -- 791 us contra 631.
        // No es que la 3060 sea lenta: la cuenta por pixel es LIGERA y lo que
        // cuesta es escribir 1 MiB en la RAM del PC por PCIe (ese arranque, en
        // Gen1). Se dice como es en vez de pintar "0 veces".
        p.texto_escala(40, y, "AQUI GANO LA CPU: trabajo ligero", TENUE, 2);
        y += 36;
        fila(y, Linea::nueva().t(b"la cuenta por pixel es poca; lo que cuesta es llevar 1 MiB a tu RAM por PCIe"), TENUE);
        y += 40;
    }
    fila(y, Linea::nueva().d(buenos as u64).t(b" de 262144 pixeles iguales a la CPU, bit a bit"), if buenos as usize == fractal::PIXELES { VERDE } else { 0x00FF_5555 });
    y += 28;
    fila(
        y,
        Linea::nueva().t(match que {
            Vista::Triangulo => b"aritmetica entera: las aristas suman siempre el area" as &[u8],
            Vista::Escena => b"aritmetica entera: la CPU no dibuja, solo rehace la cuenta",
            _ => b"aritmetica entera Q4.28: sin redondeos distintos",
        }),
        TENUE,
    );
    y += 28;
    fila(y, Linea::nueva().t(b"1 MiB de tu RAM, prestado a la 3060 por la IOMMU"), TENUE);
    fila(p.alto.saturating_sub(48), Linea::nueva().t(b"pulsa cualquier tecla para volver al escritorio"), TENUE);
    p.vaciar();
    true
}

/// `gpu fractal`: lo que falte, el fractal, y el panel a pantalla completa.
pub(crate) fn orden_fractal(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 calcula el fractal (y la CPU, lo mismo)", INK_DIM);
    let r = calcular_fractal();
    let visto = matches!(r, Ok(v) if panel(p, v, Vista::Fractal));
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = visto };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  EL FRACTAL DE TU 3060, A PANTALLA COMPLETA (M5d F): mira la fila `fractal`\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el fractal no salio: mira la fila `fractal`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu triangulo`: lo que falte, el triangulo, y el panel a pantalla completa.
pub(crate) fn orden_triangulo(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 dibuja su primer triangulo", INK_DIM);
    let r = dibujar_triangulo();
    let visto = matches!(r, Ok(v) if panel(p, v, Vista::Triangulo));
    // SAFETY: como `panel_abierto`.
    unsafe { *core::ptr::addr_of_mut!(PANEL_ABIERTO) = visto };
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  EL PRIMER TRIANGULO DE TU 3060, A PANTALLA COMPLETA (M5d T0): mira la fila `triangulo`\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el triangulo no salio: mira la fila `triangulo`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu lienzo`: lo que falte hasta el primer sombreador, el lienzo, y
/// mostrarlo en la pantalla.
pub(crate) fn orden_lienzo(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "la 3060 pinta en la RAM del PC", INK_DIM);
    let r = hasta_el_lienzo();
    let visto = r.is_ok() && mostrar(p, 4);
    let g = &mut dsk.out.grid;
    if visto {
        g.with_ink(INK_GOOD);
        g.text(b"  LO QUE VES ARRIBA A LA DERECHA LO PINTO TU 3060: 16384 hilos, un pixel cada uno (M5d L)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el lienzo no salio: mira la fila `lienzo`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "lienzo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu sombreo`: S1..S3 si faltan, y el primer sombreador.
pub(crate) fn orden_sombreo(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "corriendo el primer sombreador de BMO-X en la 3060", INK_DIM);
    let mut r = if pedido() { Ok(0) } else { pedir() };
    if r.is_ok() && !ficha_leida() {
        r = ficha();
    }
    if r.is_ok() && !trabajado() {
        r = trabajar();
    }
    if r.is_ok() && !sombreado() {
        r = sombrear();
    }
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  EL PRIMER SOMBREADOR DE BMO-X CORRIO EN TU 3060: 32 hilos, cada uno escribio lo suyo (S4..S6 de M5d)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el sombreador no salio entero: mira la fila `sombreo`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "sombreo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `gpu computo`: S1, S2 y S3, parando en lo primero que no sale.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    paint_status(p, &dsk.run_box, "pidiendo el computo y mandandole trabajo al motor grafico", INK_DIM);
    let mut r = if pedido() { Ok(0) } else { pedir() };
    if r.is_ok() && !ficha_leida() {
        r = ficha();
    }
    if r.is_ok() && !trabajado() {
        r = trabajar();
    }
    let g = &mut dsk.out.grid;
    if r.is_ok() {
        g.with_ink(INK_GOOD);
        g.text(b"  EL MOTOR GRAFICO CORRIO NUESTRO TRABAJO: pago el semaforo de informe (S3 de M5d)\n");
    } else {
        g.with_ink(INK_ERR);
        g.text(b"  el motor grafico no corrio el trabajo: mira las filas `computo`, `ficha gr` y `gr trabajo`\n");
    }
    g.with_ink(INK_PLAIN);
    fila(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "computo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

fn no(s: &mut Output, m: u32) {
    s.with_ink(INK_ERR);
    s.text(b"NO: ");
    s.text(super::iommu::motivo(m));
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **Las filas `computo`, `ficha gr` y `gr trabajo`**, si se pidieron.
pub(crate) fn fila(s: &mut Output) {
    let c = estado();
    if let Some(r) = c.pedido {
        campo(s, b"computo");
        match r {
            Err(m) => no(s, m),
            Ok(p) => {
                s.with_ink(if pedido_bien(&Some(r)) { INK_GOOD } else { INK_ERR });
                s.text(b"0x");
                s.hex(computo::COMPUTO as u64, 8);
                s.text(b" (clase 0xC7C0, en el canal de GR0): ");
                s.text(objeto::estado(p.r.estado));
                s.with_ink(INK_ECHO);
                s.text(b"   GSP_RM_ALLOC en ");
                s.dec(p.espera_us / 1000);
                s.text(b" ms (numero ");
                s.dec(p.numero as u64);
                s.byte(b')');
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.ficha {
        campo(s, b"ficha gr");
        match r {
            Err(m) => no(s, m),
            Ok(f) => {
                s.with_ink(if f.bien() { INK_GOOD } else { INK_ERR });
                s.text(b"0x");
                s.hex(f.r.valor as u64, 8);
                s.with_ink(INK_ECHO);
                match c.lista {
                    Some(l) => {
                        s.text(b"; GR0 en la lista ");
                        s.dec(l as u64);
                        s.text(b" de la tabla");
                    }
                    None => s.text(b"; la tabla no dijo la lista de GR0"),
                }
                s.text(b"   GET_WORK_SUBMIT_TOKEN en ");
                s.dec(f.espera_us / 1000);
                s.text(b" ms (numero ");
                s.dec(f.numero as u64);
                s.byte(b')');
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.trabajo {
        campo(s, b"gr trabajo");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (semaforo, gp_get, pagado, lanzado, us) = computo::desempaquetar(v);
                if pagado {
                    s.with_ink(INK_GOOD);
                    s.text(b"EL MOTOR GRAFICO CORRIO: semaforo de informe PAGADO (0x");
                    s.hex(semaforo as u64, 8);
                    s.text(b")");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el GR NO pago: semaforo 0x" as &[u8] } else { b"no se lanzo (la MMU o el USERD no quedaron): semaforo 0x" });
                    s.hex(semaforo as u64, 8);
                }
                s.with_ink(INK_ECHO);
                s.text(b", GP_GET ");
                s.dec(gp_get as u64);
                if let Some((t, tabla)) = c.timbre {
                    s.text(b"; timbre 0x");
                    s.hex(t as u64, 8);
                    s.text(if tabla { b" (la lista de GR0 segun la tabla)" as &[u8] } else { b" (la ficha del RM)" });
                }
                s.text(b"   en ");
                s.dec(us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.sombreo {
        campo(s, b"sombreo");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenas, limpio, qmd, fin, lanzado, gp_get, us) = sombreador::desempaquetar(v);
                if sombreador::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"EL PRIMER SOMBREADOR CORRIO: ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el sombreador NO salio entero: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenas as u64);
                s.text(b" de 32 hilos escribieron lo suyo");
                if !limpio {
                    s.text(b" (y algo MAS paso de ellos)");
                }
                s.with_ink(INK_ECHO);
                s.text(b"; semaforo del QMD ");
                s.text(if qmd { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b", de informe ");
                s.text(if fin { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b", GP_GET ");
                s.dec(gp_get as u64);
                s.text(b"   en ");
                s.dec(us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.lienzo {
        campo(s, b"lienzo");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, qmd, fin, lanzado, gp_get, us) = lienzo::desempaquetar(v);
                if lienzo::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"LA 3060 PINTO EN LA RAM DEL PC: ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el lienzo NO salio entero: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 16384 pixeles como tocan");
                s.with_ink(INK_ECHO);
                s.text(b"; semaforo del QMD ");
                s.text(if qmd { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b", de informe ");
                s.text(if fin { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b", GP_GET ");
                s.dec(gp_get as u64);
                s.text(b"; IOVA 0x");
                s.hex(lienzo::IOVA, 8);
                s.text(b"   en ");
                s.dec(us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.blur {
        campo(s, b"blur");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, qmd, fin, lanzado, gp_get, us) = blur::desempaquetar(v);
                if blur::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"LA 3060 DESENFOCO ");
                    s.text(if c.blur_de_pantalla { b"UN TROZO DE TU PANTALLA: " as &[u8] } else { b"EL LIENZO: " });
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el blur NO salio igual que la CPU: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 16384 pixeles iguales a la cuenta de la CPU (7 x 7)");
                s.with_ink(INK_ECHO);
                s.text(b"; semaforo del QMD ");
                s.text(if qmd { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b", de informe ");
                s.text(if fin { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b", GP_GET ");
                s.dec(gp_get as u64);
                s.text(b"   en ");
                s.dec(us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.fractal {
        campo(s, b"fractal");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, qmd, fin, lanzado, gpu_us, cpu_us) = fractal::desempaquetar(v);
                if fractal::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"LA 3060 CALCULO EL FRACTAL (512x512): ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el fractal NO salio igual que la CPU: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 262144 pixeles iguales a la CPU");
                s.with_ink(INK_ECHO);
                s.text(b"; la 3060 en ");
                s.dec(gpu_us as u64);
                s.text(b" us, la CPU en ");
                s.dec(cpu_us as u64);
                s.text(b" us");
                if gpu_us > 0 {
                    s.text(b" (x");
                    s.dec(cpu_us as u64 / gpu_us as u64);
                    s.byte(b')');
                }
                s.text(b"; semaforos ");
                s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.triangulo {
        campo(s, b"triangulo");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, qmd, fin, lanzado, gpu_us, cpu_us) = triangulo::desempaquetar(v);
                if triangulo::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"EL PRIMER TRIANGULO DE LA 3060 (512x512): ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"el triangulo NO salio igual que la CPU: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 262144 pixeles iguales a la CPU");
                s.with_ink(INK_ECHO);
                s.text(b"; la 3060 en ");
                s.dec(gpu_us as u64);
                s.text(b" us, la CPU en ");
                s.dec(cpu_us as u64);
                s.text(b" us; semaforos ");
                s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.limpio3d {
        campo(s, b"3d");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, pagado, _, lanzado, gpu_us, _) = tresde::desempaquetar(v);
                if tresde::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"LA CLASE 3D LIMPIO EL DESTINO CON SU ROP: ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"la clase 3D NO limpio el destino entero: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 262144 pixeles del color de limpieza");
                s.with_ink(INK_ECHO);
                s.text(b"; semaforo de la clase 3D ");
                s.text(if pagado { b"PAGADO" as &[u8] } else { b"sin pagar" });
                s.text(b"   en ");
                s.dec(gpu_us as u64);
                s.text(b" us");
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    if let Some(r) = c.escena {
        campo(s, b"escena");
        match r {
            Err(m) => no(s, m),
            Ok(v) => {
                let (buenos, qmd, fin, lanzado, gpu_us, cpu_us) = escena::desempaquetar(v);
                if escena::sano(v) {
                    s.with_ink(INK_GOOD);
                    s.text(b"LA 3060 DIBUJO LA ESCENA 3D CON LUZ (512x512): ");
                } else {
                    s.with_ink(INK_ERR);
                    s.text(if lanzado { b"la escena NO salio igual que la CPU: " as &[u8] } else { b"no se lanzo: " });
                }
                s.dec(buenos as u64);
                s.text(b" de 262144 pixeles iguales a la CPU");
                s.with_ink(INK_ECHO);
                s.text(b"; la 3060 en ");
                s.dec(gpu_us as u64);
                s.text(b" us, la CPU en ");
                s.dec(cpu_us as u64);
                s.text(b" us; semaforos ");
                s.text(if qmd && fin { b"PAGADOS" as &[u8] } else { b"sin pagar" });
                s.with_ink(INK_PLAIN);
                s.byte(b'\n');
            }
        }
    }
    pipeline3d::fila(s, &c);
    giro::fila(s, &c);
    pantalla::fila(s, &c);
    video::fila(s, &c);
    imagen::fila(s, &c);
}
