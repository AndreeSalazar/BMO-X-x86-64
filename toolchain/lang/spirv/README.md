# `lang/spirv/` -- SPIR-V, el formato RECIBIDO

> Entro el **2026-09-23**, con la lista de lenguajes CERRADA desde el 17-09, y
> por una puerta distinta: **nadie escribe SPIR-V**. Lo emiten glslang, DXC,
> Naga, rust-gpu y los motores de juego, y BMO-X lo **recibe**. La lista cerro
> lo que se escribe; esto es lo que llega. Decision del propietario.
>
> El plan: [`docs/plan/PLAN_EL_SOMBREADOR.md`](../../../docs/plan/PLAN_EL_SOMBREADOR.md).
> Para que sirve: la pieza 2 de la ruta B1 de
> [`PLAN_VULKAN.md`](../../../platform/drivers/gpu/rdna4/PLAN_VULKAN.md) --
> VERRANO, Vulkan por software.

## Que se copia de Khronos y que no

SPIR-V **no se forkea: es una especificacion, no un programa.**

| De Khronos | Como |
|---|---|
| **los NUMEROS** (que `OpIAdd` es 128, que lleva tipo y resultado, cuantas palabras minimas) | se toman de `spirv.core.grammar.json` (licencia MIT, la fuente normativa de la seccion binaria de la especificacion) con [`herramientas/table.py`](herramientas/table.py). Son el contrato, igual que los del ABI: inventarlos seria no hablar SPIR-V |
| **de que FAMILIA es cada instruccion** (cual es el nucleo, cual imagen, atomico...) | lo decide `FILAS` en ese script: es el estudio, y es nuestro. Las que no nombra entran como `Otro`: el lector las recorre y el juez las niega por su nombre |
| **el codigo** de SPIRV-Tools, SPIRV-Cross, Mesa, Naga | **no se enlaza ni se copia.** Se LEE para aprender reglas, como OBS para LA MESA |
| **el banco de pruebas de Naga** (wgpu, MIT/Apache) | es la MATRIZ, como ACATS para Ada: se clona ralo FUERA del repo (`BMO-externo/naga-corpus`) y `herramientas/censo_naga.py` mide contra el. No entra ni un fichero |
| **el SDK de Vulkan** del anfitrion | solo fabrica los `.spv` de prueba ([`herramientas/fabricar.py`](herramientas/fabricar.py)); el banco no lo necesita |

## Lo que hay

| Pieza | Casilla | Estado |
|---|---|---|
| `src/table/` -- `rows.rs`, `op.rs`, `glsl.rs` | S1 | las 871 instrucciones de la gramatica + 24 de `GLSL.std.450`, generadas y cotejables (`table.py --cotejar`); partidas por oficio porque juntas pasaban de las 1.000 lineas de L6a |
| `src/reader.rs` -- `read(bytes, ids) -> Module` | S1 | hecho: cabecera, medidas, ids, cadenas, orden de secciones, funciones |
| `src/reason.rs` -- `Reason`: por que NO | S1 | cada motivo con su fila en `tests/reader.rs` y `tests/validator.rs` |
| `src/validator.rs` -- `validate(&Module) -> Verdict` y `census` | S2 | hecho: tipos que cuadran, valores antes de usarse, bloques y saltos con estructura; `censo` por familias |
| `examples/census.rs` + `herramientas/censo_naga.py` -- la matriz | S2 | contra el banco de Naga: 228 se leen, 111 de computo, **28 caben** |
| `src/interpreter/` -- `Interpreter`: el oraculo (`mod.rs` despacha y ejecuta, `values.rs` las instrucciones de valor, `buffers.rs` la disposicion std140/std430) | S3 | hecho: ejecuta el computo invocacion a invocacion; lo indefinido PARA con su motivo (`Trap`) |
| `src/math.rs` -- la aritmetica | S3, S3b | la DEFINICION: `sqrt`, `floor`, `fma`... bit a bit contra la biblioteca estandar; `sin`, `cos`, `exp`, `log`, `pow` en doble, a un ULP; el emisor tendra que igualarla |
| `emisor-x86_64/` (crate `bmo-spirv-x86-64`) -- el emisor | S4, S4b | hecho, escalar: los mismos bits que el oraculo en su banco y en los 28 de Naga que caben; las trascendentes en doble, leyendo la MISMA tabla que `math` (`math::table`) |
| `src/interface.rs` -- `interface(&Module) -> Interface` | S6 | los buffers que el modulo TOCA: `set`, `binding`, clase, lo que el codigo hace con cada uno (lee/escribe, seguido hasta su variable) y su forma (bytes fijos + paso) |
| `bsf/` (crate `bmo-bsf`) -- el BSF, BMO Format Shader | S6 | hecho: SPIR-V + interfaz + el x86-64 ya traducido, en el anexo `0x09` del `.bex`; cinco capas, ningun bit cambia sin que se note; `bmo-bsf fabricar` / `ver` |

