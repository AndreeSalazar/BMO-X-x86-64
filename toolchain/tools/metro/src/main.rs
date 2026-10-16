//! **El metro del emisor de x86-64** (2026-09-18).
//!
//! La ley de `OPTIMIZACION_MAESTRO.md` pone el orden: CORRECTO, MEDIDO, RAPIDO.
//! El emisor es "calculo puro" -- el unico camino donde la tabla dice que SI se
//! cuentan ciclos --, y hasta hoy no tenia metro: ninguna optimizacion podia
//! demostrar lo que ganaba, ni ningun cambio lo que empeoraba.
//!
//! Esto compila un banco FIJO de programas reales con los emisores de verdad
//! (C y C++ por objeto + enlace, como el build), los ejecuta en el emulador y
//! apunta tres numeros por programa:
//!
//!   pasos    instrucciones que ejecuto hasta `EXIT`. Determinista: sale igual
//!            en cualquier maquina que compile, sin el Ryzen.
//!   accesos  de esas, las que TOCAN memoria (pila, marco, memoria). Desde el
//!            19-09: el troquel por variable no bajo ni una instruccion y quito
//!            el 60 % de los accesos al marco, y el metro no lo veia.
//!   codigo   bytes de la seccion de codigo del `.bex` que se entrega.
//!   salida   la huella de lo que imprimio.
//!
//! `LINEA_BASE.txt` es un TRINQUETE: `pasos`, `accesos` y `codigo` solo pueden
//! bajar, y `salida` no puede cambiar NUNCA. Una optimizacion que cambia lo que
//! imprime un programa no es una optimizacion.
//!
//!   metro            la tabla
//!   metro --check    el juicio del build: 0 si nada sube ni cambia
//!   metro --fijar    reescribe la linea base (cuando algo BAJO)
//!   metro --desglose A DONDE se van las instrucciones, por clase y por
//!                    lenguaje (`emu::clases`). No es trinquete: es el dato
//!                    que elige la primera optimizacion, en vez de elegirla
//!                    quien mira
//!   metro --duelo    el MISMO programa en C y en TITAN++ (`duelo/`): cuantas
//!                    veces C cuesta TITAN++ (PLAN_TITAN_AFINADO)
//!   metro --caliente <programa del banco> [salida.bin]
//!                    las direcciones que MAS se ejecutan, con su clase, y
//!                    el codigo volcado a un fichero para desensamblarlo
//!                    (llvm-objcopy -I binary -O elf64-x86-64 + llvm-objdump)
//!
//! ** INTI entro el 19-09 (C5 de `PLAN_EL_TROQUEL.md`): cinco programas por
//! `cadena::compilar`, la MISMA cadena que la linea de ordenes, con el nombre
//! relativo en el manifiesto. Cuatro de los cinco tocan hardware que el
//! emulador no tiene (ventana, antena, audio, ficheros) y salen por su camino
//! de "no pude": es un camino del emisor igual que los otros, y lo que hace
//! `pulso.inti` --667.750 instrucciones-- es trabajo de verdad. `cpu.inti` se
//! queda fuera por `cpuid`, como `ciclos_C.c` por `rdtsc`.
//!
//! ** Y LA LENTE (14-10, A5 de `docs/plan/EL_FOCO.md`): este mismo banco es el
//! ORACULO de `bmo-lente-x86`. Cada programa corre con una sonda que apunta
//! cada paso (de donde a donde); al acabar, la lente lee cada instruccion que
//! corrio y lo que vino detras tiene que ser uno de sus sucesores -- la de
//! justo detras (juzga el LARGO) o el destino de su salto (juzga el
//! DESPLAZAMIENTO) --. Una que no sepa leer, o que lea mal, para el build:
//! lo que el emisor escribe y corre, la lente lo lee, en los seis lenguajes.
//!
//! [!] Lo que NO mide, dicho: los ciclos del Ryzen -- las instrucciones no son
//! ciclos; eso lo miden `c/coste.bex` y `c/ciclos.bex` en el metal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::exit;

use bmo_lower::emu::clases::{Censo, Clase};

/// Instrucciones como mucho por programa. Un programa del banco que no termina
/// es un fallo del emisor, no del metro.
const LIMITE: usize = 50_000_000;

