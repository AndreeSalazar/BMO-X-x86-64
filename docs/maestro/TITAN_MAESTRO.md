# TITAN++ MAESTRO -- el lenguaje para CONSTRUIR sobre BMO-X

> **TITAN++** -- nombre elegido por Eddi el 2026-09-29. Empezo llamandose
> "Rust++" y el propietario lo cambio: *"es una ironia que queria usar Rust++ por
> motivos pero me llevo el Titan++, ese me gusta mas"*. Los `++` se quedan como
> la ironia: dicen de donde viene la inspiracion sin tomar el nombre prestado.
> En rutas y ficheros se escribe `TITAN` (los `+` dan guerra en un nombre de
> fichero). Extension propuesta: `.titan` (sin decidir, seccion 10).

Escrito el **2026-09-29**, antes de una sola linea de codigo, con el mismo
criterio que `INTI_MAESTRO.md` y `PYTHON_MAESTRO.md`: que esta conversacion no
haya que reconstruirla.

Lo que pidio el propietario, en sus palabras:

> *"ya tengo mi Rust base que encarga de funcionar BMO-X pero ese Rust++ es para
> construir juegos, App y otros mas y si es para hablar con GPU y CPU ya le llama
> inti y otros los demas"*

> *"va a llevar INSPIRACION brutal de Borrow checker para facilitar ... para App,
> Juegos TODO E TODO"*

> *"POCOS sintaxis ... Sintaxis ULTRA SIMPLIFICADA pero deja listo el Rust asi
> por completo para facilitar procesos MAS PRECISOS"*

---

## 0. La respuesta corta, en un dibujo

```text
   TITAN++      lo que se CONSTRUYE para BMO-X: juegos, apps, herramientas,
      |         computo masivo en la 3060. Pocas palabras, la seguridad de
      |         Rust por debajo, y se compila DENTRO de BMO-X (F1)
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

**Esto ya se hizo en el mundo, y funciono.** Apple escribe el sistema en C y
Objective-C y las apps en **Swift**; Swift es exactamente el modelo de memoria
que se propone aqui (seccion 3). TITAN++ es ese reparto, con los nombres y las
reglas de BMO-X.

---

## 1. Como se llego aqui (29-09, decisiones del propietario)

| pregunta | lo que se decidio | por que |
|---|---|---|
| el nombre | **TITAN++**, no "Rust++" | nombre propio, como INTI. "Rust" es marca de la Rust Foundation y su politica limita usarla en el nombre de otros lenguajes |
| que es | un **lenguaje nuevo** | ni una libreria sobre rustc ni un traductor a Rust |
| cuanto Python | **solo el espiritu** | pocas reglas, mensajes claros, cero sorpresas. No su sintaxis, y no corre programas de Python (eso es `PYTHON_MAESTRO.md`) |
| para que | **todo lo que se construye**: apps con ventana, juegos, trabajo en la 3060, servicios, guiones, e IA | "TODO E TODO", como hace Python en otros sistemas |
| la frontera con INTI | **A**: INTI = CPU / kernel / sistema; TITAN++ = lo que INTI no hace | cada uno con su motivo |
| fundirlo con INTI | **NO** ("ni merga") | son dos lenguajes con dos trabajos |
| las apps que ya estan en INTI | **se quedan** (NAVEGAR y las demas) | no hay que migrar nada |

** Y lo que esto REABRE, dicho a la vista: `docs/METAS.md` tenia "La lista de
lenguajes" **CERRADA** desde el 17/18-09 por decision del propietario. Desde el
29-09 la reabre el mismo propietario para que entre TITAN++. La fila de METAS lo
dice.

---

## 2. Por que un compilador PROPIO: por F1

`plan/PLAN_ESTRUCTURA.md`: se pulsa **F1** en el escritorio y se abre **el
taller**, una app que es terminal Y compilador en UN `.bex`. Se escribe, se
compila, y aparece un `.bex` en el disco de BMO-X. Sin instalar nada.

Si TITAN++ se tiene que compilar DENTRO de F1, **rustc no puede ser su
compilador**:

```text
   rustc + LLVM    decenas de megas, necesitan el std de un anfitrion,
                   no caben en un .bex de Ring 3
   TITAN++         un frontend de BMO-X + el emisor-x86_64 que ya compila
                   C, C++, COBOL, Ada e INTI
