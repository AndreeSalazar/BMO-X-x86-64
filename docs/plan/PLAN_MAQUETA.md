# PLAN MAQUETA

> El compilador de composicion del escritorio. **No es un navegador, y la
> diferencia no es de medida: es de direccion.**
>
> Escrito el **2026-08-17**. Nace de una idea del propietario y de tres frases suyas
> que ya decidieron el esquema entero:
>
> > *"crear un compilador compositor profesional que es usar con HTML + CSS
> > exclusivo como para compositor, para construir pero no es navegador"*
>
> > *"en arch linux es como sentir que tienes control (...) pero optimizacion o
> > Gentoo no tienen sentido, BMO-X base ya cubre TODO en silicio, encima es
> > composicion para facilitar"*
>
> > *"HTML inspirado como ASTRO pero profesional"*
>
> Decidido en la misma conversacion: **cinco generaciones, sin herencia, nombre
> MAQUETA**.

---

## 0. QUE ES, EN UNA FRASE

**Un compilador que lee un arbol de cajas y unas reglas, y emite las
coordenadas ya calculadas.** La aritmetica de pixeles que hoy se escribe a mano
en `Ultra_userspace/services/director/src/scene/` pasa a ser texto que se lee.

```
   HOY       calc.rs:       CALC_BTN = 72;  CALC_GAP = 6;  fn button(row,col)
   LUEGO    calc.maqueta:  display:flex; gap:6px
```

### ⚠ El medida del problema, MEDIDO el 17-08 (y era menor de lo que dije)

La primera version de este documento decia *"7.780 lineas de `scene/`"*. **Es
falso y sobrevende.** Contado:

```
   13.829   el servicio gui entero
   -3.000   mouse.rs (699) + keys/* (1.374) + desktop/mod.rs (381) + gato.rs (515)
            -- CERO llamadas de dibujo entre los cuatro. MAQUETA no los toca nunca
   ~1.657   literales numericos en scene/; quitando 0/1/2 quedan ~700 magicos
      118   de las 214 lineas de calc.rs son maquetacion (el resto es la maquina
            de estados de la calculadora, y se queda en Rust para siempre)
```

**El escritorio no es sobre todo pintura: es sobre todo enrutado de entrada.**
Decirlo al reves habria hecho que la primera medida contra `calc.rs` pareciera un
fracaso cuando es un exito.

---

## 1. LA INSPIRACION, DICHA CON PRECISION

### Gentoo NO transfiere, y conviene tener escrito por que

Las USE flags existen porque **aguas arriba** alguien compilo un binario para
todo el mundo y tu estas recuperando una decision que te quitaron. **BMO-X no
tiene aguas arriba.** Eres el propietario del compilador, del kernel y del disco. No
hay nada que reclamar; el `-march=native` es un hombre peleando con un
empaquetador que aqui no existe.

### De Arch transfiere UNA cosa, y no es el rendimiento

**Nada esta generado por una herramienta que no puedas leer.** `PKGBUILD` es un
script. `/etc` es texto tuyo. Esa es la sensacion de control.

Hoy el escritorio de BMO-X es lo contrario: cambiar el hueco entre dos teclas de
6 a 8 pixeles es **editar Rust y recompilar un servicio**. Eso es mas opaco que
Arch, no menos.

### ★ Y aqui se puede ser MEJOR que Arch, no igual

El control de Arch es **en ejecucion**: editas, reinicias, y puedes arrancar a un
sistema roto. MAQUETA es control **en compilacion**: el escritorio es texto, pero
un texto que no compila **no llega a arrancar nunca**. La misma lectura, sin la
posibilidad de romperlo en caliente.

### ★★ De ASTRO transfiere la tesis entera

Astro no es un framework de navegador: es una **herramienta de construccion**. Su
rasgo definitorio es que **los componentes desaparecen al compilar** y solo queda
marcado plano; lo que necesita vivir se declara aparte, como **isla**.

Eso es exactamente esto:

| Astro | MAQUETA |
|---|---|
| el componente desaparece al compilar | la caja se vuelve un rect en una tabla |
| cero runtime por defecto | cero codigo en el aparato por defecto |
| **isla** = el trozo que si vive | **`<island>`** = el rect que rellena Rust |
| estilos con ambito por componente | igual, y **es lo que hace innecesaria la herencia** |

### ✳ Lo que ninguna distro da, dicho sin adorno

Ninguna distro **compone** el escritorio: todas **configuran uno ya escrito**.
GNOME y KDE traen un compositor en C/C++ y te dan un panel de ajustes; Arch te da
a elegir **cual** de los escritorios ya escritos instalas. El hueco no esta al
nivel de la distro -- esta **debajo**, y es donde vive BMO-X.

### ★ El regalo ironico: el navegador como REGLA, no como destino

