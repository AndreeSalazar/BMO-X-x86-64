# PLAN AYNI -- que la GPU gaste MENOS, con un cerebro pequeno en la CPU

> El propietario (12-10), despues de leer el plan de nitidez: *"y si mejor
> inspirar eso en la red neuronal y redefinirlo por completo? porque no es mi
> objetivo FPS, ya con FSR de AMD es suficiente; lo que yo quiero es
> simplemente REDUCIR EL COSTO de GPU."*

Este plan **reemplaza** al de nitidez del mismo dia (LLIPIQ, que ya no
existe: la nitidez y el reescalado los pone FSR, que es abierto y basta).

**AYNI** (quechua: la ayuda mutua, el trabajo de uno devuelto por el otro) es
la libreria de VERRANO que hace que la GPU **trabaje menos por fotograma**
con la imagen que el ojo ve igual. La CPU le ahorra trabajo a la GPU; la GPU
le devuelve tiempo, vatios y temperatura.

Tiene dos piezas con nombre propio:

* **UMA** (quechua: la cabeza) -- una red neuronal **diminuta**, en la
  **CPU**, que decide **cuanto trabajo merece cada trozo de la pantalla**.
  No dibuja nada, no mejora pixeles: REPARTE el trabajo. Cuesta cero GPU.
* **PACHA** (quechua: el mundo y el tiempo, la misma palabra) -- lo que UMA
  mira para decidir: el espacio (x, y, z) y el fotograma anterior. Es la
  "4D" del propietario: la CPU, que ve el tiempo, guia a la GPU, que dibuja
  el instante.

---

# 0. LA RESPUESTA CORTA

**Si, y tiene mas sentido que el plan de nitidez.** Lo que la red de DLSS
hace de verdad es AHORRAR: deja dibujar menos pixeles y adivina el resto. Si
FSR ya pone el reescalado, lo que queda por ganar es **decidir donde NO
gastar**. Para eso no hace falta una red grande en la GPU: basta una pequena
en la CPU, porque BMO-X ve cada dibujo.

| | DLSS | FSR | AYNI (BMO-X) |
|---|---|---|---|
| para que | dibujar menos pixeles y reconstruir | lo mismo, sin red | **sombrear menos donde no se nota**, y no dibujar lo que nadie vera |
| la red | grande, en la GPU (tensor cores) | ninguna | **diminuta, en la CPU** (UMA): cero GPU |
| de donde aprende | meses de entrenamiento de NVIDIA | -- | **del propio juego, mientras juegas** (las baldosas maestras, seccion 3) |
| lo que sabe | color, profundidad, vectores | lo mismo | todo eso **y cada dibujo**: sus triangulos, su PSO, si es interfaz |
| la tarjeta | solo NVIDIA RTX | cualquiera | cualquiera con sombreado variable; sin el, AYNI da lo de A0 y A3 |
| junto con FSR | -- | -- | **si**: AYNI decide cuanto se dibuja, FSR reescala |

**Lo que AYNI NO es:** no inventa fotogramas (sube el input lag), no afila
(eso es FSR), y no promete un numero antes de medirlo: cada capa se mide con
el contador de la propia 3060 (seccion 4).

---

# 1. DONDE SE VA EL COSTO DE LA GPU (y por eso las capas)

Un fotograma cuesta, de lo que mas suele pesar a lo que menos:

```text
   1. fotogramas que nadie ve      dibujar 200 por segundo en un monitor de 60:
                                    el 70 % del trabajo se tira
   2. sombrear pixeles             el sombreador de pixeles corre una vez por
                                    pixel: cielo, niebla, pared lisa, sombra
                                    oscura... la misma cuenta cara en todos
   3. pixeles de mas               la resolucion interna (FSR ya lo ataca)
   4. dibujos que no se ven        un lote tapado del todo, o de menos de un
                                    pixel
   5. la GPU a reloj alto sin hacer nada entre fotogramas
```

AYNI tiene una capa por cada una, de la mas segura a la mas atrevida.

---

# 2. LAS CAPAS