```

No es una preferencia: es lo que F1 obliga. Y encaja con la regla de la casa,
*un frontend nuevo sobre el emisor que ya existe* (`METAS.md` cat. 2: cinco
compiladores propios, sin LLVM ni GCC). **rustc SI sirve de inspiracion**: su
arquitectura (texto -> arbol -> tipos -> comprobador -> codigo) se imita; su
medida no.

**Lo que F1 pide antes**, y hoy no esta:

```text
   la ventana de F1               PLAN_ESTRUCTURA, casilla 1 (ABIERTA)
   el autohospedaje               PLAN_AUTOHOSPEDAJE (APARCADO)
   memoria dinamica en Ring 3     un asignador compartido en bmo-userland:
                                  hoy solo PROTON-X tiene #[global_allocator]
```

Hasta que eso exista, el compilador de TITAN++ corre en el anfitrion (Windows),
como todos los demas hoy. Que corra en F1 es el ultimo escalon (T6), no el
primero.

---

## 3. EL CORAZON: el borrow checker, inspirado y no copiado

### 3.1 Que es, y por que TITAN++ lo necesita

El borrow checker es la parte del compilador que **demuestra, antes de
ejecutar**, dos cosas:

```text
   nadie usa un dato despues de que se libero
   nadie escribe un dato mientras otro lo esta leyendo
```

Es lo que hace a Rust seguro **sin recolector de basura**. "Deja listo el Rust
por completo" quiere decir eso: sin el, TITAN++ seria otro C con otra ropa.

### 3.2 Por que el de Rust es pesado

El de Rust razona sobre **tiempos de vida** (`'a`) que cruzan de una funcion a
otra: quien presto que, a quien, hasta cuando. Es de lo mas dificil de rustc
(decenas de miles de lineas, y mucho tiempo de gente que no hacia otra cosa), y los tiempos de vida **se ven
en la sintaxis**: `&'a mut T`. Justo lo contrario de "pocas palabras".

La leccion: **lo dificil de Rust no son sus palabras, son sus reglas**. Las
reglas se pueden esconder; no se pueden borrar. Solo hay dos formas honradas:

```text
   que el compilador las DEDUZCA   -> el lenguaje renuncia a guardar referencias
   que se MUESTREN con palabras    -> ya no son "pocas"
```

### 3.3 Los tres modelos

| | modelo | lo que escribe el programador | el comprobador |
|---|---|---|---|
| 1 | Rust tal cual: prestamos con tiempos de vida | `&`, `&mut`, `'a` | enorme; razona entre funciones |
| **2** | **valores mutables** (Swift, Hylo) | casi nada: cada parametro dice si **lee**, **cambia** o **se queda** el valor; no hay referencias guardadas | **local, funcion a funcion**: nadie cambia lo que otro lee a la vez |
| 3 | solo mover y copiar | nada | el mas chico: "no uses lo que ya entregaste" |

### 3.4 La eleccion: el modelo 2, construido desde el 3

**TITAN++ toma el modelo 2**, y lo construye en dos pasos:

```text
   paso A (modelo 3)   "ya lo entregaste": un valor que se mueve no se vuelve
                       a usar. Chico, y ya util
   paso B (modelo 2)   LA LEY DE EXCLUSIVIDAD: mientras algo se CAMBIA, nadie
                       mas lo lee ni lo cambia. Se comprueba en cada llamada
```

Los modos de un parametro, como ideas (las PALABRAS las elige el propietario,
seccion 10):

```text
   lee       el que llama sigue siendo el propietario; nadie lo cambia   (por defecto)
   cambia    se presta para cambiarlo; nadie mas lo toca mientras dura
   se queda  el valor pasa a ser de la funcion; el que llama ya no lo tiene
```

Un boceto, **NO una gramatica** (solo para ver la idea):

