//! **LI0 -- EL CENSO DE LA 3060** (12-10, `PLAN_EL_LIBRETO.md`).
//!
//! El metal del 12-10: con `smp all` y sin reinicio, Cyberpunk dibujo su
//! primer fotograma -- POR LA CPU, 63,9 s el fotograma. La consola decia una
//! puerta por lote (la PRIMERA que no pasa, y una vez): `SV_Position en el de
//! pixeles`, `un render target de float`, `SV_VertexID`... Eso no dice cual
//! abrir primero. Este censo si:
//!
//! ```text
//!    por cada lote     TODAS las puertas que no pasa (las del lote y las de
//!                      su PSO), no solo la primera
//!    por cada puerta   cuantos PSO distintos para y cuantos LOTES (el peso
//!                      en el fotograma: un PSO de mil lotes pesa mil)
//!    y si se abriera   cuantos lotes irian a la 3060 si esa fuera la UNICA
//!                      que les falta ("solo esta")
//! ```
//!
//! Sin Ring 0 y sin la 3060: la traduccion y el juez corren en la CPU (corre
//! aunque el GSP no despierte). Es una COTA: un lote que pasa todas puede
//! caer despues en el vigia o en el pegado de prueba, y eso lo sigue diciendo
//! la puerta.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bmo_proton_x::lote::{ElementoIa, Enlace, Fuente, Lote};

use crate::pso::{self, NoVa};
use crate::{emitir_con, Abi, NoEmite};

/// Lo de una puerta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cuenta {
    /// PSO distintos que para.
    pub psos: u32,
    /// Lotes que para (con todas sus veces).
    pub lotes: u64,
    /// Lotes a los que SOLO les falta esta.
    pub solo: u64,
}

/// **El censo de una corrida.**
#[derive(Default)]
pub struct Censo {
    pub lotes: u64,
    /// Lotes que pasan todas las puertas del censo.
    pub lotes_3060: u64,
    /// PSO vistos (por su `Enlace`) y lo que les falta a ELLOS.
    psos: BTreeMap<usize, Vec<String>>,
    pub psos_3060: u32,
    puertas: BTreeMap<String, Cuenta>,
    /// Los PSO con mas lotes, con su primera puerta.
    pesos: BTreeMap<usize, u64>,
}

impl Censo {
    pub const fn nuevo() -> Self {
        Censo { lotes: 0, lotes_3060: 0, psos: BTreeMap::new(), psos_3060: 0, puertas: BTreeMap::new(), pesos: BTreeMap::new() }
    }

    /// **Apunta un lote.** `del_destino`: lo que el que dibuja ya sabe que no
    /// pasa (un render target de float, un lote sin pixeles).
    pub fn apuntar(&mut self, l: &Lote, del_destino: &[&'static str], z_a_la_3060: bool) {
        self.lotes += 1;
        let clave = l.enlace as *const Enlace as usize;
        if !self.psos.contains_key(&clave) {
            let m = del_pso(l.enlace, l.entradas, l.paso as u32);
            if m.is_empty() {
                self.psos_3060 += 1;
            }
            for p in &m {
                self.puertas.entry(p.clone()).or_default().psos += 1;
            }
            self.psos.insert(clave, m);
        }
        *self.pesos.entry(clave).or_default() += 1;
        let mut todas: Vec<String> = del_destino.iter().map(|s| String::from(*s)).collect();
        todas.extend(del_lote(l, z_a_la_3060).into_iter().map(String::from));
        todas.extend(self.psos[&clave].iter().cloned());
        todas.sort();
        todas.dedup();
        if todas.is_empty() {
            self.lotes_3060 += 1;
        }
        let solo = todas.len() == 1;
        for p in todas {
            let c = self.puertas.entry(p).or_default();
            c.lotes += 1;
            if solo {
                c.solo += 1;
            }
        }
    }

    /// Las puertas, de la que mas lotes para a la que menos.
    pub fn puertas(&self) -> Vec<(&str, &Cuenta)> {
        let mut v: Vec<(&str, &Cuenta)> = self.puertas.iter().map(|(k, c)| (k.as_str(), c)).collect();
        v.sort_by(|a, b| b.1.lotes.cmp(&a.1.lotes).then(b.1.solo.cmp(&a.1.solo)).then(a.0.cmp(b.0)));
        v
    }

    /// **El resumen**, una linea por puerta (las `n` que mas pesan), para el
    /// DIARIO y la consola.
    pub fn resumen(&self, n: usize) -> String {
        let mut s = format!(
            "# la 3060 (LI0): de {} PSO, {} irian; de {} lotes, {} irian ({}%). Las puertas, por lotes que paran:\n",
            self.psos.len(),
            self.psos_3060,
            self.lotes,
            self.lotes_3060,
            if self.lotes == 0 { 0 } else { self.lotes_3060 * 100 / self.lotes }
        );
        for (p, c) in self.puertas().into_iter().take(n) {
            s.push_str(&format!("#   {:>7} lotes ({:>4} PSO), {:>6} solo por ella: {p}\n", c.lotes, c.psos, c.solo));
        }
        let mut pesados: Vec<(&usize, &u64)> = self.pesos.iter().collect();
        pesados.sort_by(|a, b| b.1.cmp(a.1));
        for (k, lotes) in pesados.into_iter().take(5) {
            let primera = self.psos.get(k).and_then(|m| m.first()).map_or("pasa", |x| x.as_str());
            s.push_str(&format!("#   PSO de {lotes} lotes: {primera}\n"));
        }
        s
    }
}

/// **Las puertas del LOTE** (las mismas de `Puerta::preparar`, todas).
pub fn del_lote(l: &Lote, z_a_la_3060: bool) -> Vec<&'static str> {
    let mut v = Vec::new();
    if l.oclusion {
        v.push("una consulta de oclusion abierta (la 3060 no cuenta pixeles)");
    }
    if l.uavs.is_some() {
        v.push("UAV en sus sombreadores (RWTexture, RWBuffer, Interlocked)");
    }
    if l.enlace.vs.olas_propias() || l.enlace.ps.olas_propias() {
        v.push("las olas (Wave*, Quad*)");
    }
    if l.enlace.vs.deriva() || l.enlace.ps.deriva() {
        v.push("las derivadas (ddx, ddy, fwidth)");
    }
    if (l.enlace.ps.mip_por_derivadas() || l.enlace.ps.lee_con_nivel()) && l.recursos.mip_importa() {
        v.push("una textura de varias mips (la 3060 tiene la mip 0)");
    }
    if l.instancias != 1 || l.entradas.iter().any(|e| e.por_instancia.is_some()) {
        v.push("varias instancias o datos por instancia");
    }
    if !l.reglas.mezcla.trivial() {
        v.push("la mezcla (o escribir solo algunos canales)");
    }
    if l.reglas.stencil.is_some() {
        v.push("el STENCIL");
    }
    if l.reglas.profundidad.is_some() && !z_a_la_3060 {
        v.push("la Z sin la sombra en bloque");
    }
    v
}

