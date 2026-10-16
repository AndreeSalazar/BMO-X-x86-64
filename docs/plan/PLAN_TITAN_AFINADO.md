# PLAN TITAN AFINADO -- TITAN++ a la altura de C, sin aprender de CPU

> Pedido del propietario, **2026-10-16**:
>
> > *"vamos a afinar mas fuerte mi titan++ como nivel de Cpp y C pero sin
> > lastres del pasado, pero ojo que titan++ afine no significa que deba saber
> > de CPU porque inti hace eso mejor que titan++ y verrano con sus librerias
> > para las GPU [...] en C y Cpp puedo entender pero no asi, porque se
> > llenaria de datos o escrituras que no necesito"*.

---

# 1. LA LEY DE ESTE PLAN

```text
   TITAN++     dice QUE se hace: numeros, listas, textos, funciones. Nunca
               un registro, una cache, un `inline`, un `restrict`, un SIMD
               ni una instruccion. Lo que escribe el que programa no cambia
               para que vaya mas rapido
   E1          (su emisor) decide COMO: es el que sabe de x86-64, y todo lo
               que este plan afina vive ahi
   INTI        el lenguaje que SI habla con la CPU (`crudo`, las katanas)
   VERRANO     la GPU, con sus librerias
```

**Afinar TITAN++ es afinar su emisor, no su gramatica.** Si una mejora pide
una palabra nueva que habla de la maquina, no es de este plan: es de INTI.
Los lastres de C y C++ que NO entran: punteros a mano, `register`,
`volatile` para ir rapido, macros, cabeceras, el comportamiento indefinido
que el compilador aprovecha para correr.

Y lo que TITAN++ SI paga, a proposito, y se queda: mirar que una cuenta no
desborde (T0060), que un indice este dentro de la lista, que no se divida
por cero. Eso es lo que C no hace y por eso C explota. El precio se MIDE
(AF5); no se quita.

---

# 1b. EL FUEGO REAL -- lo que hace unico a BMO-X

El propietario, 16-10: *"bmo-X no es tipico como clang ni otros: prepara bien
el CPU y expulsa brutalmente todo para exprimir, y se aplicara en cualquier
perfil, solo en x86_64 [...] clang puede hacer su truco si quiere pero
cuando se trata del fuego real ahi es donde yo gano el terreno [...] el
monopolio del fuego real"*.

Clang y GCC hacen sus trucos AL COMPILAR, y despues el programa corre en un
sistema que no controlan: otros procesos, interrupciones, un planificador
que lo mueve de nucleo, memoria repartida, una frecuencia que sube y baja.
BMO-X es la maquina ENTERA. Al CORRER puede:

```text
   preparar        un nucleo para el trabajo y nada mas en el (sin otros
                   procesos ni sus interrupciones), la topologia y la cache
                   que el kernel ya mide al arrancar (`cpu_vendor/`)
   la memoria      el trabajo en memoria contigua y en paginas grandes,
                   cerca de su nucleo
   el silicio      lo que la CPU de ESTA maquina sabe hacer, leido al
                   arrancar y comparado con su perfil (B4): cualquier
                   x86-64, cada uno con lo suyo
   medir           los contadores del Zen 3 (C4): los ciclos de verdad
```

**La regla que se sigue de esto**: el juez final no es el emulador ni el
versus de instrucciones, es el RYZEN corriendo (AF10). Las fases A y B
ponen el codigo de TITAN++ a la altura; el fuego real es donde BMO-X gana.

---

# 2. ★★★ EL NUMERO: EL DUELO

El MISMO programa escrito en C y en TITAN++ (`toolchain/tools/metro/duelo/`),
compilado por los emisores de la casa y corrido en el emulador. Se cuentan
instrucciones (el metro: no son ciclos, pero son lo mismo en cualquier
maquina). Lo que imprimen es identico, o el duelo no vale.
`metro --duelo` lo escribe; y los ocho programas estan en el banco del metro,
asi que sus pasos son TRINQUETE: solo pueden bajar.

