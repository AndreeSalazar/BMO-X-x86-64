//! El log del kernel, la memoria que se pide, y las salidas de texto.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

pub fn klog_texto(n: u64, dst: &mut [u8]) -> usize {
    let mut escritos = 0usize;
    let mut trozo = 0u64;
    while escritos < dst.len() {
        let w = invoke(CURRENT_TASK, OP_KLOG_TEXTO, n, trozo, 0).value;
        if w == 0 {
            break;
        }
        for k in 0..8 {
            let b = ((w >> (k * 8)) & 0xFF) as u8;
            if b == 0 || escritos >= dst.len() {
                return escritos;
            }
            dst[escritos] = b;
            escritos += 1;
        }
        trozo += 1;
    }
    escritos
}

/// **Un bloque de memoria pedido al kernel.**
///
/// * Esto NO es un `malloc` y no lo pretende. Es memoria entregada entera:
/// pides una vez, te dan un bloque contiguo, y **se devuelve entero** -- solo
/// (`request`, al salir del alcance) o nunca (`residente`, y hay que decirlo).
///
/// El asignador se escribe ENCIMA, aqui en Ring 3, con la politica que quiera
/// cada uno. Esa es la razon de que el kernel no traiga uno: un `malloc`
/// general dentro del kernel seria escribir una politica que el programa de al
/// lado no usa, y encima cobrarsela con un syscall por llamada.
///
/// El caso que lo decidio: DOOM pide ~8 MiB una vez al arrancar y se los
/// administra el con su `Z_Zone`. Para eso, esto es exactamente lo que hace
/// falta y ni un byte mas.
pub struct Memoria {
    cap: u64,
    base: u64,
    bytes: u64,
    /// `true` = vive lo que viva el proceso y `Drop` no lo toca. Se pide con
    /// [`Memoria::residente`], que es la forma de DECIRLO.
    residente: bool,
}

impl Memoria {
    /// **Pide `bytes` PRESTADOS: vuelven solos al salir del alcance.**
    ///
    /// `None` si no hay RAM contigua, si pasa del tope por peticion (64 MiB) o
    /// si este proceso ya gasto sus ocho peticiones.
    ///
    /// *** LA VIDA UTIL SE DECLARA AL PEDIR (2026-09-21, `PLAN_LA_VIDA_UTIL`
    /// 2a). Hasta hoy todo bloque era residente POR ACCIDENTE: esto no tenia
    /// `Drop`, asi que dejar caer un `Memoria` era fugarlo, y el fichero de
    /// `fondo.rs` decia "se suelta al acabar" sobre un valor que solo se caia.
    /// El propietario lo pidio con estas palabras: *"liberar la memoria cuando ya
    /// entra pero tiene que salir, en tiempo real"*. Eso no es un recolector:
    /// es propiedad. Quien pide ya sabe cuando sale, y el compilador tambien.
    ///
    /// ```text
    ///    request(bytes)     PRESTADA    Drop -> MEM_OP_SOLTAR. Sale, vuelve
    ///    residente(bytes)   PERMANENTE  sin Drop. Se dice, y se cuenta
    /// ```
    pub fn request(bytes: u64) -> Option<Self> {
        Self::pedir(bytes, false)
    }

    /// **Pide `bytes` PARA SIEMPRE: vive lo que viva el proceso.**
    ///
    /// Es lo que necesita `Pantalla::activar_doble_bufer`: pide ~8 MB, se
    /// queda con la direccion y deja caer el `Memoria`, porque ese lienzo se
    /// usa hasta el ultimo fotograma. Con `request` se quedaria sin lienzo en
    /// la linea siguiente. La diferencia entre las dos no es una excepcion
    /// vergonzante: es la vida util, dicha donde se pide.
    pub fn residente(bytes: u64) -> Option<Self> {
        Self::pedir(bytes, true)
    }

    fn pedir(bytes: u64, residente: bool) -> Option<Self> {
        let cap = invoke(CURRENT_TASK, OP_MEMORIA_PEDIR, bytes, 0, 0).valor()?;
        let base = invoke(cap, MEM_OP_BASE, 0, 0, 0).valor()?;
        Some(Self { cap, base, bytes, residente })
    }

    /// Se pidio con [`Memoria::residente`]?
    pub fn es_residente(&self) -> bool {
        self.residente
    }

    /// La direccion del primer byte.
    ///
    /// Esta MAPEADO: a partir de aqui se escribe con `mov` y el kernel no se
    /// entera de nada. Un syscall por byte seria justo lo contrario de
    /// entregar memoria.
    pub fn base(&self) -> *mut u8 {
        self.base as *mut u8
    }

    /// **Devolverlo.** Consume el bloque: despues de esto su memoria ya no es
    /// tuya y escribir en ella es un `#PF`.
    ///
    /// `true` devuelto, `false` no se pudo -- hoy el unico motivo es que siga
    /// PRESTADO a otro proceso, y el motivo esta en CABINA (F11).
    ///
    /// *** TOMA `self` Y NO `&self`, y es la mitad del valor de esto. Con una
    /// referencia quedaria un `Memoria` en manos del programa con una `base`
    /// que ya no esta mapeada -- exactamente el mismo argumento que lleva
    /// escrito `Pantalla::soltar` desde que existe.
    ///
    /// ** El `Drop` (2026-09-21) hace esto mismo para lo PRESTADO, y nada
    /// para lo RESIDENTE. Esta funcion queda para quien quiere el veredicto
    /// --`false` = sigue prestado a otro-- o soltar un residente a mano.
    ///
    /// [!] `forget` despues del syscall: sin el, `Drop` volveria a pedir
    /// SOLTAR sobre un handle que ya no existe. No romperia nada --el kernel
    /// lo niega--, pero seria un renglon de CABINA por cada bloque devuelto,
    /// y un "no" que se produce a proposito no es un no.
    pub fn soltar(self) -> bool {
        let ok = invoke(self.cap, MEM_OP_SOLTAR, 0, 0, 0).valor().unwrap_or(0) == 1;
        core::mem::forget(self);
        ok
    }

