//! **Lo que se CONSULTA y no cambia nada**: `INFO_*`, `CABINA_*`, `AUTOPSIA_*`
//! y `KLOG_*`.
//!
//! Cincuenta y ocho constantes con una propiedad en comun que decide el
//! reparto: **ninguna ejerce poder**. Leer un contador, una linea del log o el
//! informe de una muerte no concede nada, y por eso ninguna pide capability --
//! el mismo trato que tiene `INFO` desde el principio.
//!
//! # Por que es la familia que mas hay que vigilar
//!
//! Porque existe TRES veces: la implementa el kernel (`core/informe.rs`), la
//! declara este fichero y la consume el userland. Una fila escrita en dos de los
//! tres sitios es **un campo que contesta otra cosa de la que se pidio, sin que
//! nada falle al compilar**. Lo comprueba `build.ps1` sacando la lista de los
//! tres ficheros, nunca a mano.

/// Bytes de RAM que el asignador de marcos gobierna.
pub const INFO_RAM_TOTAL: u64 = 0x01;

/// Bytes libres AHORA.
pub const INFO_RAM_LIBRE: u64 = 0x02;

/// Marcos totales de 4 KiB.
pub const INFO_RAM_MARCOS: u64 = 0x03;

/// Marcos libres.
pub const INFO_RAM_MARCOS_LIBRES: u64 = 0x04;

/// Frecuencia del TSC en Hz. Es la que mide el tiempo de verdad en esta
/// maquina, no un numero nominal de la etiqueta.
pub const INFO_TSC_HZ: u64 = 0x05;

/// **Los ciclos que ESTA tarea ha corrido de verdad**, sin los que paso
/// esperando turno.
///
/// == *** POR QUE HACE FALTA, Y LO QUE COSTO NO TENERLO (2026-09-08) ========
///
/// Ring 3 solo podia medirse con `rdtsc`, que es RELOJ DE PARED: no distingue
/// *"trabaje"* de *"me echaron del CPU"*. El 08-09 esa confusion costo una caza
/// entera -- el compositor decia `cuerpo 1066 ms` y de esos solo **2,36 us por
/// vuelta** eran trabajo suyo.
///
/// ** Con esto, "el compositor trabaja mucho" y "al compositor no le dan turno"
/// dejan de leerse igual. Escalon **E1** de `docs/plan/PLAN_EL_COMPAS.md`: no se
/// puede presupuestar lo que no se mide.
pub const INFO_CPU_PROPIO: u64 = 0x4F;

/// **El ritmo del bus USB y su peor trabajo**, que es donde empieza la latencia.
///
/// ```text
///    bits  0..15   el periodo del bus, en ms
///    bits 16..47   el peor trabajo visto de la vuelta, en us
///    bits 48..55   cual: 0 bombeo, 1 rescate, 2 emergencia, 3 purga, 4 radar
/// ```
///
/// == *** POR QUE EXISTE: LA MANO Y EL PIXEL (2026-09-09) ==================
///
/// El periodo del bus es **el primer sumando** del camino de la mano al pixel, y
/// hoy son 4 ms fijos. Un raton declara su `bInterval` en el descriptor --muchos
/// piden 1-- y el sistema lo lee, se lo pasa al Endpoint Context y lo escribe en
/// el log; **el hilo que drena el anillo sigue yendo a 250 Hz**.
///
/// ** El otro campo es el precio de arreglarlo: a 1 ms la vuelta se hace cuatro
/// veces mas a menudo, y `peor_us` dice si cabe. Con esto la decision es un dato.
///
/// [!] Y hasta hoy los dos numeros solo los leia `cabina/cockpit.rs`, que es una
/// pantalla de RING 0 -- de donde no se vuelve. Ver `docs/plan/PLAN_EL_PIXEL.md`.
pub const INFO_USB_RITMO: u64 = 0x50;

// == *** LOS DOCE DEL DMA -- que el `save` diga lo que CABINA ya decia =======
//
// Peticion del propietario, 2026-09-10: *"el save actualizar por completo, y cabina
// tambien"*.
//
// El bit en vuelo, el perro guardian del plazo, el portero duro y el centinela
// se cablearon entre el 09-09 y el 10-09, y los cuatro contaban **solo para una
// pantalla de RING 0**. El propietario vive en el escritorio y al shell de Ring 0 no
// se vuelve: o sea que los numeros existian y **no llegaban a quien los pidio**.
//
// ** Es exactamente lo que le paso a `INFO_USB_RITMO` el 09-09, y esta escrito
// en su propia cabecera: *"hasta hoy los dos numeros solo los leia
// cabina/cockpit.rs, que es una pantalla de RING 0 -- de donde no se vuelve"*.
//
// > Una cuenta que solo se ve donde no se puede volver es una cuenta que se
// > mira una vez y se olvida.
//
// [!] Los tres que TIENEN QUE SER CERO --pisados, choques, caducados, rotas--
// son los que dan sentido a los demas: sin ellos, `vivos` es un numero sin
// contraste.

/// **Marcos con un DMA EN VUELO ahora mismo. Al apagar, CERO.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_VUELO_VIVOS: u64 = 0x51;

/// **Marcos que cambiaron de titular CON un DMA dentro. **CERO** (R-DMA-3).**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_VUELO_PISADOS: u64 = 0x52;

/// **Veces que dos aparatos pidieron el mismo marco. **CERO** (R-DMA-4).**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_VUELO_CHOQUES: u64 = 0x53;

/// **Que aparato es el que mas ha callado teniendo trabajo abierto (1..15).**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_MUDO_APARATO: u64 = 0x54;

/// **Y cuanto callo, en TICKS del TSC. De aqui sale el plazo de R-DMA-8 (N5b).**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_MUDO_TICKS: u64 = 0x55;

/// **Vuelos que pasaron de plazo. **CERO** (R-DMA-8).**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_CADUCADOS: u64 = 0x56;

/// **Maestros del bus que alcanzan la RAM y este kernel NO encendio.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_AJENOS_VISTOS: u64 = 0x57;

/// **De esos, a cuantos se les retiro el BME. Con el cerrojo en `Mirar`, 0.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_AJENOS_CERRADOS: u64 = 0x58;

/// **Puentes con BME: intocables a proposito -- cerrarlos calla la rama.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_PUENTES: u64 = 0x59;

/// **Veces que se miro el borde de la pagina de rebote.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_CENTINELA_MIRADAS: u64 = 0x5A;

/// **Veces que estaba ROTO: el aparato escribio mas alla de lo que declaro.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_CENTINELA_ROTAS: u64 = 0x5B;

/// **Veces que el HBA dijo haber movido MAS sectores de los pedidos.**
///
/// Del carril del DMA. Ver `NEUTRO/DMA/REGLAS.txt`.
pub const INFO_DMA_HBA_DE_MAS: u64 = 0x5C;

// == *** LO QUE CUESTA TENER LOS DOCE EN PIE (2026-09-10) ==================
//
// Hasta hoy levantar los nucleos costaba once girando al 100%, asi que el
// numero que hacia falta era `girando`. Con `MWAITX` la pregunta cambia: ya no
// es *cuantos giran*, es **cuanto tiempo estan apagados de verdad**.
//
// ** Y son DOS campos y no uno a proposito: mil siestas de un microsegundo se
// ven igual de bien en la cuenta y no apagan nada. El que demuestra el ahorro
// es el tiempo; la cuenta solo dice si el mecanismo se uso.
//
// El tercero --el C-state-- es el que dice si el silicio se lo tomo en serio:
// C1 para el nucleo y le deja los relojes; C6 lo apaga. Ver `plat/smp/dormir.rs`.

/// **Veces que un obrero se durmio de verdad con MWAITX. 0 = no hay MONITORX.**
///
/// Del carril de AXION. Ver `plat/smp/dormir.rs`.
pub const INFO_SMP_SIESTAS: u64 = 0x5D;

/// **TICKS del TSC pasados durmiendo, sumando todos los obreros.**
///
/// Del carril de AXION. Ver `plat/smp/dormir.rs`.
pub const INFO_SMP_MS_APAGADOS: u64 = 0x5E;

/// **Que tan hondo se duerme: el C-state, ya en numero (1 = C1). 0 = no duerme.**
///
/// Del carril de AXION. Ver `plat/smp/dormir.rs`.
pub const INFO_SMP_CSTATE: u64 = 0x5F;

/// **Siestas que algo corto antes del plazo. Alta en reposo = alguien despierta.**
///
/// Del carril de AXION. Ver `plat/smp/dormir.rs`.
pub const INFO_SMP_SIESTAS_CORTAS: u64 = 0x60;

/// **QUIEN es el maestro ajeno `i` (0..3)**: `vendor<<48 | device<<32 | bdf`.
/// Cero si no hay tantos.
///
/// ** Existe porque `INFO_DMA_AJENOS_VISTOS` decia CUANTOS y la pregunta M3 de
/// la tanda del 10-09 era CUALES. El arranque contesto `3` y los nombres se
/// quedaron en el scroll del arranque, donde nadie los lee. Del carril del DMA.
pub const INFO_DMA_AJENO_0: u64 = 0x61;
pub const INFO_DMA_AJENO_1: u64 = 0x62;
pub const INFO_DMA_AJENO_2: u64 = 0x63;
pub const INFO_DMA_AJENO_3: u64 = 0x64;

/// **Bytes que la RAM conservo del arranque anterior**, y que ya estan en
/// `CAIDA.TXT`. Cero = no habia rastro. Es la respuesta a *"la placa conserva
/// la DRAM en un reinicio en caliente?"*. Ver `cabina/caida.rs`.
pub const INFO_CAIDA_RECUPERADO: u64 = 0x65;
/// Cuantos arranques ha visto la caja negra en RAM. 1 = esta es la primera
/// vez que hay cabecera; si en el siguiente sale 2, la RAM sobrevivio.
pub const INFO_CAIDA_GENERACION: u64 = 0x66;
/// **Veces que el BSP durmio HONDO (`mwaitx`) en vez de `hlt`.** 0 = sin
/// `MONITORX`, o nunca estuvo ocioso. W1 de `docs/plan/PLAN_VATIOS.md`.
pub const INFO_BSP_REPOSOS: u64 = 0x67;
/// **Ticks del TSC que el BSP paso dormido hondo.** Contra el TSC total es el
/// porcentaje del tiempo en que la maquina no hacia nada -- y lo APAGABA.
pub const INFO_BSP_TICKS_REPOSO: u64 = 0x68;

/// **La frecuencia efectiva del nucleo AHORA, en Hz.** `0` = no se puede medir.
///
/// No es [`INFO_TSC_HZ`]: ese es el reloj de referencia, que no cambia nunca.
/// Este es a que va el nucleo de verdad, que en un Zen 3 se mueve entre 3,7 y
/// 4,6 GHz segun cuantos trabajen.
///
/// ** Es una MEDIDA, no un dato: sale de restar dos lecturas de MPERF/APERF, o
/// sea que **preguntarlo dos veces seguidas da la velocidad de ese intervalo**.
/// Un panel que se repinta obtiene la del ultimo refresco, que es lo que quiere.
pub const INFO_CPU_HZ_REAL: u64 = 0x20;

/// **Milivatios del PAQUETE desde la ultima consulta.** `0` = no se puede medir.
/// Medida por diferencia, como [`INFO_CPU_HZ_REAL`].
pub const INFO_CPU_MW_PAQUETE: u64 = 0x21;

/// **Milivatios del NUCLEO EN EL QUE SE LEE.** No de todos.
///
/// [!] La primera version de esta linea decia "de los nucleos", en plural, y el
/// metal del 12-08 mostro por que eso es poner un dato que no existe: con once
/// nucleos GIRANDO al 100%, este numero **bajo** de 11,9 a 9,2 W. No es que
/// consumieran menos: es que `CORE_ENERGY_STAT` es un contador **por nucleo** y
/// solo se lee el del BSP. Los otros once no aparecen aqui en absoluto.
///
/// Para verlos hace falta que **cada nucleo lea el suyo**, o sea trabajo
/// repartido -- la seccion 5 de `AXION_MAESTRO.md` antes que esto.
pub const INFO_CPU_MW_NUCLEO_ACTUAL: u64 = 0x22;

/// ** LOS CONTADORES QUE SOLO CRECEN (2026-09-12).
///
/// `INFO_CPU_HZ_REAL` y los dos `MW_*` de arriba se miden "desde la ultima
/// consulta" con UNA lectura anterior para todo el sistema: si dos programas
/// preguntan, cada uno ve el intervalo del otro. Estos cuatro no guardan nada de
/// nadie -- solo suben -- y cada lector resta los suyos. La resta, probada, esta
/// en `bmo_juicio::consumo`. `0` = no se sabe.
///
/// Microjulios del paquete desde el arranque. El kernel acumula las vueltas del
/// registro de 32 bits una vez por segundo.
pub const INFO_CPU_UJ_PAQUETE: u64 = 0x69;
/// Microjulios del nucleo que contesta (el BSP), desde el arranque.
pub const INFO_CPU_UJ_NUCLEO: u64 = 0x6A;
/// `MPERF` del BSP, crudo y de 64 bits.
pub const INFO_CPU_MPERF: u64 = 0x6B;
/// `APERF` del BSP, crudo y de 64 bits. Frecuencia = TSC_HZ x dAPERF / dMPERF.
pub const INFO_CPU_APERF: u64 = 0x6C;

