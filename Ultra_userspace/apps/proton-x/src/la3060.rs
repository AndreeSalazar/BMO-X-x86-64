//! **P3b4c: EL EJECUTOR DE LA 3060** (`Plataforma::dibujar`) -- cada lote de
//! D3D12 del `.exe`, a la 3060 por la PUERTA ESTRECHA del kernel
//! (`IOMMU_OP_GPU_DIBUJAR`); y si no se puede, en la CPU con los sombreadores
//! nativos de la casa, DICIENDO por que.
//!
//! La CPU dirige: traduce el PSO una vez (`bmo_proton_x_sm86::puerta`), copia
//! los datos del lote a la receta y llama. La 3060 dibuja en el back buffer
//! de la casa (la RAM de esta app), y el `Present` de siempre lo lleva a la
//! ventana.
//!
//! ```text
//!    la puerta dice NO    se dice UNA vez por motivo (EL REGISTRO), y ese
//!                         lote va por la CPU
//!    el kernel dice NO    tres seguidos: la 3060 se deja para el resto de
//!                         la vida del proceso (sin escritorio que la haya
//!                         preparado no va a cambiar), y se dice
//!    la 3060 no paga      A LA PRIMERA: el canal GR tomo un Xid y esta
//!                         muerto hasta reiniciar; insistir costo 1 s por
//!                         lote en el metal (28-09). Se dice la escalera
//! ```
//!
//! # POR PARTES (29-09)
//!
//! El metal (28-09 22:36): BMOX-12 por la 3060 a 59-90 fps con `dibujar`
//! 10-15 ms, y la 3060 dibujando en ~1 ms. Para saber DONDE se va el resto,
//! una linea por segundo:
//!
//! ```text
//!    [3060] N lotes/s; por lote: puerta P us (la receta, en esta app),
//!           kernel K us = 3060 A + preparar B + sombra C + resto R;
//!           en caliente H de N
//! ```
//!
//! `A` es la espera del semaforo tras el timbre (el dibujo); `B`, escribir
//! ordenes y datos en la VRAM; `C`, las copias de la sombra por el motor de
//! copia; `R`, todo lo demas del syscall: prestar y devolver el back buffer y
//! las texturas por la IOMMU, pegar y juzgar los programas, las tablas. `H`,
//! los lotes que el kernel preparo EN CALIENTE (sin releer la VRAM).
//!
//! El metal (29-09 07:02) lo partio: kernel ~10,3 ms = 3060 1,1 + preparar
//! 5,4 + sombra 1,15 + resto 2,6, y la puerta 1 us. Preparar era la mitad,
//! siempre en frio: desde ese dia el destino tambien va en caliente
//! (`gpu_trabajo/cubo.rs`, `tuberia::huella_fija`).
//!
//! ** Z6 (29-09): con el lote en 247 us, `resto` eran 105 sin saber de quien. Debajo
//! de cada `[3060]` va su reparto, de los contadores del kernel
//! (`INFO_RECETA`):
//!
//! ```text
//!    [resto] por lote: leer L + pegar G + texturas T + paquete Q + devolver D
//!            + en_frio sin la 3060 F + fuera de la receta O us
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use bmo_proton_x::lote::{Lote, NoDibuja};
use bmo_proton_x::trama::{Cuenta, Destino};
use bmo_proton_x_sm86::puerta::{self, Blanco, Puerta};
use bmo_userland as bmo;

struct Estado {
    puerta: Option<Puerta>,
    /// Los motivos ya dichos (cada uno una vez).
    dichos: Vec<String>,
    /// NO seguidos del kernel; a 3, la 3060 se deja.
    negados: u32,
    apagada: bool,
    /// Lotes dibujados por la 3060 (para decir el primero).
    por_la_3060: u64,
    /// Lo medido desde la ultima linea `[3060]` (ver POR PARTES).
    partes: Partes,
    /// P3b4c.9 Z1: ya se le pidio al escritorio la pantalla directa.
    pantalla_pedida: bool,
    /// Z1: un lote fue por la CPU con la pantalla pedida: no se vuelve a
    /// pedir (un fotograma a medias entre la pantalla y la RAM no se muestra).
    sin_pantalla: bool,
}

