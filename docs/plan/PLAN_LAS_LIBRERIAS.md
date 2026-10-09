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

> **08-10, LB3:** el contrato ya esta escrito -- PROMETEO,
> `platform/shared/prometeo` (`bmo_prometeo::Tarjeta`: SABE, EMITE, JUZGA y
> SIMULA; ENTREGA espera al primero que entregue) --, y la 3060 lo cumple en
> `platform/shared/proton-x-sm86/src/tarjeta.rs`. El emisor de GPU de TITAN++
> ya no nombra a ninguna tarjeta. El CODIGO del Programa sigue en PROTON-X:
> LB3b.
>
> **08-10, a mitad de LB6:** la tarjeta salio de `proton-x-sm86` a su crate,
> con el nombre de su APARATO: `toolchain/forge/tarjeta-rtx3060-12g` (4.1).
>
> **08-10, LB3b:** el codigo tambien. El Programa vive en
> `platform/shared/prometeo/src/programa.rs`, con su interprete y lo que el
> interprete necesita; PROTON-X lo toma de `bmo-prometeo` y lo re-exporta en
> sus rutas de siempre (`dxil::programa`).

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

> **08-10, LB4:** el x86-64 del Programa ya es una TARJETA de PROMETEO
> (`toolchain/forge/tarjeta-cpu`, con el `nativo` de PROTON-X como su
> emisor), y una gpu fn corre AL CORRER en la CPU: E1 la llama
> (`emisor-x86_64/src/e1/gpu.rs`). Y TITAN++ ya corrio en el Ryzen (LB2):
> `hola.bex` y `dos.bex`.

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
  `e1/mod.rs:517` y `:658` (la fn y su llamada), `e1/valor.rs:41` (un f32),
  `e1/numero.rs:192` (su `round`) y `e1/escribe.rs:126` (escribirlo). Es la
  regla D2 (la ley L26) mas la falta de G4. Desde LB1 (08-10) ese rechazo es
  T0040, el NO del programa en su linea, y no un "fallo del compilador"
  (las lineas, las de despues de LB1).
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
  - **08-10, LB4:** hecho. Un programa que lee llama a una gpu fn y la corre
    la CPU, con los bits del oraculo; el f32 vive en E1 como DATO (se guarda,
    se pasa y vuelve por `round`). La 3060 al correr sigue siendo LB8.

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

> **08-10, LB5:** hecho lo de TITAN++: el frontend los deja (con su N
> ESCRITO, DL4 y la ley L33), el escritor escribe el Programa estructurado,
> y las llamadas entre gpu fn van en linea (5.1). Quedan los vecinos (LB9) y
> el entero (DL5).

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

> **09-10, DL10 (LB6b):** sacado de `ptxas` y hecho para la DIVISION: la 3060
> da los bits de IEEE, aguanta el error de su MUFU y su camino lento va
> dentro del cuerpo -- costo 105 instrucciones, no exactitud --. La cuenta de
> `ptxas` sola NO bastaba: con un inverso a medio ULP mal redondeado su
> correccion cae del lado malo; la prueba de Tuckerman, si. La raiz sigue
> como MUFU: hacerla exacta cambia el SASS de BMOX-12 que corre en el metal,
> y va con el propietario.

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

> **08-10, despues: H1, H2 y H3 los cierra LB1** (las lineas citadas arriba
> son las de `804fe8c`, donde se encontraron). **H4**: una nota fechada en
> cada sitio, mas tres que se encontraron al hacerlo: el README (*"a `gpu fn`
> runs on the 3060"*, que no corre en la 3060: corre en la 3060 SIMULADA al
> compilar; y el banco ya no es de 87 programas), V4 y M4 de
> [`PLAN_VERRANO.md`](PLAN_VERRANO.md) ("el emisor SPIR-V a SM86") y G3 de
> CENTAURO (`check` ya no se queda en el f32 del calculo). **H5 sigue: es
> LB2.**

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

> **08-10, a mitad de LB6:** DL6 se TOMO como la recomienda el plan -- la
> FIRMA --, para que LB6 no parara; queda que el propietario la confirme. Si
> elige una palabra nueva, cambia `gpu::forma` (el frontend) y nada mas: el
> escritor, la tarjeta y el sobre reciben la forma ya dicha.
>
> **09-10:** confirmada, con DL7, DL8 y DL10 (4.1).

## 4.1 Lo que ya contesto el propietario

**DL1 y DL2 -- el 08-10, a mitad de LB1:** *"por cierto libreria "gpu
general" = ese mismo se engordara que se llevara todo el emisor de GPU para
que aplique"*.

LA LECTURA, que hay que confirmar antes de LB3:

```text
   gpu general   UNA libreria comun, la que CRECE: se lleva el emisor de GPU
                 entero, para que valga para cualquier tarjeta
   DL1           el Programa SALE de PROTON-X: es de lo general, no de la
                 capa de Windows (PROTON-X lo re-exporta)
   DL2           el contrato vive en lo general, y lo general no es una tabla
                 quieta: engorda con lo que cada tarjeta nueva descubra comun
```

Lo que se lleva, medido en `toolchain/lang/titan/emisor-sm86/src/lib.rs`
(614 lineas, 08-10): casi todo ya es general; SM86 es el nombre y tres
llamadas --

```text
   GENERAL, a "gpu general"   el escritor gpu fn -> Programa (`programa`,
                              `eval`, `straight`, `inverso_exacto`); la
                              bateria de bordes y la comparacion con la casa
                              y el calculo (`battery`, `compare`, `verify`);
                              el `Oracle` del calculo; el limite dicho en su
                              sitio (LB1)
   DE LA 3060, por contrato   emitir (`emitir_con` y sus dos ABI), juzgar
                              (`juzgar_cuerpo_de_app`), simular (`correr`),
                              sus techos (64 registros) y el porque de su
                              limite (el MUFU)
```

Y lo que NO se lleva, si la lectura es buena: lo de cada tarjeta segun
[`PLAN_EL_AISLAMIENTO.md`](PLAN_EL_AISLAMIENTO.md) -- su emisor del Programa
a SU ISA (`platform/shared/proton-x-sm86`, `platform/shared/bmo-sm86`), su
juez `no_std` (el kernel lo usa en SU puerta), su simulador y su puerta.

[!] Dos cosas a confirmar:

- **"Todo el emisor de GPU"** es el de TITAN++ (lo de arriba), o tambien el
  paso del Programa a cada ISA. Lo segundo cambia EL_AISLAMIENTO y la regla
  L-b: *"un emisor o un juez 'para todas' seria uno que no es preciso para
  ninguna"*, y el propietario el 26-09: *"TIENEN QUE AISLARSE POR
  COMPLETO"*. Recomendado: lo primero; lo que dos ISA tengan de verdad en
  comun (asignar registros, ordenar por latencias) entra en lo general
  cuando haya DOS que lo usen, no antes.
