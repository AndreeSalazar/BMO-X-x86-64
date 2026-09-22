//! **RAM_VERIFY -- que puede hacer con este fichero el que lo cargue.**
//!
//! Idea del propietario, 2026-08-12: *"eso es para ser RAM_verify, para verificar los
//! archivos que se van a aplicar... porque asi las tablas que pusimos son el
//! motivo para cumplir lo que necesita, NO por condicion"*.
//!
//! La PARTE IX de `docs/identidad/LA_RAM.md` dice que herramienta de transporte va en cada
//! sitio. Este modulo la convierte en algo que **se comprueba sobre el fichero**
//! en vez de en un criterio que alguien recuerda.
//!
//! # Que significa "NO por condicion"
//!
//! Que no es un `if` en tiempo de ejecucion que decide y se calla. Es una
//! propiedad del fichero, contestada **antes de escribirlo**, con su motivo
//! cuando la respuesta es que no.
//!
//! La diferencia importa por algo concreto: hoy el cargador ya tiene ramas que
//! caen al camino lento sin decir nada --`disk::tramo_dma` rebota si el destino
//! no esta seguido, `Archivo::leer_de` se cae al camino viejo si el kernel no
//! conoce la op-- y **funcionan**. Pero un sistema que se cae al camino lento en
//! silencio no puede contestar *"por que va lento"*, y esa es la pregunta que
//! este proyecto quiere poder contestar siempre.
//!
//! # [!] ESTO INFORMA, NO RECHAZA. Y es a proposito.
//!
//! En BEF2 (2026-09-19) la pregunta es una sola: **empieza la region en un
//! multiplo de pagina del fichero?** El kernel pone cada region en una
//! pagina nueva, asi que esa es la unica condicion para poder REFLEJAR la
//! pagina del disco en vez de copiarla (`docs/identidad/LA_RAM.md`, PARTE IX).
//! El escritor tiene la palanca (`Escritor::alinear_a_pagina`) y la decision
//! de usarla es la B9 de `docs/plan/PLAN_BEF_NATIVO.md`: cuesta relleno y se
//! mide con DOOM antes de elegir. Mientras tanto, esto mide cuanto se pierde.

use bmo_abi::bef2::{leer, Region};

/// Tamano de pagina. **No se importa del kernel a proposito**: este crate corre
/// en el anfitrion y no puede depender de `Ultra_kernel`. 4096 no se va a mover.
pub const PAGE: u64 = 4096;

/// Como puede viajar una region, segun la PARTE IX de `docs/identidad/LA_RAM.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transporte {
    /// **Herramienta 1 -- no viaja.** El contenido es deducible (ceros), asi que
    /// no hay nada que transportar. Es la mejor de todas y la primera pregunta.
    NoViaja,
    /// **Herramienta 7 -- se puede MAPEAR del disco.** El fichero puede
    /// entregarse sin leerlo: la region empieza en una pagina del fichero, y el
    /// kernel la pone en una pagina de memoria.
    Mapeable,
    /// **Herramienta 2/3 -- hay que COPIARLA.** Que no es un fallo: para poco
    /// dato es lo correcto. Lo que si es un fallo es no saber por que.
    Copia,
}

impl Transporte {
    pub fn nombre(&self) -> &'static str {
        match self {
            Transporte::NoViaja => "no viaja",
            Transporte::Mapeable => "mapeable",
            Transporte::Copia => "copia",
        }
    }
}

/// Una region, con su veredicto.
#[derive(Debug, Clone)]
pub struct Fila {
    pub region: Region,
    pub transporte: Transporte,
    pub file_size: u64,
    pub mem_size: u64,
    pub motivo: String,
}

/// El informe de un `.bex`: cuanto no viaja, cuanto se podria mapear y cuanto
/// hay que copiar.
#[derive(Debug, Clone, Default)]
pub struct InformeRam {
    pub filas: Vec<Fila>,
    pub no_viajan: u64,
    pub mapeables: u64,
    pub copiados: u64,
}

impl InformeRam {
    /// Lo que hoy se lee del disco para arrancar el programa.
    pub fn se_leen_hoy(&self) -> u64 {
        self.mapeables + self.copiados
    }
    /// Lo que dejaria de leerse si el cargador reflejara en vez de copiar.
    pub fn ahorro_si_se_mapea(&self) -> u64 {
        self.mapeables
    }
}

