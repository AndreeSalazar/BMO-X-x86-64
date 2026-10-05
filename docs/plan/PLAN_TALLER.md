# PLAN DEL TALLER -- F1 de BMO-X

> ** **Se llamaba ESTRUCTURA** (y este fichero, PLAN_ESTRUCTURA) hasta el
> 29-09. El propietario: *"reemplazar el nombre ESTRUCTURA a taller = y ya"*. La
> app, su fichero (`sys/taller.bex`), su carpeta (`Ultra_userspace/apps/taller`)
> y este plan se llaman igual desde entonces; lo que se cuenta con fecha
> anterior conserva el nombre que tenia.
>
> **Lo que afirma**: que se pulsa `F1` en el escritorio, se abre una ventana con
> historial, se teclea `compilar hola.ada`, y aparece un `.bex` en el disco de
> BMO-X. Sin instalar nada, porque no hay nada que instalar.
>
> **Como se cae**: la ventana no abre, o abre y no recibe teclas, o compila y el
> `.bex` que escribe no lo admite el cargador.
>
> Escrito el **2026-09-06**, cuando el propietario pidio *"un terminal que se convierta
> compilador, que llames como F1 en Escritorio, pero que sea APP"*.

---

## 0. EL REPARTO DEL NOMBRE, PARA QUE NO SE CONFUNDA NUNCA

```text
   VALKYRIE-ABI   JUZGA      no ejecuta jamas. No tiene anillo
   TALLER         FABRICA    un `.bex` de Ring 3, y apunta a V-ABI
```

** No son dos capas de lo mismo: son el estandar y una herramienta que lo
cumple. `gcc` apunta a POSIX y no es POSIX. Si algun dia TALLER se
llamara VALKYRIE, la frase de `VALKYRIE-ABI/README.md` --*"no tiene anillo
porque no tiene ni una instruccion en la maquina de destino"*-- seria falsa el
mismo dia.

---

## 1. LO QUE YA ESTA, Y ES MAS DE LO QUE PARECE

`F1` **esta libre**. En `Ultra_userspace/services/director/src/desktop/keys/app.rs`
la constante `SC_F1 = 0x3B` existe solo como frontera del rango reservado
(`SC_F1..=SC_F10`), y **nada la usa**. Las teclas de funcion las retiene el
escritorio y no bajan a la app de delante, asi que atar `F1` es una linea en el
sitio donde ya se deciden `F11` y `F12`.

Y REX ya trae las seis piezas que un terminal necesita:

```text
   superficie.h   172   dibujar en TU memoria y ofrecerla al DIRECTOR
   entrada.h      372   teclado y raton por buzon
   scroll.h       140   una ventana que se mueve sobre un historial
   archivo.h       65   leer el fuente, escribir el .bex
   paquete.h      261   las tablas que viajan DENTRO del propio .bex
   monton.h       109   malloc sobre un bloque del kernel
```

★ `scroll.h` merece decirse aparte: es **scrollback de terminal**, escrito como
funciones puras sin heap ni buffer escondido. Se prueba entero sin encender la
maquina.

---

## 2. ** LA FRONTERA DECIDE QUE COMANDOS EXISTEN, Y NO ES NEGOCIABLE

El terminal de hoy **no es una app**: son 5.761 lineas en
`Ultra_userspace/services/director/src/commands/` mas 978 de
`scene/consola.rs`, todo dentro del DIRECTOR. Y cuando se mira que hacen:

```text
   reports.rs  1.199    system.rs  908    disco.rs  420    red.rs  309
```

son en su mayoria **lectores de instrumentos del kernel**. Y
`VALKYRIE-ABI/FRONTERA.txt` deja fuera de la superficie de app, por prefijo:

```text
   DISCO_  ES_  CABINA_  USB_  AUTOPSIA_  KLOG_  SYSCALL_  MAQ_
```

### ** CORREGIDO EL 06-09: la frontera es MAS ESTRECHA de lo que decia aqui

La primera version de esta seccion decia que ESTRUCTURA *"no puede correr `cpu`,
`mem`, `disco` ni `red`"*. **Es falso**, y lo desmiente la propia fila de la
frontera:

> `DISCO_` -- *los paneles. Un programa lee el disco por `BMO_INFO_DISCO_*`,
> **que SI esta***

Lo que la frontera deja fuera son las **operaciones de panel**, no las lecturas.
Y las lecturas ya estan publicadas: `bmo/verde.h` de REX trae **49 constantes
`BMO_INFO_*`**, entre ellas 12 de `CPU_`, 10 de `DISCO_` y 8 de `NET_`, todas
por `OP_INFO`.

```text
   TALLER SI PUEDE    cpu  mem  disco  red        (las LECTURAS, por OP_INFO)
   TALLER NO PUEDE    cabina  klog  autopsia      (las ORDENES de panel)
                          usb  syscall  maq  es
```

** Y lo que queda fuera tiene un motivo que no es tecnico: *"cada fila de la
puerta de los terceros es una promesa que hay que mantener despues"*. Publicar
`KLOG_` convierte el formato del log del kernel en un contrato con terceros, y
ese formato cambia cada vez que se depura algo.

### La consecuencia sigue siendo la misma: son DOS terminales, no uno

```text
   la consola del DIRECTOR   el panel de INSTRUMENTOS   cpu, mem, red, cabina
   el TALLER (F1)            el taller                  compilar, leer, escribir
```

Y eso **no es una limitacion que rodear**: es el primer caso en el que la
frontera se cobra, y contesta sola una pregunta de esquema que si no habria que
discutir. La consola de instrumentos se queda donde esta porque **es un
instrumento**; el taller sale porque es una app.

---

## 3. ⚠ EL CELO OBLIGA A QUE SEA UN SOLO FICHERO

`EJECUTAR` pide autoridad, se fija al nacer y solo desde Ring 0
(`Ultra_kernel_x86-64/kernel/src/ring0/task/autoridad.rs`). Un `.bex` **no puede
lanzar otro**.

Eso descarta el esquema obvio --un terminal que invoca al compilador-- y deja el
correcto:

```text
   taller.bex       el terminal Y el compilador, en el MISMO fichero,
                    con sus tablas en la seccion 0x0B
   el ESCRITORIO    lanza lo que TALLER compilo, cuando el propietario hace clic
```

[!] Se llamaba `estructura.bex` en este plan, y ese nombre NO PUEDE EXISTIR en
el disco de BMO-X: el FAT32 busca por nombre 8.3 (8 letras). Se descubrio el
29-09 en el Ryzen --F1 dio "un nombre no cabe en 8.3" en CABINA-- y paso a
`sys/taller.bex`. Ese mismo dia la app tambien paso a llamarse TALLER (ver
la cabecera).

** Y eso convierte "sin instalar" en algo literal en vez de en un eslogan: **un
fichero que trae dentro lo que necesita**, y que se lee con `paquete.h` sin
copiar nada. La cabecera ya cita al propietario diciendo la idea:

> *"es un bef pero ese bex es el mismo que abre la caja: no lo duplica, lo lee y
> punto."*

---

## 4. ★★ TALLER Y EL AUTOHOSPEDAJE SON EL MISMO TRABAJO

El frontend de Ada es **Rust**. Si TALLER tiene que contenerlo, TALLER es
un `.bex` de Rust -- y la pregunta es que tiene ya la cara de Rust.

### ** CORREGIDO EL 06-09: SE MIDIO CONTRA LA CRATE EQUIVOCADA

La primera version de esta seccion comparaba REX contra **`bmo-rt`** y concluia
que *"la cara de Rust tiene cero"*. Era falso, y el error fue de medida: `bmo-rt`
(`toolchain/lang/base/`, 1.371 lineas) es **el arranque y el monton** --crt0,
syscall, heap, string, fmt, ffi--, o sea el equivalente de la `crt0` y poco mas.

**La cara de Rust de la superficie es `bmo-userland` v2.0.0**, cuya propia
descripcion lo dice: *"Runtime de Ring 3: los dos syscalls, capabilities y la
pantalla"*. Son **3.901 lineas** y es lo que enlaza el DIRECTOR.

```text
   bmo-rt          1.371   crt0, syscall, heap, string, fmt, ffi   EL ARRANQUE
   bmo-userland    3.901   archivo, pantalla, entrada, memoria,    LA SUPERFICIE
                           disco, red, sonido, proceso, estratos
```

### El hueco de verdad, medido contra la crate correcta

| lo que necesita TALLER | REX (C) | `bmo-userland` (Rust) |
|---|---|---|
| `archivo` -- leer el fuente, escribir el `.bex` | si | **ya estaba** (344 lineas) |
| `pantalla` -- dibujar | si | **ya estaba** (623) |
| `entrada` -- teclas y raton | si | **ya estaba** (139) |
| `monton` | si | **ya estaba** (`memoria`, 143) |
| `paquete` -- las tablas dentro del fichero | si | **HECHO el 06-09** (escalon 2) |
| `scroll` -- el historial | si | falta como modulo reutilizable |

** Asi que el escalon 2 no eran cinco modulos: era **UNO**, y el unico que
quedaba de verdad. Y su cimiento ya estaba puesto sin que nadie lo dijera --
`Archivo::saltar` lleva escrito desde antes *"hacia falta para leer un PAQUETE:
la seccion de recursos vive al final"*.

Lo que queda es `scroll`, y no bloquea nada hasta el escalon 4: `scene/
historial.rs` (172 lineas) ya lo hace dentro del DIRECTOR y de ahi sale la forma.

---

## 4b. ** CUANTAS PUERTAS CUESTA UNA COMPILACION -- contadas, no estimadas

El propietario pidio el numero. Sale de **leer el codigo y contar**, no de estimar:
cada llamada esta escrita en `Ultra_userspace/userland/src/archivo.rs` y se
puede marcar con el dedo.

```text
   fuente        ->  salida          HOY   CON BLOQUE   factor
   ------------------------------------------------------------
   hola_C.c      ->  holac.bex       656         11        59x
   blit_C.c      ->  blit.bex       2144         11       194x
   leer_C.c      ->  leer.bex       3490         11       317x
   coste_C.c     ->  coste.bex     10657         11       968x
```

