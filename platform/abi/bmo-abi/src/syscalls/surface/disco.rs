//! **Lo que se le pide AL DISCO.** Los `DISCO_OP_*` y sus motivos.
//!
//! Es la sexta familia del contrato, y nace con fichero propio por lo que es y
//! no por lo que mide: **es la unica que ACTUA sobre el almacen**. `informe`
//! contesta preguntas, `entrada` cuenta hechos fisicos, `objetos` opera sobre
//! handles que alguien concedio... y esto **le da ordenes al aparato donde vive
//! el trabajo del propietario**.
//!
//! # Por que hay motivos y no un booleano
//!
//! Porque un `0` obligaria a adivinar cual de las cinco puertas dijo que no, y
//! son cinco conversaciones distintas: *"este disco no sabe"* es una propiedad
//! del aparato, *"no esta armado"* es un estado que se puede ganar, *"fuera de
//! la ventana"* es un bug del que llama, y *"el disco fallo"* es hardware.
//!
//! La respuesta viaja empaquetada porque por la puerta cabe **un** numero:
//!
//! ```text
//!   (motivo << 56) | sectores
//!
//!   motivo 0 = HECHO, y entonces `sectores` es lo que se recorto de verdad
//!   motivo > 0 = no se hizo (o se hizo a medias, ver DISCO_TRIM_FALLO)
//! ```

/// **Devolverle al disco la cola libre del volumen ESTRATOS.**
///
/// Sin argumentos: **el rango no lo elige quien llama**. Lo calcula el kernel a
/// partir de `log_head` --el puntero que solo avanza-- y lo comprueba contra la
/// ventana de escritura. Un TRIM con LBA a gusto del llamante seria una orden de
/// borrado apuntable a cualquier sector desde Ring 3, y eso no es una operacion:
/// es un agujero.
pub const DISCO_OP_TRIM_LIBRE: u64 = 0x01;

/// **`FLUSH CACHE` a mano.** Devuelve 1 si el disco confirmo.
///
/// Existe porque este disco declara `SOLO_BARRERA`: no tiene condensadores, asi
/// que la barrera es lo unico que separa "el disco se quedo los bytes" de "los
/// bytes sobrevivirian a un corte". Poder pedirla desde donde se trabaja es lo
/// que hace comprobable esa frase.
pub const DISCO_OP_BARRERA: u64 = 0x02;

/// **EL METRO: leer y cronometrar.** `arg1` = MiB a leer (0 = 64, techo 1024).
///
/// Devuelve `(motivo << 56) | MB/s` con los motivos `DISCO_BANDA_*`, y deja el
/// detalle en `INFO_DISCO_BANDA` y `INFO_DISCO_BANDA_ORDEN`.
///
/// ** SOLO LEE, y dentro de la particion de datos desde su principio. Es la
/// primera cifra MEDIDA del perfil del disco (R-DISCO9, LEY 24): hasta el
/// 23-09 todo lo que se sabia de su velocidad era la caja.
///
/// [!] Tiene el disco para si mientras mide: 64 MiB son ~130 ms en los que
/// nadie mas lee. Lo pide una persona, avisada antes.
pub const DISCO_OP_BANDA: u64 = 0x03;

// -- Los motivos, en el byte alto de la respuesta ---------------------------

/// Se hizo. `sectores` dice cuantos.
pub const DISCO_TRIM_HECHO: u64 = 0;
/// No hay disco listo.
pub const DISCO_TRIM_SIN_DISCO: u64 = 1;
/// El disco **no declara TRIM** (palabra 169). No se manda a ver si suena.
pub const DISCO_TRIM_NO_SOPORTADO: u64 = 2;
/// El gate de identidad o la ventana de escritura dijeron que no.
pub const DISCO_TRIM_SIN_PERMISO: u64 = 3;
/// No hay volumen ESTRATOS montado, o su cola libre esta vacia.
pub const DISCO_TRIM_SIN_VOLUMEN: u64 = 4;
/// El rango no es representable: cero sectores, o fuera de LBA48.
pub const DISCO_TRIM_RANGO: u64 = 5;
/// El disco rechazo la orden. **`sectores` lleva lo que SI se recorto** antes
/// de romperse: un recorte a medias no se deshace, y callarlo haria que el
/// sistema volviera a mandar lo que ya estaba hecho.
pub const DISCO_TRIM_FALLO: u64 = 6;