/// El banco. **Fijo a proposito**: si cambia, la linea base se rehace entera.
/// Fuera, con su motivo:
///   c/ciclos_C.c, c/coste_C.c   opcodes que el emulador no ejecuta (rdtsc)
///   inti/sondas/cpu.inti        idem (cpuid)
///   inti/ejemplos/cubo.inti     BORRADO el 09-10: VERRANO lo manda TITAN++
///                               (corte 4 de PLAN_INTI_SAMURAI); su relevo,
///                               `nivel11/cubo_gira`, esta abajo
const BANCO: &[(&str, &str)] = &[
    ("c", "toolchain/lang/c/examples/hola_C.c"),
    ("c", "toolchain/lang/c/examples/memoria_C.c"),
    ("c", "toolchain/lang/c/examples/vivaldi_C.c"),
    ("c", "toolchain/lang/c/examples/blit_C.c"),
    ("c", "toolchain/lang/c/examples/sonido_C.c"),
    ("c", "toolchain/lang/c/examples/musica_C.c"),
    ("c", "toolchain/lang/c/examples/scroll_C.c"),
    ("c", "toolchain/lang/c/examples/caja_C.c"),
    ("c", "toolchain/lang/c/examples/leer_C.c"),
    ("c", "toolchain/lang/c/examples/cubo_C.c"),
    ("c", "toolchain/lang/c/examples/guia_C.c"),
    ("c", "toolchain/lang/c/examples/imagen_C.c"),
    ("c", "toolchain/lang/c/examples/raycaster_C.c"),
    ("c", "toolchain/lang/c/examples/sonda_C.c"),
    // A6 (14-10): pide POPCNT y BMI1, y su oraculo lo juzga.
    ("c", "toolchain/lang/c/examples/bits_C.c"),
    ("cpp", "toolchain/lang/cpp/examples/1-clases/cuentas.cpp"),
    ("cobol", "toolchain/lang/cobol/examples/1-basico/hola.cob"),
    ("cobol", "toolchain/lang/cobol/examples/2-decimal/banco.cob"),
    ("cobol", "toolchain/lang/cobol/examples/2-decimal/hola_COBOL.cob"),
    ("cobol", "toolchain/lang/cobol/examples/3-presentacion/extracto.cob"),
    ("cobol", "toolchain/lang/cobol/examples/5-tablas/conceptos.cob"),
    ("cobol", "toolchain/lang/cobol/examples/6-condiciones/cartera.cob"),
    ("cobol", "toolchain/lang/cobol/examples/7-empaquetado/cuentas.cob"),
    ("cobol", "toolchain/lang/cobol/examples/8-parrafos/cierre.cob"),
    ("cobol", "toolchain/lang/cobol/examples/9-decision/comision.cob"),
    ("ada", "toolchain/lang/ada/examples/1-basico/cierre.adb"),
    ("inti", "toolchain/lang/inti/sondas/pulso.inti"),
    ("inti", "toolchain/lang/inti/ejemplos/musica.inti"),
    // ** TITAN++ entro el 04-10, con sus programas BIEN de los niveles 0-3.
    // Hasta el nivel 3 todo valor se sabe al compilar y el `.bex` solo
    // escribe resultados: sus numeros son el SUELO (E0), no una victoria
    // sobre nadie. Estan aqui para que cada escalon del emisor (E1, los
    // registros, con el nivel 4) se mida desde el primer dia.
    ("titan", "toolchain/lang/titan/ejemplos/nivel0/hola.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel0/dos_saludos.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel1/centauro.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel2/constelacion.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel3/semaforo.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel3/calculadora.titan"),
    // Nivel 4 (04-10): bucles, corridos enteros al compilar.
    ("titan", "toolchain/lang/titan/ejemplos/nivel4/tabla_del_siete.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel4/primos.titan"),
    // Nivel 5 (04-10): recursion, corrida entera al compilar.
    ("titan", "toolchain/lang/titan/ejemplos/nivel5/factorial.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel5/euclides.titan"),
    // Nivel 6 (04-10): dec exacto, tablas y registros.
    ("titan", "toolchain/lang/titan/ejemplos/nivel6/factura.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel6/ordenar.titan"),
    // Nivel 7 (04-10): prestar y entregar, y la precision de COBOL.
    ("titan", "toolchain/lang/titan/ejemplos/nivel7/banco.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel7/ordena_prestada.titan"),
    // Nivel 8 (04-10): enum con datos y match exhaustivo.
    ("titan", "toolchain/lang/titan/ejemplos/nivel8/formas.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel8/pago.titan"),
    // Nivel 9 (04-10): paquetes de varios ficheros, con `mod`, `use` y `pub`.
    ("titan", "toolchain/lang/titan/ejemplos/nivel9/flota/src/main.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel9/tienda/src/main.titan"),
    // Nivel 10 (04-10): `trait`, lo que un valor sabe hacer.
    ("titan", "toolchain/lang/titan/ejemplos/nivel10/formas.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel10/motor/src/main.titan"),
    // Nivel 11 (04-10): gpu fn, una celda por hilo de la 3060 (G1).
    ("titan", "toolchain/lang/titan/ejemplos/nivel11/mezcla/src/main.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel11/activa/src/main.titan"),
    // ** 09-10: el RELEVO de `cubo.inti` -- el cubo que gira, contado por gpu
    // fn al correr y publicado en la lamina de VERRANO (LB7). En el emulador
    // nadie compone: escribe sus tres cuentas y lo dice.
    ("titan", "toolchain/lang/titan/ejemplos/nivel11/cubo_gira/src/main.titan"),
    // ** F1 y F4 de EL_FOCO (10-10): la ventana de TITAN++ y NAVEGAR, que
    // releva al de INTI (corte 4c). Desde el shell: sin escritorio que
    // componga, lo dicen y salen.
    ("titan", "toolchain/lang/titan/ejemplos/nivel11/ventana/src/main.titan"),
    ("titan", "Ultra_userspace/apps/navegar/src/main.titan"),
    // ** R1 de EL_FOCO (10-10): RESOLUCION, la primera que GUARDA; y la
    // ventana de arriba, que desde ese dia lee la medida antes de abrirse.
    ("titan", "Ultra_userspace/apps/resolucion/src/main.titan"),
    // ** Corte 4e de INTI (10-10): las herramientas de imagen, de TITAN++.
    // Sin ficheros, BICO dice sus dos 1 y sale; PNG escribe su fichero.
    ("titan", "Ultra_userspace/apps/bico/src/main.titan"),
    ("titan", "Ultra_userspace/apps/png/src/main.titan"),
    // Nivel 12 (05-10): lo que viene de fuera -- E1, el primer codigo de
    // TITAN++ que corre DE VERDAD en la maquina (PLAN_LA_ENTRADA).
    ("titan", "toolchain/lang/titan/ejemplos/nivel12/pregunta.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel12/hasta_fin.titan"),
    // `numero(t)` y el programa ENTERO al correr: dec, casos, llamadas.
    ("titan", "toolchain/lang/titan/ejemplos/nivel12/adivina.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel12/la_cuenta.titan"),
    // Nivel 13 (05-10): listas y mapas -- el primer valor en el monton.
    ("titan", "toolchain/lang/titan/ejemplos/nivel13/agenda.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel13/mundo.titan"),
    // Nivel 14 (10-10): el byte -- `[byte]`, UN byte por celda (TA1 de
    // PLAN_LA_TINTA).
    ("titan", "toolchain/lang/titan/ejemplos/nivel14/pixeles.titan"),
    ("titan", "toolchain/lang/titan/ejemplos/nivel14/histograma.titan"),
    // ** EL DUELO (16-10, PLAN_TITAN_AFINADO): el MISMO programa en C y en
    // TITAN++. En el banco, sus pasos son trinquete como los de todos; con
    // `--duelo`, la tabla de cuantas veces C es TITAN++.
    ("c", "toolchain/tools/metro/duelo/primos.c"),
    ("titan", "toolchain/tools/metro/duelo/primos.titan"),
    ("c", "toolchain/tools/metro/duelo/fib.c"),
    ("titan", "toolchain/tools/metro/duelo/fib.titan"),
    ("c", "toolchain/tools/metro/duelo/bytes.c"),
    ("titan", "toolchain/tools/metro/duelo/bytes.titan"),
    ("c", "toolchain/tools/metro/duelo/ordenar.c"),
    ("titan", "toolchain/tools/metro/duelo/ordenar.titan"),
];

