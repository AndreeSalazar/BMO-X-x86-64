# PLAN LA MESA -- el control de sonido de BMO-X, como app de ventana

> Abierto el 2026-09-22 por decision del propietario: *"que pongan todo eso de 7.1 y
> 3D en una app ventana simple, el control de configuracion de sonido MAESTRO
> total, inspirado en Adobe Premiere Pro POR COMPLETO... y que el audio se
> divida por completo: el audio de juegos, de apps, de cualquiera, por si
> quiero editar ese proximo video"*.
>
> [`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md) es la ONDA (formatos, ganancia,
> mezcla, 3D). Este es **quien la manda**: una app de Ring 3 con ventana,
> y el modelo de PISTAS que hay debajo.

---

# 0. PRIMERO, LA CORRECCION: que hace Windows y que no

El propietario lo dijo con una duda honesta --*"en Windows no es posible, o no se si
estoy mal"*-- y la respuesta es **a medias**, asi que se escribe antes de
prometer nada:

| lo que se quiere | Windows | BMO-X hoy |
|---|---|---|
| volumen por aplicacion | **SI**, el Mezclador de volumen, desde Vista (2007) | no |
| cada app a una salida distinta | **SI**, desde Windows 11 | no |
| **grabar cada app en su PROPIA pista** | **no de serie**: pide la API de *process loopback* (Win 10 2004+) y un programa de terceros (OBS, VoiceMeeter) | no, y es lo que este plan persigue |
| ver los numeros de lo que le pasa a la onda | no (hay pico por app, nada mas) | **si**, es la ley del amplificador |

Asi que **el "XD" no va por donde el propietario creia**, y va por otro sitio que es
mejor: lo distinto de BMO-X no es tener una perilla por app --eso lo tiene
medio mundo-- es que **la cadena entera es UN programa de Ring 3 que el
propietario posee**. No hay un servicio de audio cerrado en medio. Por eso una
pista se puede DERIVAR a un fichero sin pedirle permiso a nadie, y por eso cada
pista puede decir su pico, su RMS y cuanto la sujeto el limite. Eso ultimo no
lo da ningun sistema de escritorio de serie.

* **Lo que se promete**: pistas separadas, con su ganancia, su medidor y su
  derivacion a fichero.
* **Lo que NO se promete**: que sea un DAW. Ni edicion, ni efectos, ni
  automatizacion. Ver seccion 6.

---

# 1. EL MODELO: UN ARBOL DE DOS NIVELES

El propietario, al leer la primera version de este plan: *"no solo voz, sistema, eso;
sino que DENTRO del contenido: el juego EXPONE TODO el audio que ofrece, en
subcategorias, y lo mismo todos. Uno simple MAESTRO aplica todo, pero si
quieres control total se pueda"*. Tiene razon y cambia el modelo: una pista
por programa no basta.

```text
   GRUPO  juego      [ganancia][M][S][medidor]
     |-- PISTA  armas      [ganancia][M][S][medidor]
     |-- PISTA  pasos      [ganancia][M][S][medidor]
     |-- PISTA  musica     [ganancia][M][S][medidor]
     '-- PISTA  voces      [ganancia][M][S][medidor]
   GRUPO  sistema    [ganancia][M][S][medidor]
     '-- PISTA  (sin nombre: el grupo entero)
   GRUPO  musica     [ganancia][M][S][medidor]
     '-- PISTA  (sin nombre)
                                                    todo -> [MAESTRO]
```

**Por que DOS niveles y no uno ni tres.** Uno no basta: un juego que solo sabe
decir "juego" obliga a bajarlo entero cuando lo unico que molestaba eran las
armas. Tres ya es un DAW --grupos dentro de grupos es jerarquia sin fin-- y
este repo tiene una regla contra la esencia sin acotar. Dos es lo que tiene
una mesa de verdad (buses y canales) y es lo que se puede terminar.

## 1.1 -- EXPONER: la misma ley que la tabla del aparato

★★ **El arbol lo declara quien lo tiene, no lo adivina quien lo pinta.** Es
literalmente la misma ley que hizo falta para los formatos del audifono (S0 de
[`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md)): el aparato escribe su tabla, y el
programa escribe su arbol. La mesa **no supone** que un juego tiene "efectos y
musica": muestra lo que el juego EXPUSO, sean dos cosas o sean diecisiete.

