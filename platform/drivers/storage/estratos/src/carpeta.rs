//! **E1: CARPETAS DE CUALQUIER MEDIDA.** La lista de entradas como FLUJO.
//!
//! === Que resuelve ===
//!
//! Una carpeta admitia **36 entradas**: `:entradas` se leia a UN bloque de
//! 4096 bytes y se reescribia en otro, y 4096 / 112 = 36. El formato nunca tuvo
//! ese tope --`Attr::en_bloques` admite cuatro niveles desde el primer dia y el
//! formateador del anfitrion ya escribia carpetas grandes-- pero el kernel no
//! sabia REPUBLICARLAS, asi que el formateador se negaba a escribirlas.
//!
//! Pedido del propietario (25-09): *"puedes convertir mi ESTRATOS pueda guardar
//! TODOOOO esos elementos"*. Sin esto no cabe ni el `valve/` de Half-Life.
//!
//! === Como, sin `alloc` ===
//!
//! La lista NO se carga entera en ningun sitio. Se hace en dos pasadas sobre
//! la lista de hoy, las dos a trozos:
//!
//! ```text
//!   examinar    lee la lista y DECIDE: esta el nombre? cuantas quedan?
//!               -> un Veredicto, y con el lo que va a costar
//!   reescribir  la vuelve a leer y va echando la lista NUEVA a un
//!               flujo::Arbol, entrada a entrada, un bloque de paso
//! ```
//!
//! Dos pasadas y no una porque **se reserva antes de escribir**: la transaccion
//! tiene que saber cuantos bloques cuesta la lista nueva ANTES de poner el
//! primero, y eso depende de lo que diga la lista vieja (si el nombre ya esta,
//! `guardar` no agrega; si no esta, `quitar` no puede). Leer dos veces una
//! carpeta es barato; pedir sitio a mitad de escribir, la maquina de estados
//! no lo deja.
//!
//! === El formato NO cambia ===
//!
//! La lista es el mismo flujo contiguo de entradas de 112 bytes que ya escribia
//! el formateador, partido en trozos de 4096 por `flujo::Arbol`. Como 4096 no
//! es multiplo de 112, **una entrada puede quedar a caballo de dos bloques**, y
//! por eso `recorrer` lleva un resto de hasta 111 bytes de un trozo al
//! siguiente.
//!
//! ** Y una carpeta de 36 o menos sale **byte a byte igual que hoy**: un bloque,
//! sin indireccion (`levels = 0`), la raiz ES el dato. Lo prueba
//! `treinta_y_seis_salen_igual_que_con_el_bloque_de_siempre`.

use crate::flujo::{plan_de, Arbol, Plan};
use crate::objects::{
    Attr, BlockPtr, Entrada, Nodo, Tipo, ATTR_ENTRADAS, BLOQUE, ENTRADA_LEN, NODO_LEN,
};
use crate::read::{descender, Fuente};
use crate::FormatError;

/// **Lo que se le hace a la lista.** Los cinco verbos del kernel, en uno.
#[derive(Clone, Copy, Debug)]
pub enum Cambio<'a> {
    /// Una entrada MAS. Falla si el nombre ya esta.
    Con { nombre: &'a str, nodo: BlockPtr },
    /// Crear o sustituir: si el nombre esta, su entrada apunta a `nodo`; si no,
    /// se agrega al final.
    Guardar { nombre: &'a str, nodo: BlockPtr },
    /// La entrada `nombre` apunta a otro nodo. Es lo que se le hace a cada
    /// carpeta DE PASO de una ruta. Falla si no esta.
    Repuntar { nombre: &'a str, nodo: BlockPtr },
    /// Una entrada MENOS. Falla si no esta.
    Sin { nombre: &'a str },
    /// El mismo nodo con otro nombre, en su mismo sitio.
    Renombrar { viejo: &'a str, nuevo: &'a str },
}

impl Cambio<'_> {
    /// El nombre que se busca en la lista.
    fn buscado(&self) -> &str {
        match self {
            Cambio::Con { nombre, .. }
            | Cambio::Guardar { nombre, .. }
            | Cambio::Repuntar { nombre, .. }
            | Cambio::Sin { nombre } => nombre,
            Cambio::Renombrar { viejo, .. } => viejo,
        }
    }
}

/// **Por que un cambio no se puede hacer.** Cada uno manda a mirar otra cosa.
///
/// ** Antes los tres primeros eran el mismo `BadField`, y el kernel lo decia
/// asi: *"repetido, ausente o ya ocupado"*, sin saber cual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choque {
    /// `Con` de un nombre que ya esta (sin mirar mayusculas).
    Repetido,
    /// `Repuntar`, `Sin` o `Renombrar` de un nombre que no esta.
    NoEsta,
    /// `Renombrar` a un nombre que ya lleva OTRA entrada.
    Ocupado,
    /// El nombre nuevo no vale: vacio o de mas de 63 bytes.
    NombreMalo,
    /// La lista nueva no cabe ni con cuatro niveles. Con un disco de verdad no
    /// pasa: son millones de entradas.
    Llena,
    /// La lista de hoy no se lee: un bloque que no cuadra con su suma, o un
    /// medida que no es un numero entero de entradas.
    Roto(FormatError),
}

/// **Lo que dijo `examinar`**: cuantas habia, cuantas quedan, y cual se toca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Veredicto {
    /// Entradas de la lista de hoy.
    pub antes: u64,
    /// Entradas de la lista nueva.
    pub despues: u64,
    /// La posicion de la entrada que el cambio toca, si toca una.
    cual: Option<u64>,
    /// Se agrega una entrada al final?
    agrega: bool,
}

