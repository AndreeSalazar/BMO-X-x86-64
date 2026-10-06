//! **Los recursos de un sombreador, de su parte PSV0** (03-10, N5.1): cada
//! textura, muestreador, cbuffer y UAV con su ESPACIO y sus registros.
//!
//! [carril]  VERDE     lee bytes del contenedor; no toca la maquina
//! [cuesta]  DATO      un espacio mal leido da la textura de otro
//! [riesgo]  ESPEJO    el orden lo pone dxc (y FXC no tiene PSV0); el banco lo
//!                     compara con lo que dice `dxc -dumpbin` de un sombreador
//!                     con tres espacios
//! [consumo] NADA      una vez por sombreador, al crear el PSO
//!
//! `dx.op.createHandle(clase, rangeId, indice, ...)` dice la CLASE, el numero
//! del rango DENTRO de su clase y el REGISTRO absoluto, pero no el espacio:
//! ese esta en los metadatos `dx.resources` (numeros de LLVM, pesados de
//! leer) y, plano, en la parte PSV0 del contenedor:
//!
//! ```text
//!    PSV0      u32 medida de la cabecera, la cabecera, u32 cuantos recursos,
//!              u32 medida de cada uno (16 o 24), y cada uno:
//!              tipo, espacio, primer registro, ultimo registro (, clase, banderas)
//!    el orden  los cbuffers, los muestreadores, los SRV y los UAV, cada clase
//!              en el orden de su rangeId (medido con dxc: ver `pruebas`)
//! ```
//!
//! Hasta el 03-10 la casa ignoraba el espacio y solo admitia t0..t31 y
//! s0..s15: el primer sombreador de Cyberpunk con un recurso mas alla se
//! quedaba sin correr ("un recurso fuera de t0..t31 o s0..s15").

use alloc::vec::Vec;

/// Las clases de `createHandle`.
pub const SRV: u8 = 0;
pub const UAV: u8 = 1;
pub const CBV: u8 = 2;
pub const MUESTREADOR: u8 = 3;

/// **Un recurso declarado**: su clase, su espacio y sus registros (el ultimo
/// incluido; un array sin medida llega hasta `u32::MAX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recurso {
    pub clase: u8,
    pub espacio: u32,
    pub desde: u32,
    pub hasta: u32,
    /// El `PSVResourceType` (3 SRV con tipo, 4 crudo, 5 estructurado...).
    pub tipo: u32,
    /// El `ResourceKind` de DXIL (2 Texture2D... 10 TypedBuffer, 11
    /// RawBuffer, 12 StructuredBuffer); 0 si la PSV0 es la vieja de 16 bytes.
    pub especie: u32,
}

/// `ResourceKind` de los buferes.
pub const BUFER_TIPADO: u32 = 10;
pub const BUFER_CRUDO: u32 = 11;
pub const BUFER_ESTRUCTURADO: u32 = 12;

impl Recurso {
    /// **Como se direcciona, si es un bufer** (N5.3): por su especie, o
    /// sin ella, por su tipo (con tipo, sin especie, no se sabe: textura).
    pub fn modo_de_bufer(&self) -> Option<crate::bufer::Modo> {
        use crate::bufer::Modo;
        match (self.especie, self.tipo) {
            (BUFER_TIPADO, _) => Some(Modo::Tipado),
            (BUFER_CRUDO, _) | (0, 4) => Some(Modo::Crudo),
            (BUFER_ESTRUCTURADO, _) | (0, 5) => Some(Modo::Estructurado),
            _ => None,
        }
    }
}

