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
   el PROGRAMA de la casa gpu fn -> Programa (PROMETEO), el MISMO            HECHO (07-10)
        |                 que sale de los sombreadores de PROTON-X
   el emisor SM86         Programa -> las instrucciones de la 3060 (E3..E6)  HECHO, en linea recta
        |                 [ faltan bucles y vecinos en la gpu fn: IL1 ]
   el JUEZ DEL SASS       ESTRICTO: la libreria de lo que sabe la 3060        HECHO (J1)
        |                 (una AMD tendria SU libreria)
   la 3060 simulada       el oraculo de cada build: sus bits, contra la      HECHO (07-10)
        |                 casa y el calculo
   VERRANO                la API de dibujo: la GPU NO COMPILA NADA           V0-V1c HECHO; V2+ no
        |
   la 3060                el computo de verdad (G4) y el 3D de verdad        del METAL, del propietario
```

**SIN SPIR-V** (decision del propietario, 07-10: *"gpu - sm86 - luego el juez
verifica estrictamente y el 3060"*): el escritor de SPIR-V de TITAN++ se
quito. Un solo emisor y un solo juez para TITAN++ y para PROTON-X: lo que
ILLAPA abra en la 3060, Cyberpunk lo usa sin copiarlo.

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

> **08-10, al dia (H4 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md)):**
> las dos primeras filas son de antes del 07-10. Ya no hay SPIR-V: la gpu fn la
> escribe `toolchain/lang/titan/prometeo` (PROMETEO) al Programa de la casa, en linea
> recta, y los BUCLES ya los sabe el emisor SM86 con su juez (E6); lo que los
> rechaza es TITAN++ (`gpu.rs`, T0090) -- LB5 de LAS_LIBRERIAS. Los VECINOS
> piden ademas un asa que ponga el kernel (R7 no deja leer memoria a una app):
> LB9. Y la QMD corre desde el 24-09: lo que falta para COMPUTAR es la PUERTA
> de una app (LB8 = G4 = LI2f).

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

> **08-10, al dia (H4 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md)):**
> el oraculo ya no es el de SPIR-V: es la 3060 simulada
> (`platform/shared/proton-x-sm86/src/simula.rs`) sobre el SASS que el juez
> acepto, contra la casa y el calculo. IL1 se escribe en el escritor del
> Programa (`toolchain/lang/titan/prometeo`), no en un emisor de SPIR-V,
> y se parte en dos: los bucles (LB5) y los vecinos por el asa (LB9).
>
> **08-10, LB5:** los bucles, hechos (con su N escrito: la ley L33), y las
> llamadas entre gpu fn, en linea. IL1 sigue abierta por los vecinos (LB9).
>
> **08-10, LB6:** y DIBUJA: una gpu fn de vertice o de pixel, por su firma,
> con registros de f32. La piel de IL6 (una suma de matrices por vertice)
> ya tiene donde escribirse; le faltan las constantes (el cbuffer) y que
> VERRANO la ponga a dibujar (LB7).
>
> **18-10, LB9:** los VECINOS, hechos en el anfitrion (AMARILLO): un
> parametro `campo: [f32]` llega ENTERO a cada hilo, `celda()` dice cual es
> el suyo, y fuera de la lista se lee 0 (DL9). La gpu fn sale como un
> computo de la casa, pasa la bateria de vecinas por la 3060 simulada, la
> CPU, la casa y el calculo, y al correr la corre la CPU de reserva. La
> primera DENSIDAD de un fluido ya esta en el banco
> ([`nivel11/densidad`](../../toolchain/lang/titan/ejemplos/nivel11/densidad/src/main.titan)):
> cada gota suma sus nueve vecinas con el nucleo (1 - d^2)^3. IL1 queda
> HECHA; en la 3060 de verdad espera a LB8 (la puerta de computo).
>
> **18-10, IL2:** la REJILLA, sin atomicos ni palabras nuevas: `piso(x)`
> (los bits de `f32::floor`) da la celda de cada gota y el bit de cada
> hilo; las gotas se ordenan por celda con un orden BITONICO -- log2(n) por
> (log2(n) + 1) / 2 pasos, cada uno dos gpu fn con su pareja --, cada celda
> encuentra donde empiezan las suyas con una busqueda binaria de 24 vueltas
> (hasta 16 millones de gotas), y una gota mira solo sus nueve celdas
> ([`nivel11/rejilla`](../../toolchain/lang/titan/ejemplos/nivel11/rejilla/src/main.titan):
> 128 gotas, las mismas 332 vecinas que mirando a todas, y Python dice lo
> mismo). Lo que cuesta, dicho: el orden es n log2(n)^2 / 2 comparaciones
> (con atomicos, un conteo seria n); cada UAV ata cinco registros de la
> 3060. IL3 (PBF) ya tiene donde apoyarse.
>
> **18-10, IL3:** la SANGRE, en 2D
> ([`nivel11/sangre`](../../toolchain/lang/titan/ejemplos/nivel11/sangre/src/main.titan)):
> una presa de 8 x 8 gotas cae en una caja de 16 x 16 durante 40 pasos, con
> todo lo de Macklin y Muller -- la densidad (poly6), la lambda, la correccion
> con la presion artificial (la tension de la superficie), la vorticidad y la
> viscosidad de XSPH --, cada parte una gpu fn sobre la rejilla de IL2, y los
> totales por `casa.suma`. Conserva su densidad (la media 0.915, la mayor
> 1.154) y se abre hasta 13.78; un espejo en Python da lo mismo salvo el
> orden de las sumas. La raiz va sin dividir (`mates.inv_raiz`), con los
> mismos bits en cada tarjeta: la de la 3060 sigue siendo MUFU, y hacerla
> exacta es del propietario (DL10). Lo que falta, dicho: 3D, mas gotas (el
> banco tarda: cada paso son unas 70 gpu fn por cuatro respuestas) y la 3060
> de verdad (LB8).

- [ ] IL0 -- las decisiones de la seccion 8, contestadas por el propietario
- [x] IL1 -- (18-10, en el anfitrion: los bucles por LB5, los vecinos por LB9) `gpu fn` CON VECINOS: leer celdas de OTRA tabla por indice y bucles acotados, en el lenguaje, el oraculo y el emisor de SPIR-V (el que escribio G2 de `PLAN_EL_CENTAURO.md`, hoy en linea recta); y que el emisor SM86 los sepa traducir (E6 de `PLAN_LA_LENGUA_DE_LA_3060.md`)
- [x] IL2 -- (18-10, en el anfitrion: `nivel11/rejilla`, un orden bitonico y una busqueda binaria, sin atomicos) LA REJILLA DE VECINOS: cada particula a su celda, ordenadas por celda (un conteo y un orden, en `gpu fn`), para que cada una encuentre a las de al lado sin mirar a todas
- [x] IL3 -- (18-10, en el anfitrion y en 2D: `nivel11/sangre`, 64 gotas, la presa cae y conserva su densidad) PBF, la sangre: densidad, la correccion de posicion, la presion artificial (la tension superficial), la vorticidad y la viscosidad (Macklin y Muller 2013). La prueba, en el anfitrion: una presa que cae conserva su densidad
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

# 6d. HACIA HOUDINI, Y LA CARNE DE VERDAD (15-10)

> El propietario, 15-10: *"me gustaria que mi ILLAPA ENCARE a Houdini por
> completo, CAPAZ de generar todo iguales en lo que es basicamente, que
> tengan mismo render al estilo RENDERMAN [...] tengo curiosidad TAN grande
> en ver la carne REAL en ILLAPA con mi VERRANO"*.

**Lo honesto primero.** Houdini son tres decadas de SideFX; "por completo" no es
una meta que se pueda tachar. Lo que SI se puede: que lo BASICO de cada
red de Houdini tenga su igual en ILLAPA, y que cada uno se pruebe contra
una escena que en Houdini da un resultado conocido. Lo que ILLAPA tiene y
Houdini no: la imagen comprobada bit a bit (el juez de la CPU) y la misma
maquina de la app al metal.

Lo que hace a Houdini ser Houdini, y donde vive aqui:

```text
   HOUDINI                    QUE ES                          EN ILLAPA
   SOPs (geometria por        cada paso es un nodo; cambiar   HO1 (nuevo)
   nodos)                     uno rehace los de abajo
   atributos (P, N, Cd...)    datos pegados a cada punto      HO1: laminas por atributo
   VEX / wrangle              un trozo de codigo por punto,   `gpu fn` de TITAN++: ya
                              en paralelo                     existe; HO2 la pega a HO1
   Vellum (tela, pelo,        XPBD sobre particulas           IL4
   blando)
   FLIP (liquido de cine)     particulas + rejilla            IL13
   Pyro (humo y fuego)        rejilla de volumen              IL12
   RBD (romper cosas)         fractura de Voronoi + rigidos   HO3 (nuevo)
   FEM / musculos (la         solidos de tetraedros,          HO4, HO5 (nuevo)
   CARNE)                     musculos que se contraen,
                              grasa y piel que se deslizan
   Karma / RenderMan          trazador de rayos fisico        IL14 y LZ1..LZ9
   USD / Solaris              la escena como capas            IL9 (ESTRATOS)
