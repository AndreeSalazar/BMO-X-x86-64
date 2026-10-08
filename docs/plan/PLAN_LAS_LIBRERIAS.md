# PLAN LAS LIBRERIAS -- TITAN++, el modelo general; cada GPU, una libreria (la primera: SM86, la RTX 3060 12G)

> Abierto el **2026-10-08**. El propietario, al empezar la sesion: *"ponte
> al dia con eso en git commit y eso para empezar a estudiar por completo el
> objetivo con TITAN++ con VERRANO"*; y al leer lo primero: *"claro dime el
> plan para iniciar y asi poder construir el TITAN++ como el nuevo modelo de
> programacion en general pero que tenga librerias de SM86 en RTX 3060 12G
> TODAS LAS GPU son librerias"*.
>
> **Esto es el ESTUDIO y el ORDEN, nada mas**: con este plan no cambia ni una
> linea de codigo, ni una ley, ni una casilla de otro plan. Lo de antes no se
> repite, se nombra: la API en [`PLAN_VERRANO.md`](PLAN_VERRANO.md); el
> aislamiento por tarjeta en [`PLAN_EL_AISLAMIENTO.md`](PLAN_EL_AISLAMIENTO.md);
> la lengua de la 3060 y su juez en
> [`PLAN_LA_LENGUA_DE_LA_3060.md`](PLAN_LA_LENGUA_DE_LA_3060.md); el nivel 11 en
> [`PLAN_EL_CENTAURO.md`](PLAN_EL_CENTAURO.md); la escuela en
> [`PLAN_ILLAPA.md`](PLAN_ILLAPA.md); el examen en
> [`PLAN_EL_LIBRETO.md`](PLAN_EL_LIBRETO.md); el porque de TITAN++ en
> [`TITAN_MAESTRO.md`](../maestro/TITAN_MAESTRO.md).

---

# 0. LA RESPUESTA CORTA

```text
   el modelo     TITAN++ es el lenguaje con el que se CONSTRUYE en BMO-X: la
                 CPU por su emisor (E1) y por INTI, los aparatos por los
                 permisos del Titan.toml, y la GPU por sus LIBRERIAS
   la libreria   lo que UNA tarjeta sabe hacer con el Programa de la casa:
                 saber, emitir, juzgar, simular y entregar. TITAN++ no nombra
                 ninguna tarjeta: pide las librerias que haya
   la primera    SM86, la RTX 3060 12G: casi entera HOY, repartida en cinco
                 crates (seccion 1)
   la segunda    la CPU misma (`nativo`, el x86-64 del Programa): la RESERVA,
                 y la prueba de que el contrato vale para dos sin comprar otra
                 tarjeta
   la tercera    una AMD, o un DIALECTO nuevo de NVIDIA: cuando lo haya, sin
                 tocar TITAN++ ni la libreria de SM86
```

**Donde esta hoy (medido el 08-10, seccion 2):** la cadena de la 3060 esta
entera, pero solo AL COMPILAR: `gpu fn` -> el Programa de la casa -> SM86 ->
el juez ESTRICTO -> la 3060 simulada, con 467 pruebas en verde. **Al CORRER,
TITAN++ todavia no le dice nada a la GPU ni a VERRANO.**

**Por donde se empieza (seccion 6):** que `titan check` y `titan build` digan
lo mismo; un arranque para que TITAN++ deje de ser amarillo; el CONTRATO de
una libreria con la de SM86 dentro; y la libreria de la CPU, que es la que
deja correr una `gpu fn` en un programa de verdad SIN esperar a Ring 0.
Despues: los bucles, la `gpu fn` que dibuja y TITAN++ mandando a VERRANO.

## El modelo general, y que parte es de este plan

```text
   la CPU        E0 (todo al compilar) y E1 (el programa entero corre):
                 HECHOS, niveles 0-13 (GRAMATICA). E2, llamar a INTI por .bo
                 + bmo-enlazar: TITAN_MAESTRO 7.3
   los aparatos  el byte, la ventana, la entrada y el disco: TA1-TA4 de
                 PLAN_LA_TINTA (ILLAPA pide los mismos: se hacen UNA vez)
   los datos     las listas y los mapas (nivel 13), ESTRATOS como base de
                 datos: PLAN_LISTAS_Y_MAPAS, PLAN_LOS_DATOS
   la GPU        ESTE plan: las librerias, y TITAN++ mandando a VERRANO
```

---

# 1. QUE ES UNA LIBRERIA

## 1.1 Una palabra, dos cosas que no se mezclan

```text
   la BIBLIOTECA de TITAN++   las funciones del lenguaje: print, lee, round,
                              push, get... (GRAMATICA). No sabe de maquinas
   una LIBRERIA de GPU        lo que una tarjeta sabe hacer con el Programa de
                              la casa. TITAN++ no la nombra: la PIDE
```

Se usa la palabra del propietario, *libreria*, justo para que no se confunda
con la biblioteca del lenguaje. Y no es nueva: PLAN_ILLAPA ya llama al juez
del SASS *"la libreria de lo que sabe la 3060 (una AMD tendria SU
libreria)"*.

## 1.2 El contrato: cinco piezas, y donde estan las de SM86 HOY

