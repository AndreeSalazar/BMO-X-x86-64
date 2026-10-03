# PLAN EL SONIDO -- mono, estereo, 5.1, 7.1 y 3D, con sus tablas

> Abierto el 2026-09-22 por decision del propietario: *"hay que meter el mono, 3D
> sound, el 5.1, el 7.1, TODO tipo de sonido en general como tablas de guias
> para que cualquier elemento se entere, porque el audio esta muerto y eso mi
> audifono necesita; pero ULTRA DINAMICO, y que tenga amplificador de sonido
> como DaVinci para amplificar lo que quieras, al estilo Premiere Pro"*.
>
> Este documento es la GUIA: las tablas que cualquiera --una app, un accesorio
> de un tercero, el que lea el `save`-- necesita para saber que forma tiene el
> sonido en BMO-X. [`PLAN_AUDIO.md`](PLAN_AUDIO.md) sigue siendo el plan del
> APARATO (enumerar, reclamar, el tubo); este es el plan de la ONDA: lo que viaja por el.

---

# 0. EL DATO INCOMODO, PRIMERO

El `save` del 2026-09-22 a las 00:21 dice del audifono 7.1:

```text
    audifono 3 ranura | canales 3 | mute 1 | reproduce 1
    tubo 1 | frecuencia 48000 Hz | trama 192 B | max packet 192 B
```

**192 bytes por milisegundo a 48.000 Hz son 48 tramas x 2 canales x 2 bytes.**
O sea: el alt setting que BMO-X esta usando de ese audifono es **ESTEREO**, no
7.1. Y `canales 3` no es lo que parece: ese numero sale del *Feature Unit* del
AudioControl (el control de volumen: maestro + izquierda + derecha), no de la
interfaz de reproduccion. Dos cosas distintas con la misma palabra en el mismo
informe -- se arregla en S0.

Por que estereo: `bmo_uaudio::find_playback` devuelve **el PRIMER** alternate
setting que trae un endpoint isocrono de salida, y deja de mirar. Si el aparato
ofrece ademas uno de 6 u 8 canales, hoy **nadie lo ha visto**. No se sabe si lo
tiene. Lo que si se sabe es que no se ha mirado, y eso es lo primero que hay que
arreglar: **la tabla no la escribe el programador, la escribe el APARATO**.

★ Y hay una tercera posibilidad, que es la mas probable con un audifono de dos
auriculares: que su "7.1" sea **virtual** --dos transductores y un procesado
que simula ocho posiciones--, en cuyo caso el 7.1 de verdad no esta en el cable,
esta en el software. Eso no lo hace falso: lo hace **nuestro trabajo** (S7), y
por eso este plan llega hasta ahi.

---

# 1. LAS TABLAS -- lo que hay que saber para hablar de sonido aqui

## 1.1 -- Una muestra, una trama, un intervalo

```text
   MUESTRA    un numero: la posicion del cono del altavoz en un instante
   TRAMA      una muestra POR CANAL: lo que suena a la vez
   INTERVALO  las tramas que caben en 1 ms (el latido del bus USB)
```

Todo lo demas sale de esos tres. `trama 192 B` del save es el INTERVALO en
bytes, y el nombre esta mal puesto: se corrige en S0.

## 1.2 -- Formato de la muestra

| formato | bytes | rango | quien lo usa | en BMO-X |
|---|---|---|---|---|
| PCM 8 bits sin signo | 1 | 0..255, silencio en 128 | los efectos de DOOM en el WAD | entra, se convierte |
| PCM 16 bits con signo | 2 | -32.768..32.767 | **lo normal en USB Audio**, WAV, CD | ✅ el unico que sale al tubo hoy |
| PCM 24 bits en 3 | 3 | -8,3M..8,3M | audio profesional | se declara, no se usa |
| PCM 24 bits en 4 | 4 | idem, alineado | aparatos que prefieren alinear | ⚠ `subframe` 4 con `bits` 24: ya se distingue |
| PCM 32 bits / flotante | 4 | -1,0..1,0 | mezcla interna de un DAW | **la mezcla interna, en 32 bits ENTEROS** |

★ **Dentro se mezcla en 32 bits con signo y se sale en 16.** No por gusto: ocho
canales de 16 bits sumados se salen de 16 bits, y lo que se sale se oye como un
chasquido. Los bits de mas son la HABITACION (headroom) donde la suma cabe antes
de que el limitador decida (S4).

## 1.3 -- La tabla de FRECUENCIAS, y cuales salen exactas

Bytes por intervalo de 1 ms, estereo 16 bits (`rate/1000 x canales x subframe`):

| Hz | tramas en 1 ms | B/ms estereo | exacto? | de donde viene |
|---|---|---|---|---|
| 8.000 | 8 | 32 | ✅ | telefonia |
| 11.025 | 11,025 | 44,1 | ❌ **fraccion** | los efectos de DOOM |
| 16.000 | 16 | 64 | ✅ | voz de banda ancha |
| 22.050 | 22,05 | 88,2 | ❌ **fraccion** | la mitad del CD |
| 32.000 | 32 | 128 | ✅ | radio digital |
| 44.100 | 44,1 | 176,4 | ❌ **fraccion** | **el CD, y casi todo el MP3** |
| 48.000 | 48 | 192 | ✅ | **lo que suena hoy**, video, USB |
| 88.200 | 88,2 | 352,8 | ❌ fraccion | el doble del CD |
| 96.000 | 96 | 384 | ✅ | alta resolucion |
| 176.400 | 176,4 | 705,6 | ❌ fraccion | el cuadruple del CD |
| 192.000 | 192 | 768 | ✅ | el techo de USB Audio 1.0 |

★★ **Aqui esta, en una tabla, por que 48.000 suena y 44.100 no.** Una frecuencia
"exacta" cabe entera en el milisegundo del bus; una con fraccion NO, y hay que
mandar 44 tramas en unos milisegundos y 45 en otros, con un acumulador que
reparta el 0,1 restante. Sin eso el reloj se va y **se oye**: el audio se
adelanta o se atrasa poco a poco hasta chasquear. Es S2, y es lo que hoy TRUNCA.

## 1.4 -- Los REPARTOS de canales, con su mapa de bits

USB Audio 1.0 declara el reparto en `wChannelConfig` (Tabla 3-16 del estandar),
un bit por posicion. Esta es la tabla completa, que es la que un tercero necesita
para saber que le va a llegar:

| bit | posicion | abreviatura |
|---|---|---|
| 0 | frontal izquierda | L |
| 1 | frontal derecha | R |
| 2 | frontal centro | C |
| 3 | bajos (LFE) | LFE |
| 4 | trasera izquierda | LS |
| 5 | trasera derecha | RS |
| 6 | frontal centro-izquierda | LC |
| 7 | frontal centro-derecha | RC |
| 8 | trasera centro | S |
| 9 | lateral izquierda | SL |
| 10 | lateral derecha | SR |
| 11 | arriba | T |

Y los repartos con nombre, con lo que cuestan a 48.000 Hz y 16 bits:

| nombre | canales | posiciones | B/ms | `wChannelConfig` |
|---|---|---|---|---|
| mono | 1 | C (o L sola) | 96 | `0x004` o `0x001` |
| **estereo** | 2 | L R | **192** | `0x003` |
| 2.1 | 3 | L R LFE | 288 | `0x00B` |
| cuadrafonico | 4 | L R LS RS | 384 | `0x033` |
| 4.1 | 5 | L R LFE LS RS | 480 | `0x03B` |
| **5.1** | 6 | L R C LFE LS RS | **576** | `0x03F` |
| 6.1 | 7 | L R C LFE LS RS S | 672 | `0x13F` |
| **7.1** | 8 | L R C LFE LS RS SL SR | **768** | `0x63F` |
| 7.1 (variante) | 8 | L R C LFE LS RS LC RC | 768 | `0x0FF` |

★ **Lo que esta tabla decide a primera vista**: un aparato que declara
`max packet 192` **no puede** llevar 5.1 en ese alt setting, porque 5.1 son 576
bytes y no caben. Por eso el numero del `save` ya contesta la pregunta sin
abrir el aparato -- y por eso S0 hace que el `save` los liste TODOS.

## 1.5 -- EL CAMINO, de la fuente al cable

```text
   FUENTE          WAV, los efectos de DOOM, un tono, la antena
     |  1.2: se convierte a 32 bits con signo
   REMUESTREO      de SU frecuencia a la del aparato        (S1, S2)
     |
   PANORAMA        de sus canales a los del aparato: paneo, mezcla o HRTF (S5, S6, S7)
     |
   MEZCLA          N fuentes -> UNA, sumando en 32 bits     (S3)
     |
   GANANCIA        el amplificador, en dB, con su medidor   (S4)
     |
   LIMITADOR       lo que se sale, se DOBLEGA, y se cuenta  (S4)
     |  vuelve a 16 bits
   BLOQUE PRESTADO el aparato lo lee por DMA, sin copias    (A4, HECHO)
     |
   TUBO ISOCRONO   una trama cada milisegundo               (HECHO)
```

Cada nivel de esa escalera es una casilla de la seccion 3. **Lo que esta
HECHO es de la mitad para abajo**; lo que falta es de la mitad para arriba, y es
todo aritmetica entera en Ring 3.

---

# 2. ULTRA DINAMICO: LA LEY DE ESTE PLAN

El propietario lo pidio con esas palabras, y aqui significa una regla que se puede
comprobar:

> **Ni una frecuencia, ni un numero de canales, ni un reparto se escribe en el
> codigo. El APARATO los declara, BMO-X los LEE, el `save` los MUESTRA, y la
> cadena se adapta. Si luego enchufas otro, no se recompila nada.**

Tres consecuencias, que son las que hacen falta para que esto no se quede en
una frase:

1. **Se leen TODOS los alt settings, no el primero.** Un audifono ofrece a
   menudo tres o cuatro (estereo 48k, estereo 44,1k, y a veces multicanal).
   Elegir el primero es elegir por accidente.
2. **La eleccion se JUSTIFICA.** `elegido: alt 1, estereo 48.000 -- el de mas
   canales que cabe en su max packet y cuya frecuencia es exacta`. Un numero sin
   su motivo es un numero del autor, no del que lee (regla del 20-09).
3. **Lo que no se puede, se dice.** Una fuente 5.1 en un aparato estereo no
   falla: se MEZCLA a estereo, y el informe dice que se mezclo. Callarlo seria
   fingir que sono como se pidio.

---

# 3. LAS CASILLAS