fn u32_en(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// La clase de `createHandle` de un `PSVResourceType`.
fn clase(tipo: u32) -> Option<u8> {
    Some(match tipo {
        1 => MUESTREADOR,
        2 => CBV,
        3..=5 => SRV,
        6..=9 => UAV,
        _ => return None,
    })
}

/// **Los recursos de una parte PSV0**, en su orden. `None` si no se lee.
pub fn de_psv0(p: &[u8]) -> Option<Vec<Recurso>> {
    let cabecera = u32_en(p, 0)? as usize;
    let mut o = 4 + cabecera;
    let n = u32_en(p, o)? as usize;
    o += 4;
    if n == 0 {
        return Some(Vec::new());
    }
    let paso = u32_en(p, o)? as usize;
    o += 4;
    if paso < 16 || n > 4096 {
        return None;
    }
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let r = o + i * paso;
        // Un tipo que no se conoce (0, "invalido") se guarda igual: cuenta en
        // el orden de su clase... pero sin clase no hay orden. Se salta.
        let tipo = u32_en(p, r)?;
        let Some(clase) = clase(tipo) else { continue };
        let especie = if paso >= 24 { u32_en(p, r + 16)? } else { 0 };
        v.push(Recurso { clase, espacio: u32_en(p, r + 4)?, desde: u32_en(p, r + 8)?, hasta: u32_en(p, r + 12)?, tipo, especie });
    }
    Some(v)
}

/// **Los hilos de un grupo** (N5.5, 05-10): `[numthreads(x, y, z)]` de un
/// sombreador de computo. La PSV0 lo trae desde su `PSVRuntimeInfo2` (36
/// bytes de la cabecera, y luego x, y, z); antes, o en otra etapa (la de la
/// cabecera, en su byte 24, no es 5), [0; 3]. Medido con dxc 1.9:
/// `computo.dxil` da 52 bytes de cabecera, etapa 5 y (64, 1, 1).
pub fn hilos_de_psv0(p: &[u8]) -> [u32; 3] {
    let cabecera = u32_en(p, 0).unwrap_or(0) as usize;
    if cabecera < 48 || p.get(4 + 24) != Some(&5) {
        return [0; 3];
    }
    [u32_en(p, 4 + 36).unwrap_or(0), u32_en(p, 4 + 40).unwrap_or(0), u32_en(p, 4 + 44).unwrap_or(0)]
}

/// **Lo de un sombreador de GEOMETRIA** (E2.3b, 05-10), de su PSV0: el
/// `GSInfo` de la cabecera (`InputPrimitive`, `OutputTopology`) y su
/// `MaxVertexCount` (16 bits en el byte 26). Medido con dxc 1.9 sobre el GS
/// de nBodyGravity: punto (1), tira de triangulos (5), 4 vertices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometria {
    /// D3D_PRIMITIVE de su entrada: 1 punto, 2 linea, 3 triangulo, 6 y 7
    /// linea y triangulo con adyacencia.
    pub entrada: u32,
    /// D3D_PRIMITIVE_TOPOLOGY de su salida: 1 puntos, 3 tira de lineas, 5
    /// tira de triangulos.
    pub salida: u32,
    /// Los vertices que puede emitir, a lo mas (`[maxvertexcount(n)]`).
    pub maximo: u32,
}

impl Geometria {
    /// Los vertices de su primitiva de entrada.
    pub fn vertices(&self) -> usize {
        match self.entrada {
            1 => 1,
            2 => 2,
            3 => 3,
            6 => 4,
            7 => 6,
            _ => 0,
        }
    }
}

/// [`Geometria`] de la PSV0 `p`, si es de un GS (la etapa del byte 24 es 2).
pub fn geometria_de_psv0(p: &[u8]) -> Option<Geometria> {
    let cabecera = u32_en(p, 0)? as usize;
    if cabecera < 36 || p.get(4 + 24) != Some(&2) {
        return None;
    }
    let maximo = u32::from(u16::from_le_bytes([*p.get(4 + 26)?, *p.get(4 + 27)?]));
    Some(Geometria { entrada: u32_en(p, 4)?, salida: u32_en(p, 8)?, maximo })
}