## La API habla ingles; la casa, castellano

** Decision del propietario (2026-09-23): *"mantener la esencia en ingles el
shader... tipico si voy a meter juegos Triple A"*. Lo que llama un motor o un
juego va en INGLES (`read`, `validate`, `census`, `Module`, `Instruction`,
`Reason`, `Verdict`...), y los nombres de la especificacion van tal cual
(`OpIAdd`, `GLCompute`, `Workgroup`). Los COMENTARIOS y los textos que salen en
pantalla (`Reason::name`, el `Display` de `Error`) siguen en castellano, que es
la regla de toda la casa.

## Las dos reglas

1. **`no_std` y sin `alloc`** (la biblioteca; `examples/census.rs` es una
   herramienta del anfitrion que la usa desde fuera). La memoria --la tabla de
   ids-- la da quien llama. Es lo que permite que el MISMO lector corra en el anfitrion (AOT) y
   dentro de una app de BMO-X (JIT). Por eso no hay `Vec` aqui dentro.
2. **Los bytes son de un tercero.** Ningun `.spv` puede hacer que el lector
   entre en panico: el banco corta cada fichero en cada palabra y voltea cada
   byte de tres de ellos.

## Las TRES puertas: Vulkan, OpenGL y DirectX, un solo x86-64

El propietario, el 23-09: *"OpenGL, DirectX y Vulkan, TODO esos 3 elementos
para dejar listo ... nivel atomo en x86-64 puro, para no tener choques ... mas
sencillo asi por enfoque una sola cosa"*. Y es exactamente asi: **las tres
APIs llegan al MISMO SPIR-V**, y desde ahi el camino es uno solo --lector,
juez, oraculo, emisor--, sin una rama por API.

| Puerta | Lenguaje | Compilador (Vulkan SDK) | Lo que cambia en el SPIR-V | Prueba |
|---|---|---|---|---|
| **Vulkan** | GLSL | `glslc --target-env=vulkan1.0` | nada: es la referencia | `pruebas/*.comp` |
| **OpenGL** | GLSL | `glslc --target-env=opengl` | nombres y el orden de las decoraciones; los bindings igual | `pruebas/opengl/*.spv` |
| **DirectX** | HLSL | `dxc -spirv -T cs_6_0` (Microsoft) | `register(tN/uN/bN)` pasa a `Binding N`, `DescriptorSet 0`; `cbuffer` = `Uniform` + `Block`; no importa `GLSL.std.450` si no lo usa | `pruebas/hlsl/*.hlsl` |

`emisor-x86_64/tests/tres_apis.rs` corre suma, saxpy, mandelbrot y las cinco
trascendentes por las TRES: cada una da los mismos bits que su oraculo, y
**las tres dan los mismos buffers entre si**. Un sombreador de un juego de
DirectX y el mismo de uno de Vulkan calculan lo mismo en BMO-X.

[!] Lo que la tabla NO dice todavia: en HLSL `t0` y `u0` caen los dos en el
`Binding 0` (DXC no los separa sin `-fvk-t-shift`/`-fvk-u-shift`). Los HLSL de
`pruebas/hlsl/` usan numeros distintos a proposito; un juego de verdad pedira
el corrimiento, y eso es trabajo de VERRANO (la API), no del sombreador.

## Lo que NO es

Ni VERRANO (la API), ni el rasterizador, ni la GPU. Solo la etapa de COMPUTO,
que es la unica que se juzga entera sin pantalla.

## Las pruebas

`pruebas/*.comp` (GLSL) y su `.spv` al lado. `fuera.comp` usa A PROPOSITO lo que
el subconjunto no traduce (imagen, atomico, barrera): el lector lo lee entero y
el juez (S2) lo tendra que negar nombrando cada cosa.
