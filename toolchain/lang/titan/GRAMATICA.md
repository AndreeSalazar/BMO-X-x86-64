# TITAN++ -- LA GRAMATICA, nivel a nivel

> El **porque** de cada decision esta en `docs/maestro/TITAN_MAESTRO.md`. Aqui
> va **lo que se escribe**, y solo lo que el frontend YA entiende: un nivel se
> escribe aqui el dia que su banco (`ejemplos/nivelN/`) pasa entero.

Decidido por el propietario el **2026-09-30**:

```text
   los bloques      por SANGRIA, como INTI (no llaves)
   el primer hola   en la CONSOLA (F12), como INTI; la ventana, despues
```

---

## Nivel 0 -- un programa que saluda (1 palabra: `fn`)

```text
# hola.titan
mod main "saluda"

fn main()
    print("hola")
```

### Las piezas

```text
   comentario    `#` hasta el final de la linea (fuera de un texto)
   cabecera      la PRIMERA linea que no es blanca ni comentario:
                    mod NOMBRE "que hace"
                 (U3: un modulo dice lo que hace; la misma regla que el
                 lector de F1, `platform/shared/titan-lector`)
   funcion       fn NOMBRE()           y debajo, SANGRADO, su cuerpo
   llamada       NOMBRE(TEXTO, ...)    una por linea
   texto         "..." en una linea; dentro valen  \"  \\  \n
   nombre        una letra y despues letras, cifras o _
```

### La sangria (la misma regla que INTI)

```text
   un nivel .............. CUATRO espacios, exactos
   un tabulador .......... T0010, sin excepciones
   un ancho que no es
     multiplo de 4, o que
     salta dos niveles, o
     vuelve a un margen
     que nadie abrio ..... T0012
```

### Lo que ya existe dentro del nivel 0

```text
   print(TEXTO, ...)      escribe los textos en la consola, uno detras de otro,
                          y salta de linea al acabar
   una fn propia          se puede definir y llamar sin argumentos
   fn main()              OBLIGATORIA: por ahi empieza el programa (T0050)
```

### Lo que el nivel 0 dice que TODAVIA NO

Una de las 25 palabras de un nivel que aun no existe no es un error de
sintaxis cualquiera: es **T0040**, y dice en que nivel llega (el ejemplo ya es
del frontend de hoy, que va por el nivel 4):

```text
   return           T0040  `return` llega en el nivel 5 (funciones con resultado)
   type Nave        T0040  `type` llega en el nivel 6 (registros)
```

---

## Nivel 1 -- calcular (2 palabras: `fn`, `let`) -- 04-10

```text
# centauro.titan
mod main "el centauro cuenta sus estrellas"

fn main()
    let patas = 4
    let torso = 1
    let estrellas = 120 + 35 * 2
    print("patas: ", patas, ", en total ", patas + torso)
    print(estrellas / 5, " cada una, y sobran ", estrellas % 7)
```

### Las piezas nuevas

```text
   let           let NOMBRE = VALOR      un nombre para un valor
   valor         un numero entero, un "texto", un nombre con valor,
                 y + - * / % con parentesis, y un - delante
   print         ahora escribe numeros y textos: print("area: ", area)
```

La precedencia es la de la escuela: `*` `/` `%` antes que `+` `-`, y los
parentesis primero. `+` entre dos textos los pone uno detras del otro.

### Las reglas, y quien las dice

```text
   EL JUEZ (juez.rs, el borrow checker) -- que puede cada nombre en cada linea
     un nombre se lee DESPUES de su `let`             si no, T0054
     un nombre, un valor (no se tapa con otro `let`)  si no, T0055
     sin `mut` no cambia nunca                        si no, T0056
                                                      (`mut` llega en el nivel 2)
   EL CALCULO (calc.rs) -- lo que se sabe al compilar se calcula al compilar
     64 bits, y desbordar es un ERROR                 T0060
     entre cero, ni / ni %                            T0061
     una division que no da entera es un NO:          T0062
       7 / 2 no es 3; el resto es 7 % 2, y los
       decimales EXACTOS (dec) llegan con los tipos
     un texto con un numero no se suma ni convierte   T0063
```

En el nivel 1 todo valor se sabe antes de correr (todavia no se lee nada de
fuera), asi que el `.bex` ya lleva los resultados: `titan ir` los muestra.

---

## Nivel 2 -- contar (3 palabras: `fn`, `let`, `mut`) -- 04-10

```text
mod main "el centauro cuenta sus vueltas"

fn main()
    let mut vueltas = 0
    vueltas = vueltas + 1
    print("vueltas: ", vueltas)
