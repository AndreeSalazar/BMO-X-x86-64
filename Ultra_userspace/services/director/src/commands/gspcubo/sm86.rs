//! **EL BACKEND DE LA RTX 3060 12G PARA VERRANO** -- la UNICA puerta del
//! escritorio entre la API de dibujo y la 3060 (`PLAN_EL_AISLAMIENTO.md` A2).
//!
//! [carril]  ROJO      arma el paquete que el kernel sube a la VRAM de la 3060
//! [consumo] NADA      corre cuando el propietario teclea `gpu verrano`
//!
//! # Por que una puerta
//!
//! `verrano.rs` (el cubo, el banco, la comparacion con la CPU) y `tablero.rs`
//! hablan VERRANO: `Frame`, `Vertex`, `Backend::draw`, `Stats`. No saben que
//! hay una 3060 debajo, ni que su codigo es SM86, ni que tiene un juez. Todo
//! eso vive AQUI, y en ningun otro fichero de `gspcubo/` (lo vigila
//! `la-3060`, regla S). El dia que haya otra GPU, trae SU fichero como este
//! -- su `kind` del BSF, su juez, sus ordenes -- y la API no cambia.
//!
//! # Lo que pasa al abrir
//!
//! ```text
//!    1  el sobre      el BSF con el `kind` de ESTA tarjeta (SM86, ABI
//!                     SM86_V1) y sus hashes; sin el, NO: no se compila nada
//!                     en marcha para salir del paso
//!    2  el juez       los dos programas, tal como viajan, con los registros
//!                     que la tarjeta les va a dar: un BODRIO no se manda
//!    3  la tarjeta    el motor grafico (lo que falte hasta `lienzo`) y la
//!                     ficha del GR
//! ```
//!
//! Y el kernel vuelve a juzgar en su puerta (`CUBO_VERRANO`): lo de aqui es
//! para decirlo claro en la pantalla, lo de alli es lo que no se puede saltar.

use bmo_bsf::{abi, kind, Bsf, Given, ModuleView, READS};
use bmo_gpu_ga10x::sass::juez;
use bmo_gpu_ga10x::{cubo as cu, tuberia as tu};
use bmo_userland as bmo;
use bmo_verrano::{check, Backend, Error, Frame, Image, Rect, Stats, VERTEX_BYTES, VERTEX_COLOR, VERTEX_POSITION};

use super::super::After;
use super::Texto;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

/// Como se dice esta tarjeta en la pantalla, y en las etiquetas del tablero.
pub(super) const NOMBRE: &[u8] = b"la RTX 3060 12G (SM86)";
pub(super) const ETIQUETA: &[u8] = b"LA 3060";

/// El sobre de los programas: fabricado en el anfitrion
/// (`ga10x/tests/bsf_sm86.rs`, que lo juzga antes), viaja dentro de `d.bex`.
const SOBRE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../platform/drivers/gpu/ga10x/sombreadores/cubo.bsf"));

/// **E5: los programas de BMOX-12 para la 3060** -- los `.cso` de FXC por
/// PROTON-X (su `Programa`, el emisor con las entradas en registros y el
/// pegamento del driver), fabricados y juzgados en el anfitrion
/// (`ga10x/tests/bmox12_sm86.rs`, que exige que sean ESTOS bytes). Aqui se
/// vuelven a juzgar al abrir, y el kernel en su puerta.
const BMOX12_VS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../platform/drivers/gpu/ga10x/sombreadores/bmox12_vs.sm86"));
const BMOX12_PS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../platform/drivers/gpu/ga10x/sombreadores/bmox12_ps.sm86"));

/// Lo que mide la caja del paquete que se le pasa a [`abrir`]: el paquete
/// mas grande que el kernel acepta (`tuberia::MAX_PAQUETE`), en paginas.
pub(super) const CAJA: u64 = (tu::MAX_PAQUETE as u64).div_ceil(4096) * 4096;

/// **Lo mas que dibuja un `draw` de V0**: la tanda (`anillo::MAX_VERTICES`).
///
/// [!] Los vertices van al paquete AL VUELO (`tuberia::escribir_paquete_de`):
/// ya no hay una copia en la pila. El 28-09 la habia --un `[Vertice;
/// tuberia::MAX_VERTICES]`, 384 KiB contra la pila de Ring 3 de 64 KiB-- y el
/// primer `gpu verrano` se salio por el fondo y tumbo la maquina.
const V0_MAX: usize = bmo_gpu_ga10x::anillo::MAX_VERTICES;

/// Las opciones que son de ESTA tarjeta (las palabras que las piden).
#[derive(Clone, Copy, Default)]
pub(super) struct Opciones {
    /// `sinldg`: el programa de vertice SIN sus LDG, fabricado aqui (la
    /// prueba de una variable del cuelgue del 25-09). No dibuja el cubo.
    pub sin_ldg: bool,
    /// `ligero`: las ordenes sin la escalera de T1c.
    pub ligero: bool,
    /// `anillo` (V1b): las ordenes de `ligero`, los vertices en RAM del PC
    /// y cada fotograma EN VUELO -- la CPU envia el siguiente mientras la
    /// tarjeta dibuja este (`bmo_gpu_ga10x::anillo`).
    pub anillo: bool,
    /// `coopera` (V1c): el anillo, y la CPU le dice a la tarjeta QUE hace
    /// falta limpiar -- donde estaba el cubo y donde va a estar --, en vez
    /// de la ventana entera (`anillo::ordenes_con`).
    pub coopera: bool,
    /// `exige` (V1c): antes del banco, los relojes de la tarjeta AL MAXIMO
    /// (`PERF_BOOST` al GSP-RM, 60 s): la CPU no deja que trabaje en reposo.
    pub exige: bool,
    /// `reposo` (E1): sin gobernador -- la tarjeta a los relojes que tenga,
    /// para medirla a proposito en reposo.
    pub reposo: bool,
    /// `bmox12` (E5): la tarjeta TRANSFORMA e ILUMINA ella con los
    /// programas de BMOX-12 traducidos por PROTON-X; se le dan el cbuffer y
    /// los vertices sin transformar del fotograma, no los de recorte. De
    /// uno en uno (no en el banco).
    pub bmox12: bool,
    /// El fotograma (el primer numero de las palabras; 30 si no hay).
    pub fotograma: u32,
    /// `antihorario` (P3b4b): con `bmox12`, delante es antihorario para el
    /// descarte del hardware (por defecto, horario: el de D3D). Por si el
    /// metal dice que la 3060 cuenta el giro al reves.
    pub antihorario: bool,
    /// `enram` (P3b4b 3): el destino es un bloque de RAM del escritorio --
    /// como el back buffer de una app, o la ventana de una app -- y la
    /// comparacion lee de ahi, no de la pantalla. ** Q0a2 (EL_FOCO, 10-10):
    /// tambien SIN `bmox12` -- la lamina de una app, en el banco --, de la
    /// medida del fotograma, y la LIMPIEZA la hace la 3060 (VRN1 con color),
    /// no la CPU llenando el bloque.
    pub enram: bool,
    /// ** Q0a3 (EL_FOCO, 10-10): la 3060 dibuja DIRECTO en la `Image` que se
    /// le da -- un bloque del escritorio, alineado a pagina y con filas de 128
    /// bytes: la ventana de una app con lamina --, sin bloque propio y sin
    /// copiarla de vuelta. No es una palabra de la orden: lo pide
    /// `gspcubo::laminas`.
    pub en_la_imagen: bool,
    /// `z` (P3b4c): con `bmox12`, el cubo SIN descarte de caras y CON la
    /// prueba de profundidad de la 3060 (LESS, se escribe, limpia a 1.0). Si
    /// la Z funciona, las caras de atras quedan detras: IGUAL a D3D12.
    pub z: bool,
    /// `ambas` (P3b4c): con `bmox12`, SIN descarte y SIN Z -- el testigo de
    /// `z`: las caras de atras se pintan encima y tiene que salir DISTINTO.
    pub ambas: bool,
}