## [X] S0 -- EL CENSO DICE LA VERDAD, y la tabla la llena el aparato -- **HECHO el 2026-09-22**

Hoy `find_playback` toma el primero y para. Aqui: recorrer la configuracion
entera, guardar **todos** los alt settings de reproduccion (hasta un tope
dicho), y de cada uno su `alt`, canales, `wChannelConfig`, bits, `subframe`,
frecuencias y `max packet`. Elegir despues, con la regla de 2.2, y guardar el
motivo.

En el `save`, una tabla nueva bajo `audio`:

```text
    formatos que ofrece el aparato ..................................
      alt  canales  reparto   bits  frecuencias          max pkt  elegido
        1        2  L R         16  44100 48000              192  <- si (exacta, cabe)
        2        2  L R         24  48000                    288
        3        6  L R C LFE LS RS  16  48000               576  (no cabe en 1 ms)
```

Y se arregla de paso la confusion de 0: `canales` pasa a ser `canales de
volumen` (Feature Unit) y `trama` pasa a ser `bytes por ms`.

**Tam: M.** Es parseo y presentacion, sin metal nuevo.

### [X] HECHO (2026-09-22): el aparato escribe su propia tabla

`bmo_uaudio::stream::todas_las_reproducciones` recorre la configuracion entera
y guarda **todos** los alternate settings de reproduccion (hasta ocho);
`elegir` toma el de mas canales entre los que CABEN en su `wMaxPacketSize` a
una frecuencia exacta, y `find_playback` pasa a ser esas dos. Donde habia un
`return` --el que hacia que nadie supiera si este audifono ofrece mas-- ahora
hay un `entregar(p)` y se sigue. Tres pruebas nuevas con un aparato de tres
formatos (estereo 48k, estereo 44,1k y 5.1 que NO cabe): se ven los tres, y se
elige el primero por la regla, no por el orden.

El kernel los guarda en `reclamar` y los publica por `INFO_AUDIO_FORMATOS`
(0x88), `INFO_AUDIO_FORMATO` (0x89) y `INFO_AUDIO_FRECUENCIA` (0x8A), con sus
gemelos en el ABI y en `userland`. Y el `save` los muestra con la cuenta hecha:

```text
    formatos que el aparato DECLARA, y la cuenta de cada uno .......
    formatos                2          alternate settings con endpoint isocrono de salida
      alt  canales   bits  B/ms  max pkt  sinc    frecuencias
        1        2     16   192      192  adapt   48000  <- ELEGIDO
        2        2     16   176      192  adapt   44100
      la cuenta: B/ms = (Hz / 1000) x canales x bytes por muestra. Si pasa de
      `max pkt`, ese formato NO cabe en el milisegundo del bus y no se puede usar.
      192 B/ms a 48 kHz son 48 x 2 x 2: ESTEREO. Un 5.1 pediria 576.
```

De paso se arreglan las dos palabras que mentian: `canales` pasa a ser
**`canales de volumen`** (son los del Feature Unit, el mando, no los que
suenan) y `trama` pasa a ser **`bytes por ms`**. Y cada formato va tambien a
`DATOS.TXT` crudo (`audio.formatoNN_formato`, `audio.formatoNN_hz`).

★ **Con esto, el proximo arranque contesta la pregunta de fondo**: si la tabla
trae una fila de 6 u 8 canales, el 7.1 esta en el cable; si solo hay filas de
2, su 7.1 es virtual y lo tenemos que hacer nosotros (S7).

## [ ] S1 -- LA CADENA, con una fuente y sin remuestrear

Una fuente PCM (WAV o un tono) de la misma frecuencia que el aparato, convertida
a 32 bits, por la cadena entera hasta el bloque prestado. Es lo que `musica.inti`
ya hace a mano; aqui se convierte en una pieza reutilizable de Ring 3
(`bmo-sonido`), para que la use tambien DOOM y cualquier tercero.

**Tam: M.** ⚠ **Esta casilla la aprueba el metal, no el emulador**: si no se OYE,
lo de arriba no importa.

## [ ] S2 -- LA FRACCION: 44.100 Hz y sus parientes

El acumulador de 1.3: llevar la parte fraccionaria y mandar 44 o 45 tramas segun
toque. Sin esto, la mitad de la musica del mundo (CD, MP3) no se puede tocar sin
remuestrear, y remuestrear 44,1 a 48 **a mano** es peor que mandarlo tal cual si
el aparato acepta 44.100.

**Tam: S.** Es un contador y un resto. Cierra la fila ⛔ del ADN de
[`PLAN_AUDIO.md`](PLAN_AUDIO.md) 6.2.

## [ ] S3 -- EL MEZCLADOR: N fuentes, una salida

Hasta N fuentes vivas a la vez (8 basta: es lo que usa DOOM), cada una con su
frecuencia, sus canales y su estado. Suma en 32 bits. Reglas:

* una fuente que se queda sin datos **no chasquea**: se desvanece en unos pocos
  milisegundos y se calla (un corte seco es un chasquido, y es el fallo numero
  uno de los mezcladores caseros);
* la cuenta de `huecos` que ya tiene el `save` pasa a decir **cual** fuente
  llego tarde;
* la mezcla es de Ring 3, **no del kernel**: dos propietarios escribiendo en el
  aparato no es mezclar, es ruido (lo dice `userland/src/sonido.rs` desde el
  primer dia). El que mezcla es UN proceso con la capability.

**Tam: L.**

## [X] S4 -- EL AMPLIFICADOR, con medidor y limitador (*"como DaVinci"*) -- **HECHO el 2026-09-22**

Lo que el propietario pidio, y tiene un motivo concreto suyo: *"soy una persona
medio sordo"*. Su audifono declara **-45,0 a 0,0 dB**: 0 dB es su techo, y por
encima **el aparato no da mas**. Subir de ahi solo se puede en el software, y
subir en el software sin red es exactamente como se rompe el sonido. Asi que
esta casilla son tres piezas, no una:

```text
   GANANCIA     un factor por fuente y uno maestro, en dB (-inf .. +24 dB)
                  q = 10^(dB/20) en coma fija; +6 dB = x2 exacto
   MEDIDOR      pico y RMS de cada fuente y del maestro, en dBFS
                  es el vumetro de Premiere: sin el, amplificar es a ciegas
   LIMITADOR    lo que pasa de 0 dBFS se DOBLEGA (no se corta), y se CUENTA
```

Tabla de ganancias, para que nadie tenga que calcularlo:

| dB | factor | que hace |
|---|---|---|
| -20 | 0,10 | una decima parte |
| -6 | 0,50 | la mitad exacta |
| -3 | 0,71 | la mitad de POTENCIA |
| 0 | 1,00 | tal cual |
| +3 | 1,41 | el doble de potencia |
| **+6** | **2,00** | **el doble de amplitud** |
| +12 | 3,98 | cuatro veces |
| +20 | 10,0 | diez veces |
| +24 | 15,8 | el techo que se propone |

★★ **Y la regla que hace que esto sea de BMO-X y no de un reproductor
cualquiera: el amplificador NO MIENTE.** Si el limitador tuvo que doblegar
muestras, el `save` dice cuantas y el escritorio lo muestra mientras pasa. Un
amplificador que sube 20 dB y recorta el 30 % de las muestras "suena mas alto"
y suena MAL, y el que lo escucha no tiene forma de saberlo. Aqui si:

```text
    ganancia maestro     +12.0 dB     lo que se pidio
    pico                  -0.4 dBFS   lo mas alto que llego a la salida
    doblegadas              1284      OK muestras que el limitador sujeto (de 480.000)
```

Con eso, subir hasta oirlo bien es una decision con su numero delante, no una
ruleta. **Tam: L.**

### [X] HECHO: `platform/shared/bmo-amplificador` (2026-09-22)

El propietario corrigio el orden de este plan y tenia razon: *"solo ponle
amplificador porque es personal, y para sorpresa el amplificador es lo que SUMA
LA BASE... empezamos desde el inicio, GENESIS"*. Lo es, y se ve en una lista:

```text
   mezclar N fuentes  =  SUMAR con ganancia, y no pasarse
   amplificar         =  ganancia, y no pasarse
   bajar 5.1 a 2      =  SUMAR con ganancia (0,707), y no pasarse
   situar en el 3D    =  ganancia distinta por canal, y no pasarse
```

Los cuatro son la MISMA pieza, asi que se escribio primero. Un crate `puro`,
sin dependencias, sin `unsafe` (`forbid`) y **sin una sola coma flotante**:

* `Ganancia` en 1/256 de dB --la unidad del Feature Unit de USB, o sea la que
  ya sale en el `save`--, resuelta a un factor Q16.16 con dos tablas (dB
  enteros y dieciseisavos) mas una interpolacion. `porcentaje(200)` = +6 dB;
  `db_entero(-3)`; `SILENCIO` es cero de verdad y no un susurro;
* `sumar()` en un acumulador de 32 bits, que es **la base**: ocho fuentes a
  pleno caben enteras antes de que el limite decida;
* `Limite` con envolvente (ataque ~1 ms, relajo ~100 ms): **baja la ganancia
  en vez de cortar la punta**, que es la diferencia entre un limitador y un
  recortador. Cuenta `sujetadas` y `dobladas`;
* `Medidor` con pico y **RMS** en dBFS (raiz entera y `log2` en coma fija),
  porque el pico no dice si algo se oye flojo y el RMS si.

**26 pruebas**, y tres de ellas cazaron tres defectos de la primera version
antes de que llegaran a ningun sitio:

| la prueba | lo que cazo |
|---|---|
| `la_ganancia_sube_siempre_que_se_le_pide_mas` | por debajo de -24 dB el reciproco pedia a la tabla un factor que no tiene y **saturaba**: la curva de volumen bajaba al subir |
| `el_limite_baja_la_ganancia_en_vez_de_recortar` | el relajo empujaba hacia "ninguna reduccion" en vez de hacia lo que la onda permite: sube-y-baja, y **888 muestras recortadas a pelo** con una onda sostenida |
| `el_rms_distingue_una_cancion_floja_de_una_fuerte` | el margen que este ayudante habia puesto a ojo (30 dB) contra el que sale de la cuenta (29,6) |

### El orden, corregido otra vez (2026-09-22): S4b NO era lo siguiente

