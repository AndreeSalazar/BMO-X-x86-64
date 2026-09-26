# PLAN EL AISLAMIENTO -- cada GPU con su emisor, su juez y su puerta

> El propietario (2026-09-26), despues del tablero del banco: *"el .bsf es
> independiente no? [...] el emisor SOLO para GPU RTX 3060 12G ese mismo
> TIENEN QUE AISLARSE POR COMPLETO porque hay razones y si tengo otra GPU no
> es lo mismo [...] las GPU son variados y choca si mezcla eso lo aprendi de
> mi x86-64 ELIJE UNO y pagar una sola vez [...] SPIR-V esta bien PEEERO tiene
> que enfrentar al juez celoso y asi genera un .bsf independiente que YA
> SABE"*. Y despues: *"si y aplicar por favor MAS PRECISOS no quiero tener que
> pelear luego con otras GPU"*.

---

## 0. La respuesta corta

```text
   lo comun      SPIR-V (la ENTRADA), el sobre BSF (el FORMATO) y VERRANO
                 (la API). Ninguno de los tres sabe que GPU hay debajo
   lo de cada    su EMISOR (SPIR-V -> su ISA), su JUEZ (sus bits de control,
   GPU           sus latencias, sus registros) y su PUERTA (el backend que
                 arma lo que su kernel sube). Nada de esto se comparte:
                 las GPU son distintas y mezclarlas es donde choca
   el sobre      lleva un objetivo por `kind`; quien lo abre toma SOLO el de
                 SU tarjeta. Si no esta, NO: no se compila en marcha
   el juez       dos veces: al fabricar el sobre (sin juez no hay sobre) y
                 en la puerta del kernel (lo que no se puede saltar)
```

La razon es la de x86-64: *elegir uno y pagar una vez*. Con GPU "uno" es
**uno por tarjeta**: SM86 tiene 2 registros gastados en el contador de
programa, una tabla de latencias de Ampere y un formato de control de 17
bits que otra familia no tiene. Un emisor o un juez "para todas" seria uno
que no es preciso para ninguna.

---

## 1. Donde vive cada pieza (desde el 26-09)

| pieza | donde | de quien |
|---|---|---|
| la entrada | `toolchain/lang/spirv` (el frontend) | de TODOS |
| el sobre | `toolchain/lang/spirv/bsf` (`bmo-bsf`) | de TODOS: formato + la capa profunda (relee el SPIR-V). **Sin emisores** |
| emisor x86-64 | `toolchain/lang/spirv/emisor-x86_64` | de la CPU |
| su adaptador al sobre | `toolchain/lang/spirv/bsf-x86_64` (`bmo-bsf-x86-64`): `x86_64_target`, `reproducir`, la herramienta `bmo-bsf-x86-64 fabricar` | de la CPU |
| el SASS de la 3060 | `platform/drivers/gpu/ga10x/src/trabajos/tuberia.rs` (a mano, hasta E1..E5) | de la 3060 |
| su juez | `platform/drivers/gpu/ga10x/src/sass/juez.rs` | de la 3060 |
| su fabrica del sobre | `platform/drivers/gpu/ga10x/tests/bsf_sm86.rs` (juzga antes de escribir) | de la 3060 |
| la API | `platform/shared/verrano` (`Frame`, `Backend`, `Stats`) | de TODOS |
| VERRANO en el escritorio | `gspcubo/verrano.rs`, `gspcubo/tablero.rs` | de TODOS |
| la puerta del escritorio | `gspcubo/sm86.rs`: el `kind`, el juez, las ordenes, lo que se le explica al propietario | de la 3060 |
| la puerta del kernel | `gpu_trabajo/cubo.rs` (`CUBO_VERRANO`): juzga y sube | de la 3060 |

**Otra GPU** trae SU fila en cada "de la 3060" de esta tabla, y ninguna fila
"de TODOS" cambia. Si una tuviera que cambiar, el aislamiento fallo.

---

## [x] A1 -- EL SOBRE SIN EMISORES

`bmo-bsf` enlazaba `bmo-spirv-x86-64`: `x86_64_target`, `EMITTER` y
`Bsf::reproduce` vivian dentro. O sea que el sobre que lleva el SASS de la
3060 arrastraba un emisor de CPU, y el escritorio (que abre el sobre) tambien.

