//! **Los hilos de un `.exe`, decididos** (P4, 27-09): quien corre, quien
//! espera, y a que.
//!
//! BMO-X no tiene hilos de Ring 3 (una tarea es un hilo, y el FUERO los deja
//! para cuando haya SMP cableado). Un `.exe` de Windows los crea igual, asi
//! que PROTON-X se los da DENTRO de su tarea: hilos COOPERATIVOS, cada uno con
//! su pila y su TEB, que se ceden el turno cuando uno ESPERA (un evento, una
//! seccion critica, `Sleep`, la cola de mensajes). Es el modelo M:1 -- el de
//! las fibras de Windows y el de muchos runtimes --, y para un juego es lo que
//! pasa de verdad: sus hilos pasan la vida esperando trabajo.
//!
//! Aqui va solo la DECISION, pura y con banco; el cambio de pila y de GS es de
//! la casa (`proton-x-casa/src/hilos.rs`):
//!
//! ```text
//!    objetos       eventos (manual o automatico), semaforos, hilos (se
//!                  encienden al acabar), mutex (con abandono) y
//!                  temporizadores esperables (P4f2)
//!    cerrojos      secciones criticas y SRW, por la DIRECCION del .exe:
//!                  propietario, recursion, lectores
//!    condiciones   SleepConditionVariable*, Wake(All)ConditionVariable
//!    el turno      [`Planificador::siguiente`]: el primero que pueda seguir,
//!                  en rueda desde el que tiene el turno; si nadie puede y
//!                  hay un plazo, esperar a el; si nadie puede NUNCA, es un
//!                  BLOQUEO MUTUO, y se dice
//! ```
//!
//! Lo que falta y se dice: un hilo que da vueltas sin llamar a nadie (sin
//! esperar, sin mensajes) no suelta el turno -- no hay reloj que se lo quite.
//!
//! **El turno PRESTADO** (T1 y T2 de `PLAN_LOS_DOCE_DIRECTORES`, 07-10).
//! Cyberpunk, en el metal: 96 cortes de sonido, uno de 33 s. El hilo del
//! sonido espera su evento, pero quien tiene el turno esta DENTRO de algo
//! largo (una lista de dibujo por la CPU, mil PSO) y no espera nada. Lo
//! largo pregunta de vez en cuando por un hilo URGENTE ([`Planificador::
//! urgente`]: el que espera un evento del sonido) que ya pueda seguir; si lo hay, le PRESTA el turno, y el siguiente turno vuelve al que
//! presto ([`Planificador::prestado`]) y no a la rueda. Asi lo largo sigue
//! de un tiron para todos los demas: ningun otro hilo del juego entra en
//! D3D12 a mitad de una lista.

use alloc::vec::Vec;

pub type Id = usize;

pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 0x102;
/// Un mutex cuyo propietario acabo sin soltarlo (P4f2).
pub const WAIT_ABANDONED_0: u32 = 0x80;

/// Un objeto que se puede esperar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objeto {
    Evento { manual: bool, encendido: bool },
    Semaforo { cuenta: u32, max: u32 },
    /// Se enciende cuando el hilo acaba.
    Hilo(Id),
    /// `CreateMutex` (P4f2): de un hilo, con recursion. Si su propietario
    /// acaba sin soltarlo, queda ABANDONADO y el siguiente lo coge con
    /// WAIT_ABANDONED_0 + i.
    Mutex { propietario: Option<Id>, cuenta: u32, abandonado: bool },
    /// `CreateWaitableTimer` (P4f2): se enciende al vencer (ns); con periodo,
    /// vuelve a vencer. Manual: sigue encendido; si no, lo apaga quien lo coge.
    Temporizador { manual: bool, encendido: bool, vence: Option<u64>, periodo: u64 },
    /// Cerrado (`CloseHandle`): su numero no se reusa.
    Cerrado,
}

/// En que esta un hilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    Listo,
    /// `WaitFor*`: a estos objetos, todos o cualquiera, hasta un plazo (ns).
    Espera { objetos: Vec<usize>, todos: bool, plazo: Option<u64> },
    /// `Sleep`.
    Dormido { hasta: u64 },
    /// `CREATE_SUSPENDED`: cuantas veces.
    Suspendido(u32),
    /// Esperando un cerrojo (seccion critica o SRW) en esta direccion.
    Cerrojo { dir: u64, exclusivo: bool },
    /// Dentro de `SleepConditionVariable*`, hasta que lo despierten.
    Condicion { dir: u64, plazo: Option<u64> },
    Terminado(u32),
}