    /// **Devolverlo, y si sigue PRESTADO a otro, DORMIR hasta que vuelva.**
    ///
    /// Es el bucle `soltar -> WAIT -> soltar` del `PLAN_LA_VIDA_UTIL` 7:
    /// `MEM_OP_SOLTAR` contesta 1 (devuelto) o un PAR (la secuencia del bloque
    /// que vio, por dos); con esa secuencia `WAIT` duerme --no gira-- hasta
    /// que el prestatario suelte o muera, o venza `plazo_ns`. Lo que WAIT
    /// devuelve es una secuencia, no un veredicto: por eso se vuelve a
    /// preguntar, y quien dice que si sigue siendo el kernel.
    ///
    /// `Err(self)` = vencio el plazo y el bloque sigue siendo tuyo, entero.
    /// `plazo_ns = 0` es esperar sin plazo, y no es lo normal: un compositor
    /// que se duerme sin plazo sobre un bloque de una app que no suelta es
    /// un compositor que no pinta.
    pub fn soltar_esperando(self, plazo_ns: u64) -> Result<(), Self> {
        loop {
            let v = invoke(self.cap, MEM_OP_SOLTAR, 0, 0, 0).valor().unwrap_or(0);
            if v == 1 {
                core::mem::forget(self);
                return Ok(());
            }
            let visto = v >> 1;
            let r = crate::sys::wait(self.cap, visto, plazo_ns);
            if r.value == visto {
                return Err(self);
            }
        }
    }

    /// **El handle del bloque**, para las operaciones que lo reciben.
    ///
    /// * Es una capability, no una direccion: quien la recibe puede comprobar
    /// que es tuya y con que derechos, cosa que con un puntero no se puede.
    /// Lo usa `estratos::crear_desde` para entregar el contenido de un fichero
    /// sin mandarlo de ocho en ocho.
    pub fn handle(&self) -> u64 {
        self.cap
    }

    /// Lo que se pidio.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Lo que el kernel dice que lleva entregado a este proceso -- que puede ser
    /// mas que `bytes()` si se pidio varias veces, y siempre esta redondeado a
    /// paginas enteras.
    pub fn entregado(&self) -> u64 {
        invoke(self.cap, MEM_OP_BYTES, 0, 0, 0).value
    }
}

/// **Lo prestado vuelve al salir del alcance.** Lo residente, no.
///
/// Si el kernel contesta que NO (el bloque sigue PRESTADO a otro proceso), el
/// bloque se queda: el kernel no lo suelta y este `Drop` no puede esperar --
/// no hay a quien devolverle un veredicto. El motivo esta en CABINA, y el paso
/// siguiente (`soltar -> WAIT -> soltar`) es la clave nueva de WAIT del
/// `PLAN_LA_VIDA_UTIL` 7, que todavia no existe.
impl Drop for Memoria {
    fn drop(&mut self) {
        if !self.residente {
            invoke(self.cap, MEM_OP_SOLTAR, 0, 0, 0);
        }
    }
}

/// Un campo de TEXTO en `dst`. Devuelve cuantos bytes se escribieron.
///
/// Viaja de 8 en 8 con el cero como final, igual que la ruta de `ejecutar`: en
/// esta superficie no hay punteros de Ring 3 hacia el kernel.
pub fn info_texto(campo: u64, dst: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut trozo = 0u64;
    while n < dst.len() {
        let w = invoke(CURRENT_TASK, OP_INFO_TEXTO, campo, trozo, 0).value;
        if w == 0 {
            break;
        }
        for k in 0..8 {
            let b = ((w >> (k * 8)) & 0xFF) as u8;
            if b == 0 || n >= dst.len() {
                return n;
            }
            dst[n] = b;
            n += 1;
        }
        trozo += 1;
    }
    n
}

/// Reiniciar la maquina. No vuelve.
///
/// Reiniciar es tocar puertos de E/S (`0xCF9`, el 8042) y Ring 3 no puede
/// hacerlo: se le pide al kernel, que ya tenia el reinicio de tres pasos para
/// su propio shell. Si volviera --no deberia-- se cede el turno en vez de seguir
/// como si nada, por la misma razon que en [`salir`].
pub fn reiniciar() -> ! {
    invoke(CURRENT_TASK, OP_REINICIAR, 0, 0, 0);
    loop {
        yield_screen();
    }
}

/// Escribir en la consola del kernel.
///
/// La puerta admite 8 bytes empaquetados en little-endian por llamada, con el
/// cero como final. Es deliberadamente pobre: es la consola de arranque, no la
/// salida de nadie en serio. Cuando el compositor exista, el terminal sera un
/// proceso Ring 3 y esto quedara para lo que es -- decir "estoy vivo" antes de
/// que haya con que decirlo.
pub fn consola(texto: &str) {
    for trozo in texto.as_bytes().chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        invoke(CURRENT_TASK, OP_CONSOLE_WRITE, u64::from_le_bytes(w), 0, 0);
    }
}

