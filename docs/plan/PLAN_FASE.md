# PLAN FASE -- el escritorio que se TRANSFORMA

> Abierto el **2026-10-04** a peticion del propietario: *"elegante pero
> transformers e intimidante"*, *"que TODOS tengan animacion, hasta si lo
> cierras"*, y despues: *"con color de acuerdo al gato que tengo, pero con
> glitch"*. Con su esquema de tres modos y su vocabulario.

---

# 0. LA IDEA EN UNA LINEA

**El escritorio no cambia de tema: CAMBIA DE FORMA.** Cada modo es un chasis
distinto, y pasar de uno a otro es verlo armarse -- placas que llegan, se
encajan y se encienden --, no un fundido. En reposo no se mueve nada que no
diga algo (`[consumo]` como siempre: se anima por EVENTO).

# 1. EL VOCABULARIO (el del propietario, y es el de la industria)

```text
   FUI                 Futuristic / Fantasy User Interface: alta densidad de
                       informacion, vectores en movimiento, graficos
                       circulares, telemetria, paneles que se reconfiguran
                       (Iron Man, Jarvis, SpaceX, las salas de la NASA)
   HUD sci-fi /        nada esta quieto si mide algo: trazos vectoriales,
   data-dense UI       barras de carga dinamicas, graficos de frecuencia,
                       contadores en tiempo real
   esqueumorfismo      la interfaz imita piezas FISICAS: actuadores, placas
   mecanico            metalicas, hidraulica que se ensambla y desensambla
```

La regla de BMO-X encima: **todo lo que se ve es VERDAD.** Los contadores son
del hierro (APERF/MPERF, los vatios del paquete, la RAM, la red), nunca
numeros de adorno. Un FUI de pelicula miente; este no.

# 2. LOS COLORES: los del GATO, con GLITCH

El gato del fondo es un holograma CIAN en una ciudad de neon MAGENTA. FASE
toma esos dos (`tema.maqueta`, `.fase`): las placas casi negras con filo de
holograma, el acento cian, y el glitch es el magenta corrido contra el cian
(dos fantasmas desplazados que convergen al encajar cada pieza).

# 3. LOS TRES MODOS (el esquema del propietario)

```text
   [ MODO ESCRITORIO / VITALES ]   paneles rectangulares, tablas de procesos
            |                      e hilos (lo de hoy: `marco = fino`)
            v  animacion en tiempo real
   [ MODO CABINA / DEV MODE ]      los paneles se repliegan como placas
            |                      blindadas; se despliega la matriz de nodos
            v  cambio de chasis    y graficos (`marco = fase`)
   [ MODO JUEGO / PLAY MODE ]      el chasis mecanico se CIERRA y deja la
                                   pantalla 100 % limpia para el bare-metal
```

# 4. LA ESCALERA

- [x] **F1 -- EL MODO FASE EXISTE.** `marco = fase` en `sys/director.cfg` (y en
  el panel de aspecto: fino, hacker, fase). El acento, el panel y los marcos de
  las ventanas pasan a los colores del gato; lo configurado no se mezcla con lo
  visto (`scene/estilo.rs`: `configurado()` y `estilo()`).
  `platform/shared/bmo-config`, `Ultra_userspace/services/director/src/scene/estilo.rs`.
- [x] **F2 -- LA SEGUNDA BARRA.** A la derecha, la barra TACTICA: seis placas
  con lo que el hierro dice (GHz por APERF/MPERF propios, vatios del paquete,
  memoria, programas vivos, red, tiempo encendido). Columna RESERVADA: las
  ventanas no entran (`chrome::area_util`).
  `Ultra_userspace/services/director/src/scene/tactico.rs`.
- [x] **F3 -- TRANSFORMARSE.** Al entrar: la espina cian baja, las placas
  llegan volando una tras otra con rebote y glitch, aterrizan, el filo arde y
  el valor arranca parpadeando. ~1,3 s, y en reposo NADA. Visto en el
  anfitrion fotograma a fotograma.
  [!] No se ha visto en el Ryzen.
- [ ] **F4 -- REPLEGARSE.** Al salir de FASE, las placas se van (hoy la
  columna desaparece de golpe): la columna sigue reservada hasta que acaba la
  animacion, y entonces se devuelve a las ventanas.
  `Ultra_userspace/services/director/src/scene/tactico.rs`.
- [ ] **F5 -- EL PANEL IZQUIERDO TAMBIEN SE ARMA.** Sus secciones como placas
  que se repliegan y vuelven en los colores del gato, no un repintado.
  `Ultra_userspace/services/director/src/scene/lateral.rs`.
- [ ] **F6 -- LO DATA-DENSE.** Graficos circulares (la carga por nucleo, los
  vatios como un dial), trazos vectoriales y graficos de frecuencia en las
  placas: lo que ya mide el panel, dibujado como FUI.
  `Ultra_userspace/services/director/src/scene/tactico.rs`.