/// Un cerrojo: una seccion critica (con recursion) o un SRW (con lectores).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cerrojo {
    pub dir: u64,
    pub propietario: Option<Id>,
    pub recursion: u32,
    pub lectores: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Hilo {
    estado: Estado,
    /// Lo que devuelve su ultima espera (WAIT_OBJECT_0 + i o WAIT_TIMEOUT).
    resultado: u32,
    /// Una condicion lo desperto (Wake*ConditionVariable).
    despertado: bool,
    /// `SuspendThread` (30-09): mientras no sea 0, no corre, este en lo que
    /// este (su espera sigue donde estaba al reanudarlo).
    suspension: u32,
    /// `SetThreadPriority` (T2, 07-10): de -15 a 15, como Windows. Se
    /// GUARDA y se devuelve; no presta el turno (ver [`Planificador::
    /// urgente`]): la usaran los directores (H4 del mismo plan).
    prioridad: i32,
    /// Espero un objeto del sonido ([`Planificador::del_sonido`]): es el
    /// hilo del sonido, este esperando o ya listo.
    del_sonido: bool,
}

/// **Lo que toca ahora.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turno {
    /// Que corra este hilo (puede ser el mismo que pregunta).
    Hilo(Id),
    /// Nadie puede seguir todavia; el primer plazo vence en este instante (ns).
    Esperar(u64),
    /// Nadie puede seguir NUNCA: todos esperan algo que solo otro hilo que
    /// tambien espera podria dar.
    Bloqueo,
}

/// **El planificador de un proceso.** El hilo 0 es el principal, y empieza
/// con el turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planificador {
    hilos: Vec<Hilo>,
    objetos: Vec<Objeto>,
    cerrojos: Vec<Cerrojo>,
    /// Quien espera en que condicion, por orden de llegada.
    condiciones: Vec<(u64, Id)>,
    pub actual: Id,
    /// T1 (07-10): quien PRESTO el turno a un urgente; el siguiente turno es
    /// suyo, si puede seguir.
    pub prestado: Option<Id>,
    /// T1 (07-10): los objetos del SONIDO (el evento de un flujo de WASAPI,
    /// `SetEventHandle`): quien los espera es el hilo del sonido.
    sonido: Vec<usize>,
}

impl Default for Planificador {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Planificador {
    pub fn nuevo() -> Self {
        Planificador {
            hilos: alloc::vec![Hilo { estado: Estado::Listo, resultado: 0, despertado: false, suspension: 0, prioridad: 0, del_sonido: false }],
            objetos: Vec::new(),
            cerrojos: Vec::new(),
            condiciones: Vec::new(),
            actual: 0,
            prestado: None,
            sonido: Vec::new(),
        }
    }

    pub fn cuantos(&self) -> usize {
        self.hilos.len()
    }

    pub fn estado(&self, h: Id) -> Option<&Estado> {
        self.hilos.get(h).map(|x| &x.estado)
    }

    pub fn resultado(&self, h: Id) -> u32 {
        self.hilos.get(h).map(|x| x.resultado).unwrap_or(0)
    }

    pub fn objeto(&self, o: usize) -> Option<Objeto> {
        self.objetos.get(o).copied()
    }

    // -- Objetos --------------------------------------------------------------

    pub fn nuevo_objeto(&mut self, o: Objeto) -> usize {
        self.objetos.push(o);
        self.objetos.len() - 1
    }

    /// Un hilo nuevo y su objeto (lo que devuelve `CreateThread`).
    pub fn crear(&mut self, suspendido: bool) -> (Id, usize) {
        let estado = if suspendido { Estado::Suspendido(1) } else { Estado::Listo };
        self.hilos.push(Hilo { estado, resultado: 0, despertado: false, suspension: 0, prioridad: 0, del_sonido: false });
        let id = self.hilos.len() - 1;
        (id, self.nuevo_objeto(Objeto::Hilo(id)))
    }

    /// `SetThreadPriority` (T2): `false` si no es un hilo. Se guarda tal cual
    /// (Windows acepta -15, -2..2 y 15; lo demas lo rechaza el de fuera).
    pub fn poner_prioridad(&mut self, h: Id, p: i32) -> bool {
        match self.hilos.get_mut(h) {
            Some(x) => {
                x.prioridad = p;
                true
            }
            None => false,
        }
    }

