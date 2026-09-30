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
//! **La cuenta va por TIRADAS** (30-09): cada region guarda sus trozos
//! hechos `(primera pagina, ultima + 1, proteccion)`, ordenados; lo que no
//! esta en ninguno esta solo reservado. Antes era un u32 POR PAGINA, y en el
//! metal Cyberpunk reservo 64 GiB de golpe: 16 M paginas, 64 MiB de cuenta,
//! y el monton del cargador dijo que no. Ahora la cuenta crece con lo que
//! se HACE, no con lo que se reserva.
//!
//! Lo que no es Windows, dicho: sin la reserva del kernel (P0.4c), RESERVAR
//! ya gasta la memoria (bloques hechos, no direcciones sin paginas). Y
//! reservar en una direccion FIJA no se sabe: la dice quien tiene la
//! memoria, no el `.exe`.

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
    /// Las tiradas HECHAS, en paginas desde `base`: `(desde, hasta, prot)`,
    /// ordenadas, sin solaparse, y dos seguidas con la misma proteccion van
    /// juntas. Lo demas, solo reservado.
    hechas: Vec<(u64, u64, u32)>,
}

impl Region {
    fn paginas(&self) -> u64 {
        self.tam / PAGINA
    }

    /// Las tiradas hechas que caen en `[a, b)` (recortadas).
    fn hechas_en(&self, a: u64, b: u64) -> Vec<(u64, u64, u32)> {
        self.hechas.iter().filter(|t| t.0 < b && t.1 > a).map(|t| (t.0.max(a), t.1.min(b), t.2)).collect()
    }

    /// Los huecos (solo reservados) de `[a, b)`.
    fn huecos_en(&self, a: u64, b: u64) -> Vec<(u64, u64)> {
        let mut v = Vec::new();
        let mut k = a;
        for t in self.hechas.iter().filter(|t| t.0 < b && t.1 > a) {
            if t.0 > k {
                v.push((k, t.0));
            }
            k = k.max(t.1);
        }
        if k < b {
            v.push((k, b));
        }
        v
    }

    /// Quitar `[a, b)` de las hechas (partiendo las que lo crucen).
    fn quitar(&mut self, a: u64, b: u64) {
        let mut v = Vec::with_capacity(self.hechas.len() + 1);
        for &(x, y, p) in &self.hechas {
            if y <= a || x >= b {
                v.push((x, y, p));
                continue;
            }
            if x < a {
                v.push((x, a, p));
            }
            if y > b {
                v.push((b, y, p));
            }
        }
        self.hechas = v;
    }

    /// Poner `[a, b)` hecha con `prot` (lo que hubiera ahi se reemplaza), y
    /// juntar las vecinas iguales.
    fn poner(&mut self, a: u64, b: u64, prot: u32) {
        self.quitar(a, b);
        let i = self.hechas.iter().position(|t| t.0 >= b).unwrap_or(self.hechas.len());
        self.hechas.insert(i, (a, b, prot));
        let mut v: Vec<(u64, u64, u32)> = Vec::with_capacity(self.hechas.len());
        for &t in &self.hechas {
            match v.last_mut() {
                Some(u) if u.1 == t.0 && u.2 == t.2 => u.1 = t.1,
                _ => v.push(t),
            }
        }
        self.hechas = v;
    }
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
        let n = tam / PAGINA;
        let hechas = if hecha && n > 0 { alloc::vec![(0, n, prot)] } else { Vec::new() };
        self.v.push(Region { base, tam, prot_inicial: prot, hechas });
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
        let (a, b) = ((ini - r.base) / PAGINA, (fin - r.base) / PAGINA);
        for (x, y) in r.huecos_en(a, b) {
            if !dar(r.base + x * PAGINA, (y - x) * PAGINA) {
                return Err(NoVirtual::SinMemoria);
            }
            r.poner(x, y, prot);
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
        let (a, b) = ((ini - r.base) / PAGINA, (fin - r.base) / PAGINA);
        // Tiradas seguidas aunque cambie la proteccion: devolver no la mira.
        let mut juntas: Vec<(u64, u64)> = Vec::new();
        for (x, y, _) in r.hechas_en(a, b) {
            match juntas.last_mut() {
                Some(u) if u.1 == x => u.1 = y,
                _ => juntas.push((x, y)),
            }
        }
        for (x, y) in juntas {
            devolver(r.base + x * PAGINA, (y - x) * PAGINA);
        }
        r.quitar(a, b);
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
        let a = (dir - r.base) / PAGINA;
        let (hasta, estado, prot) = match r.hechas.iter().find(|t| t.0 <= a && a < t.1) {
            Some(t) => (t.1, MEM_COMMIT, t.2),
            None => (r.hechas.iter().find(|t| t.0 > a).map_or(r.paginas(), |t| t.0), MEM_RESERVE, 0),
        };
        Some(Consulta { base: r.base + a * PAGINA, base_region: r.base, prot_inicial: r.prot_inicial, tam: (hasta - a) * PAGINA, estado, prot })
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
        let (a, b) = ((ini - r.base) / PAGINA, (fin - r.base) / PAGINA);
        if !r.huecos_en(a, b).is_empty() {
            return Err(NoVirtual::Direccion);
        }
        let antes = r.hechas_en(a, a + 1)[0].2;
        r.poner(a, b, prot);
        Ok(antes)
    }
}
