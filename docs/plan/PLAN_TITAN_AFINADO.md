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
