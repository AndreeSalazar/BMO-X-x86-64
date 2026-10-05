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

**Se dice en los DOS lados** (`f(mut x)`, no solo `fn f(mut n)`): quien lee la
llamada sabe que le puede pasar a `x` sin abrir la funcion -- en C, `f(&x)` solo
dice "quiza". Y un `mut` en un parametro que nunca cambia es T0057, como el de
un `let`: pide una copia.

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
   trait NOMBRE                 y debajo, sus fn SIN cuerpo: lo que promete.
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
no cual es (T0087).

Entre modulos, como todo lo demas: `pub trait Pieza` en `pieza.titan`, y en
otro `trait pieza.Pieza for Rueda` y `pieza.vueltas(p, 2)`.

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
   let xs: [f32; n] = [...]        un numero entra a f32 SOLO por un tipo
                                   declarado (D4): ahi se redondea, una vez
   round(x, 2)                     la puerta de vuelta: el f32 se escribe
                                   entero en decimal y se redondea como todo
                                   round de TITAN++ (la mitad, lejos del cero)
```

**Dentro de una `gpu fn`** todo numero es f32 (`2.0`, `1`), y hay `let`,
`if` / `else` y `return` con `+ - * /` y comparaciones. No hay `print` (la 3060
no tiene consola), tablas, textos, registros, llamadas ni bucles: el bucle de
una gpu fn ES la tabla. Las llamadas y los bucles dentro de un hilo llegan con
el escritor de SPIR-V (G2).

**En la CPU** (D2, la regla de la casa): un f32 se GUARDA o se PASA a otra gpu
fn, y nada mas -- ni se suma, ni se compara, ni se imprime. Para usarlo, se
vuelve dec con `round`. Un f32 y un dec no se mezclan nunca solos.

**El permiso**: una `gpu fn` necesita que el Titan.toml pida `gpu` (U2), y cada
llamada a una gpu fn queda en el certificado del `.bex` con su linea: es lo que
`titan juez --concede gpu` y el kernel comparan.

### Las reglas, y quien las dice

```text
   LA GPU (gpu.rs)
     una gpu fn con print, tablas, textos, llamadas,      T0090
     bucles, valores mut / take, o sin resultado
     una gpu fn sin `gpu` en el Titan.toml                T0088
   EL CALCULO
     un f32 contado, comparado o impreso en la CPU        T0091
     un f32 y un dec juntos                               T0063
     tablas de distinto largo, o un dec donde va un f32   T0071
     sin tipo declarado
     `round` de un infinito o un NaN de la 3060           T0062
   LA ESCALERA
     `f64`: no tiene sitio (la 3060 cuenta en f32 y la    T0040
     CPU en dec)
```

[!] Hoy el calculo corre cada hilo en la CPU con f32 IEEE de precision simple
-- cada operacion redondeada una vez, como la 3060 --, y el `.bex` lleva los
resultados. Que lo escriba SPIR-V y lo juzgue el juez de spirv es G2; que los
calcule el oraculo de spirv, G3; que corra en la 3060, G4 (Ring 0).

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
| T0040 | lo que todavia no existe: una palabra de un nivel que no llego, o el tipo `f64` |
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
| T0066 | el programa sigue corriendo despues de un millon de pasos: un bucle sin salida (el calculo) |
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
| T0080 | se usa un modulo que no es hijo ni esta en la cabecera con `use` (el paquete) |
| T0081 | un `use` del que no se usa nada (el paquete) |
| T0082 | se llama a algo de otro modulo que no es `pub` (el paquete) |
| T0083 | `mod hijo` sin su fichero, o un fichero que dice otro nombre (el paquete) |
| T0084 | un ciclo entre modulos: las capas solo bajan (el paquete) |
| T0085 | un valor que no cumple el trait que pide un parametro: se dice en la llamada (el calculo) |
| T0086 | un `trait ... for` que no cumple su trait: le falta una fn, le sobra, o tiene otra firma (el comportamiento) |
| T0087 | un trait donde solo va un parametro entero: en un `let`, un campo, una tabla o un resultado (el comportamiento) |
| T0088 | se usa un nodo de BMO-X (`gpu`, `director`) que el Titan.toml no pide (el paquete, U2) |
| T0089 | un Titan.toml que no se lee: una linea que no es seccion, clave o comentario (el paquete) |
| T0090 | una `gpu fn` que no es una celda de la 3060: print, tablas, textos, llamadas, bucles, mut / take, o sin resultado (la gpu) |
| T0091 | un f32 contado, comparado o impreso en la CPU: alli se guarda o se pasa, y vuelve con `round` (el calculo, D2) |

## El banco: lo que dice cada ejemplo de si mismo

```text
   # espera: BIEN        compila...
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
   titan check hola.titan              bien, o el mensaje de 4 partes
   titan ir    hola.titan              la IR propia: lo que recibe el emisor
   titan build hola.titan -o hola.bex  el .bex, con su manifiesto, por el gate
   titan check flota/src/main.titan    un PAQUETE (nivel 9): sigue sus `mod`
                                       desde `flota/`, como F1
```

En la maquina: `run titan/hola.bex` en la consola (F12). `build.ps1` deja
`titan/hola.bex` y `titan/dos.bex` en el disco.