```text
   el aparato declara sus formatos   ->  la tabla F1
   el PROGRAMA declara sus pistas    ->  la tabla F2
   nadie adivina ninguna de las dos
```

Un programa que no tenga nada que exponer abre su grupo con **una pista sin
nombre**, que significa *"el grupo entero"*. Asi `musica.ibx` sigue sonando sin
saber que hay mesa (regla 5 de abajo).

## 1.2 -- Las reglas, y el motivo de cada una

1. **Nombre, no `pid`.** Un pid cambia en cada arranque; `juego` no.
2. **Mudo y SOLO son distintos**, en los dos niveles: mudo calla lo suyo; solo
   calla todo lo demas. Y **el solo de una PISTA gana al de un GRUPO**, porque
   es lo mas fino que se puede pedir y quien lo pulsa quiere oir ESA.
3. **Tres medidores, tres preguntas distintas**: el maestro dice *si te
   pasas*, el grupo dice *que grupo*, la pista dice *cual*. Un solo medidor al
   final no puede contestar las dos ultimas, y ese es todo el motivo del arbol.
4. **Las ganancias se MULTIPLICAN**: pista x grupo x maestro. Bajar las armas
   6 dB y el juego entero 6 dB deja las armas 12 dB abajo, que es lo que
   cualquiera espera de una mesa.
5. **El que no pide pista va a `otros`.** Un programa viejo sigue sonando.
6. **Cuando no caben mas, se DICE y no se roba una.**
7. **Derivar no es bajar el volumen.** Una pista que se graba se escribe con
   su ganancia puesta y *antes* del maestro, porque el maestro es para los
   oidos y la grabacion es para despues.

## 1.3 -- Y por que esto es lo mismo que ya hay

Una pista es una [`Ganancia`] con su medidor; un grupo, lo mismo; el maestro es
el [`Amplificador`] entero. No hay pieza nueva: **hay un arreglo de las que ya
estan** (`bmo-amplificador`, del 22-09), y por eso M0 costo un fichero.

---

# 2. LA VENTANA, tomada de Premiere Pro

Lo que se toma de Premiere no es el aspecto: es **el orden de lectura**. En su
mezclador de audio, cada pista es una COLUMNA vertical con el medidor a la
izquierda del deslizador, los botones arriba y el numero abajo. Se lee de un
vistazo porque todo lo de una pista esta en la misma columna.

```text
  +-- SONIDO ---------------------------------------------------------- [_][X] --+
  |                                                                              |
  |  GRUPO juego  [M][S]  -3.0 dB  |  GRUPO sistema [M][S]  |   || MAESTRO        |
  |   armas   pasos   musica  voces |   (el grupo entero)    |   ||                |
  |   [M][S]  [M][S]  [M][S]  [M][S]|   [M][S]               |   || ||||||||       |
  |   ||||||  ||      ||||||  |     |   ||                   |   || ||||||         |
  |   ||||||  ||      ||||    |     |   |                    |   || ||||           |
  |   ||||    |       ||      |     |   |                    |   || ||             |
  |   -4.2    -21.8   -8.1    -48.0 |   -19.4 dB             |   || -1.4 dBFS      |
  |   +0.0    -6.0    +0.0    +0.0  |   +0.0                 |   ||  +12.0 dB      |
  |   [ - ]   [ - ]   [ - ]   [ - ] |   [ - ]                |   || doblegadas 0   |
  |                                 |                        |   || sujetadas 1284 |
  |  [grabar TODAS las pistas]  o  [grabar] en una columna       || del aparato 60%|
  |                                                                              |
  +------------------------------------------------------------------------------+
  |  aparato  1B3F:2008   48.000 Hz   2 canales   16 bits   192 B/ms             |
  |  del aparato [========------] 60 %     |  tarde 0   huecos 29   tramas 15920 |
  +------------------------------------------------------------------------------+
```