/// Los duelos: el nombre de cada par de `duelo/` (`.c` y `.titan`).
const DUELOS: [&str; 4] = ["primos", "fib", "bytes", "ordenar"];

#[derive(Debug, Clone, PartialEq)]
struct Medida {
    pasos: u64,
    accesos: u64,
    codigo: u64,
    salida: String,
    /// A donde se fueron los `pasos`. No va a la linea base: es diagnostico.
    censo: Censo,
    /// Lo que dijo la lente (A5): cuantas instrucciones distintas leyo, o lo
    /// que no supo leer o leyo mal. No va a la linea base: es un juez.
    lente: Result<usize, Vec<String>>,
}

/// ** EL ORACULO DE LA LENTE: cada paso que dio el programa, `(de, a)`, es un
/// sucesor que la lente dice de la instruccion en `de`.
fn juzgar_lente(codigo: &[u8], pasos: &std::collections::HashSet<(usize, usize)>, declara: u64) -> Result<usize, Vec<String>> {
    use bmo_lente_x86::{lee, Sigue};
    let mut leidas = std::collections::HashMap::new();
    let mut quejas = Vec::new();
    let bytes = |a: usize| codigo[a..(a + 12).min(codigo.len())].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
    let mut orden: Vec<&(usize, usize)> = pasos.iter().collect();
    orden.sort();
    for &&(de, a) in &orden {
        let i = match leidas.entry(de).or_insert_with(|| lee(&codigo[de..])) {
            Ok(i) => *i,
            Err(e) => {
                if quejas.len() < 20 {
                    quejas.push(format!("lente: en 0x{de:x} no sabe leer ({e:?}): {}", bytes(de)));
                }
                continue;
            }
        };
        let (de64, a64) = (de as u64, a as u64);
        // ** A6: lo que corrio pide a la CPU, y el `.bex` lo tiene que DECLARAR.
        // Un `popcnt` sin su linea es un `#UD` en una CPU sin el.
        if !i.fila.pide.is_empty() {
            let bit = bmo_abi::bef::requisitos::instrucciones::por_nombre(i.fila.pide).map_or(u64::MAX, |e| e.bit);
            if declara & bit != bit && quejas.len() < 20 {
                quejas.push(format!("lente: en 0x{de:x} corre `{}`, que pide {}, y el .bex no lo declara: {}", i.texto(de64).as_str(), i.fila.pide, bytes(de)));
            }
        }
        let vale = match i.sigue(de64) {
            Sigue::Siguiente(n) => a64 == n,
            Sigue::Condicional { cae, salta } => a64 == cae || a64 == salta,
            Sigue::Salta(t) => a64 == t,
            Sigue::Llama { destino, .. } => a64 == destino,
            Sigue::Vuelve | Sigue::Indirecto => true,
            Sigue::Para => false,
        };
        if !vale && quejas.len() < 20 {
            quejas.push(format!("lente: en 0x{de:x} lee `{}` ({} B) y el programa siguio en 0x{a:x}: {}", i.texto(de64).as_str(), i.largo, bytes(de)));
        }
    }
    if quejas.is_empty() { Ok(leidas.len()) } else { Err(quejas) }
}

