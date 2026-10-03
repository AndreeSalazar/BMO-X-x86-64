# PLAN HERMES -- F3 de BMO-X: dos BMO-X que se hablan, sin servidor de nadie

> Pedido por el propietario (2026-10-02), despues de leer el analisis del
> esbozo del 01-10: *"es para construir por completo eso en F3 [...] todo lo
> que dijiste anotar pero no subestimes si solo leiste, vamos a empezar desde
> inicio TOTAL"*. Y dos reglas que mandan sobre todo lo de abajo:
>
> > *"lo del ANTENA = no lo mezcles, no es lo mismo, mi objetivo es
> > diferente: ANTENA es para conectar con Google"*
>
> > *"SIEMPRE AISLANDO por completo"*
>
> **Lo que afirma**: que se pulsa `F3`, entra el gato con alas y su glitch, y
> desde ahi se escribe, se zumba, se comparte una foto o un video con otra
> maquina BMO-X -- y que ni la app, ni lo que llega, ni quien lo manda pueden
> tocar nada fuera de la carpeta `F:/hermes/` (F: es ESTRATOS).
>
> **Como se cae**: un mensaje que se lee sin saber de quien es; un fichero que
> llega y sale de la cuarentena sin juez; una app de mensajes que puede abrir
> `sys/` o el disco `d:`; o una clave privada que puede firmar lo que ejecuta.
>
> [!] **Corregido el mismo dia.** La primera lectura de este analisis decia que
> `entrantes` no cabia en 8.3 y que no habia letra `F:`. Las dos cosas eran
> falsas: `F:` es ESTRATOS (`PLAN_EL_HUD.md`; la LUDOTECA lo pinta: *"F:
> montado"*) y ESTRATOS tiene nombres largos. Se miro FAT32 en vez de mirar
> donde el propietario habia puesto la carpeta. Lo que SI pesa de ESTRATOS es
> otra cosa, y esta en la seccion 5.
>
> **La cara**: [`../arte/maqueta_hermes.html`](../arte/maqueta_hermes.html),
> HERMES (F3) y la LUDOTECA (F4) en la misma maqueta, con las dos teclas.
>
> Sustituye al esbozo de `PLAN_LA_LUDOTECA.md` seccion 7 ("HERMES, en F3"),
> que se queda como el origen de los nombres. Las casillas viven aqui.

---

## 0. La respuesta corta

```text
   HERMES        la app de F3: mensajes, tertulias, el MURO, el CANAL y los
                 ENVIOS. Negra, con el gato y su glitch en la entrada
   HERMES/1      el protocolo: como hablan DOS BMO-X. Uno con otro, sin
                 servidor en medio, cifrado de punta a punta
   un AMIGO      una clave publica X25519 aceptada UNA vez (como known_hosts)
   AISLADO       tres procesos, tres jaulas: la PUERTA (red y clave), la APP
                 (la cara) y el JUEZ (lo que llega). Ninguno ve lo del otro
```

*** **Y no es la ANTENA.** La ANTENA (`bmo-antena`, `ANTENA/1`, puerto 7117)
es como BMO-X le pide la web y Google a un movil. HERMES es BMO-X con BMO-X.
No comparten crate, ni puerto, ni protocolo, ni una linea de codigo, ni el
relevo de fuera de casa. Si algun dia se parecen, se parecen por casualidad.

---

## 1. Lo que hay HOY, medido el 02-10 (y lo que no)

El esbozo del 01-10 se escribio de memoria. Esto es lo que dice el arbol:

```text
   PIEZA                     ESTADO                  DONDE
   ----------------------------------------------------------------------
   X25519                    [X] probado             bmo-cripto/x25519.rs
   AES-GCM                   [X] probado             bmo-cripto/gcm.rs
   SHA-256, HMAC, HKDF       [X] probado             bmo-cripto/{sha256,hmac}.rs
   el AZAR (RDRAND)          [X] sin respaldo        bmo-cripto/azar.rs
   Ed25519                   SOLO COMPROBAR          bmo-cripto/ed25519.rs
                             firmar NO, a proposito  (PLAN_SEGURIDAD C3)
   TCP con escuchar/aceptar  [~] en codigo           bmo-pila/src/tcp
                             el metal espera (G5)    (PLAN_RED_TX G5)
   DNS                       [X] en el anfitrion     bmo-pila/src/dns.rs
   TLS 1.3, X.509            NO EXISTEN              (bmo-cripto, cabecera)
   PNG                       [X]                     bmo-imagen/src/png.rs
   JPEG                      [X] SOLO baseline       bmo-imagen/src/jpeg.rs
                             el progresivo, NO
   video MPEG-1              [?] hay que mirarlo     ga10x/src/trabajos/video.rs
   UTF-8 y emojis al pintar  NO                      bmo-dibujo
   microfono                 NO (el audio solo SALE) PLAN_EL_SONIDO
   F: = ESTRATOS             nombres largos SI       userland/estratos.rs
   escribir en ESTRATOS      un fichero nace ENTERO  userland/estratos.rs
                             de un bloque de RAM     (crear_desde)
                             y NO hay "agregar un
                             trozo"
   FAT32 (sys/)              8.3, y no agrega a un   kernel fsys/fs.rs,
                             fichero ya cerrado      charla.rs
   una app en UNA carpeta    NO EXISTE: cualquier    kernel obj/file.rs,
                             proceso abre cualquier  obj/directory.rs
                             ruta, `d:` incluido
   red para una app de F3    NO: lo lanzado desde    kernel task/autoridad.rs
                             Ring 3 nace con         (NINGUNA = sin RED)
                             autoridad NINGUNA
```

*** **Las dos ultimas filas son las que mas pesan**, y el esbozo no las veia.
"HERMES recibe la capacidad de `F:/hermes/` y nada mas del disco" no se puede
escribir hoy: el kernel **no tiene** una carpeta como capacidad. Y una app que
lanza el escritorio no tiene red. Las dos se resuelven con piezas nuevas del
kernel (H3 y H4), no con cuidado en la app.

---

## 2. Lo que se corrigio del esbozo del 01-10

```text
   ESBOZO 01-10                        AHORA                       POR QUE
   ----------------------------------------------------------------------------
   "cada BMO-X tiene una clave          una clave X25519 FIJA       para demostrar
    Ed25519"                            por maquina                 quien eres con
                                                                    Ed25519 hay que
                                                                    FIRMAR, y C3
                                                                    dice que BMO-X
                                                                    no firma
   "TCP con X25519 + AES-GCM"           el saludo Noise (XX la      sin autenticar
                                        primera vez, IK con un      el saludo, el
                                        amigo ya aceptado)          vecino de la LAN
                                                                    se pone en medio
   "fuera de casa, TLS 1.3 o el         fuera de casa, EL MISMO     TLS no existe,
    relevo de la ANTENA"                HERMES/1 por un relevo      X.509 es para
                                        HERMES propio (H11)         servidores, y
                                                                    la ANTENA es
                                                                    otra cosa
   "llega por trozos a la cuarentena    ESTRATOS aprende a          hoy un fichero
    F:/hermes/entrantes/"               escribir por trozos (H8a)   de ESTRATOS
                                                                    nace entero de
                                                                    la RAM: 2 GiB
                                                                    no caben
   "HERMES recibe la capacidad de       la carpeta como capacidad   hoy cualquier
    F:/hermes/"                         en el kernel (H3)           proceso abre
                                                                    cualquier ruta
   "OFERTA (nombre, bytes, sha256,      el tipo lo decide el JUEZ   el tipo lo
    tipo)" y AUTOMATICO por tipo        mirando los bytes           escribe quien
                                                                    manda
   el nombre del fichero que llega      lo pone quien RECIBE        un nombre
                                        (sacado de su suma); el     ajeno es una
                                        del amigo solo se muestra    ruta ajena
   "los emojis van solos"               viajan solos; PINTARLOS     bmo-dibujo no
                                        es un atlas propio (H7)     sabe UTF-8
   "el muro pide leer PNG/JPEG"         ya se leen; falta el        bmo-imagen
                                        JPEG progresivo             existe
```

---

## 3. Las tres jaulas

```text
        la red
          |
   +------v---------------------------+
   |  LA PUERTA HERMES   (servicio)   |  autoridad RED, y nada mas
   |  TCP, el saludo Noise, la clave  |  ve: clave.bin, amigos.txt y
   |  privada, la lista de amigos     |  ESCRIBE en entrantes/
   |  NO decodifica fotos ni video    |
   +------+---------------------------+
          | bmo-cola: mensajes ya abiertos
   +------v---------------------------+     +---------------------------+
   |  HERMES (la app de F3)           |     |  EL JUEZ DE LA CUARENTENA |
   |  la cara: mensajes, tertulias,   |     |  mira los bytes de verdad |
   |  muro, canal, envios             |     |  (PNG, JPEG, video) y la  |
   |  autoridad NINGUNA: sin red,     |     |  suma; mueve a fotos/,    |
   |  sin clave, sin LANZAR           |     |  videos/, muro/ o BORRA   |
   |  ve: charla/ fotos/ videos/ muro/|     |  ve: entrantes/ y nada mas|
   +----------------------------------+     +---------------------------+
```

*** **Por que tres y no uno.** Lo que mas falla en una app de mensajes es lo
que LEE: un decodificador de JPEG o de video con un fichero hecho a mala idea.
Aqui ese codigo vive en el JUEZ, que no tiene red ni clave, y si se cae se cae
solo. La PUERTA, que tiene la clave y el cable, no decodifica nada. Y la APP,
que es la que mas lineas tiene, no tiene ni lo uno ni lo otro.

- **Ningun `.exe` ni `.bex` sale de la cuarentena.** Ni con AUTOMATICO ni con
  permiso: el juez solo conoce tres salidas (fotos, videos, muro) y BORRAR.
  Ninguna de las tres jaulas tiene la autoridad LANZAR.
- **La clave privada no sale de la PUERTA.** Ni a la app, ni al disco en
  claro fuera de `F:/hermes/`. Y es una clave de ACUERDO (X25519), no de FIRMA:
  con ella no se puede falsificar un `.bex`, y C3 sigue entero.

---

## 4. El protocolo HERMES/1

### 4.1 Quien eres

Cada BMO-X genera UNA clave X25519 fija con el AZAR la primera vez que abre
HERMES. Su huella (los primeros bytes de su SHA-256, en grupos) es lo que se
lee en voz alta o se compara en las dos pantallas. **Un amigo es una huella
aceptada una vez**, y desde entonces una clave distinta con el mismo nombre es
un AVISO rojo, no un amigo.

### 4.2 El saludo

El patron de Noise, sobre lo que `bmo-cripto` ya tiene: X25519 para el
acuerdo, HKDF-SHA256 para las claves, AES-256-GCM para cifrar.

```text
   la primera vez   XX   los dos se muestran la clave fija, cifrada
   con un amigo     IK   el que llama ya sabe la clave del otro
   cada sesion      claves EFIMERAS nuevas, del AZAR: lo de ayer no se
                    descifra con lo de hoy
   cada direccion   su clave y su contador; el nonce es el contador y
                    NUNCA se repite (gcm.rs lo exige). Contador agotado =
                    se corta y se saluda otra vez
```

*** **Antes del saludo no se lee nada.** Ni un nombre, ni un verbo, ni un
largo. Una clave que no esta en `amigos.txt` y no es una invitacion abierta a
mano en la pantalla corta la conexion en el primer mensaje. El puerto que
escucha solo sabe decir el saludo.

### 4.3 Lo que viaja dentro

```text
   TEXTO     UTF-8, hasta 2 KiB; los emojis son UTF-8 y van solos
   ZUMBIDO   sin cuerpo; la ventana tiembla, destella y suena. Uno cada
             10 s por amigo como mucho: el resto se cuenta, no se ejecuta
   GUINO     un numero de un catalogo cerrado; nada que interpretar
   OFERTA    nombre (solo para mostrar), bytes, sha256. El TIPO no viaja
   SI / NO   la respuesta a una OFERTA
   TROZO     hasta 60 KiB de un envio aceptado, con su numero (64 no
             caben: un mensaje de Noise mide 65.535 con su etiqueta)
   PIDE      el muro, el canal o una PAGINA de un amigo: lo que ESE amigo
             publica. Una pagina llega como CARA (seccion 7)
   REACCION  el numero de un mensaje y UN emoji; uno por persona y mensaje,
             y mandar el mismo otra vez lo quita (como WhatsApp)
```

Los bytes exactos de cada verbo estan en `platform/shared/bmo-hermes/src/trama.rs`,
y la medida exacta de cada mensaje del saludo en `Patron::medida_vacia`.

Lista blanca, como `bmo-pila`: un verbo que no esta aqui, un largo que se
pasa, un trozo fuera de orden o un UTF-8 roto cortan la conexion, y **cada
corte tiene su nombre** (L6i/L6j).

---

## 5. Un envio pide permiso

```text
   1  OFERTA llega          la app muestra: quien, cuanto, el nombre (de el)
   2  MANUAL                se pregunta cada vez
      AUTOMATICO            por amigo; acepta hasta 2 GiB y deja al JUEZ
                            decidir si es foto o video
   3  NO                    no se guarda ni un byte
   4  SI                    los trozos llegan a F:/hermes/entrantes/<suma>
                            (nombre puesto aqui, no por quien manda)
   5  el JUEZ               suma == sha256 de la OFERTA, y los primeros bytes
                            dicen PNG, JPEG o video. Si no: BORRAR
   6  fuera                 F:/hermes/fotos, videos o muro, y ahora si con
                            el nombre largo que puso el amigo, ya juzgado
```

[!] **Hoy esto no se puede escribir en ESTRATOS.** Un fichero de ESTRATOS
nace entero desde un bloque de memoria (`crear_desde`): un video de 2 GiB
pediria 2 GiB de RAM de una vez. Hay dos salidas, y se elige en H8a:

```text
   A  un ESCRITOR POR TROZOS en ESTRATOS: abrir, agregar, cerrar. La pieza
      buena, y la que sirve tambien a PROTON-X y al TALLER
   B  cada trozo es un fichero suelto en entrantes/, y el JUEZ los junta al
      final. Sin tocar ESTRATOS, pero el disco paga el doble un momento
```

Con las dos, un envio cortado sigue donde se quedo: lo que ya llego esta en
el disco con su numero.

---

## 6. La carpeta

```text
   F:/hermes/
     clave.bin        la clave X25519 de esta maquina (solo la PUERTA)
     amigos.txt       una linea por amigo: huella, nombre, MANUAL/AUTO
     charla/          una conversacion por amigo o tertulia
     entrantes/       la cuarentena (solo la PUERTA escribe, solo el JUEZ lee)
     fotos/  videos/  lo que el juez dejo pasar
     muro/            lo que ESTA maquina publica para sus amigos
```

En ESTRATOS, con nombres largos. Con H3, para HERMES `F:/hermes/` **es la
raiz**: `sys/`, `d:` y el resto de los discos no existen para ninguna de las
tres jaulas. Y cada jaula ve solo SU parte: la PUERTA no ve `fotos/` y el
JUEZ no ve `clave.bin`.

---

## 7. Las PAGINAS: un navegador sin Google

> El propietario (03-10): *"vamos a poner emojis tipicos que ya tiene
> Microsoft y WhatsApp [...] pero alli en "+" o algo simple es construir tu
> propia pagina ya con .maqueta que es HTML + CSS es suficiente no? [...]
> vamos a mejorar MAS y MAS porque eso es entre comillas navegador en BARE
> metal sin Google"*.

### 7.1 La respuesta: SI basta, y la mitad ya esta escrita

Un navegador manda el documento **y trae el motor que lo maqueta**: HTML,
CSS, JavaScript, fuentes, imagenes de cualquier sitio. Por eso un navegador
son millones de lineas, y por eso cada pagina puede hacer casi cualquier cosa
en tu maquina.

BMO-X ya tiene la otra forma, y la tiene ESCRITA:

```text
   MAQUETA            .maqueta = HTML + CSS en un fichero, con LISTA CERRADA
                      (toolchain/tools/maqueta, docs/componente/LA_MAQUETA_EXIGE.md)
   la CARA            la maquetacion YA RESUELTA: rectangulos, letras y golpes
                      (platform/shared/bmo-maqueta-cara). ~1 KB por pantalla
   el lector          desconfia de cada numero: una CARA corrupta no tumba a
                      quien la pinta
```

Una pagina de HERMES es eso: **quien la escribe la compila, y lo que viaja es
la CARA**. Quien la recibe no maqueta, no interpreta HTML ni CSS, no ejecuta
nada: lee una CARA con el lector que ya desconfia y pinta rectangulos y
letras con `bmo-dibujo`. Con la cuenta del formato (cabecera 20 B, trazo
20 B, golpe 12 B, y las cadenas), las cinco paginas de la maqueta salen entre
437 y 782 bytes.

*** **Y lo que una pagina no puede tener no es un filtro: no existe.** No hay
`<script>`, ni eventos, ni `<a href>`, ni `<img src>`, ni `url()`. MAQUETA
"rechaza lo que no entiende" (seccion 0 de su contrato), y esas etiquetas no
estan en la lista. Ni Google, ni anuncios, ni rastreadores, ni cookies: no
porque se bloqueen, sino porque no hay con que escribirlos.

### 7.2 Lo que le falta a MAQUETA para hacer paginas (H13)

Medido contra `LA_MAQUETA_EXIGE.md`, que se escribio para el escritorio:

```text
   FALTA                      PROPUESTA                         POR QUE ASI
   -------------------------------------------------------------------------
   ir a otra pagina           destino="hermes://amigo/pagina"   el id YA es la
                              en un <div> o <span> con id       clave de la tabla
                                                                de golpeo: un
                                                                golpe con destino
   fotos                      imagen="muro/nombre" en un <div>  solo del muro de
                              con width y height                quien publica,
                                                                ya juzgadas
   letra mas grande           font-size: 16 | 32 | 48 | 64 px   una sola letra
                                                                de 8x16, por
                                                                escalas enteras
   parrafos                   white-space: normal               hoy la
                                                                comprobacion B
                                                                rechaza el texto
                                                                que no cabe
   paginas largas             el visor desplaza; la CARA ya     nada nuevo en el
                              lleva su alto                     formato
```

El contrato dice como entra algo nuevo: *"Agregar algo a MAQUETA empieza por
anadirlo a este fichero"*. Y la CARA pasa a la version 2 (un golpe con
destino, un trazo de imagen), porque el lector compara la version por
IGUALDAD.