### El desglose de `hola_C.c`, que es de donde sale todo

```text
   abrir fuente (ruta 2 + abrir 1)           3
   esperar (cabe en la ventana de 64 KiB)    1
   leer      1839 / 7                      263
   COMPILAR                                  0     <-- todo en memoria
   crear salida (ruta 2 + crear 1)           3
   escribir  2695 / 7                      385
   cerrar                                    1
                                          ----
                                            656
```

** Y la fila que manda es la de **CERO**. Compilar no cuesta ni una puerta: el
parser, el codegen y el emisor trabajan en memoria del proceso. **Todo el coste
de puerta de una compilacion es mover bytes**, y por eso el camino de bloque lo
cambia todo y afinar el compilador no cambiaria nada.

### Por que el numero nuevo es 11 y no crece

```text
   abrir 3 + esperar 1 + pedir el bloque 1 + leer_en 1
   + crear 3 + escribir_de 1 + cerrar 1  =  11
```

**No depende del medida.** `ARCH_OP_LEER_EN` no pasa por la ventana de 64 KiB
--el rango va del disco al bloque, sin escala-- asi que un fuente de 30 KiB
cuesta las mismas once puertas que uno de 1 KiB.

### [!] LO QUE ESTA CUENTA NO ES

**No es una medicion en metal.** Es una cuenta del codigo, y tiene dos huecos
declarados:

1. **El bucle de `esperar_entero`** vale 1 vuelta solo porque los ficheros de
   ejemplo caben en la ventana de 64 KiB (`obj/file.rs`, `WINDOW`). Un fuente
   mayor gira mas veces, y cuantas depende del disco.
2. **La ruta** se cuenta como 2 paquetes de ocho bytes. Una ruta larga cuesta
   mas, y es la parte que menos importa.

El kernel ya sabe contar puertas --`INFO_SYSCALL_CUENTA`-- asi que confirmarlo
es restar el contador antes y despues. Eso pide **un arranque**, y entra en la
misma deuda que todo lo demas de esta semana.

---

## 5. LOS ESCALONES

Ordenados por la regla de la casa: **lo que no toca nada va primero.**

```text
   [x] 1  F1 abre una ventana VACIA   HECHO y visto en el metal el 06-09 (una
                                      ventana del DIRECTOR). Desde el 29-09 F1
                                      ya no la abre: LANZA `sys/taller.bex`
                                      (seccion 8.5, B1), y esa ventana se retiro

   [x] 2  PAQUETE en Rust            HECHO 06-09 --
                                      `Ultra_userspace/userland/src/paquete.rs`
                                      y `Archivo::mi_imagen`. Era el UNICO que
                                      faltaba de los cinco: los otros cuatro ya
                                      estaban en `bmo-userland` (ver seccion 4)

   [ ] 2b la ventana con REJILLA      `scroll` como modulo reutilizable, de la
                                      forma que ya tiene `scene/historial.rs`

   [ ] 3  taller.bex DIBUJA           una ventana con su rejilla y su cursor,
                                      sin leer una tecla. Se compara contra
                                      `scene/consola.rs`, que ya lo hace

   [ ] 4  y LEE TECLAS                por el buzon de `entrada`, con el
                                      historial de `scroll`. Ya es un terminal,
                                      y todavia no compila nada

   [ ] 5  los comandos que la         `ls`, `cat`, `escribir`. Y los que la
          FRONTERA permite            frontera deja fuera SE DICEN, con el
                                      motivo: "eso es un instrumento, esta en
                                      la consola del DIRECTOR"

   [ ] 6  `compilar hola.ada`         el frontend de Ada dentro del mismo
                                      .bex, con sus tablas en la seccion 0x0B.
                                      Es el escalon 6 de PLAN_AUTOHOSPEDAJE
                                      visto desde aqui

   [ ] 7  exportar donde le digan     a ESTRATOS o a FAT32, y que la ventana
                                      de datos lo muestre
```

### Como se cae cada uno

| escalon | si esta bien | si falla |
|---|---|---|
| 1 | aparece un rectangulo al pulsar F1 | la tecla no llega: se la come el rango reservado |
| 2 | `cargo test` de `bmo-rt` cubre los cinco | un modulo compila y no hace lo que su gemelo en C |
| 3 | la rejilla se ve igual que la del DIRECTOR | el DIRECTOR no compone su superficie |
| 4 | se teclea y sale, y la rueda sube | el buzon se llena y se pierden teclas |
| 5 | `cpu` contesta **por que** no esta | contesta "no existe", que es una respuesta peor |
| 6 | sale un `.bex` que `bmo-verify` admite | se queda sin monton, o el `.bex` sale distinto |
| 7 | el fichero aparece en la ventana de datos | -- |

★ El escalon 5 no es de relleno. *"Ese comando es un instrumento y vive en la
consola del DIRECTOR"* es una respuesta que muestra el sistema; *"comando
desconocido"* deja al que lo teclea creyendo que falta trabajo.

---

## 6. LO QUE **NO** ENTRA, Y SE DICE PARA QUE NO CREZCA SOLO

* ~~**Un editor.** TALLER compila lo que hay en el disco. Editar es otra app
  y otro plan.~~ **DEROGADO el 29-09 por el propietario**: F1 sera un editor de
  NODOS para TITAN++ (seccion 8). Lo que sobrevive de la regla vieja es el
  ORDEN: el editor va DESPUES del escalon 7, no antes.
* **Los comandos de instrumentos.** Seccion 2. Si algun dia uno hace falta de
  verdad, se borra su prefijo de `VALKYRIE-ABI/FRONTERA.txt` **y se escribe por
  que** -- que es el momento en que la decision se toma a la vista.
* **Lanzar lo que compila.** Seccion 3. Eso es del escritorio, y es el celo.
* **Los otros tres lenguajes.** Ada primero por lo que mide
  `PLAN_AUTOHOSPEDAJE` seccion 1; C, COBOL e INTI arrastran `toml`.

---

## 7. LO QUE SERIA UN ERROR

* **Sacar los 5.761 de `commands/` del DIRECTOR para "reaprovecharlos".** La
  mayoria no puede cruzar la frontera, asi que lo que se moveria es codigo que
  despues hay que devolver.
* **Darle autoridad a TALLER** para que lance lo que compila. Es el tercer
  bit, y `autoridad.rs` ya dejo escrita la pregunta que va antes.
* **Llamarlo VALKYRIE.** Seccion 0.
* **Empezar por el escalon 6** porque es el que se ve. Sin el 2, no hay ventana
  que lo muestre.

---

## 8. ** EL TALLER COMO GRAFO DE NODOS (29-09)

> El propietario, con una captura de **Ultra-Omega** -- un editor de nodos suyo,
> en Rust y Vulkan, en Windows: *"ESTE ES EL F1 que voy a implementar en mi
> ESTRUCTURA el F1 asi tienen que ir para facilitar en TITAN++ con todo y
> organizar"*. El lenguaje: [`docs/maestro/TITAN_MAESTRO.md`](../maestro/TITAN_MAESTRO.md).

### 8.1 Lo que muestra la captura

```text
   arriba        File  Edit  View  Run
   izquierda     EXPLORER: el espacio de trabajo y su carpeta nodes/
   el lienzo     nodos con cabecera de color y el lenguaje (RUST), un titulo
                 ("Hola Mundo", "Rust Node 2"), un puerto `in` y uno `out`, y
                 dentro el `source` del nodo
   abajo         F5 Run, Del Delete, Tab Templates, O Open,
                 Ctrl+Shift+P Commands; cuantos nodos y enlaces; el zoom
```

### 8.2 Por que encaja: un programa TITAN++ YA ES un grafo

`TITAN_MAESTRO` U3: cada modulo dice que hace y con que conecta (`use`), y el
compilador exige que las dependencias solo bajen. **Los modulos son los nodos y
los `use` son las aristas.** El editor no inventa una estructura: dibuja la que
el compilador ya comprueba.

```text
   en Ultra-Omega                 en F1 con TITAN++
   un nodo con cabecera RUST      un MODULO; la cabecera dice su lenguaje
                                  (TITAN, INTI, C...)
   el titulo del nodo             la linea que dice que hace el modulo
   `source` dentro del nodo       el texto del .titan
   out / in                       out = sus `use`; in = quien lo usa
   EXPLORER / nodes/              src/ y el Titan.toml
   F5 Run                         `titan build`, y pedir al ESCRITORIO lanzar
   Tab Templates                  `titan new` con plantillas
   Ctrl+Shift+P Commands          la consola del taller (los escalones 4-5)
```

### 8.3 Las reglas, para que el grafo no se vuelva una jaula

1. **La verdad es el TEXTO.** Los `.titan` y el `Titan.toml` son lo que se
   compila, se versiona y se lee sin F1. El grafo es una VISTA: las posiciones
   de los nodos van aparte (donde, lo decide el propietario: `TITAN_MAESTRO`
   13.5). Motivo: ABC murio por obligar a vivir en su entorno
   (`INTI_MAESTRO` seccion 2), y `titan build` tiene que funcionar sin F1.
2. **Una arista es una DEPENDENCIA, no un flujo de ejecucion.** Los editores
   de nodos donde el cable es "lo que pasa despues" (los de programacion
   visual) se vuelven un plato de espaguetis en cuanto el programa crece. Aqui
   el cable es un `use`, y dibujar uno HACIA ARRIBA (un ciclo) el editor lo
   rechaza con la misma regla y el mismo mensaje que el compilador.
3. **F1 es un `.bex` de Ring 3.** Dibuja con REX y VERRANO, no con Vulkan:
   Ultra-Omega en Windows es el PROTOTIPO del esquema, no el binario. Y la
   frontera de la seccion 2 y el celo de la seccion 3 siguen igual: F5 no
   lanza nada, se lo pide al ESCRITORIO.