- [ ] **F7 -- LAS VENTANAS SE ABREN Y SE CIERRAN COMO PIEZAS.** Cada ventana con
  su entrada y su salida unica, escrita en su `.maqueta` (`@estado cerrada` y
  `transition` con retraso por pieza) y `transform` por estado en MAQUETA.
  `toolchain/tools/maqueta/emit/src/rust.rs`.
- [ ] **F8 -- MODO JUEGO.** Al pasar un juego a pantalla completa, las dos
  barras se CIERRAN como un chasis (las placas se repliegan hacia los bordes)
  y la pantalla queda limpia; al salir, se vuelve a armar.
  `Ultra_userspace/services/director/src/desktop/`.

# 5. LA PETICION ENTERA, ANOTADA (para aplicarla en MAQUETA)

El propietario: *"anotar por completo eso todo, para aplicar luego en
maqueta TODO, esa peticion exagerada"*. Esto es TODO lo que pidio entre el
04-10 y hoy, con sus palabras, para que nada se pierda entre una sesion y
otra:

```text
   "que TODOS tengan vida y epico, y la barra lateral con animaciones
    especiales y unicas"                       -> el escritorio con vida (hecho:
                                                  la barra, Alt+Tab, la rejilla)
   "TODOS tienen animacion, hasta inclusive si lo cierras con maqueta: algo
    especial, como estilo transformers, que se abre y se cierra UNICO en
    cada uno"                                  -> seccion 7
   "vi una pagina con la interfaz TODA ROJA, todo tiene animacion"
                                               -> el primer color de FASE; el
                                                  propietario lo cambio despues
   "mi BMO-X puede tener sus 2 barras laterales, con interfaz elegante pero
    transformers e intimidante"                -> F2 (hecho)
   "con color de acuerdo al gato que tengo, pero con glitch"
                                               -> seccion 2 (hecho)
   su esquema: ESCRITORIO -> CABINA -> JUEGO, "animacion en real-time
    (VERRANO)", "cambio sub-milisegundo"       -> seccion 3, F8 y seccion 8
   su vocabulario: FUI, HUD data-dense, esqueumorfismo mecanico
                                               -> seccion 1
   "que mi CSS propio maqueta sea mejor, como NEXT.JS"
                                               -> `PLAN_MAQUETA.md` 6c-6h
```

La regla que lo ordena todo: **la forma de cada animacion se escribe en el
`.maqueta`** (que se juzga al compilar y se ve en la foto del anfitrion y en
el navegador), y el aparato solo MEZCLA piezas ya calculadas. Nada de esto
mete un motor de animacion en Ring 3.

# 6. LO QUE MAQUETA TIENE QUE APRENDER

Cada escalon con el CSS que lo inspira (o la razon de que no haya CSS), y
donde iria. Ninguno se hace todavia: esto es el papel.

- [ ] **M1 -- `transform` POR ESTADO.** `translate(x, y)` y `scale(s)` como
  PINTURA: no mueven la maquetacion (como en CSS), asi que una pieza puede
  salir volando sin descolocar a sus vecinas. Es lo que hace falta para que
  una ventana se "arme". `rotate` despues, si se paga.
  `toolchain/tools/maqueta/node/src/value.rs`, `toolchain/tools/maqueta/emit/src/movimiento.rs`.
- [ ] **M2 -- ENTRAR Y SALIR.** Dos estados con nombre fijo, `@estado
  cerrada` y el reposo: el escritorio pinta `cerrada -> reposo` al abrir una
  ventana y `reposo -> cerrada` al cerrarla o minimizarla (el CSS de hoy lo
  llama `@starting-style`).
  `toolchain/tools/maqueta/emit/src/rust.rs`, `Ultra_userspace/services/director/src/desktop/`.
- [ ] **M3 -- EL ESCALON.** `escalon: 60ms` en un padre reparte un retraso
  creciente entre sus hijos (lo que en CSS se escribe a mano con
  `transition-delay` y `:nth-child`): las placas llegan UNA TRAS OTRA.
  `toolchain/tools/maqueta/cascade/src/style.rs`.
- [ ] **M4 -- EL GLITCH COMO PROPIEDAD.** `glitch: 3px` en una caja: mientras
  dura su transicion, dos fantasmas de su borde (el magenta y el cian de la
  paleta) corridos y parpadeando, que convergen al acabar. Es lo de
  `scene/tactico.rs`, pero dicho en la maqueta.
  `toolchain/tools/maqueta/emit/src/movimiento.rs`, `platform/shared/bmo-pinta/src/lib.rs`.
- [ ] **M5 -- LAS ESQUINAS CORTADAS.** `corner-shape: bevel` (CSS 4): la placa
  blindada, con el chaflan de las piezas de metal en vez de la curva.
  `platform/shared/bmo-pinta/src/lib.rs`.
- [ ] **M6 -- LOS DATOS SE MUEVEN.** Un hueco de datos con `transition`: cuando
  cambia el numero, no salta -- RUEDA (los digitos suben, como el reloj del
  panel), y una barra se llena hasta el valor nuevo. Es el "contador en tiempo
  real" del HUD, sin bucle: se anima cuando el dato cambia.
  `toolchain/tools/maqueta/emit/src/rust.rs`.
