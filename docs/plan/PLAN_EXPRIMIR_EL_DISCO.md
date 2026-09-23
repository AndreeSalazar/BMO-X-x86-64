# PLAN: EXPRIMIR EL DISCO -- por el PERFIL, no por la caja

> Escrito el 2026-09-23. Mismo formato que `PLAN_ALMACENAMIENTO.md`: casillas
> ordenadas, cada una con **que la bloquea** y **como se sabe que quedo hecha**.

El propietario: *"exprimir TODA la velocidad del disco... a BASE de PERFIL, que
pregunta por completo a nivel de atomos que es y aplica por eso: ese es el
motivo PARA EXPRIMIR CON TODO"*.

La primera respuesta a esa frase empezo por *"el fabricante dice 500 MB/s"*, y
eso es justo lo que prohibe la LEY 24: **una estimacion generica es la
estimacion de OTRO proyecto**. Este plan empieza preguntandole al aparato que
es, y cada paso cita el atomo con el que decide.

---

## 0. Lo que el hardware YA contesto (y donde estaba)

| Atomo | Registro / palabra | Valor en el Ryzen | Dice |
|---|---|---|---|
| NCS | `CAP` 12:8 | 31 | **32 ranuras** de comando en el HBA |
| SNCQ | `CAP` 30 | 1 | la controladora **sabe encolar** |
| ISS | `CAP` 23:20 | 3 | hasta **Gen3**, 6 Gb/s = 600 MB/s de datos (8b/10b) |
| S64A | `CAP` 31 | 1 | su DMA alcanza toda la RAM |
| NP | `CAP` 4:0 | 7 | 8 puertos |
| cola, NCQ | IDENTIFY 75, 76 | (en `disco`) | lo que el DISCO admite |
| enlace | IDENTIFY 76, 77 | (en `disco`) | lo que el DISCO negocio |
| sector fisico | IDENTIFY 106, 209 | (en `disco`) | la frontera de alineado |

`cap = 0xEF36FF27` vivia **en un comentario** de
`platform/drivers/storage/ahci/src/arranque.rs`. Y el contraste ya decia el
hallazgo sin suponer nada: **la controladora ofrece 32 ordenes a la vez y el
driver usa una** (`RANURAS_EN_USO = 1`, la ranura 0, esperando girando).

---

## 1. LAS CASILLAS

### [x] P0 -- COMPLETAR LA PREGUNTA (HECHO: `60769ccf8`, visto en el Ryzen el 2026-09-23 00:05)

Lo que faltaba preguntar, y ya se pregunta:

- el `CAP` del HBA como **dato**, no como comentario: `AhciController::cap`
  (`platform/drivers/storage/ahci/src/controller.rs`);
- el `PxSSTS` del puerto del disco: `bmo_ahci::port_ssts`. Su `SPD` es la
  generacion que negocio **el HBA**; la palabra 77 es la que dice **el disco**.
  Son los dos extremos del mismo cable, y el informe marca si discrepan;
- la **cache de escritura** (palabras 82-85): `padre::Cache` en
  `platform/drivers/storage/identify/src/padre.rs`, con la guarda de la 83 y
  cuatro pruebas (sin guarda no se afirma nada: un `FFFFh` diria "si" a todo).

Salen por `INFO_DISCO_HBA` (0x92) e `INFO_DISCO_CACHE` (0x93), crudos y
descifrados, y los pinta la tabla de `disco` y `informe/DISCO.TXT` del `save`.

**Como se sabe:** el `save` del Ryzen trae en DISCO.TXT las filas `hba`
(`32 ranuras, NCQ, hasta Gen3, DMA 64 bits   CAP 0xEF36FF27`), `port` y `cache`.

**Lo que contesto el metal** (`save` del 23-09, 00:05):

```text
    hba       32 ranuras, NCQ, hasta Gen3, DMA 64 bits   CAP 0xEF36FF27
    port      2: enlace Gen3   activo   SSTS 0x133
    queue     1 de 32   31 PARADAS
    sector    512 B fisico
    cache     ENCENDIDA (85): un OK es 'aceptado', no 'guardado', FLUSH EXT, FUA
```

Los dos extremos del cable dicen Gen3 (el HBA por `SPD`, el disco por la 77).
La cache esta ENCENDIDA y el perfil dice sin condensadores: D5 queda con su
dato, y la decision sigue siendo del propietario. Y el disco sabe **FUA**: una
escritura que no vuelve hasta estar en la NAND, que es la alternativa fina a
un FLUSH de todo.

### [ ] D0 -- EL METRO (codigo HECHO 2026-09-23; se cierra con el primer numero)