/// **Las palabras de ESTA tarjeta** y que hacen: las que entiende
/// [`Opciones::de`], y las que la caja sugiere tras `gpu verrano` (una lista,
/// la misma para las dos cosas: si una deja de entenderse, deja de sugerirse).
pub(super) const PALABRAS: &[(&[u8], &[u8])] = &[
    (b"bmox12", b"E5: la 3060 transforma e ilumina con los programas de BMOX-12 traducidos por PROTON-X"),
    (b"ligero", b"sin la escalera de diagnostico: los fps de verdad"),
    (b"anillo", b"la CPU envia el fotograma siguiente mientras la tarjeta dibuja este"),
    (b"coopera", b"el anillo, y la CPU le dice a la tarjeta que limpiar"),
    (b"maximo", b"coopera y los relojes de la tarjeta al maximo"),
    (b"exige", b"los relojes de la tarjeta al maximo antes del banco"),
    (b"reposo", b"sin gobernador: la tarjeta en reposo, a proposito"),
    (b"sinldg", b"el de vertice SIN sus LDG (la prueba del 25-09): no dibuja el cubo"),
    (b"antihorario", b"con bmox12: delante es ANTIHORARIO para el descarte de la 3060 (si el cubo sale del reves)"),
    (b"enram", b"la 3060 dibuja en RAM del escritorio (como en el back buffer de una app), no en la pantalla; con banco inti, la lamina (Q0a2)"),
    (b"z", b"con bmox12: sin descarte y con la PROFUNDIDAD de la 3060; bien = IGUAL a D3D12"),
    (b"ambas", b"con bmox12: sin descarte y SIN profundidad, el testigo de z: tiene que salir DISTINTO"),
    (b"textura", b"T3: la 3060 MUESTREA por la puerta de las apps los 96 puntos de la prueba de CUDA; bien = IGUAL a la 3060 bajo Windows"),
];

impl Opciones {
    pub(super) fn de(palabras: &[u8]) -> Self {
        let mut o = Opciones { fotograma: palabras.split(|&c| c == b' ').find_map(super::numero).unwrap_or(30).min(359), ..Opciones::default() };
        for w in palabras.split(|&c| c == b' ') {
            match w {
                b"bmox12" => o.bmox12 = true,
                b"antihorario" => o.antihorario = true,
                b"enram" => o.enram = true,
                b"z" => o.z = true,
                b"ambas" => o.ambas = true,
                b"sinldg" => o.sin_ldg = true,
                b"ligero" => o.ligero = true,
                b"anillo" => {
                    o.anillo = true;
                    o.ligero = true;
                }
                b"coopera" => {
                    o.coopera = true;
                    o.anillo = true;
                    o.ligero = true;
                }
                b"exige" => o.exige = true,
                b"reposo" => o.reposo = true,
                // Todo lo que hay: la CPU coopera Y exige.
                b"maximo" => {
                    o.exige = true;
                    o.coopera = true;
                    o.anillo = true;
                    o.ligero = true;
                }
                _ => {}
            }
        }
        o
    }

    /// Si `w` es una palabra de ESTA tarjeta.
    pub(super) fn conoce(w: &[u8]) -> bool {
        PALABRAS.iter().any(|&(p, _)| p == w)
    }

    /// Como se dice el modo en el tablero.
    pub(super) fn modo(&self) -> &'static [u8] {
        if self.exige {
            return if self.coopera {
                b"maximo: coopera y relojes EXIGIDOS"
            } else if self.anillo {
                b"anillo, relojes EXIGIDOS"
            } else if self.ligero {
                b"ligero, relojes EXIGIDOS"
            } else {
                b"escalera, relojes EXIGIDOS"
            };
        }
        if self.coopera {
            b"coopera: la CPU le recorta la limpieza"
        } else if self.anillo {
            b"anillo: la CPU orquesta, no espera"
        } else if self.ligero {
            b"ligero, sin escalera"
        } else {
            b"con la escalera de T1c"
        }
    }
}

/// Lo que se supo al abrir.
pub(super) struct Abierto {
    /// Instrucciones que el juez miro (las dos, juntas).
    pub instrucciones: usize,
    pub bytes_vs: usize,
    pub bytes_ps: usize,
    /// De donde salen los programas (lo que se dice en la linea).
    pub origen: &'static [u8],
}