impl Veredicto {
    /// Bytes de la lista nueva: lo que se declara en el atributo.
    pub fn bytes(&self) -> u64 {
        self.despues * ENTRADA_LEN as u64
    }

    /// El arbol de la lista nueva. `None` = queda vacia y no gasta bloque.
    pub fn plan(&self) -> Option<Plan> {
        if self.despues == 0 {
            None
        } else {
            plan_de(self.bytes())
        }
    }

    /// **Los bloques que cuesta la lista nueva.** Es lo que se reserva.
    ///
    /// Uno hasta 36 entradas, que es lo de siempre; tres de 37 a 72 (dos de
    /// datos y un indice); y asi.
    pub fn bloques(&self) -> u64 {
        self.plan().map_or(0, |p| p.total)
    }
}

/// **Recorre las entradas de una lista, en orden y enteras.**
///
/// `cada` recibe los 112 bytes crudos de cada una y devuelve `false` para
/// parar. Devuelve cuantas entrego.
///
/// ** Una lista que no acaba en una entrada entera es `BadField`, no un final
/// temprano: un listado cortado que pasara por bueno es como se republica una
/// carpeta sin la mitad de sus nombres.
pub fn recorrer(
    src: &mut dyn Fuente,
    lista: Option<&Attr>,
    scratch: &mut [[u8; BLOQUE]],
    cada: &mut dyn FnMut(&[u8; ENTRADA_LEN]) -> bool,
) -> Result<u64, FormatError> {
    let Some(a) = lista else { return Ok(0) };
    let size = a.size;
    if size % ENTRADA_LEN as u64 != 0 {
        return Err(FormatError::BadField);
    }
    if let Some(d) = a.datos_residentes() {
        let mut n = 0u64;
        for e in d.chunks_exact(ENTRADA_LEN) {
            n += 1;
            let e: &[u8; ENTRADA_LEN] = e.try_into().map_err(|_| FormatError::BadField)?;
            if !cada(e) {
                break;
            }
        }
        return Ok(n);
    }
    let raiz = a.raiz().ok_or(FormatError::BadField)?;

    let mut resto = [0u8; ENTRADA_LEN];
    let mut en_resto = 0usize;
    let mut vistos = 0u64;
    let mut n = 0u64;
    let mut parado = false;
    descender(src, &raiz, a.levels, scratch, &mut |trozo| {
        // Solo lo que la lista mide: lo de detras no son entradas.
        let util = (trozo.len() as u64).min(size - vistos) as usize;
        vistos += util as u64;
        let mut t = &trozo[..util];
        while !t.is_empty() {
            if en_resto == 0 && t.len() >= ENTRADA_LEN {
                // Entera dentro del trozo: se entrega sin copiarla.
                let (e, sigue) = t.split_at(ENTRADA_LEN);
                t = sigue;
                n += 1;
                if !cada(e.try_into().expect("112 bytes")) {
                    parado = true;
                    return false;
                }
            } else {
                // A caballo de dos trozos: se junta en el resto.
                let k = (ENTRADA_LEN - en_resto).min(t.len());
                resto[en_resto..en_resto + k].copy_from_slice(&t[..k]);
                en_resto += k;
                t = &t[k..];
                if en_resto == ENTRADA_LEN {
                    en_resto = 0;
                    n += 1;
                    if !cada(&resto) {
                        parado = true;
                        return false;
                    }
                }
            }
        }
        vistos < size
    })?;
    if !parado && (vistos != size || en_resto != 0) {
        return Err(FormatError::BadField);
    }
    Ok(n)
}