/// Recorre las cuatro regiones de un BEF2 y dice como viajaria cada una. Un
/// fichero que no pasa el juez no produce un informe inventado: sale vacio.
pub fn auditar_ram(bef: &[u8]) -> InformeRam {
    let mut inf = InformeRam::default();
    let Ok(v) = leer(bef) else {
        return inf;
    };
    for r in [Region::Codigo, Region::Constantes, Region::Datos, Region::Ceros] {
        let t = v.tramo(r);
        let (file_size, mem_size) = if matches!(r, Region::Ceros) {
            (0, v.ceros as u64)
        } else {
            (t.bytes as u64, t.bytes as u64)
        };
        if mem_size == 0 {
            continue;
        }
        let (transporte, motivo) = clasificar(r, t.offset as u64, file_size);
        match transporte {
            Transporte::NoViaja => inf.no_viajan += mem_size,
            Transporte::Mapeable => inf.mapeables += file_size,
            Transporte::Copia => inf.copiados += file_size,
        }
        inf.filas.push(Fila { region: r, transporte, file_size, mem_size, motivo });
    }
    inf
}

/// La regla, sola, para poder probarla sin un fichero.
fn clasificar(region: Region, file_offset: u64, file_size: u64) -> (Transporte, String) {
    if matches!(region, Region::Ceros) || file_size == 0 {
        return (Transporte::NoViaja, String::new());
    }
    let resto = file_offset % PAGE;
    if resto != 0 {
        return (
            Transporte::Copia,
            format!(
                "empieza en el byte {} del fichero, {} bytes pasada la pagina: para mapearla tendria que empezar en una",
                file_offset, resto
            ),
        );
    }
    (Transporte::Mapeable, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ** LA HERRAMIENTA 1 GANA SIEMPRE Y VA PRIMERO.
    ///
    /// Los ceros no viajan, y eso no es una optimizacion del transporte: es que
    /// no hay transporte. La PARTE IX lo pone de primera pregunta por esto.
    #[test]
    fn lo_que_no_existe_no_viaja() {
        let (t, m) = clasificar(Region::Ceros, 0, 0);
        assert_eq!(t, Transporte::NoViaja);
        assert!(m.is_empty(), "no viajar no necesita excusa");
    }

    /// ** ALINEADO A 8 NO ES ALINEADO A PAGINA, Y ES EL ERROR FACIL.
    #[test]
    fn alineado_no_es_lo_mismo_que_en_pagina() {
        let (t, m) = clasificar(Region::Codigo, 0x200, 1000);
        assert_eq!(t, Transporte::Copia);
        assert!(m.contains("512"), "el motivo dice cuanto se pasa: {m}");
        let (t, _) = clasificar(Region::Codigo, 0x1000, 1000);
        assert_eq!(t, Transporte::Mapeable);
    }

    /// ** EL ESTADO DE HOY, FIJADO COMO TEST: el escritor pone el codigo justo
    /// detras del prologo, no en una pagina. El dia que `alinear_a_pagina` se
    /// encienda (B9), esto pasa a `Mapeable` y se celebra.
    #[test]
    fn hoy_ningun_bex_es_mapeable_y_este_test_lo_fija() {
        let mut e = bmo_abi::bef2::Escritor::ejecutable();
        e.codigo(vec![0xC3; 64]).constantes(vec![1; 16]);
        let img = e.construir().unwrap();
        let inf = auditar_ram(&img);
        assert!(inf.filas.iter().all(|f| f.transporte == Transporte::Copia));
        assert_eq!(inf.mapeables, 0, "si esto sube es que el escritor alinea: borra este test y celebra");
        // Y con la palanca puesta, las dos regiones se pueden reflejar.
        let mut e = bmo_abi::bef2::Escritor::ejecutable();
        e.codigo(vec![0xC3; 64]).constantes(vec![1; 16]).alinear_a_pagina(true);
        let inf = auditar_ram(&e.construir().unwrap());
        assert!(inf.filas.iter().all(|f| f.transporte == Transporte::Mapeable));
        assert_eq!(inf.mapeables, 80);
    }

    /// El informe suma por clase, y las tres sumas tienen que cuadrar con lo que
    /// se lee del disco. Un informe cuyos totales no cuadran es peor que
    /// ninguno.
    #[test]
    fn las_tres_sumas_cuadran() {
        let mut inf = InformeRam::default();
        inf.no_viajan = 492_784;
        inf.mapeables = 0;
        inf.copiados = 807_072;
        assert_eq!(inf.se_leen_hoy(), 807_072);
        assert_eq!(inf.ahorro_si_se_mapea(), 0, "hoy no se ahorra nada, y por eso se mide");
    }

    /// Un fichero mas corto que su cabecera no produce un informe inventado.
    #[test]
    fn un_fichero_truncado_no_se_inventa() {
        let inf = auditar_ram(&[0u8; 4]);
        assert!(inf.filas.is_empty());
        assert_eq!(inf.se_leen_hoy(), 0);
    }
}