- **Donde vive.** El Programa y el contrato los usan tambien PROTON-X y la
  CPU, y `platform/shared` solo puede usar lo puro (el guardian `capas`):
  van ABAJO, en un crate neutro (por ejemplo `platform/shared/gpu-general`).
  El escritor de TITAN++ lee la IR de TITAN++: vive ARRIBA
  (`toolchain/lang/titan/emisor-sm86` pasaria a `emisor-gpu`). "gpu
  general" serian dos crates con un nombre. Recomendado: asi.

**Confirmado el 08-10.** El propietario: *"si confirmo las dos, vamos con
LB3, pero eso poner nombre epico la libreria"*; y a mitad de LB3: *"el ISA
bueno pero debes aislar por completo, TODAS LAS GPU en emisor SON AISLADAS
por completo luego el JUEZ procesa cada uno y el principal "SUPREMO JUEZ" que
bueno ya eso es el motivo lel para GPU Final"*. Asi quedo (LB3):

```text
   el nombre        PROMETEO: el titan que les llevo el fuego a todos
   abajo            platform/shared/prometeo (bmo-prometeo): el Programa de
                    la casa y el contrato `Tarjeta`
   arriba           toolchain/lang/titan/prometeo (bmo-titan-prometeo): el
                    emisor de GPU de TITAN++ entero (era emisor-sm86)
   cada GPU         SU tarjeta, AISLADA: su emisor (del Programa a SU ISA), su
                    juez y su simulador. La 3060:
                    platform/shared/proton-x-sm86/src/tarjeta.rs
   el SUPREMO JUEZ  el de la puerta del kernel, delante de la GPU final: el
                    mismo juez de esa tarjeta, otra vez, y el que no se puede
                    saltar (J2). Hoy, el de VERRANO; el del computo, LB8
```

**DL3 -- el 08-10, despues de LB3b.** La pregunta llevaba la recomendacion
(*"SI, como ley nueva y sellada"*), y la foto de `run titan/dos.bex` en el
Ryzen; el propietario: *"es buena continua y vamos hasta el final"*. Asi
quedo (LB4):

```text
   la ley        L32: una gpu fn corre tambien en la CPU, su RESERVA -- el
                 x86-64 de la tarjeta de la CPU, juzgado y con los mismos
                 bits que la casa y el calculo --; en un programa que lee es
                 lo que corre AL CORRER; y el f32 sigue sin salir de la gpu
                 fn si no es por `round` (L26). Sellada (82229cc73dc8)
   la tarjeta    toolchain/forge/tarjeta-cpu: su emisor es el `nativo` de
                 PROTON-X; su juez, el contrato de la llamada; su simulador,
                 el emulador de la casa. En la fragua y no en platform/shared
                 por su simulador, que es una herramienta (capas)
   en cada build las DOS tarjetas, la 3060 y la CPU: cada gpu fn se escribe,
                 se juzga y pasa la bateria en las dos, con los mismos bits,
                 o no hay .bex. Las celdas de un programa que no lee siguen
                 siendo las de la 3060 (la primera)
```

**DL4 -- el 08-10, despues de la primera mitad de LB5.** La pregunta: *"DL4:
tome la recomendacion del plan -- solo `range` con N escrito. La confirmas
como ley sellada?"*, con el tope de la obra de una celda (65536) dicho al
lado; el propietario: *"si ve hasta el final"*. Asi quedo (LB5):

```text
   la ley        L33: un hilo de la GPU ACABA por construccion -- dentro de
                 una gpu fn solo hay `for` con sus vueltas ESCRITAS, la obra
                 de una celda se sabe al compilar y no pasa de 65536, y una
                 llamada a otra gpu fn se escribe EN LINEA --. Sellada
                 (555cb58adfd1)
   la obra       cada operacion y cada linea escrita es 1, y un `for` sus
                 vueltas por (2 + lo de dentro): con el N escrito es la misma
                 en cada celda, tambien en las que llegan al correr (LB4)
   el `while`    sigue fuera de una gpu fn (T0090), hasta E7 visto en el
                 metal
```

**LA TARJETA POR SU APARATO, y UNA SOLA GPU -- el 08-10, a mitad de LB6.** El
propietario: *"que la GPU no sea por ISA sea por especificamente muy precisos
no es ISA sino propio ISA modular eso y ya con eso es mas facil de detectar
porque falla [...] (solo: 3060 12G o nombre especifico para este que emitir
es) y asi elegante para priorizar"*; y luego: *"Solo uno, un GPU que haremos
porque la verdad no tengo otros GPU y seria jugar a adivinanza"*. Asi quedo:

```text
   la tarjeta    toolchain/forge/tarjeta-rtx3060-12g (bmo-tarjeta-rtx3060-12g),
                 al lado de la de la CPU: la RTX 3060 12G por su APARATO
                 exacto -- NVIDIA GeForce RTX 3060 12G, GA106, 10DE:2503 o
                 10DE:2504, 12288 MiB, las constantes del driver
                 (lectura::identidad) -- y su ISA, SASS sm_86 (el emisor de
                 PROTON-X), como un modulo SUYO. Vivia en
                 proton-x-sm86/src/tarjeta.rs con el nombre de su ISA (SM86);
                 ni un bit del SASS cambio
   la ficha      PROMETEO: `Ficha` lleva el `Aparato` (modelo, chip, PCI,
                 memoria) y `Aparato::es(fabricante, dispositivo)`. Cada NO
                 dice el aparato ("el juez de la RTX 3060 12G dijo que no"), y
                 `titan sm86` dice "SASS sm_86 SOLO para NVIDIA GeForce RTX
                 3060 12G"
   la prioridad  `titan` (TARJETAS): la RTX 3060 12G primero; la CPU, la
                 reserva, la ultima
   DL11          contestada: `titan build` escribe para la UNICA GPU que hay,
                 la RTX 3060 12G, y para la CPU, siempre, de reserva. Nada de
                 adivinar tarjetas que no estan
   LB10          retirada: no hay otra GPU que probar
```

**DL6, DL7, DL8 y DL10 -- el 09-10, despues de LB6 en el anfitrion.** Las
cuatro preguntas llevaban su recomendacion, y el propietario tomo las cuatro.
Asi quedo:

```text
   DL6           la FIRMA: una gpu fn que devuelve un registro con su
                 `posicion` es de vertice; la que recibe ese registro y
                 devuelve 4 f32, de pixel. Ninguna palabra nueva: siguen 25
                 de 30. Es lo que LB6 ya hizo (`gpu::forma`)
   DL7           `screen` AHORA, para la lamina: la puerta `Door::Screen`
                 del certificado, que nadie usaba -- la app no toca la GPU --;
                 y `gpu = "draw"` cuando llegue la puerta estrecha
   DL8           las DOS, en ese orden: primero la LAMINA, como INTI (la app
                 cuenta los vertices y el escritorio los dibuja con el sobre:
                 el corte 4 de INTI_SAMURAI); despues la PUERTA ESTRECHA
                 (VRN2, la que VERRANO va a unificar, VC2)
   DL10          los bits EXACTOS, tambien en la 3060: la division y la raiz
                 con la secuencia de redondeo correcto, que aguanta el error
                 del MUFU, con su camino para los bordes DENTRO del cuerpo (R7
                 no deja salir). La ley L29 sigue siendo verdad en el metal
```