| pieza | lo que da | la de SM86, hoy |
|---|---|---|
| 1 SABE | que operaciones del Programa emite, cuales NO (con su motivo) y sus techos: registros, instrucciones | `NoEmite` de `platform/shared/proton-x-sm86`; los techos en el juez (64 registros, 128 instrucciones) y lo que la tarjeta no dice en `PERFIL/GPU_3060.txt` |
| 2 EMITE | Programa -> su codigo, con su ABI | `proton-x-sm86` (E3..E6) y `platform/shared/bmo-sm86` (los 128 bits) |
| 3 JUZGA | SI o NO antes de que la tarjeta vea un bit; `no_std`, porque lo usan el build y el kernel | `platform/drivers/gpu/ga10x/src/sass/juez.rs` (R1..R9; R7 = el cuerpo de una app) |
| 4 SIMULA | su codigo en el anfitrion, con los bits de la casa: el ORACULO | `proton-x-sm86/src/simula.rs` |
| 5 ENTREGA | su objetivo en el BSF (`kind`, `abi`) y su PUERTA hasta la tarjeta | `bmo-bsf`: kind 2, abi `SM86_V1` y `SM86_PUERTA_V1`; la puerta estrecha (`IOMMU_OP_GPU_DIBUJAR`, la receta VRN2) y `CUBO_VERRANO` |

**SM86 ya es una libreria de hecho**: las cinco piezas existen y el metal las
vio (BMOX-12 IGUAL a D3D12 el 28-09, con lo que emitio la casa: E5). Lo que
no existe es el CONTRATO escrito: hoy `bmo-titan-sm86` nombra a mano
`bmo-proton-x-sm86` y `bmo-gpu-ga10x`, y el Programa vive dentro de PROTON-X
(`platform/shared/proton-x/src/dxil/programa.rs`), la capa de Windows.

## 1.3 Las reglas de una libreria

```text
   L-a  TITAN++ no nombra una tarjeta: pide "las librerias". Como hoy
        `ir.rs` falla si la IR nombra un registro (TITAN_MAESTRO 7.4)
   L-b  una libreria no comparte su emisor, su juez ni su puerta con otra:
        EL_AISLAMIENTO, con su guardian (`la-3060`, regla S)
   L-c  la ENTRADA comun es el Programa de la casa, no SPIR-V (E3 el 28-09,
        LI7 el 07-10). [!] EL_AISLAMIENTO todavia dice SPIR-V: es de antes
        de E3
   L-d  una libreria que no sabe una operacion dice NO con su motivo; la gpu
        fn va entonces a OTRA (la CPU de reserva) o no hay .bex. Nunca se
        traduce a medias
   L-e  el juez de cada libreria vive con SU tarjeta y es `no_std`: lo usan
        el build y el kernel (J2 de LA_LENGUA)
   L-f  los mismos bits: cada libreria, en su simulador, da los de la casa.
        Si una tarjeta aproxima (el MUFU de la 3060), eso es una DECISION
        (DL10), nunca un descuido
```

---

# 2. LO QUE HAY, medido el 2026-10-08

## 2.1 La cadena, eslabon a eslabon

| eslabon | donde | estado |
|---|---|---|
| la `gpu fn` (nivel 11) | `toolchain/lang/titan/src/gpu.rs` | anfitrion (G1, 04-10) |
| gpu fn -> Programa | `toolchain/lang/titan/emisor-sm86` (`bmo-titan-sm86`) | anfitrion, en LINEA RECTA (LI7, 07-10) |
| Programa -> SASS de SM86 | `platform/shared/proton-x-sm86`, `platform/shared/bmo-sm86` | anfitrion E3..E6; METAL E5 (28-09) |
| el juez del SASS | `platform/drivers/gpu/ga10x/src/sass/juez.rs` | en el build y en el kernel (J1, J2) |
| la 3060 simulada | `platform/shared/proton-x-sm86/src/simula.rs` | el oraculo de cada `titan build` (07-10) |
| VERRANO V0..V1c | `platform/shared/verrano` y el escritorio | METAL (26-09: IGUAL a D3D12; `maximo` 28.596 fps) |
| la puerta estrecha (VRN2) | `IOMMU_OP_GPU_DIBUJAR`, `gpu_trabajo/cubo.rs` | METAL, con los lotes de PROTON-X |
| el x86-64 del Programa | `platform/shared/proton-x/src/nativo.rs` | Ring 3; lo usa PROTON-X, TITAN++ no |
| computo de UNA APP en la 3060 | -- | NO (la QMD si: 24-09) |
| una gpu fn AL CORRER | `toolchain/lang/titan/emisor-x86_64/src/e1/mod.rs` | NO: la rechaza |
| TITAN++ en el Ryzen | `run titan/hola.bex` | NUNCA: amarillo (README) |

## 2.2 Las pruebas

```text
   cargo test -p bmo-titan-front -p bmo-titan-sm86 -p bmo-titan-x86-64 \
              -p bmo-proton-x-sm86 -p bmo-gpu-ga10x -p bmo-verrano

   bmo-titan-front     70   el lenguaje y sus bancos
   bmo-titan-sm86      10   LI7: 9 del crate y el banco del nivel 11
   bmo-titan-x86-64    27   el emisor de la CPU (E0, E1) y su banco
   bmo-proton-x-sm86   34   el emisor de la 3060 y el .bsf vivo
   bmo-gpu-ga10x      318   el driver de la 3060 en el anfitrion: el juez,
                            el pegamento, la receta, el BSF del cubo
   bmo-verrano          8   la API, el backend CPU y la lamina
                      ---
                      467   en verde, 0 en rojo
```

Y `titan sm86` sobre los dos ejemplos del nivel 11 (`mezcla`, `activa`):

```text
   mezcla   (a + b) / 2.0       3 instrucciones, 7 registros   el juez: si
   suma     a + b               2 instrucciones, 6 registros   el juez: si
   activa   if x > 0.0 ...      8 instrucciones, 8 registros   el juez: si
```

