# PLAN EL HUD -- el escritorio como Hyprland, con UN motivo por pieza

> Abierto el **2026-09-22** a peticion del propietario: *"mejorar todo el
> DIRECTOR, el escritorio HUD como Hyprland... elegante... la barra lateral...
> interfaz en tiempo real"*, y la condicion que ordena todo lo de abajo:
> *"TODO MEZCLADO pero UN MOTIVO, UN MOTIVO; practico, simple y adictivo, ULTRA
> SUPER COMODO"*.
>
> Estudio previo: el de ese mismo dia sobre Windows, Mac y Linux desde el
> origen (en el chat; resumen en la memoria del proyecto). De ahi salen la
> tecla del gestor (Super, y luego Ctrl), Fitts y el borde de foco.

---

# 0. LA REGLA DEL PLAN: una pieza, un motivo

Cada pieza existe por UNA razon que se dice en una linea. Si una pieza no
cabe en una linea, no es una pieza: son dos, o es decoracion.

```text
   pieza                    el motivo
   H1  la tecla Ctrl        el gestor tiene tecla PROPIA y no le quita ninguna a nadie
   H2  borde + huecos       ver de un vistazo a DONDE van las teclas
   H3  la barra lateral     lo que hace la maquina, SIN abrir nada
   H4  el mosaico           ninguna ventana TAPA a otra, y no se ordena a mano
   H5  el panel             todo lo que no es una ventana, en UN solo sitio
```

**Lo que NO entra, y por que** (el precio de lo que Hyprland hace con GPU):

| no | por que |
|---|---|
| desenfoque, transparencia | el alfa aqui es de 1 bit y no hay GPU; el desenfoque de un degradado es el degradado |
| animaciones | cada fotograma animado es latencia; aqui se pinta y se vuelca en la misma vuelta |
| escritorios numerados 1..5 | ya rechazado con motivo en `scene/barra.rs`: cinco numeros que no hacen nada |

---

# 1. LAS PIEZAS

## [ ] H1 -- LA TECLA DEL GESTOR: CTRL (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** el gestor tiene tecla propia y no le quita ninguna a nadie.

Fue **Super** una tarde (`e866a4a3`). Tras el metal de las 16:56 el
propietario la cambio: *"reemplaza con control + (cualquier atajo) porque
BMO-X va a vivir como el estilo de Windows"*. Ctrl ya era del escritorio
(`keys/app.rs::del_escritorio` no se lo da a ninguna app), asi que el motivo
sigue en pie: no se le quita una tecla a nadie. Todo Alt era del escritorio, y
eso tenia un precio escrito: a DOOM no le llegaba el ladeo (Alt+flechas).

```text
   Ctrl + flechas           encajar: mitad izquierda / derecha, arriba maximiza,
                            abajo deshace (el Win+flechas de Windows 7)
   Ctrl + Shift + flechas   mover 24 px
   Ctrl + Q                 cerrar lo de delante (la X y Alt+F4: UN cierre)
   Ctrl + Enter             Ejecutar, delante y con el teclado
   Ctrl + F                 pantalla completa (lo mismo que Alt+Enter)
   Ctrl + Tab               el modo del foco (Alt+Tab elige ventana)
   Ctrl + B / Ctrl + T      barra lateral / mosaico
   Alt                      es de la APP, menos Alt+Tab, Alt+F4 y Alt+Enter
```

**Los choques, y como se resolvieron** (el kernel cuece Ctrl+letra en su
codigo de control, `keyboard::feed_full`, asi que se compara con el codigo):

| choque | que se hizo |
|---|---|
| Ctrl+Alt ES AltGr (`@ # [ ]`) | los atajos exigen Ctrl SIN Alt; el toque de Ctrl+Alt sigue escondiendo Ejecutar |
| Ctrl+M es el byte de Enter, Ctrl+I el de Tab | el modo del foco va con Ctrl+Tab, no con M |
| Ctrl+W ya borra una palabra en Ejecutar | cerrar es Ctrl+Q |
| Ctrl+C frena la corrida / limpia la linea | se queda; COPIAR pasa a Ctrl+Shift+C (el de Windows Terminal) |
| Ctrl+arriba/abajo copiaban y pegaban | ahora encajan; pegar sigue en Ctrl+V |

