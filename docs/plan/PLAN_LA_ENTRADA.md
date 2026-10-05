# PLAN LA ENTRADA -- TITAN++ lee de fuera, y una parte del programa corre de verdad

> Abierto el **2026-10-05**. El propietario: *"el titan ya funciona, pero que
> mas faltarian?"*, y de la lista que salio, *"empieza el paso 2"*.
>
> Es el **E1** de la escalera del emisor (`docs/maestro/TITAN_MAESTRO.md` 7.3),
> y la decision que 7.4 dejo escrita para este dia: *"el dia que algo venga de
> fuera, ese contrato se escribe, se mide en el metro y se decide con el
> propietario"*.

---

## 1. El limite de hoy, dicho entero

TITAN++ tiene los niveles 0 a 11 y compila, juzga y corre. Pero **todo programa
se sabe entero al compilar**: `calc.rs` lo CORRE en el anfitrion -- cada
llamada, cada vuelta de cada bucle -- y el `.bex` solo lleva lo que escribe
(`Module::flat`) y `EXIT`. Es el suelo E0, y es honesto: mientras nada venga
de fuera, no hay nada que calcular en la maquina.

```text
   lo que E0 NO puede hacer     preguntar algo y contestar segun lo que le
                                digan; leer un fichero; un juego; el ASISTENTE
                                (la primera app de la F2) -- nada interactivo
   lo que E0 tampoco hace bien  un bucle de mil millones de vueltas se calcula
                                ENTERO al compilar (el presupuesto STEPS lo
                                corta con T0066, en vez de emitirlo)
```

## 2. Lo que ya esta, y no se reinventa

| pieza | donde | que da |
|---|---|---|
| leer una linea de la consola | `toolchain/forge/bmo-lower/src/console.rs` (`read_line`) | la usan C y COBOL (`ACCEPT`); cede el turno si no hay nada |
| escribir un bufer | `console.rs` (`write_buffer`) | un texto que solo se sabe al correr |
| entero y decimal a texto | `toolchain/forge/bmo-lower/src/fmt.rs` (`write_i64`, `write_decimal_scaled`) | los digitos de un `int` y de un `dec(p, s)` |
| texto a decimal | `fmt.rs` (`parse_decimal_scaled`) | lo que se teclea, a `dec` |
| copiar, comparar bytes | `toolchain/forge/bmo-lower/src/memoria.rs` | textos de largo conocido |
| entrada en el emulador | `toolchain/forge/bmo-lower/src/emu/sistema.rs` (`poner_entrada`) | el banco puede TECLEAR por el programa |
| el esqueleto de bloques | `toolchain/lang/titan/emisor-x86_64/src/lib.rs` (`emit_blocks`) | `call`/`ret`/`jmp` con parches: se dejo para E1 |
| el permiso | `Permission::Input` en `platform/shared/titan-contrato/src/graph.rs` | `input = true` en `[permissions]` |

**La forja comparte contratos y librerias, nunca cerebros** (`toolchain/forge/README.md`):
lo de arriba no sabe que es TITAN++, igual que no sabe que es COBOL.

## 3. Las decisiones del propietario

**DECIDIDAS el 05-10, las tres (a):** `lee()` de libreria; el emisor propio
de TITAN++ con la forja; `numero(t)` da un caso que el `match` mira entero.

** Una correccion al hacerlo, dicha: D1 decia "pide `input = true`". El
contrato del certificado dice otra cosa (`Door::Console` en
`platform/shared/titan-contrato/src/certificate.rs`): la consola del PROPIO
programa no pide permiso, y es la misma puerta por la que escribe `print`.
`lee()` lee esa consola, asi que no pide permiso y el certificado la nombra
en su linea como consola. `input` queda para el teclado y el raton de una
ventana (REX), como siempre.

### D1 -- como entra lo de fuera en el lenguaje

```text
   (a) RECOMENDADA  una funcion de la libreria, como `print`: `lee()` da un
                    texto, la linea tecleada. No es palabra nueva: las 25
                    palabras y sus leyes no se tocan. Pide `input = true` en
                    el Titan.toml (U2): un programa que lee lo dice, y su .bex
                    lleva la puerta en el certificado
   (b)              una palabra nueva (`input`, `lee`): la palabra 26
```

### D2 -- quien escribe el codigo que corre

```text
   (a) RECOMENDADA  el EMISOR PROPIO de TITAN++ (7.1: "chico, correcto, sin
                    carrera") con las piezas de la forja, como C y COBOL. El
                    frontend sigue sin saber de maquinas (7.4): lo que viaja
                    al emisor es la IR, y la IR ya es el formato. INTI queda
                    para lo CALIENTE (E2: `.bo` + bmo-enlazar)
   (b)              un formato nuevo hacia INTI y que INTI lo baje todo: ata
                    TITAN++ a la IR de INTI, y los mensajes del juez de INTI
                    no hablarian del .titan
```

### D3 -- un texto que deberia ser numero, y no lo es

```text
   (a) RECOMENDADA  `numero(t)` da un CASO (nivel 8): `Es(n)` o `NoEs`, y el
                    `match` obliga a mirar los dos. Sin null, sin excepcion,
                    sin un 0 inventado -- como `pago` del nivel 8
   (b)              un error AL CORRER que para el programa
```

