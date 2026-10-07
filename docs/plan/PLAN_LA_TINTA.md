# PLAN LA TINTA -- dibujar manga en BMO-X, y que TITAN++ cargue con lo pesado

> Abierto el 2026-10-07 a peticion del propietario, despues de probar el
> escritorio de mision en su Ryzen: *"puedes analizar como se hizo Clip Studio
> Paint para hacer manga y Adobe Photoshop? para tener ese plan en mi BMO-X y
> asi TITAN++ haga ese trabajo pesado como mi primer test"*.
>
> Este plan es el ANALISIS y la escalera. **No se escribe codigo hasta que el
> propietario conteste las decisiones de la seccion 7.**

---

# 0. PRIMERO, LA CONTRADICCION: la identidad dice que NO

[`../identidad/LOS_TRES_VERTICES.md`](../identidad/LOS_TRES_VERTICES.md),
seccion 5, lo dice con todas sus letras:

> **Un sistema que promete Photoshop es un sistema que no termina la calculadora.**

Y tiene razon en lo que dice: Photoshop son treinta y cinco anios de
herramientas, y prometerlo entero es no acabar nunca. Asi que este plan **no
promete Photoshop ni Clip Studio Paint**. Promete otra cosa, mas chica y que
si se acaba:

```text
   LO QUE SE PROMETE      una MESA DE DIBUJO para manga, escrita en TITAN++:
                          capas, pincel con presion, tinta vectorial, vinetas,
                          tramas, relleno, historial, y la pagina lista para
                          imprimir. El nucleo que se usa todos los dias.
   LO QUE NO              la suite entera: 3D, IA generativa, nube, plugins,
                          video, CMYK de imprenta, los 200 filtros.
```

La frase de la identidad no la cambia este plan: la cambia el propietario con
`--sellar` si quiere (decision D1). Mientras tanto, la frase y este plan dicen
lo mismo: **el nucleo, no la suite**.

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

# 3. QUE TIENE BMO-X HOY, Y QUE FALTA

Medido en el arbol el 07-10, sin adivinar:

| hace falta | hoy en BMO-X | falta |
|---|---|---|
| **un lapiz con presion** | NO. El USB del kernel lee teclado y raton; ningun digitalizador (la pagina HID 0x0D de las tabletas) | el driver de la tableta, con presion e inclinacion. Es metal: se prueba en el Ryzen |
| **una ventana desde TITAN++** | NO. La GRAMATICA lo dice: *"el primer hola en la CONSOLA; la ventana, despues"*. TITAN++ escribe con `print` y lee con `lee()` | un nodo `screen`: pedir una superficie, escribir pixeles, presentarla |
| **el raton/lapiz de la ventana** | NO desde TITAN++: *"el teclado y el raton de una VENTANA son otra cosa (`input`, con REX)"* | el nodo `input` de la ventana |
| **leer y guardar ficheros** | NO desde TITAN++ | un nodo de disco: guardar la obra en ESTRATOS y exportar PNG |
| **pixeles baratos** | TITAN++ cuenta en `int` de 64 bits y `dec`; `f32` solo dentro de una `gpu fn`. No hay byte | un tipo de 8 bits y tablas grandes en un bloque de memoria (ver la cuenta de abajo) |
| **la 3060 para la fusion** | SI en el lenguaje: una `gpu fn` es UNA celda, un hilo por celda, sin vecinos ni bucles. Correr en la 3060 de verdad es G4 (del propietario), y VERRANO V0 son 8 triangulos sin mezcla | nada para la fusion (es de una celda); para desenfocar y corregir, leer VECINOS (G2, el escritor de SPIR-V con bucles) |
| **el historial** | **SI, y mejor que nadie**: ESTRATOS es copiar-al-escribir. Una tesela es un bloque; deshacer es volver a la version de antes | conectar las teselas a ESTRATOS |
| **la letra** | la de la casa (`bmo-letra`), proporcional y suave | el texto vertical, para los globos |

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

