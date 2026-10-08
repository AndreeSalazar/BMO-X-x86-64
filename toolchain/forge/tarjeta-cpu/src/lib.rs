//! # bmo-tarjeta-cpu -- la CPU, la SEGUNDA tarjeta de PROMETEO: la reserva
//!
//! generacion: nieto -- la hermana de la tarjeta de la 3060 (`proton-x-sm86`)
//!
//! LB4 de `docs/plan/PLAN_LAS_LIBRERIAS.md` (08-10), con DL3 del propietario:
//! que la CPU CORRA una gpu fn como su reserva, con los bits de la casa (la
//! ley L32 de TITAN++). Es una tarjeta mas, por el MISMO contrato que la 3060
//! (`bmo_prometeo::Tarjeta`), sin tocar una linea de PROMETEO ni de la
//! libreria de la 3060: lo que el plan llamaba "la prueba de que el contrato
//! vale para dos, sin comprar otra tarjeta".
//!
//! ```text
//!    SABE      la cuenta de UNA celda sin llamar a nadie: sus entradas y su
//!              salida, la aritmetica de f32 (la division general tambien:
//!              `divss` es IEEE, redondea como la casa), las comparaciones,
//!              los enteros de 32 bits, `Elige` y los saltos y bucles del
//!              Programa. Lo demas -- texturas, matematica, UAV, computo,
//!              geometria, olas -- es su LIMITE, dicho con su porque
//!              ([`no_sabe`])
//!    EMITE     el x86-64 con SSE de PROTON-X (`bmo_proton_x::nativo`: un
//!              sombreador traducido UNA vez, juzgado contra el interprete en
//!              `proton-x-casa`), con un PROLOGO que pone sus registros: quien
//!              lo llama no prepara nada
//!    JUZGA     el CONTRATO DE LA LLAMADA, corriendolo ([`Corrida`]): vuelve,
//!              no llama al kernel, conserva rbx, rbp, r12..r15, rsp y el
//!              MXCSR, deja eax = 0, y no escribe fuera de sus registros, su
//!              salida y su pila. Es lo que su puerta (E1 de TITAN++) da por
//!              hecho, y se mira en CADA celda que simula
//!    SIMULA    sus BYTES, en el emulador de la casa (`bmo_lower::emu`): los
//!              mismos que viajan dentro del .bex
//!    ENTREGA   su codigo dentro del .bex: lo llama E1 al correr
//! ```
//!
//! ## La llamada (lo que su puerta tiene que poner)
//!
//! ```text
//!    rdi   sus REGISTROS: `registros` palabras de 4 bytes. Las llena el
//!          prologo con los iniciales del Programa: quien llama da el sitio
//!    rsi   las ENTRADAS: un [u32; 4] por entrada (una celda, en el
//!          componente 0)
//!    rdx   un puntero valido cualquiera: el cbuffer, que un Programa de
//!          esta tarjeta no lee
//!    rcx   la SALIDA: un [u32; 4] por salida
//!    r8    0: las llamadas a Rust de `nativo` (texturas, matematica); aqui
//!          no hay a quien llamar
//!    vuelve con eax = 0, y rbx, rbp, r12..r15, rsp y el MXCSR como estaban
//!    (System V). Pisa lo demas
//! ```

use bmo_lower::emu::{run_acotado, Machine, MXCSR_REINICIO, STACK_TOP};
use bmo_prometeo::programa::{Op, Programa};
use bmo_prometeo::{Codigo, Ficha, NoEmite, Para, Tarjeta};

/// La CPU, la segunda tarjeta (la reserva).
pub struct Cpu;

/// La tarjeta que da quien arma una herramienta (`titan`).
pub static CPU: Cpu = Cpu;