### D4 (no se pregunta: es la regla 1 de INTI) -- lo que solo falla al correr

Desbordar o dividir por cero con un valor tecleado **no se puede ver al
compilar**. Al correr, **atrapa**: escribe el NO con su linea del `.titan`
(`T0060`, `T0061`) y sale con codigo de error. Nunca da la vuelta en silencio.

## 4. La idea del compilador: lo que se sabe se calcula, lo que no, se emite

```text
   PROGRAMA SIN `lee()`     igual que hoy, E0: se corre al compilar entero
   PROGRAMA CON `lee()`     el calculo marca cada valor SABIDO o AL CORRER:
                              - un literal, o una cuenta de sabidos: sabido
                              - `lee()`, o una cuenta con algo al correr:
                                al correr
                            el juez (prestamos) y las CLASES siguen enteros al
                            compilar: lo que cambia es solo QUIEN calcula
```

## 5. Los escalones

- [x] R0 -- las decisiones D1-D3 del propietario, escritas aqui (seccion 3): las tres (a), el 05-10
- [x] R1 -- HECHO el 05-10 (sin permiso: ver la correccion de la seccion 3) -- `lee()` en el frontend: libreria (`toolchain/lang/titan/src/check.rs`), clase texto, pide `input` (`toolchain/lang/titan/src/gpu.rs` y el permiso como `use gpu`), sin `lee()` en una `gpu fn`; GRAMATICA y su codigo de error si falta el permiso
- [x] R2 -- HECHO el 05-10 (`Module::reads_outside` en `toolchain/lang/titan/src/ir.rs`; pruebas en `toolchain/lang/titan/tests/entrada.rs`) -- SABIDO o AL CORRER en `toolchain/lang/titan/src/calc.rs`: un programa con `lee()` no se corre al compilar; las clases y el juez si
- [x] R3 -- HECHO el 05-10 en `toolchain/lang/titan/emisor-x86_64/src/e1.rs` (pruebas en `toolchain/lang/titan/emisor-x86_64/tests/e1.rs`; un `print` calcula sus partes antes de escribir) -- el emisor E1 en `toolchain/lang/titan/emisor-x86_64/src/lib.rs`: textos (literal y tecleado), `print` mezclado, `int` en la pila con `jo` que atrapa, `if`, `while`, `for` -- el subconjunto de los niveles 0-4
- [x] R4 -- HECHO el 05-10, y el metro tambien teclea (`toolchain/tools/metro/src/main.rs`) -- el banco TECLEA: lineas `# entra:` en `toolchain/lang/titan/ejemplos/` que el banco del emisor (`toolchain/lang/titan/emisor-x86_64/tests/banco.rs`) da por `poner_entrada`, y compara las `# sale:`
- [x] R5 -- HECHO el 05-10: el NIVEL 12, "lo que viene de fuera" (0 palabras nuevas), con `toolchain/lang/titan/ejemplos/nivel12/pregunta.titan` y `hasta_fin.titan` en los dos bancos y en el metro (`toolchain/tools/metro/LINEA_BASE.txt`). `adivina` espera a `numero(t)` (R6)
- [ ] R6 -- `numero(t)` y su caso (D3), y `dec` al correr con `fmt.rs`
- [ ] R7 -- llamadas con valores al correr (nivel 5), tablas y registros (6), prestamos (7): el resto de la escalera, nivel a nivel
- [ ] R8 -- el PRESUPUESTO de plegado (7.3): un bucle sin `lee()` que pasa de STEPS se EMITE en vez de dar T0066
- [ ] R9 -- los papeles: `docs/maestro/TITAN_MAESTRO.md` 7.3 (E1 hecho), `toolchain/lang/titan/GRAMATICA.md`, y las leyes nuevas con `--sellar` del propietario (`toolchain/tools/titan-leyes/LEYES.txt`)

| se hace | si esta bien | si falla |
|---|---|---|
| `let nombre = lee()` / `print("hola ", nombre)` sin `input` | NO, con el permiso que falta | compila: el permiso no se mira |
| el mismo, con `input = true`, y el banco teclea `Ada` | sale `hola Ada` en el emulador | sale `hola ` o nada: el bufer no llego al `print` |
| `hola.titan` (sin `lee()`) | el MISMO `.bex` que hoy, byte a byte (el metro) | cambio: E1 se colo donde no hacia falta |
| teclear un numero que desborda en una suma | sale el NO T0060 con su linea, codigo de error | da la vuelta en silencio |

## 6. Lo que este plan NO es

- **No es el compilador dentro de BMO-X** (T6, `docs/plan/en_pausa/PLAN_AUTOHOSPEDAJE.md`):
  esto sigue compilando en el anfitrion; lo que cambia es lo que el `.bex` hace.
- **No es la ventana ni el raton** para TITAN++: eso es REX y viene despues.
  La primera entrada es la consola, porque la forja ya la sabe leer.
- **No toca INTI.** Si D2 sale (b), este plan se reescribe antes de R3.