// [!] AQUI VIVIA `INFO_CPU_MW_NUCLEOS`, y su borrado es la leccion.
//
// Se renombro a `INFO_CPU_MW_NUCLEO_ACTUAL` porque el plural mentia, y se dejo
// el nombre viejo como `#[deprecated]` "para no romper a quien lo use". **El
// guardian de contrato paro el build**, y tenia razon:
//
//   [X] OP_INFO field contract: INFO_CPU_MW_NUCLEOS falta en kernel, userland
//
// Una constante que vive en el ABI y no existe en los otros dos lados ES la
// deriva que ese guardian existe para cazar -- da igual que este marcada como
// obsoleta. Un alias amable en un CONTRATO no es amable: es un tercer nombre
// para un numero, y el contrato pasa a tener dos verdades.
//
// Y aqui no habia nada que no romper: el nombre nacio y murio el mismo dia.
/// **Que sabe medir el perfil de este silicio**, como banderas.
/// bit 0 = frecuencia efectiva / bit 1 = consumo.
///
/// Es lo que permite a la terminal decir QUE esta aplicando, en vez de pintar
/// ceros y dejar al que mira sin saber si el sensor no existe o el valor es 0.
pub const INFO_CPU_SENSORES: u64 = 0x23;

/// El pid de la ranura `n`. **`0` = no hay mas**, y es la condicion de parada.
pub const INFO_MEM_QUIEN_PID: u64 = 0x24;

/// Bytes que ese proceso tiene pedidos ahora mismo.
pub const INFO_MEM_QUIEN_BYTES: u64 = 0x25;

/// Cuantas peticiones lleva hechas. Distingue *"pidio un bloque grande"* de
/// *"esta pidiendo sin parar"*, que es la diferencia entre un juego y una fuga.
pub const INFO_MEM_QUIEN_PETICIONES: u64 = 0x26;


/* == LA RED, VISTA DESDE RING 3 ====================================
 *
 * ** Siete campos, y hasta hoy eran CERO: el kernel encontraba la NIC, le leia
 * la MAC y el estado del enlace, y **nada de eso cruzaba a Ring 3**. Un panel de
 * red en el compositor no era una cuestion de dibujar: era imposible, porque no
 * habia forma de preguntar.
 *
 * Son campos de INFORME y no operaciones sobre un handle a proposito: leer si
 * hay cable no es un privilegio, es una pregunta -- el mismo criterio que
 * `core::report` aplica a la RAM y a los nucleos. Transmitir SI necesitara una
 * capability; mirar, no.
 *
 * [!] Y son SIETE y no uno con banderas dentro. Un campo por hecho es lo que
 * permite que el panel diga *"hay NIC, no hay enlace"* en vez de *"red: 0"*, que
 * es la diferencia entre un diagnostico y un adorno.
 */

/// Hay una NIC reconocida: `1` o `0`. Lo primero que hay que saber, y lo unico
/// que distingue *"no hay tarjeta"* de *"hay tarjeta y no hay cable"*.
pub const INFO_NET_PRESENTE: u64 = 0x27;

/// `vendor << 16 | device` del PCI. En esta placa, `0x10EC8168` -- una Realtek
/// RTL8168. Se entrega crudo: el numero ES la identificacion, y traducirlo a un
/// nombre bonito en el kernel seria meter una tabla de fabricantes en Ring 0.
pub const INFO_NET_VENDOR_DEVICE: u64 = 0x28;

/// La MAC, los seis bytes en los 48 bits bajos, byte 0 el mas significativo.
///
/// ** Cabe en UN campo y por eso va en uno. Una MAC son 48 bits y un campo de
/// informe son 64: partirla en dos habria sido inventarse un problema de
/// ensamblado en el lado del cliente.
pub const INFO_NET_MAC: u64 = 0x29;

/// El byte `PHYstatus` **CRUDO**, sin interpretar.
///
/// ** Y va crudo a proposito, que es la misma decision que ya tomo el driver:
/// *"se guarda sin interpretar ademas de interpretado: el dia que un bit no
/// cuadre, el byte entero es la prueba y las funciones son la opinion"*. Un
/// panel que solo muestra la opinion no puede ayudar el dia que la opinion falle.
pub const INFO_NET_PHY_CRUDO: u64 = 0x2A;

/// Megabits que declara el enlace: 10, 100, 1000 -- o `0` si esta abajo.
///
/// [!] El cero no es un error: es *"no hay cable"*, y es una respuesta.
pub const INFO_NET_MEGABITS: u64 = 0x2B;

/// El receptor esta armado: `1` o `0`. Distingue *"no llega nada"* de *"no
/// estamos escuchando"*, que es la confusion mas cara de depurar en una red.
pub const INFO_NET_RX_ARMADO: u64 = 0x2C;

/// Tramas recibidas desde que se armo. **La cifra que dice si el cable vive.**
pub const INFO_NET_RX_TRAMAS: u64 = 0x2D;
/// Bytes de trama recibidos, sin el FCS: lo que se leyo de verdad.
pub const INFO_NET_RX_BYTES: u64 = 0x4A;
/// **`MPC`: las tramas que la TARJETA tiro** por no haber descriptor
/// libre. Es el unico contador de red que no lleva BMO-X -- lo lleva el
/// silicio, y sin el, "40 tramas recibidas" es una cifra sin denominador.
pub const INFO_NET_RX_PERDIDAS: u64 = 0x4B;
/// El reparto por protocolo, empaquetado: `arp | ipv4<<16 | ipv6<<32 |
/// otros<<48`, 16 bits cada uno. Mismo criterio que `INFO_NET_PCI`, que
/// tambien mete tres numeros en uno: por la puerta cabe UN valor.
pub const INFO_NET_RX_TIPOS: u64 = 0x4C;

/// Donde esta en el bus: `bus << 16 | dispositivo << 8 | funcion`.
///
/// Hace falta para el caso raro y real de dos NIC: sin esto, dos tarjetas dan
/// dos informes identicos y no hay forma de decir de cual habla cada uno.
pub const INFO_NET_PCI: u64 = 0x2E;
/// Tramas que la tarjeta devolvio y NO eran limpias (error, partida, enana, o
/// que no cabian). Se cuentan y se devuelven: un anillo que paraba en ellas se
/// atascaba para siempre (2026-09-13).
pub const INFO_NET_RX_MALAS: u64 = 0x6D;

/// -- ** LO QUE `save` NO DECIA Y CABINA SI (2026-09-17) ------------------
///
/// Eddi: *"Save tiene que decir todo en CABINA... mas organizado"*. Tres cosas
/// que solo se leian en F11 o en `cabina fallos`: que controlador USB maneja
/// este kernel y que llego por cada puerto, como van los prestamos (las
/// ventanas), y los avisos. El movil del propietario enchufado en el xHC que no se
/// maneja fue lo que lo hizo visible: F11 callaba y `save` no sabia nada.
///
/// # `INFO_USB_CENSO`: los controladores, empaquetados
///
/// `[0..8)` xHC censados, `[8..16)` aparatos que ve el ELEGIDO, `[16..24)`
/// aparatos en OTRO xHC que este kernel no mira (huerfanos), `[24..48)`
/// bus/dev/func del elegido como `bus<<16 | dev<<8 | func`, bit 63 = hay
/// elegido.
pub const INFO_USB_CENSO: u64 = 0x6E;
/// El libro del portero: `[0..16)` fichas escritas, `[16..32)` que no
/// cupieron, `[32..48)` admitidas, `[48..64)` rechazadas.
pub const INFO_USB_FICHAS: u64 = 0x6F;
/// La ficha `n >> 8` (indice en los bits altos, como `INFO_MEM_QUIEN_*`):
/// `vid<<48 | pid<<32 | puerto<<24 | clase<<16 | subclase<<8 | proto`, los
/// mismos `papeles` que CABINA. Cero si no hay tal ficha.
pub const INFO_USB_FICHA: u64 = 0x70;
/// El veredicto de la ficha `n >> 8` (`bmo_uhid::VEREDICTO_*`) en `[0..8)`, y
/// en `[8..16)` su DETALLE: con "no se pudo preparar" (7), el `cc` con que el
/// xHC nego el Configure Endpoint. Cero si no hay tal ficha.
pub const INFO_USB_FICHA_VEREDICTO: u64 = 0x71;
/// Los prestamos: `[0..8)` ofertas vivas, `[8..16)` tomadas, `[16..24)`
/// huerfanas, `[32..64)` negadas desde el arranque.
pub const INFO_PRESTAMOS: u64 = 0x72;

/// ** LAS CACHES, MEDIDAS (2026-09-19): CPUID 0x8000001D en el arranque, una
/// por campo. Hasta hoy el kernel solo tenia la tabla de la hoja de AMD y la
/// imprimia como si la hubiera preguntado.
///
/// ```text
///    bits  0..23   medida en KiB
///    bits 24..31   linea en bytes
///    bits 32..39   vias (0 = totalmente asociativa)
///    bits 40..47   hilos que la comparten
///    bit  62       NO coincide con lo esperado (`PERFIL/CPU.txt`)
///    bit  63       medida
/// ```
///
/// `0` = no se pudo medir. NO se rellena con lo esperado: un hueco dicho es
/// mejor que una suposicion que parece una medida (LEY 24).
pub const INFO_CPU_CACHE_L1D: u64 = 0x73;
pub const INFO_CPU_CACHE_L1I: u64 = 0x74;
pub const INFO_CPU_CACHE_L2: u64 = 0x75;
pub const INFO_CPU_CACHE_L3: u64 = 0x76;

/// -- ** LA FICHA DE CADA PROGRAMA (2026-09-20) -----------------------------
///
/// Eddi: *"con el SAVE ese mismo tiene que redactar TODO"*. El cargador sabia
/// todo esto al admitir un `.bex` y lo tiraba; el escritorio solo podia decir
/// "pid y MiB". Ahora el registro de programas del kernel (8 fichas, las mas
/// viejas se caen y `INFO_PROGRAMAS_OLVIDADOS` las cuenta) se lee entero desde
/// Ring 3: lo que el BEF2 DECLARO y lo que el cargador hizo con ello.
///
/// El indice del programa viaja en `n >> 8`, como en `INFO_MEM_QUIEN_*`; el
/// nombre y la etiqueta salen por `OP_INFO_TEXTO` con `INFO_TXT_PROG_*`.
/// **Cero = no hay tal programa**, y es la condicion de parada. Son campos de
/// OTROS (piden la autoridad LANZAR, que el escritorio tiene).
///
/// `INFO_PROG_QUIEN`:
/// ```text
///    bits  0..15   pid
///    bits 16..31   tid
///    bits 32..39   regiones que trajo (1..=4)
///    bits 40..47   firma: 0 sin tabla de hashes (imagen embebida), 1 solo
///                  integridad (ALGO_NINGUNO), 2 Ed25519 y la clave en el ancla
///    bits 48..55   indice de la clave en el ancla, cuando firma == 2
///    bit  63       admitido (0 = no paso la puerta)
/// ```
pub const INFO_PROG_QUIEN: u64 = 0x77;
/// `[0..32)` bytes del fichero, `[32..64)` bytes mapeados en el proceso.
pub const INFO_PROG_IMAGEN: u64 = 0x78;
/// **Dos indices**: `n >> 8` = `programa * 4 + region`, con la region como en
/// la cabecera de BEF2 (0 codigo, 1 constantes, 2 datos, 3 ceros). Bytes en
/// memoria de esa region; 0 = no la trae.
pub const INFO_PROG_REGION: u64 = 0x79;
/// El cierre: `[0..16)` relocs aplicadas, `[16..24)` cierres que CUADRARON con
/// su hash (regiones + relocs), `[24..32)` cierres sin hash con el que
/// comparar, `[32..64)` el `xcr0` que la imagen declaro (los 32 bits bajos,
/// que son los que existen).
pub const INFO_PROG_CIERRE: u64 = 0x7A;

/// **El veredicto del PEOR retraso del latido del bus USB** (2026-09-21).
///
/// `el latido del bus llego TARDE 1266 ms` salio en dos saves seguidos y no
/// decia QUIEN. Esto lo dice: `[0..16)` el retraso en ms, `[16..24)` el tid que
/// tuvo el CPU mientras el bus esperaba, `[24..40)` cuantos de esos ms fueron
/// suyos, `[40..56)` cuantos ticks dio el reloj durante el retraso (0 con un
/// retraso grande = alguien tenia las interrupciones CERRADAS), `[56..64)` lo
/// que costo la vuelta anterior del propio bus, en ms (si es ~ el retraso, el
/// culpable era el bus). Todo cero = nunca llego tarde por encima del umbral.
pub const INFO_USB_LATIDO: u64 = 0x7B;
/// El tick del reloj en que paso ese peor retraso. `0` = nunca paso.
pub const INFO_USB_LATIDO_CUANDO: u64 = 0x7C;