/// **Busca `nombre` en la lista**, sin distinguir mayusculas. Para en cuanto
/// lo encuentra.
pub fn buscar(
    src: &mut dyn Fuente,
    lista: Option<&Attr>,
    nombre: &str,
    scratch: &mut [[u8; BLOQUE]],
) -> Result<Option<Entrada>, FormatError> {
    let mut hallada = None;
    let mut mala = None;
    recorrer(src, lista, scratch, &mut |b| match Entrada::decode(b) {
        Ok(e) if e.se_llama(nombre) => {
            hallada = Some(e);
            false
        }
        Ok(_) => true,
        Err(x) => {
            mala = Some(x);
            false
        }
    })?;
    match mala {
        Some(x) => Err(x),
        None => Ok(hallada),
    }
}

/// **Primera pasada: decide si el cambio se puede y cuanto va a costar.**
///
/// No escribe nada. Si contesta `Err`, no se ha pedido ni un bloque.
pub fn examinar(
    src: &mut dyn Fuente,
    lista: Option<&Attr>,
    cambio: &Cambio,
    scratch: &mut [[u8; BLOQUE]],
) -> Result<Veredicto, Choque> {
    // El nombre nuevo se valida ANTES de leer nada.
    match cambio {
        Cambio::Con { nombre, .. } | Cambio::Guardar { nombre, .. } => {
            Entrada::nueva(nombre, BlockPtr::NULO).map_err(|_| Choque::NombreMalo)?;
        }
        Cambio::Renombrar { nuevo, .. } => {
            Entrada::nueva(nuevo, BlockPtr::NULO).map_err(|_| Choque::NombreMalo)?;
        }
        _ => {}
    }
    let buscado = cambio.buscado();
    let mut cual = None;
    let mut choque = None;
    let mut i = 0u64;
    let antes = recorrer(src, lista, scratch, &mut |b| {
        let e = match Entrada::decode(b) {
            Ok(e) => e,
            Err(x) => {
                choque = Some(Choque::Roto(x));
                return false;
            }
        };
        if let Cambio::Renombrar { viejo, nuevo } = cambio {
            if e.se_llama(nuevo) && !e.se_llama(viejo) {
                choque = Some(Choque::Ocupado);
                return false;
            }
        }
        if cual.is_none() && e.se_llama(buscado) {
            cual = Some(i);
            if let Cambio::Con { .. } = cambio {
                choque = Some(Choque::Repetido);
                return false;
            }
        }
        i += 1;
        true
    })
    .map_err(Choque::Roto)?;
    if let Some(c) = choque {
        return Err(c);
    }
    let (despues, agrega) = match cambio {
        Cambio::Con { .. } => (antes + 1, true),
        Cambio::Guardar { .. } => {
            if cual.is_some() {
                (antes, false)
            } else {
                (antes + 1, true)
            }
        }
        Cambio::Repuntar { .. } | Cambio::Renombrar { .. } => {
            cual.ok_or(Choque::NoEsta)?;
            (antes, false)
        }
        Cambio::Sin { .. } => {
            cual.ok_or(Choque::NoEsta)?;
            (antes - 1, false)
        }
    };
    let v = Veredicto { antes, despues, cual: if agrega { None } else { cual }, agrega };
    if despues > 0 && v.plan().is_none() {
        return Err(Choque::Llena);
    }
    Ok(v)
}

