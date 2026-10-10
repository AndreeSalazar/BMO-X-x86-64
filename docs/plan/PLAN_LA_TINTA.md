# PLAN LA TINTA -- ADOBE GENERAL + CLIP STUDIO PAINT, con libros y manga en total

> Abierto el 2026-10-07 a peticion del propietario, despues de probar el
> escritorio de mision en su Ryzen: *"puedes analizar como se hizo Clip Studio
> Paint para hacer manga y Adobe Photoshop? para tener ese plan en mi BMO-X y
> asi TITAN++ haga ese trabajo pesado como mi primer test"*.
>
> Y el mismo dia, al leer la primera version: *"solo es plan, no dije que eso
> sea ahora ... solo poner eso en plan ADOBE GENERAL + CLIP STUDIO PAINT CON
> LIBROS Y MANGAS EN TOTAL, eso nada mas, para tener control propio"*. Es
> **para sus necesidades, no para un mercado**: dibujar sus personajes, su
> manga y sus libros con herramientas que son suyas.
>
> **Es un plan LARGO y no es para ahora.** Este fichero es el analisis y la
> escalera entera; se empieza el dia que el propietario lo diga.

---

# 0. LA IDENTIDAD, y lo que contesto el propietario

[`../identidad/LOS_TRES_VERTICES.md`](../identidad/LOS_TRES_VERTICES.md),
seccion 5, dice:

> **Un sistema que promete Photoshop es un sistema que no termina la calculadora.**

El propietario, al leerlo: *"la ironia no es imposible"*. Las dos cosas son
verdad a la vez, y el plan las junta asi:

```text
   LA META          el TOTAL: lo de Adobe que hace falta para dibujar, pintar
                    y maquetar (Photoshop, Illustrator, InDesign) + Clip
                    Studio Paint, con libros y manga enteros
   EL ORDEN         el NUCLEO primero (lo que se usa cada dia), el total
                    despues, peldanio a peldanio: cada uno se acaba y se
                    usa antes de subir al siguiente
   LO QUE NO        un clon para vender: ni sus menus, ni sus formatos
```

Lo que la frase advierte -- prometer una suite y no acabar nada -- se evita
con el orden, no renunciando a la meta. Si la frase se reescribe es del
propietario, con `--sellar` (decision D1).

---

# 1. COMO ESTA HECHO PHOTOSHOP

Lo que importa no es la lista de menus, es **la maquina de debajo**. Son cinco
piezas, y todo lo demas son herramientas encima de ellas.

## 1a. El documento: capas de TESELAS, no una imagen

Un documento es una pila de capas, y **cada capa no es un bloque de pixeles:
es un mosaico de teselas** (cuadrados de pixeles de una sola capa). Photoshop
tiene su propia memoria virtual: las teselas viven en paginas, y las que no
caben en la RAM bajan a ficheros de paso en el disco (el *scratch disk*).
Cada operacion RECORRE TESELAS: la memoria virtual se ocupa de que la tesela
de origen y la de destino esten en memoria en cada paso. Es lo que deja
editar un fichero mas grande que la RAM.

```text
   documento
     |-- capa "fondo"      [T][T][T][T]     una tesela = 256 x 256 (o asi)
     |-- capa "tinta"      [T][ ][ ][T]     una tesela VACIA no ocupa nada
     '-- capa "color"      [ ][T][T][ ]
```

Y esto es lo que hace barato todo lo demas:

* **Lo vacio no cuesta.** Una capa de tinta en una pagina tiene casi todo
  transparente: solo existen las teselas que tocaste.
* **Deshacer es guardar teselas.** El historial no copia la imagen: copia
  las teselas que un trazo cambio, antes de cambiarlas.
* **Trabajar es local.** Un trazo toca diez teselas, no la pagina entera.

## 1b. La fusion: una formula por pixel

Pintar la pantalla es COMPONER: de abajo arriba, cada capa se mezcla con lo
que hay debajo con su modo y su opacidad. Cada modo es una formula de un
pixel, sin vecinos:

```text
   normal       r = a*s + (1-a)*d
   multiplicar  r = s*d                 (la tinta y las sombras)
   pantalla     r = 1 - (1-s)*(1-d)     (las luces)
   superponer   multiplicar o pantalla segun d
```

Encima van las MASCARAS (una capa de grises que dice cuanto se ve), el
RECORTE (una capa que solo pinta donde pinta la de abajo) y las capas de
AJUSTE (curvas, niveles, tono: una formula que se aplica al componer, sin
tocar los pixeles -- lo no destructivo).

## 1c. La seleccion: otra capa de grises

Una seleccion es una mascara de 8 bits (las "hormigas" son solo su borde
pintado). Por eso se puede difuminar, sumar, restar y guardar. El cubo de
pintura, la varita magica y el lazo solo FABRICAN esa mascara.

## 1d. Los algoritmos gordos

Lo que hace a Photoshop Photoshop son cuatro o cinco algoritmos con nombre y
apellido, publicados:

| herramienta | algoritmo | desde |
|---|---|---|
| pincel corrector | edicion de Poisson: resolver una ecuacion para que el parche herede la luz de alrededor (Todor Georgiev) | Photoshop 7, 2002 |
| relleno segun contenido | PatchMatch: buscar al azar parches parecidos y propagar los buenos (Barnes, Shechtman, Finkelstein, Goldman) | CS5, 2010 |
| desenfoque gaussiano | separable: una pasada horizontal y otra vertical | siempre |
| licuar, deformar | una malla que se deforma y se re-muestrea | |

Todos tienen algo en comun: **leen VECINOS**. Eso importa para la 3060
(seccion 3).

## 1e. La pantalla, en la GPU

El lienzo se pinta en la GPU: acercar, alejar y girar son texturas y
piramides de resoluciones (*mipmaps*), no recalcular la imagen. Algunos
filtros (desenfoques de la galeria, pintura al oleo, Camera Raw) tambien
corren alli. El resto, en la CPU.

---

# 2. COMO ESTA HECHO CLIP STUDIO PAINT

**La misma maquina de debajo** (capas, teselas, fusion, mascaras, seleccion),
y su formato lo confirma: un `.clip` es una base de datos SQLite con los
datos del documento, y los pixeles en bloques de 256 x 256 comprimidos que se
descomprimen cuando hacen falta (lo midieron quienes lo leyeron por fuera:
`unclip`, `clipparse`, `clipstudio.js`). Lo que lo hace ser *el de manga* es
lo que pone ENCIMA:

## 2a. El motor de pincel (lo que mas se nota)

Un trazo no es una linea: es una fila de **sellos** (*dabs*). El lapiz manda
puntos con su presion; el motor los suaviza, los reparte a un espaciado fijo
y estampa la punta en cada uno:

```text
   el lapiz        (x, y, presion, inclinacion, tiempo)  a 200-1000 por segundo
   ESTABILIZADOR   media de los ultimos N puntos: la linea temblorosa sale lisa
   ESPACIADO       un sello cada 5-10 % del diametro, a lo largo de la curva
   CURVAS          presion -> tamanio, presion -> opacidad (una curva por cosa)
   LA PUNTA        un circulo suave, o una imagen (textura de lapiz, pincel)
   ANTIALIAS       el borde del sello cubre parte de un pixel
```

El "entintado bonito" de CSP es casi todo esto: el estabilizador y la curva
de presion. Con raton, sin presion, se pierde la mitad.

## 2b. La tinta vectorial

Una capa VECTORIAL no guarda pixeles: guarda **los trazos** (los puntos de
control con el ancho de cada punto) y los pinta al vuelo a cualquier zoom.
Por eso:

* la **goma hasta la interseccion** borra un trozo de trazo justo hasta donde
  cruza otro (limpiar los cruces del entintado es un clic);
* **cambiar el grosor** de una linea ya hecha es cambiar un numero;
* acercar no pixela.

## 2c. Lo que es SOLO de manga