```text
   duelo      que mide                         16-10 al    tras        tras AF3,
                                               empezar     AF0 + AF1   AF4 y AF6
   ---------------------------------------------------------------------------
   primos     bucles, `%`, `break`              3,0 x C     2,33 x C    1,33 x C
   fib        llamadas (fib 22 recursivo)       2,6 x C     2,31 x C    1,25 x C *
   bytes      llenar y sumar 200000 bytes       3,4 x C     3,03 x C    1,50 x C
   ordenar    indices y un while con `and`      2,9 x C     2,43 x C    1,11 x C *
```
(* tras AF7, la fn ligera (`fib`), y AF5a, la celda con una comparacion
(`ordenar`). **Los cuatro en la meta de la fase A.**)

**La meta: 1,5 x C o menos en los cuatro**, con las comprobaciones de
seguridad puestas (C no las tiene: igualarlo del todo seria quitarlas).

---

# 2b. ★★★ EL VERSUS: contra el C, el C++ y el Rust DE SIEMPRE

El propietario, 16-10: *"me gustaria saber [...] en velocidad que traiga en
mi bmo-X vs C y Cpp y rust el estandar [...] para que tu pruebes luego y eso
poner como versus"*. Lo corre `python toolchain/tools/metro/versus.py`:
los mismos cuatro programas (`metro/duelo/estandar/`) por GCC 13 y Clang 18 a
`-O2` (C y C++) y rustc 1.101 `-O`, contados por valgrind con la MISMA vara
que el metro de BMO-X -- instrucciones de x86-64 ejecutadas --, y sin lo que
gasta cada uno en arrancar (su programa vacio se resta). Lo que imprimen es
identico, o el versus no vale.

```text
   instrucciones del TRABAJO, 16-10 (tras AF1)

   duelo       TITAN++    C casa     gcc C   clang C   g++ C++  clang++   rustc
   ---------------------------------------------------------------------------
   primos   11.096.584  4.752.793  2.986.596 3.691.408 2.987.501 3.692.286 4.894.404
   fib       2.378.742  1.031.745    611.965   630.590   612.214   630.805   562.635
   bytes    18.202.302  6.000.176  2.002.020   590.006 3.473.108 2.536.513 2.545.243
   ordenar   6.172.762  2.537.282    643.202   908.967   758.051   922.848   554.486

   cuantas veces MAS hace TITAN++ (1,0 es empate)

   duelo      C casa   gcc C  clang C  g++ C++  clang++  rustc
   ------------------------------------------------------------
   primos       2,3     3,7     3,0      3,7      3,0     2,3
   fib          2,3     3,9     3,8      3,9      3,8     4,2
   bytes        3,0     9,1    30,9      5,2      7,2     7,2
   ordenar      2,4     9,6     6,8      8,1      6,7    11,1
```

Como se lee, sin adornarlo:

* **Hoy TITAN++ hace de 2,3 a 11 veces las instrucciones del estandar**, y
  de 2,3 a 3 veces las del C de la casa. Es la distancia de un emisor joven
  (todo pasa por la pila) a cuarenta anios de GCC y LLVM.
* **`bytes` contra Clang (30,9)** es el SIMD: Clang suma 16 bytes por
  instruccion. Eso no lo tendra TITAN++ escribiendo vectores (la ley de este
  plan), sino el emisor haciendolo solo, o la 3060 por VERRANO.
* **Rust es la referencia justa**: mira los indices como TITAN++ (C y C++
  no miran nada). Y es lo que las apps de Rust de BMO-X YA son: se compilan
  con este mismo rustc (LLVM). En `primos`, TITAN++ ya hace 2,3 veces lo
  de Rust.
* **La meta realista, en orden:** el C de la casa (1,5 x), despues Rust en
  los bucles enteros (AF3, los registros, es el salto grande), y el SIMD
  como trabajo del emisor, no del que escribe.

**Tras AF7 y AF5a** (el versus, otra vez el 16-10): fib 1,3 x el C de la
casa y 2,3 x Rust; ordenar 1,1 x el C de la casa y 5,1 x Rust (Rust no
llama a nada en su bucle y sus indices ya los demuestra LLVM: es la fase
B). primos y bytes, como abajo.

**El mismo dia, tras AF3, AF4 y AF6** (el versus vuelto a correr):