/// **El backend**: el paquete (programas del BSF + vertices) al kernel, y la
/// ventana leida de vuelta si se pide.
pub(super) struct Aparato<'a> {
    ficha: u64,
    paquete: &'a mut [u8],
    vs: &'static [u8],
    ps: &'static [u8],
    /// El de vertice de `sinldg`, fabricado aqui (0 bytes = el del sobre).
    propio: [u8; 4 * tu::PALABRAS_VS],
    propio_n: usize,
    ligero: bool,
    anillo: bool,
    coopera: bool,
    /// V1c: la caja del fotograma anterior (`Frame::cover`); `None` = no se
    /// sabe, se limpia todo.
    antes: Option<Rect>,
    /// V1c: pixeles que se mandaron limpiar, y en cuantos fotogramas.
    limpiados: u64,
    dibujos: u64,
    /// E1: el gobernador de los relojes.
    gobierno: Gobierno,
    /// E2: el cronometro de cada fase de `draw`.
    fases: Fases,
    /// E5: el fotograma de BMOX-12 que se dibuja (`Opciones::bmox12`).
    bmox12: Option<u32>,
    antihorario: bool,
    /// P3b4c: `z` y `ambas` (ver `Opciones`).
    z: bool,
    ambas: bool,
    /// `enram`: el bloque de RAM donde dibuja la 3060 y su medida en pixeles
    /// (1280x720). El BLOQUE mismo, no su direccion: vive lo que el aparato
    /// (un `Memoria` suelto se devuelve al salir de su alcance, y la 3060
    /// escribiria en RAM ya devuelta).
    enram: Option<(bmo::Memoria, usize)>,
    /// El modulo de vertice del BSF: su tabla de buffers juzga cada `Frame`.
    vertice: ModuleView<'static>,
    /// Leer la imagen de vuelta (la comparacion la quiere); el banco no.
    pub leer: bool,
    pub leer_ms: u64,
    /// Q0a3: dibuja en la `Image` misma (`Opciones::en_la_imagen`).
    en_la_imagen: bool,
}

/// **E2 -- DE QUE SON LOS US DEL ESCRITORIO** (26-09). Con `maximo` la pared
/// era ~35 us: 15 los prepara el kernel, 18 son de la 3060 EN PARALELO, y
/// ~20 no se veian. Tres fases de `draw`, en ciclos del TSC, sumadas:
///
/// ```text
///    cuentas   los vertices a bits y la caja de `coopera` (`Frame::cover`,
///              coma flotante POR SOFTWARE en el escritorio: E6)
///    paquete   escribir el paquete: los ~700 B de programas otra vez (E3)
///              y los vertices
///    puerta    la llamada al kernel entera; dentro, lo que el kernel dice
///              que tardo en preparar (`Stats::prepare_us`); lo demas es
///              entrar, salir, juzgar la huella y la espera del anillo
/// ```
#[derive(Default)]
struct Fases {
    n: u64,
    cuentas: u64,
    paquete: u64,
    puerta: u64,
    preparar_us: u64,
}

/// Donde cae la ventana de 1280x720 en esta pantalla (la misma cuenta que el
/// kernel: `cubo::ventana`).
pub(super) fn ventana(p: &bmo::Pantalla) -> (u32, u32) {
    (((p.ancho - cu::ANCHO) / 2) & !31, (p.alto - cu::ALTO) / 2)
}

/// Los dos programas de ESTA tarjeta en el sobre, comprobados (capas 1 a 4
/// de lo que se toma).
/// Y el modulo de vertice entero: su tabla de buffers es el CONTRATO de lo
/// que su programa lee (ver [`contrato`]).
fn programas(bsf: &Bsf<'static>) -> Option<(&'static [u8], &'static [u8], ModuleView<'static>)> {
    let mut vs = None;
    let mut ps = None;
    for m in bsf.modules() {
        let codigo = m.target(kind::SM86, abi::SM86_V1, 0).and_then(|t| t.code().ok());
        match m.name() {
            b"cubo_vertice" => vs = codigo.map(|c| (c, m)),
            b"cubo_pixel" => ps = codigo,
            _ => {}
        }
    }
    let (vs, modulo) = vs?;
    Some((vs, ps?, modulo))
}

// ** EL CONTRATO ENTRE INTI, VERRANO Y EL BSF (26-09) -- sin nada que pelear.
//
// ```text
//    la app    escribe `Vertex` de VERRANO en la lamina: desde el 09-10,
//              TITAN++ (`director.publica`, con los numeros de
//              `bmo_verrano::VERTEX_*`); antes, INTI
//    VERRANO   los lleva en `Frame::vertices`, tal cual
//    el BSF    su modulo `cubo_vertice` DICE, sacado de su SPIR-V, que lee
//              elementos de `stride` bytes desde el byte `base` del buffer 0
//    la 3060   lee el paquete que arma esta puerta: `tuberia::Vertice`
// ```
//
// Al ABRIR se exige que los cuatro digan lo mismo (`contrato`); al DIBUJAR,
// que el buffer sea un numero entero de elementos (`ModuleView::check`). Y la
// copia al paquete es de BITS, no una conversion: `tuberia::Vertice` y
// `Vertex` tienen la misma forma, y esto no compila si dejan de tenerla.
const _: () = assert!(tu::BYTES_VERTICE == VERTEX_BYTES && VERTEX_POSITION == 0 && VERTEX_COLOR == 16);

/// **El contrato**, comprobado: lo que el programa de vertice del BSF lee es
/// lo que VERRANO (y INTI) escriben. `Err` = lo que no cuadra, dicho.
fn contrato(m: &ModuleView) -> Result<(), &'static [u8]> {
    if m.binding_count() != 1 {
        return Err(b"  NO  el programa de vertice del BSF pide otros buffers que el de los vertices: VERRANO V0 solo da uno");
    }
    let b = m.binding(0);
    if (b.set, b.binding, b.storage, b.access) != (0, 0, true, READS) {
        return Err(b"  NO  el programa de vertice del BSF no lee un buffer de almacenamiento set 0 binding 0: no es el que VERRANO le da");
    }
    if b.base_bytes != 0 || b.stride != VERTEX_BYTES as u32 {
        return Err(b"  NO  el BSF lee vertices de otra medida que la de VERRANO (Vertex: 32 bytes, posicion y color): no se entienden");
    }
    Ok(())
}