- [ ] **M7 -- EL DIAL.** Un arco que llena un dato (`<arco dato="carga"
  desde="-120" hasta="120"/>`): los graficos circulares del FUI, con la pluma
  de la casa y el `stroke-dasharray` de SVG como espejo en el navegador.
  `toolchain/tools/maqueta/node/src/markup.rs`, `platform/shared/bmo-pinta/src/lib.rs`.
- [ ] **M8 -- LA GRAFICA.** `<grafica dato="historia"/>`: una polilinea de un
  dato que es una lista (la frecuencia, los vatios, la carga por nucleo), el
  "trazado vectorial" del HUD.
  `toolchain/tools/maqueta/node/src/markup.rs`, `toolchain/tools/maqueta/emit/src/rust.rs`.
- [ ] **M9 -- LA PLACA QUE SE PARTE.** Una caja dicha como N placas que, en
  `@estado cerrada`, se separan hacia sus bordes (con M1 y M3): la hidraulica
  del esqueumorfismo mecanico, sin escribir cada placa a mano.
  `toolchain/tools/maqueta/compone/src/lib.rs`.
- [ ] **M10 -- LOS MODOS COMO TEMA.** `@modo cabina { :root { --acento: ... } }`:
  un modo cambia las variables de `:root`, y el cambio de modo es una
  transicion de color en todo lo que las usa (lo que hoy hace a mano
  `estilo::visto`).
  `toolchain/tools/maqueta/node/src/variables.rs`.
- [ ] **M11 -- LAS BARRAS EN MAQUETA.** El panel izquierdo y la barra tactica
  escritos como `.maqueta` con sus listas, sus datos, sus estados y su glitch:
  `scene/lateral.rs` y `scene/tactico.rs` dejan de dibujar a mano.
  `Ultra_userspace/services/director/src/scene/tactico.rs`.
- [ ] **M12 -- EL PRESUPUESTO EN EL VEREDICTO.** El compilador calcula cuantos
  pixeles pinta cada fotograma de una transicion, y una que pase del
  presupuesto (seccion 8) no compila, con el numero: "cambio sub-milisegundo"
  dicho como regla y no como deseo.
  `toolchain/tools/maqueta/verdict/src/lib.rs`.
- [ ] **M13 -- EL ESPEJO DE LO QUE SE MUEVE.** El navegador (`foto.js`) abre
  los estados, el `transform`, el escalon y el glitch, y el ESPEJO compara la
  tira del anfitrion con la del navegador fotograma a fotograma.
  `toolchain/tools/espejo-cara/foto.js`.
- [ ] **M14 -- LA ANIMACION EN LA GPU.** Lo de 6f de `PLAN_MAQUETA.md`: las
  mismas piezas mezcladas en la 3060 (VERRANO) cuando la 3060 pinta el
  escritorio; el CPU solo dice `k`.
  `docs/plan/PLAN_MAQUETA.md`.

# 7. CADA VENTANA, SU TRANSFORMACION

*"Unico en cada uno."* Una propuesta por ventana, para escribirla en su
`.maqueta` con M1-M4 y M9; el propietario elige o cambia.

```text
   VITALES     las filas de procesos se despliegan como una persiana, de
               arriba abajo, y los numeros ruedan hasta su valor
   CABINA      se repliega en placas blindadas y se despliega la matriz de
               nodos y graficos (el MODO CABINA del esquema)
   ESTRATOS    las capas llegan apiladas desde el fondo, una sobre otra
   HERMES      las alas del logo se abren y arrastran el panel con ellas
   LUDOTECA    las puertas de la maquina recreativa se abren hacia los lados
   BANK CAT    la puerta de la caja fuerte gira y la tarjeta sale de dentro
   TALLER      la rejilla del plano se dibuja linea a linea y las piezas caen
               en su sitio
   Ejecutar    la linea de comandos se enciende como un CRT: una raya y luego
               el cuadro
   DOOM y      la pantalla se cierra en una raya horizontal, como un tubo que
   los juegos  se apaga (el MODO JUEGO del esquema, F8)
   al CERRAR   lo mismo hacia atras, con su glitch, mas rapido que al abrir
```

# 8. EL PRESUPUESTO (lo que cuesta, medido y no supuesto)

- En reposo: **NADA**. Todo se anima por EVENTO (abrir, cerrar, cambiar de
  modo, cambiar un dato); ningun bucle de animacion vive en reposo (L6h).
- Una animacion son ~15-20 fotogramas (~300 ms); cada uno cuesta lo que
  repintar la caja que se mueve -- en el anfitrion, una tarjeta con muchas
  piezas son ~0,5 ms (`PLAN_MAQUETA.md` 6e).
- La meta del propietario, "cambio sub-milisegundo", es por fotograma: lo
  vigila M12 en el compilador, y se MIDE en el Ryzen antes de darlo por bueno.

[!] Nada de esto se ha visto en el Ryzen todavia.
