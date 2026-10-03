# PLAN LA ANTENA AOT -- el HONOR, optimizado al extremo y en un lenguaje de verdad nativo

> Escrito el **2026-10-03**. El propietario: *"estudiar la ANTENA por completo
> para optimizar al extremo; no se que lenguaje de programacion AOT puro hay
> en el celular para que alimente a mi BMO-X, porque mi celular HONOR es el
> intermedio para ir a ver YouTube en Google"*.
>
> Se escribio despues de leer la antena de hoy (`toolchain/tools/antena`:
> `antena.py` 334 lineas, `navegador.py` 321, `lamina_juez.py` 93,
> `arrancar.sh`, `GUIA_MOVIL.md`), `PLAN_CLOUD_LOCAL.md`, `PLAN_NAVEGAR.md` y
> `PLAN_LA_RED_SOLA.md` (D1: la antena lleva Google, YouTube y su navegador).

---

# 0. LA RESPUESTA CORTA

```text
   EL LENGUAJE    RUST, compilado para `aarch64-linux-android` (Tier 2 en
                  rustc, con `std`): codigo maquina ARM64, sin maquina
                  virtual, sin recolector, sin JIT. Es el MISMO lenguaje de
                  BMO-X, asi que la antena usa LOS MISMOS crates: el juez de
                  la lamina, ANTENA/1, la LUDOTECA, la cripto -- el mismo
                  codigo a los dos lados del cable, no una traduccion
   HOY            Python + ffmpeg + un Chromium dentro de Termux (con proot):
                  tres interpretes y un navegador entero para mandar pixeles
   LA FORMA       UN binario estatico, `antena`, que corre en Termux desde el
                  primer dia; despues, dentro de una APP chica (AA0) cuyo
                  unico Kotlin es lo que Android OBLIGA (permisos, captura de
                  pantalla, la WebView)
   YOUTUBE        en el REPRODUCTOR OFICIAL del movil (la app o la web), y la
                  antena manda sus PIXELES a BMO-X (modo ESPEJO, S6): asi
                  YouTube no sale de sus condiciones y BMO-X no toca Google
```

---

# 1. QUE LENGUAJE: los AOT de verdad que hay en un Android (03-10)

AOT "puro" quiere decir: lo que corre en el movil es codigo maquina ARM64
generado ANTES, sin interprete, sin JIT y sin una maquina virtual debajo.

| lenguaje | AOT puro? | lo que trae debajo | encaja con BMO-X? |
|---|---|---|---|
| **Rust** (`aarch64-linux-android`) | SI | nada: ni GC ni runtime | **SI: es el de la casa**; los crates `no_std` de BMO-X compilan tal cual |
| C / C++ (NDK, Clang) | SI | nada | a medias: sin la seguridad de memoria, y BMO C solo emite x86-64 |
| Swift (SDK oficial de Android desde Swift 6.3, marzo 2026) | SI | conteo de referencias (ARC) | no: otra casa, y ningun crate en comun |
| Go | SI | su runtime y un recolector | no: el GC pausa justo el envio de fotogramas |
| Kotlin/Native, Dart AOT (Flutter), .NET NativeAOT | SI | runtime y recolector | no, por lo mismo |
| Java / Kotlin normal | NO | ART: dex compilado a medias en la instalacion, mas JIT | solo para lo que Android exige (seccion 3) |
| Python (hoy) | NO | el interprete | se va |