| pieza | que es por dentro |
|---|---|
| **vinetas** (*koma*) | una capa de MARCO: poligonos con borde; cada vineta es una carpeta que RECORTA lo de dentro |
| **tramas** (*screentone*) | una PROPIEDAD de capa: los grises se convierten en puntos de semitono (lineatura, angulo, forma), como el papel de trama de imprenta |
| **relleno con referencia** | el cubo mira OTRA capa (la de tinta) para saber donde parar, y **cierra huecos**: un hueco de pocos pixeles en la linea no deja escapar el color |
| **reglas** | perspectiva de 1, 2 y 3 puntos, simetria, y las de efecto de manga: lineas de velocidad y concentricas (el "zoom" de un golpe) |
| **texto y globos** | el texto vertical japones, y el globo como forma con su rabito |
| **paginas** | una obra son N paginas con su sangrado y su zona segura, y se exporta a 600 ppp en blanco y negro de 1 bit para imprenta |
| **modelos 3D** | maniquies para posar (fuera de este plan) |

---

## 2d. LO DE ADOBE QUE NO ES PHOTOSHOP: el vector y los LIBROS

"Adobe general" son tres maquinas distintas, y Photoshop es solo la primera:

```text
   PHOTOSHOP     PIXELES     capas de teselas, fusion, filtros   (seccion 1)
   ILLUSTRATOR   VECTOR      curvas de Bezier con relleno y trazo
   INDESIGN      PAGINAS     el libro: texto que fluye por las paginas
```

**Illustrator**: un dibujo es una lista de CAMINOS (curvas de Bezier
cubicas) con su relleno y su trazo; se pintan al vuelo a cualquier tamanio.
Lo gordo es la geometria: unir, restar e intersecar formas (operaciones
booleanas entre curvas), desplazar un trazo (*offset*) y las mesas de trabajo.
Es la misma familia que la tinta vectorial de Clip Studio (2b), con una
diferencia: alli el trazo tiene ancho por punto (presion), aqui es una forma.

**InDesign** (los LIBROS): un documento son PAGINAS que heredan de paginas
MAESTRAS (el numero de pagina, la cabecera, los margenes), y el texto vive en
MARCOS enlazados: lo que no cabe en uno pasa al siguiente, de pagina en
pagina. Encima van los ESTILOS (de parrafo y de caracter: cambiar uno cambia
el libro entero), las imagenes ancladas al texto, y la composicion del
parrafo: InDesign reparte los cortes de linea mirando el parrafo ENTERO, no
linea a linea, que es la idea del algoritmo de Knuth y Plass de TeX -- es lo
que hace que un libro justificado no tenga rios de blanco. Y los guiones,
por idioma. Sale a PDF, el formato de imprenta.

**Clip Studio Paint EX** tambien hace libros, del lado del manga: una OBRA de
muchas paginas, con su orden, su numeracion y sus pliegos, que sale a
imprenta o a libro electronico.

---

# 3. QUE TIENE BMO-X HOY, Y QUE FALTA

Medido en el arbol el 07-10, sin adivinar:

| hace falta | hoy en BMO-X | falta |
|---|---|---|
| **un lapiz con presion** | NO. El USB del kernel lee teclado y raton; ningun digitalizador. El propietario tiene una **WACOM basica** | el driver de SU Wacom (ver 3b). Es metal: se prueba en el Ryzen |
| **una ventana desde TITAN++** | NO. La GRAMATICA lo dice: *"el primer hola en la CONSOLA; la ventana, despues"*. TITAN++ escribe con `print` y lee con `lee()` | un nodo `screen`: pedir una superficie, escribir pixeles, presentarla |
| **el raton/lapiz de la ventana** | NO desde TITAN++: *"el teclado y el raton de una VENTANA son otra cosa (`input`, con REX)"* | el nodo `input` de la ventana |
| **leer y guardar ficheros** | NO desde TITAN++ | un nodo de disco: guardar la obra en ESTRATOS y exportar PNG |
| **pixeles baratos** | TITAN++ cuenta en `int` de 64 bits y `dec`; `f32` solo dentro de una `gpu fn`. No hay byte | un tipo de 8 bits y tablas grandes en un bloque de memoria (ver la cuenta de abajo) |
| **la 3060 para la fusion** | SI en el lenguaje: una `gpu fn` es UNA celda, un hilo por celda, sin vecinos ni bucles. Correr en la 3060 de verdad es G4 (del propietario), y VERRANO V0 son 8 triangulos sin mezcla | nada para la fusion (es de una celda); para desenfocar y corregir, leer VECINOS (bucles en la `gpu fn`, que el SPIR-V de hoy no tiene) |
| **el historial** | **SI, y mejor que nadie**: ESTRATOS es copiar-al-escribir. Una tesela es un bloque; deshacer es volver a la version de antes | conectar las teselas a ESTRATOS |
| **la letra** | la de la casa (`bmo-letra`), proporcional y suave | el texto vertical, para los globos |