/// **La retencion mas larga de un cerrojo del kernel**, en ciclos de TSC
/// (2026-09-21). Un cerrojo tomado es `cli`: mientras se retiene, el reloj no
/// suena y el orquestador esta ciego. `0 choques` no dice nada de esto: en un
/// nucleo nadie pelea y aun asi un cerrojo puede retenerse un segundo. El
/// nombre del cerrojo, en [`INFO_TXT_CERROJO_PEOR`]. Se convierte a tiempo
/// con [`INFO_TSC_HZ`].
pub const INFO_SPIN_RETENIDO: u64 = 0x7D;
/// **Las veces que el tick hizo valer el rango**: le quito el CPU a una tarea
/// antes de acabarse su quantum porque otra de mas rango estaba en pie. Es la
/// cuenta de la expropiacion al despertar (`scheduler::on_timer`).
pub const INFO_EXPROPIADAS: u64 = 0x7E;

/// **El COMPAS del n-esimo hilo de kernel con contrato** (n en `[8..)`), EX3
/// del `PLAN_EL_COMPAS` (2026-09-21): `[0..8)` tid, `[8..24)` periodo en ms,
/// `[24..40)` presupuesto en us por periodo, `[40..64)` periodos que cerro
/// con mas gastado que presupuesto (los incumplimientos). `0` = no hay mas.
/// El nombre, en [`INFO_TXT_COMPAS_NOMBRE`].
pub const INFO_COMPAS: u64 = 0x7F;
/// La otra mitad: `[0..32)` el turno mas largo visto, en us; `[32..64)` los
/// turnos contados. Con los dos se lee si el hilo cumple lo que declaro.
pub const INFO_COMPAS_VUELTAS: u64 = 0x80;
/// La LINEA del fichero donde se tomo el cerrojo de [`INFO_SPIN_RETENIDO`];
/// el fichero, en [`INFO_TXT_CERROJO_SITIO`]. El nombre dice que cerrojo; el
/// sitio dice que funcion lo retuvo. `0` = ninguno se ha soltado aun.
pub const INFO_SPIN_RETENIDO_LINEA: u64 = 0x81;

/// -- ** EL AUDIO, ENTERO Y SIN HANDLE (2026-09-21) ------------------------
///
/// Preguntar QUE HAY no es lo mismo que tener derecho a usarlo: estas se
/// leen por `OP_INFO`, sin el handle de audio, que es de un solo propietario.
///
/// El audifono USB reclamado: `[0..8)` su ranura (0 = no hay) | `[8..16)`
/// canales | bit 16 tiene mute | bit 17 declara reproduccion | bit 18 el
/// aparato CONFIRMO el ultimo volumen | `[24..32)` pct mandado (0xFF =
/// ninguno) | `[32..40)` pct pedido y aun no mandado (0xFF = ninguno) |
/// `[40..48)` el mapa de [`AUDIO_OP_DEVICES`] (altavoz PC, HDA, USB).
pub const INFO_AUDIO_APARATO: u64 = 0x82;
/// El volumen en 1/256 dB, cuatro `i16` en dos bytes cada uno: `[0..16)`
/// minimo del aparato | `[16..32)` maximo | `[32..48)` lo ultimo mandado |
/// `[48..64)` lo que el aparato dijo tener al confirmar.
pub const INFO_AUDIO_RANGO: u64 = 0x83;
/// El tubo de reproduccion: `[0..24)` frecuencia en Hz | `[24..40)` bytes
/// por trama | `[40..56)` `wMaxPacketSize` | bit 56 abierto | bit 57
/// armado (mandando silencio). `0` = no hay tubo.
pub const INFO_AUDIO_TUBO: u64 = 0x84;
/// `[0..32)` tramas isocronas encoladas desde el arranque | `[32..64)` las
/// que llegaron TARDE a su microtrama.
pub const INFO_AUDIO_TRAMAS: u64 = 0x85;
/// `[0..32)` huecos (vueltas sin trama que mandar) | `[32..64)` tramos del
/// bufer prestado que el juez del DMA VETO.
pub const INFO_AUDIO_HUECOS: u64 = 0x86;
/// `[0..32)` pid del propietario del audio (0 = nadie) | `[32..64)` bytes
/// pendientes en el bufer prestado.
pub const INFO_AUDIO_PROPIETARIO: u64 = 0x87;
/// **Cuantos formatos de reproduccion declara el audifono, y cual se cogio**
/// (2026-09-22): `[0..8)` cuantos | `[8..16)` el indice del elegido.
///
/// [!] Hasta hoy solo se guardaba UNO --el primero que el aparato declaraba--
/// y no habia forma de saber si ofrecia mas. El `save` los muestra todos porque
/// **la tabla la escribe el aparato, no el programador**.
pub const INFO_AUDIO_FORMATOS: u64 = 0x88;
/// El formato `i` (`INFO_AUDIO_FORMATO | (i << 8)`): alt, canales, bits,
/// subframe, `wMaxPacketSize`, cuantas frecuencias, si CABE en 1 ms, si es el
/// elegido y su sincronia. Ver `uaudio::info_formato` en el kernel.
pub const INFO_AUDIO_FORMATO: u64 = 0x89;
/// La frecuencia `k` del formato `i`, en Hz:
/// `INFO_AUDIO_FRECUENCIA | (i << 8) | (k << 12)`.
pub const INFO_AUDIO_FRECUENCIA: u64 = 0x8A;
/// **EL MAESTRO** (2026-09-22): `[0..16)` el fader | `[16..32)` la parte que
/// se le pidio al APARATO | `[32..48)` la ganancia DIGITAL que hay puesta ahora
/// (por donde va la rampa); los tres `i16` en 1/256 dB | bit 48 mudo | bit 49
/// el escritorio lo ha movido alguna vez (antes, el aparato tiene el volumen de
/// fabrica y no se le toca) | `[56..64)` el estado: 0 sin tubo, 1 en marcha,
/// 2 no es PCM de 16 bits, 3 la trama no cabe en la etapa, 4 sin marco. En 2,
/// 3 y 4 el sonido pasa SIN TOCAR y el fader no hace nada: se dice.
pub const INFO_AUDIO_MAESTRO: u64 = 0x8B;
/// El medidor del maestro, **lo que sale al cable**, en ventanas de 50 ms:
/// cuatro `i16` en 1/256 dBFS -- `[0..16)` pico izquierdo | `[16..32)` pico
/// derecho | `[32..48)` RMS izquierdo | `[48..64)` RMS derecho. -96 dB = nada.
pub const INFO_AUDIO_MEDIDOR: u64 = 0x8C;
/// El limite del maestro: `[0..32)` muestras DOBLEGADAS desde el arranque (la
/// luz de RECORTE: si sube, lo que sale ya no es la onda) | `[32..48)` lo que
/// el limite esta bajando ahora (`i16`, 0 = nada) | `[48..64)` ventanas del
/// medidor cerradas (da la vuelta): si no sube, el medidor esta parado.
pub const INFO_AUDIO_LIMITE: u64 = 0x8D;
/// **El volumen con el que vino el aparato**, leido con `GET_CUR` al
/// reclamarlo y antes de mandarle nada: `[0..16)` el `i16` en 1/256 dB |
/// bit 16 = se leyo.
pub const INFO_AUDIO_FABRICA: u64 = 0x8E;
/// **LOS TIRONES** (2026-09-22): el silencio que sale con el productor YA en
/// marcha, que es el que se oye -- `huecos` suma ademas el del arranque, en el
/// que la app aun no ha escrito nada. `[0..32)` tramas (ms) en silencio en
/// marcha | `[32..48)` cuantos CORTES (rachas seguidas) | `[48..64)` el corte
/// mas largo, en ms.
pub const INFO_AUDIO_TIRONES: u64 = 0x8F;
/// **LAS VOCES** (2026-09-22): `[0..16)` canales que suenan (bit n = canal n)
/// | `[16..48)` bytes del banco | `[48..64)` pid del banco (0 = no hay).
pub const INFO_AUDIO_VOCES: u64 = 0x90;
/// `[0..32)` voces tocadas | `[32..48)` rechazadas por el juez (el motivo, en
/// CABINA) | `[48..64)` ordenes que no cupieron en la cola.
pub const INFO_AUDIO_VOCES_CUENTA: u64 = 0x91;

/// -- ** EL METRO DE LA PUERTA -------------------------------------------
///
/// Cuantas puertas ha servido el kernel, y cuantos ciclos ha pasado DENTRO de
/// `dispatch` sirviendolas. Se leen los dos y se dividen.
///
/// Existen porque el 2026-08-16 `c/coste.bex` midio una puerta desde Ring 3 en
/// **2615 ciclos** contra 20 de una llamada normal, y ese numero no podia
/// contestar la pregunta siguiente: **donde se van.** Restando lo de dentro de
/// `dispatch` al total queda **lo que tarda el stub de ensamblador** -- los
/// pushes, el `xsave64`, el `xrstor64` y el `iretq`. Sin esa resta, tocar el
/// stub seria operar sobre una sospecha, y es el codigo que produjo el `#GP` en
/// `xrstor`.
///
/// [!] **Se leen como DELTA, no como absoluto**: antes y despues del bucle que
/// se quiera medir. No hay operacion de puesta a cero a proposito -- un delta
/// mide justo la poblacion que interesa y no arrastra lo que hizo la maquina
/// arrancando. Y las dos lecturas son dos puertas, que tambien se cuentan.
///
/// ** LA RESPUESTA, medida en el Ryzen el mismo dia: `2663 = dispatch 318 +
/// stub 2345`, o sea el **88% en el ensamblador**. Y resolver una capability
/// son **83 ciclos**, 76 de ellos dentro de `dispatch`. Estos dos campos ya
/// hicieron su trabajo; siguen aqui porque el reparto vuelve a hacer falta
/// cada vez que se toque `entry.rs`.
pub const INFO_SYSCALL_CUENTA: u64 = 0x2F;

/// Ciclos de TSC acumulados dentro de `dispatch`. Ver [`INFO_SYSCALL_CUENTA`].
pub const INFO_SYSCALL_CICLOS: u64 = 0x30;

/// -- ** EL REPARTO DENTRO DEL STUB ---------------------------------------
///
/// El primer reparto dejo el 88% de una puerta en el ensamblador y no supo
/// decir en QUE parte. Cambiar `xsave64` por `xsaveopt64` --el sospechoso
/// nombrado-- compro 45 ciclos de 2345: **el 2%**. Estos dos campos parten esa
/// mitad ciega en tres trozos que se leen igual, como delta:
///
/// ```text
///    GUARDA     la cabecera a cero + el `xsaveopt64`
///    CICLOS     dentro de `dispatch`  (ya existia)
///    RESTAURA   las comprobaciones del sello + el `xrstor64`
///    resto      total - los tres = `syscall` + pushes + pops + `iretq`
/// ```
///
/// **`resto` es la casilla que decide.** Grande significa que el coste esta en
/// las dos transiciones de privilegio y que afinar el stub no lo va a mover --
/// lo que se mueve entonces es `sysretq` en vez de `iretq`, o agrupar llamadas.
///
/// [!] Se dividen entre [`INFO_SYSCALL_CUENTA`], la MISMA cuenta de puertas que
/// los ciclos de `dispatch`: las tres etapas ocurren una vez por puerta, asi
/// que las cuatro cifras se reparten el mismo denominador y **tienen que sumar
/// menos que el total**. Si suman mas, el instrumento miente y se dice.
pub const INFO_SYSCALL_CICLOS_GUARDA: u64 = 0x35;

/// Ciclos devolviendo el contexto. Ver [`INFO_SYSCALL_CICLOS_GUARDA`].
pub const INFO_SYSCALL_CICLOS_RESTAURA: u64 = 0x36;

