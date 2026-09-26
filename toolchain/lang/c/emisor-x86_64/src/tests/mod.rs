//! **El banco de pruebas de BMO C.**
//!
//! Estaba entero dentro de `lib.rs`: **2520 de sus 2628 lineas eran este
//! modulo**, y encontrar un test era buscar por nombre en un fichero donde
//! ya no cabia nada mas. Aqui viven los AYUDANTES --los que compilan y
//! ejecutan-- y cada tema tiene su fichero.
//!
//! No se movio a `tests/` de integracion a proposito: alli solo se ve la API
//! publica, y la mitad de estas pruebas miran el AST o el preprocesador por
//! dentro. Un banco que solo puede probar lo publico no puede probar lo que
//! mas se rompe.
//!
//! ## El reparto se TERMINO el 2026-08-06
//!
//! El trabajo estaba a medias: habia nueve ficheros por tema al lado y este
//! `mod.rs` seguia quedandose con **112 tests sueltos en 1784 lineas**. Un
//! fallo decia su nombre y ya; de que iba, a buscarlo. Ahora **el fichero es
//! la categoria** y `cargo test printf::` corre esa parte sola.
//!
//! Aqui dentro no queda ni un `#[test]`: solo los NUEVE AYUDANTES, que es lo
//! unico que este fichero deberia haber tenido nunca.
//!
//! | | |
//! |---|---|
//! | El lenguaje se reconoce | `parseo` 28 - `estructuras` 16 - `enumeraciones` 3 |
//! | El programa CORRE | `ejecucion` 6 - `flotante` 10 - `globales` 3 - `matriz` 5 |
//! | El sistema debajo | `syscalls` 6 - `intrinsecos` 8 - `puerta` - `cargador` 5 |
//! | Lo de siempre | `printf` 11 - `punteros_funcion` 8 - `preprocesador` |
//!
//! Y los que ya estaban: `agregados`, `almacenamiento`, `entrada`,
//! `inicializadores`, `memoria`, `semantic`, `silencios`.
//!
//! 238 verdes antes del corte, 238 despues. No se reescribio ni un test.

use super::*;

mod agregados;
mod almacenamiento;
mod cadenas;
mod cargador;
mod cotejo_de_disposicion;
mod doom_probe;
mod ejecucion;
mod entrada;
mod enumeraciones;
mod estructuras;
mod flotante;
mod globales;
mod inicializadores;
/// Las de `<string.h>`, `<ctype.h>` y `<stdlib.h>` que faltaban para que el
/// unity build de DOOM llegue al final. Escritas en C, no en el codegen.
mod libc;
mod intrinsecos;
mod matriz;
mod memoria;
/// El MONTON de Ring 3 (`<bmo/monton.h>`): `malloc` de verdad sobre UN bloque
/// de `KIND_MEMORIA`. `memoria` prueba el contrato del kernel; esto, el reparto
/// que se escribe encima.
mod monton;
/// `<bmo/musica.h>`: notas, figuras y tempo sobre `KIND_AUDIO`. Aqui no suena
/// nada -- se comprueba la PARTITURA, que es lo unico que una libreria de
/// musica puede prometer sin un altavoz delante.
mod musica;
mod voces;
mod parseo;
mod simbolos;
mod sonda_param_array;
mod sonda_resta_de_punteros;
mod flotante_por_lugar;
mod nunca_adivina;
mod literales_y_sufijos;
mod sonda_columnas_de_doom;
mod sonda_planos_de_doom;
mod sonda_visplanes_de_doom;
mod sonda_abs_de_doom;
mod sonda_planos_altos_de_doom;
mod sonda_escala_de_muro;
mod sonda_marcar_planos;
mod sonda_extern_de_doom;
mod sonda_segloop_entero;
mod sonda_enum_tapa_local;
mod bandera_de_pantalla;
mod sonda_layout_sha1;
mod sonda_sha1;
mod preprocesador;
mod printf;
mod puerta;
mod punteros_funcion;
mod semantic;
mod silencios;
mod sombras;
mod sin_pila;
mod tres_operandos;
mod bucles;
mod argumentos;
mod convencion;
/// Las funciones SINTETIZADAS: emitidas una vez, alcanzadas con `call`. Aqui
/// se cuenta **cuantas veces sale el cuerpo**, que es lo que un test de
/// comportamiento no puede ver.
mod sintetizadas;
/// La tabla v1 (0x100..0x1FF) ya no existe: un nombre suyo no compila y un
/// `use` sin modulo tampoco. Antes los dos compilaban hacia `rax = 10`.
mod tabla_v1;
/// La tabla de configuracion de DOOM en ocho lineas: la forma exacta con la
/// que murio en el Ryzen el 2026-08-13.
mod tabla_de_config;
/// El operador de negacion que valia -256, y por que DOOM iba a escribir su
/// configuracion encima del WAD.
mod argv_de_doom;
mod escalado_de_doom;
mod varargs_de_doom;
// == THE CENSUS FAMILY ==============================================
//
// Ten axes, 143 cells, half a second: `cargo test -p bmo-c-x86-64 probe_`.
// This is the answer to "what does BMO C support", and it CANNOT go stale --
// each probe compares its whole report against a written constant, so fixing a
// BROKEN or breaking a GOOD fails the test until the census is updated.
//
// The axes are ENUMERATED (the product of two lists), not invented, and the
// rows come from DOOM's source rather than from reading the standard.

