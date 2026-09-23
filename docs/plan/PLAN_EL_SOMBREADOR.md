# PLAN EL SOMBREADOR -- SPIR-V a x86-64, en el anfitrion y en el Ryzen

> Escrito el **2026-09-23**, el mismo dia en que `c/sello.bex` devolvio 42 desde
> un bloque SELLADO en el Ryzen y murio escribiendo en el, que era lo correcto.
> Mismo formato que `PLAN_EXPRIMIR_EL_DISCO.md`: casillas ordenadas, cada una con
> **que la bloquea** y **como se sabe que quedo hecha**.
>
> El propietario: *"con VULKAN eso es TODO frontend y SPIR-V ese mismo... podriamos
> probar aqui en SPIR-V para madurar en BMO-X?"*. Y el nombre de la API lo puso
> el: **VERRANO**, para no pisar una marca.

Es la **pieza 2 de la ruta B1** de
[`PLAN_VULKAN.md`](../../platform/drivers/gpu/rdna4/PLAN_VULKAN.md) (Vulkan por
software). La pieza 3, las paginas ejecutables, quedo hecha hoy: `MEM_OP_SELLAR`.

---

# 0. LO QUE ESTE PLAN NO ES, dicho antes que lo que es

- **No es VERRANO.** La API (instance, device, queue, pipeline, swapchain) es la
  pieza 5 de B1 y no empieza aqui.
- **No es el rasterizador.** Vertices, fragmentos, triangulos y z-buffer son la
  pieza 4. Aqui solo entra la etapa de **COMPUTO** (`GLCompute`): un programa que
  lee buffers y escribe buffers, sin pantalla. Es la unica etapa que se puede
  juzgar entera sin rasterizador, y el rasterizador propio
  (`platform/shared/bmo-dibujo`) ya esta esperando como oraculo para
  cuando lleguen las otras dos.
- **No es la GPU.** Todo corre en el Ryzen. La RX 9060 XT es B2.
- **No sirve a la banca ni a Ada.** Hay que decirlo: es la rama de la hoja de
  ruta que da *algo que se ve moverse*, no la del objetivo principal.

★ **Lo que SI da hoy, aunque no haya ni un juego:** es el **primer cliente de
AVX2**. El `save` de las 08:41 lo dice en su capitulo 2 -- *"AVX2: emisor listo,
sin clientes"*. Un sombreador de computo es exactamente el trabajo que AVX2
existe para hacer: la misma operacion sobre ocho invocaciones a la vez.

---

# 1. TRES DECISIONES ANTES DE LA PRIMERA LINEA

## 1a. [ABIERTA -- la decide el propietario] EL SITIO: la lista de lenguajes esta CERRADA

`toolchain/lang/README.md` cerro la lista el **2026-09-17**. Y la regla de
una arquitectura dice que lo unico agnostico
son los FRONTENDS: *lenguaje = frontend + `emisor-x86_64/`*. SPIR-V tiene
exactamente esa forma, asi que el choque es real:

| Opcion | Contra |
|---|---|
| **`toolchain/lang/spirv/`** + `emisor-x86_64/` | reabre una lista cerrada. A favor: SPIR-V no es un lenguaje que alguien ESCRIBA, es un formato que se RECIBE (lo emiten glslang, DXC, Naga, rust-gpu); la lista cerro lo que se escribe |
| `platform/drivers/gpu/verrano/sombreador/` | junto a quien lo usa, pero `platform/` es de Ring 0 y de aparatos, y esto es una herramienta de Ring 3 y del anfitrion |
| `toolchain/forge/` | no: forge comparte CONTRATOS, nunca cerebros. Un traductor es un cerebro |

**Recomendacion: `toolchain/lang/spirv/`, con una fila en el README que diga que
entra como formato recibido y no como lenguaje.** Pero reabrir una lista cerrada
no lo decide un plan.

## 1b. AOT primero, JIT despues -- y el MISMO codigo para los dos

El propio `PLAN_VULKAN` lo dejo escrito en el sobre BSF: *"BMO-X esta en
condiciones de consola, no de PC. Una maquina, una GPU conocida, un sistema
operativo. El objetivo se sabe al compilar."* Las consolas precompilan en el
estudio y no compilan nada al arrancar.

Asi que el camino normal es **AOT**: el `.spv` se traduce en el anfitrion y el
`.bex` lleva el codigo ya hecho (casilla S6). El **JIT** --traducir en el Ryzen
y SELLAR-- es para lo que llegue sin precompilar, y para demostrar que `SELLAR`
sirve para lo que se hizo (casilla S5).