> **09-10, despues:** la division, HECHA en el anfitrion (LB6b, 5.1); la
> raiz, con el propietario (cambia el SASS de BMOX-12 que corre en el metal).

---

# 5. LAS CASILLAS

## 5.0 Empezar: en el anfitrion, sin Ring 0

- [ ] **LB0 -- LAS DECISIONES de la seccion 4**, contestadas por el
  propietario. No hacen falta todas para empezar: LB1 y LB2 no piden
  ninguna; LB3 pide DL1 y DL2; LB4, DL3.
  - **Bloquea:** nada.
  - **Como se sabe:** cada DL con su respuesta y su fecha, aqui.
  - **08-10:** DL1 y DL2, contestadas con *"gpu general"* y confirmadas el
    mismo dia (4.1): PROMETEO. Las demas, abiertas.
  - **08-10, despues:** DL3, contestada (4.1): la ley L32, con LB4.
  - **08-10, mas tarde:** DL4, contestada (4.1): la ley L33, con LB5.
  - **08-10, a mitad de LB6:** DL11, contestada (4.1): solo la RTX 3060 12G,
    y la CPU de reserva.

- [x] **LB1 -- LOS NO QUE DICEN LA VERDAD (H1, H2, H3), y los papeles (H4).**
  **HECHO el 08-10** (el codigo, `129ebf8`).
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
  - **Lo hecho:**
    - el NO de una libreria de la GPU es de DOS clases (`calc::DeviceNo`,
      `toolchain/lang/titan/src/calc.rs`): el LIMITE que dice a proposito
      (un `Message`: el NO del programa, en su sitio) y el FALLO de su
      escritor, su juez o su oraculo. La division general es T0090 en la
      linea y la columna de la DIVISION, no de la llamada, con su porque
      (L29: MUFU.RCP + FMUL no dan los bits de la casa) y su como (una
      potencia de dos);
    - lo que E1 todavia no emite (una gpu fn o un f32 en un programa que
      lee, una conversion...) es T0040 -- *lo que todavia no existe*, el
      mismo sentido de siempre (L02) -- en su fichero y su linea, con un
      porque y un como que nombran la casilla que lo trae (LB4, LB7);
    - `titan check` corre LO MISMO que `titan build`, sin escribir
      (`check_package`): dicen lo mismo por construccion; y
      `titan sm86 -o CARPETA` la crea;
    - uno que no estaba en H1-H3: el escritor de la 3060 decia la linea del
      PAQUETE con el nombre del fichero hijo (solo cuadraba en la raiz);
      ahora, la del fichero;
    - H4: notas con fecha en ILLAPA, LA_TINTA, CENTAURO, TITAN_MAESTRO,
      EL_AISLAMIENTO, VERRANO y COMO_LE_HABLA_NVIDIA; el README, corregido
      en su sitio (dice lo de hoy: no es una historia). La GRAMATICA, al dia.
  - **Como se supo -- y por que cambio el "como se sabe":** las sondas NO
    entraron al banco de `ejemplos/`. Ese banco es el del FRONTEND
    (`lower_package`, sin libreria), y por L05 el frontend no ve el limite
    de una tarjeta: con `# espera: T0090` diria que compilo. Viven en las
    pruebas de quien dice el NO:
    `a_known_limit_is_the_program_s_no_in_check_and_in_build`
    (`toolchain/lang/titan/emisor-x86_64/src/lib.rs`: la division, T0090
    3,14; `lee()` y una gpu fn, T0040 2,1; un f32 en un programa que lee,
    T0040 5,9; check == build en las tres, y `x / 2.0` dice bien),
    `a_limit_of_the_library_is_the_program_s_no_where_it_is_written` y
    `a_limit_in_a_child_module_is_said_in_its_file_and_its_line`
    (`toolchain/lang/titan/emisor-sm86/src/pruebas.rs`; la segunda cae con
    la linea vieja: mutacion probada). 470 pruebas en verde (eran 467);
    check y build, el mismo codigo de salida en los 116 `.titan` de
    `ejemplos/`; `titan_leyes`, `ascii_sweep`, `la_3060`, `capas`,
    `maestros`, `enlaces` y `planes` limpios.

- [ ] **LB2 -- TITAN++ EN EL RYZEN, y la lamina de INTI.** Un arranque, en
  F12: `run titan/hola.bex` y `run titan/dos.bex`; y en la misma tanda,
  `run inti/cubo.ibx` y `gpu verrano banco inti` (el cuarto paso de E6 de
  PLAN_VERRANO), para que la lamina se haya visto ANTES de que TITAN++ la
  escriba.
  - **Bloquea:** un arranque, del propietario.
  - **Como se sabe:** la fila de TITAN++ del README pasa a verde con su foto
    (o la foto es el informe); `gpu verrano banco inti` acaba con `IGUAL al
    juez`.
  - **08-10, la mitad:** `run titan/hola.bex` escribio `hola` en el Ryzen,
    con su foto, y la copia en `datos/titanhol.txt`
    ([`METAL_2026-10-08.md`](../metal/METAL_2026-10-08.md)): la primera vez
    que TITAN++ corre en el metal. Faltan `run titan/dos.bex`, `run
    inti/cubo.ibx` y `gpu verrano banco inti`.
  - **08-10, despues:** `run titan/dos.bex` escribio `hola desde saluda` y
    `hola otra vez`, con su foto y su copia en `datos/titandos.txt` (seccion
    1b de la bitacora): una llamada entre funciones y un `print` de dos
    textos, en el metal. Los dos programas de TITAN++ que despliega el build
    ya corrieron. Faltan los de INTI: `run inti/cubo.ibx` y `gpu verrano
    banco inti`.

