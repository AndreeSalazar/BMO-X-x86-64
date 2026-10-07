//! **Lo que se le pide a `CURRENT_TASK`.** Los `TASK_OP_*`.
//!
//! Cuarenta y dos operaciones sobre la propia tarea: quien soy, ceder, salir,
//! abrir un canal, reclamar la pantalla, lanzar un programa, preguntar por el
//! sistema. Es la familia mas grande del contrato **y la que mas crece**, y por
//! eso tiene fichero propio.
//!
//! [!] Dos operaciones no pueden llevar el mismo numero, y casi paso: la
//! autopsia se escribio en `0x1D` y `0x1E`, que ya eran PANTALLA_SOLTAR y
//! ENTRADA_SOLTAR -- o sea que **leer el informe de un fallo habria soltado la
//! pantalla**. Hoy lo vigila `build.ps1` barriendo el kernel entero.

pub const TASK_OP_GET_PID: u64 = 0x01;

pub const TASK_OP_GET_TID: u64 = 0x02;

pub const TASK_OP_YIELD: u64 = 0x03;

pub const TASK_OP_EXIT: u64 = 0x04;

/// `INVOKE(CURRENT_TASK, CHANNEL_OPEN, index)` -> the caller's estuary
/// capability handle for BMO Channel `index`. Fails with NEEDS_CAP when
/// the process was not granted that estuary.
pub const TASK_OP_CHANNEL_OPEN: u64 = 0x05;

/// `INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)` -> emit up to 8 bytes of
/// text (packed little-endian in `packed`, NUL-terminated within the word)
/// to the kernel bootstrap console. This is the debug door that lets the
/// very first Ring 3 program prove the CPL3->CPL0 path visually before a
/// real console capability/estuary service exists; it will migrate to a
/// console handle once the display server lands.
pub const TASK_OP_CONSOLE_WRITE: u64 = 0x06;

/// Crea un endpoint atendido por este proceso: `arg0` es el estuario por el
/// que se le entregaran las llamadas, y devuelve el handle del endpoint.
///
/// Es lo unico que Endpoint RPC agrega a la superficie. Llamar, atender y
/// responder NO son operaciones nuevas: son lo que `INVOKE` y `WAIT` ya
/// significan cuando el handle resuelve a un endpoint o a un reply. La
/// superficie sigue siendo de tres puertas.
pub const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;

/// LEE de la consola asignada al proceso. La PAREJA de `CONSOLE_WRITE`.
///
/// Sin esto un programa lanzado desde un terminal no puede recibir nada: la
/// capability del teclado la tiene el compositor, y darsela a cada hijo seria
/// romper la exclusividad que hace que la entrada tenga un solo propietario. El
/// terminal que lo lanzo le pasa lo que se teclea, por el mismo objeto que ya
/// usa para hablar.
///
/// Devuelve `(n << 56) | bytes_LE` con hasta SIETE bytes, y `n = 0` cuando no
/// hay nada todavia -- que no es un error: un programa que sondea no debe morir
/// por preguntar.
///
/// ## ** UN PAQUETE NO CRUZA NUNCA UN SALTO DE LINEA
///
/// Si entre los bytes disponibles hay un `\n`, el paquete **acaba ahi**, con el
/// salto incluido. Es contrato y no detalle de implementacion: lo cumplen el
/// kernel (`ring0/obj/console.rs`, `read_entry`) y el emulador del banco de
/// pruebas (`bmo-lower::emu`), y **tiene que cumplirlo cualquier otra cosa que
/// algun dia sirva esta operacion**.
///
/// Lo que compra: el que lee LINEAS no necesita guardar nada entre llamadas.
/// Sin la regla lo necesita, y el codigo que emite el compilador no tiene
/// donde -- cada `ACCEPT` de COBOL es una emision independiente. Eso costo un
/// fallo mudo real: la calculadora del escritorio manda `12.50\n3\n4\n` de
/// golpe, el primer paquete traia `12.50\n3` y el `3` --la operacion que se
/// pedia-- se perdia con el resto del paquete. El motor contestaba una cuenta
/// que nadie habia pedido y nadie se enteraba.
///
/// El que lee bytes en crudo no pierde nada con la regla: recibe lo mismo, solo
/// que en paquetes que acaban donde acaba una linea.
pub const TASK_OP_CONSOLE_READ: u64 = 0x0F;

/// Acumula hasta 8 bytes de una RUTA en el renglon del proceso.
///
/// La superficie congelada no acepta punteros, asi que una ruta viaja de 8 en
/// 8 y la consume la siguiente operacion que necesite una. **Un solo renglon**
/// para `EJECUTAR`, `DIR_ABRIR` y los dos de archivo: inventar un mecanismo
/// por cada consumidor seria tener cuatro sitios donde se pierde un byte.
pub const TASK_OP_RUTA: u64 = 0x0B;

/// Lanza lo acumulado con [`TASK_OP_RUTA`] y vacia el renglon. Devuelve el tid.
///
/// * Estas tres (`0x0C`-`0x0E`) vivian **solo dentro del kernel**: se anadieron
/// a `ring0/syscall.rs` y nunca subieron aqui, asi que el guardian de deriva de
/// `build.ps1` no las miraba -- no puede comparar lo que en un lado no existe.
/// La superficie es el contrato; el kernel es una implementacion suya.
pub const TASK_OP_EJECUTAR: u64 = 0x0C;

/// Crea una consola y devuelve su handle de LECTURA. Quien la crea es el
/// terminal: la consola es suya y la drena a su ritmo.
pub const TASK_OP_CONSOLA_CREAR: u64 = 0x0D;

/// Abre un directorio del volumen de datos. La ruta se acumula antes con
/// [`TASK_OP_RUTA`] -- el mismo renglon que usa `EJECUTAR`.
pub const TASK_OP_DIR_ABRIR: u64 = 0x0E;

/// Abre un archivo del volumen de datos para LEER. La ruta viene del renglon.
pub const TASK_OP_ARCHIVO_ABRIR: u64 = 0x10;

/// Abre un archivo del volumen de datos para ESCRIBIR (lo crea).
///
/// Son dos operaciones y no un argumento de modo porque crear puede fallar por
/// motivos que abrir no tiene --volumen de solo lectura, nombre que no es 8.3--
/// y mezclarlas obligaria a devolver errores que no aplican a la mitad de las
/// llamadas.
pub const TASK_OP_ARCHIVO_CREAR: u64 = 0x11;

/// Saca hasta 7 bytes: `(n << 56) | bytes_LE`. `n == 0` = se acabo.
///
/// La cuenta va en el byte alto y NO se corta en el primer cero, al reves que
/// la consola: un archivo no es texto y un `\0` en medio es un dato.
/// Reinicia la maquina. No vuelve.
///
/// Reiniciar es tocar puertos de E/S (`0xCF9`, el 8042), que Ring 3 no puede
/// --ni debe-- hacer; por eso es una operacion y no un permiso ambiental.
/// **Hoy no esta atada a una capability**, igual que `EJECUTAR`: las dos
/// quieren la misma el dia que exista.
pub const TASK_OP_REINICIAR: u64 = 0x12;

/// Conectar con un endpoint de RPC ya creado.
pub const TASK_OP_ENDPOINT_CONNECT: u64 = 0x08;

/// **Soltar la pantalla sin morirse.** Pareja de reclamarla.
///
/// Existe porque prestar la pantalla y quedarse la ENTRADA no es prestar: es
/// dejar a un programa pintando en una habitacion cerrada. Las dos capabilities
/// van juntas o no van.
pub const TASK_OP_PANTALLA_SOLTAR: u64 = 0x1D;

/// Soltar la entrada. La otra mitad de [`TASK_OP_PANTALLA_SOLTAR`].
pub const TASK_OP_ENTRADA_SOLTAR: u64 = 0x1E;

/// **El censo de audio**: que el aparato diga como quiere las muestras.
///
/// Devuelve 1 si encontro uno de reproduccion; los ocho numeros van a CABINA,
/// porque por la puerta cabe uno. Paso 0 de `docs/maestro/AUDIO_MAESTRO.md`.
pub const TASK_OP_AUDIO_CENSO: u64 = 0x28;

// -- S1 del suelo de Ring 3: LA VENTANA DE UN APARATO -----------------------
//
// *** EL ARGUMENTO ES QUE APARATO, Y NO UNA DIRECCION. Es la decision entera y
// va escrita en el contrato, no solo en el kernel:
//
// > Un proceso que puede decir *"mapeame la fisica 0x1000"* es un proceso que
// > esta pidiendo ser el kernel.
//
// Con esa operacion, en tres pasos --mapear donde viven las tablas de pagina,
// ponerse el bit U/S, quitar el NX-- los siete muros del aislamiento caen a la
// vez. No por un bug: por la operacion funcionando como se pidio.
//
// Asi que el proceso nombra un aparato de una lista CERRADA, la fisica sale del
// censo del kernel, y aun asi pasa por `bmo-mmio-juicio` antes de mapearse. Ver
// `docs/plan/terminado/PLAN_SUELO_RING3.md` y `docs/identidad/EL_AISLAMIENTO.md`.

/// **Tomar la ventana de registros de un aparato.**
///
/// `arg0` = cual. Hoy la lista es de uno: `0` = el controlador xHCI.
///
/// Devuelve un handle `KIND_MMIO`, y lo que concede hoy es menos de lo que el
/// nombre sugiere, a proposito:
///
/// ```text
///    UNA pagina      la primera del BAR
///    SOLO LECTURA    la pagina se mapea sin escritura, y sin RIGHT_WRITE
///    exclusiva       un propietario a la vez, como la pantalla y el audio
/// ```
///
/// ** Escribir en un aparato desde Ring 3 es otra decision, y va **despues** de
/// que leer este probado en metal.
pub const TASK_OP_APARATO_TOMAR: u64 = 0x2E;

/// Devolverla sin morirse. La pareja de [`TASK_OP_APARATO_TOMAR`], por el mismo
/// motivo que `PANTALLA_SOLTAR`: sin ella, la unica forma de soltar un aparato
/// seria terminar.
///
/// [!] Y si el proceso muere sin soltarla, el kernel la recupera igual
/// (`revoke_all`). Un driver que revienta no puede dejar su aparato ocupado
/// hasta el proximo reinicio.
pub const TASK_OP_APARATO_SOLTAR: u64 = 0x2F;

// -- S3 del suelo de Ring 3: EL LATIDO --------------------------------------

/// **Tomar el LATIDO**: el derecho a que `WAIT` despierte cuando late el reloj.
///
/// Devuelve un handle `KIND_LATIDO` con solo `RIGHT_WAIT`. No lleva argumentos
/// y **no es exclusivo**: el reloj no se gasta, y cien procesos pueden esperarlo
/// a la vez.
///
/// # Como se usa, y por que hacen falta los dos pasos
///
/// ```text
///    visto = INVOKE(h, LATIDO_OP_CUENTA)      cuantos van
///    visto = WAIT(h, visto, timeout_ns)       duerme hasta el siguiente
/// ```
///
/// *** Preguntar ANTES es obligatorio. Sin el testigo, la primera espera se
/// duerme contra un numero inventado: si acierta por debajo vuelve en el acto y
/// si acierta por encima **no despierta nunca**.
pub const TASK_OP_LATIDO_TOMAR: u64 = 0x30;

pub const TASK_OP_INFO: u64 = 0x13;

/// Un dato de TEXTO. `arg0` = campo (`INFO_TXT_*`), `arg1` = que trozo.
///
/// Devuelve 8 bytes empaquetados en little-endian, el cero corta -- el mismo
/// formato que `TASK_OP_RUTA` y `TASK_OP_CONSOLE_WRITE`, y por la misma razon:
/// aqui no hay `copy_to_user`, asi que el texto viaja por valor.
pub const TASK_OP_INFO_TEXTO: u64 = 0x14;

/// Reclama raton + teclado. **Exclusivo**: mientras un proceso lo tenga, el
/// shell de Ring 0 deja de leer el teclado fisico. No es un reparto -- dos
/// lectores de la misma cola se robarian las letras.
pub const TASK_OP_INPUT_CLAIM: u64 = 0x0A;

/// Reclama la pantalla. Tambien exclusivo.
pub const TASK_OP_FRAMEBUFFER_CLAIM: u64 = 0x09;

/// **Pide un bloque de memoria.** `arg0` = bytes. Devuelve el handle de una
/// capability `KIND_MEMORIA`; la direccion se pregunta con `MEM_OP_BASE`.
///
/// * NO es un `malloc`: entrega **un bloque grande, entero y contiguo**, y no
/// hay forma de devolverlo. El asignador no es trabajo del kernel -- se escribe
/// encima, en Ring 3, con la politica que quiera cada lenguaje. El caso que lo
/// decidio es DOOM: pide ~8 MiB una vez y se los administra el.
pub const TASK_OP_MEMORIA_PEDIR: u64 = 0x15;