/// Las sumas de un segundo: lotes y ns/us de cada parte.
#[derive(Clone, Copy, Default)]
struct Partes {
    desde_ns: u64,
    lotes: u64,
    puerta_ns: u64,
    kernel_ns: u64,
    tarjeta_us: u64,
    preparar_us: u64,
    sombra_us: u64,
    /// Los lotes que el kernel preparo EN CALIENTE (sin releer la VRAM).
    calientes: u64,
    /// P3b4c.9 Z1: los lotes que quedaron EN LA PANTALLA (sin bajar a la RAM).
    en_pantalla: u64,
    /// Z6: los contadores de `INFO_RECETA` al empezar el segundo.
    receta: [u64; 8],
}

/// Los contadores de la receta del kernel, por piezas (`INFO_RECETA`).
fn leer_receta() -> [u64; 8] {
    core::array::from_fn(|k| bmo::info(bmo::INFO_RECETA | (k as u64) << 8))
}

impl Partes {
    /// Suma un lote; pasado un segundo, dice la linea y empieza otra.
    fn sumar(&mut self, puerta_ns: u64, kernel_ns: u64, r: u64, ahora: u64) {
        if self.desde_ns == 0 {
            self.desde_ns = ahora;
            self.receta = leer_receta();
        }
        let (us, _, _, _) = puerta::desempaquetar(r);
        self.lotes += 1;
        self.puerta_ns += puerta_ns;
        self.kernel_ns += kernel_ns;
        self.tarjeta_us += us as u64;
        let (caliente, preparar) = puerta::preparado(r);
        self.preparar_us += preparar as u64;
        self.calientes += caliente as u64;
        self.en_pantalla += puerta::a_pantalla(r) as u64;
        self.sombra_us += puerta::copia_us(r) as u64;
        let pasado = ahora.saturating_sub(self.desde_ns);
        if pasado >= 1_000_000_000 {
            let n = self.lotes.max(1);
            let (k, a, b, c) = (
                self.kernel_ns / 1000 / n,
                self.tarjeta_us / n,
                self.preparar_us / n,
                self.sombra_us / n,
            );
            bmo::consola(&alloc::format!(
                "[3060] {} lotes/s; por lote: puerta {} us, kernel {k} us = 3060 {a} + preparar {b} + sombra {c} + resto {}; en caliente {} de {n}, en la pantalla {}\n",
                self.lotes * 1_000_000_000 / pasado.max(1),
                self.puerta_ns / 1000 / n,
                k.saturating_sub(a + b + c),
                self.calientes,
                self.en_pantalla,
            ));
            // ** Z6: EL RESTO, PARTIDO. Lo que el kernel sumo por pieza en
            // este segundo, por lote; `en_frio` sin lo que ya dice la linea
            // de arriba (3060, preparar, sombra), y lo que el syscall costo
            // fuera de la receta (la puerta del kernel, el despacho).
            let ahora_r = leer_receta();
            let d: [u64; 8] = core::array::from_fn(|i| ahora_r[i].wrapping_sub(self.receta[i]));
            let nr = d[0].max(1);
            let us = |i: usize| d[i] / nr / 1000;
            bmo::consola(&alloc::format!(
                "[resto] por lote: leer {} + pegar {} + texturas {} + paquete {} + devolver {} + en_frio sin la 3060 {} + fuera de la receta {} us\n",
                us(1),
                us(2),
                us(3),
                us(4),
                us(6),
                us(5).saturating_sub(a + b + c),
                k.saturating_sub(us(7)),
            ));
            *self = Partes {
                desde_ns: ahora,
                receta: ahora_r,
                ..Partes::default()
            };
        }
    }
}