4. **Un grafo de FLUJO si tiene sitio, pero aparte y despues:** donde el
   cable ES el dato -- la tuberia de la 3060, el sonido de LA MESA. Queda
   anotado; no entra en estos escalones.

### 8.4 Los escalones del editor (detras del 7)

```text
   [ ] 8  el lienzo: los modulos de un paquete como nodos, leidos de src/ y de
          sus `use`, sin editar nada. Pide: el escalon 3 (dibujar) y el
          frontend de TITAN++ hasta T2 (sabe leer `mod` y `use`)
   [ ] 9  el texto del nodo se EDITA y se guarda en su .titan; `titan check`
          marca el nodo que no compila
   [x] 10 dibujar un cable escribe un `use`; uno hacia arriba se rechaza
          HECHO en el anfitrion el 04-10 (8.13): del pin OUT a otro nodo
   [ ] 11 F5: `titan build`, y el .bex se le ofrece al ESCRITORIO
```

| escalon | si esta bien | si falla |
|---|---|---|
| 8 | los nodos y cables son los mismos que dice `titan check` | el grafo miente: dibuja un `use` que no esta |
| 9 | el .titan en disco cambia y compila | se edita el nodo y el fichero no cambia |
| 10 | el cable y el `use` aparecen y desaparecen juntos | un ciclo se dibuja sin error |
| 11 | el .bex aparece y el ESCRITORIO lo lanza con un clic | F1 intenta lanzarlo y el celo dice NO |

### 8.5 La BASE, antes de los escalones 8-11 (29-09): B0-B3