Con Ctrl (sin Alt) un caracter imprimible no se escribe: un Ctrl+1 sin atajo
se tira, para que el dia que lo tenga no haya escrito un `1` antes. Super
vuelve a ser de la app.

| que | afirma | como se cae |
|---|---|---|
| Ctrl+flechas con Datos delante | encaja a una mitad, con huecos | no se mueve: la flecha no llega con `MOD_CTRL` |
| DOOM en ventana, Alt+flechas | DOOM ladea | se mueve la ventana: la regla de `keys/app.rs` no cambio |
| Ctrl+Q con una app delante | se cierra | no hace nada: no llego el 0x11 |
| escribir `@` con AltGr y con Ctrl+Alt | sale la arroba | salta un atajo: la guarda de Alt no esta |
| Ctrl+Shift+C y luego Ctrl+V en Ejecutar | la linea se duplica | limpia la linea: el Shift no se mira |

## [ ] H2 -- EL BORDE DE FOCO Y LOS HUECOS (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ver de un vistazo a donde van las teclas.

* El marco de la ventana con foco lleva el borde del ACENTO; las demas, el
  color de su ventana (que dice CUAL es). El `col.active_border` de Hyprland.
  Lo pone UN sitio, `desktop/foco.rs`, cuando el foco cambia, y repinta las del
  sistema de la mas vieja a la de delante.
* HUECOS de 8 px (`scene/chrome.rs::HUECO`): encajar y maximizar miden en
  `area_util`, la pantalla menos la barra y menos los huecos. Una ventana
  pegada a otra se lee como una; con aire, dos.
* Y como se pinta cada ventana vive en UN sitio (`paint::pintar_ventana`): era
  un cierre dentro de `keys::edges`, y el borde era la segunda copia.

| que | afirma | como se cae |
|---|---|---|
| clic en CABINA con Datos abierta | CABINA con borde azul, Datos con el suyo | los dos azules: `seguir` no corre |
| Ctrl+izquierda y Ctrl+derecha en dos ventanas | 8 px entre ellas y con los bordes | pegadas: `snap` no mide en `area_util` |

## [ ] H3 -- LA BARRA LATERAL EN VIVO (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ver lo que hace la maquina, sin abrir nada.

> Esa misma tarde crecio: la barra de arriba se fundio en ella (H5), y las
> fichas, la luz y el reloj viven ahora aqui. Lo de abajo es como nacio.

Al escribirla el motivo se afino: "lo abierto" ya lo dicen las fichas de
arriba, y ponerlo otra vez aqui era dos sitios para lo mismo. La barra es el
HUD EN TIEMPO REAL: una columna de 112 px con cinco instrumentos --cpu,
memoria, vatios, pulso (vueltas del escritorio) y sonido (el medidor del
maestro)--, cada uno con su cifra y su grafica de los ultimos 11 s (44
muestras a 4 por segundo). Ctrl+B la esconde.

* `scene/lateral.rs`: su caja, su color para `scene_color`, la historia y el
  pintado. Las mismas cuentas que la barra de arriba.
* La columna es RESERVADA (la `exclusive zone` de Hyprland): `area_util`, los
  topes del arrastre, de las flechas y de `fit`, y la rejilla de iconos leen
  `lateral::margen()`. Por eso su repintado de 4 Hz nunca pinta encima de una
  ventana.

| que | afirma | como se cae |
|---|---|---|
| arrancar el escritorio | la columna a la izquierda, con las cinco graficas moviendose | vacia: `latido` no corre o `will_paint` no llega |
| DOOM sonando | la grafica de sonido sube en verde/ambar | quieta: el medidor del maestro no se lee |
| arrastrar una ventana a la izquierda | se para en el borde de la columna | la tapa: un tope no lee `margen()` |
| Ctrl+B | se va, y la rejilla y las ventanas ocupan su sitio | queda un trozo pintado: `repintar_escritorio` no la borra |

## [ ] H4 -- EL MOSAICO (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ninguna ventana tapa a otra, y no se ordena a mano.