    /// `GetThreadPriority`.
    pub fn prioridad(&self, h: Id) -> Option<i32> {
        self.hilos.get(h).map(|x| x.prioridad)
    }

    /// `SuspendThread`: la cuenta de antes, o `None` si no es un hilo (o ya
    /// termino).
    pub fn suspender(&mut self, h: Id) -> Option<u32> {
        let x = self.hilos.get_mut(h)?;
        if matches!(x.estado, Estado::Terminado(_)) {
            return None;
        }
        let antes = x.suspension + if let Estado::Suspendido(n) = x.estado { n } else { 0 };
        x.suspension += 1;
        Some(antes)
    }

    /// `ResumeThread`: la cuenta de antes, o `None` si no es un hilo.
    pub fn reanudar(&mut self, h: Id) -> Option<u32> {
        let x = self.hilos.get_mut(h)?;
        if x.suspension > 0 {
            let antes = x.suspension + if let Estado::Suspendido(n) = x.estado { n } else { 0 };
            x.suspension -= 1;
            return Some(antes);
        }
        Some(match x.estado {
            Estado::Suspendido(n) => {
                x.estado = if n <= 1 { Estado::Listo } else { Estado::Suspendido(n - 1) };
                n
            }
            _ => 0,
        })
    }

    pub fn terminar(&mut self, h: Id, codigo: u32) {
        if let Some(x) = self.hilos.get_mut(h) {
            x.estado = Estado::Terminado(codigo);
        }
        self.condiciones.retain(|&(_, q)| q != h);
        // Sus mutex quedan abandonados, como en Windows.
        for o in &mut self.objetos {
            if let Objeto::Mutex { propietario, cuenta, abandonado } = o {
                if *propietario == Some(h) {
                    *propietario = None;
                    *cuenta = 0;
                    *abandonado = true;
                }
            }
        }
    }

    /// `ReleaseMutex` del hilo `quien`: `false` si no es suyo (ERROR_NOT_OWNER).
    pub fn soltar_mutex(&mut self, o: usize, quien: Id) -> bool {
        match self.objetos.get_mut(o) {
            Some(Objeto::Mutex { propietario, cuenta, .. }) if *propietario == Some(quien) => {
                *cuenta -= 1;
                if *cuenta == 0 {
                    *propietario = None;
                }
                true
            }
            _ => false,
        }
    }

    /// `SetWaitableTimer`: vence en `vence` (ns), y luego cada `periodo` (0,
    /// una vez). Lo apaga mientras tanto, como Windows.
    pub fn poner_temporizador(&mut self, o: usize, vence: u64, periodo: u64) -> bool {
        match self.objetos.get_mut(o) {
            Some(Objeto::Temporizador { encendido, vence: v, periodo: p, .. }) => {
                *encendido = false;
                *v = Some(vence);
                *p = periodo;
                true
            }
            _ => false,
        }
    }

    /// `CancelWaitableTimer`: deja de vencer (y lo que estaba, se queda).
    pub fn cancelar_temporizador(&mut self, o: usize) -> bool {
        match self.objetos.get_mut(o) {
            Some(Objeto::Temporizador { vence, .. }) => {
                *vence = None;
                true
            }
            _ => false,
        }
    }

    /// Los temporizadores que vencieron para `ahora`, encendidos.
    fn vencer(&mut self, ahora: u64) {
        for o in &mut self.objetos {
            if let Objeto::Temporizador { encendido, vence: Some(v), periodo, .. } = o {
                if *v <= ahora {
                    *encendido = true;
                    if *periodo == 0 {
                        if let Objeto::Temporizador { vence, .. } = o {
                            *vence = None;
                        }
                    } else {
                        // Los periodos que se perdieron no se acumulan.
                        let p = *periodo;
                        *v += (ahora - *v) / p * p + p;
                    }
                }
            }
        }
    }

    /// El codigo de salida, o `None` si sigue vivo (STILL_ACTIVE).
    pub fn salida(&self, h: Id) -> Option<u32> {
        match self.hilos.get(h)?.estado {
            Estado::Terminado(c) => Some(c),
            _ => None,
        }
    }

    pub fn encender(&mut self, o: usize, si: bool) -> bool {
        match self.objetos.get_mut(o) {
            Some(Objeto::Evento { encendido, .. }) => {
                *encendido = si;
                true
            }
            _ => false,
        }
    }