El propietario eligio empezar por aqui (*"la base con borrow checker todo y
nodos grafos"*). El comprobador de verdad espera a la gramatica de TITAN++ (T0),
asi que la base construye todo lo que el comprobador ALIMENTA, con un paquete
de ejemplo escrito a mano (`asteroids`):

```text
   [x] B0  titan-contrato      platform/shared/titan-contrato: el grafo (todo
                               es un nodo: el Titan.toml, los modulos, la
                               3060, el DIRECTOR), los eventos del comprobador
                               y el mensaje de 4 partes. Sin unsafe, sin
                               monton. 20 pruebas en el anfitrion, verdes
   [x] B1  sys/taller.bex      Ultra_userspace/apps/taller: F1 LANZA la
                               app (keys/windows.rs); la ventana interna del
                               DIRECTOR se retiro entera. VISTO EN EL RYZEN el
                               29-09 13:57 (tras el renombre a taller.bex)
   [~] B2  el lienzo           los 8 nodos y sus cables (curvas de bmo-dibujo)
                               VISTOS en el Ryzen el 29-09 13:57. Arrastrar,
                               + / - y 0: sin confirmar en el metal
   [x] B3  la animacion        los 11 eventos del ejemplo: el pulso del `mut`
                               que va y vuelve, el `take` que se mueve y deja
                               el origen en gris, el cable a la 3060 encendido
                               hasta que vuelve, el CHOQUE en rojo con su
                               mensaje de 4 partes, y el NO del permiso `net`
                               en el nodo principal. VISTA en el Ryzen el 29-09
                               13:57: evento 5/11, `buf` hacia la 3060, las
                               fichas `world mut`, `bullet entregado/suyo`
```

Lo que la captura del Ryzen dejo ver, y queda por hacer:

- **Maximizar no agranda el lienzo.** El marco crece y la superficie se queda en
  1280x760 con un hueco vacio: la app ignora `SUP_EV_CONFIGURE`.
- **El titulo dice `tid 7`.** El DIRECTOR no sabe el nombre de una app (lo sabe
  quien la lanzo, no quien la compone): es de `scene/surface.rs`, no de aqui.
- **Los cables entre nodos de la MISMA fila** (`physics -> ship`) salen por
  abajo y entran por arriba, y cruzan por detras de `rock`. Visto primero en la
  vista previa en HTML y confirmado en el metal.

**Como se mira en el Ryzen** (despues de `build.ps1` y desplegar):

| se hace | si esta bien | si falla |
|---|---|---|
| F1 en el escritorio | sale una ventana de 1280x760 con 8 nodos y sus cables, y la animacion empieza sola | nada: falta `sys/taller.bex` en el disco, o `run` dice por que |
| esperar ~10 s | el pulso ambar va de `main` a `ship`, vuelve; `bullet` pasa a `rock`; el cable a la 3060 se enciende en verde y se apaga; `ship` y `physics` parpadean en rojo con QUE/DONDE/POR QUE/COMO abajo; `net: no` destella | la animacion se para a medias: mirar la consola (`TALLER:`) |
| arrastrar un nodo | el nodo sigue al raton y sus cables con el | el nodo no se mueve: el estado del puntero del buzon no llega (+8/+12) |
| arrastrar el fondo, `+`, `-`, `0` | el lienzo se mueve, acerca, aleja, encuadra | las letras no llegan al buzon |
| espacio, `n`, `r` | pausa, un paso, repite | igual |
| minimizar la ventana | la app deja de pintar (R-APP8) y al volver repinta | se queda en negro al volver |
| Esc | la app se cierra y lo dice en la consola | no se cierra: Esc no llega como letra |

[!] Lo que la base NO es: el ejemplo esta escrito a mano. Los escalones 8-11
son los mismos dibujos alimentados por `titan check` de un paquete `.titan` de
verdad -- y eso espera a T0 (la gramatica, del propietario).

### 8.6 L1: la BIBLIOTECA en ESTRATOS, en tiempo real (29-09)

El propietario: *"en F1 se sincroniza en tiempo real con biblioteca en ESTRATOS
para organizar carpetas, nombres"*, con el Explorer de Windows como inspiracion
y no como plantilla. Eligio las tres cosas que siguen.

**Lo que se decidio:**

- **Se sigue `mod`, como cargo.** El grafo NO es un listado de carpetas: es lo
  que el codigo declara. Un `.titan` que nadie nombra no es un nodo, y un `mod`
  sin fichero es un PROBLEMA dicho, no un hueco.
- **La cabecera es la del boceto.** La primera linea de cada `.titan` es
  `mod nombre "que hace"`; debajo, `use a, b` (cables hacia abajo) y `mod a, b`
  (los hijos). El resto del fichero es del lenguaje y el lector no lo mira.
- **Un indice, `titan/biblioteca.toml`** (`[packages] nombre = "ruta"`), y
  ningun cambio en Ring 0.

```text
   titan/biblioteca.toml          [packages] asteroids = "titan/asteroids"
   titan/asteroids/Titan.toml     [package] name, [permissions], [layout]
   titan/asteroids/src/main.titan mod main "..."  / use director / mod ship, rock, physics
                   src/physics.titan              use ship, gpu / mod collide
                   src/physics/collide.titan      (los hijos de un modulo, en su carpeta)
```

**Las piezas:**

```text
   [x] titan-lector    platform/shared/titan-lector: el TOML minimo, la
                       cabecera, el indice, y `read_package`, que anda en
                       anchura desde src/main.titan. No conoce el kernel: lee
                       por un `Source`. Dice 12 clases de problema en
                       castellano (falta el mod, nombre que no casa, `use` de
                       nadie, permiso no pedido -- el U2 --, ciclo, ...).
                       24 pruebas; la semilla se DEMUESTRA igual al ejemplo
                       escrito a mano
   [x] store.rs        la biblioteca en ESTRATOS. La primera vez la SIEMBRA
                       (carpetas, ficheros, el indice el ULTIMO: una siembra
                       cortada se termina la vez siguiente). Sin ESTRATOS
                       montado, el ejemplo en memoria, y lo dice
   [x] explorer.rs     la columna izquierda: EXPLORER, de donde viene y en que
                       generacion, BIBLIOTECA (clic: abre otro paquete),
                       ARCHIVOS como arbol (clic: ese nodo se enciende y el
                       lienzo lo centra) y PROBLEMAS. Un clic en un nodo del
                       lienzo enciende su fichero: van en las dos direcciones
   [x] el guion        `sample::script_for(grafo)` busca los nodos por NOMBRE:
                       si al paquete le falta uno de los que nombra el guion,
                       no hay animacion -- nunca un prestamo hacia un nodo que
                       no esta
```

**El tiempo real es un numero.** En ESTRATOS escribir ES commitear, y cada
commit sube `INFO_ES_GENERACION`. F1 lo pregunta en cada latido (una llamada) y
solo relee los ficheros si se movio. Un `renombra` en F12, un `vuelve`, otra
app que guarda: F1 lo muestra en el latido siguiente (100 ms quieto, 16 moviendose).

**Lo que NO hace, a proposito:** no recorre ESTRATOS con el cursor del kernel.
Ese cursor es UNO y es del panel F12; moverlo desde aqui cambiaria lo que F12
muestra. Por eso sigue `mod` (rutas conocidas, `Archivo::leer_de`) en vez de
listar carpetas -- y es tambien lo que el propietario eligio.

**Lo medido -- CORREGIDO el mismo 29-09.** Aqui ponia que el camino mas hondo
de `taller.bex` eran 42.032 B. **Era falso**: lo saco un script propio que
sumaba los `subq ..., %rsp` y no veia los marcos grandes, que se reservan en
BUCLE (la sonda de pila). `pila.py --ring3`, que mide como el kernel, dijo
**79.152 B contra 64 KiB**: el L1 tal como se commiteo (`6399200cc`) se habria
caido al abrir con ESTRATOS montado. Nadie lo vio porque el guardian solo
media cuatro programas (`director`, `proton-x`, `coste`, `sombra`).

Arreglado en 8.7: `taller` entra en la lista del guardian (el build para si no
cabe), `read_package_into` llena el `Loaded` EN SU SITIO (~8 KiB que se
copiaban al volver) y los `use` van en una tabla plana (13 KiB de filas casi
vacias -> 1,7 KiB). Queda en **54.448 B de 65.536** (`_start` 28.368,
`read_all` 18.752). [!] La leccion es la de siempre: la medida de un guardian
vale lo que vale su lista.

**Como se mira en el Ryzen** (despues de `build.ps1` y desplegar, con ESTRATOS
montado):

| se hace | si esta bien | si falla |
|---|---|---|
| F1 la primera vez | la consola dice `TALLER: sembre titan/asteroids en ESTRATOS`; a la izquierda `ESTRATOS gen N`, `> asteroids` y 6 ficheros en arbol | `no se pudo sembrar`: la consola dice que ruta |
| clic en `collide.titan` | el nodo `collide` se enmarca en cian y el lienzo lo centra | nada: el clic no llega a x < 250 |
| clic en el nodo `ship` | `ship.titan` se enciende en la columna | igual, al reves |
| F12: `renombra titan/asteroids/src/rock.titan roca.titan` | sin tocar F1, `rock` desaparece, sale en PROBLEMAS `main declara mod rock y su fichero no esta`, la animacion se va y el panel dice por que; la generacion sube | F1 no cambia: el latido no pregunta la generacion |
| F12: `vuelve 1` | `rock` vuelve, la animacion vuelve a empezar | igual |
| F1 sin ESTRATOS montado | `ESTRATOS no esta montado: ejemplo en memoria`, y todo lo de 8.5 sigue igual | la app se cierra |

**[!] VISTO EN EL RYZEN el 29-09 noche: F1 PARO LA MAQUINA.** Pantalla azul
de Ring 0: `#PF`, `err=0x1` (proteccion leyendo desde el KERNEL), `rip` en
`memcpy`, `cr2=0xE07AD000`, corria `tid=07` (el taller). La cuenta de los
bloques de F1 desde `MEMORIA_VA_BASE` (superficie 0x3B6000 + buzon 0x1000 +
logo 0x40000 + cielo 0x3B6000 = 0xE07AD000) dice que es la primera pagina del
bloque de 4 KiB con el que F1 SIEMBRA la biblioteca.

No era F1: era el kernel. `guardar_desde` entregaba a `copiar::traer` la VA
de Ring 3 del bloque, y con CR4.SMAP encendido Ring 0 no puede leerla; y el
limite se media contra la suma de TODOS los bloques del proceso. Es el fallo
que `LEER_EN` y `ESCRIBIR_DE` corrigieron el 24-08 y que este camino (otro
verbo, el mismo renglon) no recibio. F1 fue el primero en guardar desde un
bloque despues de SMAP. Arreglado en `367cedfb0` (`syscall/gesto.rs`,
`origen_tomar`: `fisica_de` + el espejo), y comprobado sin metal que NO quita
nada: los bloques son contiguos (`alloc_frames_contig`), el asignador no pasa
del espejo (16 GiB), y las cuatro llamadas de Ring 3 (dos de F1, `nuevo` y
`guarda` de F12) caen dentro de su bloque. `guarda` de F12 tenia el mismo
fallo sin que nadie lo hubiera pisado.

| se hace | si esta bien | si falla |
|---|---|---|
| F1 con el kernel de `367cedfb0` | el logo, y la consola dice `sembre titan/asteroids en ESTRATOS` | pantalla azul otra vez: foto de `rip`/`cr2` |
| F12: `guarda prueba.txt` | sale la generacion nueva | pantalla azul en `memcpy`: el kernel desplegado es el viejo |

**Lo siguiente:** 8.7 (colgar arrastrando). Despues, sin hacer: crear,
renombrar y borrar DESDE la columna (hoy se hace en F12 y F1 lo ve); abrir un
`.titan` en el editor; que un cable dibujado escriba el `use` (escalon 10). Y
los que siguen abiertos de 8.5.

### 8.7 L2: el PADRE dice donde vive su hijo, y colgar es arrastrar (29-09)

El propietario: *"en ESTRATOS no importa si se ve desordenado, la idea es que
el archivo jerarquia dominante en F1"* -- y *"en window pense porque no poner
archivo encima"*. En Windows solo una carpeta tiene hijos. En BMO-X el `mod`
ya lo hacia: `main.titan` esta ENCIMA de `ship.titan` porque lo declara. Se
eligio la opcion 1 de las dos que se discutieron:

```text
   1  la jerarquia la DECLARA el fichero (`mod`)   ELEGIDA: cero Ring 0, viaja
                                                   con el fichero, `vuelve` la trae
   2  un nodo de ESTRATOS con `:datos` Y           NO: cambio de formato en Ring 0
      `:entradas` (objects.rs ya dice que un       (~25 sitios miran el `tipo`),
      directorio es "un nodo con :entradas")       4 atributos por nodo, y FAT32
                                                   la pierde al copiar
```

**La ruta la dice el padre, y es ABSOLUTA dentro del paquete:**

```text
   mod ship in "naves/ship.titan"     desde la carpeta de Titan.toml, SIEMPRE
   mod collide                        sin `in`: donde la pondria cargo
                                      (src/x.titan, o src/<padre>/x.titan)
```

- `in` ya era una de las 25 palabras: **no se agrega ninguna**.
- Absoluta y no relativa al padre, a proposito: las mismas palabras son el
  mismo fichero lo declare quien lo declare. Por eso cambiar de padre **no
  mueve ni un byte** en el disco: solo cambian dos cabeceras.
- Nunca fuera del paquete: sin `/` delante, sin `.` ni `..`, termina en
  `.titan` (`text::is_package_path`). Lo que no cumple es `BadLine` con su
  numero de linea.

**Colgar es arrastrar** (`titan-lector::hang`, `explorer::draw_drag`):

```text
   arrastrar rock.titan sobre ship.titan (o sobre el NODO ship del lienzo)

   naves/ship.titan   + mod rock in "src/rock.titan"    (no esta donde cargo
   src/main.titan     - rock  (de `mod rock, physics`)   la buscaria bajo ship)
   src/rock.titan       intacto
```

- **Primero se comprueba sin tocar el disco** (`plan`): el destino se pinta en
  VERDE si se puede y en ROJO con el porque si no -- *"physics ya depende de
  ship: seria un ciclo"*. Tambien NO a mover `main`, a colgar de `Titan.toml`,
  de la 3060 o del DIRECTOR, y a donde ya esta.
- **Se escribe primero el padre NUEVO.** Si la segunda escritura falla, el hijo
  queda declarado dos veces (un problema que se DICE: *"hay dos nodos rock"*)
  y no por nadie (un fichero que desaparece de la vista). Hay prueba de eso.
- La ruta se escribe solo si hace falta: devolverlo a su sitio escribe otra
  vez un `mod collide` a secas (prueba: vuelve byte a byte a la semilla).
- Un clic torpe NUNCA reescribe nada: solo cuenta como soltar si el raton se
  movio mas de 4 px.
- F1 no relee por su cuenta: la generacion se movio y el latido siguiente lo
  lee, como veria el cambio cualquier otro. La seleccion sobrevive por NOMBRE.
- Los dos buffers (leer y reescribir una cabecera) son las dos mitades de UN
  bloque prestado: lo reescrito ya esta donde `guardar_desde` lo toma, y
  guardar no copia nada. El bloque vuelve solo al acabar (`Drop`).

**La columna muestra el arbol DECLARADO, no el de carpetas** (`FileEntry` lleva
su `parent` y su `depth`; `tree_order` los pone en profundidad, en su sitio):
guias verticales por nivel, y abajo **EN DISCO**, la ruta de verdad del fichero
seleccionado -- el orden de la pantalla no esconde el del disco.

**Lo que NO hace, a proposito:** no recoloca el nodo en el lienzo. Las
posiciones son del `[layout]` de `Titan.toml`, y moverlas sin que nadie lo pida
seria el programa decidiendo por el propietario. Tras colgar, el cable nuevo se
ve, pero el nodo sigue donde estaba (y los cables de la misma fila se enredan:
lo de 8.5 sigue abierto). Y el guion de ejemplo deja de animarse si le falta un
cable que nombra: el panel lo DICE en vez de animar algo inventado.

**Visto sin el metal: la CAMARA.** Un binario del PC que compila los MISMOS
`canvas.rs`, `view.rs`, `explorer.rs` y `player.rs` (por `#[path]`) con un
disco en memoria, y saca PNG. Cazo tres fallos antes del Ryzen: el pie de la
columna se salia por abajo, el panel decia "este paquete no trae eventos"
cuando lo que faltaba era un cable, y la `/` de una ruta partida caia al
principio de la linea siguiente.

**Como se mira en el Ryzen** (despues de `build.ps1` y desplegar):

| se hace | si esta bien | si falla |
|---|---|---|
| arrastrar `rock.titan` sobre `ship.titan` | `ship.titan` se enmarca en VERDE mientras se arrastra; al soltar, `rock.titan` sale DEBAJO de `ship.titan`, abajo *"rock cuelga ahora de ship; su fichero no se movio"* y `EN DISCO .../src/rock.titan`; la generacion sube 2 | el marco no sale: el puntero del buzon no llega mientras se arrastra |
| arrastrar `physics.titan` sobre el NODO `ship` | el nodo se enmarca en ROJO con *"physics ya depende de ship: seria un ciclo"*; al soltar, nada cambia en el disco | se escribe algo: `plan` no se esta mirando |
| F12: `vuelve 2` | `rock` vuelve bajo `main` (las dos escrituras eran dos versiones) | solo una vuelve: `vuelve 1` deshizo media operacion |
| un clic sin mover sobre un fichero | lo selecciona y centra su nodo; el disco no cambia | la generacion sube con un clic |

### 8.8 L3: el aspecto de TITAN++ (29-09)

El propietario puso el logo en `docs/arte/` (*"hacer epico... que mi TITAN++
asi, todo en mi F1"*). F1 es el taller de TITAN++ y se viste con SUS colores; el
escritorio sigue con los del gato (`Los colores del gato`), y no chocan: cada
uno lleva la identidad de lo que es.

**El logo DENTRO de F1, sin decodificador JPEG.** El gato eran tres colores
planos y bastaron dos mascaras de 1 bit. Este es 68 % negro y el resto halos y
degradados (medido), asi que el trabajo caro se hace en el PC, una vez:

```text
   docs/arte/titan.jpg                    la fuente (1254x1254)
   docs/arte/titan_a_logo.py              512x512, lo casi negro a negro EXACTO,
                                          128 colores, PackBits; comprueba que
                                          descomprime igual antes de escribir
   Ultra_userspace/apps/taller/arte/      TLG1: 106.089 B, commiteado -- el
   titan.bin                              build no depende de Python
   taller/src/art.rs                      15 lineas de PackBits + la paleta
```

- **La presentacion** al abrir: el logo entero sube, se sostiene y se funde
  dejando ver el taller (350 + 1.100 + 500 ms). Un clic o una tecla la cortan,
  y ese primer toque SOLO la corta: no selecciona un nodo que aun no se vio.
- **El cielo** (SIMPLIFICADO el 30-09, a pedido del propietario: *"la entrada
  mantener asi nada mas"*, y de fondo *"azul estrella y colores, pero
  simplificado"*): degradado de noche, dos nebulosas suaves con los colores
  del anillo -- azul abajo a la izquierda, violeta arriba a la derecha -- y 520
  estrellas (siempre las mismas) blancas, azules y alguna violeta. El logo es
  la ENTRADA; detras del grafo solo hay cielo, y nada compite con los nodos. La
  primera version ponia el dibujo del logo al 30 % detras (29-09): se quito, y
  con el el muestreo bilineal que solo servia para eso. Se pinta UNA vez en su
  propio bloque; cada fotograma es una copia, los mismos bytes que antes
  escribia `clear`.
- **La paleta** sale del logo: noche azul, azul electrico -> violeta (el
  anillo). Nodos con cabecera en degradado y HALO de su color (rojo mientras
  son el que falla), cables con halo y nucleo, el elegido con un halo mas
  ancho, la columna con su borde en degradado, `TITAN++` en la barra con los
  `++` de azul a violeta y el lema *CODE . NODES . BEYOND* con sus puntos de
  luz.

**Lo que se defendio:**

- **La decoracion nunca para el taller.** Si falta memoria para el logo o el
  cielo, o los bytes no son lo que dice el formato, F1 es el mismo en un color
  liso. `decode` probado con bytes HOSTILES: el recurso cortado en 42 puntos y
  3.000 mutaciones de 4 bytes -- 0 panicos, tambien en debug (desbordes
  comprobados); un bufer chico se rechaza.
- **La pila**: 55.552 B de 65.536 (`pila.py --ring3`, que ya mide `taller`).
  La paleta del logo (1 KiB) es lo que crecio.
- **Lo que cuesta**: `taller.bex` pasa de 157.528 a 272.216 B (tope 1 MiB), y dos
  bloques mas a la vez (el logo, 256 KiB; el cielo, 3,9 MiB) de los 8 por
  proceso.

**Visto con la CAMARA** (el mismo codigo de dibujo en el PC): a 384 px el logo
se quedaba chico en una ventana de 760 de alto y sus letras se emborronaban;
por eso es de 512.

**Como se mira en el Ryzen:**

| se hace | si esta bien | si falla |
|---|---|---|
| F1 | fondo negro, el logo sube en ~1/3 s, se queda, y se funde en el taller | sale el taller sin logo: la consola no dice nada, pero no hubo memoria o los bytes no pasaron `decode` |
| clic durante el logo | el logo se va al instante y NO se selecciona nada | se selecciona un nodo: el primer toque no se trago |
| el lienzo | estrellas de colores y dos nebulosas suaves (azul abajo a la izquierda, violeta arriba a la derecha); ningun dibujo detras de los nodos | negro liso: no hubo bloque para el cielo |
| arrastrar el lienzo | los nodos se mueven sobre el cielo, que se queda quieto | el cielo tiembla o se rompe: la copia no cubre la ventana |

### 8.9 L4: los cables LATEN, y un error se VE y GUIA (30-09)

El propietario: *"los cables tengan animacion de color blanco"*, *"en la parte
de Explorer mejores con iconos y algo simple"*, y para los fallos *"rojo
grueso y que diga error... animacion divertida... y si hay multiples, por algo
son nodos, para guiar"* -- inspirado en los nodos de Houdini y Blender, pero
con animacion propia.

```text
   el pulso        una cometa BLANCA baja por cada cable, del que usa al usado
                   (la direccion de todo `mod` y `use`), cada cable con su fase.
                   No sobre un cable ocupado por un prestamo: ese ya habla
   los iconos      8x10, dibujados con `rect`: un paquete, una hoja con T (un
                   .titan), una hoja con lineas (Titan.toml), un aviso (un
                   problema), un disco (EN DISCO)
   un ERROR        borde rojo grueso, halo rojo que RESPIRA, una onda que sale
                   del nodo, y la etiqueta ERROR montada en la esquina de su
                   cabecera (flota dos pixeles)
   varios          ERROR 1/3, 2/3... en el orden en que se lee un grafo (de
                   arriba abajo, de izquierda a derecha), unidos por un camino
                   rojo de trazos EN MARCHA hacia el siguiente. [e] lleva la
                   camara al siguiente y lo selecciona
```

**De donde sale un error, y en que nodo cae** (`faults.rs`): de los problemas
del lector -- falta un `mod` (el padre que lo declara), un `use` de nadie o un
permiso no pedido (el que usa), dos con el mismo nombre, un ciclo (los dos
extremos), sin Titan.toml o sin main (la raiz) -- y del NO del comprobador que
este en pantalla (Conflict: los dos nodos; Denied: ese). Un problema que no
nombra un nodo que exista (una cabecera que no se entiende, un fichero cuyo
nombre no casa) se queda en PROBLEMAS: inventarle un nodo seria apuntar al
sitio equivocado. El EXPLORER pinta en rojo el fichero de cada nodo en falta.

**Lo que cuesta, y la regla de la casa.** Lo que se mueve obliga a repintar la
ventana entera. Por eso el pulso y la respiracion del rojo corren a ~30 fps
**solo mientras la ventana se ve y alguien la toco en los ultimos 20 s**; en
reposo el rojo se QUEDA, quieto, y F1 vuelve a dormir 100 ms sin pintar (*si no
hace nada, no consume*). El primer toque lo despierta.

**Visto con la CAMARA**: la primera etiqueta iba 24 px encima del nodo y
tapaba la ultima linea del de arriba (los nodos estan a ~20 px); ahora va a
caballo de la esquina de su propia cabecera. El pulso de un pixel no se veia:
ahora la cabeza son cinco. Y `core` no tiene `sqrt` sin `std`: el largo de un
trazo sale de una raiz entera.

**Como se mira en el Ryzen:**

| se hace | si esta bien | si falla |
|---|---|---|
| F1 y esperar | puntos blancos bajan por los cables sin parar | cables quietos: el latido de 33 ms no llega |
| no tocar nada 20 s | los pulsos se paran; al mover el raton vuelven | siguen para siempre: F1 no descansa |
| F12: `renombra titan/asteroids/src/rock.titan roca.titan` | `main` en rojo con ERROR, su onda, `main.titan` en rojo | el problema sale en la lista pero ningun nodo se enciende |
| ademas un `use ufo` en physics | ERROR 1/2 en main y 2/2 en physics, y el camino rojo entre los dos | sin camino, o numeros al reves |
| `e` | la camara salta al siguiente error y lo selecciona | no pasa nada: la `e` no llega como letra |

### 8.10 L5: el EXPLORER organiza el DISCO, a la manera del propietario (04-10)

El propietario: *"que TALLER tenga motivos de existencia, su EXPLORER unico que
puedan organizar carpetas, archivos, todo"*, inspirado en VS Code, y *"tener
absoluto control de organizar a mi manera en archivos sin tener que pelear con
orden de A hasta la Z ... PERSONALIZAR TODO"*. Eligio las cuatro acciones:
nuevo archivo / carpeta, renombrar, quitar y carpetas plegables.

**La columna organiza el DISCO; el lienzo organiza los MODULOS.**

```text
   ARCHIVOS        [+][+]    nuevo archivo, nueva carpeta (donde esta lo elegido)
     v docs                  TODO lo que hay en la carpeta del paquete, no solo
         LEEME.txt           lo que nombra un `mod`
     v src
         main.titan          un modulo: el clic enciende su nodo
         ufo.titan           un .titan que nadie declara: en gris
       > physics             una carpeta plegada (la flecha la abre)
     Titan.toml
```

**Listar sin tocar el cursor de F12.** Se iba a pedir una pregunta nueva en
Ring 0 y NO hizo falta: el objeto `KIND_DIRECTORIO` (01-10, nacido para
PROTON-X) ya lista ESTRATOS con un handle POR PROCESO (`bmo::Directorio`), y
F1 abre UNA carpeta cada vez (son ocho para todo el sistema). Lo de FAT32 con
el mismo nombre (`titan/` guarda los `.bex` de T3) se salta. Cero cambios en
Ring 0.

**El orden es del propietario, no de la A a la Z.** Arrastrar una fila sobre
otra de la MISMA carpeta la pone delante (mitad de arriba) o detras (mitad de
abajo), y queda en `Titan.toml`, al lado de `[layout]` (TITAN_MAESTRO 13.5: el
nodo principal guarda la vista):

```toml
[explorer]                       # carpeta = sus hijos, en el orden elegido
"." = ["docs", "src", "Titan.toml"]
"src" = ["main.titan", "ship.titan"]

[explorer.folded]                # las carpetas que se quedan cerradas
"src/physics" = true
```

Lo que nadie coloco va detras, en el orden del DISCO (el de creacion): lo
nuevo sale al final de su carpeta. Un renombre conserva el sitio (cuelga del
elemento, no del nombre viejo).

**Los gestos, y lo que arrastran de su modulo** (`titan-lector::organize`,
probado en el anfitrion con un ESTRATOS en memoria):

| gesto | como | en el disco | y en las cabeceras |
|---|---|---|---|
| nuevo archivo | `[+]`, menu, y se escribe el nombre en la fila | `crear_desde` | un `.titan` nace con `mod x "..."`; si habia un modulo elegido, ese lo declara (`mod x in "..."` si cargo no lo buscaria ahi) |
| nueva carpeta | `[+]` de carpeta, o menu | `crear_carpeta` | -- |
| renombrar | doble clic, menu, o F2 si el escritorio la deja pasar (F1-F10 son suyas) | `renombrar` | su `mod rock` -> `mod roca`, y el del padre |
| quitar | Supr dos veces (la primera pregunta, 3 s), o menu | `quitar` (queda en el historial) | el padre deja de declararlo; `Titan.toml` y `src/main.titan` no se quitan |
| mover a carpeta | soltarlo sobre una carpeta (o en ARCHIVOS: la raiz) | leer + `crear_desde` + `quitar` | el padre pasa a `mod x in "nueva/ruta.titan"` |
| ordenar | soltarlo entre dos hermanos | -- | `[explorer]` de `Titan.toml` |
| plegar | la flecha, Enter, izquierda / derecha | -- | `[explorer.folded]` |
| declarar | soltar un `.titan` gris sobre un NODO del lienzo | -- | ese modulo lo declara |

[!] **Cambia un gesto de L2 (8.7)**: soltar un fichero sobre OTRA FILA ya no lo
cuelga: ahora ordena o mueve en el disco. Colgar (cambiar quien lo declara) se
hace soltandolo sobre un NODO del lienzo, que ya lo hacia. Sin ESTRATOS (el
ejemplo en memoria) la columna sigue siendo el arbol declarado y L2 sigue
igual.

**Lo que NO reescribe, a proposito**: los `use rock` de OTROS ficheros (seria
leer y escribir el paquete entero por un nombre), los hijos que un modulo
guarda en la carpeta de su nombre, y los `mod ... in` que pasaban por una
carpeta renombrada. Salen en PROBLEMAS con sus nombres y el lienzo los pinta en
rojo; la segunda linea de la nota lo dice. Moviendo CARPETAS: todavia no (la
nota lo dice). Y nada se pierde: cada gesto dice su `vuelve N`.

**Lo medido**: `taller` pasa a 56.368 B de pila de 65.536 (`pila.py --ring3`,
eran 55.696). El arbol (~8 KiB) vive en un bloque prestado, no en la pila: un
`Tree` son solo enteros, asi que cualquier byte es uno valido.

**Visto con la CAMARA** (ahora en el repositorio:
`toolchain/tools/espejo-cara`, `cargo run -p bmo-espejo-cara --bin cara-taller
-- <carpeta>`, pinta `explorer.rs` de verdad sobre un ESTRATOS en memoria):
cazo el cursor del nombre dibujado a mitad del texto y la segunda linea de la
nota cortada.

**Como se mira en el Ryzen** (despues de `build.ps1` y desplegar, con ESTRATOS
montado):

| se hace | si esta bien | si falla |
|---|---|---|
| F1 | ARCHIVOS muestra `Titan.toml` y `src` con sus ficheros, y los dos botones `[+]` | solo salen los modulos: `Directorio` no abrio la carpeta (CABINA, `dir`) |
| `[+]` con `ship.titan` elegido, `wing.titan`, Enter | `wing.titan` en `src`, un nodo `wing` colgando de `ship`, y la nota con su `vuelve 2` | el nombre no llega: las letras no van al buzon mientras se escribe |
| arrastrar `rock.titan` sobre la mitad de arriba de `main.titan` | sale una linea azul delante de `main`; al soltar, `rock` queda delante y `Titan.toml` tiene `[explorer]` | se cuelga en vez de ordenarse: es el binario de antes |
| clic en la flecha de `physics` | se pliega; tras cerrar y abrir F1 sigue plegada | se abre otra vez: `[explorer.folded]` no se escribio |
| doble clic en `rock.titan`, `roca.titan`, Enter | `roca.titan` en su sitio, el nodo `roca`, y PROBLEMAS con los `use rock` de `ship` y `physics` | el nodo desaparece: no se reescribio la cabecera |
| Supr, Supr sobre `roca.titan` | la primera pide confirmar (parpadea en rojo), la segunda lo quita; `vuelve 2` en F12 lo trae | se quita a la primera |
| clic derecho en una fila | el menu de cuatro entradas; Esc lo cierra sin cerrar F1 | Esc cierra F1: el menu no se quedo la tecla |

### 8.11 L6: la solapa ESPACIO -- cada nodo, un astro (04-10)

El propietario: *"un tab simple pero que muestren todos los nodos dinamicos,
unicos y divertidos, que son representantes del espacio, como el centauro del
espacio"*. Arriba del lienzo, dos solapas: **GRAFO** (lo de siempre) y
**ESPACIO** (`space.rs`); `[t]` cambia de una a otra.

```text
   el paquete (Titan.toml)   EL CENTAURO, una constelacion: las cuatro patas son
                             BMO-X; el torso y el arco, TITAN++ (TITAN_MAESTRO
                             6b.6, dibujado)
   un modulo                 un PLANETA propio: color, anillo, medida y giro
                             salen de su NOMBRE (FNV-1a): dos nunca iguales, y
                             cada uno siempre el mismo. Sus lunas: lo que usa
   la 3060                   un PULSAR, dos haces que giran
   el DIRECTOR               una ESTACION, con sus paneles y su luz
   una dependencia           un ASTEROIDE
   un modulo en fallo        una SUPERNOVA roja que respira
   un cable                  una ruta de luz, y la cometa que baja por ella
```

- **No decide nada**: los astros estan donde el `[layout]` pone los nodos, asi
  que un clic, arrastrar y el EXPLORER funcionan igual en las dos solapas.
- **Solo se mueve si lo miras y lo tocas** (el reloj de las cometas, 20 s
  despues del ultimo toque): en reposo, el cielo se queda quieto y F1 duerme.
- **Sin coma flotante**: `core` no tiene `sin`; un cuarto de onda vive en una
  tabla de 17 numeros. El halo es REDONDO y propio (`halo`): el `glow` del
  lienzo es cuadrado, hecho para las cajas del grafo -- lo cazo la CAMARA
  (`cara-taller` saca ahora `espacio.png` y `grafo.png`).
- **La pila**: `taller` sigue cabiendo (`pila.py --ring3`).

| se hace | si esta bien | si falla |
|---|---|---|
| F1, `t` | el CENTAURO arriba, un planeta por modulo, el pulsar y la estacion; abajo, la leyenda | la solapa no cambia: la `t` no llega |
| mover el raton | las lunas giran, las estrellas titilan, las cometas bajan | todo quieto: el latido de 33 ms no llega |
| 20 s sin tocar | el cielo se queda quieto | sigue moviendose: F1 no descansa |
| clic en un planeta | su fichero se enciende en el EXPLORER | no pasa nada: el clic no cae en su caja |

### 8.12 L7: el nodo es lo que HACE, en vivo; y ESPACIO con sus solapas (04-10)

El propietario: *"los nodos tienen que tener algo unico que representan, pero
tambien cuando el programador cambia, todo el nodo se cambia en tiempo real en
la forma que representa: el mut, el que envia paquetes, el que representa
conectores fuertes"*, *"en VS Code es 2D, pero en 3D lo esconde"*, y *"que en
ese cuadro tengan solapas para tener ideas de las guias y porque, con lineas y
que sean ordenadas"*.

**Los RASGOS** (`titan-lector::traits`): al leer cada `.titan` (los mismos
bytes que ya se traian para la cabecera, cero lecturas de mas) se cuenta lo
que su CUERPO hace -- `fn`, `let`, `let mut`, `x = ...`, `print`, llamadas.
Viajan en cada `FileEntry`, y como F1 relee el paquete cuando sube la
generacion de ESTRATOS, **guardar el fichero cambia el astro en el latido
siguiente**. [!] Es una lectura rapida, no el compilador: cuenta, no juzga.

```text
   fn       la medida del planeta        let      sus LUNAS, quietas
   let mut  ANILLOS ambar que giran      x = ...  mas rapido giran
   print    un EMISOR: paquetes que suben hacia la consola
   llamadas COMETAS                      sin cuerpo  un PROTOPLANETA
```

**Los cables tienen CLASE, y la clase da el color, FIJO** (*"colores que te
limitan para no tener que pelear"*): `mod` violeta trenzado (el lazo fuerte:
quien declara a quien), `use` cian (depende de el), verde la 3060, azul el
sistema. El mismo color en todas las solapas (`astros::Cable`).

**ESPACIO tiene tres solapas** (`t` las recorre todas):

```text
   CIELO 3D    el paquete en perspectiva, con suelo de luz: la hondura es lo
               hondo que esta cada modulo en el arbol declarado (el metal --
               3060, DIRECTOR -- al fondo). Gira solo mientras F1 esta vivo;
               arrastrar el cielo lo gira a mano; clic en un astro lo elige y
               abajo se lee lo que es, contado de sus rasgos
   ELEMENTOS   doce tarjetas: cada elemento ANIMADO con el mismo pintor del
               cielo (`astros.rs`: el catalogo no puede mentir), que es y por
               que se ve asi
   GUIA        seis porques numerados, en dos columnas con sus lineas: la
               verdad es el texto, del texto al astro en vivo, los dos jueces,
               los colores, por que 3D, las teclas
```

3D con enteros: girar sobre el eje vertical, inclinar hacia el ojo y dividir
por la distancia; un seno de 64 pasos con el paso de en medio interpolado
(1024 por vuelta) para que el giro lento no salte; los lejanos primero, y los
NOMBRES en una segunda pasada para que un planeta cercano no tape el nombre de
uno lejano (lo cazo la camara).

**La semilla `asteroids` tiene cuerpos** desde hoy (TITAN++ de los niveles
0-2): cada nodo del primer paquete se ve distinto. Antes eran solo cabeceras,
a proposito: el cuerpo no estaba decidido.

**Lo medido**: `taller` 57.264 B de pila de 65.536 (`pila.py --ring3`).
**Visto con la CAMARA**: `cara-taller` saca `cielo.png`, `elementos.png` y
`guia.png`.

| se hace | si esta bien | si falla |
|---|---|---|
| F1, `t` | CIELO 3D: el centauro, los planetas sobre un suelo de luz, girando | plano: la solapa es la de antes |
| arrastrar el cielo | gira con el raton | se mueve el lienzo: el arrastre no se quedo |
| F12: poner `let mut n = 0` y `n = n + 1` a `rock.titan` | en el latido siguiente `rock` estrena un anillo ambar que gira | no cambia: el latido no relee |
| `t` otra vez, y otra | ELEMENTOS con doce tarjetas que se mueven; GUIA con sus seis puntos | -- |

### 8.13 L8: GRAFO con pines, a lo Unreal Engine 5 -- un cable es un `use` (04-10)

El propietario: *"un estilo de Unreal Engine 5 en nodos, que tienen como para
agarrar los cables, pero los cables se pueden colorear pero tambien te limita
para no tener que pelear ... solo colores que ya estan conectados"*.

```text
   cada nodo         un pin IN arriba (lleno si alguien lo usa) y, si es un
                     modulo, un pin OUT abajo (lleno si usa a alguien)
   tirar del OUT     un cable en la mano, hasta otro nodo:
                       cian       se puede soltar
                       ROJO       no, y dice por que, ANTES de soltar
   soltarlo          se escribe `use <nodo>` en la cabecera del modulo
                     (titan-lector::wire + edit::add_use); el latido
                     siguiente lo relee y el cable ya es del texto
   los colores       los da la CLASE, fijos (astros::Cable): mod violeta,
                     use cian, 3060 verde, sistema azul. Nadie elige un
                     color: el color dice lo que es el cable
```

- **Se rechaza donde lo rechazaria el compilador** (U3, las dependencias solo
  bajan): un cable que cierra un ciclo, uno que ya existe, uno hacia el
  paquete, uno desde algo que no es un modulo. Probado en el anfitrion con un
  ESTRATOS en memoria (`wire::tests`).
- `use gpu` sin `gpu` en `[permissions]` se escribe y sale en PROBLEMAS (U2):
  el lector ya lo decia, y el cable no lo esconde.
- Es el **escalon 10** de 8.4, hecho.

**Visto con la CAMARA**: `cable_bien.png` (rock -> ship, cian) y
`cable_ciclo.png` (ship -> physics, rojo: *"physics ya depende de ship: seria
un ciclo"*).

| se hace | si esta bien | si falla |
|---|---|---|
| tirar del pin de abajo de `rock` hasta `ship` | cable cian; al soltar, `rock.titan` tiene `use ship` y el cable se queda | el nodo se mueve: el clic no cayo en el pin |
| tirar de `ship` hasta `physics` | rojo, con su motivo; al soltar no se escribe nada | se escribe: `plan` no se miro |

### 8.14 L9: APARIENCIA y LOGICA, cortadas; MAQUETA viste a F1; el `if` se ve (04-10)

El propietario: *"si es para mejorar apariencia usa MAQUETA, MAQUETA ya tiene
mejoras ... pero divide bien en apariencia y la logica de TITAN++"*. El corte,
dicho como una regla que se comprueba:

```text
   LOGICA       lo que el paquete ES y lo que se le puede hacer
                titan-lector, titan-contrato, el compilador, store.rs.
                NI UN COLOR: `titan-lector` tiene una prueba
                (`the_logic_names_no_look`) que falla si su codigo
                nombra un color, un pixel o un pintor

   APARIENCIA   como se DIBUJA
                aspecto/titan.maqueta   los colores, como los lee MAQUETA
                src/tema_gen.rs         generado de el (`maqueta --paleta`),
                                        nunca a mano
                src/aspecto.rs          los PAPELES (BG, INK, CYAN...), las
                                        medidas y las piezas SUAVES de
                                        MAQUETA 2 (`bmo-pinta`): caja
                                        redonda, borde, resplandor, degradado
                view, space, astros,    los pintores: LEEN la logica y no
                guia, explorer          deciden nada de ella
```

- **Las piezas suaves son las de MAQUETA, no unas de F1**: `bmo-pinta` es el
  pintor de MAQUETA 2 (04-10, "nitidez de matematica"), el mismo que corre el
  escritorio y la foto del anfitrion. F1 solo escribe el ADAPTADOR (`Soft`):
  su lienzo contestando las dos preguntas del pintor. Un nodo, una ficha, una
  solapa y una etiqueta tienen ahora el borde de una curva exacta.
- **La paleta de F1 es la de TITAN++, no la del gato**: vive al lado de la app
  (`aspecto/titan.maqueta`) y no en el tema de la casa, para que `.accent` no
  pelee por ser el ojo del gato alli y la luz cian aqui.
- **El nivel 3 en el cielo**: `Traits::ifs` cuenta cada `if` y `else if`, y el
  planeta lleva una **ESTRELLA DOBLE** por decision: dos estrellas que giran,
  una dorada (el camino que corre) y una brasa gris (el lado MUERTO: el
  compilador lo decidio y no dejo bytes). La semilla `ship` decide si le queda
  combustible; ELEMENTOS tiene su ficha y la GUIA lo cuenta.
- **ELEMENTOS en fichas horizontales**: la figura viva a la izquierda, el que y
  el porque a la derecha, tres columnas. Con cinco, los porques se cortaban
  (visto con la camara).

[!] **La LETRA de la casa (`bmo-letra`, proporcional y suave) NO entra
todavia**, y no por gusto: un glifo nuevo cuesta ~13 KiB de pila
(`userland/src/pantalla/verde/fina.rs` lo midio) y el marco mas hondo de F1
va por ~57 KiB de los 64 KiB de Ring 3 (`pila.py`: 57 296). Entra el dia que
ese marco baje; hasta entonces, la 8x16.

**Visto con la CAMARA**: `grafo.png` (nodos redondos con su resplandor, la
etiqueta del pin y la solapa suaves), `elementos.png` (13 fichas, la ESTRELLA
DOBLE entre ellas), `cielo.png` (`ship: ... 1 if (doble)`) y `guia.png`.

| se hace | si esta bien | si falla |
|---|---|---|
| abrir F1 en el GRAFO | nodos con esquinas redondas y suaves, sin escalera | esquinas cuadradas: `aspecto.rs` no esta en el binario |
| escribir un `if` en `ship.titan` y guardar | en CIELO, `ship` gana una estrella doble dorada | nada cambia: el latido no relee, o `Traits::ifs` no cuenta |
| cambiar un color en `titan.maqueta` y regenerar | F1 entero cambia ese papel, sin tocar Rust | hay que tocar Rust: alguien escribio el color fuera de `aspecto.rs` |

---

Ver [`PLAN_AUTOHOSPEDAJE.md`](en_pausa/PLAN_AUTOHOSPEDAJE.md) (el mismo trabajo desde el
otro lado, y la medida que elige Ada),
[`VALKYRIE-ABI/FRONTERA.txt`](../../VALKYRIE-ABI/FRONTERA.txt) (los ocho
prefijos que deciden la seccion 2),
[`EL_ORQUESTAL.md`](../identidad/EL_ORQUESTAL.md) (por que la autoridad no
viaja) y [`PLAN_REX.md`](PLAN_REX.md) (las cabeceras de las que se copia la
forma).

### 8.15 L10: la TAB de los NODOS MAESTROS -- a lo Houdini y Blender (05-10, HECHO)

El propietario, 05-10: *"cuando termine ahi me dejas las TAB elegante, en ellas
el estilo de Houdini con Blender que muestran opciones totales de NODOS maestro
ya hechos como ejemplos y porque, como tutoriales y pruebas"*.

**Hecho el mismo 05-10** (M1-M7 abajo; lo que cambio del plan, en *Lo que se
aprendio al hacerlo*). Lo que toma de cada uno:

```text
   Houdini     TAB en el editor de nodos: una lista que se FILTRA mientras se
               escribe, y Enter pone el nodo donde esta el raton
   Blender     el menu de AGREGAR por familias (Shift+A), y sus plantillas:
               cosas ya hechas para partir de ellas
   lo de casa  cada nodo maestro trae su PORQUE y su PRUEBA: no es un ejemplo
               cualquiera, es uno que compila, corre y escribe lo que dice
```

** LA IDEA QUE LO HACE FACIL Y HONESTO: LOS NODOS MAESTROS SON EL BANCO. Cada
programa `BIEN` de `toolchain/lang/titan/ejemplos/nivelN/` ya es un nodo
maestro: el compilador lo acepta, el banco del emisor lo CORRE en el emulador y
compara sus lineas `# sale:`, y su comentario dice por que existe. Asi que la
TAB no inventa un catalogo que pueda mentir:

```text
   el nodo           el programa del banco, hecho modulo del paquete
   su familia        su NIVEL: saludar, calcular, decidir, repetir ...
                     la 3060 -- la TAB, en orden, ES el curso de TITAN++
   su porque         el comentario de su segunda linea
   lo que trae       las palabras de su nivel (fichas: `if`, `match`, `gpu`)
   su prueba         sus lineas `# sale:`: lo que escribe al correr, y que
                     dos bancos comprueban en cada build
   su astro          sus RASGOS (`titan-lector::traits`) sobre su texto: la
                     misma figura que tendra en el cielo, antes de ponerlo
```

**Lo que se ve** (APARIENCIA: `aspecto/titan.maqueta` y un pintor `tab.rs`):

```text
   TAB         se abre sobre el GRAFO (y sobre el CIELO): una caja redonda con
               un campo de busqueda arriba; Esc o TAB la cierran
   la lista    a la izquierda, por familias (el nivel, con su numero y su
               nombre); cada fila, el nombre del nodo y que hace en una linea
   la ficha    a la derecha, el nodo elegido: su astro VIVO (`astros.rs`), su
               porque, sus fichas de palabras y su prueba (`# sale:`), como en
               ELEMENTOS
   escribir    filtra por nombre, palabra o porque ("match", "dinero", "3060")
   Enter       lo pone en el paquete, donde estaba el raton
```

**Lo que hace** (LOGICA: `titan-lector`, sin un color, como manda
`the_logic_names_no_look`):

```text
   poner un nodo   escribe `src/<nombre>.titan` -- el programa, con su
                   cabecera renombrada al nombre del nodo -- y `mod <nombre>`
                   en la cabecera de `main`; si el nombre ya esta, `_2`. El
                   latido siguiente lo relee, como un cable (8.13)
   la regla        lo que se pone COMPILA: viene del banco, y el banco pasa
```

### Lo que se aprendio al hacerlo

```text
   28 nodos        los programas BIEN de un fichero, y los paquetes de UN solo
                   modulo (nivel9/con_permiso, nivel11/activa y mezcla). Los de
                   VARIOS modulos (nivel9/flota y tienda, nivel10/motor) no son
                   UN nodo: no entran, y el generador lo dice al correr
   el nombre       un programa que usa su propio nombre dentro (`let area` en
                   `area`) no puede ser el modulo `area` (T0055): ese nombre
                   tambien esta COGIDO, y el nodo es `area_2`. Lo encontro la
                   prueba del compilador, no una revision
   donde cae       Enter escribe su fila en [layout] de Titan.toml, centrada
                   donde estaba el raton: TRES escrituras (el fichero, la fila,
                   el mod en main), en ese orden -- cortado a medias queda un
                   fichero que nadie declara, nunca un mod que apunta a nada
   lo que pide     un nodo de la 3060 en un paquete sin `gpu` en [permissions]
                   se rechaza ANTES de escribir un byte, con el motivo
   la prueba       `toolchain/lang/titan/tests/maestros.rs` pone CADA nodo en la
                   semilla (como F1) y compila el paquete entero: "lo que se
                   pone COMPILA" es un test, no una promesa
   la pila         57.408 -> 57.840 de 65.536: la tabla es `static` (.rodata)
```

### Las casillas

- [x] M1 -- el generador: `toolchain/tools/maestros` lee los programas `BIEN` de `toolchain/lang/titan/ejemplos/` y escribe `platform/shared/titan-lector/src/maestros_gen.rs` (una tabla constante: nombre, nivel, porque, palabras, fuente, salida); generado, nunca a mano, como `tema_gen.rs`
- [x] M2 -- su guardian: `maestros.py --check` en `Ultra_kernel_x86-64/build/guardianes.ps1`: la tabla y el banco dicen lo mismo (un ejemplo nuevo entra en la TAB solo, uno borrado sale)
- [x] M3 -- la logica: `platform/shared/titan-lector/src/maestros.rs` filtra (nombre, familia, palabra, porque, nivel) y PONE (el fichero nuevo, su fila de `[layout]` y la linea `mod`), con pruebas sobre un ESTRATOS en memoria como `wire::tests`; y `toolchain/lang/titan/tests/maestros.rs` compila cada nodo puesto en la semilla
- [x] M4 -- la apariencia: las clases `.tab`, `.tab-fila`, `.tab-elegida` en `Ultra_userspace/apps/taller/aspecto/titan.maqueta`, `tema_gen.rs` regenerado, y el pintor `Ultra_userspace/apps/taller/src/tab.rs` con las piezas suaves de `aspecto.rs`
- [x] M5 -- las teclas: TAB, escribir, flechas, Enter, Esc, y el clic en una fila; viven en `Ultra_userspace/apps/taller/src/main.rs` (donde estan las demas teclas) y `Ultra_userspace/apps/taller/src/tab.rs`, no en `view.rs`
- [x] M6 -- la pila: `pila.py --ring3` sigue por debajo de 65.536 con la TAB abierta: 57.840 (la tabla es `static`: va en `.rodata`, no en la pila; ver `toolchain/tools/pila/pila.py`)
- [x] M7 -- la camara: `cara-taller` (`toolchain/tools/espejo-cara/src/bin/cara_taller.rs`) saca `tab.png` (abierta, con la ficha de `semaforo`) y `tab_filtro.png` (filtrando "match": quedan los 3 del nivel 8)

| se hace | si esta bien | si falla |
|---|---|---|
| F1, en el GRAFO, TAB | la caja con las familias del nivel 0 al 11 y la ficha del primero | no se abre: la tecla no llega a `view.rs` |
| escribir `match` | quedan los nodos del nivel 8 (y los que lo nombran en su porque) | no filtra, o filtra solo por nombre |
| Enter sobre `semaforo` | aparece el nodo; `src/semaforo.titan` existe y `main` dice `mod semaforo` | aparece y no esta en el disco: no se guardo |
| `titan check` del paquete | bien | NO: el nodo no vino del banco, o se renombro mal |

### 8.16 L11: el nodo se EDITA en su sitio; cada uno con su FORMA; `hola` imprime a la vista (05-10, HECHO)

El propietario, 05-10: *"formas de nodos unicos que representan, y cuando le
doy click a los nodos en grafos tengan click derecho o doble click para
modificar escrituras en tiempo real ... el ejemplo de hola mundo, es simple,
pero en la que imprime, el nodo de imprimir o logo de impresora, mas facil"*.

```text
   la FORMA         la cabecera de cada nodo lleva, de derecha a izquierda,
                    lo que HACE (`iconos.rs`): PLAY en main, la IMPRESORA si
                    escribe, el ROMBO si decide, el BUCLE si repite, el
                    CRISTAL si tiene tipos, el ANILLO si algo cambia, la CAJA
                    si nombra valores, la FLECHA si llama, el GANCHO si
                    devuelve, el CHIP en la 3060. Mismo color que en el CIELO
   la IMPRESORA     el cuerpo del nodo dice lo que imprime: el texto EXACTO
                    si todos los argumentos son textos escritos ahi
                    (`print("hola mundo")` -> "hola mundo"), y los argumentos
                    tal cual si algo se calcula. Lo lee el lector
                    (`traits::Said`), y una prueba del compilador comprueba
                    que lo que el nodo dice es lo que el programa imprime
   la CONSOLA       el panel de abajo, sin guion: el primer print de cada
                    nodo, con el mismo criterio
   2 CLICS          en un nodo abren su CODIGO en un panel a la derecha (el
                    grafo sigue a la vista): colores de TITAN++, la sangria
                    sigue sola con Enter, TAB son 4 espacios (un tabulador es
                    T0010). Se GUARDA SOLO al parar de escribir (~1 s): una
                    version de ESTRATOS, el latido relee y el nodo cambia
                    -- su impresora, sus formas -- mientras se mira. Esc
                    guarda y cierra
   CLIC DERECHO     en un nodo: el menu de su fichero, con `Editar codigo`
                    arriba (renombrar, quitar, nuevo: lo de siempre)
   HOLA             la semilla siembra un segundo paquete, `titan/hola`: un
                    nodo, `print("hola mundo")`. El mas chico que hay
```

** Lo que NO hace todavia, dicho para que nadie lo busque: **correr** el
programa editado dentro de BMO-X. La consola LEE el texto (lo que un `print`
de textos escritos imprime es ese texto); lo que se calcula lo calcula el
compilador, y el compilador vive en el anfitrion (`titan build`) -- correrlo
dentro de F1 es el autohospedaje (T6, `PLAN_AUTOHOSPEDAJE`). El
`titan/hola.bex` del FAT32 se compila del banco en cada build, no de lo que se
edita en F1: lanzarlo desde aqui mostraria otro programa, y por eso no se hace.

** La pila, de paso: la tienda (`Store`, ~13 KiB) salio de la pila de `_start`
a un bloque propio, construida campo a campo: F1 baja de 57.840 a 40.032 de
65.536 (`pila.py --ring3`).

- [x] E1 -- la forma: `Ultra_userspace/apps/taller/src/iconos.rs` (los iconos) y su uso en la cabecera de cada nodo en `Ultra_userspace/apps/taller/src/view.rs`
- [x] E2 -- la impresora: `Said` en `platform/shared/titan-lector/src/traits.rs` (con sus pruebas) y la prueba cruzada `what_the_printer_on_the_node_says_is_what_the_program_prints` en `toolchain/lang/titan/tests/maestros.rs`
- [x] E3 -- el editor: `Ultra_userspace/apps/taller/src/editor.rs`; leer y guardar en `Ultra_userspace/apps/taller/src/store.rs` (`read_text`, `save_text`); 2 clics, clic derecho y autoguardado en `Ultra_userspace/apps/taller/src/main.rs`; `Editar codigo` en el menu de `Ultra_userspace/apps/taller/src/explorer.rs`
- [x] E4 -- hola: el paquete en `platform/shared/titan-lector/src/seed.rs` (solo en una biblioteca NUEVA: una que ya existe no se toca en silencio; el nodo `hola` tambien esta en la TAB)
- [x] E5 -- la camara: `toolchain/tools/espejo-cara/src/bin/cara_taller.rs` saca `hola.png`, `menu_nodo.png`, `editor.png` y `editor_guardado.png`
- [ ] E6 -- correr lo editado dentro de BMO-X: espera al compilador dentro de F1 (T6, `docs/plan/en_pausa/PLAN_AUTOHOSPEDAJE.md`)

| se hace | si esta bien | si falla |
|---|---|---|
| abrir `hola` en F1 | un nodo `main` con PLAY e IMPRESORA, y `"hola mundo"`; la CONSOLA dice `> hola mundo` | sin impresora: `Traits::writes` no cuenta, o el nodo no lee sus rasgos |
| 2 clics en `main`, cambiar el texto, esperar 1 s | el panel dice `guardado`, el nodo y la CONSOLA dicen lo nuevo | dice `escribiendo...` siempre: `guardar_desde` fallo (CABINA, F11) |
| clic derecho en un nodo | el menu, con `Editar codigo` arriba | nada: el nodo no tiene fichero en el disco (el ejemplo en memoria) |
| `vuelve 1` en F12 | el texto de antes, y el nodo con el | -- |