```

**La carne REAL** sale de juntar tres peldanios: la forma que se mueve por
dentro (HO4 y HO5: hueso, musculo, grasa, piel), la luz que entra en la piel
(LZ5, la dispersion bajo la superficie) y la imagen final por rayos (IL14).
Ninguno solo basta: una simulacion buena con luz de plastico parece goma.

## Su escalera (despues de IL11; HO1 y HO2 pueden adelantarse)

- [ ] HO1 -- GEOMETRIA POR NODOS: una red de nodos (crear, transformar, copiar sobre puntos, unir) donde cada nodo lee y escribe atributos por punto en laminas; cambiar un nodo rehace solo los de abajo. **Como se sabe:** la misma red, rehecha entera y por partes, da los mismos bytes
- [ ] HO2 -- EL WRANGLE: una `gpu fn` de TITAN++ como nodo de HO1, corrida en la 3060 sobre cada punto y juzgada por la CPU
- [ ] HO3 -- ROMPER: fractura de Voronoi y los trozos como rigidos del solucionador de IL4
- [ ] HO4 -- LA CARNE: solidos de tetraedros con elementos finitos (el modelo neo-hookeano), estables con pasos grandes. **Como se sabe:** una viga que cae se dobla igual que la formula y conserva su volumen
- [ ] HO5 -- LOS MUSCULOS: fibras que se contraen con una orden, pegadas al hueso del glTF (IL6), y la grasa y la piel que se deslizan encima. La prueba visible: un brazo que dobla el codo y el biceps que se hincha
- [ ] HO6 -- LA PRUEBA CONTRA HOUDINI: cinco escenas simples (una tela que cae, una presa de agua, un cubo que se rompe, una viga blanda, un brazo) hechas aqui y en Houdini Apprentice (en Windows), comparadas a ojo y por numeros (volumen, energia, tiempo); se anota cual gana en que

**Lo que pide de abajo:** `f32` en la 3060 ya lo hay; HO4 pide resolver
sistemas grandes en la 3060 (gradiente conjugado: muchas `gpu fn` seguidas
y una suma por bloques) y, por eso, IL1 y IL2 primero.

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