```text
   duelo       TITAN++   C casa   gcc C  clang C  g++  clang++  rustc  gcc Zen3  clang Zen3
   ----------------------------------------------------------------------------------------
   primos    6.339.380   1,3 x    2,1 x   1,7 x  2,1 x  1,7 x   1,3 x   2,2 x     1,7 x
   fib       1.891.583   1,8 x    3,1 x   3,0 x  3,1 x  3,0 x   3,4 x   3,2 x     3,0 x
   bytes     9.002.405   1,5 x    4,5 x  15,3 x  2,6 x  3,5 x   3,5 x  24,7 x    47,4 x
   ordenar   3.989.054   1,6 x    6,2 x   4,4 x  5,3 x  4,3 x   7,2 x   6,3 x     5,3 x
```

En `primos` TITAN++ hace ya las mismas instrucciones que Rust de mas
(1,3 x) y le queda 1,7 x a Clang. `bytes` y `primos` estan en la meta de
la fase A (1,5 x el C de la casa); `fib` (las llamadas, AF7) y `ordenar`
(los indices, AF5) son lo que falta.

Y los veteranos A TOPE (`-O3 -march=znver3`, la CPU de BMO-X), el mismo dia
(antes de AF2):

```text
   duelo      gcc Zen3    clang Zen3    TITAN++ es
   ------------------------------------------------
   primos    2.946.597    3.632.247    3,8 x / 3,1 x
   fib         598.462      630.590    4,0 x / 3,8 x
   bytes       364.569      190.018   49,9 x / 95,8 x
   ordenar     637.753      750.314    9,7 x / 8,2 x
```

**Lo que dice**: en el codigo de todos los dias (bucles, llamadas,
indices) `-O3` y conocer la CPU casi no mueven a los veteranos: ahi todo
es emitir bien (registros, saltos, no mover de mas). Solo el bucle de DATOS
(`bytes`) se dispara, y es el SIMD (AVX2: 32 bytes por instruccion).

---

# 2c. ★★★ SUPERAR A LOS VETERANOS: donde la casa tiene ventaja de verdad

El propietario, 16-10: *"puedes intentar superar los veteranos [...] en la
casa mi bmo-X ese mismo, pueda exprimir con brutalidad"*.

Ganarle a GCC y a LLVM EN TODO es trabajo de anios y de miles de personas;
no se promete. Pero hay seis cosas que la casa SABE y ellos NO, y ahi si se
les puede ganar -- sin que TITAN++ aprenda de CPU: lo sabe el emisor.

```text
   1  UNA SOLA CPU        GCC compila para "cualquier x86-64" (SSE2) salvo
                          que se lo pidan; BMO-X tiene SU Ryzen escrito en
                          PERFIL/CPU.txt. Pero usar AVX2 lo DECIDE el
                          propietario: hoy su columna `mano` dice `no`, y su
                          `visto` espera la foto de `ext` en el Ryzen
   2  SIN ALIAS           en TITAN++ dos nombres nunca son la misma memoria
                          (prestar y leer lo mismo es T0076): el emisor puede
                          tener valores en registros y vectorizar SIN mirar.
                          C necesita `restrict` (y casi nadie lo escribe)
   3  EL PROGRAMA ENTERO  `build_package` ve todos los modulos: una fn
                          chica se copia dentro de quien la llama, y las
                          llamadas de dentro no pagan la convencion del
                          sistema (lo que cuesta `fib`)
   4  LA SEGURIDAD        TITAN++ mira indices como Rust; lo que se puede
       DEMOSTRADA         DEMOSTRAR dentro (`for i in range(len(a))`) no se
                          mira dos veces. Ahi se le gana a Rust, que no
                          siempre lo demuestra
   5  EL RELOJ DEL METAL  los contadores del Zen 3 ya se leen desde TITAN++
                          (`director.contar`, C4): el emisor puede medir
                          ciclos de verdad en el Ryzen y elegir con ellos
   6  LA 3060             un bucle de datos grande no tiene por que correr
                          en la CPU: `gpu fn` y VERRANO (el SIMD de verdad
                          son 3584 nucleos)
```

**La escalera hacia ellos**, cada fase juzgada por el versus:

```text
   FASE A  alcanzar el C de la casa (1,5 x)        AF2, AF3, AF4
   FASE B  alcanzar a Rust y a GCC -O2 (~1 x)      AF3 entero, AF5, AF7
                                                   (registros, seguridad
                                                   demostrada, llamadas)
   FASE C  GANARLES donde la casa sabe mas         AF8 a AF10, abajo
```

---

