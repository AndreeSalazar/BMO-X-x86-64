# PLAN LLIPIQ -- la nitidez de BMO-X, y PACHA, su corazon de espacio y tiempo

> El propietario (12-10, tras el primer fotograma de Cyberpunk): *"puedes
> estudiar el FSR? ... mi nombre propio ... MUY FUERTE, que mejore TODA la
> pantalla ... que eduque a la GPU, no solo la 3060: VERRANO tendra su
> libreria ... mi propio DLSS, elegante, que consuma lo MENOS posible ...
> que en vez de pixeles cubra toda la pantalla y no calcule del todo ... que
> mejore los bordes y las lineas. Mi idea loca: algo 4D, los 3D orquestados
> por el 4D, la CPU guiando constantemente y la GPU dibujando preciso el LOD
> maestro ... SE PUEDE."*

**LLIPIQ** (quechua, *"lo que resplandece"*: de *llipiy*, brillar) es la
libreria de NITIDEZ de VERRANO. No sube los FPS: hace que lo que la GPU ya
dibujo se vea mejor -- bordes, lineas, detalle fino -- gastando lo minimo.

**PACHA** (quechua: el mundo, y a la vez el tiempo -- la misma palabra para
espacio y tiempo) es su corazon de cuatro dimensiones: x, y, z y el fotograma
anterior. Es la "4D" del propietario dicha con nombre de la casa.

---

# 0. LA RESPUESTA CORTA

**Si, se puede.** Y BMO-X puede llegar MAS lejos que FSR por una razon de
arquitectura, no de talento: FSR es una libreria que un juego llama y que solo
ve lo que el juego le da. En BMO-X, PROTON-X y VERRANO **ven cada dibujo**:
los vertices, los triangulos, el sombreador, el destino. Lo que FSR adivina
mirando pixeles, BMO-X lo puede SABER.

| | FSR (AMD) | DLSS (NVIDIA) | LLIPIQ (BMO-X) |
|---|---|---|---|
| quien lo llama | el juego, si lo integro | el juego, si lo integro | **BMO-X, a cualquier juego** (o el juego, si llama a FSR/DLSS) |
| los bordes | adivinados del color (EASU) | adivinados por una red | **sabidos**: PROTON-X tiene los triangulos (L3) |
| el tiempo (4D) | el juego da jitter y vectores | igual | **BMO-X pone el jitter** (el pegamento del de vertices) y, si el juego no da vectores, los saca el (L5) |
| la red neuronal | no | si, cerrada, tensor cores | no al principio; una propia y pequena, al final y solo si hace falta (L7) |
| la tarjeta | cualquiera | solo NVIDIA | **cualquiera**: vive en VERRANO como Programa de la casa (PROMETEO), juzgado en la CPU |
| licencia | MIT (abierta) | cerrada | de la casa; lo que venga de FSR, con su credito en `NOTICE` |

**Lo que LLIPIQ NO es, dicho antes:**

* **No inventa fotogramas** (lo que hace FSR 3 / DLSS 3 "frame generation"):
  eso sube los FPS mostrados y SUBE el input lag. El propietario quiere
  nitidez y latencia baja; los FPS los da BMO-X por otro lado (`smp all`,
  la 3060 dibujando, la pantalla directa).
* **No es magia sin coste**: cada pasada cuesta, y se mide (seccion 6).
* **No es una red neuronal al empezar.** Una red bien hecha necesita datos de
  entrenamiento y meses; FSR 2 demuestra que sin red se llega muy lejos.

---

# 1. LO QUE SE ESTUDIO DE FSR (la fuente, MIT, no de memoria)

`GPUOpen-Effects/FidelityFX-FSR` (`ffx_fsr1.h`, v1.20210629) y
`GPUOpen-Effects/FidelityFX-FSR2` (`README.md`, 2.2.1), leidos el 12-10.

## 1.1 FSR 1: una sola imagen

```text
   EASU  Edge Adaptive Spatial Upsampling   reescala 1x a 4x de AREA
         - un Lanczos(2) aproximado y RAPIDO, en UNA pasada
         - "direccional y anisotropo": mira la direccion del borde en una
           cruz de 12 texeles (gather4 por canal) y estira el filtro A LO
           LARGO del borde, no a traves
         - sin anillos: el resultado se limita al min/max de los 2x2
           texeles vecinos
         - pide: color en [0,1] YA con su tonemapping (gamma ~2), sin ruido
           fuerte; va al FINAL del fotograma, antes de la interfaz
   RCAS  Robust Contrast Adaptive Sharpening   NITIDEZ, sin reescalar
         - 5 taps en cruz: salida = (w*(n+e+o+s) + m) / (4w + 1), con w
           NEGATIVO (el lobulo que afila)
         - resuelve el w MAXIMO que no recorta (ni bajo 0 ni sobre 1):
           nitidez maxima sin halos
         - un paso alto normalizado detecta RUIDO y no lo afila
           (la nitidez va a los bordes de verdad, no al grano)
   extras LFGA (grano), SRTM (tonemapper reversible para HDR), TEPD (dither)
```

