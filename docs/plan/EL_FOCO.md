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
   pieza            lo que TITAN++ ya tiene          lo que le falta
   bico.inti        leer un fichero entero           escribir BYTES (TA1 de
                    (`director.fichero`, `byte`) y   PLAN_LA_TINTA): hoy
                    guardar un TEXTO (`guarda`, R1)  `guarda` solo lleva texto
   png.inti         las tablas y el monton (13)      lo mismo: escribir bytes;
                                                     el CRC y el Adler, en int
   musica.inti      --                               el sonido (Q2)
   cpu.inti,        SE QUEDAN: son la puerta de TITAN++ a la CPU (y con
   pulso.inti       ellas el monton, mientras `cpu.inti` lo use)
```

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
   Q2b escribir BYTES desde TITAN++ (TA1), y el corte 4e de INTI: bico.inti
       y png.inti a TITAN++ (seccion 1d)
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
- [ ] **Q0a4 -- el cubo de TITAN++ pide su ventana**: `cubo_gira` abre
      `director.ventana` ademas de su lamina (TITAN++ ya sabe las dos).
- [ ] **Q0b -- la 3060 COMPONE**: el volcado de las ventanas por el motor de
      copia (o un programa de computo como `imagen`), no por la CPU.
- [ ] **Q0c -- las gpu fn al correr en la 3060** = LB8 de
      PLAN_LAS_LIBRERIAS (la puerta de computo, Ring 0, con el propietario;
      pide G0).

