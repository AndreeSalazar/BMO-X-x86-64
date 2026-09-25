//! **LAS TRES OPERACIONES QUE MANDAN SOBRE LA MAQUINA**: los nucleos, el sello
//!
//! [carril]  ROJO      los nucleos, el reposo y la red: manda sobre la maquina
//! [consumo] NADA      las puertas de AXION: corren cuando alguien las pide; el
//!                     que enciende nucleos es plat/smp
//! de ESTRATOS y la administracion del disco.
//!
//! ## Por que salen del despachador (L6a, L6b)
//!
//! *** Y antes que nada, la MEDIDA que corrigio el diagnostico (2026-08-24).
//!
//! `invoke_current_task` son 795 lineas y el censo lo marcaba `CON MONSTRUO`,
//! o sea *"partirla es esquema, no tijeras"*. Se midio su estado compartido y
//! salio esto:
//!
//! ```text
//!    locales declarados a nivel del `match`   0
//!    estado compartido                        los TRES parametros, y nada mas
//!    el `match` empieza                       en la primera linea del cuerpo
//! ```
//!
//! ** No es un monstruo: es un DESPACHADOR con unos cuarenta y cinco brazos
//! independientes. Un monstruo de verdad --el de `task/admitir.rs`-- comparte
//! decenas de locales entre sus ramas y por eso pide un struct antes de
//! tocarlo. Este no comparte NADA, y cada brazo es una funcion esperando a que
//! le pongan nombre.
//!
//! [!] La media dijo `mixto`, el detector de monstruos dijo `CON MONSTRUO`, y
//! **los dos se equivocaron de la misma forma**: midieron el MEDIDA de la
//! funcion y no lo que decide si se puede partir, que es su ESTADO.
//!
//! ## Y por que estas tres y no otras
//!
//! Porque son las tres mas grandes --155, 18 y 70 lineas-- y porque contestan
//! la misma pregunta: **son las unicas operaciones que le dicen a la maquina lo
//! que tiene que hacer**, en vez de pedirle algo. Las demas abren un fichero,
//! escriben en la consola o preguntan un numero.
//!
//! ** Que se apunte en CABINA ANTES y DESPUES no es telemetria: las tres pueden
//! cambiar el estado del hardware, y la primera operacion que cambia el almacen
//! no puede ser silenciosa ni cuando funciona.
//!
//! ## [!] Esto NO es un reparto puro de L6d, y se dice
//!
//! El CUERPO de cada brazo se movio tal cual --ni una linea cambia de
//! contenido-- pero el brazo del `match` paso de llevar el cuerpo dentro a ser
//! una llamada. Eso es una linea distinta por operacion, y se cuenta como lo
//! que es en vez de llamarlo "mover texto".

use super::*;

/// **PONER UN NUMERO EN EL ATRIL.** Ver `plat::smp::atril`.
///
/// De uno en uno porque por la puerta caben dos numeros y un encargo lleva
/// cuatro -- el mismo idioma que `OP_RUTA` antes de `OP_EJECUTAR`.
pub(crate) fn atril(campo: u64, valor: u64) -> BmoStatus {
    if campo >= crate::ring0::plat::smp::atril::CAMPOS {
        return BmoStatus::ok_value(0);
    }
    BmoStatus::ok_value(u64::from(crate::ring0::plat::smp::atril::poner(campo, valor)))
}

/// **TOCAD.** `parte` es un numero del catalogo de `bmo-orquesta`, `pedidos` un
/// maximo de atriles (`0` = los que convengan).
///
/// *** Es la primera operacion del sistema que pone a TRABAJAR a mas de un
/// nucleo con datos de Ring 3. Se apunta en CABINA antes y despues por el mismo
/// motivo que las otras tres de este fichero: **lo que cambia el estado de la
/// maquina no puede ser silencioso ni cuando funciona**.
pub(crate) fn tocar(parte: u64, pedidos: u64) -> BmoStatus {
    let pid = crate::ring0::task::scheduler::current_pid();
    let n = crate::ring0::plat::smp::atril::tocar(pid, parte, pedidos);
    if n == crate::ring0::plat::smp::atril::NO_HAY_ORQUESTA {
        return BmoStatus::ok_value(0);
    }
    crate::ring0::cabina::info("orquesta", "atriles que tocaron", n);
    BmoStatus::ok_value(n)
}

