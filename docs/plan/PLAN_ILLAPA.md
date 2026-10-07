# PLAN ILLAPA -- el motor de CGI en tiempo real de BMO-X: motor grafico + Houdini propio, en TITAN++

> Abierto el 2026-10-07. El propietario: *"mi propio houdini ... que sean
> buenas para trabajar y jugar ... una idea simple que es mi propio motor que
> es MOTOR GRAFICOS + HOUDINI propio que simplemente se fusionan uno solo para
> crear mi primer CGI Real time"*. Lo que quiere hacer con el: VFX y
> personajes que sangran. Y la cadena hasta la 3060, en sus palabras:
> *"VERRANO es lo que conecta con juez con GPU emitiendo el SM86"*.
>
> **Es un plan LARGO y no es para ahora**, como
> [`PLAN_LA_TINTA.md`](PLAN_LA_TINTA.md): se empieza el dia que el propietario
> lo diga. Las ideas son publicas (seccion 9); el codigo, el nombre y la
> forma, de BMO-X.

---

# 0. EL NOMBRE

**ILLAPA**, elegido por el propietario el 07-10: el dios inca del trueno, la
lluvia y la tormenta. Manda en el clima -- en los fluidos, el fuego y el rayo
--, que es justo lo que simula un motor de efectos. Y es familia de INTI, el
sol, que ya vive en BMO-X.

---

# 1. LA IDEA: UN SOLO PROGRAMA, DOS MODOS

```text
   PAUSA    el TALLER de efectos (lo de Houdini): los nodos de F1 editan la
            escena, se ajusta un parametro y se ve al instante
   PLAY     el JUEGO (lo del motor): la misma escena corre a 60 fps, y lo
            que se simula reacciona a lo que haces
```

No son dos programas que se pasan ficheros: es **el mismo**. Lo que se edita
es lo que se juega. Que se puede, ya lo demostraron otros, y en publico:

* **JangaFX**, un estudio chico, hizo EmberGen (fuego y humo en tiempo real,
  usado por mas de 150 estudios de juegos) y **LiquiGen**: liquidos en tiempo
  real -- agua, sangre, ketchup, baba --, con nodos, un trazador de rayos en
  vivo y salida a VAT para los juegos. Su solucionador es APIC.
* **Unreal**: el editor y el juego son el mismo motor.

---

# 2. EL CORAZON: UN SOLO SOLUCIONADOR DE PARTICULAS PARA TODO

Lo que hace posible la fusion es una familia de metodos publicada:

* **Vellum**, el de Houdini (tela, pelo, cuerpos blandos, granos y, desde
  Houdini 19, fluidos sencillos), es *Position Based Dynamics* extendida:
  **XPBD**. Lo dice la documentacion publica de SideFX.
* **Position Based Fluids** (Macklin y Muller, SIGGRAPH 2013): liquidos
  incompresibles con la estabilidad de PBD, que deja dar pasos de tiempo
  grandes. Es lo que pide el tiempo real.
* **Unified Particle Physics for Real-Time Applications** (Macklin, Muller,
  Chentanez y Kim, SIGGRAPH 2014): UN solucionador de particulas con
  restricciones que hace gases, liquidos, solidos deformables, rigidos y
  tela, **todos chocando entre si**, en tiempo real.

Para ILLAPA quiere decir esto:

```text
   la SANGRE        particulas de fluido (PBF): densidad, presion, viscosidad
   la CARNE / PIEL  cuerpos blandos: restricciones de forma
   la ROPA          tela: restricciones de distancia, que se moja y se pega
   los ESCOMBROS    cuerpos rigidos: restricciones de forma rigida
                    ... y todo choca con todo, en el MISMO paso
```

Y encaja con TITAN++ sin forzarlo: cada paso del solucionador es *"haz esto
con cada particula"*, que es exactamente una `gpu fn` (una celda, un hilo por
celda) -- el VEX de Houdini es eso mismo: un programa por punto.

---

# 3. LA CADENA HASTA LA 3060

La que describio el propietario, pieza a pieza, con lo que hay y lo que no:

