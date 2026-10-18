# TITAN++ -- LA GRAMATICA, nivel a nivel

> El **porque** de cada decision esta en `docs/maestro/TITAN_MAESTRO.md`. Aqui
> va **lo que se escribe**, y solo lo que el frontend YA entiende: un nivel se
> escribe aqui el dia que su banco (`ejemplos/nivelN/`) pasa entero.

Decidido por el propietario el **2026-09-30**:

```text
   los bloques      por SANGRIA, como INTI (no llaves)
   el primer hola   en la CONSOLA (F12), como INTI; la ventana, despues
```

**Las reglas de abajo no cambian de paso**: las que el lenguaje promete estan
en `toolchain/tools/titan-leyes/LEYES.txt`, cada una con su porque y el test o
el programa que la hace cumplir, y el build para si una pierde su prueba o su
texto cambia sin sellarse con un motivo.

**Cada ejemplo `BIEN` es tambien un nodo de la TAB de F1** (05-10): el
programa que se escribe aqui para probar un nivel es el que el propietario
pone en su paquete con TAB y Enter. Su segunda linea de comentario (la de
justo antes de `mod main`) es su PORQUE en la TAB: que diga por que existe.

## La escalera de un vistazo

| nivel | palabras nuevas | lo que deja escribir | sus codigos |
|---|---|---|---|
| 0 | `fn` | un programa que saluda: `print`, fn sin argumentos | T0001-T0053 |
| 1 | `let` | valores con nombre y el calculo exacto | T0054, T0055, T0060-T0063 |
| 2 | `mut` | valores que cambian | T0056, T0057, T0064 |
| 3 | `if else true false and or not` | decidir | T0058, T0065 |
| 4 | `for in while break continue` | repetir | T0066, T0067 |
| 5 | `return` | funciones con parametros y resultado | T0068-T0070 |
| 6 | `type` | `dec` exacto, tablas `[T; n]`, registros | T0071-T0073 |
| 7 | `take` | prestar (`mut`) y entregar (`take`); `dec(p, s)` y `round` de COBOL | T0074-T0077 |
| 8 | `enum match` | casos con datos, y un `match` que los cubre todos | T0078, T0079 |
| 9 | `mod use pub` | paquetes de varios ficheros, con su `Titan.toml` (U2) | T0080-T0084, T0088, T0089 |
| 10 | `trait` | lo que un valor sabe hacer, y fn para cualquiera que lo sepa | T0085-T0087 |
| 11 | `gpu` | la 3060: `gpu fn`, una celda por hilo; `f32` vive alli | T0090, T0091 (el plan: `docs/plan/PLAN_EL_CENTAURO.md`) |
| 12 | (ninguna) | lo que viene de fuera: `lee()` de la biblioteca, y el programa CORRE en la maquina | los de siempre, y T0060-T0062 tambien al correr (el plan: `docs/plan/PLAN_LA_ENTRADA.md`) |
| 13 | (ninguna) | lo que crece: listas `[T]` y mapas `{K: V}`, con `push`, `pop`, `put`, `get`... | T0063, T0069, T0071, T0072, T0077, T0079 de nuevo (el plan: `docs/plan/PLAN_LISTAS_Y_MAPAS.md`) |
| 14 | (ninguna) | el BYTE: `byte` de 0 a 255, `byte(x)`, y `[byte]` con UN byte por celda | T0060 y T0071 de nuevo (el plan: TA1 de `docs/plan/PLAN_LA_TINTA.md`) |
| 15 | (ninguna) | la LAMINA: RAM del kernel con UN propietario, `lamina(n)`, que se presta, se entrega y se suelta sola | T0077, y T0060 y T0072 al correr (el plan: A4b de `docs/plan/EL_FOCO.md`) |

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
del frontend de hoy, que va por el nivel 10):

```text
   use nave         T0040  `use` llega en el nivel 9 (varios ficheros)
   fn f(x: f32)     T0040  el tipo `f32` llega en el nivel 11 (la 3060)
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

### La division ENTERA: `div` y `rem` (18-10, sin palabras nuevas)

`/` sigue siendo exacta: 7 / 2 es un NO, no un 3 callado. Cuando lo que se
quiere ES el cociente entero -- el bloque de una coordenada, la vuelta de
un angulo, la columna de un pixel --, se pide por su nombre:

```text
   div(a, b)     el cociente de Euclides        div(-7, 2) es -4
   rem(a, b)     el resto, NUNCA negativo       rem(-7, 2) es 1
                 a == div(a, b) * b + rem(a, b), siempre
```

Antes cada app escribia `fn div(a: int, d: int) -> int` con
`(a - a % d) / d` (once copias) y `(x % m + m) % m` para la vuelta. Ese
`div` corta hacia el cero: con negativos NO es el de la biblioteca
(-1 entre 10000 da 0 a mano y -1 con `div`). Una fn propia llamada `div` o
`rem` sigue mandando: la que escribe el programa gana, como con `get`.
Un bucle de 10000 vueltas con la fn a mano gasta 860.169 instrucciones en
E1; con `div` y `rem`, 290.169 (el emulador del anfitrion, no el Ryzen).

```text
   div / rem de un dec, o con un dec            T0063
   entre cero                                   T0061
   div(MIN, -1) no cabe                         T0060 (rem(MIN, -1) es 0)
   dentro de una gpu fn                         T0090 (son de la CPU)
```

Ejemplos: [division_entera](ejemplos/nivel1/division_entera.titan),
[euclides_entero](ejemplos/nivel5/euclides_entero.titan).

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

## Nivel 5 -- funciones con resultado (16 palabras: + `return`) -- 04-10

```text
# euclides.titan
mod main "Euclides y los primos"

fn mcd(a: int, b: int) -> int
    if b == 0
        return a
    return mcd(b, a % b)

fn main()
    print("mcd(48, 18) = ", mcd(48, 18))
```

**Con esto ya se escribe cualquier algoritmo** (TITAN_MAESTRO 14.14): valores
que entran, uno que sale, y una funcion que se llama a si misma.

### Las piezas nuevas

```text
   fn f(a: int, b: text)    parametros, cada uno con su tipo
   -> int                   lo que devuelve: int, text o bool
   return VALOR             lo da y se acaba la funcion
   return                   se acaba, en una fn sin `->`
   f(3)                     una llamada ES un valor: let x = f(3), f(3) + 1
```

`int`, `text` y `bool` son TIPOS, no palabras (no gastan techo). `f32`, `dec`
y las tablas llegan con los tipos (nivel 6); prestar un parametro para
cambiarlo (`mut`) o entregarlo (`take`), en el nivel 7. Hasta entonces un
parametro llega y SOLO se lee.

### Las reglas, y quien las dice

```text
   LOS NOMBRES (check.rs)
     una llamada con mas o menos valores de los que pide   T0068
       y el POR QUE copia la linea de la fn que los pide
     usar como valor algo que no devuelve nada (`print`,   T0069
     una fn sin `->`), o un `return` que no cuadra con
     lo que promete la primera linea
     T0053 se queda SOLO para ciclos de fn sin parametros: una que recibe
     un valor puede decidir distinto en cada llamada, y si para o no lo
     dice CORRERLA
   EL JUEZ
     los parametros nacen vivos y no cambian              T0056
     un camino que llega al final de una fn que prometio  T0070
     un valor, sin `return` (un `if` que devuelve en sus
     dos lados esta bien: lo de detras no tiene camino)
   EL CALCULO
     cada valor que se pasa, de la clase que pide su      T0071
     parametro; lo que se devuelve, de la que promete
     la recursion se CORRE al compilar, como los bucles: mas de 10 000
     llamadas anidadas es T0066 (su caso de parada no llega)
```

---

## Nivel 6 -- los tipos (17 palabras: + `type`) -- 04-10

```text
# factura.titan
mod main "una factura que no pierde centimos"

type Linea
    cosa: text
    precio: dec
    cantidad: int

fn importe(l: Linea) -> dec
    return l.precio * l.cantidad

fn main()
    let compra = [Linea { cosa: "cafe", precio: 1.75, cantidad: 2 }, ...]
    let mut total = 0.00
    for l in compra
        total = total + importe(l)
    print("total: ", total)
```

### Las piezas nuevas

```text
   dec                 el DECIMAL EXACTO: 12.50, 0.1 + 0.2 = 0.3. Sin float
   [1, 2, 3]  [0; 10]  una TABLA: una clase, un largo fijo; su tipo [int; 3]
   t[i]   t[i] = v     una celda (desde la 0), y cambiarla (con `mut`)
   for x in t          sus celdas, una por vuelta
   len(t)              cuantas celdas
   type Nave           un REGISTRO: sus campos debajo, uno por linea
       x: dec
   Nave { x: 1.0 }     uno nuevo, con TODOS sus campos (no hay null)
   n.x   n.x = v       un campo, y cambiarlo (con `mut`)
```

### `dec` y no float, y por que (el propietario, 04-10)

*"Evita la float, siempre decimal."* Un `dec` es un entero y cuantas de sus
cifras son decimales: `12.50` es 1250 con 2. **Sumar** alinea los decimales,
**multiplicar** los suma (`12.50 * 3 = 37.50`, la regla de COBOL) y **dividir**
da el decimal EXACTO (`10.00 / 4 = 2.50`) -- o un NO si no acaba (`1.0 / 3`,
T0062): TITAN++ no corta un numero a escondidas. Un `int` entra en un `dec` sin
perder nada (13 es 13.00); al reves perderia, y no se hace solo. El `%` es de
enteros.

[!] El x86-64 SI tiene floats en hardware (SSE2 es obligatorio): lo que no
tiene es BASE 10, y por eso un float se equivoca con `0.1`. `dec` es el numero
de Grace Hopper a la velocidad de un entero. `f32` es de la 3060 (`gpu fn`,
nivel 11), y si se pide antes lo dice (T0040).

### Las reglas, y quien las dice

```text
   LOS NOMBRES
     un tipo que no existe                              T0051
     un registro sin un campo, con uno de mas o dos     T0073
     veces el mismo; un `type` que se contiene a si
     mismo (un valor sin fin: TITAN++ guarda valores,
     no punteros)
   EL JUEZ
     cambiar una celda o un campo es cambiar el valor:  T0056
     pide `mut`, y cuenta como el cambio que un `mut`
     promete
   EL CALCULO
     una tabla de UNA clase ([1, 2.5] es de `dec`)       T0071
     un campo que el registro no tiene                  T0073
     una celda fuera de su tabla -- se CORRE al         T0072
     compilar y se ve: en C eso lee memoria de otro