Ctrl+T lo enciende y lo apaga (y lo dice en la linea de estado). Al
escribirlo se eligio MAESTRO Y PILA (dwm, el `master` de Hyprland) y no
`dwindle`: es el que se predice sin mirar. Una ventana: el area util entera;
dos o mas: la primera a la izquierda, las demas apiladas a la derecha, con
huecos. La primera es la app si hay una, si no Ejecutar. Solo recoloca
cuando CAMBIA que ventanas hay (`desktop/mosaico.rs`), asi que arrastrar una
no se pelea con la mano.

| que | afirma | como se cae |
|---|---|---|
| Ctrl+T con Ejecutar y Datos abiertas | Ejecutar a la izquierda, Datos a la derecha, sin taparse | nada se mueve: `seguir` no corre o la firma no cambia |
| abrir CABINA (F11) con el mosaico puesto | la pila de la derecha se parte en dos | se abre encima: la firma no ve la ventana nueva |
| con DOOM en ventana | DOOM a la izquierda, lo demas apilado | DOOM no se mueve: su marco no se coloca |
| lo que se dice | una ventana con minimo mayor que su hueco (Ejecutar) asoma: es a proposito | -- |

## [ ] H5 -- EL PANEL: LA BARRA DE ARRIBA SE FUNDE EN LA LATERAL (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** todo lo que no es una ventana, en un solo sitio.

El propietario, tras el metal de las 16:56: *"la barra de arriba vamos a
quitar y mejorar eso, para que sea mucho mas elegante... ya cumplio su
parte"*. De tres formas propuestas (fundir en la lateral, isla centrada, tira
de 28 px) eligio **fundir**. El motivo de fondo: eran dos barras para lo
mismo --cpu, memoria y vatios salian en las dos-- y la de arriba se habia
llenado de instrumentos de los dias de cazar averias (~1.100 px de numeros).

```text
   arriba del panel   la marca BMO-X
                      las fichas, en vertical (Ejecutar, ESTRATOS, CABINA, apps)
                      (aire: las fichas crecen hacia abajo)
   abajo del panel    la luz del bus (el testigo)
                      cpu / memoria / vatios / pulso CON SU AGUJA / sonido
                      el reloj y el dia; el vol (clic = el maestro)
   a CABINA           el reparto del pulso, el volcado y la entrada, en una
                      linea encima del pie, 4 Hz y SOLO con CABINA delante
```

* `scene/barra.rs` se borro; `TASKBAR_H` tambien. Las ventanas ganan 40 px de
  alto: `area_util`, los topes y el centrado de una ventana nueva miden desde
  arriba y a la DERECHA del panel. La rejilla de iconos empieza en `y = 24`.
* El panel mide 160 px (la grafica pasa a 68 muestras, 17 s) y sigue siendo
  columna RESERVADA. Las opciones de la barra en `sys/director.cfg` son ahora
  las suyas: `barra_flotante`, `barra_hueco`, y `cpu`/`memoria`/`vatios`/`reloj`
  encienden cada instrumento (el pulso y el sonido no se apagan).
* **Escondido (Ctrl+B) queda una TIRA de 6 px con la luz del bus, y un clic
  lo trae.** La ficha de CABINA estaba siempre en la barra porque *"un panel de
  diagnostico al que solo se llega con el aparato que puede estar roto no es un
  panel de diagnostico"*: esconder el panel no puede dejar al raton sin camino.
* Cambiar el panel en el editor de aspecto recoloca las ventanas por el mismo
  camino que Ctrl+B (`desktop::lateral_cambio`).
* De paso: el numero del testigo (`REPARADO x3`) se ponia en `tx + ancho`, y
  `texto` devuelve donde ACABA: caia el doble de lejos.