/// Las extensiones de x86-64 que el `.bex` declara usar (A6).
fn declara(bex: &[u8]) -> u64 {
    use bmo_abi::bef::requisitos::{Tabla, CLASE_INSTRUCCIONES};
    let Ok(v) = bmo_abi::bef2::leer(bex) else { return 0 };
    let Some(t) = v.anexo(bmo_abi::bef2::ANEXO_REQUISITOS).and_then(Tabla::abrir) else { return 0 };
    t.iter().filter(|r| r.clase == CLASE_INSTRUCCIONES && r.es_obligatorio()).fold(0, |m, r| m | r.cantidad)
}

/// **El recorrido de B1** (`bmo_lente_x86::recorre`) sobre el codigo de un
/// `.bex`, con lo que declara: el veredicto, el largo del codigo y el mapa de lo
/// leido.
fn recorrer(bex: &[u8]) -> (bmo_lente_x86::recorre::Recorrido, usize, Vec<u8>) {
    let Ok(v) = bmo_abi::bef2::leer(bex) else {
        return (bmo_lente_x86::recorre::Recorrido { desborda: true, ..Default::default() }, 0, Vec::new());
    };
    let c = v.region(bmo_abi::bef2::Region::Codigo);
    let d = declara(bex);
    let mut visto = vec![0u8; c.len().div_ceil(8)];
    let mut pila = vec![0u32; 1 << 16];
    let r = bmo_lente_x86::recorre::recorre(
        c.len(),
        v.entrada as usize,
        v.relocs().filter(|r| r.destino == bmo_abi::bef2::Region::Codigo).map(|r| r.addend),
        |en, b| {
            let n = (c.len() - en).min(b.len());
            b[..n].copy_from_slice(&c[en..en + n]);
            n
        },
        |n| bmo_abi::bef::requisitos::instrucciones::por_nombre(n).is_some_and(|e| d & e.bit != 0),
        &mut visto,
        &mut pila,
    );
    (r, c.len(), visto)
}

/// Lo que le falta a un recorrido para quedar demostrado, dicho: con los
/// bytes de cada sitio (y lo que la lente lee en ellos, si lee algo).
fn que_falta(r: &bmo_lente_x86::recorre::Recorrido, c: &[u8]) -> String {
    let sitio = |en: usize| {
        let b: Vec<String> = c[en..(en + 10).min(c.len())].iter().map(|b| format!("{b:02x}")).collect();
        match bmo_lente_x86::lee(&c[en..(en + 15).min(c.len())]) {
            Ok(i) => format!("`{}` [{}]", i.texto(en as u64).as_str(), b.join(" ")),
            Err(_) => format!("[{}]", b.join(" ")),
        }
    };
    let mut q = Vec::new();
    if let Some((en, e)) = r.no_declarada { q.push(format!("en 0x{en:x} una instruccion de {e}, que el .bex NO declara: {}", sitio(en))); }
    if let Some((en, e)) = r.no_sabe { q.push(format!("en 0x{en:x} no sabe leer ({e:?}): {}", sitio(en))); }
    if let Some((de, a)) = r.fuera { q.push(format!("en 0x{de:x} un salto sale del codigo, a 0x{a:x}")); }
    if r.desborda { q.push("el recorrido desbordo".into()); }
    q.join("; ")
}

/// `--recorre <fichero.bex>`: el veredicto de B1 para cualquier `.bex`.
fn recorre_fichero(ruta: &str) {
    let bex = match std::fs::read(ruta) {
        Ok(b) => b,
        Err(e) => { eprintln!("recorre: no se lee {ruta}: {e}"); std::process::exit(2) }
    };
    let (r, largo, _) = recorrer(&bex);
    println!("{ruta}: {} instrucciones, {} de {} B de codigo alcanzados desde la entrada, {} saltos indirectos; {} piden algo NO declarado, {} se degradan sin ello (tzcnt/lzcnt)", r.leidas, r.bytes, largo, r.indirectos, r.no_declaradas, r.degradan);
    if r.demostrado() {
        println!("DEMOSTRADO: todo lo alcanzable se lee, es de lo declarado y no sale del codigo");
    } else {
        let c = bmo_abi::bef2::leer(&bex).map(|v| v.region(bmo_abi::bef2::Region::Codigo).to_vec()).unwrap_or_default();
        println!("NO DEMOSTRADO: {}", que_falta(&r, &c));
        std::process::exit(1);
    }
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..")
}