```text
   A0 EL PLAZO          no dibujar fotogramas que el monitor no mostrara:
                        uno por VBlank, empezado lo MAS TARDE posible (la
                        entrada se lee justo antes: menos input lag Y menos
                        trabajo). Sin red: solo reloj. Es lo que mas ahorra
   A1 EL MAPA DE UMA    la 3060 tiene sombreado variable POR HARDWARE (abajo):
                        una imagen pequena, una casilla por baldosa de 16x16,
                        dice cuantas veces corre el sombreador ahi: 1 por
                        pixel, 1 por cada 2x2, 1 por cada 4x4... UMA escribe
                        esa imagen cada fotograma
   A2 LO QUE NO CAMBIO  una baldosa que ningun dibujo toco distinto (la
                        interfaz quieta, un menu, una app de escritorio) no se
                        vuelve a sombrear: la tasa "0" (no correr) y el pixel
                        de antes se queda
   A3 LA RESOLUCION     UMA predice cuanto costara el fotograma y elige la
       JUSTA            resolucion interna para acabar JUSTO antes del VBlank;
                        FSR reescala. Ni un pixel de mas cuando sobra tiempo,
                        ni un fotograma tarde cuando falta
   A4 LO QUE NO SE VE   un lote tapado del todo por la profundidad del
                        fotograma anterior, o de menos de un pixel, no se
                        manda. CONSERVADOR: ante la duda, se dibuja
   A5 EL RELOJ          con el fotograma acabado antes de tiempo, pedirle al
                        GSP un estado de rendimiento mas bajo (menos vatios).
                        LEJOS y con decision del propietario: es tocar el
                        reloj de la tarjeta
```

**El sombreado variable de la 3060, leido de la fuente** (NVIDIA
`open-gpu-doc`, `classes/3d/clc797.h`, la clase AMPERE_B que la casa ya usa
en GR0):

```text
   NVC797_SET_SHADING_RATE_INDEX_SURFACE_ADDRESS_A/B (0x12a8 + j*32)
       la imagen de tasas: una casilla por baldosa, en VRAM
   NVC797_SET_VARIABLE_PIXEL_RATE_SHADING_INDEX_TO_RATE_A (0x2a04 + j*16)
       la tabla casilla -> tasa: X1_PER_RASTER_PIXEL, X1_PER_2X1, 1X2, 2X2,
       4X2, 2X4, 4X4 ... y PS_X0_CULL_RASTER_PIXELS (no correr)
   NVC797_SET_VARIABLE_PIXEL_RATE_SHADING_CONTROL (0x2a00 + j*16)
       encendido por viewport
   NVC797_SET_STATISTICS_COUNTER_PS_INVOCATIONS_ENABLE (bit 9)
       el CONTADOR de veces que corrio el sombreador de pixeles: la medida
```