```text
   ILLAPA (TITAN++)       las gpu fn del solucionador y del dibujo
        |
   emisor-spirv           gpu fn -> SPIR-V GLCompute, con su mapa al fuente   HECHO (G2 de PLAN_EL_CENTAURO)
        |                 [ en linea recta: SIN bucles ni vecinos ]
   el juez de SPIR-V      validar + el subconjunto de la 3060 + el oraculo     HECHO
        |
   el emisor SM86         SPIR-V -> las instrucciones de la 3060              PLAN_LA_LENGUA_DE_LA_3060
        |                 [ faltan saltos, bucles y enteros: E6 ]
   el JUEZ DEL SASS       lee lo que salio, instruccion a instruccion          V3b de PLAN_VERRANO
        |
   el BSF (kind SM86)     el sobre: el codigo YA traducido, con sus hashes     HECHO (V0b)
        |
   VERRANO                la API de dibujo: la GPU NO COMPILA NADA             V0-V1c HECHO; V2+ no
        |
   la 3060                el computo de verdad (G4) y el 3D de verdad          del METAL, del propietario
```

**[!] Una precision (07-10, al leer este plan con el codigo):** el emisor
de la 3060 que EXISTE (`proton-x-sm86`, E3) no lee SPIR-V: lee el `Programa`
de la casa, por decision del 28-09. Del lado de SPIR-V solo esta el
subconjunto (E1). El eslabon que falta es un lector de SPIR-V al `Programa`:
es **LI7** de [`PLAN_EL_LIBRETO.md`](PLAN_EL_LIBRETO.md), y va antes que IL1
para que lo que aprenda ILLAPA caiga en el mismo emisor que usa Cyberpunk.

**La regla de VERRANO vale entera para ILLAPA**: la 3060 no compila; los
programas viajan ya traducidos y juzgados, y de un fotograma al siguiente solo
cambian los DATOS (las posiciones de las particulas, la camara).

---

# 4. LO QUE FALTA, Y DE QUIEN ES

| falta | donde | por que ILLAPA lo necesita |
|---|---|---|
| `gpu fn` con VECINOS y BUCLES | nuevo: G2 de `PLAN_EL_CENTAURO.md` escribe SPIR-V en linea recta, y la GRAMATICA deja los bucles para despues; el emisor SM86 los aprende en E6 de `PLAN_LA_LENGUA_DE_LA_3060.md` | una gota de fluido mira a las gotas de al lado; hoy una `gpu fn` solo ve su celda |
| la 3060 COMPUTANDO de verdad | G4 de `PLAN_EL_CENTAURO.md` (la QMD y el banco 0) | el solucionador corre alli; es del METAL |
| la PROFUNDIDAD | V2 / M1 de `PLAN_VERRANO.md` (X5b de `PLAN_EL_CUBO.md`) | que lo de atras no tape lo de adelante |
| las CONSTANTES | V3 / M2 de `PLAN_VERRANO.md` | la camara y los huesos en un buffer |
| las TEXTURAS | M3 de `PLAN_VERRANO.md` | la piel, las manchas de sangre |
| la MEZCLA (alfa) | VERRANO hoy pinta sin mezcla | la sangre es semitransparente; el humo, mas |
| el BYTE, la VENTANA, la ENTRADA, el DISCO desde TITAN++ | TA1-TA4 de `PLAN_LA_TINTA.md` | los mismos que pide LA TINTA: se hacen UNA vez |
| leer glTF | nada todavia en el arbol | el personaje, hecho en Blender y traido como fichero |

**De lo de arriba, casi todo ya esta en marcha por otro lado**: el camino de
la 3060 es el mismo que pide Cyberpunk por PROTON-X, y las puertas de
TITAN++ las pide LA TINTA. ILLAPA no abre un frente nuevo: llega cuando esos
dos caminos se juntan.

---

# 5. EL PRIMER CGI: la escena

Chica, entera y a 60 fps en la 3060:

```text
   un personaje glTF (hecho en Blender), animado con su esqueleto
   + una camara que se mueve
   + un golpe: la sangre sale como particulas PBF, choca con el cuerpo y
     con el suelo, gotea y se queda
   + el liquido pintado como SUPERFICIE (no como puntitos): mojado y con brillo
   + PAUSA -> los nodos de F1 cambian la viscosidad o la fuerza del golpe;
     PLAY -> vuelve a correr
```

El liquido como superficie, en tiempo real, se hace en la pantalla (*screen
space fluid rendering*): cada particula se pinta como una esfera en un mapa
de profundidad, ese mapa se suaviza, de el salen las normales, y con ellas el
brillo y la refraccion. No hace falta construir una malla.

