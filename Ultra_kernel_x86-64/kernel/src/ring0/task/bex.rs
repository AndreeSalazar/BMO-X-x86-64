//! Ring 0 BEX admission gate.
//!
//! [carril]  ROJO      el gate de admision de una imagen BEX
//! [consumo] NADA      corre cuando alguien lanza o admite una tarea
//!
//! This module performs the allocation-free part of loading a BEX image:
//! validate the x86-64/ABI contract and create a fixed-size mapping plan.
//! It deliberately does not execute code.  The process subsystem will later
//! consume this plan to allocate user pages, copy sections and enter Ring 3.

use bmo_bex_gate as gate;

// ** EL CONTRATO NO SE ESCRIBE AQUI: viene de la puerta (`bmo-bex-gate`), una
// sola copia atada por prueba al juez de `bmo-abi`.
pub use gate::{Anexo, Cual, Region, ANEXO_FIRMA, ANEXO_RELOCS, ANEXO_REQUISITOS, FIRMA_INDICE};

/// Lo mas que puede medir el anexo de firma, **segun el formato**: la cabecera
/// son 8 bytes y hay como mucho una entrada de 40 por region con bytes (3) y
/// por anexo (16). Un limite que sale del contrato no hay que subirlo nunca.
pub const MAX_FIRMA: usize = 8 + (3 + gate::MAX_ANEXOS) * 40;

/// Un trozo del fichero que el cargador LEE y no mapea: relocs, firma,
/// requisitos. `bytes == 0` = la imagen no lo trae.
#[derive(Clone, Copy)]
pub struct Tramo {
    pub file_offset: u64,
    pub bytes: u64,
    /// El `que` con el que la firma lo nombra (`0x80 | indice`).
    pub que: u8,
}

const SIN_TRAMO: Tramo = Tramo { file_offset: 0, bytes: 0, que: 0 };

/// **El plan de carga de un BEF2**: lo que hay que MAPEAR y lo que hay que
/// LEER, y nada mas. Sale de la puerta ya comprobado.
///
/// ** B8 (2026-09-20): el plan habla de REGIONES. Hasta B8 la puerta
/// presentaba las regiones de BEF2 "como secciones" para que Ring 0 no
/// cambiara, y el permiso de cada pagina se decidia mirando un `kind` --la
/// propiedad central del formato, *el permiso lo da el HUECO*, vivia en un
/// adaptador. Ahora el permiso lo da `Cual`, que es el hueco.
pub struct BexLoadPlan {
    /// Offset del punto de entrada DENTRO del codigo; no es una direccion.
    pub entrada: u64,
    /// Los componentes XSAVE que el programa declara. La puerta ya rechazo lo
    /// que este kernel no preserva.
    pub xcr0: u64,
    /// Las regiones que existen, en el orden de la cabecera (que es el del
    /// fichero): codigo, constantes, datos, ceros.
    pub regiones: [Region; 4],
    pub cuantas: usize,
    /// Los relocs: se leen, se aplican sobre las regiones ya copiadas, y se
    /// olvidan. Cero paginas en el proceso.
    pub relocs: Tramo,
    /// Los hashes con los que se cierra cada region y cada anexo al aterrizar.
    /// Ver `task/landing.rs`.
    pub firma: Tramo,
    /// Lo que el programa DECLARA que va a pedir (regla 7 de `LA_RAM.md`): se
    /// lee ANTES de reservar el primer marco. Ver `bmo-carga-juicio`.
    pub requisitos: Tramo,
    /// **Cuanto mide el INDICE**: la cabecera y la tabla de anexos, que es lo
    /// que cubre el hash `FIRMA_INDICE` (2026-09-20). Cabe en el prologo por
    /// construccion: la puerta ya rechazo una tabla que no llegara.
    pub indice_bytes: usize,
}

impl BexLoadPlan {
    /// La region `cual`, si la imagen la trae.
    pub fn region(&self, cual: Cual) -> Option<&Region> {
        self.regiones[..self.cuantas].iter().find(|r| r.cual == cual)
    }
    /// El indice en `regiones` de la region `cual`.
    pub fn indice_de(&self, cual: Cual) -> Option<usize> {
        self.regiones[..self.cuantas].iter().position(|r| r.cual == cual)
    }
}

