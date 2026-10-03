# La receta del NYA (y del resto del gato)

> Pedido del propietario, 2026-10-03: *"el nya, que se pueda anotar como
> hiciste, para guardar luego"*. Esto es eso: como sale cada sonido del gato,
> con los numeros exactos, para poder rehacerlo, cambiarlo o llevarlo a otro
> sitio sin tener que leer el codigo.
>
> Va APARTE de la musica de 8 bits ("El emisor salta" y las demas piezas de
> ambiente): esa tiene su propia receta abajo, en su seccion, y no comparte
> nada con el gato salvo el banco y el reloj.

Donde vive de verdad:

| que | donde |
|---|---|
| las curvas, el maullido, el ronroneo, el bufido, el cencerro, el 808 | `platform/shared/bmo-fondo/src/neko.rs` |
| las tablas (resonancias, el polo del ronroneo) | `platform/shared/bmo-fondo/src/neko_tablas.rs` |
| que aviso lleva que sonido, y a que nivel | `platform/shared/bmo-fondo/src/avisos.rs` (`notas_neko`, `nivel_neko`) |
| el NEKO PHONK (patron, bombeo) | `platform/shared/bmo-fondo/src/compositor.rs` (`PatronPhonk`, `ganancia_bombeo`) |
| la version de la maqueta, en Web Audio | `docs/arte/maqueta_hermes.html` (la parte `NEKO PHONK`) |

Todo es a **48 kHz**, sin coma flotante en Rust (Q16 / Q28 / Q30, dB en
1/256), y con coma flotante en la maqueta. Los dos suenan igual: la maqueta es
el boceto, el Rust lo que va al metal.

---

## 1. El NYA: una sierra por dos resonancias que se mueven

Un maullido es una **vocal**: la boca del gato va de *i* a *a* a *u*. Una vocal
son dos resonancias (los **formantes**, F1 y F2) sobre una fuente rica en
armonicos. Eso es todo el truco:

```text
   sierra (tono f0) --+--> paso banda F1 (Q = 5) --> x3 --+
                      |                                    +--> envolvente --> salida
                      +--> paso banda F2 (Q = 5) --> x2 --+
```

- **Fuente**: una SIERRA (PolyBLEP en Rust, `sawtooth` en la maqueta). Tiene
  todos los armonicos, que es lo que las resonancias necesitan para "esculpir".
- **Resonancias**: dos paso banda de ganancia 0 dB en el pico, **Q = 5**.
  Estrechas: se comen casi toda la energia, por eso se devuelve con ganancia
  (**x3** la primera, **x2** la segunda en Rust; **1,4** y **0,9** en la
  maqueta, que mide distinto).
- **El movimiento**: f0, F1 y F2 siguen una CURVA de cuatro puntos, en linea
  recta entre ellos. En Rust se recalcula cada **16 muestras** (0,33 ms): de
  sobra para el oido y barato.

### Las curvas: (ms, f0, F1, F2), en Hz

| maullido | 0 ms | punto 2 | punto 3 | fin |
|---|---|---|---|---|
| **nya** (corto, de llamar) | 0: 620, 300, 2400 | 60: 760, 700, 2200 | 150: 820, 950, 1500 | **280**: 600, 650, 1100 |
| **miau** (entero) | 0: 520, 350, 2500 | 150: 700, 800, 1700 | 350: 640, 900, 1300 | **560**: 460, 450, 800 |
| **mrrp** (el trino, sube) | 0: 380, 400, 1200 | 80: 470, 550, 1300 | 160: 600, 650, 1600 | **230**: 720, 700, 1900 |
| **pregunta** (sube al final) | 0: 560, 500, 2000 | 150: 640, 700, 1900 | 260: 760, 750, 2000 | **360**: 920, 800, 2200 |
| **triste** (cae) | 0: 820, 850, 1700 | 200: 720, 800, 1450 | 420: 560, 600, 1100 | **620**: 420, 450, 850 |

