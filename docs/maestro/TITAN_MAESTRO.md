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

---

## 0. La respuesta corta

```text
   el ALCANCE de C++        hace TODO: juegos, apps, herramientas, calculo
   la SUPERFICIE de Python  pocas palabras, se lee de corrido
   la SEGURIDAD de Rust     un comprobador de prestamos, sin recolector
   y lo que SOLO BMO-X da   el comprobador conoce los prestamos del KERNEL
                            y de la 3060, y los permisos del programa
```

La ultima fila es la respuesta a *"puede hacer MAS que C++"*: C++ no sabe
nada del sistema en el que corre; TITAN++ nace sabiendo el de BMO-X (seccion 3).

```text
   TITAN++      lo que se CONSTRUYE: juegos, apps, herramientas, computo en la
      |         3060. `titan run`, `Titan.toml`, modulos ordenados
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
| que es | un **lenguaje nuevo** | ni una libreria sobre rustc ni un traductor a Rust |
| inspiracion | **C++ TOTAL en el alcance**; Python solo en el espiritu; Rust en el comprobador | "hace TODO", y con pocas palabras (seccion 2 reconcilia las dos cosas) |
| para que | apps con ventana, juegos, trabajo en la 3060, servicios, guiones, e IA | "TODO E TODO" |
| la frontera con INTI | **A**: INTI = CPU / kernel / sistema; TITAN++ = lo que INTI no hace | cada uno con su motivo |
| fundirlo con INTI | **NO** ("ni merga") | dos lenguajes, dos trabajos |
| las apps que ya estan en INTI | **se quedan** (NAVEGAR y las demas) | no hay que migrar nada |
| la herramienta | **tipo cargo**: `titan run`, `Titan.toml`, `mod` | "todo organizado" |

** Y lo que esto REABRE: `docs/METAS.md` tenia "La lista de lenguajes"
**CERRADA** desde el 17/18-09 por decision del propietario; el 29-09 la reabre
el mismo propietario para que entre TITAN++. La fila de METAS lo dice.

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
   herencia y jerarquias          NO: composicion + interfaces
   excepciones                    NO: los errores son datos, como en INTI
   cinco formas de inicializar    NO: una
   muros de errores de plantilla  NO: un generico dice que le falta, en una frase
```

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
   buf = gpu.bufer(1024)
   trabajo = gpu.lanza(sumar, cambia buf)   el bufer queda PRESTADO a la 3060
   imprime(buf[0])                          NO compila: la 3060 aun lo tiene
   espera(trabajo)
   imprime(buf[0])                          bien
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
                   `red` NO COMPILA
```

Antes de ejecutar un juego se sabe **exactamente** que puede tocar: si no pidio
`red`, no hay una linea en el que pueda hablar con la red, y eso lo demostro el
compilador, no una promesa. C++ no puede: no sabe que es un permiso. Hay
lenguajes de investigacion con capacidades; ninguno tiene debajo un kernel que
las haga cumplir.

### U3. MODULAR dentro del compilador

La regla principal de la casa (MODULAR, y la ley L8: la dependencia solo baja)
hoy la hacen cumplir guardianes de Python sobre el Rust. En TITAN++ la cumple
el compilador:

```text
   cada modulo dice en UNA linea que hace          sin ella, no compila
   cada modulo dice con que conecta (`usa`)        y el compilador lo compara
                                                   con lo que de verdad llama
   las capas solo bajan                            un ciclo es un error
```

Un juego de TITAN++ **no se puede volver un monolito sin que el compilador lo
diga**.

### U4. El `.bex` lleva su fuente, y F1 lo abre

La idea ya estaba en `INTI_MAESTRO` 13g.5: la seccion `Resources` (0x0B) del
formato BEF puede llevar el fuente que produjo el `.bex`. En TITAN++, con
`fuente = true` en el manifiesto:

```text
   lo que corre es lo que puedes leer    y esta firmado
   se comprueba recompilando             el mismo fuente da los mismos bits
   F1 abre cualquier programa TITAN++    lo lees, lo cambias, lo vuelves a construir
```

---

## 4. La herramienta: `titan`, `Titan.toml` y los modulos

### 4.1 Las ordenes (como cargo)

```text
   titan new asteroides    un paquete nuevo, con su Titan.toml y su src/
   titan check             tipos + comprobador, sin emitir nada (rapido)
   titan build             el .bex
   titan run               construir y lanzar
   titan test              las pruebas del paquete