> **08-10, al dia (H4 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md)):**
> la fila de la 3060 se midio antes de quitar SPIR-V (07-10). La gpu fn baja
> hoy al Programa de la casa, y los bucles ya los sabe el emisor SM86 (E6):
> lo que los rechaza es TITAN++ (LB5). Los vecinos piden un asa del kernel
> (LB9), y correr en la 3060 de verdad, la PUERTA de computo de una app (LB8;
> la QMD corre desde el 24-09).

## La cuenta que manda: una pagina de manga

```text
   B4 a 600 ppp (lo de imprenta)      6.071 x 8.598  =  52 millones de pixeles
   una capa RGBA de 8 bits            209 MB
   una capa de grises de 8 bits        52 MB
   la tinta en 1 bit                  6,5 MB
   la misma pagina en [int] de 64     420 MB por capa   <- por eso hace falta el byte
   en teselas de 256                  24 x 34 = 816 teselas; una capa de tinta
                                      toca, a ojo, una de cada tres
```

Con teselas y sin las vacias, una pagina de cinco capas cabe holgada. Sin
teselas y en `int`, no cabe ni una.

---

## 3b. La Wacom: el driver mas chico que se puede escribir

El propietario: *"tengo tableta grafica, eso podria ayudar a estudiar a crear
propio driver"*. Lo es, y por la razon buena: comparado con el GSP de la 3060
(firmware, colas, mensajes, `PLAN_LA_3060.md`), una tableta es de lo mas
pequenio que hay. Lo que pide:

```text
   1. QUIEN ES          su VID:PID por USB (Wacom es 056A), del censo USB
                        que el kernel ya hace (`INFO_USB_CENSO`)
   2. COMO HABLA        su descriptor HID: las tabletas basicas (One by Wacom,
                        Intuos S) mandan x, y, presion y botones en informes
                        de pocos bytes; algunas en el modo HID de
                        digitalizador (pagina 0x0D) y otras en el suyo propio
                        hasta que se les pide otro. Se MIDE con la tableta
                        enchufada antes de escribir una linea
   3. QUE DA            x, y en su rejilla (miles de puntos), la presion
                        (2048 o 4096 niveles segun el modelo), la punta, el
                        boton lateral y, si la hay, la goma del lapiz
   4. A DONDE VA        al mismo camino que el raton (`input`), con la
                        presion de mas: la ventana que tiene el foco la recibe
```

Se escribe para ESA tableta primero (su modelo exacto es D3), y solo despues
se generaliza.

---

# 4. EL REPARTO: que hace TITAN++ y que no

La peticion es que **TITAN++ haga el trabajo pesado**. Se toma al pie de la
letra, con una linea clara:

```text
   TITAN++ (la app entera, un paquete con su Titan.toml)
     el motor de pincel     sellos, espaciado, estabilizador, curvas
     la fusion              los modos, capa a capa   -> gpu fn: una celda
     las tramas             gris -> punto            -> gpu fn: una celda
     el relleno             la inundacion y cerrar huecos
     la tinta vectorial     los trazos y la goma hasta la interseccion
     el historial           que teselas cambio cada trazo
     el formato             la obra en ESTRATOS, PNG de salida

   EL RESTO (Rust y kernel): SOLO PUERTAS
     la tableta             el driver USB HID, en el kernel
     la ventana             la superficie y su presentar, del DIRECTOR
     el disco               ESTRATOS, el que ya hay
```

Cada `gpu fn` de la fusion pasa por el oraculo que ya existe (E0 == E1 == el
interprete de SPIR-V, bit a bit) antes de tocar la 3060. Y la fusion tiene
SIEMPRE su camino en la CPU: sin la 3060 despierta, se dibuja igual, mas
despacio. Ninguna parte de la mesa depende de que la 3060 este viva.

> **08-10 (H4 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md)):** el
> oraculo que ya existe es la 3060 simulada sobre el SASS juzgado, no el
> interprete de SPIR-V (07-10). Y E1 todavia NO corre una gpu fn (T0040): el
> camino en la CPU de una gpu fn es LB4, la CPU como libreria.

**Por que es un buen primer test de TITAN++**: un programa de dibujo pide
justo lo que un lenguaje nuevo tiene que demostrar -- bucles calientes sobre
millones de celdas (velocidad de E1), memoria grande sin fugas (el juez de
prestamos), la `gpu fn` en serio (la fusion) y entrada en vivo (latencia). Si
TITAN++ entinta una pagina sin trabarse, ha pasado el examen.

---

# 4b. EL FORMATO PROPIO: la `.obra`

> El propietario, 07-10: *"me gustaria inspirarme de .psd ... pero que se
> pueda aplicar exclusivo en BMO-X, pero ese guardado tiene git propio"*. Y
> al leer la propuesta: *".obra me encanta"*.

## Lo que se copia del `.psd`, y lo que no

Un `.psd` es UN fichero binario, en este orden:

```text
   cabecera "8BPS"  ->  modo de color  ->  recursos (guias, perfil ICC...)
   ->  capas y mascaras  ->  la imagen YA COMPUESTA, al final
```

* **Se copia** la imagen compuesta: cualquiera que no entienda las capas la
  puede mostrar igual. En la `.obra` es `vista.png`.
* **No se copia** que sea un bloque: cambiar un pixel reescribe el fichero
  entero, y guardar encima pierde lo de antes. No tiene historia.

## La `.obra` es un NODO de ESTRATOS, no un fichero

```text
   mi_manga.obra/
     obra.txt          el manifiesto: tamanio, paginas, orden de las capas,
                       su modo y su opacidad (texto, como `director.cfg`)
     vista.png         la compuesta: F2 la muestra sin entender nada mas
     pag01/
       fondo/          una CAPA = una carpeta
         t_03_07       una TESELA de 256 = un fichero, solo las pintadas
       tinta/
       tramas/
     pag02/
       ...
```

## Y el "git propio" sale de lo que ESTRATOS ya es

| lo de git | en la `.obra` | lo pone |
|---|---|---|
| un commit | cada GUARDADO es una version; las teselas que no cambiaron se COMPARTEN con la anterior (copiar-al-escribir), asi que cien guardados de una pagina no pesan cien paginas | ESTRATOS |
| una etiqueta | marcar una version con nombre ("entintado listo"): queda PERMANENTE | `estratos::marcar` |
| una rama | "boceto-A" y "boceto-B" de la misma escena, o probar otro color sin miedo | `PLAN_LAS_RAMAS.md` |
| la mezcla | por NODOS, y cada capa es un nodo: el fondo pintado en una rama y la tinta en otra se juntan solas; solo pregunta si las dos tocaron la MISMA capa (y se puede afinar hasta la misma tesela) | la mezcla de R5, con su candado |
| el historial | la TRAYECTORIA de F12 (HM6d): cada guardado un encendido, las marcadas puntos de paso, las ramas juntandose en la MEZCLA | `scene/trayectoria.rs` |