Hecho (26-09): todo eso paso a `bmo-bsf-x86-64` (`reproduce` es ahora
`reproducir(&bsf, ..)`), con la herramienta, el banco del formato
(`tests/formato.rs`, `silicio.rs`) y `examples/medir.rs`. El sobre se queda
con el formato y con `deep` (releer el SPIR-V, la entrada comun). `Fault::at`
y `spirv_fault` son publicos para que cada adaptador diga sus fallos igual.
El build llama a `bmo-bsf-x86-64 fabricar` (`build/ejemplos.ps1`).

- **Como se sabe:** `bmo-bsf` compila sin ningun emisor; los 9 del banco del
  formato pasan en su sitio nuevo; `la-3060` regla S1 falla si vuelve uno
  (probado metiendo `bmo-spirv-x86-64` a proposito: lo caza).

## [x] A2 -- UNA PUERTA EN EL ESCRITORIO

`gspcubo/verrano.rs` importaba `bmo_gpu_ga10x::{cubo, tuberia}`, el juez y
`kind::SM86`. Ahora todo eso esta en `gspcubo/sm86.rs`, y `verrano.rs` entra
por una linea: `use super::sm86 as destino;`. La puerta da:

```text
   Opciones::de(palabras)   lo que es de ESTA tarjeta (`ligero`, `sinldg`)
   abrir(..)                el sobre con SU kind, el juez, el motor, la ficha
   Aparato: Backend         dibuja; `Stats` dice device/prepare_us y warm
   ventana(p)               donde cae la ventana (la cuenta del kernel)
   fallo(..)                por que no dibujo, con lo que ESTA sabe contar
```

Y la API aprendio lo que antes se sacaba de los bits del kernel:
`Stats::prepare_us` y `Stats::warm` (la CPU los deja a 0 y `false`).

- **Como se sabe:** `verrano.rs` y `tablero.rs` no nombran la 3060, SM86, el
  juez ni el sobre (`la-3060` S3, probado con un `use bmo_gpu_ga10x` a
  proposito); `gpu verrano` y `gpu verrano banco` dicen lo mismo que antes
  en el metal.

## [x] A3 -- EL JUEZ EN LA PUERTA DEL KERNEL (J2)

`CUBO_VERRANO` juzga los dos programas del paquete con
`juez::juzgar_programa` y los registros de las ordenes antes de prepararlos.
Un BODRIO vuelve con **`IOMMU_NO_BODRIO` (87)**, en los tres sitios, con su
texto en `commands/iommu.rs`, y la regla y la instruccion en la cabina. En
caliente no se repite (los programas son los de un dibujo que ya se juzgo:
`CALIENTE_PROGRAMAS`). `juez::MAX_INSTRUCCIONES` bajo a 64 (1 KiB de copia):
`pila` sigue limpio, 6.761 bytes libres en el camino mas hondo.

- **Como se sabe:** `la-3060` cuenta 87 motivos iguales en los tres sitios y
  S4 exige la llamada; en el metal, un `cubo.bsf` roto a proposito vuelve con
  el motivo 87 y la 3060 no lo ve (falta verlo en el metal).

## [x] A4 -- EL GUARDIAN

`toolchain/tools/la-3060/la_3060.py`, regla **S** (S1 el sobre, S2 la API,
S3 VERRANO en el escritorio, S4 la puerta del kernel). Corre en cada build.

## [ ] A5 -- LA SEGUNDA TARJETA (cuando la haya)

Lo que tendra que traer, y nada mas: su driver `platform/drivers/gpu/<suyo>`
con su emisor (E1..E5 de [`PLAN_LA_LENGUA_DE_LA_3060.md`](PLAN_LA_LENGUA_DE_LA_3060.md)
son los de la 3060), su juez, su `kind` en `bmo_bsf::kind` (un numero
nuevo, nunca el de otra) y su puerta `gspcubo/<suyo>.rs`. `verrano.rs` elige
la puerta por la tarjeta que hay; hoy hay una.

- **Bloquea:** una segunda GPU en la maquina.
- **Como se sabe:** su llegada no cambia ninguna fila "de TODOS" de la
  seccion 1, y `la-3060` S sigue limpio.

---

## Lo que NO es esto

- **No es quitar el driver del escritorio.** Los paneles `gsp*` del
  escritorio son el banco de diagnostico de la 3060 y la nombran a
  proposito (`bmo-gpu-ga10x` es `capa: puro`, y L8 lo deja). Lo aislado es la
  API y el sobre: lo que un juego usaria.
- **No es un IR comun entre GPUs.** La entrada comun es SPIR-V y se queda en
  la entrada: despues, cada tarjeta lo suyo.