/// The census HARNESS: sweep a matrix of cells and compare the whole report.
/// Shared by all seven probes below.
mod census;
/// L2 de PLAN_LA_LUDOTECA: lo que Quake pide a la coma flotante, ejecutado.
mod sonda_quake;
mod serie_de_math_h;
/// CONTAINER x OPERATION -- the census of what BMO C can do. 28 cells, green.
mod probe_language;
/// LAYOUT -- where each field falls and how big the aggregate is, measured
/// against the WAD format, the only outside authority this compiler has. The
/// axis `R_Init` and `P_Init` are about to step on. Found 9 broken of 12.
mod probe_layout;
/// WIDTH -- narrowing, widening, promoting. The axis of DOOM's `SHORT(x)`,
/// which is `(signed short)` and which every WAD field goes through. Clean.
mod probe_widths;
/// TABLES -- whether the bytes the compiler puts in the `.bex` are the ones the
/// source said. The axis of `tables.c` and `info.c`. Clean.
mod probe_tables;
/// SIGNEDNESS -- the four operations that ask about the top bit. The axis of
/// `angle_t`, and where the width of `rax` hid the defect at 32 bits. Found 4.
mod probe_signedness;
/// CONTROL FLOW -- switch, goto and recursion. The axis of the playsim's 85
/// `switch` and of `R_RenderBSPNode`, which recurses down two branches. Clean.
mod probe_control_flow;
/// ASSIGNMENT -- the shorthand forms. Found the double evaluation of the lvalue
/// in `a[i++] += 1`; closed 2026-08-13 with a new AST node. 16/16.
mod probe_assignment;
/// STRINGS -- the eight bytes of a lump name. `strncasecmp` and `strncpy(8)`
/// are every lookup in the WAD, so this axis gates `R_Init` onwards.
mod probe_strings;
/// THE HEAP -- the one block everything else is carved out of. DOOM calls
/// `malloc` ONCE, for six megabytes, and its 94 `Z_Malloc` calls live inside.
mod probe_heap;
/// THE KEYMAP -- the sparse table DOOM's input rides on: 128 slots, twenty
/// written by designator, out of order, with values above 127, read with an
/// index that only exists at run time. A slot that answers zero drops the key.
mod probe_keymap;
/// FILE I/O -- **where the bytes land**. The axis that killed DOOM on
/// 2026-08-13: `fread` into a stack buffer returned zero without writing, so
/// the WAD header was garbage and DOOM said its own WAD was not a WAD.
mod probe_file_io;
// ** N0c: la superficie en INTI contra la de C, byte a byte.
mod gemelos_inti;
// El OBJETO (.bo): E2 de PLAN_EL_ENLAZADOR.
mod objeto;

// -- Banco de pruebas: EJECUTAR el programa, no mirarlo --------------
//
// Mismo criterio que en COBOL: un formateo que produce digitos erroneos
// se ve perfectamente sano en un volcado de bytes.

/// Compila y ejecuta un programa C, devolviendo lo que el kernel habria
/// pintado.
fn run_c(source: &str) -> String {
    let bef = compile_source_to_bef(source).expect("el programa debe compilar");
    ejecutar_bef(&bef)
}