### 7.3 Donde corre el compilador (H14)

Hoy MAQUETA es del anfitrion (`std`). Para que el `+` de HERMES publique una
pagina, MAQUETA tiene que correr en BMO-X: sus cinco crates en `no_std`,
dentro de la jaula de la APP. Quien RECIBE no necesita el compilador: solo el
lector de la CARA. El trabajo pesado y el codigo grande se quedan en la
maquina de quien escribe.

### 7.4 Los emojis (H7)

```text
   el CATALOGO   Unicode hasta Emoji 15.0: 1.870, en 9 grupos, con tonos de
                 piel. Nombres en castellano de CLDR (via emojibase, MIT):
                 el buscador encuentra "corazon" sin acento
   el DIBUJO     los de WhatsApp son de Meta y no se pueden copiar. Los de
                 Microsoft (Fluent Emoji) se publican con licencia MIT:
                 se comprueba al traerlos, y el atlas se hace en el
                 anfitrion y vive en ESTRATOS
   el PROTOCOLO  nada nuevo: un emoji es UTF-8 dentro de TEXTO, o el unico
                 cuerpo de una REACCION
```

### 7.5 La ONDA: toda la musica, y nunca se duerme

Pedido el 03-10: *"como Spotify, inspiracion para tener musica en total [...]
mp3, todos sonidos [...] con animacion unica que represente"*; y despues:
*"que encuentren TODO el disco en FAT32 y ESTRATOS"*, y *"se mantenga siempre
despierto ese audio para que sigan jugando [...] una notificacion escondida en
la pantalla con animacion en tiempo real [...] pausa o reproducir y control
del audio [...] y recomendacion"*.