```

Las tablas y los registros son VALORES: `let b = a` copia. Prestarlos sin
copiar (`mut`, `take`) es el nivel 7.

---

## Nivel 7 -- prestar y entregar (18 palabras: + `take`), y la PRECISION de COBOL -- 04-10

```text
# banco.titan
type Cuenta
    titular: text
    saldo: dec(9, 2)

fn interes(saldo: dec(9, 2), tasa: dec(5, 4)) -> dec(9, 2)
    return round(saldo * tasa, 2)      # el redondeo se ESCRIBE

fn abona(mut c: Cuenta, tasa: dec(5, 4))
    c.saldo = c.saldo + interes(c.saldo, tasa)

fn main()
    let mut ana = Cuenta { titular: "ana", saldo: 1250 }
    abona(mut ana, 0.0225)              # prestada: se ve el cambio al volver
```

### Prestar y entregar: el borrow checker entero

```text
   fn f(n: T)           una COPIA: el que llama conserva la suya, intacta
   fn f(mut n: T)       PRESTADO para cambiarlo: f(mut x), con `let mut x`;
                        al volver, x tiene lo que f dejo en n -- sin copiar
   fn f(take n: T)      ENTREGADO: f(take x); despues x ya no es tuyo
```

**O1, 16-10:** una funcion propia tambien recibe `f(mut block.durability)`
o campos anidados, con la misma firma `fn f(mut d: Durability)`. El propietario
debe ser mutable y seguir vivo. Por ahora se reserva el propietario completo:
`f(mut block.left, block.right)` y dos campos hermanos prestados en la misma
llamada son T0076, aunque sus nombres sean distintos. Se puede leer antes y
pasar una copia. No se habilitan `take block.field` ni `mut blocks[i]` para
funciones propias. Las primitivas de colecciones mantienen sus reglas previas.
[Ejemplo de bloques](ejemplos/nivel10/bloques.titan),
[pruebas nativas y rechazos](emisor-x86_64/tests/composition.rs).

**Se dice en los DOS lados** (`f(mut x)`, no solo `fn f(mut n)`): quien lee la
llamada sabe que le puede pasar a `x` sin abrir la funcion -- en C, `f(&x)` solo
dice "quiza". Y un `mut` en un parametro que nunca cambia es T0057, como el de
un `let`: pide una copia. **Nunca es nunca** (11-10, MC1 de `PLAN_MUNDO`): si
cambia en UN camino, cuenta, aunque otro vuelva pronto sin tocarlo
(`if t[i] != 0` / `return false`) -- lo mismo que donde dos caminos se juntan
(`ejemplos/nivel7/cambia_en_un_camino.titan`). Y en la maquina, prestado es un
puntero: una tabla de 2000 prestada hacia dentro no gasta su tamanio en cada
marco (`emisor-x86_64/tests/pila.rs`).

**Y lo que solo se LEE tampoco se copia** (16-10, E1): `fn f(n: T)` es una
copia para quien lee el programa, pero si `f` nunca cambia `n` ni se lo
queda (no lo presta, no lo entrega, no lo guarda entero en un `let`, un `=`,
un `return`, una tabla, un registro o un `push`), la copia y el original no
se distinguen, y la maquina pasa un PUNTERO. Es para listas, tablas y mapas
sin `dec` dentro. Medido: pasar una `[byte]` de 20 KB mil veces costaba 200
millones de pasos y cuesta lo que leerla (`e1/mod.rs`, `solo_se_lee`).

### La precision de COBOL (el propietario: "precision fuerte para no generar bug")

```text
   dec(7, 2)            7 cifras, 2 decimales: el PIC 9(5)V99 de COBOL
   let p: dec(7, 2) = 12.5    el tipo DECLARADO; se guarda 12.50
   round(x, 2)          el redondeo ESCRITO (el ROUNDED de COBOL: la mitad,
                        lejos del cero): 1.255 -> 1.26, 10.00 / 3 -> 3.33
```

Todo valor que va a un `dec(7, 2)` -- un `let` con tipo, un parametro, un
resultado, un campo -- tiene que CABER: mas cifras que las declaradas es T0074
(el SIZE ERROR de COBOL), y mas decimales tambien (cortarlos seria perder dinero
en silencio). COBOL lo corta callado si no se escribe ON SIZE ERROR; **TITAN++
no compila**. Si hay que redondear, `round` lo dice a la vista.

### Las reglas, y quien las dice

```text
   LOS NOMBRES
     `mut` / `take` en la llamada que no cuadran con    T0077
     el parametro, o fuera de una llamada
   EL JUEZ
     leer lo que se entrego con `take`                  T0075
       (un `let mut` que se entrego puede tomar otro valor: es tuyo otra vez)
     prestar dos veces el mismo valor en una llamada,   T0076
     o prestarlo y a la vez leerlo: LA REGLA DE ORO DE
     FORTRAN (TITAN_MAESTRO 2b.2), demostrada
     prestar con `mut` lo que no es `let mut`           T0056
   EL CALCULO
     un valor que no cabe en su dec(p, s)               T0074
```

---

## Nivel 8 -- casos con datos (20 palabras: + `enum match`) -- 04-10

```text
# formas.titan
enum Forma
    Circulo(dec)            # un caso que LLEVA un valor
    Rect(dec, dec)          # ... o dos
    Nada                    # ... o ninguno

fn area(f: Forma) -> dec
    match f
        Circulo(r)          # r es el dec que lleva el Circulo
            return 3.14 * r * r
        Rect(ancho, alto)
            return ancho * alto
        Nada
            return 0

fn main()
    for f in [Circulo(2.0), Rect(3.5, 2), Nada]
        print(f, " mide ", area(f))      # Circulo(2.0) mide 12.56
```

### Lo que se escribe

```text
   enum NOMBRE          y debajo, sangrados, sus casos: uno por linea
     Caso               un caso sin datos
     Caso(T, T...)      un caso que lleva valores, de esos tipos
   Caso(v, ...)         CONSTRUYE un valor del enum (sin `Forma::` delante:
   Caso                 el nombre de un caso es unico en todo el fichero)
   match VALOR          y debajo, una rama por caso, cada una con su bloque
     Caso(a, b)         la rama nombra los valores que el caso lleva; esos
                        nombres viven en su bloque y mueren al cerrarse
   a == b               dos valores de un enum son iguales si son el mismo
                        caso con los mismos valores
```

**Un `match` cubre TODOS los casos, y no hay `_`.** El dia que alguien agrega
`Triangulo` a `Forma`, cada `match` que no dice que hacer con el deja de
compilar y dice DONDE (T0078). El `switch` de C deja caer el caso nuevo en
silencio; un comodin `_` haria lo mismo, por eso TITAN++ no lo tiene (T0079).

**Sin null y sin excepciones**: lo que puede salir mal es un CASO
(`Hecho(dec)` / `Falta(dec)`), y el `match` obliga a mirarlo antes de usar el
valor. Un caso lleva valores, no punteros: un enum que se contiene a si mismo
no termina nunca (T0079).

### Las reglas, y quien las dice

```text
   LOS NOMBRES
     un caso que no existe, de otro enum, dos veces en    T0079
     el mismo `match`, con mas o menos nombres que
     valores lleva, o un `_`
     un `match` al que le falta un caso                    T0078
     un caso con datos escrito sin ellos (`Circulo`)       T0068
     un caso solo en su linea: construye y nadie guarda    T0069
     un valor llamado como un caso (`let Nada = 1`)        T0055
   EL JUEZ
     un nombre de una rama leido fuera de ella             T0058
   EL CALCULO
     un `match` sobre un valor que no es de su enum, o     T0071
     un caso con un valor de otra clase
```

---

## Nivel 9 -- varios ficheros (23 palabras: + `mod use pub`) -- 04-10

```text
flota/
   src/main.titan       mod main "una flota repartida en modulos"
                        mod nave, puerto          <- sus HIJOS
   src/nave.titan       mod nave "una nave: su combustible y sus saltos"
   src/puerto.titan     mod puerto "dice donde esta cada nave"
                        use nave                  <- con quien HABLA
```

```text
# src/nave.titan
mod nave "una nave: su combustible y sus saltos"

pub type Nave                     # pub: se ve desde fuera
    nombre: text
    combustible: dec(5, 2)
    saltos: int

pub fn salta(mut n: Nave)
    n.combustible = n.combustible - gasto()
    n.saltos = n.saltos + 1

fn gasto() -> dec(5, 2)           # sin pub: es de nave, y solo de nave
    return 6.00

# src/main.titan
fn main()
    let mut c = nave.nueva("centauro", 50.00)
    nave.salta(mut c)             # modulo.cosa
```

### La cabecera, entera

```text
   mod NOMBRE "que hace"      la PRIMERA linea: quien es (sin ella, T0001)
   mod a, b                   sus hijos: viven en a.titan y b.titan, junto a
                              main.titan si el padre es main, y si no en la
                              carpeta del padre (physics.titan -> physics/a.titan)
   mod a in "x/a.titan"       un hijo donde el PADRE diga (ruta desde el paquete)
   use a, b                   con quien habla: un modulo del paquete, o un nodo
                              de BMO-X (`gpu`, `director`)
   use casa.medida            un modulo de LA CASA: la libreria de TITAN++ que
                              viaja dentro del compilador (18-10)
```

### La casa: lo que todas las apps comparten (18-10)

ECCE, RESOLUCION y TINTA llevaban el mismo `medida.titan` copiado, y cuatro
apps el mismo `es_cifra`. Ahora se piden: `use casa.medida` en la cabecera y
`medida.ancho()` en el cuerpo, como cualquier modulo. Los modulos de la casa
viven en [`casa/`](casa/README.md) y entran en el compilador; un paquete no
lleva copia ni ruta.

```text
   use casa.nada                no hay ese modulo en la casa (dice cuales hay) T0051
   mod cifras + use casa.cifras dos modulos con un nombre                      T0052
   use cifras (sin casa.)       el de la casa se pide con su nombre entero     T0051
   un modulo de la casa que usa un modulo de la app: la casa esta DEBAJO      T0051
   use casa.medida sin screen   lo que la casa usa de BMO-X lo pide la app     T0088 (U2)