/// **Por que no se admitio.**
///
/// == Dos clases de motivo, y por eso son dos variantes ==
///
/// - `Formato` es lo que dice **la puerta** (`bmo-bex-gate`): la imagen esta mal
///   formada. Ese veredicto es el mismo que da el toolchain al compilar, porque
///   es literalmente el mismo codigo.
/// - Lo demas es lo que **solo el que carga puede saber**: que una seccion no
///   cuadro con su hash al aterrizar. Eso no se puede saber mirando el fichero;
///   se sabe habiendolo traido.
///
/// Antes esto tenia veinte variantes que repetian una a una las del validador de
/// `bmo-abi`. Dos listas de motivos que **tienen** que decir lo mismo son dos
/// listas que un dia dejan de decirlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BexError {
    /// La imagen esta mal formada. Ver [`bmo_bex_gate::Falta`].
    Formato(gate::Falta),
    /// ** UNA SECCION NO CUADRA CON SU HASH.
    ///
    /// La imagen llego ENTERA --el medida cuadra-- y **por dentro no es la que
    /// se escribio**. Es el fallo que ningun contador de bytes puede ver: un
    /// sector que se lee sin error y trae datos de otro sitio da un fichero del
    /// medida correcto y corrupto.
    ///
    /// No lo puede decir la puerta, y por eso vive aqui: la puerta mira **el
    /// fichero**, y esto solo se sabe mirando **lo que aterrizo**. Ver
    /// `task/aterrizaje.rs`.
    HashNoCuadra,
    /// ** EL PROLOGO NO TRAJO LA TABLA DE SECCIONES ENTERA.
    ///
    /// Quien llama puede leer mas bytes y volver a preguntar, que es distinto de
    /// rechazar la imagen. Es la traduccion de `Falta::TablaFueraDeLoLeido` a la
    /// unica accion que tiene sentido para un cargador: **volver a intentarlo con
    /// mas**.
    PrologoCorto,
}

impl BexError {
    /// Una linea corta, en el idioma del sistema. La usan CABINA y el shell.
    ///
    /// ** EL MOTIVO, CON SU NAME. Aqui hubo un `Err(_)` durante meses: trece
    /// motivos distintos entraban por la misma puerta y salian con la frase
    /// *"payload failed BEX admission"*. Un cargador que sabe por que rechaza y
    /// no lo dice obliga a adivinar entre "el fichero llego a medias", "otra
    /// arquitectura" y "el entry cae fuera" -- tres cosas que se arreglan en tres
    /// sitios que no se parecen en nada. Costo una tanda de fotos el 2026-08-09.
    pub fn name(&self) -> &'static str {
        match self {
            BexError::Formato(f) => f.nombre(),
            BexError::HashNoCuadra => "una region NO CUADRA con su hash: la imagen esta corrupta",
            BexError::PrologoCorto => "la tabla de anexos no cabe en el prologo leido",
        }
    }
}