/// **Cuantas lineas del log del kernel se pueden leer.** `arg0` elige el dato:
/// 0 = disponibles ahora, 1 = escritas desde el arranque (la resta son las que
/// se cayeron por el borde del anillo).
///
/// * Esto **no da privilegio, da vista**. Ring 3 no ejecuta nada en Ring 0:
/// pide texto por su numero y recibe bytes, igual que `TASK_OP_INFO`. En un
/// sistema de capabilities *ver* y *poder* son cosas separadas, y juntarlas es
/// como se acaba teniendo un "modo administrador".
///
/// Hace falta desde que **el escritorio es el arranque**: mientras el
/// compositor tiene la pantalla, el panel del kernel no se pinta, y con el
/// desaparecia el relato entero de como arranco la maquina.
pub const TASK_OP_KLOG_INFO: u64 = 0x16;

/// **Ocho bytes de una linea del log.** `arg0` = linea (**0 es la mas
/// reciente**), `arg1` = trozo de 8 en 8. Cero = se acabo, igual que
/// `TASK_OP_INFO_TEXTO`.
pub const TASK_OP_KLOG_TEXTO: u64 = 0x17;

/// ** LA AUTOPSIA de un fallo de Ring 3.
///
/// El klog cuenta el relato de la maquina; esto guarda el INFORME de cada
/// muerte: vector, codigo de error, la direccion que se toco, el `rip`, la
/// pila, **que programa era** y lo ultimo que llego a escribir.
///
/// Existe porque la linea que dejaba CABINA --`CPL3: tarea eliminada, BMO sigue
/// vivo` con el `rip` detras-- alcanza para saber QUE paso y no para
/// saber DONDE. Un fallo que no se puede mandar a nadie se cuenta de memoria, y
/// contar un fallo de memoria es como se pierden los fallos.
///
/// * El kernel captura en RAM y **no toca el disco**: se corre dentro de un
/// fault, y el fallo puede ser justo del disco. Quien lo persiste es Ring 3,
/// que esta vivo y tiene la capability. El kernel CONTESTA, no actua.
///
/// `arg0` = campo (ver `AUTOPSIA_*`), y para `AUTOPSIA_RENGLONES`, `arg1` = que
/// informe (**0 es el mas reciente**).
pub const TASK_OP_AUTOPSIA_INFO: u64 = 0x1F;

/// **Ocho bytes de un renglon del informe.** `arg0` empaqueta
/// `(informe << 32) | fila` y `arg1` es el trozo de 8 en 8. Cero = se acabo.
///
/// Van los dos indices en un solo argumento porque la puerta tiene tres y dos
/// ya estan ocupados por la operacion y el trozo. Es la misma aritmetica que
/// usa `INPUT_OP_*` para el raton.
pub const TASK_OP_AUTOPSIA_TEXTO: u64 = 0x20;

/// **Reclamar el SONIDO.** Devuelve un handle `HandleKind::AudioEngine`: el
/// derecho a hacer ruido, exclusivo como la pantalla.
///
/// Es el CONTRATO y no un driver. Lo unico que suena hoy es el altavoz del PC;
/// HD Audio --codec, DMA, anillo de buffers-- es otra pieza y no existe todavia.
/// Se escribe el contrato primero a proposito: un motor de audio sin la
/// pregunta de quien tiene derecho a usarlo acaba en un sistema donde cualquier
/// programa pita encima de cualquier otro.
///
/// Las operaciones sobre el handle son `AUDIO_OP_*`.
/// **CABINA leida desde Ring 3.** `arg0` = campo (`CABINA_*`), `arg1` = que
/// evento (0 = el mas reciente).
///
/// El klog ya se podia leer, pero es la transcripcion en texto plano: **no
/// lleva severidad**. CABINA si -- severidad, capa y modulo por evento-- y sin
/// eso una linea que dice que el SMP levanto doce nucleos llega igual que
/// cualquier otra.
///
/// Contesta y no concede: **ni una de estas dos operaciones escribe nada**. Ver
/// y poder son cosas separadas.
pub const TASK_OP_CABINA_INFO: u64 = 0x23;

/// Ocho bytes del modulo o del mensaje. `arg0` = `(evento << 32) | cual`,
/// `arg1` = el trozo. El cero corta.
pub const TASK_OP_CABINA_TEXTO: u64 = 0x24;

/// **Abrir MI PROPIA imagen, para leer los datos que lleva dentro.**
///
/// Devuelve un handle de `KIND_ARCHIVO` de LECTURA sobre el `.bex` desde el que
/// se lanzo este proceso. No lleva argumentos: el programa no dice **cual** --
/// dice *"el mio"*, y quien sabe cual es el kernel.
///
/// ** Por que no vale con `TASK_OP_ARCHIVO_ABRIR` y la ruta: porque abrir por
/// ruta es **pedir por nombre lo que se tiene por derecho**. Un programa que
/// escribe su propia ruta podria escribir otra, y en un sistema de capabilities
/// eso es exactamente lo que no se hace. Ademas, un binario movido de sitio
/// dejaria de encontrarse a si mismo.
///
/// Falla si el kernel no recuerda de donde salio -- pasa con los programas que
/// el propio kernel embebe, que no vienen de ninguna ruta.
pub const TASK_OP_MI_PAQUETE: u64 = 0x25;

/// **Quien me lanzo**, como TID. `0` si no hay nadie.
///
/// Una app dibuja en su memoria y se la OFRECE al que la puso en pantalla (ver
/// `<bmo/superficie.h>`). Ofrecer exige nombrar al destinatario, y el hijo no
/// tiene forma de nombrarlo: [`MEM_OP_OFRECER`] habla en tids, y el tid del
/// compositor no aparece en ningun sitio de su espacio.
///
/// ** Y por eso NO es un registro de nombres. La pregunta no es *"quien manda"*
/// --eso seria autoridad ambiental, y quien la leyera podria pedirle cosas a
/// alguien que nunca se las ofrecio-- sino **"quien me lanzo a MI"**: local,
/// concreta, y no concede nada.
///
/// El `0` no es un error: es la respuesta correcta cuando nadie compone --
/// lanzado desde el shell de Ring 0--, y el programa que lo reciba se cae al
/// camino de la pantalla exclusiva.
pub const TASK_OP_MI_PADRE: u64 = 0x26;

/// **Abrir un archivo SIN esperar a que llegue entero.**
///
/// Misma ruta y mismo handle que [`TASK_OP_ARCHIVO_ABRIR`]; la diferencia es
/// **cuando vuelve**. `ABRIR` no vuelve hasta que el fichero esta en RAM, y con
/// un `.bex` de 813 KB eso deja al que lo pidio sin existir durante toda la
/// lectura -- si el que pide es el escritorio, el escritorio no pinta.
///
/// Este vuelve en cuanto sabe que el archivo esta ahi. Los bytes llegan a
/// trozos.
///
/// ** Desde el paso D1 del disco (2026-09-23), si el kernel tiene HILO DEL
/// DISCO, los trozos los trae el: manda la orden, suelta el disco y duerme
/// hasta la IRQ. El handle sale entonces con `RIGHT_WAIT`, y quien lo abrio
/// duerme con `WAIT(handle, ARCH_OP_LISTO, plazo)` mientras llegan -- el CPU es
/// de otro mientras el aparato trabaja. Sin hilo, lo de siempre: cada
/// [`ARCH_OP_LISTO`] trae un trozo girando dentro del kernel.
pub const TASK_OP_ARCHIVO_ASINC: u64 = 0x27;

/// **Tomar lo que otro me haya ofrecido.** Devuelve un handle `KIND_PRESTADO`, o
/// `0` si no hay nada. El mapeo ocurre DENTRO de esta llamada, en el espacio de
/// quien la hace -- por eso se toma en vez de que el otro te lo coloque.
pub const TASK_OP_TOMAR: u64 = 0x1C;
// ** ESTE 0x1C LO COMPARTIA `TASK_OP_LIENZO_REFLEJO`, y no era simetria: era una
// constante MUERTA okupando un numero VIVO. Quien escribiera el nombre bonito
// habria invocado esto. Borrada el 2026-09-02; el esquema de `KIND_LIENZO` salio
// del kernel hace tiempo (ver `obj/loan.rs` y `docs/identidad/LIENZO.md`).
//
// [!] Y no se RESERVA el numero, al reves que con `BMO_CHANNEL_KICK`. Alli la
// regla era *"el numero no se recicla"* porque estaba libre; aqui **ya estaba
// reciclado**: es de `TOMAR` y lo lleva usando desde siempre.
//
// Lo encontro `R15` el dia que se escribio. Este proyecto ya habia pagado el
// mismo error --`MEMORIA_PEDIR` en el `0x12` de `REINICIAR`, y pedir memoria
// habria reiniciado la maquina-- y desde entonces la regla es listar los
// opcodes ORDENADOS antes de elegir. Ahora hay quien lo comprueba.

pub const TASK_OP_AUDIO_CLAIM: u64 = 0x21;

/// Soltar el sonido siendo su propietario y **seguir vivo**.
///
/// Va desde el primer dia por lo que costo que faltara en la pantalla: alli la
/// unica forma de dejar de ser propietario era morir, asi que el escritorio no podia
/// prestarla ni queriendo. El mismo hueco aqui seria que el primer programa que
/// pite se queda el aparato para siempre.
pub const TASK_OP_AUDIO_RELEASE: u64 = 0x22;

/// **Despertar los otros nucleos.** Devuelve `alive<<32 | esperados`, ambos sin
/// contar el BSP.
///
/// * Existe porque el comando `smp` vivia **solo en el shell de Ring 0**, y ese
/// shell deja de leer el teclado en cuanto el compositor reclama `KIND_INPUT`.
/// O sea: habia codigo que no se podia ejecutar desde donde el propietario estaba
/// sentado. Un mando al que no se llega es un mando que no existe.
///
/// Y encaja sin tocar nada de lo congelado: la superficie sigue siendo tres
/// syscalls, y esto es **una fila mas** en la tabla de operaciones de la tarea
/// -- que es exactamente por donde la arquitectura dice que la API crece.
///
/// [!] Bloquea mientras dura el bring-up (hasta ~10 ms por nucleo). Quien la
/// llama deberia avisar en pantalla ANTES, porque es la unica operacion del
/// sistema que puede tardar un segundo entero.
pub const TASK_OP_SMP_DESPERTAR: u64 = 0x1B;

/// **SELLAR: cierra una transaccion vacia en ESTRATOS.** Devuelve la generacion
/// nueva, o 0 si no se pudo.
///
/// * **Es la primera operacion de la superficie que ESCRIBE EN EL DISCO**, y
/// por eso lleva su propia operacion en vez de esconderse detras de un campo de
/// otra: lo que cambia el estado del almacen se pide por su nombre.
///
/// Lo que hace es deliberadamente lo mas chico posible: ni un bloque de
/// datos, el mismo estrato, y el commit va a **la copia del superbloque que no
/// manda**. Recorre el camino entero --`FLUSH CACHE`, barrera, commit, vaciar
/// otra vez-- sin poder perder nada aunque salga mal. Ver
/// `ring0/fsys/estratos.rs::sellar`.
pub const TASK_OP_ESTRATOS_SELLAR: u64 = 0x18;

/// **CREAR UN FICHERO EN ESTRATOS.** `arg0` es la suborden (`ES_CREAR_*`).
///
/// === Por que el contenido viaja de 8 en 8, y no por un puntero ===
///
/// Porque la superficie congelada no acepta punteros: no hay `copy_from_user` y
/// traducir una direccion de Ring 3 contra su espacio es infraestructura que no
/// existe. El nombre usa el renglon de [`TASK_OP_RUTA`], que ya sirve para
/// `EJECUTAR` y para abrir; el contenido usa el suyo.
///
/// ** Y el contenido lleva CUENTA EXPLICITA (`arg2`) donde la ruta se corta en
/// el primer cero. No es un capricho: en una ruta un `\0` no puede aparecer, y
/// **en un fichero si**. Un fichero que se corta en su primer byte nulo no es un
/// fichero: es la mitad de uno.
///
/// Devuelve la generacion nueva, o `0` con el motivo en CABINA.
pub const TASK_OP_ES_GESTO: u64 = 0x2A;

