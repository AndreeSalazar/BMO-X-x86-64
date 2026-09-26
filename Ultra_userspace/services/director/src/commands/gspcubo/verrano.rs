//! **`gpu verrano [fotograma]`: VERRANO V0 y V1** -- el cubo del estudio D3D
//! por la API de dibujo de BMO-X, con sus DOS backends y el mismo fotograma:
//!
//! ```text
//!    el aparato  lo que haya detras de la puerta `destino` (hoy la RTX 3060
//!                12G: `sm86.rs`). Este fichero NO la nombra: habla VERRANO
//!    la CPU      `bmo_verrano::cpu::Cpu::LA_3060`: el juez con la regla 4
//! ```
//!
//! Se comparan pixel a pixel, y la huella del aparato contra D3D12 (0, 30,
//! 60) y contra lo medido bajo BMO-X (32). Lo que se ve en la ventana es lo
//! que escribio el aparato, leido de la pantalla.
//!
//! ** AISLADO (26-09, `docs/plan/PLAN_EL_AISLAMIENTO.md` A2): aqui no entra
//! ni el driver de la 3060, ni `kind::SM86`, ni su juez. Lo vigila `la-3060`
//! (regla S): otra GPU es otro `destino`, y este fichero no cambia.
//!
//! [consumo] NADA      corre cuando el propietario lo teclea

use bmo_cubo::referencia as rf;
use bmo_userland as bmo;
use bmo_verrano::cpu::Cpu;
use bmo_verrano::lamina::{Lamina, Leido};
use bmo_verrano::{Backend, Frame, Image, Vertex, Viewport};

use super::super::After;
use super::sm86 as destino;
use super::tablero::Tablero;
use super::{numero, Texto, CLARO, FONDO, ROJO, TENUE, VERDE};
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

/// Los vertices que caben en un fotograma de la escena (3 por triangulo).
const MAX_VERTICES: usize = 3 * bmo_cubo::tanda::CABEN;

