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

## 1a. [DECIDIDA el 23-09: `lang/spirv/`, formato RECIBIDO] EL SITIO: la lista de lenguajes esta CERRADA

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

## [x] S0 -- EL SITIO

> **Hecho el 23-09.** El propietario: *"si, lang/spirv como formato recibido"*.
> `toolchain/lang/spirv/` (crate `bmo-spirv-front`), fila en el README de
> `lang/` que dice por que no reabre la lista, `isa` lo cuenta como frontend y
> `AMBITOS.txt` tiene `spirv`.

- **Bloquea:** la decision 1a, del propietario.
- **Hecha cuando:** existe la carpeta con su README (que es, que NO es, y el
  subconjunto de la seccion 2), el guardian `isa` y la regla de disposicion la
  aceptan, y `AMBITOS.txt` tiene su ambito de commit.

## [x] S1 -- EL LECTOR: bytes de SPIR-V a un modulo

> **Hecho el 23-09**, sin `alloc`: `read(bytes, ids) -> Module | Error` (nacio
> como `leer`; la API paso al ingles el mismo dia). La tabla la genera
> `herramientas/table.py` desde la
> gramatica NORMATIVA de Khronos (MIT) -- los numeros son el contrato, las
> filas y las familias son el estudio -- y `--cotejar` dice si se desvio. 23
> motivos con su fila; los cinco `.spv` se leen enteros; cortar cada fichero
> en cada palabra y voltear cada byte nunca da panico. 33 pruebas.

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

## [x] S2 -- EL JUEZ: el subconjunto, o por que no

> **Hecho el 23-09**, `no_std` sin `alloc`: `validate(&Module) -> Verdict |
> Error` (nacio como `juzgar`) en una pasada, preguntando los tipos a la tabla de ids del lector.
> 37 motivos nuevos, una fila cada uno; `fuera.spv` cae en lo primero que
> aparece (la memoria `Workgroup`) y `census` nombra las tres familias.
>
> ** Y LA MATRIZ: el banco de pruebas de NAGA (wgpu, MIT/Apache), clonado
> FUERA del repo (`BMO-externo/naga-corpus`) y fabricado con tres
> herramientas del anfitrion (`spirv-as`, `naga`, `glslc`):
>
> | | |
> |---|---|
> | fabricados | 228 (`naga` nego 14 WGSL y `glslc` 6 GLSL: son de otros backends) |
> | se leen | **228** (eran 194 con la tabla corta: ahora tiene las 871 de la gramatica, familia `Otro`) |
> | traen entrada de computo | 111 |
> | **caben** | **28** |
> | lo que mas falta en los de computo | **matrices 16**, modelo de memoria de Vulkan (cap 5345) 8, memoria `Workgroup` 8, `Float64` 5, `Int64` 4 |
>
> ** Ni un NO del juez en esos 228 es "los tipos no cuadran", "uso antes de
> definir" o "salto sin estructura": todo lo que cae, cae por estar FUERA del
> subconjunto. Con tres fabricantes distintos, el juez no rechaza SPIR-V
> valido por un error suyo. Lo que NO mide: si acepta SPIR-V invalido --
> el corpus solo trae validos, y eso lo cuidan las filas de `tests/validator.rs`.
>
> `py toolchain/lang/spirv/herramientas/censo_naga.py` lo repite.

- **Bloquea:** S1.
- Capacidad, modelo, etapa, tipo o instruccion fuera de la tabla -> NO con su
  nombre. Ids usados antes de definirse (salvo donde SPIR-V lo permite: `OpPhi`
  y los saltos), tipos que no cuadran, control de flujo sin estructura.
- **Hecha cuando:** una fila por motivo, y un sombreador de verdad fabricado por
  `glslc` que use algo de fuera (una imagen, un atomico) sale negado con la
  palabra correcta.

## [x] S3 -- EL ORACULO: un interprete en el anfitrion

> **Hecho el 23-09**, `no_std` sin `alloc`: `Interpreter::new(&Module,
> workspace)` + `dispatch(groups, buffers, fuel) -> Stats | Trap`. La memoria
> la da quien llama (`workspace_words` dice cuanta): un sitio FIJO por id,
> porque SPIR-V prohibe la recursion. Punteros de dos palabras (arena o
> buffer); en un buffer manda la disposicion de sus decoraciones (std140,
> std430).
>
> - `math` es LA DEFINICION de la aritmetica (`sqrt`, `floor`, `ceil`,
>   `fract`, `fma` con un solo redondeo por "redondeo a impar"...): comparada
>   bit a bit con la biblioteca estandar en millones de valores. Y cierra lo
>   que SPIR-V deja abierto: `min`/`max` con NaN dan el segundo, `mix` sin
>   fusionar, flotante a entero satura. El emisor (S4) tiene que igualarlo.
> - Lo indefinido PARA con su motivo, la palabra y la invocacion: division
>   por cero, `INT_MIN / -1`, desplazar 32 o mas, salirse de un buffer, un
>   buffer que falta, poco combustible, y **un valor que la invocacion no
>   definio** -- que es la dominancia que el juez no mira, cazada en marcha
>   (`indefinido.spvasm`, escrito a mano: la invocacion 1 para, la 0 no).
> - Los cuatro de S3 dan lo MISMO que Rust bit a bit (suma, saxpy con su
>   limite, mandelbrot 32x32 pixel a pixel, colores con su funcion), y
>   `collatz` da 111/118/178 para 27/97/871.
> - La matriz de Naga: el oraculo EJECUTA los 28 que caben; uno para por
>   combustible con razon (Collatz desde 0, con los buffers a cero).