# 3. DONDE SE VAN LAS INSTRUCCIONES (el bucle de `primos`, desensamblado)

```text
   mov [rbp-0x68], rax     el resultado, a la pila...
   mov rax, [rbp-0x68]     ...y de vuelta, en la instruccion siguiente
   cmp rax, rdx
   setle al                una comparacion hecha un si/no,
   movzx rax, al           guardado,
   mov [rbp-0x70], rax     leido otra vez
   mov rax, [rbp-0x70]     y mirado
   test rax, rax           para saltar
   je ...
   movabs rax, 0x0         un cero de diez bytes
   jo ...                  (la seguridad: se queda)
```

Casi la mitad de lo que hace es MOVER: cada valor va a la pila y vuelve, cada
local vive en memoria, y cada comparacion se fabrica y se vuelve a mirar.

---

# 4. LA ESCALERA

Cada peldanio: el duelo antes y despues, el banco entero de TITAN++ (E0 == E1
en sus cientos de programas) y el metro sin que ninguna salida cambie.

- [x] **AF0 -- los numeros cortos.** Un numero que cabe en 32 bits se carga
      con 5 o 7 bytes y no con 10 (sin `xor`: no pisa las banderas de un
      `cmp` pendiente). **16-10:** el banco del metro baja de 494846 a
      474143 bytes de codigo con AF1
- [x] **AF1 -- saltar con el `cmp`.** Un `if` o un `while` que compara dos
      enteros (`<`, `<=`, `>`, `>=`, `==`, `!=`, y un `and` de ellos, con su
      atajo) salta con la comparacion misma, sin el si/no de en medio.
      **16-10:** primos 3,0 -> 2,33, ordenar 2,9 -> 2,43; el banco del metro
      entero, -5,7 % de instrucciones; ninguna salida cambio
- [x] **AF-1 (antes de AF0) -- lo que solo se LEE no se copia** (16-10, por
      la letra de ECCE): una lista que una fn solo lee le llega como puntero.
      200 millones de pasos a 1,2 en la prueba (`tests/prestado.rs`)
- [x] **AF-V -- EL VERSUS** (16-10): `metro/versus.py` contra GCC, Clang y
      rustc, con la tabla de la seccion 2b. Se vuelve a correr tras cada
      peldanio y la tabla se reescribe aqui
- [x] **AF2 -- el valor que ya esta en el registro.** Un `load` justo
      detras del `store` del mismo sitio usa el registro; cada sitio al que
      puede llegar un salto pasa por `etiqueta()` y lo olvida. Y **AF2b**:
      `x + 1`, `x - 1` e `if x < 10` con el numero DENTRO de la instruccion;
      `let x = 0` y `x = 1` derechos a su sitio. **16-10:** contra el C de la
      casa, primos 2,33 -> 1,73, fib 2,31 -> 1,94, bytes 3,03 -> 2,67,
      ordenar 2,43 -> 2,04; el banco del metro, 60,5 -> 53,6 millones de
      instrucciones y -19 % de accesos a memoria
