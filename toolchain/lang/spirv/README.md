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
| **los NUMEROS** (que `OpIAdd` es 128, que lleva tipo y resultado, cuantas palabras minimas) | se toman de `spirv.core.grammar.json` (licencia MIT, la fuente normativa de la seccion binaria de la especificacion) con [`herramientas/tabla.py`](herramientas/tabla.py). Son el contrato, igual que los del ABI: inventarlos seria no hablar SPIR-V |
| **QUE instrucciones tienen fila y de que familia son** | lo decide `FILAS` en ese script: es el estudio, y es nuestro |
| **el codigo** de SPIRV-Tools, SPIRV-Cross, Mesa | **no se enlaza ni se copia.** Se LEE para aprender reglas, como OBS para LA MESA |
| **el SDK de Vulkan** del anfitrion | solo fabrica los `.spv` de prueba ([`herramientas/fabricar.py`](herramientas/fabricar.py)); el banco no lo necesita |

## Lo que hay

| Pieza | Casilla | Estado |
|---|---|---|
| `src/tabla.rs` | S1 | 194 filas, generadas y cotejables (`tabla.py --cotejar`) |
| `src/lector.rs` -- bytes a un `Modulo` | S1 | hecho: cabecera, medidas, ids, cadenas, orden de secciones, funciones |
| `src/motivo.rs` -- por que NO | S1 | 23 motivos, cada uno con su fila en `tests/lector.rs` |
| el juez del subconjunto | S2 | pendiente |
| el oraculo (interprete) | S3 | pendiente |
| `emisor-x86_64/` | S4 | pendiente: sera OTRO crate, porque este no nombra maquinas |

## Las dos reglas

1. **`no_std` y sin `alloc`.** La memoria --la tabla de ids-- la da quien
   llama. Es lo que permite que el MISMO lector corra en el anfitrion (AOT) y
   dentro de una app de BMO-X (JIT). Por eso no hay `Vec` aqui dentro.
2. **Los bytes son de un tercero.** Ningun `.spv` puede hacer que el lector
   entre en panico: el banco corta cada fichero en cada palabra y voltea cada
   byte de tres de ellos.

## Lo que NO es

Ni VERRANO (la API), ni el rasterizador, ni la GPU. Solo la etapa de COMPUTO,
que es la unica que se juzga entera sin pantalla.

## Las pruebas

`pruebas/*.comp` (GLSL) y su `.spv` al lado. `fuera.comp` usa A PROPOSITO lo que
el subconjunto no traduce (imagen, atomico, barrera): el lector lo lee entero y
el juez (S2) lo tendra que negar nombrando cada cosa.