Lo que hay en cada columna, de arriba abajo:

El grupo manda una FILA arriba y sus pistas son las columnas de debajo, con
una raya que separa un grupo del siguiente. Asi se ve de un vistazo lo que hay
que ver: **que grupo esta alto, y dentro de el, cual de sus pistas**.

| fila | que es | por que esta |
|---|---|---|
| nombre | `juego`, `armas`... | **lo EXPUSO el programa**; la mesa no lo invento |
| `[M][S]` | mudo y solo | ver 1.2 |
| barras | el medidor: **pico y RMS a la vez** | el pico dice si te pasas, el RMS si se oye |
| primera cifra | el pico en dBFS | lo que SALIO |
| segunda cifra | la ganancia en dB | lo que se PIDIO |
| `[ - ]` | el deslizador (teclado: flechas) | -- |
| `[grabar]` | esta pista a su propio `.wav` | lo que Windows no da de serie (sec. 0 y M4) |

Y abajo, **la franja del aparato**: lo que el `save` ya sabe decir, pero
mientras pasa. Las dos perillas separadas y dichas por su nombre --*del
aparato* (hasta 0,0 dB) y *maestro* (por software, hasta +24 dB)-- porque
confundirlas es lo que hizo que el propietario no oyera mas fuerte el 22-09.

---

# 3. LAS TABLAS DINAMICAS, que es lo que la app EXPONE

El propietario: *"tablas dinamicas maestras, que si declara mi BMO-X exponga para
facilitar y jugar con cualquier cosa"*. La regla es la misma de
[`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md) 2 --**la tabla la escribe el aparato,
no el programador**-- y la app es donde se ve. Tres tablas, con una tecla cada
una:

```text
   F1  APARATO     los formatos que declara (los de S0), con su cuenta hecha:
                   alt, canales, bits, B/ms, max packet, sincronia, frecuencias
                   y cual esta ELEGIDO y por que

   F2  PISTAS      nombre, quien la abrio, su formato de entrada, su ganancia,
                   pico, RMS, si esta muda, si esta en solo, si esta derivando

   F3  LA CADENA   lo que le pasa a una muestra desde la fuente hasta el cable,
                   con el numero de cada paso: remuestreo (de X a Y Hz),
                   panorama, mezcla, ganancia, limite. Es la tabla que dice
                   POR QUE suena como suena
```

Y la ley de las tres: **si una fila no se puede llenar, se dice vacia con su
motivo**, no se inventa. Un `--` con una nota es informacion; un cero
inventado, no.

---

# 4. LAS CASILLAS

## [X] M0 -- LA MESA, sin ventana -- **HECHO el 2026-09-22**

El modelo de 1 como codigo puro: N pistas con nombre, ganancia, mudo, solo y
medidor; un maestro; y `mezclar()` que saca el bloque. Va **dentro de
`bmo-amplificador`** (modulo `mesa`), porque una pista es un amplificador y la
mesa es una suma con ganancia: no hay pieza nueva que inventar, y meterlo en un
crate aparte seria partir lo que es la misma cuenta.

**Tam: M.** Se prueba entero en el anfitrion, sin metal y sin ventana.

### [X] HECHO: `bmo-amplificador::mesa` (13 pruebas)

`Mesa` con ocho `Pista`s: nombre, ganancia, mudo, solo y **medidor propio**.
`abrir` por nombre (dos programas que dicen `juego` comparten pista), `echar`
una pista al acumulador con su ganancia, `maestro` para sacar el bloque.
Cuando no caben mas, **se dice y no se roba una ajena**.

Las pruebas que valen la pena mirar, porque son el modelo hecho numero:

| prueba | lo que fija |
|---|---|
| `muda_calla_esa_y_solo_calla_las_otras` | los dos botones son distintos, y mudo gana a solo sobre la misma pista |
| `el_medidor_de_la_pista_dice_quien_se_pasa` | el maestro sabe que la suma se paso; **la pista dice cual fue** |
| `una_pista_callada_no_ensucia_el_acumulador_ni_su_medidor` | una pista muda que mostrara barras seria un instrumento que miente |
| `cuatro_pistas_a_tope_no_rompen_nada` | 120.000 en el acumulador, ni una muestra fuera al salir, y el maestro CUENTA lo que sujeto |
| `la_mesa_entera_con_el_maestro_subido` | el caso del propietario: tres pistas flojas y +12 dB, sin doblegar nada |

## [ ] M1 -- EL PRODUCTOR: la mesa alimenta el tubo

Es el P2 de [`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md): un programa de Ring 3 que
reclama el sonido, toma el bloque prestado y lo llena con lo que la mesa saca.
Con M0 hecho esto es pegamento, no invento.