Es barato y va en cualquier imagen. Lo que no puede: inventar detalle que no
esta (un cable de un pixel que parpadea sigue parpadeando).

## 1.2 FSR 2: el tiempo (la "4D")

```text
   el juego da   color (resolucion baja), profundidad, VECTORES DE MOVIMIENTO,
                 el JITTER de la camara (Halton 2,3: cada fotograma la imagen
                 se corre un sub-pixel distinto), mascara reactiva (lo que no
                 debe arrastrar historia: particulas, transparencias)
   1 piramide de luminancia   exposicion y deteccion de cambios
   2 reconstruir y dilatar    vectores y profundidad engordados en los bordes
   3 depth clip               lo DESOCLUIDO (lo que acaba de aparecer) no usa
                              historia (separacion de Akeley)
   4 locks                    detalle fino (cables, rejas) se "bloquea" para
                              no perderlo
   5 reproyectar y acumular   la historia del fotograma anterior + el actual
                              (Lanczos): cada pixel junta MUCHAS muestras en el
                              tiempo -> resolucion de verdad, no inventada
   6 RCAS
   coste: 1920x1080 calidad, 0,2 a 0,6 ms en Radeon de 2020-2022
```

**La idea de fondo de FSR 2 es exactamente la del propietario**: el detalle
que un fotograma no tiene lo tienen los anteriores, si se sabe donde estaba
cada cosa. El tiempo es la cuarta dimension, y la orquesta quien sabe el
movimiento.

---

# 2. PACHA: LA "4D" DEL PROPIETARIO, EN PIEZAS QUE SE PUEDEN CONSTRUIR

*"Los 3D orquestados por el 4D, la CPU guiandola constantemente, la GPU
dibuja preciso."* En BMO-X eso se parte en tres papeles:

```text
   la CPU (PROTON-X, VERRANO)   ORQUESTA: sabe cada dibujo -- sus triangulos,
                                su PSO, su destino, y el fotograma anterior.
                                Decide el jitter, sabe que se movio, sabe
                                donde hay bordes de geometria
   la 3060                      DIBUJA preciso: la escena a su resolucion, y
                                las pasadas de LLIPIQ en VRAM (sin volver a
                                la RAM: el quirofano de VEL3)
   el juez (la CPU, despues)    COMPRUEBA: la misma cuenta en la CPU, pixel a
                                pixel en el banco (como `completa::del_bloque`)
```

**"En vez de pixeles, que cubra toda la pantalla y no calcule del todo"** son
dos ideas, y las dos estan:

1. **No calcular del todo**: el camino caro (reconstruir un borde) SOLO en
   los pixeles que tienen un borde. La mayoria de la pantalla (cielo, pared,
   suelo) se reescala con lo barato. FSR ya lo intuye (EASU es caro solo
   donde hay contraste); LLIPIQ lo SABE (L3: la mascara de bordes de la
   geometria). Menos calculo = menos vatios.
2. **Cubrir la pantalla, no el pixel**: un borde de triangulo no es una
   escalera de pixeles: es una RECTA con su posicion exacta, en coma
   flotante, que PROTON-X tiene. LLIPIQ reconstruye el borde desde la recta
   (la cobertura analitica de cada pixel) y no desde el color.

**"El LOD maestro"**: el nivel de detalle lo elige el juego, pero BMO-X sabe
que reescala desde mas abajo (renderizar a 1280x720 y presentar a 1920x1080):
el *mip bias* de las texturas se corrige (FSR 2 lo pide al juego, seccion
"Mipmap biasing"; BMO-X lo pone en el muestreador, L4), para que la textura se
vea con el detalle de la resolucion de SALIDA.

---

# 3. LAS CAPAS DE LLIPIQ (de la mas barata a la mas fuerte)