/// Como la nombra la casa en un mensaje.
pub const NOMBRE: &str = "la CPU";
/// Su codigo maquina.
pub const LENGUA: &str = "x86-64 con SSE";
/// Los registros de un cuerpo: palabras de 4 bytes en la PILA de quien lo
/// llama (4 KiB). No es un techo del silicio: es lo que E1 reserva en un
/// marco sin acercarse a lo que la pila de una tarea aguanta.
pub const REGISTROS: u32 = 1024;
/// Las instrucciones que corre una celda, como mucho: un cuerpo que no
/// vuelve es un NO de su juez, no un cuelgue del compilador.
///
/// 08-10, LB5: de 1M a 4M (2^22, el mismo tope que el simulador de la
/// 3060). Con los bucles, una gpu fn en el tope de su obra (`gpu.rs` de
/// TITAN++: 65536 por celda) corre aqui hasta ~12 instrucciones por unidad
/// -- `if` anidados con una condicion que no cuesta nada --, unas 800.000:
/// con 1M el margen era de un 20 %; con 4M, cinco veces
/// (`pruebas_bucles.rs` de `bmo-titan-prometeo` lo mide en cada patron).
pub const PASOS: usize = 1 << 22;

/// La pila que puede usar un cuerpo, por debajo de quien lo llama: `nativo`
/// guarda cinco registros y su `Contexto` (unos 150 bytes).
const PILA: u64 = 64 * 1024;

/// Lo que un cuerpo tiene que conservar, con el valor que se le pone antes
/// de llamarlo: rbx, rbp, r12, r13, r14, r15.
const CONSERVA: [(usize, &str, u64); 6] = [
    (3, "rbx", 0x0B0B_0B0B_0B0B_0B0B),
    (5, "rbp", 0x0B0B_0B0B_0B0B_0B05),
    (12, "r12", 0x0B0B_0B0B_0B0B_0B12),
    (13, "r13", 0x0B0B_0B0B_0B0B_0B13),
    (14, "r14", 0x0B0B_0B0B_0B0B_0B14),
    (15, "r15", 0x0B0B_0B0B_0B0B_0B15),
];

/// **Lo que esta tarjeta todavia no sabe correr**: la primera operacion del
/// Programa que no es la cuenta de una celda, y su porque. Cada una de las
/// que SI sabe tiene su prueba contra el interprete (`pruebas.rs`): lo que
/// su simulador no sabe correr, esta tarjeta no lo emite.
pub fn no_sabe(p: &Programa) -> Option<(usize, &'static str)> {
    p.ops.iter().enumerate().find_map(|(k, op)| {
        let por_que = match op {
            Op::Entrada { .. } | Op::Salida { .. } | Op::Mul { .. } | Op::Add { .. } | Op::Sub { .. } | Op::Div { .. } | Op::Mad { .. } | Op::Dot { .. } | Op::Rsqrt { .. } | Op::Sqrt { .. } | Op::Saturate { .. } | Op::Abs { .. } | Op::Min { .. } | Op::Max { .. } => return None,
            Op::Compara { .. } | Op::Elige { .. } | Op::Copia { .. } | Op::SumaEntera { .. } | Op::Entera { .. } | Op::Convierte { .. } => return None,
            Op::Si { .. } | Op::SiNo | Op::FinSi | Op::Bucle | Op::RomperSi { .. } | Op::Romper | Op::Continuar | Op::FinBucle => return None,
            Op::Constantes { .. } | Op::ConstantesEn { .. } => "lee un cbuffer, y la puerta de la CPU solo da entradas y una salida",
            Op::Mate { .. } | Op::Muestra { .. } | Op::Lee { .. } | Op::EligeTextura { .. } => "llama al interprete en Rust (la matematica, las texturas), y dentro de un .bex no hay a quien llamar",
            Op::IdHilo { .. } | Op::Barrera | Op::LeeCompartida { .. } | Op::EscribeCompartida { .. } | Op::AtomicoCompartido { .. } => "es de computo (hilos, barreras, memoria compartida), y una celda de la CPU corre sola",
            Op::EscribeUav { .. } | Op::LeeUav { .. } | Op::MedidasUav { .. } | Op::Contador { .. } | Op::Atomico { .. } => "lee o escribe memoria de fuera (un UAV), y la puerta de la CPU solo da entradas y una salida",
            Op::EntradaDe { .. } | Op::Emite { .. } | Op::Corta { .. } => "es de geometria",
            Op::Ola { .. } => "mira a los otros carriles de su ola, y una celda de la CPU corre sola",
            Op::Descarta { .. } => "tira un pixel, y una celda no es un pixel",
            Op::LeeIndexado { .. } | Op::EscribeIndexado { .. } => "indexa sus registros, y eso todavia no tiene su prueba en esta tarjeta",
        };
        Some((k, por_que))
    })
}

