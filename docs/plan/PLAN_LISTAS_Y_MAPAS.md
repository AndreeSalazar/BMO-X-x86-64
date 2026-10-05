# PLAN LISTAS Y MAPAS -- lo que CRECE mientras el programa corre

> Abierto el **2026-10-05**. El propietario, despues de cerrar E1: *"si dale,
> prepara el plan de listas y mapas"*. Salio de la pregunta de si TITAN++
> necesitaba "reconstruir Java": no hace falta otro lenguaje, hacen falta
> NIVELES, y este es el que mas falta.
>
> Es el **nivel 13** de TITAN++ (`toolchain/lang/titan/GRAMATICA.md`) y el
> primer valor de TITAN++ que vive en el MONTON en vez de en el marco.

---

## 0. La respuesta corta

```text
   que falta     una coleccion que CRECE: hoy solo hay tablas [T; n], de
                 medida fija y sabida al compilar
   que trae      [T]          una lista: crece y encoge al correr
                 {K: V}       un mapa: una clave lleva a un valor
                 0 palabras nuevas (las 25 se quedan en 25): tipos con
                 simbolos que ya existen, y funciones de la biblioteca
   lo dificil    no es la gramatica: es la MEMORIA. Una lista vive en el
                 monton, y TITAN++ no tiene recolector: se suelta cuando su
                 propietario acaba (la IR ya lo dice: `Op::Drop`)
   la vara       la de siempre: el calculo (E0) es el patron y E1 escribe
                 lo mismo, programa a programa y al azar -- y ademas, al
                 acabar, NADA queda pedido sin soltar
```

---

## 1. El limite de hoy, dicho entero

```text
   [int; 5]          cinco celdas, ni una mas: la medida es parte del tipo
   un texto          248 bytes como mucho al correr (E1)
   lo que NO se puede escribir hoy
     una agenda       cuantos contactos? los que se tecleen
     contar palabras  cuantas distintas? no se sabe hasta leerlas
     un inventario    nombre -> cuantos: un MAPA
     un compilador    tokens, arboles, tablas de nombres: todo crece
     un mundo         los trozos del mapa que se han cargado, por su
                      coordenada (seccion 6)
```

## 2. Lo que ya esta, y no se reinventa

| pieza | donde | que da |
|---|---|---|
| tablas como valores | `toolchain/lang/titan/src/calc.rs` (`Const::Table`) | el calculo ya guarda celdas en un `Vec`: una lista es casi lo mismo sin la medida en el tipo |
| el juez de prestamos | `toolchain/lang/titan/src/juez.rs` | copia por defecto, `mut` presta, `take` entrega: lo que una lista necesita para no copiarse sin querer |
| soltar al acabar | `Op::Drop` en `toolchain/lang/titan/src/ir.rs` | la IR ya dice DONDE muere cada local: ahi se suelta su memoria |
| un caso para lo que puede faltar | el nivel 8, y `numero(t)` (`toolchain/lang/titan/src/prelude.rs`) | sin null: lo que puede no estar es un caso que el `match` mira |
| la lista de libres | `platform/shared/bmo-monton/src/freelist.rs` | alineada a 16, junta lo soltado; la usan PROTON-X, HERMES, BANK CAT y la LUDOTECA |
| pedir memoria al kernel | `TASK_OP_MEMORIA_PEDIR` (el respaldo de `toolchain/lang/base/bmo-rt/src/heap/backend.rs`) | bloques de `KIND_MEMORIA`; el emulador ya la sabe (`toolchain/forge/bmo-lower/src/emu/sistema.rs`) |
| el emisor E1 | `toolchain/lang/titan/emisor-x86_64/src/e1/mod.rs` | cada valor con su forma fija (`e1/forma.rs`): una lista en el marco es su ASA (puntero, largo, capacidad) |

## 3. Las decisiones del propietario

### D1 -- como se escriben

```text
   (a) RECOMENDADA  con los simbolos que ya hay, como las tablas:
                      [int]                una lista de int
                      {text: int}          un mapa de texto a int
                      [1, 2, 3]            una lista escrita (si el tipo
                                           pide [int]); {"ana": 3} un mapa
                      push(mut l, x)  pop(mut l)  len(l)  for x in l
                      put(mut m, k, v)  get(m, k)  has(m, k)  remove(mut m, k)
                    en ingles corto, como print, len, range y round. 0
                    palabras nuevas
   (b)              palabras de tipo: list[int], map[text, int] -- se leen
                    solas, pero son dos nombres mas que aprender y `[ ]`
                    queda con dos usos parecidos
```

### D2 -- que pasa con `let b = a` (y al pasarla a una fn)

```text
   (a) RECOMENDADA  LA MISMA REGLA QUE TODO: una COPIA. `b` es otra lista;
                    tocar `b` no toca `a`. Para no copiar se escribe lo que
                    ya existe: f(mut l) presta, f(take l) entrega. El juez
                    ademas convierte en entrega la copia que es el ULTIMO
                    uso de `a` (no se ve, y no cambia nada de lo que se
                    escribe: solo ahorra la copia)
   (b)              como Rust: `let b = a` ENTREGA, y `a` deja de valer.
                    Mas rapido por defecto, pero una tabla y una lista
                    harian cosas distintas con la misma linea
```

### D3 -- lo que no esta

```text
   (a) RECOMENDADA  un CASO, como numero(t): get(m, k) da Hay(v) o NoHay,
                    pop(mut l) da Hay(x) o NoHay, y el match obliga a mirar
                    los dos. l[i] fuera de la lista es T0072 al correr,
                    como en una tabla
   (b)              un NO al correr (T00xx) cuando la clave no esta: mas
                    corto de escribir, pero es la excepcion por otra puerta
```