★ **Y eso impone una regla desde la casilla S1**: el lector, el juez y el emisor
se escriben `#![no_std]`, **sin `alloc`**, con la memoria que da quien llama.
Es lo que permite que el MISMO codigo corra en el anfitrion (AOT, la prueba) y
dentro de una app de BMO-X (JIT). Si S1 se escribe con `Vec` "porque en el
anfitrion hay", S5 se convierte en reescribirlo.

## 1c. Las pruebas: el SDK de Vulkan del anfitrion, fuera del arbol de enlace

El anfitrion tiene el Vulkan SDK 1.4.350 (`glslc`, `glslangValidator`,
`spirv-as`, `spirv-dis`, `dxc`). Se usa **solo para fabricar los `.spv` de
prueba**: cada prueba guarda su GLSL fuente **y** el `.spv` fabricado, los dos en
el repo, y un script los regenera. El banco **no necesita el SDK para correr**, y
nada de el se enlaza -- la misma regla que los cero crates de terceros del
kernel.

---

# 2. EL SUBCONJUNTO -- y lo que no entra se rechaza CON MOTIVO

Una tabla propia del subconjunto, no la gramatica entera de Khronos
(`spirv.core.grammar.json`): lo mismo que `intrinsics.toml`, **una fila por
instruccion que se sabe traducir**, y lo que no tiene fila se niega nombrandola.

| Que | Entra |
|---|---|
| Version | SPIR-V 1.0 a 1.6 en la cabecera; se lee la que sea, se usa el subconjunto |
| Capacidades | `Shader`. Cualquier otra -> NO, con su nombre |
| Modelo | `Logical` + `GLSL450`. `Physical*` no (punteros de verdad en un sombreador) |
| Etapa | `GLCompute` con `LocalSize` fijo. Vertex/Fragment: NO en este plan |
| Tipos | `void`, `bool`, `int` 32 con y sin signo, `float` 32, vectores 2-4, arrays de medida fija, structs, punteros `StorageBuffer`/`Uniform`/`Function`/`Input`(solo los `BuiltIn`) |
| Instrucciones | aritmetica entera y flotante, comparaciones, logicas, conversiones, `OpLoad`/`OpStore`/`OpAccessChain`, `OpCompositeConstruct/Extract`, `OpVectorShuffle`, `OpSelect`, control estructurado (`OpSelectionMerge`, `OpLoopMerge`, `OpBranch`, `OpBranchConditional`, `OpPhi`, `OpReturn`), `OpFunctionCall` |
| `GLSL.std.450` | `FAbs SAbs Floor Ceil Fract Sqrt InverseSqrt FMin FMax UMin UMax SMin SMax FClamp Mix Step Fma` -- las que son UNA o dos instrucciones de SSE. `Sin/Cos/Exp/Log/Pow` esperan a la casilla S3b |
| Fuera | imagenes, samplers, atomicos, barreras de grupo (`OpControlBarrier`), memoria compartida `Workgroup`, `float64`, `int8/16/64` -- cada uno con su motivo |

**Las barreras y la memoria `Workgroup` estan fuera a proposito**: son las que
piden varias invocaciones corriendo A LA VEZ, y eso es la pieza 6 (hilos), que
tiene cero de 51 operaciones hoy.

---

# 3. LAS CASILLAS

## [ ] S0 -- EL SITIO

- **Bloquea:** la decision 1a, del propietario.
- **Hecha cuando:** existe la carpeta con su README (que es, que NO es, y el
  subconjunto de la seccion 2), el guardian `isa` y la regla de disposicion la
  aceptan, y `AMBITOS.txt` tiene su ambito de commit.

## [ ] S1 -- EL LECTOR: bytes de SPIR-V a un modulo

La cabecera (`0x07230203`, version, generador, `bound`, esquema) y el flujo de
instrucciones (`palabras << 16 | codigo`). De ahi sale el modulo: capacidades,
importaciones, modelo, puntos de entrada, modos, nombres de depuracion,
decoraciones, tipos, constantes, globales y funciones.

- **Bloquea:** S0.
- ★ **Son bytes de un TERCERO.** Mismo trato que `bmo-hostile`: una instruccion
  de 0 palabras, una que se sale del fichero, un id por encima de `bound`, una
  cadena sin su cero, un fichero en big-endian (legal en SPIR-V, pero nada de
  lo que BMO-X recibe lo emite) -- cada uno es un NO con motivo,
  **nunca un panico**.
- **Hecha cuando:** los `.spv` de prueba se leen enteros, y una fila por cada
  forma de romper la cabecera y el flujo dice su motivo.

## [ ] S2 -- EL JUEZ: el subconjunto, o por que no

- **Bloquea:** S1.
- Capacidad, modelo, etapa, tipo o instruccion fuera de la tabla -> NO con su
  nombre. Ids usados antes de definirse (salvo donde SPIR-V lo permite: `OpPhi`
  y los saltos), tipos que no cuadran, control de flujo sin estructura.