El propietario pregunto *"S4b, haber, es correcto ese camino?"*, y no lo era. Una
orden `audio ganancia N` con sus filas en el `save` es un mando **sobre una
onda que no existe**: `encoladas 0` desde que el tubo se abrio, o sea que NI UNA
MUESTRA ha llegado nunca al audifono. Poner un medidor ahi seria un numero
bonito midiendo silencio, que es la definicion de escrito-y-sin-ejecutar.

Y al mirarlo salio el bloqueo de verdad, que no estaba en ningun plan:

```text
   el kernel ofrece el contrato del productor       A4, campos 8..13 de AUDIO_OP_TUBO
   `musica.inti` lo usa                             invoca(cap, tubo, 8, pcm, 0)
   Rust NO PODIA                                    audio_tubo(que) manda (que, 0, 0)
```

`userland::sys::audio_tubo` pasa un solo argumento y ademas reclama y suelta el
aparato en cada llamada. O sea: **ningun programa de Ring 3 escrito en Rust
podia hacer ruido**, el DIRECTOR incluido. El amplificador estaba escrito y no
tenia donde enchufarse: la pieza existia, el cable no.

**HECHO el mismo dia: [`Sonido::tubo()`] y `Tubo`** en `userland/src/sonido.rs`,
con los verbos completos (`abierto`, `bytes_por_trama`, `frecuencia`, `armar`,
`callar`, `ofrecer`, `escrito`, `leido`, `pendientes`, `huecos`, `encoladas`,
`tarde`, `soltar`), sosteniendo la capability en vez de pedirla por llamada.

El orden que sale de ahi, y que sustituye al "S4b":

| # | que | quien lo aprueba |
|---|---|---|
| **P0** | correr `musica` en el Ryzen | **el metal**: suena algo, si o no? Cuesta un arranque y **no hay codigo que escribir** |
| **P1** | los verbos del tubo en Rust | ✅ hecho: compila; lo aprueba P2 |
| **P2** | un productor en Rust por la cadena entera (fuente -> amplificador -> bloque prestado -> tubo) | el metal: `encoladas` sube y `huecos` es 0 |
| **P3** | `audio ganancia N` y las filas del `save` (`ganancia`, `pico`, `dobladas`) | lo que era "S4b", y ahora tiene onda que medir |

★ **P0 va primero y es gratis.** Si `musica` no suena, P2 se escribiria encima
de un camino roto y el fallo se buscaria en el sitio equivocado -- que es
exactamente lo que paso el 21-09 con el paquete del EP0.

## [ ] S4c -- EL MAESTRO: la ultima etapa, en el kernel, y su panel en el escritorio (2026-09-22)

> Codigo HECHO el 22-09; la casilla se cierra cuando el metal conteste la tabla
> de abajo.

El propietario, con DOOM sonando: *"funcionan pero no tengo control de audio...
un control que viva en mi escritorio, no como app... inspirado en Premiere, para
amplificar sonido maestro"*.

### Por que no tenia control: producir y mandar eran el MISMO permiso

Mover el volumen pasaba por reclamar el sonido, y reclamarlo es exclusivo. Con
DOOM sonando, el escritorio no podia ni tocar el volumen: `audio volumen`
devolvia 0 y la ventana F10 decia *"lo tiene OTRO proceso"*. En una mesa de
verdad el que toca no lleva el fader maestro.

### La decision, con su coste delante (el propietario eligio)

| camino | lo que da | lo que cuesta |
|---|---|---|
| **kernel, ultima etapa** (ELEGIDO) | vale para DOOM, `musica` y cualquiera **sin tocarlos** | `bmo-amplificador` entra en Ring 0 (nuestro, sin dependencias ni `unsafe`) y la frase de la seccion 5 se corrige |
| el escritorio mezcla | el arbol entero de LA MESA | DOOM y `musica` cambian de contrato, y si el escritorio se atasca se corta TODO |
| solo el aparato | nada nuevo | no pasa de 0 dB: no amplifica |