/// **Abrir la 3060 para VERRANO**: el sobre, el juez y la tarjeta. `Err` ya
/// lo dijo en el panel.
pub(super) fn abrir<'a>(dsk: &mut Desktop, p: &bmo::Pantalla, caja: &'a mut [u8], op: Opciones) -> Result<(Aparato<'a>, Abierto), After> {
    let Some((vs_bsf, ps_bsf, modulo)) = Bsf::parse(SOBRE).ok().and_then(|b| programas(&b)) else {
        return Err(linea(dsk, b"  NO  el BSF no trae codigo SM86 (ABI SM86_V1) que se sostenga: no se dibuja nada", INK_ERR));
    };
    // El contrato, ANTES que nada: lo que el programa lee es lo que se le da.
    if let Err(que) = contrato(&modulo) {
        return Err(linea(dsk, que, INK_ERR));
    }
    // E5: los de BMOX-12 en vez de los de V0.
    let (vs, ps) = if op.bmox12 { (BMOX12_VS, BMOX12_PS) } else { (vs_bsf, ps_bsf) };
    // El juez, ANTES de que la 3060 vea nada.
    let r = tu::REGISTROS;
    let juicio = juez::juzgar_programa(vs, r).map_err(|b| ("vertice", b)).and_then(|a| juez::juzgar_programa(ps, r).map(|b| a.instrucciones + b.instrucciones).map_err(|b| ("pixel", b)));
    let instrucciones = match juicio {
        Ok(n) => n,
        Err((cual, b)) => {
            let mut t = Texto::nuevo();
            let _ = core::fmt::write(&mut t, format_args!("  NO  el programa de {cual} del BSF:\n  {b}\n  {}", juez::REMATE));
            return Err(linea(dsk, t.s(), INK_ERR));
        }
    };
    if (op.z || op.ambas) && !op.bmox12 {
        return Err(linea(dsk, b"  NO  z y ambas van con bmox12 (los paquetes VRN1 con indices)", INK_ERR));
    }
    // `enram`: el destino, un bloque del escritorio de hasta 1280x720 (como
    // el back buffer de una app). Q0a2: con los programas de BMOX-12 o con
    // los de V0 (la lamina), los dos por VRN1.
    let enram = if op.enram && !op.en_la_imagen {
        let pixeles = (cu::ANCHO * cu::ALTO) as usize;
        let Some(b) = bmo::Memoria::request(4 * pixeles as u64) else {
            return Err(linea(dsk, b"  NO  sin memoria para el destino de 1280x720", INK_ERR));
        };
        Some((b, pixeles))
    } else {
        None
    };
    let mut propio = [0u8; 4 * tu::PALABRAS_VS];
    let propio_n = if op.sin_ldg { tu::bytes(&tu::vertice_sin_ldg(), &mut propio) } else { 0 };
    // Lo que falte del motor grafico (con `init` por defecto, casi todo).
    if super::super::verificar::preparar_hasta(dsk, p, b"lienzo").is_err() {
        dsk.field.n = 0;
        return Err(After::Settle);
    }
    let ficha = super::super::gspcomputo::ficha_del_gr().map_err(|m| motivo(dsk, m))?;
    let origen: &'static [u8] = if op.bmox12 { b"de BMOX-12 por PROTON-X" } else { b"del BSF" };
    let abierto = Abierto { instrucciones, bytes_vs: vs.len(), bytes_ps: ps.len(), origen };
    Ok((Aparato { ficha, paquete: caja, vs, ps, propio, propio_n, ligero: op.ligero, anillo: op.anillo, coopera: op.coopera, antes: None, limpiados: 0, dibujos: 0, gobierno: Gobierno { activo: op.anillo && !op.reposo, ..Gobierno::default() }, fases: Fases::default(), bmox12: op.bmox12.then_some(op.fotograma), antihorario: op.antihorario, z: op.z, ambas: op.ambas, enram, vertice: modulo, leer: true, leer_ms: 0, en_la_imagen: op.en_la_imagen }, abierto))
}

/// Un color de VERRANO (`[r, g, b, a]` de 0 a 1) como pixel de la memoria,
/// `B8G8R8A8`: `a << 24 | r << 16 | g << 8 | b`.
fn pixel_de(c: [f32; 4]) -> u32 {
    let b = |x: f32| ((x.clamp(0.0, 1.0) * 255.0) + 0.5) as u32;
    b(c[3]) << 24 | b(c[0]) << 16 | b(c[1]) << 8 | b(c[2])
}

impl Aparato<'_> {
    /// ** Q0a2: el destino EN RAM de este fotograma (`enram`): el bloque de
    /// `abrir`, de la medida del fotograma. `Ok(None)` sin `enram`; `Err`
    /// si no cabe o sus filas no son de 128 bytes (la 3060 lo pide).
    fn destino_en_ram(&self, frame: &Frame, out: &Image) -> Result<Option<(u64, bmo_gpu_ga10x::destino::Destino)>, Error> {
        let (ancho, alto) = (frame.viewport.width, frame.viewport.height);
        let d = bmo_gpu_ga10x::destino::Destino { fila: 4 * ancho, ancho, alto, rgb: false };
        // ** Q0a3: la `Image` ES el destino (la ventana de la app).
        if self.en_la_imagen {
            let va = out.pixels.as_ptr() as u64;
            if va % 4096 != 0 || !d.valido() {
                return Err(Error::Image);
            }
            return Ok(Some((va, d)));
        }
        let Some((b, pixeles)) = self.enram.as_ref() else { return Ok(None) };
        if !d.valido() || (ancho as usize) * (alto as usize) > *pixeles {
            return Err(Error::Image);
        }
        Ok(Some((b.base() as u64, d)))
    }
}

