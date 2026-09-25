# ESPEJO -- BMO C y C++ contra GCC y Clang

> **AUTO-GENERADO** por `toolchain/tools/espejo` (`cargo run --release -p bmo-espejo -- --informe`).
> No editar a mano.

Cada programa se compila y se EJECUTA por los dos lados --GCC y Clang en el
anfitrion, BMO en el emulador de x86-64-- y se compara lo que imprime y lo
que devuelve `main`. Si GCC y Clang no coinciden, el caso se aparta.

## casos de C: **10 de 19** iguales (52 %)

| caso | que paso |
|---|---|
| 01_aritmetica.c | no compila: linea 0: printf: '%X' aun no se compila (se compilan %d %i %u %x %c %s %%; los flotantes necesitan la ruta SSE) |
| 03_structs.c | no compila: linea 2610: 'suma' devuelve un struct por valor, y eso aun no se compila: pasa un puntero al destino como parametro |
| 04_uniones_bits.c | linea 3: se esperaba `corta 1` y BMO dio `corta 9` |
| 07_cadenas.c | no enlaza: NadieLoDefine { nombre: "strcat", usado_en: "07_cadenas.c" } |
| 14_estaticos.c | linea 1: se esperaba `3` y BMO dio `0` |
| 15_vkquake_capa2.c | no compila: linea 2609: la medida de un array tiene que ser una constante que se pueda calcular al compilar |
| 16_vkquake_capa3.c | no compila: linea 2612: un puntero a funcion que devuelve float/double aun no: su llamada leeria el resultado del registro equivocado (rax, no xmm0) |
| 18_switch_apilados.c | linea 1: se esperaba `r 2 s 30` y BMO dio `r 4 s 10` |
| 19_switch_antes_del_case.c | linea 1: se esperaba `7 5` y BMO dio `7 100` |

## casos de C++: **1 de 7** iguales (14 %)

| caso | que paso |
|---|---|
| 02_herencia.cpp | no compila: linea 2610: el destructor virtual (`virtual ~P()`): llega en el PASO 5. El orden completo esta en toolchain/lang/cpp/BRECHA.md |
| 03_plantillas.cpp | no compila: linea 2608: las plantillas: llega en el PASO 6. El orden completo esta en toolchain/lang/cpp/BRECHA.md |
| 04_referencias.cpp | no compila: linea 0: las referencias `T&`: llega en el PASO 2 - necesita la indireccion automatica en cada uso. El orden completo esta en toolchain/lang... |
| 05_new_delete.cpp | no compila: linea 2621: `new` de un tipo que no es una clase (`new int`): llega en el PASO 3. El orden completo esta en toolchain/lang/cpp/BRECHA.md |
| 06_operadores.cpp | no compila: linea 2609: se esperaba Semicolon y vino Comma |
| 07_cstdio.cpp | no compila: linea 3: #include: file not found: cstdio |

## azar C (200 desde 1): **197 de 200** iguales (98 %)

| caso | que paso |
|---|---|
| azar 4 | linea 4: se esperaba `g3 0` y BMO dio `g3 292322004` |
| azar 38 | linea 4: se esperaba `g3 67` y BMO dio `g3 6` |
| azar 176 | linea 4: se esperaba `g3 24979682` y BMO dio `g3 53418244` |