```

Va arriba, antes de la primera `fn`, en cualquier orden: quien abre el fichero
sabe quien es y de quien depende sin bajar. Es la MISMA gramatica que lee F1
(`titan-lector`), asi que el grafo que dibuja F1 y el programa que compila
`titan` son el mismo arbol.

### Lo que se escribe en el cuerpo

```text
   pub fn / pub type / pub enum    lo que se ve desde fuera; lo demas es del modulo
   nave.salta(mut c)               llamar a una fn de otro modulo
   nave.Nave { ... }  n: nave.Nave un tipo de otro modulo
   forma.Circulo(2.0), forma.Nada  un caso de otro modulo (y en un `match`)
```

Se llega a un HIJO (`mod`) o a lo que se `use`, y nada mas. El paquete se
compila con su fichero raiz: `titan check flota/src/main.titan`. Un fichero
sin hijos es un paquete de un modulo: todo lo de los niveles 0-8 sigue igual.

### Las reglas, y quien las dice

```text
   EL PAQUETE (U3: "la cabecera se compara con lo que de verdad se llama")
     llamar a un modulo que no es hijo ni esta en `use`   T0080
     un `use` del que no se usa nada                      T0081
     llamar a algo que no es `pub`                        T0082
     `mod a` y no hay fichero, o el fichero dice otro      T0083
     nombre en su primera linea
     un ciclo: a usa b y b usa a, o un hijo que usa a su   T0084
     padre (las capas solo BAJAN, la ley L8)
     un `use` de un modulo que no existe                  T0051
     dos modulos con el mismo nombre                      T0052
     un valor llamado como un modulo                      T0055
   EL MANIFIESTO (U2, desde el 04-10)
     `use gpu` / `use director` sin que el Titan.toml     T0088
     pida `gpu` / `screen`
     un Titan.toml que no se lee                          T0089
```

**El `Titan.toml`** va junto a `src/` (como lo lee F1) y se lee con el mismo
lector que F1. Lo que pide viaja al `.bex`, a su `[permissions]`: es lo que
`titan juez` y el kernel comparan con lo que el programa usa. Un fichero suelto
no tiene manifiesto, asi que no pide nada.

Cada NO dice su FICHERO y su linea en el: las lineas del paquete se cuentan
seguidas, como el mapa de fuentes de rustc, y el mensaje se devuelve a su sitio.

[!] Dibujar un cable en F1 escribe un `use`, y hasta que el cuerpo llame a
algo de ese modulo `titan check` dice T0081 (y el nodo saldra rojo el dia que
el compilador viva dentro de F1, T6). Es a proposito: el cable dice una
dependencia, y una dependencia que no existe es la mentira que U3 prohibe.

---

## Nivel 10 -- comportamientos (24 palabras: + `trait`) -- 04-10

Revision de capacidades y mejoras: [OBJETOS.md](OBJETOS.md).
Ejemplo de componentes y bloques: [bloques.titan](ejemplos/nivel10/bloques.titan).

```text
# formas.titan
trait Forma                        # lo que una forma SABE hacer
    fn area(f: Forma) -> dec       #   (fn sin cuerpo; su primer valor, del trait)
    fn nombre(f: Forma) -> text

type Circulo
    r: dec

trait Forma for Circulo            # COMO lo hace un Circulo
    fn area(c: Circulo) -> dec
        return 3.14 * c.r * c.r
    fn nombre(c: Circulo) -> text
        return "Circulo"

fn describe(f: Forma)              # CUALQUIER valor que cumpla Forma
    print("un ", nombre(f), ": ", area(f))
```

### Lo que se escribe

```text
   trait NOMBRE                 y debajo, sus fn: lo que promete (con cuerpo,
                                el que tienen por defecto, O3).
     fn f(x: NOMBRE, ...) -> T  El primer valor de cada una es del trait
   trait NOMBRE for TIPO        y debajo, las MISMAS fn con su cuerpo: como
                                las cumple TIPO (un type, un enum, int, dec,
                                text o bool). Todas, y ninguna mas
   fn g(x: NOMBRE)              un parametro de un trait: recibe cualquier
                                valor que lo cumpla
   f(x)                         una llamada como cualquier otra: el TIPO del
                                primer valor elige que fn corre
```

Sin herencia (composicion y trait, TITAN_MAESTRO 14.13) y sin palabras
nuevas: no hay `impl` ni `self`, `for` ya era palabra, y el valor es un
parametro como los demas. Con `mut` y `take` igual que en cualquier fn:
`fn crece(mut x: Crece)` presta el valor y vuelve cambiado.

**Un generico se juzga UNA vez, contra su trait** (como Rust, no como las
plantillas de C++): dentro de `describe`, `f` solo sabe lo que Forma promete.
Quien llama `describe(roca)` con una Roca que no lo cumple lo oye EN LA
LLAMADA, en una frase (T0085), nunca desde dentro de `describe`.

**Un trait es el tipo de un PARAMETRO** (y del primer valor de sus fn), nunca
de un `let`, un campo, una tabla o un resultado: dice QUE sabe hacer un valor,
no cual es (T0087). Una LISTA o un MAPA de un trait si: lo trae el nivel 13
(O4), para guardar juntos valores de tipos distintos.

Entre modulos, como todo lo demas: `pub trait Pieza` en `pieza.titan`, y en
otro `trait pieza.Pieza for Rueda` y `pieza.vueltas(p, 2)`.

### Cuerpos por defecto (O3, 18-10, sin palabras nuevas)

Una fn de un `trait` puede llevar cuerpo: es lo que corre cada tipo que NO
la escribe en su `trait ... for`. El comportamiento comun se escribe una vez.

```text
trait Breakable
    fn remaining(b: Breakable) -> int       # cada tipo la escribe
    fn broken(b: Breakable) -> bool         # por defecto, para todos
        return remaining(b) == 0

trait Breakable for Stone                   # usa `broken` tal cual
    fn remaining(b: Stone) -> int
        return b.durability.remaining

trait Breakable for Wood                    # escribe la SUYA
    fn remaining(b: Wood) -> int
        return b.durability.remaining
    fn broken(b: Wood) -> bool
        return b.durability.remaining <= 2
```

No es herencia: no hay jerarquia ni clase madre. El cuerpo por defecto se
juzga UNA vez, como un generico del trait: solo usa lo que el trait promete.
Si mira un campo (`b.durability`) es T0073 en esa linea, aunque todos los
tipos de hoy lo tuvieran. Quien escribe la suya conserva la firma del trait
(T0086). Por dentro, cada tipo recibe su copia con su tipo (`broken<Stone>`),
la misma forma que las fn que escribe; E1 la corre igual que una escrita.
[Ejemplo](ejemplos/nivel10/por_defecto.titan) y
[su NO](ejemplos/nivel10/defecto_mira_un_campo.titan).

### Las reglas, y quien las dice

```text
   EL COMPORTAMIENTO
     un `trait ... for` al que le falta una fn, le sobra    T0086
     una, o una tiene otra firma; una fn de trait que
     no recibe primero un valor del trait
     el mismo `trait ... for` dos veces                     T0052
     un trait donde no va un parametro entero              T0087
     una fn de trait con el nombre de otra cosa             T0055
   EL CALCULO
     un valor que no cumple el trait que pide el            T0085
     parametro: se dice en la llamada
     un campo de un valor del que solo se sabe su trait     T0073
```

[!] Hoy el calculo, que conoce todos los valores, elige la fn de cada llamada.
El dia que algo venga de fuera (E1), cada llamada se decidira por su tipo al
compilar -- sin coste, como promete C++. El lenguaje no cambia por eso.

---

## Nivel 11 -- la 3060 (25 palabras: + `gpu`) -- 04-10, G1 de PLAN_EL_CENTAURO

```text
# mezcla/src/main.titan            (y su Titan.toml pide gpu = "compute")
gpu fn mezcla(a: f32, b: f32) -> f32     # se escribe para UNA celda
    return (a + b) / 2.0

fn main()
    let xs: [f32; 4] = [1.0, 2.0, 3.0, 4.0]   # el tipo DECLARADO dice el redondeo
    let ys: [f32; 4] = [3.0, 2.0, 1.0, 0.5]
    let c = mezcla(xs, ys)                    # tablas de 4: cuatro hilos
    for x in c
        print(round(x, 2))                    # y vuelve a dec, a la vista
```

### Lo que se escribe

```text
   gpu fn f(a: f32, ...) -> f32    una fn de la 3060: valores f32 o bool, cada
                                   uno una COPIA, y un resultado f32 o bool
   f(x)                            con valores: un hilo
   f(xs, ys)                       con TABLAS del mismo largo: un hilo por
                                   celda, y una tabla de resultados
   f(xs, ys)  con [f32]            con LISTAS (MC4, 18-10): del largo que
                                   tengan al correr, en el monton y no en la
                                   pila; una lista de resultados. Largos
                                   distintos: T0072, al correr
   f(xs, k)                        un valor SUELTO entre tablas o listas
                                   llega igual a cada celda (la camara de un
                                   fotograma), sin repetirlo en una tabla
   let xs: [f32; n] = [...]        un numero entra a f32 SOLO por un tipo
                                   declarado (D4): ahi se redondea, una vez
   round(x, 2)                     la puerta de vuelta: el f32 se escribe
                                   entero en decimal y se redondea como todo
                                   round de TITAN++ (la mitad, lejos del cero)