impl Backend for Aparato<'_> {
    fn draw(&mut self, frame: &Frame, out: &mut Image) -> Result<Stats, Error> {
        let t0 = bmo::ciclos();
        // V2 (11-10): EN RAM va por VRN1 (`tuberia`), que toma miles de
        // vertices; el anillo del cubo de V0, sus 24.
        let tope = if self.en_la_imagen || self.enram.is_some() { tu::MAX_VERTICES } else { V0_MAX };
        check(frame, out, tope)?;
        // Lo que se le da al programa, contra lo que el programa dice leer.
        let dado = Given { set: 0, binding: 0, addr: frame.vertices.as_ptr() as u64, bytes: (frame.vertices.len() * VERTEX_BYTES) as u64, writable: false };
        self.vertice.check(&[dado]).map_err(|_| Error::Vertices)?;
        // En la PANTALLA, la ventana del cubo y su FONDO son los de las
        // ordenes de X5. ** Q0a2: EN RAM, la medida del fotograma (la que
        // quepa en el bloque, con filas de 128 bytes) y su fondo lo limpia la
        // 3060 (`destino_en_ram`).
        let en_ram = self.destino_en_ram(frame, out)?;
        if en_ram.is_none() && ((frame.viewport.width, frame.viewport.height) != (cu::ANCHO, cu::ALTO) || frame.clear.map(f32::to_bits) != cu::FONDO) {
            return Err(Error::Image);
        }
        let limpiar = if self.coopera { self.recorte(frame) } else { None };
        let t1 = bmo::ciclos();
        let vs = if self.propio_n > 0 { &self.propio[..self.propio_n] } else { self.vs };
        if let Some(f) = self.bmox12 {
            // P3b4b: el cbuffer, los 24 vertices SIN transformar y sus 36
            // INDICES; que caras miran a la camara lo decide la 3060 (el
            // descarte de las traseras), no la CPU. `frame` (la tanda) es
            // solo lo que dibuja el juez de la CPU.
            let mut datos = [0u8; 2048];
            let (n, bytes, desde) = bmo_cubo::tanda::datos_indexados(f, cu::ANCHO, cu::ALTO, &mut datos).ok_or(Error::Vertices)?;
            // `enram`: el destino, limpio con el FONDO -- ** Q0a2: por la 3060
            // (VRN1 con color); hasta el 10-10 la CPU llenaba los 3,6 MB del
            // bloque en cada fotograma --.
            let destino = en_ram;
            // P3b4c: con `z` o `ambas`, sin descarte (las 12 caras a la 3060);
            // con `z`, la Z de BMOX-12 (LESS, se escribe, limpia a 1.0).
            let (descarte, z) = if self.z {
                (tu::Descarte::Ninguna, Some(bmo_gpu_ga10x::profundidad::Z { funcion: 2, escribir: true, limpiar: Some(bmo_gpu_ga10x::profundidad::UNO) }))
            } else if self.ambas {
                (tu::Descarte::Ninguna, None)
            } else {
                (tu::Descarte::Traseras, None)
            };
            let color = destino.map(|_| cu::PIXEL_FONDO);
            let dibujo = tu::Dibujo { indices: Some(desde as u32), vertices: bmo_cubo::NUM_VERTICES as u32, descarte, antihorario: self.antihorario, destino, z, color, texturas: 0, cadena: false, pantalla: false, banco: 0 };
            tu::escribir_paquete_dibujo(self.paquete, self.ficha as u32, vs, self.ps, n, &datos[..bytes], dibujo).ok_or(Error::Vertices)?;
        } else if let Some(destino) = en_ram {
            // ** Q0a2 (EL_FOCO, 10-10): los vertices de V0 -- los de la LAMINA
            // de una app, en el banco -- directo al VRN1, el destino en RAM y su
            // fondo limpiado por la 3060. Los triangulos ya vienen escogidos
            // (la tanda): sin descarte ni Z.
            let v = frame.vertices.iter().map(|s| tu::Vertice { posicion: s.position.map(f32::to_bits), color: s.color.map(f32::to_bits) });
            // ** V2 (11-10): la profundidad y el descarte que pide el Frame,
            // los de BMOX-12 (`-z`): MENOR que, se escribe, limpia a 1.0; y
            // delante horario, lo de D3D por defecto.
            let z = frame.depth.then_some(bmo_gpu_ga10x::profundidad::Z { funcion: 2, escribir: true, limpiar: Some(bmo_gpu_ga10x::profundidad::UNO) });
            let descarte = if frame.cull == bmo_verrano::Cull::Back { tu::Descarte::Traseras } else { tu::Descarte::Ninguna };
            let dibujo = tu::Dibujo { vertices: frame.vertices.len() as u32, destino: Some(destino), color: Some(pixel_de(frame.clear)), z, descarte, antihorario: false, ..tu::Dibujo::default() };
            tu::escribir_paquete_dibujo_de(self.paquete, self.ficha as u32, vs, self.ps, v, dibujo).ok_or(Error::Vertices)?;
        } else if frame.depth || frame.cull != bmo_verrano::Cull::None {
            // V2: a la pantalla (V0) no hay z-buffer ni descarte: EN RAM si.
            return Err(Error::Image);
        } else {
            let v = frame.vertices.iter().map(|s| tu::Vertice { posicion: s.position.map(f32::to_bits), color: s.color.map(f32::to_bits) });
            tu::escribir_paquete_de(self.paquete, self.ficha as u32, vs, self.ps, v, limpiar.map(|r| (r.x0 | r.x1 << 16, r.y0 | r.y1 << 16))).ok_or(Error::Vertices)?;
        }
        let t2 = bmo::ciclos();
        let modo = if self.coopera {
            bmo::CUBO_ANILLO | bmo::CUBO_COOPERA
        } else if self.anillo {
            bmo::CUBO_ANILLO
        } else if self.ligero {
            bmo::CUBO_LIGERO
        } else {
            0
        };
        let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_VERRANO | modo | self.paquete.as_ptr() as u64).map_err(Error::Device)?;
        let t3 = bmo::ciclos();
        let f = &mut self.fases;
        f.n += 1;
        f.cuentas += t1 - t0;
        f.paquete += t2 - t1;
        f.puerta += t3 - t2;
        f.preparar_us += cu::preparado(r).1 as u64;
        let (us, tris, etapas, _) = cu::desempaquetar(r);
        if !cu::sano(r) {
            return Err(Error::Device(ESCALERA | etapas));
        }
        let (warm, prepare_us) = cu::preparado(r);
        let in_flight = cu::es_en_vuelo(r);
        let (wait_us, device_us) = if in_flight { cu::vuelo(r) } else { (0, us) };
        if in_flight {
            self.gobierno.mirar(wait_us, self.dibujos);
        }
        let st = Stats { triangles: tris, device_us, prepare_us, warm, in_flight, wait_us };
        // Q0a3: lo pinto la 3060 en la `Image` misma: no hay nada que leer.
        if !self.leer || self.en_la_imagen {
            return Ok(st);
        }
        let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
        let desde = bmo::ciclos();
        let n = (out.width * out.height) as usize;
        // `enram`: lo que pinto la 3060 esta en NUESTRA RAM: se lee de ahi.
        if let Some((b, m)) = self.enram.as_ref() {
            let (va, m) = (b.base() as u64, *m);
            // SAFETY: el bloque de `abrir`; la 3060 acabo (el kernel espero
            // su semaforo y devolvio el prestamo).
            let d = unsafe { core::slice::from_raw_parts(va as *const u32, m) };
            for (o, &x) in out.pixels[..n.min(m)].iter_mut().zip(d) {
                *o = 0xFF00_0000 | x;
            }
            self.leer_ms = (bmo::ciclos() - desde) * 1000 / hz;
            return Ok(st);
        }
        for k in 0..n / 2 {
            let d = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_LEER | k as u64).map_err(Error::Device)?;
            out.pixels[2 * k] = 0xFF00_0000 | d as u32;
            out.pixels[2 * k + 1] = 0xFF00_0000 | (d >> 32) as u32;
        }
        self.leer_ms = (bmo::ciclos() - desde) * 1000 / hz;
        Ok(st)
    }

    /// Lo que el anillo dejo en vuelo, pagado (`CUBO_VACIAR`). Sin anillo
    /// no queda nada: cada `draw` ya espero el suyo.
    fn finish(&mut self) -> Result<u32, Error> {
        if !self.anillo {
            return Ok(0);
        }
        bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_CUBO, bmo::CUBO_VACIAR).map(|us| us as u32).map_err(Error::Device)
    }
}