- [x] **AF3 -- los locales enteros en REGISTROS** (16-10, el segundo
      intento). El primero se deshizo: `r14` era del MONTON y `bytes` dio
      otra suma. Ahora, en tres pasos:
      - **un registro libre**: `r15` apunta a un bloque de 16 bytes en la
        pila del programa -- `[r15]` la pila de las llamadas abiertas,
        `[r15 + 8]` el estado del monton --, y las subrutinas del monton
        cargan `r14` de alli al entrar y lo devuelven al salir
      - **el local que mas pesa** (cada uso, 10 por cada bucle que lo
        rodea; solo enteros, nunca prestados ni `mut`, y no en una fn que
        llama a una `gpu fn`) vive en `r14`, guardado al entrar y devuelto
        al volver
      - **operar EN su sitio**: `x = x + 1` es un `add` al registro o a la
        pila (con su `jo`), e `if x < 10` un `cmp` alli mismo
      **El duelo contra el C de la casa:** primos 1,73 -> 1,54, fib 1,94 ->
      1,89, bytes 2,67 -> 2,43, ordenar 2,04 -> 1,90. El banco del metro:
      53,6 -> 50,7 millones de instrucciones y 22,4 -> 18,6 millones de
      accesos (-17 %).
      **Lo que SUBIO, y se acepto a la vista** (el trinquete se rehizo con
      este motivo): el bloque cuesta 2 a 4 instrucciones al arrancar cada
      programa; cada `push` o `soltar` del monton, 3 (guardar y devolver
      `r14`); y la cuenta de la pila, al vivir en memoria, son 3 accesos por
      llamada (`fib`, que solo llama: +17 % de accesos, con menos
      instrucciones). Doce programas chicos suben unas pocas instrucciones.
      **[!] Las pruebas que NO corrian (16-10, despues de AF3).** Desde AF2
      las pruebas de dentro del emisor no compilaban (`gpu.rs` construia el
      E1 sin los campos nuevos) y el filtro con el que se leia el resultado
      no lo mostraba: el "todo pasa" de AF2 y AF3 no estaba comprobado.
      Al correrlas de verdad salieron dos NO, y los dos eran del EMULADOR,
      no de TITAN++: el `add` con inmediato (`83 /0`, `81 /0`, `05`) ponia
      el desborde siempre a falso -- el mismo hueco que el `add` de
      registros tuvo hasta el 19-08 --, y como AF2b/AF3 suman EN SU SITIO
      (`add [x], 1` + `jo`), un `x + 1` que no cabe dejo de atrapar en el
      emulador (en el silicio si atrapaba). Arreglado alli; y `ritmo` pedia
      un fotograma TARDE con 500 000 vueltas, que TITAN++ ya hace en ~13 ms:
      ahora un millon. Desde hoy se lee la linea `test result` de cada
      prueba, no un filtro de fallos
- [x] **AF4 -- `for i in range(...)` como el `for` de C, y las cuentas en
      el registro** (16-10). Cuatro cosas del emisor:
      - **la vuelta ES su contador**: `i` y el contador escondido viven en
        el mismo sitio (`i` no se puede cambiar y el contador solo cambia
        cuando `i` ya murio): la copia de cada vuelta sobra
      - **las cuentas de enteros DERECHO en `rax`**: `(i * 7) % 256` ya no
        deja cada paso en la pila para leerlo otra vez, y `let x = ...` /
        `x = ...` lo guardan una vez, en `x`
      - **el `cmp` lee el otro lado de su sitio** (`cmp r14, [fin]`), sin
        cargarlo antes
      - **un divisor escrito** (`% 256`) que no es 0 ni -1 no se mira al
        correr: se sabe al compilar que no atrapa
      El duelo: primos 1,54 -> 1,33, bytes 2,43 -> 1,93; el banco del metro
      50,7 -> 45,7 millones de instrucciones, y ninguno de los 72 subio
- [x] **AF5a -- la celda mirada con UNA comparacion** (16-10). `a[i]`
      miraba `i >= 0` y `i < len` (cinco instrucciones); ahora un `cmp` sin
      signo y un `jae`: un negativo, visto sin signo, es enorme y se sale
      igual. La MISMA seguridad (T0072, la misma linea), en listas, tablas y
      laminas; y el indice se calcula derecho en `rax`. El duelo: ordenar
      1,57 -> 1,11. El banco 40,1 -> 38,9 millones. `tests/e1.rs`
      (`a_typed_list_cell_below_zero_or_past_the_end_is_a_no`), con su
      sabotaje (el salto con signo: un -1 pasaria y lo dice)
- [ ] **AF5 -- EL PRECIO DE LA SEGURIDAD, medido.** Cuantas instrucciones del
      duelo son desbordes, indices y divisiones mirados. Y quitar SOLO los
      que se pueden DEMOSTRAR imposibles al compilar (`for i in range(len(a))`
      ya sabe que `a[i]` esta dentro): la seguridad se queda, la repeticion
      no
- [x] **AF6 -- las listas** (16-10). `push` y la celda de una lista:
      - la celda de 1 byte no se multiplica por 1, y la de 8 se corre
        (`shl`); la direccion se suma de su sitio (`add rax, [lista]`)
      - el valor que entra no se copia aparte si es un numero del marco
        (solo una CELDA se mueve al crecer la lista)
      - lo que mide la lista sube EN su sitio (`add [n], 1`)
      - `byte(x)` mira con UN `cmp` sin signo (un negativo, sin signo, es
        enorme): dos instrucciones en vez de cinco
      El duelo: bytes 1,93 -> 1,50, ordenar 1,86 -> 1,57; el banco 45,7 ->
      42,2 millones de instrucciones y 15,6 -> 14,2 millones de accesos.
      **[!] Un paso intermedio perdio la capacidad** (crecer la doblaba
      desde `rcx`, y el `cmp` nuevo ya no la dejaba alli): el monton se
      acababa al quinto `push`. Lo vio el duelo (no acababa); ahora una
      prueba lo dice (`tests/bytes.rs`)