/// **Los nucleos**: despertarlos, pararlos, o medir el reparto.
pub(super) fn smp_despertar(arg0: u64, arg1: u64) -> BmoStatus {
        use crate::ring0::plat::smp::{self, crew};
        let cuantos = if arg0 > u32::MAX as u64 { u32::MAX } else { arg0 as u32 };
        match arg1 {
            // *** 4 y 5: EL ANCHO DE BANDA DE LA MEMORIA, DESDE RING 3.
            //
            // ** Entraron el 2026-08-24 porque `banda` se escribio solo para el
            // shell de Ring 0 -- y al shell de Ring 0 el propietario NO VUELVE: vive
            // en el escritorio. Una medida a la que no se puede llegar desde
            // donde se trabaja es una medida que no existe.
            //
            // > Lo que solo es orden del kernel es codigo que el no puede usar.
            //
            // [!] Y son DOS y no una a proposito. Preparar reserva 256 MiB y los
            // llena; medir corre el barrido. Si fueran la misma llamada, el
            // coste de reservar caeria dentro del primer punto del barrido y ese
            // punto saldria lento -- justo el que sirve de referencia para todos
            // los demas. Separarlas es lo que mantiene la columna `x1` honesta.
            4 => {
                // *** `Err(_)` -- el guion bajo delataba el fallo (12-09).
                //
                // `preparar` distingue TRES motivos y los tres llegaban aqui
                // como "preparo cero bytes", con codigo de exito. El `value`
                // sigue siendo 0, asi que `banda_preparar()` contesta lo mismo
                // que ayer; lo que cambia es que ahora se sabe cual. Ver L6j.
                match crate::ring0::plat::smp::banda::preparar() {
                    Ok((bytes, _veces)) => BmoStatus::ok_value(bytes),
                    Err(motivo) => BmoStatus::negado(motivo, 0),
                }
            }
            5 => {
                use crate::ring0::plat::smp::banda;
                let i = cuantos as usize;
                if i >= banda::PUNTOS.len() {
                    return BmoStatus::ok_value(0);
                }
                let extra = banda::PUNTOS[i];
                let (vivos, _) = smp::alive();
                if extra > vivos {
                    // No se mide un punto para el que no hay obreros: saldria el
                    // numero de una carrera incompleta, que es el mas bonito de
                    // todos y el unico que no significa nada.
                    return BmoStatus::ok_value(0);
                }
                let (ticks, leidos, todos) = banda::medir(extra);
                // El bus, otra vez: el barrido son decimas de segundo sin
                // bombear el USB, y perder un evento de endpoint PARA LA BOMBA.
                crate::ring0::dev::usb::rescatar_el_bus();
                if !todos {
                    return BmoStatus::ok_value(0);
                }
                match banda::mb_por_segundo(leidos, ticks) {
                    Some(mb) if banda::creible(mb) => BmoStatus::ok_value(mb),
                    // Un numero imposible sale como 0 y no como record. Ver
                    // `banda::creible`.
                    _ => BmoStatus::ok_value(0),
                }
            }
            // Desactivar: los obreros vuelven a `hlt` y ahi se quedan.
            1 => {
                crew::parar();
                crate::ring0::core::dashboard::dashboard_log("[smp] obreros PARADOS");
                BmoStatus::ok_value(0)
            }
            // La prueba. Devuelve la aceleracion x100 --`842` son 8,42x--
            // porque por la puerta solo cabe un numero y una fraccion no
            // se puede mandar entera. El detalle en crudo va a CABINA.
            // *** EL CENSO HILO A HILO, CON SU NOMBRE. (2026-08-24)
            //
            // Peticion del propietario, con estas palabras: *"en `smp all` me gustaria
            // que detalles TODO con nombres CORE y THREAD asi para no decir x12,
            // eso es mentir si pongo asi"*.
            //
            // *** Y tiene razon. "12 de 12" presenta doce cosas como si fueran
            // doce iguales, y no lo son: son **SEIS nucleos con dos hilos cada
            // uno**. Un hilo SMT no es medio nucleo ni es un nucleo: es un
            // sitio mas para meter trabajo en el MISMO nucleo, y cuanto rinde
            // depende de si la faena deja huecos.
            //
            // Es exactamente la misma queja que la de la aceleracion, en otro
            // sitio: **un numero sin el perfil al lado no se puede juzgar.**
            //
            // El kernel ya sabia el nombre --`smp::tipo_de` lleva desde antes
            // repartiendo CORE y THREAD por el APIC id-- y lo mandaba a CABINA
            // como eventos sueltos. Lo que faltaba era **poder pedirlo**, para
            // que el escritorio pueda pintar una tabla en vez de una x.
            //
            // Lo que devuelve, empaquetado porque por la puerta cabe UN numero:
            //
            //    bits  0..8    el estado: 0 maestro, 1 obrero, 2 dormido,
            //                  3 ausente, 4 desconocido
            //    bits  8..16   1 si es CORE, 2 si es THREAD, 0 si no se sabe
            //    bits 16..32   el nucleo FISICO al que pertenece
            //    bits 32..48   cuantos hilos por nucleo dice el PERFIL
            3 => {
                let id = if arg0 > 63 { 63 } else { arg0 as u32 };
                let e = match smp::estado_de(id) {
                    smp::Estado::Maestro => 0u64,
                    smp::Estado::Obrero => 1,
                    smp::Estado::Dormido => 2,
                    smp::Estado::Ausente => 3,
                    _ => 4,
                };
                let t = match smp::tipo_de(id) {
                    "CORE" => 1u64,
                    "THREAD" => 2,
                    _ => 0,
                };
                // ** El nucleo fisico sale del PERFIL, no de un desplazamiento
                // escrito a mano. Que los hermanos SMT sean IDs consecutivos es
                // un hecho de ESTA maquina, y el sitio de un hecho de maquina es
                // el perfil (ley 24). El dia que un CPU los reparta de otra
                // forma, cambia el perfil y esta cuenta no se entera.
                let (fisico, por_nucleo) =
                    match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                        Some(n) if n.nucleos > 0 && n.hilos >= n.nucleos => {
                            let hpc = (n.hilos / n.nucleos) as u64;
                            (if hpc > 0 { id as u64 / hpc } else { id as u64 }, hpc)
                        }
                        _ => (id as u64, 0),
                    };
                BmoStatus::ok_value(e | (t << 8) | (fisico << 16) | (por_nucleo << 32))
            }
            2 => {
                let (alive, _) = smp::alive();
                let (uno, todos, partes) = crew::prueba(alive);

                // *** Y EL BUS SE RESCATA AL SALIR. (2026-08-24)
                //
                // ** `crew::prueba` corre 400.000.000 vueltas DOS veces, o sea
                // que el BSP gira ~medio segundo sin bombear el USB. Y el
                // evento de un endpoint de interrupcion ES EL PERMISO para
                // volver a encolar: perder uno no pierde una pulsacion, PARA LA
                // BOMBA.
                //
                // *** Le paso al propietario en el Ryzen el 24-08: despues de `smp
                // prueba` el teclado se quedo mudo y `reboot` no llego nunca al
                // kernel. **Los tres sintomas que reporto eran UNO.**
                //
                // [!] Se bombea AL SALIR y no durante, a proposito: esto es un
                // CRONOMETRO, y meterle trabajo entre las dos lecturas
                // contaminaria el numero que existe para medir. La medida se
                // queda limpia y el rescate es explicito.
                crate::ring0::dev::usb::rescatar_el_bus();
                crate::ring0::cabina::info("smp", "ticks con UN nucleo", uno);
                crate::ring0::cabina::info("smp", "ticks con todos", todos);
                crate::ring0::cabina::info("smp", "partes que corrieron", partes as u64);
                // * LOS TRES TESTIGOS, siempre, salga bien o mal.
                //
                // En metal el 08-08 esto contesto `0.00x` y no habia nada
                // mas que mirar: "falto una parte" no dice cuantas
                // llegaron. Estos tres numeros parten el camino en los tres
                // sitios donde se puede romper -- entrar al bucle, ver la
                // ronda, terminar la faena-- y la diferencia entre dos
                // consecutivos marca el tramo culpable.
                let (entraron, vieron, hechos) = crew::testigos();
                crate::ring0::cabina::info("smp", "obreros que ENTRARON al bucle", entraron as u64);
                crate::ring0::cabina::info("smp", "obreros que VIERON la ronda", vieron as u64);
                crate::ring0::cabina::info("smp", "obreros que TERMINARON", hechos as u64);
                // ** Y LA MEDIDA, DENUNCIADA POR ELLA MISMA.
                //
                // El 08-11 esto dio `37` ticks para 400 millones de vueltas
                // con los once obreros entrando, viendo y terminando. Los
                // testigos decian que el reparto iba bien y el numero decia
                // que no, y **nadie sospecho del reloj**. Ahora lo dice el.
                crate::ring0::cabina::info("smp", "el hash que dejo la faena", crew::suma_testigo());
                if !crew::medida_creible(uno) {
                    crate::ring0::cabina::fault(
                        "smp",
                        "esa medida es IMPOSIBLE para las vueltas que son: el cronometro miente, no el reparto",
                        uno,
                    );
                }
                if hechos < alive {
                    crate::ring0::cabina::warn(
                        "smp",
                        "faltan obreros por terminar",
                        (alive - hechos) as u64,
                    );
                }
                // * Y la otra mitad del resultado, que no es la velocidad.
                // Doce nucleos calculando a la vez es justo el momento en
                // que un choque de cerrojo aparece si va a aparecer, y una
                // aceleracion contada sin mirar esto es media medida.
                // Ver `plat/spin.rs` y `docs/maestro/SMP_MAESTRO.md`.
                let (choques, pico) = crate::ring0::plat::spin::contention();
                if choques == 0 {
                    crate::ring0::cabina::info("smp", "cerrojos: ni un choque", 0);
                } else {
                    crate::ring0::cabina::warn(
                        "smp",
                        "CHOQUES de cerrojo: alguien entro en el kernel",
                        choques as u64,
                    );
                    crate::ring0::cabina::warn(
                        "smp",
                        crate::ring0::plat::spin::worst(),
                        pico as u64,
                    );
                }
                // ===========================================================
                //  *** LA SEGUNDA MEDIDA, Y ES LA QUE HACE HONESTO EL NUMERO
                // ===========================================================
                //
                // Peticion del propietario el 2026-08-24, con estas palabras: *"el SMP
                // si no es honesto a base de Perfil no me sirve"*. Y tenia razon.
                //
                // ** La faena de arriba es una CADENA DE DEPENDENCIAS --lo dice
                // su propio comentario en `crew.rs`-- y eso la hace perfecta
                // para comprobar que el reparto funciona y **la peor posible
                // para predecir un trabajo de verdad**:
                //
                //    LATENCIA   el nucleo esta casi parado esperando, asi que
                //               el segundo hilo SMT llena esos huecos
                //               -> hasta ~2x por nucleo. EL MEJOR CASO
                //    ANCHO      las unidades saturadas. El segundo hilo no
                //               encuentra hueco porque no lo hay
                //               -> el caso REAL de un calculo denso
                //
                // *** El 24-08 el Ryzen dio 11,59x sobre 12 hilos --el 96,6%--
                // contra una prediccion escrita que decia "~6x es el techo
                // honesto". **La prediccion no estaba equivocada: la faena no
                // era la que suponia.** Y un numero que solo vale para la faena
                // que lo produjo, presentado como "lo que acelera esta maquina",
                // es un numero deshonesto por bueno que sea.
                let (uno_a, todos_a, partes_a) = crew::prueba_ancho(alive);
                // El bus otra vez: son otros ~medio segundo sin bombear.
                crate::ring0::dev::usb::rescatar_el_bus();
                crate::ring0::cabina::info("smp", "ANCHO: ticks con UN nucleo", uno_a);
                crate::ring0::cabina::info("smp", "ANCHO: ticks con todos", todos_a);
                crate::ring0::cabina::info("smp", "ANCHO: partes que corrieron", partes_a as u64);

                // === Y AHORA CONTRA EL PERFIL, que es lo que se pidio =======
                //
                // ** Una aceleracion sin el perfil al lado no se puede juzgar:
                // "11,59x" es magnifico sobre 12 hilos y seria un desastre sobre
                // 64. El numero que importa no es la x -- es **la x DIVIDIDA
                // por lo que esta maquina tiene**, y eso lo dice el perfil del
                // CPU, no una constante escrita aqui (ley 24).
                if let Some(t) = (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                    crate::ring0::cabina::count("smp", "perfil: nucleos FISICOS", t.nucleos as u64);
                    crate::ring0::cabina::count("smp", "perfil: hilos LOGICOS", t.hilos as u64);

                    let lat = if todos > 0 { uno.saturating_mul(100) / todos } else { 0 };
                    let anc = if todos_a > 0 { uno_a.saturating_mul(100) / todos_a } else { 0 };
                    crate::ring0::cabina::count("smp", "x100 LATENCIA -- el mejor caso", lat);
                    crate::ring0::cabina::count("smp", "x100 ANCHO -- el calculo denso", anc);

                    // *** LA FILA QUE CONTESTA DE VERDAD: cuanto rinde cada
                    // nucleo FISICO en un trabajo que satura las unidades. Si
                    // sale cerca de 100, esta maquina esta dando todo lo que
                    // tiene y el SMT no agrega nada -- que es exactamente lo que
                    // le va a pasar a un motor de inferencia.
                    if t.nucleos > 0 {
                        crate::ring0::cabina::count(
                            "smp",
                            "  ...ANCHO por nucleo FISICO, x100",
                            anc / t.nucleos as u64,
                        );
                    }
                    // Y la distancia entre las dos, que es la medida del SMT.
                    if anc > 0 && lat > anc {
                        crate::ring0::cabina::count(
                            "smp",
                            "  ...lo que el SMT agrega, x100 (latencia/ancho)",
                            lat.saturating_mul(100) / anc,
                        );
                    }
                }
                if partes_a == 0 {
                    crate::ring0::cabina::warn(
                        "smp",
                        "[!] la medida de ANCHO no completo: no se puede juzgar",
                        0,
                    );
                }
                crate::ring0::core::dashboard::dashboard_log("[smp] prueba de reparto hecha (latencia + ancho)");
                // *** UNA MEDIDA QUE NO SALIO, CONTESTANDO CERO (12-09).
                //
                // El `else` es "el barrido no completo, no se puede juzgar" -- y
                // el aviso de CABINA de cuatro lineas mas arriba lo dice con
                // esas palabras. Por la puerta salia un `0` con codigo de exito,
                // y el DIRECTOR lo pintaba como **"aceleracion: 0.00"** en rojo:
                // una medida FALLIDA presentada como un cero MEDIDO.
                //
                // El `value` sigue siendo 0 -- `smp_prueba()` contesta lo mismo
                // que ayer-- y el codigo dice que no. Ver L6j.
                if todos > 0 && partes > 0 {
                    BmoStatus::ok_value(uno.saturating_mul(100) / todos)
                } else {
                    BmoStatus::negado(crate::ring0::plat::smp::banda::SIN_JUICIO, 0)
                }
            }
            _ => {
                let (alive, esperados) = smp::despertar(cuantos, |_| {});
                // ** EN QUE ESTA CADA NUCLEO, A CABINA.
                //
                // Lo pidio el propietario con estas palabras: *"que el smp asi
                // natural ayude a verify los cores y hilos: que se estan
                // usando, y que la cabina con filtros pueda decir que esta
                // ejecutando"*.
                //
                // La tabla ya existia en el shell de Ring 0, y al shell de
                // Ring 0 se llega cuando el escritorio NO arranca. Desde la
                // caja del escritorio no habia forma de verla. Ahora va a
                // CABINA, que es el sitio que se mira desde los dos lados y
                // el unico que tiene filtros.
                //
                // El valor de cada evento es `nucleo * 16 + estado`, que
                // cabe en un numero y se lee de un vistazo: la decena es el
                // nucleo y la unidad el estado.
                let hilos = match (crate::ring0::cpu_vendor::profile::active().nucleos)() {
                    Some(t) => (t.hilos as u32).min(32),
                    None => alive + 1,
                };
                // ** CORE o THREAD en el propio mensaje, y en ingles.
                //
                // Lo pidio el propietario para la vista y para los FILTROS, y esa
                // segunda mitad es la que manda: CABINA filtra por texto de
                // modulo y por gravedad, asi que meter la palabra **dentro
                // del mensaje** es lo que permite leer de un vistazo cuantos
                // de los que estan en pie son nucleos de verdad.
                //
                // Y hace falta: `12 hilos` no dice si son doce nucleos o
                // seis con SMT, y de eso depende cuantos obreros pedir --
                // calculo denso quiere seis, no doce.
                for id in 0..hilos {
                    let e = smp::estado_de(id);
                    let t = smp::tipo_de(id);
                    // `CORE OBRERO` / `THREAD DORMIDO`: dos palabras, la
                    // primera dice QUE es y la segunda EN QUE esta.
                    let msg: &'static str = match (t, e) {
                        ("CORE", smp::Estado::Maestro) => "CORE   MASTER",
                        ("CORE", smp::Estado::Obrero) => "CORE   worker",
                        ("CORE", smp::Estado::Dormido) => "CORE   asleep",
                        ("CORE", smp::Estado::Ausente) => "CORE   ABSENT",
                        ("CORE", _) => "CORE   -",
                        ("THREAD", smp::Estado::Maestro) => "THREAD MASTER",
                        ("THREAD", smp::Estado::Obrero) => "THREAD worker",
                        ("THREAD", smp::Estado::Dormido) => "THREAD asleep",
                        ("THREAD", smp::Estado::Ausente) => "THREAD ABSENT",
                        ("THREAD", _) => "THREAD -",
                        _ => "?      -",
                    };
                    crate::ring0::cabina::info("smp", msg, id as u64);
                }
                // ** Y el coste, que es el numero del ahorro. Hoy es
                // incomodo a proposito: el que espera GIRA, no duerme.
                let girando = smp::girando();
                if girando > 0 {
                    crate::ring0::cabina::warn(
                        "smp",
                        "nucleos GIRANDO en vacio al 100% (con MWAIT serian 0)",
                        girando as u64,
                    );
                }
                // ** BIT 63 = LOS OBREROS ESTAN PARADOS.
                //
                // Cabe de sobra --`alive` no pasa de 32-- y hace falta
                // porque el numero solo mentia por omision: `smp stop`
                // seguido de `smp` contestaba `12 de 12`, que es cierto y se
                // lee como "el stop no hizo nada". Ring 3 pinta la mitad que
                // faltaba; el kernel no opina, solo dice el hecho.
                let parados = if crew::parados() { 1u64 << 63 } else { 0 };
                BmoStatus::ok_value(parados | ((alive as u64) << 32) | esperados as u64)
            }
        }
}

