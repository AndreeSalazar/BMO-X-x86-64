# PLAN MEDIOS -- VLC como objetivo, medido contra lo que hay

> **REABIERTO por el propietario el 2026-09-20** (estaba APARCADO desde el 26-08):
> *"tener mi reproductor de mp3, mp4 y visualizar imagenes por completo"*. La
> parte 7, al final, es la respuesta de hoy con sus numeros: las imagenes
> estan; el MP3 es codigo sin tubo; el MP4 no es MP4.

> Escrito el **2026-08-26**, a peticion del propietario: *"analizar el VLC como
> objetivo para meter esa app en mi BMO-X"*.
>
> La respuesta corta esta en la parte 1 y es un **no** con motivo. La parte 4 es
> la que importa: **lo que el propietario quiere detras de "VLC" si es alcanzable**, y
> dos de sus tres piezas ya estan en marcha.
>
> Regla de esta carpeta, y se cumple aqui: *un "no" con motivo escrito se puede
> discutir; uno sin motivo es un agujero.*

---

# 1. VLC, MEDIDO: ES NIVEL 3, Y NO POR EL LENGUAJE

`docs/identidad/QUE_DESBLOQUEA.md` ya tiene la escalera. VLC cae en el Nivel 3
--con LibreOffice y Blender-- y conviene ver **por que**, porque no es por lo que
uno esperaria.

## 1.1 -- Los medidas, en orden de magnitud

[!] Estos numeros **no estan medidos en este arbol** (VLC no esta aqui). Son
ordenes de magnitud publicos, y van marcados como tales para no mezclarlos con
las cifras de la seccion 2 del RING3_MAESTRO, que si son de este repo.

```text
   VLC (nucleo + modulos)     ~700k-1M lineas de C, y algunos modulos en C++
   modulos                    del orden de 400, cada uno un objeto cargable
   FFmpeg (libav*)            ~1,5M lineas de C + ensamblador x86 escrito a mano
```

## 1.2 -- Los cuatro bloqueantes, y ninguno es "falta C++"

| # | bloqueante | estado en BMO-X | quien lo desbloquea |
|---|---|---|---|
| 1 | **Compilacion separada** | ❌ una sola unidad de traduccion | palanca 1 de `QUE_DESBLOQUEA` |
| 2 | **Hilos** | ❌ cero syscalls de crear hilo | pieza "HILOS -- 4 piezas" |
| 3 | **POSIX de verdad** | ❌ `mmap`, `poll`, sockets, `dlopen`, locale, iconv | no esta planificado |
| 4 | **Salida de audio** | ⏳ el tubo, sin ejecutar | `PLAN_AUDIO` A1-A2 |

### El 1 mata solo

SDL 1.2 --que son del orden de **cien** ficheros `.c`-- ya no cabe en un unity
build: dos `static` con el mismo nombre en ficheros distintos dejan de ocultarse
y pasan a ser una redefinicion. Eso esta escrito y medido en `QUE_DESBLOQUEA`.

*** **VLC son cuatro ordenes de magnitud por encima de SDL.** No es que sea
dificil: es que **no existe la operacion** de compilar dos ficheros y juntarlos.

### El 2 no se puede rodear, y esto es lo especifico de VLC

Muchos programas grandes se pueden hacer de un hilo apretando. VLC no, y el
motivo es su arquitectura, no su medida:

```text
   hilo de entrada  ->  hilo de demux  ->  hilo(s) de decodificacion
                                                |
                              +-----------------+-----------------+
                              v                                   v
                    hilo de salida de VIDEO              hilo de salida de AUDIO
                    (sincroniza al reloj)                (alimenta el tubo)
```

**El desacople decodificador/salida ES el esquema.** Es lo que permite que un
fotograma que tarda no se lleve por delante al sonido. Un VLC de un hilo no es
un VLC apretado: es otro programa.

### El 3 es el que nadie cuenta