```

```text
   let mut NOMBRE = VALOR    un valor que puede cambiar
   NOMBRE = VALOR            le da otro valor (solo si es `mut`)
```

### Lo que juzga EL JUEZ, y por que

```text
   cambiar sin `mut`                  T0056  y el COMO dice en que linea poner el mut
   un `mut` que no cambia nunca       T0057  es un NO, no un aviso: `mut` es una
                                             promesa al que lee ("este se mueve"),
                                             y una promesa que nadie cumple le
                                             quita valor a todos los `mut`
   cambiar de CLASE                   T0064  un `mut` cambia de valor, no de clase:
                                             `vidas = "ninguna"` tras `let mut vidas = 3`
```

`n = n + 1` es lo de siempre: lo de la derecha se lee ANTES de que empiece el
cambio (L5 de TITAN_MAESTRO 6.8, lo que Rust tuvo que parchear con los
"prestamos en dos fases").

---

## Nivel 3 -- decidir (10 palabras: + `if else true false and or not`) -- 04-10

```text
# semaforo.titan
mod main "un semaforo que decide"

fn main()
    let segundos = 47
    let mut color = "verde"
    if segundos > 50
        color = "rojo"
    else if segundos > 40
        color = "ambar"
    print("el semaforo esta en ", color)
    print("cruzar: ", color == "verde")
```

### Las piezas nuevas

```text
   if COND           y debajo, SANGRADO, su bloque
   else              al MISMO margen que su `if`, y debajo su bloque
   else if COND      una cadena: el primero que es true, y ninguno mas
   true  false       un si-o-no: la tercera clase de valor
   == != < <= > >=   comparar: dan un si-o-no
   and  or  not      unir y dar la vuelta, con PALABRAS (no && || !)
```

La fuerza, de menos a mas: `or`, `and`, `not`, una comparacion, `+ -`,
`* / %`, y el `-` de delante. `not vidas > 0` es `not (vidas > 0)`.

### Las reglas, y quien las dice

```text
   LA GRAMATICA (parse.rs)
     se compara de dos en dos                          T0030
       `1 < x < 9` se lee distinto en Python y en C:
       se escribe `1 < x and x < 9`
     `if x = 3` no pregunta: `=` da un valor            T0030
       y el COMO lo dice: para preguntar, `==`
   EL JUEZ (juez.rs) -- ahora RECORRE bloques
     lo que nace en un bloque vive en su bloque        T0058
       y el COMO dice como sacarlo: `let mut` antes,
       y dentro solo `x = ...`
     donde dos caminos se juntan, vale lo cierto en LOS DOS: un `mut`
     cambiado en un solo lado CUENTA como cambiado (T0057 no salta)
   EL CALCULO (calc.rs) -- dos pasadas
     CLASES, en TODOS los bloques (tambien los que no corren):
       un `if` pregunta SI o NO: `if vidas` es un NO   T0065
       `not 3`, `vidas and escudo`                      T0065
       1 == "1" (nunca son iguales: pregunta trampa)   T0063
       "a" < "b" (el orden de los textos depende del
       idioma; hoy solo == y !=)                        T0063
     VALORES, solo por el camino que corre:
       cada `if` se DECIDE al compilar (nada viene de fuera todavia)
       el lado que no corre esta MUERTO: no se calcula, no deja bytes,
       y el certificado no nombra sus puertas
       por eso `if d != 0` guarda `10 / d`, y `and` / `or` paran en
       cuanto saben: `d != 0 and 10 / d > 1` no divide si d es 0
```

**T0053 sigue en pie, y ahora dice por que.** Una `fn` no recibe nada hasta el
nivel 5: cada vuelta decide IGUAL que la primera. Si la llamada de vuelta pasa
una vez, pasa siempre; si no pasa nunca, sobra. Un `if` no la salva -- los
parametros si.

`titan ir` lo muestra entero: los bloques (`b0`, `b1`...), el `si ... -> b1,
sino -> b2   (decidido al compilar)`, el `muere %2` donde se cierra un bloque y
el `(muerto: ...)` del lado que no corre.

---

## Nivel 4 -- repetir (15 palabras: + `for in while break continue`) -- 04-10

```text
# tabla_del_siete.titan
mod main "la tabla del siete"

fn main()
    let mut suma = 0
    for i in range(1, 6)
        print("7 x ", i, " = ", 7 * i)
        suma = suma + 7 * i
    print("la tabla suma ", suma)
```

### Las piezas nuevas

```text
   while COND            y debajo su bloque: se repite mientras COND sea true
   for i in range(N)     i vale 0, 1 ... N-1, una vuelta cada uno
   for i in range(A, B)  i vale A, A+1 ... B-1
   break                 corta el bucle de dentro
   continue              salta a la vuelta siguiente
