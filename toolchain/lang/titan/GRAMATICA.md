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
del frontend de hoy, que va por el nivel 1):

```text
   let mut x = 1    T0040  `mut` llega en el nivel 2 (contar)
   while ...        T0040  `while` llega en el nivel 4 (repetir)
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

### Los codigos

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
| T0053 | las llamadas vuelven a una funcion y no terminan nunca (sin `if`, nada las para) |
| T0054 | un nombre se lee antes de que un `let` le de valor (el juez) |
| T0055 | un nombre que ya tiene valor, o que es de una funcion (el juez / los nombres) |
| T0056 | se cambia un valor sin `mut` (el juez) |
| T0060 | un numero que no cabe en 64 bits: desbordar es un error (el calculo) |
| T0061 | una division o un resto entre cero (el calculo) |
| T0062 | una division que no da un numero entero (el calculo) |
| T0063 | un texto con un numero: no se suman ni se convierten solos (el calculo) |

### El banco: lo que dice cada ejemplo de si mismo

```text
   # espera: BIEN        compila...
   # sale: hola          ...y al CORRER escribe exactamente esto, linea a linea
   # espera: T0053       NO compila, con este codigo, y no escribe ningun .bex
```

Las lineas `# sale:` las comprueba el banco del emisor
(`emisor-x86_64/tests/banco.rs`): construye el `.bex`, lo carga como el
cargador del kernel y lo CORRE en el emulador. Un nivel esta hecho cuando sus
programas HACEN lo que dicen, no cuando compilan.

### Del texto al `.bex` (T3, 2026-10-04)

```text
   titan check hola.titan              bien, o el mensaje de 4 partes
   titan ir    hola.titan              la IR propia: lo que recibe el emisor
   titan build hola.titan -o hola.bex  el .bex, con su manifiesto, por el gate
```

En la maquina: `run titan/hola.bex` en la consola (F12). `build.ps1` deja
`titan/hola.bex` y `titan/dos.bex` en el disco.