fn compilar(lenguaje: &str, ruta: &Path, rel: &str, fuente: &str) -> Result<Vec<u8>, String> {
    let nombre = ruta.file_name().and_then(|n| n.to_str()).unwrap_or("unidad").to_string();
    let enlazar = |objeto: Vec<u8>| {
        bmo_enlazar::enlazar(&[(nombre.clone(), objeto)]).map_err(|e| format!("enlace: {e:?}"))
    };
    match lenguaje {
        "c" => enlazar(bmo_c_x86_64::compile_object_with_preprocessor(
            fuente, ruta, bmo_c_x86_64::CStandard::C11, bmo_c_x86_64::Libc::Copia)
            .map_err(|e| format!("linea {}: {}", e.line, e.message))?),
        "cpp" => enlazar(bmo_cpp_x86_64::compilar_fichero(fuente, ruta, true)
            .map_err(|e| format!("linea {}: {}", e.line, e.message))?),
        "cobol" => bmo_cobol_x86_64::compile_source_to_bex(fuente)
            .map_err(|e| format!("{e:?}")),
        "ada" => bmo_ada_x86_64::compilar(fuente).map_err(|e| format!("{e:?}")),
        // ** INTI por su cadena entera (`cadena::compilar`, la misma que la
        // linea de ordenes), con el nombre RELATIVO: el manifiesto lleva el
        // nombre del fichero y un camino absoluto cambiaria el `.ibx` de una
        // maquina a otra.
        "inti" => bmo_inti_x86_64::cadena::compilar(fuente, rel, &bmo_mods::Roots::find())
            .map(|c| c.bytes)
            .map_err(|e| e.to_string()),
        // TITAN++ por su cadena entera (`build_package`: frontend, juez,
        // calculo, emisor y el gate del manifiesto). Un PAQUETE (nivel 9)
        // empieza en la carpeta de encima de la del fichero, como en F1; un
        // fichero solo es un paquete de un modulo, con el mismo manifiesto.
        "titan" => {
            let carpeta = ruta.parent().map(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()).unwrap_or_default();
            let base = ruta.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
            bmo_titan_x86_64::build_package(&format!("{carpeta}/{nombre}"), fuente, &mut |p| std::fs::read_to_string(base.join(p)).ok())
                .map_err(|e| format!("{e:?}"))
        }
        otro => Err(format!("lenguaje desconocido `{otro}`")),
    }
}

/// Los bytes de la region de codigo del `.bex`: la cabecera BEF2 dice donde
/// esta y cuanto mide, y el juez del contrato lo comprueba.
fn bytes_de_codigo(bex: &[u8]) -> u64 {
    match bmo_abi::bef2::leer(bex) {
        Ok(v) => v.tramo(bmo_abi::bef2::Region::Codigo).bytes as u64,
        Err(f) => panic!("un programa del banco no es un BEF2 valido: {}", f.nombre()),
    }
}

/// FNV-1a de 64 bits: una huella, no una firma. Solo tiene que cambiar si la
/// salida cambia.
fn huella(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn medir(lenguaje: &str, rel: &str) -> Result<Medida, String> {
    let ruta = raiz().join(rel);
    let fuente = std::fs::read_to_string(&ruta).map_err(|e| format!("no se lee: {e}"))?;
    let bex = compilar(lenguaje, &ruta, rel, &fuente)?;
    let mut maquina = bmo_lower::emu::cargar_bex(&bex)?;
    // Nivel 12 de TITAN++ (05-10): un programa que LEE corre con lo que su
    // ejemplo dice que se teclea (`# entra:`), igual que en su banco.
    let entra: String = fuente.lines().filter_map(|l| l.strip_prefix("# entra: ")).map(|l| format!("{l}\n")).collect();
    maquina.poner_entrada(&entra);
    let codigo = maquina.code.clone();
    let (m, pasos) = std::panic::catch_unwind(move || {
        // la sonda de la lente: cada paso, de donde a donde
        let mut pasos = std::collections::HashSet::new();
        let mut antes: Option<usize> = None;
        let m = bmo_lower::emu::run_con(maquina, LIMITE, |rip| {
            if let Some(a) = antes {
                pasos.insert((a, rip));
            }
            antes = Some(rip);
        });
        (m, pasos)
    })
    .map_err(|_| format!("no termina en {LIMITE} instrucciones o el emulador no sabe una"))?;
    if !m.exited {
        return Err("no termino por EXIT".into());
    }
    let lente = juzgar_lente(&codigo, &pasos, declara(&bex));
    // ** B1: el recorrido desde la ENTRADA, el que hara el kernel al admitir.
    // Cada programa del banco tiene que quedar DEMOSTRADO, y todo lo que corrio
    // tiene que haberlo leido -- salvo lo que solo se alcanza por un salto
    // indirecto, que el recorrido cuenta y no puede seguir.
    let lente = {
        let (r, largo, visto) = recorrer(&bex);
        let mut quejas = Vec::new();
        if !r.demostrado() {
            let c = bmo_abi::bef2::leer(&bex).map(|v| v.region(bmo_abi::bef2::Region::Codigo).to_vec()).unwrap_or_default();
            quejas.push(format!("recorrido: no demostrado ({}/{} B): {}", r.bytes, largo, que_falta(&r, &c)));
        }
        let sin: Vec<usize> = pasos.iter().map(|p| p.0).filter(|&o| visto.get(o / 8).is_none_or(|b| b >> (o % 8) & 1 == 0)).collect();
        if !sin.is_empty() && r.indirectos == 0 {
            quejas.push(format!("recorrido: {} instrucciones corrieron y no las alcanzo (la primera en 0x{:x}), sin un salto indirecto que lo explique", sin.len(), sin.iter().min().unwrap()));
        }
        // Los dos jueces hablan: el de lo que corrio y el del recorrido.
        match lente {
            Ok(n) if quejas.is_empty() => Ok(n),
            Ok(_) => Err(quejas),
            Err(mut q) => { q.extend(quejas); Err(q) }
        }
    };
    Ok(Medida { pasos: m.pasos, accesos: m.censo.accesos(), codigo: bytes_de_codigo(&bex), salida: huella(&m.console), censo: m.censo, lente })
}

fn linea_base() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("LINEA_BASE.txt")
}