    /// `ReleaseSemaphore`: la cuenta de antes, o `None` si pasaria del maximo.
    pub fn soltar_semaforo(&mut self, o: usize, n: u32) -> Option<u32> {
        match self.objetos.get_mut(o) {
            Some(Objeto::Semaforo { cuenta, max }) => {
                let antes = *cuenta;
                let nueva = antes.checked_add(n).filter(|&c| c <= *max)?;
                *cuenta = nueva;
                Some(antes)
            }
            _ => None,
        }
    }

    pub fn cerrar(&mut self, o: usize) -> bool {
        match self.objetos.get_mut(o) {
            Some(x) if *x != Objeto::Cerrado => {
                *x = Objeto::Cerrado;
                true
            }
            _ => false,
        }
    }

    fn marcado(&self, o: usize, quien: Id) -> bool {
        match self.objetos.get(o) {
            Some(Objeto::Mutex { propietario, .. }) => propietario.is_none() || *propietario == Some(quien),
            Some(Objeto::Temporizador { encendido, .. }) => *encendido,
            Some(Objeto::Evento { encendido, .. }) => *encendido,
            Some(Objeto::Semaforo { cuenta, .. }) => *cuenta > 0,
            Some(Objeto::Hilo(h)) => matches!(self.hilos.get(*h).map(|x| &x.estado), Some(Estado::Terminado(_))),
            _ => false,
        }
    }

    /// Coge `o` para `quien`; `true` si era un mutex abandonado.
    fn consumir(&mut self, o: usize, quien: Id) -> bool {
        match self.objetos.get_mut(o) {
            Some(Objeto::Mutex { propietario, cuenta, abandonado }) => {
                *propietario = Some(quien);
                *cuenta += 1;
                return core::mem::take(abandonado);
            }
            Some(Objeto::Temporizador { manual: false, encendido, .. }) => *encendido = false,
            Some(Objeto::Evento { manual: false, encendido }) => *encendido = false,
            Some(Objeto::Semaforo { cuenta, .. }) => *cuenta -= 1,
            _ => {}
        }
        false
    }

    /// Si la espera se cumple YA, la cumple (consumiendo) y da su resultado.
    fn cumplir(&mut self, quien: Id, objetos: &[usize], todos: bool, ahora: u64) -> Option<u32> {
        self.vencer(ahora);
        if todos {
            if objetos.iter().all(|&o| self.marcado(o, quien)) {
                let mut abandonado = None;
                for (i, &o) in objetos.iter().enumerate() {
                    if self.consumir(o, quien) && abandonado.is_none() {
                        abandonado = Some(i as u32);
                    }
                }
                return Some(abandonado.map_or(WAIT_OBJECT_0, |i| WAIT_ABANDONED_0 + i));
            }
            return None;
        }
        let i = objetos.iter().position(|&o| self.marcado(o, quien))?;
        let base = if self.consumir(objetos[i], quien) { WAIT_ABANDONED_0 } else { WAIT_OBJECT_0 };
        Some(base + i as u32)
    }

    /// **`WaitFor*Object(s)` del hilo actual.** `Some` si se contesta YA; si
    /// no, el hilo queda esperando y quien llama tiene que ceder el turno
    /// hasta que vuelva a tenerlo; entonces, [`Self::resultado`].
    pub fn esperar(&mut self, objetos: &[usize], todos: bool, plazo: Option<u64>, ahora: u64) -> Option<u32> {
        if objetos.iter().any(|o| self.sonido.contains(o)) {
            let a = self.actual;
            self.hilos[a].del_sonido = true;
        }
        if let Some(r) = self.cumplir(self.actual, objetos, todos, ahora) {
            return Some(r);
        }
        if plazo.is_some_and(|p| p <= ahora) {
            return Some(WAIT_TIMEOUT);
        }
        let a = self.actual;
        self.hilos[a].estado = Estado::Espera { objetos: objetos.to_vec(), todos, plazo };
        None
    }

    /// `Sleep` del hilo actual: hasta `hasta` (ns). `Sleep(0)` es ceder.
    pub fn dormir(&mut self, hasta: u64) {
        let a = self.actual;
        self.hilos[a].estado = Estado::Dormido { hasta };
    }

    // -- Cerrojos ----------------------------------------------------------------