- [x] **LB3 -- EL CONTRATO, y SM86 dentro.** **HECHO el 08-10: PROMETEO.**
  Las cinco piezas de 1.2 escritas
  como contrato (DL2) en un crate puro, el Programa donde diga DL1, y SM86
  como la primera libreria, juntando lo que ya existe: ni una regla nueva ni
  un bit distinto. `bmo-titan-sm86` deja de nombrar `bmo-proton-x-sm86` y
  `bmo-gpu-ga10x`: pide "las librerias" (L-a), y solo quien arma la
  herramienta (`titan`) dice cuales hay.
  - **Bloquea:** DL1, DL2.
  - **Como se sabe:** el SASS de `mezcla`, `suma` y `activa`, byte a byte el
    de antes (su hash, antes y despues); las pruebas en verde (470 el
    08-10); un guardian (la regla S de `la-3060`, o una hermana) falla si el
    frontend de TITAN++ o su escritor nombran una tarjeta -- probado metiendo
    el `use` a proposito --; y una libreria DE JUGUETE, en las pruebas, entra
    sin tocar TITAN++.
  - **08-10, con DL1 y DL2 contestadas (4.1):** LB3 ES la libreria *"gpu
    general"*: abajo, el Programa (fuera de PROTON-X, que lo re-exporta) y
    el contrato; arriba, el emisor de GPU de TITAN++ (`emisor-sm86` pasa a
    `emisor-gpu`), con la 3060 como la primera tarjeta por el contrato. Lo
    que la bloquea ahora: que el propietario confirme la LECTURA (las dos
    cosas del [!] de 4.1). -- Confirmada el mismo dia, con su nombre:
    PROMETEO, y `emisor-sm86` paso a `toolchain/lang/titan/prometeo`.
  - **Lo hecho:**
    - `platform/shared/prometeo` (`bmo-prometeo`; `no_std`, generacion
      hijo): el contrato `Tarjeta` -- `ficha` (SABE: nombre, lengua,
      registros), `emitir` (EMITE, para el oraculo o para el viaje; lo que
      no sabe, un `Limite` en sus palabras o un `Fallo`), `juzgar` (JUZGA) y
      `simular` (SIMULA) --, y el Programa de la casa con su nombre
      (`bmo_prometeo::programa`);
    - la 3060 detras del contrato: `platform/shared/proton-x-sm86/src/tarjeta.rs`
      (`Sm86`, `SM86`; **08-10, a mitad de LB6:** hoy
      `toolchain/forge/tarjeta-rtx3060-12g`, `RTX_3060_12G`, por su aparato
      exacto -- 4.1), con lo que ya existia: `emitir_con` en sus dos ABI,
      `juzgar_drenado` y `juzgar_cuerpo_de_app`, `simula::correr`, los 64
      registros de `tuberia::REGISTROS` y la division como su `Limite`;
    - el emisor de GPU de TITAN++ paso a `toolchain/lang/titan/prometeo`
      (`bmo-titan-prometeo`, con `git mv`): depende de `bmo-titan-front` y de
      `bmo-prometeo`, y de nada mas. `kernels`, `write` y el `Oracle` piden
      las tarjetas (`&[&dyn Tarjeta]`), y una gpu fn pasa por CADA una -- su
      emisor, su juez, su simulador, contra la casa y el calculo --. `titan`
      dice cuales hay (`TARJETAS`, hoy la 3060; DL11 sigue abierta);
    - el calculo del frontend ya no dice "la 3060 no pudo correr": un fallo
      puede ser de cualquier tarjeta (L05);
    - la regla S5 de `la-3060`, y las leyes L28-L31 re-selladas: ningun texto
      cambio, solo donde viven sus pruebas.
  - **Como se supo:**
    - el SASS, byte a byte, antes y despues (sha256): `mezcla`
      `7aad3f9b628d`, `suma` `f7b85c6ff26d`, `activa` `faa1408e92fc`; los 37
      `.bex` de los programas BIEN del banco, iguales; y `titan sm86` dice lo
      mismo, letra a letra;
    - 476 pruebas en verde (eran 470). `tests/juguete.rs`: una tarjeta DE
      JUGUETE (su codigo es un cajon, su simulador el interprete de la casa,
      su juez un techo, y no sabe dividir) entra SOLO por lo publico: corre
      una gpu fn, se pone al lado de la 3060 con los mismos bits, dice su
      limite en sus palabras, su juez dice NO como un fallo, y sin tarjetas
      no se escribe a ciegas. Saboteado su simulador (un bit), la bateria lo
      caza;
    - S5, probada sembrando cuatro nombres a proposito (PROMETEO con
      `SM86`; el `Cargo.toml` y el codigo del emisor de TITAN++ con
      `bmo-gpu-ga10x`; el frontend con `bmo_proton_x_sm86`): los cuatro caen;
    - lo que enlaza la 3060 sigue en pie: `proton-x-casa` (237 pruebas) y la
      app PROTON-X de Ring 3, compilada para `x86_64-unknown-none`; check y
      build, el mismo codigo de salida en los 116 `.titan`; los guardianes
      del build, limpios (`avisos`: 38 contra 37 en este contenedor, por el
      aviso de `core` del nightly del 07-10; el kernel no cambio).
  - [!] **Lo que NO se hizo, a proposito:** mudar el CODIGO del Programa
    (LB3b). PROMETEO lo da con su nombre, pero sigue viviendo en PROTON-X.
    - **08-10, despues:** mudado el mismo dia, en LB3b (abajo).