// -- Los motivos del METRO (`DISCO_OP_BANDA`), en el mismo byte alto ---------

/// Medido. Los bits bajos son los MB/s.
pub const DISCO_BANDA_HECHA: u64 = 0;
/// No hay disco listo.
pub const DISCO_BANDA_SIN_DISCO: u64 = 1;
/// No hay particion de datos reconocida: no se mide fuera de lo que es de BMO-X.
pub const DISCO_BANDA_SIN_VOLUMEN: u64 = 2;
/// No hubo ni 256 KiB contiguos para el bufer.
pub const DISCO_BANDA_SIN_MEMORIA: u64 = 3;
/// Una lectura fallo a medias. Si algo se leyo, la medida parcial SI se guarda.
pub const DISCO_BANDA_FALLO: u64 = 4;
/// El reloj (TSC) no esta calibrado: sin reloj no hay metro.
pub const DISCO_BANDA_SIN_RELOJ: u64 = 5;

/// Desplazamiento del motivo dentro de la respuesta.
pub const DISCO_TRIM_MOTIVO_SHIFT: u64 = 56;
/// Mascara de los sectores.
pub const DISCO_TRIM_SECTORES_MASK: u64 = (1 << 56) - 1;

// -- ** POR QUE FALLO, cuando el motivo es `DISCO_TRIM_FALLO` ---------------
//
// `DISCO_TRIM_FALLO` dice *que el disco no acepto la orden*; estas clases dicen
// **cual de las cinco maneras**, y viajan en `INFO_DISCO_TRIM_FALLO` junto al
// `PxTFD` crudo: `(clase << 32) | tfd`.
//
// === Por que hizo falta, y se pago en metal ===
//
// El primer recorte en el Ryzen (2026-08-17) contesto *"el disco RECHAZO la
// orden"* y ahi se acabo la informacion. El driver distingue las cinco desde
// siempre, pero su `name()` las aplana en una frase y el `tfd` --el registro
// donde el aparato dice por que-- no salia del `enum`.
//
// ** Y las cinco mandan a mirar sitios distintos: `SIN_TIEMPO` acusa al
// presupuesto de espera del driver, `APARATO` acusa al disco, y `PETICION`
// acusa al que armo el payload. Llamarlas a las tres "rechazo" es perder la
// unica pista que hay.

pub const DISCO_FALLO_NINGUNO: u64 = 0;
/// El puerto no estaba preparado.
pub const DISCO_FALLO_NO_LISTO: u64 = 1;
/// El disco no solto BSY/DRQ: no se le pudo ni dar la orden.
pub const DISCO_FALLO_OCUPADO: u64 = 2;
/// **No termino dentro del limite.** No es que dijera que no: es que no
/// contesto -- y el sospechoso es el presupuesto de espera, no el aparato.
pub const DISCO_FALLO_SIN_TIEMPO: u64 = 3;
/// **El disco contesto con error.** El `PxTFD` de los bits bajos dice cual:
/// `0x01` ERR, y en el byte alto el registro de error -- `0x04` ABRT (no
/// conozco esa orden), `0x10` IDNF (ese sector no), `0x40` UNC.
pub const DISCO_FALLO_APARATO: u64 = 4;
/// La peticion era imposible antes de salir: cero bloques, o mas de lo que cabe.
pub const DISCO_FALLO_PETICION: u64 = 5;