```

**Dentro de una `gpu fn`** todo numero es f32 (`2.0`, `1`), y hay `let`,
`if` / `else` y `return` con `+ - * /` y comparaciones. No hay `print` (la 3060
no tiene consola), tablas, textos, registros (**08-10: la que DIBUJA, si --
LB6, abajo**), llamadas ni bucles: el bucle de
una gpu fn ES la tabla. Las llamadas y los bucles dentro de un hilo llegan con
el escritor de la 3060 (IL1 de `PLAN_ILLAPA.md`) -- **08-10: llegaron con LB5,
abajo** --. **La division**, hoy, solo
entre una potencia de dos (`/ 2.0`, `/ 0.25`): esa es una multiplicacion
EXACTA; la general de la 3060 no da los bits exactos, y `titan check` y
`titan build` la rechazan en su linea y su columna -- T0090, un NO del
programa, no un fallo del compilador -- (LI2g de `PLAN_EL_LIBRETO.md`,
decision pendiente; LB1 de `docs/plan/PLAN_LAS_LIBRERIAS.md`, 08-10).
**09-10 (DL10 del propietario: *"Exactos, tambien en 3060"*): la GENERAL
tambien.** La 3060 la cuenta EXACTA -- los bits de IEEE, los mismos que la
casa y el calculo, tambien con subnormales, ceros, infinitos y NaN -- con la
cuenta de `ptxas` rehecha para el cuerpo de una app (`cociente.rs` de su
emisor: aguanta el error de su MUFU), y la CPU con `divss`. Entre una
potencia de dos sigue siendo una multiplicacion. Cuesta instrucciones, no
bits: en la obra de una celda la general pesa 24 (abajo). El ejemplo:
`nivel11/cociente`.

**En la CPU** (D2, la regla de la casa): un f32 se GUARDA o se PASA a otra gpu
fn, y nada mas -- ni se suma, ni se compara, ni se imprime. Para usarlo, se
vuelve dec con `round`. Un f32 y un dec no se mezclan nunca solos.

**El permiso**: una `gpu fn` necesita que el Titan.toml pida `gpu` (U2), y cada
llamada a una gpu fn queda en el certificado del `.bex` con su linea: es lo que
`titan juez --concede gpu` y el kernel comparan.

### Las reglas, y quien las dice

```text
   LA GPU (gpu.rs)
     una gpu fn con print, tablas, textos, valores mut /  T0090
     take, sin resultado, o una llamada a la CPU
     un `while`, un `range` sin sus extremos escritos (o  T0090
     mas alla de 2^24), mas de 8 bucles uno dentro de
     otro, o una gpu fn que se llama a si misma (LB5)
     un hilo con mas de 65536 de obra por celda (LB5;     T0066
     la division general pesa 24, DL10)
     una que DIBUJA (LB6): la de pixel lee su `posicion`, T0090
     su registro prestado, una que se llama, o un
     registro en una gpu fn de celdas
     una gpu fn sin `gpu` en el Titan.toml                T0088
   LA TARJETA DE UNA LIBRERIA (por PROMETEO, LB3), en su sitio
     lo que su tarjeta todavia no hace exacto (la         T0090
     division general en la 3060 lo fue hasta el 09-10:
     DL10 la hizo exacta; hoy ninguna de `titan` tiene
     un limite)
   EL CALCULO
     dos registros comparados con `==` o `!=` en una gpu  T0090
     fn (LB6: campo a campo, por IEEE)
     un f32 contado, comparado o impreso en la CPU        T0091
     un f32 y un dec juntos                               T0063
     tablas de distinto largo, o un dec donde va un f32   T0071
     sin tipo declarado
     `round` de un infinito o un NaN de la 3060           T0062
   LA ESCALERA
     `f64`: no tiene sitio (la 3060 cuenta en f32 y la    T0040
     CPU en dec)
```

**Lo que viaja a la 3060 es su SASS, y SIN SPIR-V** (07-10, decision del
propietario: *"gpu - sm86 - luego el juez verifica estrictamente y el
3060"*). Cada gpu fn se escribe como el `Programa` de la casa -- el MISMO que
sale de los sombreadores de PROTON-X --, un hilo por celda y sin saltos (cada
`if` es un `Elige`: una gpu fn es pura, asi que da lo mismo bit a bit); el
emisor de la 3060 la hace SASS de SM86, y la juzga ESTRICTO el juez del SASS,
la libreria de lo que sabe la 3060 (esperas, barreras, registros). Si no
pasa, el fallo es del escritor o del juez, y no hay `.bex`. `titan sm86
mezcla/src/main.titan -o carpeta` deja cada gpu fn como `.sass`.

**El juez no hace adivinar**: un NO se dice en el `.titan` (`src/main.titan,
linea 3, columna 14`): el Programa lleva el sitio de cada operacion. Y en
cada build, sin escribir un test, cada gpu fn pasa una bateria de bordes --
0, -0, NaN, infinitos, subnormales, los maximos, 0.1 -- y sus celdas reales
por TRES caminos: la 3060 simulada (sobre el SASS), el interprete de la casa
(sobre el Programa) y el calculo (sobre la IR). Si no dan los mismos bits, no
hay `.bex`.

**Los resultados los da la 3060 simulada**: `titan build` corre cada gpu fn
en el simulador de la 3060, sobre el SASS que el juez acepto, y el `.bex`
lleva esas celdas. `titan check` corre LO MISMO que `titan build`, sin
escribir nada (08-10: check y build dicen lo mismo por construccion);
`titan ir` las calcula con f32 de precision simple; los dos bancos comparan
lo mismo. Que corra en la 3060 de verdad es G4: Ring 0, del propietario.

### Los bucles y las llamadas (08-10, LB5 de `docs/plan/PLAN_LAS_LIBRERIAS.md`; la ley L33)

```text
gpu fn cuadrado(x: f32) -> f32
    return x * x

gpu fn a_la_ocho(x: f32) -> f32
    let mut r = x
    for i in range(3)          # N ESCRITO: el hilo acaba por construccion
        r = cuadrado(r)        # otra gpu fn: se escribe EN LINEA aqui
    return r
```

- `for i in range(N)` o `range(A, B)`, con los extremos ESCRITOS como enteros,
  hasta 2^24: el contador es f32, como todo numero de una gpu fn. Dentro,
  `break`, `continue` y `return`. El `while` no: de el no se sabe al compilar
  cuantas vueltas da (DL4 del propietario).
- **LA OBRA de una celda** se cuenta al compilar: cada operacion y cada linea
  escrita es 1, y un `for` sus vueltas por (2 + lo de dentro). Como mucho
  65536. **09-10 (DL10):** la division GENERAL pesa 24 -- en la 3060 es una
  cuenta entera, de 36 a 88 instrucciones --; entre una potencia de dos
  ESCRITA (`/ 2.0`, `/ 4`) es un producto, y pesa 1. Un hilo de la GPU tiene que acabar pronto, y asi acaba: en la 3060,
  en la CPU al correr y en el simulador de cada tarjeta, tambien en las
  celdas que llegan al correr, que el calculo nunca ve. Pasar de ahi es
  T0066, en su `for`.
- Una gpu fn llama a otra gpu fn -- nunca a una de la CPU, ni a si misma, ni
  en circulo --, y la llamada se escribe EN LINEA: un hilo no tiene pila, y
  una gpu fn es pura. Su obra cuenta donde se la llama.
- Cada `for` es un `Bucle` de verdad en las dos tarjetas; los `if` siguen en
  linea recta. Los ejemplos: `nivel11/bucles` y `nivel11/en_linea`; sus NO,
  `nivel11/hilo_largo` (T0066) y `nivel11/gpu_mientras` (T0090).

### Las VECINAS (18-10, LB9 de `docs/plan/PLAN_LAS_LIBRERIAS.md`; DL9 del propietario)

```text
gpu fn suaviza(x: f32, campo: [f32]) -> f32
    let i = celda()                     # la celda que cuenta ESTE hilo
    return (campo[i - 1.0] + 2.0 * x + campo[i + 1.0]) * 0.25

    let ys = suaviza(xs, xs)            # xs celda a celda, y xs ENTERA
```

- Un parametro `[f32]` es la lista ENTERA: llega igual a cada hilo, y es de
  SOLO LEER (ni `mut` ni `own`). Se lee por la celda que se quiera,
  `campo[k]`, con un f32: `k` va hacia el cero (`f32 as i32`; un NaN, 0).
- **Fuera de la lista, 0 (DL9: "da 0, al correr").** La celda -1 o la que
  pasa del largo lee 0, en la 3060, en la CPU, en la casa y en el calculo.
  Por eso una vecina que no esta no pesa si su masa se lee de otra lista.
- `celda()` es el numero del hilo, en f32 (0, 1, 2...). Solo dentro de una
  gpu fn: en una fn de la CPU no hay hilo (T0090).
- Lo demas llega CELDA A CELDA como siempre (una lista, una tabla o un valor
  suelto, el mismo en cada hilo), y dice CUANTOS hilos hay: hace falta uno
  (solo listas enteras es T0090). El resultado tiene la forma del primero
  que no va suelto, como en MC4.
- Una gpu fn que mira a sus vecinas es un COMPUTO entero (un hilo por
  celda, sus listas en UAV): no se llama desde otra gpu fn (T0090), ni
  dibuja. Una lista entera sin su celda (`campo + x`) es T0090.
- Las tres respuestas de siempre (L31): cada tarjeta (la 3060 con los
  permisos de computo de R7, la CPU con `nativo_computo`), la casa
  (`Programa::despachar`) y el calculo, con los mismos bits, tambien en la
  bateria de vecinas (listas mas cortas que los hilos: los ultimos miran
  fuera). Al correr, la CPU de reserva corre el computo un hilo detras de
  otro (L32).
- Los ejemplos: `nivel11/vecinas` (el desenfoque) y `nivel11/densidad` (la
  primera densidad de un fluido, IL1); su NO, `nivel11/celda_en_la_cpu`
  (T0090).

**`piso(x)` (18-10, IL2 de `PLAN_ILLAPA`)**: el entero de abajo de un f32,
los bits de `f32::floor` -- `piso(-0.5)` es -1, `piso(-0.0)` es -0.0, un NaN
sigue NaN, y lo que no cabe en un entero (desde 2^23, el infinito) queda
como estaba. Solo dentro de una gpu fn: en la CPU es contar con un f32
(T0091, `nivel11/piso_en_la_cpu`). Con el, una gota sabe su celda
(`piso(x / h)`) y un hilo el bit `j` de su numero (`piso(i / j)`, y su
paridad), que es lo que pide un orden en pasos. El ejemplo,
`nivel11/rejilla` (IL2): 128 gotas en 16 x 16 celdas, ordenadas por celda
con un orden BITONICO (28 pasos de dos gpu fn), donde empieza cada celda
por una busqueda binaria de 24 vueltas, y las vecinas mirando solo las
nueve celdas de alrededor: las mismas 332 que mirando a todas.

**LAS REDUCCIONES (18-10, `use casa.suma`)**: la CPU no suma f32 (D2), asi
que hacer de una lista UN numero va en la GPU. `suma.total(xs)`,
`suma.punto(xs, ys)`, `suma.mayor(xs)` y `suma.menor(xs)` son gpu fn de la
casa: cada hilo reduce un BLOQUE de 64 celdas seguidas, de izquierda a
derecha, y los bloques se reducen igual hasta que queda uno (un millon de
celdas, cuatro pasos). El orden es ese y no otro, en cada tarjeta y en el
calculo: por eso dan los mismos bits. No es el de sumar una a una -- otro
orden es otro f32 -- y pierde menos: en `nivel11/sumas`, 5000 celdas suman
249.7790 (la exacta a cuatro cifras), y una a una darian 249.7784. De una
lista vacia, 0.

**LA SANGRE (18-10, IL3, `nivel11/sangre`)**: PBF en 2D con todo lo de
arriba -- vecinas, `piso`, la rejilla, las sumas -- y una raiz SIN DIVIDIR
escrita en TITAN++ (`mates.inv_raiz`: potencias de cuatro y Newton con
productos, a menos de 1.4 ULP de la de IEEE, con los mismos bits en cada
tarjeta por construccion). Dos lecciones, dichas: lo PREDICHO tambien va
dentro de la caja (una gota bajo el suelo tiene una celda negativa, y la
rejilla no la ve); y para caber en la 3060, los sitios van juntos en una
lista (`pos` = x0, y0, x1, y1...) y una fila de tres celdas es UN tramo del
orden.

[!] **Lo que cuesta un UAV en la 3060**: cada lista (las de celda a celda,
las enteras y el resultado) ata CINCO registros de los 64 de un hueco (su
base, su tope, sus elementos y su paso). Con ocho listas quedan 24 para la
cuenta; si no cabe, el NO lo dice (`no cabe en los 64 registros`). Lo que
se puede leer de otra lista, mejor de ella: `ys[celda()]` en vez de otro
valor, `ini[c + 1.0]` en vez de una lista `fin`.

### La gpu fn que DIBUJA (08-10, LB6 de `docs/plan/PLAN_LAS_LIBRERIAS.md`)

```text
type Cuatro
    x: f32
    y: f32
    z: f32
    w: f32