/// **Las puertas del PSO**, TODAS (`pso::traducir` para en la primera).
pub fn del_pso(en: &Enlace, ia: &[ElementoIa], paso: u32) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    fn poner_en(v: &mut Vec<String>, s: String) {
        if !v.contains(&s) {
            v.push(s);
        }
    }
    macro_rules! poner {
        ($s:expr) => {
            poner_en(&mut v, $s)
        };
    }
    if en.gs.is_some() {
        poner!("un sombreador de geometria".into());
    }
    for f in &en.desde_ia {
        match *f {
            Fuente::Ia(i) => {
                if let Some(e) = ia.get(i) {
                    if e.ranura != 0 {
                        poner!("un elemento de vertice en otra ranura que la 0".into());
                    }
                    if !bmo_proton_x::formato_ia::es_float32(e.formato) {
                        poner!("un formato de vertice que no es float de 32 bits".into());
                    }
                }
            }
            _ => poner!("un valor de sistema en el de vertice (SV_VertexID, SV_InstanceID)".into()),
        }
    }
    if pso::posicion_zw(en) {
        poner!("SV_Position.z o .w en el de pixeles".into());
    }
    if en.objetivos != [0] {
        poner!("varios render targets (o uno que no es el 0, o SV_Depth)".into());
    }
    let mut emitio = true;
    for (p, que) in [(&en.vs, "vertice"), (&en.ps, "pixel")] {
        let r = if crate::puerta::CON_LIBRETA {
            crate::emitir_libreta(p, 64, Abi::Registros, true).or_else(|_| emitir_con(p, 64, Abi::Registros))
        } else {
            emitir_con(p, 64, Abi::Registros)
        };
        match r {
            Ok(e) => {
                if e.codigo.len() > bmo_gpu_ga10x::sass::juez::MAX_INSTRUCCIONES {
                    poner!(format!("un programa largo (mas de {} instrucciones; el de {que})", bmo_gpu_ga10x::sass::juez::MAX_INSTRUCCIONES));
                }
            }
            Err(e) => {
                emitio = false;
                poner!(motivo_emisor(p, e));
            }
        }
    }
    // Si lo de arriba pasa, lo de la tuberia: el pegamento y el juez.
    if v.is_empty() && emitio {
        if let Err(e) = pso::traducir(en, ia, paso) {
            poner!(match e {
                NoVa::Pegamento(_, bmo_gpu_ga10x::pegamento::NoPega::Instrucciones) => "un programa largo (no cabe con el pegamento)".into(),
                NoVa::Pegamento(_, bmo_gpu_ga10x::pegamento::NoPega::Registros) => "mas registros de los que da la tuberia".into(),
                NoVa::Juez(q, _) => format!("el juez (el de {q})"),
                otro => format!("{otro:?}"),
            });
        }
    }
    v
}

/// El motivo del emisor con NOMBRE: la operacion que no sabe, no su indice.
fn motivo_emisor(p: &crate::Programa, e: NoEmite) -> String {
    match e {
        NoEmite::Operacion(i) => {
            let op = p.ops.get(i).map(|o| format!("{o:?}")).unwrap_or_default();
            let nombre = op.split([' ', '{', '(']).next().unwrap_or("?");
            format!("una operacion que el emisor no sabe: {nombre}")
        }
        NoEmite::Registros => "mas de 64 registros vivos".into(),
        NoEmite::NoSsa(_) => "un registro escrito dos veces (no SSA)".into(),
        NoEmite::Forma(_) => "saltos o bucles que el emisor no estructura".into(),
    }
}