/// -- ** EL HISTOGRAMA POR CLASE: donde se USA la puerta -------------------
///
/// `INFO_SYSCALL_CUENTA` dice **cuantas** puertas. Esto dice **de que tipo**, y
/// es la mitad que faltaba: se sabia lo que cuesta cada clase --875 / 1125 /
/// 2,2 M-- y **no cuantas veces se pide cada una**, asi que *"donde se usa mas"*
/// era una suposicion. Un coste por vez sin veces por segundo no es un
/// porcentaje y no puede ordenar el trabajo (`docs/CENSO_DE_EJES.md`, R-CENSO3).
///
/// ```text
///    0  TAREA     pseudo-capability: `INVOKE(CURRENT_TASK, ...)`   ~875
///    1  HANDLE    resolvio una capability REAL                    ~1125
///    2  CONSOLA   escritura de consola                            ~2,2 M
///    3  ESPERA    `WAIT`                                          cede el turno
/// ```
///
/// ** LAS CUATRO SE DECIDEN POR CONSTRUCCION, NO POR UNA LISTA. La primera
/// version iba a separar "operacion barata" de "operacion que camina una tabla",
/// y eso pedia una lista de operaciones escrita a mano -- que es lo que ya se
/// quedo congelada dos veces en este arbol. Estas cuatro salen de datos que el
/// despachador **ya tiene en registros**: que puerta es, si el handle era
/// `CURRENT_TASK`, y si la operacion es la de consola (que tiene su propia rama
/// desde siempre). Ninguna casilla necesita saber nada nuevo.
///
/// [!] **Por que CONSOLA se saca aparte aunque sea una operacion de tarea**:
/// porque cuesta **2.500 veces** una puerta pelada. Metida en `TAREA` se lleva
/// la media entera y tapa justo lo que se quiere ver. Es la unica operacion del
/// sistema con esa diferencia de orden de magnitud, y por eso es una casilla y
/// no el principio de una lista.
///
/// El indice va EMPAQUETADO en el campo, igual que `INFO_MEM_QUIEN_*`:
/// `INFO_SYSCALL_CLASS | (clase << 8)`. Un campo nuevo por casilla habria sido
/// cuatro numeros en el contrato congelado para responder una sola pregunta.
///
/// [!] **Y suman MENOS que [`INFO_SYSCALL_CUENTA`], a proposito**: lo que no cae
/// en ninguna casilla es una puerta que no era ninguna de las cuatro (hoy, el
/// numero de syscall retirado). Esa resta es la comprobacion del instrumento --
/// si algun dia sale grande, hay trafico que este histograma no esta viendo.
pub const INFO_SYSCALL_CLASS: u64 = 0x3A;

/// `INVOKE` sobre `CURRENT_TASK`: no resuelve ningun handle.
pub const SYSCALL_CLASS_TASK: u64 = 0x00;
/// `INVOKE` que resolvio una capability real -- paga el handle.
pub const SYSCALL_CLASS_HANDLE: u64 = 0x01;
/// Escritura de consola: dibuja glifos y hace scroll.
pub const SYSCALL_CLASS_CONSOLE: u64 = 0x02;
/// `WAIT`: la unica puerta que puede no devolver el turno.
pub const SYSCALL_CLASS_WAIT: u64 = 0x03;
/// Cuantas casillas tiene el histograma. Ver [`INFO_SYSCALL_CLASS`].
///
/// [!] Va en HEXADECIMAL como sus cuatro hermanas, y no por gusto: el guardian
/// del build las barre con el mismo patron que las operaciones
/// (`= 0x...`), asi que un `= 4` decimal las dejaria **fuera de la
/// comprobacion sin que nada avise** -- un guardian que lee menos no avisa de
/// menos: avisa de nada.
pub const SYSCALL_CLASS_COUNT: u64 = 0x04;

/// -- ** EL PRESUPUESTO DE CICLOS ------------------------------------------
///
/// Lo que una puerta **tiene permitido** costar. El metro dice lo que cuesta
/// hoy; sin esto, nada en el arbol impide que la proxima pieza lo devuelva a
/// 2000. Un numero sin contrato es una anecdota.
///
/// Cada campo trae DOS numeros empaquetados, `meta << 32 | techo`:
///
/// ```text
///    techo   la ultima medida CONFIRMADA en metal. Cruzarlo es una regresion.
///    meta    a donde tiene que llegar. No alcanzarla es DEUDA, no fallo.
/// ```
///
/// Van juntos en un campo --como [`INFO_CPU_EXT_AVERIAS`] empaqueta cuatro--
/// porque separarlos permitiria leer uno y no el otro, que es justo el error
/// que hace decir *"cumple"* a algo que no llego a la meta.
///
/// [!] **No se comprueba en el arranque, y no es un olvido**: al arrancar no se
/// ha servido ni una puerta y el metro esta vacio. Un presupuesto solo se juzga
/// contra trafico real, asi que quien lo lee es `c/coste.bex` desde Ring 3 --
/// que ademas es el unico sitio alcanzable desde el escritorio.
///
/// La tabla y el porque de cada cifra viven en
/// `ring0/syscall/presupuesto.rs`; aqui solo esta la ventana.
pub const INFO_PRESUPUESTO_PUERTA: u64 = 0x37;

/// Presupuesto de la mitad Rust. Ver [`INFO_PRESUPUESTO_PUERTA`].
pub const INFO_PRESUPUESTO_DISPATCH: u64 = 0x38;

/// Presupuesto de resolver una capability. Ver [`INFO_PRESUPUESTO_PUERTA`].
pub const INFO_PRESUPUESTO_HANDLE: u64 = 0x39;

/// **1 si las tres filas de arriba se midieron en LA MAQUINA QUE ESTA
/// CORRIENDO**; 0 si no.
///
/// # Por que un presupuesto tiene propietario
///
/// Un techo son **ticks del TSC de una placa concreta**. El mismo kernel
/// arranca en cualquier x86-64, y alli esos numeros no son ni estrictos ni
/// laxos: son **de otra maquina**. Juzgar con ellos da una falsa regresion en un
/// CPU mas lento o un falso aprobado en uno mas rapido -- las dos son el mismo
/// fallo, opinar sin derecho.
///
/// La identidad es familia y modelo de CPUID **mas la frecuencia del TSC**, con
/// un 1% de tolerancia: dos CPU del mismo modelo con TSC distinto no pueden
/// compartir una tabla escrita en ticks.
///
/// ** Y CUANDO ESTO ES 0, LOS TRES CAMPOS DE ARRIBA CONTESTAN CERO -- o sea
/// `sin declarar`, que todo cliente ya sabe leer. La proteccion vive en el
/// valor, no en que alguien se acuerde de consultar este campo: quien no lo
/// conozca pierde el MOTIVO, jamas el freno. Al reves --contestar el techo bueno
/// y confiar en que el cliente compruebe-- bastaria con un olvido para producir
/// un veredicto falso.
/// ```text
///    bit 0        coincide TODO -- el unico que decide
///    bit 1        familia y modelo coinciden
///    bit 2        el TSC coincide (dentro del 1%)
///    bits  8..15  familia ESPERADA      16..23  modelo ESPERADO
///    bits 24..31  familia LEIDA         32..39  modelo LEIDO del silicio
/// ```
///
/// ** Lleva los DOS LADOS a proposito. Un `bool` frena el trinquete y no lo
/// arregla: el dia que diga que no, hay que saber si fallo el modelo o el reloj
/// y con que numeros. Un "no" sin motivo manda a leer codigo; este manda a
/// cambiar una cifra.
pub const INFO_PRESUPUESTO_MAQUINA: u64 = 0x3D;

/// Los bits de [`INFO_PRESUPUESTO_MAQUINA`].
pub const MAQ_COINCIDE: u64 = 1 << 0;
pub const MAQ_CPU_OK: u64 = 1 << 1;
pub const MAQ_TSC_OK: u64 = 1 << 2;

/// **EL SUELO DEL HARDWARE**: `medido << 32 | ticks`.
///
/// Lo que cuesta cruzar el anillo en este silicio -- `syscall` + `sysretq` y
/// nada mas. **No es merito ni culpa de BMO**, y hoy va sumado dentro de los 792
/// ticks de una puerta sin que nada los separe.
///
/// # Para que sirve separarlo
///
/// Porque `suelo + sobrecoste` no dice si el kernel esta bien: mezcla el
/// silicio con el codigo. Restado, sale **la unica cifra de rendimiento que
/// sobrevive a un cambio de CPU**:
///
/// ```text
///    cuantas veces el suelo del hardware cuesta una puerta de BMO
///    hoy 5,3x  ->  la meta declarada seria 2,0x
/// ```
///
/// Si BMO adelgaza, ese numero baja **en todas las maquinas a la vez**.
///
/// # ** La regla que impide que esto sea una trampa
///
/// > **El suelo se MIDE. El multiplicador se ESCRIBE.**
///
/// Un presupuesto que se recalibrara solo entero se ceniria a lo que hubiera,
/// **incluida una regresion**: se convertiria en la talla nueva y el juez
/// aprobaria siempre. Se ajusta lo que es del CPU; jamas el veredicto.
///
/// [!] **Bit 32 = medido.** En `0` el numero es una estimacion del analisis y
/// **no puede derivar ningun techo**: solo vale para mirar el ratio, y quien lo
/// imprima tiene que decir que lo es.
pub const INFO_SUELO_CRUCE: u64 = 0x3E;

/// -- ** EL CENSO DE EXTENSIONES, legible desde Ring 3 ---------------------
///
/// Cuantas extensiones cubre el censo, y dos mascaras de bits sobre ESA lista
/// en ESE orden: bit `i` = la fila `i`. El nombre de cada fila se pide por
/// texto con [`INFO_TXT_EXT_NOMBRE`].
///
/// # Por que existen
///
/// El censo se escribio como orden `ext` del shell de Ring 0, y a ese shell no
/// se vuelve una vez arranca el escritorio -- el rescate `Ctrl+Alt+Esc` se
/// niega a echar al compositor a proposito. O sea que era una tabla correcta
/// que su propietario no podia mirar. Estas filas son la respuesta, y son filas de
/// tabla y no un syscall nuevo, que es para lo que `OP_INFO` existe.
///
/// # Por que mascaras y no una linea de texto ya pintada
///
/// Porque el kernel contesta HECHOS y quien pinta decide como. Un renglon
/// pre-formateado ataria a todo cliente al ancho, al orden y al color del
/// kernel. Con las mascaras, el escritorio pinta el conflicto en rojo y el
/// shell en su columna, **sin que ninguno de los dos lleve una segunda lista
/// de nombres** que un dia diga otra cosa.
pub const INFO_CPU_EXT_N: u64 = 0x31;

/// Bit `i` = el silicio DECLARA la extension `i`.
pub const INFO_CPU_EXT_HAY: u64 = 0x32;

/// Bit `i` = BMO la USA. `USA & !HAY` es un conflicto: una instruccion que
/// dara `#UD` en esta maquina.
pub const INFO_CPU_EXT_USA: u64 = 0x33;

/// Los cuatro contadores que tienen que ser cero, de 16 en 16 bits:
/// conflictos, mudas, repetidas, sin_sitio.
///
/// [!] Solo el primero se puede deducir de las mascaras. Los otros tres son
/// sobre la TABLA y no sobre el silicio -- una fila sin motivo escrito, una
/// repetida, una que no cupo -- y sin ellos un panel diria que todo va bien
/// mirando la mitad.
pub const INFO_CPU_EXT_AVERIAS: u64 = 0x34;

/// Hilos logicos y nucleos fisicos que el CPU declara.
pub const INFO_CPU_HILOS: u64 = 0x06;

pub const INFO_CPU_NUCLEOS: u64 = 0x07;

/// **CUANTO FIARSE del par `INFO_CPU_NUCLEOS` / `INFO_CPU_HILOS`.** Un mapa de
/// bits, y `0` significa *"los tres testigos dicen lo mismo"*.
///
/// ```text
///    bit 0   las dos hojas de CPUID se contradicen
///    bit 1   los hilos por nucleo NO se pudieron medir
///    bit 2   *** el PERFIL desmiente al silicio: no es este chip
///    bit 3   la MADT declara otros hilos que CPUID
/// ```
///
/// # Por que un panel necesita esto (2026-08-25)
///
/// Ese dia el escritorio pinto `27 fisicos / 54 logicos` en un 6/12 y **no tenia
/// forma de saber que dudar**: los dos campos de arriba son `u64` pelados y un
/// numero malo se pinta igual de nitido que uno bueno.
///
/// *** El aviso existia --en el log del arranque, y solo si alguien tecleaba
/// `smp`-- y ahi no lo ve nadie: **el propietario vive en el escritorio y al shell de
/// Ring 0 no se vuelve.** Un diagnostico al que no se llega desde donde se ve el
/// sintoma no es un diagnostico.
pub const INFO_CPU_TOPOLOGIA_DUDA: u64 = 0x4D;

/// Hilos por nucleo, **medidos** (`CPUID.0B.0:EBX`). `0` = no se pudo medir.
///
/// Existe porque `nucleos` se calculaba dividiendo entre un 2 escrito a mano, y
/// con eso la comprobacion `hilos == nucleos * 2` no podia fallar nunca.
pub const INFO_CPU_HILOS_POR_NUCLEO: u64 = 0x4E;

/// Tareas: ranuras ocupadas, listas para correr, y libres.
pub const INFO_TAREAS_TOTAL: u64 = 0x08;

pub const INFO_TAREAS_LISTAS: u64 = 0x09;

pub const INFO_TAREAS_LIBRES: u64 = 0x0A;

/// Ticks del temporizador desde el arranque.
pub const INFO_TICKS: u64 = 0x0B;

/// Bytes que ocupa el kernel en RAM, medidos (hasta el final de su `.bss`,
/// pila incluida). No es el medida del archivo.
pub const INFO_KERNEL_BYTES: u64 = 0x0C;