---

# 6. LA ESCALERA

Cada peldanio se prueba solo. Lo que no toca la 3060 se prueba antes en el
anfitrion, con el oraculo de SPIR-V corriendo las mismas `gpu fn`.

- [ ] IL0 -- las decisiones de la seccion 8, contestadas por el propietario
- [ ] IL1 -- `gpu fn` CON VECINOS: leer celdas de OTRA tabla por indice y bucles acotados, en el lenguaje, el oraculo y el emisor de SPIR-V (el que escribio G2 de `PLAN_EL_CENTAURO.md`, hoy en linea recta); y que el emisor SM86 los sepa traducir (E6 de `PLAN_LA_LENGUA_DE_LA_3060.md`)
- [ ] IL2 -- LA REJILLA DE VECINOS: cada particula a su celda, ordenadas por celda (un conteo y un orden, en `gpu fn`), para que cada una encuentre a las de al lado sin mirar a todas
- [ ] IL3 -- PBF, la sangre: densidad, la correccion de posicion, la presion artificial (la tension superficial), la vorticidad y la viscosidad (Macklin y Muller 2013). La prueba, en el anfitrion: una presa que cae conserva su densidad
- [ ] IL4 -- LO UNIFICADO: tela, cuerpo blando y rigido como restricciones sobre las MISMAS particulas (2014), con la rigidez de XPBD (que no dependa de cuantos pasos se den)
- [ ] IL5 -- glTF: un lector puro (malla, esqueleto, pesos de la piel y animacion), con su banco en el anfitrion
- [ ] IL6 -- EL PERSONAJE: la piel que sigue a los huesos (una suma de matrices por vertice: una `gpu fn`), y sus particulas de choque pegadas a los huesos
- [ ] IL7 -- VERRANO en 3D: profundidad, constantes, texturas y mezcla (V2, V3, M3 de `PLAN_VERRANO.md`), con muchos triangulos
- [ ] IL8 -- EL LIQUIDO COMO SUPERFICIE: profundidad de las esferas, suavizado, normales, brillo y refraccion
- [ ] IL9 -- PAUSA Y PLAY: un solo programa; en pausa los nodos de F1 editan, en play corre; la escena es un nodo de ESTRATOS con versiones y ramas, como la `.obra` de `PLAN_LA_TINTA.md`
- [ ] IL10 -- EL PRIMER CGI: la escena de la seccion 5 a 60 fps en la 3060. Del METAL: lo corre el propietario en su Ryzen
- [ ] IL11 -- HORNEAR: una simulacion grabada a texturas de animacion (VAT), que el modo PLAY reproduce barato; la calidad de cine dentro del juego

## Despues, si se pide

- [ ] IL12 -- HUMO Y FUEGO: una rejilla de volumen (lo de EmberGen), con su dibujo por rayos en la niebla
- [ ] IL13 -- LIQUIDO DE CINE: APIC o FLIP para cuando la sangre de PBF no alcance (lo de LiquiGen y Houdini)
- [ ] IL14 -- EL TRAZADOR DE RAYOS (lo de RenderMan de verdad): la imagen final, no en tiempo real, en computo de la 3060 SIN nucleos de rayos (mas lento, igual de correcto; ver 6c); los nucleos de rayos, si un dia se abren, solo lo aceleran

---

# 6b. DESPUES: LOS PERSONAJES -- esculpir, pintar y animar (con la cara)

> El propietario, 07-10: *"anotar que necesitarian ZBrush, Substance y
> Cascadeur ... con animacion en cara"*. Y antes: *"creo que empezar con VFX
> es mas que suficiente"*. Asi que esto va DESPUES de IL11, y mientras tanto
> los personajes se hacen en Windows con esas mismas herramientas y entran
> por glTF (D4).

## Que es cada una por dentro

