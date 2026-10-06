# PLAN MAQUETA 3 -- lo que le falta a un `.maqueta` para escribir lo que escribe CSS, y MEJOR

> Abierto el **2026-10-06**. El propietario: *"puedes hacer maqueta y que
> faltan mi .maqueta como estilo CSS? pero mejor claro."*
>
> La maqueta de este plan es `docs/arte/maqueta_maqueta3.html` (se abre en un
> navegador: cada tarjeta compara CSS con MAQUETA 3, con demos vivas, y las
> decisiones se pulsan). Sigue a `docs/plan/PLAN_MAQUETA.md` (6c: igualar a
> CSS, E2 y E3) y al contrato `docs/componente/LA_MAQUETA_EXIGE.md`.

---

## 0. La respuesta corta

```text
   hoy          59,30 % del CSS de las tres maquetas de docs/arte/ compila
                (1919 de 3236 declaraciones, MEDIDO, no contado)
   A  +9,1      lo que entra sin tocar ninguna ley           -> 68,4 %
   B  +20,0     lo que se hace MEJOR que CSS, al compilar    -> 88,4 %
   C  +6,0      el movimiento, como estados ya maquetados    -> 94,3 %
   nunca 5,7    con su motivo escrito (seccion 4)
```

"Mejor que CSS" tiene aqui un sentido preciso, el mismo de todo MAQUETA: **lo
que CSS resuelve en el navegador en cada cuadro, aqui se resuelve UNA vez al
compilar, y SE JUZGA**. Una celda que no cabe, un texto que se corta, dos
margenes que se funden: en CSS pasan en silencio; aqui son un error con la
cifra y el arreglo.

Y las tres leyes no se tocan: un padre no sabe que tiene padre (L7), la vista
previa en el navegador no puede mentir, y en el aparato no se maqueta nada.

## 1. La medida, y como repetirla

```text
   maqueta --cobertura docs/arte/maqueta_bankcat.html \
                       docs/arte/maqueta_hermes.html \
                       docs/arte/maqueta_taller_estratos.html
```

`toolchain/tools/maqueta/cli/src/cobertura.rs` pasa cada declaracion de esas
tres maquetas por el compilador de verdad. El 06-10: bankcat 62,04 %, hermes
59,56 %, taller_estratos 53,74 %; en total 1919 de 3236. Las 1317 que faltan,
agrupadas por la propiedad que las rechaza, son las tres pilas de abajo.

[!] Es el TECHO de cada pila, no una promesa: una declaracion que pasa la
lista todavia la juzga el veredicto (que quepa, que se pinte igual en el
navegador). Se vuelve a medir al cerrar cada escalon.

## 2. Las tres pilas

### A -- entran ya (295 usos): ninguna pide la medida del padre

| que | usos | como |
|---|---|---|
| `min-width`, `max-width`, `min-height`, `max-height` | 76 | contra el contenido PROPIO, al compilar; si no cabe en el maximo, error con la cifra |
| `overflow: hidden` (y `overflow-x`) | 40 | el recorte ya existe (`toolchain/tools/maqueta/emit/src/recorte.rs`); el veredicto dice que se corta |
| `cursor`, `pointer-events` | 30 | `cursor: pointer` DECLARA una zona que se pulsa: el modulo sale con su tabla de zonas |
| `text-align`, `text-overflow: ellipsis` | 31 | el texto se mide al compilar con la letra que lo pinta; el `...` ya lo hace el aparato (H2) |
| `inset`, offsets negativos, `z-index` | 48 | `capa: N`, un numero GLOBAL y ordenado, en vez del contexto de apilado de CSS |
| `flex-wrap`, `aspect-ratio`, `outline`, `align`/`justify-*`, `padding-block/inline`... | 70 | partir filas con hijos y anchos conocidos; la proporcion de la caja propia; el contorno es un anillo de `bmo-pinta` que no ocupa |

### B -- mejor que CSS, al compilar (646 usos)