/// `gpu verrano [fotograma] | banco [N] [opciones del aparato]`.
pub(crate) fn orden(dsk: &mut Desktop, p: &bmo::Pantalla, resto: &[u8]) -> After {
    // [!] `gpu.rs` pasa el resto CON su espacio delante (" sinldg"): sin
    // recortarlo, `sinldg` no se reconocia nunca y el 26-09 06:33 corrio el
    // programa normal con ese nombre. Se recorta aqui, una vez, para todos.
    let resto = resto.trim_ascii();
    let f = numero(resto).unwrap_or(30).min(359);
    let banco_pedido = resto.strip_prefix(b"banco");
    // Lo que no es de VERRANO (`ligero`, `sinldg`...) lo entiende el aparato.
    let op = destino::Opciones::de(resto);
    paint_status(p, &dsk.run_box, "VERRANO: el cubo por la API de BMO-X, en el aparato y en la CPU", INK_DIM);
    let (w, h) = (rf::ANCHO, rf::ALTO);
    if p.ancho < w || p.alto < h {
        return linea(dsk, b"  NO  la pantalla es mas chica que 1280x720", INK_ERR);
    }
    let n = (w * h) as usize;
    let (Some(bloque), Some(caja)) = (bmo::Memoria::request(2 * n as u64 * 4), bmo::Memoria::request(destino::CAJA)) else {
        return linea(dsk, b"  NO  sin memoria para dos fotogramas de 1280x720", INK_ERR);
    };
    // SAFETY: el bloque mide 2n palabras de 32 bits, alineado a pagina, es de
    // este proceso y solo se usa aqui; las dos mitades no se pisan. La caja,
    // `destino::CAJA` bytes del proceso, para lo que el aparato arme.
    let (gpu, cpu) = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut u32, 2 * n).split_at_mut(n) };
    // SAFETY: como arriba.
    let caja = unsafe { core::slice::from_raw_parts_mut(caja.base() as *mut u8, destino::CAJA as usize) };

    // El fotograma: la tanda del juez en vertices de VERRANO.
    let mut v = [Vertex::default(); MAX_VERTICES];
    let Some(k) = vertices(f, w, h, &mut v) else { return linea(dsk, b"  NO  la tanda no cabe", INK_ERR) };
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &v[..k], viewport: Viewport { width: w, height: h } };

    // La puerta: el sobre con el codigo de ESTE aparato, su juez, la tarjeta.
    let (mut aparato, abierto) = match destino::abrir(dsk, p, caja, op) {
        Ok(x) => x,
        Err(a) => return a,
    };

    // La pantalla ANTES del dibujo, como `gpu cubo 3060`.
    let (x0, y0) = destino::ventana(p);
    p.rect(0, 0, p.ancho, p.alto, FONDO);
    let titulo = if banco_pedido.is_some() { "VERRANO V1: EL CUBO GIRANDO" } else { "VERRANO V0: EL CUBO POR LA API DE BMO-X" };
    p.texto_escala(40, 24, titulo, CLARO, 2);
    p.vaciar();

    // ** `gpu verrano banco [N]` (V1): N fotogramas seguidos, sin leer.
    if let Some(r) = banco_pedido {
        let n = r.split(|&c| c == b' ').find_map(numero).unwrap_or(360).clamp(1, 3600);
        aparato.leer = false;
        // `inti`: los vertices NO los cuenta el escritorio: los lee de la
        // LAMINA que ofrecio una app (INTI), fotograma a fotograma.
        let inti = r.split(|&c| c == b' ').any(|w| w == b"inti");
        return banco(dsk, p, aparato, op, gpu, n, y0 + h, inti);
    }
    let s_aparato = aparato.draw(&frame, &mut Image { pixels: gpu, width: w, height: h });
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let desde = bmo::ciclos();
    let mut juez = Cpu::LA_3060;
    let scpu = juez.draw(&frame, &mut Image { pixels: cpu, width: w, height: h });
    let cpu_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    let st = match (s_aparato, scpu) {
        (Ok(a), Ok(_)) => a,
        (Err(e), _) => return destino::fallo(dsk, e, None, op),
        (_, Err(_)) => return linea(dsk, b"  NO  la CPU no dibujo el fotograma (el juez no sabe hacerlo)", INK_ERR),
    };

    if op.sin_ldg {
        return destino::sin_ldg_pagado(dsk);
    }

    // Las dos imagenes, pixel a pixel; lo que el juez no explica, aparte.
    let distinto = |i: usize| gpu[i] & 0x00FF_FFFF != cpu[i] & 0x00FF_FFFF;
    let sabido = |i: usize| rf::SIN_EXPLICAR.iter().any(|s| s.0 == f && (s.2 * w + s.1) as usize == i);
    let malos = (0..n).filter(|&i| distinto(i) && !sabido(i)).count();
    let sin_explicar = (0..n).filter(|&i| distinto(i) && sabido(i)).count();
    let primero = (0..n).find(|&i| distinto(i) && !sabido(i));
    let huella = rf::huella(gpu);
    let d3d = rf::de_la_3060(f) == Some(huella);
    let medida = rf::HUELLAS_BMO_X.iter().any(|&(g, hh)| g == f && hh == huella);

    p.marcar(x0, y0, w, h);
    for y in 0..h {
        for x in 0..w {
            p.punto_ya_marcado(x0 + x, y0 + y, gpu[(y * w + x) as usize] & 0x00FF_FFFF);
        }
    }
    let mut a = Texto::nuevo();
    a.t(b"fotograma ").d(f as u64).t(b": el aparato en ").d(st.device_us as u64).t(b" us, la CPU en ").d(cpu_us).t(b" us; huella ").x(huella);
    let mut b = Texto::nuevo();
    let (veredicto, color): (&[u8], u32) = if malos > 0 {
        b.t(b"DISTINTO: ").d(malos as u64).t(b" pixeles entre el aparato y la CPU");
        (b.s(), ROJO)
    } else if d3d {
        (b"IGUAL: VERRANO en el aparato = VERRANO en la CPU = D3D12 en la 3060 bajo Windows", VERDE)
    } else if medida {
        (b"IGUAL: VERRANO en el aparato = lo que la 3060 dibujo en X5, y = la CPU", VERDE)
    } else {
        (b"IGUAL, pixel a pixel: VERRANO en el aparato = VERRANO en la CPU", VERDE)
    };
    let mut c = Texto::nuevo();
    if let Some(i) = primero {
        c.t(b"el primero en (").d((i as u32 % w) as u64).t(b", ").d((i as u32 / w) as u64).t(b"): el aparato ").x(gpu[i] as u64 & 0xFF_FFFF).t(b", la CPU ").x(cpu[i] as u64 & 0xFF_FFFF);
    } else if sin_explicar > 0 {
        c.t(b"y ").d(sin_explicar as u64).t(b" pixel sin explicar (del silicio, como con X5 y en Windows)");
    }
    let mut d = Texto::nuevo();
    d.t(destino::NOMBRE).t(b": programas del BSF ").d(abierto.bytes_vs as u64).t(b" + ").d(abierto.bytes_ps as u64).t(b" B; ").d(st.triangles as u64).t(b" triangulos en UN dibujo; juez: PERFECTO Y PRECISO (").d(abierto.instrucciones as u64).t(b")");
    let yt = p.alto.saturating_sub(144);
    p.texto_bytes(40, yt, a.s(), CLARO);
    p.texto_bytes(40, yt + 24, veredicto, color);
    if !c.s().is_empty() {
        p.texto_bytes(40, yt + 48, c.s(), TENUE);
    }
    p.texto_bytes(40, yt + 72, d.s(), TENUE);
    p.texto_bytes(40, p.alto.saturating_sub(40), b"pulsa cualquier tecla para volver al escritorio", TENUE);
    p.vaciar();
    super::super::gspcomputo::abrir_panel();

    let g = &mut dsk.out.grid;
    g.with_ink(if malos == 0 { INK_GOOD } else { INK_ERR });
    g.text(b"  verrano: ");
    g.text(a.s());
    g.text(b"\n           ");
    g.text(veredicto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    for extra in [c.s(), d.s()] {
        if !extra.is_empty() {
            g.text(b"           ");
            g.text(extra);
            g.byte(b'\n');
        }
    }
    g.text(b"           leido de la pantalla en ");
    g.dec(aparato.leer_ms);
    g.text(b" ms\n");
    super::super::datos::anotar(b"gpu verrano us", st.device_us as u64, b"");
    dsk.field.n = 0;
    After::Settle
}

