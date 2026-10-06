# PLAN MAQUETA 3 -- lo que le falta a un `.maqueta` para escribir lo que escribe CSS, y MEJOR

> Abierto el **2026-10-06**. El propietario: *"puedes hacer maqueta y que
> faltan mi .maqueta como estilo CSS? pero mejor claro."*
>
> Y despues: *"que sean interactiva para entender"* y *"COMPLETO el CSS
> objetivo y porque [...] con .maqueta con multiples ELEMENTOS"*.
>
> La maqueta de este plan es `docs/arte/maqueta_maqueta3.html`, y se TOCA:
> cada tarjeta con LAB tiene sus controles (la rejilla que se ensancha, los
> dos margenes que el navegador funde de verdad, la opacidad premezclada, la
> secuencia que se arrastra), el mapa de TODO CSS se filtra y se abre modulo
> a modulo, los elementos de la casa se prueban, y las decisiones mueven el
> medidor. Sigue a `docs/plan/PLAN_MAQUETA.md` (6c: igualar a
> CSS, E2 y E3) y al contrato `docs/componente/LA_MAQUETA_EXIGE.md`.

---

## 0. La respuesta corta

```text
   hoy          62,45 % del CSS de las tres maquetas de docs/arte/ compila
                (2021 de 3236 declaraciones, MEDIDO el 06-10 despues de MA1;
                antes de empezar, 59,30 %)
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

## 2b. EL OBJETIVO COMPLETO: todo CSS, modulo a modulo

El objetivo no es copiar CSS: es poder escribir CUALQUIER pantalla de BMO-X
(HERMES, BANK CAT, el TALLER, F2) en `.maqueta` y que el compilador la juzgue.
CSS es la vara porque es lo que entiende la vista previa, el navegador.

La maqueta lleva el mapa entero: **37 modulos** de CSS, cada uno con su sitio
y su porque (el texto de cada uno esta alli, para leerlo pulsando):

```text
   ya esta     15   la caja, flexbox, posicion, la letra, color, fondos,
                    bordes, resplandor, transiciones, selectores, cascada
                    (MEJOR desde el dia uno: gana la ultima), variables,
                    dibujos, imagenes, listas
   pila A       6   medidas acotadas, flex que parte, posicion fina, el
                    texto, contorno, desbordar
   pila B       6   margen, flex que crece, rejilla, la familia,
                    transparencia, variables de caja
   pila C       2   transformar, animaciones
   pendiente    1   :active, :focus y :disabled como @estado con nombre
   nunca        7   filtros y mezclas (son de la 3060), combinadores y
                    herencia (L7), unidades relativas, @media (hasta que
                    haya dos pantallas), contenido generado, escritura rtl
```

## 2c. LOS ELEMENTOS DE LA CASA

Hoy la lista de etiquetas es CERRADA, 9: `maqueta`, `style`, `div`, `span`,
`island`, `svg`, `path`, `usa`, `imagen`. Y `<button>` o `<input>` se
rechazan porque "prometen semantica que no existe".

MAQUETA 3 trae **nueve elementos propios cuya semantica SI existe**: cada uno
se expande en las piezas de siempre y el compilador emite lo que lo hace
real. En el navegador, `maqueta --vista-html` los escribe como `div` con su
clase, asi que la vista previa no miente.

| elemento | se expande en | el compilador emite |
|---|---|---|
| `<boton>` | `div` con `cursor:pointer` y `@estado` realce y pulsado | su zona, `realce_en`, `pulsado_en` |
| `<campo letras="24">` | `div` + `span` con dato, juzgado con 24 letras anchas | su zona y `CAMPO_<ID>`; el texto lo guarda la app |
| `<interruptor>` | dos `@estado` (si, no) con transicion de rebote | su zona, `pintar_estado`, `pintar_transicion` |
| `<barra de="100">` | caja fija y RELLENO proporcional (no es maquetar) | `pintar_barra(p, valor)` |
| `<lista tope="5">` | `<usa repite="5">` | `LISTA_<ID>.fila(i)` |
| `<ventana alto="92">` | `overflow-y:auto` (H7) | `desplazar_<id>`, `DESPLAZA_<ID>` |
| `<solapas de="a b c">` | una fila de `<boton>` y un `@estado` por opcion | sus zonas, `pintar_estado(a)` |
| `<icono nombre="x">` | el `<svg>` de un catalogo compartido | los trazos del catalogo |
| `<globo>` | caja absoluta, `capa`, resplandor y `@secuencia` de entrada | `pintar_secuencia(entra, ms)` |

## 2d. TODO EL SVG, Y ANIMAR TODO (06-10)

El propietario: *"es para tener mi BMO-X con TODO el SVG en internet y animar
todo"*. Hoy un `<svg>` solo lleva `<path>` (sin arcos `A`). Lo que cambia, y
la ley que no:

```text
   en el anfitrion, AL COMPILAR    se lee el SVG entero, se aplana todo a
                                   trazos y se juzga
   en el aparato                   solo se pintan trazos -- como hoy
