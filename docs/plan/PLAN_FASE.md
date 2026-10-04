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

[!] Nada de esto se ha visto en el Ryzen todavia.
