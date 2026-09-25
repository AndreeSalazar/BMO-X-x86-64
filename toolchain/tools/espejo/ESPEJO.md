# ESPEJO -- BMO C y C++ contra GCC y Clang

> **AUTO-GENERADO** por `toolchain/tools/espejo` (`cargo run --release -p bmo-espejo -- --informe`).
> No editar a mano.

Cada programa se compila y se EJECUTA por los dos lados --GCC y Clang en el
anfitrion, BMO en el emulador de x86-64-- y se compara lo que imprime y lo
que devuelve `main`. Si GCC y Clang no coinciden, el caso se aparta.

## casos de C: **17 de 23** iguales (73 %)

| caso | que paso |
|---|---|
| 01_aritmetica.c | no compila: linea 0: printf: '%X' aun no se compila (se compilan %d %i %u %x %c %s %%; los flotantes necesitan la ruta SSE) |
| 03_structs.c | no compila: linea 2610: 'suma' devuelve un struct por valor, y eso aun no se compila: pasa un puntero al destino como parametro |
| 07_cadenas.c | no enlaza: NadieLoDefine { nombre: "strcat", usado_en: "07_cadenas.c" } |
| 15_vkquake_capa2.c | no compila: linea 2609: la medida de un array tiene que ser una constante que se pueda calcular al compilar |
| 16_vkquake_capa3.c | no compila: linea 2612: un puntero a funcion que devuelve float/double aun no: su llamada leeria el resultado del registro equivocado (rax, no xmm0) |
| 20_bits_empaquetados.c | linea 1: se esperaba `medida 4` y BMO dio `medida 16` |

## casos de C++: **1 de 7** iguales (14 %)

| caso | que paso |
|---|---|
| 02_herencia.cpp | no compila: linea 2610: el destructor virtual (`virtual ~P()`): llega en el PASO 5. El orden completo esta en toolchain/lang/cpp/BRECHA.md |
| 03_plantillas.cpp | no compila: linea 2608: las plantillas: llega en el PASO 6. El orden completo esta en toolchain/lang/cpp/BRECHA.md |
| 04_referencias.cpp | no compila: linea 0: las referencias `T&`: llega en el PASO 2 - necesita la indireccion automatica en cada uso. El orden completo esta en toolchain/lang... |
| 05_new_delete.cpp | no compila: linea 2621: `new` de un tipo que no es una clase (`new int`): llega en el PASO 3. El orden completo esta en toolchain/lang/cpp/BRECHA.md |
| 06_operadores.cpp | no compila: linea 2609: se esperaba Semicolon y vino Comma |
| 07_cstdio.cpp | no compila: linea 3: #include: file not found: cstdio |

## azar C (200 desde 1): **200 de 200** iguales (100 %)

