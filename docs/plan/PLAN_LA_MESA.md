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

# 1. EL MODELO: PISTAS, y por que es lo mismo que ya hay

Una pista es una fuente con nombre y sus perillas. Y aqui viene lo que hace
esto barato: **una pista ES el amplificador que ya existe**
(`bmo-amplificador`, hecho el 22-09), y la mesa es la suma de todas ellas, que
tambien es el amplificador. No hay pieza nueva: hay un ARREGLO de piezas.

```text
   PISTA 1  juego        [ganancia] [mudo] [solo] [medidor] --.
   PISTA 2  musica       [ganancia] [mudo] [solo] [medidor] --+--> [MAESTRO]
   PISTA 3  sistema      [ganancia] [mudo] [solo] [medidor] --+    ganancia
   PISTA 4  voz          [ganancia] [mudo] [solo] [medidor] --'    limite
                                                                  medidor
                                                                     |
                             cada pista puede DERIVARSE a fichero     v
                             (esa es la parte que Windows no da)    el tubo
```

Reglas del modelo, y cada una tiene su motivo:

1. **Una pista tiene NOMBRE**, no un numero de proceso. Un `pid` cambia en cada
   arranque; "juego" no. El nombre lo pone quien abre la pista.
2. **Mudo y SOLO son distintos**: mudo calla esa; solo calla TODAS LAS DEMAS.
   Es el par de botones de cualquier mesa, y sin el segundo no se puede
   escuchar una pista sola para saber cual chasquea.
3. **El medidor es de la pista, no del maestro.** El maestro dice si te pasas;
   la pista dice QUIEN se pasa.
4. **Derivar no es bajar el volumen.** Una pista derivada a fichero se graba
   con su ganancia puesta y *antes* del maestro, porque el maestro es para los
   oidos y la grabacion es para despues.
5. **Un proceso no es una pista: PIDE una.** Y si no pide ninguna, entra en la
   pista `otros`. Asi un programa viejo sigue sonando sin saber que hay mesa.

---

# 2. LA VENTANA, tomada de Premiere Pro

Lo que se toma de Premiere no es el aspecto: es **el orden de lectura**. En su
mezclador de audio, cada pista es una COLUMNA vertical con el medidor a la
izquierda del deslizador, los botones arriba y el numero abajo. Se lee de un
vistazo porque todo lo de una pista esta en la misma columna.

```text
  +-- SONIDO ---------------------------------------------------------- [_][X] --+
  |                                                                              |
  |   juego      musica     sistema    voz        otros      || MAESTRO          |
  |   [M][S]     [M][S]     [M][S]     [M][S]     [M][S]     ||                  |
  |   ||||||     ||||||     ||     |   |          |          || ||||||||         |
  |   ||||||     ||||       ||         |          |          || ||||||           |
  |   ||||       ||         |          |          |          || ||||             |
  |   ||         |          |          |          |          || ||               |
  |   -4.2 dB    -12.0      -21.8      -48.0      --         || -1.4 dBFS        |
  |    +0.0      +6.0       -3.0        0.0       0.0        ||  +12.0 dB        |
  |   [  -  ]    [  -  ]    [  -  ]    [  -  ]    [  -  ]    || doblegadas 0     |
  |   [derivar]  [derivar]  [derivar]  [derivar]  [derivar]  || sujetadas 1284   |
  |                                                                              |
  +------------------------------------------------------------------------------+
  |  aparato  1B3F:2008   48.000 Hz   2 canales   16 bits   192 B/ms             |
  |  del aparato [========------] 60 %     |  tarde 0   huecos 29   tramas 15920 |
  +------------------------------------------------------------------------------+
```

Lo que hay en cada columna, de arriba abajo:

| fila | que es | por que esta |
|---|---|---|
| nombre | `juego`, `musica`... | lo puso quien abrio la pista |
| `[M][S]` | mudo y solo | ver 1.2 |
| barras | el medidor: **pico y RMS a la vez** | el pico dice si te pasas, el RMS si se oye |
| primera cifra | el pico en dBFS | lo que SALIO |
| segunda cifra | la ganancia en dB | lo que se PIDIO |
| `[ - ]` | el deslizador (teclado: flechas) | -- |
| `[derivar]` | grabar esta pista a fichero | lo que Windows no da de serie |

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

El contrato para que un programa diga *"lo mio va en la pista `juego`"*: una
operacion nueva sobre el sonido, con el nombre. El que no pida, a `otros`.
Aqui esta el verdadero *"el audio se divide por completo"*.

**Tam: M.** Y hay que decidir si la pista se pide o **se concede**: que
cualquier programa elija su nombre es lo comodo; que lo conceda el propietario
es lo que impide que dos programas peleen por la pista `juego`.

## [ ] M4 -- DERIVAR UNA PISTA A FICHERO

Lo que Windows no da de serie: `[derivar]` escribe esa pista en un `.wav` a su
frecuencia, con su ganancia y **antes del maestro**. Para el video del
propietario, eso es una pista por elemento en vez de una mezcla de la que no se
puede sacar nada.

**Tam: M.** Depende de M0 y de poder escribir ficheros seguidos (ESTRATOS ya
guarda; el ritmo de escritura contra el de audio hay que medirlo).

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