| herramienta | que hace | que es por dentro |
|---|---|---|
| **ZBrush** | ESCULPIR: barro digital | una malla de millones de poligonos, con niveles de subdivision; cada pincel EMPUJA, TIRA o ALISA los vertices cercanos con una caida suave. Al final se HORNEA el detalle a un mapa de normales para que un personaje ligero se vea como el detallado |
| **Substance Painter** | PINTAR el modelo 3D | capas, pinceles y mascaras como Photoshop, pero proyectados sobre el modelo; materiales PBR (color, metal, rugosidad, normal) y mapas horneados (oclusion, curvatura) para que la suciedad caiga sola en las grietas y los bordes se gasten solos |
| **Cascadeur** (de Nekki) | ANIMAR el cuerpo | esqueleto, poses clave y curvas, con fisica: AutoPosing (una red neuronal coloca el resto del cuerpo a partir de unos pocos puntos) y AutoPhysics (corrige la animacion para que sea fisicamente creible, tocandola lo menos posible); desde 2024.2 mueve tambien la cara por blendshapes |
| **la animacion de la cara** | gestos, habla | BLENDSHAPES (formas de la cara guardadas como desplazamientos de vertices, que se suman con un peso cada una; el juego de referencia son las 52 de ARKit), mas huesos para la mandibula y los ojos; el habla, de fonemas a formas de boca (*visemas*) |

## Lo que NO es nuevo: ya esta en los otros planes

```text
   SUBSTANCE   =  las capas, el pincel y las mascaras de LA_TINTA, pintando sobre 3D
   ZBRUSH      =  el motor de pincel de LA_TINTA empujando VERTICES, no pixeles,
                  con la Wacom (TC1 de LA_TINTA) y la 3060 de ILLAPA
   CASCADEUR   =  el esqueleto de IL6 y el solucionador de ILLAPA (XPBD) para la fisica
   LA CARA     =  una suma de formas por vertice: una gpu fn
                  (y glTF ya las trae: se llaman "morph targets")
```

## Lo que necesitaria TITAN++ (y BMO-X) para esto

| hace falta | para que | estado |
|---|---|---|
| **el f32 en la CPU** (o hacer TODA esa cuenta en `gpu fn`) | rotaciones, cuaterniones, IK y curvas de animacion son coma flotante; hoy la regla D2 de TITAN++ deja el `f32` solo en la 3060 | una LEY: del propietario, con `--sellar` (D5) |
| mallas de millones de vertices | esculpir | el byte y las tablas grandes (TA1 de LA_TINTA) |
| `gpu fn` con vecinos y bucles | el pincel que mueve los vertices cercanos, alisar, hornear | IL1 |
| lanzar rayos contra la malla (una estructura de cajas, BVH) | hornear normales, oclusion y curvatura | nuevo; despues, los nucleos de rayos de la 3060 |
| la Wacom con presion | esculpir y pintar se hacen con lapiz | TC1 de LA_TINTA |
| ESCRIBIR glTF, no solo leerlo | que lo hecho en BMO-X salga a otros programas | nuevo (IL5 solo lee) |
| redes neuronales pequenias | lo de AutoPosing (opcional: sin el, se anima a mano con IK) | multiplicar matrices: `gpu fn`; el modelo entrenado es otro asunto |
| captura de la cara | animar la cara grabandose | la ANTENA: el movil ya es parte de BMO-X (`PLAN_CLOUD_LOCAL.md`, `PLAN_LA_ANTENA_AOT.md`); MediaPipe Face Landmarker da 52 pesos de blendshapes en Android |

## Su escalera (despues de IL11)

- [ ] IL15 -- LA CARA EN ILLAPA: blendshapes del glTF sumados en una `gpu fn`, y la mandibula y los ojos por huesos; la primera prueba, un gesto puesto a mano
- [ ] IL16 -- ANIMAR (lo de Cascadeur): esqueleto, poses clave, curvas, cinematica inversa (IK) y la fisica del solucionador de ILLAPA para que el salto caiga creible
- [ ] IL17 -- LA CARA GRABADA: el movil (la ANTENA) manda los pesos de la cara cuadro a cuadro, e ILLAPA los pone en el personaje
- [ ] IL18 -- PINTAR EN 3D (lo de Substance): las capas y el pincel de LA_TINTA proyectados sobre el modelo, materiales PBR y los mapas horneados
- [ ] IL19 -- ESCULPIR (lo de ZBrush): la malla con niveles, los pinceles con la Wacom y el horneado del detalle a normales
- [ ] IL20 -- glTF DE SALIDA: lo esculpido, pintado y animado en BMO-X sale para cualquier otro programa

---