Si las etiquetas son las de HTML y las propiedades son las de CSS de verdad,
entonces **un `.maqueta` se abre en un navegador y se ve aproximadamente bien**.
Previsualizacion gratis mientras se escribe, en Windows, sin arrancar el Ryzen.

⚠ **Aproximadamente**, y la palabra es literal: la fuente de BMO-X es de mapa de
bits y de ancho fijo, y la del navegador no. **La previsualizacion orienta; la
verdad son los ficheros dorados** (seccion 8 de `LA_MAQUETA_EXIGE.md`). Confundir
las dos cosas seria la primera forma de mentir de este compilador.

---

## 2. LA FRONTERA: lo QUIETO se compila, lo que VIVE se programa

Es la decision que impide la deriva hacia navegador, y hay que trazarla **antes**
de escribir una linea. Medido sobre los ficheros que existen hoy:

| Se compila (esta QUIETO) | No se compila (VIVE) |
|---|---|
| `calc.rs` -- es una rejilla | `vitals.rs` -- numeros que cambian |
| `launcher.rs`, `switcher.rs`, `splash.rs` | `cabina.rs`, `testigo.rs` |
| `chrome.rs` -- el marco de las ventanas | `data.rs` (956 lineas) -- el grafo de ESTRATOS |
| la barra, los paneles | `cursor.rs`, DOOM |

El dia que se quiera que un `<div>` muestre un numero que cambia, ya no se esta
escribiendo un compilador de maquetacion: se esta escribiendo el DOM. Y detras
del DOM viene todo lo demas.

**La costura entre las dos columnas es `<island>`**, y no es un invento nuevo:
una isla es un rect con nombre que otro proceso rellena -- que es **exactamente
la superficie de `PLAN_DIRECTOR.md`** (`BSUP`, `MEM_OP_OFRECER` / `TASK_OP_TOMAR`).
La mitad viva del escritorio ya tiene su mecanismo construido.

### ★★ Y la frontera de verdad no era "quieto contra se mueve": es EL NUMERO DE HIJOS

Leyendo `switcher.rs` (17-08) salio el caso que la tabla de arriba no describe.
El conmutador **esta quieto** -- no se anima, no cuenta nada -- y sin embargo su
altura es `ROW_H * lista.len() + ...`: **el numero de filas se sabe en ejecucion**,
porque son las ventanas que haya abiertas. Igual `launcher.rs` (`for i in
0..l.count`) y la lista de `mod.rs`.

Un compilador estatico no puede maquetar eso, y fingir que si es como se llega a
un motor de maquetacion en el aparato por la puerta de atras.

**La regla, y es la que decide el alcance real del proyecto:**

> **MAQUETA maqueta lo que tiene un numero de hijos CONOCIDO AL COMPILAR.
> La repeticion sobre datos vivos es de quien tiene los datos.**

En la practica: **la fila es un `.maqueta`, la lista es Rust.** El compilador
resuelve el interior de UNA fila -- sus rects relativos al origen de la fila, que
es lo irregular y lo que se equivoca una persona -- y Rust la coloca en un bucle
con un `+= alto_de_fila` que el propio compilador emitio. Es exactamente como esta
escrito hoy (`fy += ROW_H`), asi que no hay nada que reescribir: hay algo que
dejar de calcular a mano.

### ✅ Y un mecanismo que ya existia: `p.texto` devuelve la x siguiente

```rust
let mx = p.texto(x + 14, fy + 4, "modo: ", INK_DIM);
let mx = p.texto(mx,     fy + 4, modo,     ACCENT);
         p.texto(mx,     fy + 4, "   (Alt+M)", INK_DIM);
```

Eso es **flujo en linea**, y ya funciona. `<span>` no hay que inventarlo: hay que
enchufarlo a lo que hace `Pantalla` desde siempre.

---

## 3. LAS CINCO GENERACIONES

L7 nombra cuatro roles. Aplicada a este camino salen **cinco**, y fingir que son
cuatro es lo que produciria el monolito: la cascada relaciona dos *padres*
(nodo x regla) y la maquetacion relaciona dos *hijos* (caja x caja). Son
generaciones distintas.

```
   abuelo     TROZO      token de marcado / token de estilo
              no sabe si el documento es valido

   padre      PIEZA      Node / Rule -- nombrada y compuesta
              no sabe que tiene hermanos

   hijo       CASCADA    Node x Rule -> Box con estilo resuelto
              no sabe que es una pantalla

   nieto      MAQUETA    Box x Box -> rects enteros y definitivos
              no sabe que se hace con ellos

   bisnieto   VEREDICTO  es legal? es honesta?
              el unico con opinion
```