/// Programas que se han intentado admitir, y los que ya no caben en la
/// bitacora. La suma es el total de verdad.
pub const INFO_PROGRAMAS: u64 = 0x0D;

pub const INFO_PROGRAMAS_OLVIDADOS: u64 = 0x0E;

/// Hay disco listo? Esta montado el volumen de datos para escribir?
pub const INFO_DISCO_LISTO: u64 = 0x0F;

pub const INFO_DATOS_MONTADO: u64 = 0x10;

/// -- ESTRATOS ------------------------------------------------------
///
/// El volumen de datos grande. Ring 3 los necesita para poder MOSTRAR el estado
/// del almacen sin cruzar a Ring 0 por cada dato: son una fila mas de la tabla
/// de `OP_INFO`, que es como crece esta superficie sin tocar el ABI.
pub const INFO_ES_MONTADO: u64 = 0x11;

/// Generacion del superbloque: cuantas transacciones lleva el volumen.
/// **Cuando se hizo la version en curso**, en el formato de [`INFO_FECHA`].
///
/// ** `0` es **sin fechar**, no el anio cero. El campo `tiempo` del estrato
/// existia desde el primer dia y el kernel escribia un cero en el: la historia
/// del volumen no tenia fechas. Desde el 2026-08-19 lleva la de la placa, y
/// sigue siendo cero cuando la placa no da una hora creible -- una version
/// fechada en 1970 miente con mas conviccion que una sin fechar.
pub const INFO_ES_FECHA: u64 = 0x49;

pub const INFO_ES_GENERACION: u64 = 0x12;

pub const INFO_ES_BLOQUES: u64 = 0x13;

pub const INFO_ES_USADOS: u64 = 0x14;

pub const INFO_ES_BLOQUE_TAM: u64 = 0x15;

/// 0 holgado, 1 ambar, 2 rojo, 3 solo lectura. Ver `bmo_estratos::espacio`.
pub const INFO_ES_NIVEL: u64 = 0x16;

/// El gate del section 5: 1 si el volumen nacio en ESTE disco.
pub const INFO_ES_IDENTIDAD: u64 = 0x17;

/// 1 si hoy se puede escribir. Hoy siempre 0: falta cablear la E/S.
pub const INFO_ES_ESCRIBIBLE: u64 = 0x18;

/// **Bytes que Ring 3 ha PEDIDO** con `KIND_MEMORIA`, desde el arranque.
///
/// Es el unico dato de memoria que el kernel no puede deducir mirando lo que
/// cargo: la imagen y la pila de un proceso las puso el, pero un bloque pedido
/// solo existe porque alguien lo pidio. Y es la confirmacion **desde el otro
/// lado** de que la capability funciona -- el programa dice que le dieron
/// memoria; esto lo dice el kernel.
///
/// Contador que ya existia en `ring0::obj::memoria::total_handed_over()` y que
/// **no leia nadie**. Un contador que nadie consulta no es telemetria: es una
/// variable.
pub const INFO_MEM_ENTREGADA: u64 = 0x19;

/// * Quien tiene la pantalla: su `pid`, o **`0` si no la tiene nadie**.
///
/// Se PREGUNTA en vez de intentar reclamarla, y la diferencia importa: probar a
/// reclamarla para saber si esta libre **te la deja puesta**, y entonces se la
/// robas al programa al que se la ibas a prestar.
///
/// Estaba en el kernel y en el userland y **faltaba aqui**, que es justo la
/// deriva que ahora vigila `build.ps1`.
pub const INFO_PANTALLA_PROPIETARIO: u64 = 0x1A;

/// -- SMP ------------------------------------------------------------
///
/// Nucleos de aplicacion en pie, **sin contar el BSP**. Es lo que contesto el
/// bring-up, no lo que declara el CPU: la diferencia entre los dos es
/// exactamente el fallo que un panel tiene que poder mostrar.
pub const INFO_SMP_VIVOS: u64 = 0x1B;

/// * Choques de cerrojo, y la espera mas larga en vueltas de giro.
///
/// **Los dos tienen que ser CERO**, y por eso valen. Con un solo nucleo nadie
/// puede encontrar un cerrojo tomado, y los obreros de SMP solo computan: no
/// entran en el kernel, asi que no hay quien pelee. Un numero distinto de cero
/// no mide rendimiento -- dice que una de esas dos frases dejo de ser cierta.
pub const INFO_SPIN_CHOQUES: u64 = 0x1C;

pub const INFO_SPIN_PICO: u64 = 0x1D;

/// ** Recursos que una tarea muerta dejo SIN DEVOLVER, acumulados.
///
/// **Tiene que ser CERO**, y un numero distinto no acusa al programa que murio:
/// acusa al KERNEL, que dijo haberlo recuperado todo y no lo hizo.
///
/// Es la misma clase de numero que `INFO_SPIN_CHOQUES` y va al lado a
/// proposito: los dos son el sistema comprobandose a si mismo.
pub const INFO_FUGAS: u64 = 0x1E;

/// **La fecha y hora de la placa**, empaquetada en un solo numero:
/// `anio<<40 | mes<<32 | dia<<24 | hora<<16 | minuto<<8 | segundo`.
/// `0` = la maquina no sabe que dia es.
///
/// ** UN campo y no seis. La puerta contesta un numero por llamada, y seis
/// llamadas se pueden leer **a caballo de un cambio de minuto**: daria `10:59`
/// con los segundos del `11:00`. Empaquetada, la fecha es atomica por
/// construccion y no hace falta ningun cerrojo. Desempaquetarla es
/// `bmo_rtc::desempaquetar`.
pub const INFO_FECHA: u64 = 0x1F;

/// -- ** LA SALUD DEL BUS USB, COMO ESTADO ---------------------------------
///
/// La sexta exigencia de `docs/componente/EL_TECLADO_EXIGE.md`, y la regla que la ordena:
///
/// > **Una averia viva es un ESTADO, no un evento.** Un aviso se dice una vez e
/// > informa a quien ya estaba mirando; una averia que sigue ocurriendo
/// > necesita una luz encendida mientras dure, y donde vive el propietario.
///
/// Las cinco exigencias anteriores del teclado estan cumplidas y **cada una
/// tiene su contador** -- pero todos vivian en funciones de kernel que solo se
/// leen desde el shell de Ring 0, al que no se vuelve. Estas dos filas son lo
/// que convierte ese cuadro de mandos en algo que el escritorio puede pintar.
///
/// # `INFO_USB_SALUD`: los bits, mas la EDAD DEL LATIDO
///
/// ```text
///    bit 0   hay controlador xHCI            sin esto, lo demas es cero y no
///                                            significa "roto"
///    bit 1   teclado adoptado
///    bit 2   teclado con transferencia ENCOLADA   <- "enumero" != "escucha"
///    bit 3   su endpoint en Running SEGUN EL HARDWARE
///    bit 4   raton adoptado
///    bit 5   raton bombeando
///    bit 6   raton en Running
///    bit 7   USBSTS dice HSE o HCE: el controlador esta muerto
///   16..31   milisegundos desde el ultimo latido del hilo del bus
/// ```
///
/// ** La edad viaja PEGADA a los bits y no en otro campo, porque es lo que
/// permite no fiarse de ellos. Los bits son una foto que saca el bombeo; si el
/// hilo del bus muere, la foto se congela y seguiria contestando *"todo bien"*
/// para siempre. La edad **envejece sola**, asi que delata al que la escribe.
/// `0xFFFF` = hace mucho, o no hay reloj con el que saberlo: las dos piden la
/// misma reaccion, que es dejar de creerse el resto de la palabra.
///
/// El raton va al lado del teclado a proposito: **la asimetria entre los dos es
/// medio diagnostico**. Lo que le pasa a uno y no al otro no puede ser del hilo
/// del bus, ni del CR3 del MMIO, ni de la enumeracion -- solo puede ser algo
/// por endpoint.
pub const INFO_USB_SALUD: u64 = 0x3B;

pub const USB_SALUD_XHCI: u64 = 1 << 0;
pub const USB_SALUD_KBD: u64 = 1 << 1;
pub const USB_SALUD_KBD_BOMBA: u64 = 1 << 2;
pub const USB_SALUD_KBD_CORRE: u64 = 1 << 3;
pub const USB_SALUD_RATON: u64 = 1 << 4;
pub const USB_SALUD_RATON_BOMBA: u64 = 1 << 5;
pub const USB_SALUD_RATON_CORRE: u64 = 1 << 6;
pub const USB_SALUD_XHC_AVERIADO: u64 = 1 << 7;
/// Donde empieza la edad del latido, en milisegundos, y su mascara.
pub const USB_SALUD_EDAD_SHIFT: u64 = 16;
pub const USB_SALUD_EDAD_MASK: u64 = 0xFFFF;
/// *"Hace mucho, o no se puede saber"*. Ver arriba por que comparten valor.
pub const USB_SALUD_EDAD_VIEJA: u64 = 0xFFFF;

/// **Los cuatro contadores que tienen que ser CERO**, de 16 en 16 bits y
/// saturados -- el mismo empaquetado que [`INFO_CPU_EXT_AVERIAS`], y por el
/// mismo motivo: en campos separados se puede leer uno y no el otro, que es
/// como se dice *"todo bien"* habiendo mirado la mitad.
///
/// ```text
///    0..15   eventos PERDIDOS del aparcadero  E2  el endpoint se queda mudo
///   16..31   recuperaciones FALLIDAS          E3  se resucito y no salio
///   32..47   recuperaciones                   E3  hay errores de bus
///   48..63   barridos que REPARARON algo      E5  se pierden avisos de puerto
/// ```
///
/// Los dos primeros son averia; los dos ultimos son **desgaste**: el sistema se
/// repara solo y funciona, pero cada uno es medio segundo en que el teclado no
/// respondia. Quien pinta decide si eso es rojo o ambar; lo que no puede es
/// decir que no lo sabia.
///
/// Saturan a `0xFFFF` en vez de dar la vuelta: un contador que vuelve a cero
/// **apaga la luz**, que es justo el fallo que esta fila existe para no repetir.
pub const INFO_USB_AVERIAS: u64 = 0x3C;

// -- ** LO QUE EL DISCO CONTESTA (2026-08-17) -------------------------------
//
// Hasta hoy BMO-X le preguntaba al disco tres cosas --modelo, serie y
// capacidad-- y **no sabia si su disco giraba**. Mientras tanto el arbol si
// opinaba: el esquema de ESTRATOS razona sobre TRIM y la ley dice que un disco
// *"da caudal cuando tiene cola"*. Ninguna de las dos frases es falsa; ninguna
// estaba comprobada. Es L5 al reves -- hardcodea contratos, pregunta hechos.
//
// ** Los tres primeros campos son HECHOS y el cuarto es el VEREDICTO, y estan
// separados a proposito (L7): quien pinte puede mostrar lo que dijo el aparato
// aunque no este de acuerdo con lo que se concluyo de ello. Un veredicto sin su
// evidencia al lado no se puede discutir.
//
// Los numeros y su origen: `docs/componente/EL_DISCO_EXIGE.md`.

/// # `INFO_DISCO_MEDIO`: gira o no gira (palabra 217)
///
/// ```text
///    0..15   la palabra 217 CRUDA, tal como la dio el disco
///   16..17   0 no contesta - 1 NO ROTA - 2 ROTA - 3 valor reservado
///   32..47   revoluciones por minuto, 0 si no rota o no contesta
/// ```
///
/// ** **`no contesta` es un estado propio y no se colapsa a "es un HDD".** Los
/// SSD tempranos devolvian `0000h`, y por eso Windows 7 no se fio de esta
/// palabra sola: cruzaba su valor con una prueba real de lectura aleatoria. Es
/// R-FW2 --*lo que el firmware declara se comprueba contra lo que el aparato
/// hace*-- once anios antes de que esta casa la escribiera.
///
/// La palabra cruda viaja al lado del veredicto porque un rango reservado hay
/// que poder verlo, no deducirlo.
pub const INFO_DISCO_MEDIO: u64 = 0x3F;

pub const DISCO_MEDIO_CRUDO_MASK: u64 = 0xFFFF;
pub const DISCO_MEDIO_CLASE_SHIFT: u64 = 16;
pub const DISCO_MEDIO_CLASE_MASK: u64 = 0x3;
pub const DISCO_MEDIO_NO_CONTESTA: u64 = 0;
pub const DISCO_MEDIO_NO_ROTA: u64 = 1;
pub const DISCO_MEDIO_ROTA: u64 = 2;
pub const DISCO_MEDIO_RESERVADO: u64 = 3;
pub const DISCO_MEDIO_RPM_SHIFT: u64 = 32;
pub const DISCO_MEDIO_RPM_MASK: u64 = 0xFFFF;