type Vertice
    posicion: Cuatro                          # donde cae (SV_Position)
    color: Cuatro

gpu fn cubo_vertice(v: Vertice) -> Vertice    # de VERTICE: deja su posicion
    return v

gpu fn cubo_pixel(v: Vertice) -> Cuatro       # de PIXEL: recibe eso, deja su color
    return v.color
```

- **Por su FIRMA** (DL6, tomada como la recomienda el plan; el 09-10, el
  propietario la confirmo): una gpu fn que recibe UN registro y devuelve otro con un campo
  `posicion` de cuatro f32 es de VERTICE; una que recibe un registro con su
  `posicion` y devuelve un registro de cuatro f32 -- su color -- es de PIXEL.
  Lo demas es una celda, como siempre. Cada campo de un registro de dibujo es
  un f32 o un registro de 1 a 4 f32: un ELEMENTO de la tuberia (8 como
  mucho), y cada f32 un componente.
- Dentro: `let` de registros, `v.campo`, `w.campo = ...` (con `let mut w =
  v`), registros escritos (`Cuatro { x: 1.0, y: 0.0, z: 0.0, w: 1.0 }`), y todo
  lo de una celda: `if`, `for` con su N escrito, llamadas EN LINEA a gpu fn de
  celdas.
- **La de pixel no lee su `posicion`**: es de la tarjeta, que con ella sabe
  que pixel pinta. Dos registros no se comparan con `==`: campo a campo, por
  IEEE. Y una que dibuja NO SE LLAMA: es una etapa de la tuberia, y la pone a
  dibujar VERRANO (LB7). Cada una, T0090 en su sitio.
- Sale como el Programa de la casa POR ELEMENTOS -- el mismo que sale de los
  sombreadores de PROTON-X; un componente que su campo no tiene sale 0 -- y
  pasa la bateria de bordes en las dos tarjetas, componente a componente,
  contra la casa y el calculo. `titan sm86` ademas la PEGA a la tuberia de
  VERRANO de la RTX 3060 12G (`.sm86`, juzgada como en la puerta del kernel)
  y mete las del paquete en su SOBRE (`cubo.bsf`: un BSF con un modulo por
  cada una y el MAPA de la casa de fuente).
- El ejemplo: `nivel11/cubo`, el cubo de V0. Lo que lee su vertice es el
  `Vertex` de VERRANO (32 bytes: la posicion y el color), asi que su sobre lee
  lo que lee hoy el `cubo.bsf` de SASS a mano. En el anfitrion da los bits de
  la tanda de `bmo_cubo` (`emisor-x86_64/tests/cubo.rs`); en el metal, del
  propietario.

### El DIRECTOR: la lamina de VERRANO (09-10, LB7b de `docs/plan/PLAN_LAS_LIBRERIAS.md`)

```text
use director                                  # y en el Titan.toml: screen = true

fn main()
    if not director.lamina(24)                # 24 vertices en cada ranura
        print("nadie compone")                # lanzado desde el shell
        return
    for f in range(360)
        director.publica(f, n, posiciones, colores)   # [f32; 4k] las dos
        director.espera(16)                   # milisegundos
```

- **Lo que el director sabe hoy, y nada mas** (DL7 y DL8, del propietario el
  09-10: `screen` ahora, y primero la LAMINA, como INTI):
  - `director.lamina(c)`: pide el bloque de una lamina de VERRANO -- la
    cabecera `BVER` y dos ranuras de `c` vertices --, escribe su cabecera
    ENTERA y se la OFRECE a quien lanzo el programa. Da si, si la tomo; no,
    si nadie compone (lanzado desde el shell), si no hay memoria, o si el
    programa ya tiene su lamina;
  - `director.publica(f, n, posiciones, colores)`: los `n` primeros vertices
    de las tablas -- cuatro f32 de posicion (x, y, z, w, en recorte) y cuatro
    de color cada uno --, en la ranura que nadie lee, con su sello; y la
    secuencia LO ULTIMO: eso publica el fotograma `f`. Da no si no hay
    lamina, si `n` no son triangulos enteros, o si no caben en la ranura o en
    las tablas;
  - `director.espera(ms)`: duerme hasta el siguiente fotograma.
- **Un programa que habla con el director CORRE** (E1, como uno que lee):
  quien compone, y cuando, solo se sabe al correr. Sus gpu fn corren al
  correr, en la CPU, su reserva (LB4); las cuentas son las de la 3060, bit a
  bit (lo dice la bateria de cada build).
- **El certificado lo dice**: cada llamada abre la puerta de la PANTALLA
  (`screen`) en su linea, y el juez de la puerta de carga la mira.
- **Y no hay cerrojo**: el trato de la lamina es el de
  `platform/shared/verrano/src/lamina.rs` -- un escritor, un lector, un sello
  por ranura --, el mismo que escribe INTI. Una app que se para deja
  publicado su ultimo fotograma, y ya.
- Lo que el director NO acepta, cada uno en su sitio: llamarlo sin `use
  director` (T0080) o sin `screen` en el Titan.toml (T0088); desde una gpu fn
  (T0090: un hilo de la 3060 cuenta una celda, no publica fotogramas); unas
  tablas que no son de f32, o de distinto largo, o que no traen cuatro por
  vertice (T0071); `espera` como un valor (T0069); prestarle o entregarle
  algo (T0077: lo lee, no se lo queda).
- El ejemplo: `nivel11/cubo_gira`, el cubo de `bmo_cubo` contado por sus gpu
  fn y publicado: la prueba `emisor-x86_64/tests/lamina.rs` lo lanza un
  escritorio de mentira, que lee la lamina MIENTRAS corre con el lector de
  VERRANO: cada fotograma que da por bueno es la tanda de `bmo_cubo`, bit a
  bit. En el metal, `gpu verrano banco` sobre ella es del propietario.

### El DIRECTOR: el RITMO (10-10, el cuello de botella de `EL_FOCO`)

- `director.espera(ms)` duerme hasta el PLAZO del fotograma siguiente, no
  `ms` despues del trabajo: el programa lleva su plazo (el TSC, en su bloque)
  y duerme lo que FALTA. Un bucle de `trabajo; director.espera(16)` hace
  fotogramas de 16 ms, no de trabajo + 16 + lo que tarde en despertar. Si va
  tarde, cede el turno y se pone en hora (no amontona). Mas de 1000 ms, la
  siesta de siempre. La prueba: `emisor-x86_64/tests/ritmo.rs`, con el reloj
  del emulador (1 GHz).

### El DIRECTOR: la VENTANA (10-10, F1 de `docs/plan/EL_FOCO.md`)

```text
use director                                  # y en el Titan.toml: screen = true

fn main()
    if not director.ventana(320, 200)         # ancho y alto, en pixeles
        print("nadie compone")
        return
    director.rect(0, 0, 320, 200, 20 * 65536 + 30 * 256 + 60)
    director.pixel(10, 10, 16777215)          # un color: r * 65536 + g * 256 + b
    director.fila(y, pixeles)                 # una fila de una tabla de int
    director.presenta()                       # el dibujo esta ENTERO
```

- **La misma superficie que INTI y C**: la cabecera BSUP de
  `bmo_abi::syscalls::surface::superficie` (los numeros, por su nombre), los
  pixeles BGRA de 32 bits (opacos), un buzon de 64 ranuras detras, y la cola
  privada con el asa del bloque. Todo a cero (negro) ANTES de ofrecerla.
- `director.ventana(ancho, alto)`: pide el bloque, lo describe y se lo OFRECE
  a quien lanzo el programa. Da si, si se ofrecio; no, con una medida fuera
  de 1..4096, sin memoria, lanzado desde el shell, o si el programa ya tiene
  su ventana (UNA APP, UNA VENTANA: la regla del DIRECTOR).
- `director.pixel(x, y, color)`, `director.rect(x, y, ancho, alto, color)`
  y `director.fila(y, pixeles)` pintan, RECORTADO a la ventana: lo de fuera
  no se pinta, y sin ventana no pasa nada. No dan nada.
- `director.presenta()` sube la SECUENCIA de la cabecera: es lo unico que
  hace que el escritorio la componga, y va DESPUES del ultimo pixel (R-APP4).
- El certificado: la puerta de la PANTALLA, en la linea de la primera
  llamada (como la lamina). Desde una gpu fn, no (T0090).
- El ejemplo: `nivel11/ventana`, un degradado y una barra que cruza la
  ventana; la prueba `emisor-x86_64/tests/ventana.rs` la lanza el escritorio
  de mentira y lee sus pixeles, uno a uno.

### El DIRECTOR: lo que se LEE, lo que LLEGA y la LETRA (10-10, F2 y F3 de `EL_FOCO`)

```text
if director.toma() or director.fichero("datos/ejemplo.lam")
    let n = director.medida()                 # cuantos bytes se tienen
    let b = director.byte(0)                  # uno, o -1 fuera
let que = director.evento()                   # 0 nada, 1 tecla pulsada, 2 soltada,
                                              # 3 letra, 4 raton, 5 la ventana cambio,
                                              # 6 la rueda, 7 movimiento relativo