/// **Segunda pasada: escribe la lista nueva** a partir del bloque `base`.
///
/// `v` es lo que contesto [`examinar`] sobre esta MISMA lista; los nodos de
/// `cambio` ya son los de verdad. Ocupa exactamente `v.bloques()` bloques y
/// devuelve `(raiz, niveles)`, o `None` si la lista queda vacia.
///
/// ** Si la lista de hoy ya no es la que se examino --otro numero de entradas--
/// se para con `BadField` en vez de escribir una carpeta inventada.
///
/// `indice` son los bloques de indice del arbol (uno por nivel) y `paso` el
/// bloque donde se juntan las entradas antes de salir. `scratch` es el del
/// descenso por la lista vieja: tienen que ser buffers distintos, porque se
/// lee de uno mientras se escribe en los otros.
#[allow(clippy::too_many_arguments)]
pub fn reescribir(
    src: &mut dyn Fuente,
    lista: Option<&Attr>,
    cambio: &Cambio,
    v: &Veredicto,
    base: u64,
    scratch: &mut [[u8; BLOQUE]],
    indice: &mut [[u8; BLOQUE]],
    paso: &mut [u8; BLOQUE],
    poner: &mut dyn FnMut(u64, &[u8]) -> bool,
) -> Result<Option<(BlockPtr, u8)>, FormatError> {
    let Some(plan) = v.plan() else {
        // Vacia: no hay arbol. Pero la lista de hoy tiene que ser la que se
        // examino, o se estaria vaciando una carpeta que no es esa.
        let n = recorrer(src, lista, scratch, &mut |_| true)?;
        return if n == v.antes { Ok(None) } else { Err(FormatError::BadField) };
    };
    let mut arbol = Arbol::nuevo(plan, base, indice)?;
    let mut lleno = 0usize;
    let mut salidas = 0u64;
    let mut fallo: Option<FormatError> = None;

    // Echa una entrada al bloque de paso; si se llena, sale al arbol. Una
    // entrada puede quedar partida entre dos bloques, y asi debe ser: es el
    // mismo flujo contiguo que lee `recorrer`.
    let mut echar = |e: &[u8; ENTRADA_LEN],
                     arbol: &mut Arbol,
                     lleno: &mut usize,
                     poner: &mut dyn FnMut(u64, &[u8]) -> bool|
     -> Result<(), FormatError> {
        let mut t: &[u8] = e;
        while !t.is_empty() {
            let k = (BLOQUE - *lleno).min(t.len());
            paso[*lleno..*lleno + k].copy_from_slice(&t[..k]);
            *lleno += k;
            t = &t[k..];
            if *lleno == BLOQUE {
                arbol.empujar(&paso[..], poner)?;
                *lleno = 0;
            }
        }
        Ok(())
    };

    let mut i = 0u64;
    let leidas = recorrer(src, lista, scratch, &mut |b| {
        let es_esta = v.cual == Some(i);
        i += 1;
        let r = if !es_esta {
            salidas += 1;
            echar(b, &mut arbol, &mut lleno, poner)
        } else {
            let vieja = match Entrada::decode(b) {
                Ok(e) => e,
                Err(x) => {
                    fallo = Some(x);
                    return false;
                }
            };
            let nueva = match cambio {
                Cambio::Sin { .. } => None,
                Cambio::Repuntar { nodo, .. } | Cambio::Guardar { nodo, .. } => {
                    Some(Ok(vieja.con_nodo(*nodo)))
                }
                Cambio::Renombrar { nuevo, .. } => Some(Entrada::nueva(nuevo, vieja.nodo)),
                // `Con` nunca tiene `cual`: examinar lo habria rechazado.
                Cambio::Con { .. } => Some(Err(FormatError::BadField)),
            };
            match nueva {
                None => Ok(()),
                Some(Err(x)) => Err(x),
                Some(Ok(e)) => {
                    salidas += 1;
                    echar(&e.encode(), &mut arbol, &mut lleno, poner)
                }
            }
        };
        match r {
            Ok(()) => true,
            Err(x) => {
                fallo = Some(x);
                false
            }
        }
    })?;
    if let Some(x) = fallo {
        return Err(x);
    }
    if leidas != v.antes {
        return Err(FormatError::BadField);
    }
    if v.agrega {
        let (nombre, nodo) = match cambio {
            Cambio::Con { nombre, nodo } | Cambio::Guardar { nombre, nodo } => (*nombre, *nodo),
            _ => return Err(FormatError::BadField),
        };
        salidas += 1;
        echar(&Entrada::nueva(nombre, nodo)?.encode(), &mut arbol, &mut lleno, poner)?;
    }
    if lleno > 0 {
        arbol.empujar(&paso[..lleno], poner)?;
    }
    if salidas != v.despues {
        return Err(FormatError::BadField);
    }
    Ok(Some((arbol.cerrar(poner)?, plan.niveles)))
}