(`/ 2.0` pasa porque es una multiplicacion EXACTA por 0,5: `inverso_exacto`.)

## 2.3 Tres sondas, escritas fuera del arbol

Tres paquetes con `gpu = "compute"` en su Titan.toml, para preguntar:

```text
   la sonda                    titan check   titan build
   ---------------------------------------------------------------------------
   un `for` en una gpu fn      T0090         T0090: "Los bucles dentro de un
                                             hilo llegan con el escritor de la
                                             3060 (IL1)"
   `x / 3.0` en una gpu fn     bien          T0090 en la LLAMADA: "es un fallo
                                             del escritor de la 3060 ... no del
                                             programa" -- "avisa con este
                                             programa"
   `lee()` y una gpu fn        bien          "el .bex no paso el gate (... no
                                             se emite al correr todavia ... G4
                                             ...): es un fallo del compilador"
```

La primera es justa y dice por que. Las otras dos dicen algo que no es
(H1 y H2, en 3.6).

---

# 3. LO QUE FALTA ENTRE TITAN++ Y VERRANO: cinco distancias, y lo que se encontro

## 3.1 LA VOZ: TITAN++ no tiene con que hablarle a VERRANO

- Hoy la lamina de VERRANO (`BVER`, dos ranuras con su sello:
  `platform/shared/verrano/src/lamina.rs`) la escribe INTI:
  `toolchain/forge/sem-asm/tables/lang/inti/runtime/verrano.inti`
  (`verrano_lamina`, `verrano_empieza`, `verrano_acaba`) y la app
  `toolchain/lang/inti/ejemplos/cubo.inti`.
- TITAN_MAESTRO (7.2 y el E4 de 7.3) ya dice que eso pasa a TITAN++, y
  [`PLAN_INTI_SAMURAI.md`](PLAN_INTI_SAMURAI.md) (3.1) espera ese relevo para su
  corte 4: el cubo sale de INTI *"con su relevo en TITAN++"*.
- En TITAN++ no hay nada de eso: ni biblioteca, ni permiso de dibujar, ni
  forma de pedir y ofrecer un bloque. (El certificado ya tiene la puerta
  `Door::Screen`, en `platform/shared/titan-contrato/src/certificate.rs`;
  nadie la usa.)
- [!] Y la lamina de INTI **nunca se vio en el metal**: el cuarto paso de E6
  de PLAN_VERRANO dice "falta el metal" (`run inti/cubo.ibx` y `gpu verrano
  banco inti`).
- Hay un segundo camino, ya visto en el metal: la PUERTA ESTRECHA
  (`IOMMU_OP_GPU_DIBUJAR`, la receta VRN2), *"la unica orden de la 3060 que
  no pide la autoridad MAQUINA"* (`gpu_trabajo/cubo.rs:234`). Cualquier app
  manda CUERPOS juzgados (R7) y sus DATOS; el kernel pone las lecturas y el
  pegamento, y el dibujo cae en SU RAM. Es la de PROTON-X, y la que VERRANO
  va a unificar (VC2).
- De quien: del anfitrion, las dos. Ninguna pide Ring 0.

## 3.2 EL TIEMPO: una gpu fn solo corre AL COMPILAR

- `titan build` corre cada gpu fn en la 3060 simulada y el `.bex` lleva las
  CELDAS, no el programa (GRAMATICA, nivel 11).
- Un programa que corre de verdad (E1) rechaza la gpu fn y cualquier `f32`:
  `e1/mod.rs:468` y `:608` (la fn y su llamada), `e1/valor.rs:41` (un f32),
  `e1/numero.rs:185` (su `round`) y `e1/escribe.rs:126` (escribirlo). Es la
  regla D2 (la ley L26) mas la falta de G4.
- **Una correccion que cambia la medida de lo que falta.** G4 de
  [`PLAN_EL_CENTAURO.md`](PLAN_EL_CENTAURO.md) dice "LANZAR computo: la QMD y el
  banco constante 0", y
  [`COMO_LE_HABLA_NVIDIA.md`](../../platform/drivers/gpu/ga10x/COMO_LE_HABLA_NVIDIA.md)
  (3d) dice "aun no lanza un kernel (no hay QMD)". **La QMD existe**
  (`platform/drivers/gpu/ga10x/src/motores/sombreador.rs`: `qmd_con`,
  `qmd_rejilla`) y corre en el metal desde el **24-09**: el primer sombreador
  a las 16:21 (32 de 32), el blur y el fractal a las 16:56
  ([`METAL_2026-09-25.md`](../metal/METAL_2026-09-25.md), seccion 1); despues
  `giro`, `pantalla`, `video` e `imagen`. Lo que falta es la **PUERTA para el
  cuerpo de una APP**: la de dibujar existe (DIBUJAR), la de computar no. Y es
  la MISMA pieza que **LI2f** de EL_LIBRETO (el computo del juego): una vez,
  para TITAN++ y para Cyberpunk.
- De quien: esa puerta es Ring 0, del propietario, y pide antes E7 visto en
  el metal (`gpu eterno ya`): con bucles, un cuerpo puede no volver.
- [!] Lo que NO pide Ring 0: correr la gpu fn en la CPU, con la libreria
  x86-64 (LB4). El `nativo` de PROTON-X ya hace *"un sombreador, traducido
  UNA vez a x86-64 con SSE"*, con el interprete de la casa como juez bit a
  bit.

## 3.3 LA FORMA: una gpu fn es computo de UNA celda

- La firma (`gpu.rs:52-69`, ley L27): valores `f32` o `bool`, copias, y UN
  resultado.