/// Desplazamiento de la clase dentro de `INFO_DISCO_TRIM_FALLO`.
pub const DISCO_FALLO_CLASE_SHIFT: u64 = 32;
/// Mascara del `PxTFD` crudo.
pub const DISCO_FALLO_TFD_MASK: u64 = 0xFFFF_FFFF;

// == ** CREAR UN FICHERO EN ESTRATOS: las subordenes de `TASK_OP_ES_GESTO` ===
//
// Viven aqui y no en `objetos` por lo que son: **ordenes que cambian el
// almacen**, la misma familia que `DISCO_OP_*`. Que una escriba en el aparato y
// la otra en el volumen no las separa -- las dos son lo que este fichero reune.

/// Vacia el renglon del contenido. Se manda ANTES de acumular nada.
///
/// ** Existe para que un intento a medias no envenene al siguiente: si un
/// programa muere despues de mandar tres trozos, el renglon se queda con ellos
/// dentro. Empezar limpiando es mas barato que un tiempo de expiracion.
pub const ES_GESTO_LIMPIAR: u64 = 0x00;

/// Acumula contenido. `arg1` son 8 bytes en little-endian, y **cuantos de esos
/// ocho valen** viaja empaquetado con la suborden: `ES_GESTO_DATOS | (n << 8)`.
///
/// ** Se parte `arg0` porque por la puerta caben dos argumentos y los dos estan
/// ocupados. Es el mismo idioma que `INFO_MEM_QUIEN_*` y `AUTOPSIA_TEXTO`:
/// cuando cabe un numero y hacen falta dos, se parte el numero.
///
/// El cero NO corta -- ver `TASK_OP_ES_GESTO`.
pub const ES_GESTO_DATOS: u64 = 0x01;

/// **Cierra la transaccion.** El nombre sale del renglon de `TASK_OP_RUTA` y el
/// contenido del de arriba. Devuelve la generacion nueva, o `0`.
pub const ES_GESTO_FICHERO: u64 = 0x02;

/// **Crea una carpeta vacia** donde diga la ruta.
///
/// Una carpeta recien nacida es un nodo de directorio SIN `:entradas`. No es un
/// nodo a medias: un directorio es un nodo con `:entradas`, y uno vacio es uno
/// que todavia no la tiene.
pub const ES_GESTO_CARPETA: u64 = 0x03;

/// **Quita la entrada** que diga la ruta.
///
/// ** No destruye nada. Se publica un arbol nuevo sin esa entrada; el bloque de
/// ayer, el nodo del fichero y el estrato anterior siguen donde estaban.
/// **Borrar en ESTRATOS es dejar de nombrar**, y lo que se suelta de verdad es
/// cosa del recolector.
pub const ES_GESTO_QUITAR: u64 = 0x04;

/// **Renombra la entrada** que diga la ruta. El nombre NUEVO viaja por el
/// renglon del contenido ([`ES_GESTO_DATOS`]).
///
/// * El nodo NO se toca: la entrada nueva apunta al mismo bloque, asi que el
/// contenido, los atributos y la `:firma` siguen siendo los de antes.
/// Renombrar un fichero firmado no le invalida la firma.
pub const ES_GESTO_RENOMBRAR: u64 = 0x05;

/// **Trae un fichero de FAT32 a ESTRATOS.**
///
/// La ruta lleva el DESTINO; el renglon del contenido ([`ES_GESTO_DATOS`])
/// lleva el ORIGEN, como texto.
///
/// ** El contenido NO cruza la puerta. Viajan dos NOMBRES, y el kernel lee la
/// fuente el mismo: meter un fichero por el renglon de ocho en ocho serian 512
/// llamadas por bloque, y ese renglon no esta hecho para eso.
///
/// Es lo que hace util el techo que `flujo` levanto: el formato ya sabia partir
/// un fichero en bloques, pero Ring 3 seguia sin poder entregarlo.
pub const ES_GESTO_COPIA: u64 = 0x06;