```

| escalon | que entra | como |
|---|---|---|
| S1 | `circle`, `ellipse`, `rect` (con `rx`), `line`, `polyline`, `polygon` | se convierten en `path` al leerlos: las formas son caminos con nombre |
| S2 | el arco `A` en `d` | se aplana en curvas, como ya se aplanan `C` y `Q` |
| S3 | `<g>` y `transform` (`translate`, `scale`, `rotate`, `matrix`) | se aplican a los puntos al compilar. En un dibujo SI se gira: es geometria, no la letra de la casa |
| S4 | `fill-rule`, `stroke-dasharray`, `stroke-dashoffset`, `opacity` del dibujo | el discontinuo son trozos de camino, calculados al compilar; la opacidad, premezclada (M4) |
| S5 | `linearGradient` y `radialGradient` de dos paradas | lo mismo que el degradado de una caja |
| S6 | **`<svg src="logo.svg">`: un SVG de internet, tal cual** | se lee el fichero, se aplana y se juzga; lo que no se puede (`text`, `filter`, `mask`, `image`, `foreignObject`, `script`) es error con la LISTA de lo que tiene, en vez de pintarlo a medias |
| S7 | animar: `<animate>`, `<animateTransform>` y los `@keyframes` de dentro | cada paso es una `@secuencia` de dibujos YA aplanados; si dos pasos no tienen los mismos puntos, se dice al compilar |

## 3. Las decisiones del propietario -- DECIDIDAS el 06-10: las siete en (a)

El propietario, con la captura de la seccion 6 de la maqueta delante (las
siete en (a)): *"me encanto por completo, asi que a completar todo eso [...]
el CSS COMPLETO pero no todo para no romper reglas en mi BMO-X"*. Cada una
cambia lo que dice `docs/componente/LA_MAQUETA_EXIGE.md`, y se escribe ahi el
dia que entra su codigo, no antes.

```text
   M1  margin         (a) entra, error solo donde CSS fundiria   (b) sigue fuera
   M2  font-family    (a) `bmo`, exportada como fuente web       (b) rechazada
   M3  grid, flex:1   (a) con la medida de la caja dicha          (b) esperar
   M4  opacity        (a) mezclada al compilar sobre fondo liso  (b) esperar al alfa
   M5  animacion      (a) @secuencia de estados                  (b) solo estados
   M6  var por caja   (a) constantes de UNA caja                 (b) solo :root
   E1  elementos      (a) los nueve de la seccion 2c             (b) solo los 9 de hoy
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
- [x] M0b -- HECHO el 06-10: la maqueta INTERACTIVA (`docs/arte/maqueta_maqueta3.html`): siete LAB que juzgan en vivo (rejilla, margenes medidos en el navegador, variables de caja, opacidad premezclada, flex:1, la secuencia que se arrastra, las zonas), el mapa de los 37 modulos de CSS (seccion 2b), los elementos que se prueban (seccion 2c), y las decisiones que mueven el medidor; probada en Chromium con los clics de verdad
- [x] M-dec -- HECHO el 06-10: las decisiones M1-M6 y E1, las siete en (a), escritas en la seccion 3 de este plan
- [ ] ME -- los elementos de la seccion 2c segun E1: la etiqueta en `toolchain/tools/maqueta/node/src/markup.rs`, su expansion antes de la cascada, y lo que emite en `toolchain/tools/maqueta/emit/src/rust.rs`; el catalogo de iconos junto a `toolchain/tools/maqueta/tema/tema.maqueta`
- [x] MA1 -- HECHO el 06-10: la parte de la pila A que solo toca la maquetacion -- `text-align` (con la comprobacion L: en una caja flex o en un parrafo no alinea, y se dice), `min-width`, `max-width`, `min-height`, `max-height` (`sujeta` en `toolchain/tools/maqueta/layout/src/measure.rs`: `max` gana a `width`, `min` gana a `max`, y acotan tambien lo que llena y lo que se estira), y los atajos `inset`, `padding-block` y `padding-inline` (`toolchain/tools/maqueta/node/src/style/atajos.rs`). Escrito antes en `docs/componente/LA_MAQUETA_EXIGE.md` 3e; 9 pruebas nuevas, las 248 de MAQUETA en verde. Medido: 59,30 % -> **62,45 %** (2021 de 3236)
- [ ] MA -- la pila A entera en `toolchain/tools/maqueta/node/src/value.rs` (la lista), el nieto en `toolchain/tools/maqueta/layout/src/flow.rs` y su juicio en `toolchain/tools/maqueta/verdict/src/fit.rs`; cada propiedad con su fichero dorado en `toolchain/tools/maqueta/pruebas/`
- [ ] MB -- la pila B segun M1-M4 y M6: la rejilla en `toolchain/tools/maqueta/layout/src/flow.rs`, las variables de caja en `toolchain/tools/maqueta/node/src/variables.rs`, la premezcla en `toolchain/tools/maqueta/emit/src/paleta.rs`, y la fuente web desde `platform/shared/bmo-letra`
- [ ] MC -- la pila C segun M5: `@secuencia` junto a `@estado` en `toolchain/tools/maqueta/node/src/style.rs` y el movimiento en `toolchain/tools/maqueta/emit/src/movimiento.rs`
- [ ] MS -- la seccion 2d, de S1 a S7: las formas y los grupos en `toolchain/tools/maqueta/node/src/markup.rs`, el aplanado (arcos, transformaciones, discontinuos) en `toolchain/tools/maqueta/emit/src/orden.rs`, y `<svg src>` como lee hoy `<imagen src>` en `toolchain/tools/maqueta/compone/src/lib.rs`
- [ ] MR -- volver a medir con `toolchain/tools/maqueta/cli/src/cobertura.rs` despues de cada pila, y escribir la cifra de verdad en la seccion 0