/// Los vertices de VERRANO del fotograma `f` (la tanda del juez).
fn vertices(f: u32, w: u32, h: u32, v: &mut [Vertex; MAX_VERTICES]) -> Option<usize> {
    let t = bmo_cubo::tanda::de_fotograma(f, w, h)?;
    let mut k = 0;
    for tri in t.tris() {
        for &pos in tri.clip.iter() {
            v[k] = Vertex { position: pos, color: tri.color };
            k += 1;
        }
    }
    Some(k)
}

/// ** V1 -- EL CUBO EN MOVIMIENTO, CON FPS (`gpu verrano banco [N]`).
///
/// N fotogramas seguidos por VERRANO (el cubo gira: fotograma `i` = angulo
/// `i` de 360), cada uno dibujado por el aparato directo en su ventana de la
/// pantalla -- se VE girar --, SIN leerlo de vuelta: leer 1280x720 por la
/// puerta cuesta ~2 s y es justo lo que el banco no mide. Al final, UNO se
/// lee y se juzga (el 30, contra la huella de D3D12): unos fps que dibujan
/// otra cosa no valen nada.
///
/// Lo que se mide, dicho: `pared` es todo (preparar el paquete, la puerta,
/// subir los vertices y esperar el semaforo); `aparato` es solo lo
/// que la tarjeta tardo en dibujar. La tabla de `estudio-d3d` (D3D12 ~3.800
/// fps en Windows) mide un bucle de presentacion, no esto: se ponen al lado,
/// no se igualan.
///
/// ** Y se ve MIENTRAS (26-09): el tablero (`tablero.rs`) debajo del cubo,
/// repintado cada 66 ms, con lo que cuesta el propio tablero descontado de
/// los fps y dicho aparte. `preparar` es lo que el kernel tarda en dejar el
/// fotograma listo en la VRAM: en frio relee ~4.000 palabras por PCIe, en
/// caliente (del segundo fotograma en adelante) solo escribe los vertices.
/// Las opciones del aparato (en la 3060, `ligero`: sin la escalera de T1c;
/// `anillo`: el fotograma EN VUELO) las entiende el aparato, no este
/// fichero.
///
/// ** V1b, "la CPU no espera: ORQUESTA" (26-09). Dos cosas de aqui:
///
/// ```text
///    las tandas   los vertices de los 360 angulos se cuentan ANTES, una
///                 vez: en el escritorio la coma flotante es por software
///                 (decenas de us por fotograma), y un juego trae su
///                 animacion hecha. El banco mide ORQUESTAR y dibujar, no
///                 la trigonometria; lo que costo contarlas se dice aparte
///    el cierre    con un aparato que deja fotogramas EN VUELO, lo que
///                 quede se espera (`Backend::finish`) DENTRO del reloj:
///                 unos fps que no esperan al ultimo fotograma no valen
/// ```
fn banco(dsk: &mut Desktop, p: &bmo::Pantalla, mut aparato: destino::Aparato, op: destino::Opciones, gpu: &mut [u32], n: u32, fin_y: u32, inti: bool) -> After {
    let (w, h) = (rf::ANCHO, rf::ALTO);
    // ** `inti` (26-09): la LAMINA que ofrecio la app. Se abre UNA vez, y su
    // cabecera no se cree (`Lamina::abrir`).
    let tomada = if inti {
        match dsk.table.lamina() {
            Some(t) => Some(t),
            None => return linea(dsk, b"  NO  nadie ofrecio una lamina de VERRANO: lanza antes la app de INTI que la publica (run inti/cubo.ibx)", INK_ERR),
        }
    } else {
        None
    };
    // SAFETY: `tomar_prestado_de` mapeo `bytes` desde `base` en este proceso
    // y la lamina no se suelta mientras dura el banco (solo `reap_dead`, que
    // corre en el bucle del escritorio, no aqui). Se ve como palabras
    // ATOMICAS: la app las escribe a la vez, y asi lo dice el tipo.
    let palabras = tomada.map(|t| unsafe { core::slice::from_raw_parts(t.base as *const core::sync::atomic::AtomicU32, (t.bytes / 4) as usize) });
    let lamina = match palabras.map(Lamina::abrir) {
        None => None,
        Some(Ok(l)) => Some(l),
        Some(Err(_)) => return linea(dsk, b"  NO  la lamina ofrecida no se sostiene (magia, version o capacidad que no cabe en lo prestado): no se dibuja", INK_ERR),
    };
    let mut de_inti = DeInti { tid: tomada.map_or(0, |t| t.tid), ..DeInti::default() };
    let mut vi = [Vertex::default(); MAX_VERTICES];
    let modo = op.modo();
    let mut tablero = Tablero::nuevo(p, fin_y, n, destino::ETIQUETA, modo, b"PERFECTO Y PRECISO");
    // Sin banda (pantalla de 1280x720), las mismas cuentas sin pintarlas.
    let mut cuentas = Cuentas::default();
    let mut v = [Vertex::default(); MAX_VERTICES];
    // Las tandas, antes (ver arriba).
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1000);
    let angulos = n.min(360) as usize;
    let Some(bloque) = bmo::Memoria::request((angulos * MAX_VERTICES * core::mem::size_of::<Vertex>()) as u64) else {
        return linea(dsk, b"  NO  sin memoria para las tandas del banco", INK_ERR);
    };
    // SAFETY: el bloque mide `angulos * MAX_VERTICES` vertices (`repr(C)`,
    // de f32: cualquier bit vale), alineado a pagina, es de este proceso y
    // solo se usa aqui; vive hasta el final de esta funcion.
    let tandas = unsafe { core::slice::from_raw_parts_mut(bloque.base() as *mut Vertex, angulos * MAX_VERTICES) };
    let mut cuantos = [0u8; 360];
    let desde = bmo::ciclos();
    for (f, (t, c)) in tandas.chunks_exact_mut(MAX_VERTICES).zip(cuantos.iter_mut()).enumerate() {
        let Some(k) = vertices(f as u32, w, h, &mut v) else { return linea(dsk, b"  NO  la tanda no cabe", INK_ERR) };
        t.copy_from_slice(&v);
        *c = k as u8;
    }
    let tandas_us = (bmo::ciclos() - desde) * 1_000_000 / hz;
    // `exige`: la tarjeta al maximo ANTES del reloj del banco.
    let mut exigido = Texto::nuevo();
    if op.exige {
        aparato.exigir(&mut exigido);
    }
    let bucle = bmo::ciclos();
    let mut dentro = 0u64;
    // Las vueltas hechas: `n`, salvo que la app de la lamina se calle antes.
    let mut hechos = n;
    for i in 0..n {
        let f = (i % 360) as usize;
        let k = cuantos[f] as usize;
        // ** Con lamina, AL RITMO DE LA APP (26-09). Cada vuelta dibuja un
        // fotograma NUEVO de la app: si lo ultimo publicado ya se dibujo, se
        // duerme 1 ms y se vuelve a mirar. La primera version dibujaba lo
        // ultimo sin esperar, a ~28.000 vueltas por segundo contra los 60
        // de la app: 360 vueltas en 13 ms, el mismo fotograma 470 veces y el
        // giro sin verse. Asi N son N fotogramas de la app (360 = una vuelta
        // entera, 6 s), y si la app deja de publicar 2 s, el banco acaba.
        let de_la_app: Option<&[Vertex]> = match lamina.as_ref() {
            None => None,
            Some(l) => {
                let limite = bmo::ciclos() + 2 * hz;
                loop {
                    match l.leer(&mut vi) {
                        Leido::Fotograma { fotograma, vertices } if !de_inti.visto || fotograma != de_inti.fotograma => {
                            de_inti.nuevo(fotograma, &vi[..vertices]);
                            break;
                        }
                        Leido::Fotograma { .. } | Leido::Nada => de_inti.esperando += 1,
                        // Pillada a medio escribir: la siguiente mirada la vera entera.
                        Leido::Rota => de_inti.rotos += 1,
                        Leido::Mentira => return linea(dsk, b"  NO  la lamina dice un numero de vertices imposible (mas que su capacidad o no triangulos enteros): la app miente, se para", INK_ERR),
                    }
                    if bmo::ciclos() > limite {
                        break;
                    }
                    bmo::wait(0, 0, 1_000_000);
                }
                if bmo::ciclos() > limite {
                    de_inti.callada = true;
                    hechos = i;
                    break;
                }
                Some(&de_inti.v[..de_inti.n])
            }
        };
        let vertices = de_la_app.unwrap_or(&tandas[f * MAX_VERTICES..][..k]);
        let frame = Frame { clear: bmo_cubo::FONDO_F, vertices, viewport: Viewport { width: w, height: h } };
        let desde = bmo::ciclos();
        match aparato.draw(&frame, &mut Image { pixels: gpu, width: w, height: h }) {
            Ok(st) => {
                let ciclos = bmo::ciclos() - desde;
                dentro += ciclos;
                cuentas.apuntar(ciclos, &st);
                if let Some(t) = tablero.as_mut() {
                    t.apuntar(ciclos, &st);
                    t.quizas(p);
                }
            }
            Err(e) => return destino::fallo(dsk, e, Some(i), op),
        }
    }
    let bucle = bmo::ciclos() - bucle;
    // ** VERRANO ENTIENDE A INTI: el ultimo fotograma que la app publico se
    // compara, vertice a vertice y bit a bit, con el del juez de la CPU.
    let mut juicio_inti = Texto::nuevo();
    if lamina.is_some() {
        de_inti.juzgar(w, h, &mut juicio_inti);
    }
    // El cierre: lo que quedo en vuelo, dentro del reloj.
    let desde = bmo::ciclos();
    let cierre_us = match aparato.finish() {
        Ok(us) => us,
        Err(e) => return destino::fallo(dsk, e, Some(hechos), op),
    };
    let cierre = bmo::ciclos() - desde;
    cuentas.ciclos += cierre;
    if let Some(t) = tablero.as_mut() {
        t.cierre(cierre);
    }
    // El juicio: el 30 otra vez, por el MISMO camino (con lo que el aparato
    // reuse y en el mismo modo), leido y comparado con D3D12.
    aparato.leer = true;
    let k = vertices(30, w, h, &mut v).unwrap_or(0);
    let frame = Frame { clear: bmo_cubo::FONDO_F, vertices: &v[..k], viewport: Viewport { width: w, height: h } };
    let igual = aparato.draw(&frame, &mut Image { pixels: gpu, width: w, height: h }).is_ok() && rf::de_la_3060(30) == Some(rf::huella(gpu));
    let veredicto: &[u8] = if igual {
        b"el fotograma 30, leido al final: IGUAL a D3D12 en la 3060 bajo Windows"
    } else {
        b"PERO el fotograma 30, leido al final, NO es el de D3D12: esos fps no valen"
    };

    let (mut a, mut b) = (Texto::nuevo(), Texto::nuevo());
    let (fps, tarjeta) = match tablero.as_mut() {
        Some(t) => {
            t.final_(p, veredicto, igual);
            t.resumen(&mut a, &mut b);
            (t.fps(), t.tarjeta_media())
        }
        None => {
            cuentas.resumen(&mut a, &mut b, modo);
            let yt = p.alto.saturating_sub(120);
            p.texto_bytes(40, yt, a.s(), CLARO);
            p.texto_bytes(40, yt + 24, b.s(), TENUE);
            p.texto_bytes(40, yt + 48, veredicto, if igual { VERDE } else { ROJO });
            p.texto_bytes(40, p.alto.saturating_sub(40), b"pulsa cualquier tecla para volver al escritorio", TENUE);
            p.vaciar();
            (cuentas.fps(), cuentas.tarjeta / cuentas.muestras.max(1) as u64)
        }
    };
    super::super::gspcomputo::abrir_panel();
    let g = &mut dsk.out.grid;
    g.with_ink(if igual { INK_GOOD } else { INK_ERR });
    g.text(b"  verrano ");
    g.text(a.s());
    g.text(b"\n           ");
    g.text(b.s());
    g.text(b"\n           ");
    g.text(veredicto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    let mut c = Texto::nuevo();
    c.t(b"tandas de vertices contadas ANTES: ").d(angulos as u64).t(b" angulos en ").d(tandas_us).t(b" us (fuera del reloj); el cierre, ").d(cierre_us as u64).t(b" us esperando lo que quedo en vuelo (dentro)");
    g.text(b"           ");
    g.text(c.s());
    g.byte(b'\n');
    if !exigido.s().is_empty() {
        g.text(b"           ");
        g.text(exigido.s());
        g.byte(b'\n');
    }
    let mut gobierno = Texto::nuevo();
    aparato.cerrar(&mut gobierno);
    if !gobierno.s().is_empty() {
        g.text(b"           ");
        g.text(gobierno.s());
        g.byte(b'\n');
    }
    if !juicio_inti.s().is_empty() {
        g.with_ink(if de_inti.igual { INK_GOOD } else { INK_ERR });
        g.text(b"           ");
        g.text(juicio_inti.s());
        g.byte(b'\n');
        g.with_ink(INK_PLAIN);
    }
    let mut nota = Texto::nuevo();
    aparato.nota(&mut nota);
    if !nota.s().is_empty() {
        g.text(b"           ");
        g.text(nota.s());
        g.byte(b'\n');
    }
    // ** E2: de que es la pared. El bucle entero = lo de dentro de `draw`
    // (partido por la puerta, abajo) + el tablero + lo demas del bucle.
    let tablero_ciclos = tablero.as_ref().map_or(0, |t| t.pintar_ciclos());
    let por = |c: u64| c * 10_000_000 / hz / hechos.max(1) as u64;
    let mut e2 = Texto::nuevo();
    e2.t(b"E2, la pared del bucle: ");
    destino::decimas(&mut e2, por(bucle));
    e2.t(b" por fotograma = draw ");
    destino::decimas(&mut e2, por(dentro));
    e2.t(b" + tablero ");
    destino::decimas(&mut e2, por(tablero_ciclos));
    e2.t(b" + resto ");
    destino::decimas(&mut e2, por(bucle.saturating_sub(dentro + tablero_ciclos)));
    let mut e2b = Texto::nuevo();
    aparato.nota_fases(&mut e2b);
    for linea_e2 in [e2.s(), e2b.s()] {
        if !linea_e2.is_empty() {
            g.text(b"           ");
            g.text(linea_e2);
            g.byte(b'\n');
        }
    }
    if let Some((c, pq, pu, pr)) = aparato.fases() {
        for (clave, v) in [(b"gpu verrano e2 cuentas" as &[u8], c), (b"gpu verrano e2 paquete", pq), (b"gpu verrano e2 puerta", pu), (b"gpu verrano e2 preparar", pr), (b"gpu verrano e2 pared", por(bucle))] {
            super::super::datos::anotar(clave, v, b"decimas de us");
        }
    }
    // Con lamina el ritmo es el de la app (~60), no el del aparato: va aparte.
    let (kf, ku): (&[u8], &[u8]) = if lamina.is_some() {
        (b"gpu verrano banco inti fps", b"gpu verrano banco inti us")
    } else if op.exige && op.coopera {
        (b"gpu verrano banco maximo fps", b"gpu verrano banco maximo us")
    } else if op.coopera {
        (b"gpu verrano banco coopera fps", b"gpu verrano banco coopera us")
    } else if op.anillo {
        (b"gpu verrano banco anillo fps", b"gpu verrano banco anillo us")
    } else if op.ligero {
        (b"gpu verrano banco ligero fps", b"gpu verrano banco ligero us")
    } else {
        (b"gpu verrano banco fps", b"gpu verrano banco us")
    };
    super::super::datos::anotar(kf, fps, b"fps");
    super::super::datos::anotar(ku, tarjeta, b"us");
    dsk.field.n = 0;
    After::Settle
}