//// * Escribe en el disco. Se apunta en CABINA ANTES y DESPUES, pase lo
//// que pase: la primera operacion que cambia el almacen no puede ser
//// silenciosa ni cuando funciona.
pub(super) fn estratos_sellar(_arg0: u64, _arg1: u64) -> BmoStatus {
        crate::ring0::cabina::info(
            "estratos",
            "sellado pedido por un proceso de Ring 3",
            scheduler::current_pid() as u64,
        );
        match crate::ring0::fsys::estratos::seal() {
            Ok(g) => BmoStatus::ok_value(g),
            Err(e) => {
                // *** SELLAR ES GUARDAR UN FICHERO, y esto contestaba EXITO.
                //
                // `ok_value(0)` es codigo de exito con generacion cero -- y una
                // generacion cero se lee como una generacion buena. Los once
                // motivos que `WriteError` distingue existian desde el primer
                // dia y **ninguno cruzaba la puerta**: solo CABINA se enteraba,
                // y CABINA es el panel del kernel, no donde mira quien guarda.
                //
                // ** El `value` sigue siendo 0, asi que quien leia la generacion
                // lee lo mismo que ayer. Lo que cambia es que el codigo dice que
                // NO y las banderas dicen cual de los once. L6i.
                crate::ring0::cabina::warn("estratos", e.name(), 0);
                BmoStatus::negado(e.codigo(), 0)
            }
        }
}