| crate | generacion | la pregunta que responde (L6b) |
|---|---|---|
| `bmo-maqueta-lex` | abuelo | que trozos hay en el fichero |
| `bmo-maqueta-node` | padre | que es cada pieza, con su nombre |
| `bmo-maqueta-cascade` | hijo | que regla le toca a que nodo |
| `bmo-maqueta-layout` | nieto | donde cae cada caja |
| `bmo-maqueta-verdict` | bisnieto | si esto esta bien o no |

### ★★ EL EMISOR NO ES UNA GENERACION: ES EL CONSUMIDOR

Y por la ley -- *ninguna generacion sabe quien la consume* -- **nadie de la
cadena sabe que existe**.

Consecuencia concreta, y es la que paga el reparto: la pregunta de si la salida
debe ser **codigo Rust generado** o **un recurso BEF (seccion 0x0B)** **ya no hay
que contestarla ahora**. Se empieza por Rust, que se prueba contra `calc.rs` en
una tarde, y el dia del recurso se agrega un segundo emisor **sin tocar ni una de
las cinco generaciones**. La ley convirtio una decision irreversible en una
reversible.

### ⚠ Correccion honesta a L7b

L7b dice *"el nieto siempre fuera del binario que mide"*, y su razon era el
hardware: `bmo-juicio` vive en `platform/shared/` porque el kernel es `no_main` y
no corre un test.

**Aqui esa razon no existe**: MAQUETA corre entera en el anfitrion y todo se
prueba en `cargo test`. Asi que el corte del veredicto necesita **otra** razon o
es decorativo, y la tiene:

> El veredicto se separa porque **es la unica pieza con opinion, y las opiniones
> cambian**. Cada propiedad nueva que se decida rechazar toca el veredicto. Si
> vive pegado a la aritmetica, la aritmetica se toca cada vez que cambia la
> politica -- y la aritmetica es lo que no debe moverse nunca.

---

## 4. ★★ LO QUE LA LEY DECIDE POR NOSOTROS

Esta es la parte que justifica haber discutido el reparto antes de escribir. **La
jerarquia no ordena el trabajo: elige el subconjunto de CSS.**

### El abuelo prohibe el navegador, sin que nadie lo decida

*abuelo = no sabe si el documento es valido.* La recuperacion de errores de HTML5
-- el `<p>` que se autocierra, la etiqueta mal anidada que el navegador arregla --
**exige que el tokenizador consulte el estado del arbol**, o sea que el abuelo
sepa quien lo consume. **L7a lo prohibe.**

No hace falta ser estricto por disciplina. **La ley que ya estaba escrita no deja
ser un navegador.** Es el mismo movimiento que los 246 ciclos: convierte una
intuicion en algo que se puede refutar.

### El padre prohibe tres cosas de CSS, y por eso no hay herencia

*padre = no sabe que tiene hermanos, ni que tiene padre.*

- **herencia** (`color` que baja de padre a hijo) -> exige conocer al ancestro. **Fuera.**
- **selectores de descendencia** (`.panel .boton`) -> exige conocer a los ancestros. **Fuera.**
- **`%` y `auto`** -> exigen conocer el medida del contenedor. **Fuera.**

Y el ambito por componente de Astro es justo lo que hace que no se echen de
menos: si las reglas de un fichero solo tocan a sus cajas, la herencia era un
arreglo para no repetirse dentro de un documento gigante que aqui no existe.

### ✅ El texto, que es lo que hunde a los motores de maquetacion, aqui es gratis

La parte cara de CSS no son las cajas: es el texto -- metricas, kerning, shaping,
salto de linea, fallback de fuente. **BMO-X tiene fuente de mapa de bits y de
ancho fijo**, asi que medir texto es:

```
   ancho = texto.len() * bmo::GLIFO_ANCHO
```

Exacto, entero, en tiempo de compilacion y sin dependencias. **Ese es el
argumento entero de viabilidad, y no vale para nadie mas.**

---

## 5. EL SITIO

**`toolchain/tools/maqueta/`**, con los cinco crates dentro.

El razonamiento, por si hay que revisarlo: `toolchain/lang/` es para lenguajes
que producen `.bex` pasando por la base (`c`, `cobol`, `ada`). MAQUETA **no
produce codigo**: produce coordenadas. Su parentesco es con `c-gen`, `cobol-gen`,
`fontgen` y `bmo-pack` -- generadores de anfitrion. Va en `tools/`.

⚠ **Nombres en INGLES**, identificadores y comentarios, desde la primera linea
(regla del 2026-08-08, incumplida tres veces). `maqueta` sobrevive como **nombre
de producto** -- como CABINA o DOOM -- no como identificador. **El disparador del
fallo es exactamente este**: crear ficheros nuevos en un arbol cuyos vecinos
estan en castellano.

---

## 6. LA ESCALERA