/// ** Lo que VERRANO recibio de la app por la lamina, y su juicio.
struct DeInti {
    /// El ultimo fotograma entero, para repetirlo si el siguiente se rompe.
    v: [Vertex; MAX_VERTICES],
    n: usize,
    fotograma: u32,
    visto: bool,
    /// Fotogramas DISTINTOS recibidos, repetidos por rotos, vueltas sin nada.
    distintos: u32,
    rotos: u32,
    esperando: u32,
    igual: bool,
    /// De que app es la lamina.
    tid: u32,
    /// La app dejo de publicar (2 s sin un fotograma nuevo) y el banco acabo antes.
    callada: bool,
}

impl Default for DeInti {
    fn default() -> Self {
        DeInti { v: [Vertex::default(); MAX_VERTICES], n: 0, fotograma: 0, visto: false, distintos: 0, rotos: 0, esperando: 0, igual: false, tid: 0, callada: false }
    }
}

impl DeInti {
    fn nuevo(&mut self, fotograma: u32, v: &[Vertex]) {
        if !self.visto || fotograma != self.fotograma {
            self.distintos += 1;
        }
        self.v[..v.len()].copy_from_slice(v);
        self.n = v.len();
        self.fotograma = fotograma;
        self.visto = true;
    }

    /// El ultimo de la app contra el juez de la CPU, bit a bit.
    fn juzgar(&mut self, w: u32, h: u32, t: &mut Texto) {
        t.t(b"INTI (tid ").d(self.tid as u64).t(b") por la lamina: ").d(self.distintos as u64).t(b" fotogramas distintos, ").d(self.rotos as u64).t(b" repetidos por rotos, ").d(self.esperando as u64).t(b" vueltas sin nada");
        if !self.visto {
            t.t(b"; la app no publico NINGUNO");
            return;
        }
        let mut juez = [Vertex::default(); MAX_VERTICES];
        let k = vertices(self.fotograma % 360, w, h, &mut juez).unwrap_or(usize::MAX);
        let bits = |v: &Vertex| (v.position.map(f32::to_bits), v.color.map(f32::to_bits));
        self.igual = k == self.n && self.v[..self.n].iter().zip(&juez[..self.n]).all(|(a, b)| bits(a) == bits(b));
        t.t(b"; el ultimo, el ").d(self.fotograma as u64).t(if self.igual { b": IGUAL al juez, bit a bit" } else { b": DISTINTO del juez" });
    }
}