/// # `INFO_DISCO_ENLACE`: el cable y la cola (palabras 75, 76 y 77)
///
/// ```text
///    0..2    generaciones SOPORTADAS: bit0 Gen1, bit1 Gen2, bit2 Gen3
///    4..6    generacion NEGOCIADA (1..3). 0 = el disco no lo dice
///    8       NCQ soportado
///   16..23   profundidad de cola, ** con el sesgo de -1 ya deshecho **
///   24..31   ranuras que BMO usa de verdad hoy
///   32..39   ranuras OCIOSAS: la resta de las dos de arriba
/// ```
///
/// ** **Soportado y negociado son dos campos porque son dos preguntas.** Un
/// disco Gen3 en un puerto Gen2 declara 3 y corre a 2; quedarse con la 76 da un
/// techo que no existe.
///
/// ** Y las ranuras usadas viajan aqui, junto a las que el disco admite, para
/// que **la resta se vea sin leer codigo**: hoy son 1 de 32.
pub const INFO_DISCO_ENLACE: u64 = 0x40;

pub const DISCO_ENLACE_GEN1: u64 = 1 << 0;
pub const DISCO_ENLACE_GEN2: u64 = 1 << 1;
pub const DISCO_ENLACE_GEN3: u64 = 1 << 2;
pub const DISCO_ENLACE_NEGOCIADA_SHIFT: u64 = 4;
pub const DISCO_ENLACE_NEGOCIADA_MASK: u64 = 0x7;
pub const DISCO_ENLACE_NCQ: u64 = 1 << 8;
pub const DISCO_ENLACE_COLA_SHIFT: u64 = 16;
pub const DISCO_ENLACE_COLA_MASK: u64 = 0xFF;
pub const DISCO_ENLACE_USADAS_SHIFT: u64 = 24;
pub const DISCO_ENLACE_USADAS_MASK: u64 = 0xFF;
pub const DISCO_ENLACE_OCIOSAS_SHIFT: u64 = 32;
pub const DISCO_ENLACE_OCIOSAS_MASK: u64 = 0xFF;

/// # `INFO_DISCO_GEOMETRIA`: el sector fisico y donde cae el LBA 0
///
/// ```text
///    0..3    ** EXPONENTE **: hay 2^n sectores logicos en uno fisico
///    4       la palabra 106 paso su guarda (bit15=0 y bit14=1)
///    8..21   desplazamiento del LBA 0 dentro del primer sector fisico
///   22       la palabra 209 paso su guarda
///   23       TRIM soportado (palabra 169 bit 0)
/// ```
///
/// ** **Los bits 0..3 son un exponente, no una cuenta**: un `3` son OCHO
/// sectores logicos por fisico. Es la misma familia de campo que el `bInterval`
/// del teclado, que se leyo como numero siendo exponente y dejo un teclado
/// sondeado cada 35 minutos (R-DISCO2).
///
/// ** El desplazamiento existe por la herencia de MS-DOS: la primera particion
/// empezaba en el **LBA 63**, que no es multiplo de 8, asi que sobre un disco de
/// 4096 B fisicos cada escritura de 4 KB caia a caballo de dos sectores. Le
/// importa a ESTRATOS porque su log crece en bloques de 4096: **desalineado,
/// cada avance paga dos sectores fisicos en vez de uno, en silencio.**
pub const INFO_DISCO_GEOMETRIA: u64 = 0x41;

pub const DISCO_GEO_EXP_MASK: u64 = 0xF;
pub const DISCO_GEO_106_VALIDA: u64 = 1 << 4;
pub const DISCO_GEO_DESPL_SHIFT: u64 = 8;
pub const DISCO_GEO_DESPL_MASK: u64 = 0x3FFF;
pub const DISCO_GEO_209_VALIDA: u64 = 1 << 22;
pub const DISCO_GEO_TRIM: u64 = 1 << 23;

/// # `INFO_DISCO_JUICIO`: el veredicto -- y es el unico campo que OPINA
///
/// ```text
///    0       hay PERFIL para este disco
///    1       medio solido CONFIRMADO (no basta con que el perfil lo diga)
///    2       ** la barrera FLUSH CACHE es lo unico que hay **
///    3       el recolector puede avisar al disco (TRIM)
///    4       el rendimiento del perfil esta MEDIDO, no es de catalogo
///    5       solido SIN TRIM  -- R-DISCO10
///    6       desalineado
///    7       el enlace negocio por debajo de lo que el disco sabe hacer
///    8..15   ranuras ociosas
///   16..47   frontera de escritura en KiB. ** 0 = no se puede alinear **
/// ```
///
/// Lo emite `bmo-disco-juicio`, que vive en `platform/shared/` y no en el
/// kernel **porque alli se puede probar** (L7b). En este componente equivocarse
/// no da un fault en pantalla: se lleva el trabajo de alguien.
///
/// ** **El bit 2 vale 1 tambien cuando NO hay perfil**, y ese es el esquema: no
/// saber si el disco tiene condensadores **no autoriza a suponer que los
/// tiene**. Un juez de rendimiento que se calla deja una cifra sin publicar; uno
/// de almacenamiento que se calla tiene que dejar el sistema en el camino que no
/// pierde datos.
///
/// ** Y la frontera contesta **0 sin perfil** en vez de un valor por defecto: el
/// bloque de borrado no lo expone ningun SSD de consumo (R-DISCO8), asi que sin
/// perfil no se alinea a nada -- y quien escriba tiene que saberlo en vez de
/// alinear a un numero inventado.
pub const INFO_DISCO_JUICIO: u64 = 0x42;

pub const DISCO_JUICIO_HAY_PERFIL: u64 = 1 << 0;
pub const DISCO_JUICIO_SOLIDO: u64 = 1 << 1;
pub const DISCO_JUICIO_SOLO_BARRERA: u64 = 1 << 2;
pub const DISCO_JUICIO_TRIM: u64 = 1 << 3;
pub const DISCO_JUICIO_MEDIDO: u64 = 1 << 4;
pub const DISCO_JUICIO_SOLIDO_SIN_TRIM: u64 = 1 << 5;
pub const DISCO_JUICIO_DESALINEADO: u64 = 1 << 6;
pub const DISCO_JUICIO_ENLACE_BAJO: u64 = 1 << 7;
pub const DISCO_JUICIO_OCIOSAS_SHIFT: u64 = 8;
pub const DISCO_JUICIO_OCIOSAS_MASK: u64 = 0xFF;
pub const DISCO_JUICIO_FRONTERA_SHIFT: u64 = 16;
pub const DISCO_JUICIO_FRONTERA_MASK: u64 = 0xFFFF_FFFF;

/// # `INFO_DISCO_TRIM_SECTORES`: cuanto se le ha devuelto al disco
///
/// Sectores de 512 B recortados desde el arranque. Cero significa **que nadie lo
/// ha pedido**, no que no se pueda: recortar en BMO-X lo pide una persona (la
/// seccion 9 de ESTRATOS: *politica, no automatismo*), asi que este numero es la
/// prueba de que la orden se dio y de cuanto cubrio.
pub const INFO_DISCO_TRIM_SECTORES: u64 = 0x43;

/// # `INFO_DISCO_TRIM_ORDENES`: en cuantos `DATA SET MANAGEMENT` cupo
///
/// ** Va al lado del anterior y no sobra: los mismos sectores en una orden o en
/// trescientas dicen cosas distintas del techo que declara el disco (palabra
/// 105). Es la unica pista si un dia recortar se vuelve lento, y sin ella
/// "cuanto" no tiene con que compararse.
pub const INFO_DISCO_TRIM_ORDENES: u64 = 0x44;

/// # `INFO_DISCO_COLA_LBA` / `..._SECTORES`: **el rango que se va a recortar**
///
/// El primer LBA de la cola libre del volumen ESTRATOS y cuantos sectores mide.
/// `0` = no hay volumen montado, o la cola esta vacia.
///
/// ** Existen para que **la propuesta y la orden lean el mismo numero**. Ring 3
/// podia sacarlos de `INFO_ES_BLOQUES`, `INFO_ES_USADOS` y `INFO_ES_BLOQUE_TAM`,
/// y esa cuenta paralela es exactamente la clase de cosa que esta casa persigue:
/// dos fuentes de una sola verdad se separan, y separarse aqui significa
/// **mostrar un rango y recortar otro**. Los dos lados llaman ahora a
/// `estratos::cola_libre()`.
///
/// [!] Y siguen siendo una FOTO: entre preguntar y mandar la orden, el volumen
/// puede haber crecido. No importa, y por eso se dice -- el kernel recalcula al
/// recortar y la ventana de escritura juzga lo que se manda, no lo que se
/// pinto.
pub const INFO_DISCO_COLA_LBA: u64 = 0x45;
pub const INFO_DISCO_COLA_SECTORES: u64 = 0x46;

/// # `INFO_DISCO_TRIM_BLOQUES`: cuanto cabe en UNA orden (palabra 105)
///
/// Bloques de payload de 512 B. Uno son 64 descriptores, o sea hasta ~2 GiB de
/// disco por orden.
///
/// ** Nunca contesta 0: ACS-3 garantiza que un bloque siempre se admite, y el
/// cero de la palabra 105 es **el disco callandose**, no un "no puedo". La
/// traduccion la hace `bmo-identify`, no quien pregunta.
pub const INFO_DISCO_TRIM_BLOQUES: u64 = 0x47;

/// # `INFO_DISCO_TRIM_FALLO`: **por que no se pudo recortar**
///
/// `(clase << 32) | PxTFD`, con las clases en `DISCO_FALLO_*`. `0` = ninguno,
/// y un recorte que sale bien lo devuelve a cero -- un campo que solo sube
/// contaria el fallo de ayer como si fuera el de hoy.
///
/// ** Existe porque en metal *"el disco respondio con error"* resulto no ser
/// un diagnostico: no separa un ABRT de un timeout, que mandan a mirar sitios
/// opuestos. El `PxTFD` va **crudo**, como el `PHYstatus` de la red: el byte
/// es la prueba y las palabras son la opinion.
pub const INFO_DISCO_TRIM_FALLO: u64 = 0x48;

// -- ** EL PERFIL DEL DISCO, COMPLETO: el otro extremo del cable y el metro --
//
// Paso P0 y D0 de exprimir el disco (2026-09-23). La LEY 24: una decision del
// disco cita su ATOMO, y una cifra de la caja es de OTRO proyecto. Los cuatro
// campos de arriba dicen lo que el DISCO contesta; estos dicen lo que la
// CONTROLADORA ofrece, que hace el disco con lo que se le escribe, y cuanto
// lee DE VERDAD en esta maquina.

/// # `INFO_DISCO_HBA`: la controladora y su puerto, CRUDOS
///
/// ```text
///    0..31   `CAP` del HBA, crudo
///   32..43   `PxSSTS` del puerto del disco, crudo (DET 3:0, SPD 7:4, IPM 11:8)
///   48..52   el puerto
///   63       hay foto
/// ```
///
/// ** Crudos a proposito: el registro es la prueba y las palabras la opinion.
/// Lo que se lee del `CAP` (estandar AHCI 1.3.1):
///
/// ```text
///    NP    4:0    puertos, -1          SNCQ  30   sabe encolar (NCQ)
///    NCS  12:8    ranuras, -1          S64A  31   DMA a toda la RAM
///    ISS  23:20   generacion maxima
/// ```
///
/// El `SPD` del puerto es la generacion que negocio **el HBA**; la palabra 77
/// (`INFO_DISCO_ENLACE`) es la que dice **el disco**. Son los dos extremos del
/// mismo cable, y que discrepen es un hallazgo, no un redondeo.
pub const INFO_DISCO_HBA: u64 = 0x92;

pub const DISCO_HBA_CAP_MASK: u64 = 0xFFFF_FFFF;
pub const DISCO_HBA_SSTS_SHIFT: u64 = 32;
pub const DISCO_HBA_SSTS_MASK: u64 = 0xFFF;
pub const DISCO_HBA_PUERTO_SHIFT: u64 = 48;
pub const DISCO_HBA_PUERTO_MASK: u64 = 0x1F;
pub const DISCO_HBA_HAY: u64 = 1 << 63;

/// # `INFO_DISCO_CACHE`: la cache de escritura (palabras 82-85)
///
/// ```text
///    0       la palabra 83 paso su guarda (sin ella, nada de abajo vale)
///    1       cache de escritura volatil SOPORTADA   (82 bit 5)
///    2       y ENCENDIDA ahora                      (85 bit 5)
///    3       FLUSH CACHE EXT                        (83 bit 13)
///    4       WRITE DMA FUA EXT                      (84 bit 6, con su guarda)
///   16..31   la palabra 82 cruda
///   32..47   la palabra 85 cruda
///   48..63   la palabra 83 cruda
/// ```
///
/// ** El bit 2 es el que decide la barrera (paso D5): con la cache APAGADA, un
/// WRITE que vuelve OK ya esta en la NAND. Encendida y sin condensadores, el
/// `FLUSH` es lo unico que separa aceptado de guardado.
pub const INFO_DISCO_CACHE: u64 = 0x93;