**Lo que ni Adobe ni Clip Studio tienen**: Photoshop guarda una FOTO del
dibujo; la `.obra` guarda la HISTORIA del dibujo, con ramas. Clip Studio
tiene historial de deshacer, pero ni ramas ni mezcla.

---

# 5. LA ESCALERA

Cada peldanio es UNA cosa que se puede probar sola. El orden es el de la
casa: primero lo que no toca nada, despues lo que pide el metal.

## 5a. Lo que TITAN++ necesita antes (el lenguaje y sus puertas)

- [ ] TA0 -- las decisiones de la seccion 7, contestadas por el propietario
- [ ] TA1 -- el BYTE: un tipo de 8 bits sin signo y las tablas grandes en un bloque de memoria pedido (`[u8]` de millones de celdas), con su codigo T y su ejemplo en `toolchain/lang/titan/ejemplos/`; nivel nuevo de la GRAMATICA
- [x] TA2 -- la VENTANA: el nodo `screen` de TITAN++ (pedir superficie, escribir una fila de pixeles, presentar), con su permiso en el Titan.toml y su linea en el certificado. **HECHA el 10-10** (F1 de [`EL_FOCO.md`](EL_FOCO.md)): `director.ventana`, `pixel`, `rect`, `fila` y `presenta`
- [x] TA3 -- la ENTRADA de la ventana: el nodo `input` (raton: x, y, botones; y la presion cuando llegue TC1), con REX. **HECHA el 10-10** (F3 de [`EL_FOCO.md`](EL_FOCO.md)): `director.evento`, `codigo`, `raton_x`, `raton_y`, `botones` y `se_ve`; la presion de la Wacom, cuando llegue TC1
- [ ] TA4 -- el DISCO: leer y escribir ficheros de ESTRATOS desde TITAN++, con permiso
- [ ] TA5 -- la MEDIDA: un banco de velocidad de E1 sobre un bucle de pixeles (cuantos millones de celdas por segundo), escrito ANTES de prometer latencia

## 5b. EL PRIMER TEST: un lienzo que funciona (con raton)

- [ ] TB1 -- EL LIENZO MINIMO en TITAN++: una capa, un pincel redondo con antialias, la goma, y exportar PNG. La prueba: el tiempo de un trazo a lo largo de la pantalla, medido
- [ ] TB2 -- CAPAS Y FUSION: N capas con opacidad y los modos normal, multiplicar y pantalla; la fusion como `gpu fn` con su camino en la CPU, y el oraculo de los tres
- [ ] TB3 -- TESELAS E HISTORIAL: capas en teselas de 256 (las vacias no existen) y deshacer/rehacer sobre ESTRATOS
- [ ] TB5 -- LA `.obra` (seccion 4b): el manifiesto `obra.txt`, una carpeta por capa, un fichero por tesela pintada y la `vista.png` compuesta; leerla y escribirla desde TITAN++, con su banco en el anfitrion
- [ ] TB6 -- GUARDAR ES UNA VERSION: cada guardado publica un estrato, las teselas sin tocar se comparten, y marcar con nombre la hace permanente
- [ ] TB7 -- RAMAS Y MEZCLA DE LA OBRA: una rama por intento, y la mezcla por capas (cada capa un nodo de R5); la pregunta solo cuando dos ramas tocaron la misma capa, y despues afinarla a la misma tesela
- [ ] TB8 -- LA TRAYECTORIA DE LA OBRA: F12 (`scene/trayectoria.rs`) pinta la historia de una `.obra`, con la miniatura de cada version sacada de su `vista.png`
- [ ] TB4 -- SELECCION Y RELLENO: la mascara de seleccion, el cubo con referencia a otra capa y "cerrar huecos"

## 5c. Manga