`dlopen` tiene salida --VLC compila con `--disable-shared` y los modulos entran
estaticos-- pero `mmap`, `poll`, los sockets, `iconv` y el locale no la tienen.
Y aqui hay una frase de `QUE_DESBLOQUEA` que aplica igual: *"Git, Vim, CPython:
POSIX grande. **No con 88 operaciones**."*

## 1.3 -- Y el que decide, aunque parezca el menor: FFmpeg

Sin `libavcodec` VLC no decodifica casi nada. FFmpeg trae **ensamblador x86
escrito a mano** para las rutas calientes, y su sistema de construccion es un
proyecto por si mismo.

> **Portar VLC es, en la practica, portar FFmpeg primero.** Y eso es mas grande
> que todo lo que hay hoy en este repo junto.

---

# 2. LO QUE SI DICE ALGO UTIL: VLC COMO CRITERIO, NO COMO OBJETIVO

★ Hay una cosa que este analisis regala, y es la razon de escribirlo aunque la
respuesta sea no:

> **Las dos piezas que VLC exige antes que ninguna otra son exactamente las dos
> que este proyecto ya tiene en la cola: el tubo de audio y la compilacion
> separada.**

O sea que VLC **no cambia el orden de trabajo**. Lo confirma. Y eso es lo mejor
que puede hacer un objetivo grande que todavia no se puede tocar.

---

# 3. LO QUE HAY QUE MIRAR ANTES QUE EL DECODIFICADOR, Y CASI NADIE MIRA

★★ Antes de discutir codecs, el techo de esta maquina para reproducir video ya
esta medido -- **y no es el decodificador.**

`docs/componente/EL_COMPOSITOR_Y_EL_ESCANER.md` y la sonda de DOOM dejaron dos
numeros que mandan aqui igual:

```text
   volcar la pantalla entera        27,6 ms
   un fotograma a 60 Hz             16,7 ms
   DOOM a 1600x1000                 el deficit ENTERO es el blit (~300 MB/s)
```

*** **A pantalla completa, esta maquina no llega a 60 fotogramas por segundo
pintando, ANTES de decodificar nada.** Un video a 24 fps con 41,7 ms por
fotograma si cabe -- pero el presupuesto se reparte asi:

```text
   41,7 ms por fotograma
   -27,6 el blit a pantalla completa
   ------
    14,1 ms para decodificar, convertir de YUV a RGB y escalar
```

[!] Y **la conversion de YUV a RGB no es gratis**: es un recorrido de todos los
pixeles, o sea la misma clase de coste que el blit. Para 1080p son dos
recorridos de ~8 MB donde solo cabia uno.

★ **La consecuencia practica, y es una decision de esquema, no un detalle**: el
primer video de BMO-X va en una **ventana chica**, no a pantalla completa. A
480x270 el blit baja a ~1/25 y el presupuesto entero se abre. Pantalla completa
llega despues, y **midiendo**, no suponiendo.

---

# 4. EL CAMINO QUE SI EXISTE, EN CUATRO ESCALONES

Ninguno de estos es VLC. Los cuatro juntos son *"reproducir medios en BMO-X"*,
que es lo que se pidio detras del nombre.

## M1 -- SONIDO: un `.wav` que suene  ->  **casi hecho**

`platform/shared/bmo-sonido` ya lee WAV, entrega PCM y **comprueba que cabe en
lo que el aparato acepta** (`cabe_en(frecuencia, canales, bits)`). 309 lineas,
227 de pruebas.

```text
   lo que falta:  que el tubo abra   <- PLAN_AUDIO A1, en vuelo el 26-08
```

★ Cuando `audio` diga `TUBO ABIERTO`, esto son **dias**, no semanas. No hay
decodificador que escribir: un WAV ya es PCM.

## M2 -- SONIDO COMPRIMIDO: MP3  ->  ya es A5 de `PLAN_AUDIO`

`minimp3` es **un solo fichero**, o sea unity build por esquema. Y el aviso que ya
esta escrito ahi sigue vigente y se repite porque es el que se olvida:

> ⚠ `minimp3` usa coma flotante, y la ruta de coma flotante del frontend de C de
> esta casa tiene 9 tests **y ninguno la ejecuta**. Comprobar eso es media tarde
> y va **antes** de traer 2.000 lineas, no despues.

## M3 -- VIDEO: `pl_mpeg`  ->  **el objetivo realista, y es Nivel 0**

`pl_mpeg` es una cabecera unica de C99: **MPEG-1 video + MP2 audio, sin ninguna
dependencia**, decodifica a YUV y trae la conversion a RGB.

| | |
|---|---|
| medida | del orden de 5k lineas, **un fichero** |
| dependencias | ninguna |
| lengua | C99 -- el frontend que ya existe |
| salida | un buffer de pixeles, que es exactamente lo que BMO sabe pintar |
| sonido | MP2, y sale por el mismo tubo que M1 |

*** **Esto es lo mas cerca de "VLC" que esta maquina puede estar este anio, y la
distancia entre M3 y VLC no es de calidad: es de catalogo.** MPEG-1 no es H.264.
Lo que M3 demuestra --demux, decodificar, sincronizar video con audio, pintar a
tiempo-- es el 100% de la arquitectura de un reproductor. Lo que le falta a M3
para ser VLC son **codecs y formatos**, que es trabajo que se suma, no esquema que
se rehace.

[!] Y lo mismo por escrito para que no se lea como una promesa disfrazada: un
`.mp4` con H.264 **no** se abre con esto, y no hay atajo. H.264 sin FFmpeg y sin
aceleracion por GPU es un proyecto propio.

## M4 -- El reproductor de verdad  ->  bloqueado, y por lo mismo que VLC

Varios formatos, varias pistas, buscar dentro del fichero, subtitulos. Eso ya
pide **hilos** y **compilacion separada**. Es el escalon en el que la respuesta
deja de ser "escribe esto" y pasa a ser "termina las dos palancas".

---

# 5. EL ORDEN, Y DONDE ENGANCHA CON LO QUE YA HABIA

```text
   [ ] el tubo abre (A1)          <- lo unico que bloquea M1, y es un ARRANQUE
   [ ] M1  WAV                    dias despues del tubo
   [ ] M2  MP3 (= A5)             media tarde de comprobar la coma flotante antes
   [ ] M3  pl_mpeg en ventana     el primer video, y la primera sincronizacion
   ----------------------------------------------------------------------------
   [ ] palanca 1: compilacion separada
   [ ] hilos (4 piezas)
   [ ] M4  un reproductor de verdad
   ----------------------------------------------------------------------------
   VLC   requiere ademas POSIX grande y FFmpeg. No esta en esta lista.
```

★ Fijarse en donde cae la raya: **M1, M2 y M3 no piden ni una pieza de sistema
que no este ya empezada.** M4 pide las dos palancas grandes. VLC pide, encima de
esas dos, un POSIX que nadie ha planificado y un FFmpeg que es mas grande que
este repo.

---

# 7. LO QUE PIDIO EL DUENO EL 20-09, Y LO QUE ES CADA COSA

## 7.1 Imagenes -- HECHO el 20-09, y va antes que el sonido a proposito

No pide ningun tubo, ningun arranque y ningun flotante: es lo unico de esta
lista que se termina entero en el anfitrion. `platform/shared/bmo-imagen`:

```text
   BICO, BMP, QOI      13-09    los planos
   PNG                 20-09    inflate PROPIO (RFC 1951, 474 lineas, la forma de
                                `puff`), profundidad 8, gris/RGB/paleta/gris+alfa/
                                RGBA, tRNS, varios IDAT; 16 bits y Adam7 son
                                `Variante` con nombre
   JPEG baseline       20-09    Huffman, IDCT ENTERA (cosenos en punto fijo: los
                                mismos pixeles en cualquier maquina, y sin SSE que
                                preservar), 4:4:4 / 4:2:2 / 4:2:0, gris, DRI;
                                progresivo es `Variante` con nombre
```