- [x] **LB3b -- LA MUDANZA DEL PROGRAMA (el resto de DL1).** **HECHO el
  08-10.** El codigo del
  Programa de la casa sale de PROTON-X a PROMETEO, y la flecha se da la
  vuelta: PROTON-X lo toma de `bmo-prometeo` (y lo re-exporta en
  `dxil::programa`, para no cambiar una linea de las suyas). Lo que se muda
  no es un fichero: es el formato y el interprete con lo que el interprete
  necesita -- `mates`, `bufer`, `formato_ia`, `textura`, `bc`, `trama`,
  `cuadros`, `olas`, `ranuras` --, y `programa.rs` se parte en dos: el
  formato y el interprete abajo, el traductor de DXIL en PROTON-X.
  - **Bloquea:** una ventana tranquila en PROTON-X. `programa.rs` tuvo 18
    commits e `interprete.rs` 17 entre el 30-09 y el 07-10 (Cyberpunk):
    mudarlos con trabajo vivo encima es un conflicto seguro. Cuando el
    propietario diga.
  - **Como se sabe:** `bmo-prometeo` deja de depender de `bmo-proton-x`, y
    `bmo-proton-x` depende de `bmo-prometeo`; las pruebas de PROTON-X, de
    `proton-x-casa`, de la 3060 y de TITAN++, en verde sin cambiar una; el
    SASS y los `.bex` de LB3, byte a byte; y S5 mirando tambien que
    PROMETEO no dependa de PROTON-X.
  - **08-10, el propietario:** *"vamos con LB3b, mudar el Programa a
    PROMETEO, ya funciona y ahora toca el siguiente, no?"* -- con la foto de
    `run titan/hola.bex` en el Ryzen (LB2). La ventana, tranquila: lo ultimo
    en los ficheros que se mudan es del 07-10, y ninguna rama tenia trabajo
    en PROTON-X fuera de main.
  - **Lo hecho:**
    - con `git mv` (su historia los sigue), de `platform/shared/proton-x/src`
      a `platform/shared/prometeo/src`: `interprete`, `ranuras` y `carriles`
      (de `dxil/`), `mates`, `bufer`, `formato_ia`, `textura` y `bc`;
    - partidos en dos, lo que HACE abajo y lo de DXIL arriba:
      `dxil/programa.rs` (el formato -- `Op`, `Reg`, `Programa`,
      `MalaForma`... -- y su `impl` a `prometeo/src/programa.rs`; el
      traductor, `Compilador`, se queda), `dxil/olas.rs` (lo que hace cada
      operacion de ola a `prometeo/src/olas.rs`; como se lee del DXIL, se
      queda) y `dxil/computo.rs` (`Programa::despachar` a
      `prometeo/src/despacho.rs`; `preparar`, el CS de D3D12, se queda);
    - de `trama`, solo la prueba de profundidad (`Profundidad`, a
      `prometeo/src/profundidad.rs`): es lo unico de `trama` que el
      interprete usa. La lista de arriba decia `trama` y `cuadros`; medido
      el cierre, no hacian falta, y se quedan en PROTON-X con `mezcla`, el
      stencil, `nativo` y los traductores de DXIL y de SM5;
    - la flecha, al reves: `bmo-prometeo` no depende de nada, y
      `bmo-proton-x` depende de `bmo-prometeo` y lo re-exporta en las rutas
      de siempre (`dxil::programa`, `dxil::olas`, `bmo_proton_x::textura`...):
      quien lo usa no cambio una linea;
    - dentro de lo mudado, solo dos rutas (`crate::trama::Profundidad` pasa
      a `crate::profundidad::Profundidad`), tres funciones de `bufer` que
      `trama` usa (de `pub(crate)` a `pub`) y los comentarios que nombraban
      la ruta vieja;
    - S5 de `la-3060` mira tambien que PROMETEO no nombre PROTON-X, ni en su
      `Cargo.toml` ni en su codigo; y PX4 de `toolchain/tools/proton-x` (sin
      licencias ajenas) sigue al codigo: mira tambien
      `platform/shared/prometeo/src`.
  - **Como se supo:**
    - 984 pruebas en verde antes y despues, y las mismas: ningun fichero de
      pruebas cambio, y las 32 de los ficheros mudados corren ahora en
      `bmo-prometeo` (las de `bmo-proton-x` pasan de 248 a 216). En los diez
      crates de la 3060, PROTON-X, PROMETEO, TITAN++ y VERRANO
      (`bmo-proton-x`, `bmo-proton-x-casa`, `bmo-proton-x-sm86`,
      `bmo-gpu-ga10x`, `bmo-prometeo`, los tres de TITAN++, `bmo-verrano` y
      `bmo-cubo`);
    - el SASS de LB3, byte a byte (`mezcla` `7aad3f9b628d`, `suma`
      `f7b85c6ff26d`, `activa` `faa1408e92fc`), y los 37 `.bex`, iguales;
    - la app PROTON-X de Ring 3, PROMETEO y PROTON-X compilan para
      `x86_64-unknown-none`; el kernel no enlaza ninguno de los dos (`cargo
      tree`);
    - S5, probada sembrando `bmo-proton-x` en el `Cargo.toml` de PROMETEO y
      `use bmo_proton_x::trama;` en su codigo: los dos caen. PX4, sembrando
      una cabecera SPDX en `prometeo/src/textura.rs`: cae;
    - los guardianes, limpios -- los del build, el contrato y el de PROTON-X
      (291 fuentes, eran 278: las de PROMETEO) --, con `avisos` en 38 contra
      37, el aviso de `core` del nightly, como en LB3; y la herencia
      (`censo-modular/herencia.py`): PROMETEO y PROTON-X son hijo, y ninguna
      dependencia sube.

- [x] **LB4 -- LA CPU, LA SEGUNDA LIBRERIA (la reserva).** **HECHO el
  08-10.** El x86-64 del
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
  - **08-10, el propietario:** *"es buena continua y vamos hasta el final"*
    (DL3, en 4.1).
  - [!] **Una correccion a esta casilla:** el "E7b de PLAN_VERRANO" no existe
    (el E7b que hay es de LA_LENGUA, y es otra cosa), y el `xcr0` no hacia
    falta tocarlo: el escritor de BEF2 pone x87+SSE en todo ejecutable
    (`Escritor::ejecutable`), y los `.bex` de TITAN++ ya lo decian. Lo que
    queda es la prueba de que el de una gpu fn lo dice.
  - **Lo hecho:**
    - **la tarjeta de la CPU**, `toolchain/forge/tarjeta-cpu`
      (`bmo-tarjeta-cpu`, `CPU`), por el contrato de PROMETEO y sin tocarlo:
      - SABE la cuenta de una celda -- entradas y salida, la aritmetica de
        f32 (la division general tambien: `divss` es IEEE), comparaciones,
        enteros de 32, `Elige`, saltos y bucles --; lo demas (texturas,
        matematica, UAV, computo, olas) es su LIMITE, con su porque;
      - EMITE el `nativo` de PROTON-X con un prologo que pone sus registros;
      - JUZGA el contrato de la llamada, corriendolo: vuelve, no llama al
        kernel, conserva rbx, rbp, r12..r15, rsp y el MXCSR, deja eax = 0 y
        no escribe fuera de sus registros, su salida y su pila. Y lo mira en
        CADA celda que simula;
      - SIMULA sus bytes en el emulador de la casa: los mismos que van en el
        `.bex`;
    - **las dos tarjetas en cada build** (`TARJETAS` de `titan`): la 3060 y
      la CPU. Cada gpu fn pasa la bateria y sus celdas reales en las dos;
    - **E1 llama a la gpu fn al correr** (`emisor-x86_64/src/e1/gpu.rs`): un
      hilo por celda, con su sitio en el marco de quien llama, y el cuerpo de
      la CPU una vez al final del `.bex`;
    - **el f32 como DATO en E1**: un literal, un int o un dec a f32 al mas
      cercano (una subrutina de enteros: el `parse::<f32>` de su decimal
      exacto), y `round(x, n)` de vuelta a dec (otra: el f32 escrito entero
      y cortado, la mitad lejos del cero; T0062 un infinito o un NaN, T0060
      lo que no cabe). E1 no cuenta en f32: pasa bits (D2);
    - **el emulador de la casa aprendio lo que el x86 de una gpu fn usa**
      (`toolchain/forge/bmo-lower/src/emu`): `and`/`cmp` con eax e
      inmediato, `xor r, r/m`, `test r/m, imm32`, `cmovcc`, `cmpss`,
      `andps`/`andnps`/`orps`, `ldmxcsr`/`stmxcsr` (con su MXCSR), un `run`
      que no revienta y el registro de lo que escribe un programa. Y se
      encontraron CUATRO cosas que modelaba distinto del silicio, todas en 32
      bits: `shl`/`shr`/`sar` por `cl` enmascaraban la cuenta a 6 bits y
      `sar` miraba el bit 63; `cdq` era `cqo`; `div` e `idiv` miraban rax en
      vez de edx:eax. Ningun emisor de la casa desplazaba ni dividia en 32
      bits hasta hoy;
    - **la ley L32** (DL3), sellada; la regla S5 de `la-3060` cuenta tambien
      la tarjeta de la CPU como un nombre que lo general no dice; el ejemplo
      `nivel12/reserva` en el banco y en la TAB (35 nodos maestros).
  - **Como se supo:**
    - la tarjeta: cada operacion que dice saber, contra el interprete en los
      valores que muerden (las de f32 en 18 bordes y por los dos caminos de
      `nativo`, la fila de SSE y el cuerpo con saltos; los enteros, en 12;
      los saltos y un bucle); el juez diciendo que NO a seis formas de
      romper la llamada; sus limites, dichos sin emitir;
    - las dos sondas de LB1 que eran T0040 -- una gpu fn y un f32 en un
      programa que lee -- corren y escriben lo que diria el calculo;
    - el oraculo de E1 mira ahora tambien los programas con gpu fn: 37
      programas (2 con gpu fn), E1 == el calculo. O sea: la CPU al correr da
      lo que la 3060 simulada al compilar;
    - el azar con gpu fn (`E1_AZAR_GPU`): una gpu fn con cuerpo al azar,
      alimentada con numeros tecleados (por un int y por un dec calculado al
      correr), como valor y como tabla, y `round` a la vuelta, contra el mismo
      programa con los numeros escritos: 120 casos de siempre, y 1500 con
      cada una de dos semillas (la 11: 1221 iguales y 279 con el mismo NO; la
      977: 1232 y 268), sin un desacuerdo. El unico que salio al escribirla
      fue mio: `round` de -2^63 da i64::MIN, que cabe en un dec;
    - las dos subrutinas, corridas en el emulador contra la regla del calculo
      (copiada): 200.000 casos de cada una; y una regla sembrada mal cae;
    - el `.bex` de un programa con gpu fn dice x87+SSE en su `xcr0`;
    - lo de antes, igual: los 37 `.bex` de los programas BIEN, byte a byte;
      el SASS de `mezcla`, `suma` y `activa`, igual; y los bancos de todos los
      lenguajes que usan el emulador (C, COBOL, Ada, C++, INTI, SPIR-V, el
      enlazador, el espejo, el metro): 1305 en verde antes de los cambios del
      emulador (origin/main) y las mismas 1305 despues.