- Un programa de VERRANO tiene otra forma: el de vertice lee sus elementos y
  deja la posicion y los genericos; el de pixel lee los genericos y deja el
  color. Es lo que piden `proton-x-sm86/src/pso.rs` (`traducir`) y el
  pegamento (`pegamento::vertice_con_libreta_en`, `pixel_con_libreta_en`):
  VARIAS salidas, y entradas por atributo.
- El Programa ya lo sabe decir (`Entrada` y `Salida` por elemento y
  componente, `programa.rs:178-180`): es lo que la 3060 dibujo IGUAL a D3D12
  con BMOX-12 el 28-09.
- A TITAN++ le falta una gpu fn que reciba y devuelva un REGISTRO de `f32`
  (posicion, color) y que se sepa de vertice o de pixel. Hoy un registro
  dentro de una gpu fn es T0090 (`gpu.rs:148`). Con TIPOS, no con palabras:
  los tipos no gastan techo, y las palabras van 25 de 30 (TITAN_MAESTRO 4.4).

## 3.4 LA MEMORIA: bucles y vecinos (IL1)

**Los bucles** los sabe ya el emisor de la 3060 (E6, 02-10): `Si`, `SiNo`,
`FinSi`, `Bucle`, `RomperSi`, `Romper`, `Continuar` y `FinBucle`
(`proton-x-sm86/src/lib.rs:661-701`), con el planificador que drena antes de
cada salto, el simulador con su tope (`SinFin`) y el juez con R8 y R9;
programas al azar (4.000 una vez) iguales a la casa. Lo que falta es SOLO de
TITAN++:

```text
   el frontend   los niega: T0090 (`gpu.rs:91-92`)
   el escritor   los niega: "un salto hacia arriba"
                 (`emisor-sm86/src/lib.rs:357-365`), porque escribe en LINEA
                 RECTA (cada `if` es un Elige). Con bucles tiene que escribir
                 el Programa ESTRUCTURADO
   el calculo    ya los corre (es el de la CPU), con su presupuesto de pasos
                 por celda (`calc.rs:342`)
   las llamadas  entre gpu fn llegan aqui tambien (hoy T0090): una gpu fn es
                 pura, asi que ponerla EN LINEA da los mismos bits
```

[!] **El contador.** Dentro de una gpu fn todo numero es `f32` (GRAMATICA,
nivel 11): `for i in range(N)` con `i` en f32 es exacto hasta 2^24 vueltas.
Para INDICES de verdad (los vecinos, la rejilla de IL2) hace falta el ENTERO,
y ahi choca una ley: en TITAN++ desbordar ATRAPA (T0060), y la 3060 da la
vuelta en 32 bits (DL5).

**Los vecinos** no son trabajo del emisor solo:

- el emisor niega hoy toda lectura de memoria de un programa (`IdHilo`,
  `LeeUav`, `EscribeUav`, la memoria compartida y los atomicos:
  `proton-x-sm86/src/lib.rs:849-853`);
- y aunque las supiera, **el juez no las dejaria pasar**: R7, la lista blanca
  del cuerpo de una app (`juez.rs:456-489`), dice *"Nada de LDG/STG/ALD/AST/IPA,
  ni un banco de constantes, ni un salto fuera"*. Es lo que hace segura la
  puerta estrecha: las direcciones las decide el kernel;
- el camino que ya existe para leer memoria sin darle una direccion a la app
  es el ASA (`juez.rs:493-499`, R7 con texturas): un `TEX` solo con un asa que
  puso el kernel. Un vecino leido asi queda acotado POR CONSTRUCCION (fuera de
  rango, 0: lo de D3D con un bufer tipado, y lo que el Programa ya hace con
  la memoria compartida, `programa.rs:222-223`).

O sea: toca una regla del juez que vive en el kernel, Ring 0, del propietario
(DL9). Y es la misma pieza que LI2f (los UAV) y LI2c (las texturas) de
EL_LIBRETO.

## 3.5 LOS BITS: que cuenta como "el mismo resultado"

- **La division general**: el escritor de TITAN++ la rechaza
  (`emisor-sm86/src/lib.rs:41-45` y `:413`) y PROTON-X la manda a la CPU
  (`proton-x-sm86/src/lib.rs:843`). Solo pasa entre una potencia de dos.
- **La raiz** (`Sqrt`, `Rsqrt`) si se emite, como MUFU, y el simulador la hace
  COMO LA CASA: *"la 3060 da una APROXIMACION de un ULP o dos"*
  (`simula.rs:20-22`). Hoy no muerde a TITAN++ (una gpu fn no llama a nada);
  el dia que pueda, la ley L29 -- *"los resultados de una gpu fn los da el
  oraculo ... y son los mismos que cuentan la casa y el calculo"* -- dejaria
  de ser verdad en la 3060 de verdad.
- **El seno y el coseno** (`Op::Mate`) no se emiten (`lib.rs:855`), y en la
  CPU TITAN++ no cuenta en f32 (D2). Y se ve con el cubo: `bmo_cubo` reduce el
  angulo con `x / DOS_PI` y saca la raiz por Newton, dividiendo
  (`platform/shared/bmo-cubo/src/num.rs`). O sea: **sin DL10, TITAN++ no
  puede contar el cubo de VERRANO bit a bit**, ni en la CPU (D2) ni en la
  3060 (la division).