40 filas contra ficheros que escribio Pillow (y el JPEG contra lo que
decodifica libjpeg: error medio <= 2, con el maximo tolerando el croma que
libjpeg suaviza y aqui va al vecino). Un PNG pide un TALLER de 41 KiB (la
ventana del inflate y dos filas) que el visor pide al kernel: la pila de Ring
3 son 64 KiB. El escritorio abre `.png` y `.jpg` en el visor; `inti.png` y
`arranque.jpg` van al disco de datos para verlo.

- [x] PNG en `bmo-imagen`, con su inflate -- 20-09
- [x] JPEG baseline en `bmo-imagen` -- 20-09
- [x] el visor pide el taller y abre `.png` y `.jpg` -- 20-09
- [ ] el METAL: abrir `datos/inti.png` y `datos/arranque.jpg` en el Ryzen
- [ ] JPEG progresivo (pide 4 MiB de coeficientes: otro bloque del kernel) y
      PNG de 16 bits, si alguna foto real los trae

## 7.2 MP3 -- es codigo, pero no suena sin el tubo

El MP3 son dos piezas y solo una es del decodificador:

```text
   decodificar   MPEG-1 Layer III: Huffman, requantizar, reordenar, IMDCT,
                 sintesis polifasica. En Rust `no_std` propio (como el PNG),
                 ~2.500 lineas, sin flotantes de C que no estan probados
                 (el metro dice `flotante 0.0` en C: ningun programa los corre)
   que suene     PLAN_AUDIO A1 -- SET_INTERFACE: EL RYZEN LO NEGO el 25-08,
                 corregido el 26-08, SIN EJECUTAR desde entonces
```

Se puede escribir y probar el decodificador contra ficheros del anfitrion sin
la maquina; lo que no se puede es OIRLO. Va DESPUES del arranque que diga
`TUBO ABIERTO`, que es un arranque, no un dia de codigo.

- [ ] A1 en el Ryzen: `TUBO ABIERTO` (`PLAN_AUDIO.md`)
- [ ] M1 WAV suena (dias despues de A1)
- [ ] M2 el decodificador de MP3, propio en Rust, probado contra PCM del anfitrion
- [ ] M2b MP3 suena por el mismo tubo

## 7.3 MP4 -- no es MP4 lo que se puede; es MPEG-1 y la antena

Un `.mp4` es H.264 + AAC, y eso ya esta contestado en la parte 6: **no**, sin
FFmpeg y sin GPU es un proyecto propio cada uno. Lo que si existe es M3:
`pl_mpeg` (MPEG-1 video + MP2 audio, C99, un fichero, MIT) en una ventana, y
un `.mp4` se convierte FUERA con una linea (`ffmpeg -c:v mpeg1video -c:a mp2
video.mpg`), o lo convierte la antena (`PLAN_CLOUD_LOCAL.md` S1/S4/S6, que
tambien es como llega Minecraft a esta pantalla: PIXELES).

- [ ] M3 `pl_mpeg` compilado con BMO C (pide la coma flotante de C PROBADA
      primero: media tarde, y es la misma condicion que minimp3 tenia)
- [ ] M3b un `.mpg` en una ventana, con su sonido por el tubo de M1
- [ ] S1 de CLOUD_LOCAL: el mismo pl_mpeg recibiendo de la antena

# 6. LO QUE ESTE PLAN NO PROMETE

- **VLC no.** Ni acotado, ni recortado, ni "una version chica de VLC". Un VLC
  sin hilos y sin FFmpeg no es VLC: es M3 con el nombre de otro.
- **H.264, HEVC, VP9: no.** Sin FFmpeg y sin GPU, cada uno es un proyecto.
- **Pantalla completa a 60 fps: no todavia**, y el numero que lo impide es el
  blit, no el decodificador. Ver la parte 3.
- **Y ninguna de estas fechas es una fecha.** Son ordenes: lo que va antes y lo
  que va despues. La unica casilla con una condicion exacta es M1, y su condicion
  es que el Ryzen arranque y diga `TUBO ABIERTO`.