impl Aparato<'_> {
    /// **V1c: lo que hace falta limpiar** antes de dibujar `frame`: su caja
    /// (`Frame::cover`) unida a la del anterior (lo que hay que borrar); lo
    /// demas de la ventana ya es fondo. `None` = la ventana entera (el
    /// primero, o una caja que no se sabe). La prueba de que da lo mismo:
    /// `bmo_verrano::cpu`, `la_limpieza_recortada_da_lo_mismo`.
    fn recorte(&mut self, frame: &Frame) -> Option<Rect> {
        let ahora = frame.cover();
        let r = self.antes.zip(ahora).map(|(a, c)| a.union(c));
        self.antes = ahora;
        self.limpiados += r.unwrap_or(Rect::full(frame.viewport)).area();
        self.dibujos += 1;
        r
    }

    /// **E2**: cuanto de cada fotograma se fue en cada fase de `draw`, en
    /// decimas de us por fotograma: `(cuentas, paquete, puerta, preparar)`.
    pub(super) fn fases(&self) -> Option<(u64, u64, u64, u64)> {
        let f = &self.fases;
        if f.n == 0 {
            return None;
        }
        let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
        let d = |c: u64| c * 10_000_000 / hz / f.n;
        Some((d(f.cuentas), d(f.paquete), d(f.puerta), f.preparar_us * 10 / f.n))
    }

    /// Las fases de E2, dichas.
    pub(super) fn nota_fases(&self, t: &mut Texto) {
        if let Some((c, pq, pu, pr)) = self.fases() {
            t.t(b"E2, dentro de draw por fotograma: cuentas ");
            decimas(t, c);
            t.t(b", paquete ");
            decimas(t, pq);
            t.t(b", puerta ");
            decimas(t, pu);
            t.t(b" (el kernel prepara ");
            decimas(t, pr);
            t.t(b", el resto es entrar, salir y esperar)");
        }
    }

    /// Lo que la puerta tiene que decir del banco (V1c: cuanto se limpio).
    pub(super) fn nota(&self, t: &mut Texto) {
        if self.coopera && self.dibujos > 0 {
            let ventana = self.dibujos * (cu::ANCHO * cu::ALTO) as u64;
            t.t(b"la CPU recorto la limpieza: ").d(self.limpiados * 100 / ventana).t(b"% de la ventana de media (").d(self.limpiados / self.dibujos).t(b" pixeles por fotograma, de ").d((cu::ANCHO * cu::ALTO) as u64).t(b")");
        }
    }
}

/// **E1 -- EL GOBERNADOR de los relojes.** El orquestador decide solo, por
/// lo que mide: si la CPU espera (media de los ultimos [`VENTANA`]
/// fotogramas en vuelo por encima de [`UMBRAL_US`]), el cuello es la
/// tarjeta, y se le EXIGE (`PERF_BOOST`, sin parar el banco a esperar la
/// rampa); mientras siga el trabajo, se RENUEVA antes de que caduque (el RM
/// la da por [`SUBIDA_SEGUNDOS`]); y al acabar se SUELTA: la tarjeta vuelve
/// a reposo. Solo mira fotogramas en vuelo: en los demas modos la CPU ya
/// espera cada uno y esperar no dice quien es el cuello.
#[derive(Default)]
struct Gobierno {
    activo: bool,
    /// Cuando se subio la ultima vez (ciclos); `None` = no se subio.
    subido: Option<u64>,
    esperas: [u32; VENTANA],
    vistas: usize,
    /// Cuantas ordenes se mandaron (subir y renovar) y cuantas acepto el RM.
    pedidas: u32,
    aceptadas: u32,
    /// En que fotograma exigio por primera vez el gobernador.
    primera: Option<u64>,
}

/// Los fotogramas que se miran para decidir.
const VENTANA: usize = 8;
/// Esperar de media mas que esto es que la tarjeta no da abasto.
const UMBRAL_US: u32 = 10;
/// Se renueva a los 50 s de una subida de 60.
const RENOVAR_S: u64 = bmo_gpu_ga10x::control::SUBIDA_SEGUNDOS as u64 - 10;

impl Gobierno {
    fn mirar(&mut self, espera_us: u32, dibujo: u64) {
        if !self.activo {
            return;
        }
        self.esperas[self.vistas % VENTANA] = espera_us;
        self.vistas += 1;
        let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);
        let ahora = bmo::ciclos();
        let toca = match self.subido {
            Some(desde) => ahora.wrapping_sub(desde) / hz >= RENOVAR_S,
            None => self.vistas >= VENTANA && self.esperas.iter().map(|&e| e as u64).sum::<u64>() / VENTANA as u64 > UMBRAL_US as u64,
        };
        if toca {
            if self.subido.is_none() && self.primera.is_none() {
                self.primera = Some(dibujo);
            }
            self.subir();
        }
    }

    fn subir(&mut self) {
        self.pedidas += 1;
        if super::super::gsprelojes::mandar(true) {
            self.aceptadas += 1;
        }
        self.subido = Some(bmo::ciclos());
    }
}

impl Aparato<'_> {
    /// **Exigir** (`exige`): la tarjeta al maximo antes de darle trabajo
    /// (con la rampa y el P-state, que aqui SI se puede esperar), y lo que
    /// paso, dicho en `t`. El gobernador lo renovara y lo soltara.
    pub(super) fn exigir(&mut self, t: &mut Texto) {
        exigir(t);
        self.gobierno.activo = true;
        self.gobierno.subido = Some(bmo::ciclos());
    }

    /// **Al acabar**: el gobernador suelta lo que subio, y lo cuenta en `t`.
    pub(super) fn cerrar(&mut self, t: &mut Texto) {
        let g = &mut self.gobierno;
        if g.subido.is_none() {
            if g.activo && g.vistas > 0 {
                t.t(b"el gobernador no exigio: la CPU no espero a la tarjeta (la tarjeta daba abasto)");
            }
            return;
        }
        let soltado = super::super::gsprelojes::mandar(false);
        match g.primera {
            Some(f) => t.t(b"el gobernador EXIGIO solo en el fotograma ").d(f),
            None => t.t(b"exigido antes del banco"),
        };
        t.t(b"; ").d(g.pedidas as u64).t(b" orden(es) al GSP-RM, ").d(g.aceptadas as u64).t(b" aceptada(s); al acabar, ").t(if soltado { b"SOLTADO: la tarjeta vuelve a reposo" as &[u8] } else { b"no se pudo soltar (baja sola a los 60 s)" });
        g.subido = None;
    }
}