- Es LI2g de EL_LIBRETO y D5 de ILLAPA, que EL_LIBRETO ya dijo que son la
  misma decision (seccion 8). **Y tiene una tercera salida que no esta
  escrita**: los bits exactos TAMBIEN en la 3060, con la secuencia de
  redondeo correcto que `ptxas` pone para `div.rn.f32` y `sqrt.rn.f32` (el
  MUFU aproxima, y los pasos de Newton con su correccion dejan el IEEE). La
  casa ya hizo lo mismo con la division de ENTEROS (E6d): copio la cuenta de
  `ptxas` y probo que aguanta el error del MUFU moviendo el inverso de -64 a
  +1 ULP en 50.000 pares. Costaria instrucciones, no exactitud; y su camino
  lento para los bordes tendria que ir DENTRO del cuerpo (R7 no deja salir).
  Hay que sacarlo de `ptxas` (el oro) antes de afirmarlo.

## 3.6 Lo que se encontro al estudiar

```text
   H1  check y build no dicen lo mismo con `x / 3.0`, y el NO culpa al
       compilador de un limite que la GRAMATICA ya documenta
   H2  lo mismo con `lee()` y una gpu fn: el "todavia no" de E1 sale como
       "el .bex no paso el gate ... fallo del compilador"
   H3  `titan sm86 -o CARPETA` no crea la carpeta
   H4  textos que dicen algo que ya no es
   H5  TITAN++ nunca corrio en el Ryzen
```

- **H1.** `titan check` dice `bien`; `titan build` dice T0090 en la LLAMADA
  (no en la division), con *"es un fallo del escritor de la 3060, de su juez o
  del oraculo, no del programa"* y *"avisa con este programa"*
  (`toolchain/lang/titan/src/calc.rs:576-581`). La GRAMATICA dice que esa
  division es un limite de hoy, a proposito (LI2g). El NO tendria que salir en
  `check`, en la linea y la columna de la division, con su COMO (una potencia
  de dos, o esperar a DL10). Que el divisor sea una potencia de dos es una
  propiedad del NUMERO, no de una tarjeta: el frontend lo puede mirar sin
  nombrar ninguna.
- **H2.** `titan check` dice `bien`; `titan build`: *"el .bex no paso el gate
  (... no se emite al correr todavia ... G4 de PLAN_EL_CENTAURO): es un fallo
  del compilador"* (`toolchain/lang/titan/emisor-x86_64/src/main.rs:96`). El
  "todavia no" de E1 (`e1/mod.rs:174`, `later`) es un limite documentado, no
  un fallo. Y una nota: la gpu fn de la sonda solo toca valores que se saben
  al compilar; cae en E1 porque el plegado es TODO o NADA (`calc.rs:286`).
- **H3.** *"no pude escribir .../mezcla.sass: No such file or directory"*
  (`emisor-x86_64/src/main.rs:122-126`). Viene de cuando era `titan spirv`.