fn leer_base() -> Result<BTreeMap<String, Medida>, String> {
    let texto = std::fs::read_to_string(linea_base()).map_err(|e| format!("no hay linea base: {e}"))?;
    let mut out = BTreeMap::new();
    for l in texto.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() != 5 { return Err(format!("linea base mal formada (programa pasos accesos codigo salida): `{l}`")); }
        let n = |s: &str| s.parse::<u64>().map_err(|_| format!("numero mal formado: `{s}`"));
        out.insert(c[0].to_string(), Medida { pasos: n(c[1])?, accesos: n(c[2])?, codigo: n(c[3])?, salida: c[4].to_string(), censo: Censo::default(), lente: Ok(0) });
    }
    Ok(out)
}

/// **El juicio**: que subio, que cambio y que falta. `(quejas, cuantos bajaron)`.
fn juzgar(medidas: &BTreeMap<String, Medida>, base: &BTreeMap<String, Medida>) -> (Vec<String>, usize) {
    let mut quejas = Vec::new();
    let mut bajaron = 0;
    for (p, m) in medidas {
        match base.get(p) {
            None => quejas.push(format!("{p}: no esta en la linea base (fijala con --fijar)")),
            Some(b) => {
                if m.salida != b.salida {
                    quejas.push(format!("{p}: LA SALIDA CAMBIO ({} -> {}): una optimizacion no cambia lo que hace un programa", b.salida, m.salida));
                }
                if m.pasos > b.pasos {
                    quejas.push(format!("{p}: instrucciones SUBEN {} -> {}", b.pasos, m.pasos));
                }
                if m.accesos > b.accesos {
                    quejas.push(format!("{p}: accesos a memoria SUBEN {} -> {}", b.accesos, m.accesos));
                }
                if m.codigo > b.codigo {
                    quejas.push(format!("{p}: codigo SUBE {} -> {} B", b.codigo, m.codigo));
                }
                if m.pasos < b.pasos || m.accesos < b.accesos || m.codigo < b.codigo { bajaron += 1; }
            }
        }
    }
    for p in base.keys() {
        if !medidas.contains_key(p) {
            quejas.push(format!("{p}: esta en la linea base y ya no en el banco"));
        }
    }
    (quejas, bajaron)
}

/// Una fila del desglose: cada clase como porcentaje del total de esa fila.
fn fila_desglose(nombre: &str, c: &Censo) {
    let t = c.total().max(1) as f64;
    print!("{nombre:<12} {:>10}", c.total());
    for k in Clase::TODAS {
        print!(" {:>6.1}", c.de(k) as f64 * 100.0 / t);
    }
    println!();
}

/// `--desglose`: por lenguaje y en total. Por programa NO, a proposito: el
/// banco lo dominan tres programas de C (blit, memoria, vivaldi) y una tabla de
/// 25 filas esconde eso en vez de ensenarlo; por lenguaje es donde se decide
/// que emisor se toca primero.
fn desglose(medidas: &BTreeMap<String, Medida>) {
    let mut por_lenguaje: BTreeMap<&str, Censo> = BTreeMap::new();
    let mut total = Censo::default();
    for (lenguaje, rel) in BANCO {
        if let Some(m) = medidas.get(*rel) {
            por_lenguaje.entry(lenguaje).or_default().sumar(&m.censo);
            total.sumar(&m.censo);
        }
    }
    print!("{:<12} {:>10}", "lenguaje", "pasos");
    for k in Clase::TODAS { print!(" {:>6}", k.nombre()); }
    println!("   (% de los pasos de esa fila)");
    for (l, c) in &por_lenguaje { fila_desglose(l, c); }
    fila_desglose("TOTAL", &total);
    // Y la conclusion en una linea, porque es lo que se lee.
    let (pila, marco) = (total.de(Clase::Pila), total.de(Clase::Marco));
    println!(
        "
marco + pila = {} de {} ({:.1} %): lo que un reparto de registros quitaria como techo",
        pila + marco, total.total(), (pila + marco) as f64 * 100.0 / total.total().max(1) as f64
    );
}