Como leer el **nya**: empieza con F1 bajo y F2 alto (*i*: "n-i"), F1 sube y F2
baja hasta la *a* (950 / 1500), y cierra hacia la *u* (650 / 1100). El tono
sube de 620 a 820 y cae a 600: la "sonrisa" del nya. El ultimo punto dice
tambien lo que dura (280 ms).

### La envolvente

- **Ataque de 35 ms**, desde 30 dB por debajo (no desde cero: un maullido
  empieza "ya sonando", no con un golpe).
- Despues **cae hasta -50 dB** al final de la curva.
- En la maqueta: `vol * 0,03` -> `vol` en 35 ms (exponencial) -> `0,0003` al
  final.

### Subirlo o bajarlo (`semitonos`)

Solo se mueve el **tono** (f0); las resonancias se quedan donde estan. Asi un
nya mas agudo sigue siendo la misma vocal, el mismo gato: no se vuelve una
ardilla. En Rust la razon sale de la tabla de notas
(`inc_midi(60 + s) / inc_midi(60)`, en Q16), de -24 a +24 semitonos.

### La tabla de resonancias (`neko_tablas.rs`)

**128 pasos logaritmicos de 120 Hz a 8 kHz** (un 3,4 % cada paso), cada uno un
paso banda del recetario RBJ con Q = 5 y 0 dB en el pico, en Q28:
`b0, b1 (= 0), b2 (= -b0), a1, a2`. Para una frecuencia se toma el paso **mas
cercano**. La prueba `la_tabla_de_resonancias_cuadra` la regenera con la
formula y la compara: si se cambia Q o el rango, se regenera y la prueba dice
si cuadra.

Formula (la del recetario, para regenerar):

```text
w0 = 2 pi f / 48000      alfa = sin(w0) / (2 Q)
b0 =  alfa / (1 + alfa)  b1 = 0   b2 = -b0
a1 = -2 cos(w0) / (1 + alfa)      a2 = (1 - alfa) / (1 + alfa)
```

---

## 2. El RONRONEO: ruido grave que late a 26 Hz

```text
ruido blanco --> paso bajo 250 Hz --> paso bajo 250 Hz --> x10 --> x latido --> envolvente
```

- **Dos** pasos bajos de un polo seguidos (12 dB por octava; el polo, en Q16,
  es `RONRONEO_POLO = 2110`). Con uno solo el ruido dejaba demasiados agudos y
  "ronroneaba" a 5 kHz: se oia como un siseo.
- **x10** de ganancia: dos polos tan bajos se llevan casi toda la energia.
- **El latido**: la amplitud va del **30 % al 100 %**, 26 veces por segundo
  (un seno). Eso es lo que lo hace ronroneo y no viento.
- **Envolvente**: entra en la mitad de su largo desde -24 dB, y se va a -40 dB
  (una respiracion).
- En la maqueta: dos `lowpass` a 250 Hz, un LFO de 26 Hz con ganancia 0,35
  sobre una base de 0,65.

## 3. El BUFIDO: aire a 4 kHz

```text
ruido blanco --> paso banda 4,2 kHz --> x2 --+--> envolvente
ruido blanco ------------------------> /4 --+
```

- Paso banda en **4.200 Hz** (de la misma tabla de resonancias), mas una
  cuarta parte del ruido sin filtrar (el soplido).
- Ataque de **20 ms** desde -20 dB, cae a -50 dB.
- En la maqueta: `bandpass` 4200 Hz, Q = 2.

## 4. El CENCERRO del 808 (del phonk)

- **Dos cuadradas** en razon **1 : 1,48** (la del 808 original: dos notas que
  no casan, y por eso suena a metal).
- Golpe que cae **9 dB en 18 ms** y una cola corta que acaba a -60 dB a los
  **380 ms**.
- Esta en el **bus que bombea** (ver el NEKO PHONK).

## 5. El 808 que grune

- Un **seno** que arranca **una octava arriba** y cae a su nota a la mitad
  cada **45 ms** (el `OCHO_CAE_Q30` de `neko.rs`).