struct Celda(core::cell::UnsafeCell<Estado>);
// SAFETY: una tarea; los hilos de la casa son cooperativos.
unsafe impl Sync for Celda {}
static ESTADO: Celda = Celda(core::cell::UnsafeCell::new(Estado {
    puerta: None,
    dichos: Vec::new(),
    negados: 0,
    apagada: false,
    por_la_3060: 0,
    partes: Partes {
        desde_ns: 0,
        lotes: 0,
        puerta_ns: 0,
        kernel_ns: 0,
        tarjeta_us: 0,
        preparar_us: 0,
        sombra_us: 0,
        calientes: 0,
        en_pantalla: 0,
        receta: [0; 8],
    },
    pantalla_pedida: false,
    sin_pantalla: false,
}));

/// **P3b4c.9 Z1: soltar la pantalla directa** si se pidio, y no volver a
/// pedirla: lo que venga va por la RAM y lo compone el escritorio. Un lote
/// por la CPU ANTES de pedirla no la cierra (aun no hay nada en la pantalla).
fn soltar_pantalla(e: &mut Estado) {
    if e.pantalla_pedida {
        crate::plataforma::pedir_pantalla(false);
        e.pantalla_pedida = false;
        e.sin_pantalla = true;
        bmo::consola("PROTON-X: la pantalla directa se suelta (Z1): el escritorio vuelve a componer la ventana\n");
    }
}

fn decir(e: &mut Estado, motivo: String) {
    if e.dichos.iter().any(|d| *d == motivo) {
        return;
    }
    bmo::consola("PROTON-X: este lote va por la CPU: ");
    bmo::consola(&motivo);
    bmo::consola("\n");
    e.dichos.push(motivo);
}

/// **Un dibujo que la 3060 no pago**, dicho entero: cuanto espero el kernel,
/// hasta que escalon llego, y lo que se hace.
fn no_pagado(r: u64) -> String {
    let (us, tris, etapas, lanzado) = puerta::desempaquetar(r);
    let si = |b: u32| if etapas & b != 0 { "SI" } else { "NO" };
    let donde = match (lanzado, etapas) {
        (false, _) => "no se lanzo: el kernel no pudo poner la receta en el canal",
        (true, 0) => "no pago NI el estado: el canal GR ya estaba muerto (un Xid anterior; el `gsp aviso` de `gpu verrano` lo dice)",
        (true, 1) => "el estado SI, los vertices NO: se paro en el programa de VERTICES",
        (true, _) => "los vertices SI, el dibujo NO: se paro al RASTERIZAR o en el programa de PIXEL (un Xid 69 = un metodo o un valor que el motor no acepta)",
    };
    alloc::format!(
        "PROTON-X: la 3060 NO PAGO un lote de {tris} triangulo(s): el kernel espero {} ms; escalera: estado {} vertices {} dibujo {} -> {donde}.\n\
         PROTON-X: un canal GR que no paga queda MUERTO hasta reiniciar: la 3060 se deja YA (cada lote mas costaria otro segundo); el resto, por la CPU (contesto {r:#x})\n",
        us / 1000,
        si(1),
        si(2),
        si(4),
    )
}

