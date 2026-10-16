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

# 2. ★★★ EL NUMERO: EL DUELO

El MISMO programa escrito en C y en TITAN++ (`toolchain/tools/metro/duelo/`),
compilado por los emisores de la casa y corrido en el emulador. Se cuentan
instrucciones (el metro: no son ciclos, pero son lo mismo en cualquier
maquina). Lo que imprimen es identico, o el duelo no vale.
`metro --duelo` lo escribe; y los ocho programas estan en el banco del metro,
asi que sus pasos son TRINQUETE: solo pueden bajar.

```text
   duelo      que mide                         16-10 al    16-10 tras
                                               empezar     AF0 + AF1
   ---------------------------------------------------------------------
   primos     bucles, `%`, `break`              3,0 x C     2,33 x C
   fib        llamadas (fib 22 recursivo)       2,6 x C     2,31 x C
   bytes      llenar y sumar 200000 bytes       3,4 x C     3,03 x C
   ordenar    indices y un while con `and`      2,9 x C     2,43 x C
```

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
- [ ] **AF2 -- el valor que ya esta en el registro.** No guardar en la pila
      lo que se va a leer en la instruccion siguiente, ni releer lo que se
      acaba de escribir: E1 recuerda que hay en RAX hasta que algo lo pisa
- [ ] **AF3 -- los locales enteros en REGISTROS.** Los contadores de un
      bucle (`k`, `d`, `j`, `i`) viven en un registro mientras el bucle
      corre, como hizo INTI en I1 (`PLAN_EL_TROQUEL.md`: -34 % de accesos).
      El que mas promete, y el mas delicado: las llamadas y los `mut`
      prestados lo obligan a guardarlos
- [ ] **AF4 -- `for i in range(...)` como el `for` de C.** Hoy el contador y
      su tope van y vienen de la pila en cada vuelta
- [ ] **AF5 -- EL PRECIO DE LA SEGURIDAD, medido.** Cuantas instrucciones del
      duelo son desbordes, indices y divisiones mirados. Y quitar SOLO los
      que se pueden DEMOSTRAR imposibles al compilar (`for i in range(len(a))`
      ya sabe que `a[i]` esta dentro): la seguridad se queda, la repeticion
      no
- [ ] **AF6 -- las listas de bytes.** `push` y la celda de una `[byte]` sin
      llamar a crecer en cada byte cuando ya cabe (`bytes` es el duelo peor)
- [ ] **AF7 -- las llamadas.** El marco de una fn pequenia sin lo que no usa
      (`fib`)

---

# 5. LO QUE ESTE PLAN NO HACE

* **No da palabras de maquina a TITAN++.** Ni `inline`, ni `register`, ni
  vectores, ni ensamblador. Quien quiera hablar con la CPU escribe INTI.
* **No quita la seguridad para ganar el duelo.** Solo la que se demuestra
  repetida (AF5).
* **No promete ciclos del Ryzen.** El duelo cuenta instrucciones; los
  ciclos los mide `c/coste.bex` en el metal, y se apuntaran cuando AF3 este.