```text
   fun mover(cambia jugador, lee mapa)
       jugador.x = jugador.x + 1
       si mapa.solido(jugador.x, jugador.y)
           jugador.x = jugador.x - 1

   mover(heroe, nivel)       bien
   mover(heroe, heroe.mapa)  NO: `heroe` se CAMBIA y a la vez se LEE su mapa.
                             El compilador lo dice, con sitio y motivo
```

Sin un `&`, sin un `'a`: la seguridad de Rust con lo que el programador ya
entiende ("esto lo cambio", "esto solo lo miro").

### 3.5 Lo que se pierde, dicho ahora

**No se guardan referencias dentro de estructuras.** Un grafo con punteros de
nodo a nodo se escribe con **indices** (una tabla de nodos, y cada arista dice
un numero). Es el precio real del modelo 2.

** Y para juegos resulta ser una ventaja: el esquema con que se hacen hoy los
motores grandes (ECS: entidades que son indices, componentes en tablas) ES ese.
Un juego en TITAN++ se escribe como ya se escribe un juego rapido.

### 3.6 Los mensajes: el comprobador tiene que EXPLICAR

Un borrow checker que dice "no" sin porque es el que hace odiar a Rust. TITAN++
hereda de INTI el **formato de cuatro partes** de sus mensajes (`INTI_MAESTRO`
F1): que paso, donde, por que es un error, y como se arregla. Cada regla del
comprobador tiene su programa roto de prueba, como el juez del SASS.

### 3.7 Los precedentes

```text
   Swift   inout + la ley de exclusividad, en produccion desde hace una decada
           (Apple). La prueba de que el modelo 2 sirve para apps de verdad
   Hylo    "mutable value semantics": el modelo 2 llevado al extremo. Investigacion
   Mojo    sintaxis tipo Python + propiedad + GPU. Pero con muchas palabras
   Rust    el origen de todo. Modelo 1
```

---

## 4. A quien llama TITAN++, y como

TITAN++ **no toca hardware**. Llama a lo que ya existe:

| para | llama a | por |
|---|---|---|
| CPU y sistema | **INTI** | compilacion separada: `.bo` + `bmo-enlazar` (HECHA para C, C++ e INTI) |
| dibujar con la 3060 | **VERRANO** | la API de dibujo de Ring 3 |
| ventana, teclado, raton, disco, sonido, red | **REX / bmo-userland** | los dos syscalls, como cualquier `.bex` |

[!] Una dependencia de la casa que ya esta anotada: **INTI todavia no declara
funciones AJENAS** (`externo`, en ESPERA en METAS: "la gramatica es de Eddi").
Para que TITAN++ llame a INTI basta el sentido que ya existe (INTI como
biblioteca); para que INTI llame a TITAN++, falta esa palabra.

---

## 5. La 3060: computo masivo

"Computo masivo en la 3060" quiere decir que partes del programa **corren
DENTRO de la GPU**. Lo que ya hay y lo que falta:

```text
   HECHO   SPIR-V -> SASS con juez (PLAN_LA_LENGUA_DE_LA_3060, E1-E5):
           un subconjunto de SPIR-V, el codificador bit a bit contra
           NVIDIA, el emisor y el juez que dice NO con motivo
   HECHO   dibujar: VERRANO, el cubo bit a bit con D3D12
   FALTA   LANZAR computo: la QMD y el banco constante 0 (el mapa medido
           esta en platform/drivers/gpu/ga10x/COMO_LE_HABLA_NVIDIA.md, 3d).
           Hoy la 3060 dibuja; todavia no calcula por encargo
```

El camino de TITAN++: las funciones marcadas para la GPU **bajan al subconjunto
de SPIR-V** que la casa ya sabe llevar a SASS. Se reutilizan emisor y juez
enteros; TITAN++ no escribe SASS.

** **La idea que solo sale aqui:** el borrow checker y la GPU hablan del mismo
problema. Un bufer que la 3060 esta escribiendo es un bufer **prestado para
cambiar**: la CPU no lo puede leer hasta que el trabajo acaba (`WAIT`). La ley
de exclusividad de la seccion 3.4 **es** esa regla. En TITAN++ mandar un bufer a
la GPU podria ser un prestamo `cambia` que el compilador vigila hasta la espera:
el error clasico de GPU (leer antes de que termine) se vuelve un error de
compilacion. Es una idea, anotada, no un compromiso.