//// ** ADMINISTRAR EL DISCO, y por eso se apunta ANTES de obedecer.
///
//// === Por que esto vive en la superficie y no es una orden de Ring 0 ===
///
//// Porque al shell de Ring 0 **no se vuelve**: en cuanto el compositor
//// reclama la entrada, ese shell deja de leer el teclado. Una orden que
//// solo existe alli es codigo que el propietario de la maquina no puede usar --
//// ya paso con `smp`, con `audio` y con `ext`, y las tres tuvieron que
//// subir. Recortar el disco nace directamente arriba.
///
//// === Y lo que NO cruza esta puerta ===
///
//// El LBA. Ninguna orden de esta familia lo acepta: el rango lo calcula el
//// kernel --la cola libre de ESTRATOS, que sale de `log_head`-- y lo
//// vuelve a comprobar contra la ventana de escritura. Dejar que Ring 3
//// dijera donde recortar seria un borrado apuntable a cualquier sector,
//// incluida la ESP donde vive el arranque del propietario.
pub(super) fn disco(arg0: u64, arg1: u64) -> BmoStatus {
        use crate::ring0::dev::disk::{self, Recorte};
        match arg0 {
            DISCO_OP_TRIM_LIBRE => {
                crate::ring0::cabina::info(
                    "disk",
                    "recorte de la cola libre pedido por un proceso de Ring 3",
                    scheduler::current_pid() as u64,
                );
                // El rango sale del volumen, no del llamante. Sin volumen
                // montado --o con la cola vacia-- no hay nada que devolver, y
                // eso es un motivo propio: no es que el disco no pueda.
                let Some((lba, sectores)) = crate::ring0::fsys::estratos::cola_libre() else {
                    return BmoStatus::ok_value(
                        DISCO_TRIM_SIN_VOLUMEN << DISCO_TRIM_MOTIVO_SHIFT,
                    );
                };
                let (motivo, hechos) = match disk::recortar(lba, sectores) {
                    Recorte::Hecho { sectores, ordenes } => {
                        crate::ring0::cabina::info("disk", "sectores devueltos al disco", sectores);
                        crate::ring0::cabina::info("disk", "ordenes DATA SET MANAGEMENT", ordenes);
                        (DISCO_TRIM_HECHO, sectores)
                    }
                    Recorte::SinDisco => (DISCO_TRIM_SIN_DISCO, 0),
                    Recorte::NoLoSoporta => (DISCO_TRIM_NO_SOPORTADO, 0),
                    // El motivo en palabras va a CABINA porque por la puerta
                    // cabe un numero; el numero dice CUAL de las puertas.
                    Recorte::SinPermiso(why) => {
                        crate::ring0::cabina::warn("disk", why, lba);
                        (DISCO_TRIM_SIN_PERMISO, 0)
                    }
                    Recorte::RangoImposible => (DISCO_TRIM_RANGO, 0),
                    // ** Lo que SI se recorto viaja con el fallo. Un recorte a
                    // medias no se deshace, y callarlo haria que el sistema
                    // volviera a mandar lo que ya estaba hecho.
                    Recorte::Fallo { sectores } => (DISCO_TRIM_FALLO, sectores),
                };
                BmoStatus::ok_value(
                    (motivo << DISCO_TRIM_MOTIVO_SHIFT)
                        | (hechos & DISCO_TRIM_SECTORES_MASK),
                )
            }
            // La barrera a mano. Este disco declara `SOLO_BARRERA` --no tiene
            // condensadores-- asi que esto es literalmente lo unico que tiene
            // para terminar lo que empezo.
            DISCO_OP_BARRERA => BmoStatus::ok_value(disk::flush() as u64),
            // ** EL METRO (D0, 23-09). Solo lee, y el SITIO no lo elige quien
            // llama: la particion de datos desde su principio. `arg1` es
            // cuanto, y el kernel le pone techo.
            DISCO_OP_BANDA => {
                crate::ring0::cabina::info(
                    "disk",
                    "metro de lectura pedido por un proceso de Ring 3",
                    scheduler::current_pid() as u64,
                );
                let (motivo, mb_s) = disk::medir_banda(arg1);
                BmoStatus::ok_value((motivo << 56) | (mb_s & ((1 << 56) - 1)))
            }
            // Una orden que no existe se contesta con cero, igual que en el
            // cursor: quien pregunte de mas se entera, y sin obligar al
            // llamante a distinguir dos formas de "nada".
            _ => BmoStatus::ok_value(0),
        }
}