let tecla = director.codigo()                 # su scancode o su letra (Latin-1)
let x = director.raton_x()                    # y raton_y(), botones()
                                              # evento 7: dx/dy con signo
director.raton_captura(true)                  # solicita; no cede hardware
if director.raton_capturado()                 # el DIRECTOR concedio (con foco)
    director.raton_captura(false)             # suelta; el cursor reaparece
let muescas = director.rueda()                # con signo: arriba +, abajo -. Sola
                                              # o con Alt; Ctrl + rueda es del
                                              # escritorio (como todo Ctrl)
let ruta = director.argumento()               # lo de detras de la ruta al lanzar
                                              # ("" si nada): el explorador abre
                                              # asi un fichero con su app (15-10)
if director.se_ve()                           # no minimizada ni tapada (R-APP8)
    x = director.texto(8, 8, "hola", 1, 16777215)   # la x de detras
    x = director.letra(x, 8, 241, 2, 16777215)      # un glifo (aqui la n con tilde)
let t0 = director.ms()                        # el reloj, en ms (TA5): para MEDIR
director.profundidad(true)                    # la lamina con z-buffer (V2)
```

- **La profundidad** (V2 de `docs/plan/PLAN_VERRANO.md`, 11-10):
  `profundidad(descarta)`, despues de `lamina(c)`, dice en la cabecera de la
  lamina como se dibuja: con z-buffer (lo de delante tapa, se dibuje en el
  orden que se dibuje) y, con `true`, sin las caras de DETRAS -- las que no
  son horarias en la pantalla: cada cara, horaria vista desde fuera, como en
  D3D --. La app da los triangulos enteros; quien se ve lo decide VERRANO.
  El ejemplo: `nivel11/dos_cubos`.

- **El reloj** (TA5 de `docs/plan/PLAN_LA_TINTA.md`, 10-10): `ms()` son los
  milisegundos del reloj del procesador (`rdtsc` y los ciclos por segundo
  que dice el kernel). Su cero es el de la maquina: sirve para restar --
  lo que tarda un trazo, un fotograma --, no para decir la hora. Si el
  kernel no sabe el reloj, 0: una resta de ceros dice "no se mide".

- **Lo tenido** (F2): `toma()` -- lo que alguien OFRECIO a este programa, la
  pagina del antenista: ocho intentos, un fotograma entre uno y otro -- o
  `fichero(ruta)` -- uno entero, de 1 byte a 256 KiB, en un bloque suyo --.
  Se lee ANTES de la ventana, si hace falta: va en un bloque de 48 bytes del
  programa (`rbx`). `byte(i)` da 0..255, o -1 fuera; nada se escribe.
- **Lo que llega** (F3): el buzon de la ventana (64 ranuras, el contrato de
  `superficie.rs`): `evento()` toma el siguiente y dice QUE es; sus datos,
  hasta el siguiente. Sin ventana, nada (0).
- **La letra**: la de BMO-X, 8x16 de `fontgen` (la del kernel y la de INTI):
  ASCII, los 25 extras Latin-1 del castellano, y `?` para lo que no tiene.
  Escala 1..16 (cada pixel, un cuadrado), recortada a la ventana. Dan la x
  de detras.
- Las pruebas: `emisor-x86_64/tests/entrada.rs` (lo ofrecido, el fichero y
  los que no, los eventos del buzon, la letra pixel a pixel contra la tabla
  de `fontgen`).

### El DIRECTOR: lo que se GUARDA (10-10, TA4 de `PLAN_LA_TINTA`, R1 de `EL_FOCO`)

```text
if director.guarda("datos/resol.txt", "1280 720\n")   # si, si llego al disco
```

- `director.guarda(ruta, texto)`: el fichero ENTERO, creado de cero
  (`TASK_OP_ARCHIVO_CREAR`), el texto de 7 en 7 (`ARCH_OP_ESCRIBIR`) y
  cerrado, que es donde llega al disco. Da si, si entraron todos los bytes
  y el disco dijo que si; no, sin ruta o si el disco dice que no (y entonces
  no queda NADA: un fichero a medias se parece demasiado a uno entero). No
  pide ventana, y lo tenido (`medida`, `byte`) no cambia.
- La primera app que lo usa: RESOLUCION (`Ultra_userspace/apps/resolucion`),
  que guarda la medida elegida; `nivel11/ventana` la lee al abrirse (su
  `medida.titan`). Las pruebas: `tests/entrada.rs` y `tests/resolucion.rs`.

```text
if director.crea("datos/foto.bic")            # UNO a la vez
    director.escribe(66)                      # un byte, 0..255
    if not director.cierra()                  # ahi llega al disco
        print("no se guardo")
```

- **BYTE A BYTE** (10-10, corte 4e de INTI): `crea(ruta)` lo crea de cero
  (da no con otro abierto, o sin ruta), `escribe(b)` mete un byte -- de 7 en
  7 al kernel -- y `cierra()` manda lo que quede y cierra: si, si entro todo
  y el disco lo guardo. Un byte fuera de 0..255 NO se recorta: no se
  escribe y `cierra` dice que no. [!] El kernel guarda lo que tenga al
  cerrar: quien no quiera un fichero a medias lo comprueba TODO antes de
  `crea` (lo hace `apps/bico`). Lo tenido no cambia.
- Los que lo usan: `Ultra_userspace/apps/bico` (BMP y QOI a BICO) y
  `apps/png` (un PNG valido), que eran de INTI. Las pruebas:
  `tests/disco.rs`, `tests/bico.rs` (los ficheros rotos de siempre) y
  `tests/png.rs` (los mismos bytes que `png.inti`).

> **08-10, LB4 de `docs/plan/PLAN_LAS_LIBRERIAS.md`:** las tarjetas son DOS,
> la 3060 y la CPU -- la RESERVA de toda GPU (la ley L32) --, y cada gpu fn
> pasa la bateria en las dos con los mismos bits. Un programa que no lee
> sigue llevando las celdas de la 3060; uno que lee (nivel 12) la corre AL
> CORRER, en la CPU.

---

## Nivel 12 -- lo que viene de fuera (25 palabras: + ninguna; `lee()` y `numero(t)`) -- 05-10, E1 de PLAN_LA_ENTRADA

```text
# pregunta.titan
mod main "saluda a quien teclee su nombre"

fn main()
    print("como te llamas?")
    let nombre = lee()
    print("hola ", nombre, "!")
```

**`lee()` trae la linea que se teclea en la consola del programa**, sin su
salto, como un texto. Es de la BIBLIOTECA, como `print`: no hay palabra nueva,
las 25 se quedan en 25 (decision del propietario, 05-10). No pide permiso: la
consola del propio programa es la misma puerta por la que escribe `print`
(`Door::Console` del certificado), y el certificado la nombra en su linea. El
teclado y el raton de una VENTANA son otra cosa (`input`, con REX).

**Lo que cambia por dentro es grande, y se dice entero.** Hasta el nivel 11 el
calculo CORRIA el programa al compilar y el `.bex` solo escribia lo que salia.
Con `lee()` eso ya no se puede: lo tecleado no se sabe al compilar. Asi que un
programa que lee se JUZGA entero al compilar -- sus clases, sus prestamos --
y se EMITE para correr de verdad en la maquina (E1 del emisor,
`emisor-x86_64/src/e1/mod.rs`). Un programa que no lee sale igual que siempre.

```text
   lee()               una linea, como texto (126 bytes como mucho: una mas
                       larga es un NO T0060, nunca un corte callado)
   lee() + 1           NO T0063: un texto no se suma a un numero, ni tecleado
   if lee() ...        NO T0065: un si/no se pregunta como a cualquier texto
   lee()  (suelta)     NO T0069: lo tecleado se perderia nada mas llegar
   lee(1)              NO T0068: `lee` no pide nada
```

**Lo que solo falla al correr, ATRAPA** (la regla 1 de INTI): una suma que no
cabe en 64 bits (T0060), un `/` o `%` por cero (T0061), una division entre
enteros que no es exacta (T0062). El programa escribe el NO con su linea --
`NO T0060 al correr, linea 6: el resultado no cabe en 64 bits` -- y sale. Un
`print` calcula todas sus partes ANTES de escribir: un NO nunca deja media
linea delante.

### `numero(t)`: lo tecleado, como numero -- un CASO, no un int

```text
# adivina.titan (el corazon)
    let t = lee()
    match numero(t)
        Es(n)
            if n < secreto
                print("mas alto")
            ...
        NoEs
            print("eso no es un numero: ", t)
```

**`numero(t)` da `Es(n)` si el texto ES un entero, y `NoEs` si no** -- el
caso del nivel 8, y el `match` obliga a mirar los dos: sin null, sin
excepcion, sin un 0 inventado (decision D3 del propietario, 05-10). La regla
es una sola, la misma al compilar y al correr (`src/prelude.rs`):

```text
   "42", "  42 ", "+7", "-0"      Es: blancos alrededor, un signo, cifras
   "4x", "", "-", "1 2", "12.5"   NoEs: un entero, y nada mas
   "9223372036854775808"          NoEs: no cabe en 64 bits
   numero(lee()) + 1              NO T0063: un caso no se suma; mira el caso
```

`Numero`, `Es` y `NoEs` los pone el preludio SOLO si el programa llama a
`numero`; un programa que ya usa esos nombres para lo suyo y llama a `numero`
da T0055.

### El programa entero corre en la maquina

E1 emite TODO lo de los niveles 0 a 10 -- `dec` exacto, tablas, registros,
casos, llamadas con valores, `mut` y `take`, `round`, `dec(p, s)`, traits --
para que un valor tecleado llegue a cualquier sitio del lenguaje. Y lo que el
calculo veria corriendo, la maquina lo ve corriendo, con su linea:

```text
   T0060   no cabe en 64 bits (un int, las cifras de un dec, un texto de mas
           de 248 bytes, una linea tecleada de mas de 126)
   T0061   / o % por cero
   T0062   una division que no acaba exacta (7 / 2 entre int, 1.0 / 3),
           o `round` de un infinito o un NaN que dejo una gpu fn (LB4)
   T0072   una celda que no esta en la tabla (t[i] con i tecleado)
   T0074   el PIC: un numero que no cabe en su dec(p, s)
   T0066   las llamadas se anidan mas de lo que cabe en la pila: el NO sale
           en la linea de la LLAMADA, como lo dice el calculo