**Tam: M.** Lo aprueba el metal: `encoladas` sube y `huecos` es 0.

## [ ] M2 -- LA VENTANA

> **22-09, S4c de [`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md):** el propietario pidio
> el control *"en mi escritorio, no como app"*, y el MAESTRO ya vive ahi --un
> panel del DIRECTOR (F10 o el indicador de la barra) sobre una etapa del
> kernel--. Lo que sigue abierto aqui es el ARBOL. Y la pregunta del lenguaje
> de abajo se contesta sola si la mesa crece dentro de ese mismo panel: el
> DIRECTOR es Rust y enlaza `bmo-amplificador` sin puente. Queda decidirlo con
> el propietario cuando haya dos fuentes que mezclar.

La app de 2, con el aspecto de Premiere y el teclado: flechas para la ganancia,
`M` y `S`, `F1`/`F2`/`F3` para las tablas. Ofrece su lamina al DIRECTOR como
cualquier otra app (`MEM_OP_OFRECER`), asi que **no necesita nada nuevo del
kernel**.

**Tam: L.** Y una decision que hay que tomar con el propietario antes de escribirla:
**en que lenguaje**. Las apps de ventana de hoy son INTI (`navegar.inti`);
la mesa quiere `bmo-amplificador`, que es Rust, y **INTI no enlaza Rust**. O la
app es Rust por `bex-link`, o la mesa se expone al modo INTI por syscalls. Se
pregunta antes, no despues.

## [ ] M3 -- QUE CADA UNO PIDA SU PISTA

El contrato para que un programa **EXPONGA su arbol**: una operacion sobre el
sonido que toma `(grupo, pista)` y devuelve por donde mandar esas muestras.
`abrir(b"juego", b"armas")`, `abrir(b"juego", b"pasos")`... El que no pida
nada, a `otros`. Aqui esta el verdadero *"el audio se divide por completo"*, y
es la puerta por la que un juego --o DOOM, cuando tenga su mezclador-- dice lo
que ofrece.

**Tam: M.** Y hay que decidir si la pista se pide o **se concede**: que
cualquier programa elija su nombre es lo comodo; que lo conceda el propietario
es lo que impide que dos programas peleen por la pista `juego`.

## [ ] M4 -- GRABAR: una pista, un fichero (*"Fraps u OBS?"*)

El propietario pregunto por los dos, y la respuesta la decide **la licencia antes
que el gusto**:

| | que es | se puede leer? | sirve de modelo? |
|---|---|---|---|
| **OBS Studio** | en GitHub, **GPLv2** | leer SI, **copiar NO**: este repo es Apache-2.0 y Ring 0 esta cerrado a codigo de terceros (la misma regla que con Linux) | **su arquitectura NO**: escenas, fuentes, codificadores y complementos es lo contrario de "ULTRA SIMPLIFICADO" |
| **Fraps** | cerrado, sin fuente | no hay nada que leer | **su COMPORTAMIENTO SI**, y es exactamente lo que se pidio |

Asi que se toma **Fraps, y solo su comportamiento**, que cabe en una linea:

```text
   UNA tecla empieza. UNA tecla para. UN fichero. Cero configuracion.
   Y un numero en pantalla mientras graba, para saber que esta grabando.
```

Eso es todo el programa. Nada de escenas ni de perfiles: si hay que abrir un
dialogo para grabar, ya se perdio el momento que se queria grabar.

★ **Y lo que Fraps no podia hacer, aqui sale gratis**: Fraps grababa UNA pista
mezclada. Esta mesa tiene el arbol entero en la mano, asi que puede escribir
**un `.wav` por pista** --`juego-armas.wav`, `juego-musica.wav`,
`sistema.wav`-- con su ganancia puesta y **antes del maestro**. Para el video
del propietario, eso es una pista por elemento en vez de una mezcla de la que ya no
se puede sacar nada. Es la fila que Windows no tiene de serie (seccion 0).

**Lo que hay que medir antes de prometerlo**: a 48 kHz estereo, cada pista son
**192 KB/s**. Ocho a la vez son 1,5 MB/s sostenidos contra ESTRATOS mientras el
tubo pide una trama cada milisegundo. El SSD da de sobra; lo que no se sabe es
el RITMO del camino de escritura. Se mide antes, no despues.

**Tam: L**, y depende de M0 (hecho) y de esa medida.

## [ ] M4b -- DERIVAR UNA PISTA A FICHERO

Lo que Windows no da de serie: `[derivar]` escribe esa pista en un `.wav` a su
frecuencia, con su ganancia y **antes del maestro**. Para el video del
propietario, eso es una pista por elemento en vez de una mezcla de la que no se
puede sacar nada.

**Tam: M.** Es M4 con el interruptor puesto en una sola pista en vez de en
todas: la misma pieza, y por eso no se escribe dos veces.

## [ ] M5 -- EL 3D EN LA MESA

Cuando S5 y S7 de [`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md) esten: una pista gana
`angulo` y `distancia`, y la ventana una rueda. **Y aqui es donde el 7.1 de
este audifono se hace de verdad**, porque el metal ya contesto que por el cable
no viene (un solo formato, dos canales: 3e-quinquies del informe del metal).