| que | afirma | como se cae |
|---|---|---|
| arrancar el escritorio | sin barra arriba; el panel con la marca, Ejecutar y CABINA, la luz verde, las graficas, la hora y el vol | una tira oscura arriba: algo sigue pintando `TASKBAR` |
| abrir DOOM y minimizarlo | su ficha aparece en el panel, apagada; un clic lo trae | no vuelve: `ficha_en` no casa con la fila pintada |
| clic en el vol | el maestro se abre al lado del panel, con el pie a la altura del vol | se abre arriba a la derecha: `junto_a_la_barra` no se cambio |
| Ctrl+B y clic en la tira | se va dejando la luz; el clic lo trae y las ventanas se corren | la tira no contesta: `en_la_tira` no se mira |
| CABINA delante | una linea `latido .../s pinta .. cuerpo .. puerta ..  volcado ..  entrada ..` encima del pie | vacia: `instrumentos` no corre o no cabe en el ancho |
| la aguja del pulso | gira cuatro veces por segundo | quieta: `dictamen` no se llama o no gira |

## [ ] H6 -- ESTRATOS ABRE EN "ESTE EQUIPO": LAS UNIDADES EN TARJETAS (2026-09-29)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** que discos hay y cuanto les queda, de un vistazo y todos juntos.

El propietario, con la captura de `Este equipo` de su Windows delante:
*"que ESTRATOS cambie en apariencia asi, me gusta eso ... en ESTRATOS se ve
como basico ... al estilo hyprland"*. La ventana del F12 abre ahora en la
solapa `equipo` (antes de `numeros`): una tarjeta por unidad --BMO (A:),
Personal (D:), ESTRATOS (F:) y EFI-- con el disco, su sistema, la barra de lo
usado (azul; roja pasado el 90%) y "X GB disponibles de Y GB". La elegida
lleva el borde en DEGRADADO, el `col.active_border` de Hyprland; las tarjetas,
aire entre ellas y esquinas suavizadas. `C:` no sale: BMO-X ni lo mira.

* Lo libre es MEDIDO en el mapa de cada volumen, no leido de una pista:
  `bmo_fat32::FatVolume::libres` cuenta la FAT (no se fia del `FSInfo`, que
  este driver no actualiza) y `bmo_ntfs::Volumen::libres` el `$Bitmap` (una
  vez, al montar el disco Personal). Llega a Ring 3 por `INFO_UNIDAD` (0xC4).
  Los dos tienen prueba: la FAT de juguete y los discos de `mkntfs`, contra lo
  que dice `ntfsinfo`.
* Se mide al abrir la ventana, al volver a la solapa y con `R`; pintar solo
  mira lo medido. ENTRAR (o doble clic) explora la unidad; en Personal (D:)
  dice que explorar llega con N1b.