```

**La vara de E1 es el calculo** (`emisor-x86_64/tests/e1.rs`): cada programa
BIEN del banco se emite TAMBIEN por E1 y tiene que escribir lo mismo, letra a
letra; cada NO del banco que el calculo encuentra corriendo, E1 lo encuentra
con el mismo codigo y en la misma linea; y miles de cuentas al azar con
numeros tecleados (`E1_AZAR=semilla,casos`) dan lo mismo en los dos. Donde el
calculo hace un paso en 128 bits, E1 tambien (`emisor-x86_64/src/e1/ancho.rs`).

Un bucle SIN fin que no lee sigue siendo T0066 al compilar: el presupuesto de
plegado (R8) cambia una ley, y las leyes las sella el propietario.

> **08-10, LB4:** la gpu fn y el f32 YA corren en un programa que lee. La gpu
> fn, en la CPU, su reserva: el x86-64 de la tarjeta de la CPU, juzgado y
> con los bits del oraculo en cada build (L32). El f32, como DATO: un
> literal, un int o un dec entra al f32 mas cercano por su tipo declarado
> (`let a: f32 = x`), se guarda y se pasa, y vuelve por `round` -- E1 no
> cuenta en f32 (D2) --. El ejemplo: `nivel12/reserva`. Lo de abajo es lo que
> decia antes de LB4.

**Lo que la maquina TODAVIA NO corre es T0040**, *lo que todavia no existe*
(08-10, LB1 de `docs/plan/PLAN_LAS_LIBRERIAS.md`): una `gpu fn` o un `f32` en un
programa que lee -- la gpu fn hoy solo corre al compilar, en la 3060 simulada,
y llega al correr con su libreria (LB4); el f32 vive en la 3060 (LB7) --, y lo
demas que E1 aun no emite. Se dice en `check` y en `build`, en su linea, con
su porque y su como; antes salia como "el .bex no paso el gate ... fallo del
compilador".

---

## Nivel 13 -- lo que crece (25 palabras: + ninguna; `[T]`, `{K: V}` y su biblioteca) -- 05-10, PLAN_LISTAS_Y_MAPAS

```text
# agenda.titan (el corazon)
    let mut todos: [text] = []
    let mut veces: {text: int} = {}
    let mut nombre = lee()
    while nombre != "fin"
        push(mut todos, nombre)
        match get(veces, nombre)
            Hay(n)
                put(mut veces, nombre, n + 1)
            NoHay
                put(mut veces, nombre, 1)
        nombre = lee()
```

**Una LISTA crece y encoge al correr; un MAPA lleva de una clave a un
valor.** Se escriben con los simbolos que ya habia, sin palabra nueva
(decisiones D1-D4 del propietario, 05-10):

```text
   [int]                 una lista de int        [] vacia, con su tipo dicho
   {text: int}           un mapa                 {} vacio, {"ana": 3} escrito
   push(mut l, x)        x al final              l[i], l[i] = v, len(l), for x in l
   let u = pop(mut l)    el ultimo, como caso: Hay(x) o NoHay -- y la lista lo pierde
   put(mut m, k, v)      k lleva a v (si estaba, su valor nuevo)
   get(m, k)             Hay(v) o NoHay           has(m, k): true o false
   remove(mut m, k)      k ya no esta             for k in m: sus claves, en el orden
                                                  en que ENTRARON
   Opcion[int]           el tipo de lo que dan get y pop, para escribirlo:
                         fn busca(m: {text: int}, k: text) -> Opcion[int]
```

**Es un VALOR, como todo** (D2): `let b = a` es OTRA lista; `push(mut a, x)`
cambia solo `a`. Se presta con `mut` -- tambien una PARTE de un valor:
`push(mut nave.carga, x)` -- y se entrega con `take`, como cualquier valor.
`[1, 2, 3]` escrito es una tabla hasta que va donde se dice una lista
(`let l: [int] = [1, 2, 3]`): ahi se vuelve lista, y un `2` en una `[dec]` es
un decimal.

**Lo que no esta es un CASO** (D3): `get` y `pop` dan `Hay(v)` o `NoHay`, y el
`match` obliga a mirar los dos. `Hay` y `NoHay` los da la biblioteca: no se
escriben a mano. La clave de un mapa es un int, un text, un bool o un registro
de esos (un decimal no: 1.0 y 1.00 son el mismo numero).

```text
   let l = []              NO T0071: una lista vacia no dice de que es
   push(l, 1)              NO T0077: push cambia su lista, se le presta: mut l
   let l: [int] = ...      NO T0056 en push(mut l, 1): sin `let mut` no cambia
   let x = Hay(3)          NO T0079: Hay lo da la biblioteca
   m[0]  (un mapa)         NO T0063: sus valores se piden por su clave, con get
   print(pop(mut l))       NO T0069: pop va solo, o como el valor de un let o un match
   l[9]  (fuera)           NO T0072, al compilar o al correr
```

**En la maquina** (E1, `emisor-x86_64/src/e1/coleccion.rs`): una lista es su
ASA -- puntero, cuantas, cuantas caben -- y sus celdas viven en un MONTON
propio (`e1/monton.rs`) que crece al doble y suelta lo que muere: un local al
cerrarse su bloque o al volver su fn, lo viejo de un `=`, lo que una cuenta
calculo y nadie se quedo. Sin recolector: se suelta donde el juez ya sabe que
el valor muere. Al acabar no queda NADA pedido -- y si quedara, el programa lo
dice. La vara, la de siempre: el calculo, programa a programa y al azar
(`emisor-x86_64/tests/e1.rs`).

### Listas y mapas de un trait (O4, 18-10, sin palabras nuevas)

Una `[Nodo]` guarda valores de TIPOS DISTINTOS que cumplen `trait Nodo`, cada
uno con su tipo; un `{text: Nodo}`, por nombre. Recorrerla llama a la fn de
cada uno: lo que en otros lenguajes es una lista de objetos con herencia,
aqui es una lista de un comportamiento.

```text
let red: [Nodo] = [Rejilla { lado: 4, paso: 1000 }, Mueve { dx: 5, dy: 0, dz: 0 }]
push(mut red, Copia { veces: 3, dy: 500 })      # cualquiera que cumpla Nodo
for n in red
    puntos = aplica(n, puntos)                  # la fn de SU tipo
```

```text
   push / put / una lista escrita con un valor que no cumple el trait   T0085
   `let n: Nodo`, un campo `Nodo`, `[Nodo; 3]`: el trait SUELTO          T0087
```

La ley L21 no cambia: el trait suelto sigue siendo tipo de un parametro y
nada mas; lo nuevo es la LISTA o el MAPA de el, que tambien van en campos,
resultados y parametros. En la maquina cada celda lleva la etiqueta de su
tipo y mide lo que el mayor de los que cumplen el trait; el monton suelta
cada una segun su tipo. Lo que aun no: prestar una celda para cambiarla
(`cambia(mut red[0])`, el mismo limite que las demas celdas, O1).
[Ejemplo: geometria por nodos](ejemplos/nivel13/red_de_nodos.titan).

---

## Nivel 14 -- el byte (25 palabras: + ninguna; `byte` y `byte(x)`) -- 10-10, TA1 de PLAN_LA_TINTA

```text
# pixeles.titan (el corazon)
    let mut px: [byte] = []
    for y in range(alto)
        for x in range(ancho)
            push(mut px, gris(x, y))      # gris devuelve un byte
    px[y * ancho + 3] = byte(0)           # la goma
    let v = px[i]                         # leido, cuenta como un int
    suma = suma + v
```

**Un BYTE es un numero entero de 0 a 255, y una lista de bytes guarda UN
byte por celda.** Es lo que pide un programa de dibujo: una pagina de manga
de imprenta son 52 millones de pixeles, 420 MB en `[int]` y 52 MB en `[byte]`
(`docs/plan/PLAN_LA_TINTA.md`, la cuenta que manda).

```text
   byte                  el tipo: let b: byte = ...   [byte]   fn f(c: byte) -> byte
   byte(x)               un int a byte: si x no va de 0 a 255, NO T0060
                         (al compilar si se sabe; al correr si viene de fuera)
   leido                 cuenta como el int que es: b + 1, b < 200, t[b],
                         range(b), print(b) -- y su resultado es un int
   un byte donde va      un int o un dec: entra (200 es 200, nada se pierde)
   un int donde va       un byte: NO T0071 -- se dice con byte(x), que mira
                         que quepa. Nunca un corte callado
   [byte] y [int]        dos listas distintas: una [byte] no se mira como
                         [int] (seria otra lista, no otra mirada): T0071
   push(mut b, "texto")  en una [byte], las letras del texto, una detras de
                         otra (16-10, PL6 de PLAN_ECCE: un fichero que es
                         texto y bytes a la vez, como un PDF)
```

```text
   let b: byte = 7         NO T0071: un int no entra callado; byte(7)
   push(mut px, 7)         NO T0071: lo mismo en una [byte]
   byte(256)               NO T0060: no cabe en un byte
   byte(i) con i = 300     NO T0060 al correr, en su linea
```

**En la maquina** (E1, `emisor-x86_64/src/e1/coleccion.rs`): un byte SUELTO
--un local, un campo, el valor de un mapa, lo que lleva un `Hay`-- es una
palabra de 8, como un int: la aritmetica no sabe que existe. Solo la CELDA
de una `[byte]` mide un byte: se lee con `movzx` y se escribe con el byte
bajo. La vara, la de siempre (`emisor-x86_64/tests/bytes.rs`): el calculo y
la maquina escriben lo mismo, o el mismo NO en la misma linea; y la medida
que justifica el tipo -- tres millones de celdas caben en `[byte]` y no
caben en `[int]` en el monton que da el emulador --.

---

## Nivel 15 -- la lamina (25 palabras: + ninguna; `lamina` y `lamina(n)`) -- 14-10, A4b de EL_FOCO

```text
# dos_laminas.titan (el corazon)
fn llena(mut l: lamina, v: int)
    for i in range(len(l))
        l[i] = byte(v)

fn suma(take l: lamina) -> int
    let mut s = 0
    for i in range(len(l))
        s = s + l[i]
    return s

fn main()
    let mut a = lamina(100)        # 100 bytes, pedidos al kernel
    llena(mut a, 3)                # PRESTADA: se ve el cambio al volver
    print(suma(take a))            # ENTREGADA: ahora es de `suma`, que la suelta
