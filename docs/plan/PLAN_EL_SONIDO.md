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

Cada peldano de esa escalera es una casilla de la seccion 3. **Lo que esta
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
[`PLAN_DOOM.md`](PLAN_DOOM.md) 5.3e.

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

### Lo que NO es

No es LA MESA: una sola perilla para todo lo que suena, no una por programa ni
por pista. El arbol sigue siendo de Ring 3 ([`PLAN_LA_MESA.md`](PLAN_LA_MESA.md)).

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
  ecualizador, compresor multibanda). El "como DaVinci" de este plan es la
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
