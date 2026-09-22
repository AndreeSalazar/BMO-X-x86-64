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

## [ ] S0 -- EL CENSO DICE LA VERDAD, y la tabla la llena el aparato

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

**Tam: M.** Es parseo y presentacion, sin metal nuevo. **Y es lo primero**,
porque sin esta tabla las demas casillas se hacen a ciegas.

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

## [ ] S4 -- EL AMPLIFICADOR, con medidor y limitador (*"como DaVinci"*)

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
* **Nada de esto es del kernel.** El kernel entrega el tubo, el bloque prestado
  y el volumen del aparato. Mezclar, amplificar y situar es Ring 3 -- y por eso
  un tercero puede escribir el suyo sin pedir permiso a nadie
  ([`PLAN_AUDIO.md`](PLAN_AUDIO.md) 5).

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