/// **El recurso `rango` de la clase `clase`** (lo que dice `createHandle`).
pub fn rango(recursos: &[Recurso], clase: u8, rango: u32) -> Option<Recurso> {
    recursos.iter().filter(|r| r.clase == clase).nth(rango as usize).copied()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La PSV0 de `prueba/espacios.dxil` (dxc 1.9, ps_6_0): un cbuffer, dos
    /// muestreadores (s20 en space1 y s2) y tres SRV (t3, t40 en space1 y el
    /// array t7..t10 en space2). `dxc -dumpbin` lo dice asi; el orden de los
    /// muestreadores, por rangeId: s20 es el 0 y s2 el 1.
    #[test]
    fn la_psv0_de_dxc_con_tres_espacios() {
        let d = include_bytes!("../../prueba/espacios.dxil");
        let s = crate::dxil::leer(d).unwrap();
        let vistos: alloc::vec::Vec<(u8, u32, u32, u32)> = s.recursos.iter().map(|r| (r.clase, r.espacio, r.desde, r.hasta)).collect();
        assert_eq!(vistos, [(CBV, 0, 0, 0), (MUESTREADOR, 1, 20, 20), (MUESTREADOR, 0, 2, 2), (SRV, 0, 3, 3), (SRV, 1, 40, 40), (SRV, 2, 7, 10)]);
        // Las texturas: SRV con tipo (3), Texture2D (2); ninguna es un bufer.
        assert!(s.recursos.iter().filter(|r| r.clase == SRV).all(|r| (r.tipo, r.especie) == (3, 2) && r.modo_de_bufer().is_none()));
        assert_eq!(rango(&s.recursos, SRV, 2).map(|r| (r.espacio, r.desde)), Some((2, 7)));
        assert_eq!(rango(&s.recursos, MUESTREADOR, 0).map(|r| (r.espacio, r.desde)), Some((1, 20)));
        assert_eq!(rango(&s.recursos, UAV, 0), None);
    }

    #[test]
    fn una_psv0_rota_no_se_lee() {
        assert_eq!(de_psv0(&[]), None);
        assert_eq!(de_psv0(&[4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), Some(Vec::new()), "sin recursos");
        // Dice tener uno de 8 bytes: menos de los 16 que mide.
        assert_eq!(de_psv0(&[0, 0, 0, 0, 1, 0, 0, 0, 8, 0, 0, 0]), None);
    }
}

/// **El paso de cada bufer ESTRUCTURADO** (N5.3b, 05-10), de los metadatos
/// `dx.resources` del modulo: `(uav, espacio, registro, paso)`. La PSV0 no lo
/// trae, y una vista en la RAIZ (`SetGraphicsRootShaderResourceView`) no lo
/// lleva: solo el sombreador sabe cuanto mide su elemento.
///
/// ```text
///    !dx.resources = !{!{SRVs}, !{UAVs}, !{CBVs}, !{samplers}}
///    un SRV  !{id, global, nombre, espacio, registro, cuantos, forma, muestras, extra}
///    un UAV  !{id, global, nombre, espacio, registro, cuantos, forma, coherente,
///              contador, ROV, extra}
///    extra   !{etiqueta, valor, ...}: la 1 es el paso del estructurado
/// ```
///
/// Los numeros son constantes del modulo: `entero` dice cuanto vale el
/// valor `i` (o `None` si no es un entero).
pub(super) fn pasos_estructurados(m: &super::bits::Bloque, entero: impl Fn(usize) -> Option<i64>) -> Vec<(bool, u32, u32, u32)> {
    let md = Metadatos::de(m);
    let mut v = Vec::new();
    let Some(listas) = md.raiz(b"dx.resources") else { return v };
    for (k, uav) in [(0usize, false), (1, true)] {
        let Some(lista) = listas.get(k).and_then(|&r| md.dentro(r)) else { continue };
        for &e in lista {
            let Some(c) = md.dentro(e) else { continue };
            let extra = if uav { 10 } else { 8 };
            let (Some(espacio), Some(registro)) = (c.get(3).and_then(|&x| md.numero(x, &entero)), c.get(4).and_then(|&x| md.numero(x, &entero))) else { continue };
            let Some(par) = c.get(extra).and_then(|&x| md.dentro(x)) else { continue };
            for p in par.chunks_exact(2) {
                if md.numero(p[0], &entero) == Some(1) {
                    if let Some(paso) = md.numero(p[1], &entero) {
                        v.push((uav, espacio as u32, registro as u32, paso as u32));
                    }
                }
            }
        }
    }
    v
}

/// **Las banderas del sombreador** (05-10), de `dx.entryPoints`: la etiqueta
/// 0 de sus propiedades (`!{i32 0, i64 banderas}`); 0 si no las trae. La
/// que importa aqui es [`TEMPRANA`].
///
/// ```text
///    !dx.entryPoints = !{!{funcion, nombre, firmas, recursos, propiedades}}
///    propiedades     !{etiqueta, valor, ...}: la 0, las banderas
/// ```
pub(super) fn banderas(m: &super::bits::Bloque, entero: impl Fn(usize) -> Option<i64>) -> u64 {
    let md = Metadatos::de(m);
    let props = md.raiz(b"dx.entryPoints").and_then(|e| e.get(4)).and_then(|&p| md.dentro(p));
    props.and_then(|p| p.chunks_exact(2).find(|p| md.numero(p[0], &entero) == Some(0)).and_then(|p| md.numero(p[1], &entero))).unwrap_or(0) as u64
}

/// `[earlydepthstencil]` en las banderas de un sombreador de pixeles: la
/// prueba de profundidad ANTES de el, aunque escriba UAV.
pub const TEMPRANA: u64 = 0x8;

/// Un metadato de LLVM 3.7, lo justo para los de arriba.
enum Md {
    Valor(u64),
    Nodo(Vec<u64>),
    Otro,
}

/// **Los metadatos del modulo**, numerados como los numera LLVM: cada
/// registro define uno, salvo el nombre, la clase y el nodo con nombre.
struct Metadatos {
    todos: Vec<Md>,
    /// Cada nodo con nombre (`dx.resources`...) y el PRIMERO al que apunta
    /// (`!dx.entryPoints = !{!10}`: el !10).
    nombrados: Vec<(Vec<u8>, u64)>,
}

impl Metadatos {
    fn de(m: &super::bits::Bloque) -> Metadatos {
        // Los registros de METADATA de LLVM 3.7 que hacen falta aqui.
        const VALOR: u64 = 2;
        const NODO: u64 = 3;
        const NOMBRE: u64 = 4;
        const NODO_DISTINTO: u64 = 5;
        const CLASE: u64 = 6;
        const CON_NOMBRE: u64 = 10;
        let (mut todos, mut nombrados, mut nombre) = (Vec::new(), Vec::new(), None);
        for b in m.bloques.iter().filter(|b| b.id == 15) {
            for r in &b.registros {
                match r.codigo {
                    NOMBRE => nombre = Some(r.ops.iter().map(|&c| c as u8).collect::<Vec<u8>>()),
                    CON_NOMBRE => {
                        if let (Some(n), Some(&k)) = (nombre.take(), r.ops.first()) {
                            nombrados.push((n, k));
                        }
                    }
                    CLASE => {}
                    VALOR => todos.push(Md::Valor(r.ops.get(1).copied().unwrap_or(u64::MAX))),
                    NODO | NODO_DISTINTO => todos.push(Md::Nodo(r.ops.clone())),
                    _ => todos.push(Md::Otro),
                }
            }
        }
        Metadatos { todos, nombrados }
    }

    fn nodo(&self, i: u64) -> Option<&Vec<u64>> {
        match self.todos.get(i as usize) {
            Some(Md::Nodo(v)) => Some(v),
            _ => None,
        }
    }

    /// El nodo al que apunta el nodo con nombre `nombre`.
    fn raiz(&self, nombre: &[u8]) -> Option<&Vec<u64>> {
        self.nombrados.iter().find(|n| n.0 == nombre).and_then(|n| self.nodo(n.1))
    }

    /// Dentro de un nodo, cada referencia es su numero + 1 (0, nada).
    fn dentro(&self, r: u64) -> Option<&Vec<u64>> {
        r.checked_sub(1).and_then(|i| self.nodo(i))
    }

    /// El entero de la referencia `r` (los numeros son constantes del
    /// modulo: `entero` dice cuanto vale el valor `i`).
    fn numero(&self, r: u64, entero: &impl Fn(usize) -> Option<i64>) -> Option<i64> {
        match r.checked_sub(1).and_then(|i| self.todos.get(i as usize)) {
            Some(Md::Valor(v)) => entero(*v as usize),
            _ => None,
        }
    }
}