- [ ] TC1 -- LA WACOM: medir su VID:PID y su descriptor HID, y escribir el driver para ESE modelo (x, y, presion, punta, boton lateral, goma si la tiene) hacia `input` (ver 3b). Del METAL: lo prueba el propietario con SU tableta
- [ ] TC2 -- EL MOTOR DE PINCEL: sellos con espaciado, estabilizador, curvas de presion a tamanio y opacidad, y puntas de imagen
- [ ] TC3 -- LA TINTA VECTORIAL: trazos guardados como puntos con ancho, pintados al vuelo; la goma hasta la interseccion y cambiar el grosor
- [ ] TC4 -- LAS VINETAS: la capa de marco, con poligonos que recortan lo de dentro
- [ ] TC5 -- LAS TRAMAS: la propiedad de capa que convierte grises en puntos (lineatura, angulo), como `gpu fn` de una celda
- [ ] TC6 -- LAS REGLAS: perspectiva de 1, 2 y 3 puntos, simetria, y lineas de velocidad y concentricas
- [ ] TC7 -- TEXTO Y GLOBOS: la letra de la casa en vertical, y el globo con su rabito
- [ ] TC8 -- LA OBRA: N paginas con sangrado y zona segura, y la salida para imprenta (600 ppp, 1 bit)

## 5d. Lo de Photoshop

- [ ] TD1 -- LOS VECINOS en la 3060: `gpu fn` que lee celdas de al lado (pide bucles y vecinos en una `gpu fn`: G2 de `PLAN_EL_CENTAURO.md` escribe SPIR-V en linea recta, y el emisor SM86 los aprende en E6 de `PLAN_LA_LENGUA_DE_LA_3060.md`); el primero, el desenfoque gaussiano separable
- [ ] TD2 -- CAPAS DE AJUSTE: curvas y niveles que se aplican al componer, sin tocar pixeles
- [ ] TD3 -- MASCARAS Y RECORTE: la mascara de capa y la capa que solo pinta donde pinta la de abajo
- [ ] TD4 -- EL PINCEL CORRECTOR: edicion de Poisson
- [ ] TD5 -- EL RELLENO SEGUN CONTENIDO: PatchMatch
- [ ] TD6 -- TRANSFORMAR Y DEFORMAR: escalar, girar, perspectiva y la malla de licuar, con re-muestreo

> **08-10 (H4 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md)):** TD1 ya no
> espera a un SPIR-V en linea recta (quitado el 07-10) ni a E6 (hecho en el
> anfitrion): espera a que TITAN++ deje escribir bucles en una gpu fn (LB5) y
> a los vecinos por el asa del kernel (LB9).

## 5e. Lo de Illustrator: el vector

- [ ] TE1 -- CAMINOS: curvas de Bezier con relleno y trazo, editables punto a punto, pintadas al vuelo (comparte el pintor con la tinta vectorial de TC3)
- [ ] TE2 -- LA GEOMETRIA: unir, restar e intersecar formas, y desplazar un trazo
- [ ] TE3 -- MESAS DE TRABAJO y la salida en SVG (el lector de SVG de MAQUETA ya existe: `toolchain/tools/maqueta`)

## 5f. Los LIBROS

- [ ] TF1 -- PAGINAS Y MAESTRAS: el documento de N paginas que heredan de su maestra (margenes, cabecera, numero de pagina)
- [ ] TF2 -- MARCOS DE TEXTO ENLAZADOS: el texto fluye de un marco al siguiente, de pagina en pagina
- [ ] TF3 -- ESTILOS de parrafo y de caracter, con la letra de la casa
- [ ] TF4 -- LA COMPOSICION: cortes de linea mirando el parrafo entero (la idea de Knuth y Plass) y los guiones del castellano
- [ ] TF5 -- IMAGENES EN LA PAGINA: las paginas de manga (TC8) y las ilustraciones entran al libro
- [ ] TF6 -- LA SALIDA: PDF para imprenta, y libro electronico

# 6. LO QUE ESTE PLAN NO HACE

* **No es un clon ni es para vender.** Ni los menus, ni los atajos de Adobe o
  de Celsys. Es para las necesidades del propietario.
* **Ni `.psd` ni `.clip`** (decidido el 07-10): *"con ABI se puede pero
  sinceramente no voy a meterme con ellos"*. La obra se guarda en su `.obra`
  (seccion 4b), y sale en PNG, SVG y PDF, que lee todo el mundo.