* Ver `Ultra_userspace/services/director/src/scene/data/equipo.rs`.
* **Los iconos, VECTORIALES** (el mismo dia; el propietario: *"investigar
  los mejores iconos y eso en svg ... porque se ven feo"*). Los de 16 px
  agrandados eran cuadros de 2x2. Ahora son FORMAS --cajas redondeadas,
  circulos, poligonos, trazos-- sobre la rejilla de 24 que usan Fluent (Windows
  11) y Lucide, rasterizadas a la medida con 4x4 muestras y en degradado:
  `bmo_dibujo::icono` (8 pruebas en el anfitrion). Un disco para A: y D:, tres
  capas para ESTRATOS, un chip para EFI y un candado ambar encima de lo que es
  de solo lectura; y la carpeta y la hoja del explorador. Los dibujos son de
  BMO-X (`scene/dibujos.rs`): de esos juegos se tomaron las reglas, no los
  trazados.

| que | afirma | como se cae |
|---|---|---|
| F12 | abre en `equipo` con cuatro tarjetas; A:, D: y F: con barra y "disponibles de" | `numeros` como siempre: la vista por defecto no cambio |
| la tarjeta de A: | lo libre cuadra con lo que dice Windows de BMO (A:) (30,3 GB de 31,9 el 29-09) | otra cifra: la cuenta de la FAT no salta el relleno |
| la tarjeta de D: | 26,5 GB de 111 GB y `solo lectura` | "no montada": N1a no monto el NTFS (la cabina dice por que) |
| flechas y ENTRAR sobre ESTRATOS | el explorador en ESTRATOS | no se mueve: las teclas no llegan a `eq_mover` |

---

# 2. LA CARA DE MISION -- todo BMO-X como el HUD de una nave (06-10)

El propietario, 06-10: *"puedes hacer que HUD sean todo como estilo de NASA
al viaje en otro planeta unicos? TODOS hasta el escritorio"*.

La maqueta es `docs/arte/maqueta_hud_nasa.html`, y se toca: una consola de
mando abajo (o las teclas F1..F12 de verdad) abre cada instrumento, y arriba
se elige la paleta. **Cada tecla es un instrumento DISTINTO**, y eso es la
regla de la seccion 0 dicha en otro idioma -- un instrumento, un motivo:

```text
   ESC  ESCRITORIO  la ventana de la nave llegando al planeta: reticula que
                    sigue al puntero, trayectoria, telemetria en las esquinas
   F1   TALLER      el plano de ensamblaje: nodos = modulos con puertos
   F2   ARCHIVOS    la bahia de carga: un compartimento por disco y su rango
   F3   HERMES      la red de espacio profundo: un amigo, una antena
   F4   LUDOTECA    el simulador: cada juego, un parche de mision
   F5   BANK CAT    el combustible: el saldo es lo que queda en el tanque
   F6   RED         el enlace de telemetria: la onda, el ping
   F7   CPU         la propulsion: los 12 nucleos son 12 motores
   F8   MEMORIA     el soporte vital: un tanque por titular de marcos
   F9   FALLO       la ALARMA MAESTRA y el panel de precaucion
   F10  SONIDO      el lazo de audio: los 8 canales del casco
   F11  CABINA      el registro de vuelo, con hora de mision
   F12  ESTRATOS    la trayectoria: versiones = encendidos, marcadas = puntos
                    de paso, ramas = trayectorias que se juntan en la MEZCLA
```

Las piezas son las MISMAS en todas (el marco de esquinas en L, la regla de
marcas, la lectura con su unidad, la barra), y por eso todo se ve de una
mision sin repetir un instrumento.

[!] **Esto CAMBIA una linea de la seccion 0**: *"animaciones: cada fotograma
animado es latencia"*. Sigue siendo verdad, y la forma de cumplirla es la del
globo del puntero: se anima SOLO mientras algo cambia (un motor que sube, la
nave que recorre la trayectoria), y quieto son CERO fotogramas. Y lo que se
anima son piezas YA maquetadas que el aparato mezcla (`@estado`, y la
`@secuencia` de `docs/plan/PLAN_MAQUETA_3.md`): en el aparato no se maqueta
nada.

### Las decisiones del propietario

**HD1, HD2, HD3 y HD4, DECIDIDAS el 06-10.** HD1 no fue ninguna de las tres paletas
que se ofrecieron: *"me imagine un planeta mi gato del logo pero que es
dominante, que es estrella, como fondo animado y presentable en escritorio;
cuando todo se configura normal en inicio y luego en escritorio por
completo"*. HD3: *"el escritorio primero"*. HD2: *"si, la letra de la
casa"*. HD4: *"me gustan el gato SOL de PLASMA, NEBULOSA, y lo otro que es
quieta con la CPU si no hace nada, pero con RTX 3060 12G ya con eso se
encargue, pero como siempre se aisle para estar ordenado"*.

```text
   HD1  la paleta       GATO: sale del LOGO (docs/arte/bmo-x-gato-hd.svg) --
                        el cian del ojo #5EF2E6, el magenta #FF2E88, el azul
                        #3DA5FF y el violeta de la ciudad del tema. APOLLO,
                        ARTEMIS y MARTE se quedan en la maqueta para comparar
        la ESTRELLA     el logo es la estrella DOMINANTE del sistema: el gato
                        dentro, la corona que gira despacio, el halo que
                        respira; un planeta la orbita. Es el fondo del
                        escritorio
        el INICIO       el arranque es la encuesta GO / NO-GO: cada sistema
                        dice GO cuando el kernel lo mide listo, mientras la
                        estrella se ENCIENDE; con todo en GO, el escritorio
   HD2  la letra        la de la casa (bmo-letra) en su peso de numeros; la
                        maqueta usa IBM Plex solo porque el navegador no tiene la
                        de la casa (eso es M2 de PLAN_MAQUETA_3)
   HD3  el orden        el ESCRITORIO primero
   HD4  la estrella     COMO es la estrella. El propietario pidio otros
                        estilos, "no tan literal", todos vivos y cada uno con
                        sus elementos: seis en docs/arte/maqueta_estrella_gato.html
                        ECLIPSE       el gato negro tapa su estrella; corona y
                                      anillo (Interstellar, los eclipses)
                        CONSTELACION  estrellas sobre su contorno que titilan
                                      (los mapas de Hevelius)
                        NEBULOSA      cientos de particulas dentro de su forma
                                      (Hubble y Webb) -- la 3060
                        HOLOGRAMA     lineas de luz, la barra que escanea, el
                                      parpadeo del proyector (Leia)
                        ORBITAS       la cara y las orejas son orbitas con su
                                      satelite (Kepler)
                        SOL DE PLASMA granulos que hierven y arcos que saltan
                                      (el SDO de la NASA) -- la 3060
                        ELEGIDAS: SOL DE PLASMA y NEBULOSA. Conviven asi (la
                        maqueta lo propone y se ve en el escritorio entero):
                        la NEBULOSA es el INICIO -- se junta en la forma del
                        gato y se condensa en el SOL, que es el escritorio.
                        Una estrella nace de una nebulosa; la de BMO-X tambien.
                        Quien quiera la nebulosa en el escritorio la elige
                        el motor  con la 3060 la estrella esta VIVA; sin ella (o sin
                        el sobre de su tarjeta), la CPU pinta UN fotograma y
                        la deja QUIETA. En reposo, cero fotogramas
                        aislada  la estrella viva es un programa de VERRANO en
                        su PROPIA pieza: entra como SPIR-V, la 3060 lo juzga
                        y lo sube por su puerta, y el director solo habla
                        VERRANO (`Frame`) -- no sabe que tarjeta hay. Otra
                        GPU trae su fila, como manda PLAN_EL_AISLAMIENTO
```

Las seis comparten UNA cabeza de gato (un camino) y los ojos del logo: el
estilo cambia la materia, no al gato. Cuatro se pintan en la CPU con lo que
MAQUETA 3 ya promete (degradados, arcos, el camino aplanado al compilar, la
`@secuencia`); NEBULOSA y SOL DE PLASMA son trabajo de la 3060, y por eso,
si se eligen, HM3 empieza por un paso CPU (la misma estrella, quieta) y la
version viva espera al compositor de la GPU. Se eligieron las dos de la 3060,
asi que ese es el camino: HM3 (quieta, CPU) y HM3c (viva, la 3060).

El escritorio ENTERO, con todo junto, es `docs/arte/maqueta_escritorio_mision.html`
(el codigo de la estrella va aparte, en `docs/arte/estrella_gato.js`, y lo
comparten las dos maquetas: lo que pinta la estrella no sabe donde se pinta).

### Los escalones

- [x] HM0 -- HECHO el 06-10: la maqueta interactiva `docs/arte/maqueta_hud_nasa.html`: trece pantallas, cada una un instrumento distinto, con las piezas comunes, tres paletas y las teclas F1..F12 de verdad; probada en Chromium (las trece se pintan sin un error)
- [x] HM-dec -- HECHO el 06-10: HD1 (la paleta GATO, la ESTRELLA GATO y el INICIO) y HD3 (el escritorio primero), escritas en esta seccion; HD2 sigue abierta
- [x] HM0b -- HECHO el 06-10: la maqueta con HD1 dentro (`docs/arte/maqueta_hud_nasa.html`): la paleta GATO por defecto, la estrella gato con el logo de verdad en el escritorio, y la pantalla INICIO que se enciende y pasa sola al escritorio; probada en Chromium
- [x] HM-dec2 -- HECHO el 06-10: HD2 (la letra de la casa), escrita en esta seccion
- [x] HM0c -- HECHO el 06-10: los seis estilos de la estrella (HD4) en `docs/arte/maqueta_estrella_gato.html`: cada uno vivo, con su referencia y donde se pinta (CPU o la 3060), en galeria y en grande sobre el escritorio con el HUD; pulida a pedido del propietario ("cambia y mejorar un poco mas"): nariz y bigotes para que se lea gato, brillo, la lente de Gargantua, las puntas del Webb, la malla que gira, manchas solares y bucles de plasma, y el planeta que pasa por detras; probada en Chromium (los seis se pintan sin un error)
- [x] HM1 -- HECHO el 06-10: la paleta GATO de mision en `toolchain/tools/maqueta/tema/tema.maqueta` (`.mision` y once colores mas: el ojo, el neon, el azul, la tinta, GO, NO-GO...), cada uno del logo o de lo que el tema ya decia; generada a `Ultra_userspace/services/director/src/scene/tema_gen.rs` con `maqueta --paleta`, y el test del tema la comprueba
- [x] HM-dec3 -- HECHO el 06-10: HD4 (SOL DE PLASMA y NEBULOSA; quieta en la CPU, viva en la 3060 y aislada), escrita en esta seccion
- [x] HM0d -- HECHO el 06-10: el ESCRITORIO DE MISION entero, `docs/arte/maqueta_escritorio_mision.html`: el INICIO (la encuesta GO / NO-GO mientras la nebulosa se condensa en el sol), la barra de mision con la hora T+ y las luces, la lateral viva (propulsion, soporte vital, telemetria, la trayectoria de ESTRATOS y el motor de la estrella), la reticula que es el puntero, CAPCOM, la consola ESC y F1..F12, el mosaico que nada tapa (F1, F12, ESC), y el interruptor 3060 viva / CPU quieta; la estrella, aparte en `docs/arte/estrella_gato.js`, con SOL DE PLASMA y NEBULOSA pulidas (espiculas, rotacion, eyeccion; pilares de polvo y la nebulosa que se junta); probada en Chromium sin un error
- [ ] HM2 -- las piezas comunes (marco, regla, lectura, barra) como `.maqueta` de verdad, con sus ficheros dorados en `toolchain/tools/maqueta/pruebas/`; piden MAQUETA 3 (pila A y la seccion 2d de `docs/plan/PLAN_MAQUETA_3.md`)
- [ ] HM3 -- el escritorio de mision en la CPU: el SOL DE PLASMA QUIETO en `Ultra_userspace/services/director/src/scene/fondo.rs` (UN fotograma, pintado cuando algo cambia; en reposo ninguno), el planeta, la reticula y las esquinas; los colores, de `tema_gen.rs` (HM1)
- [ ] HM3c -- la estrella VIVA en la 3060: un programa de VERRANO en su propia pieza del director, que solo habla VERRANO; el sombreador entra como SPIR-V y la 3060 lo juzga y lo sube por su puerta (la fila de la 3060 de `docs/plan/PLAN_EL_AISLAMIENTO.md`). Sin la 3060 o sin su sobre: HM3, quieta
- [ ] HM3b -- el INICIO: la encuesta GO / NO-GO y la NEBULOSA que se condensa en el SOL (con la 3060, viva; en la CPU, la nebulosa quieta y luego el sol quieto: dos fotogramas), en `Ultra_userspace/services/director/src/scene/arranque.rs` y `Ultra_userspace/services/director/src/scene/splash.rs`; cada GO sale de lo que el kernel ya mide, no de un reloj
- [ ] HM4 -- los vitales F6, F7 y F8 como instrumentos: `Ultra_userspace/services/director/src/desktop/vitales.rs`
- [ ] HM5 -- F9, F10, F11 y el globo: `Ultra_userspace/services/director/src/scene/cabina.rs`, `Ultra_userspace/services/director/src/scene/sound.rs`, `Ultra_userspace/services/director/src/scene/globo.rs`
- [ ] HM6 -- las apps: F5 BANK CAT (`Ultra_userspace/services/director/src/desktop/bankcat.rs`), F1 el TALLER, F3 HERMES, F4 la LUDOTECA y F12 ESTRATOS, cada una con su instrumento
- [ ] HM7 -- del PROPIETARIO: la foto de cada pantalla en el Ryzen

---

Ver [`PLAN_DIRECTOR.md`](PLAN_DIRECTOR.md) (el compositor) y
[`PLAN_EL_PIXEL.md`](PLAN_EL_PIXEL.md) (por que no hay animaciones).
