//! `bmo-verify` -- el gate de verificacion (unico checkpoint comun).
//!
//! Reemplaza el rol de seguridad que tendria un IR central, pero como
//! CONTRATO, no como embudo: cada lenguaje emite su BEF por su cuenta y el
//! verificador lo revisa de forma independiente.
//!
//! ```text
//! BEF (de cualquier lenguaje) -> [bmo-verify] -> pasa?
//!                                               si -> admitido al BMO ABI
//!                                               no -> rechazado (con razones)
//! ```
//!
//! **NO es un stub**: delega en el validador estructural REAL de
//! `bmo_abi::bef2::lector` (cabecera, regiones, anexos, relocs,
//! relocs, firma, flags). Este crate es la CARA de toolchain de ese gate:
//! los frontends llaman `verify()` sin acoplarse a la estructura interna de
//! bmo-abi.
//!
//! # [!] LO QUE ESTE GATE PRUEBA, Y LO QUE NO (corregido 2026-08-12)
//!
//! La cabecera decia: *"si el BEF pasa, esta probado seguro -> puede correr como
//! Software Isolated Process"*. **Eso era falso y hay que decirlo**, porque es
//! justo la clase de frase por la que un dia alguien quita el Ring 3.
//!
//! Lo que este gate comprueba es el **ENVASE**: magic, version, tabla de
//! secciones dentro de rango, relocations que apuntan a sitios que existen, ABI
//! compatible, y --con la firma-- que los bytes son los que se compilaron.
//!
//! Lo que NO comprueba, y no puede: **lo que hacen las instrucciones**. Un
//! `.bex` lleva codigo x86-64 nativo. Ninguna inspeccion del contenedor
//! demuestra que ese codigo no vaya a escribir donde no debe -- para eso haria
//! falta o un lenguaje verificable (el IL tipado de Singularity, el bytecode de
//! WASM) o aislamiento por software al estilo NaCl. BEF no es ninguno de los
//! dos, **a proposito**: es codigo nativo desde la primera instruccion.
//!
//! > **Quien contiene el comportamiento en BMO-X no es este gate: es el Ring 3 y
//! > las capabilities.** El hardware. Este gate contiene la FORMA.
//!
//! Y las dos cosas juntas siguen valiendo mucho -- que un binario no pueda
//! salir del toolchain si el kernel lo va a rechazar es una garantia real. Pero
//! es una garantia de integridad y de contrato, no de seguridad de memoria.

// -- ** RAM_VERIFY: que puede hacer el cargador con este fichero --------------
//
// Fichero aparte porque es otra pregunta. `verify()` contesta *"es admisible?"*;
// esto contesta *"como va a viajar?"*. Comparten el fichero de entrada y nada
// mas -- y la regla de esta casa es que dos preguntas distintas no viven en el
// mismo cajon.
//
// Idea del propietario el 2026-08-12: que las tablas de `docs/identidad/LA_RAM.md` dejen de ser
// criterio que alguien recuerda y pasen a ser algo que se comprueba sobre el
// archivo que se va a aplicar.

/// Como puede viajar cada seccion de un BEF. Ver `docs/identidad/LA_RAM.md`, PARTE IX.
pub mod ram;

// -- ** DECLARACION: que dice este binario de si mismo ------------------------
//
// La tercera pregunta. `verify()` mira el ENVASE y `ram` mira COMO VIAJA; esto
// mira lo que el binario DECLARA. Es opcional a proposito: exigirlo dentro de
// `verify()` rechazaria hoy todo lo que compila BMO C, COBOL y Ada.
pub mod declaracion;

/// Lo que dice el gate de un `.bex`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    Rejected(Vec<String>),
}

impl Verdict {
    pub fn is_ok(&self) -> bool {
        matches!(self, Verdict::Ok)
    }
}

/// **El gate del toolchain**: la puerta del kernel (`bmo-bex-gate`, lo que
/// decidira Ring 0 sobre estos mismos bytes) y despues el juez del contrato
/// (`bef2::leer`, que ademas comprueba cada hash). Los dos tienen que decir
/// que si; el primero que diga que no, lo dice con su nombre.
///
/// ** BEF2 y solo BEF2 desde el 2026-09-19 (B6): BEF1 murio, y un `.bex` con
/// su magic se rechaza aqui igual que en el kernel.
pub fn verify(bef: &[u8]) -> Verdict {
    if let Err(falta) = bmo_bex_gate::revisar(bef, bef.len()) {
        return Verdict::Rejected(vec![String::from(falta.nombre())]);
    }
    match bmo_abi::bef2::leer(bef) {
        Ok(_) => Verdict::Ok,
        Err(f) => Verdict::Rejected(vec![String::from(f.nombre())]),
    }
}

/// Un OBJETO (`.bo`) por su contrato: `bef2::objeto::read`, que es lo que
/// lee `bmo-enlazar`. La puerta del kernel NO se le pregunta: un objeto nunca
/// llega al kernel, y la puerta lo rechazaria (`EsUnObjetoSinEnlazar`).
pub fn verify_object(bef: &[u8]) -> Verdict {
    match bmo_abi::bef2::objeto::read(bef) {
        Ok(_) => Verdict::Ok,
        Err(falta) => Verdict::Rejected(vec![format!("{falta:?}")]),
    }
}