/// **ARMAR Y SONDEAR LA RED.** Ver `TASK_OP_RED` en `ops.rs` para el por que.
pub(super) fn red(arg0: u64, _arg1: u64) -> BmoStatus {
    use crate::ring0::red as net;
    match arg0 {
        RED_OP_ARMAR => {
            // ** ANTES de obedecer, con quien lo pide. Misma regla que el
            // disco: la primera operacion que cambia el estado de un aparato no
            // puede ser silenciosa ni cuando funciona.
            crate::ring0::cabina::info(
                "red",
                "armar el receptor, pedido por un proceso de Ring 3",
                scheduler::current_pid() as u64,
            );
            // ** AL APARATO, AHORA -- `releer()` y no `identidad()`.
            //
            // *** Esto se armaba mirando LA FOTO DEL ARRANQUE, y esa foto
            // contesta la pregunta equivocada de las dos maneras:
            //
            // ```text
            //    cable quitado despues de arrancar  ->  armaba igual, y el
            //                                           motivo "sin enlace"
            //                                           no salia nunca
            //    cable puesto despues de arrancar   ->  se NEGABA a armar con
            //                                           el cable enchufado
            // ```
            //
            // ** Y aqui no vale el argumento del panel: esto no se repinta a
            // 60 Hz, se TECLEA una vez. Una lectura de MMIO por orden del
            // propietario es exactamente lo que hay que gastar.
            let Some(id) = net::releer() else {
                crate::ring0::cabina::warn("red", "no hay tarjeta que este kernel sepa leer", 0);
                return BmoStatus::ok_value(RED_SIN_TARJETA);
            };
            // *** SIN CABLE NO SE ARMA, y es un motivo propio.
            //
            // ** No es que el anillo falle: es que no van a llegar tramas por
            // correcto que sea todo lo demas. Separarlo de un fallo del anillo
            // es lo que impide pasar una tarde buscando un bug en un driver que
            // funciona -- la leccion del `cero es lo esperado` del paso 1.
            if !id.enlace_arriba() {
                crate::ring0::cabina::warn("red", "el enlace esta ABAJO: enchufa el cable", 0);
                return BmoStatus::ok_value(RED_SIN_ENLACE);
            }
            if !net::rx_start() {
                crate::ring0::cabina::warn("red", "el receptor no se pudo armar", 0);
                return BmoStatus::ok_value(RED_NO_ARMA);
            }
            // Y DESPUES, con lo que trajo la primera vuelta.
            let n = net::rx_poll();
            crate::ring0::cabina::count("red", "receptor ARMADO. tramas en la 1a vuelta", n as u64);
            crate::ring0::cabina::count("red", "  ...y en total desde el arranque", net::rx_tramas());
            BmoStatus::ok_value(RED_ARMADO_OK)
        }
        // ** Vaciar lo que llego. No cambia nada del aparato: devuelve los
        // descriptores que la tarjeta ya uso, que es lo que hace que el anillo
        // no se llene. Devuelve cuantas tramas se leyeron ESTA vez.
        RED_OP_SONDEAR => {
            if !net::rx_activo() {
                return BmoStatus::ok_value(0);
            }
            BmoStatus::ok_value(net::rx_poll() as u64)
        }
        // *** EL GATE RED: se paga aqui UNA vez (E3 de `PLAN_RED_TX`).
        //
        // ** La autoridad la fijo Ring 0 al crear el proceso y no se delega; la
        // firma del `.bex` ya la juzgo el cargador. Lo que falta lo pregunta
        // `bmo_puerta_red::pase`, y cada no sale POR SU NOMBRE en las banderas.
        RED_OP_ABRIR => {
            let pid = scheduler::current_pid();
            let (ms, cupo) = bmo_puerta_red::pase::desempaquetar(_arg1);
            crate::ring0::cabina::info("red", "GATE RED: pide pase el pid", pid as u64);
            let aspace = crate::ring0::mm::vmm::read_cr3();
            match net::puerta::abrir(pid, aspace, autoridad::tiene(pid, autoridad::RED), ms, cupo) {
                Ok(generacion) => {
                    match cap::grant(pid, cap::KIND_RED, cap::RIGHT_READ | cap::RIGHT_WRITE | cap::RIGHT_WAIT, generacion) {
                        Some(h) => BmoStatus::ok_value(h),
                        None => {
                            net::puerta::cerrar(pid);
                            let no = bmo_puerta_red::pase::NoPase::SinMemoria;
                            BmoStatus::negado(no.codigo(), 0)
                        }
                    }
                }
                Err(no) => {
                    crate::ring0::cabina::warn("red", no.texto(), pid as u64);
                    BmoStatus::negado(no.codigo(), 0)
                }
            }
        }
        RED_OP_CERRAR => {
            let pid = scheduler::current_pid();
            let habia = net::puerta::cerrar(pid);
            if let Ok(r) = cap::resolve(pid, _arg1, cap::RIGHT_READ) {
                if r.kind == cap::KIND_RED {
                    cap::revoke(pid, _arg1);
                }
            }
            BmoStatus::ok_value(habia as u64)
        }
        RED_OP_ESTADO => BmoStatus::ok_value(net::puerta::estado()),
        RED_OP_VUELOS => BmoStatus::ok_value(net::puerta::vuelos()),
        RED_OP_LATIDOS => BmoStatus::ok_value(net::puerta::latidos()),
        _ => unsupported(),
    }
}