```text
   la ONDA       una seccion de F3: listas tuyas y de tus amigos (llegan por
                 HERMES/1 como cualquier envio, con su JUEZ), sin cuentas,
                 sin anuncios, sin algoritmo. Cada cancion, lista y sonido
                 del sistema dibuja su audio a su manera
   EN TU DISCO   la orden `sonidos` del escritorio: recorre DATOS (FAT32) y
                 ESTRATOS, lee 64 bytes de cada fichero y dice que es por
                 DENTRO: SUENA, OFICIAL (y que falta), NO OFICIAL (y por
                 que), y que nombres MIENTEN. En codigo el 03-10 (H16)
   la PASTILLA   el audio es del ESCRITORIO, no de una ventana: cerrar F3 o
                 abrir un juego no lo para. Una pastilla escondida arriba
                 asoma al pasar el raton o al cambiar de cancion; pausa,
                 siguiente, volumen y una recomendacion de un amigo, sin
                 quitar el foco. Mientras un juego suena, la musica baja
   el ORIGEN     cada formato tiene su escalon y no se finge ninguno:
                 WAV es M1 (despues de A1, el tubo del auricular), MP3 es
                 M2 y M2b de PLAN_MEDIOS, FLAC y Vorbis van despues, Opus va
                 con la voz (H12), AAC con el CANAL (M6 de PLAN_LA_3060)
```