`disco banda [MiB]` (`Ultra_kernel_x86-64/kernel/src/ring0/dev/disk/banda.rs`,
`DISCO_OP_BANDA` 0x03): lee N MiB (64 si no se dice, techo 1024) de la
particion de datos por el camino de verdad del driver, cronometra **solo las
ordenes**, y deja en `INFO_DISCO_BANDA` / `_ORDEN` los MB/s, la orden mas rapida
y la mas lenta, y **cuantos sectores traian datos**.

Tres decisiones, con su motivo:

1. **Solo lee.** La cifra del perfil (`sostenido_mb_s`) es la ESCRITURA despues
   de agotar la cache SLC: decenas de GB escritos, desgaste y un sitio donde
   escribirlos. Eso lo decide el propietario, no un metro que se lanza con una
   orden. Por eso esta medida **no** convierte `Cifra::catalogo(450)` en
   medida: es otra cifra.
2. **El % de sectores con datos va al lado.** Un SSD contesta un sector que
   nunca se escribio desde su mapa, sin leer la NAND. Una banda sobre ceros no
   es la del disco, y el informe lo dice en rojo por debajo del 90 %.
3. **La orden mas lenta contra la mas rapida.** La media esconde a un disco
   que se para a medio camino (sin DRAM, el mapa del FTL); la distancia no.

**Como se sabe:** `disco banda` en el Ryzen da un numero, `cable` dice que
parte del techo Gen3 usa, y ese numero se pega aqui abajo como el ANTES de D1.

### [ ] D1 -- ASINCRONO + WAIT: cero congelones

Emitir, volver, y que la IRQ despierte a quien espera. No depende de un atomo:
es lo que quita la sordera. Las piezas existen (`bmo_ahci::emitir`,
`bmo_ahci::sondear`, `dev/disk/irq.rs`, `CLAVE_ESPERA`, y el stub del vector 49
ya sabe devolver OTRO contexto). **Bloquea:** D0 medido (sin el ANTES no se sabe
si D1 gano o solo cambio de sitio el tiempo).

#### *** Lo que se encontro al ir a escribirlo (2026-09-23)

**Un syscall corre ENTERO con las interrupciones cerradas**: `MSR_SFMASK` apaga
`IF` en la entrada y solo `sysretq` la devuelve (`syscall/entry.rs`; lo repiten
`dev/uaudio.rs` y `obj/audio.rs`). Y casi toda la E/S del disco la pide un
syscall (abrir un `.bex`, FAT32, ESTRATOS, el `save`). Tres consecuencias:

1. **"Que quien lee DUERMA hasta la IRQ" no se puede hacer dentro del
   syscall.** No hay donde dormir: el cambio de tarea se consuma en el epilogo
   del trap y lo que se restaura es el estado de Ring 3. Abrir `IF` a mitad de
   un syscall para hacer `hlt` meteria expropiacion en un codigo que da por
   hecho que no la hay (`current_tid_en_trap` sin cerrojo, `gs:[0x10]` que un
   trap anidado pisaria).
2. **La IRQ del disco no puede llegar durante un syscall.** `AVISOS` no se
   mueve ahi dentro; lo que termina cada orden es la red de seguridad de
   `run_command_hasta` (preguntar por MMIO cada 4.096 vueltas). El "escuchar
   es barato" solo vale fuera de un syscall.
3. **Dormir de verdad pide sacar la E/S del syscall**: un HILO DEL DISCO de
   kernel (como el del bus USB, con `IF` abierta, que si puede aparcar con
   `hlt` y al que la IRQ despierta en el acto), y el syscall se convierte en
   PEDIR + `WAIT`. O sea: el D1 "sin tocar el ABI" no existe en esta casa;
   el D1 que duerme es el D1 con ABI.

#### Lo que se escribio (2026-09-23, decision del propietario: hilo + PEDIR/WAIT)

```text
   dev/disk/hilo.rs     el HILO DEL DISCO: prioridad 1 (sobre las apps, bajo el
                        bus USB). Da vueltas a un PASO que le da el arranque
                        (`dev` no nombra a `obj`: L8) y duerme sobre su llave
   dev/disk/vuelo.rs    UNA orden en el aparato sin que nadie tenga el disco.
                        Quien toma el disco la TERMINA el (cosechar): nadie
                        espera nunca a que el hilo corra, y por eso un syscall
                        con IF=0 no se abraza con el
   dev/disk/irq.rs      el aviso despierta al hilo, y el vector 49 cambia de
   plat/irq.rs          tarea EN EL ACTO (`despertar_desde_irq`)
   fat32/src/plan.rs    PLANEAR sin leer: los clusters seguidos, una orden
                        (hasta 4 MiB); cinco pruebas con el disco de mentira
   obj/cargando.rs      el PASO: planea, manda directo al bufer del fichero,
                        y al acabar sube la secuencia y despierta al lector
   syscall/mod.rs       WAIT sobre un KIND_ARCHIVO: el brazo DEROGADO vuelve
   userland/archivo.rs  `esperar_entero` duerme con WAIT (red de 100 ms)
```