- [x] **AF7 -- las llamadas: la fn LIGERA** (16-10). Una fn de 1 a 4 `int`
      copiados que da un `int` (o nada) recibe sus valores en `rdx`, `rcx`,
      `r8` y `r9` y devuelve el resultado en `rax`: sin el bloque de
      punteros, sin el sitio del resultado y sin copiar cada valor de un
      puntero al entrar. Lo decide el emisor (`e1/llamada.rs`, `ligera`):
      ni la de un trait ni una fn a la que salta un trait (su salto lleva el
      bloque), ni `main`. Si al volver muere algo que soltar, el resultado
      espera aparte; si no, ni se guarda.
      El duelo: fib 1,83 -> 1,25. El banco 42,2 -> 40,1 millones de
      instrucciones y 14,2 -> 12,5 millones de accesos. `tests/ligera.rs`,
      con su sabotaje (los registros cruzados al llegar: lo dice)
- [ ] **AF8 -- COPIAR DENTRO (fase C, ventaja 3).** Una fn chica de un
      paquete se emite dentro de quien la llama: sin `call`, sin marco
- [x] **AF9a -- LA SUMA DE UNA LISTA DE BYTES EN VECTORES** (16-10,
      `e1/vector.rs`). El propietario: *"sobre simd eso usa aprende de
      clang y superarlo"*. Lo que hace Clang: `psadbw` contra cero suma 8
      bytes, y un `xmm` hace dos a la vez. El emisor reconoce `for b in a`
      sobre una `[byte]` cuyo cuerpo es solo `s = s + b`, y ANTES del
      bucle suma de 32 en 32 (`movdqu`, `psadbw`, `paddq`); el bucle de
      siempre hace lo que queda. El mismo NO en la misma linea (los bytes
      solo suben la suma). SSE2: toda CPU de x86-64. El emulador aprendio
      `movdqu`, `pshufd`, `paddq`, `pxor` y `psadbw` (con su prueba contra
      la suma uno a uno). El duelo: bytes 1,03 -> 0,62 x el C de la casa,
      1,5 x Rust y 1,1 x g++. El banco 33,1 -> 30,6 millones de
      instrucciones y 9,9 -> 7,9 millones de accesos. Sube el codigo de
      `bytes` (+127 B: la parte en vectores), a la vista. `tests/bytes.rs`
      (largos 0, 1, 31, 32, 33 y 1000; el desborde en su linea), con
      sabotaje (el segundo bloque de 16 leyendo el primero: lo dice)
- [ ] **AF9 -- el resto del bucle de DATOS en vectores.** Llenar una lista
      (`push` de una cuenta), otras reducciones (`[int]`, max, min) y, con
      la decision del propietario en PERFIL, AVX2 (32 bytes por
      instruccion). Clang a tope (`-O3 -march=znver3`) sigue 10 x por
      delante en `bytes`: llena la lista en vectores. **Lo decide el
      propietario** (AVX2)
- [ ] **AF10 -- MEDIR EN EL RYZEN (fase C, ventaja 5).** El duelo corrido en
      el metal con `director.contar`: ciclos de verdad, no instrucciones,
      para TITAN++ y para el C de la casa. Y el dia que gane, que lo diga el
      Ryzen y no el emulador

---

# 4b. ★★★ FASE B: A LA ALTURA DE LOS VETERANOS -- emision PRECISA

El propietario, 16-10: *"tomando lo que ellos hicieron pero mi casa BMO-X
pueda exprimir en emision PRECISA"*. Lo que GCC y LLVM hacen, hecho por el
emisor de la casa con lo que la casa SABE (una CPU, sin alias, el programa
entero, la seguridad demostrada). TITAN++ sigue sin una palabra de CPU.