```
   [x] 0   los dos libros: este y LA_MAQUETA_EXIGE.md
   [x] 1   abuelo   lexer de dos modos (marcado / estilo)
   [x] 2   padre    Node y Rule
   [x] 3   hijo     cascada por clase y etiqueta, ultimo gana
   [x] 4   nieto    maquetacion: bloque + flex en un eje
   [x] 5   bisnieto veredicto: las seis comprobaciones
   [x] 6   emisor A -> Rust generado, y calc.rs como primera victima
   [ ] 7   ficheros dorados como oraculo -> `toolchain/tools/maqueta/pruebas/calc.dorado`
           [!] la carpeta existe y tiene UN fichero: calc.maqueta. O sea la
               ENTRADA del oraculo, y no su respuesta esperada
   [X] 8   emisor B -> recurso BEF 0x0B -> `toolchain/tools/maqueta/emit/src/bef.rs`
           HECHO 25-08, y con el 1 de PLAN_LA_CARA_VIAJA delante: el FORMATO
           salio a `platform/shared/bmo-maqueta-cara` antes que el emisor,
           porque un emisor propietario del formato deja al lector deduciendolo
           [!] MISMO escalon que el 2 de PLAN_LA_CARA_VIAJA: se marcaron
               juntos, que era la condicion escrita
   [~] 9   `<island>` a una superficie BSUP  (parsea y viaja; falta el otro lado)
```

## 6b. MAQUETA 2: LA CARA BONITA (2026-10-04)

El propietario (04-10): *"TITAN++ es para apps mas pesadas, pero si quieren
una interfaz bonita, con MAQUETA sobran"*. Asi que MAQUETA aprende a pintar
lo que hace bonita una interfaz -- letra de verdad, sombra, degradado, radio
suave y dibujo vectorial -- **sin dejar de ser un compilador que rechaza**.

```
   [x] M1  la letra: font-size, font-weight, letter-spacing, line-height,
           text-transform. Mide en el anfitrion con `bmo-letra` (la misma
           que pinta), asi que el veredicto B ya no adivina con `len * 8`
   [x] M2  el acabado: box-shadow `0 0 Npx #RRGGBBAA` (resplandor) y
           background-image `linear-gradient(90|180deg, #A, #B)`
   [x] M3  el dibujo: `<svg viewBox>` + `<path d>` con stroke, fill y pluma
           redonda; el camino se aplana en el anfitrion a 1/16 de pixel y
           viaja como puntos, no como texto que alguien tenga que parsear
   [x] M4  un pintor para los tres sitios: `platform/shared/bmo-pinta`
           (no_std). El codigo generado, la CARA v2 y la foto del anfitrion
           lo llaman a el, y un test compara los pixeles byte a byte
   [x] M5  el escritorio lo usa: `Pantalla::pieza`, `letra` y `medir`
           (`userland/src/pantalla/verde/fina.rs`), sin monton: la letra es
           una `LetraFija` estatica con cerrojo. Comprobado: la tarjeta
           generada (32 `p.pieza`) compila en el director sin un aviso, y
           un test pinta la pantalla y la foto del anfitrion IGUALES
           [!] y lo que lo pide no es una cara generada todavia: es
               `scene/fino.rs` a mano (el conmutador, la rejilla, los
               titulos, el reloj). Nada de esto se vio en el Ryzen aun
```

Medido contra Chromium con `pruebas/tarjeta.maqueta`: **parecido 92,25 %**
(igual 89,97 %). Nada de esto se probo en el Ryzen todavia: es anfitrion.

## 6c. IGUALAR A CSS, MEDIDO Y NO CONTADO (2026-10-04)

El propietario: *"cuanto falta para llegar a ser igual o mejor que CSS?"* y
*"dale, vamos a igualar"*. La medida es `maqueta --cobertura` sobre las dos
maquetas de `docs/arte/`: cada declaracion pasada por el compilador de verdad
(ver `LA_MAQUETA_EXIGE.md` seccion 8).

```
   [x] E1  los atajos y los lados: `background`, `border`, `border-<lado>`,
           `padding-*`, `border-*-width/color`, padding de 2 y 3 valores,
           `transparent`, y `var(--x)` de `:root`
           -> `toolchain/tools/maqueta/node/src/variables.rs`, `style.rs`
           37,52 % -> 56,46 % (+18,94), medido
   [ ] E2  lo que mas falta ahora, por usos: `display:grid` (117),
           `flex` (58), `opacity` (56), `width`/`border-radius` en `%`
           (35 + 27: los circulos de `50%`), `overflow` (35), `min/max-width`
           (55), `position:relative` (29), `text-align` (21)
           -> `toolchain/tools/maqueta/node/src/value.rs` (la lista) y el
              nieto (`layout/`) para `grid` y `flex`
   [ ] E3  las variables que las maquetas definen POR CAJA (`--c` en un
           `style="..."`, 44 + 22 rechazos): hoy se rechazan porque en CSS se
           heredan. Decidir si entran como constantes de UNA caja
           -> `toolchain/tools/maqueta/node/src/variables.rs`