/// **EL HANDLE SOBRE UN HIJO QUE YO LANCE.** `arg0` = el tid que devolvio
/// [`TASK_OP_EJECUTAR`]. Devuelve un handle de `KIND_TAREA`, o falla.
///
/// No concede nada: la capability se concedio al LANZAR, y esto solo la
/// encuentra -- exactamente lo mismo que hace [`TASK_OP_CHANNEL_OPEN`] con el
/// canal que ya se sembro. Quien no lanzo ese proceso no tiene la capability, y
/// aqui no hay nada que buscar.
///
/// ** Y ese es el reparto que evita un `root` con otro nombre: el DIRECTOR
/// cierra lo que EL lanzo porque tiene su handle, no porque sea el DIRECTOR.
/// Ver `docs/plan/PLAN_DIRECTOR.md`, paso 3.
pub const TASK_OP_HIJO: u64 = 0x2B;

/// El cursor de ESTRATOS: `arg0` es la pregunta ([`ES_NODO_RAIZ`] y compania),
/// `arg1` su argumento cuando lo lleva.
pub const TASK_OP_ES_NODO: u64 = 0x19;

/// **ADMINISTRAR EL DISCO.** `arg0` es la orden (los `DISCO_OP_*`).
///
/// === Por que UNA operacion con ordenes dentro, y no una por cada cosa ===
///
/// Porque son **el mismo acto sobre el mismo aparato**, y la superficie ya tiene
/// esa forma para el cursor de ESTRATOS ([`TASK_OP_ES_NODO`]) y para la entrada.
/// Una fila por orden llenaria la tabla de opcodes de cosas que solo se
/// diferencian en el verbo, y esta tabla ya casi se choco consigo misma una vez.
///
/// === Que NO lleva, y es lo importante ===
///
/// **Ni un LBA.** Ninguna orden de esta familia acepta que el llamante diga
/// donde tocar: el rango lo calcula el kernel y lo comprueba contra la ventana
/// de escritura. Un recorte apuntable desde Ring 3 no seria una operacion del
/// sistema -- seria un borrado a distancia con formulario.
///
/// Lo mismo que `TASK_OP_ESTRATOS_SELLAR`: **lo que cambia el estado del almacen
/// se pide por su nombre**, y aqui ademas se contesta con el motivo cuando no se
/// puede (ver `DISCO_TRIM_*`).
pub const TASK_OP_DISCO: u64 = 0x29;

/// **ARMAR Y SONDEAR LA RED desde donde vive el propietario.** `arg0` = `RED_OP_*`.
///
/// ## *** Por que esta operacion existe (2026-08-24)
///
/// El `net` del escritorio esta escrito para SOLO INFORMAR, y cuando el propietario
/// pidio `net rx` en el Ryzen le contesto *"receptor apagado (net rx en Ring
/// 0)"* -- mandandole a un shell **al que no se vuelve**.
///
/// > Un camino que solo existe en Ring 0 es un camino que el propietario de su propia
/// > maquina no puede tomar.
///
/// La respuesta no es que el escritorio toque la NIC: es que **Ring 3 pida y el
/// kernel decida**, con la misma forma y la misma regla que `TASK_OP_DISCO` --
/// se apunta en CABINA antes y despues.
///
/// [!] Hasta E3 no podia transmitir. Desde el 2026-09-13 puede de UNA manera:
/// `RED_OP_ABRIR`, el GATE RED -- se paga una vez, cada NO tiene nombre, y lo
/// que sale pasa por el grifo del kernel con el radar de 4 ms mirando.
pub const TASK_OP_RED: u64 = 0x2C;

/// **Que cuenta la placa de si misma.** `arg0` = `PLACA_OP_*`, `arg1` = indice.
///
/// Contesta y no concede, igual que `INFO` y el klog: no cambia nada.
pub const TASK_OP_PLACA: u64 = 0x2D;

/// Armar el receptor. Idempotente: armar dos veces no arma dos anillos.
pub const RED_OP_ARMAR: u64 = 0x01;
/// Vaciar lo que llego y devolver los descriptores. Cuantas tramas se leyeron.
pub const RED_OP_SONDEAR: u64 = 0x02;
/// **El GATE RED.** `arg1` = plazo en ms (32 bits bajos) y cupo de tramas (32
/// altos). Devuelve un handle `HandleKind::Red`; el buzon queda en
/// [`RED_BUZON_VA`]. Si no, `ERROR_NEGADO` con el motivo en las banderas
/// (`bmo_puerta_red::pase::NoPase`).
pub const RED_OP_ABRIR: u64 = 0x03;
/// Cerrar el pase propio. `arg1` = el handle.
pub const RED_OP_CERRAR: u64 = 0x04;
/// `(abierto << 63) | (motivo << 56) | (ultimo no << 48) | (negadas << 24) | salieron`.
pub const RED_OP_ESTADO: u64 = 0x05;
/// `(despegues << 32) | aterrizajes` del anillo de salida: tramas dadas a la
/// tarjeta y tramas que la tarjeta devolvio ENVIADAS.
pub const RED_OP_VUELOS: u64 = 0x06;
/// Latidos del GATE RED servidos desde el arranque. Con el reloj de Ring 3 dice
/// cada cuanto late de verdad.
pub const RED_OP_LATIDOS: u64 = 0x07;
/// **Lee un registro MII del PHY** (2026-10-03, "por que va a 10 Mbit").
/// `arg1` = el registro, 0..=15. Devuelve `(1 << 63) | valor`; `0` si no hay
/// tarjeta o el PHY no contesta. Solo lee: no pide autoridad.
///
/// Con `arg1 = 0xFF`, el VEREDICTO de `bmo_net::mii`: `(1 << 63) | causa |
/// (lo que anunciamos << 8) | (lo que anuncia el otro << 24) | (lo comun <<
/// 40)`, las velocidades en Mbit. Causa: 1 bien, 2 ANUNCIAMOS POCO (lo dejo
/// el apagado de Windows con WOL: se arregla renegociando), 3 el otro da
/// poco, 4 cable de dos pares, 5 sin autonegociar, 6 PHY apagado, 7 sin
/// enlace.
pub const RED_OP_MII: u64 = 0x08;
/// **Anuncia 10/100/1000 y RENEGOCIA** el enlace (lo que hace el driver de
/// cualquier sistema al arrancar). Pide la autoridad RED. El enlace cae unos
/// segundos. Devuelve `(1 << 63) | (bmcr << 32) | (gbcr << 16) | anar`.
pub const RED_OP_RENEGOCIAR: u64 = 0x09;
/// Donde queda el buzon del pase en el proceso. 7 paginas.
pub const RED_BUZON_VA: u64 = 0x0000_0002_0000_0000;

pub const RED_ARMADO_OK: u64 = 0;
/// Sin cable. Es un motivo propio: no es que el anillo falle, es que no van a
/// llegar tramas por correcto que sea todo lo demas.
pub const RED_SIN_ENLACE: u64 = 1;
pub const RED_NO_ARMA: u64 = 2;
pub const RED_SIN_TARJETA: u64 = 3;

pub const PLACA_OP_CUANTAS: u64 = 0x01;
/// `arg1` = indice. La firma en los cuatro bytes bajos; bit 32 = paso su suma,
/// bit 33 = es AML y aqui no se ejecuta.
pub const PLACA_OP_TABLA: u64 = 0x02;
/// La base de ECAM, o 0 si no hay MCFG.
pub const PLACA_OP_ECAM: u64 = 0x03;
/// Los registros del primer IOMMU, o 0 si no hay IVRS -- y eso significa que
/// **nada limita adonde escribe un aparato con DMA**.
pub const PLACA_OP_IOMMU: u64 = 0x04;


// == LA PUERTA DE LA ORQUESTA ==============================================
//
// *** BMO-X no es un sistema operativo: es Bare Metal ORQUESTAL. Un SO
// multiplexa --finge que cada programa esta solo y le deja improvisar-- y una
// orquesta COORDINA: cada atril sabe su parte, su entrada y lo que cuesta.
//
// Hasta hoy la maquina sabia repartir una faena entre doce nucleos --
// `smp::crew`, probado en metal-- y **nadie le habia dado nunca una de verdad**:
// el unico que llamaba a `repartir` era un banco de pruebas. Estas dos
// operaciones son el gesto de entrada del director.
//
// ** LO QUE NO SE HACE, Y ES LA DECISION MAS IMPORTANTE DE LAS DOS:
//
//    lo que NO   Ring 3 manda un PUNTERO A FUNCION y el nucleo lo llama
//    lo que SI   Ring 3 NOMBRA una parte del catalogo y manda los datos
//
// La primera es una escalada de privilegios con otro nombre: codigo de Ring 3
// corriendo con el privilegio del kernel, en un nucleo que ni siquiera tiene
// TSS propia. No hay forma de hacerla segura y no se intenta.
//
// Y la segunda no es un arreglo: **es lo que hace una orquesta**. A una orquesta
// se le dan partituras escritas. BMO-X ya hace exactamente esto dos veces --dos
// syscalls con un opcode, y los 62 intrinsecos de `sem-asm` en una tabla-- asi
// que el catalogo es el mismo concepto un piso mas arriba. El catalogo vive en
// `bmo-orquesta`, y ahi se prueba.

/// **PONER UN NUMERO EN EL ATRIL**, antes de decir *tocad*.
///
/// `arg0` = que campo (0 destino, 1 origen, 2 total, 3 dato), `arg1` = el valor.
///
/// ** De uno en uno porque por la puerta caben DOS numeros por llamada y un
/// encargo lleva cuatro. Es el mismo idioma que `OP_RUTA` antes de
/// `OP_EJECUTAR`: se acumula y despues se ejecuta. Inventar aqui una convencion
/// nueva seria tener dos formas de pasar argumentos largos en el mismo sistema.
///
/// [!] `destino` y `origen` son direcciones **de Ring 3**, las que devolvio
/// `MEM_OP_BASE`. La traduccion a fisica la hace el kernel, y por eso una app no
/// puede nombrar memoria que no sea suya: `memory::fisica_de` solo traduce
/// dentro de sus propios bloques.
pub const TASK_OP_ATRIL: u64 = 0x31;

/// **TOCAD.** `arg0` = numero de parte del catalogo, `arg1` = cuantos atriles
/// como mucho (`0` = los que el perfil crea convenientes).
///
/// Devuelve cuantos atriles tocaron de verdad, o un error si el encargo no pasa
/// el juez de `bmo_orquesta::se_puede_tocar`.
///
/// ** BLOQUEA hasta que la barrera se cierra. Es lo correcto: quien reparte
/// tiene que poder contar con que al volver el trabajo esta hecho -- y los
/// obreros giran en `pause`, asi que no hay a quien avisar despues.
pub const TASK_OP_TOCAR: u64 = 0x32;

/// **MIS ARGUMENTOS**: lo que venia detras de la ruta en `OP_RUTA`, 8 bytes por
/// trozo. `arg0` = numero de trozo; el valor `0` dice que se acabo.
///
/// `EJECUTAR` parte la linea en el PRIMER espacio: delante la ruta, detras el
/// texto que el hijo lee aqui. Un texto de mas de 128 bytes no se recorta: el
/// lanzamiento se niega con `ERROR_INVALID_ARGUMENT` antes de crear el proceso.
///
/// ** Un trozo de argumento es un trozo de ruta: quien recibe un nombre de
/// fichero se lo pasa a `OP_RUTA` palabra a palabra, sin copiarlo.
pub const TASK_OP_ARGUMENTOS: u64 = 0x33;

/// **EL MANDO DEL MAESTRO**: mover el fader del sonido o callarlo (2026-09-22).
/// `arg0` = [`AUDIO_MANDO_FADER`] o [`AUDIO_MANDO_MUDO`], `arg1` = el valor.
///
/// *** NO PIDE EL SONIDO, y esa es la razon de que exista. Reclamar el sonido es
/// EXCLUSIVO: mientras DOOM suena, nadie mas puede tenerlo, y hasta hoy el
/// volumen solo se movia reclamandolo -- o sea que con un juego sonando el
/// escritorio no podia tocar ni el volumen. Producir y mandar eran el mismo
/// permiso; en una mesa de verdad no: el que toca no lleva el fader maestro.
///
/// ** Y SOLO LA PUEDE USAR QUIEN TIENE LA PANTALLA, o sea el escritorio. El
/// maestro es la perilla de la habitacion: un programa puede producir sonido,
/// no subirle la ganancia al oido de nadie. Cualquier otro recibe
/// `ERROR_PERMISSION_DENIED`, con el motivo en CABINA.
///
/// Lo que el mando deja puesto se LEE sin handle, por `OP_INFO`:
/// `INFO_AUDIO_MAESTRO`, `INFO_AUDIO_MEDIDOR` e `INFO_AUDIO_LIMITE`.
pub const TASK_OP_AUDIO_MANDO: u64 = 0x34;