# 6c. LA LUZ: lo de RenderMan, a nivel de juego y SIN RTX

> El propietario, 07-10: *"falto iluminacion estilo RENDERMAN ... algo eso el
> nivel para jugar ... sin RTX"*.

**RenderMan** (Pixar) es un trazador de rayos fuera de tiempo real: sigue la
luz rebote a rebote, con materiales fisicos, luces de area, piel que deja
pasar la luz por dentro y un quitarruido al final. Un fotograma tarda minutos.
**A 60 fps no se traza: se APROXIMA**, y las aproximaciones estan publicadas y
funcionan sin nucleos de rayos -- todos los juegos anteriores a las RTX
(2018) se veian asi, y Unreal 5 tiene un modo de luz global (Lumen) por
software, sin RTX.

**Y encaja con TITAN++ sin pedir nada nuevo al lenguaje**: toda esta cuenta es
coma flotante en la 3060, una formula por pixel -- una `gpu fn`. No pide el
`f32` en la CPU (D5).

| lo de RenderMan | a 60 fps, sin RTX | quien lo publico |
|---|---|---|
| materiales fisicos | el modelo "principled" de Disney con GGX: color, metal, rugosidad. **El mismo que trae el glTF**, asi que los personajes de Substance llegan ya con el | Burley, Disney, SIGGRAPH 2012 |
| la luz de alrededor (el cielo, la sala) | un mapa del entorno FILTRADO de antemano por rugosidad, mas una tabla: dos lecturas por pixel | Karis, "Real Shading in Unreal Engine 4", 2013 |
| luces de area (una ventana, un panel) | cosenos transformados linealmente (LTC): una tabla y una formula | Heitz y otros, SIGGRAPH 2016 |
| sombras suaves | mapas de sombra en cascada, con el borde que se ablanda con la distancia | oficio comun |
| la sombra de los rincones | oclusion ambiental en pantalla (SSAO, GTAO) | oficio comun |
| **la piel que no parece plastico** | dispersion bajo la superficie EN PANTALLA: un desenfoque separable con el perfil de la piel. Es lo que mas se nota en un personaje | Jimenez y otros, 2015 |
| la luz que rebota | sondas de luz, o rayos por SOFTWARE contra campos de distancia (lo de Lumen sin RTX) | Epic, Unreal 5 |
| el aire con luz | niebla con volumen, en una rejilla pegada a la camara | oficio comun |
| el "look" de cine | color en HDR (coma flotante) y el tono final de cine (ACES) | la Academia de Cine |

**La sangre gana mucho con esto**: mojada es un brillo fuerte (rugosidad
baja) y una capa de barniz encima; espesa, deja pasar algo de luz (la misma
dispersion que la piel). Sin luz buena, la mejor simulacion parece pintura.

## Lo que pide a VERRANO

Pantallas intermedias de coma flotante (HDR) y varias a la vez (el
*G-buffer*), profundidad, texturas con niveles (*mipmaps*) y mapas de cubo, y
pasadas de computo entre dibujo y dibujo. Lo mismo que piden los juegos que
corre PROTON-X (`PLAN_LA_ESCALERA_PROTON_X.md`): no es un frente aparte.

## Su escalera

El primer CGI (IL10) pide **LZ1 a LZ4**; lo demas lo mejora despues.

- [ ] LZ1 -- HDR Y EL TONO DE CINE: pintar en coma flotante y bajar a la pantalla con ACES
- [ ] LZ2 -- MATERIALES FISICOS: el modelo de Disney con GGX, leido del material PBR del glTF
- [ ] LZ3 -- LA LUZ DEL ENTORNO: el mapa filtrado por rugosidad y su tabla (Karis 2013)
- [ ] LZ4 -- SOMBRAS: mapas de sombra en cascada, con borde suave
- [ ] LZ5 -- LA PIEL Y LA SANGRE: dispersion bajo la superficie en pantalla (Jimenez 2015), y el barniz de lo mojado
- [ ] LZ6 -- LOS RINCONES: oclusion ambiental en pantalla
- [ ] LZ7 -- LUCES DE AREA: LTC (Heitz 2016)
- [ ] LZ8 -- LA LUZ QUE REBOTA: sondas, y despues rayos por software contra campos de distancia
- [ ] LZ9 -- EL AIRE: niebla con volumen, que el humo de IL12 reutiliza