### D4 -- en que orden recorre `for k in m`

```text
   (a) RECOMENDADA  en el orden en que se METIERON las claves (como los
                    dict de Python): se puede leer, y el calculo y la
                    maquina dan lo mismo siempre -- el oraculo de E1 lo exige
   (b)              ordenado por la clave: tambien determinista, pero cada
                    `put` cuesta mas, y una clave de un tipo propio tendria
                    que saber ordenarse
```

## 4. La forma en la maquina (E1), para que se vea el coste

```text
   [T] en el marco      24 bytes: puntero a las celdas, largo, capacidad
   sus celdas           en el monton, seguidas, con la forma de T
                        (`e1/forma.rs`): una lista de registros son
                        registros seguidos, sin punteros dentro
   crecer               al llenarse, el doble: un bloque nuevo, copiar,
                        soltar el viejo
   {K: V}               las entradas en una lista (el orden de D4) y un
                        indice por hash con sondeo lineal; las claves:
                        int, text, bool y registros de esos (igualdad y
                        hash campo a campo)
   soltar               en cada `Op::Drop` y al salir de la fn: lo que el
                        juez ya sabe que muere
   el monton            `TASK_OP_MEMORIA_PEDIR` para los bloques y una lista
                        de libres como la de `bmo-monton`, emitida UNA vez
                        (como las subrutinas de E1)
```

Lo que solo falla al correr ATRAPA con su linea, como siempre: el monton se
acaba (T0060), un indice fuera (T0072). Nunca un puntero colgando: un valor
tiene UN propietario y nadie guarda referencias (TITAN_MAESTRO 14.3).

## 5. Los escalones

- [ ] L0 -- las decisiones D1-D4 del propietario, escritas aqui (seccion 3)
- [ ] L1 -- el FRONTEND: los tipos `[T]` y `{K: V}`, sus literales y las funciones de la biblioteca (`toolchain/lang/titan/src/parse/expr.rs`, `toolchain/lang/titan/src/check.rs`); el juez con copia, prestamo y entrega de una lista (`toolchain/lang/titan/src/juez.rs`), y la copia del ultimo uso convertida en entrega
- [ ] L2 -- el CALCULO (E0): `Const::List` y `Const::Map` con el orden de D4 (`toolchain/lang/titan/src/calc.rs`), sus clases (`toolchain/lang/titan/src/calc/clase.rs`) y los casos de D3 en el preludio (`toolchain/lang/titan/src/prelude.rs`)
- [ ] L3 -- el MONTON de E1: pedir bloques con `TASK_OP_MEMORIA_PEDIR` y una lista de libres emitida una vez, como `platform/shared/bmo-monton/src/freelist.rs`, en `toolchain/lang/titan/emisor-x86_64/src/e1/`; su prueba: pedir, soltar y reusar mil veces sin pisar lo vivo
- [ ] L4 -- LISTAS al correr: el asa de 24 bytes en `toolchain/lang/titan/emisor-x86_64/src/e1/forma.rs`, `push`/`pop`/`len`/`l[i]`/`for`, la copia entera y el soltar en `Op::Drop` (`toolchain/lang/titan/emisor-x86_64/src/e1/valor.rs`)
- [ ] L5 -- MAPAS al correr: entradas en orden, indice por hash, claves int/text/bool/registro, en `toolchain/lang/titan/emisor-x86_64/src/e1/`
- [ ] L6 -- las VARAS: los dos oraculos y el azar de `toolchain/lang/titan/emisor-x86_64/tests/e1.rs` con listas y mapas, y una mas: al acabar cada programa, el monton tiene TODO soltado (el emulador cuenta los bloques)
- [ ] L7 -- el NIVEL 13 en `toolchain/lang/titan/ejemplos/nivel13/`: una agenda, contar palabras, un inventario, y sus NO; en los dos bancos, en el metro (`toolchain/tools/metro/src/main.rs`) y en la TAB (`toolchain/tools/maestros/maestros.py`)
- [ ] L8 -- los papeles: el nivel 13 de `toolchain/lang/titan/GRAMATICA.md` y `docs/maestro/TITAN_MAESTRO.md`; las leyes nuevas con `--sellar` del propietario (`toolchain/tools/titan-leyes/LEYES.txt`)

## 6. Por que ahora: el primer mundo

El propietario pregunto el mismo dia por un juego de cubos al estilo de
Minecraft, hecho en TITAN++ y como primera vez en BMO-X. Un mundo asi es,
antes que nada, DATOS QUE CRECEN:

```text
   el mundo          {Trozo: [Bloque]}   cada trozo cargado, por su
                                         coordenada (un registro como clave)
   lo que se ve      [Cara]              las caras visibles de un trozo,
                                         rehechas al poner o quitar un bloque
   el inventario     {Bloque: int}
```

Sin listas y mapas no se puede ni empezar; con ellos, el primer escalon de
ese juego (un mundo de cubos en texto, que se recorre y se cambia tecleando)
se escribe el dia que este plan acabe. Lo demas de ese juego (la ventana, la
profundidad y las texturas de VERRANO, lo caliente en INTI) va en su propio
plan.

## 7. Lo que este plan NO es

- **No son los genericos del autor.** `[T]` y `{K: V}` son tipos de la casa,
  como `[T; n]`; una `fn` que vale para cualquier T es otro nivel.
- **No es un recolector de basura.** Se suelta donde el juez ya sabe que el
  valor muere; nada corre de fondo.
- **No toca INTI ni la 3060.** Una lista vive en la CPU, en el monton del
  programa.