```text
   B1  mas locales en REGISTROS          r14 siempre; rbx, r12 y r13 si el
                                         programa no habla con el director.
                                         El otro lado de una cuenta, de su
                                         registro o su sitio (add rax, r14)
   B2  LO MENOS QUE VALE cada local,     d = 2 y d = d + 1: d >= 2 SIEMPRE.
       demostrado al compilar            Su division no mira el 0 ni el -1;
                                         x % 256 de un no negativo es un and
   B3  EL BUCLE GIRADO                   la vuelta mira la condicion abajo y
                                         salta arriba solo si se cumple: un
                                         salto por vuelta, no dos
   B4  instrucciones PRECISAS            imul con el numero dentro, byte(x)
                                         que se demuestra que cabe, el valor
                                         de una fn ligera derecho a su
                                         registro, a + b con b ya en rax
   B5  LA FORMA INTERMEDIA con           el salto grande que queda: cada
       registros virtuales               paso a un registro, y el reparto al
                                         final (lo que GCC y LLVM hacen). Hoy
                                         cada paso de una cuenta compleja
                                         pasa por la pila
   B6  AF5 entero                        la celda que se demuestra dentro
                                         (`for i in range(len(a))`) no se
                                         mira en cada vuelta
   B7  lo que no cambia, FUERA del       `len(a)`, la direccion de la lista:
       bucle                             una vez, no en cada vuelta
```

- [x] **B1, B2, B3, B4** (16-10). Medido, el mismo dia (y tras B5a, mas
  abajo: bytes 1,0 x el C de la casa y 2,4 x Rust, ordenar 1,0 x y 4,6 x):

```text
   duelo      C casa   gcc C  clang C  g++  clang++  rustc  gcc Zen3  clang Zen3
   ------------------------------------------------------------------------------
   primos      0,9 x   1,4 x   1,1 x  1,4 x  1,1 x   0,9 x   1,4 x     1,2 x
   fib         1,1 x   1,9 x   1,9 x  1,9 x  1,9 x   2,1 x   2,0 x     1,9 x
   bytes       1,2 x   3,6 x  12,2 x  2,1 x  2,8 x   2,8 x  19,8 x    37,9 x
   ordenar     1,1 x   4,4 x   3,1 x  3,7 x  3,0 x   5,1 x   4,4 x     3,8 x
```

  **En `primos` TITAN++ hace MENOS instrucciones que el C de la casa y que
  Rust** (0,9 x), y esta a 1,1 de Clang, con cada desborde mirado. El banco
  del metro: 38,9 -> 34,8 millones de instrucciones, 12,3 -> 11,4 millones
  de accesos, y el codigo de los 72 baja (463.017 -> 462.577 B).
  **Lo que SUBIO, aceptado a la vista:** el bucle girado escribe la
  condicion dos veces (arriba, la primera vez, y abajo): cuatro programas
  crecen de 8 a 57 bytes (png, resolucion, cubo_gira, agenda). Es lo que
  GCC hace en `-O2`: un salto menos POR VUELTA a cambio de unos bytes una
  vez. Ninguna instruccion subio.
  Pruebas: `tests/ligera.rs` (lo demostrado no se mira, lo que no, atrapa en
  su linea; el bucle girado mira su condicion las mismas veces), con dos
  sabotajes (la resta ignorada en la cota; el salto sin dar la vuelta).
- [x] **B5a -- la CELDA derecha** (16-10). Leer `a[i]` de una lista de
      enteros o de bytes deja su direccion en `rax` y el valor encima, sin
      guardar la direccion en la pila; `a[i] = v` calcula `v` derecho y lo
      guarda una vez; `push` escribe en su celda desde `rcx`; la vuelta de
      `for x in lista` se lee igual. Y el bucle girado (B3) ya gira con
      `and` (`while j >= 0 and a[j] > v`). El duelo: ordenar 1,11 -> 1,00,
      bytes 1,20 -> 1,03. El banco 34,8 -> 33,5 millones, 11,4 -> 10,7
      millones de accesos. **Lo que subio, a la vista:** `agenda` +2
      accesos (al CRECER una lista la capacidad se lee dos veces; cada
      `push` que no crece ahorra una instruccion) y unos bytes de codigo en
      `ordenar` y `agenda` (la condicion con `and`, escrita dos veces).
      `tests/ligera.rs`, con sabotaje (la lectura derecha sin mirar el
      limite: lo dice)