[!] **Por que el censo NO es de HERMES.** Recorrer el disco entero es
justo lo que H3 le prohibe a la app de F3: su raiz es `F:/hermes/` y `d:` le
dice NO. Por eso `sonidos` es una orden del ESCRITORIO, que ya ve los dos
volumenes; HERMES solo recibe lo que el propietario le pase. Y como Ring 3 no
escribe en la CABINA (solo la lee), el censo sale en Ejecutar y `save` lo
guarda.

Las recomendaciones las dice un amigo a mano, con su frase, y llegan como un
verbo mas de HERMES/1. Ninguna maquina elige por ti.

---

## 8. Los escalones, desde el principio

Antes de la red, la cara y la jaula. Antes del metal, el anfitrion.

- [~] **H0 -- las decisiones, por escrito.** Propuestas el 02-10 en las
      secciones 2 a 6: clave X25519 fija (C3 no se toca), saludo Noise, tres
      jaulas, nada de ANTENA. **Como se sabe:** el propietario las da por
      buenas o las cambia, y este plan se corrige el mismo dia.
- [x] **H1 -- la maqueta.** HECHO el 02-10:
      [`../arte/maqueta_hermes.html`](../arte/maqueta_hermes.html). F3 abre
      HERMES y F4 la LUDOTECA; la entrada con el gato y su glitch, mensajes con
      emojis, guinos y ZUMBIDO, tertulias, el MURO, el CANAL, los ENVIOS con su
      cuarentena, los amigos con su huella y las tres jaulas en vivo. Y desde
      el 03-10: los emojis completos con buscador, tonos y recientes, las
      reacciones, el menu `+`, y PAGINAS (el visor `hermes://` y un editor de
      `.maqueta` que juzga con el contrato y marca la propuesta H13). Y el
      03-10 por la tarde: el inicio de PAGINAS con buscador y atajos, y el
      CANAL a fondo (H10). Y el 03-10 por la noche: la ONDA (seccion 7.5),
      que suena de verdad con WebAudio, EN TU DISCO con el censo, y la
      PASTILLA, que sigue sonando con HERMES cerrado o con la LUDOTECA
      abierta. **Como
      se sabe:** el propietario la abre y dice si es la cara.