/// `--caliente`: el perfil de UN programa, direccion por direccion.
fn caliente(rel: &str, salida: Option<&str>) {
    let Some((lenguaje, _)) = BANCO.iter().find(|(_, r)| *r == rel) else {
        println!("`{rel}` no esta en el banco del metro");
        exit(1);
    };
    let ruta = raiz().join(rel);
    let fuente = std::fs::read_to_string(&ruta).expect("leer el fuente");
    let bex = compilar(lenguaje, &ruta, rel, &fuente).expect("compilar");
    let mut maquina = bmo_lower::emu::cargar_bex(&bex).expect("cargar");
    // y teclea lo que dice el ejemplo, como `medir`: sin eso, un programa que
    // lee espera para siempre
    let entra: String = fuente.lines().filter_map(|l| l.strip_prefix("# entra: ")).map(|l| format!("{l}\n")).collect();
    maquina.poner_entrada(&entra);
    let codigo = maquina.code.clone();
    let mut cuentas: BTreeMap<usize, u64> = BTreeMap::new();
    let m = bmo_lower::emu::run_con(maquina, LIMITE, |rip| *cuentas.entry(rip).or_default() += 1);
    if let Some(s) = salida {
        std::fs::write(s, &codigo).expect("escribir el codigo");
        // y TODAS las cuentas al lado, `direccion veces`, para sumar por
        // patron con lo que sea: la tabla de abajo solo muestra 40
        let cuentas_txt: String = cuentas.iter().map(|(a, c)| format!("{a:x} {c}
")).collect();
        std::fs::write(format!("{s}.cuentas"), cuentas_txt).expect("escribir las cuentas");
        println!("codigo volcado en {s} ({} B) y las cuentas en {s}.cuentas", codigo.len());
    }
    // Las 40 direcciones mas calientes, en ORDEN DE DIRECCION: asi el bucle
    // se lee de arriba abajo, con su clase al lado.
    let mut top: Vec<(usize, u64)> = cuentas.iter().map(|(&a, &c)| (a, c)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    top.truncate(40);
    top.sort();
    println!("{:>8}  {:>10}  {:<10}  bytes        ({} pasos en total)", "rip", "veces", "clase", m.pasos);
    for (a, c) in top {
        let clase = bmo_lower::emu::clases::clasificar(&codigo[a..]).nombre();
        let bytes: Vec<String> = codigo[a..(a + 8).min(codigo.len())].iter().map(|b| format!("{b:02x}")).collect();
        println!("{a:>8x}  {c:>10}  {clase:<10}  {}", bytes.join(" "));
    }
}

fn main() {
    let modo = std::env::args().nth(1).unwrap_or_default();
    if modo == "--recorre" {
        recorre_fichero(&std::env::args().nth(2).unwrap_or_default());
        return;
    }
    // ** EL DUELO: TITAN++ contra C, el mismo programa. La meta de
    // PLAN_TITAN_AFINADO: llegar a C sin que TITAN++ aprenda de CPU.
    if modo == "--duelo" {
        println!("{:<10} {:>12} {:>12} {:>8}", "duelo", "pasos C", "TITAN++", "veces C");
        for d in DUELOS {
            let c = medir("c", &format!("toolchain/tools/metro/duelo/{d}.c"));
            let t = medir("titan", &format!("toolchain/tools/metro/duelo/{d}.titan"));
            match (c, t) {
                (Ok(c), Ok(t)) => {
                    if c.salida != t.salida {
                        eprintln!("metro: el duelo {d} no escribe lo mismo en C y en TITAN++");
                        exit(1);
                    }
                    println!("{:<10} {:>12} {:>12} {:>8.2}", d, c.pasos, t.pasos, t.pasos as f64 / c.pasos as f64);
                }
                (c, t) => {
                    eprintln!("metro: el duelo {d}: C {:?}, TITAN++ {:?}", c.err(), t.err());
                    exit(1);
                }
            }
        }
        return;
    }
    if modo == "--caliente" {
        let rel = std::env::args().nth(2).unwrap_or_default();
        let salida = std::env::args().nth(3);
        caliente(&rel, salida.as_deref());
        return;
    }
    // ** LOS PROGRAMAS DEL BANCO SE MIDEN A LA VEZ (13-10): uno por hilo de
    // la CPU. Cada medida es suya --compilar y emular, sin nada compartido--
    // y el emulador cuenta instrucciones, no tiempo: el numero es el mismo en
    // fila o a la vez. En fila eran 11 s de un solo nucleo en cada build.
    let siguiente = std::sync::atomic::AtomicUsize::new(0);
    let hilos = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(BANCO.len());
    let resultados: Vec<(usize, Result<Medida, String>)> = std::thread::scope(|s| {
        let obreros: Vec<_> = (0..hilos)
            .map(|_| {
                s.spawn(|| {
                    let mut hechos = Vec::new();
                    loop {
                        let i = siguiente.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((lenguaje, rel)) = BANCO.get(i) else { break };
                        hechos.push((i, medir(lenguaje, rel)));
                    }
                    hechos
                })
            })
            .collect();
        obreros.into_iter().flat_map(|h| h.join().expect("un hilo del metro cayo")).collect()
    });
    let mut por_orden: Vec<_> = resultados;
    por_orden.sort_by_key(|(i, _)| *i);
    let mut medidas = BTreeMap::new();
    let mut rotos = Vec::new();
    for (i, r) in por_orden {
        let rel = BANCO[i].1;
        match r {
            Ok(m) => { medidas.insert(rel.to_string(), m); }
            Err(e) => rotos.push(format!("{rel}: {e}")),
        }
    }
    if !rotos.is_empty() {
        for r in &rotos { println!("  [X] {r}"); }
        println!("metro: {} programa(s) del banco no se pudieron medir", rotos.len());
        exit(1);
    }
    // ** El oraculo de la lente (A5): lo que corrio, leido.
    let mut leidas = 0;
    let mut mal = Vec::new();
    for (p, m) in &medidas {
        match &m.lente {
            Ok(n) => leidas += n,
            Err(q) => mal.extend(q.iter().map(|x| format!("{p}: {x}"))),
        }
    }
    if !mal.is_empty() {
        for q in &mal { println!("  [X] {q}"); }
        println!("metro: la lente y lo que corrio no casan ({} queja(s)): o falta una fila en `platform/shared/bmo-lente-x86/src/tabla.rs`, o el emisor no declara lo que pide a la CPU", mal.len());
        exit(1);
    }
    let (pasos, accesos, codigo): (u64, u64, u64) =
        medidas.values().fold((0, 0, 0), |a, m| (a.0 + m.pasos, a.1 + m.accesos, a.2 + m.codigo));

    match modo.as_str() {
        "--fijar" => {
            let mut t = String::from(
                "# EL METRO DEL EMISOR -- linea base (toolchain/tools/metro).\n\
                 # TRINQUETE: `pasos`, `accesos` y `codigo` solo bajan; `salida` no cambia nunca.\n\
                 # Se reescribe con `cargo run --release -p bmo-metro -- --fijar`.\n\
                 # programa  pasos  accesos  codigo  salida\n");
            for (p, m) in &medidas {
                t.push_str(&format!("{p} {} {} {} {}\n", m.pasos, m.accesos, m.codigo, m.salida));
            }
            std::fs::write(linea_base(), t).expect("escribir la linea base");
            println!("metro: linea base fijada -- {} programas, {pasos} instrucciones, {accesos} accesos a memoria, {codigo} B de codigo", medidas.len());
        }
        "--desglose" => desglose(&medidas),
        "--check" => {
            let base = match leer_base() {
                Ok(b) => b,
                Err(e) => { println!("guardian MUERTO: {e}"); exit(1); }
            };
            let (quejas, bajaron) = juzgar(&medidas, &base);
            if !quejas.is_empty() {
                for q in &quejas { println!("  [X] {q}"); }
                println!("metro: {} incumplimiento(s)", quejas.len());
                exit(1);
            }
            let aviso = if bajaron > 0 { format!("; {bajaron} bajaron: fija con --fijar") } else { String::new() };
            println!("clean: metro del emisor -- {} programas (C, C++, COBOL, Ada, INTI, TITAN++), {pasos} instrucciones, {accesos} accesos a memoria, {codigo} B de codigo, ninguna salida cambio{aviso}; la lente lee las {leidas} instrucciones distintas que corrieron, y el recorrido desde la entrada (B1) los demuestra a todos", medidas.len());
        }
        _ => {
            println!("{:<58} {:>12} {:>9} {:>9}  salida", "programa", "pasos", "accesos", "codigo");
            for (p, m) in &medidas {
                println!("{p:<58} {:>12} {:>9} {:>9}  {}", m.pasos, m.accesos, m.codigo, m.salida);
            }
            println!("{:<58} {pasos:>12} {accesos:>9} {codigo:>9}", "TOTAL");
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(pasos: u64, codigo: u64, salida: &str) -> Medida {
        Medida { pasos, accesos: pasos / 2, codigo, salida: salida.into(), censo: Censo::default(), lente: Ok(0) }
    }
    fn con_accesos(mut x: Medida, accesos: u64) -> Medida {
        x.accesos = accesos;
        x
    }
    fn uno(x: Medida) -> BTreeMap<String, Medida> {
        BTreeMap::from([("p".to_string(), x)])
    }

    #[test]
    fn igual_es_limpio() {
        assert_eq!(juzgar(&uno(m(10, 20, "a")), &uno(m(10, 20, "a"))), (vec![], 0));
    }
    #[test]
    fn bajar_es_limpio_y_se_cuenta() {
        let (q, b) = juzgar(&uno(m(9, 19, "a")), &uno(m(10, 20, "a")));
        assert!(q.is_empty());
        assert_eq!(b, 1);
    }
    #[test]
    fn subir_instrucciones_se_caza() {
        assert_eq!(juzgar(&uno(m(11, 20, "a")), &uno(m(10, 20, "a"))).0.len(), 1);
    }
    #[test]
    fn subir_accesos_se_caza_aunque_las_instrucciones_no_suban() {
        assert_eq!(juzgar(&uno(con_accesos(m(10, 20, "a"), 9)), &uno(con_accesos(m(10, 20, "a"), 8))).0.len(), 1);
        assert!(juzgar(&uno(con_accesos(m(10, 20, "a"), 7)), &uno(con_accesos(m(10, 20, "a"), 8))).0.is_empty());
    }
    #[test]
    fn subir_codigo_se_caza() {
        assert_eq!(juzgar(&uno(m(10, 21, "a")), &uno(m(10, 20, "a"))).0.len(), 1);
    }
    /// La que mas importa: mas rapido y DISTINTO no es mas rapido, es roto.
    #[test]
    fn cambiar_la_salida_se_caza_aunque_baje() {
        let (q, _) = juzgar(&uno(m(5, 10, "b")), &uno(m(10, 20, "a")));
        assert!(q.iter().any(|x| x.contains("LA SALIDA CAMBIO")));
    }
    #[test]
    fn un_programa_que_falta_o_sobra_se_caza() {
        assert_eq!(juzgar(&uno(m(1, 1, "a")), &BTreeMap::new()).0.len(), 1);
        assert_eq!(juzgar(&BTreeMap::new(), &uno(m(1, 1, "a"))).0.len(), 1);
    }
}