**Lo que cambia sin tocar una app**: `Archivo::leer_de` --el que usan todas
para leer un fichero-- ya iba por la carga a trozos. Ahora el lector DUERME y
el CPU es de otro mientras el aparato trabaja.

**Lo que NO cambia, dicho**: quien llama a `leer_de` sigue esperando SU
fichero. El escritorio que abre una imagen de 8 MiB sigue sin pintar esos ms;
lo que se gana es que el resto del sistema si corre. Para que el escritorio
pinte mientras carga, tiene que usar `leer_de_asinc` y mirar entre fotograma
y fotograma -- eso es de la app, no del kernel.

**Como se sabe en el metal** (fila `thread` de `disco` y del `save`):

| Mirar | Lo bueno | Lo que diria que no |
|---|---|---|
| `thread` | `N ordenes en vuelo` sube al abrir una imagen | `sin hilo` |
| despertares por la IRQ | sube con las ordenes | `SIN IRQ`: vive de la red de 2 ms |
| terminadas por otro | cerca de 0 | cerca de las ordenes: el hilo llega tarde |
| abrir DOOM y el visor | igual que antes o mas rapido | cuelgue o fichero corto (CABINA: `archivo corto`) |

#### *** Lo que dijo el metal (23-09, 06:57), y la segunda vuelta

```text
    retenido  134 us   `disco` en dev\disk\hilo.rs:145
    thread    8 ordenes en vuelo   0 despertares por la IRQ   0 terminadas por otro
              SIN IRQ: armada y NO LLEGA: vive de la red de 2 ms
```

El hilo FUNCIONA (DOOM y los ficheros llegan) y tenia dos defectos, los dos
de pieza y no de numero:

1. **El paso del hilo giraba sobre el disco.** Planear un tramo es seguir la
   cadena de la FAT, y cuando el sector no estaba en la cache de UNO del
   volumen, `planear_tramo` lo leia EN SINCRONO -- dentro del paso, con el
   cerrojo `disco` y las interrupciones cerradas. 134 us es una lectura de
   512 B de este SATA. **La pieza nueva**: el plan es PURO
   (`bmo_fat32::Plan`: `Tramo` / `Falta(sector)` / `Nada`, sobre una
   `VentanaFat` que otro trajo), y el hilo trae la FAT **como trae los
   datos**: una orden en vuelo a una pagina propia (`fsys/fs.rs`, 8 sectores =
   1.024 entradas), con el contador de escrituras del volumen para no servirla
   vieja. El camino sincrono recorre la cadena con el MISMO codigo. Y el
   "este trozo por el camino de siempre" que habia dentro del paso (otra
   lectura sincrona escondida) se quito: si un tramo no cabe, se para y se
   dice.
2. **El aviso no tenia un solo propietario.** Lo borraba solo el manejador de la
   IRQ; quien acababa una orden PREGUNTANDO (`sondear`, en un syscall) lo
   dejaba puesto, y con MSI de un mensaje el HBA avisa en el FLANCO: con
   `IS.IPS` a 1 no hay flanco y no vuelve a mandar. Ahora lo consume el
   primero que ve la orden acabada (`bmo_ahci::consumir_aviso`). Con el MSI
   programado preguntando (el APIC por CPUID, no un `0` escrito; la mascara
   por vector a cero; MSI-X apagado si el firmware lo dejo encendido).

Y como "no llega" son seis sitios que se ven igual, la fila nueva `irq`
(`INFO_DISCO_AVISO`, 0x98) dice **el primero que falla**: el aparato (su
MSI leido de vuelta), el HBA (`GHC.IE`, `PxIE`, `IS`), el LAPIC (su IRR) y el
vector (sus entradas).

| Mirar | Lo bueno | Lo que diria que no |
|---|---|---|
| `retenido` | ya no es `disco` en `hilo.rs` | `disco` en `hilo.rs` otra vez: queda algo sincrono en el paso |
| `irq` | `LLEGA: el vector 49 despierta al hilo` | la frase dice el peldano que falla |
| `thread` | los despertares suben con las ordenes | `SIN IRQ` |

**07:54, el metal otra vez, y el VECINO.** El save dijo `thread 9 ordenes en
vuelo, 0 despertares por la IRQ` y la escalera `el vector 49 ENTRA, pero el
aviso no es del puerto`. Los bits crudos (`DATOS.TXT`, `disco_aviso =
0xC00000000C310001`): MSI encendido y sin mascara, direccion y vector bien,
GHC.IE y PxIE puestos, nada en IRR, **una sola entrada** en nueve ordenes.