/// **Encender o apagar la IOMMU** (M0c, 2026-09-24). `arg0` = `IOMMU_OP_*`.
///
/// Solo quien tiene la pantalla. Antes de encender, el kernel hace `FLUSH
/// CACHE` del disco: lo guardado justo antes llega al disco de verdad. Si el
/// `COMPLETION_WAIT` no vuelve en 10 ms, la IOMMU se APAGA sola y contesta
/// `ERROR_NEGADO` con `IOMMU_NO_CONTESTA` en las banderas. `Ok` lleva
/// `us | eventos << 32`.
pub const TASK_OP_IOMMU: u64 = 0x35;

/// **MI GS DE RING 3** (PROTON-X P1d, 2026-09-27). `arg0` = la base del GS de
/// ESTE hilo, de la mitad de usuario (`< 2^47`; si no, `ERROR_INVALID_ARGUMENT`).
///
/// Un `.exe` de Windows x64 encuentra su TEB en `gs:[0x30]` y su PEB en
/// `gs:[0x60]`: lo pone el compilador de Microsoft en cualquier funcion. Hasta
/// P1d el GS de Ring 3 valia 0 para todos. El kernel lo guarda por hilo y el
/// relevo lo pone en `KERNEL_GS_BASE` SOLO cuando cambia: los hilos que no lo
/// piden no pagan nada. Contesta los ciclos del `wrmsr` (0 si ya estaba), que
/// es lo que cuesta un relevo entre dos hilos con distinto GS.
pub const TASK_OP_PON_GS: u64 = 0x36;

/// **LA DECLARACION DE IMAGEN** (PROTON-X P0.4b, 2026-09-30). `arg0` = la VA
/// de la tabla de partes, dentro de un bloque PROPIO (`TASK_OP_MEMORIA_PEDIR`):
/// por la puerta no viajan punteros, el kernel la lee por la fisica del
/// bloque. `arg1` = cuantas partes (hasta [`IMAGEN_MAX_PARTES`]).
///
/// Cada parte son [`IMAGEN_PARTE_BYTES`]: sus bytes en memoria (u64), de que
/// PE es (u16: 0, 1, 2... en orden) y si es su codigo (u8, 1 o 0), el resto a
/// cero. De cada PE, el codigo antes que los datos.
///
/// Se juzga UNA vez contra la RAM libre de ese momento menos el margen del
/// kernel: dinamico, no un tope escrito. `Ok` lleva la base de la ventana de
/// imagenes; el NO, `ERROR_NEGADO` con el motivo (`IMAGEN_*`) en las banderas
/// y su valor. Una declaracion por proceso; al morir, todo vuelve a cero.
pub const TASK_OP_IMAGEN_DECLARAR: u64 = 0x37;
/// Donde quedo la parte `arg0` (su VA).
pub const TASK_OP_IMAGEN_PARTE: u64 = 0x38;
/// Sellar la parte de CODIGO `arg0`: R+X sin W, irreversible (W^X).
pub const TASK_OP_IMAGEN_SELLAR: u64 = 0x39;
/// Lo que mide una parte en la tabla.
pub const IMAGEN_PARTE_BYTES: u64 = 16;
/// Cuantas partes caben en una declaracion.
pub const IMAGEN_MAX_PARTES: u64 = 128;
/// Los NO de la imagen. Espejo de `ring0::obj::imagen`.
pub const IMAGEN_YA_DECLARADA: u32 = 1;
pub const IMAGEN_SIN_TABLA: u32 = 2;
pub const IMAGEN_DE_MAS: u32 = 3;
pub const IMAGEN_PARTE_VACIA: u32 = 4;
pub const IMAGEN_DESORDENADA: u32 = 5;
/// El valor lleva lo que pide y lo que hay, en MiB: `pide << 32 | hay`.
pub const IMAGEN_SIN_RAM: u32 = 6;
pub const IMAGEN_SIN_VENTANA: u32 = 7;
pub const IMAGEN_SIN_RANURA: u32 = 8;
pub const IMAGEN_SIN_MARCOS: u32 = 9;
pub const IMAGEN_NO_MAPEA: u32 = 10;
pub const IMAGEN_NO_HAY_PARTE: u32 = 11;
pub const IMAGEN_NO_ES_CODIGO: u32 = 12;
pub const IMAGEN_YA_SELLADA: u32 = 13;
pub const IMAGEN_SIN_NX: u32 = 14;
pub const IMAGEN_NO_REMAPEA: u32 = 15;

