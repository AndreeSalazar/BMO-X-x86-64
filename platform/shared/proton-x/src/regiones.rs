//! **Las regiones de `VirtualAlloc`** (P4e, 27-09): reservar, hacer
//! (commit), deshacer, soltar, consultar y proteger, pagina a pagina.
//!
//! ```text
//!    VirtualAlloc(NULL, n, RESERVE[|COMMIT])   una region nueva, a 64 KiB
//!    VirtualAlloc(dir, n, COMMIT)              las PAGINAS que tocan
//!                                              [dir, dir+n) se hacen; las
//!                                              que no lo estaban, a CERO
//!    VirtualFree(dir, n, DECOMMIT)             se deshacen (siguen reservadas)
//!    VirtualFree(base, 0, RELEASE)             la region entera se va
//!    VirtualQuery(dir)                         la tirada de paginas iguales
//!    VirtualProtect(dir, n, prot)              solo sobre paginas hechas
//! ```
//!
//! Aqui va la CUENTA: que pagina esta hecha y con que proteccion, y los
//! errores de Windows. De donde sale la memoria lo pone la casa (el monton
//! de P4e, con alineacion de 64 KiB), y poner a cero tambien: cada pagina que
//! se hace se pasa a `a_cero`.
//!
//! Lo que no es Windows, dicho: RESERVAR aqui ya gasta la memoria (el kernel
//! de BMO-X da bloques hechos, no direcciones sin paginas), asi que un
//! programa que reserve 1 GiB "por si acaso" no cabe. Y reservar en una
//! direccion FIJA no se sabe: la dice quien tiene la memoria, no el `.exe`.

use alloc::vec;
use alloc::vec::Vec;

pub const PAGINA: u64 = 4096;
/// `dwAllocationGranularity`: donde empieza una region.
pub const GRANO: u64 = 64 * 1024;

pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_RESERVE: u32 = 0x2000;
pub const MEM_DECOMMIT: u32 = 0x4000;
pub const MEM_RELEASE: u32 = 0x8000;
pub const MEM_FREE: u32 = 0x1_0000;
pub const MEM_PRIVATE: u32 = 0x2_0000;

/// Por que no: `ERROR_INVALID_ADDRESS` (487) o `ERROR_INVALID_PARAMETER` (87).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoVirtual {
    Direccion,
    Parametro,
    /// Quien tiene las paginas no pudo darlas (P0.4c: el kernel dijo que no
    /// hay RAM). ERROR_NOT_ENOUGH_MEMORY.
    SinMemoria,
}