pub const DISCO_CACHE_VALIDA: u64 = 1 << 0;
pub const DISCO_CACHE_SOPORTADA: u64 = 1 << 1;
pub const DISCO_CACHE_ENCENDIDA: u64 = 1 << 2;
pub const DISCO_CACHE_FLUSH_EXT: u64 = 1 << 3;
pub const DISCO_CACHE_FUA: u64 = 1 << 4;
pub const DISCO_CACHE_W82_SHIFT: u64 = 16;
pub const DISCO_CACHE_W85_SHIFT: u64 = 32;
pub const DISCO_CACHE_W83_SHIFT: u64 = 48;

/// # `INFO_DISCO_BANDA`: la ultima LECTURA MEDIDA (`DISCO_OP_BANDA`)
///
/// ```text
///    0..31   microsegundos que tardo el disco (solo las ordenes)
///   32..47   MiB leidos
///   48..55   % de sectores que traian datos
///   56..63   cuantas veces se ha medido desde el arranque (satura en 255)
/// ```
///
/// `0` = nunca se midio. MB/s = `MiB * 1.048.576 / us`.
///
/// ** El % de datos no es decoracion: un SSD contesta un sector que nunca se
/// escribio **desde su mapa, sin leer la NAND**. Una banda medida sobre ceros
/// no es la del disco, y quien la pinte tiene que poder decirlo.
///
/// [!] Es LECTURA. La escritura sostenida del perfil (`sostenido_mb_s`) es otra
/// cifra: la de despues de agotar la cache SLC, y no se mide sin escribir
/// decenas de GB.
pub const INFO_DISCO_BANDA: u64 = 0x94;

pub const DISCO_BANDA_US_MASK: u64 = 0xFFFF_FFFF;
pub const DISCO_BANDA_MIB_SHIFT: u64 = 32;
pub const DISCO_BANDA_MIB_MASK: u64 = 0xFFFF;
pub const DISCO_BANDA_DATOS_SHIFT: u64 = 48;
pub const DISCO_BANDA_DATOS_MASK: u64 = 0xFF;
pub const DISCO_BANDA_VECES_SHIFT: u64 = 56;

/// # `INFO_DISCO_BANDA_ORDEN`: como se repartio esa medida
///
/// ```text
///    0..15   sectores por orden (8192 = una entrada de PRDT llena, 4 MiB)
///   16..39   microsegundos de la orden MAS RAPIDA
///   40..63   microsegundos de la MAS LENTA
/// ```
///
/// ** La distancia entre las dos es el dato: si la lenta es muchas veces la
/// rapida, el disco se paro a medio camino (recolector interno, mapa sin
/// DRAM), y eso no lo dice la media.
pub const INFO_DISCO_BANDA_ORDEN: u64 = 0x95;

pub const DISCO_BANDA_ORDEN_SECTORES_MASK: u64 = 0xFFFF;
pub const DISCO_BANDA_ORDEN_MEJOR_SHIFT: u64 = 16;
pub const DISCO_BANDA_ORDEN_PEOR_SHIFT: u64 = 40;
pub const DISCO_BANDA_ORDEN_US_MASK: u64 = 0xFF_FFFF;

/// # `INFO_GPU_*`: la grafica, PREGUNTADA en solo lectura (2026-09-23)
///
/// ```text
///   CHIP     0..31 BOOT_0 | 32..47 device PCI | 48..55 mascara de cabezas
///            56..58 la cabeza que pinta | 62 es Ampere | 63 hallada
///   MODO     0..15 px visibles | 16..31 lineas visibles | 32..47 htotal
///            48..62 vtotal | 63 valido
///   BORRADO  0..15 donde empieza el VBLANK | 16..31 donde acaba
///            32..62 reloj de pixel en kHz
///   TIEMPO   0..31 el cuadro MEDIDO en ns (dos vueltas de la linea)
///            32..47 vueltas | 48..62 cambios de linea | 63 medido
///   LINEA    0..15 la linea que barre AHORA | 16 en VBLANK | 63 valida
/// ```
///
/// ** `docs/maestro/GPU_NVIDIA_MAESTRO.md` (6b): el VBLANK de la RTX 3060 sin
/// el firmware del GSP. Antes de construirlo se pregunta si se puede, y esto
/// es la pregunta: si la linea da la vuelta, se puede.
pub const INFO_GPU_CHIP: u64 = 0x9A;
pub const INFO_GPU_MODO: u64 = 0x9B;
pub const INFO_GPU_BORRADO: u64 = 0x9C;
pub const INFO_GPU_TIEMPO: u64 = 0x9D;
pub const INFO_GPU_LINEA: u64 = 0x9E;

pub const GPU_BOOT0_MASK: u64 = 0xFFFF_FFFF;
pub const GPU_DEVICE_SHIFT: u64 = 32;
pub const GPU_CABEZAS_SHIFT: u64 = 48;
pub const GPU_CABEZA_SHIFT: u64 = 56;
pub const GPU_AMPERE: u64 = 1 << 62;
pub const GPU_HALLADA: u64 = 1 << 63;
pub const GPU_MODO_VALIDO: u64 = 1 << 63;
pub const GPU_TIEMPO_MEDIDO: u64 = 1 << 63;
pub const GPU_LINEA_VBLANK: u64 = 1 << 16;
pub const GPU_LINEA_VALIDA: u64 = 1 << 63;

/// # `INFO_GPU_ESPERA`: volcar DETRAS del rayo (E1 de `PLAN_LA_3060.md`)
///
/// ```text
///   PREGUNTA  8..19 y0 | 20..31 y1 (filas de lo visible, [y0, y1))
///             32..47 lo que tarda la copia POR FILA, en ns (lo mide quien vuelca)
///   RESPUESTA 0..31 ns que esperar (0 = ya) | 61 no cabe ni esperando
///             63 valida (0 = no hay rayo que mirar: se copia sin mas)
/// ```
///
/// ** El Ryzen dijo el 23-09 (15:22) que la linea que barre la 3060 se lee por
/// MMIO sin firmware. Sin page flip, lo unico que evita partir un cuadro es no
/// copiar donde el rayo esta leyendo: esto contesta cuanto esperar para eso.
pub const INFO_GPU_ESPERA: u64 = 0x9F;
pub const GPU_ESPERA_Y0_SHIFT: u64 = 8;
pub const GPU_ESPERA_Y1_SHIFT: u64 = 20;
pub const GPU_ESPERA_FILAS_MASK: u64 = 0xFFF;
pub const GPU_ESPERA_NS_FILA_SHIFT: u64 = 32;
pub const GPU_ESPERA_NS_FILA_MASK: u64 = 0xFFFF;
pub const GPU_ESPERA_NS_MASK: u64 = 0xFFFF_FFFF;
pub const GPU_ESPERA_NO_CABE: u64 = 1 << 61;
pub const GPU_ESPERA_VALIDA: u64 = 1 << 63;

/// # `INFO_SERIE`: el puerto serie, por COLA (2026-09-23)
///
/// ```text
///    0..31   bytes apuntados desde el arranque
///   32..47   bytes perdidos: la cola estaba llena
///   48..62   lo mas que llego a haber esperando
///   63       va por cola (0 = cada byte lo paga quien escribe)
/// ```
///
/// ** El `save` del 23-09 (06:57): `latido tarde 68 ms` del bus USB, y el CPU
/// lo tuvo EL PROPIO hilo del bus 59 ms -- girando sobre el UART, ~87 us por
/// byte, mientras escribia su log. Desde entonces escribir apunta y se va, y
/// esta fila dice que nadie espera al puerto.
pub const INFO_SERIE: u64 = 0x99;

pub const SERIE_APUNTADOS_MASK: u64 = 0xFFFF_FFFF;
pub const SERIE_PERDIDOS_SHIFT: u64 = 32;
pub const SERIE_PERDIDOS_MASK: u64 = 0xFFFF;
pub const SERIE_PICO_SHIFT: u64 = 48;
pub const SERIE_PICO_MASK: u64 = 0x7FFF;
pub const SERIE_COLA: u64 = 1 << 63;

/// # `INFO_IOMMU_*`: la IOMMU de AMD, PREGUNTADA en solo lectura (2026-09-23)
///
/// ```text
///   DONDE      0..35 base de sus registros en paginas de 4 KiB | 36..51 su BDF
///              52..59 tipo del IVHD elegido | 62 muda (todo unos, o fuera
///              del physmap) | 63 hallada
///   CONTROL    el registro 0x0018, crudo (bit 0 = TRADUCE)
///   ESTADO     el registro 0x2020, crudo
///   FUNCIONES  el registro 0x0030 (EFR), crudo
///   TABLA      el registro 0x0000, crudo: la tabla de dispositivos que haya
///   CENSO      0..15 mayor BDF del IVRS | 16..23 sueltos | 24..31 rangos
///              32..39 alias | 40..47 especiales | 48..55 IVMD
///              56..59 entradas raras (HID o sin nombre) | 60 cortado
///              61 hay entrada ALL | 63 valido
///   ESPECIAL   PREGUNTA 8..11 indice. RESPUESTA 0..15 BDF | 16..23 handle
///              24..31 tipo (1 IOAPIC, 2 HPET) | 32..39 banderas | 63 valida
///   IVMD       PREGUNTA 8..11 indice | 12..13 parte. RESPUESTA parte 0 el
///              inicio, 1 el largo, 2: 0..15 BDF | 16..31 aux | 32..39 tipo
///              40..47 banderas | 63 valida
/// ```
///
/// ** M0a de `docs/plan/PLAN_LA_3060.md`: el VBLANK de la 3060 por MSI pide su
/// Bus Master, y eso va detras de la IOMMU. Antes de encenderla se pregunta si
/// el firmware ya la dejo encendida, que sabe y a quien atiende.
pub const INFO_IOMMU_DONDE: u64 = 0xA0;
pub const INFO_IOMMU_CONTROL: u64 = 0xA1;
pub const INFO_IOMMU_ESTADO: u64 = 0xA2;
pub const INFO_IOMMU_FUNCIONES: u64 = 0xA3;
pub const INFO_IOMMU_TABLA: u64 = 0xA4;
pub const INFO_IOMMU_CENSO: u64 = 0xA5;
pub const INFO_IOMMU_ESPECIAL: u64 = 0xA6;
pub const INFO_IOMMU_IVMD: u64 = 0xA7;

pub const IOMMU_BASE_PAGINAS_MASK: u64 = 0xF_FFFF_FFFF;
pub const IOMMU_BDF_SHIFT: u64 = 36;
pub const IOMMU_TIPO_SHIFT: u64 = 52;
pub const IOMMU_MUDA: u64 = 1 << 62;
pub const IOMMU_HALLADA: u64 = 1 << 63;
pub const IOMMU_CENSO_UNOS_SHIFT: u64 = 16;
pub const IOMMU_CENSO_RANGOS_SHIFT: u64 = 24;
pub const IOMMU_CENSO_ALIAS_SHIFT: u64 = 32;
pub const IOMMU_CENSO_ESPECIALES_SHIFT: u64 = 40;
pub const IOMMU_CENSO_IVMD_SHIFT: u64 = 48;
pub const IOMMU_CENSO_RARAS_SHIFT: u64 = 56;
pub const IOMMU_CENSO_CORTADO: u64 = 1 << 60;
pub const IOMMU_CENSO_TODOS: u64 = 1 << 61;
pub const IOMMU_CENSO_VALIDO: u64 = 1 << 63;
pub const IOMMU_INDICE_SHIFT: u64 = 8;
pub const IOMMU_PARTE_SHIFT: u64 = 12;
pub const IOMMU_VALIDA: u64 = 1 << 63;

/// # `INFO_DISCO_AVISO`: la ESCALERA del aviso del disco (2026-09-23)
///
/// ```text
///    0..15   entradas al vector 49, del disco o no
///   16       el aparato tiene MSI ENABLE (leido de vuelta, no lo escrito)
///   17       su mascara por vector CALLA el mensaje
///   18       MSI-X encendido: el aparato ignora MSI
///   19       el IS del HBA tiene el bit de OTRO puerto: sin flanco, callado
///   20       GHC.IE: el HBA avisa
///   21       PxIE.DHRE: el puerto avisa al acabar una orden
///   22       IS del HBA con el bit del puerto: un aviso SIN CONSUMIR
///   23       PxIS con algo
///   24       el LAPIC tiene el vector PENDIENTE (IRR) y la CPU no lo cogio
///   25       la ranura 0 ocupada (una orden en el aparato)
///   26       la direccion del MSI es la que se escribio
///   27       el dato del MSI es el vector
///   28..35   el APIC al que se mando
///   36..43   el APIC de la CPU que pregunta
///   44..59   avisos de OTROS puertos limpiados desde el arranque
///   62       se armo
///   63       leido
/// ```
///
/// ** Nacio el 23-09 a las 06:57: `armada y NO LLEGA` son seis sitios donde se
/// puede perder un aviso, y desde fuera se ven igual. Cada bit es uno de esos
/// sitios preguntado a quien lo tiene.
pub const INFO_DISCO_AVISO: u64 = 0x98;