- [x] **H2 -- `bmo-hermes`, puro y con banco.** HECHO el 03-10:
      `platform/shared/bmo-hermes`, `no_std` y sin `unsafe`, encima de
      `bmo-cripto` y de nada mas. El saludo Noise XX/IK como maquina de
      estados (`noise.rs`), los nueve verbos de la seccion 4.3 (`trama.rs`),
      el marco de dos bytes para TCP (`marco.rs`), la huella (`huella.rs`) y
      el grifo del ZUMBIDO (`grifo.rs`), con 23 rechazos con nombre. **Como
      se sabe:** `cargo test` en ese crate: los vectores publicos de Noise
      para `25519_AESGCM_SHA256` (XX e IK, de cacophony y snow) salen byte a
      byte; el que se pone en medio no pasa por amigo; un bit tocado en
      cualquier byte del saludo lo tumba; y `tests/hostile.rs` (70.000 casos)
      no lo hace caer ni abre basura. Compila para `x86_64-unknown-uefi`.
- [~] **H3 -- la carpeta como capacidad.** EN CODIGO el 03-10; falta el metal.
      Un proceso nace con una RAIZ y todo lo que nombra se resuelve debajo:
      - el juez, puro: `platform/shared/bmo-raiz-juicio`. La ruta no se
        COMPARA con la raiz, se CONSTRUYE dentro, y se rechaza todo tramo
        que pueda subir. Medido en el arbol: FAT32 sigue `.` y `..`,
        `to_8_3("...")` da `..`, y `to_8_3(".. .")` tambien -- este ultimo
        lo encontro la prueba de las cien mil rutas contra una FAT32 de
        mentira, con la regla de "solo puntos" ya escrita. Tambien fuera:
        cualquier letra de unidad (`d:` es el disco Personal), blancos en
        el borde y controles;
      - el kernel: `task/raiz.rs` la fija en `proc.rs` justo despues de dar
        el pid y antes de crear la tarea (no hay un instante suelto), la
        aplica en `ruta_tomar`, el unico sitio por donde entra una ruta, y
        la olvida en `cap::revoke_all` junto a la autoridad. Las cuatro
        operaciones que ven el volumen entero sin ruta (cursor de ESTRATOS,
        sus nombres, sellar y el disco) y MARCAR/VOLVER dicen NO a un
        proceso encerrado (`syscall/op_raiz.rs`);
      - la pone QUIEN LANZA: `TASK_OP_RAIZ_HIJO` (0x3C, en `bmo-abi`) y
        `bmo::raiz_del_siguiente_hijo(b"hermes")`; un hijo de un encerrado
        nace, como poco, igual de encerrado.
      **Como se sabe:** en el Ryzen, el escritorio lanza HERMES con raiz
      `hermes`; HERMES pide `sys/director.bex`, `d:` y `../sys` y recibe NO
      con su motivo en CABINA (`raiz`); pide `charla/x.txt` y lo abre.
      Hasta entonces: `cargo test` en el juez (8 pruebas, cien mil rutas) y
      el kernel compila y enlaza con los mismos 12 avisos.
