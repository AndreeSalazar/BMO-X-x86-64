# EL MARCADOR -- TITAN++ (BMO-X) contra C, C++ y Rust

Lo escribe `python toolchain/tools/metro/versus.py --escribe`; no se toca
a mano. La vara: INSTRUCCIONES de x86-64 del trabajo (el metro de BMO-X
para TITAN++ y el C de la casa; valgrind para el resto, sin su arranque).
El numero es TITAN++ / el otro: por debajo de 1, TITAN++ hace MENOS.
GANA por debajo de 0,95; EMPATA hasta 1,05; pierde por encima.

Medido: 2026-10-16.

```text
duelo         TITAN++  C casa        gcc C         clang C       g++ C++       clang++ C++   rustc Rust    gcc Zen3      clang Zen3    
primos      3,503,401  GANA 0.74     pierde 1.17   GANA 0.95     pierde 1.17   GANA 0.95     GANA 0.72     pierde 1.19   EMPATA 0.96   
fib           773,985  GANA 0.75     pierde 1.26   pierde 1.23   pierde 1.26   pierde 1.23   pierde 1.38   pierde 1.29   pierde 1.23   
bytes       2,496,507  GANA 0.42     pierde 1.25   pierde 4.23   GANA 0.72     EMPATA 0.98   EMPATA 0.98   pierde 6.85   pierde 13.14  
ordenar     1,186,491  GANA 0.47     pierde 1.84   pierde 1.31   pierde 1.57   pierde 1.29   pierde 2.14   pierde 1.86   pierde 1.58   
```

**La cuenta: 8 gana, 3 empata y 21 pierde, de 32.**

[!] Lo que esto NO dice: los ciclos del Ryzen (una instruccion de SIMD
hace el trabajo de varias, y una division cuesta unas 40 veces una suma).
Eso lo mide el metal (AF10). Y TITAN++ mira desbordes e indices; C y C++
no miran nada, Rust mira los indices.

Lo que FALTA para ganar donde pierde esta en
`docs/plan/PLAN_TITAN_AFINADO.md`, seccion 2d (EL MARCADOR).