```

`i` es NUEVO en cada vuelta y lo pone el bucle: no es un `mut`, y cambiarlo
es T0056 (el COMO dice: para saltar vueltas, `continue`). `range` cuenta con
numeros, y su final se lee UNA vez, antes de la primera vuelta. Recorrer una
tabla (`for p in planetas`) llega con las tablas (nivel 6).

### Las reglas, y quien las dice

```text
   LA GRAMATICA
     `break` / `continue` fuera de un bucle            T0067
   EL JUEZ -- ahora con el primer salto HACIA ARRIBA
     el final de un bucle vuelve a su pregunta: el juez recorre los bloques
     hasta que ninguna entrada se mueve (el punto fijo que el nivel 3
     prometio), y despues juzga. Lo que nace en la vuelta muere con ella, y
     `break` / `continue` cierran los bloques que dejan atras
   EL CALCULO -- el programa ENTERO, corrido al compilar
     nada viene de fuera todavia, asi que no se calcula linea a linea (una
     linea dentro de un `for` vale otra cosa en cada vuelta): se CORRE el
     programa, cada llamada, cada `if`, cada vuelta, y el .bex es lo que
     escribe. Un millon de pasos y sigue                T0066
       un `while` sin salida se dice al compilar, en su linea
```

`titan ir` lo muestra entero: los bloques con su salto hacia arriba
(`salta b1`) y, abajo, *lo que escribe, CORRIDO al compilar*. El dia que algo
venga de fuera (el teclado), esa parte ira a la maquina como codigo de verdad
(E1, TITAN_MAESTRO 7.3).

---

## Los codigos

| codigo | que |
|---|---|
| T0001 | falta la cabecera `mod nombre "que hace"` |
| T0010 | un tabulador en la sangria |
| T0012 | una sangria que no es de 4 en 4, o salta, o vuelve a donde no hubo |
| T0020 | un texto sin cerrar |
| T0021 | un caracter que no es del lenguaje |
| T0022 | una `\` en un texto que no es `\"`, `\\` ni `\n` |
| T0030 | se esperaba otra cosa en ese sitio |
| T0031 | una funcion sin cuerpo |
| T0040 | una palabra de un nivel que aun no existe |
| T0050 | no hay `fn main()` |
| T0051 | se llama a algo que no existe |
| T0052 | una funcion definida dos veces |
| T0053 | las llamadas vuelven a una funcion y no terminan nunca (sin parametros, ni un `if` las para) |
| T0054 | un nombre se lee antes de que un `let` le de valor (el juez) |
| T0055 | un nombre que ya tiene valor, o que es de una funcion (el juez / los nombres) |
| T0056 | se cambia un valor sin `mut` (el juez) |
| T0057 | un `mut` que no cambia nunca (el juez) |
| T0058 | un nombre que nacio en un bloque que ya se cerro (el juez) |
| T0060 | un numero que no cabe en 64 bits: desbordar es un error (el calculo) |
| T0061 | una division o un resto entre cero (el calculo) |
| T0062 | una division que no da un numero entero (el calculo) |
| T0063 | un texto con un numero: no se suman ni se convierten solos (el calculo) |
| T0064 | un `mut` que cambiaria de clase: numero, texto o si-o-no (el calculo) |
| T0065 | se pedia un si-o-no y llego otra cosa: `if vidas`, `not 3` (el calculo) |
| T0066 | el programa sigue corriendo despues de un millon de pasos: un bucle sin salida (el calculo) |
| T0067 | `break` o `continue` fuera de un bucle (la gramatica) |

## El banco: lo que dice cada ejemplo de si mismo

```text
   # espera: BIEN        compila...
   # sale: hola          ...y al CORRER escribe exactamente esto, linea a linea
   # espera: T0053       NO compila, con este codigo, y no escribe ningun .bex
```

Las lineas `# sale:` las comprueba el banco del emisor
(`emisor-x86_64/tests/banco.rs`): construye el `.bex`, lo carga como el
cargador del kernel y lo CORRE en el emulador. Un nivel esta hecho cuando sus
programas HACEN lo que dicen, no cuando compilan.

## Del texto al `.bex` (T3, 2026-10-04)

```text
   titan check hola.titan              bien, o el mensaje de 4 partes
   titan ir    hola.titan              la IR propia: lo que recibe el emisor
   titan build hola.titan -o hola.bex  el .bex, con su manifiesto, por el gate
```

En la maquina: `run titan/hola.bex` en la consola (F12). `build.ps1` deja
`titan/hola.bex` y `titan/dos.bex` en el disco.
