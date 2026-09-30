//! **Las tablas de `__CxxFrameHandler4`** (30-09): el formato COMPRIMIDO de
//! las excepciones de C++ de MSVC 2019 y despues, el de Cyberpunk. Aqui solo
//! se LEE: se pasa a la forma plana de `cxx.rs`, y el manejador es el mismo
//! que el de FH3.
//!
//! Medido en `tanda4m.exe` (tanda4.cpp con `cl` 19.44, en el Windows del
//! propietario, 30-09):
//!
//! ```text
//!    numero      1 a 5 bytes; los bits bajos del primero dicen cuantos
//!                (x0: 1, x01: 2, x011: 3, x0111: 4, 1111: 5, los 4 de
//!                detras enteros). Un "desplazamiento" son 4 bytes tal cual.
//!    FuncInfo4   una cabecera (catch 1, separada 2, bbt 4, mapa 8, try
//!                0x10), y: [bbt: numero] [mapa: desp] [try: desp]
//!                ip->estado: desp [catch: marco del padre: numero]
//!    mapa        cuantos; cada uno: numero (tipo en los 2 bits bajos, y
//!                los BYTES que hay que ir hacia atras, desde el principio
//!                de esta entrada, hasta la del estado siguiente: llegar al
//!                principio del mapa es el -1); tipo 1 (destructor sobre
//!                un objeto del marco) y 2 (sobre un puntero del marco):
//!                funcion (desp) y objeto (numero); tipo 3: un funclet (desp)
//!    try         cuantos; cada uno: bajo, alto, alto del catch (numeros) y
//!                sus catch (desp)
//!    catch       cuantos; cada uno: cabecera (adjetivos 1, tipo 2, objeto
//!                4, continuaciones como RVA 8, cuantas continuaciones en
//!                los bits 4 y 5), [adjetivos: numero] [tipo: desp]
//!                [objeto: numero] funclet (desp) y 0, 1 o 2 continuaciones
//!                (numero desde el principio de la funcion, o desp)
//!    ip->estado  cuantos; cada uno: ip (numero, lo que avanza desde el
//!                anterior, desde el principio de la funcion) y estado + 1
//! ```
//!
//! Con continuaciones en la tabla, el funclet del catch devuelve el INDICE
//! de la que toca (0 o 1: el mismo funclet sirve a varios catch). Un funclet
//! de catch tiene SUS tablas (`catch`) y dice donde guardo el marco de su
//! funcion (`marco del padre`, desde el suyo).

use alloc::vec::Vec;

use bmo_proton_x::desenrollar::Memoria;

use crate::cxx::{Accion, Catch};
use crate::excepciones::Viva;

/// Un try de FH4, ya leido.
pub(crate) struct Intento4 {
    pub(crate) bajo: i32,
    pub(crate) alto: i32,
    pub(crate) catches: Vec<Catch>,
}

/// Un FuncInfo4, ya leido entero.
pub(crate) struct Fh4 {
    /// Donde esta (para reconocer la misma funcion en otro despacho).
    pub(crate) clave: u64,
    /// Si es el funclet de un catch, donde guardo el marco del padre.
    pub(crate) marco_del_padre: Option<u32>,
    /// El mapa de estados: (estado siguiente, que hacer).
    pub(crate) mapa: Vec<(i32, Accion)>,
    pub(crate) intentos: Vec<Intento4>,
    /// (ip desde el principio de la funcion, estado), en orden.
    ips: Vec<(u64, i32)>,
}

/// Leer, avanzando.
struct Lector<'a> {
    m: &'a Viva,
    d: u64,
}

impl Lector<'_> {
    fn byte(&mut self) -> Option<u8> {
        let b = self.m.u8_en(self.d)?;
        self.d += 1;
        Some(b)
    }

    /// Un numero comprimido.
    fn numero(&mut self) -> Option<u32> {
        let b0 = self.m.u8_en(self.d)?;
        let n = match b0 & 0xF {
            x if x & 1 == 0 => 1,
            x if x & 3 == 1 => 2,
            x if x & 7 == 3 => 3,
            7 => 4,
            _ => 5,
        };
        let v = if n == 5 {
            self.m.u32_en(self.d + 1)?
        } else {
            let mut v = 0u32;
            for k in 0..n {
                v |= (self.m.u8_en(self.d + k)? as u32) << (8 * k);
            }
            v >> n
        };
        self.d += n;
        Some(v)
    }

    /// Un desplazamiento: 4 bytes tal cual.
    fn desp(&mut self) -> Option<i32> {
        let v = self.m.u32_en(self.d)? as i32;
        self.d += 4;
        Some(v)
    }
}

