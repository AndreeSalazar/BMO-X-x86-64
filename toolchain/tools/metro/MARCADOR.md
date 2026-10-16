# EL MARCADOR -- TITAN++ (BMO-X) contra C, C++ y Rust

Lo escribe `python toolchain/tools/metro/versus.py --escribe`; no se toca
a mano. La vara: INSTRUCCIONES de x86-64 del trabajo (el metro de BMO-X
para TITAN++ y el C de la casa; valgrind para el resto, sin su arranque).
El numero es TITAN++ / el otro: por debajo de 1, TITAN++ hace MENOS.
GANA por debajo de 0,95; EMPATA hasta 1,05; pierde por encima.

Medido: 2026-10-16.

```text
duelo         TITAN++  C casa        gcc C         clang C       g++ C++       clang++ C++   rustc Rust    gcc Zen3      clang Zen3    
primos      4,108,215  GANA 0.86     pierde 1.38   pierde 1.11   pierde 1.38   pierde 1.11   GANA 0.84     pierde 1.39   pierde 1.13   
fib         1,031,889  EMPATA 1.00   pierde 1.69   pierde 1.64   pierde 1.69   pierde 1.64   pierde 1.83   pierde 1.72   pierde 1.64   
bytes       3,696,193  GANA 0.62     pierde 1.85   pierde 6.26   pierde 1.06   pierde 1.46   pierde 1.45   pierde 10.14  pierde 19.45  
ordenar     1,462,140  GANA 0.58     pierde 2.27   pierde 1.61   pierde 1.93   pierde 1.58   pierde 2.64   pierde 2.29   pierde 1.95   
```

**La cuenta: 4 gana, 1 empata y 27 pierde, de 32.**

[!] Lo que esto NO dice: los ciclos del Ryzen (una instruccion de SIMD
hace el trabajo de varias, y una division cuesta unas 40 veces una suma).
Eso lo mide el metal (AF10). Y TITAN++ mira desbordes e indices; C y C++
no miran nada, Rust mira los indices.

Lo que FALTA para ganar donde pierde esta en
`docs/plan/PLAN_TITAN_AFINADO.md`, seccion 2d (EL MARCADOR).