## 5.1 La escuela, en el anfitrion

- [x] **LB5 -- IL1a, LOS BUCLES en una gpu fn** (sin vecinos todavia), por el
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
  - **HECHO el 08-10**, en tres commits (`eca65be`, `a9b9615`, `b80e97e`).
    DL4, del propietario: *"si ve hasta el final"* (4.1).
  - **Lo hecho:**
    - el frontend (`gpu.rs`): `for i in range(N)` y `range(A, B)` con los
      extremos ESCRITOS (hasta 2^24: el contador es f32), con `break`,
      `continue` y `return` dentro, y la llamada a otra gpu fn. Cada salida
      tiene su NO en su sitio: el `while`, un extremo no escrito, mas de 8
      bucles uno dentro de otro, y una llamada a la CPU, a si misma o sola
      (T0090); y demasiada OBRA para un hilo (T0066);
    - LA OBRA de una celda (`gpu::obra`, guardada en `ir::Function::obra`),
      contada al compilar, con su tope: 65536. Es la ley L33;
    - el escritor (`bmo-titan-prometeo`) escribe el Programa ESTRUCTURADO:
      - cada `for` es un `Bucle`: `RomperSi` en su cabeza y en cada
        `break`, `FinBucle` en su paso, y dentro de su `Si` si lo de fuera
        no siempre llega;
      - lo que cruza la vuelta vive en su CASA, un registro que cada
        asignacion escribe con su predicado: un `break` es un `RomperSi` y
        ya, y lo que cuesta una vuelta crece con lo que se escribe en ella;
      - los `if` siguen en linea recta, tambien dentro de un bucle;
      - una llamada se escribe EN LINEA, con sus bucles y sus `return`;
      - sin bucles, el Programa sale como antes, byte a byte;
    - la bateria se mide en obra (`BATERIA_OBRA`), y la tarjeta de la CPU
      corre hasta 4M instrucciones por celda, como la 3060;
    - un fallo de antes, que encontro esta casilla: un `continue` o un
      `break` dentro de un `if`, con el nombre de la vuelta leido despues,
      tumbaba el compilador. Las clases del calculo y E1 leian los bloques
      en linea. Ya no, con su ejemplo (`nivel4/salta_y_sigue`);
    - el entero NO entro: DL5 sigue abierta.
  - **Como se supo:**
    - en las dos tarjetas, contra un espejo en Rust, con los bits de Rust en
      cada celda, por el simulador de cada tarjeta y por la casa:
      - una potencia;
      - una serie con sus coeficientes escritos (la de 1 / (1 - x));
      - `e` por diez cuadrados en f32;
      - `break`, `continue` y `return` dentro de dos bucles dentro de un
        `if`;
      - cuatro gpu fn que se llaman, una con un bucle dentro del bucle de
        otra;
    - las pruebas del NO, cada una en su linea y su columna. Un Programa que
      no sale (escrito a mano: TITAN++ no sabe escribirlo) es `SinFin` en la
      3060 y "no vuelve" en la CPU, nunca un cuelgue;
    - en el tope de la obra, once patrones corren en el presupuesto de cada
      simulador con margen: la 3060 necesita hasta unas 3,5 instrucciones por
      unidad, y la CPU hasta unas 12,3;
    - los bancos: el del frontend, el del escritor, el del emisor (corre cada
      `.bex`) y los dos oraculos de E1 (E1 == el calculo en 41 programas; el
      NO, con el mismo codigo y en la misma linea);
    - lo de antes, igual: los 38 `.bex` de los programas BIEN que ya
      compilaban, byte a byte; el SASS de `mezcla`, `suma` y `activa`, igual.

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
  - **08-10, HECHO EN EL ANFITRION.** La casilla sigue abierta por el metal,
    que es del propietario. DL6, tomada como la recomienda el plan (4); el
    09-10, confirmada (4.1).
  - **Lo hecho:**
    - el frontend (`gpu.rs`): la FORMA de una gpu fn por su firma (`Forma`:
      de celdas, de vertice, de pixel). Dentro de una que dibuja hay
      registros de f32 (`let`, `v.campo`, `w.campo = ...`, registros
      escritos) y todo lo de una celda. Sus NO van en su sitio (T0090): la
      de pixel que lee su `posicion`, su registro prestado, una que se
      llama, un registro en una gpu fn de celdas; y en el calculo, dos
      registros comparados con `==` (se comparan campo a campo, por IEEE);
    - el calculo (`calc::run_gpu_dibujo`): la referencia, por elementos;
    - el escritor, partido en tres ficheros sin cambiar un byte de lo de
      antes:
      - `lib.rs`: las tarjetas, sus jueces y el oraculo;
      - `escribe.rs`: la gpu fn hecha Programa;
      - `dibujo.rs`: la que dibuja.
      Un registro es un `Valor` de varias hojas, con sus `if`, sus bucles y
      sus casas hoja a hoja. La de vertice y la de pixel salen como el
      Programa por elementos y componentes (un componente que su campo no
      tiene sale 0) y pasan la bateria en las dos tarjetas, componente a
      componente;
    - la RTX 3060 12G DIBUJA (`tarjeta-rtx3060-12g/src/dibujo.rs`):
      - emite con las entradas en registros;
      - comprueba los bits contra la casa (`vivo::comprobar`);
      - pega con el pegamento de E5, sin libreta, como `pegados()` del banco
        de BMOX-12;
      - juzga como la puerta del kernel (`juzgar_programa`);
      - mete las de un paquete en su SOBRE: un BSF de SM86 con el ABI
        `SM86_V1`, un modulo por gpu fn con su nombre, y el MAPA de la casa
        de fuente (`vivo::mapa_de`: el mismo texto que el MAPA de un PSO);
    - `titan sm86` deja cada una pegada (`.sm86`) y el sobre del paquete
      (`.bsf`);
    - el ejemplo `nivel11/cubo`: el cubo de V0, con sus dos gpu fn
      (`cubo_vertice` y `cubo_pixel`).
  - **Como se supo:**
    - la tanda (`emisor-x86_64/tests/cubo.rs`): el cubo en TITAN++ da los
      bits de la tanda de `bmo_cubo` en los fotogramas 0, 30, 60 y 123. Son
      54 vertices: cada posicion de recorte bit a bit y cada color en 8
      bits, en las DOS tarjetas, en la casa y en el calculo;
    - el sobre:
      - sale IGUAL dos veces, tambien desde otra carpeta;
      - sus dos programas, PERFECTO Y PRECISO: el de vertice, 28
        instrucciones con el pegamento; el de pixel, 10;
      - cumple el contrato que el escritorio exige a `cubo.bsf` al abrir:
        `cubo_vertice` lee un bufer set 0 binding 0 de 32 bytes por vertice,
        el `Vertex` de VERRANO;
    - a mano, en las dos tarjetas:
      - un vertice que trabaja: un campo que cambia, un `if` que cambia un
        registro entero, y un bucle con `break` que llama a otra gpu fn;
      - un pixel que vuelve desde dentro de su bucle;
      - la posicion del pixel no cambia lo que pinta, ni con un NaN;
      - una casa que dice otra cosa es un NO con su entrada;
    - lo de antes, igual: los 42 `.bex` de los programas BIEN, byte a byte,
      y el SASS de las 16 gpu fn del nivel 11.
  - **Queda, del propietario (el metal):** `gpu verrano` con el `cubo.bsf`
    de TITAN++ donde hoy va el de SASS a mano, en el 0, el 30 y el 60. Lo
    fabrica `titan sm86 toolchain/lang/titan/ejemplos/nivel11/cubo/src/main.titan
    -o CARPETA`.