impl NoVirtual {
    pub fn error(self) -> u32 {
        match self {
            NoVirtual::Direccion => 487,
            NoVirtual::Parametro => 87,
            NoVirtual::SinMemoria => 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Region {
    base: u64,
    tam: u64,
    prot_inicial: u32,
    /// Por pagina: 0 si solo esta reservada; si no, su proteccion.
    paginas: Vec<u32>,
}

/// Lo que `VirtualQuery` escribe en su `MEMORY_BASIC_INFORMATION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consulta {
    pub base: u64,
    pub base_region: u64,
    pub prot_inicial: u32,
    pub tam: u64,
    pub estado: u32,
    pub prot: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Regiones {
    v: Vec<Region>,
}

/// Las paginas que tocan `[dir, dir + tam)`: principio y fin.
pub fn paginas(dir: u64, tam: u64) -> (u64, u64) {
    (dir & !(PAGINA - 1), (dir.saturating_add(tam) + PAGINA - 1) & !(PAGINA - 1))
}

impl Regiones {
    pub const fn nuevas() -> Self {
        Regiones { v: Vec::new() }
    }

    /// Una region nueva en `base` (a 64 KiB) de `tam` (a paginas). Hecha
    /// entera con `prot`, o solo reservada. Quien la da ya la puso a cero si
    /// va hecha.
    pub fn nueva(&mut self, base: u64, tam: u64, prot: u32, hecha: bool) {
        let n = (tam / PAGINA) as usize;
        self.v.push(Region { base, tam, prot_inicial: prot, paginas: vec![if hecha { prot } else { 0 }; n] });
    }

    /// La region donde cae `[ini, fin)` entero.
    fn donde(&self, ini: u64, fin: u64) -> Option<usize> {
        self.v.iter().position(|r| ini >= r.base && fin <= r.base + r.tam && ini < fin)
    }

    /// **Hacer** (MEM_COMMIT) las paginas de `[dir, dir + tam)`: las que no
    /// lo estaban pasan por `a_cero` (principio, bytes) y toman `prot`; las
    /// que ya estaban se quedan como estan. La primera pagina.
    pub fn hacer(&mut self, dir: u64, tam: u64, prot: u32, mut a_cero: impl FnMut(u64, u64)) -> Result<u64, NoVirtual> {
        self.hacer_con(dir, tam, prot, |d, n| {
            a_cero(d, n);
            true
        })
    }

    /// **Hacer, pidiendo las paginas** (P0.4c): como [`Regiones::hacer`],
    /// pero `dar(principio, bytes)` puede decir que no (el kernel no tiene
    /// RAM): esa tirada NO se marca y se contesta `SinMemoria`. Las tiradas
    /// de antes SI quedaron hechas (las dio quien las tiene): no es "todo o
    /// nada" como Windows, pero la cuenta y las paginas dicen lo mismo.
    pub fn hacer_con(&mut self, dir: u64, tam: u64, prot: u32, mut dar: impl FnMut(u64, u64) -> bool) -> Result<u64, NoVirtual> {
        if tam == 0 {
            return Err(NoVirtual::Parametro);
        }
        let (ini, fin) = paginas(dir, tam);
        let i = self.donde(ini, fin).ok_or(NoVirtual::Direccion)?;
        let r = &mut self.v[i];
        let (a, b) = (((ini - r.base) / PAGINA) as usize, ((fin - r.base) / PAGINA) as usize);
        let mut k = a;
        while k < b {
            if r.paginas[k] != 0 {
                k += 1;
                continue;
            }
            let desde = k;
            while k < b && r.paginas[k] == 0 {
                k += 1;
            }
            if !dar(r.base + desde as u64 * PAGINA, (k - desde) as u64 * PAGINA) {
                return Err(NoVirtual::SinMemoria);
            }
            r.paginas[desde..k].fill(prot);
        }
        Ok(ini)
    }

    /// **Deshacer** (MEM_DECOMMIT). `tam` 0 desde la base: la region entera.
    pub fn deshacer(&mut self, dir: u64, tam: u64) -> Result<(), NoVirtual> {
        self.deshacer_con(dir, tam, |_, _| {})
    }

    /// **Deshacer, devolviendo las paginas** (P0.4c): `devolver(principio,
    /// bytes)` por cada tirada que SI estaba hecha, antes de marcarla.
    pub fn deshacer_con(&mut self, dir: u64, tam: u64, mut devolver: impl FnMut(u64, u64)) -> Result<(), NoVirtual> {
        let (ini, fin) = if tam == 0 {
            let r = self.v.iter().find(|r| r.base == dir).ok_or(NoVirtual::Parametro)?;
            (r.base, r.base + r.tam)
        } else {
            paginas(dir, tam)
        };
        let i = self.donde(ini, fin).ok_or(NoVirtual::Direccion)?;
        let r = &mut self.v[i];
        let (a, b) = (((ini - r.base) / PAGINA) as usize, ((fin - r.base) / PAGINA) as usize);
        let mut k = a;
        while k < b {
            if r.paginas[k] == 0 {
                k += 1;
                continue;
            }
            let desde = k;
            while k < b && r.paginas[k] != 0 {
                k += 1;
            }
            devolver(r.base + desde as u64 * PAGINA, (k - desde) as u64 * PAGINA);
        }
        r.paginas[a..b].fill(0);
        Ok(())
    }

    /// **Soltar** (MEM_RELEASE): `dir` es la base de una region y `tam` es
    /// 0, como pide Windows. La medida que tenia.
    pub fn soltar(&mut self, dir: u64, tam: u64) -> Result<u64, NoVirtual> {
        if tam != 0 {
            return Err(NoVirtual::Parametro);
        }
        let i = self.v.iter().position(|r| r.base == dir).ok_or(NoVirtual::Parametro)?;
        Ok(self.v.swap_remove(i).tam)
    }

    /// **`VirtualQuery`**: desde la pagina de `dir`, la tirada de paginas en
    /// el mismo estado y con la misma proteccion. `None` si `dir` no es de
    /// ninguna region.
    pub fn consultar(&self, dir: u64) -> Option<Consulta> {
        let r = self.v.iter().find(|r| dir >= r.base && dir < r.base + r.tam)?;
        let a = ((dir - r.base) / PAGINA) as usize;
        let p = r.paginas[a];
        let b = r.paginas[a..].iter().position(|&x| x != p).map_or(r.paginas.len(), |n| a + n);
        Some(Consulta {
            base: r.base + a as u64 * PAGINA,
            base_region: r.base,
            prot_inicial: r.prot_inicial,
            tam: (b - a) as u64 * PAGINA,
            estado: if p == 0 { MEM_RESERVE } else { MEM_COMMIT },
            prot: p,
        })
    }

    /// **`VirtualProtect`**: todas las paginas de `[dir, dir + tam)` hechas
    /// (si no, ERROR_INVALID_ADDRESS, como Windows). La proteccion que tenia
    /// la primera.
    pub fn proteger(&mut self, dir: u64, tam: u64, prot: u32) -> Result<u32, NoVirtual> {
        if tam == 0 || prot == 0 {
            return Err(NoVirtual::Parametro);
        }
        let (ini, fin) = paginas(dir, tam);
        let i = self.donde(ini, fin).ok_or(NoVirtual::Direccion)?;
        let r = &mut self.v[i];
        let (a, b) = (((ini - r.base) / PAGINA) as usize, ((fin - r.base) / PAGINA) as usize);
        if r.paginas[a..b].contains(&0) {
            return Err(NoVirtual::Direccion);
        }
        let antes = r.paginas[a];
        r.paginas[a..b].fill(prot);
        Ok(antes)
    }
}