```

Los nombres en ingles como cargo, o en castellano como INTI: lo decide el
propietario (seccion 13). Uno solo, no los dos.

[!] **`titan run` choca con una regla de F1**, y hay que decirlo ahora:
`PLAN_ESTRUCTURA.md` seccion 3 -- un `.bex` **no puede lanzar otro** (`EJECUTAR`
se fija al nacer y solo desde Ring 0). Dentro de F1, `titan run` construye y le
pide al ESCRITORIO que lo lance; el que da el clic es el propietario. En el
anfitrion (Windows), `titan run` no existe: alli solo hay `build`.

### 4.2 El manifiesto: `Titan.toml`

Boceto; las CLAVES las decide el propietario con las palabras (seccion 13):

```toml
[paquete]
nombre  = "asteroides"
version = "0.1.0"
edicion = "2026"
fuente  = true            # U4: el .bex lleva su fuente

[permisos]                # U2: al .bex Y al compilador
pantalla = true
entrada  = true
sonido   = true
gpu      = "dibujo"       # "dibujo" | "computo"
disco    = "lectura"      # solo su carpeta
red      = false

[dependencias]
fisica = { ruta = "../fisica" }
motor  = { ruta = "../motor", huella = "sha256:..." }   # si cambia, no compila

[perfil.rapido]
optimizar = 3
```

** **Sin registro de internet**, a proposito. Las dependencias van por RUTA, y
con HUELLA si son de otro. La casa cerro Ring 0 a externos por el caso xz (una
dependencia envenenada); TITAN++ no abre esa puerta por el lado de las apps:
lo que entra en un paquete se ve en su manifiesto y su huella se comprueba.

### 4.3 Los modulos: una sola forma

Rust tiene dos formas de poner un modulo en carpetas (`mod.rs` o `nombre.rs`
junto a `nombre/`), y confunde. TITAN++ tiene UNA:

```text
   asteroides/
      Titan.toml
      src/
         main.titan        mod nave, roca, fisica
         nave.titan
         roca.titan
         fisica.titan      mod choque          (fisica es carpeta Y fichero)
         fisica/
            choque.titan
```

Y cada fichero empieza diciendo que es y con que conecta (U3):

```text
   mod fisica "mueve los cuerpos y resuelve los choques"
   usa nave, roca
```

---

## 5. Por que un compilador PROPIO: por F1

`plan/PLAN_ESTRUCTURA.md`: se pulsa **F1** en el escritorio y se abre **el
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
inspiracion**: la arquitectura (texto -> arbol -> tipos -> comprobador ->
codigo) y la experiencia (`run`, el manifiesto, los modulos) se imitan; su
medida no.

**Lo que F1 pide antes**, y hoy no esta:

```text
   la ventana de F1               PLAN_ESTRUCTURA, casilla 1 (ABIERTA)
   el autohospedaje               PLAN_AUTOHOSPEDAJE (APARCADO)
   memoria dinamica en Ring 3     un asignador compartido en bmo-userland:
                                  hoy solo PROTON-X tiene #[global_allocator]
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

Razona sobre **tiempos de vida** (`'a`) que cruzan de una funcion a otra. Es de
lo mas dificil de rustc (decenas de miles de lineas, y mucho tiempo de gente que
no hacia otra cosa), y los tiempos de vida **se ven en la sintaxis**: `&'a mut
T`. Lo contrario de "pocas palabras".

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
| **2** | **valores mutables** (Swift, Hylo) | cada parametro dice si **lee**, **cambia** o **se queda** el valor | **local, funcion a funcion** |
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

Los modos de un parametro (las PALABRAS las elige el propietario):

```text
   lee       el que llama sigue siendo el propietario            (por defecto)
   cambia    se presta para cambiarlo; nadie mas lo toca mientras dura
   se queda  el valor pasa a ser de la funcion
```

Boceto (NO gramatica):

```text
   fun mover(cambia jugador, lee mapa)
       jugador.x = jugador.x + 1
       si mapa.solido(jugador.x, jugador.y)
           jugador.x = jugador.x - 1

   mover(heroe, nivel)       bien
   mover(heroe, heroe.mapa)  NO: `heroe` se CAMBIA y a la vez se LEE su mapa
```

Sin un `&`, sin un `'a`.

### 6.5 Lo que se pierde, dicho ahora

**No se guardan referencias dentro de estructuras.** Un grafo se escribe con
**indices**. Es el precio real del modelo 2, y para juegos es una ventaja: los
motores rapidos de hoy (ECS: entidades que son indices, componentes en tablas)
ya se escriben asi.

### 6.6 Los mensajes

Un comprobador que dice "no" sin porque es el que hace odiar a Rust. TITAN++
hereda de INTI el **formato de cuatro partes**: que paso, donde, por que es un
error, y como se arregla. Cada regla del comprobador entra con su programa roto
de prueba, como el juez del SASS.

### 6.7 Los precedentes

```text
   Swift   inout + la ley de exclusividad, en produccion desde hace una decada
   Hylo    "mutable value semantics": el modelo 2 llevado al extremo
   Mojo    sintaxis tipo Python + propiedad + GPU, pero con muchas palabras
   Rust    el origen de todo. Modelo 1
```

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