- [ ] **H4 -- la PUERTA HERMES.** Un servicio que nace con RED (y nada mas),
      con raiz en `F:/hermes/`, que escucha en un puerto propio (no el 7117) y
      habla con la app por `bmo-cola`. **Como se sabe:** la app de F3 sigue
      con autoridad NINGUNA y aun asi manda un mensaje a la PUERTA.
- [~] **H5 -- la app en F3, en una sola maquina.** EN CODIGO el 03-10; falta
      el metal. El propietario: *"F3 su ventana propia [...] dale para
      terminar"*. Lo que hay:
      - `Ultra_userspace/apps/hermes` -> `sys/hermes.bex` (en `build.ps1` y
        en `ejemplos.ps1`, que ahora enlaza TALLER, LUDOTECA y HERMES con una
        sola funcion). F3 la lanza o la alterna desde `keys::windows`, antes
        del foco; sin marco, como la LUDOTECA.
      - La ENTRADA: el gato con ALAS (nueve plumas por lado y su membrana,
        que se abren y aletean) y el glitch; HERMES se escribe debajo.
      - El riel de las NUEVE secciones de la maqueta, cada una con su gesto
        (el bocadillo que escribe, las cabezas que botan, la foto que se
        voltea, la tele, las barras, la pagina, la flecha, los dos que se
        orbitan, los barrotes) y su ENTRADA: las notas llegan por la
        derecha, las tertulias suben, el MURO voltea seis postales, el CANAL
        se enciende como una tele (raya, se abre, barras y nieve, SIN
        EMISION), los pasos de ENVIOS se encienden en orden, la huella se
        escribe, las jaulas caen con rebote.
      - DE VERDAD: tus notas y cuatro tertulias se guardan en
        `sys/hermsg.txt` (canal, hora, texto); el ZUMBIDO (uno cada 10 s)
        sacude, destella en rosa y SUENA (pide `aviso zumbido` al
        escritorio); la ONDA lista las trece piezas y un clic la toca en el
        ESCRITORIO (pide `fondo N`; `desktop::pide` acepta ahora esas dos
        lineas), con un ecualizador de 32 barras que sigue al medidor del
        maestro. Cerrar HERMES no corta la musica.
      - LO QUE AUN NO, dicho en su seccion con su escalon: amigos y huella
        (H6), MURO (H9), CANAL (H10), paginas (H13-H15), ENVIOS (H8), la
        PUERTA (H4) y el JUEZ (H8). Nada de gente inventada.
      - Visto en el anfitrion con un arnes que pinta los mismos
        `pintar.rs` y `entrada.rs` a PNG (cada seccion, la entrada, y la
        medida minima 960 x 600, sin panico con las comprobaciones de
        desborde). Pila de Ring 3: 3.784 B de 65.536.
      Lo que sigue de abajo es el pedido original y su prueba:
      `sys/hermes.bex`
      (`Ultra_userspace/apps/hermes`), F3 la abre o la esconde como F4 a la
      LUDOTECA; `consumo` se sigue escribiendo en Ejecutar. Entrada con alas,
      mensajes y tertulias guardados, zumbido con sonido. Y, como pidio el
      propietario el 03-10 (*"TODOS tienen animaciones unicas"*), cada cosa con
      su gesto propio, como cada tienda de la LUDOTECA: cada seccion del riel
      se mueve a su manera y ENTRA a su manera (los mensajes desde su lado, el
      MURO voltea las fotos, el CANAL se enciende como una tele, los pasos de
      ENVIOS se encienden en orden, la huella se escribe, las jaulas bajan), y
      cada amigo lleva el suyo. La maqueta los muestra todos. **Como se sabe:** en
      el Ryzen, F3 abre HERMES, se escribe, se cierra con Alt+F4, se vuelve a
      abrir y lo escrito sigue ahi.