```

Lo que NO se persigue con propiedades, y es una decision: `font-family` (la
letra de la casa) y `margin` (seccion 3b de `LA_MAQUETA_EXIGE.md`). La
animacion SI, pero no como CSS: ver 6d.

## 6d. LAS PIEZAS: MAQUETA COMPUESTA, CON ESTADOS (2026-10-04)

El propietario: *"podemos tener multiples maqueta_1 hasta el infinito con que
sea abstraido por la maqueta principal que se encarga de hacer animacion,
colorido, expansion... MAQUETA y TITAN++, eso es todo"*.

Y es la salida de la frontera que la seccion 2 dejo escrita: **el escritorio
esta hecho de LISTAS**, y una lista no se podia maquetar porque cuantos hijos
tiene se sabe al correr. Con piezas, una FILA es una maqueta de medida fija
--se compila y se juzga sola-- y repetirla N veces en el aparato es
`y = i * paso`: no hace falta un motor de maquetacion.

```
   [x] P1  COMPONER: `<usa src="fila.maqueta"/>`. La pieza se compila y se
           juzga SOLA y se injerta maquetada; sus reglas no salen; sus ids
           salen con el del `<usa>` delante
           -> `toolchain/tools/maqueta/compone/src/lib.rs`
           el navegador compone igual (Shadow DOM en `foto.js`) y mide lo
           mismo: `pruebas/escaparate.maqueta`, 788 x 276 los dos
   [ ] P2  REPETIR: `<repite pieza="fila" eje="column" paso="34">`. El
           CUANTOS lo da TITAN++ o Rust al correr; el COMO, la pieza. El
           emisor A genera la pieza una vez y un bucle de `y = i * paso`
           -> `toolchain/tools/maqueta/compone/` y `emit/src/rust.rs`
   [ ] P3  ESTADOS: la principal declara estados (reposo, abierta,
           expandida) y CADA UNO se maqueta entero en el anfitrion; el
           veredicto juzga todos. Pasar de uno a otro es interpolar dos
           listas de cajas YA calculadas (sitio, medida, color): se mueve sin
           maquetar nada en el aparato
           -> `toolchain/tools/maqueta/compone/` (los estados) y
              `platform/shared/bmo-pinta` (la interpolacion, la misma en los
              tres sitios)
   [ ] P4  EMOJIS DE LA CASA: dibujados como `<svg>` con la pluma de la casa,
           INSPIRADOS en los famosos, no traidos (la regla del propietario:
           sin objetos de terceros). TITAN++ decide cual va con cada codigo
           Unicode; MAQUETA los pinta nitidos a cualquier talla
           -> `toolchain/tools/maqueta/pruebas/` (las piezas) y la tabla en
              TITAN++
