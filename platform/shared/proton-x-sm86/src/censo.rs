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
    /// **Las puertas JUNTAS** de cada lote (metal 12-10): casi ningun lote de
    /// Cyberpunk lo para una sola, asi que "cuantos solo por ella" no dice que
    /// abrir. Esto si: cada combinacion y sus lotes.
    juntas: BTreeMap<Vec<String>, u64>,
}

impl Censo {
    pub const fn nuevo() -> Self {
        Censo { lotes: 0, lotes_3060: 0, psos: BTreeMap::new(), psos_3060: 0, puertas: BTreeMap::new(), pesos: BTreeMap::new(), juntas: BTreeMap::new() }
    }

    /// **Apunta un lote.** `del_destino`: lo que el que dibuja ya sabe que no
    /// pasa (un render target de float, un lote sin pixeles).
    pub fn apuntar(&mut self, l: &Lote, del_destino: &[&'static str], z_a_la_3060: bool) {
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
        self.contar(todas);
    }

    /// Un lote con TODAS sus puertas (las del destino, del lote y del PSO).
    pub(crate) fn contar(&mut self, mut todas: Vec<String>) {
        self.lotes += 1;
        todas.sort();
        todas.dedup();
        if todas.is_empty() {
            self.lotes_3060 += 1;
        }
        let solo = todas.len() == 1;
        if !todas.is_empty() {
            *self.juntas.entry(todas.clone()).or_default() += 1;
        }
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
        let puertas = self.puertas();
        for (i, (p, c)) in puertas.iter().take(n).enumerate() {
            s.push_str(&format!("#   [{}] {:>7} lotes ({:>4} PSO), {:>6} solo por ella: {p}\n", i + 1, c.lotes, c.psos, c.solo));
        }
        // Las combinaciones, por los numeros de arriba ([+] = una de mas abajo).
        for (lotes, nombres) in self.juntas_que_mas(5) {
            // Por numero (las de mas abajo, `usize::MAX`, al final).
            let mut pos: Vec<usize> = nombres.iter().map(|q| puertas.iter().take(n).position(|(p, _)| *p == q.as_str()).unwrap_or(usize::MAX)).collect();
            pos.sort();
            let idx: Vec<String> = pos.iter().map(|&i| if i == usize::MAX { String::from("[+]") } else { format!("[{}]", i + 1) }).collect();
            s.push_str(&format!("#   juntas: {lotes} lotes se abren con {}\n", idx.join(" + ")));
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

impl Censo {
    /// Las `n` combinaciones de puertas que mas lotes paran.
    pub fn juntas_que_mas(&self, n: usize) -> Vec<(u64, &Vec<String>)> {
        let mut v: Vec<(u64, &Vec<String>)> = self.juntas.iter().map(|(k, l)| (*l, k)).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));
        v.truncate(n);
        v
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
    // L2 (14-10): las del de pixeles ya van; las del de vertices, no.
    if l.enlace.vs.deriva() {
        v.push("derivadas en el sombreador de vertices");
    }
    // G5 (13-10): la de las mips (la de una 2D con nivel ya va: `mipmap`).
    // ** S1 (14-10): y la de un `Sample`, tambien (su lambda, `mipmap`).
    if crate::mipmap::nivel_sin_mips(&l.enlace.ps) && l.recursos.mip_importa() {
        v.push("SampleLevel o Load de un array o una 3D de varias mips");
    }
    // Metal 13-10 17:53: lo que la puerta mira de cada TEXTURA que lee el
    // de pixeles, con su nombre (el censo no lo veia: "es BC, float o
    // sRGB" salia en la consola y no en el censo).
    for q in texturas_del_lote(l) {
        v.push(q);
    }
    if l.instancias != 1 || l.entradas.iter().any(|e| e.por_instancia.is_some()) {
        v.push("varias instancias o datos por instancia");
    }
    // 12-10: la mezcla ya va (en el sombreador); lo que no, por su nombre.
    if let Err(m) = crate::puerta::mezcla_puede(l) {
        v.push(m);
    }
    if l.reglas.stencil.is_some() {
        v.push("el STENCIL");
    }
    if l.reglas.profundidad.is_some() && !z_a_la_3060 {
        v.push("la Z sin la sombra en bloque");
    }
    v
}

/// **Lo que para un lote en sus TEXTURAS** (lo de `puerta::escribir`), por
/// la clase de cada una: lo que la 3060 todavia no lee, para saber CUAL abrir.
fn texturas_del_lote(l: &Lote) -> Vec<&'static str> {
    use bmo_proton_x::dxil::programa::Op;
    use bmo_proton_x::textura::{Clase, Como, Textura};
    let mut v = Vec::new();
    for o in &l.enlace.ps.ops {
        let (t, s, muestrea) = match *o {
            Op::Muestra { t, s, .. } => (t, s, true),
            Op::Lee { t, s, .. } => (t, s, false),
            _ => continue,
        };
        let Some(Some(tx)) = l.recursos.texturas.get(t as usize) else { continue };
        let q = match tx.como {
            // 13-10: BC1-5 sin signo, BC7 y sRGB ya van (`mipmap::formato_de`).
            _ if crate::mipmap::formato_de(tx).is_some() && tx.mapeo & 0xFFF == Textura::MAPEO & 0xFFF && tx.clase == Clase::Plana => {
                if muestrea && !matches!(l.recursos.muestreadores.get(s as usize), Some(Some(_))) {
                    "un muestreador sin atar"
                } else if crate::mipmap::formato_de(tx).is_some_and(|f| f.flotante()) && matches!(l.recursos.muestreadores.get(s as usize), Some(Some(m)) if m.u == bmo_proton_x::textura::Direccion::Borde || m.v == bmo_proton_x::textura::Direccion::Borde) {
                    // F1: las de float ya van; con BORDE, no.
                    crate::puerta::FLOAT_CON_BORDE
                } else if matches!(l.recursos.muestreadores.get(s as usize), Some(Some(m)) if m.lod.min.is_some_and(|f| f != m.filtro)) && tx.niveles_vista() > 1 {
                    // G5/S1: la mip la elige el sombreador con UN filtro.
                    "un muestreador con MIN y MAG distintos sobre una textura de varias mips"
                } else {
                    continue;
                }
            }
            Como::Bloques(_) => "una textura BC6 (HDR) o BC4/BC5 con signo",
            Como::Stencil8 => "el stencil visto como textura",
            _ if tx.mapeo & 0xFFF != Textura::MAPEO & 0xFFF => "una textura con otro mapeo de canales",
            _ if tx.clase != Clase::Plana => "una textura que no es 2D (array, 3D o cubo)",
            _ if muestrea && !matches!(l.recursos.muestreadores.get(s as usize), Some(Some(_))) => "un muestreador sin atar",
            _ => continue,
        };
        if !v.contains(&q) {
            v.push(q);
        }
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
                    // LI2e (12-10): lo que la 3060 lee lo dice ella
                    // (`pegamento::Elemento::con_formato`), como `pso`.
                    match bmo_gpu_ga10x::pegamento::Elemento::con_formato(e.desde, e.formato) {
                        None => poner!("un formato de vertice que la 3060 no lee todavia (R11G11B10_FLOAT)".into()),
                        Some(_) if e.desde % 4 != 0 => poner!("un elemento de vertice que no empieza en 4 bytes".into()),
                        Some(_) => {}
                    }
                }
            }
            // LI2e (12-10): SV_VertexID y SV_InstanceID, de la 3060.
            Fuente::Vertice | Fuente::Instancia => {}
        }
    }
    if pso::posicion_zw(en) {
        poner!("SV_Position.z o .w en el de pixeles".into());
    }
    if let Err(m) = crate::puerta::objetivos_de(en) {
        poner!(m.into());
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
                    poner!(format!("un programa largo ({}; el de {que})", tramo_largo(e.codigo.len())));
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

/// **Cuanto mide un programa largo**, por tramos (metal 12-10: el censo decia
/// "mas de 128" y no si con 256 bastaba o hacian falta miles). Es lo que
/// decide el medida del hueco de LI2b.
pub fn tramo_largo(n: usize) -> String {
    let max = bmo_gpu_ga10x::sass::juez::MAX_INSTRUCCIONES;
    let mut techo = max * 2;
    while techo < n && techo < 8192 {
        techo *= 2;
    }
    if n > techo {
        format!("mas de {techo} instrucciones")
    } else {
        format!("de {} a {techo} instrucciones", techo / 2 + 1)
    }
}

/// El motivo del emisor con NOMBRE: la operacion que no sabe, no su indice.
pub fn motivo_emisor(p: &crate::Programa, e: NoEmite) -> String {
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