- [x] **LB6b -- DL10, LA DIVISION EXACTA EN LA 3060.** **HECHA en el
  anfitrion el 09-10.** La tercera salida de 3.5: los bits de IEEE tambien en
  la 3060, con la cuenta de `ptxas` para `div.rn.f32` rehecha para el cuerpo
  de una app (R7: sin FCHK, sin el CALL a su camino lento, sin guardas fuera
  de un BRA). La raiz queda (abajo).
  - **Bloquea:** DL10 (contestada el 09-10).
  - **Como se sabe:** el simulador de la 3060, sobre lo que emite su emisor,
    da los bits de `x / y` de Rust en cada clase de numero, tambien con el
    inverso del MUFU movido; el juez dice que si (R7 incluida); y TITAN++ la
    escribe, la juzga y pasa su bateria en las dos tarjetas.
  - **Lo hecho:**
    - el oro: `platform/shared/bmo-sm86/oro_reales.ptx` (la division y la
      raiz de `ptxas`, CUDA 12.9) y sus palabras en `codifica::ORO_DL10`,
      con la FFMA de redondeo dirigido (`ffma_redondeo`);
    - el simulador hace la FFMA EXACTA en sus cuatro redondeos
      (`proton-x-sm86/src/fma.rs`: el producto en f64, TwoSum y el redondeo
      a impar) y deja mover el inverso del MUFU (`Maquina::inverso_ulp`);
    - el emisor emite `Op::Div` (`proton-x-sm86/src/cociente.rs`): en la
      ventana [2^-50, 2^50), la cuenta de `ptxas` y, en vez de su ultima
      correccion, la prueba de TUCKERMAN con residuos exactos; fuera,
      DENTRO del cuerpo: NaN, infinitos y ceros como IEEE, los subnormales
      por 2^64, y el cociente que se pasa (infinito) o se queda corto
      (SUBNORMAL, con su empate al par). La cuenta es UNA para los dos
      caminos (un salto hacia atras, que drena): 105 instrucciones de
      codigo; al correr, 36 en la ventana y hasta 88 fuera;
    - la tarjeta de la RTX 3060 12G ya no dice la division como su limite;
      el escritor de TITAN++ habla de la tarjeta que no la sepa sin nombrar
      ninguna, y la de juguete (que no divide) prueba LB1 desde hoy;
    - la OBRA: la division general pesa 24 (`gpu::OBRA_DIVISION`); entre
      una potencia de dos ESCRITA, 1 -- es un producto, con el mismo
      `inverso_exacto` en el frontend y en el escritor --. Con eso la 3060
      sigue en ~3.6 instrucciones por unidad en su peor patron;
    - el banco: `nivel11/cociente` (una division general, y un bucle que
      divide); el azar de E1, con divisiones generales;
    - L30 la cumple ahora el NO del juez de la 3060 (sellado el 09-10:
      ningun texto de ley cambia).
  - **Como se supo:**
    - `pruebas_cociente.rs`: los bits de IEEE en 1,4 millones de pares
      (cada clase contra cada clase, los bordes de la ventana, cocientes que
      se pasan y que se quedan cortos, subnormales con su empate); el
      inverso del MUFU movido de -4 a +4 ULP; el juez en los dos ABI y R7;
      entre una constante y en una variable (`x = x / 7` en un bucle). La
      primera version, la de `ptxas` tal cual, la tumbo esta prueba
      (`0x80000001 / 0x1fffffff` con el inverso a -4 ULP);
    - `fma.rs`: la FFMA al mas cercano contra `mul_add`, y los tres
      dirigidos contra el valor exacto contado con enteros;
    - TITAN++: la division en las dos tarjetas con los bits de Rust (en un
      bucle, en una llamada), por el oraculo de `titan build`, y el azar de
      E1 contra el oraculo (120 programas, y 600 con otra semilla);
    - lo de antes, igual: los 43 `.bex` de los programas BIEN, byte a byte,
      y lo que `titan sm86` deja del nivel 11 (21 ficheros, el cubo
      incluido).
  - **Queda:**
    - **la raiz** (`Sqrt`, `Rsqrt`), con el propietario: hoy es MUFU, y el
      simulador la aproxima como la 3060. Hacerla exacta cambia el SASS de
      los sombreadores de BMOX-12 que ya corren en el metal;
    - **la medida**: una division son 105 instrucciones, y un hueco de la
      tuberia de VERRANO lleva 128 con su pegamento: en una gpu fn que
      DIBUJA casi no cabe nada mas (lo dice el pegamento). Un camino lento
      compartido entre divisiones, o un hueco mas grande, es del
      propietario;
    - **el metal**: que la 3060 de verdad de los mismos bits (la cuenta
      aguanta el error de su MUFU; verlo es de LB8).