- **Hecha cuando:** una fila por motivo, y un sombreador de verdad fabricado por
  `glslc` que use algo de fuera (una imagen, un atomico) sale negado con la
  palabra correcta.

## [ ] S3 -- EL ORACULO: un interprete en el anfitrion

Ejecuta un sombreador de computo sobre buffers, invocacion a invocacion. Es
lento a proposito: es la **definicion** de lo que el sombreador hace, y todo lo
de despues se juzga contra el.

- **Bloquea:** S2.
- **Hecha cuando:** cuatro sombreadores escritos en GLSL -- suma de vectores,
  `saxpy`, un Mandelbrot por pixel (bucles y flotantes) y una tabla de colores
  con `OpSelect`/`OpPhi` -- dan lo mismo que el mismo calculo escrito a mano en
  Rust.

### [ ] S3b -- las funciones que no son una instruccion

`Sin Cos Exp Log Pow` no existen en SSE. Se escriben **una vez**, en un sitio, y
las usan el oraculo Y el emisor -- si cada uno tuviera la suya, S4 dejaria de
poder compararse bit a bit con S3.

## [ ] S4 -- EL EMISOR x86-64, escalar

Cada invocacion es una llamada: `fn(id_global, buffers)`. Flotantes en SSE
escalar, enteros en los registros generales, con el ensamblador propio
(`sem-asm`) y sin una sola dependencia nueva.

- **Bloquea:** S3.
- ★ **La prueba es DIFERENCIAL**: el codigo emitido corre en el emulador de
  `bmo-lower` (el mismo que juzga a C) y tiene que dar **los mismos bits** que el
  oraculo. Por eso el emisor **no fusiona** `a*b+c` en un FMA por su cuenta: el
  redondeo cambia y la comparacion dejaria de ser exacta. `Fma` solo cuando el
  sombreador lo pide.
- **Hecha cuando:** los cuatro de S3, y cada fila de instruccion de la tabla,
  dan igual en el oraculo y en el emulado.

## [ ] S5 -- EN EL RYZEN: el JIT, y el primer uso de verdad de SELLAR

Una app de Ring 3 que lleva un `.spv`, lo traduce **en la maquina**, lo escribe
en un bloque de `bmo_codigo_pedir`, lo SELLA y lo ejecuta sobre sus buffers.

- **Bloquea:** S4, y que el lector/juez/emisor sean `no_std` sin `alloc` (1b).
- **Hecha cuando:** en el Ryzen sale lo mismo que en el anfitrion, con **tres
  numeros medidos**: lo que tardo en traducir, lo que tarda una pasada, y lo que
  tarda la misma pasada escrita a mano. Sin esos numeros S7 no se toca.

## [ ] S6 -- EL SOBRE: el codigo ya hecho viaja dentro del `.bex`

El BSF de `PLAN_VULKAN`: la seccion `Shaders = 0x0A` (reservada desde hace
tiempo) con el SPIR-V, su BLAKE3 y el x86-64 **ya traducido en el anfitrion**.
El kernel la salta, como toda seccion que no conoce.

- **Bloquea:** S4. No depende de S5.
- **Hecha cuando:** `bmo-pack` la escribe, la app la lee, la firma del indice la
  cubre, y una app lanza su sombreador sin traducir nada al arrancar.

## [ ] S7 -- LOS CARRILES: 4 u 8 invocaciones por instruccion

SSE lleva 4 flotantes, AVX2 lleva 8. Traducir cada operacion del sombreador a UNA
operacion sobre 8 invocaciones es para lo que AVX2 existe.

- **Bloquea:** los numeros de S5. Por las
  reglas de optimizacion, esto es **lo ultimo**: si S5
  dice que el tiempo se va en otra parte, S7 espera.
- El control de flujo divergente (unas invocaciones entran al `if` y otras no)
  se lleva con mascaras. Es la parte dificil y se dice ahora.
- **Hecha cuando:** mismo resultado bit a bit que el escalar, y el factor
  medido en el Ryzen.

---

# 4. EL ORDEN Y LO QUE CUESTA

S0 -> S1 -> S2 -> S3 -> S4, en fila. Despues S5 y S6 son independientes, y S7
espera a S5.

**No se dan semanas ni meses.** Por la LEY 24 una estimacion generica es la
estimacion de otro proyecto. Lo que se puede contar son piezas: ocho casillas,
dos de ellas grandes (S4 y S7), **ninguna pide nada que la maquina no tenga
hoy**: SELLAR se vio en el Ryzen hoy a las 08:41, el emulador ya juzga a C, y AVX2
esta en el silicio sin nadie que lo use.