/// **QUE NECESITA DE VERDAD ESTE FICHERO.** Devuelve cuantos bytes hay que leer.
///
/// === El escalon 2 de `LA_RAM.md`, en una funcion ===
///
/// El cargador leia el fichero ENTERO a un estatico de 4 MiB, y despues se
/// quedaba con el codigo y los datos. Con un paquete que lleva un WAD dentro,
/// eso es traerse cinco megabytes de bodega para ejecutar ochocientos kilos.
///
/// ** La pregunta correcta no es *"cuanto mide"* sino **"que necesita"**, y el
/// fichero sabe contestarla: la cabecera de 64 B dice donde esta cada region
/// y la tabla de anexos donde esta cada anexo. De todo eso, el cargador solo
/// toca:
///
/// | | Para que |
/// |---|---|
/// | codigo, constantes, datos | se copian al espacio del proceso |
/// | ceros | no ocupan fichero: se declaran |
/// | RELOCS | se leen, se aplican y se olvidan |
/// | FIRMA | los hashes con los que se comprueba lo anterior |
/// | REQUISITOS | lo que el programa declara, antes de reservar nada |
///
/// Todo lo demas --recursos, simbolos, depuracion, manifiesto, y **cualquier
/// tipo que este kernel no conozca**-- es data para otro. Los recursos se leen
/// en EJECUCION, por `TASK_OP_MI_PAQUETE`, y por su propia puerta: el cargador
/// no tiene por que adelantarlos a RAM para que el programa los pida despues.
///
/// > **La RAM es la zona donde se ejecuta, no la bodega. La bodega es el disco.**
///
/// `Err(PrologoCorto)` si la tabla no cabe en lo que se le paso: quien llama
/// puede leer mas y volver a preguntar, que es distinto de rechazar la imagen.
pub fn necesita(prologo: &[u8]) -> Result<usize, BexError> {
    // Se le pregunta a la puerta, no se recorre la tabla otra vez. `usize::MAX`
    // como medida de fichero porque aqui **no se esta validando la imagen**: se
    // esta preguntando cuanto hay que traer, y los limites de verdad se
    // comprueban en `inspect` con el medida real. Meter aqui un medida inventado
    // rechazaria imagenes buenas por una cuenta que ni siquiera es esta.
    match gate::revisar(prologo, usize::MAX) {
        Ok(rev) => Ok(rev.hasta_donde_hace_falta() as usize),
        // La tabla no cabio en lo leido: quien llama puede traer mas y volver a
        // preguntar. Es la unica falta que se traduce a una ACCION en vez de a
        // un rechazo, y por eso es la unica que se distingue aqui.
        Err(gate::Falta::TablaFueraDeLoLeido) => Err(BexError::PrologoCorto),
        Err(f) => Err(BexError::Formato(f)),
    }
}


/// **El plan de carga de una imagen que todavia no es de fiar.**
///
/// No `alloc`, no disco, no relocs, no tablas de paginas, no salto: solo la
/// decision (la puerta) y el plan encima. Por eso se puede llamar antes de
/// admitir nada.
///
/// ## Los DOS limites, que no son el mismo (2026-08-10)
///
/// - `bytes` es **lo que se leyo**: el prologo (cabecera + tabla de anexos).
/// - `tam_fichero` es **lo que mide el fichero en el disco**, contra el que se
///   comprueban `total` y los limites de TODAS las regiones y anexos,
///   incluidos los que no se leyeron.
///
/// ** Confundirlos convierte una imagen cortada en una imagen valida, que es el
/// fallo que `ImagenIncompleta` existe para cazar.
pub fn inspect(bytes: &[u8], tam_fichero: usize) -> Result<BexLoadPlan, BexError> {
    // ** LA DECISION NO SE TOMA AQUI. Aqui vivian doscientas lineas de
    // comprobaciones que eran las MISMAS que las del contrato, escritas otra
    // vez porque aquel usa `alloc`. Ahora la decision es `bmo-bex-gate`, sin
    // `alloc` y sin dependencias, y este modulo hace lo unico que solo el
    // puede hacer: **el plan**.
    let rev = gate::revisar(bytes, tam_fichero).map_err(BexError::Formato)?;

    const VACIA: Region = Region { cual: Cual::Codigo, file_offset: 0, file_size: 0, mem_size: 0 };
    let mut plan = BexLoadPlan {
        entrada: rev.entrada(),
        xcr0: rev.xcr0(),
        regiones: [VACIA; 4],
        cuantas: 0,
        relocs: SIN_TRAMO,
        firma: SIN_TRAMO,
        requisitos: SIN_TRAMO,
        indice_bytes: gate::CABECERA + rev.cuantos_anexos() * gate::ANEXO,
    };
    for r in rev.regiones() {
        plan.regiones[plan.cuantas] = r;
        plan.cuantas += 1;
    }
    let tramo = |a: Option<Anexo>| match a {
        Some(a) => Tramo { file_offset: a.file_offset, bytes: a.file_size, que: a.que },
        None => SIN_TRAMO,
    };
    plan.relocs = tramo(rev.anexo(ANEXO_RELOCS));
    plan.firma = tramo(rev.anexo(ANEXO_FIRMA));
    plan.requisitos = tramo(rev.anexo(ANEXO_REQUISITOS));
    // Todo otro anexo --recursos, manifiesto, katanas, simbolos, o un tipo que
    // este kernel no conoce-- se valido y NO se lee: data para otro.
    Ok(plan)
}