```

★ **La ventaja, dicha con precision** (CSS SI anima: `animation`,
`transition`, `@keyframes`; las maquetas de `docs/arte/` lo usan 61 veces).
Lo que CSS no tiene es esto: cada estado VERIFICADO al compilar (un texto que
no cabe en "abierta" no compila), y la animacion sin recalcular la
maquetacion en la maquina -- el navegador si la recalcula, y por eso a veces
tironea --. Y lo que se ve en la foto del anfitrion es lo que pinta el Ryzen.

[!] La seccion 7 dice *"MAQUETA compila una imagen QUIETA"*. Con P3 deja de
ser verdad dicho asi, y se reescribe cuando P3 exista: lo que sigue siendo
verdad es que **en el aparato no se maqueta nada**.

## 6e. DE QUIEN SE TOMA CADA IDEA (2026-10-04)

El propietario: *"MAQUETA, que framework inspirado puede tener todo eso? por
motivos TODOS, hasta Next.js"* y *"con todo eso INSPIRACION... que TODOS
tengan vida"*. La regla es la de la casa: **la esencia, no el codigo**. Ni una
dependencia de ninguno.

| de | su idea | en MAQUETA |
|---|---|---|
| Svelte | el framework es un COMPILADOR y desaparece al compilar | el pariente mas cercano: MAQUETA ya es eso |
| SwiftUI | animar es pasar de un estado a otro; el sistema interpola los dos | P3, tal cual |
| Figma | componentes con variantes; *Smart Animate* entre dos frames | `<usa>` (P1) y los estados (P3) |
| Flutter | pinta TODO con su motor; las restricciones bajan, las medidas suben | `bmo-pinta`; "una pieza mide lo que mide" |
| Vue | un fichero con su plantilla y su estilo de ambito propio | un `.maqueta`; el Shadow DOM de la regla |
| Next.js | lo que corre al CONSTRUIR separado de lo que corre en el cliente; rutas por fichero | anfitrion contra aparato; pantallas como ficheros, quiza |
| Elm | errores del compilador que explican | el "por que" y el "en su lugar" de cada error |
| Tailwind | los tokens de estilo en un sitio | `tema.maqueta` |
| WPF (XAML + C#) | la cara en un fichero declarativo y la logica en otro lenguaje; `UserControl`, `DataTemplate`, `VisualStateManager` | MAQUETA + TITAN++; `<usa>` (P1), repetir (P2), estados (P3) -- pero con la maquetacion hecha AL COMPILAR y sin runtime de .NET |

**Lo que NO se toma**, aunque venga en los mismos: el DOM virtual de React
(comparar arboles en cada fotograma es el coste que MAQUETA evita
calculando antes), la hidratacion de Next.js (aqui no hay nada que
"despertar") y los arboles de dependencias de npm.

### Cuanta memoria come una maqueta: medido

| maqueta | cajas | la CARA (lo que viaja) | sus pixeles en pantalla |
|---|---|---|---|
| `calc.maqueta` | 28 | 2.041 bytes | 574 KB |
| `tarjeta.maqueta` | 14 | 609 bytes | 361 KB |
| `escaparate.maqueta` (dos piezas) | 31 | 1.084 bytes | 870 KB |

La descripcion de una cara es unas **800 veces** mas chica que lo que pinta.
No hay arbol vivo, ni motor de maquetacion, ni script: hay una lista de cajas
ya calculadas. Con P3, cada ESTADO es otra lista asi (del orden de 1 KB por
tarjeta).

[!] **Lo que SI cuesta la vida, y no es RAM**: cada fotograma de una
animacion es CPU y vatios. Por eso cada pieza que anime lleva su letrero
`[consumo]` (L6h): animar cuando algo cambia -- un clic, un estado nuevo --,
no un bucle eterno de adorno. Que todo PUEDA tener vida no quiere decir que
todo se mueva a la vez: la elegancia de 04-10 es, sobre todo, lo que se
quita.

## 6f. MAQUETA EN LA 3060: `maqueta.bsf` Y LA CARA COMO DATOS (2026-10-04)

El propietario: *"se puede convertir en VRAM para mi GPU... un poco como
.bsf?"*. Si, y con la regla del BSF dicha por el: *"la GPU no compila nada;
de un fotograma a otro solo cambian los datos"*.

```
   maqueta.bsf   los PROGRAMAS del pintor, UNA vez para todo el sistema (como
                 `platform/drivers/gpu/ga10x/sombreadores/cubo.bsf`): caja
                 suave, borde, resplandor, degradado, letra de una textura,
                 trazo de SVG
   la CARA       los DATOS de cada maqueta: la lista de trazos que ya viaja
                 (609 bytes la tarjeta), en la VRAM como buffer de
                 INSTANCIAS -- una orden pinta la cara entera
```

La interfaz del BSF ya describe buffers como *"bytes fijos + paso del arreglo
sin medida"*: la forma exacta de la CARA. Y con P3, los estados son buffers en
la VRAM y cada fotograma cambia un numero.

```
   [x] G0  los pixeles a la VRAM por el motor de copia (el volcado por la
           3060, 25-09) -> `Ultra_userspace/userland/src/pantalla/roja.rs`
   [ ] G1  el formato del lado GPU: la CARA como buffer de instancias en la
           interfaz del BSF. Papel y pruebas en el anfitrion, sin tarjeta
           -> `toolchain/lang/spirv/bsf/` y `platform/shared/bmo-maqueta-cara/`
   [ ] G2  el primer programa: cajas lisas y degradados, a mano como el del
           cubo, contra el backend CPU de VERRANO
           -> `platform/shared/verrano/` y `platform/drivers/gpu/ga10x/`
   [ ] G3  esquinas suaves, resplandor y letra -- cuando existan V3 (las
           constantes), M3 (las texturas) y E3 (el emisor SPIR-V a SM86)
           -> `docs/plan/PLAN_VERRANO.md`, `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`
   [ ] G4  los estados de P3 en la VRAM: la CPU manda un numero por fotograma