Es decir: lo que el propietario imaginaba ("la CPU guia constantemente y la
GPU dibuja preciso") **ya esta en el silicio**: la GPU obedece un mapa. Lo
que falta es quien lo escriba bien. Ese es UMA.

**Lo que ya existe y se reconoce**: el "content adaptive shading" (NVIDIA,
Wolfenstein: Youngblood, 2019) decide la tasa con reglas fijas sobre el
fotograma anterior. AYNI anade dos cosas que un juego no tiene: **ve cada
dibujo** (sabe donde hay interfaz, bordes de geometria, cuantos triangulos
pasan por cada baldosa) y **aprende** del propio juego.

---

# 2b. LA 4D Y LA 5D, DICHAS BIEN

> El propietario (12-10): *"el 4D no existe, solo los 3D; sin embargo ese
> mismo 4D guia los 1D, 2D y 3D ... y si vamos con el 5D, cambia algo?"*

**La 4D es el tiempo.** No se dibuja nunca, pero ordena todo lo que si se
dibuja. Como una sombra: un objeto 3D proyecta una sombra 2D, y la sombra no
sabe por que tiene esa forma; manda el objeto, que no esta en la pared. Un
juego es un escalon mas arriba: **cada fotograma es un corte 3D** de algo que
es 4D (la escena a lo largo del tiempo). La GPU dibuja cortes; la CPU ve la
serie de cortes.

```text
   1D   un pixel a lo largo del tiempo   si lleva 10 fotogramas igual, no se
        es una linea de valores; un      recalcula (A2); un borde que se
        borde tambien es 1D              desplaza, se sabe hacia donde
   2D   la imagen, sus baldosas          que baldosas tenian detalle y cuales
                                         eran cielo liso (A1)
   3D   los triangulos, la profundidad   que se mueve, que esta quieto, que
                                         quedo tapado (A4)
   4D   la historia: no se ve            PACHA: la que decide todo lo de arriba
```

Funciona porque **el mundo no salta**: en 16 ms casi nada cambia. Y deja de
funcionar donde el mundo SI salta (un corte de camara, una explosion): ahi la
historia no sirve, y por eso estan las maestras, el umbral y `ayni off`.

**La 5D: la intencion del jugador.** Fisicamente nadie ha visto una quinta
dimension (las de la teoria de cuerdas o Kaluza-Klein son hipotesis). Pero
para AYNI hay un eje mas que si sirve, y es barato: **el futuro no es una
linea, se ramifica, y quien elige la rama es el jugador**. La entrada (raton,
mando, teclado) llega ANTES que el fotograma que la muestra, asi que la CPU
sabe hacia donde va el siguiente corte antes de que la GPU lo dibuje:

* **Giro rapido de camara**: la pantalla entera se emborrona al ojo; el
  detalle fino no se ve y se puede sombrear menos.
* **Apuntando quieto**: el ojo esta en el centro; ahi, todo el detalle.
* **Sin tocar nada** (un menu, una pausa): lo que no cambio (A2) manda.

Cambia **poco y concreto**: un numero mas por baldosa para UMA (la velocidad
de la entrada y la distancia al centro). No anade input lag: la entrada ya
estaba leida para el juego.

**La 5D que predice** (el propietario, despues: *"la CPU ya sabe que quiere
el jugador ... cuando apunta con el puntero se crea una posibilidad propia,
neuronal, que acepta o no, y eso ahorra en tiempo real"*). Es la parte
divertida, y se puede, con una frontera clara:

* **BMO-X ve la entrada ANTES que el juego**: el kernel recibe el raton y el
  teclado y luego se los da a la app. Eso no es adivinar: es saber primero.
* **El movimiento corto se predice bien**: la mano tiene inercia. Un puntero
  que frena sobre un sitio va a parar ahi (en interfaces se llama prediccion
  del punto final, *endpoint prediction*, y se estudia desde hace anos).
  Unos pocos fotogramas hacia delante, no segundos.
* **La decision NO se predice perfecta**: el jugador puede cambiar de idea
  en el ultimo milisegundo. Por eso cada prediccion lleva su **confianza**
  (la "posibilidad que acepta o no"): por encima del umbral se usa, por
  debajo no se hace nada.

**La regla de oro de la 5D: predecir para AHORRAR o PREPARAR, nunca para
ACTUAR.**

```text
   si (ahorra o prepara)   menos detalle lejos de donde va el puntero; el
                           fotograma empezado mas tarde si el raton esta
                           quieto; la escena quieta a casi cero; preparar
                           (subir el reloj, tener listo) justo antes del clic
   no (actuar)             mover el puntero, hacer el clic o apuntar por el
                           jugador. Eso cambia el JUEGO, no el coste: es
                           ayuda de punteria, y en un juego en linea
                           (Warframe) es trampa para su antitrampas
```

Si la prediccion falla, el fotograma cuesta **lo de siempre** (sin AYNI): la
5D nunca empeora la imagen, solo ahorra menos. El juego no necesita saber
nada.

**Los papeles, dichos asi**: la CPU es la cabeza (ve el tiempo y la mano, y
decide); la GPU es el cuerpo (los pixeles gordos, en paralelo, sin pensar).
Con una condicion: **la cabeza tiene que ser ligera**. UMA y la 5D juntas
caben en una fraccion de milisegundo; si la cabeza tardara mas que lo que
ahorra, el cuerpo la esperaria y el plan no tendria sentido.

---

# 3. UMA: LA RED, INSPIRADA Y DIMINUTA

```text
   por cada baldosa de 16x16       (1920x1080 = 8160 baldosas)
      entra (PACHA, ~9 numeros)    el contraste del fotograma anterior ahi,
                                   el borde mas fuerte, el salto de
                                   profundidad, cuanto se movio, lo oscuro
                                   (el ojo ve menos en la sombra), cuantos
                                   triangulos la cruzan, si un lote de
                                   INTERFAZ la toca, si cambio algo, y la
                                   5D: la entrada del jugador (seccion 2b)
      la red                       9 -> 16 -> 4: unas 200 multiplicaciones
      sale                         la tasa: 1x1, 2x1/1x2, 2x2, 4x4
```

**El coste de UMA**: 8160 x 200 = 1,6 millones de multiplicaciones por
fotograma. En un nucleo del 5600X, con AVX2, es del orden de una decima de
milisegundo. Cero en la GPU.

**Como aprende sin datos de nadie -- las baldosas maestras.** Cada
fotograma, unas pocas baldosas (8, rotando por la pantalla) se sombrean
SIEMPRE a 1x1 aunque UMA diga otra cosa. La CPU lee esas 8 baldosas (8 KiB) y
calcula, de la propia baldosa completa, como se habria visto a 2x2 o a 4x4
(un pixel repetido por cuadro). La diferencia, pesada como la ve el ojo (el
contraste esconde el error, la sombra tambien), es la **leccion**: UMA se
corrige un paso. Asi aprende de TU juego, en TU monitor, mientras juegas.

* Las maestras cuestan lo que habrian costado sin AYNI: nada extra.
* El peso del error es UN numero que pone el propietario (`ayni suave` /
  `fuerte`): cuanta diferencia acepta a cambio de cuanto ahorro.
* Si UMA se equivoca, el error lo dicen las propias maestras, y el umbral
  sube solo (baldosas de vuelta a 1x1).

**Las reglas que UMA no puede romper** (se aplican DESPUES de la red):

* Un lote de interfaz (texto, HUD) va a 1x1 siempre. PROTON-X lo sabe: es
  otro PSO, dibujado al final.
* Una baldosa con un borde de geometria va como mucho a 2x1/1x2 a lo largo
  del borde, nunca 4x4.
* Sin sombreado variable en la tarjeta, UMA no corre: AYNI da A0 y A3.

---

# 4. COMO SE MIDE EL AHORRO (sin numeros prometidos)

* **El contador de la 3060** (`PS_INVOCATIONS`, por el semaforo de informe):
  veces que corrio el sombreador de pixeles, con AYNI y sin AYNI, la misma
  escena. Es el numero que dice el ahorro, no una estimacion.
* **El tiempo de GPU del fotograma** (FRAPS-X) y, si el GSP los da, los
  vatios.
* **El error de las maestras**, en el DIARIO: el ahorro no vale si el error
  sube.
* En lo publicado de otros, el sombreado variable da de un digito a pocas
  decenas por ciento segun cuanto pese el pixel en el juego. El plazo (A0)
  es lo grande: un juego que dibujaba 3 fotogramas por cada uno mostrado
  pasa a 1.

---

# 5. EL PLAZO, EL V-SYNC Y EL INPUT LAG

```text
   VBlank -- (espera) -- leer la entrada -- UMA (0,1 ms) -- dibujar -- FSR
          -- la pantalla directa (Z1) -- VBlank
```

* A0 empieza el fotograma lo mas tarde que A3 cree que llega: la entrada que
  ves es la mas reciente. Menos trabajo y menos input lag van juntos.
* Si un fotograma no llega, sale el anterior y A3 baja la resolucion del
  siguiente: nunca un fotograma tarde (`PLAN_EL_PLAZO.md`).
* Sin compositor (Z1): nada se copia de mas entre el juego y el monitor.

---

# 6. DONDE VIVE

```text
   platform/shared/verrano-ayni/     UMA (la red, sus pesos, su leccion), las
                                     reglas y la cuenta de las maestras.
                                     Puro: sin GPU dentro, probado en el banco
   el emisor de cada GPU             la 3060: los metodos de AMPERE_B de
                                     arriba. Otra tarjeta, otro emisor, la
                                     MISMA UMA
   PROTON-X                          lo que solo el ve: que lote es interfaz,
                                     cuantos triangulos cruzan cada baldosa,
                                     que cambio entre fotogramas
   la CPU que dibuja HOY             el mismo mapa sirve a la trama de
                                     PROTON-X (`trama.rs`): su coste esta casi
                                     todo en los pixeles, y Cyberpunk tarda
                                     63,9 s por fotograma en la CPU
```

---

# 7. LAS CASILLAS

- [ ] **AY0 -- el contrato, en papel.** Que entra (las ~9 de PACHA por
      baldosa), que sale (una tasa por baldosa), las reglas que la red no
      rompe. **Como se sabe:** `verrano-ayni/src/lib.rs` con el contrato y
      su prueba de forma.
- [ ] **AY1 -- A0, el plazo.** Un fotograma por VBlank, empezado tarde.
      **Como se sabe:** FRAPS-X dice fotogramas dibujados = mostrados, y el
      tiempo entre leer la entrada y el VBlank baja.
- [ ] **AY2 -- UMA en el banco.** La red, la leccion de las maestras y las
      reglas, en la CPU, sobre imagenes guardadas. **Como se sabe:** una
      imagen con cielo liso y un borde: el cielo sale 4x4, el borde 1x1; la
      prueba del NO: con la regla de interfaz quitada, el texto sale 2x2 y la
      prueba cae.
- [ ] **AY3 -- el mapa en la 3060.** La imagen de tasas y su tabla por los
      metodos de AMPERE_B, y el contador `PS_INVOCATIONS`. **Como se sabe:**
      un lote que llena la pantalla con todo a 2x2 cuenta 1/4 de las
      invocaciones que a 1x1 (el contador, en el metal).
- [ ] **AY4 -- el juez sabe de tasas.** El juez de la CPU dibuja con el mismo
      mapa y da los mismos bits que la 3060. **Como se sabe:** como VE5, la
      comparacion bit a bit en el metal.
- [ ] **AY5 -- UMA en vivo, con un juego.** **Como se sabe:** en el DIARIO,
      por escena, las invocaciones con y sin AYNI y el error de las
      maestras; el error por debajo del umbral del propietario.
- [ ] **AY6 -- el mismo mapa en la trama de la CPU.** **Como se sabe:** el
      fotograma de Cyberpunk por la CPU baja de 63,9 s con la imagen dentro
      del umbral.
- [ ] **AY7 -- A2, lo que no cambio.** **Como se sabe:** una app de
      escritorio quieta: las invocaciones por fotograma caen a casi cero y
      el pixel no cambia.
- [ ] **AY8 -- A3, la resolucion justa, con FSR.** **Como se sabe:** con
      carga variable, cero fotogramas tarde y la resolucion moviendose sola.
- [ ] **AY9 -- A4, lo que no se ve.** **Como se sabe:** lotes ahorrados
      contados, y en el banco ni un pixel distinto (si sale uno, la capa se
      apaga: CONSERVADOR).
- [ ] **AY10 -- A5, el reloj, DECISION DEL PROPIETARIO.** Pedir al GSP un
      estado de rendimiento mas bajo cuando sobra tiempo. Riesgo: es el reloj
      de la tarjeta.
- [ ] **AY11 -- la 5D: la prediccion del puntero.** Con la entrada que el
      kernel ve primero, predecir donde para el puntero y con que confianza;
      solo para ahorrar o preparar (seccion 2b). **Como se sabe:** en
      sesiones grabadas, el acierto de la prediccion a 1, 2 y 4 fotogramas;
      en vivo, el ahorro con y sin ella; y la prueba del NO: la entrada que
      llega al juego es identica bit a bit con la 5D encendida y apagada.

**El orden**: AY0 -> AY1 (lo que mas ahorra, sin red) -> AY2 -> AY3 -> AY4
-> AY5 -> AY6 -> AY7 -> AY8 -> AY11 -> AY9 -> AY10. AY2 y AY6 se pueden hacer YA
(la CPU dibuja hoy); AY3 en adelante esperan a que la 3060 dibuje los lotes
de Cyberpunk (`PLAN_EL_LIBRETO`, LI0..LI6).

---

# 8. LO QUE PUEDE FALLAR, dicho antes

* **Que se note**: el 4x4 en una textura con detalle fino se ve borroso. Las
  maestras lo miden y la regla del borde lo limita; el umbral lo pone el
  propietario.
* **Que UMA aprenda mal** en una escena nueva: las maestras rotan por toda
  la pantalla y el umbral sube solo. Y `ayni off` siempre existe.
* **El sombreado variable con derivadas** (`ddx`/`ddy`): a 2x2 se calculan
  sobre pixeles gruesos; el juez de AY4 tiene que hacerlo igual que el
  silicio.
* **A4 borra algo que si se veia**: por eso es conservadora y va despues.
* **A5 toca el reloj**: la ultima, y solo si el propietario lo pide.
