//! **El banco de pruebas de BMO C++.**
//!
//! Mismo criterio que C y COBOL: **se ejecutan los bytes emitidos**, no se
//! comparan contra bytes escritos a mano. Un codegen que produce numeros
//! erroneos se ve sanisimo en un volcado hexadecimal.
//!
//! Aqui viven los ayudantes; cada tema tendra su fichero segun crezca.

use super::*;
use bmo_lower::emu::{run, Machine};

mod matriz;

/// Compila un programa de C++ y devuelve lo que el kernel habria pintado.
fn run_cpp(fuente: &str) -> String {
    let bef = compile_source_to_bef(fuente)
        .unwrap_or_else(|e| panic!("debe compilar: {}", e.message));
    ejecutar_bef(&bef)
}

/// Compila y ejecuta, y devuelve la maquina entera.
///
/// [!] **El codigo de retorno de `main` NO es observable**, y no es un descuido
/// de C++: `TASK_OP_EXIT` no acepta codigo de salida hoy --el kernel hace
/// revoke + reap-- asi que el codegen de C descarta `rax` a proposito y lo deja
/// escrito. Un test que comprobara el 42 estaria comprobando algo que el
/// sistema no expone.
fn ejecutar(fuente: &str) -> Machine {
    let bef = compile_source_to_bef(fuente)
        .unwrap_or_else(|e| panic!("debe compilar: {}", e.message));
    maquina_de_bef(&bef)
}

/// La imagen se rearma en el MISMO orden en que el codegen la dispuso:
/// codigo, luego rodata, luego data. Los `lea [rip+disp]` con los que se
/// alcanzan las cadenas se calcularon asumiendo que van pegadas detras del
/// codigo; cargar solo la seccion CODE deja esos punteros en el vacio.
///
/// Es copia deliberada del ayudante de C: el banco de pruebas es de cada
/// lenguaje (ver `HERENCIA.md`, regla 4 -- los tests no se mezclan).
fn ejecutar_bef(bef: &[u8]) -> String {
    maquina_de_bef(bef).console
}

/// *** THE LOADER, AS THE REAL ONE LOADS (2026-09-17).
///
/// C++ was parked on 2026-08-12 at 108 of 110 rows, with the diagnosis *"the C++
/// frontend did not receive C's fix for .data and relocations"*. **It was the
/// other way round**: this frontend lowers to C's AST and emits with C's
/// codegen, so it had the fix all along. What had NOT received it was this
/// HARNESS -- a copy of C's taken on 08-08, before C's harness learned three
/// things the loader does:
///
/// ```text
///    every section starts on its own page      `ring0/task/proc.rs`
///    Bss is zeros after the last section       `Codegen::separar_bss`
///    relocations are applied by the loader     a global `int g = 42` was 0
/// ```
///
/// The two red rows were the harness running a program the kernel would never
/// run. Still a deliberate COPY of `toolchain/lang/c/emisor-x86_64/src/tests/mod.rs`
/// (`maquina_de_bef_con`), per `HERENCIA.md` rule 4: each language owns its
/// bench.
fn maquina_de_bef(bef: &[u8]) -> Machine {
    use bmo_abi::bef2::{leer, Region};

    // ** BEF2 (2026-09-19): las cuatro regiones estan en la cabecera y el juez
    // ya las comprobo. Este arnes solo las COLOCA como el cargador -- cada una
    // en su pagina -- y aplica los relocs.
    let v = leer(bef).expect("el .bex que sale del compilador tiene que ser valido");

    const PAGE: usize = 4096;
    let mut imagen: Vec<u8> = Vec::new();
    let mut base = [usize::MAX; 4];
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

    let mut m = Machine::new(imagen);
    m.solo_lectura = solo_lectura;
    m.rip = v.entrada as usize;
    run(m, 500_000)
}

/// *** THE EXAMPLE THAT GOES TO THE DISK says what its header promises, byte
/// for byte. `build/ejemplos.ps1` compiles this same file to `cpp/cuentas.bex`,
/// so a change that breaks it is caught here and not in a photo of the Ryzen.
#[test]
fn el_ejemplo_cuentas_dice_lo_que_promete() {
    let ruta = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/1-clases/cuentas.cpp");
    let fuente = std::fs::read_to_string(ruta).expect("the example must exist");
    assert_eq!(
        run_cpp(&fuente),
        "abre corriente\nabre ahorro\ncorriente 12000\nahorro 10300\ntotal 22300\ncierra ahorro\ncierra corriente\n"
    );
}

// -- Paso 0: que emita un byte ---------------------------------------

/// * **La prueba de vida del paso 0.**
///
/// Antes de esto, el frontend no producia bytes para NINGUNA entrada -- ni
/// para un fichero vacio. Si este test se pone en rojo, C++ ha vuelto a no
/// existir, de lo que de todo lo demas.
#[test]
fn emite_un_bef_de_verdad() {
    let bef = compile_source_to_bef("int main() { return 42; }")
        .expect("el paso 0 es que ESTO compile");
    assert!(
        bef.len() > bmo_abi::bef2::CABECERA,
        "un BEF con cabecera y nada dentro no es un BEF"
    );
    assert_eq!(
        u32::from_le_bytes(bef[..4].try_into().unwrap()),
        bmo_abi::bef2::MAGIC,
        "los primeros cuatro bytes tienen que ser el magic del BEF",
    );
}