Lo que encaja con todo: este HBA tiene **ocho puertos** (`CAP` NP=7) y el driver
solo apagaba y limpiaba el suyo. Con MSI de un mensaje el HBA avisa en el FLANCO
de su `IS` entero: un bit de OTRO puerto puesto lo deja a 1 y el disco no vuelve
a avisar. La escalera no lo podia ver: miraba solo el bit del puerto 2, y el
peldano `entradas > 0` tapaba al de "sin consumir".

En codigo, sin metal: `habilitar_irq` apaga PxIE y limpia PxIS de los otros
puertos implementados (`PI`); `consumir_aviso` limpia tambien los bits ajenos
del `IS` y los cuenta (`bmo_ahci::AJENOS`); la escalera gana el bit 19
`IS_AJENO` (antes que `entradas`) y la cuenta en 44..59 (`ajenos N` en la fila).

| que | afirma | como se cae |
|---|---|---|
| `thread` tras cargar DOOM | `despertares por la IRQ` SUBE | sigue en 0 con `ajenos 0`: el vecino no era, y hay que seguir bajando |
| la fila `irq` | `LLEGA` y `ajenos` > 0 | `ajenos` > 0 y sigue sin llegar: habia vecino, pero no era el unico |
| `DMA mas mudo` | deja de ser el disco con ~4 ms | sigue: la red de 2 ms sigue siendo la que termina |

### [ ] D3 -- DMA directo al bloque prestado + PRD multiples

> **Cuestionar antes (23-09):** hoy ninguna orden pasa de 1 MiB (`TROZO_HILO`),
> asi que el techo de 4 MiB no lo toca nadie. Se decide con D0: si la orden de
> 1 MiB ya llena el cable, esta casilla se cierra con el numero y sin codigo.

Una orden de hoy pide como mucho 4 MiB (una entrada de PRDT, `DBC` de 22 bits);
el protocolo admite 32 MiB (`READ DMA EXT`, 65.536 sectores). **Como se sabe:**
`disco banda` con la orden de 8 MiB contra la de 4 MiB.

### [ ] D2 -- NCQ: la cola que decide el orquestador

> **Cuestionar antes (23-09):** NCQ gana cuando hay VARIAS ordenes a la vez
> (lecturas sueltas, varios lectores). Hoy el disco tiene UN cliente (el hilo)
> que lee trozos seguidos de 1 MiB: una cola de 32 no tiene con que llenarse.
> Cuesta el camino FPDMA entero (`SActive`, 32 ranuras, el FIS SDB) para una
> ganancia que ninguna maquina de hoy ejecuta. Se mide primero (D0) y se
> decide con el numero; el propietario decide si se abre.

`cola = min(NCS del HBA, profundidad del disco, la parte que el orquestador le
da a quien tiene el foco)`. Tres respuestas, y el kernel decide con las tres:
eso es orquestar. **Bloquea:** D1. Y el TRIM **encolado** es el que tiene
historial de corrupcion (`NO_NCQ_TRIM` de Linux): el TRIM se queda fuera de la
cola hasta que el metal diga otra cosa.

### [x] D4 -- Tramos alineados al sector fisico (palabra 106) -- CERRADA SIN CODIGO

Si el fisico es de 4 KiB, todo tramo que no empiece y acabe en frontera hace
que el disco reescriba de mas. **Como se sabe:** `sector` en la tabla de `disco`
dice el fisico; si es 512, esta casilla se cierra sin codigo.

El metal (23-09, 00:05): `sector  512 B fisico`. Un sector fisico es uno
logico: no hay frontera que respetar en ESTE disco. Lo que si sigue importando
es el bloque de BORRADO (2 MiB, `Deducido` en el perfil), que no se pregunta.

### [ ] D5 -- FLUSH solo donde hace falta (palabra 85)

Con la cache **apagada** un WRITE que vuelve OK ya esta en la NAND y la barrera
no compra nada; **encendida** y sin condensadores (el perfil dice `false`), la
barrera es lo unico. Que se pierde si se va la luz **lo decide el propietario**,
no este plan. **Bloquea:** P0 visto en metal (la fila `cache`).

---

## 2. LA REGLA DE ESTE PLAN

Cada decision del disco cita su atomo. Si no hay atomo, se pregunta ANTES de
decidir -- y si el aparato no lo contesta (el bloque de borrado, el TBW), va al
perfil de `platform/shared/bmo-disco-juicio/src/perfil.rs` con su `Origen`.
