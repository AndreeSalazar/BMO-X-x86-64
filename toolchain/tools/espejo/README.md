# ESPEJO -- BMO C y BMO C++ contra GCC y Clang

El propietario (25-09): *"una app que verifique mi C y C++ por completo, para
tomar la base y compararse con C y C++ de terceros en tiempo real, para
autocompletar y ahorrar el esfuerzo"*.

`toolchain/lang/c/BRECHA.md` pregunta **compila?** con sondas. ESPEJO pregunta lo
siguiente: **hace lo mismo?** El mismo programa se compila y se EJECUTA por los
dos lados --GCC y Clang en el anfitrion, BMO en el emulador de x86-64
(`bmo_lower::emu`)-- y se compara lo que imprime, linea a linea.

## Uso

```text
cargo run --release -p bmo-espejo --                 casos de C y C++ y 200 al azar
cargo run --release -p bmo-espejo -- casos [c|cpp]   los casos de casos/
cargo run --release -p bmo-espejo -- uno f.c         un programa, con lo que imprimio BMO
cargo run --release -p bmo-espejo -- azar 500 [--desde S] [--cpp]
cargo run --release -p bmo-espejo -- reducir f.c     un fallo, reducido a lo minimo
cargo run --release -p bmo-espejo -- corpus <dir>    el C de un juego, pelado capa a capa
cargo run --release -p bmo-espejo -- vigilar casos c EN TIEMPO REAL: al cambiar el
                                                     compilador, vuelve a mirar y dice
                                                     que se ARREGLO y que se ROMPIO
... --informe                                        ademas escribe ESPEJO.md
```

## Las reglas que lo hacen justo

| Regla | Por que |
|---|---|
| GCC y Clang tienen que COINCIDIR; si no, el caso se aparta | Si dos compiladores serios no dan lo mismo, el programa depende de algo que el estandar no fija (el orden de los argumentos de una llamada, por ejemplo). Pedirle a BMO una de las dos no es justo. |
| Se compara lo IMPRESO, no lo que devuelve `main` | En BMO-X `EXIT` no lleva codigo (`bmo_lower::task::exit`): ese numero no llega a ningun sitio. |
| Los programas al azar no tienen comportamiento indefinido | Aritmetica sin signo con cast en cada operando, desplazamientos enmascarados, divisores `| 1`, indices `& 7`, y las funciones solo escriben sus locales. Ver `src/azar.rs`. |
| El oraculo compila con `-Werror=return-type` | El reductor fabrica funciones sin `return` al quitar lineas; eso es indefinido y se rechaza. |
| El reductor no quita etiquetas `case`/`default` | Sin ellas aparece OTRO fallo (codigo antes del primer `case`) que tapa el que se busca. |
| Un fallo busca el MISMO fallo al reducirse | Si era "g3 sale distinto", sigue siendo g3. |

## Lo que encontro, y lo que ya se arreglo (25-09)

Arreglados el mismo dia, con `espejo vigilar` abierto diciendo `+ ARREGLADO`
uno a uno, y fijados tambien como pruebas de Rust en
`toolchain/lang/c/emisor-x86_64/src/tests/silencios.rs` (para una maquina sin
GCC):

| fallo | donde estaba | caso |
|---|---|---|
| etiquetas apiladas de `switch` | el parser solo guardaba un caso con cuerpo | `18` |
| codigo antes del primer `case` | se guardaba con la marca del `default` | `19` |
| `++k` de una `static` local | el atajo del `++` no pasaba por el alias `funcion.k`, y el codegen ponia un CERO CALLADO | `14` |
| escribir en un nombre que no existe | `emit_store_var` no tenia `else`: la escritura se perdia | prueba de Rust |
| campo de bits que no corta | el ancho se leia y se tiraba | `04` |
| `c ? a : b` con ramas de distinto ancho | el tipo era el de la primera rama | `21` |
| acarreo de 32 bits bajo un `&`/`|`/`^` | esos no tenian tipo, y la suma de encima tampoco | `22` |
| constante sin signo plegada | salia como inmediato extendido con signo | `23` |

Con eso: **1000 de 1000** programas al azar iguales, y el metro sin una
salida cambiada (+241 instrucciones en 30 programas: el precio de los
recortes que faltaban).

Queda, dicho: la DISPOSICION de los campos de bits (`20`, no se empaquetan),
`%X`, devolver un struct por valor, `strcat`, las capas 2 y 3 de vkQuake, y
que el lexer acepta un salto de linea dentro de una cadena.

## Lo que encontro el primer dia (25-09)

- **Etiquetas apiladas** (`case 0: case 1: case 2:`): BMO mandaba el 1 y el 2 al
  `default`. Salio de un programa al azar, reducido. Quake y DOOM lo usan en
  cada tabla. -> `casos/c/18_switch_apilados.c`
- **Codigo antes del primer `case`**: BMO lo ejecutaba. -> `19_switch_antes_del_case.c`
- **Campos de bits**: `f.a = 9` en 3 bits guarda 9, no 1. -> `04_uniones_bits.c`
- **`static` local**: `static int n = 0; return ++n;` devuelve 0 siempre. -> `14_estaticos.c`
- Lo que ya se sabia que faltaba, ahora con su caso: `%X`, devolver un struct
  por valor, `strcat`, `sizeof` en la medida de un array y el puntero a funcion
  que devuelve `float` (las capas 2 y 3 de vkQuake 0.50).
- **C++**: `<cstdio>`, y un literal `ull` por encima del maximo con signo
  (*"entero fuera de rango"*), que el frontend de C si acepta.
- **El ABI**: `sizeof(long) == 8`. BMO es LP64, como System V: coincide con Linux.

Los fallos al azar se guardan en `target/espejo/fallos/` con su semilla.