**Por que es un buen primer test de TITAN++**: un programa de dibujo pide
justo lo que un lenguaje nuevo tiene que demostrar -- bucles calientes sobre
millones de celdas (velocidad de E1), memoria grande sin fugas (el juez de
prestamos), la `gpu fn` en serio (la fusion) y entrada en vivo (latencia). Si
TITAN++ entinta una pagina sin trabarse, ha pasado el examen.

---

# 5. LA ESCALERA

Cada peldanio es UNA cosa que se puede probar sola. El orden es el de la
casa: primero lo que no toca nada, despues lo que pide el metal.

## 5a. Lo que TITAN++ necesita antes (el lenguaje y sus puertas)

- [ ] TA0 -- las decisiones de la seccion 7, contestadas por el propietario
- [ ] TA1 -- el BYTE: un tipo de 8 bits sin signo y las tablas grandes en un bloque de memoria pedido (`[u8]` de millones de celdas), con su codigo T y su ejemplo en `toolchain/lang/titan/ejemplos/`; nivel nuevo de la GRAMATICA
- [ ] TA2 -- la VENTANA: el nodo `screen` de TITAN++ (pedir superficie, escribir una fila de pixeles, presentar), con su permiso en el Titan.toml y su linea en el certificado
- [ ] TA3 -- la ENTRADA de la ventana: el nodo `input` (raton: x, y, botones; y la presion cuando llegue TC1), con REX
- [ ] TA4 -- el DISCO: leer y escribir ficheros de ESTRATOS desde TITAN++, con permiso
- [ ] TA5 -- la MEDIDA: un banco de velocidad de E1 sobre un bucle de pixeles (cuantos millones de celdas por segundo), escrito ANTES de prometer latencia

## 5b. EL PRIMER TEST: un lienzo que funciona (con raton)

- [ ] TB1 -- EL LIENZO MINIMO en TITAN++: una capa, un pincel redondo con antialias, la goma, y exportar PNG. La prueba: el tiempo de un trazo a lo largo de la pantalla, medido
- [ ] TB2 -- CAPAS Y FUSION: N capas con opacidad y los modos normal, multiplicar y pantalla; la fusion como `gpu fn` con su camino en la CPU, y el oraculo de los tres
- [ ] TB3 -- TESELAS E HISTORIAL: capas en teselas de 256 (las vacias no existen) y deshacer/rehacer sobre ESTRATOS
- [ ] TB4 -- SELECCION Y RELLENO: la mascara de seleccion, el cubo con referencia a otra capa y "cerrar huecos"

## 5c. Manga

- [ ] TC1 -- LA TABLETA: el driver USB HID de digitalizador en el kernel (presion, inclinacion, la goma del lapiz). Del METAL: lo prueba el propietario con SU tableta
- [ ] TC2 -- EL MOTOR DE PINCEL: sellos con espaciado, estabilizador, curvas de presion a tamanio y opacidad, y puntas de imagen
- [ ] TC3 -- LA TINTA VECTORIAL: trazos guardados como puntos con ancho, pintados al vuelo; la goma hasta la interseccion y cambiar el grosor
- [ ] TC4 -- LAS VINETAS: la capa de marco, con poligonos que recortan lo de dentro
- [ ] TC5 -- LAS TRAMAS: la propiedad de capa que convierte grises en puntos (lineatura, angulo), como `gpu fn` de una celda
- [ ] TC6 -- LAS REGLAS: perspectiva de 1, 2 y 3 puntos, simetria, y lineas de velocidad y concentricas
- [ ] TC7 -- TEXTO Y GLOBOS: la letra de la casa en vertical, y el globo con su rabito
- [ ] TC8 -- LA OBRA: N paginas con sangrado y zona segura, y la salida para imprenta (600 ppp, 1 bit)

## 5d. Lo de Photoshop, despues y solo si se pide

- [ ] TD1 -- LOS VECINOS en la 3060: `gpu fn` que lee celdas de al lado (pide G2 de `PLAN_EL_CENTAURO.md`); el primero, el desenfoque gaussiano separable
- [ ] TD2 -- CAPAS DE AJUSTE: curvas y niveles que se aplican al componer, sin tocar pixeles
- [ ] TD3 -- EL PINCEL CORRECTOR: edicion de Poisson
- [ ] TD4 -- EL RELLENO SEGUN CONTENIDO: PatchMatch
- [ ] TD5 -- IMPORTAR: leer `.psd` y `.clip` (solo leer, y solo lo que esta documentado por fuera), para traer las obras que ya existen

