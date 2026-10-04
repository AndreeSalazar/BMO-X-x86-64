# TITAN++ MAESTRO -- el lenguaje para CONSTRUIR sobre BMO-X

> **TITAN++** -- nombre elegido por Eddi el 2026-09-29. Empezo llamandose
> "Rust++" y el propietario lo cambio: *"es una ironia que queria usar Rust++ por
> motivos pero me llevo el Titan++, ese me gusta mas"*. Los `++` se quedan como
> la ironia: dicen que viene de C++ y de Rust sin tomar ningun nombre prestado.
>
> **Extension: `.titan`** (decidida el 29-09). La herramienta: `titan`. El
> manifiesto: `Titan.toml`. En rutas se escribe `TITAN` (los `+` dan guerra en
> un nombre de fichero).
>
> ** `.titan` es el **FUENTE**. Lo que sale es un **`.bex`** (BEF2), el mismo
> ejecutable que sale de C, C++, COBOL, Ada e INTI: TITAN++ **no reemplaza ni
> inventa** formato de ejecutable. El cargador, la firma y el DIRECTOR no se
> enteran de en que lenguaje se escribio (es lo que `PYTHON_MAESTRO` llama "BEF
> enmascara el lenguaje").
>
> **Las palabras: en INGLES, ultra simplificadas** (decidido el 29-09): el
> lenguaje, las ordenes de `titan` y las claves del `Titan.toml`. La PROSA de
> este documento y lo que sale en pantalla siguen en castellano sin tilde, como
> el resto de BMO-X.

Escrito el **2026-09-29**, antes de una sola linea de codigo, con el mismo
criterio que `INTI_MAESTRO.md` y `PYTHON_MAESTRO.md`: que esta conversacion no
haya que reconstruirla.

Lo que pidio el propietario, en sus palabras:

> *"ese Rust++ es para construir juegos, App y otros mas y si es para hablar
> con GPU y CPU ya le llama inti y otros los demas"*

> *"va a llevar INSPIRACION brutal de Borrow checker para facilitar ... para
> App, Juegos TODO E TODO"*

> *"es inspirado en C++ TOTAL porque hace TODO y TITAN++ me imagino que puede
> hacer MAS que C++ ... es con 'cargo run' pero el toml tomar inspiracion con
> mod, y todo organizado"*

> *"TITAN++ podria llevar herencias de COBOL tambien debido robustez en
> matematicas y FORTRAN con RUST en borrow checker"*

---

## 0. La respuesta corta

```text
   el ALCANCE de C++          hace TODO: juegos, apps, herramientas, calculo
   la SUPERFICIE de Python    pocas palabras (25), se lee de corrido
   la SEGURIDAD de Rust       un comprobador de prestamos, sin recolector
   los NUMEROS de COBOL       decimales exactos: el dinero no se redondea solo
   las TABLAS de FORTRAN      arreglos enteros como valores, y su velocidad
   y lo que SOLO BMO-X da     el comprobador conoce los prestamos del KERNEL
                              y de la 3060, y los permisos del programa
```

La ultima fila es la respuesta a *"puede hacer MAS que C++"*: C++ no sabe
nada del sistema en el que corre; TITAN++ nace sabiendo el de BMO-X (seccion 3).

```text
   TITAN++      lo que se CONSTRUYE: juegos, apps, herramientas, computo en la
      |         3060. `titan run`, `Titan.toml`, modulos ordenados, y F1 lo
      |         muestra como un GRAFO (4.5)
      v  llama a
   INTI         el lenguaje de la CPU y del sistema ("el C de BMO-X")
   VERRANO      dibujar con la 3060 (Ring 3)
   REX / bmo-userland    ventana, entrada, disco, sonido, red
      |
      v  INVOKE + WAIT (las dos puertas, congeladas)
   Rust base    BMO-X mismo: kernel, drivers, DIRECTOR. rustc, en el anfitrion
```

Cada capa tiene UN motivo, y por eso no se pisan:

```text
   Rust base   hace que la maquina FUNCIONE          lo escribe quien hace BMO-X
   INTI        HABLA con la CPU y el sistema          lo escribe quien toca el metal
   TITAN++     CONSTRUYE encima                       lo escribe quien hace apps y juegos
```

**El reparto ya existe en el mundo y funciono**: Apple escribe el sistema en C
y Objective-C y las apps en **Swift**, cuyo modelo de memoria es el de la
seccion 6.

---

## 1. Como se llego aqui (29-09, decisiones del propietario)

| pregunta | lo que se decidio | por que |
|---|---|---|
| el nombre | **TITAN++**, no "Rust++" | nombre propio, como INTI. "Rust" es marca de la Rust Foundation y su politica limita usarla en el nombre de otros lenguajes |
| la extension | **`.titan`** | decidido |
| las palabras | **ingles ultra simplificado**; la lista de 25 (4.4), ACEPTADA | decidido |
| que es | un **lenguaje nuevo** | ni una libreria sobre rustc ni un traductor a Rust |
| inspiracion | **C++ TOTAL en el alcance**; Python solo en el espiritu; Rust en el comprobador; **COBOL** en los numeros; **FORTRAN** en las tablas | "hace TODO", con pocas palabras (secciones 2 y 2b) |
| para que | apps con ventana, juegos, trabajo en la 3060, servicios, guiones, e IA | "TODO E TODO" |
| la frontera con INTI | **A**: INTI = CPU / kernel / sistema; TITAN++ = lo que INTI no hace | cada uno con su motivo |
| fundirlo con INTI | **NO** ("ni merga") | dos lenguajes, dos trabajos |
| las apps que ya estan en INTI | **se quedan** (NAVEGAR y las demas) | no hay que migrar nada |
| la herramienta | **tipo cargo**: `titan run`, `Titan.toml`, `mod` | "todo organizado" |
| el taller (F1) | **un editor de NODOS**: el prototipo es Ultra-Omega, del propietario | seccion 4.5 y `docs/plan/PLAN_TALLER.md` seccion 8 |

** Y lo que esto REABRE: `docs/METAS.md` tenia "La lista de lenguajes"
**CERRADA** desde el 17/18-09 por decision del propietario; el 29-09 la reabre
el mismo propietario para que entre TITAN++. La fila de METAS lo dice.

---

## 1b. Por que un lenguaje PROPIO: los extranjeros no se tocan

El propietario, el 29-09: *"el C, C++ y otros mas no tengo control y no puedo
modificar del TODO ... son lenguajes extranjeros para facilitar ... si quiero
Apps famosos no puedo usar esas bases"*.

Es el motivo de fondo, y lo explica todo lo demas:

```text
   un lenguaje EXTRANJERO   esta en BMO-X para recibir codigo de FUERA tal cual
                            (DOOM compila sin tocarlo; la banca, en COBOL)
                            -> si se le agregan las reglas de BMO-X, el codigo
                               de fuera deja de compilar, y se pierde justo
                               lo que daba. NO SE TOCA
   un lenguaje PROPIO       esta para crear codigo AQUI
                            -> puede saber de BMO-X: los prestamos, los
                               permisos, la 3060 (seccion 3). Solo se puede
                               porque es de la casa
```

`INTI_MAESTRO` ya lo decia de los cuatro frontends: *"estan ahi porque son de
otros"*. El reparto completo:

```text
   codigo de FUERA, para correr aqui    C, C++, COBOL, Ada, PROTON-X  (no se tocan)
   codigo de AQUI, para el sistema      INTI
   codigo de AQUI, para construir       TITAN++
```

[!] Y lo que esto NO promete: **las apps famosas de fuera** (juegos de
Windows, programas conocidos) no llegan por TITAN++; llegan por PROTON-X o
portando su fuente con C y C++. TITAN++ es para las **propias**: las que se
escriben para BMO-X y pueden llegar a ser las famosas de la casa.

---

## 2. C++ TOTAL: lo que se toma, lo que no, y como caben las dos cosas

"Inspirado en C++ TOTAL" y "pocas palabras" parecen chocar: C++ es de los
lenguajes MAS grandes que existen. No chocan si se separa **que puede hacer**
un lenguaje de **cuantas formas tiene de hacerlo**:

```text
   de C++ se toma el ALCANCE     lo que se puede construir con el: todo
   de C++ NO se toma la MEDIDA   cuatro decadas agregando sin quitar nada
```

### 2.1 Lo que TITAN++ toma de C++

| de C++ | en TITAN++ | por que |
|---|---|---|
| llega a todo: del motor al juego | igual | es la razon de la inspiracion |
| abstracciones sin coste: lo que no usas no lo pagas | igual, y el metro lo mide | un juego no puede pagar lo que no usa |
| RAII: se libera al salir del bloque | lo hace el PROPIETARIO del valor (seccion 6) | es la misma idea, con el comprobador detras |
| plantillas y `constexpr` | genericos + calculo al compilar | pero con mensajes que se leen (2.2) |
| sobrecarga de operadores | si | vectores y matrices para juegos: `a + b` |
| memoria a mano, sin recolector | si, pero segura | la parte de Rust |

### 2.2 Lo que NO toma, y la casa ya lo habia decidido para su frontend de C++

`toolchain/lang/cpp/BRECHA.md`: el frontend de C++ de BMO-X ya va **sin
excepciones ni RTTI a proposito**. TITAN++ sigue la misma linea:

```text
   comportamiento indefinido      NO: cero UB, las reglas de INTI (seccion 6.5 de
                                  INTI_MAESTRO)
   preprocesador y cabeceras      NO: modulos desde el primer dia (C++ tardo
                                  hasta C++20 en tenerlos y sigue con cabeceras)
   herencia y jerarquias          NO: composicion + interfaces (`trait`)
   excepciones                    NO: los errores son datos, como en INTI
   cinco formas de inicializar    NO: una
   muros de errores de plantilla  NO: un generico dice que le falta, en una frase
```

---

## 2b. COBOL y FORTRAN: lo que se HEREDA (ideas, no sintaxis)

El propietario, el 29-09: *"herencias de COBOL tambien debido robustez en
matematicas y FORTRAN con RUST en borrow checker"*.

Tiene sentido, y por una razon precisa: cada uno de los dos lleva DECADAS
resolviendo UN problema mejor que nadie. Se hereda la **solucion**, no la
sintaxis (COBOL tiene cientos de palabras reservadas; el techo de TITAN++ es 30).
Y ninguno de los dos cuesta una palabra nueva: los dos entran como **TIPOS**.

### 2b.1 De COBOL: el numero que no miente

```text
   el problema     0.1 + 0.2 = 0.30000000000000004 en coma flotante binaria.
                   Con dinero eso es un error, no un redondeo
   lo que COBOL    decimales de PUNTO FIJO: PIC 9(7)V99 son 7 enteros y 2
   resolvio        decimales EXACTOS; y ON SIZE ERROR cuando no cabe
   en TITAN++      un tipo `dec(9, 2)`: 9 digitos, 2 decimales, exacto
                   desbordar NO trunca: es un error (como INTI y Ada)
                   redondear es una llamada con NOMBRE: round(x, 2, half_even)
```

** Y esto ya lo pago INTI: `INTI_MAESTRO` 10.3 *"un solo `numero`, y exacto
por defecto"*, con la ley de los 64 bits (10.3b). **TITAN++ usa la MISMA
aritmetica decimal que INTI**, no otra: un `dec` que pasa de un lenguaje al
otro vale lo mismo en los dos. La banca sigue siendo COBOL (`PLAN_BANCA.md`, el
lenguaje extranjero se queda); lo que TITAN++ hereda es que sus apps (una
tienda, una hoja de cuentas) no pierdan un centimo.

### 2b.2 De FORTRAN: la tabla entera como valor, y su velocidad

```text
   el problema     calcular sobre millones de numeros sin escribir el bucle
                   a mano, y que vaya a la velocidad del silicio
   lo que FORTRAN  1. arreglos de VARIAS dimensiones como valores:
   resolvio           A = B + C * 2.0 suma tablas enteras
                   2. funciones ELEMENTALES: sqrt(A) se aplica a cada celda
                   3. la regla de oro: los argumentos de una funcion NO SE
                      SOLAPAN en memoria. Por eso el compilador puede ir
                      mas rapido que en C, donde dos punteros pueden ser
                      el mismo
   en TITAN++      [f32; 1024] y [f32; n, m] como valores; A = B + C * 2.0;
                   una funcion sin permisos (U2) y sin `mut` es ELEMENTAL
                   sola: nadie la marca
```

*** **La conexion que hace buena la idea del propietario: la regla de oro de
FORTRAN ES la ley de exclusividad del borrow checker.** FORTRAN la tiene como
PROMESA del programador (si la rompe, el resultado es indefinido); TITAN++ la
tiene DEMOSTRADA por el compilador (seccion 6). Un parametro `mut` no se solapa
con nada: el emisor puede tratar el bucle como FORTRAN lo trata, y no hace
falta ninguna palabra `restrict` de C. "FORTRAN con Rust en borrow checker" es
exactamente eso: **la velocidad de FORTRAN, con la prueba de Rust**.

Y la segunda conexion: una funcion ELEMENTAL sobre una tabla es justo lo que
corre en la 3060. `gpu fn` aplicada a `[f32; n]` es un trabajo de computo de
`n` hilos (seccion 8).

[!] Lo que NO se hereda de FORTRAN: `GOTO`, los `COMMON` (memoria global
compartida), `EQUIVALENCE` (dos nombres para la misma memoria: rompe la
exclusividad) y la columna fija. Y la vectorizacion SIMD no es automatica
(`INTI_MAESTRO` 13.7): `A = B + C` baja a un bucle explicito o a la 3060, y el
metro dice cuanto cuesta.

---

## 3. LO UNICO: lo que ni C++ ni Rust pueden hacer, porque no tienen BMO-X debajo

C++ y Rust son lenguajes **para cualquier sistema**, y por eso no saben nada
del suyo. TITAN++ es **solo para BMO-X**, y eso deja hacer cuatro cosas que
ellos no pueden. Las cuatro salen de piezas que YA existen en la casa.

### U1. El prestamo no acaba en el programa: UN comprobador, TRES fronteras

En BMO-X la memoria **se presta**: el DIRECTOR le OFRECE una lamina a NAVEGAR
(`MEM_OP_OFRECER`) y NAVEGAR la toma; una peticion se devuelve al soltarla
(`request` PRESTADA con `Drop`); a la 3060 se le presta RAM del PC por la IOMMU.
Es el mismo modelo que el borrow checker, pero en el KERNEL.

```text
   dentro del programa    lo que hace Rust
   entre programas        los prestamos del kernel (ofrecer / tomar / devolver)
   con la 3060            un bufer prestado a la GPU hasta el WAIT
```

TITAN++ puede ser **el unico lenguaje cuyo comprobador conoce las tres**.
Boceto (NO gramatica):

```text
   buf = gpu.buffer(1024)
   job = gpu.launch(add, mut buf)     el bufer queda PRESTADO a la 3060
   print(buf[0])                      NO compila: la 3060 aun lo tiene
   wait(job)
   print(buf[0])                      bien
```

En C++ con CUDA ese error sale en ejecucion, a veces, con basura en pantalla.
En Rust hace falta `unsafe` o una biblioteca de fuera. En TITAN++ es un error de
compilacion con sitio y motivo, porque el lenguaje sabe como presta BMO-X.

### U2. Los permisos son parte del tipo

Un programa de BMO-X habla por capabilities, y el `.bex` declara lo que pide.
En TITAN++ los permisos del `Titan.toml` (seccion 4) van a DOS sitios:

```text
   al .bex         el kernel concede o niega al lanzarlo   (como hoy)
   al compilador   una funcion que toca la red en un paquete que no pidio
                   `net` NO COMPILA
```

Antes de ejecutar un juego se sabe **exactamente** que puede tocar: si no pidio
`net`, no hay una linea en el que pueda hablar con la red, y eso lo demostro el
compilador, no una promesa. C++ no puede: no sabe que es un permiso. Hay
lenguajes de investigacion con capacidades; ninguno tiene debajo un kernel que
las haga cumplir. (Y de regalo: una funcion que no usa NINGUN permiso ni `mut`
es pura, que es lo que la hace elemental en 2b.2.)

### U3. MODULAR dentro del compilador

La regla principal de la casa (MODULAR, y la ley L8: la dependencia solo baja)
hoy la hacen cumplir guardianes de Python sobre el Rust. En TITAN++ la cumple
el compilador:

```text
   cada modulo dice en UNA linea que hace          sin ella, no compila
   cada modulo dice con que conecta (`use`)        y el compilador lo compara
                                                   con lo que de verdad llama
   las capas solo bajan                            un ciclo es un error
```

Un juego de TITAN++ **no se puede volver un monolito sin que el compilador lo
diga**. Y ese mismo grafo de `use` es el que F1 dibuja (4.5).

### U4. El `.bex` lleva su fuente, y F1 lo abre

La idea ya estaba en `INTI_MAESTRO` 13g.5: la seccion `Resources` (0x0B) del
formato BEF puede llevar el fuente que produjo el `.bex`. En TITAN++, con
`source = true` en el manifiesto:

```text
   lo que corre es lo que puedes leer    y esta firmado
   se comprueba recompilando             el mismo fuente da los mismos bits
   F1 abre cualquier programa TITAN++    lo lees, lo cambias, lo vuelves a construir
```

---

## 4. La herramienta: `titan`, `Titan.toml`, los modulos, las palabras y F1

### 4.1 Las ordenes (como cargo)

```text
   titan new asteroids     un paquete nuevo, con su Titan.toml y su src/
   titan check             tipos + comprobador, sin emitir nada (rapido)
   titan build             el .bex
   titan run               construir y lanzar
   titan test              las pruebas del paquete
```

[!] **`titan run` choca con una regla de F1**, y hay que decirlo ahora:
`PLAN_TALLER.md` seccion 3 -- un `.bex` **no puede lanzar otro** (`EJECUTAR`
se fija al nacer y solo desde Ring 0). Dentro de F1, `titan run` construye y le
pide al ESCRITORIO que lo lance; el que da el clic es el propietario. En el
anfitrion (Windows), `titan run` no existe: alli solo hay `build`.

### 4.2 El manifiesto: `Titan.toml`

Boceto (las claves en ingles, como el lenguaje):

```toml
[package]
name    = "asteroids"
version = "0.1.0"
edition = "2026"
source  = true            # U4: el .bex lleva su fuente

[permissions]             # U2: al .bex Y al compilador
screen = true
input  = true
sound  = true
gpu    = "draw"           # "draw" | "compute"
disk   = "read"           # solo su carpeta
net    = false

[dependencies]
physics = { path = "../physics" }
engine  = { path = "../engine", hash = "sha256:..." }   # si cambia, no compila

[profile.fast]
opt = 3
```

** **Sin registro de internet**, a proposito. Las dependencias van por RUTA, y
con HUELLA si son de otro. La casa cerro Ring 0 a externos por el caso xz (una
dependencia envenenada); TITAN++ no abre esa puerta por el lado de las apps:
lo que entra en un paquete se ve en su manifiesto y su huella se comprueba.

### 4.3 Los modulos: una sola forma

Rust tiene dos formas de poner un modulo en carpetas (`mod.rs` o `nombre.rs`
junto a `nombre/`), y confunde. TITAN++ tiene UNA:

```text
   asteroids/
      Titan.toml
      src/
         main.titan        mod ship, rock, physics
         ship.titan
         rock.titan
         physics.titan     mod collide         (physics es carpeta Y fichero)
         physics/
            collide.titan
```

Y cada fichero empieza diciendo que es y con que conecta (U3):

```text
   mod physics "moves bodies and resolves collisions"
   use ship, rock
```

### 4.4 Las 25 palabras (ACEPTADAS el 29-09)

```text
   funciones y valores    fn  let  mut  take  return
   decidir y repetir      if  else  match  for  in  while  break  continue
   formas                 type  enum  trait
   modulos                mod  use  pub
   la 3060                gpu
   logica                 true  false  and  or  not
```

| palabra | que hace |
|---|---|
| `fn` | una funcion |
| `let` | nombra un valor; no cambia salvo que diga `mut` |
| `mut` | se puede cambiar: un nombre (`let mut x`) o un parametro prestado para cambiarlo (seccion 6) |
| `take` | el parametro SE QUEDA el valor: el que llama ya no lo tiene |
| `return` | devuelve |
| `if` `else` `match` | decidir; `match` exige cubrir todos los casos |
| `for` `in` `while` `break` `continue` | repetir |
| `type` | un registro con campos (y, si se pide, su forma exacta en bytes) |
| `enum` | uno de varios casos, cada uno con sus datos |
| `trait` | lo que una forma sabe hacer (sustituye a la herencia) |
| `mod` `use` `pub` | modulos: cual es, con que conecta, que se ve desde fuera |
| `gpu` | la funcion corre en la 3060 (seccion 8) |
| `true` `false` `and` `or` `not` | logica, con palabras y no con `&&` `||` `!` |

**El techo: 30.** Quedan 5 libres a proposito. Una palabra nueva entra solo si
quita una confusion, y un guardian las cuenta (seccion 11). `dec`, `f32`,
`[f32; n, m]` y demas son TIPOS, no palabras: no gastan techo. `wait`, `print`
y `round` son funciones de la biblioteca.

### 4.5 F1: el taller como GRAFO de nodos

El propietario, el 29-09, con una captura de **Ultra-Omega** -- un editor de
nodos suyo, en Rust y Vulkan, en Windows: *"ESTE ES EL F1 que voy a implementar
en mi ESTRUCTURA ... asi tienen que ir para facilitar en TITAN++ con todo y
organizar"*.

Tiene sentido, y encaja mejor de lo que parece, por U3: **un programa TITAN++
YA ES un grafo** -- sus modulos son los nodos y sus `use` son las aristas, y el
compilador exige que las aristas solo bajen. El editor de nodos no inventa una
estructura: **dibuja la que el compilador ya comprueba**.

```text
   en la captura                  en TITAN++
   un nodo con cabecera RUST      un MODULO; la cabecera dice el lenguaje
                                  (TITAN, INTI, C...): mezclar se ve
   el titulo "Hola Mundo"         la linea que dice que hace (U3)
   `source` dentro del nodo       el texto del .titan
   los puertos in / out           out = sus `use`; in = quien lo usa
   el EXPLORER (nodes/)           la carpeta del paquete TAL CUAL esta en el
                                  disco, en el orden del propietario (8.10)
   F5 Run                         titan build (y pedir al ESCRITORIO lanzar)
   Tab Templates                  titan new con plantillas
   Ctrl+Shift+P Commands          la consola del TALLER
```

El plan del taller es de `docs/plan/PLAN_TALLER.md`, seccion 8, con sus reglas
(la verdad es el texto, las aristas son dependencias y no flujo, F1 es opcional).

---

## 5. Por que un compilador PROPIO: por F1

`docs/plan/PLAN_TALLER.md`: se pulsa **F1** en el escritorio y se abre **el
taller**, una app que es terminal Y compilador en UN `.bex`.

Si TITAN++ se tiene que compilar DENTRO de F1, **rustc no puede ser su
compilador**:

```text
   rustc + LLVM    decenas de megas, necesitan el std de un anfitrion,
                   no caben en un .bex de Ring 3
   TITAN++         un frontend de BMO-X + el emisor-x86_64 que ya compila
                   C, C++, COBOL, Ada e INTI
```

No es una preferencia: es lo que F1 obliga. **rustc y cargo SI sirven de
inspiracion** (seccion 6.8): la arquitectura y la experiencia se imitan; su
medida no.

**Lo que F1 pide antes**, y hoy no esta:

```text
   la ventana de F1               PLAN_TALLER, casilla 1 (ABIERTA)
   el autohospedaje               PLAN_AUTOHOSPEDAJE (APARCADO)
   memoria dinamica en Ring 3     un asignador compartido: desde el 03-10
                                  existe `bmo-monton` y lo usan PROTON-X,
                                  HERMES, BANK CAT y LUDOTECA (corregido el
                                  04-10: aqui decia "solo PROTON-X")
```

Hasta entonces el compilador corre en el anfitrion, como todos los demas hoy.

---

## 6. EL CORAZON: el borrow checker, inspirado y no copiado

### 6.1 Que es, y por que TITAN++ lo necesita

La parte del compilador que **demuestra, antes de ejecutar**:

```text
   nadie usa un dato despues de que se libero
   nadie escribe un dato mientras otro lo esta leyendo
```

Es lo que hace a Rust seguro **sin recolector de basura**. Sin el, TITAN++
seria otro C con otra ropa.

### 6.2 Por que el de Rust es pesado

Razona sobre **tiempos de vida** (`'a`) que cruzan de una funcion a otra, y los
tiempos de vida **se ven en la sintaxis**: `&'a mut T`. Lo contrario de "pocas
palabras". La medida exacta esta en 6.8.

**Lo dificil de Rust no son sus palabras, son sus reglas.** Las reglas se
pueden esconder; no se pueden borrar:

```text
   que el compilador las DEDUZCA   -> el lenguaje renuncia a guardar referencias
   que se MUESTREN con palabras    -> ya no son "pocas"
```

### 6.3 Los tres modelos

| | modelo | lo que escribe el programador | el comprobador |
|---|---|---|---|
| 1 | Rust tal cual: prestamos con tiempos de vida | `&`, `&mut`, `'a` | enorme; razona entre funciones |
| **2** | **valores mutables** (Swift, Hylo) | cada parametro dice si **lee**, **cambia** (`mut`) o **se queda** (`take`) el valor | **local, funcion a funcion** |
| 3 | solo mover y copiar | nada | el mas chico: "no uses lo que ya entregaste" |

### 6.4 La eleccion: el modelo 2, construido desde el 3

```text
   paso A (modelo 3)   "ya lo entregaste": un valor que se mueve no se vuelve
                       a usar
   paso B (modelo 2)   LA LEY DE EXCLUSIVIDAD: mientras algo se CAMBIA, nadie
                       mas lo lee ni lo cambia
   paso C (U1)         la misma ley, extendida a los prestamos del kernel y
                       de la 3060
```

Los modos de un parametro:

```text
   (nada)   LEE: el que llama sigue siendo el propietario      (por defecto)
   mut      CAMBIA: se presta para cambiarlo; nadie mas lo toca mientras dura
   take     SE QUEDA: el valor pasa a ser de la funcion
```

Boceto (NO gramatica):

```text
   fn step(mut player, map)
       player.x = player.x + 1
       if map.solid(player.x, player.y)
           player.x = player.x - 1

   step(hero, level)       bien
   step(hero, hero.map)    NO: `hero` se CAMBIA y a la vez se LEE su mapa
```

Sin un `&`, sin un `'a`: dos palabras (`mut`, `take`) hacen el trabajo de
`&`, `&mut`, `'a` y `move`.

### 6.5 Lo que se pierde, dicho ahora

**No se guardan referencias dentro de estructuras.** Un grafo se escribe con
**indices**. Es el precio real del modelo 2, y para juegos es una ventaja: los
motores rapidos de hoy (ECS: entidades que son indices, componentes en tablas)
ya se escriben asi.

### 6.6 Los mensajes

Un comprobador que dice "no" sin porque es el que hace odiar a Rust. TITAN++
hereda de INTI el **formato de cuatro partes**: que paso, donde, por que es un
error, y como se arregla. Cada regla del comprobador entra con su programa roto
de prueba, como el juez del SASS. (6.8 dice por que esto es la mitad del
trabajo, medido.)

### 6.7 Los precedentes

```text
   Swift   inout + la ley de exclusividad (SE-0176), en produccion desde hace
           casi una decada
   Hylo    "mutable value semantics": el modelo 2 llevado al extremo
   Mojo    sintaxis tipo Python + propiedad + GPU, pero con muchas palabras
   Rust    el origen de todo. Modelo 1
```

### 6.8 Como se hizo el de rustc (investigado el 29-09)

**1. Primero sobre el arbol, y fallo durante mucho tiempo.**
Hasta la edicion 2018, un prestamo duraba lo que el bloque `{ }` que lo
contenia (tiempos de vida "lexicos"). Eso rechazaba programas correctos: la RFC
2094 (NLL, agosto de 2017) enumera los casos -- una referencia guardada en una
variable, un prestamo en una rama de un `match` que bloqueaba las demas, un
prestamo devuelto en una sola rama, y reasignar un `&mut`.

**2. La solucion fue una representacion intermedia nueva: MIR.** Un grafo de
bloques basicos, *"far less complex than the HIR"* (la guia de rustc). Con NLL,
un tiempo de vida paso a ser **un conjunto de puntos del grafo**, calculado con
restricciones de "sigue vivo aqui" y "este dura mas que aquel", iterando hasta
que nada cambia.

**3. Los siete pasos de hoy** (guia de rustc, `mir_borrowck`):

```text
   1  copia local del MIR
   2  regiones nuevas en todo el MIR (replace_regions_in_mir)
   3  flujo de datos: que se movio y cuando
   4  segundo chequeo de tipos, que genera restricciones entre regiones
   5  inferencia de regiones: donde tiene que valer cada tiempo de vida
   6  que prestamos estan vivos en cada punto
   7  recorrido final: cada accion contra los prestamos vivos, y los errores
```

**4. El caso dificil tardo de 2018 a 2026.** Devolver
un prestamo en UNA sola rama (el "caso 3", un `get_mut` o valor por defecto en
un `HashMap`) no lo resolvio NLL. Lo resuelve **Polonius alfa**, activado en
nightly el 04-08-2026: sensible al flujo, acepta todo lo de NLL y mas, cuesta
10-20 % de compilacion en algunos casos (el peor, 2-3x), y quieren
estabilizarlo antes de fin de 2026.

**5. Lo que mide, contado el 29-09** en `compiler/rustc_borrowck/src` por la API
de GitHub:

```text
   63 ficheros, 1.533.464 bytes
   diagnostics     640 KB   42 %   <- EXPLICAR el error
   (raiz)          345 KB   22 %
   type_check      262 KB   17 %   \
   region_infer    168 KB   11 %    > las REGIONES: 36 %
   polonius        107 KB    7 %   /
   constraints      12 KB    1 %
```

*** **Casi la mitad del borrow checker de Rust es explicar por que dice NO. Y
un tercio existe solo porque hay referencias guardadas con tiempos de vida.**

**6. Lo que TITAN++ saca de ahi:**

```text
   L1  comprobar sobre una IR PROPIA con grafo de bloques, no sobre el arbol
       (la leccion de rustc). La casa prohibe una IR compartida entre lenguajes
       (toolchain/lang/cobol/cobol.md: "un cerebro compartido"): TITAN++ tiene
       la suya, como INTI tiene su ir/
   L2  con el modelo 2 DESAPARECE el 36 % de regiones: sin referencias
       guardadas no hay tiempos de vida que deducir. Quedan el flujo de "ya lo
       entregaste" y el choque de accesos, funcion por funcion
   L3  el caso 3 que tardo de 2018 a 2026 NO EXISTE: una funcion no puede
       devolver un prestamo; devuelve un valor o un indice
   L4  los mensajes son el 42 %: el formato de cuatro partes va desde T1, y
       cada regla con su programa roto
   L5  una regla de ORDEN escrita: lo que solo se LEE se evalua antes de que
       empiece un `mut`. Es `v.push(v.len())`, que Rust tuvo que parchear con
       los "prestamos en dos fases"
```

### 6.9 Los esteroides: el comprobador que protege a BMO-X

El mismo mecanismo -- en la IR, un acceso EMPIEZA y ACABA -- es el que Swift usa
para la exclusividad local. TITAN++ lo lleva mas lejos, porque sabe de BMO-X:

```text
   acceso local        empieza en la llamada con `mut`, acaba al volver
   prestamo al kernel  empieza en `offer`, acaba al devolver     (U1)
   prestamo a la 3060  empieza en `gpu.launch`, acaba en `wait`  (U1)
   un permiso          se comprueba en cada llamada a la puerta  (U2)
   un asa del kernel   (una ventana, un fichero) es un valor con propietario:
                       se devuelve UNA vez, al acabar su propietario, y no
                       se puede usar despues
```

Lo que BMO-X hoy protege en ejecucion (el kernel dice NO con motivo), TITAN++
lo protege ANTES: el programa que lo romperia no llega a compilarse.

---

## 6b. LOS DOS JUECES: el compilador y el kernel COOPERAN (29-09)

> *"mi Kernel con su burocracia y el borrow checker es que los 2 se cooperan
> para facilitar por completo"* -- el propietario, 29-09.

> *"BMO-X por algo es que es las 4 patas y TITAN++ es el torso"*

### 6b.1 La pregunta, y por que la respuesta es NO

El propietario pregunto si TITAN++ necesita compilador, si el kernel -- el
orquestador soberano -- ya dice si algo esta validado o no. **Si lo necesita**,
y por dos motivos que no dependen de gustos:

1. **Alguien tiene que convertir el texto en instrucciones.** Un `.titan` es
   texto y la CPU solo ejecuta codigo maquina. Sin compilador, alguien lo
   interpreta mientras corre. Si ese interprete vive en el kernel, es justo lo
   que BMO-X ya prohibio: *AML nunca en Ring 0* ("un interprete de bytecode en
   Ring 0 es lo que paga un generalista") y Ring 0 cerrado. Si vive en Ring 3,
   sigue siendo un compilador, solo que mas lento.
2. **El kernel no ve dentro del programa.** Dos `mut` al mismo valor, un
   prestamo que dura mas que su propietario, un `take` que se reusa: todo eso
   pasa en la memoria de la app **sin una sola llamada al sistema**. El kernel
   ve las PUERTAS (INVOKE y WAIT), no las habitaciones. No puede validar lo que
   no ve.

Asi que la idea buena no es "sin compilador": es **dos jueces, cada uno donde
ve**.

### 6b.2 Lo que juzga cada uno

```text
   EL COMPILADOR (antes de correr)          EL KERNEL SOBERANO (mientras corre)
   DENTRO del programa                      ENTRE programas, y con la maquina

   dos `mut` a la vez                       de quien es cada bloque (capabilities)
   un prestamo que se escapa                un prestamo entre procesos que vuelve
   un `take` que se reusa                   (MEM_OP_OFRECER, `request` con Drop)
   un permiso que no se pidio (U2)          que permisos se conceden DE VERDAD
   el prestamo a la 3060 antes del `wait`   W^X (sellar), la firma, la puerta BEF2
   (U1, en la IR)                           la RAM, la pila, el foco, los vatios
```

Ninguno sustituye al otro. El compilador **promete** lo de dentro; el kernel
**hace cumplir** lo de fuera, y lo hace igual para un `.bex` de TITAN++ que
para uno de C: su burocracia -- cada puerta pide sus papeles -- no se relaja
porque el programa diga que es bueno.

[!] Y esto no es futuro: **ya pasa**.
Cuando F1 ofrece su ventana al DIRECTOR, el kernel es un borrow checker ENTRE
procesos: presta el bloque, sabe quien lo tiene y avisa cuando vuelve. La U1
era exactamente esto -- que el comprobador de TITAN++ conozca los prestamos del
kernel, para que los dos jueces hablen el mismo idioma.

### 6b.3 Como cooperan: el CERTIFICADO que viaja en el `.bex`

El compilador deja en el `.bex` (un anexo de BEF2, como el del sombreador) un
**certificado corto**: por cada modulo, que permisos usa (U2), que prestamos
del kernel hace (U1: ofrecer, tomar, lanzar a la 3060) y en que LINEA. Nada del
cuerpo del programa: nombres, puertas y lineas.

Y la puerta de carga -- que ya valida el perfil que viaja DENTRO del `.bex`
(P1) -- compara tres cosas que cuestan una resta cada una:

```text
   lo que el certificado dice que usa
   lo que el Titan.toml pidio            ->  si no cuadran: NO, y dice cual
   lo que el proceso recibe de verdad
```

Es la idea del **codigo con su prueba** (proof-carrying code, Necula, 1996):
**compilar es caro, comprobar es barato**. El compilador hace el trabajo caro
una vez, en el PC o en F1; la puerta hace el barato cada vez que carga.

*** LO QUE EL KERNEL NUNCA HACE CON EL CERTIFICADO: creerselo para CONCEDER.
Un permiso lo da SIEMPRE la capability, como hoy; un `.bex` de C, que no trae
certificado, corre exactamente igual. El certificado sirve para **NOMBRAR**:
cuando el kernel dice NO en una puerta, el certificado dice que modulo y que
linea la abrio. Si un certificado miente, lo unico que se rompe es la
explicacion -- nunca la seguridad. Y el kernel tampoco vuelve a correr el
borrow checker: eso seria un comprobador de tipos en Ring 0, lo mismo que el
punto 1 de 6b.1 prohibe.

### 6b.4 El camino de un NO hasta el nodo: F1 y CABINA

Los dos jueces dicen NO, y en los dos casos el NO **acaba en un nodo**. El
taller no hace buscar: lleva la camara al nodo que fallo, lo pinta en rojo y
pone el mensaje de cuatro partes, para que nadie pierda el tiempo:

```text
   NO del COMPILADOR                         NO del KERNEL
   el comprobador -> diag de 4 partes        una puerta dice NO
          |                                  -> evento en CABINA (Process /
          |                                     Syscall, con su valor)
          |                                  -> F1 lo lee (TASK_OP_CABINA_*)
          |                                  -> el CERTIFICADO dice que modulo
          |                                     y que linea abrio esa puerta
          v                                         v
                     F1: la camara va AL NODO, rojo, y abajo
                     QUE / DONDE / POR QUE / COMO
                     (el QUE del kernel, el DONDE del certificado)

   y CABINA (F11) expone la MISMA linea para quien mire la maquina entera
```

F1 ya sabe hacer la mitad de la izquierda: B3 anima el `Conflict` y el
`Denied` del ejemplo con su mensaje (PLAN_TALLER 8.5), y L2 ya centra la
camara en un nodo al pulsar su fichero (8.7). CABINA ya graba los NO del
kernel y Ring 3 ya puede leer su anillo. Lo que no existe es el puente.

### 6b.5 Lo que falta, en orden

```text
   J0  HOY: F1 anima los NO del ejemplo; CABINA graba los del kernel; Ring 3
       lee el anillo; el `.bex` llevara sus [permissions] en T3 (U2)
   J1  el CERTIFICADO: su forma en un anexo de BEF2 (va con T3, cuando
       `titan build` escriba el primer `.bex`)
   J2  la puerta de carga lo COMPARA con el Titan.toml y con lo concedido;
       si no cuadra, NO con el nombre de lo que sobra (Ring 0, y chico)
   J3  el NO del kernel lleva, como DATO y no solo como prosa, QUE puerta y
       QUE proceso -- mirar primero si el evento de CABINA ya basta
   J4  F1 lee ese NO, busca en el certificado, y lleva la camara al nodo
```

Sin fechas (LEY 24). Y el orden importa: J2 antes que J4, porque un NO que
llega a un nodo sin que la puerta lo haya comprobado seria una explicacion sin
juicio detras.

**Donde esta, el 04-10:**

```text
   J1  HECHO en el anfitrion. `bmo_titan_contrato::certificate`: la forma,
       quien la escribe y quien la lee, `no_std` y sin monton. El compilador la
       saca de la IR que EL JUEZ ya juzgo (`manifest::certificate`) y viaja en
       el MANIFIESTO del .bex, como `[certificado]`, firmada con el codigo. Un
       anexo propio de BEF2 es cambiar el formato (Ring 0, del propietario):
       hasta entonces va ahi, que el cargador ya salta. Hasta 16 lineas por
       puerta, las primeras, y las demas se cuentan: el kernel lo lee en cada
       carga y no puede crecer con el programa
   J2  EL JUICIO, ESCRITO Y PROBADO; FALTA CABLEARLO. `certificate::judge`
       compara certificado / pedido / concedido y solo sabe decir "de
       acuerdo" o NO con puerta y linea -- nunca concede. `titan juez X.bex
       [--concede ...]` lo hace HOY en el PC, y una prueba le pasa un .bex
       FALSIFICADO (dice usar la red sin pedirla) y lo caza. Llevarlo a la
       puerta de carga es una llamada en Ring 0: del propietario
   J3-J4  sin empezar
```

### 6b.6 El centauro

Las PATAS (BMO-X) deciden donde se puede pisar: que memoria es de quien, que
puertas se abren, cuanta energia se gasta. Nadie cruza a terreno ajeno, y eso
no se negocia. El TORSO (TITAN++) decide que hacen las manos: que valor se
presta, a quien, hasta cuando. Las patas no ven lo que llevan las manos, y el
torso no decide el terreno. Por eso ninguno sustituye al otro, y por eso
corren juntos.

### 6b.7 El reparto en una frase (30-09)

> *"TITAN++ sera la preparacion de Borrow checker con BMO-X ... el borrow
> checker hace checkeo rapido y el kernel le dice si o no"* -- el propietario.

Asi es, con UNA precision para que no haya sorpresas:

```text
   el borrow checker PREPARA      comprueba ANTES, una vez: no cuesta nada
                                  mientras corre. Y empaqueta lo que vio (el
                                  certificado, 6b.3)
   el kernel DECIDE               en cada puerta, SI o NO: una resta contra
                                  lo concedido. Su SI es el ULTIMO
```

La precision: **los dos NO valen solos, y ningun SI obliga al otro.**

- Un NO del borrow checker (dos `mut` a la vez dentro del programa) es
  FINAL: el programa no llega a existir, y el kernel no podria anularlo
  aunque quisiera -- no lo ve.
- Un SI del borrow checker es NECESARIO pero no SUFICIENTE: el kernel sigue
  diciendo NO si la puerta no esta concedida, con certificado o sin el. Un
  `.bex` que dice "soy bueno" no abre ninguna puerta; la abre la capability.

Cooperan porque cada uno hace lo que al otro le sale caro: el comprobador,
lo de dentro, antes y una vez; el kernel, lo de fuera, siempre y barato. Y
los dos acaban en el mismo sitio cuando dicen que no: el nodo en F1 (6b.4).

### 6b.8 El proceso, paso a paso (30-09)

El propietario pregunto como iria el borrow checker hasta el kernel: *"es un
archivo fantasma o algo? ... seria mejor JIT para que mi kernel sepa y luego
sorpresa verifique?"*

```text
   titan build  (en el PC o dentro de F1)
     texto -> arbol -> tipos -> IR -> BORROW CHECKER
        |  NO -> se para: mensaje de 4 partes, el nodo en rojo en F1.
        |        No sale ningun .bex: ese error no llega NUNCA al kernel
        |  SI -> el emisor x86-64 escribe el codigo
        v
   el .bex = codigo + [permissions] + CERTIFICADO + fuente (U4), FIRMADO
        |
   al CARGAR: la puerta BEF2 (Ring 0) lee el certificado y lo compara con
        |     el Titan.toml y con lo que concede -- una resta cada cosa
        v
   al CORRER: cada puerta pide su capability, como hoy, a TODO programa
```

- **El "archivo fantasma" existe, pero va DENTRO.** El certificado es un anexo
  del `.bex`, firmado con el codigo. Fuera seria un fichero que se pierde, se
  queda viejo o se cambia por otro; dentro y firmado, codigo y certificado
  viajan juntos y no se separan. Nadie lo ve ni lo toca: por eso "fantasma".
- **JIT para la CPU: NO**, y ya estaba dicho (seccion 10): codigo que se
  escribe en marcha es codigo que el juez no puede ver. Y no le daria nada al
  kernel: con JIT sigue viendo puertas, no habitaciones; el JIT cambia
  CUANDO se compila, no QUE ve el kernel.
- **La "sorpresa" ya existe**: el kernel comprueba TODAS las puertas, en cada
  llamada. El programa no puede saber cual se mira, porque se miran todas.
- **El numero ya esta medido en la casa**: el sombreador abre un formato ya
  traducido y comprobado (BSF, hash al TOMAR) en 4,5 us, y el JIT tarda 17,2
  us. Compilar antes y comprobar al tomar gano 4x. El certificado es lo mismo
  llevado al `.bex` entero.
- **La unica excepcion posible es la 3060**: una `gpu fn` puede viajar en el
  `.bex` como SPIR-V y terminarse en SASS con el PERFIL de la 3060 que la va a
  correr (LEY 24). Si se hace, es por el camino del sombreador (formato
  comprobado, hash al tomar), no un JIT del programa. Hoy el plan es que SASS
  salga al construir (T5).

---

## 7. A quien llama TITAN++, y como

TITAN++ **no toca hardware**:

| para | llama a | por |
|---|---|---|
| CPU y sistema | **INTI** | compilacion separada: `.bo` + `bmo-enlazar` (HECHA para C, C++ e INTI) |
| dibujar con la 3060 | **VERRANO** | la API de dibujo de Ring 3 |
| ventana, teclado, raton, disco, sonido, red | **REX / bmo-userland** | los dos syscalls |

[!] INTI todavia no declara funciones AJENAS (`externo`, en ESPERA en METAS).
Para que TITAN++ llame a INTI basta lo que ya hay; para que INTI llame a
TITAN++, falta esa palabra.

---

## 8. La 3060: computo masivo

```text
   HECHO   SPIR-V -> SASS con juez (PLAN_LA_LENGUA_DE_LA_3060, E1-E5)
   HECHO   dibujar: VERRANO, el cubo bit a bit con D3D12
   FALTA   LANZAR computo: la QMD y el banco constante 0 (el mapa medido esta en
           platform/drivers/gpu/ga10x/COMO_LE_HABLA_NVIDIA.md, 3d)
```

Las funciones `gpu fn` de TITAN++ **bajan al subconjunto de SPIR-V** que la casa
ya lleva a SASS: se reutilizan emisor y juez enteros. Una `gpu fn` elemental
(2b.2) aplicada a una tabla de `n` celdas es un trabajo de `n` hilos. Y el
prestamo del bufer es la U1.

---

## 9. IA: los dos sentidos

```text
   que una IA ESCRIBA TITAN++   25 palabras + mensajes exactos + UNA forma de
                                hacer cada cosa = menos errores al generarlo
   cargas de IA EN la 3060      tensores: tablas de FORTRAN (2b.2) en la 3060
                                (seccion 8), y depende de lanzar computo
```

El asistente de IA dentro de BMO-X sigue **APARCADO** (METAS cat. 2).

---

## 10. Lo que TITAN++ NO es

```text
   NO es Python          no corre programas de Python (eso es PYTHON_MAESTRO)
   NO es Rust            no compila crates de crates.io ni usa rustc
   NO es C++             no lee C++ (para eso esta el frontend de C++)
   NO es COBOL ni FORTRAN  hereda sus soluciones, no su sintaxis (2b)
   NO reemplaza al .bex  .titan es el fuente; lo que corre es un .bex (BEF2)
   NO escribe el kernel  el kernel y los drivers son la Rust base; el metal, INTI
   NO sustituye a INTI   INTI y sus apps se quedan
   NO tiene recolector   la memoria se libera cuando su propietario acaba
   NO guarda referencias en estructuras (6.5)
   NO tiene JIT          codigo que se escribe en marcha es codigo que el juez
                         no puede ver
   NO baja de internet   dependencias por ruta y con huella (4.2)
   NO es multiarquitectura  x86-64 en la CPU y sm_86 en la 3060
   NO sale de BMO-X      EXCLUSIVO a proposito (decidido el 30-09): fuera no
                         hay segundo juez -- en Windows o Linux quedaria solo
                         el compilador, y seria un lenguaje mas. Ni Windows,
                         ni Linux, ni consolas (esas solo corren codigo que
                         firma su fabricante). Seccion 14
   NO depende de F1      `titan build` funciona sin el editor de nodos
```

---

## 11. Los riesgos

| riesgo | por que existe | que lo vigila |
|---|---|---|
| **crecer como C++** | C++ agrego durante cuatro decadas y nunca quito | el TECHO de 30 palabras (4.4): un guardian cuenta la tabla de la gramatica, como el techo de no-ASCII. Una palabra nueva entra solo si quita una confusion |
| **ABC**: "facil" ya fracaso | no se podia extender y obligaba a vivir en su entorno (`INTI_MAESTRO` seccion 2) | enlaza con C, INTI y REX desde el primer dia; F1 es opcional, no una jaula |
| **el editor de nodos se vuelve la jaula** | si el grafo es la verdad, el codigo solo se puede leer con F1 | la verdad es el TEXTO (`.titan` + `Titan.toml`); el grafo es una vista (PLAN_TALLER 8) |
| un TERCER lenguaje propio | Rust base + INTI + TITAN++, una persona | la frontera de la seccion 1: si cabe en INTI, va en INTI |
| el comprobador crece sin control | el de Rust mide 1,5 MB (6.8) | el modelo 2 quita las regiones; cada regla entra con su programa roto |
| los juegos piden velocidad | "facil y lento" no sirve para juegos | el metro del emisor mide TITAN++ igual que a C e INTI; la exclusividad da la velocidad de FORTRAN (2b.2) |
| las palabras cambian a mitad | una gramatica que se mueve rompe todo lo escrito | la lista de 25 se acepto el 29-09 y la gramatica se escribe ANTES del lexer (T0) |

---

## 12. El orden, un escalon cada vez

```text
   T0  este documento + la GRAMATICA escrita por el propietario sobre las 25
       palabras, las ordenes y las claves del Titan.toml
   T1  texto -> arbol, en el anfitrion; `titan check` lee Titan.toml y los
       modulos. Los mensajes ya en cuatro partes
   T2  tipos (con `dec` y las tablas de 2b) + "ya lo entregaste" (paso A) + la
       linea de cada modulo y sus `use` (U3)
   T3  la IR propia (6.8, L1) y `titan build`: un .bo por el emisor-x86_64,
       enlazado con bmo-enlazar, con los [permissions] en el .bex (U2). Un
       "hola" en una ventana, visto en el Ryzen
   T4  la ley de exclusividad (paso B): el modelo 2 entero
   T5  `gpu fn` -> SPIR-V -> SASS, y el prestamo a la 3060 (U1)
                                   pide: lanzar computo en ga10x
   T6  el compilador DENTRO de F1, el grafo de nodos, `titan run`, y el .bex
       con su fuente (U4)
                                   pide: TALLER, el asignador de Ring 3 y el
                                   autohospedaje
```

Sin fechas a proposito: una estimacion de un lenguaje que no existe es una
estimacion de otro proyecto (LEY 24).

Los DOS JUECES (6b) van cosidos a estos escalones: el certificado (J1) nace con
T3, la puerta que lo compara (J2) y el NO que llega al nodo (J3-J4) con T6.

---

## 13. Lo que decide el propietario antes de T1

1. ~~La extension~~ -> **`.titan`**, decidida el 29-09.
2. ~~Las palabras~~ -> **ingles ultra simplificado**, y la lista de 25 aceptada
   el 29-09 (4.4). Techo propuesto: 30.
3. **El modelo 2** (sin referencias guardadas, sin un `'a`): confirmado?
4. **El primer programa**: un juego chico, una app con ventana, o un calculo
   en la 3060? Decide que se construye primero en T3.
5. ~~El editor de nodos: donde van las posiciones~~ -> **en `[layout]`
   dentro de `Titan.toml`**, hecho en L1 (PLAN_TALLER 8.6, 29-09).
6. **Los simbolos** (14.2): la propuesta minima de abajo, o cual.
7. ~~Llaves o sangria~~ -> **SANGRIA, como INTI**, decidido el 30-09 (y el
   primer `hola` sale en la CONSOLA). Ver `toolchain/lang/titan/GRAMATICA.md`.

---

## 14. LA ANATOMIA DE TITAN++, punto por punto (30-09)

Pedida por el propietario para madurarlo: *"que anatomia seria mi TITAN++ y
todos esos elementos, todo en puntos"*. Cada pieza dice si YA EXISTE, si esta
DECIDIDA o si FALTA -- lo que no existe no se escribe como si existiera.

### 14.1 Lo que se escribe

```text
   Titan.toml             el nodo principal: [package], [permissions],
                          [layout] (las posiciones de F1)        EXISTE (L1)
   titan/biblioteca.toml  que paquetes hay                       EXISTE (L1)
   un .titan              1a linea `mod x "que hace"`; debajo `use a, b`,
                          `mod a, b` y `mod x in "ruta"`         EXISTE (L1-L2)
                          y el CUERPO, que es la gramatica        FALTA (T0)
```

### 14.2 La sintaxis: MUY POCAS piezas

Si. Esa es la decision desde el 29-09, y se sostiene con numeros:

```text
   palabras      25 aceptadas (4.4), techo 30 con guardian   DECIDIDO
   tipos         int, dec, f32, tablas [f32; n, m], texto...
                 son TIPOS, no palabras: no gastan techo      DECIDIDO
   logica        and / or / not con palabras, sin && || !     DECIDIDO
   simbolos      la propuesta de abajo                        FALTA (T0)
```

**PROPUESTA de simbolos para T0 -- NO es gramatica**, es lo minimo que casi
cualquier lenguaje necesita, para que el propietario diga si, no o cual:

```text
   agrupar       ( )  [ ]
   separar       ,  :  .
   registro      { }   solo para ESCRIBIR uno: Ship { x: 0.0 } (los bloques
                       van por sangria, 30-09)
   asignar       =
   comparar      ==  !=  <  <=  >  >=
   calcular      +  -  *  /  %
   devolver      ->          (el tipo que sale de una fn)
   texto         "..."
   comentario    #  hasta el final de la linea

   y lo que se EVITA a proposito:
   && || !       ya son palabras
   & y * de punteros   no hay referencias guardadas (6.5): `mut` y `take`
                       dicen lo que & y * decian en C
   ::            un solo separador de caminos: el punto
   <T>           los genericos, si llegan, por otro camino que no parezca
                 una comparacion
```

Unos veinte simbolos y 25 palabras: se aprende en una tarde, y una IA lo
escribe con menos errores (seccion 9).

### 14.3 El modelo: valores, no punteros

```text
   un valor tiene UN propietario; al acabar el propietario, se libera
   `mut`   se presta para cambiarlo, y nadie mas lo toca mientras dure
   `take`  se entrega: el que lo tenia ya no lo tiene
   sin recolector, sin referencias guardadas en estructuras (6.4, 6.5)
```

DECIDIDO como plan (modelo 2, seccion 6); el propietario lo confirma en 13.3.

### 14.4 Lo que SOLO BMO-X le da (seccion 3)

```text
   U1  el prestamo conoce al KERNEL (ofrecer/tomar) y a la 3060 (hasta el wait)
   U2  los permisos son parte del tipo: usar la 3060 sin pedirla no compila
   U3  MODULAR dentro del compilador: cada modulo dice su linea y sus `use`
   U4  el .bex lleva su fuente, y F1 lo abre
```

U2 en su forma minima YA EXISTE en el lector (L1: `use gpu` sin permiso es un
problema dicho). El resto espera al compilador.

### 14.5 El compilador por dentro

```text
   lector de cabeceras   titan-lector: Titan.toml + mod/use         EXISTE
   el contrato           titan-contrato: grafo, eventos, 4 partes   EXISTE
   T1  texto -> arbol    `titan check`, mensajes de 4 partes        NIVEL 0 HECHO
                         (toolchain/lang/titan, 30-09)
   T2  tipos             numero y texto, y su mezcla es un NO        EN CURSO
                         (calc.rs, nivel 1); faltan `dec`, tablas
   T3  IR + emisor       NIVEL 0 HECHO en el anfitrion (04-10):     EN CURSO
                         IR propia (src/ir.rs) + emisor propio
                         (emisor-x86_64/, `titan build`), SIN el de
                         INTI. Falta verlo en el Ryzen, y el .bo con
                         bmo-enlazar llega cuando llame a INTI
   T4  borrow checker    EL JUEZ existe (juez.rs, nivel 1): cada     EN CURSO
                         local en UN estado por punto, sobre la IR;
                         falta la ley de exclusividad (el modelo 2)
   T5  gpu fn            SPIR-V -> SASS, el prestamo a la 3060      FALTA
   T6  dentro de F1      el compilador en el taller, `titan run`    FALTA
```

### 14.6 Lo que sale: el `.bex`

```text
   codigo x86-64            el mismo BEF2 que C, C++, COBOL, Ada, INTI
   [permissions]            lo que pidio el Titan.toml (U2)          T3
   el CERTIFICADO           permisos, prestamos del kernel y su linea
                            (6b.3 y 6b.8)                            J1
   la fuente                para que F1 lo abra (U4)                 T6
   la firma                 bmo-firmar, sobre todo lo anterior       EXISTE
```

### 14.7 Los dos jueces (6b)

```text
   el COMPILADOR   dentro, antes, una vez: su NO es final
   el KERNEL       entre programas, siempre, barato: su SI es el ultimo
   el puente       el certificado NOMBRA, la capability CONCEDE
```

### 14.8 Donde se ve

```text
   F1 (TALLER)   el grafo, el comprobador animado, los errores en rojo que
                 guian (ERROR 1/n + el camino), [e] al siguiente      EXISTE
                 (L1-L4; el comprobador es el del ejemplo hasta T4)
                 y el EXPLORER que organiza el disco: nuevo, renombrar,
                 quitar, mover, plegar, el orden propio (L5, 04-10)   EXISTE
   CABINA        el NO del kernel, para quien mira la maquina entera  EXISTE
   el puente     el NO del kernel llevado a su nodo en F1             J3-J4
   `titan`       new / check / build / run / test (4.1)               FALTA
```

### 14.9 A quien llama (seccion 7)

```text
   INTI      la CPU y el sistema
   VERRANO   dibujar con la 3060
   REX       ventana, entrada, disco, sonido, red -- por las DOS puertas
```

### 14.10 Lo que CONSTRUIRA, y lo que no

| que | como lo ve la casa |
|---|---|
| JSON, formatos, herramientas | SI: dias de trabajo, y un buen primer programa de verdad |
| motores graficos, juegos | SI: su razon de ser (VERRANO, la 3060, U1). Meses |
| **un navegador PROPIO** | SI como meta, y **no de Google** (el propietario, 30-09: *"Chromium fue solo inspiracion para tener navegador mio"*). Pero el de la casa: NAVEGAR (hoy en INTI) + la ANTENA (el movil mastica la web y BMO-X pinta) + MAQUETA con el subconjunto que elige L7. TITAN++ puede ser el lenguaje de sus piezas nuevas; un Chromium son decenas de millones de lineas, y L7 ya dice que BMO-X no es un navegador generalista |
| correr fuera de BMO-X | NO, decidido: exclusivo (seccion 10) |

### 14.11 Lo que todavia no se sabe

- Los simbolos (14.2): son del propietario, en T0.
- El primer programa (13.4): decide que se construye en T3.
- El modelo 2 (13.3): confirmado o no.
- ~~Que queria decir *"aprende de todo"*~~ -> aclarado el 30-09: que BMO-X
  va reuniendo lo que hace falta; no es una pieza del lenguaje.

### 14.12 BOCETOS: como quedaria (30-09) -- NO es gramatica

Cuatro programas con SOLO las 25 palabras y los simbolos de 14.2, para ver el
lenguaje entero de un vistazo. Los bloques van por **SANGRIA, como INTI**
(decidido el 30-09); la gramatica de cada nivel se escribe en
`toolchain/lang/titan/GRAMATICA.md` el dia que su banco pasa, y hasta entonces
esto es un punto de partida, no una decision.

**1. Prestar (`mut`) y entregar (`take`), y el error que no deja compilar**

```text
type Ship
    x: f32
    fuel: dec

fn push(mut s: Ship, dx: f32)       # se presta para cambiarlo
    s.x = s.x + dx

fn scrap(take s: Ship) -> dec       # se lo queda: el que llama ya no lo tiene
    return s.fuel

fn main()
    let mut ship = Ship { x: 0.0, fuel: 12.50 }
    push(mut ship, 3.0)
    let left = scrap(take ship)
    print(ship.x)                   # NO compila
```

```text
QUE      `ship` se usa despues de entregarlo
DONDE    main, linea 14
POR QUE  `scrap(take ship)` en la linea 13 se lo quedo
COMO     usa `left`, o presta con `mut` en vez de entregar con `take`
```

Y en F1, el nodo `main` en rojo con su ERROR (PLAN_TALLER 8.9).

**2. Un JSON con `enum` y `match`**

```text
enum Json
    Null
    Bool(bool)
    Num(dec)
    Text(text)
    List([Json])

fn show(v: Json) -> text
    match v
        Null -> "null"
        Bool(b) -> if b then "true" else "false"
        Num(n) -> n.to_text()
        Text(t) -> "\"" + t + "\""
        List(items) -> "[" + join(items, ",") + "]"
```

`match` obliga a cubrir TODOS los casos: un caso nuevo en `Json` sin su rama
aqui no compila. (`then` no es una de las 25: como se escribe un `if` en una
linea es de T0.)

**3. La 3060, con permiso y prestamo (U1 y U2)**

```text
# Titan.toml ->  [permissions]  gpu = "compute"

gpu fn add(a: [f32; 1024], mut out: [f32; 1024])
    for i in range(1024)
        out[i] = out[i] + a[i]

fn main()
    let a = [1.0; 1024]
    let mut b = [0.0; 1024]
    let job = gpu.launch(add, a, mut b)
    print(b[0])          # NO compila: la 3060 todavia tiene `b`
    wait(job)
    print(b[0])          # bien: ya volvio
```

Sin `gpu = "compute"` en el `Titan.toml`, ni siquiera `gpu fn` compila (U2).

**4. Un bucle de juego**

```text
fn main()
    let mut ship = Ship { x: 100.0, fuel: 50.00 }
    while true
        let keys = input.read()
        if keys.left and ship.fuel > 0.00
            push(mut ship, -2.0)
        draw(ship)
        wait(frame)          # duerme hasta el siguiente fotograma: no gira
```

### 14.13 LA LISTA MAESTRA DEL FRONTEND: lo que se toma y lo que NO (30-09)

Pedida por el propietario para construir el frontend: lo que se toma, FUERTE;
lo que no, en ROJO y de TODAS las inspiraciones. Se lee asi:

```diff
+ VERDE: SE TOMA
- ROJO:  NO SE TOMA, y el motivo
```

(Los bloques `diff` salen en color en GitHub y en VS Code; en texto plano, el
`+` y el `-` dicen lo mismo.)

**Python -- la SUPERFICIE: que se lea de corrido**

```diff
+ los bloques por SANGRIA (decidido el 30-09), como INTI
+ pocas palabras, en ingles, que se leen como frases
+ and / or / not con palabras
+ # para comentar, hasta el final de la linea
+ for x in ..., y range(n)
+ una sola forma obvia de hacer cada cosa
- tipos que cambian solos (x = 5 y luego x = "hola"): el comprobador necesita
-   saber que es cada cosa
- None: un enum con su caso vacio dice lo mismo, y match obliga a mirarlo
- recolector de basura: se libera cuando el propietario acaba
- correr sin compilar: los errores saldrian en casa del usuario, no del autor
```

**JavaScript -- la FORMA: literales que ya conoce todo el mundo**

```diff
+ literales de registro: Ship { x: 0.0, fuel: 12.50 }
+ la flecha -> (el tipo que sale de una fn, las ramas de match)
+ texto que se suma: "a" + b
- bloques con { }: T0 eligio SANGRIA el 30-09, como INTI (las llaves quedan
-   solo para escribir un registro)
- == que convierte tipos ("1" == 1 es true): la fuente de fallos mas famosa
- null Y undefined: dos formas de "no hay nada"
- this, prototipos y clases que cambian en marcha
- todo es un objeto que cualquiera puede tocar: aqui cada valor tiene UN propietario
- el modelo de un solo hilo con callbacks: aqui se espera con wait
```

**C++ -- el ALCANCE: llega a todo, y no paga lo que no usa (seccion 2)**

```diff
+ hace TODO: del motor al juego y a la herramienta
+ abstracciones sin coste: lo que no usas no lo pagas
+ RAII: se libera al salir del bloque (lo hace el propietario)
+ genericos y calculo al compilar, con mensajes que se leen
+ sobrecarga de operadores: a + b con vectores y matrices
- comportamiento indefinido: cero UB, las reglas de INTI
- preprocesador y cabeceras: modulos desde el primer dia
- herencia y jerarquias: composicion + trait
- excepciones: los errores son datos
- cinco formas de inicializar: una
- muros de errores de plantilla: un generico dice que le falta, en una frase
```

**Rust -- la SEGURIDAD: el comprobador, sin su peso (seccion 6)**

```diff
+ el borrow checker: nadie cambia lo que otro esta usando
+ memoria sin recolector y segura
+ enum con datos y match que obliga a cubrir todos los casos
+ los errores como valores
- & , &mut y los tiempos de vida 'a: mut y take hacen su trabajo
- referencias guardadas dentro de estructuras (6.5)
- unsafe como salida de emergencia: lo que el comprobador no ve, no existe
- la medida de rustc: 1,53 MB de comprobador (6.8); TITAN++ razona funcion a funcion
- crates.io: dependencias por ruta y con huella, sin bajar de internet
```

**Swift y Hylo -- el MODELO: valores mutables (6.3)**

```diff
+ cada parametro dice si lee, cambia (mut) o se queda (take)
+ la ley de exclusividad, comprobada funcion a funcion
- clases con referencias compartidas y su cuenta de referencias
```

**COBOL -- los NUMEROS: el dinero no se redondea solo (2b.1)**

```diff
+ decimales exactos: dec(9, 2), el MISMO de INTI
+ desbordar es un error, no un truncado
+ redondear es una llamada con nombre: round(x, 2, half_even)
- la sintaxis de frases largas (ADD A TO B GIVING C)
- las divisiones y la columna fija
```

**FORTRAN -- las TABLAS: la velocidad del silicio (2b.2)**

```diff
+ tablas de varias dimensiones como valores: A = B + C * 2.0
+ funciones elementales: se aplican a cada celda (y corren en la 3060)
+ la regla de oro, ahora DEMOSTRADA: los argumentos no se solapan
- GOTO
- COMMON: memoria global compartida
- EQUIVALENCE: dos nombres para la misma memoria (rompe la exclusividad)
- la columna fija
```

**Lo que SOLO da BMO-X -- y por eso TITAN++ no sale de aqui (3, 6b, 10)**

```diff
+ el comprobador conoce los prestamos del KERNEL y de la 3060 (U1)
+ los permisos son parte del tipo (U2)
+ MODULAR dentro del compilador (U3)
+ el .bex lleva su fuente y F1 lo abre (U4)
+ dos jueces: el compilador PREPARA, el kernel DECIDE
- JIT en la CPU: codigo que se escribe en marcha es codigo que el juez no ve
- correr fuera de BMO-X: sin el kernel, queda un juez solo
```

### 14.14 LA ESCALERA: hasta donde llega, y con cuantas palabras (30-09)

El propietario: *"no vamos a empezar directo... hasta que tan ULTRA
SIMPLIFICADO es capaz?"*. El frontend no se construye de golpe: se sube
NIVEL A NIVEL, y cada nivel agrega pocas palabras y ya sirve para algo.

| nivel | palabras nuevas | total | lo que ya se puede escribir |
|---|---|---|---|
| 0 | `fn` | 1 | un programa que saluda: `fn main() { print("hola") }` |
| 1 | `let` | 2 | calcular: `let area = 3 * 4` |
| 2 | `mut` | 3 | contar, acumular: `let mut n = 0` y `n = n + 1` |
| 3 | `if else true false and or not` | 10 | DECIDIR: una calculadora, un semaforo, reglas |
| 4 | `for in while break continue` | 15 | REPETIR: tablas de multiplicar, buscar, ordenar |
| 5 | `return` | 16 | funciones con resultado: **ya se escribe cualquier algoritmo** |
| 6 | `type` | 17 | registros: un jugador, una nave, una factura |
| 7 | `take` | 18 | el borrow checker entero: prestar y entregar |
| 8 | `enum match` | 20 | casos con datos: **un JSON**, un menu, estados de un juego |
| 9 | `mod use pub` | 23 | varios ficheros: **una app o un juego de verdad**, y F1 los muestra como grafo |
| 10 | `trait` | 24 | comportamientos compartidos: un motor con piezas que se cambian |
| 11 | `gpu` | 25 | **computo en la 3060** |

**La respuesta corta: con 16 palabras ya se escribe cualquier algoritmo; con
20, un JSON; con 23, una app; con 25, la 3060.** Lo que va de 16 a 25 no da
potencia de calculo: da ORDEN (tipos, casos, modulos) y SEGURIDAD (prestamos,
permisos).

**Cuantas listas de ejemplos: una por nivel, doce en total, y cada una con
DOS clases de programa**:

```text
   BIEN   compila y dice exactamente lo esperado (su salida escrita al lado)
   NO     NO compila, y su mensaje de 4 partes esta escrito al lado
```

Asi cada nivel es un BANCO, como el de `bmo-c-front`, que ejecuta los
programas y no solo los compila: el frontend sube un nivel cuando su lista
pasa entera, y ninguno se da por hecho sin ella. Con 2-4 programas por
nivel salen unos 30-40 ejemplos para todo el lenguaje.

**Y los niveles, cosidos a los escalones del compilador (seccion 12):**

```text
   T1  texto -> arbol      niveles 0-5    (el primero, solo el 0: `print("hola")`)
   T2  tipos               niveles 6 y 8
   T3  IR + emisor         el nivel 0 CORRE en el Ryzen, en una ventana
   T4  borrow checker      nivel 7 (paso A: "ya lo entregaste"; paso B: la ley)
   U3  modulos             nivel 9
   T5  la 3060             nivel 11
```

**NIVEL 0 -- T1 HECHO el 30-09** (`toolchain/lang/titan`, `bmo-titan-front`):

```text
mod main "saluda"

fn main()
    print("hola")
```

se lee y sale su arbol (`titan check`, `titan arbol`); lo que no, sale con su
mensaje de 4 partes y un codigo estable (T0001-T0052, `GRAMATICA.md`). El banco
(`ejemplos/nivel0/`) son 2 programas BIEN y 12 NO, UNO POR CODIGO: una prueba
exige que cada codigo lo provoque algun ejemplo. Una palabra de un nivel que no
existe dice cual la trae (`let` -> T0040, nivel 1). Y **el TITAN guardian**: la
prueba de `words.rs` exige las 25 palabras, el techo de 30 y que cada nivel
sume lo que dice la tabla de arriba.

**NIVEL 0 -- T3 HECHO EN EL ANFITRION el 04-10** (`emisor-x86_64/`,
`bmo-titan-x86-64`, la orden `titan`):

```text
   texto -> arbol -> IR (src/ir.rs, sin maquina: una prueba lo vigila)
         -> bytes (los textos como inmediatos por la puerta de bmo-lower,
            la secuencia que hello-bex ya corrio en el metal)
         -> .bex con su MANIFIESTO (lenguaje, nivel, modulo, que hace,
            [permissions]) por `exige_manifiesto`: nunca un binario mudo
```

- **Sin el emisor de INTI** (el propietario, 04-10: INTI se reduce a la
  PRECISION, sin apoyo; TITAN++ lleva lo suyo). Lo compartido es de la casa:
  `bmo-lower`, `bmo-abi`, `bmo-verify`.
- **El banco CORRE**: cada programa BIEN se construye, se carga como lo carga
  el kernel y se ejecuta en el emulador; su consola tiene que ser la de sus
  lineas `# sale:`. El mismo fuente da el mismo `.bex`, byte a byte.
- **T0053**, regla nueva: en el nivel 0 no hay `if`, asi que una llamada que
  vuelve sobre si misma (`main -> main`, `a -> b -> a`) no termina nunca y en
  la maquina seria una tarea muerta por la pila. Se dice al compilar.

[!] **Falta el metal**: `build.ps1` despliega `titan/hola.bex` y
`titan/dos.bex`; en el Ryzen, `run titan/hola.bex` en F12 tiene que escribir
`hola`. Hasta esa foto es 🟡. La ventana, despues.