```text
   L1 LLIPIQ-ESPACIAL   reescalar + nitidez en UNA imagen (EASU + RCAS de la
                        casa). Cualquier imagen: un juego, el escritorio, un
                        video. Sin pedir nada a nadie
   L2 LA NITIDEZ SOLA   RCAS a resolucion nativa: lo mas barato, lo que pidio
                        el propietario ("nitidez, no FPS")
   L3 LOS BORDES SABIDOS la mascara de bordes de la GEOMETRIA: PROTON-X dibuja
                        las aristas de silueta en una pasada minima; LLIPIQ
                        reconstruye solo ahi
   L4 EL LOD MAESTRO    el mip bias puesto por BMO-X al reescalar
   L5 PACHA             el tiempo: jitter puesto por BMO-X + historia +
                        vectores (del juego, o sacados por BMO-X) + depth clip
                        + locks (FSR 2 de la casa)
   L6 LA PUERTA FFX     el `ffxDispatch` de Cyberpunk (E0.4) servido POR
                        LLIPIQ: el juego ya le da color, profundidad y vectores
                        a FSR; BMO-X los recibe y hace PACHA con datos
                        perfectos
   L7 LA RED (lejos)    una red pequena propia, en los tensor cores de la 3060
                        (mma de SM86), solo si L5/L6 se quedan cortos
```

**Donde vive cada cosa** (la regla de la casa: VERRANO sin una GPU dentro,
`la-3060`):

```text
   platform/shared/verrano-llipiq/   las cuentas como Programa de la casa
                                     (PROMETEO) y su juez en la CPU; ni una
                                     tarjeta dentro
   el emisor de cada GPU             sm86 hoy (proton-x-sm86); otra GPU, otro
                                     emisor, la MISMA libreria
   el kernel                         la puerta estrecha que YA existe para
                                     esto: IOMMU_OP_GPU_COMPLETAR (VEL2): una
                                     app pasa SU imagen por un programa del
                                     KERNEL, en SU bloque
   PROTON-X                          el jitter (pegamento del de vertices), la
                                     mascara de bordes (L3), la puerta FFX (L6)
```

---

# 4. EL CONSUMO MINIMO (la regla, con numeros)

* **Presupuesto por pixel de salida**: L2 = 5 lecturas; L1 = 12 + 5; L3 =
  solo en los pixeles de borde (se mide que fraccion son: el censo); L5 = el
  de FSR 2 (0,2-0,6 ms a 1080p en una Radeon de gama media): la meta es
  quedar por DEBAJO de eso en la 3060.
* **Coma flotante de 16 bits** donde no cambia el resultado (FSR tiene sus
  versiones `H` / `Hx2` empaquetadas: dos cuentas por instruccion).
* **Una pasada por capa**, en VRAM, sin volver a la RAM (VEL3).
* **El ahorro de verdad**: dibujar el juego a 1280x720 y presentar a
  1920x1080 son 2,25 veces menos pixeles sombreados. LLIPIQ cuesta una
  fraccion de eso: menos vatios y menos temperatura, con la imagen igual o
  mejor. Eso es "exprimir": no mas trabajo, mejor trabajo.

---

# 5. V-SYNC, EL PLAZO Y EL INPUT LAG

LLIPIQ va al FINAL del fotograma, justo antes del escaneo:

```text
   el juego dibuja -> LLIPIQ (en VRAM) -> la pantalla directa (Z1) -> el VBlank
```

* Con la pantalla directa no hay compositor: LLIPIQ es la ULTIMA mano antes
  del monitor, sin copia de mas.
* El plazo lo pone `PLAN_EL_PLAZO.md` (V-Sync, VBlank): LLIPIQ se lanza en
  cuanto el fotograma esta y tiene que acabar antes del VBlank; si no llega,
  ese fotograma sale SIN LLIPIQ (nunca tarde): la latencia manda sobre la
  nitidez.
* Sin generacion de fotogramas: ni un fotograma de retraso anadido.

---

# 6. LAS CASILLAS

- [ ] **LL0 -- el contrato, en papel.** Que entra (color, su formato y su
      espacio: SDR o HDR; profundidad, vectores, jitter si los hay), que
      sale, y que se promete (los bits del juez). **Como se sabe:** el
      contrato en `verrano-llipiq/src/lib.rs` y su prueba de forma.
- [ ] **LL1 -- RCAS de la casa (L2), en la CPU y juzgado.** La cuenta de
      `ffx_fsr1.h` (MIT, con su credito en `NOTICE`) como Programa de la
      casa; la CPU la corre como juez. **Como se sabe:** en el banco, una
      imagen con un borde y una con ruido: el borde se afila, el ruido no
      (la prueba del NO: sin la deteccion de ruido, el ruido SI se afila y la
      prueba cae).