/// **Marca la version en curso con un nombre**, que viaja por el renglon de la
/// ruta.
///
/// ** Un nombre no describe una version: la hace PERMANENTE. `con_nombre()` es
/// lo que el recolector mira para no soltar un estrato jamas, asi que los gestos
/// automaticos van SIN nombre y esto es el acto aparte de una persona.
///
/// Y es tambien la referencia que hace posible una rama: el superbloque apunta a
/// una sola punta, asi que una version vieja a la que se quiera volver tiene que
/// estar nombrada o nadie la alcanza.
///
/// Cuesta UN bloque: el estrato nuevo apunta a la MISMA raiz. Marcar un volumen
/// de 400 GiB cuesta lo mismo que marcar uno vacio.
pub const ES_GESTO_MARCAR: u64 = 0x07;

/// **Vuelve a la version `arg1` pasos atras.** `0` es la de ahora.
///
/// ** No copia nada. Los bloques de aquella version siguen todos en el disco
/// --nada se sobreescribio nunca-- asi que volver es publicar UN estrato que
/// apunta a la misma raiz. Volver un volumen de 400 GiB cuesta lo mismo que
/// volver uno vacio.
///
/// ** Y lo de en medio NO se pierde: el estrato nuevo tiene por padre la punta
/// de ahora, no la version a la que se vuelve. Es un *revert*, no un *reset* --
/// se deshace el contenido y se conserva el registro de que se deshizo.
pub const ES_GESTO_VOLVER: u64 = 0x08;

/// **DE DONDE SALE EL CONTENIDO: un bloque de `KIND_MEMORIA` propio.**
///
/// `arg1` es el handle del bloque. Los bits altos de `arg0` llevan el
/// DESPLAZAMIENTO dentro de el: `ES_GESTO_ORIGEN | (offset << 8)`.
///
/// Anota nada mas: no lee un byte y no escribe en el disco. Lo ejecuta
/// [`ES_GESTO_FICHERO_DE`], que es quien trae la cuenta.
///
/// === ** POR QUE UN HANDLE Y NO UN PUNTERO ===
///
/// Porque un puntero de Ring 3 habria que validarlo, y esa infraestructura no
/// existe en esta superficie -- a proposito. Un bloque de `KIND_MEMORIA` **lo
/// entrego el kernel**, asi que comprobar que el rango cae dentro es una RESTA
/// contra lo que se entrego, no un recorrido de tablas de pagina.
///
/// Es exactamente la forma que ya tienen `ARCH_OP_LEER_EN` y
/// `ARCH_OP_ESCRIBIR_DE` para FAT32, y la que este renglon no tenia.
///
/// ** Se pide con `RIGHT_READ` y no con `RIGHT_WRITE`: el kernel LEE el bloque,
/// no escribe dentro. Exigir mas autoridad de la que la operacion usa es
/// justo lo que un sistema de capabilities no debe hacer.
pub const ES_GESTO_ORIGEN: u64 = 0x09;

/// **Crea un fichero con el contenido del bloque anotado en
/// [`ES_GESTO_ORIGEN`].** `arg1` son los BYTES a tomar.
///
/// La ruta lleva el destino entero, igual que [`ES_GESTO_FICHERO`].
///
/// === Lo que esto cambia, y por que no es "el renglon pero mas grande" ===
///
/// El renglon acumula de ocho en ocho y para en [`ES_GESTO_MAX`]. Un MiB por
/// ahi serian 131.072 cruces de anillo. Aqui son DOS llamadas --anotar y
/// ejecutar-- para cualquier medida, porque **el contenido no viaja: viaja
/// donde esta**.
///
/// ** Y quita el rodeo que hoy es obligatorio. Sin esto, la unica forma de
/// meter mas de 96 bytes en ESTRATOS es dejarlos antes en FAT32 y copiarlos
/// con [`ES_GESTO_COPIA`] -- o sea que el documento de una aplicacion tiene que
/// pasar por un sistema de ficheros **que sobreescribe** para llegar al que no
/// sobreescribe. Todo el argumento de ESTRATOS tiene delante un tramo donde no
/// se cumple.
///
/// El techo que queda es el del volumen, y lo dice el nivel de ocupacion.
pub const ES_GESTO_FICHERO_DE: u64 = 0x0A;