/// **Exigir** (`exige`): la tarjeta al maximo antes de darle trabajo, y lo
/// que paso, dicho en `t`.
fn exigir(t: &mut Texto) {
    let p = |t: &mut Texto, k: Option<u8>| {
        match k {
            Some(k) => t.t(b"P").d(k as u64),
            None => t.t(b"P?"),
        };
    };
    match super::super::gsprelojes::exigir() {
        Some((bien, antes, despues)) => {
            t.t(if bien { b"la CPU le EXIGIO a la tarjeta: PERF_BOOST aceptado, " as &[u8] } else { b"la CPU le exigio a la tarjeta, y el GSP-RM NO acepto PERF_BOOST: " });
            p(t, antes);
            t.t(b" -> ");
            p(t, despues);
            t.t(b" (`gpu salud`, fila `relojes`)");
        }
        None => {
            t.t(b"no se pudo exigir: el GSP-RM aun no esta");
        }
    }
}

/// `Error::Device(ESCALERA | etapas)`: se lanzo y no se pago entero.
const ESCALERA: u32 = 0x5E00;

/// **Decir por que la 3060 no dibujo**, con lo que ESTA tarjeta sabe contar:
/// la escalera de T1c y los avisos del GSP. `en` = el fotograma del banco.
pub(super) fn fallo(dsk: &mut Desktop, e: Error, en: Option<u32>, op: Opciones) -> After {
    match e {
        Error::Device(m) if m & 0xFF00 == ESCALERA => {
            let g = &mut dsk.out.grid;
            g.with_ink(INK_ERR);
            match en {
                Some(i) => {
                    g.text(b"  NO  el banco se paro en el fotograma ");
                    g.dec(i as u64);
                }
                None => g.text(b"  NO  VERRANO en la 3060 no se pago entero"),
            }
            if op.coopera {
                g.text(b" (coopera: sin escalera; sin `coopera` dice donde)");
            } else if op.anillo {
                g.text(b" (anillo: sin escalera; sin `anillo` dice donde)");
            } else if op.ligero {
                g.text(b" (ligero: sin escalera; sin `ligero` dice donde)");
            }
            g.text(b"; la escalera:\n");
            g.with_ink(INK_PLAIN);
            super::super::gspcomputo::escalera(g);
            // Y lo que el GSP conto: un Xid 31 es un FALLO DE PAGINA (la
            // direccion que lee el LDG), un 13 una excepcion del sombreador.
            super::super::gspcola::avisos(g, 4);
            if op.sin_ldg {
                g.with_ink(INK_ERR);
                g.text(b"  SIN LDG y aun asi colgado en los VERTICES: los LDG quedan ABSUELTOS; es otra cosa del programa\n");
                g.with_ink(INK_PLAIN);
            }
            dsk.field.n = 0;
            After::Settle
        }
        Error::Device(m) => motivo(dsk, m),
        _ => linea(dsk, b"  NO  el fotograma no es valido para la tuberia fija de la 3060 (1280x720, el fondo del estudio)", INK_ERR),
    }
}

