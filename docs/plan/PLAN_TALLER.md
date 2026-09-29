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
   [ ] 10 dibujar un cable escribe un `use`; uno hacia arriba se rechaza
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

---

Ver [`PLAN_AUTOHOSPEDAJE.md`](en_pausa/PLAN_AUTOHOSPEDAJE.md) (el mismo trabajo desde el
otro lado, y la medida que elige Ada),
[`VALKYRIE-ABI/FRONTERA.txt`](../../VALKYRIE-ABI/FRONTERA.txt) (los ocho
prefijos que deciden la seccion 2),
[`EL_ORQUESTAL.md`](../identidad/EL_ORQUESTAL.md) (por que la autoridad no
viaja) y [`PLAN_REX.md`](PLAN_REX.md) (las cabeceras de las que se copia la
forma).