/// **El ejecutor** (`Plataforma::dibujar`).
pub fn dibujar(l: &Lote, d: &mut Destino) -> Result<Cuenta, NoDibuja> {
    // SAFETY: ver `Celda`.
    let e = unsafe { &mut *ESTADO.0.get() };
    // N5.12: un lote de solo profundidad (sin pixeles) no tiene back buffer
    // que darle a la puerta: por la CPU.
    // N5.16 (05-10): un render target de FLOAT (HDR) tampoco: la puerta
    // escribe 8 bits por canal, y el lote iria a parar a un sitio que no
    // tiene esa forma. Por la CPU, y dicho UNA vez.
    if !e.apagada && d.flotante.is_some() {
        decir(e, String::from("un render target de float (HDR, N5.16): la puerta de la 3060 solo sabe BGRA8 y RGBA8"));
    }
    if !e.apagada && !d.pixeles.is_empty() && d.flotante.is_none() {
        let blanco = Blanco {
            va: d.pixeles.as_ptr() as u64,
            ancho: d.ancho,
            alto: d.alto,
            bgra: d.bgra,
            cadena: d.cadena,
        };
        let p = e.puerta.get_or_insert_with(Puerta::nueva);
        let t0 = crate::plataforma::ahora_ns();
        match p.preparar(l, blanco) {
            Ok(_) => {
                let caja = p.caja.as_ptr() as u64;
                let t1 = crate::plataforma::ahora_ns();
                let r = bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_DIBUJAR, caja);
                let t2 = crate::plataforma::ahora_ns();
                match r {
                    Ok(r) if puerta::sano(r) => {
                        e.partes.sumar(t1 - t0, t2 - t1, r, t2);
                        p.despues(l, true);
                        e.negados = 0;
                        if e.por_la_3060 == 0 {
                            bmo::consola("PROTON-X: la 3060 dibuja los lotes (P3b4c: la puerta estrecha, la receta VRN2)\n");
                        }
                        e.por_la_3060 += 1;
                        // ** P3b4c.9 Z1: con el back buffer de la cadena ya por
                        // la 3060, se le pide al escritorio la pantalla directa;
                        // si la da, el kernel dibuja alli y el `Ok` lo dice.
                        if d.cadena
                            && !e.pantalla_pedida
                            && !e.sin_pantalla
                            && (d.ancho, d.alto) == (1280, 720)
                        {
                            crate::plataforma::pedir_pantalla(true);
                            e.pantalla_pedida = true;
                            bmo::consola("PROTON-X: pido la pantalla directa (Z1): si el escritorio la da, el fotograma no sale de la VRAM\n");
                        }
                        let (_, tris, _, _) = puerta::desempaquetar(r);
                        return Ok(Cuenta {
                            dibujados: tris,
                            en_pantalla: puerta::a_pantalla(r),
                            ..Cuenta::default()
                        });
                    }
                    Ok(r) => {
                        // El metal (28-09): cada NO pagado costo 1 s entero
                        // (el tope del kernel) y 0 fps. Un canal GR que no
                        // paga tomo una excepcion (Xid): esta MUERTO hasta
                        // reiniciar y cada lote mas esperaria otro segundo.
                        // Se deja a la PRIMERA.
                        p.despues(l, false);
                        e.apagada = true;
                        bmo::consola(&no_pagado(r));
                    }
                    Err(m) if m == bmo::IOMMU_NO_CANAL_MUERTO => {
                        p.despues(l, false);
                        e.apagada = true;
                        bmo::consola("PROTON-X: el kernel dice que el canal de GR de la 3060 esta MUERTO (tomo un Xid; el numero, en `cabina`): la 3060 se deja YA; el resto, por la CPU. Reinicia para recuperarla\n");
                    }
                    Err(m) => {
                        p.despues(l, false);
                        e.negados += 1;
                        decir(e, alloc::format!("el kernel dijo que no a la receta (motivo {m}; el porque, en `cabina fallos`)"));
                        if e.negados >= 3 {
                            e.apagada = true;
                            bmo::consola("PROTON-X: tres NO seguidos del kernel: la 3060 se deja; el resto, por la CPU\n");
                        }
                    }
                }
            }
            Err(motivo) => {
                p.despues(l, false);
                decir(e, motivo);
            }
        }
    }
    // Z1: un lote por la CPU con la pantalla pedida -- lo de la 3060 esta en
    // la pantalla y esto en la RAM: la pantalla se suelta, y lo compone el
    // escritorio.
    if d.cadena {
        soltar_pantalla(e);
    }
    bmo_proton_x_casa::nativo::dibujar(l, d)
}