- [ ] **LL2 -- RCAS en la 3060**, por la puerta de COMPLETAR (VEL2), en
      VRAM. **Como se sabe:** el juez de la CPU da los mismos bits que la
      3060 en el metal (como VE5).
- [ ] **LL3 -- EASU de la casa (L1)**, CPU y 3060, mismo camino. **Como se
      sabe:** 1280x720 -> 1920x1080 sin anillos (la prueba del min/max) y con
      el borde estirado a lo largo, no a traves.
- [ ] **LL4 -- `llipiq` en el escritorio**: `llipiq off | nitidez | 1.5x |
      2x`, para cualquier app por PROTON-X. **Como se sabe:** FRAPS-X dice
      los ms de LLIPIQ por fotograma; F11 los vatios antes y despues.
- [ ] **LL5 -- los bordes sabidos (L3).** PROTON-X dibuja las aristas de
      silueta de cada lote en una mascara (una pasada de lineas, barata) y
      LLIPIQ reconstruye solo ahi. **Como se sabe:** en el banco, un
      triangulo inclinado: el borde de LLIPIQ esta mas cerca de la recta
      exacta que el de EASU solo (error medido contra la cobertura
      analitica), y los pixeles sin borde cuestan lo de L1.
- [ ] **LL6 -- el LOD maestro (L4)**: el mip bias en el muestreador cuando
      se reescala. **Como se sabe:** una textura de rayas a 1280x720 ->
      1920x1080 tiene el detalle de la de 1920x1080 nativa (sin parpadeo).
- [ ] **LL7 -- PACHA (L5): el jitter de la casa.** El pegamento del de
      vertices suma el sub-pixel de Halton(2,3) a la posicion, sin que el
      juego lo sepa; la historia y la acumulacion. **Como se sabe:** una
      escena QUIETA converge en N fotogramas a la imagen de resolucion
      completa (error contra la nativa, bajando fotograma a fotograma).
- [ ] **LL8 -- PACHA con movimiento**: vectores del juego si los da;
      depth clip y locks como FSR 2. **Como se sabe:** una escena que se
      mueve sin estela (ghosting medido en los bordes de lo que se mueve).
- [ ] **LL9 -- la puerta FFX (L6)**: el `ffxDispatch` de Cyberpunk (E0.4)
      servido por PACHA con los datos del juego. **Como se sabe:** en el
      DIARIO, `ffxFsr2ContextDispatch` atendido por LLIPIQ; en la pantalla,
      el juego con su "FSR" puesto y la imagen de BMO-X.
- [ ] **LL10 -- la red (L7), DECISION DEL PROPIETARIO**: solo si LL8/LL9 se
      quedan cortos, y con un plan propio (datos, entrenamiento, los tensor
      cores de SM86 por el emisor de la casa).

**El orden**: LL0 -> LL1 -> LL2 (la primera nitidez en la pantalla, barata)
-> LL3 -> LL4 (el propietario lo usa) -> LL5 (lo que FSR no puede) -> LL6 ->
LL7 -> LL8 -> LL9. Y antes de todo, lo de ahora: que la 3060 dibuje los
lotes de Cyberpunk (`PLAN_EL_LIBRETO`, LI0..LI6) -- LLIPIQ necesita un
fotograma en la VRAM al que aplicarse.

---

# 7. LO QUE PUEDE FALLAR, dicho antes

* **La estela (ghosting)** en lo que se mueve: es el fallo clasico de lo
  temporal. Se mide (LL8) y el depth clip y la mascara reactiva son su cura.
* **La interfaz**: LLIPIQ va ANTES de la interfaz del juego (EASU lo pide):
  si BMO-X no sabe donde acaba la escena y empieza la interfaz, el texto se
  afila de mas. PROTON-X lo puede saber (el lote que dibuja la interfaz es
  otro PSO); se apunta en LL4.
* **HDR**: EASU y RCAS piden color en [0,1]; con render targets de float
  (Cyberpunk) va el tonemapper reversible (SRTM de FSR) alrededor.
* **El jitter de la casa (LL7) con un juego que ya tiene su TAA**: dos
  jitters se pelean. Se detecta (el juego ya mueve la proyeccion) y no se
  pone el de la casa.
* **La red (LL10)**: es la casilla de mas riesgo y menos certeza; por eso va
  la ultima y con decision del propietario.