/// **`gpu verrano textura` (P3b4c.8 T3)**: la prueba de CUDA del propietario
/// (`docs/metal/tex_cuda/`), por la 3060 en BMO-X. Una textura 4x4 en RAM del
/// escritorio, 8 recetas VRN2 por la PUERTA DE LAS APPS
/// (`IOMMU_OP_GPU_DIBUJAR`: el kernel presta la textura solo lectura, pone
/// sus TIC/TSC y el asa), cada una con 12 cuadros de uv constante; el pixel
/// del centro de cada cuadro contra `texturas::prueba::ESPERADO` -- lo que la
/// 3060 dio bajo CUDA, que el muestreador de la casa iguala en el banco.
pub(super) fn textura(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    use bmo_gpu_ga10x::receta;
    use bmo_gpu_ga10x::texturas::prueba as pr;
    let (w, h) = (cu::ANCHO, cu::ALTO);
    if p.ancho < w || p.alto < h {
        return linea(dsk, b"  NO  la pantalla es mas chica que 1280x720", INK_ERR);
    }
    // El GR preparado (su ficha la pone el kernel en la receta).
    if super::super::verificar::preparar_hasta(dsk, p, b"lienzo").is_err() {
        dsk.field.n = 0;
        return After::Settle;
    }
    if let Err(m) = super::super::gspcomputo::ficha_del_gr() {
        return motivo(dsk, m);
    }
    let pixeles = (w * h) as usize;
    let (Some(destino), Some(texturab), Some(cajab)) = (bmo::Memoria::request(4 * pixeles as u64), bmo::Memoria::request(4096), bmo::Memoria::request((receta::MAX_RECETA as u64).div_ceil(4096) * 4096)) else {
        return linea(dsk, b"  NO  sin memoria para el destino, la textura y la receta", INK_ERR);
    };
    // SAFETY: tres bloques de este proceso, alineados a pagina, de las
    // medidas pedidas, que solo se usan aqui y no se pisan.
    let (dst, tex, caja) = unsafe {
        (
            core::slice::from_raw_parts_mut(destino.base() as *mut u32, pixeles),
            core::slice::from_raw_parts_mut(texturab.base() as *mut u32, pr::TEXELES),
            core::slice::from_raw_parts_mut(cajab.base(), receta::MAX_RECETA),
        )
    };
    tex.copy_from_slice(&pr::texeles());
    let mut datos = [0u8; pr::BYTES_DATOS];
    let (mut buenas, mut us) = (0u32, 0u64);
    let mut por_modo = [0u32; 8];
    let mut primera: Option<(usize, usize, u32)> = None;
    // Los 96 leidos, y los 12 del BARRIDO (el noveno dibujo, `m == 8`).
    let mut dados = [[0u32; 12]; 9];
    for m in 0..9 {
        let n = if m < 8 {
            pr::receta(m, texturab.base() as u64, destino.base() as u64, &mut datos, caja)
        } else {
            pr::receta_con(false, 3, &pr::BARRIDO, texturab.base() as u64, destino.base() as u64, &mut datos, caja)
        };
        if n.is_none() {
            return linea(dsk, b"  NO  la receta de la prueba no se sostiene (el banco la escribe: esto no deberia pasar)", INK_ERR);
        }
        match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_DIBUJAR, caja.as_ptr() as u64) {
            Ok(v) if cu::sano(v) => {
                us += cu::desempaquetar(v).0 as u64;
                for k in 0..pr::PUNTOS.len() {
                    let (x, y) = pr::centro(k);
                    // SAFETY: dentro del destino (1280x720); lo escribio la
                    // 3060 por DMA: se lee con `read_volatile`.
                    let dado = unsafe { core::ptr::read_volatile(&dst[(y * w + x) as usize]) };
                    dados[m][k] = dado;
                    if m == 8 {
                        continue;
                    }
                    if dado == pr::ESPERADO[m][k] {
                        buenas += 1;
                        por_modo[m] += 1;
                    } else if primera.is_none() {
                        primera = Some((m, k, dado));
                    }
                }
            }
            Ok(v) => {
                let (us, tris, etapas, lanzado) = cu::desempaquetar(v);
                let mut t = Texto::nuevo();
                t.t(b"  NO  la 3060 no pago la receta de ").t(pr::NOMBRES[m].as_bytes()).t(b": lanzado ").d(lanzado as u64).t(b", escalera ").d(etapas as u64).t(b", ").d(tris as u64).t(b" triangulos, ").d(us as u64).t(b" us (el `gsp aviso` de `gpu` dira si fue un Xid)");
                return linea(dsk, t.s(), INK_ERR);
            }
            Err(mo) => return motivo(dsk, mo),
        }
    }
    // Lo ultimo que pinto (Linear Border), en la ventana.
    let (x0, y0) = ventana(p);
    p.marcar(x0, y0, w, h);
    for y in 0..h {
        for x in 0..w {
            let c = dst[(y * w + x) as usize];
            p.punto_ya_marcado(x0 + x, y0 + y, (c & 0xFF) << 16 | (c & 0xFF00) | (c >> 16 & 0xFF));
        }
    }
    p.vaciar();
    let g = &mut dsk.out.grid;
    g.with_ink(if buenas == 96 { INK_GOOD } else { INK_ERR });
    g.text(b"  textura: ");
    g.dec(buenas as u64);
    g.text(if buenas == 96 { b" de 96: IGUAL a la 3060 bajo CUDA (Windows), bit a bit en 8 bits; la 3060 dibujo en " as &[u8] } else { b" de 96 como la 3060 bajo CUDA: DISTINTO; la 3060 dibujo en " });
    g.dec(us);
    g.text(b" us las 8
");
    g.with_ink(INK_PLAIN);
    g.text(b"           ");
    for (m, &k) in por_modo.iter().enumerate() {
        g.text(pr::NOMBRES[m].as_bytes());
        g.text(b" ");
        g.dec(k as u64);
        g.text(if m + 1 < 8 { b"/12, " as &[u8] } else { b"/12
" });
    }
    if let Some((m, k, dado)) = primera {
        let (u, v) = pr::PUNTOS[k];
        let mut t = Texto::nuevo();
        let _ = core::fmt::write(&mut t, format_args!("           el primero: {} en ({u}, {v}): la 3060 {dado:#010x}, CUDA {:#010x} (R en el byte bajo)
", pr::NOMBRES[m], pr::ESPERADO[m][k]));
        g.text(t.s());
    }
    // ** TODO lo leido, para ver el patron y no adivinar: en los de PUNTO,
    // que texel salio (`xy`: columna y fila; BB el borde; -- vacio; ?? otra
    // cosa); en los LINEALES, el pixel (RRGGBB). Y debajo lo que dijo CUDA.
    let etiqueta = |t: &mut Texto, p: u32, punto: bool| {
        if !punto {
            let _ = core::fmt::write(t, format_args!(" {:02X}{:02X}{:02X}", p & 0xFF, p >> 8 & 0xFF, p >> 16 & 0xFF));
            return;
        }
        match pr::que_texel(p) {
            Some(16) => t.t(b" BB"),
            Some(17) => t.t(b" --"),
            Some(i) => t.t(b" ").d((i % 4) as u64).d((i / 4) as u64),
            None => t.t(b" ??"),
        };
    };
    for m in 0..9 {
        let punto = m < 4 || m == 8;
        let mut t = Texto::nuevo();
        t.t(b"           ").t(if m < 8 { pr::NOMBRES[m].as_bytes() } else { b"BARRIDO Point Clamp" }).t(b": 3060");
        for k in 0..12 {
            etiqueta(&mut t, dados[m][k], punto);
        }
        t.t(b"\n           ").t(if m < 8 { b"             CUDA" as &[u8] } else { b"         si 4 texeles" });
        for k in 0..12 {
            if m < 8 {
                etiqueta(&mut t, pr::ESPERADO[m][k], punto);
            } else {
                t.t(b" ").d(pr::BARRIDO_BIEN[k] as u64).d(pr::BARRIDO_BIEN[k] as u64);
            }
        }
        t.t(b"\n");
        g.text(t.s());
    }
    super::super::datos::anotar(b"gpu verrano textura", buenas as u64, b"de 96");
    dsk.field.n = 0;
    After::Settle
}

/// `sinldg` salio bien: lo que eso quiere decir.
pub(super) fn sin_ldg_pagado(dsk: &mut Desktop) -> After {
    linea(dsk, b"  SIN LDG la 3060 PAGO los VERTICES y el dibujo: el cuelgue es del LDG (o de la direccion que lee). El cubo sale vacio a proposito", INK_GOOD)
}

/// `v` decimas de us, como `12.3 us`.
pub(super) fn decimas(t: &mut Texto, v: u64) {
    t.d(v / 10).t(b".").d(v % 10).t(b" us");
}

fn motivo(dsk: &mut Desktop, m: u32) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ERR);
    g.text(b"  NO  VERRANO en la 3060 no dibujo: motivo ");
    g.dec(m as u64);
    g.text(b" (`gpu` lo explica)\n");
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}

fn linea(dsk: &mut Desktop, texto: &[u8], tinta: u8) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(tinta);
    g.text(texto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}