    fn cerrojo(&mut self, dir: u64) -> &mut Cerrojo {
        let i = match self.cerrojos.iter().position(|c| c.dir == dir) {
            Some(i) => i,
            None => {
                self.cerrojos.push(Cerrojo { dir, propietario: None, recursion: 0, lectores: 0 });
                self.cerrojos.len() - 1
            }
        };
        &mut self.cerrojos[i]
    }

    pub fn ver_cerrojo(&self, dir: u64) -> Option<Cerrojo> {
        self.cerrojos.iter().find(|c| c.dir == dir).copied()
    }

    /// Coger un cerrojo para `h` si se puede. Una seccion critica deja al
    /// propietario volver a entrar (recursion); un SRW exclusivo, no.
    fn coger(&mut self, h: Id, dir: u64, exclusivo: bool, recursivo: bool) -> bool {
        let c = self.cerrojo(dir);
        match (c.propietario, exclusivo) {
            (None, true) if c.lectores == 0 => {
                c.propietario = Some(h);
                c.recursion = 1;
                true
            }
            (Some(d), true) if d == h && recursivo => {
                c.recursion += 1;
                true
            }
            (None, false) => {
                c.lectores += 1;
                true
            }
            _ => false,
        }
    }

    /// `Enter*`/`Acquire*` del hilo actual. `true`: cogido. `false`: queda
    /// esperandolo (y hay que ceder el turno).
    pub fn entrar(&mut self, dir: u64, exclusivo: bool, recursivo: bool) -> bool {
        let a = self.actual;
        if self.coger(a, dir, exclusivo, recursivo) {
            return true;
        }
        self.hilos[a].estado = Estado::Cerrojo { dir, exclusivo };
        false
    }

    /// `TryEnter*`/`TryAcquire*`: sin esperar.
    pub fn probar(&mut self, dir: u64, exclusivo: bool, recursivo: bool) -> bool {
        let a = self.actual;
        self.coger(a, dir, exclusivo, recursivo)
    }

    /// `Leave*`/`Release*` del hilo actual. `false` si no era suyo.
    pub fn salir(&mut self, dir: u64, exclusivo: bool) -> bool {
        let a = self.actual;
        let c = self.cerrojo(dir);
        if exclusivo {
            if c.propietario != Some(a) {
                return false;
            }
            c.recursion -= 1;
            if c.recursion == 0 {
                c.propietario = None;
            }
            true
        } else if c.lectores > 0 {
            c.lectores -= 1;
            true
        } else {
            false
        }
    }

    /// Soltar una seccion critica ENTERA (para dormir en una condicion): la
    /// recursion que tenia, para devolverla al volver.
    pub fn soltar_todo(&mut self, dir: u64) -> u32 {
        let a = self.actual;
        let c = self.cerrojo(dir);
        if c.propietario != Some(a) {
            return 0;
        }
        let r = c.recursion;
        c.propietario = None;
        c.recursion = 0;
        r
    }

    /// Devolver la recursion guardada (el hilo ya es propietario otra vez).
    pub fn poner_recursion(&mut self, dir: u64, r: u32) {
        let a = self.actual;
        let c = self.cerrojo(dir);
        if c.propietario == Some(a) && r > 0 {
            c.recursion = r;
        }
    }

    // -- Condiciones --------------------------------------------------------------

    /// El hilo actual se duerme en la condicion `dir` (ya solto su cerrojo).
    pub fn dormir_en(&mut self, dir: u64, plazo: Option<u64>) {
        let a = self.actual;
        self.hilos[a].despertado = false;
        self.hilos[a].estado = Estado::Condicion { dir, plazo };
        self.condiciones.push((dir, a));
    }

    /// `WakeConditionVariable` (uno, el que llego antes) o `WakeAll`.
    pub fn despertar(&mut self, dir: u64, todos: bool) {
        while let Some(i) = self.condiciones.iter().position(|&(d, _)| d == dir) {
            let (_, h) = self.condiciones.remove(i);
            self.hilos[h].despertado = true;
            if !todos {
                break;
            }
        }
    }

    // -- El turno -------------------------------------------------------------------