/// **LA RESERVA** (PROTON-X P0.4c, 2026-09-30): la memoria que un juego pide
/// EN MARCHA. En la ventana de reserva ([`RESERVA_VENTANA_BASE`],
/// [`RESERVA_VENTANA_BYTES`]) HACER las paginas de `[arg0, arg0+arg1)` que
/// faltan: a cero, R+W, sin ejecucion; alineado a pagina y como mucho
/// [`RESERVA_MAX_POR_VEZ`]. Se juzga cada vez contra la RAM libre de ese
/// momento menos el margen del kernel. `Ok` lleva los bytes nuevos; el NO,
/// `ERROR_NEGADO` con el motivo (`RESERVA_*`) y su valor. Reservar (elegir
/// direcciones) no pasa por aqui: es cuenta de Ring 3. Al morir, todo vuelve.
pub const TASK_OP_RESERVA_HACER: u64 = 0x3A;
/// DESHACER las paginas de `[arg0, arg0+arg1)`: `Ok` lleva los bytes devueltos.
pub const TASK_OP_RESERVA_DESHACER: u64 = 0x3B;
/// **LA RAIZ DEL SIGUIENTE HIJO** (H3 de `docs/plan/PLAN_HERMES.md`, 03-10):
/// la ruta acumulada con [`TASK_OP_RUTA`] es la carpeta donde nacera
/// ENCERRADO el siguiente hijo que lance quien llama. Se lee DENTRO de la raiz
/// de quien llama, asi que un hijo nunca ve mas disco que su padre. Encerrado
/// quiere decir: toda ruta que escriba se resuelve debajo de esa carpeta (sin
/// `..`, sin letras de unidad) y las operaciones que ven el volumen entero sin
/// ruta (el cursor de ESTRATOS, sellar, el disco, las versiones) le dicen NO.
/// `Ok` o `ERROR_PERMISSION_DENIED` con el motivo en CABINA.
pub const TASK_OP_RAIZ_HIJO: u64 = 0x3C;
/// **EL ATRIL DEL FONDO** (`PLAN_EL_SONIDO.md` S4e, 03-10): la musica de
/// fondo del escritorio y sus avisos, en un banco SUYO que el orquestador
/// mezcla aunque otro proceso tenga el sonido (DOOM), agachandola sola bajo
/// un aviso o la app. Solo quien tiene la pantalla; a cualquier otro,
/// `ERROR_PERMISSION_DENIED` con el motivo en CABINA. `arg0` = orden `[0..4)`
/// ([`AUDIO_FONDO_BANCO`]...) | canal `[4..8)` | izq `[8..17)` | der
/// `[17..26)` | bucle `[26]`; `arg1` segun la orden. Las muestras son siempre
/// S16 mono a 48 kHz.
pub const TASK_OP_AUDIO_FONDO: u64 = 0x3D;
/// **LOS SUB-DIRECTORES** (H4.3 de `PLAN_LOS_DOCE_DIRECTORES`, 07-10): una
/// faena de la app repartida en Ring 3 por los obreros (`smp all` antes).
/// Ver `bmo_orquesta::ring3` (el juez, con banco) y
/// `ring0::plat::smp::ring3` (lo que pisa el CPU).
///
/// `SUB_INFO`: cuantos obreros sanos hay (las partes que se pueden pedir son
/// estos + 1, la de la app). `0`: no hay obreros.
pub const TASK_OP_SUB_INFO: u64 = 0x3E;
/// `SUB_PREPARAR`: `arg0` = los bloques (uno por parte, seguidos), `arg1` =
/// `bmo_orquesta::ring3::empaquetar(bytes de cada bloque, partes)`.
pub const TASK_OP_SUB_PREPARAR: u64 = 0x3F;
/// `SUB_REPARTIR`: `arg0` = la funcion (`extern "sysv64" fn(k, n, arg)`,
/// vuelve a `FIN`), `arg1` = `arg`. Las partes `1..n` a los obreros; la `0`
/// la corre quien llama al volver. `ERROR_NEGADO` con el motivo
/// (`bmo_orquesta::ring3::NoRing3`) si no.
pub const TASK_OP_SUB_REPARTIR: u64 = 0x40;
/// `SUB_ESPERAR`: `0` si sigue en marcha; si no, `ACABADA` (bit 63) | un bit
/// por parte que NO salio bien (la rehace quien llama). Al tope (5 s) las
/// que faltan cuentan como no hechas.
pub const TASK_OP_SUB_ESPERAR: u64 = 0x41;
/// Los NO de los sub-directores (espejo de `bmo_orquesta::ring3::NoRing3`).
pub const SUB_PARTES: u32 = 1;
pub const SUB_NO_ES_RING3: u32 = 2;
pub const SUB_BLOQUE: u32 = 3;
pub const SUB_DESBORDA: u32 = 4;
pub const SUB_OCUPADO: u32 = 5;
pub const SUB_SIN_OBREROS: u32 = 6;
/// **EL FUTEX** (07-10, como `futex(2)` de Linux): esperar en una palabra
/// de 32 bits de Ring 3. `arg0` = la direccion (alineada a 4), `arg1` =
/// `bmo_futex::empaquetar(visto, ms)`. Si la palabra ya no vale `visto`,
/// `ERROR_NEGADO` con `FUTEX_CAMBIO` en el acto; si vale, el hilo duerme
/// hasta un `FUTEX_DESPERTAR` en esa direccion o el plazo (`u32::MAX` ms:
/// sin plazo). Al volver, quien llama mira su palabra otra vez.
pub const TASK_OP_FUTEX_ESPERAR: u64 = 0x42;
/// `FUTEX_DESPERTAR`: `arg0` = la direccion, `arg1` = cuantos como mucho.
/// Devuelve cuantos desperto.
pub const TASK_OP_FUTEX_DESPERTAR: u64 = 0x43;
/// Los NO del futex (espejo de `bmo_futex::NoFutex`).
pub const FUTEX_DESALINEADA: u32 = 1;
pub const FUTEX_NO_ES_RING3: u32 = 2;
pub const FUTEX_CAMBIO: u32 = 3;
/// La ventana de reserva: 384 GiB desde `0x20_0000_0000`, hasta el final
/// de `PML4[0]` (era de 128: Cyberpunk aparta mas de 130 GiB al arrancar).
pub const RESERVA_VENTANA_BASE: u64 = 0x0000_0020_0000_0000;
pub const RESERVA_VENTANA_BYTES: u64 = 384 << 30;
/// Lo mas que se hace o deshace en una llamada.
pub const RESERVA_MAX_POR_VEZ: u64 = 64 << 20;
/// Los NO de la reserva. Espejo de `ring0::obj::reserva`.
pub const RESERVA_RANGO_MALO: u32 = 1;
pub const RESERVA_FUERA: u32 = 2;
pub const RESERVA_DE_MAS: u32 = 3;
/// El valor lleva lo que pide y lo que hay, en MiB: `pide << 32 | hay`.
pub const RESERVA_SIN_RAM: u32 = 4;
/// El valor: cuantos bytes SI se hicieron (y se quedan hechos).
pub const RESERVA_SIN_MARCOS: u32 = 5;
pub const RESERVA_NO_MAPEA: u32 = 6;
pub const IOMMU_OP_ENCENDER: u64 = 0x01;
pub const IOMMU_OP_APAGAR: u64 = 0x02;
/// Cegar la 3060 (M0e): su DMA no alcanza la RAM; sus interrupciones si pasan.
pub const IOMMU_OP_CEGAR_GPU: u64 = 0x03;
/// Devolverle la vista (de paso).
pub const IOMMU_OP_VER_GPU: u64 = 0x04;
/// E2: el VBLANK de la 3060 por MSI. Solo si esta CIEGA (el candado): con la
/// IOMMU apagada o la entrada de paso, `ERROR_NEGADO` con `IOMMU_NO_GPU_VE`.
/// `Ok` lleva `bdf << 32 | cabeza`.
pub const IOMMU_OP_E2_ENCENDER: u64 = 0x05;
/// E2 apagado: el aviso quitado y el Bus Master retirado. `Ok(1)` si estaba.
pub const IOMMU_OP_E2_APAGAR: u64 = 0x06;
/// M0d: la 3060 TRADUCIDA por su dominio (vacio al principio: sigue sin ver
/// nada, pero por el camino que despues presta). `Ok` lleva `us | bdf << 32`.
pub const IOMMU_OP_TRADUCIR_GPU: u64 = 0x07;
/// M0d: prestarle la pagina de prueba, SOLO LECTURA, en 0x10000000. Pide la
/// 3060 TRADUCIDA. `Ok` lleva la direccion fisica de la pagina.
pub const IOMMU_OP_PRESTAR_PRUEBA: u64 = 0x08;
/// M0d3: la PRUEBA DE FUEGO -- el DMA del falcon del GSP trae la pagina
/// prestada a su DMEM y se compara. `Ok` lleva las palabras que cuadran (1024).
/// Pide la 3060 TRADUCIDA, la pagina prestada y el Bus Master de E2.
pub const IOMMU_OP_GPU_FUEGO: u64 = 0x09;
/// M0d3: la FRONTERA -- lo mismo desde una direccion NO prestada: `Ok(1)` si
/// la IOMMU lo paro con un evento con el BDF de la 3060 y esa direccion.
pub const IOMMU_OP_GPU_FRONTERA: u64 = 0x0A;
/// L0b: FWSEC -- el kernel juzga el descriptor que empieza en `arg1` (offset en
/// la ROM). `Ok` lleva cuantos trozos de 4 KiB hay que copiar.
pub const IOMMU_OP_FWSEC_PREPARAR: u64 = 0x0B;
/// L0b: FWSEC -- copiar el trozo `arg1` de la ROM al bufer. Sin escribir en la 3060.
pub const IOMMU_OP_FWSEC_TROZO: u64 = 0x0C;
/// L0b: FWSEC -- la orden FRTS, la firma del fusible, el prestamo, IMEM y DMEM
/// por DMA seguro y STARTCPU. Vuelve al arrancar: si acabo, `INFO_GPU_FWSEC`.
pub const IOMMU_OP_FWSEC_CORRER: u64 = 0x0D;
/// L0c2: el GSP-RM -- el kernel abre `fw/gsp/gsp.bin` y `bootldr.bin` por su
/// cuenta, encuentra `.fwimage` y la firma ga10x y pide marcos. `Ok` lleva
/// cuantos trozos de 512 KiB hay que copiar.
pub const IOMMU_OP_GSP_PREPARAR: u64 = 0x0E;
/// L0c2: copiar el trozo `arg1` del `.fwimage` a sus marcos, EN ORDEN.
pub const IOMMU_OP_GSP_TROZO: u64 = 0x0F;
/// L0c2: la radix3, la `GspFwWprMeta` y el prestamo. `Ok` = paginas prestadas.
pub const IOMMU_OP_GSP_PRESTAR: u64 = 0x10;
/// L0c2: leer el trozo `arg1` POR LA RADIX3 y la IOMMU, EN ORDEN; al ultimo,
/// `INFO_GPU_GSP` dice si el BLAKE3 cuadra con lo copiado y con la 570.144.
pub const IOMMU_OP_GSP_COMPROBAR: u64 = 0x11;
/// L0c3a: los argumentos de LIBOS, los tres logs, `rmargs`, las dos colas y la
/// pagina de vaciado, prestados a la 3060 ESCRIBIBLES; y cada puntero que
/// seguira el GSP, seguido por la IOMMU. `Ok` = punteros seguidos.
pub const IOMMU_OP_GSP_LIBOS: u64 = 0x12;
/// L0c3b: la pagina de vaciado registrada y el GSP con sus argumentos de LIBOS
/// en el buzon. Vuelve al arrancarlo; si se paro, `INFO_GPU_DESPIERTO`.
pub const IOMMU_OP_GSP_DESPERTAR: u64 = 0x13;
/// L0c3b: con el GSP parado, el booter FIRMADO al SEC2 con la WPR meta en su
/// buzon. Vuelve al arrancar el SEC2 (`Ok` = la firma usada).
pub const IOMMU_OP_GSP_BOOTER: u64 = 0x14;
/// L0c3b: el SEC2 parado con MAILBOX0 = 0, y el registro OS del GSP escrito.
/// Despues, el RISC-V del GSP se mira en `INFO_GPU_DESPIERTO`.
pub const IOMMU_OP_GSP_ACABAR: u64 = 0x15;
/// L0c4b1: mover el `readPtr` de la CPU sobre la cola del GSP a `arg1`
/// (0..63): devolverle al GSP los huecos de lo ya leido. `Ok` = el de antes.
pub const IOMMU_OP_GSP_LEIDO: u64 = 0x16;
/// L0c4b2a: SetSystemInfo y SetRegistry a las paginas 0 y 1 de la cola de la
/// CPU, armados por el kernel con lo que lee del PCI; ANTES de despertar el
/// GSP y una sola vez. `Ok(2)` = el `writePtr` nuevo.
pub const IOMMU_OP_GSP_SISTEMA: u64 = 0x17;
/// L0c4b2c: un tramo (1 ms) del secuenciador que pidio el GSP; el kernel lo
/// lee el mismo de la cola del GSP y solo deja tocar el falcon del GSP. `Ok`
/// = como va (`INFO_GPU_DESPIERTO_BUZON` con selector 2); con `SEC_HECHO`,
/// acabo y el GSP-RM volvio. Llamar hasta eso o un NO.
pub const IOMMU_OP_GSP_SECUENCIAR: u64 = 0x18;
/// L0c4b3a: devolverle a BAR1 (0xB80F40) el valor que tenia antes del
/// secuenciador, el del GOP: la pantalla vuelve a ver lo que pinta la CPU.
/// Solo tras `GSP_INIT_DONE`. `Ok` = el de ahora | el del GSP-RM << 32.
pub const IOMMU_OP_GSP_BAR1: u64 = 0x19;
/// L1a: la primera RPC -- GET_GSP_STATIC_INFO a la cola de la CPU, armada por
/// el kernel, y el timbre del GSP. La respuesta llega por la cola del GSP.
/// `Ok` = la pagina | el numero de la pregunta << 32.
pub const IOMMU_OP_GSP_ESTATICA: u64 = 0x1A;
/// L1b: GSP_RM_ALLOC de uno de NUESTROS objetos en el RM, armado por el
/// kernel: `arg1` = 0 cliente, 1 dispositivo, 2 subdispositivo (asas fijas,
/// las de `bmo_gpu_ga10x::objeto`). `Ok` = la pagina | el numero << 32.
pub const IOMMU_OP_GSP_OBJETO: u64 = 0x1B;
/// L1b: GSP_RM_CONTROL sobre nuestro subdispositivo, armado por el kernel:
/// `arg1` = 0 el P-state (la lista, `bmo_gpu_ga10x::control`). `Ok` = la
/// pagina | el numero << 32.
pub const IOMMU_OP_GSP_CONTROL: u64 = 0x1C;
/// L1c2: la CPU escribe en la VRAM por la ventana PRAMIN de BAR0: una pagina en
/// la direccion fija `bmo_gpu_ga10x::vram::PRUEBA`, guardada y devuelta. `Ok`
/// = `vram::empaquetar` (buenas | devueltas << 16 | ventana devuelta << 31 |
/// ventana de antes << 32).
pub const IOMMU_OP_GPU_VRAM: u64 = 0x1D;
/// L1c3: la raiz de NUESTRO espacio de direcciones -- una pagina de VRAM fija
/// (`bmo_gpu_ga10x::vram::DIRECTORIO`) a cero, y `SET_PAGE_DIRECTORY` con la
/// direccion y el espacio fijos. Una vez por arranque. `Ok` = la pagina | el
/// numero << 32; la respuesta llega por la cola del GSP.
pub const IOMMU_OP_GPU_DIRECTORIO: u64 = 0x1E;
/// L1d0: leer la entrada `arg1` (0..4) de la raiz PD3 por PRAMIN, solo lectura.
/// `Ok` = la PDE cruda. Solo con el directorio puesto.
pub const IOMMU_OP_GPU_RAIZ: u64 = 0x1F;
/// L1d1: mapear las 16 paginas de `vram::TRAMO` en `vram::TRAMO_VA` de nuestro
/// espacio (tablas a cero, entradas de la hoja a la raiz, releidas). Una vez
/// por arranque, con la entrada de la raiz vacia. `Ok` = escrituras |
/// releidas iguales << 16.
pub const IOMMU_OP_GPU_TRAMO: u64 = 0x20;
/// L1d2b: pedir el canal `AMPERE_CHANNEL_GPFIFO_A` (`bmo_gpu_ga10x::canal`):
/// sus tres paginas del tramo a cero, el bufer de metodos prestado escribible
/// en `canal::IOVA_METODOS`, y el `GSP_RM_ALLOC` con sus 368 B fijos. Una vez
/// por arranque, con el tramo mapeado. `Ok` = pagina | numero << 32.
pub const IOMMU_OP_GPU_CANAL: u64 = 0x21;
/// L1d2c: una orden que ENCIENDE el canal, `arg1` = su indice en
/// `control::Control::TODOS` (solo BIND y GPFIFO_SCHEDULE), con el canal
/// pedido. `Ok` = pagina | numero << 32.
pub const IOMMU_OP_GPU_CANAL_ORDEN: u64 = 0x22;
/// L1d3: pedir el copiador (`AMPERE_DMA_COPY_B` sobre COPY2, colgado del
/// canal; `bmo_gpu_ga10x::copia`). Con el canal pedido. `Ok` = pagina |
/// numero << 32.
pub const IOMMU_OP_GPU_COPIADOR: u64 = 0x23;
/// L1d2d y L1d3: la primera copia VRAM a VRAM por el canal, `arg1` = la ficha
/// de `GET_WORK_SUBMIT_TOKEN`. Una vez por arranque, con el copiador pedido.
/// `Ok` = `copia::empaquetar(buenas, GP_GET, pagado, lanzada, us)`.
pub const IOMMU_OP_GPU_COPIA: u64 = 0x24;
/// L1d3, el diagnostico: leer UNA palabra de VRAM del tramo por PRAMIN,
/// `arg1` = la direccion (`copia::legible`: dentro del tramo, alineada a 4).
/// Solo lectura. `Ok` = la palabra. Con el bit 63, `arg1` bajo es UN registro
/// de BAR0 de `copia::registro_legible` (el reloj de la ventana del timbre).
/// Con los bits 63:62 = 01, `arg1` bajo es la BASE de una lista de ejecucion
/// (`copia::lista_legible`) y 33:32 que se lee: 0 la config de su CHRAM, 1 la
/// de su timbre, 2 la entrada de NUESTRO canal en su CHRAM.
pub const IOMMU_OP_GPU_LEER: u64 = 0x25;
/// M5 G0: preguntar al RM que buferes de contexto pide el motor grafico
/// (`INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO`), sobre sus asas INTERNAS:
/// `arg1` = cliente | subdispositivo << 32 (las de `GET_GSP_STATIC_INFO`).
/// Una pregunta. `Ok` = pagina | numero << 32.
pub const IOMMU_OP_GSP_GR: u64 = 0x26;
/// M5 G1: pedir el canal de GR0 (`canal::GR`: chid 2, motor GR0, instancia y
/// GPFIFO en las paginas 5 y 6 del tramo, su bufer de metodos en 0x3A01_0000).
/// Con el canal de copia pedido; una vez por arranque. Despues, BIND y
/// SCHEDULE por `IOMMU_OP_GPU_CANAL_ORDEN` con `AtarGr`/`ProgramarGr`.
pub const IOMMU_OP_GPU_CANAL_GR: u64 = 0x27;
/// M5 G2: `arg1` = bytes | cero_hasta << 32 (de `gr::repartir`): pone a
/// cero `[gr::VRAM, +cero_hasta)` por PRAMIN, mapea `bytes` desde `gr::VRAM`
/// en `gr::VA` (`gr::mapear`) e invalida la MMU. Una vez por arranque, con el
/// tramo mapeado. `Ok` = escrituras | releidas << 16 | paginas a cero << 32.
pub const IOMMU_OP_GPU_GR_MEMORIA: u64 = 0x28;
/// M5 G3, antes: una de las ocho medidas del RM de G0, `arg1` = fila (0..8,
/// el orden de `gr::TABLA`) | medida << 32. El kernel rehace con ellas el
/// reparto (`gr::desde_medidas`, `gr::repartir`) y lo exige igual al de G2.
/// `Ok` = las filas puestas (un bit cada una).
pub const IOMMU_OP_GPU_GR_MEDIDA: u64 = 0x29;
/// M5 G3: `NV2080_CTRL_CMD_GPU_PROMOTE_CTX` con los nueve buferes de G2 (560
/// B de `gr::promover`), sobre nuestro subdispositivo. Con G1, G2 y las ocho
/// medidas; una vez por arranque. `Ok` = pagina | numero << 32.
pub const IOMMU_OP_GSP_GR_PROMOVER: u64 = 0x2A;
/// M5 G4: AMPERE_B (0xC797) colgado del canal de GR0, sin parametros: el RM
/// corre el contexto de oro. Tras G3; una vez por arranque. `Ok` = pagina |
/// numero << 32.
pub const IOMMU_OP_GSP_GR_TRESDE: u64 = 0x2B;
/// M5d S1: AMPERE_COMPUTE_B (0xC7C0) colgado del canal de GR0, sin
/// parametros. Tras G4; una vez por arranque. `Ok` = pagina | numero << 32.
pub const IOMMU_OP_GSP_COMPUTO: u64 = 0x2C;
/// M5d S3: el primer trabajo del motor grafico, `arg1` = la ficha del canal
/// de GR0 con su lista (`computo::ficha_valida`): SET_OBJECT del computo y un
/// semaforo de INFORME en el tramo. Una vez por arranque. `Ok` =
/// `computo::empaquetar(semaforo, GP_GET, lanzado, us)`.
pub const IOMMU_OP_GPU_TRABAJO_GR: u64 = 0x2D;
/// M5d S4..S6: el primer sombreador, `arg1` = la ficha de S3: el programa
/// (`sombreador::CODIGO`), el QMD V03_00 y las ordenes en el tramo, la entrada
/// 1 del GPFIFO de GR0. Tras S3; una vez por arranque. `Ok` =
/// `sombreador::empaquetar(..)`.
pub const IOMMU_OP_GPU_SOMBREO: u64 = 0x2E;
/// M5d L: el lienzo, `arg1` = la ficha de S3: 16 marcos de RAM del PC
/// prestados ESCRIBIBLES en `lienzo::IOVA` y mapeados en `lienzo::VA` (una vez
/// por arranque), y un programa de 128 x 128 hilos que pinta un degradado. Tras
/// el primer sombreador; una vez por arranque. `Ok` = `lienzo::empaquetar(..)`.
pub const IOMMU_OP_GPU_LIENZO: u64 = 0x2F;
/// M5d L: leer el lienzo, `arg1` = el par de pixeles (0..8192); con el bit
/// 32, la SALIDA del blur. Solo lectura, tras pintarlo. `Ok` = pixel 2k |
/// pixel 2k+1 << 32 (0x00RRGGBB).
pub const IOMMU_OP_GPU_LIENZO_LEER: u64 = 0x30;
/// M5d B: subir dos pixeles AL lienzo, `arg1` = `blur::subir(k, p0, p1)`.
/// Tras pintarlo y sin un blur en marcha. `Ok` = k.
pub const IOMMU_OP_GPU_LIENZO_ESCRIBIR: u64 = 0x31;
/// M5d B: el blur, `arg1` = la ficha de S3: la media de 7 x 7 de cada pixel
/// del lienzo en la SALIDA (`blur::IOVA`, prestada y mapeada una vez), en la
/// siguiente entrada del GPFIFO de GR (se puede repetir). `Ok` =
/// `blur::empaquetar(..)`: los pixeles iguales a la cuenta de la CPU.
pub const IOMMU_OP_GPU_BLUR: u64 = 0x32;
/// M5d F: el fractal, `arg1` = la ficha de S3: Mandelbrot de 512 x 512 en 1
/// MiB del PC (`fractal::IOVA`, prestado y mapeado una vez), en la siguiente
/// entrada del GPFIFO de GR (se puede repetir; comparte los motivos del blur).
/// `Ok` = `fractal::empaquetar(..)`: pixeles iguales a la CPU, y lo que
/// tardaron la 3060 y la CPU. Se lee con `IOMMU_OP_GPU_LIENZO_LEER` y el bit 33.
pub const IOMMU_OP_GPU_FRACTAL: u64 = 0x33;
/// M5d T0: el triangulo por computo, `arg1` = la ficha de S3: las tres
/// funciones de arista en 512 x 512 hilos, en el MISMO MiB que el fractal.
/// `Ok` = `triangulo::empaquetar(..)` (el formato del fractal). Se lee con
/// `IOMMU_OP_GPU_LIENZO_LEER` y el bit 33.
pub const IOMMU_OP_GPU_TRIANGULO: u64 = 0x34;
/// M5 T1a: la clase 3D (AMPERE_B) limpia el MiB del fractal como destino de
/// render de 512 x 512 con su ROP, sin programas. `arg1` = la ficha de S3.
/// `Ok` = `tresde::empaquetar(..)`. Se lee con `IOMMU_OP_GPU_LIENZO_LEER` y el
/// bit 33.
pub const IOMMU_OP_GPU_LIMPIAR_3D: u64 = 0x35;
/// M5d E: la escena 3D con luz (esfera, suelo con sombra, cielo), en 512 x
/// 512 hilos y en el MISMO MiB que el fractal. `arg1` = la ficha de S3. `Ok`
/// = `escena::empaquetar(..)`. Se lee con `IOMMU_OP_GPU_LIENZO_LEER` y el bit
/// 33.
pub const IOMMU_OP_GPU_ESCENA: u64 = 0x36;
/// M5 T1c: el triangulo por el RASTERIZADOR de la 3060 (AMPERE_B con un
/// programa de vertice y uno de pixel) sobre el destino de T1a. `arg1` = la
/// ficha de S3. `Ok` = `raster::empaquetar(..)`. Se lee con
/// `IOMMU_OP_GPU_LIENZO_LEER` y el bit 33.
pub const IOMMU_OP_GPU_RASTER: u64 = 0x37;
/// M5 T2a: el triangulo de T1c con un color por vertice, mezclado por el
/// rasterizador (IPA). `arg1` = la ficha de S3. `Ok` = `color3d::empaquetar(..)`.
/// Se lee con `IOMMU_OP_GPU_LIENZO_LEER` y el bit 33.
pub const IOMMU_OP_GPU_COLOR_3D: u64 = 0x38;
/// M5 T1c: lo que quedo del ultimo dibujo 3D, palabra `arg1`: 0 la escalera
/// de semaforos (bit 0 el estado, 1 los vertices, 2 el dibujo entero), 1
/// `NV_PGRAPH_INTR`, 2 `NV_PGRAPH_EXCEPTION`, 3 `NV_PGRAPH_STATUS`, 4 y 5 los
/// escalones del estado (bits 0..31 y 32..63; `raster::culpable`). Solo lee.
pub const IOMMU_OP_GPU_DIAG_3D: u64 = 0x39;
/// M5d G: UN fotograma (256 x 256) de la esfera que gira y bota, por computo.
/// `arg1` = la ficha de S3 (bits 0..31) y el fotograma (32..39, menor que
/// `giro::FOTOGRAMAS`). `Ok` = `giro::empaquetar(..)`. Se lee con
/// `IOMMU_OP_GPU_LIENZO_LEER` y el bit 33 (los primeros 256 KiB).
pub const IOMMU_OP_GPU_GIRO: u64 = 0x3A;
/// L0c5: el apagado en orden del GSP, 1 de 3: la RPC
/// UNLOADING_GUEST_DRIVER (47) al GSP-RM despierto. `Ok` = pagina | numero
/// << 32, como toda RPC; la respuesta se espera por la cola.
pub const IOMMU_OP_GSP_DESPEDIR: u64 = 0x3B;
/// L0c5, 2 de 3: tras despedir (suspendido o no, como nouveau), FWSEC-SB en
/// el falcon del GSP. `Ok` = la firma usada.
pub const IOMMU_OP_GSP_CERRAR: u64 = 0x3C;
/// L0c5, 3 de 3: tras FWSEC-SB (bien o no, como nouveau), el booter de
/// descarga (`boot_ul.bin`) en el SEC2. `Ok` = la firma usada.
pub const IOMMU_OP_GSP_DESCARGAR: u64 = 0x3D;
/// L0c5: como va el apagado, leido en vivo. Bits: 0 despedido, 1 suspendido,
/// 2 FWSEC-SB arrancado, 3 su falcon parado, 4 SB bien, 5 descargador
/// arrancado, 6 SEC2 parado, 7 HECHO (la WPR2 abajo); 8..23 el error de SB;
/// 32..63 el MAILBOX0 del falcon que toque.
pub const IOMMU_OP_GSP_APAGADO: u64 = 0x3E;
/// M5d P: UN fotograma de la pantalla ENTERA, pintado por la 3060
/// directamente en el framebuffer del GOP (en su VRAM), a la resolucion del
/// GOP. `arg1` = la ficha de S3 (bits 0..31), el fotograma (32..55) y el bit
/// 56 para cargar el programa (el primero de cada tanda). `Ok` =
/// `pantalla::empaquetar(..)`: las muestras iguales a la CPU (de 1024).
pub const IOMMU_OP_GPU_PANTALLA: u64 = 0x3F;
/// El bit 56 de `IOMMU_OP_GPU_PANTALLA`: cargar el programa, el QMD y las ordenes.
pub const PANTALLA_CARGAR: u64 = 1 << 56;
/// C1 (25-09): comprobar solo las 16 muestras que rotan, no las 1024.
pub const PANTALLA_POCAS: u64 = 1 << 57;
/// D2: el bit 58 de `IOMMU_OP_GPU_PANTALLA`: NO pintar; comparar UNA FILA (`y`
/// en los bits 0..15, el fotograma en 32..55) pixel a pixel contra la CPU.
/// `Ok(malos | cpu_us << 32)`. Una fila por llamada: un syscall corre con las
/// interrupciones cerradas, y la pantalla entera son ~2 s (metal 25-09 13:35).
pub const PANTALLA_FILA: u64 = 1 << 58;
/// Compositor por GPU, paso 1: el motor de copia de la 3060 lleva el lienzo
/// del escritorio a la pantalla del GOP. `arg1` = la VA del lienzo (un bloque
/// del que llama, de la medida de la pantalla). `Ok` = `volcado::empaquetar(..)`.
pub const IOMMU_OP_GPU_VOLCADO: u64 = 0x40;
/// Compositor por GPU, paso 1b: el volcado en CADA fotograma. `arg1` lleva la
/// suborden en 63..60 (`volcado::ARMAR`, `CAJA`, `SOLTAR`, `COMO_VA`); la
/// CAJA ultima de un fotograma toca el timbre y vuelve con la valla pagada.
pub const IOMMU_OP_GPU_VOLCADOR: u64 = 0x41;
/// M6 V0: el formato de una tanda de video NV12. `arg1` = ancho (0..15),
/// alto (16..31) y la ficha de S3 (32..63). `Ok` = escala | x0 << 8 |
/// y0 << 32: como cae en la pantalla (agrandado por un entero y centrado).
pub const IOMMU_OP_GPU_VIDEO_FORMATO: u64 = 0x42;
/// M6 V0: UN fotograma NV12 (Y y detras UV intercalado) de un bloque
/// KIND_MEMORIA del que llama, convertido a RGB y agrandado por la 3060
/// directamente en el framebuffer del GOP. `arg1` = la VA del bloque, y el
/// bit 63 para cargar el programa (el primero de cada tanda). Prestado SOLO
/// LECTURA durante la llamada. `Ok` = `video::empaquetar(..)` (de 256 muestras).
pub const IOMMU_OP_GPU_VIDEO: u64 = 0x43;
/// El bit 63 de `IOMMU_OP_GPU_VIDEO`: cargar el programa, el QMD y las ordenes.
pub const VIDEO_CARGAR: u64 = 1 << 63;
/// D2: el formato de una tanda de imagenes de 32 bits `0x00RRGGBB` (DOOM:
/// 320 x 200). `arg1` = ancho (0..15), alto (16..31) y la ficha de S3
/// (32..63). `Ok` = escala | x0 << 8 | y0 << 32, como `IOMMU_OP_GPU_VIDEO_FORMATO`.
pub const IOMMU_OP_GPU_IMAGEN_FORMATO: u64 = 0x46;
/// D2: UNA imagen de un bloque KIND_MEMORIA del que llama, agrandada por la
/// 3060 directamente en el framebuffer del GOP. `arg1` = la VA del bloque, y
/// el bit 63 para cargar el programa. Prestada SOLO LECTURA durante la
/// llamada. `Ok` = `imagen::empaquetar(..)` (de 256 muestras).
pub const IOMMU_OP_GPU_IMAGEN: u64 = 0x47;
/// El bit 63 de `IOMMU_OP_GPU_IMAGEN`: cargar el programa, el QMD y las ordenes.
pub const IMAGEN_CARGAR: u64 = 1 << 63;
/// El bit 62 de `IOMMU_OP_GPU_IMAGEN` (D2c): la VA es de un PRESTAMO que el
/// que llama TOMO (el fotograma de otra app, p. ej. DOOM), no de un bloque
/// suyo. El kernel exige que sea suyo y que sus marcos vayan seguidos.
pub const IMAGEN_PRESTADO: u64 = 1 << 62;
/// P3b4c: LA PUERTA ESTRECHA, la unica orden de la 3060 para una app SIN la
/// autoridad `MAQUINA`: `arg1` = la VA de una RECETA (VRN2,
/// `bmo_gpu_ga10x::receta`) en un bloque del que llama. La 3060 dibuja en el
/// DESTINO de la receta -- un bloque ESCRIBIBLE del que llama -- con el
/// pegamento del kernel; la pantalla SOLO si el escritorio se la dio
/// (`IOMMU_OP_GPU_PANTALLA_PARA`) y el destino es un back buffer de la
/// cadena. `Ok` = lo de `cubo::empaquetar` (bit 62: quedo en la pantalla).
pub const IOMMU_OP_GPU_DIBUJAR: u64 = 0x48;
/// P3b4c.9 Z1: el ESCRITORIO (con `MAQUINA` y la pantalla) le da la pantalla
/// a la app de la tarea `arg1` (su tid; 0 = a nadie): sus recetas con un back buffer de
/// la cadena dibujan en la ventana de la pantalla, sin tocar su RAM.
pub const IOMMU_OP_GPU_PANTALLA_PARA: u64 = 0x49;
/// EL REINTENTO LIMPIO DEL GSP (29-09), tras un booter parado con error
/// (0x15) y el GSP-RM sin arrancar: CERRAR = FWSEC-SB, DESCARGAR = el booter
/// de descarga (baja la WPR2), SUBIR = con la WPR2 abajo, el despertar a cero
/// y FWSEC-FRTS otra vez (el escritorio sigue EN SEGUIDA con despertar y el
/// booter). REINTENTO = como va, en vivo: 0 SB arrancado, 1 su falcon parado,
/// 2 descargador arrancado, 3 SEC2 parado, 4 la WPR2 abajo, 5 FRTS otra vez,
/// 6 SB SALTADO (tras el 0x15 el falcon del GSP queda cerrado y no carga);
/// 8..15 intentos; 32..63 MAILBOX0 del falcon que toque.
pub const IOMMU_OP_GSP_REINTENTO_CERRAR: u64 = 0x4A;
pub const IOMMU_OP_GSP_REINTENTO_DESCARGAR: u64 = 0x4B;
pub const IOMMU_OP_GSP_REINTENTO_SUBIR: u64 = 0x4C;
pub const IOMMU_OP_GSP_REINTENTO: u64 = 0x4D;
/// E7: el trabajo ETERNO -- un computo que no acaba nunca, a proposito, en el
/// canal de GR: lo tiene que CORTAR el vigilante del kernel (el TDR de
/// BMO-X). `arg1` = la ficha del timbre de GR (la de S3). `Ok(v)`:
/// `lanzado | pagado << 1 | cortado << 2 | us << 8 | final del corte << 32`
/// (`bmo_gpu_ga10x::eterno::empaquetar`). Deja el canal de GR MUERTO hasta
/// reiniciar: es la ultima prueba de una sesion.
pub const IOMMU_OP_GPU_ETERNO: u64 = 0x4E;
/// P1: EL PASE de la GPU, neutro (`bmo_pase_gpu::orden`): ABRIR con la VA del
/// lienzo (el lienzo prestado para quedarse, `Ok` = la VA del buzon), CERRAR
/// o ESTADO en los bits 63..60 de `arg1`.
pub const IOMMU_OP_GPU_PASE: u64 = 0x44;
/// X5: el cubo del estudio D3D por la 3060, sin Windows, en una ventana de
/// 1280x720 centrada en la pantalla. DIBUJAR: `arg1` = la ficha de S3 (bits
/// 0..31) y el fotograma (32..40, menor que 360); `Ok` = `cubo::empaquetar(..)`.
/// LEER (con [`CUBO_LEER`]): `arg1` = `CUBO_LEER | k`, los pixeles `2k` y
/// `2k + 1` de la ventana, fila a fila, como `0x00RRGGBB`; `Ok(p0 | p1 << 32)`.
pub const IOMMU_OP_GPU_CUBO: u64 = 0x45;
/// El bit 63 de `IOMMU_OP_GPU_CUBO`: LEER en vez de DIBUJAR.
pub const CUBO_LEER: u64 = 1 << 63;
/// El bit 62 de `IOMMU_OP_GPU_CUBO`: VERRANO V0, `arg1` = este bit | la VA
/// de un paquete del que llama (`bmo_gpu_ga10x::tuberia::Paquete`: los dos
/// programas tomados del BSF y los vertices). `Ok` como DIBUJAR.
pub const CUBO_VERRANO: u64 = 1 << 62;
/// El bit 61, con [`CUBO_VERRANO`]: las ordenes SIN la escalera de T1c
/// (V1 `ligero`). `Ok` lleva ademas lo que costo preparar
/// (`cubo::preparado`: en caliente o no, y sus us).
pub const CUBO_LIGERO: u64 = 1 << 61;
/// El bit 60, con [`CUBO_VERRANO`]: EL ANILLO (V1b). Las ordenes de
/// `ligero`, los vertices en RAM del PC y el fotograma EN VUELO: el `Ok`
/// vuelve sin esperar a que la 3060 lo pague (`cubo::es_en_vuelo`; sus us son
/// lo que la CPU espero a que su ranura quedara libre).
pub const CUBO_ANILLO: u64 = 1 << 60;
/// El bit 59 (solo): VACIAR el anillo -- esperar a que la 3060 pague todo
/// lo que quedo en vuelo. `Ok(us esperados)`.
pub const CUBO_VACIAR: u64 = 1 << 59;
/// El bit 58, con [`CUBO_ANILLO`]: COOPERA (V1c). La limpieza recortada a
/// lo que dice la cabecera del paquete (`tuberia::Paquete::limpiar`): la CPU
/// sabe donde estaba el cubo y donde va a estar; la 3060 solo limpia eso.
pub const CUBO_COOPERA: u64 = 1 << 58;
/// Motivos del NO, en las banderas de `ERROR_NEGADO`.
pub const IOMMU_NO_ESCRITORIO: u32 = 1;
pub const IOMMU_NO_TABLAS: u32 = 2;
pub const IOMMU_NO_YA_ENCENDIDA: u32 = 3;
pub const IOMMU_NO_CONTESTA: u32 = 4;
pub const IOMMU_NO_APAGADA: u32 = 5;
/// No hay NVIDIA en el BDF de la sonda.
pub const IOMMU_NO_SIN_GPU: u32 = 6;
/// E2: la 3060 no esta ciega en la IOMMU -- el candado no abre.
pub const IOMMU_NO_GPU_VE: u32 = 7;
/// E2: la tarjeta no anuncia MSI.
pub const IOMMU_NO_SIN_MSI: u32 = 8;
/// E2: la sonda no dejo cabeza, modo o BAR0.
pub const IOMMU_NO_SIN_CABEZA: u32 = 9;
/// E2: el vector 50 no se instalo al arrancar.
pub const IOMMU_NO_SIN_VECTOR: u32 = 10;
/// E2: la pantalla no acepto el aviso (se releyo y no estaba).
pub const IOMMU_NO_E2_NO_ARMA: u32 = 11;
/// M0d: no hubo paginas contiguas para las tablas del dominio de la 3060.
pub const IOMMU_NO_SIN_AREA: u32 = 12;
/// M0d: prestar pide la 3060 TRADUCIDA (`gpu traducir`).
pub const IOMMU_NO_NO_TRADUCIDA: u32 = 13;
/// M0d: el prestamo no se hizo (ya prestado, o sin tablas).
pub const IOMMU_NO_PRESTAMO: u32 = 14;
/// M0d: el oraculo no vio lo prestado al releer: se quito.
pub const IOMMU_NO_RELEIDA: u32 = 15;
/// M0d3: sin el Bus Master de E2 (`gpu vblank`) la 3060 no puede hacer DMA.
pub const IOMMU_NO_SIN_BUS_MASTER: u32 = 16;
/// M0d3: el falcon no dejo hacer el DMA; el motivo, en `INFO_GPU_FUEGO`.
pub const IOMMU_NO_FUEGO: u32 = 17;
/// M0d3: la pagina de prueba no esta prestada (`gpu prestar`).
pub const IOMMU_NO_SIN_PRUEBA: u32 = 18;
/// L0b: el descriptor de la ROM no es un FWSEC v3 del GSP que quepa.
pub const IOMMU_NO_FWSEC_DESC: u32 = 19;
/// L0b: sin PREPARAR, o faltan trozos por copiar.
pub const IOMMU_NO_FWSEC_SIN_PREPARAR: u32 = 20;
/// L0b: el fusible no pide ninguna de las firmas del descriptor.
pub const IOMMU_NO_FWSEC_FIRMA: u32 = 21;
/// L0b: la orden FRTS o la firma no se pudieron poner.
pub const IOMMU_NO_FWSEC_PARCHE: u32 = 22;
/// L0b: ya hay WPR2: hace falta reiniciar la 3060.
pub const IOMMU_NO_WPR2_YA: u32 = 23;
/// L0b: el firmware de arranque de la tarjeta no acabo (o no se deja leer).
pub const IOMMU_NO_GFW: u32 = 24;
/// No es la 3060 12G: su VRAM no son 12288 MiB (25-09).
pub const IOMMU_NO_OTRA_TARJETA: u32 = 84;
/// L0b: el falcon no dejo resetearse o cargar por DMA.
pub const IOMMU_NO_FWSEC_FALCON: u32 = 25;
/// L0c2: no esta `fw/gsp/gsp.bin` o `fw/gsp/bootldr.bin`.
pub const IOMMU_NO_GSP_FICHERO: u32 = 26;
/// L0c2: estan, pero no son un GSP-RM (ELF, `.fwimage`, firma ga10x) o un bootloader.
pub const IOMMU_NO_GSP_FORMATO: u32 = 27;
/// L0c2: no hubo marcos para la imagen, la radix3 o lo auxiliar.
pub const IOMMU_NO_GSP_MARCOS: u32 = 28;
/// L0c2: fuera de orden (sin preparar, un trozo saltado, prestar sin copiar).
pub const IOMMU_NO_GSP_ORDEN: u32 = 29;
/// L0c2: el disco devolvio menos de lo pedido.
pub const IOMMU_NO_GSP_DISCO: u32 = 30;
/// L0c2: ya prestado: no se escribe encima de lo que la 3060 puede leer.
pub const IOMMU_NO_GSP_YA_PRESTADO: u32 = 31;
/// L0c2: la VRAM no se deja repartir.
pub const IOMMU_NO_GSP_SIN_VRAM: u32 = 32;
/// L0c2: la radix3 no lleva, por la IOMMU, a donde tiene que llevar.
pub const IOMMU_NO_GSP_RADIX: u32 = 33;
/// L0c3a: sin el GSP-RM prestado (L0c2) no hay a quien darle LIBOS.
pub const IOMMU_NO_LIBOS_ORDEN: u32 = 34;
/// L0c3a: no hubo marcos para los argumentos, los logs o las colas.
pub const IOMMU_NO_LIBOS_MARCOS: u32 = 35;
/// L0c3a: un puntero no lleva, por la IOMMU, a donde tiene que llevar.
pub const IOMMU_NO_LIBOS_PUNTERO: u32 = 36;
/// L0c3b: falta algo de antes EN ESTE ARRANQUE: la WPR2 de FWSEC, el GSP-RM
/// por su radix3 (`gpu radix`) o LIBOS (`gpu libos`).
pub const IOMMU_NO_DESPERTAR_ANTES: u32 = 37;
/// L0c3b: `fw/gsp/boot_ld.bin` no esta, o no es un booter del SEC2 que quepa.
pub const IOMMU_NO_BOOTER: u32 = 38;
/// L0c3b: el fusible del SEC2 no pide ninguna firma del booter.
pub const IOMMU_NO_BOOTER_FIRMA: u32 = 39;
/// L0c3b: la pagina de vaciado no se releyo en 0x100C10.
pub const IOMMU_NO_VACIADO: u32 = 40;
/// L0c3b: el falcon del GSP no se reseteo, o no esta parado para el booter.
pub const IOMMU_NO_GSP_FALCON: u32 = 41;
/// L0c3b: el SEC2 no dejo resetearse, cargar el booter por DMA o arrancar.
pub const IOMMU_NO_SEC2: u32 = 42;
/// L0c3b: el SEC2 todavia no se paro.
pub const IOMMU_NO_SEC2_NO_PARA: u32 = 43;
/// L0c3b: el booter se paro con MAILBOX0 distinto de 0: su codigo de error.
pub const IOMMU_NO_BOOTER_MAL: u32 = 44;
/// L0c3b: el booter ya corrio en este arranque; otra vez pide reiniciar la 3060.
pub const IOMMU_NO_YA_DESPIERTO: u32 = 45;
/// L0c4b1: el GSP no esta despierto en este arranque: no hay cola que devolver.
pub const IOMMU_NO_COLA_ANTES: u32 = 46;
/// L0c4b1: el puntero pedido no es un hueco de la cola (0..63).
pub const IOMMU_NO_COLA_PUNTERO: u32 = 47;
/// L0c4b2a: sin `gpu libos` no hay cola de la CPU donde escribir.
pub const IOMMU_NO_SISTEMA_ANTES: u32 = 48;
/// L0c4b2a: ya se mandaron, o el GSP ya desperto.
pub const IOMMU_NO_SISTEMA_YA: u32 = 49;
/// L0c4b2c: el GSP no desperto, o lo primero de su cola no es un secuenciador entero.
pub const IOMMU_NO_SEC_ANTES: u32 = 50;
/// L0c4b2c: una orden fallo o se sale del falcon del GSP (el detalle, selector 2).
pub const IOMMU_NO_SEC_FALLO: u32 = 51;
/// L0c4b2c: ya se corrio en este arranque.
pub const IOMMU_NO_SEC_YA: u32 = 52;
/// L0c4b3a: no hay BAR1 que devolver (sin secuenciador corrido, o sin la de antes).
pub const IOMMU_NO_BAR1: u32 = 53;
/// L1a: el GSP-RM no esta arrancado, o no hay colas.
pub const IOMMU_NO_RPC_ANTES: u32 = 54;
/// L1a: la cola de la CPU esta llena.
pub const IOMMU_NO_RPC_LLENA: u32 = 55;
/// L1b: no es uno de los tres objetos.
pub const IOMMU_NO_RPC_OBJETO: u32 = 56;
/// El mensaje armado no esta en el contrato de la cola: no sale.
pub const IOMMU_NO_RPC_CONTRATO: u32 = 57;
/// No es una de las ordenes de control de la lista.
pub const IOMMU_NO_RPC_CONTROL: u32 = 58;
/// L1c2: sin 3060, o la prueba de la VRAM ya en curso.
pub const IOMMU_NO_VRAM: u32 = 59;
/// L1c3: el directorio ya se puso en este arranque.
pub const IOMMU_NO_DIRECTORIO_YA: u32 = 60;
/// L1d1: la raiz ya tenia esa entrada ocupada, o el tramo ya se mapeo.
pub const IOMMU_NO_TRAMO: u32 = 61;
/// L1d2b: sin el tramo mapeado, o el canal ya se pidio.
pub const IOMMU_NO_CANAL: u32 = 62;
/// L1d2b: sus paginas no quedaron a cero, o su bufer de metodos no se presto.
pub const IOMMU_NO_CANAL_MEMORIA: u32 = 63;
/// L1d2c: no es BIND ni SCHEDULE, o el canal no se pidio.
pub const IOMMU_NO_CANAL_ORDEN: u32 = 64;
/// L1d3: sin copiador, copia ya hecha, o una ficha que no es de nuestro canal.
pub const IOMMU_NO_COPIA: u32 = 65;
/// L1d3: el tramo no se releyo igual por PRAMIN: no se toco el timbre.
pub const IOMMU_NO_COPIA_PREPARAR: u32 = 66;
/// M5 G2: medidas fuera de lo que cabe, el tramo sin mapear, o ya se hizo.
pub const IOMMU_NO_GR_MEMORIA: u32 = 68;
/// M5 G3: sin G1 y G2, sin las ocho medidas, un reparto distinto del de G2, o
/// ya se promovio.
pub const IOMMU_NO_GR_PROMOVER: u32 = 69;
/// M5 G4: sin G3, o el objeto 3D ya se pidio.
pub const IOMMU_NO_GR_TRESDE: u32 = 70;
/// M5d S1: sin el oro (G4), o el computo ya se pidio.
pub const IOMMU_NO_COMPUTO: u32 = 71;
/// M5d S3: sin el computo, una ficha que no es del canal de GR0, o ya se hizo.
pub const IOMMU_NO_TRABAJO_GR: u32 = 72;
/// M5d S3: el tramo no se releyo igual: no se toco el timbre.
pub const IOMMU_NO_TRABAJO_GR_PREPARAR: u32 = 73;
/// M5d S4..S6: sin S3, una ficha que no es del canal de GR0, o ya se hizo.
pub const IOMMU_NO_SOMBREO: u32 = 74;
/// M5d S4..S6: el tramo no se releyo igual: no se toco el timbre.
pub const IOMMU_NO_SOMBREO_PREPARAR: u32 = 75;
/// M5d L: sin el primer sombreador, una ficha ajena, o ya se pinto.
pub const IOMMU_NO_LIENZO: u32 = 76;
/// M5d L: el lienzo no se presto, sus PTE no estaban vacias, o el tramo no se
/// releyo: no se toco el timbre.
pub const IOMMU_NO_LIENZO_PREPARAR: u32 = 77;
/// M5d B: sin el lienzo, una ficha ajena, el GPFIFO gastado, o uno en marcha.
pub const IOMMU_NO_BLUR: u32 = 78;
/// M5d B: la salida no se presto, sus PTE no estaban vacias, o el tramo no se
/// releyo: no se toco el timbre.
pub const IOMMU_NO_BLUR_PREPARAR: u32 = 79;
/// L0c5: el GSP no esta en el paso de antes del apagado (sin despertar, sin
/// despedir, sin FWSEC-SB), o ese paso ya se dio; tras despedir, toda RPC.
pub const IOMMU_NO_APAGAR: u32 = 80;
/// M5d P: BAR1 no esta en modo fisico (sin `gpu init`), el framebuffer del
/// GOP no cae dentro de BAR1 y por debajo de 64 MiB de VRAM, o no se mapeo.
pub const IOMMU_NO_PANTALLA: u32 = 81;
/// L0c5: el GSP ya se apago en orden en este arranque; la 3060 no trabaja
/// hasta el siguiente. Toda orden de trabajo pedida despues lo dice al instante.
pub const IOMMU_NO_GSP_APAGADO: u32 = 82;
/// El volcado por la 3060 no se puede: sin la copia de L1d3, sin la pantalla
/// del GOP en modo fisico, un lienzo que no es del que llama, o uno en marcha.
pub const IOMMU_NO_VOLCADO: u32 = 83;
/// M6 V0: sin el canal de GR y su ficha, sin la pantalla en modo fisico, un
/// formato impar o mas grande que la pantalla, un fotograma que no es un
/// bloque del que llama, o uno ya en marcha.
pub const IOMMU_NO_VIDEO: u32 = 85;
/// P1: el pase no se abrio (o no habia que cerrar). El POR QUE exacto, un
/// `bmo_pase_gpu::pase::NoPase`, lo dice la suborden ESTADO.
pub const IOMMU_NO_PASE: u32 = 86;
/// J2: un programa del paquete de VERRANO que el juez del SASS rechaza (TOMA
/// TU BODRIO): no llega a la 3060. La regla y la instruccion, en la cabina.
pub const IOMMU_NO_BODRIO: u32 = 87;
/// D2: la imagen no se puede, por lo mismo que el video (`IOMMU_NO_VIDEO`):
/// sin el canal de GR, sin la pantalla en modo fisico, un formato que no cabe,
/// un fotograma que no es un bloque del que llama, o uno ya en marcha.
pub const IOMMU_NO_IMAGEN: u32 = 88;
/// P3b4c: el canal de GR tomo una excepcion (un Xid; el GSP-RM lo mato): todo
/// trabajo del GR dice NO al instante hasta reiniciar.
pub const IOMMU_NO_CANAL_MUERTO: u32 = 90;
/// El reintento limpio del GSP: no hay un booter parado con error que
/// deshacer, un paso fuera de orden, o la WPR2 no bajo.
pub const IOMMU_NO_REINTENTO: u32 = 91;
/// L0c3b: la WPR2 ya EXTENDIDA antes de nuestro booter (otro booter corrio).
pub const IOMMU_NO_GPU_CALIENTE: u32 = 67;
/// El fader, en 1/256 dB con signo (`arg1` como `i64`). El kernel lo recorta a
/// -96..+24 dB y devuelve lo que quedo puesto, tambien como `i64`.
pub const AUDIO_MANDO_FADER: u64 = 1;
/// Callar (`arg1 != 0`) o descallar. Con rampa: no es un corte seco.
pub const AUDIO_MANDO_MUDO: u64 = 2;
/// **EL OIDO** (`PLAN_EL_SONIDO.md` S4f, 03-10): el perfil de quien escucha,
/// que el maestro aplica a TODO antes de la ganancia y el limite. Cada orden
/// devuelve el perfil que quedo, empaquetado como `INFO_AUDIO_OIDO`.
/// El balance: `arg1` de -100 (solo izquierda) a +100 (solo derecha).
pub const AUDIO_MANDO_BALANCE: u64 = 3;
/// Mono (`arg1 != 0`): los dos lados sumados, para quien oye por uno.
pub const AUDIO_MANDO_MONO: u64 = 4;
/// Los tres tonos, en dB enteros de -12 a +12: estante a 100 Hz, campana a
/// 1 kHz, estante a 3,5 kHz.
pub const AUDIO_MANDO_GRAVES: u64 = 5;
pub const AUDIO_MANDO_MEDIOS: u64 = 6;
pub const AUDIO_MANDO_AGUDOS: u64 = 7;
/// Todo el oido a plano.
pub const AUDIO_MANDO_PLANO: u64 = 8;
/// **EL MODO 3D** (`PLAN_EL_SONIDO.md` S7, 03-10), para todo lo que suena:
/// `arg1` = 0 apagado, 1 cerca (altavoces virtuales a +-30 grados), 2 sala
/// (y reflejos tempranos), 3 amplio (+-60 grados), 4 orbita (la escena gira
/// alrededor de la cabeza). Devuelve el ajuste, como `INFO_AUDIO_ESPACIO`.
pub const AUDIO_MANDO_3D: u64 = 9;
/// Segundos por vuelta de la orbita, 2..=60.
pub const AUDIO_MANDO_3D_VUELTA: u64 = 10;
/// **EL EMPUJE** (03-10, "que suene al 200 %"): `arg1 != 0` lo enciende. Con
/// la ganancia por encima de 0 dB, dobla las puntas con una curva suave
/// antes del limite, en vez de bajar toda la onda: mas fuerte con el mismo
/// techo, con algo de color en los golpes. Devuelve lo mismo que
/// `INFO_AUDIO_OIDO` (bit 49: empuje puesto).
pub const AUDIO_MANDO_EMPUJE: u64 = 11;