```

**Una LAMINA es RAM que el programa le pide al kernel y TIENE EN LA MANO.**
El propietario, 14-10: *"RAM = convertir como LAMINA ABSOLUTA, LAMINA ese
mismo para administrar y llamar, no quiero tener que lidiar como hizo C con
C++"*. Lo que en C se escribe a mano (`malloc`, `free`, y el `free` de mas o
de menos) aqui no se escribe:

```text
   lamina                el tipo: let mut l: lamina = ...   fn f(mut l: lamina)
   lamina(n)             n bytes, por la PUERTA DE LA RAM del programa (A4a):
                         si el kernel no los da, NO T0060 al correr, en su linea
   l[i]    l[i] = b      sus celdas son BYTES, como las de una [byte]: leida
                         cuenta como el int que es; se escribe con byte(x)
   len(l)                cuantos bytes; fuera de ellos, NO T0072 al correr
   f(mut l)  f(take l)   se PRESTA (nivel 7: un puntero) o se ENTREGA (su asa
                         se mueve; quien la entrego ya no la tiene: T0075)
   al morir quien        se suelta SOLA (MEM_OP_SOLTAR): al cerrar su bloque,
   la tiene              al volver su fn, al tomar otro valor, con break, con
                         continue. Nadie la suelta dos veces
```

**Y lo que NO, todo T0077: una lamina no se COPIA.** Una copia seria dos
nombres para el mismo bloque del kernel, y uno lo soltaria debajo del otro.

```text
   let b = a               NO T0077: se copiaria
   print(a)                NO T0077: imprimirla es copiarla
   f(a)  con  fn f(l: lamina)   NO T0077: recibirla como copia
   fn f() -> lamina        NO T0077: devolverla (aun: va con `director.ofrece`)
   [a]   {k: a}   T { l: a }    NO T0077: una lamina no va dentro de otro valor
   push(mut a, byte(1))    NO T0077: una lamina no crece ni encoge
   a[0] = 7                NO T0071: un int no entra callado en un byte
   lamina(10)  suelta      NO T0069: pedir RAM que nadie guarda
```

**Un programa con una lamina CORRE** (como uno que lee, nivel 12): si el
kernel da la RAM, solo se sabe al correr. Y una `gpu fn` no pide laminas
(T0090): un hilo de la 3060 cuenta una celda.

**En la maquina** (E1, `emisor-x86_64/src/e1/lamina.rs`): su asa son cuatro
palabras -- donde empiezan sus bytes, cuantos, cuantos caben y su asa del
KERNEL --; las tres primeras son las de una lista, a proposito, y por eso
`l[i]` lee y escribe un byte con el mismo `movzx` que una `[byte]`. Pedir y
soltar salen por la puerta de la RAM (`e1/ram.rs`). La vara
(`emisor-x86_64/tests/la_lamina.rs`): el emulador cuenta los bloques que se
piden y los que vuelven -- veinte laminas en un bucle con un tope de ocho a
la vez solo caben si cada vuelta suelta la suya --, y cada lamina se suelta
UNA vez (saboteado: quien la entrega tambien la soltaria, y cae).

---

### El sonido: `use sonido` (Q2 de EL_FOCO, 15-10)

```text
use director                     # leer la partitura (screen = true)
use sonido                       # y en el Titan.toml: sound = true
    if not sonido.toma()          # el sonido es nuestro (y el tubo, armado)
    let mut banco = lamina(2097152)
    sonido.banco(banco)           # la lamina, prestada como banco: sus bytes
    banco[0] = byte(184)          # las muestras: 16 bits con signo, mono
    sonido.toca(0, 0, n, 24000)   # canal, desde que byte, cuantas, a cuantos Hz
    while sonido.suena(0)         # el orquestador la toca; el programa espera
        director.espera(5)
    sonido.calla(0)               # (255: todos)
    sonido.suelta()               # callar, desarmar lo que se armo, soltar
```

**La lamina es el banco, y lo que suena lo mezcla el KERNEL** (las voces del
orquestador): el programa declara y no empuja muestras a tiempo, asi que si
se atasca el sonido no se corta. `sonido.pita(hz, ms)` es el altavoz del PC
(la placa del propietario no lo tiene). Una gpu fn no suena: el sonido es de
la CPU. El primero que suena con esto: `Ultra_userspace/apps/musica`.

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
| T0040 | lo que todavia no existe: una palabra de un nivel que no llego, el tipo `f64`, o lo que la maquina todavia no corre (una `gpu fn` o un `f32` en un programa que lee, y lo demas que E1 aun no emite) |
| T0050 | no hay `fn main()` |
| T0051 | se llama a algo que no existe |
| T0052 | una funcion definida dos veces |
| T0053 | un ciclo de fn SIN parametros: vuelve siempre igual y no termina nunca |
| T0054 | un nombre se lee antes de que un `let` le de valor (el juez) |
| T0055 | un nombre que ya tiene valor, o que es de una funcion (el juez / los nombres) |
| T0056 | se cambia un valor sin `mut` (el juez) |
| T0057 | un `mut` que no cambia nunca (el juez) |
| T0058 | un nombre que nacio en un bloque que ya se cerro (el juez) |
| T0060 | un numero que no cabe en 64 bits: desbordar es un error (el calculo) |
| T0061 | una division o un resto entre cero (el calculo) |
| T0062 | una division que no es exacta: 7 / 2 entre ints, o un decimal que no acaba como 1.0 / 3 (el calculo) |
| T0063 | un texto con un numero: no se suman ni se convierten solos (el calculo) |
| T0064 | un `mut` que cambiaria de clase: numero, texto o si-o-no (el calculo) |
| T0065 | se pedia un si-o-no y llego otra cosa: `if vidas`, `not 3` (el calculo) |
| T0066 | el programa sigue corriendo despues de un millon de pasos: un bucle sin salida (el calculo); o el hilo de una `gpu fn` hace mas de 65536 de obra por celda -- la division general pesa 24 --, dicho al compilar (la gpu, L33) |
| T0067 | `break` o `continue` fuera de un bucle (la gramatica) |
| T0068 | una llamada con mas o menos valores de los que pide la fn (los nombres) |
| T0069 | se usa como valor algo que no devuelve nada, o un `return` que no cuadra con su `->` (los nombres) |
| T0070 | una fn que promete un valor tiene un camino sin `return` (el juez) |
| T0071 | un valor de otra clase donde un parametro, un `->`, un campo o una tabla dicen una (el calculo) |
| T0072 | una celda fuera de su tabla: `t[5]` de una `[int; 5]` (el calculo) |
| T0073 | un campo que el registro no tiene, que le falta o que esta dos veces (los nombres / el calculo) |
| T0074 | un valor que no cabe en su `dec(p, s)`: mas cifras o mas decimales (el SIZE ERROR de COBOL, el calculo) |
| T0075 | se lee lo que se entrego con `take` (el juez) |
| T0076 | el mismo valor prestado dos veces, o prestado y leido, en una llamada (el juez: la regla de FORTRAN) |
| T0077 | `mut` / `take` en la llamada que no cuadran con el parametro, o fuera de una llamada (los nombres) |
| T0078 | un `match` que no cubre todos los casos de su enum (los nombres) |
| T0079 | una rama de `match` que no vale: un caso que no existe, de otro enum, repetido, con mas o menos nombres, o un `_`; o un enum que se contiene a si mismo (los nombres) |
| T0080 | se usa un modulo que no es hijo ni esta en la cabecera con `use` (el paquete); o el director, sin `use director` (09-10, LB7b) |
| T0081 | un `use` del que no se usa nada (el paquete) |
| T0082 | se llama a algo de otro modulo que no es `pub` (el paquete) |
| T0083 | `mod hijo` sin su fichero, o un fichero que dice otro nombre (el paquete) |
| T0084 | un ciclo entre modulos: las capas solo bajan (el paquete) |
| T0085 | un valor que no cumple el trait que pide un parametro: se dice en la llamada (el calculo) |
| T0086 | un `trait ... for` que no cumple su trait: le falta una fn, le sobra, o tiene otra firma (el comportamiento) |
| T0087 | un trait donde solo va un parametro entero: en un `let`, un campo, una tabla o un resultado (el comportamiento) |
| T0088 | se usa un nodo de BMO-X (`gpu`, `director`) que el Titan.toml no pide (el paquete, U2) |
| T0089 | un Titan.toml que no se lee: una linea que no es seccion, clave o comentario (el paquete) |
| T0090 | una `gpu fn` que no es una celda de la 3060: print, tablas, textos, mut / take, sin resultado, una llamada a la CPU, al director o a si misma, un `while`, o un `range` sin sus extremos escritos (la gpu); o lo que su libreria todavia no hace exacto, en su sitio (la division general en la 3060, hasta que DL10 la hizo exacta el 09-10) |
| T0091 | un f32 contado, comparado o impreso en la CPU: alli se guarda o se pasa, y vuelve con `round` (el calculo, D2) |

## El banco: lo que dice cada ejemplo de si mismo

```text
   # espera: BIEN        compila...
   # entra: Ada          (nivel 12) ...el banco TECLEA esto, linea a linea...
   # sale: hola          ...y al CORRER escribe exactamente esto, linea a linea
   # espera: T0053       NO compila, con este codigo, y no escribe ningun .bex
```

Un ejemplo es un fichero (`ejemplos/nivel8/formas.titan`) o, desde el nivel 9,
una CARPETA: un paquete, cuyo `src/main.titan` lleva esas lineas
(`ejemplos/nivel9/flota/`). Si el NO esta en otro fichero del paquete, el
banco lo dibuja sobre ese fichero.

Las lineas `# sale:` las comprueba el banco del emisor
(`emisor-x86_64/tests/banco.rs`): construye el `.bex`, lo carga como el
cargador del kernel y lo CORRE en el emulador. Un nivel esta hecho cuando sus
programas HACEN lo que dicen, no cuando compilan.

## Del texto al `.bex` (T3, 2026-10-04)

```text
   titan check hola.titan              bien, o el mensaje de 4 partes: lo
                                       MISMO que diria build (08-10)
   titan ir    hola.titan              la IR propia: lo que recibe el emisor
   titan build hola.titan -o hola.bex  el .bex, con su manifiesto, por el gate
   titan check flota/src/main.titan    un PAQUETE (nivel 9): sigue sus `mod`
                                       desde `flota/`, como F1
```

En la maquina: `run titan/hola.bex` en la consola (F12). `build.ps1` deja
`titan/hola.bex` y `titan/dos.bex` en el disco. El 08-10 `hola.bex` escribio
`hola` en el Ryzen (`docs/metal/METAL_2026-10-08.md`): el primero en el metal.
Y `dos.bex`, sus dos lineas, el mismo dia.