- [ ] **H6 -- texto entre dos BMO-X en la LAN.** Pide G5 en el metal. **Como
      se sabe:** dos maquinas se aceptan por huella, se escriben, y una
      captura del cable no tiene ni una palabra en claro.
- [ ] **H7 -- los emojis se pintan.** `bmo-dibujo` aprende UTF-8 y un atlas
      de emojis sacado de Fluent Emoji (seccion 7.4), con el selector, el
      buscador en castellano, los tonos de piel y las REACCIONES. **Como se
      sabe:** el mensaje del banco con emojis y tonos se ve igual en las dos
      maquinas, y una reaccion llega y se quita.
- [ ] **H8a -- ESTRATOS escribe por trozos.** La salida A o la B de la
      seccion 5. **Como se sabe:** un fichero de 4 GiB entra en ESTRATOS con
      64 KiB de RAM prestada, y su suma cuadra.
- [ ] **H8 -- los envios con permiso.** OFERTA, MANUAL/AUTOMATICO, la
      cuarentena y el JUEZ como proceso aparte. **Como se sabe:** una foto
      pasa; un `.bex` renombrado a `.jpg` se BORRA; un NO no deja ni un byte.
- [ ] **H9 -- el MURO.** PNG y JPEG ya se leen; falta el JPEG progresivo, que
      es el de muchas fotos. **Como se sabe:** el muro de un amigo se ve con
      sus corazones, pedido por HERMES/1 a SU maquina.
- [ ] **H10 -- el CANAL.** El video viaja COMPRIMIDO (H.264 o AV1 en un
      `.mp4` con su indice delante) por TROZOS, el JUEZ comprueba la suma y
      la caja, y lo descomprime la 3060 con NVDEC, no la CPU: es M6 de
      `PLAN_LA_3060.md` (V1 la caja, V2 las cabeceras, V3 NVDEC, V4 AV1), y
      no depende de internet, solo de la 3060. Y la cara, como la maqueta del 03-10
      (*"me gusta ese nombre, pero puede ser mas profundo cada uno"*): una
      pagina por canal (videos, listas, sobre), y cada video con capitulos,
      reacciones, comentarios entre amigos y "a continuacion" sacado SOLO del
      mismo canal y sus listas. Sin anuncios, sin algoritmo, sin contador de
      visitas. **Como se sabe:** un video de un
      amigo, en AV1, se ve entero en el Ryzen mientras llega; la CPU no
      descomprime un fotograma (lo dice la fila de M6) y la caja la abrio el
      JUEZ en su jaula.