/// * Y que ademas CORRA, **desde la fuente**.
///
/// Emitir bytes con la forma correcta y que no ejecuten es el fallo que un
/// test de cabecera no ve. Esta es la prueba de vida completa del paso 0:
/// texto de C++ -> parseo -> descenso -> codegen de C -> bytes -> **ejecucion**.
///
/// `maquina_de_bef` ya exige que el programa termine por la puerta; si se
/// quedara dando vueltas o se saliera del codigo emitido, esto se pone rojo.
#[test]
fn el_programa_minimo_corre_desde_la_fuente() {
    let m = ejecutar("int main() { return 42; }");
    assert!(m.exited, "tiene que salir por INVOKE(EXIT)");
}

/// ** **El contrato de `HERENCIA.md`, ejecutable.**
///
/// Si C++ produce el `Program` de C, entonces para una fuente que es valida en
/// los dos lenguajes tiene que salir **exactamente el mismo BEF, byte a byte**.
///
/// Es la prueba mas fuerte que el paso 0 puede dar y no necesita observar
/// nada: cualquier cosa que el descenso se invente --un tipo distinto, una
/// ranura de pila de mas, un nodo perdido-- cambia los bytes y esto lo caza.
/// El dia que difieran, o C++ dejo de heredar o alguien los combino.
#[test]
fn los_bytes_son_identicos_a_los_de_bmo_c() {
    for fuente in ["int main() { return 42; }", "int main() { return 0; }"] {
        let de_cpp = compile_source_to_bef(fuente).expect("C++ debe compilarlo");
        let de_c = bmo_c_x86_64::compile_source_to_bef(fuente).expect("C debe compilarlo");
        assert_eq!(
            de_cpp, de_c,
            "el BEF de C++ y el de C tienen que ser identicos para {fuente:?}",
        );
    }
}

/// Un fichero vacio tiene que dar un error, no desbordar la pila.
///
/// Es literalmente la entrada con la que el frontend anterior moria: 12,12 MB
/// de `IrModule` construidos en la pila antes de mirar el AST.
#[test]
fn el_fichero_vacio_no_desborda_la_pila() {
    let e = compile_source_to_bef("").expect_err("sin `main` no hay programa");
    assert!(e.message.contains("main"), "el error tiene que decir que falta `main`: {}", e.message);
}

/// Un error de sintaxis tiene que llevar **la linea de verdad**.
///
/// Es lo que el frontend anterior no podia dar: sin lexer no hay token con
/// linea, y el parser contaba saltos a mano mientras adivinaba.
#[test]
fn los_errores_llevan_la_linea_real() {
    let e = compile_source_to_bef("int main() {\n  int x = 1;\n  int y = ;\n  return 0;\n}")
        .expect_err("`int y = ;` no es una expresion");
    assert_eq!(e.line, 3, "la linea tiene que ser la 3: {e:?}");
}

// -- La regla del descenso: rechazar DICIENDO en que paso llega ------

/// Lo que todavia no baja se rechaza **con el paso escrito**. Nunca en
/// silencio -- ese era el pecado del `parse_body` anterior, que hacia
/// `pos += 1` con lo que no reconocia y dejaba desaparecer cuerpos enteros.
#[test]
fn lo_que_falta_se_rechaza_diciendo_el_paso() {
    let casos: &[(&str, &str)] = &[
        ("namespace n { }\nint main(){ return 0; }", "PASO 4"),
        ("template<class T> T f(T x) { return x; }\nint main(){ return 0; }", "PASO 6"),
    ];
    for (fuente, paso) in casos {
        let e = compile_source_to_bef(fuente)
            .expect_err(&format!("esto no puede compilar todavia: {fuente:?}"));
        assert!(
            e.message.contains(paso),
            "el rechazo tiene que decir en que paso llega.\n  fuente: {fuente:?}\n  dijo:   {}",
            e.message,
        );
    }
}

/// ** El monton entra UNA vez (2026-09-18). `new` lo trae solo si la unidad no
/// lo incluyo; si lo incluyo, traerlo otra vez daria dos `malloc` en el mismo
/// programa -- y el codegen de C no lo rechaza, asi que ninguna fila que
/// EJECUTA lo veria. Se cuenta en el arbol de C.
#[test]
fn el_monton_no_se_duplica() {
    let cuenta = |src: &str| {
        let c = bmo_cpp_front::preproceso::a_c(src, std::path::Path::new("entrada.cpp"), false)
            .expect("compila");
        c.functions.iter().filter(|f| f.name == "malloc").count()
    };
    let clase = "class P { public: int x; P(int v) : x(v) {} };
                 int main() { P *p = new P(1); delete p; return 0; }";
    assert_eq!(cuenta(clase), 1, "sin #include: el monton lo trae `new`");
    assert_eq!(cuenta(&format!("#include <bmo/monton.h>
{clase}")), 1, "con #include: uno, no dos");
    assert_eq!(cuenta("int main() { return 0; }"), 0, "sin `new`, no hay monton");
}