/// Igual, pero pasando ANTES por el preprocesador -- que es lo que hace la
/// linea de ordenes y lo que el camino de biblioteca NO hace.
fn run_c_con_pp(source: &str) -> String {
    let bef = compile_with_preprocessor(source, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("con preprocesador debe compilar");
    ejecutar_bef(&bef)
}

/// Compila con preprocesador y ejecuta SEMBRANDO la maquina antes.
///
/// Hace falta desde que C puede emitir la puerta: un programa que lee el
/// raton necesita que haya un raton que leer. Sin esto, todo lo que use
/// `<bmo/entrada.h>` se probaria contra ceros, que es indistinguible de
/// un driver muerto.
fn run_c_sembrado(source: &str, sembrar: impl FnOnce(&mut bmo_lower::emu::Machine)) -> String {
    let bef = compile_with_preprocessor(source, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("con preprocesador debe compilar");
    ejecutar_bef_con(&bef, sembrar)
}

fn ejecutar_bef(bef: &[u8]) -> String {
    ejecutar_bef_con(bef, |_| {})
}

/// Igual que [`run_c`], pero devuelve **la maquina entera**.
///
/// Para lo que un programa no puede contarse a si mismo: cuanta memoria le
/// entrego el kernel, que llamadas cruzaron la puerta y en que orden. Un
/// programa que imprime "todo bien" es un testigo, no una prueba.
fn run_c_maquina(source: &str) -> bmo_lower::emu::Machine {
    let bef = compile_source_to_bef(source).expect("el programa debe compilar");
    maquina_de_bef(&bef)
}

fn ejecutar_bef_con(
    bef: &[u8],
    sembrar: impl FnOnce(&mut bmo_lower::emu::Machine),
) -> String {
    maquina_de_bef_con(bef, sembrar).console
}

fn maquina_de_bef(bef: &[u8]) -> bmo_lower::emu::Machine {
    maquina_de_bef_con(bef, |_| {})
}

fn maquina_de_bef_con(
    bef: &[u8],
    sembrar: impl FnOnce(&mut bmo_lower::emu::Machine),
) -> bmo_lower::emu::Machine {
    use bmo_abi::bef2::{leer, Region};
    use bmo_lower::emu::{run, Machine};

    // ** BEF2 (2026-09-19). Antes esto recorria la tabla de secciones a mano;
    // ahora la cabecera dice las cuatro regiones y el juez las comprueba, asi
    // que el arnes solo tiene que COLOCARLAS -- y colocarlas como el cargador:
    // cada una en su propia pagina, en el orden codigo, constantes, datos,
    // ceros.
    let v = leer(bef).expect("el .bex que sale del compilador tiene que ser valido");

    const PAGE: usize = 4096;
    let mut imagen: Vec<u8> = Vec::new();
    // Donde acabo cada region, por su numero (0 codigo, 1 constantes, 2 datos,
    // 3 ceros). Es lo que necesitan los relocs.
    let mut base = [usize::MAX; 4];
    // ** Codigo y constantes, SOLO LECTURA: el kernel los mapea RX y R+NX
    // (2026-09-19), asi que un programa que escriba en una cadena literal
    // tiene que reventar AQUI y no en el Ryzen.
    let mut solo_lectura = Vec::new();

    for (n, r) in [Region::Codigo, Region::Constantes, Region::Datos, Region::Ceros]
        .iter()
        .enumerate()
    {
        let bytes = v.region(*r);
        let ceros = if matches!(r, Region::Ceros) { v.ceros as usize } else { 0 };
        if bytes.is_empty() && ceros == 0 {
            continue;
        }
        while !imagen.is_empty() && imagen.len() % PAGE != 0 {
            // `0xCC` y no cero: si el flujo se sale del codigo, la maquina para
            // en vez de seguir por basura interpretable.
            imagen.push(0xCC);
        }
        base[n] = imagen.len();
        let desde = imagen.len();
        imagen.extend_from_slice(bytes);
        imagen.resize(imagen.len() + ceros, 0);
        if matches!(r, Region::Codigo | Region::Constantes) {
            let hasta = (imagen.len() + PAGE - 1) / PAGE * PAGE;
            solo_lectura.push((desde as u64, hasta as u64));
        }
    }
    assert!(base[0] != usize::MAX, "el .bex no trae codigo");

    // * LOS RELOCS, aplicados como los aplicara el cargador: aqui la "direccion
    // virtual" de una region es su offset en esta imagen plana, porque el
    // emulador direcciona desde cero.
    for r in v.relocs() {
        let donde = base[r.donde as usize];
        let destino = base[r.destino as usize];
        assert!(
            donde != usize::MAX && destino != usize::MAX,
            "un reloc nombra una region que este .bex no lleva"
        );
        let at = donde + r.offset as usize;
        let valor = (destino as u64).wrapping_add(r.addend);
        imagen[at..at + 8].copy_from_slice(&valor.to_le_bytes());
    }

    let mut machine = Machine::new(imagen);
    machine.solo_lectura = solo_lectura;
    machine.rip = v.entrada as usize; // `main` no tiene por que estar al principio
    sembrar(&mut machine);
    let machine = run(machine, 500_000);
    assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
    machine
}

/// Busca una subsecuencia de bytes dentro del BEF ya escrito.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