---

# 7. LO QUE ESTE PLAN NO HACE

* **No trae Houdini ni Blender.** De Houdini se aprenden sus ideas, que son
  publicas; ni su codigo, ni su nombre. Blender se queda en Windows: se hace
  alli el personaje y BMO-X lee el **glTF** (un formato abierto, sin duenio).
  Traer Blender entero serian millones de lineas ajenas, que
  `docs/identidad/LOS_TRES_VERTICES.md` no admite, y su licencia (GPL) no
  casa limpia con la de BMO-X.
* **No modela ni anima personajes AL PRINCIPIO.** Esculpir, pintar y animar
  es la seccion 6b, despues del primer CGI; hasta entonces se hacen en
  Windows (ZBrush, Substance, Cascadeur o Blender) y entran por glTF.
* **No es un renderizador de cine al principio.** Primero se ve bien a 60
  fps; el trazador de rayos es IL14.
* **No adelanta el metal.** La 3060 computando (G4) y el primer CGI (IL10)
  son del propietario y de su Ryzen.

---

# 8. LAS DECISIONES DEL PROPIETARIO

```text
   D1  EL NOMBRE        CONTESTADA (07-10): ILLAPA
   D2  DONDE VIVE       que tecla o que sitio (F1..F12 ya estan todas): una
                        nueva en la rejilla, o un modo de F1
   D3  LA ESCENA        la de la seccion 5, u otra para el primer CGI
   D4  EL glTF          traer los personajes de Blender como glTF (el formato
                        abierto), o esperar a otro camino
   D5  EL f32 EN LA CPU para animar (6b): una ley nueva de TITAN++, o toda la
                        cuenta de rotaciones en gpu fn. Con --sellar
```

---

# 9. DE DONDE SALE LO DE ARRIBA

* Vellum es XPBD: [SideFX, Vellum Overview](https://www.sidefx.com/docs/houdini/vellum/overview.html) y [Houdini 19, Vellum](https://www.sidefx.com/docs/houdini/news/19/vellum.html).
* Position Based Fluids (Macklin y Muller, SIGGRAPH 2013): [SIGGRAPH History](https://history.siggraph.org/?p=108378).
* Unified Particle Physics for Real-Time Applications (Macklin, Muller, Chentanez y Kim, SIGGRAPH 2014): [su video](https://vimeo.com/94622661) y [un resumen](https://geeks3d.com/20140516/unified-particle-physics-for-real-time-applications).
* LiquiGen y EmberGen de JangaFX: [3DVF](https://3dvf.com/en/jangafx-launches-liquigen-real-time-liquid-simulation-at-your-fingertips/), [CG Channel](https://www.cgchannel.com/2024/02/jangafx-releases-liquigen-0-1/) y [80.lv](https://80.lv/articles/jangafx-shows-its-real-time-fluid-simulation-tool).
* El liquido pintado en la pantalla (*screen space fluid rendering*), la
  rejilla de vecinos y la piel por huesos son conocimiento comun del oficio
  (las charlas y ejemplos de NVIDIA de 2008-2010 lo cuentan).
* La luz de 6c: Burley, *Physically-Based Shading at Disney* (SIGGRAPH 2012); Karis, *Real Shading in Unreal Engine 4* (SIGGRAPH 2013); Heitz, Dupuy, Hill y Neubelt, *Real-Time Polygonal-Light Shading with Linearly Transformed Cosines* (SIGGRAPH 2016); Jimenez y otros, *Separable Subsurface Scattering* (2015); y la documentacion de Lumen de Epic (su modo por software). Citados de memoria: se leen antes de escribir la primera linea.
* Cascadeur, AutoPosing, AutoPhysics y la cara por blendshapes: [CG Channel, Cascadeur 2024.2](https://www.cgchannel.com/2024/09/nekki-unveils-cascadeur-2024-2/).
* MediaPipe Face Landmarker y sus 52 pesos de blendshapes: [Google, Face landmark detection guide](https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker).
* Lo de BMO-X: `PLAN_VERRANO.md`, `PLAN_LA_LENGUA_DE_LA_3060.md`,
  `PLAN_EL_CENTAURO.md` y `toolchain/lang/titan/GRAMATICA.md` (nivel 11),
  leidos el 07-10.