- **H4.** Lo que dice algo que ya no es:
  - **el camino SPIR-V de la gpu fn, despues de LI7 (07-10):** PLAN_ILLAPA
    (la primera fila de la tabla de la seccion 4, la entrada de la seccion 6
    e IL1); PLAN_LA_TINTA (la fila "la 3060 para la fusion", la seccion de la
    fusion y TD1); PLAN_EL_CENTAURO (el dibujo de la seccion 0, D5 y la
    seccion 2); TITAN_MAESTRO (6b, 7.2, 7.3, 7.4, 8, el T5 de 12 y de 14.5, y
    la entrada del nivel 11); y PLAN_EL_AISLAMIENTO (la seccion 0 y "Lo que NO
    es esto": la entrada comun era SPIR-V antes de E3);
  - **"aun no lanza un kernel (no hay QMD)"**: COMO_LE_HABLA_NVIDIA, 3d
    (escrito el 04-10); la QMD corre desde el 24-09;
  - **el README**, fila "Games on the GPU, step one" de *What is next*: dice
    *"next: V1, the cube in motion with fps; then SPIR-V to SM86 by BMO-X's own
    compiler"*. V1, V1b y V1c se vieron el 26-09, y el camino a SM86 es el
    Programa de la casa desde el 28-09 (E3; en el metal, E5).
- **H5.** README, *What is next*: *"TITAN++ on the metal -- 14 levels; its
  bank of 87 programs passes in the emulator"*, bloqueado por *"`run
  titan/hola.bex` from F12"*. Todo TITAN++ es amarillo, y lo verde cuesta un
  arranque: `build.ps1` ya deja `titan/hola.bex` y `titan/dos.bex` en el
  disco (GRAMATICA, "Del texto al .bex").

---

# 4. LAS DECISIONES DEL PROPIETARIO

```text
   DL1  DONDE VIVE EL PROGRAMA. Se queda en PROTON-X
        (`bmo_proton_x::dxil::programa`) o sale a un crate NEUTRO (por
        ejemplo `platform/shared/bmo-programa`) y PROTON-X lo re-exporta.
        Recomendado: que SALGA -- el formato comun de TODAS las librerias no
        puede vivir dentro de la capa de Windows; con la re-exportacion,
        PROTON-X no cambia una linea de las suyas
   DL2  COMO SE ESCRIBE EL CONTRATO. Un trait de Rust en un crate puro (las
        cinco piezas de 1.2) o una tabla de datos. Recomendado: el trait para
        lo que es codigo (emitir, juzgar, simular) y el PERFIL para los techos
        (lo que la tarjeta dice de si)
   DL3  LA CPU COMO LIBRERIA, y la ley D2. Que la CPU CORRA una gpu fn como su
        reserva -- con los bits de la casa, que juzga el interprete -- no es
        que el programa "cuente un f32 en la CPU": el f32 sigue sin salir de
        la gpu fn si no es por `round`. Recomendado: SI, como ley nueva,
        sellada (`--sellar`)
   DL4  LOS BUCLES de una gpu fn. Solo `for i in range(N)` con N escrito
        (acaba por construccion), o tambien `while` (pide E7 visto en el metal
        antes de que uno llegue a la 3060). Recomendado: primero solo `range`
        con N escrito
   DL5  EL ENTERO dentro de una gpu fn: 32 bits; y desbordar, que hace (en
        TITAN++ ATRAPA, T0060; la 3060 da la vuelta)
   DL6  COMO DICE una gpu fn QUE DIBUJA: por su FIRMA (un registro de entrada
        y uno de salida; los tipos no gastan techo) o con una palabra nueva
        (quedan 5 de 30). Recomendado: la firma
   DL7  EL PERMISO de dibujar: `gpu = "draw"` en el Titan.toml (hoy existe
        `gpu = "compute"`), o `screen`
   DL8  EL CAMINO de TITAN++ a VERRANO: la lamina (como INTI: la app cuenta
        los vertices y el escritorio dibuja con los programas del BSF) o la
        puerta estrecha (como PROTON-X: la app trae sus cuerpos juzgados y
        sus DATOS). Recomendado: las dos, en ese orden -- la lamina es la que
        espera el corte 4 de INTI_SAMURAI; la puerta, la que VERRANO va a
        unificar (VC2)
   DL9  LOS VECINOS: por un asa que pone el kernel (fuera de rango, 0, como
        D3D; toca R7: Ring 0) o probados en rango al compilar (T0072)
   DL10 LI2g = D5, UNA vez para todo: los bits exactos (con la tercera salida
        de 3.5: el redondeo correcto tambien en la 3060) o la tolerancia de
        D3D, en la division, la raiz y el filtrado
   DL11 PARA QUE LIBRERIAS escribe `titan build`: todas las que haya, o las
        que diga el PERFIL de la maquina (y la CPU, siempre, de reserva)
```

---

# 5. LAS CASILLAS

## 5.0 Empezar: en el anfitrion, sin Ring 0

- [ ] **LB0 -- LAS DECISIONES de la seccion 4**, contestadas por el
  propietario. No hacen falta todas para empezar: LB1 y LB2 no piden
  ninguna; LB3 pide DL1 y DL2; LB4, DL3.
  - **Bloquea:** nada.
  - **Como se sabe:** cada DL con su respuesta y su fecha, aqui.

- [ ] **LB1 -- LOS NO QUE DICEN LA VERDAD (H1, H2, H3), y los papeles (H4).**
  `titan check` dice el NO de la division general en su linea y su columna, y
  el de una gpu fn en un programa que lee, cada uno con su motivo y su COMO;
  "fallo del compilador" queda para lo que lo es; `-o CARPETA` la crea; y los
  textos de H4 dicen lo que es, sin reescribir la historia (una nota con
  fecha, como la de G2 en CENTAURO).
  - **Bloquea:** nada.
  - **Como se sabe:** las tres sondas de 2.3 entran al banco
    (`toolchain/lang/titan/ejemplos/nivel11/`) con el MISMO codigo en check y
    en build; `cargo test -p bmo-titan-front -p bmo-titan-x86-64` en verde;
    `enlaces` y `ascii_sweep` limpios.

- [ ] **LB2 -- TITAN++ EN EL RYZEN, y la lamina de INTI.** Un arranque, en
  F12: `run titan/hola.bex` y `run titan/dos.bex`; y en la misma tanda,
  `run inti/cubo.ibx` y `gpu verrano banco inti` (el cuarto paso de E6 de
  PLAN_VERRANO), para que la lamina se haya visto ANTES de que TITAN++ la
  escriba.
  - **Bloquea:** un arranque, del propietario.
  - **Como se sabe:** la fila de TITAN++ del README pasa a verde con su foto
    (o la foto es el informe); `gpu verrano banco inti` acaba con `IGUAL al
    juez`.

- [ ] **LB3 -- EL CONTRATO, y SM86 dentro.** Las cinco piezas de 1.2 escritas
  como contrato (DL2) en un crate puro, el Programa donde diga DL1, y SM86
  como la primera libreria, juntando lo que ya existe: ni una regla nueva ni
  un bit distinto. `bmo-titan-sm86` deja de nombrar `bmo-proton-x-sm86` y
  `bmo-gpu-ga10x`: pide "las librerias" (L-a), y solo quien arma la
  herramienta (`titan`) dice cuales hay.
  - **Bloquea:** DL1, DL2.
  - **Como se sabe:** el SASS de `mezcla`, `suma` y `activa`, byte a byte el
    de antes (su hash, antes y despues); las 467 pruebas en verde; un
    guardian (la regla S de `la-3060`, o una hermana) falla si el frontend de
    TITAN++ o su escritor nombran una tarjeta -- probado metiendo el `use` a
    proposito --; y una libreria DE JUGUETE, en las pruebas, entra sin tocar
    TITAN++.

- [ ] **LB4 -- LA CPU, LA SEGUNDA LIBRERIA (la reserva).** El x86-64 del
  Programa (`platform/shared/proton-x/src/nativo.rs`, y `nativo_computo` para
  los que saltan) como libreria con sus cinco piezas: SABE
  (`nativo::por_que_no`), EMITE, JUZGA (el interprete, bit a bit, y el gate
  del `.bex`), SIMULA (el interprete ES la referencia) y ENTREGA (su codigo
  dentro del `.bex`). Con ella, E1 llama a una gpu fn AL CORRER: un programa
  que lee y usa una gpu fn ya funciona, sin Ring 0.
  - **Bloquea:** LB3, DL3.
  - **Como se sabe:** la sonda `lee()` + gpu fn pasa a BIEN, con `# entra:` y
    `# sale:`, y corre en el emulador; el azar de E1 (`E1_AZAR`) con llamadas
    a gpu fn da en cada celda los bits del oraculo (la 3060 simulada); y el
    `.bex` declara SSE en su `xcr0`, para que la cabecera no mienta (E7b de
    PLAN_VERRANO).

## 5.1 La escuela, en el anfitrion

- [ ] **LB5 -- IL1a, LOS BUCLES en una gpu fn** (sin vecinos todavia), por el
  contrato: el frontend deja lo que diga DL4 (y `break` y `continue`), el
  escritor escribe el Programa ESTRUCTURADO (de `Si` a `FinBucle`) en vez de
  la linea recta, las llamadas entre gpu fn van EN LINEA, y el entero entra
  si DL5 lo dijo.
  - **Bloquea:** LB3, DL4 (y DL5 para el entero).
  - **Como se sabe:** una gpu fn con un bucle (una potencia; una serie de
    Taylor con sus coeficientes escritos como literales f32, sin dividir) da
    los mismos bits por la 3060 simulada, la casa y el calculo en la bateria
    de bordes, y el juez dice que si; y con LB4 hecho (va antes en el orden),
    por la CPU los mismos. Las pruebas del NO: un bucle sin salida es T0066 en
    su linea y `SinFin` en el simulador, nunca un cuelgue del compilador.

- [ ] **LB6 -- LA gpu fn QUE DIBUJA.** Una gpu fn de VERTICE y una de PIXEL,
  dichas por su firma (DL6), escritas en TITAN++ y pegadas y juzgadas como las
  de BMOX-12 (`pso::traducir`: emisor, pegamento, juez). Las primeras: las dos
  del cubo de V0 (la de vertice deja la posicion y el color que le dan; la de
  pixel, el color), que hoy son SASS a mano
  (`platform/drivers/gpu/ga10x/src/trabajos/tuberia.rs`; E5 de LA_LENGUA:
  "Queda: cubo.bsf sin SASS a mano"), con el MAPA de la casa como fuente del
  BSF (A9, `MAPA_MAGIC` de `bmo-bsf`).
  - **Bloquea:** LB3, DL6.
  - **Como se sabe:** en el anfitrion, el simulador da los bits de la tanda de
    `bmo_cubo` y el sobre sale igual dos veces (determinista); en el metal,
    del propietario: `gpu verrano` con esos programas (como `gpu verrano
    bmox12` con los de PROTON-X) dice `IGUAL ... = D3D12 en la 3060 bajo
    Windows` en el 0, el 30 y el 60.

- [ ] **LB7 -- TITAN++ MANDA A VERRANO.** Una app de TITAN++ que dibuja el
  cubo girando, por el camino de DL8: el permiso (DL7) y su puerta en el
  certificado; en E1, el `f32` como DATO -- guardar, pasar y copiar a un
  bloque, lo que la ley L26 ya deja --; y los 360 fotogramas contados por gpu
  fn: en la 3060 simulada AL COMPILAR (eso es hornear), o al correr por LB4.
  Es el relevo que pide el corte 4 de INTI_SAMURAI (`cubo.inti` ->
  `cubo.titan`).
  - **Bloquea:** LB2 (la lamina vista), LB4 o el horneado, LB6 (si va por la
    puerta) y **DL10**: sin ella no se cuenta el cubo de `bmo_cubo` bit a bit
    (3.5).
  - **Como se sabe:** en el anfitrion, los 360 fotogramas publicados son los de
    `bmo_cubo::tanda`; en el metal, la app de TITAN++ corriendo y `gpu verrano
    banco inti` (o su hermana) sobre su lamina acaba con `IGUAL al juez`, como
    hoy con INTI.

## 5.2 Lo que es de Ring 0 (del propietario)

- [ ] **LB8 -- LA PUERTA DE COMPUTO (G4 de CENTAURO = LI2f de EL_LIBRETO).** La
  hermana de DIBUJAR para el cuerpo de computo de una app: la QMD que ya
  corre, el cuerpo juzgado con R7, las lecturas y escrituras puestas por el
  kernel, y el resultado en SU RAM. Con ella la libreria de SM86 ENTREGA
  computo, y una gpu fn corre en la 3060 al correr.
  - **Bloquea:** E7 visto en el metal (`gpu eterno ya`) y G0 (el GSP que
    despierta siempre); es Ring 0: se decide con el propietario.
  - **Como se sabe:** el banco del nivel 11 corrido por la 3060 AL CORRER, con
    las celdas del oraculo; y `olas` (A10) por la 3060, con su huella (la
    prueba de LI2f).

- [ ] **LB9 -- LOS VECINOS POR EL ASA (IL1b).** R7 aprende una lectura que no
  da direccion: por un asa que pone el kernel, acotada por su descriptor
  (DL9). El emisor la sabe poner (`IdHilo` y la lectura), y el simulador y la
  CPU (LB4) la hacen igual, con el 0 fuera de rango.
  - **Bloquea:** LB5, LB8 y DL9; es Ring 0.
  - **Como se sabe:** el desenfoque de TD1 (LA_TINTA) escrito en TITAN++, con
    los bits de la CPU; y R7 sigue diciendo NO a un `LDG` crudo (su prueba del
    NO).

## 5.3 La tercera libreria

- [ ] **LB10 -- UNA TARJETA MAS, sin tocar a nadie.** Cuando la haya (A5 de
  EL_AISLAMIENTO): una AMD (RDNA, con la ISA publicada) o un DIALECTO de
  NVIDIA (*"una NVIDIA nueva no es un compilador nuevo: es un DIALECTO"*,
  PLAN_VERRANO 2b: otra tabla de latencias y otra cabecera).
  - **Bloquea:** una segunda tarjeta en la maquina.
  - **Como se sabe:** entra con sus cinco piezas y no cambia ni una linea de
    TITAN++ ni de la libreria de SM86; `la-3060` S sigue limpio.

---

# 6. EL ORDEN

```text
   ahora, en el anfitrion   LB1 -> LB3 -> LB4 -> LB5 -> LB6 -> LB7
   el metal, en paralelo    LB2 cuanto antes (un arranque); despues, el final
                            de LB6 y de LB7
   las decisiones           LB0, a medida que cada casilla las pida
   Ring 0, del propietario  LB8 y LB9, despues de E7 en el metal
   cuando haya otra GPU     LB10
   y la escuela sigue       IL2 en adelante en PLAN_ILLAPA; el examen, el
                            censo LI0 de EL_LIBRETO cada vez que algo se abra
```

**Por que asi**, con el criterio de [`EL_ORDEN.md`](EL_ORDEN.md) (desbloquea,
corrige una mentira, ya esta medido, pide el metal, es grande):

- **LB1 primero, porque CORRIGE UNA MENTIRA**: un NO que culpa al compilador
  de un limite conocido manda a buscar un fallo que no existe.
- **LB2, porque PIDE EL METAL** y es barato: un arranque convierte catorce
  niveles amarillos en verdes, o dice por que no.
- **LB3 y LB4 juntos, porque DESBLOQUEAN lo que pidio el propietario**: con el
  contrato y DOS librerias (la 3060 y la CPU), "todas las GPU son librerias"
  queda probado en el anfitrion sin comprar otra tarjeta; y LB4 da gpu fn a
  los programas que corren, que es lo que hace a TITAN++ un modelo GENERAL y
  no un calculador que pliega al compilar.
- **LB5 antes que LB6**, porque es la puerta mas dura de Cyberpunk (LI2b)
  vista desde la escuela, y no puede llegar al metal por accidente: hoy
  ninguna gpu fn llega (3.2).
- **LB8 y LB9 al final**, porque son Ring 0, y lo que toca Ring 0 va cuando
  todo lo de encima ya esta probado (la misma razon de EL_LIBRETO, seccion 5).

---

# 7. LO QUE PUEDE FALLAR, dicho antes

- **Mover el Programa (DL1) toca a PROTON-X**, la zona mas viva del arbol.
  Con la re-exportacion no cambia su codigo; si se rompe una ruta, se ve al
  compilar, no en el metal.
- **La CPU de reserva puede ser lenta.** Es la reserva, no el camino: el metro
  (`toolchain/tools/metro`) dira cuanto, y lo que la 3060 pague se mide contra
  ella.
- **Un bucle puede colgar la 3060** el dia que algo llegue al metal: por eso
  DL4 empieza por `range` con N escrito, y LB8 pide E7 antes.
- **El cubo bit a bit pide DL10.** Sin una division exacta en algun sitio, el
  cubo de TITAN++ no es el de `bmo_cubo`, y el juez de VERRANO lo diria.
- **Una libreria de juguete puede esconder un contrato malo**: la prueba de
  verdad es LB4 (una segunda libreria con codigo real) y, el dia que llegue,
  LB10.

---

# 8. LO QUE ESTE PLAN NO ES

- **No es una gramatica nueva.** Lo que cambie del lenguaje lo dicen las
  decisiones, y entra por la GRAMATICA y por las leyes, con `--sellar`.
- **No reescribe PROTON-X ni VERRANO**: los usa. VC1..VC5 siguen en
  PLAN_VERRANO; LI0..LI7, en EL_LIBRETO.
- **No toca Ring 0.** LB8 y LB9 dicen lo que piden; son del propietario.
- **No es un IR nuevo**: el Programa ya es el comun desde E3 y LI7. Este plan
  solo lo saca de la capa de Windows, si DL1 lo dice.
- **No compra otra GPU**: la segunda libreria es la CPU.

---

# 9. DE DONDE SALE

- Los commits del 06 y el 07-10, en especial `43234b9` (LI7), `dcf91eb`
  (ILLAPA, la escuela), `3aa9869` (EL_LIBRETO), `6cdd524`, `bf9d76e` y
  `365cc36` (ILLAPA), `dbd13e9` (Cyberpunk, 68 s) y `2d76059` (V8).
- Leidos el 08-10: PLAN_VERRANO, PLAN_EL_LIBRETO, PLAN_ILLAPA,
  PLAN_EL_CENTAURO, PLAN_LA_LENGUA_DE_LA_3060, PLAN_EL_AISLAMIENTO,
  PLAN_INTI_SAMURAI, EL_ORDEN, ABIERTO, TITAN_MAESTRO (4.4, 7 y 8), la
  GRAMATICA de TITAN++ (niveles 4, 11, 12 y 13), LEYES.txt (L25 a L31),
  COMO_LE_HABLA_NVIDIA (3d) y METAL_2026-09-25 (seccion 1).
- El codigo, citado arriba con su linea, al commit `804fe8c`.
- Las ordenes, en el anfitrion: la de 2.2; `titan sm86` y `titan build` sobre
  `toolchain/lang/titan/ejemplos/nivel11/`; y las tres sondas de 2.3,
  escritas fuera del arbol.