---

## 6. IA: los dos sentidos

```text
   que una IA ESCRIBA TITAN++     pocas palabras + mensajes exactos = menos
                                  errores al generarlo. Una gramatica chica
                                  se aprende de pocos ejemplos
   cargas de IA EN la 3060        tensores: es la seccion 5, y depende de lo
                                  mismo (lanzar computo)
```

El asistente de IA dentro de BMO-X sigue **APARCADO** (METAS cat. 2, decision
del 10-09). TITAN++ no lo desaparca: lo hace mas facil el dia que se retome.

---

## 7. Lo que TITAN++ NO es

```text
   NO es Python          no corre programas de Python (eso es el interprete
                         de PYTHON_MAESTRO)
   NO es Rust            no compila crates de crates.io ni usa rustc
   NO escribe el kernel  el kernel y los drivers son la Rust base; el metal,
                         INTI
   NO sustituye a INTI   INTI y sus apps se quedan
   NO tiene recolector   la memoria se libera cuando su propietario acaba
   NO guarda referencias en estructuras (seccion 3.5)
   NO tiene JIT          todo se compila antes: codigo que se escribe en
                         marcha es codigo que el juez no puede ver
   NO es multiarquitectura  x86-64 en la CPU y sm_86 en la 3060: una
                         arquitectura, un repositorio
```

---

## 8. Los riesgos

| riesgo | por que existe | que lo vigila |
|---|---|---|
| **ABC**: "Python pero ultra facil" ya fracaso | no se podia extender y obligaba a vivir en su entorno (`INTI_MAESTRO` seccion 2) | TITAN++ enlaza con C, INTI y REX desde el primer dia; F1 es opcional, no una jaula |
| un TERCER lenguaje propio que mantener | Rust base + INTI + TITAN++, una persona | la frontera de la seccion 1: si una pieza cabe en INTI, va en INTI |
| el comprobador crece sin control | el borrow checker de Rust es de lo mas grande de rustc | el modelo 2 es LOCAL por funcion; una regla nueva entra con su programa roto |
| los juegos piden velocidad | un lenguaje "facil" que va lento no sirve para juegos | el metro del emisor (25 programas) mide TITAN++ igual que a C e INTI |
| las palabras cambian a mitad | una gramatica que se mueve rompe todo lo escrito | la gramatica la escribe el propietario ANTES del lexer (T0) |

---

## 9. El orden, un escalon cada vez

```text
   T0  este documento + la GRAMATICA escrita por el propietario (las palabras)
   T1  texto -> arbol, en el anfitrion. Los mensajes ya en cuatro partes
   T2  tipos + "ya lo entregaste" (paso A del comprobador)
   T3  un .bo por el emisor-x86_64, enlazado con bmo-enlazar: un "hola" en
       una ventana (REX) visto en el Ryzen
   T4  la ley de exclusividad (paso B): el modelo 2 entero
   T5  funciones de GPU -> SPIR-V -> SASS         pide: lanzar computo en ga10x
   T6  el compilador DENTRO de F1                 pide: ESTRUCTURA, el asignador
                                                  de Ring 3 y el autohospedaje
```

Sin fechas a proposito: nada de esto se ha medido todavia, y una estimacion de
un lenguaje que no existe es una estimacion de otro proyecto (LEY 24).

---

## 10. Lo que decide el propietario antes de T1

1. **Las palabras.** INTI eligio castellano en ASCII, en una tabla. TITAN++:
   castellano, ingles, o la misma tabla que INTI?
2. **La extension**: `.titan`?
3. **El modelo 2** (sin referencias guardadas, sin un `'a` a la vista):
   confirmado, o el propietario quiere ver el 1?
4. **El primer programa**: un juego chico, una app con ventana, o un calculo en
   la 3060? Es el que decide que se construye primero en T3.