- [x] **B5b -- E2, EL EMISOR CON REGISTROS VIRTUALES** (16-10, la primera
      pieza; `e1/e2.rs`). El propietario: *"no es siempre parche, siempre
      cambiar piezas"*. Una fn se baja a instrucciones de tres direcciones
      sobre registros VIRTUALES; se calcula cuales viven en cada sitio (con
      los bucles) y cuales cruzan una llamada; un barrido lineal los reparte
      (los que cruzan, a preservados; los demas, a los de paso) y cada uno
      pide el registro que necesita (el valor de una llamada, el suyo; el
      resultado, `rax`) si nada lo pisa. Los valores de una llamada van con
      un MOVIMIENTO PARALELO (`g(b, a)` cruza dos registros). Los NO, los de
      E1. Hoy toma las fn LIGERAS de enteros (cuentas, comparaciones con
      `and`/`or`, bucles, llamadas a otras ligeras); lo que no sabe, o lo
      que no cabe en los registros, sigue por E1, fn a fn.
      El duelo: fib 1,14 -> 1,06 (y su pila: sin marco, dos registros
      guardados). El banco 33,5 -> 33,1 millones de instrucciones, 10,7 ->
      9,9 millones de accesos (-7 %), y menos codigo; ninguno subio.
      `tests/ligera.rs` (`e2_gives_the_same_numbers_and_the_same_noes`), con
      sabotaje (las copias en orden en vez del movimiento paralelo: lo dice)
- [x] **B5c -- LA REGION: un bucle de CUALQUIER fn, en E2** (16-10). Una
      fn de E1 (con `print`, `lee`, `push`... fuera del bucle) emite por E2
      cada bucle que E2 sabe hacer entero: cuentas, comparaciones, `break`,
      celdas de listas que no crecen dentro (`a[i]`, `a[i] = v`, `len`, la
      vuelta de un `for`), `byte(x)`. Al ENTRAR: los locales a registros, y
      de cada lista su base y lo que mide UNA vez (lo que no cambia, fuera
      del bucle: B7 en esta pieza); dentro, la celda es UNA instruccion
      (`mov r, [base + i*8]`, como Rust) y se mira con un `cmp` sin signo;
      al SALIR, lo que cambio a su sitio. Los registros: los de paso, los de
      la division si en su tramo no hay division, los de E1 cuyo local entra
      (vuelven a el al salir), y los preservados que la fn no usa
      (guardados al entrar y devueltos al salir). Lo demostrado (B2) sirve
      tambien aqui. El duelo: ordenar 1,00 -> 0,65 x el C de la casa (3,0 x
      Rust, era 4,6), primos 0,88 -> 0,86 (0,8 x Rust). El banco 30,6 ->
      29,6 millones de instrucciones, 7,9 -> 7,1 millones de accesos. Sube,
      a la vista: `bytes` +8 instrucciones (un preservado guardado y
      devuelto al entrar y salir de un bucle) y unos bytes de codigo en
      `bytes` y `primos`. `tests/ligera.rs`
      (`a_loop_in_registers_gives_the_same_and_leaves_everything_in_its_place`),
      con sabotaje (prestar un registro de E1 cuyo local no entra: lo dice).
      **[!] Un fallo cazado por el banco:** la primera version prestaba los
      registros de E1 sin devolver lo que guardaban; `pixeles.titan`
      imprimia otra cosa y E0 lo dijo
- [ ] **B5 -- E2 entero.** Que E2 tome tambien las fn con listas, textos y
      el director (todas las de `main`), con el bucle girado y lo demostrado
      de E1; y lo que no cabe, a la pila (no a E1)
- [ ] **B6 -- AF5 entero**
- [ ] **B7 -- lo que no cambia, fuera del bucle**

---

# 5. LO QUE ESTE PLAN NO HACE

* **No da palabras de maquina a TITAN++.** Ni `inline`, ni `register`, ni
  vectores, ni ensamblador. Quien quiera hablar con la CPU escribe INTI.
* **No quita la seguridad para ganar el duelo.** Solo la que se demuestra
  repetida (AF5).
* **No promete ciclos del Ryzen.** El duelo cuenta instrucciones; los
  ciclos los mide `c/coste.bex` en el metal, y se apuntaran cuando AF3 este.