- [x] **LB7a -- EL CUBO CONTADO POR GPU FN, BIT A BIT.** **HECHA en el
  anfitrion el 09-10.** La mitad de LB7 que no pide ni permiso ni lamina:
  las cuentas de `bmo_cubo` escritas en TITAN++ como gpu fn, y con ellas los
  360 fotogramas, con los bits de `bmo_cubo`. Es lo que DL10 dejo hacer.
  - **Bloquea:** DL10 (hecha: LB6b).
  - **Como se sabe:** `toolchain/lang/titan/emisor-x86_64/tests/cubo_gira.rs`:
    cada cuenta, por cada tarjeta que da `titan` (la RTX 3060 12G simulada y
    la CPU), por la casa y por el calculo -- los cuatro iguales --, y lo que
    dan es lo de `bmo_cubo`, bit a bit.
  - **Lo hecho:**
    - el paquete `toolchain/lang/titan/ejemplos/nivel11/cubo_gira`: tres
      modulos y 22 gpu fn.
      - `num`: truncar, los dos redondeos (al lejano y al par), saturar, la
        vuelta reducida, el seno, el coseno y la tangente de `bmo_cubo::num`
        con sus mismos polinomios, y la raiz: la estimacion que alli sale de
        los BITS del f32, contada aqui con f32 exactos (potencias de dos,
        sin mirar bits), y sus cuatro pasos de Newton;
      - `cubo`: la entrada `k` de la matriz del fotograma `f` (`wvp`), un
        vertice a recorte, su subpixel (`pantalla_x`, `pantalla_y`, al par),
        si esta detras, y la luz y el color de una cara;
    - el emisor de la 3060, FRUGAL (`proton-x-sm86/src/lib.rs` y
      `saltos.rs`): si un cuerpo no cabe en sus registros, se emite otra vez
      sin dejar en registro lo que sube (una constante, una entrada del
      banco: se sube cada vez que se lee), con cada variable naciendo justo
      antes de lo primero que la toca, y soltando lo que nadie lee en cuanto
      se escribe. `cubo.wvp` pasa de 78 registros, que no caben en los 64 de
      VERRANO, a 44 (7423 instrucciones). Lo que ya cabia sale igual, byte a
      byte;
    - la CPU como tarjeta (`tarjeta-cpu/src/compacta.rs`): si un Programa
      tiene mas valores que los 1024 sitios de su pila, los que mueren dejan
      su sitio a otros. `cubo.wvp`: 1497 valores en 87 sitios;
    - el banco: `cubo_gira` imprime el seno y el coseno de 30 y 90 grados, la
      diagonal de la matriz del fotograma 0 y la cara verde con su luz.
  - **Como se supo:**
    - las cuentas de `num` contra `bmo_cubo::num`: 5223 valores (los
      angulos del cubo, un barrido, potencias de dos con sus vecinos, ceros,
      subnormales, enormes, infinitos y NaN), y la raiz en 4000 mas al azar;
    - las 16 entradas de la `wvp` de los 360 fotogramas;
    - la tanda de los 360: cada vertice a recorte, las caras que miran (la
      cuenta de `tanda::de_fotograma`, con los subpixeles de las gpu fn) y el
      color de cada una;
    - el modo frugal FORZADO en los 400 programas al azar de E6, en los dos
      ABI: los bits de la casa, y el juez dice que si. Su primera version la
      tumbo esa misma bateria (soltaba la precarga de una fila del cbuffer
      que nadie leia cuando su registro ya era de una variable);
    - lo de antes, igual: los 44 `.bex` de los programas BIEN, byte a byte,
      y lo que `titan sm86` deja del nivel 11 (23 ficheros).
  - **Queda:**
    - **LB7b, la app** (abajo, en LB7): el permiso `screen` (DL7), la lamina
      en E1 y los 360 publicados en ella;
    - **lo que mide**: `cubo.wvp` son 7423 instrucciones y `cubo.luz` 5097:
      caben en un cuerpo de computo, no en un hueco de la tuberia (128). Una
      division entre dos constantes ESCRITAS (`6.2831855 / 360.0`) aun se
      cuenta en la 3060 (105 instrucciones) en vez de plegarse al compilar;
    - **el NO de lo que no cabe**: la 3060 dice "no cabe en sus registros"
      como un fallo del compilador; con el modo frugal, lo que aun no cabe es
      un limite de la tarjeta, y deberia decirse asi, en su sitio (LB1).

- [ ] **LB7 -- TITAN++ MANDA A VERRANO.** Una app de TITAN++ que dibuja el
  cubo girando, por el camino de DL8: el permiso (DL7) y su puerta en el
  certificado; en E1, el `f32` como DATO -- guardar, pasar y copiar a un
  bloque, lo que la ley L26 ya deja --; y los 360 fotogramas contados por gpu
  fn: en la 3060 simulada AL COMPILAR (eso es hornear), o al correr por LB4.
  Es el relevo que pide el corte 4 de INTI_SAMURAI (`cubo.inti` ->
  `cubo.titan`).
  - **Bloquea:** LB2 (la lamina vista), LB4 o el horneado, LB6 (si va por la
    puerta) y **DL10**: sin ella no se cuenta el cubo de `bmo_cubo` bit a bit
    (3.5). **09-10:** la division, hecha (LB6b).
  - **09-10:** lo CONTADO, hecho en el anfitrion (LB7a): los 360 fotogramas
    por gpu fn, con los bits de `bmo_cubo`. Lo que queda es la app (LB7b):
    el permiso, la lamina y el metal.
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
  - **08-10, el propietario -- RETIRADA:** *"Solo uno, un GPU que haremos
    porque la verdad no tengo otros GPU y seria jugar a adivinanza"*. No se
    hace mientras no haya otra GPU en la maquina; el contrato de dos ya lo
    probo la CPU (LB4).

---

# 6. EL ORDEN

```text
   ahora, en el anfitrion   LB1 -> LB3 -> LB4 -> LB5 -> LB6 -> LB6b -> LB7a -> LB7
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
  - **08-10, LB3b:** asi fue. Lo unico roto se vio al compilar (`computo.rs`,
    partido en dos, se quedo sin su `vec!` y su `Vec`), y ninguna prueba
    cambio.
- **La CPU de reserva puede ser lenta.** Es la reserva, no el camino: el metro
  (`toolchain/tools/metro`) dira cuanto, y lo que la 3060 pague se mide contra
  ella.
- **Un bucle puede colgar la 3060** el dia que algo llegue al metal: por eso
  DL4 empieza por `range` con N escrito, y LB8 pide E7 antes.
  - **08-10, LB5:** desde TITAN++ ya no puede. Con el N escrito, la obra de
    cada hilo se sabe al compilar y tiene su tope (L33).
- **El cubo bit a bit pide DL10.** Sin una division exacta en algun sitio, el
  cubo de TITAN++ no es el de `bmo_cubo`, y el juez de VERRANO lo diria.
  - **09-10, LB6b:** la division exacta ya esta, en la 3060 y en la CPU.
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