impl Fh4 {
    /// Leer el FuncInfo4 de `d`, de la funcion que empieza en `inicio` (las
    /// continuaciones y los ip van desde ahi).
    pub(crate) fn leer(m: &Viva, base: u64, d: u64, inicio: u64) -> Option<Fh4> {
        let mut l = Lector { m, d };
        let cab = l.byte()?;
        if cab & 2 != 0 {
            // Codigo separado en trozos (BBT, /hotpatch...): otra tabla de
            // ip, que la casa no sabe leer todavia.
            return None;
        }
        if cab & 4 != 0 {
            l.numero()?;
        }
        let mapa = if cab & 8 != 0 { Some(l.desp()?) } else { None };
        let tries = if cab & 0x10 != 0 { Some(l.desp()?) } else { None };
        let ips = l.desp()?;
        let marco_del_padre = if cab & 1 != 0 { Some(l.numero()?) } else { None };
        let rva = |r: i32| base.wrapping_add(r as i64 as u64);
        Some(Fh4 {
            clave: d,
            marco_del_padre,
            mapa: match mapa {
                Some(r) => leer_mapa(m, rva(r), base)?,
                None => Vec::new(),
            },
            intentos: match tries {
                Some(r) => leer_intentos(m, rva(r), base, inicio)?,
                None => Vec::new(),
            },
            ips: leer_ips(m, rva(ips))?,
        })
    }

    /// El estado de `pc` (la funcion empieza en `inicio`): el de la ultima
    /// fila con ip <= pc; antes de la primera, -1.
    pub(crate) fn estado(&self, pc: u64, inicio: u64) -> i32 {
        let pc = pc.wrapping_sub(inicio);
        self.ips.iter().take_while(|(ip, _)| *ip <= pc).last().map_or(-1, |x| x.1)
    }
}

fn leer_mapa(m: &Viva, d: u64, base: u64) -> Option<Vec<(i32, Accion)>> {
    let mut l = Lector { m, d };
    let n = l.numero()?.min(1 << 16);
    // Donde empieza cada entrada (desde el principio del mapa), y lo leido.
    let mut donde = Vec::with_capacity(n as usize);
    let mut crudo = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let aqui = (l.d - d) as u32;
        let v = l.numero()?;
        let accion = match v & 3 {
            1 | 2 => {
                let f = base.wrapping_add(l.desp()? as i64 as u64);
                let objeto = l.numero()?;
                if v & 3 == 1 {
                    Accion::Destructor(f, objeto)
                } else {
                    Accion::DestructorDePuntero(f, objeto)
                }
            }
            3 => Accion::Funclet(base.wrapping_add(l.desp()? as i64 as u64)),
            _ => Accion::Nada,
        };
        donde.push(aqui);
        crudo.push((aqui.checked_sub(v >> 2)?, accion));
    }
    // El desplazamiento hacia atras, a estado: el principio del mapa es -1.
    crudo
        .into_iter()
        .map(|(destino, a)| {
            let s = if destino == 0 { -1 } else { donde.iter().position(|&x| x == destino)? as i32 };
            Some((s, a))
        })
        .collect()
}

fn leer_intentos(m: &Viva, d: u64, base: u64, inicio: u64) -> Option<Vec<Intento4>> {
    let mut l = Lector { m, d };
    let n = l.numero()?.min(1 << 16);
    let mut v = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let bajo = l.numero()? as i32;
        let alto = l.numero()? as i32;
        let _alto_del_catch = l.numero()?;
        let catches = leer_catches(m, base.wrapping_add(l.desp()? as i64 as u64), base, inicio)?;
        v.push(Intento4 { bajo, alto, catches });
    }
    Some(v)
}

fn leer_catches(m: &Viva, d: u64, base: u64, inicio: u64) -> Option<Vec<Catch>> {
    let mut l = Lector { m, d };
    let n = l.numero()?.min(256);
    let mut v = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let cab = l.byte()?;
        let adjetivos = if cab & 1 != 0 { l.numero()? } else { 0 };
        let tipo = if cab & 2 != 0 { base.wrapping_add(l.desp()? as i64 as u64) } else { 0 };
        let objeto = if cab & 4 != 0 { l.numero()? as i32 } else { 0 };
        let funclet = base.wrapping_add(l.desp()? as i64 as u64);
        let cuantas = ((cab >> 4) & 3).min(2) as usize;
        let mut seguir = [0u64; 2];
        for s in seguir.iter_mut().take(cuantas) {
            *s = if cab & 8 != 0 { base.wrapping_add(l.desp()? as i64 as u64) } else { inicio + l.numero()? as u64 };
        }
        v.push(Catch { adjetivos, tipo, objeto, funclet, marco: 0, seguir, cuantas_seguir: cuantas as u8 });
    }
    Some(v)
}

fn leer_ips(m: &Viva, d: u64) -> Option<Vec<(u64, i32)>> {
    let mut l = Lector { m, d };
    let n = l.numero()?.min(1 << 20);
    let mut v = Vec::with_capacity(n as usize);
    let mut ip = 0u64;
    for _ in 0..n {
        ip += l.numero()? as u64;
        v.push((ip, l.numero()? as i32 - 1));
    }
    Some(v)
}