* **Nada de IA generativa, nube, plugins de terceros ni modelos 3D.**
* **Nada de color de imprenta (CMYK) ni de 16/32 bits al principio.** Manga en
  blanco y negro y en 8 bits primero: es lo que se imprime.
* **No adelanta el metal.** La Wacom (TC1) y la 3060 de verdad (G4) son del
  propietario y de su Ryzen; todo lo demas se prueba en el anfitrion antes.

---

# 7. LAS DECISIONES DEL PROPIETARIO

Ninguna corre prisa: se contestan el dia que se empiece.

```text
   D1  LA IDENTIDAD     la frase de LOS_TRES_VERTICES ("un sistema que promete
                        Photoshop...") se queda, o se reescribe con --sellar?
                        (el propietario, 07-10: "la ironia no es imposible")
   D2  EL NOMBRE        como se llama la app y en que tecla vive (F1..F12 ya
                        estan todas: una nueva en la rejilla, o dentro de F1)
   D3  LA TABLETA       CONTESTADA EN PARTE (07-10): una WACOM basica. Falta
                        el modelo exacto (en Windows: Administrador de
                        dispositivos, o la etiqueta de abajo: CTL-xxxx)
   D4  LA PAGINA        el tamanio objetivo: B4 a 600 ppp en blanco y negro
                        (manga de imprenta), o 350 ppp en color
   D5  EL PRIMER TEST   TB1 con raton, o esperar a la Wacom
   D6  LOS FORMATOS     CONTESTADA (07-10): ni .psd ni .clip; lo propio es
                        la `.obra` (seccion 4b, "me encanta"), y sale en PNG,
                        SVG y PDF
```

---

# 8. DE DONDE SALE LO DE ARRIBA

* Photoshop y sus teselas, su memoria virtual y sus ficheros de paso: [How Photoshop solved working with files larger than can fit into memory](https://developer.chrome.com/blog/how-photoshop-solved-working-with-files-larger-than-can-fit-into-memory/) (Chrome for Developers, sobre Photoshop en la web).
* El pincel corrector: [Photoshop Healing Brush: a Tool for Seamless Cloning](https://ftp.fau.de/gimp/references/Photoshop_Healing_Brush_a_Tool_for_Seamless_Clonin.pdf) (Georgiev) y [Todor Georgiev](https://en.wikipedia.org/wiki/Todor_Georgiev).
* El relleno segun contenido: [PatchMatch](https://www.cs.princeton.edu/research/techreps/885) (Barnes, Princeton) y [su entrada en Photoshop CS5](https://imaging-resource.com/NEWS/1256272706.html).
* El formato `.clip` leido por fuera (solo para entender como esta hecho; no se lee, D6): [unclip](https://github.com/unai-d/unclip), [clipparse](https://pypi.org/project/clipparse/), [clipstudio.js](https://awesome.ecosyste.ms/projects/github.com%2Fsaitolume%2Fclipstudio.js).
* La tinta vectorial y la goma hasta la interseccion: [Vector Layer, guia completa](https://tips.clip-studio.com/ja-jp/articles/7586?org=1) y [Useful Vector Layers tips](https://tips.clip-studio.com/en-us/articles/7572) (CLIP STUDIO TIPS); vinetas, tramas y globos: [What tools are available?](https://www.clipstudio.net/en/comics-manga/tool/layers/).
* El motor de pincel por sellos, la fusion por formulas y el relleno con
  cierre de huecos son conocimiento comun del oficio (Krita, GIMP y
  MyPaint, que son abiertos, hacen lo mismo y se pueden leer).
* Lo de BMO-X: `toolchain/lang/titan/GRAMATICA.md` (niveles 11 y 12), el USB
  del kernel (`Ultra_kernel_x86-64/kernel/src/ring0/dev/usb/`) y
  `docs/identidad/LOS_TRES_VERTICES.md`, leidos el 07-10.