/// **GUARDA el contenido del bloque anotado: lo crea, o publica su version
/// nueva.** `arg1` son los BYTES a tomar, igual que [`ES_GESTO_FICHERO_DE`].
///
/// === El quinto verbo, y por que hacia falta ===
///
/// Los otros cuatro --crear, carpeta, quitar, renombrar-- versionan el ARBOL.
/// Ninguno versiona un FICHERO: crear rechaza un nombre repetido, asi que un
/// fichero tenia UNA version para siempre. *"Cada escritura publica un estrato
/// nuevo"* era cierto del arbol y no lo era de lo que hay dentro.
///
/// === Y por que crear-o-sustituir puede ser UN verbo aqui ===
///
/// En un sistema que sobreescribe, guardar encima de algo que no sabias que
/// existia PIERDE lo que habia, y por eso hace falta preguntar antes.
///
/// ** Aqui no puede perder nada: el nodo viejo, su contenido y el estrato que
/// lo nombraba siguen enteros y alcanzables. Guardar encima **publica una
/// version, no destruye una** -- y el historial las muestra las dos.
///
/// [`ES_GESTO_FICHERO_DE`] sigue existiendo para lo contrario: cuando la
/// intencion es CREAR y hay que enterarse de que el nombre ya estaba.
pub const ES_GESTO_GUARDAR: u64 = 0x0B;

// == ** LAS RAMAS Y LA MEZCLA: mas subordenes de `TASK_OP_ES_GESTO` ===========
//
// `docs/plan/PLAN_LAS_RAMAS.md`, R4c-2b. Mismo renglon, misma puerta, y un
// prefijo propio porque la guia de F1 las agrupa por el: no todas escriben.
//
// ```text
//   ES_RAMA_CREAR     la ruta lleva el NOMBRE de la rama nueva
//   ES_RAMA_CAMBIAR   la ruta lleva el nombre de la rama a la que se pasa
//   ES_RAMA_CONTAR    la ruta lleva la rama que se quiere mezclar. NO escribe
//   ES_RAMA_CHOQUE    arg1 = cual, arg0 >> 8 = que trozo. NO escribe
//   ES_RAMA_ELEGIR    arg1 = cual, arg0 >> 8 = MEZCLA_A, _B o _QUITAR
//   ES_RAMA_MEZCLAR   sin nada: mezcla lo contado con lo elegido
// ```

/// **Crea una rama** con el nombre que traiga la ruta, en la punta de ahora.
/// No cambia de rama. Devuelve la generacion nueva, o `0`.
///
/// ** Una rama es un NOMBRE para una punta, guardado FUERA de la historia (la
/// tabla de ramas, D5): ni copia nada ni publica un estrato. Si el volumen no
/// tenia ramas, la de ahora pasa a llamarse `principal`.
pub const ES_RAMA_CREAR: u64 = 0x0C;

/// **Cambia a la rama** que diga la ruta: el volumen pasa a seguir su punta y
/// la de ahora se queda guardada en la tabla. Devuelve la generacion, o `0`.
///
/// ** No publica estrato, y por eso nadie se vuelve antepasado de nadie: ir y
/// venir entre ramas no las mezcla. Con `volver` si pasaba, y era el agujero
/// que la tabla de ramas cerro.
pub const ES_RAMA_CAMBIAR: u64 = 0x0D;