// Estos leen los RELOCS, que es trabajo del que CARGA y no del que decide --
// por eso se quedan aqui y no bajan al crate de la puerta. (`read_u16` volvio
// con BEF2: el relleno de un reloc mide dos bytes y tiene que ser cero.)

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

/// * UNA RELOCATION, leida del fichero. Ver `SECTION_RELOCS`.
///
/// Medida de una relocation. **Ya no se escribe aqui**: viene de
/// `bmo-bex-gate`, que es el crate que los dos lados comparten.
///
/// === Por que se movio (2026-08-12) ===
///
/// Aqui decia: *"si el struct cambiara de forma, `RELOC_SIZE` y estos offsets
/// son el unico sitio a tocar"*. Y **"el unico sitio a tocar" es la definicion
/// de una duplicacion que se olvida**: mover un campo del struct de `bmo-abi`
/// compila igual, pasa todos los tests del toolchain, y este cargador escribe
/// una direccion equivocada dentro de un proceso. Corrupcion silenciosa, no un
/// fallo.
///
/// El kernel sigue sin importar `bmo-abi` --trae `alloc`-- pero SI importa
/// `bmo-bex-gate`, que no tiene dependencias. Asi que los offsets viven ahi, y
/// `bmo-abi` los clava a su struct con `offset_of!` en
/// `tests/bef_dos_lectores.rs`. De dos verdades a una verdad y una prueba.
pub use gate::RELOC_SIZE;
/// ** En BEF2 hay UN solo tipo de reloc -- "aqui va la direccion de
/// region+offset", ocho bytes -- y por eso ya no hay campo `kind` que
/// comprobar: lo que antes era "rechazar un tipo desconocido" ahora es que el
/// formato no tiene donde escribirlo.

/// Lo que hace falta de un reloc, ya descodificado.
///
/// ** BEF2 (2026-09-19): las dos secciones son REGIONES, con la numeracion de
/// `bmo_abi::bef2::Region` -- 0 codigo, 1 constantes, 2 datos, 3 ceros --, que
/// es la misma con la que la puerta las presenta y la firma las nombra. La
/// tabla cruzada que habia aqui ("esta numeracion NO es la de SECTION_*") se
/// fue con BEF1.
#[derive(Clone, Copy)]
pub struct BexReloc {
    /// La region que se parchea (0 codigo, 1 constantes, 2 datos).
    pub donde_sec: u8,
    /// Offset dentro de esa region.
    pub donde_off: u64,
    /// La region a la que apunta (las cuatro).
    pub destino_sec: u8,
    /// Offset del destino dentro de su region.
    pub destino_off: i64,
}

/// Descodifica el reloc numero `n` de la tabla, o `None` si no cabe o el
/// relleno no es cero (un reloc con basura en el relleno no es un reloc).
pub fn leer_reloc(bytes: &[u8], tabla_off: u64, tabla_size: u64, n: usize) -> Option<BexReloc> {
    let dentro = n.checked_mul(RELOC_SIZE)?;
    if (dentro + RELOC_SIZE) as u64 > tabla_size {
        return None;
    }
    let base = (tabla_off as usize).checked_add(dentro)?;
    if read_u16(bytes, base + gate::reloc::RELLENO)? != 0 {
        return None;
    }
    // Los offsets vienen del crate compartido, no de aqui. Ver `RELOC_SIZE`.
    Some(BexReloc {
        donde_sec: *bytes.get(base + gate::reloc::DONDE)?,
        destino_sec: *bytes.get(base + gate::reloc::DESTINO)?,
        donde_off: read_u32(bytes, base + gate::reloc::OFFSET)? as u64,
        destino_off: read_u64(bytes, base + gate::reloc::ADDEND)? as i64,
    })
}

/// Cuantas relocations hay en la tabla.
pub fn cuantas_relocs(tabla_size: u64) -> usize {
    (tabla_size as usize) / RELOC_SIZE
}

/// Report the currently available BEX admission capability over serial.
pub fn announce() {
    crate::ring0::dev::console::serial_write(
        "[bex] BEF2 x86-64: puerta de admision lista\n",
    );
}