Y dos mas: **solo el escritorio mueve el maestro** (el kernel lo concede a quien
tiene la pantalla), y **el techo lo pone la interfaz** (*"tener control de
interfaz"*): el fader llega hasta donde llega el crate, +24 dB.

### Lo que hay

| pieza | donde | que hace |
|---|---|---|
| `Maestro` | `bmo-amplificador/src/maestro.rs` (9 pruebas) | rampa de 1 dB por ms (8 por debajo de -40), limite **sin ataque** y medidor izquierdo/derecho; en reposo es un cable **bit a bit** |
| `Limite::inmediato` | `bmo-amplificador/src/lib.rs` | el limite de mezcla (1 ms de ataque) dejaba 982 muestras cortadas a pelo con +12 dB; este, cero |
| la etapa | `kernel/.../dev/usb/maestro.rs` | copia cada trama a un marco SUYO y le pasa el maestro; **en reposo** el xHC sigue leyendo del bufer de la app, como antes |
| el mando | `TASK_OP_AUDIO_MANDO` (0x34) | fader en 1/256 dB y mudo; **no reclama nada**; `PERMISSION_DENIED` a quien no tenga la pantalla, con el motivo en CABINA |
| las dos etapas | `maestro::mover` | dentro del rango del aparato lo da **el aparato** (limpio); por encima, aparato a tope y el resto digital |
| el de fabrica | `uaudio::leer_rango` | un `GET_CUR` al reclamar: el volumen que TRAIA el audifono, antes de que nadie le mande nada |
| lo que se lee | `INFO_AUDIO_MAESTRO/MEDIDOR/LIMITE/FABRICA` (0x8B-0x8E) | sin handle, como el resto del audio |
| el panel | `director/src/scene/sound.rs` + `desktop/sonido.rs` | F10 o el indicador de la barra: fader vertical en dB, medidores I/D con marca de pico y luz de RECORTE, numeros al lado, MUDO |
| el `save` | `save_cabina::report_maestro` | todo lo que el panel muestra, y a DATOS.TXT |

### Lo que el metal tiene que contestar

| que | afirma | como se cae |
|---|---|---|
| abrir F10 (o clic en `vol` de la barra) con DOOM sonando | el panel se abre y **DOOM sigue sonando** | "lo tiene OTRO": el mando sigue pidiendo el aparato |
| el panel sin tocar nada | `maestro` = el de fabrica, `aparato ... de fabrica` | 0 dB inventado: no se leyo el `GET_CUR` |
| los medidores con DOOM | se mueven con los disparos, verde/ambar | quietos: `ventanas` del `save` no sube |
| subir con la flecha hasta 0 dB | sube el volumen DEL APARATO, sin luz de recorte | no cambia nada: el `SET_CUR` en dB no llega |
| seguir subiendo a +12 dB | mas fuerte que nunca; `digital +12.0`; `limite` negativo en los golpes | igual de fuerte: la etapa no actua (mirar `estado` en el `save`) |
| la luz de RECORTE | apagada o casi: `dobladas` en decenas como mucho | cientos: el limite no sujeta |
| `M` | se calla en ~50 ms, sin chasquido, y vuelve igual | chasquido: la rampa no corre |
| el `save` despues | `estado 1`, `tocado 1`, las filas del maestro | `estado 2/3/4`: el maestro no actua y el panel lo dice |

### 14:07 del 22-09: el metal contesta -- FUNCIONA, y *"pelea y da tirones"*

```text
   estado 1 | tocado 1 | fader +24.0 | parte aparato 0.0 | digital +24.0
   dobladas 0 | de fabrica 0.0 dB | ventanas 816 | encoladas 40.837 | tarde 0
   huecos 4.416
```

El panel entro con DOOM sonando, el fader mando y la etapa actuo (`estado 1`,
`digital +24.0`, ni una muestra doblegada). Y el propietario: *"el audio parece
que pelea y da tirones... no sentia MAS fuerte"*. Tres causas, y ninguna es la
que parecia:

1. **El techo es fisico.** El aparato ya estaba a tope (`de fabrica 0.0 dB`),
   asi que el "mas fuerte gratis" no existia. Por encima de 0 dBFS no hay mas
   onda: lo que el fader sube de ahi, el limite lo tiene que bajar. DOOM saca
   sus efectos a unos **-12 dBFS** (volumen de efectos 8 de 15 --64 de 127-- y
   paneo lineal, que en el centro deja 1/4 por lado), asi que **a partir de
   +12 dB el fader ya no sube el volumen: lo APLASTA**. A +24 son 12 dB de
   limite en cada disparo.
2. **Y aplastar con un relajo corto es BOMBEAR**: entre disparo y disparo el
   limite devuelve la ganancia, el fondo sube, y el siguiente lo hunde. Eso es
   la "pelea". El relajo ademas era la MITAD de lo escrito: los "100 ms" se
   contaban por muestra intercalada, 50 ms reales en estereo. **Arreglado**: el
   maestro sabe sus canales y devuelve en 250 ms (`RELAJO_MS`); la prueba
   `con_el_fader_arriba_el_fondo_no_bombea` fija que la cola sube menos de la
   mitad que antes.
3. **Los `huecos` no eran los tirones.** 4.966 con 74 s de DOOM y 4.416 con
   35 s: casi lo mismo con el doble de juego. Un fallo repartido por la partida
   crece con ella; este no. Era el ARRANQUE de DOOM (ofrece su bloque en
   `I_InitSound` y tarda ~5 s en cargar antes de mezclar). Ahora el kernel lo
   separa: `INFO_AUDIO_TIRONES` (0x8F) cuenta solo lo que falta **ya sonando**,
   en cortes y con el mas largo, y el `save` lo dice en tres filas.

Y lo que se oia como tirones tenia un cuarto socio en DOOM: sus efectos suben
de 11.025 a 48.000 Hz en ESCALONES, y +24 dB subia tambien ese rechinar. Ver
[`PLAN_DOOM.md`](terminado/PLAN_DOOM.md) 5.3e.

Lo que el panel dice ahora: la fila `aplasta` es lo MAS que el limite bajo en
la ventana (antes, lo del instante de cerrarla, que solia ser la cola), en
rojo pasados 6 dB, con el aviso *"aplasta mas de 6 dB: subir ya no da mas
fuerza"*.

| que | afirma | como se cae |
|---|---|---|
| DOOM con el fader a +12 | mas fuerte que a 0 y `aplasta` en ambar, pocos dB | rojo: DOOM sale mas fuerte de lo calculado |
| a +24 | `aplasta` en ROJO y el aviso; suena mas denso, no a tirones | tirones igual: no era el bombeo, mirar `tirones` |
| `tirones` en el `save` tras jugar | 0, o pocos y cortos (`el mas largo` < 20 ms) | cientos: el productor SI llega tarde en partida |
| `huecos` | sigue en miles (el arranque), y ya no importa | -- |

**14:26, contestado**: `en marcha 3.626`, `tirones 29`, `el mas largo 1.420 ms`
y `tarde 0`. Los tirones SON de verdad y NO son del kernel ni del aparato: son
del mezclador de DOOM, que solo se rellenaba desde el fotograma. La pieza, en
[`PLAN_DOOM.md`](terminado/PLAN_DOOM.md) 5.3f. El `limite -5,2 dB` de ese `save` NO es lo
que aplasto en partida: es donde se quedo el limite cuando DOOM callo (en
silencio no pasaba onda y no se movia). **Arreglado en la pieza**: el silencio
pasa ahora por el limite, que suelta con su relajo, y el maestro vuelve al
reposo (prueba `en_el_silencio_el_limite_suelta_y_el_maestro_vuelve_al_reposo`).

### 16:09 del 22-09: el techo pasa de +24 a +52 dB, y el fader se TECLEA

Con las voces sonando el propietario pidio mas: *"no se escucho mucho, sube a 52"*,
y *"ponlo como control de numeros"*. Lo que eso es, dicho antes de hacerlo: el
aparato ya esta a 0 dB (su tope) y una onda no sale por encima de 0 dBFS, asi
que por encima de ~+24 lo que sube es lo FLOJO y el limite sujeta la punta.
Es compresion --lo que hace un audifono-- y se oye mas denso, no mas alto en
el pico. La fila `aplasta` lo dice en rojo.

* `bmo-amplificador`: `MAX_DB_MAESTRO` = +52 dB y `Ganancia::db_hasta`. Las
  pistas de LA MESA siguen en `MAX_DB` (+24). Por encima de 24 el factor sale
  por tramos de 20 dB (x10 exacto), como ya salia por abajo. 2 pruebas.
* el panel F10: escala hasta +52, y **se escriben los dB** (`40`, `-12`, `0`)
  y Enter; Retroceso borra, ESC deja lo escrito sin cerrar el panel.
* y el `save` dejo de decir `banco 16777216 B` con `banco de 0 pid`: soltar
  el banco pone su medida a cero.

### Lo que NO es

No es LA MESA: una sola perilla para todo lo que suena, no una por programa ni
por pista. El arbol sigue siendo de Ring 3 ([`PLAN_LA_MESA.md`](PLAN_LA_MESA.md)).

## [ ] S4d -- LAS VOCES DEL ORQUESTADOR: la app declara, el kernel toca (2026-09-22)

El anillo PCM obliga a la app a llegar SIEMPRE a tiempo: es multiplexar, el
sistema espera a la app. Las voces son lo contrario, y por eso son de un
ORQUESTADOR: la app presta un banco con sus muestras y dice *"toca este, asi"*;
el hilo del bus las mezcla cada trama, justo antes del maestro.

```text
                         ANILLO (el tubo, A4)     VOCES (esto)
   quien marca el paso   la app                   el orquestador
   si la app se atasca   CORTE                    sigue sonando
   para que es           un FLUJO (cancion)       un SONIDO (empieza, dura, acaba)
```

Los dos conviven: `maestro::componer` suma anillo + voces en 32 bits y el
limite del maestro lo devuelve a 16 (`pasar_acumulador`: sin atajo de reposo,
porque dos disparos a pleno ya se pasan a 0 dB). Sin voces, el camino es el de
antes, bit a bit.

**La forma no es nueva, y eso es lo que la valida**: el Sound Manager del Mac
(canales con ordenes), DirectSound (bufer estatico + voz) y OpenAL (buffers +
sources) llegaron a lo mismo. Lo que esos traian y esto no traia: bucle y
tono. **El BUCLE entro el mismo dia**, porque lo pidio un programa (la musica
de DOOM: la cancion entera en el banco, `bmo_voz_tocar_bucle`, bit 56). El
TONO sigue fuera hasta que alguien lo pida.

**La frontera, que sale del precedente**: XP mezclaba en el kernel (KMixer) y
Vista lo saco a un proceso; el Mac clasico tambien mezclaba en el kernel y Apple
lo fue sacando. Los motivos fueron codigo de terceros y efectos enchufables en
el anillo cero. Aqui no aplican --Ring 0 cerrado, crate `forbid(unsafe_code)`,
16 voces acotadas-- **mientras ningun efecto enchufable entre en Ring 0**. Esa
es la condicion de esta casilla.

Detalle, piezas y la tabla del metal: [`PLAN_DOOM.md`](terminado/PLAN_DOOM.md) 5.3g.

## [ ] S4e -- EL FONDO: musica compuesta aqui, que se aparta sola para los avisos (2026-10-03)

> Codigo HECHO el 03-10; la casilla se cierra cuando el metal conteste la tabla
> de abajo.

El propietario, despues de oir la ONDA de la maqueta de HERMES: *"me encantan
las musicas que pusiste, puedes integrar? como si fuera fondo, integrado, que
relaja al usuario hasta que aparece una notificacion para avisar [...] y
mejorar mi audio, pero nivel maestro"*.

### Las musicas no eran ficheros: eran partituras

Cada pieza de la maqueta es una semilla, un tempo, una raiz, una escala y un
timbre; el patron de cada compas sale de ahi. `platform/shared/bmo-fondo` es
la MISMA partitura y la misma sintesis, en Rust y en enteros: lo que suena en
el Ryzen es lo que sonaba en el navegador, y ocupa diez lineas, no diez megas.

| pieza | que hace | como se sabe (anfitrion) |
|---|---|---|
| el patron | `azar()` de la maqueta bit a bit, y los umbrales con la fraccion exacta de cada doble | `el_patron_es_el_de_la_maqueta`: igual que `node` paso a paso, en cuatro piezas |
| los osciladores | cuadrada y sierra con **PolyBLEP**: sin el silbido de los armonicos que pasan de 24 kHz y vuelven doblados | `la_cuadrada_no_silba`: mas de 15 dB menos de alias que la ingenua |
| bombo, caja, plato | el bombo cae de 150 a 42 Hz; caja y plato son ruido por un biquad (los coeficientes de la norma de WebAudio, en Q28) | -- |
| las envolventes | una rampa exponencial ES una recta en dB: se lleva en dB y se interpola cada 16 muestras, sin cremallera | -- |
| **el bucle sin costura** | se compone un compas ANTES del cero y se tira: la muestra 0 lleva las colas de la vuelta anterior | `el_bucle_no_tiene_costura`: la segunda vuelta es la primera muestra a muestra |
| el nivel | cada pieza calibrada a **-26 dBFS de fuerza** (RMS), picos de -5 a -10, y el limite sin trabajar | `cada_pieza_suena_igual_de_fuerte_y_el_limite_no_trabaja`: las diez dentro de 1 dB, cero sujetadas |
| la mezcla FONDO | la misma partitura con la bateria muy atras y los acordes delante: acompana, no pide atencion | `la_cancion_suena_mas_que_el_fondo` |
| los avisos | los seis sonidos del sistema, cada uno a **-6 dBFS de pico** | `los_avisos_se_oyen_y_no_se_pasan` |

Con `BMO_FONDO_WAV=<carpeta> cargo test` escribe cada pieza, cada aviso y una
demo del agache como WAV, para oirlos en el anfitrion.

### "Nivel maestro": dos piezas nuevas en `bmo-amplificador`

| pieza | que arregla |
|---|---|
| **la RAMPA de las voces** (`voces::RAMPA_TRAMAS`) | `ajustar` saltaba en la trama siguiente: un escalon de volumen es un CLIC. Ahora llega en 5 ms. Un `tocar` nuevo NO lleva rampa: el ataque es del sonido. Prueba `ajustar_no_salta_va_por_la_rampa` |
| **el AGACHE** (`agacha.rs`) | la musica BAJA sola 15 dB en 30 ms bajo un aviso (8 bajo la app), se queda 400 ms desde el ultimo, y vuelve sola en ~1,3 s. En dB, sin escalones, y en reposo un cable bit a bit. 4 pruebas |

### Por que hacia falta un SEGUNDO banco

El banco de voces era del que reclamo el sonido. Con DOOM sonando, el
escritorio no podia tocar nada: la musica de fondo se habria callado justo
cuando se pidio que siguiera. Ahora hay dos ATRILES con la misma pieza
dentro (`dev/usb/voces.rs`):

```text
   APP     el banco de quien reclamo el sonido, por AUDIO_OP_VOZ (como antes)
   FONDO   el banco del ESCRITORIO, por TASK_OP_AUDIO_FONDO (0x3D):
           canales 0..8 la musica (se agacha), 8..16 los avisos (no)
```

La trama: anillo + voces de la app, y si el fondo suena, su musica en un
cubo aparte, por el agache (pide -15 si suena un aviso, -8 si lo de la app
pasa de -40 dBFS), sumada; y los avisos encima. Despues, el maestro de
siempre. El tubo se arma tambien por el fondo (`armar_fondo`): DOOM al salir
lo calla y la musica sigue. Solo quien tiene la pantalla abre el atril, la
misma regla que el mando del maestro.

En el escritorio: la orden `fondo` (encender, `siguiente`, una pieza por su
nombre, `vol N`, `aviso`, `lista`, `apagar`), y cada `globo::avisar` --los
avisos de verdad, no los consejos que se turnan-- suena encima si el fondo
esta encendido. La siguiente pieza se compone en un segundo sitio del banco
MIENTRAS suena la de ahora: cambiar no deja hueco.

### Lo que el metal tiene que contestar

| que | afirma | como se cae |
|---|---|---|
| `fondo` | suena *Sierra al atardecer*, `compuesta aqui en N ms` con N de unos cientos | `el kernel no acepto el banco`: mirar `fondo` en CABINA (sin tubo, o no somos la pantalla) |
| dejarla un minuto | ni un clic cada ~25 s (la vuelta) | un clic regular: la costura, o la voz en bucle pierde la fraccion |
| `fondo aviso` | dos notas encima; la musica se aparta y vuelve sola en ~1 s | no baja: `FONDO.activas()` no ve el canal 8+ |
| `fondo siguiente` | cambia sin hueco ni golpe | un silencio: se compuso encima de lo que sonaba |
| abrir DOOM con el fondo sonando | sigue sonando, mas baja (-8 dB), y DOOM se oye entero | se calla: el fondo dependia del banco de la app |
| salir de DOOM | la musica sigue y vuelve a su nivel | se calla: DOOM desarmo el tubo y el fondo no lo sostuvo |
| `fondo apagar` | se va sin clic | clic: se solto antes de que acabara la rampa |

### Lo que NO es

No es la ONDA de HERMES: ni ficheros, ni listas, ni MP3 (M2 de
`PLAN_MEDIOS.md`). Es la misma partitura de la maqueta, compuesta en la
maquina.

### Y la PASTILLA, en el escritorio de verdad (03-10, por la tarde)

`scene/pastilla.rs` y `desktop/pastilla.rs`: con el fondo sonando, una rayita
de neon arriba en el centro que LATE con el medidor del maestro (lo que de
verdad sale por el cable); asoma con rebote al acercar el raton o al cambiar
de pieza, volumen o pausa; con un clic se abre: la onda en vivo, los
medidores izquierdo y derecho con su pico, pausa, siguiente, volumen y una
recomendacion (la siguiente de las tranquilas: del catalogo, sin inventar
amigos). No es una ventana: no quita el foco, y un clic fuera sigue su
camino. La pausa es de radio --la voz sigue en su bucle, callada por la
rampa-- porque el orquestador no dice por donde va una voz. [!] Con un juego
a pantalla completa el escritorio no pinta y la pastilla tampoco; la musica
sigue y se agacha sola.

| que | afirma | como se cae |
|---|---|---|
| `fondo` | aparece la rayita arriba y late con la musica | quieta: el medidor no llega (`ventanas` del `save`) |
| raton arriba al centro | asoma con el nombre, pausa y siguiente | no asoma: `CERCA` o el centro mal medido |
| clic en ella | se abre con la onda y los medidores moviendose | rastro al cerrarla: la capa no devolvio lo que tapaba |
| pausa, +, -, siguiente, la recomendacion | hacen eso, y la pastilla lo dice | el clic se va a la ventana de debajo |

## [ ] S4f -- EL OIDO: el perfil de quien escucha, en todo lo que suena (2026-10-03)

> Codigo HECHO el 03-10; la casilla se cierra cuando el metal conteste la tabla
> de abajo.

El propietario: *"vamos a mejorar el audifono, eso es para que se aplique en
general, y control y mas cosas [...] audio profesional"*. El documento ya
decia dos cosas que deciden el como: el propietario es duro de oido, y su
audifono tiene DOS transductores (seccion 3.5). Lo que se haga por el oido
va en la etapa que pasa TODO: el MAESTRO.

| mando | que hace | para que |
|---|---|---|
| **agudos** -12..+12 dB | estante a 3,5 kHz | lo primero que se pierde, y lo que hace ENTENDER una voz |
| **medios** -12..+12 dB | campana a 1 kHz (Q 0,7) | el cuerpo de la voz |
| **graves** -12..+12 dB | estante a 100 Hz | el golpe; bajarlo aclara |
| **balance** -100..+100 | atenua el lado contrario, por rampa | quien oye mejor por un oido |
| **mono** | suma los dos lados | que no se pierda lo que la mezcla puso solo en el otro |

En `bmo-amplificador::oido`, dentro del `Maestro`: ANTES de la ganancia y del
limite, asi que lo que el tono suba tambien lo sujeta el limite.

* **En plano es un cable**: las pruebas de siempre del maestro siguen
  pasando bit a bit.
* **Sin coma flotante**: los coeficientes salen de una tabla por dB entero
  (las formulas del *Audio EQ Cookbook*), a 44,1 y 48 kHz. Fuera de esas
  frecuencias el tono no se aplica, y `oido` y el `save` lo dicen.
* **Sin el soplido de los filtros graves**: el estado lleva 8 bits de mas y
  el resto de cada redondeo vuelve a la muestra siguiente. Prueba
  `tras_un_golpe_el_silencio_es_silencio` (graves a +12: tras un golpe, un
  segundo de silencio es CERO).
* Una banda que se enciende empieza de cero; el balance va por rampa.
* 5 pruebas: las tablas contra la respuesta en doble, el tono a sus
  frecuencias, el silencio, mono y balance, y el plano.

El mando: cinco ordenes nuevas en la puerta del maestro (`AUDIO_MANDO_BALANCE`
.. `AUDIO_MANDO_PLANO`, 3..8), solo el escritorio, y `INFO_AUDIO_OIDO` (0xC8)
para leerlo. En Ejecutar, `oido` (y `oido voz`, `oido musica`, `oido plano`,
`oido agudos N`, `oido balance N`, `oido mono si`), y en el `save`, sus filas
junto a las del maestro (y las del FONDO: `INFO_AUDIO_FONDO`, 0xC9).

### Lo que el metal tiene que contestar

| que | afirma | como se cae |
|---|---|---|
| `oido voz` con DOOM o el fondo sonando | las voces y los platillos mas claros, sin clic al ponerlo | clic: una banda arranco con memoria vieja |
| `oido graves 12`, y silencio | un silencio limpio, sin soplido | soplido: el redondeo del estante grave |
| `oido mono si` | lo de un lado suena en los dos | -- |
| `oido balance 100` | todo a la derecha, por una rampa corta | un golpe al cambiar |
| `oido plano` | el maestro vuelve al cable (`estado 1` y nada que hacer) | -- |

## [ ] S4h -- NEKO PHONK: el gato y el cencerro, para dar vida a BMO-X (2026-10-03)

El propietario: *"inspirate en combinar Phonk [...] y Geoxor [...] estudia
como se hicieron, pero para tener todo ese sonido en cat o neko sonido, para
dar vida en todo mi BMO-X"*.

[!] Los tres enlaces de YouTube no se pudieron abrir: la red de esta maquina
los niega (403). Lo de abajo sale de lo que esos estilos SON como tecnica, y
todo se compone en BMO-X; no se copia ni un compas.

| de | que | como se hace aqui (`bmo-fondo::neko`) |
|---|---|---|
| phonk | el CENCERRO del 808 | dos cuadradas en razon 1 : 1,48; golpe que cae 9 dB en 18 ms y cola de 380 ms |
| phonk | el 808 | un seno que cae de la octava a su nota y se SATURA (`1,5u - 0,5u^3`, empujado x3): el grunido. Prueba `el_808_grune` |
| phonk | el BOMBEO | lo melodico cae al 30 % con cada 808 y vuelve en 125 ms. Prueba `el_phonk_bombea` |
| phonk | la escala FRIGIA | la segunda menor sobre la raiz: la tension |
| Geoxor | lo tierno y lo bruto | acordes de sierra cortos y brillantes, "voces" que juegan |
| el gato | el MAULLIDO | una sierra por dos resonancias que se MUEVEN (los formantes: la vocal) con su curva de tono; cinco: nya, miau, mrrp, la pregunta, el triste. Medido: "nya" va de 680 a 820 Hz y vuelve, y se oscurece |
| el gato | el RONRONEO | ruido grave (dos polos a 250 Hz) que late a 26 Hz |
| el gato | el BUFIDO | aire a 4,2 kHz: el gato enfadado |

* **El tema NEKO de los avisos** (`avisos::Tema::Neko`): mensaje "nya",
  conecta "mrrp", error un bufido con un 808 grave, hecho "nya" y el cencerro
  que sube, lo que llega un ronroneo que crece y "nya", lo que se va el
  maullido triste... los doce a -6 dBFS de pico, con las mismas reglas de la
  VOZ (S4g) y su sitio en 3D. `fondo tema neko` (y `clasico`).
* **Tres piezas NEKO PHONK** en el compositor (`Estilo::NekoPhonk`): Neko
  drift (144), Gato de neon (128), Nyan de medianoche (136). El 808 con un
  golpe encima, la caja en el tres, el plato con redobles a fusas, la frase
  del cencerro a contratiempo, los acordes de sierra de la segunda mitad,
  "nya" al final de cada compas impar y "miau" al final de todo; a -26 dBFS
  y sin costura en el bucle. `fondo neko drift`.
* En la maqueta: el tema "neko phonk" en "La voz de BMO-X" (tambien el
  compilador: tics de cencerro, bufidos por error) y la lista "neko phonk"
  en la ONDA, con el mismo patron, en WebAudio.
* `BMO_FONDO_WAV=<carpeta> cargo test` escribe `neko_*.wav`,
  `cancion_neko_drift.wav` y `demo_neko_3d.wav`.

| que | afirma | como se cae |
|---|---|---|
| `fondo tema neko` y `fondo aviso mensaje` | un "nya" que se entiende como gato, a la izquierda | un pitido: las resonancias no se mueven |
| `fondo neko drift` | el 808 grune, el cencerro baila entre golpes, todo bombea | el 808 suena limpio: la saturacion no llega |
| `fondo aviso error` en neko | un bufido con un golpe grave, delante | -- |

* **La receta**, aparte, para guardarla: `docs/arte/receta_nya.md` (las
  curvas, la tabla de resonancias, el ronroneo, el bufido, el cencerro, el
  808, el bombeo, que aviso lleva que, y como jugar con el).

## [ ] S4i -- LA BIENVENIDA: "El emisor salta" al llegar, y se minimiza (2026-10-03)

El propietario: *"'el emisor salta' esa musica ME ENCANTA y sirve como fondo
en pantalla cuando lleguen, pero con animacion, como que encanta al usuario,
y ya para que se minimice"*. Y: *"separamos los dos"*: la musica de 8 bits
por un lado, el gato (NEKO) por el suyo, con su receta.

* **Al acabar el arranque** (`desktop::boot`, lo ultimo, despues de
  `al_arrancar`): `musica::tocar` de "El emisor salta" (buscada por nombre,
  con la mezcla FONDO) y el panel de 8 bits en el centro
  (`desktop::bienvenida` + `scene::bienvenida`).
* **El panel** (600 x 340, guarda lo que tapa como la pastilla): crece con
  rebote; estrellas en tres capas que pasan; BMO-X en letras de 5x que
  saltan una tras otra al compas, con los colores rotando; el gato de la
  intro bajado a bloques de 4, que BOTA y toca el suelo en cada golpe del
  bombo (128 pulsos, desde el compas 0 de la pieza); "El emisor salta" con
  su cursor; y once barras en bloques que siguen al **medidor del maestro**
  (sin tubo, se quedan en un bloque y lo dice: "sin tubo de audio: solo se
  ve").
* **Se minimiza** a los 8 s, o con una tecla o un clic: se encoge acelerando
  hacia arriba hasta la medida de la pastilla asomada, y la pastilla asoma
  diciendo lo que suena. La musica sigue. Solo ESC y un clic DENTRO del panel
  se quedan; las demas teclas siguen su camino (no se pierde la primera
  letra), y un clic fuera tambien.
* [!] **Es la unica vez que el escritorio arma el tubo sin que se lo pidan**
  (la regla de `armar_silencio`). Por eso es una clave del fichero:
  `bienvenida = no` en `sys/director.cfg` llega en silencio, como antes
  (`bmo-config`, prueba `la_bienvenida_se_apaga_y_se_guarda`).
* Una vista previa de la geometria, con la mascara del gato y la fuente
  reales, se renderizo en el host; en el metal no se ha visto.

| que | afirma | como se cae |
|---|---|---|
| arrancar con altavoz | suena "El emisor salta" y el panel sale en el centro; el gato bota con el bombo | el panel sale y no suena: mirar `fondo` en CABINA |
| esperar 8 s | el panel se encoge hacia arriba y asoma la pastilla con la pieza | queda un rastro: la save-under no se devolvio |
| una tecla a los 2 s | se minimiza y la letra llega a Ejecutar | la letra se pierde |
| `bienvenida = no` | se llega en silencio, sin panel y sin tubo armado | -- |

## [ ] S4j -- UN LADO SOLO: los canales del APARATO (2026-10-03)

El Ryzen, con HERMES tocando la ONDA: *"se escucha un solo lado y no
escucho ambos lados"*. Lo que dice la foto: el medidor del MAESTRO marca los
DOS lados (las barras pares e impares de la ONDA, izquierdo y derecho, van
parejas), asi que lo que sale por el cable es estereo. El fallo esta despues,
en como se le habla al AURICULAR. Dos causas, y las dos estaban en el codigo:

* **Se cogia el PRIMER Feature Unit** del descriptor. Un auricular con
  microfono declara varios (el del microfono, el del retorno de voz, el de
  los altavoces): si el primero era el del micro, BMO-X subia y bajaba el
  MICROFONO. Ahora `bmo-uaudio` recorre el grafo del AudioControl y coge el
  que tiene aguas arriba el TUBO USB (Input Terminal 0x0101). 4 pruebas
  nuevas (el auricular con el micro delante, en los dos ordenes, el
  adaptador de siempre y un grafo con ciclo); 34 en verde.
* **Se mandaba solo al MAESTRO** (canal 0) y, si lo aceptaba, izquierdo y
  derecho se quedaban como arranco el aparato -- y hay aparatos que arrancan
  con un canal al minimo o CALLADO. Ahora el descriptor dice que canal declara
  volumen y cual mute (`vol_canales`, `mute_canales`), y el kernel
  (`ring0/dev/uaudio.rs`): al RECLAMAR el aparato quita el mute de cada canal,
  lee el volumen de cada lado, lo dice en CABINA y los IGUALA al que mas
  suena (`igualar_lados`); y cada volumen o mute que se mande despues va a
  TODOS los canales que lo declaran.
* Y se ve: `INFO_AUDIO_LADOS` (0xCB) -- lo que traia cada lado y cual venia
  callado -- en el `save` (seccion de audio) y en `oido`.

| que | afirma | como se cae |
|---|---|---|
| arrancar con el auricular y `fondo` | suenan los DOS lados | sigue uno: mirar `oido` (que traia cada lado) y CABINA (`uaudio`) |
| `oido` | `el aparato traia: izquierdo X dB, derecho Y dB` | sin la linea: el aparato no declara volumen por canal |
| el fader del maestro | sube y baja los dos lados a la vez | uno se queda: el aparato no acepta el volumen por canal (CABINA lo dice) |
| `oido izq` y `oido der` (03-10: *"escucho solo por la derecha"*) | un aviso SOLO por ese lado, como la prueba de altavoces de Windows | `oido izq` no suena por la izquierda: no es la mezcla, es el aparato, el cable o el conector (un auricular de 4 polos en un enchufe de 3) |

## [ ] S4k -- LA ANATOMIA: del bit al oido, etapa por etapa (2026-10-03)

El propietario: *"investigar por que llegar a +52.0 no cumple para aumentar
el volumen fuerte en mi audifono; estudiar toda la anatomia, el audio Pro en
todos los puntos, y zero copy"*. Esta es la cadena ENTERA, leida en el
codigo, con lo que cada etapa suma, cuanto copia y cuanto tarda.

```text
 etapa                    donde                         ganancia          copias          tarda
 -----------------------  ----------------------------  ----------------  --------------  ---------
 1 la FUENTE: la musica   bmo-fondo, compuesta UNA vez  -26 dBFS de       0 por trama:    0 (ya esta)
   de fondo               en el banco (6 MiB prestados) fuerza, a         se lee EN SU
                                                        proposito: es     SITIO
                                                        de fondo
   los avisos             el mismo banco                -6 dBFS de pico   0               0
   la app (DOOM, ...)     su bloque, prestado           lo que traiga     0 hasta el 2
 2 la VOZ del fondo       el orquestador (voces.rs)     el volumen de la  0: lee el banco el de su
                                                        pastilla al       y suma en el    rampa (5 ms)
                                                        cuadrado: 70 ->   acumulador
                                                        -6,2 dB
 3 el AGACHE              agacha.rs                     -15 bajo un       0 (en sitio)    30 ms de
                                                        aviso, -8 con la                  ataque
                                                        app
 4 la MEZCLA              un acumulador de 32 bits      suma sin recorte  1 LECTURA de    0
                          (SUMA, 512 muestras)                            la app
 5 el ESPACIO (3D)        espacio.rs, si esta puesto    trims calibrados  0 (en sitio)    <= 0,66 ms
                                                                                          (el ITD)
 6 el OIDO                oido.rs, si esta puesto       +-12 dB por banda 0 (en sitio)    0
 7 la GANANCIA DIGITAL    maestro: lo que el fader      0 .. +52 dB       0 (en sitio)    rampa
                          pide POR ENCIMA del aparato
 8 el LIMITE              sin ataque, 250 ms de relajo  techo 0 dBFS      1 ESCRITURA,    0
                                                                          a la ranura
                                                                          del xHC
 9 el MEDIDOR             pico, fuerza, ventanas        --                0               --
10 el TUBO ISOCRONO       el xHC lee la ranura          --                0 (DMA)         <= 8 ms
                                                                                          (8 tramas
                                                                                          en vuelo)
11 el APARATO: el FU      SET_CUR por canal (S4j)       -45 .. 0 dB (el   --              --
   principal                                            de la casa)
12 OTRAS unidades del     detras del mezclador del      las de fabrica    --              --
   aparato                retorno del micro             -> 0 dB (S4k)
13 el DAC y el            el aparato                    su potencia, que  --              --
   amplificador                                         nadie la cambia
```

**Por que +52 no daba mas fuerte.** La cuenta de las etapas 1-8 con el
fader a +52: la musica sale a -26 dBFS, la voz le quita 6, el aparato se pone
a su tope (0 dB) y los +52 restantes son digitales: el limite la deja en
**0 dBFS** -- el panel lo mostraba (`sonido -0`, rojo). O sea: por el cable sale
lo MAXIMO que existe. Lo que se perdia estaba DENTRO del aparato, y era una
de dos (o las dos):

* sus canales izquierdo y derecho en el volumen de fabrica aunque el maestro
  estuviera a tope -- arreglado en S4j: cada volumen va a todos los canales;
* una SEGUNDA unidad de volumen en el camino: el PCM entra por la del fader,
  pasa por el mezclador que mete la voz del microfono en el oido (el retorno)
  y sale por OTRA, que nadie tocaba. Ahora `bmo-uaudio` da todas las del
  camino de reproduccion (`otras_de_reproduccion`, 2 pruebas nuevas, 36 en
  verde) y el kernel las pone a 0 dB (ganancia unidad, dentro de su rango) y
  sin mute al reclamar el aparato; `oido` y el `save` dicen cuantas abrio.

**Y la FUENTE, que era la que mas robaba: ESCUCHAR** (`bmo-fondo::escuchar`).
La musica de fondo esta a -26 dBFS a proposito; una pieza ELEGIDA no es
fondo. La ONDA de HERMES (y `fondo escuchar <pieza>`) la compone con la
mezcla de CANCION y NORMALIZA su sonoridad a **-14 dBFS** de fuerza, la de
los servicios de musica: la ganancia se BUSCA en pasadas que solo leen
(el limite se come parte de lo que se sube) y la ultima escribe; el limite
es de masterizar (50 ms de relajo, no los 250 de seguridad del maestro, que
bombearia), y se calienta con la cola del bucle para que la costura no
salte. Medido en las trece: de -14,1 a -14,4 dBFS, subiendo de 4 a 11 dB
(prueba `escuchar_normaliza_sin_costura`). O sea **+12 dB sobre el fondo
antes de tocar el fader**, sin aplastar.

[!] **Lo que no se puede, dicho:** por encima del tope del aparato, el fader
ya no sube la PUNTA -- una onda no sale de 0 dBFS --, sube lo flojo hacia
ella: es compresion. Con la musica de fondo a -32 dBFS (fuente + voz), el
aparato da todo lo suyo con el fader hacia **+26 a +30**; de ahi a +52 lo
que se gana es densidad, y el limite lo cuenta (`limite` en el `save`). Y el
DAC y el amplificador del auricular tienen su potencia: si con todo abierto
sigue flojo, es el aparato, y el `save` lo demuestra con numeros.

**ZERO COPY: donde esta, y por que no se puede quitar la ultima.**

* El banco del fondo y el de las voces se leen EN SU SITIO (etapa 2): cero
  copias por trama, desde el primer dia.
* En REPOSO (fader a 0 dB, sin mudo, sin voces, sin 3D ni oido) el xHC lee
  el bloque de la APP directamente: copia cero de verdad, de la app al
  cable (`maestro.rs`, "en reposo").
* Con cualquier proceso -- ganancia, mezcla, 3D, oido -- hace falta UNA
  pasada que lea y escriba: no hay forma de multiplicar una muestra sin
  escribir el resultado. Esa pasada escribe DIRECTAMENTE en la ranura que el
  xHC lee por DMA (etapa 8): no hay una copia mas despues. El acumulador de
  32 bits de en medio son 2 KiB que viven en la cache L1 (192 bytes de
  cable por milisegundo), y es lo que deja sumar sin recortar.
* El bloque de la app NO se escribe en sitio, a proposito: el kernel no
  escribe en la memoria de otro (`maestro.rs`, "Por que se COPIA").

| que | afirma | como se cae |
|---|---|---|
| arrancar con el auricular, `oido` | dice que traia cada lado y cuantas unidades de mas abrio | sin linea: el aparato no declara volumen por canal |
| fader a +26 con `fondo` | suena claramente mas fuerte que antes del arreglo | igual que antes: mirar CABINA (`uaudio`, "otra unidad del camino") |
| `save`, seccion de audio | `otras unidades` >= 0 y `limite` dice cuanto aplasta | -- |

## [ ] S4l -- EL EMPUJE: "que suene al 200 %" (2026-10-03)

El propietario, con los dos lados ya sonando: *"romper el audio, que
funcione mejor; amplificar, aumentar mas el volumen, como 200 %"*.

**Lo que pasaba por encima del tope.** Con el fader en la parte digital, el
limite del maestro hace bien su trabajo -- de seguridad --: cuando llega un
golpe, baja TODA la onda, y la suelta en 250 ms. Lo que se oye es el golpe a
0 dBFS y el resto hundido debajo. Mas fader = mas hundido; no mas fuerte.

**El empuje** (`bmo-amplificador::maestro::empujar`): una curva SUAVE, muestra
a muestra, ANTES del limite. Por debajo de la rodilla (60 % del techo,
-4,4 dBFS) es un cable; por encima, `K + R*d/(d+R)`, que sube con pendiente
<= 1, sin esquinas, y nunca llega al techo. Asi las puntas se DOBLAN en vez
de bajar la onda entera, y el limite casi no tiene que actuar: mas fuerza
media (RMS) con el mismo techo, a cambio de color en los golpes -- lo que hace
un amplificador apretado. Solo actua con ganancia por encima de 0 dB, y viene
APAGADO: sin el, el maestro es exactamente el de antes.

```text
   oido empuje si|no     lo enciende o lo apaga
   oido fuerte           empuje si + graves +3
   oido                  dice si esta puesto; el save: `oido empuje`
```

**Medido (pruebas de `bmo-amplificador`, 91 en verde):** una nota floja y un
golpe cada 100 ms a +18 dB -- con empuje, **mas de 3 dB mas de RMS** y el
pozo del limite por encima de -1 dB, contra mas de -6 dB sin el
(`el_empuje_suena_mas_fuerte_y_el_limite_casi_no_baja`). La curva: impar,
monotona, pendiente <= 1 y nunca en el techo
(`la_curva_del_empuje_es_suave_impar_y_no_toca_el_techo`); a 0 dB o por
debajo, el empuje no cambia ni un bit (`a_cero_db_el_empuje_no_toca_nada`).

**Como se sabe en el Ryzen:** la ONDA de HERMES sonando, fader a +30,
`oido empuje no` y `oido empuje si`: con el empuje suena claramente mas
fuerte y el `limite` del `save` baja de lo que sujetaba a casi nada.
[!] NO probado en el Ryzen todavia.

## [ ] S5 -- PANORAMA Y DISTANCIA: el sonido tiene un SITIO (2D)

Una fuente mono con una posicion (angulo y distancia) en dos canales:

* **ley de potencia constante**: `izq = cos(a)`, `der = sin(a)` con `a` de 0 a
  90 grados. En el centro cada lado recibe 0,707 (-3 dB) y la POTENCIA total no
  cambia al mover la fuente. La ley ingenua (`izq = 1-x`, `der = x`) hunde el
  centro 3 dB, y es el error clasico;
* **distancia**: atenuacion 1/d con un minimo, para que una fuente lejana no se
  vuelva infinita al acercarse a cero;
* una tabla de 64 posiciones en coma fija: sin trigonometria en tiempo real.

Esto es lo que DOOM necesita de verdad: su `I_UpdateSoundParams` da exactamente
`vol` y `sep` (separacion 0..255), que es esta casilla con otro nombre.

**Tam: M.**

## [ ] S6 -- LOS REPARTOS GRANDES: 5.1 y 7.1 de verdad

Cuando el aparato ofrezca un alt de 6 u 8 canales (S0 lo habra dicho), la cadena
tiene que saber:

* **subir** (una fuente estereo a 5.1): L y R a sus sitios, el resto en
  silencio, o con derivacion de centro si se pide;
* **bajar** (una fuente 5.1 a un aparato estereo), con los coeficientes que usa
  todo el mundo y que aqui se escriben en vez de suponerse:

| de | a L | a R |
|---|---|---|
| L | 1,000 | -- |
| R | -- | 1,000 |
| C | 0,707 | 0,707 |
| LFE | 0,707 | 0,707 (o fuera) |
| LS | 0,707 | -- |
| RS | -- | 0,707 |

* y **decirlo**: `mezclado de 5.1 a estereo` en el informe.

**Tam: M.** Depende de S0 y del aparato: si ningun aparato de esta casa ofrece
multicanal, esta casilla se escribe y **no se ejecuta**, y se dice asi.

## [ ] S7 -- EL 3D DE VERDAD: dos oidos, no ocho altavoces

Para un audifono --dos transductores-- el 5.1 y el 7.1 son **virtuales**: no hay
seis altavoces, hay dos oidos y un cerebro al que se le puede dar las pistas que
usa para situar un sonido. Tres, por orden de coste:

1. **diferencia de tiempo entre oidos (ITD)**: un retardo de hasta ~0,7 ms entre
   izquierda y derecha. Es un bufer circular y una resta: **barato y ya coloca**
   la fuente a los lados;
2. **diferencia de intensidad (ILD)**: lo de S5, pero dependiente de la
   frecuencia (la cabeza tapa los agudos mas que los graves);
3. **HRTF**: convolucionar con la respuesta medida de una cabeza. Es lo que hace
   que un sonido se oiga ARRIBA o DETRAS. Pide una tabla de respuestas y una
   convolucion por fuente: **caro, y es el unico sitio de este plan donde hace
   falta algo parecido a una FFT**.

★ 1 y 2 entran en S7. **La HRTF (3) se apunta y NO se promete**: es un proyecto
propio, y ademas necesita datos medidos que hay que conseguir con una licencia
que valga para Apache-2.0. Se dice ahora para no prometerlo luego.

**Tam: L** (sin HRTF). **XL** con ella, y entonces no es esta casilla.

### [~] EN CODIGO el 03-10: el MODO 3D, global, en el maestro

El propietario: *"se puede aplicar global si ponemos un Modo 3D, no? pero
PRO, EPICO [...] y 4D?"*. Si: las pistas 1 y 2 aplicadas por el MAESTRO a todo
lo que suena, tratando el estereo como dos ALTAVOCES VIRTUALES. Es
`bmo-amplificador::espacio`, delante del OIDO (primero se situa la escena,
despues se ajusta a los oidos de quien escucha), y en el kernel en un `static`
(las lineas de la sala son ~4 KiB y la etapa se crea en la pila del bus).

| modo | que es |
|---|---|
| `cerca` | dos altavoces virtuales a +-30 grados: ITD de Woodworth (hasta 0,66 ms) y la sombra de la cabeza en el oido lejano. El sonido sale de dentro de la cabeza y se pone delante |
| `sala` | y ocho reflejos tempranos (5 a 20 ms) en parejas espejo: el sonido se oye FUERA, a una distancia, sin empujar la escena a un lado |
| `amplio` | los altavoces a +-60 grados, con sala |
| `orbita` | **el "4D"**: la escena entera gira alrededor de la cabeza, una vuelta cada N s (2 a 60) |

* **"4D" no es una dimension del sonido**, y se dice: lo que se vende como
  audio 4D u 8D es 3D que se MUEVE. Eso es `orbita`.
* Lo de DETRAS es una pista pobre (mas oscuro): sin HRTF el cerebro a veces lo
  pone delante. Arriba y abajo, nada. Sigue siendo la 3 de arriba.
* **El mismo volumen** en los cinco modos con una musica centrada (prueba
  `cada_modo_suena_igual_de_fuerte`, dentro de 0,5 dB): comparar modos no es
  comparar volumenes.
* **Sin clics**: fundido de 20 ms de seco a 3D, la sala por su rampa, y los
  altavoces GIRAN hasta su sitio en vez de saltar; retardo fraccionario.
* Apagado es un cable; tras el sonido el silencio es cero (el paso bajo
  redondea al mas cercano: a secas se quedaba pegado en -1).
* 7 pruebas. Y `BMO_FONDO_WAV=<carpeta> cargo test` en `bmo-fondo` escribe
  `demo_3d_modos.wav`: la misma musica por los cinco modos, para oirlo con
  audifonos.

El mando: `AUDIO_MANDO_3D` (9) y `AUDIO_MANDO_3D_VUELTA` (10) en la puerta del
maestro, solo el escritorio; `INFO_AUDIO_ESPACIO` (0xCA). En Ejecutar, `3d`
(`3d sala`, `3d orbita 8`...), y en el `save` sus filas.

### [~] Y POR VOZ (03-10, por la tarde): cada sonido de DOOM en su sitio

`Voces::situar(canal, vol, angulo)`: con angulo, cada oido oye la voz con su
retardo y su sombra (los mismos caminos que el modo global,
`espacio::caminos`). El retardo no pide lineas por voz: el oido lejano LEE EL
BANCO un poco antes --el mismo sonido, mas tarde--, y antes del principio de
un sonido oye silencio, como en el mundo. Volumen, retardo y sombra van por
rampa: un monstruo que cruza por delante no chasquea (prueba
`un_monstruo_que_cruza_no_chasquea`; la primera version daba un escalon de
920 al cambiar de oido lejano, y por eso la sombra tambien va por rampa).

* El contrato: `AUDIO_OP_VOZ` ajustar con el bit 48 (`VOZ_AJUSTAR_ANGULO`)
  lleva volumen y angulo; una app de antes no lo pone y sigue igual. El fondo:
  `AUDIO_FONDO_SITUAR` (7).
* DOOM no da el angulo: da `sep = 128 - 96 sen(angulo)`, y `bmo_sonido.c` lo
  DESHACE con una tabla de arcoseno. Delante o detras no se puede deshacer (el
  seno es el mismo) y se toma delante: DOOM tampoco lo distinguia. Compila
  con `bmo-c` contra las fuentes fijadas (787.851 bytes).
* Con DOOM, `3d apagado` deja el 3D por voz puro; `3d sala` le pone la sala
  encima (los altavoces virtuales sobre una mezcla ya binaural: se oye, pero
  es mas "habitacion" que "precision").

| que | afirma | como se cae |
|---|---|---|
| un imp a la izquierda, con `3d apagado` | se oye A LA IZQUIERDA y un poco despues por la derecha, no solo mas fuerte | igual que antes: DOOM no manda el bit 48 (mirar `voces` en el `save`) |
| girar delante de un monstruo | el sonido cruza sin chasquidos | chasquido al pasar por el centro: el retardo o la sombra saltan |

## [ ] S4g -- LA VOZ DE BMO-X: los sonidos del sistema dicen que, donde y cuanto (2026-10-03)

El propietario: *"sonidos 3D propios [...] para tener como sonidos tipicos de
Windows: error, avisos y signos, y sonidos de compilador que compila TODO,
porque el sonido es IMPORTANTE para que ayude, y que sea honesto"*.

Las cuatro reglas (la maqueta las muestra en la ONDA, "La voz de BMO-X"):

1. **Que, donde, cuanto.** La forma dice que paso; el SITIO, que pieza lo
   dice; la cuenta, cuantos.
2. **Nunca miente.** "Hecho" suena solo si salio. Los errores se cuentan: un
   golpe por error, hasta cinco.
3. **Lo urgente, cerca y seco.** Lo bueno se abre. Lo que llega, viene de
   fuera.
4. **Corto y sin repetir**: menos de un segundo.

| aviso | forma | sitio |
|---|---|---|
| error | dos notas cuadradas graves que caen | delante (0) |
| advertencia | una nota que cae un poco | delante a la derecha (+30) |
| mensaje, conecta, zumbido | lo de HERMES | a la izquierda (-50), donde vive F3 |
| pregunta | sube, como una pregunta | delante |
| hecho | un acorde que se abre | delante |
| llega | sube, de fuera | detras a la izquierda (-140) |
| se va | baja | detras (+160) |
| el compilador | un tic por crate que CRUZA de -60 a +60 grados (se oye cuanto falta), un clic a la derecha por aviso, y al final la verdad | -- |

En codigo: los seis nuevos en `bmo-fondo::avisos` (12 en total, todos a -6
dBFS de pico) con `avisos::angulo`, y el escritorio los toca EN SU SITIO por
el 3D por voz del atril del fondo; `globo::avisar` usa ya error y hecho.
`fondo aviso error` (y los demas) para oirlos. `BMO_FONDO_WAV=<carpeta> cargo
test` en `bmo-fondo` escribe `demo_voz_3d.wav` con el mismo mezclador del
kernel.

**Y se MUEVEN (03-10, de noche)**: `avisos::ruta` dice por donde va cada
aviso ("llega" de -150 a -15 grados en 380 ms, "se va" de 10 a 165 en 480),
y `desktop::musica::mover`, en cada vuelta del bucle, le manda su sitio cada
16 ms mientras suena (y mientras se mueva, el bucle no se duerme). El
orquestador lleva retardo, volumen y sombra por sus rampas, asi que el camino
sale continuo: prueba `llega_se_mueve_sin_saltos`, con el mezclador del
kernel (pasa por la izquierda, acaba delante, ningun salto).

[!] Lo que falta: el compilador: el que compila TODO hoy corre
en Windows (`build.ps1`); en BMO-X sonara cuando compile dentro (H14 de
PLAN_HERMES, MAQUETA en BMO-X).

| que | afirma | como se cae |
|---|---|---|
| `3d sala` con el fondo sonando | la musica sale de la cabeza y se pone delante, igual de fuerte | mas fuerte o mas floja: el ajuste del modo |
| `3d orbita 8` | todo gira alrededor de la cabeza, una vuelta cada 8 s, sin clics | chasquidos: el retardo salta |
| cambiar de modo con DOOM | sin golpes | un golpe: el fundido o los angulos |
| `3d apagado` | el estereo de siempre | -- |

## [ ] S8 -- LO QUE PIDE EL APARATO, NO LA ONDA

Del ADN de [`PLAN_AUDIO.md`](PLAN_AUDIO.md) 6.2, que sigue pendiente y no es de
la onda sino del cable: UAC2 (un 7.1 High Speed casi seguro lo es), el endpoint
de **feedback** (el aparato dice cuantas muestras quiere por trama; sin eso, su
reloj y el nuestro se separan), y una tabla de rarezas por `vid:pid`.

**Tam: XL**, y es el que decide si un aparato distinto del de esta casa entra.

---

# 3.5 -- LO QUE EL METAL CONTESTO EL 22-09 A LAS 08:23

**SONO.** `run inti/musica.ibx` con el audifono puesto: `tubo USB`,
`encoladas 15920`, `tarde 0`. El camino entero funciona.

Y la tabla de S0, escrita horas antes justo para esta pregunta, contesta la
grande:

```text
      alt  canales   bits  B/ms  max pkt  sinc    frecuencias
        1        2     16   192      192  --     44100/48000  <- ELEGIDO
```

**UN solo formato, DOS canales.** El audifono "7.1" no lleva multicanal por el
cable: su 7.1 es virtual. Eso mueve S6 y S7 de sitio:

* **S6 (5.1 y 7.1 de verdad) se escribe y no se ejecuta** en esta casa, y asi
  queda dicho. Sirve el dia que haya un aparato que los ofrezca; la tabla de
  S0 lo dira sin que nadie lo suponga.
* **S7 (el 3D en dos oidos) deja de ser un lujo y pasa a ser EL camino**: es
  la unica forma de que este aparato situe un sonido, porque es la unica que
  tiene. Sube de sitio en el orden.

Y declara **44.100 y 48.000 en el mismo alt**: con S2 hecho, un CD se toca a su
frecuencia sin remuestrear.

## Lo que NO sono fuerte, y por que: DOS perillas, ninguna puesta

El propietario: *"dijiste el audio mas fuerte pero no sentia mas fuerte el
sonido"*. Tenia razon, y no era el amplificador:

| perilla | donde | estaba en |
|---|---|---|
| el volumen **del aparato** | Feature Unit, -45,0 a 0,0 dB | **sin poner** -- el escritorio no tenia orden |
| el nivel **de la fuente** | `musica.inti` sintetiza a +-3.000 de 32.767 | **-20,7 dBFS**, a proposito |
| la **ganancia** por software | `bmo-amplificador` | escrita, sin enchufar (P2/P3) |

Lo primero se arregla el mismo dia: **`audio volumen N`** (0..100 sobre la
escala del aparato). Es gratis y llega hasta el techo del aparato; la ganancia
por software empieza donde eso se acaba. Subir la segunda sin haber subido la
primera es amplificar algo que el aparato todavia podia dar limpio.

# 4. EL ORDEN, Y POR QUE ES ESE

```text
   S0  el censo dice la verdad        <- SIN ESTO, TODO LO DEMAS ES A CIEGAS
   S1  la cadena, una fuente          <- y el metal dice si se OYE
   S4  el amplificador                <- lo que el propietario necesita para OIRLO
   S3  el mezclador                   <- y con el, DOOM (PLAN_DOOM 5.3)
   S5  panorama y distancia           <- y DOOM tiene sonido posicional
   S2  la fraccion                    <- y entra el CD y el MP3
   S6  5.1 y 7.1, si el aparato       <- depende de S0
   S7  ITD/ILD: el 3D en dos oidos
   S8  UAC2, feedback, rarezas
```

★ **S4 va antes que S3 a proposito.** Mezclar sin ganancia ni limitador es
sumar y rezar; y la razon por la que el propietario quiere esto --oirlo mas alto--
no llega con el mezclador, llega con el amplificador. Un sistema que suena flojo
para su propietario no esta terminado, por muchos canales que tenga.

---

# 5. LO QUE ESTE PLAN **NO** PROMETE

* **No es un DAW.** No hay edicion, ni pistas, ni efectos (reverberacion,
  ecualizador, compresor multibanda). *(Corregido el 03-10: hay TRES mandos
  de tono fijos, los de un amplificador de alta fidelidad, en el OIDO (S4f),
  porque lo pidio el propietario y el sonido de esta casa es personal. Sigue
  sin haber efectos que se enchufen ni se encadenen.)* El "como DaVinci" de este plan es la
  GANANCIA con medidor y limitador, que es una pieza concreta; el resto de
  DaVinci es otro programa.
* **No hay decodificadores aqui.** MP3, AAC, Opus y compania son otro trabajo
  ([`PLAN_AUDIO.md`](PLAN_AUDIO.md) A5). Esta cadena empieza en PCM.
* **No hay HRTF prometida** (S7.3), por las dos razones dichas: coste y datos.
* **No hay entrada.** Microfono, grabacion y captura no estan en este plan.
  El dia que entren, la tabla de 1.4 vale igual, pero la cadena va al reves y
  eso se escribe entonces.
* **Casi nada de esto es del kernel.** El kernel entrega el tubo, el bloque
  prestado, el volumen del aparato **y, desde el 22-09, el MAESTRO**: la ultima
  etapa (ganancia con rampa, limite y medidor), por decision del propietario y
  con su coste escrito en S4c. Mezclar varias fuentes, el arbol de pistas y
  situar en el espacio siguen siendo Ring 3 -- y por eso un tercero puede
  escribir el suyo sin pedir permiso a nadie ([`PLAN_AUDIO.md`](PLAN_AUDIO.md) 5).
  *(Aqui ponia "Nada de esto es del kernel", y era verdad hasta S4c.)*

---

# 6. LA GUIA EN UNA PAGINA (para un tercero)

Si estas escribiendo algo que suene en BMO-X, esto es todo lo que necesitas
saber, y todo lo demas de este documento es el por que:

```text
   1. Reclama el sonido            OP_AUDIO_CLAIM -- uno a la vez, se recupera solo
   2. Pregunta que hay             AUDIO_OP_DEVICES, y la tabla de S0
   3. NO supongas la frecuencia    te la dice el aparato; 48.000 hoy, otra el dia que cambies de aparato
   4. NO supongas los canales      192 B/ms a 48 kHz = estereo 16 bits: haz la cuenta
   5. Manda PCM al bloque PRESTADO sin copias, sin una puerta por muestra
   6. Si amplificas, MIDE          pico, y cuantas muestras doblego el limitador
   7. Si mezclas o bajas canales   DILO en tu informe: el que escucha tiene derecho
```

Y la cuenta que resuelve el 90 % de las dudas:

```text
   bytes por milisegundo = (Hz / 1000) x canales x bytes por muestra
       48.000 Hz, estereo, 16 bits  ->  48 x 2 x 2 = 192 B/ms   (cabe: max pkt 192)
       48.000 Hz, 5.1,     16 bits  ->  48 x 6 x 2 = 576 B/ms   (NO cabe en ese alt)
```