/// **El nodo de la carpeta** con la lista que acaba de escribir [`reescribir`].
///
/// Vacia, el mismo nodo que una recien nacida: sin `:entradas`.
pub fn nodo_de(lista: Option<(BlockPtr, u8)>, v: &Veredicto) -> Result<[u8; NODO_LEN], FormatError> {
    let n = Nodo::nuevo(Tipo::Directorio);
    match lista {
        None => Ok(n.encode()),
        Some((raiz, niveles)) => {
            Ok(n.con(Attr::en_bloques(ATTR_ENTRADAS, v.bytes(), niveles, raiz)?)?.encode())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escritura::{entradas_con, nodo_de_directorio, ENTRADAS_POR_BLOQUE};
    use std::cell::RefCell;

    /// Un volumen en memoria que crece hacia adelante, como el log.
    struct Disco(RefCell<Vec<[u8; BLOQUE]>>);

    struct Lee<'a>(&'a Disco);

    impl Fuente for Lee<'_> {
        fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
            match self.0 .0.borrow().get(lba as usize) {
                Some(b) => {
                    dst.copy_from_slice(b);
                    true
                }
                None => false,
            }
        }
    }

    impl Disco {
        fn nuevo() -> Self {
            // Los bloques 0 y 1 son los superbloques: nada va ahi.
            Disco(RefCell::new(vec![[0u8; BLOQUE]; 2]))
        }

        fn reservar(&self, n: u64) -> u64 {
            let mut v = self.0.borrow_mut();
            let base = v.len() as u64;
            for _ in 0..n {
                v.push([0u8; BLOQUE]);
            }
            base
        }

        fn escribir(&self, lba: u64, d: &[u8]) -> bool {
            let mut v = self.0.borrow_mut();
            let Some(b) = v.get_mut(lba as usize) else { return false };
            *b = [0u8; BLOQUE];
            b[..d.len()].copy_from_slice(d);
            true
        }

        /// **Lo que hace el kernel**: examinar, reservar, reescribir, y el
        /// nodo nuevo. Devuelve la `:entradas` nueva.
        fn aplicar(&self, lista: Option<&Attr>, cambio: Cambio) -> Result<Option<Attr>, Choque> {
            let mut s = vec![[0u8; BLOQUE]; 6];
            let v = examinar(&mut Lee(self), lista, &cambio, &mut s)?;
            let base = self.reservar(v.bloques());
            let mut indice = vec![[0u8; BLOQUE]; 4];
            let mut paso = [0u8; BLOQUE];
            let r = reescribir(
                &mut Lee(self),
                lista,
                &cambio,
                &v,
                base,
                &mut s,
                &mut indice,
                &mut paso,
                &mut |lba, d| self.escribir(lba, d),
            )
            .map_err(Choque::Roto)?;
            assert_eq!(
                self.0.borrow().len() as u64,
                base + v.bloques(),
                "la lista ocupa exactamente lo reservado"
            );
            let nodo = Nodo::decode(&nodo_de(r, &v).unwrap()).unwrap();
            Ok(nodo.attr(ATTR_ENTRADAS).copied())
        }

        fn nombres(&self, lista: Option<&Attr>) -> Vec<String> {
            let mut s = vec![[0u8; BLOQUE]; 6];
            let mut v = Vec::new();
            recorrer(&mut Lee(self), lista, &mut s, &mut |b| {
                v.push(Entrada::decode(b).unwrap().nombre_str().to_string());
                true
            })
            .unwrap();
            v
        }

        fn buscar(&self, lista: Option<&Attr>, nombre: &str) -> Option<Entrada> {
            let mut s = vec![[0u8; BLOQUE]; 6];
            buscar(&mut Lee(self), lista, nombre, &mut s).unwrap()
        }

        /// Una lista escrita DE UNA VEZ, como la escribe el formateador del
        /// anfitrion: sin pasar por `reescribir`.
        fn de_una_vez(&self, nombres: &[String]) -> Attr {
            let mut cuerpo = Vec::new();
            for (i, n) in nombres.iter().enumerate() {
                cuerpo.extend_from_slice(&Entrada::nueva(n, ptr(i as u64)).unwrap().encode());
            }
            let plan = plan_de(cuerpo.len() as u64).unwrap();
            let base = self.reservar(plan.total);
            let mut indice = vec![[0u8; BLOQUE]; 4];
            let mut a = Arbol::nuevo(plan, base, &mut indice).unwrap();
            let mut poner = |lba: u64, d: &[u8]| self.escribir(lba, d);
            for t in cuerpo.chunks(BLOQUE) {
                a.empujar(t, &mut poner).unwrap();
            }
            let raiz = a.cerrar(&mut poner).unwrap();
            Attr::en_bloques(ATTR_ENTRADAS, cuerpo.len() as u64, plan.niveles, raiz).unwrap()
        }
    }

    fn ptr(n: u64) -> BlockPtr {
        BlockPtr::nuevo(1000 + n, 0, &n.to_le_bytes())
    }

    fn nombre(i: usize) -> String {
        format!("f{i:04}.dat")
    }

    /// Una carpeta de `n` entradas, creada UNA A UNA por el camino del kernel.
    fn carpeta(d: &Disco, n: usize) -> Option<Attr> {
        let mut lista = None;
        for i in 0..n {
            let nom = nombre(i);
            lista = d.aplicar(lista.as_ref(), Cambio::Con { nombre: &nom, nodo: ptr(i as u64) }).unwrap();
        }
        lista
    }

    /// ** LA CASILLA DE E1: MIL ENTRADAS, CREADAS UNA A UNA, LEIDAS ENTERAS.
    #[test]
    fn mil_entradas_creadas_una_a_una_se_leen_enteras() {
        let d = Disco::nuevo();
        let lista = carpeta(&d, 1000);
        let a = lista.unwrap();
        assert_eq!(a.size, 1000 * ENTRADA_LEN as u64);
        assert_eq!(a.levels, 1, "28 bloques de datos bajo un indice");
        let esperado: Vec<String> = (0..1000).map(nombre).collect();
        assert_eq!(d.nombres(Some(&a)), esperado, "todas, en el orden en que llegaron");
        // Y cada una lleva SU nodo: la 36 es la que queda a caballo del
        // primer bloque (bytes 4032..4144).
        for i in [0usize, 35, 36, 37, 72, 999] {
            let e = d.buscar(Some(&a), &nombre(i)).expect("esta");
            assert_eq!(e.nodo, ptr(i as u64), "entrada {i}");
        }
        assert!(d.buscar(Some(&a), "f1000.dat").is_none());
    }

    /// ** Y UNA DE 36 SALE IGUAL QUE HOY, byte a byte: el mismo bloque de
    /// entradas y el mismo nodo de carpeta que `entradas_con` +
    /// `nodo_de_directorio`. Las carpetas viejas no cambian de formato.
    #[test]
    fn treinta_y_seis_salen_igual_que_con_el_bloque_de_siempre() {
        let d = Disco::nuevo();
        let mut viejo = [0u8; BLOQUE];
        let mut usados = 0usize;
        let mut lista = None;
        for i in 0..ENTRADAS_POR_BLOQUE {
            let nom = nombre(i);
            let mut sig = [0u8; BLOQUE];
            usados = entradas_con(&viejo[..usados], &nom, ptr(i as u64), &mut sig).unwrap();
            viejo = sig;
            lista = d.aplicar(lista.as_ref(), Cambio::Con { nombre: &nom, nodo: ptr(i as u64) }).unwrap();
            let a = lista.unwrap();
            assert_eq!(a.levels, 0, "hasta 36, sin indireccion");
            let raiz = a.raiz().unwrap();
            assert_eq!(d.0.borrow()[raiz.lba as usize], viejo, "el bloque de entradas, igual");
            let nodo_hoy = nodo_de_directorio(raiz, usados as u64).unwrap();
            let v = Veredicto { antes: i as u64, despues: i as u64 + 1, cual: None, agrega: true };
            assert_eq!(nodo_de(Some((raiz, 0)), &v).unwrap(), nodo_hoy, "y el nodo, igual");
        }
        // La 37 ya no rebota: pasa a un nivel.
        let nom = nombre(36);
        let a = d.aplicar(lista.as_ref(), Cambio::Con { nombre: &nom, nodo: ptr(36) }).unwrap().unwrap();
        assert_eq!(a.levels, 1);
        assert_eq!(d.nombres(Some(&a)).len(), 37);
    }

    /// Quitar, renombrar y repuntar en una carpeta grande, justo en la entrada
    /// que cruza de bloque. El orden de las demas no se mueve.
    #[test]
    fn quitar_renombrar_y_repuntar_en_una_carpeta_grande() {
        let d = Disco::nuevo();
        let a = d.de_una_vez(&(0..100).map(nombre).collect::<Vec<_>>());

        let a = d.aplicar(Some(&a), Cambio::Sin { nombre: "F0036.DAT" }).unwrap().unwrap();
        let mut esperado: Vec<String> = (0..100).map(nombre).collect();
        esperado.remove(36);
        assert_eq!(d.nombres(Some(&a)), esperado);

        let a = d
            .aplicar(Some(&a), Cambio::Renombrar { viejo: "f0037.dat", nuevo: "Nuevo.txt" })
            .unwrap()
            .unwrap();
        esperado[36] = "Nuevo.txt".into();
        assert_eq!(d.nombres(Some(&a)), esperado, "en su mismo sitio");
        assert_eq!(d.buscar(Some(&a), "nuevo.TXT").unwrap().nodo, ptr(37), "con su nodo");

        let a = d
            .aplicar(Some(&a), Cambio::Repuntar { nombre: "F0099.DAT", nodo: ptr(7777) })
            .unwrap()
            .unwrap();
        let e = d.buscar(Some(&a), "f0099.dat").unwrap();
        assert_eq!(e.nodo, ptr(7777));
        assert_eq!(e.nombre_str(), "f0099.dat", "repuntar no le cambia el nombre");

        let a = d.aplicar(Some(&a), Cambio::Guardar { nombre: "f0001.dat", nodo: ptr(1) }).unwrap().unwrap();
        assert_eq!(d.nombres(Some(&a)).len(), 99, "guardar lo que ya esta no agrega");
        let a = d.aplicar(Some(&a), Cambio::Guardar { nombre: "otro", nodo: ptr(2) }).unwrap().unwrap();
        assert_eq!(d.nombres(Some(&a)).last().unwrap(), "otro", "y lo que no, al final");
    }

    /// Una carpeta que baja de 37 a 36 vuelve al bloque de siempre, y una que
    /// se vacia vuelve a ser una carpeta sin `:entradas`.
    #[test]
    fn al_encoger_vuelve_a_la_forma_de_siempre() {
        let d = Disco::nuevo();
        let a = d.de_una_vez(&(0..37).map(nombre).collect::<Vec<_>>());
        assert_eq!(a.levels, 1);
        let a = d.aplicar(Some(&a), Cambio::Sin { nombre: "f0000.dat" }).unwrap().unwrap();
        assert_eq!(a.levels, 0);
        assert_eq!(d.nombres(Some(&a)).len(), 36);

        let uno = d.de_una_vez(&[nombre(0)]);
        assert!(d.aplicar(Some(&uno), Cambio::Sin { nombre: "f0000.dat" }).unwrap().is_none());
    }

    /// ** Dos niveles de indice: mas de 85 bloques de entradas (3108 entradas).
    /// La carpeta la escribe el formateador de una vez, y el kernel le agrega una.
    #[test]
    fn una_carpeta_de_dos_niveles_tambien_se_republica() {
        let d = Disco::nuevo();
        let todos: Vec<String> = (0..3200).map(nombre).collect();
        let a = d.de_una_vez(&todos);
        assert_eq!(a.levels, 2);
        let a = d.aplicar(Some(&a), Cambio::Con { nombre: "ultima", nodo: ptr(5) }).unwrap().unwrap();
        let n = d.nombres(Some(&a));
        assert_eq!(n.len(), 3201);
        assert_eq!(&n[..3200], &todos[..]);
        assert_eq!(d.buscar(Some(&a), "f3199.dat").unwrap().nodo, ptr(3199));
    }

    #[test]
    fn los_choques_se_dicen_cada_uno_con_su_nombre() {
        let d = Disco::nuevo();
        let a = d.de_una_vez(&(0..50).map(nombre).collect::<Vec<_>>());
        let mut s = vec![[0u8; BLOQUE]; 6];
        let mut ex = |c: Cambio| examinar(&mut Lee(&d), Some(&a), &c, &mut s);
        // Sin distinguir mayusculas: `F0003.DAT` y `f0003.dat` serian dos
        // ficheros y uno de los dos no se podria abrir.
        assert_eq!(ex(Cambio::Con { nombre: "F0003.DAT", nodo: ptr(1) }), Err(Choque::Repetido));
        assert_eq!(ex(Cambio::Sin { nombre: "no-esta" }), Err(Choque::NoEsta));
        assert_eq!(ex(Cambio::Repuntar { nombre: "no-esta", nodo: ptr(1) }), Err(Choque::NoEsta));
        assert_eq!(
            ex(Cambio::Renombrar { viejo: "f0001.dat", nuevo: "F0049.dat" }),
            Err(Choque::Ocupado)
        );
        assert_eq!(ex(Cambio::Con { nombre: "", nodo: ptr(1) }), Err(Choque::NombreMalo));
        let largo = "x".repeat(64);
        assert_eq!(ex(Cambio::Con { nombre: &largo, nodo: ptr(1) }), Err(Choque::NombreMalo));
        // Renombrar a si mismo cambiando mayusculas SI vale.
        assert!(ex(Cambio::Renombrar { viejo: "f0001.dat", nuevo: "F0001.DAT" }).is_ok());
    }

    /// Un bit cambiado en la lista para el recorrido: no se publica nada
    /// encima de una carpeta que no se lee.
    #[test]
    fn una_lista_rota_no_se_republica() {
        let d = Disco::nuevo();
        let a = d.de_una_vez(&(0..80).map(nombre).collect::<Vec<_>>());
        let segundo = 2 + 1; // el segundo bloque de datos
        d.0.borrow_mut()[segundo][10] ^= 1;
        let mut s = vec![[0u8; BLOQUE]; 6];
        let r = examinar(&mut Lee(&d), Some(&a), &Cambio::Con { nombre: "x", nodo: ptr(1) }, &mut s);
        assert_eq!(r, Err(Choque::Roto(FormatError::BadChecksum)));
    }

    /// Un medida que no es un numero entero de entradas es corrupcion, no un
    /// final temprano.
    #[test]
    fn un_medida_suelto_es_corrupcion() {
        let d = Disco::nuevo();
        let a = d.de_una_vez(&(0..40).map(nombre).collect::<Vec<_>>());
        let malo = Attr::en_bloques(ATTR_ENTRADAS, a.size - 5, a.levels, a.raiz().unwrap()).unwrap();
        let mut s = vec![[0u8; BLOQUE]; 6];
        assert_eq!(recorrer(&mut Lee(&d), Some(&malo), &mut s, &mut |_| true), Err(FormatError::BadField));
        // Y una lista que dice medir MAS de lo que tiene, tambien.
        let largo = Attr::en_bloques(ATTR_ENTRADAS, a.size + 112, a.levels, a.raiz().unwrap()).unwrap();
        assert_eq!(recorrer(&mut Lee(&d), Some(&largo), &mut s, &mut |_| true), Err(FormatError::BadField));
    }

    /// Si la lista cambio entre examinar y reescribir, se para.
    #[test]
    fn si_la_lista_ya_no_es_la_examinada_se_para() {
        let d = Disco::nuevo();
        let a = d.de_una_vez(&(0..40).map(nombre).collect::<Vec<_>>());
        let b = d.de_una_vez(&(0..41).map(nombre).collect::<Vec<_>>());
        let mut s = vec![[0u8; BLOQUE]; 6];
        let c = Cambio::Con { nombre: "nuevo", nodo: ptr(1) };
        let v = examinar(&mut Lee(&d), Some(&a), &c, &mut s).unwrap();
        let base = d.reservar(v.bloques());
        let mut indice = vec![[0u8; BLOQUE]; 4];
        let mut paso = [0u8; BLOQUE];
        let r = reescribir(
            &mut Lee(&d),
            Some(&b),
            &c,
            &v,
            base,
            &mut s,
            &mut indice,
            &mut paso,
            &mut |lba, x| d.escribir(lba, x),
        );
        assert_eq!(r, Err(FormatError::BadField));
    }
}