/// **El prologo**: cada registro con su inicial (`mov dword [rdi + 4r],
/// imm32`). Con el, el codigo no pide nada preparado: la puerta da el sitio
/// y ya. Un registro que el Programa escribe antes de leer tambien se pone:
/// cada celda empieza como empieza en el interprete.
fn prologo(p: &Programa) -> Vec<u8> {
    let mut b = Vec::with_capacity(10 * p.iniciales.len());
    for (r, v) in p.iniciales.iter().enumerate() {
        b.extend_from_slice(&[0xC7, 0x87]);
        b.extend_from_slice(&(4 * r as u32).to_le_bytes());
        b.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    b
}

/// **Una corrida en el emulador de la casa**: el codigo, llamado como lo
/// llama su puerta, y lo que se mira despues.
pub struct Corrida {
    /// Lo que dejo en sus salidas.
    pub salidas: Vec<[u32; 4]>,
    /// Las instrucciones que corrio el cuerpo (sin la llamada que lo rodea).
    pub pasos: u64,
}

/// La llamada de su puerta, en el emulador: `call` al cuerpo y, al volver,
/// un salto al final (el emulador para al caerse del codigo).
fn arnes(bytes: &[u8]) -> Vec<u8> {
    let mut c = Vec::with_capacity(bytes.len() + 10);
    c.extend_from_slice(&[0xE8, 5, 0, 0, 0]); // call cuerpo (detras del jmp)
    c.push(0xE9); // jmp fin
    c.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    c.extend_from_slice(bytes);
    c
}

/// Los bytes de `n` palabras, para el emulador.
fn palabras(n: usize, de: impl Iterator<Item = u32>) -> Vec<u8> {
    let mut v: Vec<u8> = de.flat_map(u32::to_le_bytes).collect();
    v.resize(4 * n, 0);
    v
}

/// **Corre `c`** con estas entradas, y mira el CONTRATO DE LA LLAMADA: el NO,
/// en las palabras de esta tarjeta.
pub fn correr(c: &Codigo, entradas: &[[u32; 4]], salidas: usize) -> Result<Corrida, String> {
    let mut m = Machine::new(arnes(&c.bytes));
    let n_regs = (c.registros as usize).max(1);
    let regs = m.load_data(&palabras(n_regs, core::iter::empty()));
    let ent_bytes = palabras(4 * entradas.len().max(1), entradas.iter().flatten().copied());
    let ent = m.load_data(&ent_bytes);
    let sal = m.load_data(&palabras(4 * salidas.max(1), core::iter::empty()));
    let cb_bytes = [0u8; 16];
    let cb = m.load_data(&cb_bytes);
    m.regs[7] = regs; // rdi
    m.regs[6] = ent; // rsi
    m.regs[2] = cb; // rdx
    m.regs[1] = sal; // rcx
    m.regs[8] = 0; // r8
    for (r, _, v) in CONSERVA {
        m.regs[r] = v;
    }
    m.escritas = Some(Vec::new());
    let antes = m.pasos;
    let (m, acabo) = run_acotado(m, PASOS + 2);
    if !acabo {
        return Err(format!("no vuelve: corrio mas de {} instrucciones en una celda", PASOS));
    }
    if m.exited || !m.syscalls.is_empty() {
        return Err("llama al kernel: un cuerpo de la CPU es una cuenta, sin puertas".into());
    }
    if m.rip != m.code.len() {
        return Err("no vuelve a quien lo llamo".into());
    }
    for (r, nombre, v) in CONSERVA {
        if m.regs[r] != v {
            return Err(format!("no conserva {} (System V: quien llama lo da por guardado)", nombre));
        }
    }
    if m.regs[4] != STACK_TOP {
        return Err("no deja la pila como estaba".into());
    }
    if m.mxcsr != MXCSR_REINICIO {
        return Err(format!("deja el MXCSR en {:#x}, y quien llama lo tenia en {:#x}", m.mxcsr, MXCSR_REINICIO));
    }
    if m.regs[0] as u32 != 0 {
        return Err(format!("vuelve con eax = {}, y su puerta espera 0", m.regs[0] as u32));
    }
    // Lo que escribio: sus registros, su salida y su pila, y nada mas --
    // tambien cuando escribe lo que ya habia (el emulador apunta cada byte).
    let dentro = |a: u64, desde: u64, n: usize| a >= desde && a < desde + n as u64;
    let pila = STACK_TOP - PILA;
    for &a in m.escritas.as_deref().unwrap_or(&[]) {
        if dentro(a, ent, ent_bytes.len()) || dentro(a, cb, cb_bytes.len()) {
            return Err("escribe en sus entradas".into());
        }
        if !(dentro(a, regs, 4 * n_regs) || dentro(a, sal, 16 * salidas.max(1)) || (a >= pila && a < STACK_TOP)) {
            return Err(format!("escribe fuera de sus registros, su salida y su pila (en {:#x})", a));
        }
    }
    let palabra = |a: u64| (0..4).fold(0u32, |w, i| w | (m.read_u8_pub(a + i) as u32) << (8 * i));
    let salidas = (0..salidas).map(|s| core::array::from_fn(|k| palabra(sal + 16 * s as u64 + 4 * k as u64))).collect();
    // dos de la llamada que lo rodea: el `call` y el `jmp` del final
    Ok(Corrida { salidas, pasos: m.pasos - antes - 2 })
}

impl Tarjeta for Cpu {
    fn ficha(&self) -> Ficha {
        Ficha { nombre: NOMBRE, lengua: LENGUA, registros: REGISTROS }
    }

    /// **EMITE**: el prologo y el x86-64 de `nativo`; lo mismo para el
    /// oraculo y para el viaje (lo que el oraculo simula es lo que viaja).
    /// `instrucciones` son las que corre UNA celda con sus entradas a cero:
    /// la CPU no tiene un techo de instrucciones, tiene un tiempo.
    fn emitir(&self, p: &Programa, _para: Para) -> Result<Codigo, NoEmite> {
        if let Some((op, por_que)) = no_sabe(p) {
            return Err(NoEmite::Limite { op, que: format!("{} todavia no corre esto", NOMBRE), por_que: por_que.into() });
        }
        if p.iniciales.len() > REGISTROS as usize {
            let op = p.ops.len().saturating_sub(1);
            return Err(NoEmite::Limite { op, que: format!("{} no tiene sitio para tantos valores", NOMBRE), por_que: format!("usa {} registros, y un cuerpo de la CPU guarda {} en la pila de quien lo llama", p.iniciales.len(), REGISTROS) });
        }
        let cuerpo = bmo_proton_x::nativo::compilar(p).ok_or_else(|| NoEmite::Fallo { op: None, por_que: format!("su emisor (`nativo`) no lo tradujo: {}", bmo_proton_x::nativo::por_que_no(p).unwrap_or("sin motivo dicho")) })?;
        let mut bytes = prologo(p);
        bytes.extend_from_slice(&cuerpo);
        let mut c = Codigo { bytes, instrucciones: 0, registros: p.iniciales.len() as u32 };
        let ceros = vec![[0u32; 4]; p.entradas];
        let una = correr(&c, &ceros, p.salidas).map_err(|por_que| NoEmite::Fallo { op: None, por_que: format!("su codigo no cumple el contrato de la llamada: {}", por_que) })?;
        c.instrucciones = una.pasos as usize;
        Ok(c)
    }

    /// **JUZGA**: sus techos, y una celda corrida con el contrato mirado.
    /// Cada celda que simula lo mira otra vez: el oraculo juzga cada camino
    /// que la bateria de bordes recorre.
    fn juzgar(&self, c: &Codigo, _para: Para) -> Result<(), String> {
        if c.bytes.is_empty() {
            return Err("un cuerpo sin bytes".into());
        }
        if c.registros > REGISTROS {
            return Err(format!("{} registros, y un cuerpo de la CPU guarda {}", c.registros, REGISTROS));
        }
        correr(c, &[[0; 4]; 32], 32).map(|_| ())
    }

    /// **SIMULA**: sus bytes, en el emulador de la casa, con el contrato
    /// mirado en cada celda.
    fn simular(&self, c: &Codigo, entradas: &[[u32; 4]], salidas: usize) -> Result<Vec<[u32; 4]>, String> {
        correr(c, entradas, salidas).map(|r| r.salidas)
    }
}

#[cfg(test)]
mod pruebas;