pub const DISCO_AVISO_ENTRADAS_MASK: u64 = 0xFFFF;
pub const DISCO_AVISO_MSI_ENABLE: u64 = 1 << 16;
pub const DISCO_AVISO_MSI_MASCARA: u64 = 1 << 17;
pub const DISCO_AVISO_MSIX: u64 = 1 << 18;
/// El `IS` del HBA tiene puesto el bit de OTRO puerto (23-09, 07:54).
pub const DISCO_AVISO_IS_AJENO: u64 = 1 << 19;
/// Avisos de otros puertos limpiados desde el arranque (16 bits).
pub const DISCO_AVISO_AJENOS_SHIFT: u64 = 44;
pub const DISCO_AVISO_GHC_IE: u64 = 1 << 20;
pub const DISCO_AVISO_PXIE: u64 = 1 << 21;
pub const DISCO_AVISO_IS_HBA: u64 = 1 << 22;
pub const DISCO_AVISO_PXIS: u64 = 1 << 23;
pub const DISCO_AVISO_IRR: u64 = 1 << 24;
pub const DISCO_AVISO_CI: u64 = 1 << 25;
pub const DISCO_AVISO_DIRECCION_OK: u64 = 1 << 26;
pub const DISCO_AVISO_DATO_OK: u64 = 1 << 27;
pub const DISCO_AVISO_DESTINO_SHIFT: u64 = 28;
pub const DISCO_AVISO_CPU_SHIFT: u64 = 36;
pub const DISCO_AVISO_ARMADA: u64 = 1 << 62;
pub const DISCO_AVISO_VALIDO: u64 = 1 << 63;

/// # `INFO_ENTERRADOR`: los muertos, desmontados FUERA del cerrojo (2026-09-23)
///
/// ```text
///    0..31   entierros hechos por el enterrador
///   32..62   el mas largo, en microsegundos (con las interrupciones ABIERTAS)
///   63       el enterrador existe; sin el, se entierra dentro del cerrojo
/// ```
///
/// ** Va al lado de `retenido` a proposito: el 23-09 a las 01:08 el cerrojo del
/// planificador cerro las interrupciones 231 us al morir DOOM, porque `reap`
/// desmontaba su espacio de direcciones DENTRO. Si el enterrador hace su
/// trabajo, `retenido` baja y este numero dice cuanto costaba de verdad
/// desmontar -- ahora sin quitarle el reloj a nadie.
pub const INFO_ENTERRADOR: u64 = 0x97;

pub const ENTERRADOR_ENTIERROS_MASK: u64 = 0xFFFF_FFFF;
pub const ENTERRADOR_PEOR_US_SHIFT: u64 = 32;
pub const ENTERRADOR_VIVO: u64 = 1 << 63;

/// # `INFO_DISCO_HILO`: el HILO DEL DISCO (paso D1, 2026-09-23)
///
/// ```text
///    0..23   ordenes que mando EN VUELO (soltando el disco para dormir)
///   24..39   de esas, las que termino OTRO que tomo el disco antes
///   40..61   veces que lo desperto la IRQ del disco
///   62       la IRQ del disco quedo ARMADA (MSI programado y el HBA avisa)
///   63       el hilo existe
/// ```
///
/// ** El bit 62 llego el 23-09 a las 01:08: el Ryzen dijo `8 ordenes en vuelo,
/// 0 despertares por la IRQ`, y eso tiene dos lecturas que mandan a mirar sitios
/// distintos -- no se armo (la placa no anuncia MSI, o el vector no se instalo)
/// o se armo y no llega. Sin el bit, cualquiera de las dos era una suposicion.
///
/// ** Es la prueba de que D1 hace lo que dice: si `vuelos` sube y `irq` sube
/// con el, el CPU estuvo libre mientras el aparato trabajaba. Si `irq` se queda
/// en cero, la placa no enruta el aviso y el hilo vive de su red de 2 ms. Y si
/// `ajenas` se acerca a `vuelos`, el hilo llega tarde a sus propias ordenes.
pub const INFO_DISCO_HILO: u64 = 0x96;

pub const DISCO_HILO_VUELOS_MASK: u64 = 0xFF_FFFF;
pub const DISCO_HILO_AJENAS_SHIFT: u64 = 24;
pub const DISCO_HILO_AJENAS_MASK: u64 = 0xFFFF;
pub const DISCO_HILO_IRQ_SHIFT: u64 = 40;
pub const DISCO_HILO_IRQ_MASK: u64 = 0x3F_FFFF;
pub const DISCO_HILO_VIVO: u64 = 1 << 63;
pub const DISCO_HILO_IRQ_ARMADA: u64 = 1 << 62;

/// Fabricante ("AMD"), nombre comercial, microarquitectura y familia/modelo.
pub const INFO_TXT_CPU_VENDOR: u64 = 0x01;

pub const INFO_TXT_CPU_NOMBRE: u64 = 0x02;

pub const INFO_TXT_UARCH: u64 = 0x03;

pub const INFO_TXT_FAMILIA: u64 = 0x04;

/// El nombre de la extension `i` del censo: se pide como
/// `INFO_TXT_EXT_NOMBRE | (i << 8)`.
///
/// ** El indice viaja en los bits altos del campo, que es el idioma que esta
/// superficie ya habla (`INFO_MEM_QUIEN_*`, `AUTOPSIA_TEXTO`). Con esto los
/// treinta y seis nombres viven **en un solo sitio del arbol** --el `match`
/// exhaustivo del kernel, que el compilador obliga a completar al agregar una
/// fila-- en vez de en una copia de Ring 3 que envejece en silencio.
pub const INFO_TXT_EXT_NOMBRE: u64 = 0x05;

/// El motivo escrito a mano de esa misma fila: por que se usa, o por que no.
/// Misma forma de indexar. Es la columna que convierte el censo en una
/// decision en vez de trivia -- y la que el kernel cuenta como `muda` si esta
/// vacia.
pub const INFO_TXT_EXT_NOTA: u64 = 0x06;

/// QUE ES lo de la ficha `n >> 8` del portero, en corto (`bmo_usbred`):
/// "red RNDIS", "movil MTP", "HID"... Vacio si no hay tal ficha.
pub const INFO_TXT_USB_QUE_ES: u64 = 0x07;
/// Que se hizo con la ficha `n >> 8`, con las palabras de CABINA.
pub const INFO_TXT_USB_MOTIVO: u64 = 0x08;
/// El nombre del programa `n >> 8` del registro ("c/cubo.bex"), y su etiqueta
/// ("C", "INTI", "asm"). Vacio si no hay tal programa. Ver [`INFO_PROG_QUIEN`].
pub const INFO_TXT_PROG_NOMBRE: u64 = 0x09;
pub const INFO_TXT_PROG_TAG: u64 = 0x0A;
/// El nombre del cerrojo de [`INFO_SPIN_RETENIDO`]. `-` si ninguno se solto.
pub const INFO_TXT_CERROJO_PEOR: u64 = 0x0B;
/// El nombre del hilo de [`INFO_COMPAS`] (n en los bits altos).
pub const INFO_TXT_COMPAS_NOMBRE: u64 = 0x0C;
/// El fichero (recortado desde `ring0/`) donde se tomo el cerrojo de
/// [`INFO_SPIN_RETENIDO`].
pub const INFO_TXT_CERROJO_SITIO: u64 = 0x0D;
/// El nombre del trabajo de la vuelta del bus USB que MAS tardo desde el
/// arranque (`bombeo`, `rescate`, `emergencia`, `purga`, `radar`, ...). Los
/// microsegundos van en [`INFO_USB_RITMO`] `[16..48)`.
pub const INFO_TXT_USB_TRABAJO: u64 = 0x0E;

/// Campos de [`TASK_OP_KLOG_INFO`].
pub const KLOG_DISPONIBLES: u64 = 0x00;

pub const KLOG_TOTAL: u64 = 0x01;

/// Campos de [`TASK_OP_AUTOPSIA_INFO`].
///
/// `AUTOPSIA_TOTAL` es el que se mira en bucle: **si cambio, hay un fallo
/// nuevo**, y eso se sabe sin leer un solo renglon.
pub const AUTOPSIA_TOTAL: u64 = 0x00;

pub const AUTOPSIA_DISPONIBLES: u64 = 0x01;

pub const AUTOPSIA_RENGLONES: u64 = 0x02;

/// Campos de [`TASK_OP_CABINA_INFO`].
pub const CABINA_TOTAL: u64 = 0x00;

pub const CABINA_PERDIDOS: u64 = 0x01;

pub const CABINA_DISPONIBLES: u64 = 0x02;

pub const CABINA_SEVERIDAD: u64 = 0x03;

pub const CABINA_CAPA: u64 = 0x04;

pub const CABINA_VALOR: u64 = 0x05;

pub const CABINA_SEQ: u64 = 0x06;

pub const CABINA_TICK: u64 = 0x07;

/// **De que INTENTO salio el evento.** `0` = de ninguno.
///
/// === Por que este campo cambia lo que CABINA puede hacer ===
///
/// Los otros siete dicen **que paso**. Este dice **a que accion pertenece**, y
/// esa es otra pregunta: un lanzamiento emite eventos desde cuatro modulos
/// --`lanzar`, `proc`, `bex`, `disk`-- y hasta ahora, para saber cuales eran de
/// TU pulsacion, habia que juntarlos de memoria mirando el `#N` impreso.
///
/// El kernel ya los agrupa (`cabina::intento`) y ya pinta el numero en su
/// panel. Lo que faltaba era **entregarselo a Ring 3**, que es donde esta la
/// ventana con filtros. Sin este campo, el filtro de la caja solo podia ser por
/// gravedad: "ensename los FALLO" -- que trae los de esta accion y los de las
/// diez anteriores mezclados.
///
/// Con el, la pregunta pasa a ser la util: **"ensename TODO lo que hizo esto que
/// acabo de pulsar"**.
pub const CABINA_INTENTO: u64 = 0x08;

/// Que texto pide [`TASK_OP_CABINA_TEXTO`].

/// **EL BARRIDO: cuantos hubo de cada clase, y NUNCA se pierde ninguno.**
///
/// `n` empaqueta `(capa << 8) | severidad`. Ocho capas por cinco severidades.
///
/// # Por que esto existe, y por que un filtro no lo sustituye
///
/// El anillo son 48 eventos y gira. Un filtro --*"ensename los fallos"*-- solo
/// puede mirar lo que sobrevivio, asi que **un FAULT del arranque contesta
/// "ninguno" cuando ya se cayo**. Y esa respuesta es indistinguible de estar
/// bien, que es lo mas caro que puede decir un sistema de vigilancia.
///
/// *** El barrido se incrementa en `record` **antes del cerrojo del anillo**, asi
/// que cuenta tambien lo que el anillo va a perder -- por giro y por reentrancia.
///
/// > Lo que se pierde del anillo no se pierde de la cuenta.
pub const CABINA_BARRIDO_CUENTA: u64 = 0x10;

/// El `seq` del **ultimo** evento de esa clase. `0` = no hubo ninguno.
///
/// Con [`CABINA_VENTANA`] contesta la pregunta que importa cuando algo va mal:
/// **todavia se puede leer, o solo queda la cuenta?**
pub const CABINA_BARRIDO_ULTIMO: u64 = 0x11;

/// **El `seq` mas bajo que sigue dentro del anillo.** Todo lo anterior existio y
/// ya no se puede leer.
pub const CABINA_VENTANA: u64 = 0x12;

/// Cuantas clases tienen **todo** fuera del anillo. `0` = no se ha escapado nada.
///
/// Es el barrido resumido en un numero: el que se mira primero.
pub const CABINA_CLASES_FUERA: u64 = 0x13;

/// **Cuantos hubo de esta clase en el ULTIMO SEGUNDO cerrado.**
///
/// # Por que hace falta si ya esta la cuenta total
///
/// Porque un total no puede decir *"esta pasando AHORA"*:
///
/// ```text
///    cuenta = 400 FAULT de `vmm`   puede ser
///                                    - una tanda de hace media hora, resuelta
///                                    - cuatrocientos por segundo, ahora mismo
/// ```
///
/// *** Y esas dos cosas piden lo contrario: la primera es forense y la segunda
/// es una emergencia. **Un numero que no distingue una emergencia de un
/// recuerdo solo sirve para contarla luego.**
///
/// El indice es el mismo que `CABINA_BARRIDO_CUENTA`: `(capa << 8) | severidad`.
pub const CABINA_BARRIDO_RITMO: u64 = 0x14;

/// Cuantas ventanas de un segundo se han cerrado desde el arranque.
///
/// [!] **Sin esto, un ritmo de cero es ambiguo**: puede ser *"no paso nada en el
/// ultimo segundo"* o *"todavia no ha pasado un segundo entero"*. La primera es
/// tranquilizadora y la segunda no dice nada, y un panel que las pinte igual
/// esta mintiendo la mitad de las veces.
pub const CABINA_VENTANAS: u64 = 0x15;

pub const CABINA_TXT_MODULO: u64 = 0x00;

pub const CABINA_TXT_MENSAJE: u64 = 0x01;