/// Como `verify`, con avisos aparte. BEF2 no tiene avisos: el juez dice si o
/// dice no, y un aviso seria un no que se deja pasar.
pub fn verify_verbose(bef: &[u8]) -> (Verdict, Vec<String>) {
    (verify(bef), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bmo_abi::bef2::Escritor;

    fn minimal_valid_bef() -> Vec<u8> {
        let mut e = Escritor::ejecutable();
        e.codigo(vec![0xC3; 16]).constantes(b"ok\0".to_vec());
        e.construir().unwrap()
    }

    #[test]
    fn accepts_a_real_valid_bef() {
        assert_eq!(verify(&minimal_valid_bef()), Verdict::Ok);
    }

    #[test]
    fn rejects_garbage_with_reasons() {
        let v = verify(&[0u8; 64]); // magic malo
        match v {
            Verdict::Rejected(reasons) => assert!(!reasons.is_empty(), "debe dar razones"),
            Verdict::Ok => panic!("basura no debe pasar el gate"),
        }
    }

    #[test]
    fn rejects_too_small() {
        assert!(!verify(&[0u8; 4]).is_ok());
    }

    /// BEF1 murio: su magic se rechaza como cualquier otro fichero ajeno.
    #[test]
    fn rejects_bef1() {
        let mut b = minimal_valid_bef();
        b[0..4].copy_from_slice(b"BEF1");
        assert!(!verify(&b).is_ok());
    }

    #[test]
    fn un_objeto_es_valido_como_objeto_y_no_como_programa() {
        let mut e = Escritor::objeto();
        e.codigo(vec![0xC3; 16]);
        let bo = e.construir().unwrap();
        assert_eq!(verify_object(&bo), Verdict::Ok);
        assert!(!verify(&bo).is_ok(), "la puerta no carga un objeto");
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Auditoria {
    /// Bytes que el `.bex` lleva en secciones con contenido.
    pub bytes_totales: u64,
    /// De esos, los que estan en secciones a las que apunta ALGO.
    pub bytes_alcanzables: u64,
    /// Secciones con bytes y sin una sola referencia entrante.
    pub secciones_huerfanas: Vec<u8>,
    /// Relocations que apuntan a una seccion que este `.bex` no lleva.
    ///
    /// **Esto si es un error**, y del grave: es el caso en el que el DCE se
    /// llevo algo que hacia falta. Da pantalla negra en el arranque y ni una
    /// linea que lo relacione con el build de hace tres dias.
    pub relocs_al_vacio: usize,
    /// Relocations que caen fuera de los limites de su propia seccion.
    pub relocs_desbordadas: usize,
}

impl Auditoria {
    /// Bytes emitidos que nadie alcanza. **No es un error**: es lo que hay que
    /// mirar cuando un `.bex` crece y no se sabe por que.
    pub fn bytes_muertos(&self) -> u64 {
        self.bytes_totales.saturating_sub(self.bytes_alcanzables)
    }

    /// Hay algo que impide cargar esto. Solo lo roto -- lo que sobra no cuenta.
    pub fn hay_rotura(&self) -> bool {
        self.relocs_al_vacio > 0 || self.relocs_desbordadas > 0
    }
}

/// Audita un `.bex` ya escrito. No lo ejecuta y no lo modifica.
///
/// El recorrido es el mismo que hace el cargador de Ring 0 (`task/proc.rs`) y
/// el arnes del banco de C: se leen las secciones por su tabla y las relocs por
/// la suya. Que los tres lean el mismo formato con tres lectores distintos es
/// una debilidad conocida -- por eso `bmo-abi/tests/abi_layout.rs` fija los
/// offsets a mano.
pub fn auditar(bef: &[u8]) -> Auditoria {
    use bmo_abi::bef2::{leer, Region};

    let mut a = Auditoria::default();
    let Ok(v) = leer(bef) else {
        return a;
    };

    // ** En BEF2 las regiones son TRES con bytes (codigo, constantes, datos) y
    // estan en la cabecera: no hay tabla que recorrer ni dos numeraciones que
    // cruzar. La trampa que esta funcion tenia apuntada --los codigos de las
    // relocs no son los de `SectionKind`, y rodata coincide en las dos-- se fue
    // con la tabla: un reloc nombra una REGION y punto.
    let regiones = [Region::Codigo, Region::Constantes, Region::Datos];
    let mut tam = [0u64; 3];
    let mut existe = [false; 3];
    for (i, r) in regiones.iter().enumerate() {
        let n = v.region(*r).len() as u64;
        tam[i] = n;
        existe[i] = n > 0;
        a.bytes_totales += n;
    }

    // El CODIGO siempre es alcanzable: es por donde se entra. Sin esta linea,
    // un programa sin una sola reloc --que es lo normal-- saldria entero
    // muerto, y un auditor que grita en el caso comun no lo lee nadie.
    let mut alcanzada = [false; 3];
    alcanzada[0] = existe[0];

    for r in v.relocs() {
        let destino = r.destino as usize;
        let donde = r.donde as usize;
        // Los CEROS no tienen bytes: apuntar ahi no alcanza nada que auditar.
        if destino >= 3 || !existe[destino] {
            a.relocs_al_vacio += 1;
            continue;
        }
        alcanzada[destino] = true;
        // Y que el sitio donde se PARCHEA quepa: ocho bytes escritos justo en
        // el borde de una region pisan la siguiente.
        if donde >= 3 || r.offset as u64 + 8 > tam[donde] {
            a.relocs_desbordadas += 1;
        }
    }

    for c in 0..3 {
        if existe[c] {
            if alcanzada[c] {
                a.bytes_alcanzables += tam[c];
            } else {
                a.secciones_huerfanas.push(c as u8);
            }
        }
    }
    a
}