    /// Si `h` puede seguir ya; si su espera se cumple, la cumple.
    fn puede(&mut self, h: Id, ahora: u64) -> bool {
        if self.hilos[h].suspension > 0 {
            return false;
        }
        let estado = self.hilos[h].estado.clone();
        let (listo, resultado) = match estado {
            Estado::Listo => (true, None),
            Estado::Espera { objetos, todos, plazo } => match self.cumplir(h, &objetos, todos, ahora) {
                Some(r) => (true, Some(r)),
                None if plazo.is_some_and(|p| p <= ahora) => (true, Some(WAIT_TIMEOUT)),
                None => (false, None),
            },
            Estado::Dormido { hasta } => (hasta <= ahora, None),
            Estado::Cerrojo { dir, exclusivo } => (self.coger(h, dir, exclusivo, true), None),
            Estado::Condicion { dir, plazo } => {
                if self.hilos[h].despertado {
                    (true, Some(WAIT_OBJECT_0))
                } else if plazo.is_some_and(|p| p <= ahora) {
                    self.condiciones.retain(|&(d, q)| !(d == dir && q == h));
                    (true, Some(WAIT_TIMEOUT))
                } else {
                    (false, None)
                }
            }
            Estado::Suspendido(_) | Estado::Terminado(_) => (false, None),
        };
        if listo {
            self.hilos[h].estado = Estado::Listo;
            if let Some(r) = resultado {
                self.hilos[h].resultado = r;
            }
        }
        listo
    }

    /// El plazo mas cercano de los que esperan (para dormir hasta el).
    fn primer_plazo(&self) -> Option<u64> {
        self.hilos
            .iter()
            .flat_map(|x| {
                let (a, b) = match &x.estado {
                    // Esperar a un temporizador tambien es esperar a su plazo.
                    Estado::Espera { plazo, objetos, .. } => (*plazo, objetos.iter().filter_map(|&o| match self.objetos.get(o) {
                        Some(Objeto::Temporizador { vence, .. }) => *vence,
                        _ => None,
                    }).min()),
                    Estado::Condicion { plazo, .. } => (*plazo, None),
                    Estado::Dormido { hasta } => (Some(*hasta), None),
                    _ => (None, None),
                };
                [a, b]
            })
            .flatten()
            .min()
    }

    /// **A quien le toca**: el que presto el turno, si puede seguir (T1);
    /// si no, el primero que pueda seguir, en rueda desde el SIGUIENTE al
    /// actual (el actual, el ultimo: asi ceder es ceder).
    pub fn siguiente(&mut self, ahora: u64) -> Turno {
        if let Some(h) = self.prestado.take() {
            if h != self.actual && h < self.hilos.len() && self.puede(h, ahora) {
                return Turno::Hilo(h);
            }
        }
        let n = self.hilos.len();
        for k in 1..=n {
            let h = (self.actual + k) % n;
            if self.puede(h, ahora) {
                return Turno::Hilo(h);
            }
        }
        match self.primer_plazo() {
            Some(p) => Turno::Esperar(p),
            None => Turno::Bloqueo,
        }
    }

    /// **Este objeto es del SONIDO** (T1, 07-10): el evento de un flujo de
    /// WASAPI. Quien lo espere, desde entonces, es el hilo del sonido.
    pub fn del_sonido(&mut self, o: usize) {
        if !self.sonido.contains(&o) {
            self.sonido.push(o);
        }
    }

    /// **Un hilo URGENTE que ya puede seguir** (T1 y T2, 07-10): el del
    /// sonido -- el que espero alguno de los objetos de [`Self::del_sonido`],
    /// aunque ahora ya este LISTO (su espera se cumplio en una vuelta que
    /// eligio a otro). En rueda desde el siguiente al actual; el actual no
    /// cuenta. Si su espera se cumple, se cumple aqui (el evento automatico
    /// se gasta: es suyo).
    ///
    /// **Por que NO la prioridad alta:** el que presta esta a mitad de algo
    /// (una lista de D3D12). El hilo del sonido no toca D3D12; uno de
    /// prioridad HIGHEST del juego puede ser el que manda listas, y entraria
    /// en la cola a mitad. Prestar a cualquiera es cosa de los directores
    /// (H2.1: la casa con cerrojos), no de esto.
    pub fn urgente(&mut self, ahora: u64) -> Option<Id> {
        let n = self.hilos.len();
        for k in 1..n {
            let h = (self.actual + k) % n;
            if self.hilos[h].del_sonido && self.puede(h, ahora) {
                return Some(h);
            }
        }
        None
    }

    /// Cuantos hilos siguen vivos (no terminados).
    pub fn vivos(&self) -> usize {
        self.hilos.iter().filter(|x| !matches!(x.estado, Estado::Terminado(_))).count()
    }
}
