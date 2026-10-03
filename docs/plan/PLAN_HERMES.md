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
      CANAL a fondo (H10). **Como
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
- [ ] **H3 -- la carpeta como capacidad.** El kernel aprende a que un proceso
      nazca con una RAIZ: todo `open` y `dir_abrir` se resuelve debajo de ella
      y `d:` no existe. **Como se sabe:** HERMES pide `sys/director.bex` y
      `d:` y recibe NO con nombre en CABINA; pide `charla/x.txt` y lo abre.
- [ ] **H4 -- la PUERTA HERMES.** Un servicio que nace con RED (y nada mas),
      con raiz en `F:/hermes/`, que escucha en un puerto propio (no el 7117) y
      habla con la app por `bmo-cola`. **Como se sabe:** la app de F3 sigue
      con autoridad NINGUNA y aun asi manda un mensaje a la PUERTA.
- [ ] **H5 -- la app en F3, en una sola maquina.** `sys/hermes.bex`
      (`Ultra_userspace/apps/hermes`), F3 la abre o la esconde como F4 a la
      LUDOTECA; `consumo` se sigue escribiendo en Ejecutar. Entrada con alas,
      mensajes y tertulias guardados, zumbido con sonido. **Como se sabe:** en
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
- [ ] **H10 -- el CANAL.** Primero se lee `ga10x/src/trabajos/video.rs` para
      saber cuanto decodificador hay ya. Y la cara, como la maqueta del 03-10
      (*"me gusta ese nombre, pero puede ser mas profundo cada uno"*): una
      pagina por canal (videos, listas, sobre), y cada video con capitulos,
      reacciones, comentarios entre amigos y "a continuacion" sacado SOLO del
      mismo canal y sus listas. Sin anuncios, sin algoritmo, sin contador de
      visitas. **Como se sabe:** un video de un
      amigo se reproduce entero sin salir de la jaula del JUEZ.
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