| que | usos | CSS | MAQUETA 3 |
|---|---|---|---|
| `display: grid` y sus pistas | 204 | reparte `fr` contra el contenedor, en cada cuadro | `fr` contra el ancho de la PROPIA rejilla (como `border-radius: 50%`), y cada celda se juzga: la que no cabe es error con su fila y su columna |
| `margin` | 114 | dos margenes verticales pegados se FUNDEN (10 + 10 = 10) | entra, y es error SOLO el caso en que CSS los fundiria, con el arreglo escrito: `gap` en el padre |
| variables por caja (`style="--c:..."`) | 104 | se heredan a los hijos | valen para SU caja; si un hijo la lee, error (en el navegador la heredaria) |
| `font-family` | 84 | cualquier letra, y la de reserva si no carga | una: `bmo`, la de la casa, EXPORTADA como fuente web desde `platform/shared/bmo-letra` para que el navegador pinte la misma (hoy la foto se parece un 86 % por la letra de reserva) |
| `opacity` | 67 | mezcla alfa en cada cuadro | sobre un fondo liso y conocido, la mezcla se hace al compilar: el aparato pinta un color. Sobre una isla o una imagen, error |
| `flex: 1` | 59 | crece contra el contenedor | solo en un padre con medida DICHA; el sobrante se reparte al compilar |
| `transition` de varias propiedades | 14 | una lista | la mezcla de piezas ya lleva radio, medida y color: se acepta la lista |

### C -- el movimiento (193 usos): estados, no maquetar en el aparato

| que | usos | MAQUETA 3 |
|---|---|---|
| `@keyframes`, `animation*` | 94 | `@secuencia nombre 1600ms repite { 50% { reglas } }`: cada paso es un `@estado` ENTERO, maquetado y juzgado al compilar; el aparato solo mezcla piezas, como ya hace con las transiciones (`docs/componente/LA_MAQUETA_EXIGE.md` 3d) |
| `transform: translate / scale`, `transform-origin` | 99 | solo dentro de un estado o de una secuencia, resuelto a cajas de pixel entero; `rotate` se rechaza: la letra de la casa no se pinta torcida |

## 3. Las decisiones del propietario

Las seis cambian lo que dice `docs/componente/LA_MAQUETA_EXIGE.md`. La
maqueta las deja pulsar; la recomendada es la (a) en las seis.

```text
   M1  margin         (a) entra, error solo donde CSS fundiria   (b) sigue fuera
   M2  font-family    (a) `bmo`, exportada como fuente web       (b) rechazada
   M3  grid, flex:1   (a) con la medida de la caja dicha          (b) esperar
   M4  opacity        (a) mezclada al compilar sobre fondo liso  (b) esperar al alfa
   M5  animacion      (a) @secuencia de estados                  (b) solo estados
   M6  var por caja   (a) constantes de UNA caja                 (b) solo :root
```

## 4. Lo que NO entra, con su motivo (183 usos)

- `inherit` y la herencia (23): un padre no sabe que tiene padre (L7).
- `width: %`, `height: auto`, `calc()` (60): piden la medida del contenedor;
  `fr` de la rejilla PROPIA cubre el caso.
- `content` y los pseudo-elementos (10): una caja que no esta en el fichero
  no se puede juzgar.
- `filter`, `mix-blend-mode`, `mask`, `clip-path` (13): piden componer en el
  aparato. El desenfoque es de la 3060 (VERRANO), no de la maqueta.
- decimales de caja, `line-height: 1.4`, `font-size: 13.5px` (19): medio pixel
  de caja es un borde borroso.
- restos de JavaScript y `-webkit-` (5): no son CSS.
- y 53 sueltos de 1 a 7 usos (`overflow-wrap`, `background-size`,
  `stroke-dasharray`, `position: sticky`...): no son "nunca"; se miran uno a
  uno cuando un caso real los pida.

## 5. Los escalones

- [x] M0 -- HECHO el 06-10: la medida (seccion 1) y la maqueta del plan, `docs/arte/maqueta_maqueta3.html`, con las tres pilas, las demos y las seis decisiones
- [ ] M-dec -- las decisiones M1-M6 del propietario, escritas en la seccion 3 de este plan
- [ ] MA -- la pila A entera en `toolchain/tools/maqueta/node/src/value.rs` (la lista), el nieto en `toolchain/tools/maqueta/layout/src/flow.rs` y su juicio en `toolchain/tools/maqueta/verdict/src/fit.rs`; cada propiedad con su fichero dorado en `toolchain/tools/maqueta/pruebas/`
- [ ] MB -- la pila B segun M1-M4 y M6: la rejilla en `toolchain/tools/maqueta/layout/src/flow.rs`, las variables de caja en `toolchain/tools/maqueta/node/src/variables.rs`, la premezcla en `toolchain/tools/maqueta/emit/src/paleta.rs`, y la fuente web desde `platform/shared/bmo-letra`
- [ ] MC -- la pila C segun M5: `@secuencia` junto a `@estado` en `toolchain/tools/maqueta/node/src/style.rs` y el movimiento en `toolchain/tools/maqueta/emit/src/movimiento.rs`
- [ ] MR -- volver a medir con `toolchain/tools/maqueta/cli/src/cobertura.rs` despues de cada pila, y escribir la cifra de verdad en la seccion 0