**Rust**, y no por moda: es el unico de la tabla que comparte CODIGO con
BMO-X. El juez de la lamina que BMO-X usa (`bmo-antena/src/lamina.rs`) hoy
tiene un gemelo en Python (`lamina_juez.py`) que hay que mantener a mano; con
Rust, la antena juzga con EL MISMO crate, y no hay dos jueces que puedan
discrepar. Ya era la decision de la LUDOTECA (J2: *"Rust como el nuevo
Python"*).

---

# 2. LO QUE PESA HOY, Y A DONDE VA

```text
   pieza de hoy                     pesa                       luego
   -------------------------------  -------------------------  ----------------------------------
   antena.py (ANTENA/1, la carpeta) un interprete, siempre     `antena` en Rust: bmo-antena
                                    despierto                  (el MISMO crate que BMO-X)
   lamina_juez.py                   un segundo juez a mano     se borra: lo juzga bmo-antena
   navegador.py + Chromium en       un navegador ENTERO en un  la WebView del sistema (el
   Termux con proot                 Linux de mentira           Chromium que Android ya trae),
                                                               desde la app de AA0
   ffmpeg a MPEG-1 640x360 en       el procesador del movil    1) MPEG-1 en Rust con el
   el procesador                    entero, y calor            codificador por hardware para
                                                               DESCODIFICAR (MediaCodec)
                                                               2) CRUDO por la LAN (seccion 4)
```

---

# 3. LA FORMA: un binario, y una app chica cuando haga falta

- **Fase A (sin app):** `antena` es UN binario estatico `aarch64-linux-android`
  que se copia a Termux y se arranca como hoy (`arrancar.sh`). Hace todo lo
  de `antena.py` con los crates de la casa. Sin root, sin proot.
- **Fase B (AA0, la app):** lo que Android solo deja hacer desde su API de
  Java -- capturar la pantalla (MediaProjection), la WebView, el servicio en
  primer plano que no se duerme, el codec por hardware (MediaCodec) -- va en
  un envoltorio de Kotlin CHICO que llama a la biblioteca de Rust por JNI. Lo
  que decide y lo que juzga sigue en Rust.

[!] La regla de la LUDOTECA (*"ni un crate de fuera, ni en la antena"*)
sigue: la antena en Rust usa crates de la casa. Lo que Android trae de
fabrica (la WebView, MediaCodec) no es un crate: es el sistema del movil, y
se llama, no se copia.

---

# 4. OPTIMIZAR AL EXTREMO: lo que de verdad cuesta, medido en el cable

El cuello hoy no es el lenguaje: es **el enlace a 10 Mbit** de la RTL8168
(`PLAN_LA_RED_SOLA.md` RS5b). Con el gigabit, las cuentas cambian enteras:

```text
   que manda la antena            por fotograma   a 30 fps        cabe en
   -----------------------------  --------------  --------------  -----------------------------
   MPEG-1 640x360 (hoy)           ~4-10 KB        ~1-2 Mbit/s     10 Mbit: si, y es lo unico
   CRUDO YUV 4:2:0 640x360        338 KB          ~81 Mbit/s      100 Mbit o mas
   CRUDO YUV 4:2:0 1280x720       1,38 MB         ~330 Mbit/s     gigabit por cable (el movil
                                                                  con adaptador USB-C a
                                                                  Ethernet), no por WiFi
```

**El modo CRUDO** es el extremo de verdad: el movil NO codifica nada (cero
calor, cero perdida, cero latencia de codec) y BMO-X no descodifica nada --
pone los planos YUV en la 3060 o los convierte en la CPU y los pinta. Pide
dos cosas: el gigabit (RS5b) y la red en su nucleo (RS5c). Hasta entonces,
MPEG-1.

**Copia cero en el movil:** la captura (MediaProjection) llega en un bufer de
hardware; se lee UNA vez y se escribe al socket, sin pasar por Java.
**Copia cero en BMO-X:** cada trama llega al corral de RX del GATE RED; el
fotograma se monta directamente donde la 3060 lo lee (una copia, la del
DMA), como la ONDA lee su banco.

---

# 5. YOUTUBE: por que el ESPEJO y no "descargar el video"

`antena.py` ya lo dice: *"no descarga nada de Internet [...] las plataformas
de video tienen sus condiciones de uso"*. YouTube no permite bajar sus
videos ni reproducirlos fuera de su reproductor. El camino que las respeta:
**el movil reproduce YouTube en SU app (o en su web), con todo lo suyo, y la
antena manda a BMO-X los PIXELES de la pantalla del movil** -- como un
Chromecast o una pantalla espejo -- y el teclado y el raton de BMO-X vuelven
al movil (S6 de `PLAN_CLOUD_LOCAL.md`). BMO-X nunca habla con Google; Google
habla con el movil, por la salida que el propietario elija
(`PLAN_LA_CASA_ESCONDIDA.md` CE5: datos moviles, para que no vea la IP de
casa).

---

# 6. LOS ESCALONES

- [ ] **AO0 -- el banco de la antena, en Rust y en el anfitrion.** Un crate
      `bmo-antena-movil` (nombre provisional) que hace lo de `antena.py` con
      `bmo-antena`: la carpeta, LISTA/PIDE/PAGINA, la lista blanca de UNA IP,
      los plazos y la cuarentena. **Como se sabe:** `cargo test` contra las
      mismas conversaciones que hoy pasan con `cliente.py`, y el juez de la
      lamina es el MISMO crate que en BMO-X (`lamina_juez.py` se borra).
- [ ] **AO1 -- el binario en el HONOR (Fase A).** `cargo build --target
      aarch64-linux-android` con el NDK, copiado a Termux y arrancado por
      `arrancar.sh`. **Como se sabe:** `red hola <ip-del-movil>` desde BMO-X
      contesta `HOLA ANTENA/1 <nombre>` con la antena en Rust, y `top` en el
      movil muestra un proceso en vez de Python + ffmpeg.
- [ ] **AO2 -- la app chica (AA0, Fase B).** El envoltorio de Kotlin con la
      WebView del sistema, el servicio que no se duerme y MediaProjection;
      el resto, la biblioteca de Rust por JNI. El codigo, en el repositorio
      (AA0 de `PLAN_NAVEGAR.md`). **Como se sabe:** `PAGINA <url>` funciona
      sin Termux ni proot.
- [ ] **AO3 -- el ESPEJO con MPEG-1** (es S6, con lo que BMO-X ya sabe
      mostrar). **Como se sabe:** YouTube suena y se ve en el Ryzen,
      reproducido en el movil.
- [ ] **AO4 -- el modo CRUDO.** Despues de RS5b y RS5c: YUV sin codec por la
      LAN, a 640x360 primero y a 720p con el movil por cable Ethernet.
      **Como se sabe:** la latencia de un toque en BMO-X a verlo en pantalla,
      medida (CABINA), por debajo de la del MPEG-1.
- [ ] **AO5 -- medir el movil.** Vatios, temperatura y CPU del HONOR con
      MPEG-1 y con CRUDO, en una hoja de `docs/metal/`. Lo extremo se mide,
      no se supone.