- Empujado **x3** a una saturacion suave: `1,5 u - 0,5 u^3` (y +-1 fuera):
  grune sin el chasquido de un recorte a pelo.
- Cae 2 dB en 60 ms y despues sostiene hasta su largo.

## 6. El NEKO PHONK: el BOMBEO

Con cada 808, todo lo melodico (cencerros, maullidos, acordes) baja al **30 %**
y vuelve al **100 %** en linea recta en **125 ms** (6.000 muestras):
`ganancia_bombeo(t) = 19660 + 45876 * min(t, 6000) / 6000` en Q16. Es el
"respira con el bombo" del phonk. El bombo, la caja y el 808 NO bombean.

La escala de las piezas neko: **frigia** (`0 1 3 5 7 8 10`), la de la
segunda menor, la tension. Raices por compas: i, II bemol, i, VII bemol.

## 7. Que aviso lleva que (el tema NEKO, `fondo tema neko`)

| aviso | sonido |
|---|---|
| mensaje | un **nya** |
| zumbido | tres nya seguidos (cada 110 ms), cada uno 2 semitonos mas alto |
| conecta | un **mrrp** |
| advertencia | un mrrp 7 semitonos mas grave |
| pregunta | el maullido **pregunta** |
| hecho | un nya +5 y tres cencerros que suben (74, 77, 81) |
| llega | un **ronroneo** de 420 ms y un nya +7 al llegar (a los 330 ms) |
| se va | el **triste** |
| error | un **bufido** de 320 ms y un 808 grave (nota 33) debajo |
| juez | un bufido de 350 ms y un cencerro que cae (62) |
| captura | el plato y un nya +5 |
| arranque | un 808 (38), el motivo del cencerro en frigia (74 75 77 86) y un nya |

Niveles: cada aviso se calibro a **-6,0 dBFS de pico** (`nivel_neko`). Si se
cambia un sonido, hay que volver a medirlo: la prueba de niveles lo dice.

## 8. Como jugar con el

- **Otra vocal**: cambia F1/F2 de la curva. Referencias: *i* 300/2400, *e*
  500/1900, *a* 900/1400, *o* 550/900, *u* 350/800.
- **Mas gato, menos voz**: sube f0 (un gato real esta entre 500 y 1.000 Hz).
- **Mas tierno**: Q mas bajo (4) y el ataque mas largo (50 ms).
- **Mas bruto**: Q mas alto (7-8) y la segunda resonancia mas fuerte.
- **Escucharlo sin el metal**: `BMO_FONDO_WAV=dir cargo test -p bmo-fondo`
  escribe los WAV de prueba (los avisos neko incluidos); en la maqueta,
  `fondo tema neko` en la ONDA.

---

## Y aparte: "El emisor salta" (la de 8 bits)

No es el gato. Es una de las piezas de **ambiente** de `bmo-fondo`, y sale
entera de una **semilla**:

```text
Pieza { nombre: "El emisor salta", semilla: 17, bpm: 128, raiz: 59 (si),
        escala: pentatonica menor (0 3 5 7 10), timbre: CUADRADA, nivel: -102 }
```

- 16 pasos por compas, 8 compases por vuelta; acordes cada 4 compases
  (raiz, quinto grado, cuarto, penultimo), el arpegio calla en el octavo.
- De la semilla (el hash `azar` de la maqueta): el bajo en triangulo cada dos
  pasos (siempre en el 0 y el 4, y un 55 % en los demas), el arpegio en
  **cuadrada** (un 62 % de los pasos, un 30 % de ellos una octava arriba),
  bombo en cada negra (y un 10 % mas), caja en el 4 y el 12, plato en las
  corcheas (y un 25 % mas).
- Lo que la hace "de 8 bits": la cuadrada del arpegio a 128 pulsos y la
  pentatonica, que no tiene notas que choquen.
- En el escritorio suena con la mezcla **FONDO** (bateria muy atras,
  acordes delante) y es la de la **bienvenida** (S4i de `PLAN_EL_SONIDO.md`):
  al llegar suena con su panel de 8 bits, que se minimiza en la pastilla.