---

# 6. LO QUE ESTE PLAN NO HACE

* **No es un clon.** Ni los menus, ni los atajos, ni el formato de Adobe o de
  Celsys como formato propio. La obra se guarda en lo de BMO-X.
* **Nada de IA generativa, nube, plugins de terceros ni modelos 3D.**
* **Nada de color de imprenta (CMYK) ni de 16/32 bits al principio.** Manga en
  blanco y negro y en 8 bits primero: es lo que se imprime.
* **No adelanta el metal.** La tableta (TC1) y la 3060 de verdad (G4) son del
  propietario y de su Ryzen; todo lo demas se prueba en el anfitrion antes.

---

# 7. LAS DECISIONES DEL PROPIETARIO (antes de codificar)

```text
   D1  LA IDENTIDAD     la frase de LOS_TRES_VERTICES ("un sistema que promete
                        Photoshop...") se queda como esta (este plan promete
                        el nucleo, no la suite), o se reescribe con --sellar?
   D2  EL NOMBRE        como se llama la app y en que tecla vive (F1..F12 ya
                        estan todas: una nueva en la rejilla, o dentro de F1)
   D3  LA TABLETA       que tableta tienes (marca y modelo): TC1 se escribe
                        para ESA primero
   D4  LA PAGINA        el tamanio objetivo: B4 a 600 ppp en blanco y negro
                        (manga de imprenta), o 350 ppp en color
   D5  EL PRIMER TEST   TB1 con raton (se puede empezar ya, despues de TA1-TA4),
                        o esperar a la tableta
```

---

# 8. DE DONDE SALE LO DE ARRIBA

* Photoshop y sus teselas, su memoria virtual y sus ficheros de paso: [How Photoshop solved working with files larger than can fit into memory](https://developer.chrome.com/blog/how-photoshop-solved-working-with-files-larger-than-can-fit-into-memory/) (Chrome for Developers, sobre Photoshop en la web).
* El pincel corrector: [Photoshop Healing Brush: a Tool for Seamless Cloning](https://ftp.fau.de/gimp/references/Photoshop_Healing_Brush_a_Tool_for_Seamless_Clonin.pdf) (Georgiev) y [Todor Georgiev](https://en.wikipedia.org/wiki/Todor_Georgiev).
* El relleno segun contenido: [PatchMatch](https://www.cs.princeton.edu/research/techreps/885) (Barnes, Princeton) y [su entrada en Photoshop CS5](https://imaging-resource.com/NEWS/1256272706.html).
* El formato `.clip` leido por fuera: [unclip](https://github.com/unai-d/unclip), [clipparse](https://pypi.org/project/clipparse/), [clipstudio.js](https://awesome.ecosyste.ms/projects/github.com%2Fsaitolume%2Fclipstudio.js).
* La tinta vectorial y la goma hasta la interseccion: [Vector Layer, guia completa](https://tips.clip-studio.com/ja-jp/articles/7586?org=1) y [Useful Vector Layers tips](https://tips.clip-studio.com/en-us/articles/7572) (CLIP STUDIO TIPS); vinetas, tramas y globos: [What tools are available?](https://www.clipstudio.net/en/comics-manga/tool/layers/).
* El motor de pincel por sellos, la fusion por formulas y el relleno con
  cierre de huecos son conocimiento comun del oficio (Krita, GIMP y
  MyPaint, que son abiertos, hacen lo mismo y se pueden leer).
* Lo de BMO-X: `toolchain/lang/titan/GRAMATICA.md` (niveles 11 y 12), el USB
  del kernel (`Ultra_kernel_x86-64/kernel/src/ring0/dev/usb/`) y
  `docs/identidad/LOS_TRES_VERTICES.md`, leidos el 07-10.