```

La regla no cambia: **la foto de la 3060 tiene que salir igual que la de
`bmo-pinta`**, y el pintor de la CPU es el juez. [!] Hoy el emisor SM86 esta en
E1 (solo juzga el subconjunto) y el SASS del cubo se escribio A MANO: G2 se
escribe a mano igual, o espera a E3. Nada de esto se probo en el Ryzen.

⚠ **La estimacion de la conversacion estaba mal, y lo destapo la medida**:
contando NOMBRES de propiedad salia un 58,5 % "hoy". Era el 37,52 %:
`padding: 8px 12px` contaba porque `padding` existia, y el padre lo
rechazaba.

[!] **La trampa que salio al darle la letra al escritorio**: Cargo junta las
`features` por PAQUETE en cada compilacion, y `build.ps1` compila todo Ring 3
de una vez -- asi que el `alloc` que piden HERMES y BANK CAT le llegaba al
director, que no tiene monton, y no enlazaba. Ahora las apps con monton
piden `bmo-letra-monton`: el MISMO `lib.rs`, en otro paquete. Ver su
`Cargo.toml`.

**Y el marco de las ventanas (04-10)**: `marco = fino` (por defecto) o
`marco = hacker` en `sys/director.cfg`, y en vivo con `aspecto`. El fino
QUITA (scanlines, esquinas HUD, segmentos, el neon que corre): deja la barra
lisa, un filete del acento que se apaga hacia los extremos, botones redondos
con pluma suave y el burdeos de cerrar. Todo dentro del rectangulo de la
ventana, asi que el modelo de borrado (`scene_color`) no cambia. El hacker de
25-09 sigue entero a una linea: no se borro lo que se pidio antes.

## ⚠ LAS CASILLAS MENTIAN, Y SE RECONTARON EL 2026-08-24

Los cinco escalones de la cadena figuraban SIN HACER y estaban hechos, con su
banco verde cada uno:

```
   lex  24    node  36    cascade  22    layout  20    verdict  20
                                          = 122 filas, y ninguna roja
```

*** Y el propio `build.ps1` los ejercita en cada compilacion --"caras: 1
.maqueta, y su Rust generado dice lo mismo"-- o sea que la tuberia entera
funciona desde antes de que nadie marcara la casilla.

** Lo que revelo el recuento no es que sobrara trabajo: es que **la adopcion es
1**. El escritorio son 8.744 lineas de `scene/` y 500 salen de un `.maqueta`
--el 5,7%, y esa una es la calculadora. La maquina esta pagada y no la usa casi
nadie, que es una casilla que no existia porque nadie la habia escrito.

** El 9 pasa a `[~]`: `island` viaja de `node` a `cascade` y a `layout`, y
`Laid::islands` existe. Lo que NO hay es una superficie BSUP de verdad al otro
lado. **Parsear no es cablear**, y llamarlo hecho seria el error que este
recuento vino a arreglar.

Y el 6 se cobro el mismo dia con la PALETA: `tema.maqueta` confesaba en su
cabecera que era la fuente y las constantes de Rust la copia. Ya no hay copia.
```
```

**El escalon 6 es la prueba de que esto sirve, y tiene un numero -- afinado el
17-08 despues de contar**: de las 214 lineas de `calc.rs`, **118 son maquetacion**
(`CalcPad`, `button`, `key_at`, `contains`, las constantes y el cuerpo de
`paint_calc`). Las otras ~96 son la maquina de estados de la calculadora y **se
quedan en Rust para siempre**.

```
   118 lineas de maquetacion  ->  58 lineas de .maqueta      MEDIDO, -50%
```

### ⚠ Lo prometido era "un tercio", y no llega. La razon esta medida

Son **58 contra 118: la mitad**, no un tercio -- y el `.maqueta` ya incluye el
realce de `:hover` (en Rust, `lighten()` mas la constante `HIGHLIGHT`) y los
comentarios que explican por que cada cosa esta donde esta. Y no es que el compilador salga
mal -- es que **la calculadora es el PEOR CASO posible para MAQUETA**:

> `calc.rs` pinta veinte teclas con `for row { for col }` sobre una tabla de
> etiquetas. **Una rejilla regular YA ES maquetacion declarativa**, y ahi un
> bucle gana en lineas a veinte `<div>` escritos uno a uno.

Donde MAQUETA gana de verdad es en lo **irregular** -- `chrome.rs` (565 lineas,
el marco de las ventanas), la barra, los paneles -- que es donde no hay bucle que
valga. Elegir la calculadora primero fue elegir el caso que peor le sienta, y eso
es lo que hace que el numero sirva.

★ **Y las lineas no son lo que mas se cobra.** Lo que desaparece son
`CalcPad::button()`, `key_at()` y `contains()`: **tres funciones que son la misma
aritmetica escrita dos veces**, una para pintar y otra para responder al raton.
Eso no se reduce, se vuelve imposible.

★ Y el sitio donde mas se cobra no es el pintado: son `button()`, `key_at()` y
`contains()`. **Hoy la misma aritmetica esta escrita dos veces** -- una para
pintar la tecla y otra para saber que tecla se pulso -- y esa duplicacion es una
clase de bug entera (el boton que se dibuja en un sitio y responde en otro). El
compilador conoce el rect final, asi que emite **la lista de pintado y la tabla
de golpeo de una sola fuente**, y las tres funciones desaparecen.

---

## 7. LO QUE ESTO NO ES

- **No es un navegador.** No hay red (cero syscalls), no hay DOM, no hay script.
- **No es un motor de maquetacion en el aparato.** Todo el calculo ocurre en el
  anfitrion; en el Ryzen solo se leen rects ya calculados.
- **No es un lenguaje de aplicaciones.** Da la CARA de una app. Los clics, el
  estado y el foco siguen siendo Rust.
- **No es HTML.** Se le parece a proposito para poder usar un navegador de regla,
  y **rechaza** todo lo que no implementa. Un navegador ignora lo que no entiende;
  esto es lo contrario, y esa inversion es la definicion del proyecto.

Ver `docs/componente/LA_MAQUETA_EXIGE.md` (que acepta y que rechaza),
`docs/plan/PLAN_DIRECTOR.md` (las superficies, que son las islas) y
`META-KERNEL_HARD.md` L6 y L7 (la ley del reparto).

---

## LA SIGUIENTE VICTIMA: NINGUNA, Y ESO ES UN RESULTADO (2026-08-18)

Se midieron **las catorce ventanas** del escritorio para elegir la segunda cara.
El criterio se escribio antes de mirar, que es lo unico que separa una medida de
elegir al ganador:

```
   cara FIJA        su medida no depende de cuantos datos haya en ejecucion
   IRREGULAR        no es un for anidado sobre una rejilla
   POCO dato vivo   cada valor que cambia es una isla, y las islas no se ahorran
