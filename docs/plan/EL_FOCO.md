# EL FOCO -- una cosa a la vez, hasta el final (10-10)

> Peticion del propietario, **2026-10-10**: *"enfoca en algo, terminar eso y
> luego seguimos [...] me gustaria que enfoques mas en TITAN++ y VERRANO con
> PROTON-X + INTI esos elementos y LAMINA la RAM, el INTI en NAVEGAR si
> eliminar porque ya TITAN++ lo toma, puedes organizar?"*
>
> [`ABIERTO.md`](ABIERTO.md) dice QUE falta; [`EL_ORDEN.md`](EL_ORDEN.md), con
> que CRITERIO. Esta pagina dice QUE SE HACE AHORA: UNA cosa, con su linea de
> llegada, y la cola detras. Lo que no esta aqui espera.

---

# 1. EL MAPA: que hace cada uno

```text
   TITAN++    EL QUE ADMINISTRA. Las apps se escriben en TITAN++, y TITAN++
              conecta la CPU (por INTI) y la GPU (por VERRANO). Sus gpu fn
              cuentan en la 3060 o en la CPU, bit a bit lo mismo (LB4)
   INTI       LA PUERTA A LA CPU, y nada mas. Se queda lo de CPU: cpu.inti,
              pulso.inti y el emisor. Pierde TODO lo de app, pieza a pieza,
              el dia que su relevo en TITAN++ existe (3.3 de
              PLAN_INTI_SAMURAI); desde el 10-10 tambien bico.inti y
              png.inti (seccion 1d), y musica.inti con el sonido (Q2)
   VERRANO    LA PUERTA A LA GPU: la API de dibujo. Detras, la RTX 3060 12G
              (o la CPU, la misma API)
   LAMINA     LA RAM COMPARTIDA: quien cuenta PUBLICA en un bloque de RAM, y
              quien dibuja lo LEE ahi, sin copias ni syscalls por byte (la de
              VERRANO para vertices; la de la antena para paginas)
   PROTON-X   LOS JUEGOS DE WINDOWS: traduce D3D12 y DXIL a la casa, y lo que
              la puerta deja, a la 3060 (la receta)
```

## 1b. LA REGLA DEL PROPIETARIO (10-10): las apps VIVEN en la GPU

> *"TODAS las APP tienen que vivir en GPU no solo en CPU, en CPU solo
> calcula [...] que TITAN++ tome TODOS pero que algunos se quedan con CPU y
> otros que ejecutan sean SIEMPRE en GPU."*

```text
   TITAN++ administra TODAS las apps (INTI pierde lo de app)
   la CPU     CALCULA: la logica, las cuentas que no son de celdas, lo que
              lee de fuera. Nada de pintar pixeles a mano para la pantalla
   la 3060    EJECUTA lo que se ve: componer cada ventana en la pantalla,
              dibujar lo de VERRANO (el cubo, en su ventana), y las gpu fn
              de celdas. SIEMPRE, salvo que no este despierta (G0): entonces
              la CPU, la RESERVA de la ley L32, con los mismos bits, y lo dice
```

Lo que pide, en piezas (van en la cola, seccion 4):
la VENTANA de cada app compuesta por la 3060 (hoy el DIRECTOR copia con la
CPU); el cubo de TITAN++ dibujado por la 3060 DENTRO de su ventana (hoy, a
pantalla completa con `gpu verrano banco inti`); y las gpu fn al correr en la
3060 (LB8, la puerta de computo, Ring 0). F1..F5 no se paran por esto: la
ventana de TITAN++ es la misma superficie que la 3060 compondra.

## 1c. LA SEGUNDA REGLA (10-10): el DIRECTOR no negocia con la GPU

> *"sigue que mas falta para que el director negocie con GPU pero ojo no
> mismo GPU sino TITAN++ con VERRANO eso"*

```text
   la app (TITAN++)   CALCULA y PUBLICA en su lamina (RAM compartida):
                      `director.lamina` y `director.publica`; abre su ventana
                      (`director.ventana`) y dice su medida
   el DIRECTOR        COMPONE: toma la lamina de cada app y se la da a
                      VERRANO -- un `Frame` a una `Image` (`Backend::draw`),
                      la Image = los pixeles de la ventana de esa app --.
                      No nombra la 3060, ni un registro, ni una orden: S3 del
                      guardian `la-3060` ya lo juzga en cada build
   VERRANO            LA API: decide quien dibuja -- la 3060 si esta
                      despierta (G0), si no la CPU con los mismos bits (L32)
   el driver          ga10x, DETRAS de VERRANO (y de `gspcubo/sm86.rs`, la
                      unica puerta del escritorio que lo toma)
```

Asi Q0a2 y Q0a3 (seccion 5) se escriben CONTRA VERRANO, no contra el
aparato: el DIRECTOR pide "este Frame en esta Image de 640x360" y es la
puerta (`sm86.rs`) la que habla con la 3060. Si un dia hay otra GPU, el
DIRECTOR no cambia.