- [ ] **H11 -- fuera de casa.** Un relevo HERMES propio (no la ANTENA) que
      solo reenvia bytes ya cifrados y no sabe leer ninguno. **Como se sabe:**
      dos BMO-X en dos casas se escriben, y el relevo no tiene la clave de
      ninguno de los dos.
- [ ] **H13 -- MAQUETA para paginas.** Las cinco cosas de la seccion 7.2,
      primero en `LA_MAQUETA_EXIGE.md` y despues en los crates, y la CARA
      version 2. **Como se sabe:** las cinco paginas de la maqueta compilan
      con la MAQUETA de verdad y dan ficheros dorados; las que rechaza la
      maqueta (`<script>`, `<a href>`, `margin`, `rgba`) las rechaza tambien.
- [ ] **H14 -- MAQUETA en BMO-X.** Los cinco crates en `no_std`, dentro de la
      jaula de HERMES: el `+` compila y publica en `F:/hermes/paginas/`.
      **Como se sabe:** compilar la misma pagina en el Ryzen y en el anfitrion
      da los mismos bytes (el contrato ya exige determinismo).
- [ ] **H15 -- el visor de paginas.** PIDE pagina -> la CARA -> el lector de
      `bmo-maqueta-cara` -> `bmo-dibujo`; los golpes con destino navegan,
      atras y adelante, y `https://` contesta que eso no es HERMES. Su inicio,
      como el de Google y sin Google: un buscador que busca en TU maquina
      sobre lo que ya llego (las palabras no salen) y los atajos, con Ctrl+L,
      Alt+flechas, Alt+Inicio y `/`. **Como se
      sabe:** `hermes://nova/inicio` se ve en el Ryzen igual que la vista de
      MAQUETA en el anfitrion, y una CARA con medidas mentirosas no tumba al
      visor (un `tests/hostile.rs` para el lector).
- [ ] **H12 -- la voz.** Cuando el audio USB tambien ENTRE (microfono), con
      un codec propio. **Como se sabe:** una llamada de un minuto sin cortes.
- [~] **H16 -- el censo de los sonidos.** EN CODIGO el 03-10; falta el
      metal. `bmo-sonido::censo` (puro, en el anfitrion) mira una cabecera de
      64 bytes y da el tipo por sus bytes magicos (WAV con su codec, MP3, FLAC,
      Vorbis, Opus, AAC, M4A, AIFF, MIDI, WMA), el veredicto con su motivo y si
      el nombre miente. La orden `sonidos` del escritorio
      (`commands/sonidos.rs`) recorre DATOS y ESTRATOS con techos que DICE si
      toca. **Como se sabe:** en el Ryzen, `sonidos` lista los WAV de
      `datos/` como SUENA, un `.mp3` renombrado de un WAV como MIENTE, y un
      `.mid` como NO OFICIAL; hasta entonces, 18 pruebas en el anfitrion
      (una de 30.000 cabeceras hostiles) y el director compila.
- [~] **H17 -- el audio que no se duerme.** EN CODIGO el 03-10 lo de
      abajo de la PASTILLA: el atril del FONDO en el kernel (un segundo banco,
      del escritorio, que suena aunque DOOM tenga el sonido), el AGACHE y la
      rampa en `bmo-amplificador`, las diez piezas de la maqueta compuestas
      en enteros por `bmo-fondo`, y la orden `fondo` (S4e de
      `PLAN_EL_SONIDO.md`). Y por la tarde la PASTILLA en el director
      (`scene/pastilla.rs`), escondida arriba, latiendo con el medidor del
      maestro, y el OIDO (S4f) para todo lo que suena. Y el 3D (S7, global
      y por voz) y la VOZ DE BMO-X (S4g): la maqueta la muestra en la ONDA,
      con la cabeza vista desde arriba y el compilador que se oye cruzar.
      Y el NEKO PHONK (S4h): el tema del gato para los avisos y la lista
      "neko phonk" en la ONDA. Falta el metal.
      **Como se sabe:**
      en el Ryzen suena un WAV, se cierra HERMES, se abre DOOM II y sigue
      sonando sin cortes; la PASTILLA lo pausa sin que el juego pierda una
      tecla, y mientras el juego suena la musica baja.
- [ ] **H18 -- la ONDA en F3.** Las listas, la cola, "me gusta", las listas
      que comparte un amigo y sus recomendaciones como verbo de HERMES/1, y
      cada cancion con su animacion. **Como se sabe:** nova comparte una
      lista desde su BMO-X, llega por el JUEZ, y suena en la otra maquina.