Las funciones de GPU de TITAN++ **bajan al subconjunto de SPIR-V** que la casa
ya lleva a SASS: se reutilizan emisor y juez enteros. Y el prestamo de un bufer
a la GPU es la U1.

---

## 9. IA: los dos sentidos

```text
   que una IA ESCRIBA TITAN++   pocas palabras + mensajes exactos + UNA forma de
                                hacer cada cosa = menos errores al generarlo
   cargas de IA EN la 3060      tensores: la seccion 8, y depende de lo mismo
```

El asistente de IA dentro de BMO-X sigue **APARCADO** (METAS cat. 2).

---

## 10. Lo que TITAN++ NO es

```text
   NO es Python          no corre programas de Python (eso es PYTHON_MAESTRO)
   NO es Rust            no compila crates de crates.io ni usa rustc
   NO es C++             no lee C++ (para eso esta el frontend de C++)
   NO reemplaza al .bex  .titan es el fuente; lo que corre es un .bex (BEF2)
   NO escribe el kernel  el kernel y los drivers son la Rust base; el metal, INTI
   NO sustituye a INTI   INTI y sus apps se quedan
   NO tiene recolector   la memoria se libera cuando su propietario acaba
   NO guarda referencias en estructuras (6.5)
   NO tiene JIT          codigo que se escribe en marcha es codigo que el juez
                         no puede ver
   NO baja de internet   dependencias por ruta y con huella (4.2)
   NO es multiarquitectura  x86-64 en la CPU y sm_86 en la 3060
```

---

## 11. Los riesgos

| riesgo | por que existe | que lo vigila |
|---|---|---|
| **crecer como C++** | C++ agrego durante cuatro decadas y nunca quito | un TECHO de palabras: la tabla de la gramatica tiene un maximo y un guardian lo cuenta, como el techo de no-ASCII. Una palabra nueva entra solo si quita una confusion |
| **ABC**: "facil" ya fracaso | no se podia extender y obligaba a vivir en su entorno (`INTI_MAESTRO` seccion 2) | enlaza con C, INTI y REX desde el primer dia; F1 es opcional, no una jaula |
| un TERCER lenguaje propio | Rust base + INTI + TITAN++, una persona | la frontera de la seccion 1: si cabe en INTI, va en INTI |
| el comprobador crece sin control | el de Rust es de lo mas grande de rustc | el modelo 2 es LOCAL por funcion; cada regla entra con su programa roto |
| los juegos piden velocidad | "facil y lento" no sirve para juegos | el metro del emisor mide TITAN++ igual que a C e INTI |
| las palabras cambian a mitad | una gramatica que se mueve rompe todo lo escrito | la gramatica la escribe el propietario ANTES del lexer (T0) |

---

## 12. El orden, un escalon cada vez

```text
   T0  este documento + la GRAMATICA escrita por el propietario (las palabras,
       las ordenes, las claves del Titan.toml)
   T1  texto -> arbol, en el anfitrion; `titan check` lee Titan.toml y los
       modulos. Los mensajes ya en cuatro partes
   T2  tipos + "ya lo entregaste" (paso A) + la linea de cada modulo y sus
       `usa` (U3)
   T3  `titan build`: un .bo por el emisor-x86_64, enlazado con bmo-enlazar, con
       los [permisos] en el .bex (U2). Un "hola" en una ventana, visto en el Ryzen
   T4  la ley de exclusividad (paso B): el modelo 2 entero
   T5  funciones de GPU -> SPIR-V -> SASS, y el prestamo a la 3060 (U1)
                                   pide: lanzar computo en ga10x
   T6  el compilador DENTRO de F1, `titan run`, y el .bex con su fuente (U4)
                                   pide: ESTRUCTURA, el asignador de Ring 3 y el
                                   autohospedaje
```

Sin fechas a proposito: una estimacion de un lenguaje que no existe es una
estimacion de otro proyecto (LEY 24).

---

## 13. Lo que decide el propietario antes de T1

1. ~~La extension~~ -> **`.titan`**, decidida el 29-09.
2. **Las palabras**: castellano en ASCII como INTI, o ingles como cargo? Vale
   para el lenguaje, las ordenes (`run` o `corre`) y las claves del
   `Titan.toml`. Una sola eleccion para las tres.
3. **El modelo 2** (sin referencias guardadas, sin un `'a`): confirmado?
4. **El techo de palabras**: cuantas como maximo? (Python tiene unas 35
   palabras reservadas; C++ pasa de 90.)
5. **El primer programa**: un juego chico, una app con ventana, o un calculo
   en la 3060? Decide que se construye primero en T3.