> ** Desde el 23-09 la API publica de `lang/spirv` es INGLES (decision del
> propietario, pensando en motores y juegos de fuera); comentarios y textos de
> pantalla, castellano. El oraculo nace ya asi.

> La matriz de S2 ya dice por donde crecer DESPUES de S3: las matrices son lo
> primero que falta en el computo real (16 de los 83 que no caben).

Ejecuta un sombreador de computo sobre buffers, invocacion a invocacion. Es
lento a proposito: es la **definicion** de lo que el sombreador hace, y todo lo
de despues se juzga contra el.

- **Bloquea:** S2.
- **Hecha cuando:** cuatro sombreadores escritos en GLSL -- suma de vectores,
  `saxpy`, un Mandelbrot por pixel (bucles y flotantes) y una tabla de colores
  con `OpSelect`/`OpPhi` -- dan lo mismo que el mismo calculo escrito a mano en
  Rust.

### [x] S3b -- las funciones que no son una instruccion

> **Hecho el 23-09** en `src/math.rs`: `sin`, `cos`, `exp`, `log`, `pow`,
> calculadas en DOBLE (reduccion de rango de fdlibm con pi/2 y ln2 en dos
> trozos + polinomio, sin fusionar) y redondeadas a f32 al final.
> Deterministas por construccion: el emisor repite las mismas operaciones de
> `f64` en SSE2. **A un ULP como mucho** del valor verdadero en un millon de
> casos cada una (Vulkan pide 2^-11 absoluto en seno/coseno y 3 ULP en
> exp/log). El juez ya las acepta; el oraculo las ejecuta (`trig.comp`).

`Sin Cos Exp Log Pow` no existen en SSE. Se escriben **una vez**, en un sitio, y
las usan el oraculo Y el emisor -- si cada uno tuviera la suya, S4 dejaria de
poder compararse bit a bit con S3.

## [x] S4 -- EL EMISOR x86-64, escalar

> **Hecho el 23-09** en `toolchain/lang/spirv/emisor-x86_64/` (crate
> `bmo-spirv-x86-64`), `no_std` sin `alloc`: `emit(&Module, tablas, codigo)
> -> Program`. Dos pasadas (la primera cuenta y fija las etiquetas, la
> segunda escribe) para no necesitar una lista de parches. Codificador
> propio: solo las formas que el emulador sabe ejecutar.
>
> - La forma: cada valor en su sitio fijo del MARCO (`[rdi + 4*slot]`),
>   carga-calcula-guarda. Lento y correcto; la velocidad se mide DESPUES.
>   `init` (constantes y punteros, una vez) y `main(marco, buffers, ids,
>   combustible) -> eax` (una vez por invocacion).
> - Para igual que el oraculo: division por cero, `INT_MIN / -1`, desplazar
>   32 o mas, salirse de un buffer o de un arreglo, `OpUnreachable`,
>   combustible (por salto hacia atras). NO vigila la dominancia: eso lo
>   dice el oraculo.
> - `math` igualada: `minss`/`maxss` son exactamente `math::min/max`,
>   saturar al convertir, `trunc`/`floor`/`ceil` sobre los bits, `fma` con
>   el mismo redondeo a impar en doble.
> - **La diferencial**: suma, saxpy, mandelbrot, colores, collatz, las
>   trampas -- los mismos bits que el oraculo y la misma parada en la misma
>   invocacion. Y los **28 de Naga que caben: 28 iguales** (uno para por
>   combustible en los dos).
> - Lo cazo la diferencial: los punteros de las variables DE FUNCION no se
>   escribian (apuntaban a la palabra 0 del marco).
> - Y destapo DOS fallos del EMULADOR de `bmo-lower`, arreglados alli: no
>   sabia aritmetica `float` (`addss`...: ahora en `emu/sse.rs`, con todo el
>   SSE escalar), y calculaba las BANDERAS siempre sobre 64 bits --
>   `cmp eax, 0x80000000` decia "distinto" donde el silicio dice "igual".
>
> ### [x] S4b -- las trascendentes en el emisor
>
> **Hecho el 23-09.** Tres rutinas en doble (`sincos`, `exp`, `ln`) al
> principio del codigo, solo si el modulo las usa; `pow` = `exp(y ln x)`.
> Son `math` operacion por operacion, y para que no puedan separarse las
> constantes y coeficientes salieron de `math.rs` a una TABLA publica,
> `math::table`, que leen los DOS (huella de 10 millones de resultados
> identica antes y despues de la mudanza). Repetido a mano lo que cambia
> bits: el cuadrante con la saturacion de `k as i64` de Rust, `k / 2`
> truncando, los NaN constantes. La diferencial: `trig` y los casos raros
> (NaN, infinitos, +-0, subnormales, angulos de 1e20 y 3e38, 100 patrones
> al azar) -- los mismos bits a la primera.

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

### Las tres puertas (23-09)

Vulkan (GLSL con `glslc`), OpenGL (el mismo GLSL con `--target-env=opengl`)
y DirectX (HLSL con `dxc -spirv`, en el mismo SDK) llegan al MISMO SPIR-V.
`tres_apis.rs`: suma, saxpy, mandelbrot y las trascendentes por las tres,
los mismos bits que el oraculo y entre si. La tabla, en el README de
`lang/spirv`. Un camino, no tres.

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