**Tam: L**, y va despues.

## [ ] M6 -- LOS PRESETS

Guardar una mesa entera con nombre (`juego`, `video`, `noche`) en un fichero de
texto que se pueda leer. No es un lujo: es lo que evita volver a colocar seis
deslizadores cada arranque.

**Tam: S.**

---

# 5. EL ORDEN, Y LO QUE LO GOBIERNA

```text
   M0  la mesa sin ventana      <- pura aritmetica, se prueba hoy
   M1  el productor              <- y ENTONCES suena una mezcla de verdad
   M3  cada uno pide su pista    <- y "dividir el audio" deja de ser una idea
   M2  la ventana                <- cuando hay algo que mandar, se le pone mando
   M4  derivar a fichero         <- lo que el video del propietario pide
   M6  presets
   M5  el 3D                     <- detras de S5 y S7
```

★ **La ventana va la CUARTA a proposito.** Una ventana con seis deslizadores
que no mueven nada es una foto, y este repo tiene una regla contra eso: *nada
que compile y no haga lo que dice*. Primero que haya mezcla, despues el mando.

---

# 6. LO QUE ESTE PLAN **NO** PROMETE

* **No es un DAW.** Ni linea de tiempo, ni cortar, ni pegar, ni deshacer. La
  mesa mezcla lo que suena AHORA; editar es otro programa y probablemente otra
  maquina.
* **Sin efectos**: ni ecualizador, ni reverberacion, ni compresor. La unica
  pieza que trata la onda es la ganancia con su limite.
* **Sin entrada**: no hay microfono ni captura. Una pista "voz" tendria que
  venir de un programa que ya tenga esas muestras.
* **Sin automatizacion**: las perillas se quedan donde se dejan. Los
  deslizadores que se mueven solos con el tiempo son de un DAW.
* **Y la mesa no es del kernel.** El kernel entrega el tubo, el bloque prestado
  y el volumen del aparato; mezclar, medir y derivar es Ring 3. Un tercero
  puede escribir SU mesa y sustituir a esta sin pedir permiso
  ([`PLAN_AUDIO.md`](PLAN_AUDIO.md) 5).