/// Las cuentas del banco cuando no hay banda para el tablero.
#[derive(Default)]
struct Cuentas {
    n: u32,
    ciclos: u64,
    tarjeta: u64,
    /// Las muestras del aparato (en vuelo, no cada fotograma trae una).
    muestras: u32,
    peor: u32,
}

impl Cuentas {
    fn apuntar(&mut self, ciclos: u64, st: &bmo_verrano::Stats) {
        self.n += 1;
        self.ciclos += ciclos;
        if st.device_us > 0 || !st.in_flight {
            self.muestras += 1;
            self.tarjeta += st.device_us as u64;
            self.peor = self.peor.max(st.device_us);
        }
    }

    fn fps(&self) -> u64 {
        self.n as u64 * bmo::info(bmo::INFO_TSC_HZ).max(1000) / self.ciclos.max(1)
    }

    fn resumen(&self, a: &mut Texto, b: &mut Texto, modo: &[u8]) {
        a.t(b"banco: ").d(self.n as u64).t(b" fotogramas = ").d(self.fps()).t(b" fps de pared (").t(modo).t(b")");
        b.t(b"el aparato ").d(self.tarjeta / self.muestras.max(1) as u64).t(b" us de media, el peor ").d(self.peor as u64).t(b" us");
    }
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