## 1d. INTI, MAS DELGADO (10-10): bico y png tambien salen

> *"me hice pregunta se pueden degradar mas? [...] png, bico y otros (no
> quites el CPU porque es INTI)"*

Si: D3 de PLAN_INTI_SAMURAI se reescribe. `bico.inti` (BMP y QOI a BICO) y
`png.inti` (escribe un PNG) son HERRAMIENTAS, no la puerta a la CPU: salen
de INTI con su relevo en TITAN++, y no antes (la regla 3.1: quitar antes
solo pierde su prueba). Lo que pide cada relevo:

```text
   pieza            su relevo en TITAN++                       estado
   bico.inti        Ultra_userspace/apps/bico (`run            FUERA, 10-10 (corte
                    titan/bico.bex`): `director.fichero` lee,  4e): los mismos
                    y `crea`/`escribe`/`cierra` escriben       bytes, los mismos
                                                               ficheros rotos
   png.inti         Ultra_userspace/apps/png (`run             FUERA, 10-10 (4e):
                    titan/png.bex`): el CRC y el Adler sin     el mismo fichero,
                    operaciones de bits                        byte a byte
   musica.inti      --                                         el sonido (Q2)
   cpu.inti,        SE QUEDAN: son la puerta de TITAN++ a la CPU (y con
   pulso.inti       ellas el monton, mientras `cpu.inti` lo use)
```

**Falta el metal:** `run titan/bico.bex` (deja `datos/fotob.bic` y
`datos/fotoq.bic`, que el escritorio pinta) y `run titan/png.bex` (deja
`datos/hola.png`, que el visor abre). [!] Un cambio con INTI: el codigo 2
de bico (mas de 256 KiB) es ahora el 1 -- `director.fichero` dice si lo
leyo entero, no por que no --.

## 1e. LA LAMINA QUIROFANO (10-10): un dato, un sitio, en tiempo real

> *"educar a lamina (ram) que sea ultra educado [...] la CPU siempre
> orquesta [...] que sea en streaming y no copien para otro para que sea en
> tiempo real siempre [...] que lamina o ram sea quirofano extremo, que la
> CPU al tener la GPU ahi siempre disponible sea exprimido"*

Las reglas -- cada una con su juez, no con su buena intencion --:

```text
   1  UN DATO, UN SITIO   lo que la app cuenta se escribe UNA vez, en su
                          lamina; quien lo usa lo LEE ahi (el kernel lee el
                          paquete en su sitio, por el physmap). Copiar es la
                          excepcion, y cada copia que queda esta en la tabla
   2  SE MIRA UNA PALABRA la secuencia: sin publicacion nueva, NO se copia
                          nada (`Lamina::leer_si_nueva`, su prueba)
   3  NUNCA MEDIO         el sello por ranura: lo que se dibuja es siempre un
                          fotograma ENTERO; uno pillado a medias se reintenta
   4  SIN CERROJO         una app colgada deja su ultimo fotograma, y ya: nadie
                          espera a nadie (el escritorio no se para por una app)
   5  LA CPU ORQUESTA     decide CUANDO y QUE (el plazo del fotograma, el
                          paquete); la 3060 EJECUTA lo que se ve (limpiar,
                          dibujar, agrandar). Lo que la 3060 puede hacer, no
                          lo hace la CPU: la limpieza del destino ya es suya
   6  AL RITMO, NO A LA   el programa duerme hasta el PLAZO del fotograma
      SIESTA              siguiente (`director.espera`), no "trabajo + 16 ms"
```

**Las copias por fotograma del cubo (lamina -> 3060), antes y ahora:**

```text
   app -> lamina           1 (n x 32 B)   la publicacion: es el dato
   lamina -> vi -> de_inti 2 por CADA mirada de 1 ms   ->  1 por publicacion
   de_inti -> paquete      1 (n x 32 B)   queda; el siguiente corte: la
                                          lamina directo al paquete
   programas -> paquete    ~700 B por fotograma: queda (se pueden escribir
                           una vez: la caja del paquete no cambia)
   paquete -> VRAM         n x 32 B por BAR0 de 4 en 4: queda (el anillo y
                           el prestamo persistente, S1 de PLAN_VERRANO)
   limpiar el destino      enram: 3,6 MB por la CPU  ->  la 3060 (VRN1)
   leer de vuelta          el juicio final: 460.800 puertas -> la RAM (enram)
```

**EL CUELLO DE BOTELLA, con numeros** (de las medidas del metal, no de
memoria) y lo que se hizo:

```text
   1  el fotograma era trabajo + 16 ms de siesta    HECHO: el plazo
      + despertar tarde: 18,7 ms, el 92 % esperar   (`tests/ritmo.rs`)
      (S0 de PLAN_VERRANO)
   2  el kernel despertaba tarde (~3,5 ms medio):   HECHO: quien despierta
      la del mismo rango esperaba el quantum        entra si la otra ya gasto
      entero de la otra                             un tic (`bmo_orquesta::
                                                    turno`). Queda: el tic de
                                                    1 ms (un reloj sin tic)
   3  DOOM por la 3060: comprobar 560 us contra     HECHO: al cargar y uno de
      679 de dibujar (METAL_2026-09-25)             cada 32 (`im::toca_mirar`)
   4  la pantalla la copia la CPU: 27,6 ms          QUEDA: el motor de copia
      entera; el PCIe en Gen1 de Gen3               por defecto (649 us), el
                                                    page flip (LA_RAM 8) y el
                                                    enlace en Gen3
   5  las puertas largas con las interrupciones     HECHO en el anfitrion
      cerradas y la 3060 esperada girando dentro    (10-10): EL RESPIRO
      (`latido tarde 1632 ms`, METAL_2026-10-08)    (`ring0/dev/respiro.rs`,
                                                    neutro). Esperando a
      La de 1632 ms era "op 4", no la 3060 (esa     la 3060, las interrup-
      es la 53): lo mas probable, cerrar un         ciones se abren un
      fichero (FAT32 y el FLUSH con IF=0). Y el     instante en cada vuelta
      `yield_current` de la espera no cedia nada:   (sti/pause/cli) si la
      dentro de un syscall solo movia `current`     tarea TIENE la 3060 y no
      (quitado el mismo dia; la puerta larga sale   hay cerrojos tomados; la
      a nombre de quien LLAMO)                      pila del trap se re-
                                                    publica al volver. La
                                                    puerta es UNA (las
                                                    ordenes de la IOMMU y
                                                    las estaciones de la
                                                    muerte que devuelven el
                                                    lienzo); quien esta
                                                    dentro es intocable y
                                                    muere al salir. La
                                                    puerta larga mide solo
                                                    lo CERRADO. `RESPIRO`
                                                    es el interruptor.
                                                    Y op 4, el mismo dia: el
                                                    FLUSH del cierre de un
                                                    fichero va EN VUELO (el
                                                    hilo del disco lo
                                                    aterriza con su IRQ); la
                                                    escritura sigue dentro y
                                                    se MIDE (`guardar` y
                                                    `vaciar` en `save`).
                                                    QUEDA: el metal
   6  el camino frio del cubo, frecuente: la        HECHO en el anfitrion
      huella del caliente lleva el numero de        (10-10): el caliente
      vertices, que cambia al girar                 reescribe las ORDENES
                                                    (cientos de bytes, sin
                                                    releer) y la cuenta sale
                                                    de la huella. La prueba:
                                                    frio con 6 triangulos +
                                                    caliente con 4 = frio con
                                                    4, palabra a palabra
                                                    (saboteado: cae)
```

**Falta el metal** para los HECHOS: la lista entera, con sus ordenes, en
la seccion 6.

# 2. LO QUE YA FUNCIONA EN EL METAL (no se toca: se usa)

```text
   09-10 18:24  TITAN++ -> LAMINA -> VERRANO -> 3060: cubogira.bex publica y
                la 3060 dibuja 360 y 3600 fotogramas IGUAL al juez (LB7)
   09-10        G0: el GSP despierta, 2 de 10 con el paso 1
   09-10 22:41  Freedoom se juega en el Ryzen con la 3060 despierta (L0)
   28-09        BMOX-12 por PROTON-X y la receta: la 3060 lo dibuja
   06-10        los 92 jueces de DX12, 92 de 92 en Windows
```

**Sobre el "cuello de botella" (para que no confunda):** la 3060 NO es lo
lento. Dibuja el cubo en 0,87 ms; lo que se lleva el tiempo es todo lo de
alrededor -- la CPU preparando, copiando y esperando --: en S0 de
[`PLAN_VERRANO.md`](PLAN_VERRANO.md), el dibujo es el 7 % de la pared y el
resto el 92 %. DOOM no sufre porque a la 3060 le pide poquisimo: agrandar
320x200. Un juego grande si sufrira, y lo que hay que adelgazar es la CPU de
alrededor, no la 3060.

# 3. EL FOCO DE AHORA: TITAN++ TOMA NAVEGAR

**Por que este:** es el corte de INTI que el propietario pidio (NAVEGAR fuera
de INTI), usa las cuatro piezas (TITAN++ administra, la LAMINA de la antena
en la RAM, la ventana del DIRECTOR, INTI solo CPU), y deja a TITAN++ con lo
que toda app necesita: una ventana, sus pixeles y su entrada.

**Lo que hay hoy, sin adornos:** TITAN++ todavia NO puede tomar NAVEGAR. Le
faltan la ventana (TA2 de [`PLAN_LA_TINTA.md`](PLAN_LA_TINTA.md)), los bytes
(TA1) y la entrada (TA3); su `director` solo sabe la lamina de VERRANO. Por
eso `navegar.inti` (386 lineas) NO se borra antes: se borra en el MISMO
commit en que el de TITAN++ lo sustituye. Asi nunca hay un dia sin NAVEGAR.

- [x] **F1 -- la VENTANA en TITAN++** (TA2): `director.ventana(ancho, alto)`
      pide su superficie y la ofrece, una fila de pixeles se escribe, y
      `director.presenta()` la entrega; con `screen = true` en el Titan.toml
      y su linea en el certificado. **Como se sabe:** un ejemplo de nivel 11
      lanzado por un escritorio de mentira en el anfitrion pinta un
      degradado y el escritorio lee los mismos pixeles, bit a bit; y sus NO
      (sin `screen`, sin `use director`, desde una gpu fn).
      **HECHO en el anfitrion el 10-10:** `director.ventana(ancho, alto)`,
      `pixel`, `rect`, `fila` y `presenta` (`emisor-x86_64/src/e1/ventana.rs`,
      la superficie BSUP del ABI por sus nombres, con buzon de 64 ranuras);
      el ejemplo `nivel11/ventana` (un degradado y una barra que la cruza) se
      despliega como `titan/ventana.bex`; `tests/ventana.rs`: la cabecera,
      cada pixel recortado, la secuencia, las medidas que dice que no, los
      NO del compilador y el certificado (la PANTALLA). Saboteado el recorte
      de la x: cae. El metro, igual: el cubo que gira no paga nada.
- [x] **F2 -- los BYTES** (TA1): un tipo de 8 bits sin signo y tablas grandes
      en un bloque pedido. **Como se sabe:** su codigo T, su ejemplo y su
      nivel en la GRAMATICA; una tabla de millones de celdas sin copia.
      **HECHO el 10-10 por otro camino, el que NAVEGAR necesita:** los bytes
      se LEEN donde estan, sin copiarlos a una tabla -- `director.toma()` (lo
      que ofrece el antenista), `director.fichero(ruta)` (uno entero, hasta
      256 KiB), `director.medida()` y `director.byte(i)` (`entrada.rs`) --; y
      la LETRA de BMO-X en la ventana: `director.letra` y `director.texto`
      (`letra.rs`, la tabla de `fontgen` detras de su subrutina). El tipo
      de 8 bits de TA1 sigue abierto en `PLAN_LA_TINTA` (lo pide el lienzo,
      no NAVEGAR).
- [x] **F3 -- la ENTRADA** (TA3): el raton y las teclas que el DIRECTOR le
      manda a su ventana. **Como se sabe:** el escritorio de mentira manda un
      clic y una tecla, y el programa los ve. **HECHO el 10-10:**
      `director.evento()` (0 nada, 1 tecla pulsada, 2 soltada, 3 letra, 4
      raton, 5 la ventana cambio), `codigo()`, `raton_x()`, `raton_y()`,
      `botones()` y `se_ve()`; `tests/entrada.rs`: los cinco eventos del
      buzon con sus datos, y el buzon vacio.
- [x] **F4 -- NAVEGAR en TITAN++**: toma la lamina que le OFRECE el
      antenista (o la del disco, `datos/ejemplo.lam`), la pinta en su ventana
      y devuelve clics y teclas, como la version 3 de `navegar.inti`.
      **Como se sabe:** en el anfitrion, la de example.com pintada por el de
      TITAN++ y por el de INTI da los MISMOS pixeles.
      **HECHO el 10-10:** `Ultra_userspace/apps/navegar/` (un paquete:
      `src/main.titan` y `src/lamina.titan`, el port linea a linea de
      `navegar.inti` y de `runtime/lamina/lamina.inti`). Las NUEVE pruebas
      que tenia el de INTI, contra el de TITAN++ (`emisor-x86_64/tests/
      navegar.rs`), pasan a la primera; y los dos, corridos en el mismo
      emulador sobre el mensaje, example.com, la flecha abajo y una lamina
      rechazada, dan la MISMA ventana canal a canal (cuatro huellas FNV
      iguales, fijadas en la prueba).
- [x] **F5 -- el corte 4c de INTI**: `navegar.inti` fuera del build y del
      arbol, el icono y `run apps/navegar...` apuntan al de TITAN++, y el
      perfil `pleno` sale con el. **Como se sabe:** el build sin
      `navegar.ibx`; y en el metal, del propietario, NAVEGAR abre desde su
      icono y pinta example.com.
      **HECHO en el arbol el 10-10:** fuera `navegar.inti` y la sonda
      `sondas/ventana.inti` (su relevo, `titan/ventana.bex`), con sus dos
      pruebas; el build despliega `apps/navegar.bex` de TITAN++ con su icono
      (`bmo-pack` y el juez de carga, de acuerdo); el ANTENISTA ofrece la
      pagina a `navegar.bex`; el metro y la medida, al dia. El perfil
      `pleno` NO sale todavia: lo usan las sondas del censo de INTI (sale
      con el paso 5 de la escalera de INTI). **Falta el metal:** NAVEGAR
      desde su icono, con y sin antena.

Cada paso entra con sus pruebas en el anfitrion; el metal, UNA vez, al final
(F5).

# 4. LA COLA: lo que va despues, en este orden

```text
   Q0  LA REGLA 1b: (a) el cubo de TITAN++ en SU ventana, dibujado por la
       3060 desde su lamina; (b) cada ventana compuesta por la 3060; (c) las
       gpu fn al correr en la 3060 (LB8, Ring 0: se decide con el
       propietario). Desglosada abajo (seccion 5)
   Q1  el juez de E8f/E8g en el metal: un .exe de DX12 con un cbuffer
       indexado, SampleLevel y Load en una 2D de una mip, que la puerta mande
       a la 3060; su tabla en Windows y despues en el Ryzen
       (PLAN_LA_LENGUA_DE_LA_3060)
   Q2  el sonido en TITAN++, y el corte 4d de INTI (musica.inti fuera)
   Q2b HECHA el 10-10: TITAN++ escribe byte a byte y el corte 4e de INTI
       (bico.inti y png.inti a TITAN++, seccion 1d)
   Q3  DOOM por la 3060 cerrado (D2c, la linea [perf]) y su paleta (D3)
   Q4  las texturas con mips en la VRAM, Ring 0: lo que deja a Cyberpunk
       mandar a la 3060 sus SampleLevel de verdad
   Q5  el Sample normal de un pixel (la mip de su cuadro)
   Q6  adelgazar la CPU de alrededor del dibujo (S3 y S4 de PLAN_VERRANO)
```

- [x] **R1 -- RESOLUCION** (10-10, el propietario: *"una app simple para
      configurar hasta cuantas Resolucion que quiero y encima control en
      ellas para cualquier ventana"*): `Ultra_userspace/apps/resolucion/`,
      en TITAN++. Ocho medidas, de 640x360 a 2560x1440; las flechas, las
      cifras 1-8 o el raton eligen, Enter la GUARDA en
      `datos/resolucion.txt` ("ANCHO ALTO") con `director.guarda` (TA4,
      nuevo), Esc cierra. Quien la LEE: cada ventana de TITAN++ que la
      quiera, con su `medida.titan` -- `titan/ventana.bex` ya se abre con
      ella --; y despues Q0a, la medida del dibujo de la 3060 por VERRANO.
      Pruebas: `emisor-x86_64/tests/resolucion.rs` (5) y `entrada.rs`.
      **Falta el metal:** `apps/resolucion.bex` desde su icono, Enter, y
      `run titan/ventana.bex` con la medida nueva. [!] La medida la elige
      cada app al abrirse: el DIRECTOR no agranda una ventana ya abierta
      (eso seria otra pieza: la ventana que cambia de medida, R-APP).

- [x] **R2 -- Alt+Tab MANDA en la pantalla** (10-10, el propietario: *"que
      SIEMPRE dominen en la pantalla asi sea con pantalla completa en GPU y
      CPU porque se parpadea"*). Parpadeaba por dos caminos: una app a
      pantalla completa compuesta por la CPU se pegaba encima de la tarjeta
      (no miraba lo que tenia delante), y la 3060 -- DOOM con
      `SUP_A_LA_3060`, PROTON-X con `SUP_LA_3060_DIRECTA` -- escribe la
      pantalla ENTERA cada fotograma. Ahora (`bmo_foco::encima`, con sus
      pruebas): la caja del conmutador no la pisa ninguna app; con el
      abierto la 3060 se queda QUIETA (ni un fotograma, y a la receta
      directa se le APARTA la pantalla: dibuja en su RAM, y el juego sigue
      corriendo); al soltar Alt se vuelca lo devuelto y la 3060 VUELVE con
      un fotograma. **Falta el metal:** Alt+Tab con DOOM por la 3060
      (`gpu doom`), con Cyberpunk por PROTON-X y con una app de CPU a
      pantalla completa: la tarjeta quieta, sin parpadeo, y al soltar el
      juego vuelve. [!] Que el juego se MUEVA detras de la tarjeta (como en
      Windows) pide que la 3060 componga la tarjeta encima: Q0b.

- [x] **R3 -- las ventanas YA NO SE PISAN** (10-10, el propietario: *"al
      estar con una ventana con cualquier app choca, se mezclan o uno
      predomina"*). El DIRECTOR no tenia apilado entre apps: las pegaba en el
      orden de su hueco (la que cambiaba despues tapaba a la otra, estuviera
      delante o no), siempre ENCIMA de las ventanas del sistema, el clic iba
      a la primera de la mesa, y el CROMO de una de detras (borde, titulo y
      fondo, pintado entero al perder el foco o al soltar Alt) caia encima de
      la de delante. Ahora hay UN apilado, la lista del foco
      (`foco::apilado`; la geometria pura y probada en `bmo_foco::encima`):
      cada app se pega sin los trozos que le tapan las de delante -- apps y
      del sistema --, de atras hacia delante; si una de detras repinta su
      cromo, lo de delante que pisa se devuelve; el clic, el realce y el
      borde vivo van a la que se VE; y una del sistema esta tapada solo si
      algo de DELANTE la pisa. Una app quieta destapada por otra que se movio
      se repega. **Falta el metal:** dos apps solapadas (NAVEGAR y
      RESOLUCION, o DOOM en ventana) y una del sistema (CABINA) encima y
      debajo, con clics. [!] En el modo Puntero (Alt+M) el foco sigue al
      raton, y con el el apilado: pasar por encima trae delante. [!] Una app
      que pone la 3060 a pantalla completa sigue escribiendo la pantalla
      entera: una ventana del sistema delante de ella pide Q0b.

**Lo del propietario, en cada arranque:** apagar (no reiniciar), `gpu init`
y `save`: cada `SALIDA.TXT` es una fila de G0 hasta diez. Y una decision que
espera sin prisa: el CUBO de E8g (la division de la 3060, la de la casa, o
esperar).

# 5. Q0, DESGLOSADA: lo que dice el codigo hoy (10-10)

F1..F5 estan HECHOS en el anfitrion (TITAN++ abre ventanas, lee lo que le
ofrecen, recibe teclas, escribe la letra, y NAVEGAR es suyo con los mismos
pixeles; INTI perdio la ventana, la lamina, la entrada y la letra: cortes 4c
y 4b). Lo que la regla 1b pide despues, leido en el DIRECTOR:

```text
   el aparato de la 3060   `gspcubo/sm86.rs`: la tuberia FIJA del estudio --
                           1280x720 y su fondo, o es "no valido" --, dibuja en
                           la PANTALLA (en RAM solo con `bmox12`, `enram`) y lee
                           de vuelta para el juez. VERRANO (`Backend::draw`,
                           un `Frame` a una `Image`) ya es la API correcta
   el compositor           el DIRECTOR pega cada ventana COPIANDO con la CPU
                           (el volcado de cada fotograma); la 3060 solo toma la
                           pantalla entera (DOOM con `SUP_A_LA_3060`, PROTON-X
                           con `SUP_LA_3060_DIRECTA`)
   la memoria de la app    la 3060 lee lo PRESTADO por la IOMMU si sus marcos
                           van seguidos (`loan::fisica_tomada`, lo de DOOM)
```

- [ ] **Q0a1 -- el aparato con viewport y fondo LIBRES**: la tuberia ya
      lleva `Destino { fila, ancho, alto }`; el aparato deja de exigir
      1280x720 y el fondo del estudio para un fotograma de VERRANO. **Como se
      sabe:** el banco con 640x360 IGUAL a la CPU de VERRANO (la reserva, los
      mismos bits), en el metal.
      **10-10, el DRIVER, hecho en el anfitrion:** `cubo::Ventana` lleva su
      medida y las ordenes sacan de ella el destino de color, el recorte y
      el viewport (`x * w/2 + w/2`, `y * -h/2 + h/2`); `Destino::valido`
      acepta cualquier medida de 1 a 4096 por lado que quepa en el mapa (8
      MiB: 1920x1080 cabe); con Z, solo 1280x720 (`tuberia::cabe`: la sombra
      y el bufer de Z miden eso), en el lector de VRN1/VRN2 y en el escritor
      de la receta. La puerta de los juegos (PROTON-X) se queda en 1280x720
      HASTA VERLO en el metal, y lo dice. Falta el aparato del DIRECTOR (con
      Q0a2, que es quien lo usa) y el metal.
- [ ] **Q0a2 -- el destino EN RAM para la lamina** (como `enram` de
      `bmox12`): la 3060 dibuja en un bloque, no en la pantalla, sin leer de
      vuelta. **Como se sabe:** `gpu verrano banco inti enram` IGUAL al juez.
      **10-10, hecho en el anfitrion:** VRN1 LLEVA el color de la limpieza
      del destino (+20 = 1, +24 el pixel, la marca de la receta) y la 3060
      lo limpia -- la CPU deja de llenar 3,6 MB por fotograma en `enram` --;
      `enram` ya no pide `bmox12`: los vertices de la lamina van DIRECTO al
      VRN1 (`escribir_paquete_dibujo_de`, sin bufer), con el destino en RAM
      de la medida del fotograma; y el banco lee la lamina UNA vez por
      publicacion (`Lamina::leer_si_nueva`), no en cada mirada de 1 ms. El
      juicio final del banco con `enram` lee de la RAM, no por 460.800
      puertas. **Falta el metal:** `run titan/cubogira.bex` y, mientras
      corre, `gpu verrano banco inti enram` (IGUAL a D3D12).
      **Por VERRANO (1c):** la Image de `Backend::draw` con su medida; la
      puerta `sm86.rs` la traduce al `Destino` de Q0a1. Le falta al paquete
      VRN1 el COLOR de fondo (hoy solo lo lleva la receta VRN2): o VRN1
      crece, o el camino de la lamina pasa por la receta.
- [ ] **Q0a3 -- el DIRECTOR dibuja la lamina DENTRO de la ventana de su
      app** (por VERRANO, 1c: el DIRECTOR pide, la puerta dibuja): si el tid de una ventana tiene lamina, cada fotograma NUEVO de
      la lamina lo dibuja el aparato en los pixeles de esa ventana (Ring 0:
      la ventana prestada a la 3060 por la IOMMU, con sus marcos seguidos);
      sin la 3060, la CPU de VERRANO, y lo dice. **Como se sabe:** en el
      metal, `run titan/cubogira.bex` en una ventana que se mueve y se tapa,
      y su huella IGUAL al juez.
      **10-10, hecho en el anfitrion y SIN Ring 0:** la superficie de la app
      no le sirve a la 3060 (sus pixeles empiezan en +32: no van alineados a
      pagina), asi que la 3060 dibuja en un bloque DEL ESCRITORIO de la
      medida de la ventana y la ventana se compone desde ahi
      (`Surface::fuente`). `gspcubo::laminas`, en cada vuelta y al lado de
      `presentar_apps`: la lamina, su ventana (el mismo tid), una palabra si
      no hay nada nuevo, y con un fotograma nuevo `Backend::draw` en el
      `Aparato` -- abierto UNA vez, `en_la_imagen`: la `Image` ES el bloque,
      nada se copia de vuelta --. El DIRECTOR sigue sin nombrar la 3060
      fuera de la puerta. Si algo falla se dice UNA vez por la consola y la
      app se queda con lo que pinte ella. [!] Ventanas de ancho multiplo de
      32 y hasta 1280x720; el huella contra D3D12 solo existe a 1280x720
      (a otra medida, el juez es la CPU de VERRANO). **10-10, escrito:**
      `Cpu::juzgar` (el recuento pixel a pixel, `el_juicio_a_otra_medida`)
      y `laminas` juzga el primer fotograma de cada app y uno de cada 1024;
      la consola dice `[Q0a3] el juez ... la 3060 da lo MISMO` o cuantos
      pixeles da DISTINTO.
- [ ] **Q0a4 -- el cubo de TITAN++ pide su ventana**: `cubo_gira` abre
      `director.ventana` ademas de su lamina (TITAN++ ya sabe las dos).
      **10-10, hecho en el anfitrion:** 640x360 (16:9 como su camara, filas
      de 128 bytes), con su fondo y una linea de quien la pinta hasta que la
      3060 la tome. Y una trampa resuelta: ofrecer la lamina al MISMO
      escritorio SUSTITUYE la oferta de la ventana si aun no la tomo
      (`loan::offer`); `director.lamina`, con ventana, espera a que el
      escritorio la TOME (`SUP_TOMADA`, hasta medio segundo). La prueba
      `tests/lamina.rs`: la ventana primero, y los 360 fotogramas bit a bit.
      **Falta el metal (Q0a3 y Q0a4):** `run titan/cubogira.bex` desde
      Ejecutar: el cubo gira EN SU VENTANA, se mueve y se tapa.
- [ ] **Q0b -- la 3060 COMPONE**: el volcado de las ventanas por el motor de
      copia (o un programa de computo como `imagen`), no por la CPU.
- [ ] **Q0c -- las gpu fn al correr en la 3060** = LB8 de
      PLAN_LAS_LIBRERIAS (la puerta de computo, Ring 0, con el propietario;
      pide G0).

# 6. QUE FALTA COMPROBAR EN EL METAL (10-10)

Todo esto esta HECHO y probado en el anfitrion; ninguno se ha visto aun en
el Ryzen con la 3060. Una sesion, en este orden, y un `save` al final: cada
linea dice que se mira y que seria un NO.

```text
   QUE                         COMO                            SI SALE MAL
   R2  Alt+Tab domina          Alt+Tab con una app normal, con  la lista detras
       la pantalla             DOOM por la 3060 a pantalla      de la app, o
                               completa y con la CPU            parpadeo
   R3  las ventanas se         dos apps encima una de otra;     una se mezcla
       apilan                  clic en la de atras: sube        con la otra
   R1  la resolucion           `run apps/resolucion.bex`, una   la ventana no
                               medida, y abrir otra app         toma la medida
   4e  bico y png en TITAN++   `run titan/bico.bex` y           pixeles mal o
                               `run titan/png.bex`              el fichero no
                                                                se escribe
   Q0a2 la 3060 dibuja EN RAM  `gpu verrano banco inti enram`   huella distinta
                               con `run titan/cubogira.bex`     del juez
   Q0a3 el cubo EN SU VENTANA  `run titan/cubogira.bex` desde   la ventana se
   Q0a4                        Ejecutar: gira, se mueve, se     queda con su
                               tapa con otra; en la consola,    linea de espera;
                               `[Q0a3] el juez ... 640x360:     o el juez dice
                               la 3060 da lo MISMO`             DISTINTO
   1   el plazo del fotograma  la linea de fotograma del cubo:  ~18 ms otra vez
                               ~16,7 ms de pared, poco esperar
   2   el turno al despertar   la misma linea: despertar tarde  ~3,5 ms medio
                               < 1 ms
   3   DOOM mira 1 de 32       `gpu doom`, la linea `[perf]`    ~560 us de
                               de la 3060                       comprobar
   6   el cubo en caliente     `gpu verrano banco inti` con el  `preparar` de
                               cubo girando: casi todos los     ~5 ms en los
                               fotogramas en caliente           que giran
   M1  DOS CUBOS que se tapan  `run titan/doscubos.bex`: el de  se ven caras de
       (V2, la profundidad)    atras cruza POR DETRAS del de    dentro, o el de
                               delante; la consola: el juez     atras encima;
                               `... la 3060 da lo MISMO`        juez DISTINTO
   TB1 TINTA, el lienzo        desde su icono: pintar, la goma  ver la lista de
                               (derecho), s guarda el PNG       5b2 de PLAN_LA_
                                                                TINTA
   5   EL RESPIRO              `gpu doom` y el cubo, moviendo   pantalla azul,
                               el raton; en `save`: `respiro`   latido tarde
                               y `apartadas` SUBEN, `latido     con la 3060,
                               tarde` sin la 3060 de culpable   el raton a
                                                                tirones
   5   la puerta larga, de     cualquier `save` con una puerta  op 4 de otro
       quien la llamo          larga: el nombre es el de quien  (el disco) =
                               llamo, y si es op 4 es el disco  el siguiente
   op4 el cierre, partido      escribir un fichero (`save` ya   `vaciar` en 0
                               lo hace) y otro `save`: las      con `guardar`
                               filas `guardar` (DENTRO de la    grande = la
                               puerta) y `vaciar` (el FLUSH,    escritura era
                               ya fuera); y la puerta larga     el problema
                               ya no es op 4 por el FLUSH       (abajo)
```

**Mirado y NO es parche (10-10):** la purga (`core/purga.rs`) cede con
`yield_current` en bucle, pero corre en el hilo del bus o en el shell de
Ring 0, nunca dentro de un syscall: ahi ceder SI cambia de tarea. Y con el
respiro, la tarea que este dentro de la 3060 no se purga a medias: muere al
salir de su puerta, y la purga la espera cediendo.

**El respiro y el disco (10-10):** el disco tomado es un cerrojo tambien. Una
puerta de la 3060 que tuviera el disco y respirara dejaria al hilo del disco
girando con IF=0 en `tomar_disco` para siempre: el respiro no abre con el
disco tomado (`disk::tomado`).

**Si el respiro sale mal:** `RESPIRO = false` en `ring0/dev/respiro.rs`
y la espera vuelve a girar entera con IF=0, como antes del 10-10
(la puerta unica y la tarea intocable se quedan: no cuestan nada). Con la
pantalla azul, su foto dice donde: si es en el `cli` o justo despues, la
pila re-publicada; si es al matar una app, la tarea intocable.

**Las piezas que aun son parche** (se cambian cuando toque, no se olvidan):

```text
   op 4, la escritura     el FLUSH ya va en vuelo (10-10); los SECTORES del
                          fichero, la FAT y la entrada se siguen escribiendo
                          dentro de la puerta, esperados uno a uno. Si
                          `guardar` sale grande en el metal, la pieza: el
                          guardado entero por el hilo del disco, por trozos,
                          y la puerta solo encola
   el tic de 1 ms         el turno de quien despierta espera a un tic: un
                          reloj sin tic (2 del cuello)
   la copia de pantalla   la CPU, 27,6 ms entera: el motor de copia por
                          defecto, el page flip, el PCIe en Gen3 (4)
```

**Mirado y se queda asi, a proposito (10-10):** los ~700 B de programas que
el DIRECTOR copia al paquete en cada fotograma, y la copia de los vertices de
la lamina al paquete (n x 32 B). Juntas son menos de un microsegundo de CPU
contra un fotograma de 16,7 ms; un paquete persistente con programas
"ya escritos" agregaria estado que puede quedar viejo, para ahorrar nada que
se vea. Lo caro estaba en el kernel (el frio de la 3060, el FLUSH, la espera
con IF=0), y eso es lo que se cambio.