/// **FASE 1 de la mezcla: CUENTA** lo que costaria mezclar la rama de la ruta
/// en la de ahora, y apunta sus choques. No escribe nada.
///
/// Devuelve `(choques << MEZCLA_CHOQUES_SHIFT) | bloques`, o `0` si no se puede
/// (sin base comun, nada que mezclar, una carpeta demasiado grande: el motivo
/// va a CABINA). Los bloques suponen que cada choque se queda con lo de A.
///
/// ** DOS FASES porque a mitad de una mezcla el kernel no puede esperar a una
/// persona: tendria una transaccion abierta. Se cuenta, la persona elige con
/// calma, y solo entonces se mezcla.
pub const ES_RAMA_CONTAR: u64 = 0x0E;

/// **Lee un choque de lo contado.** `arg1` es cual; `arg0 >> 8`, que trozo.
///
/// El trozo `0` es la cabeza: `largo | lados << 8 | eleccion << 16` (lados:
/// `MEZCLA_LADO_A`, `MEZCLA_LADO_B`). Del `1` en adelante, ocho bytes de la
/// ruta cada uno, en little-endian. `0`: ese choque no existe.
///
/// * Un choque es un NODO entero con una ruta: lo cambiaron las dos ramas, de
/// formas distintas. Se muestra la ruta y que lado lo tiene; elegir es de la
/// persona (D3).
pub const ES_RAMA_CHOQUE: u64 = 0x0F;

/// **Elige un choque**: `arg1` es cual, `arg0 >> 8` es `MEZCLA_A`, `MEZCLA_B` o
/// `MEZCLA_QUITAR`. Devuelve `1` si se apunto.
///
/// ** Se elige un nodo ENTERO, no lineas: un fichero de ESTRATOS no es texto
/// por fuerza, y mezclar bytes de dos versiones a ciegas daria uno que no es
/// ninguno de los dos.
pub const ES_RAMA_ELEGIR: u64 = 0x10;

/// **FASE 2: MEZCLA** lo contado con lo elegido. Publica UN estrato de DOS
/// padres: la punta de ahora y la de la otra rama. Devuelve la generacion, o
/// `0`.
///
/// ** Dice que no sin tocar un sector si hay choques sin elegir, o si el
/// volumen cambio desde que se conto: un plan hecho sobre otro volumen no se
/// aplica. Se vuelve a contar.
pub const ES_RAMA_MEZCLAR: u64 = 0x11;

/// Elegir lo de la rama de ahora.
pub const MEZCLA_A: u64 = 1;
/// Elegir lo de la rama que entra.
pub const MEZCLA_B: u64 = 2;
/// Que no quede ninguno de los dos.
pub const MEZCLA_QUITAR: u64 = 3;
/// En la cabeza de un choque: la rama de ahora lo tiene.
pub const MEZCLA_LADO_A: u64 = 1;
/// En la cabeza de un choque: la rama que entra lo tiene.
pub const MEZCLA_LADO_B: u64 = 2;
/// Donde van los choques en la respuesta de [`ES_RAMA_CONTAR`].
pub const MEZCLA_CHOQUES_SHIFT: u64 = 32;
/// Los bloques, en la respuesta de [`ES_RAMA_CONTAR`].
pub const MEZCLA_BLOQUES_MASK: u64 = (1 << 32) - 1;
/// Cuantos choques se apuntan para elegir. Con mas, se cuentan pero no se
/// mezcla: una persona no elige a ciegas el que no ve.
pub const MEZCLA_CHOQUES_MAX: u64 = 64;

/// Cuanto contenido admite EL RENGLON. Es [`RESIDENTE_MAX`] de ESTRATOS: lo que
/// cabe DENTRO del nodo, sin gastar un bloque de datos.
///
/// ** ESTO YA NO ES EL TECHO DE UN FICHERO, y llego a serlo por accidente. Dos
/// limites distintos coincidieron en 96 --lo que cabe en el nodo y lo que
/// acumula el renglon-- y el segundo se quedo mandando sobre el primero.
/// [`ES_GESTO_FICHERO_DE`] entrega el contenido por un bloque de memoria y no
/// pasa por aqui: este numero solo mide el renglon corto.
pub const ES_GESTO_MAX: u64 = 96;