/// **ENCENDER O APAGAR LA IOMMU** (M0c, 2026-09-24). Ver `plat/iommu.rs`.
///
/// ** Solo quien tiene la pantalla, como el maestro del sonido: la frontera
/// del DMA de toda la maquina no la mueve un programa cualquiera.
///
/// ** Y ANTES DE TOCARLA, `FLUSH CACHE` del disco. El `save` que el escritorio
/// hace justo antes (modo automatico) escribe en FAT32, y esa escritura
/// termina con un OK que es "aceptado", no "guardado": el SSD lo tiene en su
/// cache. Si encender tumba la maquina, lo guardado tiene que estar en el
/// disco de verdad. La maquina trabaja para quien la usa.
pub(super) fn iommu(arg0: u64, arg1: u64) -> BmoStatus {
    use crate::ring0::plat::iommu::en_curso;
    en_curso(Some((arg0, arg1, super::ops::nombre_iommu(arg0))));
    let r = iommu_(arg0, arg1);
    en_curso(None);
    r
}

fn iommu_(arg0: u64, arg1: u64) -> BmoStatus {
    use crate::ring0::plat::iommu;
    let pid = scheduler::current_pid();
    // ** LAS DOS LLAVES (25-09). La AUTORIDAD: solo quien arranco Ring 0 (el
    // escritorio, o un `run` del shell 0) manda en la IOMMU y en la 3060 --
    // una app con la pantalla PRESTADA la tiene delante pero no es suya. Y la
    // PANTALLA, como antes: el volcado y lo que pinta la 3060 son de quien la
    // tiene ahora. El guardian `la-3060` exige que esta llave siga aqui.
    if !crate::ring0::task::autoridad::tiene(pid, crate::ring0::task::autoridad::MAQUINA) {
        crate::ring0::cabina::warn(
            "iommu",
            "la IOMMU y la 3060 son del escritorio que arranco el kernel: negado al pid",
            pid as u64,
        );
        return BmoStatus::negado(iommu::IOMMU_NO_ESCRITORIO, 0);
    }
    if crate::ring0::obj::fb::owner() != Some(pid) {
        crate::ring0::cabina::warn(
            "iommu",
            "la IOMMU es del escritorio (quien tiene la pantalla): negado al pid",
            pid as u64,
        );
        return BmoStatus::negado(iommu::IOMMU_NO_ESCRITORIO, 0);
    }
    // ** L0c5: con el GSP apagado en orden, la 3060 no trabaja hasta el
    // siguiente arranque. Se dice YA, en vez de tocar un timbre que nadie oye.
    if crate::ring0::dev::gpu_apagar::despedido() && !crate::ring0::dev::gpu_apagar::permitida(arg0) {
        return BmoStatus::negado(crate::ring0::dev::gpu_apagar::IOMMU_NO_GSP_APAGADO, 0);
    }
    let r = match arg0 {
        IOMMU_OP_ENCENDER => {
            crate::ring0::cabina::info("iommu", "ENCENDER, pedido por el escritorio", pid as u64);
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("iommu", "el FLUSH del disco antes de encender no se pudo: se sigue", 0);
            }
            iommu::encender()
        }
        IOMMU_OP_APAGAR => {
            // ** EL CANDADO DE E2, al CERRAR: sin IOMMU la 3060 veria toda la
            // RAM, asi que su Bus Master se retira ANTES de apagarla.
            crate::ring0::dev::vblank::apagar(crate::ring0::dev::vblank::E2_APAGADO_CANDADO);
            iommu::apagar()
        }
        IOMMU_OP_CEGAR_GPU | IOMMU_OP_VER_GPU => {
            // ** El BDF lo da la SONDA, y se vuelve a mirar que ahi haya una
            // NVIDIA: cegar el BDF equivocado dejaria ciego a otro aparato.
            let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else {
                return BmoStatus::negado(iommu::IOMMU_NO_SIN_GPU, 0);
            };
            if crate::ring0::dev::pci::cfg_read32(b, d, f, 0) & 0xFFFF != 0x10DE {
                crate::ring0::cabina::warn("iommu", "M0e: en el BDF de la sonda ya no hay una NVIDIA: no se toca", 0);
                return BmoStatus::negado(iommu::IOMMU_NO_SIN_GPU, 0);
            }
            let bdf = (b as u16) << 8 | (d as u16) << 3 | f as u16;
            // El mismo FLUSH que antes de encender: lo que el escritorio
            // acaba de guardar (el save, y la marca `en curso` del modo
            // armado) tiene que estar en el disco si esto tumba la maquina.
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("iommu", "el FLUSH del disco antes de cambiar la 3060 no se pudo: se sigue", 0);
            }
            if arg0 == IOMMU_OP_CEGAR_GPU {
                iommu::cegar(bdf)
            } else {
                // Lo mismo al quitarle la venda: primero E2 fuera.
                crate::ring0::dev::vblank::apagar(crate::ring0::dev::vblank::E2_APAGADO_CANDADO);
                iommu::ver(bdf)
            }
        }
        // ** E2 (2026-09-24): el VBLANK por MSI. Primera escritura en la 3060
        // que no es la IOMMU, asi que lleva el mismo FLUSH: lo que el
        // escritorio acaba de guardar tiene que estar en el disco.
        IOMMU_OP_E2_ENCENDER => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de E2 no se pudo: se sigue", 0);
            }
            crate::ring0::dev::vblank::encender()
        }
        // ** M0d (2026-09-24): la 3060 TRADUCIDA, y lo que se le presta.
        IOMMU_OP_TRADUCIR_GPU => {
            let Some((b, d, f)) = crate::ring0::dev::gpu::bdf() else {
                return BmoStatus::negado(iommu::IOMMU_NO_SIN_GPU, 0);
            };
            if crate::ring0::dev::pci::cfg_read32(b, d, f, 0) & 0xFFFF != 0x10DE {
                return BmoStatus::negado(iommu::IOMMU_NO_SIN_GPU, 0);
            }
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("iommu", "el FLUSH del disco antes de traducir la 3060 no se pudo: se sigue", 0);
            }
            iommu::traducir_gpu((b as u16) << 8 | (d as u16) << 3 | f as u16)
        }
        IOMMU_OP_PRESTAR_PRUEBA => crate::ring0::dev::gpu_prestamo::prestar_prueba(),
        // ** M0d3: el primer DMA que BMO-X le pide a la 3060. Resetea un
        // falcon: el mismo FLUSH de antes, por si tumba la maquina.
        IOMMU_OP_GPU_FUEGO | IOMMU_OP_GPU_FRONTERA => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de M0d3 no se pudo: se sigue", 0);
            }
            if arg0 == IOMMU_OP_GPU_FUEGO {
                crate::ring0::dev::gpu_prestamo::fuego()
            } else {
                crate::ring0::dev::gpu_prestamo::frontera()
            }
        }
        // ** L0b (2026-09-24): FWSEC-FRTS. Los trozos no escriben en el
        // hardware; CORRER si, y lleva el FLUSH como todo lo arriesgado.
        IOMMU_OP_FWSEC_PREPARAR => crate::ring0::dev::gpu_prestamo::fwsec_preparar(arg1),
        IOMMU_OP_FWSEC_TROZO => crate::ring0::dev::gpu_prestamo::fwsec_trozo(arg1),
        IOMMU_OP_FWSEC_CORRER => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de FWSEC no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_prestamo::fwsec_correr()
        }
        // ** L0c2 (2026-09-24): el GSP-RM prestado. Los trozos leen el disco
        // y escriben en marcos que la 3060 todavia no ve; PRESTAR cambia sus
        // tablas, y lleva el FLUSH como todo lo que toca la IOMMU.
        IOMMU_OP_GSP_PREPARAR => super::op_gsp::preparar(),
        IOMMU_OP_GSP_TROZO => super::op_gsp::trozo(arg1),
        IOMMU_OP_GSP_PRESTAR => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de prestar el GSP-RM no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_gsp::prestar()
        }
        IOMMU_OP_GSP_COMPROBAR => crate::ring0::dev::gpu_gsp::comprobar(arg1),
        // ** L0c3a: la primera memoria del PC que la 3060 puede ESCRIBIR.
        IOMMU_OP_GSP_LIBOS => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de prestar LIBOS no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::libos()
        }
        // ** L0c3b: firmware firmado en DOS falcons. El FLUSH, como FWSEC.
        IOMMU_OP_GSP_DESPERTAR | IOMMU_OP_GSP_BOOTER => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de despertar el GSP no se pudo: se sigue", 0);
            }
            if arg0 == IOMMU_OP_GSP_DESPERTAR {
                crate::ring0::dev::gpu_despertar::despertar()
            } else {
                super::op_gsp::booter()
            }
        }
        IOMMU_OP_GSP_ACABAR => crate::ring0::dev::gpu_despertar::acabar(),
        // ** L0c5: el apagado en orden. Firmware firmado otra vez: el FLUSH, como al despertar.
        IOMMU_OP_GSP_DESPEDIR => crate::ring0::dev::gpu_apagar::despedir(),
        IOMMU_OP_GSP_CERRAR | IOMMU_OP_GSP_DESCARGAR => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de apagar el GSP no se pudo: se sigue", 0);
            }
            if arg0 == IOMMU_OP_GSP_CERRAR {
                crate::ring0::dev::gpu_apagar::cerrar()
            } else {
                super::op_gsp::descargador()
            }
        }
        IOMMU_OP_GSP_APAGADO => Ok(crate::ring0::dev::gpu_apagar::info()),
        IOMMU_OP_GSP_LEIDO => crate::ring0::dev::gpu_libos::mover_lectura(arg1),
        IOMMU_OP_GSP_SISTEMA => crate::ring0::dev::gpu_libos::escribir_sistema(),
        IOMMU_OP_GSP_SECUENCIAR => crate::ring0::dev::gpu_despertar::secuenciar(),
        IOMMU_OP_GSP_BAR1 => crate::ring0::dev::gpu_despertar::devolver_bar1(),
        IOMMU_OP_GSP_ESTATICA => crate::ring0::dev::gpu_libos::preguntar_estatica(),
        IOMMU_OP_GSP_OBJETO => crate::ring0::dev::gpu_libos::pedir_objeto(arg1),
        IOMMU_OP_GSP_CONTROL => crate::ring0::dev::gpu_libos::pedir_control(arg1),
        IOMMU_OP_GPU_RAIZ => crate::ring0::dev::gpu_libos::leer_raiz(arg1),
        IOMMU_OP_GPU_TRAMO => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de mapear el tramo no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::mapear_tramo()
        }
        // ** L1d2b: memoria del PC que la 3060 ESCRIBE (el bufer de metodos) y
        // el primer canal. El FLUSH, como el directorio.
        IOMMU_OP_GPU_CANAL => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de pedir el canal no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::pedir_canal()
        }
        IOMMU_OP_GPU_CANAL_ORDEN => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de encender el canal no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::orden_canal(arg1)
        }
        IOMMU_OP_GPU_COPIADOR => crate::ring0::dev::gpu_libos::pedir_copiador(),
        IOMMU_OP_GPU_LEER => crate::ring0::dev::gpu_libos::leer_tramo(arg1),
        IOMMU_OP_GSP_GR => crate::ring0::dev::gpu_libos::preguntar_gr(arg1),
        IOMMU_OP_GPU_GR_MEMORIA => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de mapear los buferes de GR no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::mapear_gr(arg1)
        }
        IOMMU_OP_GPU_GR_MEDIDA => crate::ring0::dev::gpu_libos::medida_gr(arg1),
        // ** M5 G3 y G4: el RM toma NUESTRA VRAM como contexto de GR0 y corre
        // el contexto de oro. El FLUSH, como el canal.
        IOMMU_OP_GSP_GR_PROMOVER | IOMMU_OP_GSP_GR_TRESDE => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del contexto de GR no se pudo: se sigue", 0);
            }
            if arg0 == IOMMU_OP_GSP_GR_PROMOVER {
                crate::ring0::dev::gpu_libos::promover_gr()
            } else {
                crate::ring0::dev::gpu_libos::pedir_tresde()
            }
        }
        IOMMU_OP_GSP_COMPUTO => crate::ring0::dev::gpu_trabajo::pedir_computo(),
        // ** M5d S3: la primera vez que el MOTOR GRAFICO ejecuta algo nuestro.
        // El FLUSH, como la copia.
        IOMMU_OP_GPU_TRABAJO_GR => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del primer trabajo del GR no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::trabajo_gr(arg1)
        }
        // ** M5d S4..S6: el primer SOMBREADOR. El FLUSH, como la copia.
        IOMMU_OP_GPU_SOMBREO => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del primer sombreador no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::sombrear(arg1)
        }
        // ** M5d L: memoria del PC que la 3060 ESCRIBE (el lienzo). El FLUSH,
        // como el bufer de metodos.
        IOMMU_OP_GPU_LIENZO => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del lienzo no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::pintar_lienzo(arg1)
        }
        IOMMU_OP_GPU_LIENZO_LEER => crate::ring0::dev::gpu_trabajo::leer_lienzo(arg1),
        IOMMU_OP_GPU_LIENZO_ESCRIBIR => crate::ring0::dev::gpu_trabajo::escribir_lienzo(arg1),
        // ** M5d B: la salida del blur es memoria del PC que la 3060 ESCRIBE.
        // El FLUSH, como el lienzo.
        IOMMU_OP_GPU_BLUR => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del blur no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::blur(arg1)
        }
        // ** M5d F: 1 MiB del PC que la 3060 ESCRIBE (el fractal). El FLUSH,
        // como el blur.
        IOMMU_OP_GPU_FRACTAL => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del fractal no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::fractal(arg1)
        }
        // ** M5d T0: el mismo MiB que el fractal. El FLUSH, igual.
        IOMMU_OP_GPU_TRIANGULO => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del triangulo no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::triangulo(arg1)
        }
        // ** M5 T1a: el mismo MiB, ahora como destino de la clase 3D.
        IOMMU_OP_GPU_LIMPIAR_3D => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de la limpieza 3D no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::limpiar_3d(arg1)
        }
        // ** M5d E: el mismo MiB. El FLUSH, igual.
        // ** M5 T1c: el mismo MiB, ahora con un triangulo del rasterizador.
        IOMMU_OP_GPU_RASTER => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del triangulo 3D no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::raster(arg1)
        }
        // ** M5 T2a: el mismo dibujo, con color por vertice.
        IOMMU_OP_GPU_COLOR_3D => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del triangulo con color no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::color_3d(arg1)
        }
        // ** M5 T1c: solo lee lo que dejo el ultimo dibujo 3D.
        IOMMU_OP_GPU_DIAG_3D => crate::ring0::dev::gpu_trabajo::diag_3d(arg1),
        // ** M5d G: un fotograma; sin FLUSH del disco: son 32 seguidos.
        IOMMU_OP_GPU_GIRO => crate::ring0::dev::gpu_trabajo::giro(arg1),
        // ** M5d P: un fotograma a pantalla completa; sin FLUSH: van seguidos.
        IOMMU_OP_GPU_PANTALLA => crate::ring0::dev::gpu_trabajo::pantalla(arg1),
        // ** El volcado: `arg1` = la VA del lienzo del escritorio.
        IOMMU_OP_GPU_VOLCADO => crate::ring0::dev::gpu_trabajo::volcado(arg1),
        // ** Cada fotograma: sin FLUSH del disco ni nada que no sea la copia.
        IOMMU_OP_GPU_VOLCADOR => crate::ring0::dev::gpu_trabajo::volcador(arg1),
        // ** M6 V0: el video; sin FLUSH del disco: los fotogramas van seguidos.
        IOMMU_OP_GPU_VIDEO_FORMATO => crate::ring0::dev::gpu_trabajo::video_formato(arg1),
        IOMMU_OP_GPU_VIDEO => crate::ring0::dev::gpu_trabajo::video(arg1),
        // ** P1: EL PASE. La puerta elige el MOTOR de la GPU que hay (hoy solo
        // la 3060; otra tarjeta seria `.or_else(|| su_motor())`); `pase_gpu`
        // es neutro y no sabe cual es.
        IOMMU_OP_GPU_PASE => crate::ring0::dev::pase_gpu::orden(pid, arg1, crate::ring0::dev::gpu_trabajo::pase_nv::motor()),
        IOMMU_OP_GPU_ESCENA => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de la escena no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_trabajo::escena(arg1)
        }
        IOMMU_OP_GPU_CANAL_GR => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de pedir el canal de GR0 no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::pedir_canal_gr()
        }
        // ** L1d3: la primera vez que la 3060 EJECUTA algo nuestro. El FLUSH,
        // como el canal.
        IOMMU_OP_GPU_COPIA => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de la primera copia no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::copiar(arg1)
        }
        IOMMU_OP_GPU_DIRECTORIO => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes del directorio de paginas no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::poner_directorio()
        }
        IOMMU_OP_GPU_VRAM => {
            if !crate::ring0::dev::disk::flush() {
                crate::ring0::cabina::warn("gpu", "el FLUSH del disco antes de escribir en la VRAM no se pudo: se sigue", 0);
            }
            crate::ring0::dev::gpu_libos::probar_vram()
        }
        IOMMU_OP_E2_APAGAR => {
            let estaba = crate::ring0::dev::vblank::apagar(crate::ring0::dev::vblank::E2_APAGADO_ORDEN);
            Ok(estaba as u64)
        }
        _ => return BmoStatus::err(ERROR_INVALID_ARGUMENT),
    };
    match r {
        Ok(v) => BmoStatus::ok_value(v),
        Err(motivo) => BmoStatus::negado(motivo, 0),
    }
}