/// Prestar el banco del fondo: `arg1` = la VA de un bloque propio. Arma el
/// tubo. `Ok` lleva los bytes del banco (0 = no se acepto).
pub const AUDIO_FONDO_BANCO: u64 = 1;
/// Tocar: `arg1` = inicio en bytes `[0..32)` | muestras `[32..64)`.
pub const AUDIO_FONDO_TOCAR: u64 = 2;
/// Ajustar izq y der de un canal que suena, por una rampa de 5 ms.
pub const AUDIO_FONDO_AJUSTAR: u64 = 3;
/// Callar un canal, o todos con `arg1 = 1`.
pub const AUDIO_FONDO_CALLAR: u64 = 4;
/// Suena el canal?
pub const AUDIO_FONDO_SUENA: u64 = 5;
/// Soltar el banco y el tubo.
pub const AUDIO_FONDO_SOLTAR: u64 = 6;
/// Situar un canal del fondo en el espacio (S7 por voz): volumen = el `izq`
/// de `arg0`, `arg1` = el angulo (`i16`, grados, + derecha, 0 delante), o
/// -32768 para volver al paneo.
pub const AUDIO_FONDO_SITUAR: u64 = 7;
/// **El 3D por voz en `AUDIO_OP_VOZ` / ajustar** (S7, 03-10): con este bit
/// en `arg2`, `[0..16)` es el volumen (0..256) y `[32..48)` el angulo (`i16`,
/// grados, + derecha). Sin el, `[0..16)` izq y `[16..32)` der, como siempre.
pub const VOZ_AJUSTAR_ANGULO: u64 = 1 << 48;
/// Los canales desde aqui son AVISOS: no se agachan y agachan la musica.
pub const AUDIO_FONDO_AVISO: u64 = 8;

/// Ocho bytes del nombre del hijo `arg0`; `arg1` numera el trozo.
///
/// De ocho en ocho porque la superficie congelada no acepta punteros, y es el
/// mismo mecanismo que `KLOG_TEXTO` y `DIR_OP_NOMBRE` -- inventar uno nuevo por
/// cada cosa que devuelve texto seria tener tres sitios donde se pierde un byte.
pub const TASK_OP_ES_TEXTO: u64 = 0x1A;