```

**Aptas: 0.**

| ventana | dibujos | se estira | una caja por dato |
|---|---|---|---|
| `data.rs` (ESTRATOS) | 93 | 12 | si |
| `splash.rs` | 41 | 6 | no |
| `sound.rs` | 17 | 1 | si |
| `cabina.rs` | 14 | 2 | no |
| `vitals.rs` | 12 | 1 | no |
| `chrome.rs` | 10 | 70 | no |

### Por que, en una frase

**MAQUETA compila coordenadas absolutas para UN medida, y las ventanas de este
escritorio se estiran.** La calculadora entro porque es la unica cara de medida
fijo que hay: no se redimensiona y no depende de la pantalla. No fue casualidad
que saliera la primera, y explica por que el numero fue -50% y no el tercio
prometido -- se eligio el caso mas facil porque era el unico.

### Y de que fallan las que estan mas cerca

De **una sola cosa**, y siempre la misma:

```rust
   c.chrome.y + c.chrome.height - bmo::GLIFO_ALTO - 8    // vitals, cabina
   if x + TESTIGO_W >= p.ancho                           // testigo
```

Un renglon pegado al **borde de abajo**. En CSS eso es `bottom:8px`, y el
contrato de hoy exige `left` y `top` con `position:absolute` -- **no hay
`bottom` ni `right`**.

### Las dos salidas, y la segunda respeta la ley

1. **Las ventanas dejan de estirarse.** Es una decision de producto, no tecnica,
   y se pagaria en usabilidad.
2. ★★ **MAQUETA emite el ANCLA, no solo la coordenada.** Una caja anclada abajo
   sale como *"tu `y` es `alto_del_marco - 30`"*, y quien pinta hace **una resta**
   -- no una maquetacion. Eso NO es el motor en ejecucion que la ley prohibe:
   no se vuelve a medir nada, no se recalcula ningun flujo, y el arbol sigue
   resuelto en el anfitrion. Es la misma prueba que paso `:hover`: admisible por
   la condicion, no por util.

### [!] CORRECCION DEL MISMO DIA: las anclas NO convierten a esas tres

La tabla de arriba decia que `vitals` y `cabina` no dibujan una caja por dato, y
**era un fallo de la medida**, no de las ventanas. El barrido buscaba bucles de
la forma `while x < hijos` y estas dicen otra cosa:

```rust
   while row_n < 16                          // vitals: UNA FILA POR PROCESO
   while painted_count < count && i < any    // cabina: UNA FILA POR EVENTO
```

Las dos son **listas**, igual que ESTRATOS. Lo que se puede maquetar de ellas es
su cabecera y su pie -- cuatro cajas-- mientras el 90% que importa se queda en
Rust. Portarlas seria trabajo que **parece** progreso.

Con el agujero tapado, la cuenta vuelve a salir la misma: **aptas, cero**. Y la
conclusion se afila en vez de cambiar:

> ★★ **El escritorio de BMO-X esta hecho de LISTAS, no de caras.** La calculadora
> es la unica cara porque es lo unico cuyo contenido no sale de un dato.

Las anclas siguen siendo correctas y siguen sin tener consumidor. Y esta casa ya
sabe lo que pasa con el codigo escrito sin quien lo llame --el `pedir_lectura`
que se borro antes de entrar--: **no se escriben.**

[!] Y ESTRATOS **sigue sin calificar aunque se agreguen anclas**: dos de sus tres
vistas dibujan una caja por hijo del volumen, y cuantos hay se sabe en ejecucion.
Eso no es un limite del compilador -- es lo que separa una CARA de una LISTA.
