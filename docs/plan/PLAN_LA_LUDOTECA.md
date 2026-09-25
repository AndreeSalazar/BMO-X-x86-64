# PLAN LA LUDOTECA -- los juegos que compraste, en BMO-X, y por donde NO

> Pedido por el propietario (2026-09-25): *"los launchers son Python con
> Electron... Rust como base podria tener un crate simple pero elegante para
> tener el API de GOG y otros, asi BMO-X tendria su app simple para tener
> juegos; lo mismo con Proton de Steam, que es abierto, pero en uso
> correcto"*.

---

## 0. La respuesta corta

**Si a la app, no a meter Proton.** Un launcher (Heroic, Lutris, GOG Galaxy)
hace tres cosas muy distintas, y cada una va a un sitio distinto:

```text
   1  la LISTA      que juegos tienes, sus portadas, sus ficheros
                    -> BMO-X, un crate Rust puro y una app. SI
   2  la DESCARGA   hablar con la tienda: HTTPS, OAuth, su CDN
                    -> la ANTENA (el movil o un PC). BMO-X no tiene TLS:
                       es el muro de la criptografia (README)
   3  EJECUTAR      el juego de Windows o Linux
                    -> NO en BMO-X. Proton es Wine + DXVK + vkd3d sobre un
                       kernel Linux y un Vulkan conforme: traerlo es traer
                       POSIX y Win32 por la puerta de atras
                       (`docs/identidad/ENTRAR_EN_SU_ECOSISTEMA.md`, "Y NO,
                       con Windows no es lo mismo"; `PLAN_CLOUD_LOCAL.md`,
                       seccion 4)
```

## 1. Los tres caminos para JUGAR, del mas limpio al mas lejano

```text
   A  NATIVO: el motor abierto, compilado a .bex, con los DATOS que compraste
      DOOM ya lo hace: el motor es un port GPL fuera del arbol y el WAD va
      al lado (apps/doom1.wad; `build/ejemplos.ps1`). GOG vende DOOM, DOOM
      II, Heretic, Hexen, Quake... con sus WAD y PAK dentro: el juego que
      pagaste, corriendo en un motor que BMO-X compila. ESTE es el uso
      correcto: nada pirata, nada de Windows, y la 3060 ya pinta
   B  STREAMING: el juego corre en TU PC con Windows (o en un Linux con
      Proton, fuera de BMO-X), y BMO-X lo ve y le manda teclado y raton
      Es el modo ESPEJO (S6 de PLAN_CLOUD_LOCAL), como Moonlight / Steam
      Link, y con `gpu video` la 3060 ya pone el fotograma en pantalla
   C  PROTON DENTRO: no. Cuesta un Linux y un Vulkan enteros (NVK tardo ~2
      anios con varios expertos, PLAN_LA_3060 L2), y rompe la promesa de
      la superficie chica (`docs/identidad/LA_COMPATIBILIDAD.md`)
```

## 2. El crate: `bmo-ludoteca` (puro, no_std, con banco)

Lo que Heroic hace en miles de lineas de Electron, aqui es un formato de
lineas y una tabla:

```text
   JUEGO <id> <tienda> <titulo>              uno por juego que tienes
   FICHERO <id> <nombre> <bytes> <sha256>    lo que la antena puede traer
   MOTOR <id> <motor>                        si hay motor nativo (camino A)
```

- La ANTENA habla con la tienda (su HTTPS, su sesion) y le da a BMO-X solo
  estas lineas, como hace con las paginas (`bmo-antena/src/lamina.rs`): el
  mismo lector que dice UNA vez la linea eterna, con cuota y cuarentena.
- La tabla de MOTORES es de BMO-X, no de la tienda: `doom -> DOOM.WAD`,
  `quake -> id1/pak0.pak`... Un juego sin motor sale en la lista marcado
  "por streaming" (camino B), nunca "instalar".
- La SUMA: cada fichero llega con su sha256 y BMO-X la comprueba antes de
  guardarlo (ESTRATOS). Lo que no cuadra no entra.

## 3. Los escalones

- [ ] **J0 -- el formato, puro y con banco.** `platform/shared/bmo-ludoteca`:
      las tres lineas, la tabla de motores y el juez de la suma. **Como se
      sabe:** `cargo test -p bmo-ludoteca` en verde con lineas mutadas.
- [ ] **J1 -- la Biblioteca los muestra.** La Biblioteca del escritorio
      (`scene/data/biblioteca.rs`) lista los juegos de un fichero de lineas
      en `datos/`, con su camino (A nativo, B streaming). Sin red: el fichero
      se copia a mano. **Como se sabe:** DOOM sale como "nativo" y lanza el
      `.bex` de siempre.
- [ ] **J2 -- la antena pide la lista a GOG.** En `toolchain/tools/antena/`,
      con la sesion del propietario, como hacen Heroic y gogdl (su API no
      es publica: lo que la tienda permite es cosa del propietario de la cuenta,
      como el video en `PLAN_CLOUD_LOCAL`). **Como se sabe:** `LUDOTECA` a
      la antena devuelve las lineas de tus juegos.
- [ ] **J3 -- traer los DATOS de un juego nativo.** El WAD o PAK de un juego
      del camino A, por la antena, a ESTRATOS, con su suma. Pide TCP en el
      metal (G5). **Como se sabe:** DOOM II arranca con el `doom2.wad` que
      trajo la antena.
- [ ] **J4 -- el camino B.** Un juego de Windows lanzado en el PC y visto en
      BMO-X: es S6 (ESPEJO) con un fotograma por `gpu video`.

## 4. Lo que NO se hace

- Ningun `.exe` en BMO-X, ni Wine, ni Proton, ni una capa Win32.
- Ningun fichero de un juego en el repositorio: ni WAD ni PAK (DOOM ya lo
  cumple: `build/ejemplos.ps1`, "ni el codigo ni el WAD pueden vivir aqui").
- Nada de saltarse el DRM o las condiciones de una tienda: GOG vende sin DRM
  y es la primera por eso; Steam, solo por el camino B.

---

## 5. "Personal": la cuenta de BMO-X es una CAJA, no un login

El propietario (25-09): *"BMO-X tendria su configuracion automatica de
cuenta, `Personal`, como caja fuerte, y yo decido cuando ponerla en
internet; no login en Google ni nada"*. Encaja con lo que ya hay:

```text
   Personal      vive SOLO en el disco de BMO-X: tu lista de juegos, tus
                 partidas guardadas, tus preferencias, y la LLAVE que empareja
                 BMO-X con TU antena (P0 de PLAN_CLOUD_LOCAL). Nada mas
   el login      NUNCA en BMO-X. La cuenta de GOG la abre la antena, en SU
                 navegador; a BMO-X no llega ni la clave ni el token: le
                 llegan las lineas de `bmo-ludoteca`
   internet      un interruptor que pulsa el propietario, como el GRIFO de la
                 red (el TX detras de una puerta de un solo uso, 14-09).
                 Cerrado por defecto; abierto, solo hacia SU antena
```

Lo que NO promete todavia: que el disco este CIFRADO. Eso pide AES y es el
mismo muro de la criptografia que el TLS. Hasta entonces, "caja fuerte"
quiere decir "no sale de la maquina sin que tu lo digas", no "si te roban
el disco no lo leen".

## 6. El camino B, simple: sacar de Proton SOLO sus pixeles

El propietario: *"no es solo que sean millones de lineas: esos millones
tenemos que VERIFICARLOS, y cambian todo"*. Exacto, y por eso la unica
parte de Proton que entra en BMO-X es su SALIDA:

```text
   Proton por dentro   Wine (la API de Windows, ~25 anios), DXVK (Direct3D
                       9/10/11 -> Vulkan), vkd3d-proton (D3D12 -> Vulkan),
                       FAudio, y todo encima de un kernel Linux, glibc y un
                       Vulkan conforme. Ninguna pieza se "extrae" sola: DXVK
                       sin Vulkan no hace nada, Wine sin POSIX no arranca
   lo que se extrae    los PIXELES que Proton ya dibujo en TU PC, y de vuelta
                       las TECLAS y el RATON. Nada mas cruza
   lo que se verifica  el protocolo: una cabecera fija, un fotograma
                       comprimido (MPEG-1 hoy, H.264 con NVDEC despues) y los
                       eventos de entrada. Cabe en una pagina, con su banco
                       de lineas mutadas como `bmo-antena`; el resto -- los
                       millones -- corre y se actualiza FUERA, sin tocar BMO-X
```

Escalones del B simple, sobre los de la seccion 3:

- [ ] **J4a -- el emisor en el PC.** Un script (ffmpeg captura la ventana del
      juego y la comprime) y el protocolo `ESPEJO/1` en `bmo-antena`, con
      banco. **Como se sabe:** `cliente.py` recibe fotogramas del PC.
- [ ] **J4b -- BMO-X lo ve.** Con TCP en el metal (G5) y un decodificador
      (pl_mpeg, o NVDEC), cada fotograma a la pantalla por `gpu video`.
- [ ] **J4c -- las manos.** Las teclas y el raton de BMO-X vuelven al PC.

## 7. El nombre de la app: LUDOTECA

Elegido por el propietario el 25-09 (entre Recreativa, Ludoteca y Salon).
La app se llama **Ludoteca**, como este plan, y su crate `bmo-ludoteca`.

## 8. "El x86-64 no cambia: se puede analizar" -- lo que es verdad y lo que no

El propietario (25-09): *"todos los frontends van por AST, pero lo que se
emite en x86-64 nunca cambia; entonces se pueden analizar"*. La casa ya
apuesta a eso: el guardian `isa` dice que los frontends son lo unico
agnostico y que TODO lo que se emite es x86-64. Investigado:

```text
   VERDAD     las INSTRUCCIONES de un juego de Windows ya son x86-64 y ya
              corren en el Ryzen tal cual. Por eso Wine se llama "Wine Is
              Not an Emulator": no traduce ni una instruccion. La CPU nunca
              fue el problema
   VERDAD     lo que un binario pide de FUERA esta escrito en el propio
              binario: la tabla de IMPORTACIONES (en un .exe, las DLL y sus
              funciones; en un ELF, las bibliotecas NEEDED y los simbolos
              sin definir). Se lee sin ejecutar nada
   PRECEDENTE Native Client de Google (2009-2020) VERIFICABA codigo maquina
              x86-64 sin su fuente: un validador de unos miles de lineas
              aceptaba o rechazaba un programa entero con reglas fijas
              (saltos alineados a 32 B, accesos a memoria enmascarados, ni
              `syscall` ni `int`). Probaba SEGURIDAD -- que no se sale de su
              caja --, no que funcione; y pedia compilar con SU toolchain:
              un .exe normal no pasaba
```

Y por que eso no hace chico a Proton:

```text
   1  lo que hay que DAR son las importaciones, no las instrucciones: cada
      funcion importada (CreateFileW, D3D11CreateDevice...) tiene que existir
      y hacer lo que Windows hace. Eso ES Wine
   2  se puede verificar QUE pide un binario, no QUE HACE: lo segundo, en
      general, es indecidible (Rice). Native Client verificaba una propiedad
      (la caja), no el comportamiento
   3  el codigo que cambia en marcha: DXVK genera sombreadores al vuelo, y el
      DRM y los antitrampas (Denuvo) se ofuscan y se reescriben solos: ahi el
      analisis estatico no llega
   4  la GPU NO es x86-64: su codigo maquina cambia con cada generacion (el
      SASS de SM86 no es el de SM89). BMO-X ya lo vive: sus programas salen de
      `ptxas -arch=sm_86` y solo valen para esta 3060
```

**Lo que SI vale la pena: medir antes de decidir.** `toolchain/tools/rayosx`
lee un `.exe` o un ELF y dice cuantas funciones pide, de que bibliotecas y
por familia. Medido en el anfitrion el 25-09: hasta `/bin/ls`, un comando
chico, pide **120** funciones de `libc`; el nivel 1 de devorar un ELF
ESTATICO son ~15 llamadas. Esa distancia es la que hay que verificar.

- [x] **J-R -- `rayosx`, la medida.** HECHO el 25-09 en el anfitrion:
      `toolchain/tools/rayosx/rayosx.py`, con su banco (`--prueba`: un PE
      hecho a mano y un ELF del anfitrion). **Como se sabe:** `python
      rayosx.py --prueba` en verde.
- [ ] **J5 -- medir TUS juegos.** Pasar `rayosx` por los `.exe` y los ELF de
      Linux de tus juegos de GOG y apuntar la tabla aqui: cuantos piden solo
      SDL2 + libc + OpenGL/Vulkan (los ELF de Linux suelen ser asi). Si es
      un grupo grande y su superficie cabe en una pagina, se estudia una capa
      SDL2 en Ring 3 (la estrategia B de `ENTRAR_EN_SU_ECOSISTEMA.md`, sin
      tocar el kernel). Si no, camino A o B y nada mas. **Como se sabe:** la
      tabla medida, juego a juego, en esta seccion. Se mide EN WINDOWS,
      donde estan los juegos: `python toolchain\tools\rayosx\rayosx.py
      "C:\Program Files\GOG Galaxy\Games\Cyberpunk 2077"` (una carpeta
      vale: busca sus `.exe`, del mas grande al mas chico, y cuenta tambien
      las importaciones RETRASADAS, donde los juegos grandes ponen media API).

**La primera medida: Cyberpunk 2077 de GOG (25-09, en el Windows del
propietario).** El caso mas dificil que existe, a proposito:

```text
   ejecutable                  funciones  bibliotecas  lo que dice
   bin\x64\Cyberpunk2077.exe    663         36       sistema 589, red 58,
                                                       entrada 12, sonido 4
   tools\redmod\bin\redMod.exe  440         19       D3D11 para las herramientas
   tools\redmod\bin\scc.exe     243         16
   REDprelauncher.exe           837         26       Qt5 + Poco: el lanzador
```

Lo que dicen, leido:

- **663 es la PRIMERA capa.** Detras vienen PhysX (fisica), Bink (video),
  Oodle (compresion), ICU (texto), libcurl (red), REDGalaxy (la tienda) y los
  tres reescaladores de las tres marcas: `sl.interposer` (DLSS, NVIDIA),
  `ffx_fsr3` (AMD) y `libxess` (Intel). Son DLL CERRADAS de terceros que el
  juego TRAE y que piden, a su vez, lo suyo a Windows.
- **Y `d3d12.dll` NO sale en la tabla**, siendo un juego de DirectX 12: la
  abre en marcha (`LoadLibrary` + `GetProcAddress`), y `sl.interposer` se
  mete DELANTE de ella. La tabla dice lo que el programa pide AL ARRANCAR, no
  todo lo que llega a usar. Desde el 25-09 `rayosx` lo avisa (`[!] usa
  LoadLibrary...`) y separa las DLL que TRAE el juego de las del sistema.
- **La conclusion, medida:** entre el camino A (un motor abierto y un WAD:
  0 funciones de Windows) y Cyberpunk hay cientos de funciones en la primera
  capa, decenas de DLL cerradas debajo, y una API grafica cargada en marcha.
  El camino A no es el facil: es el HONESTO -- solo promete lo que se puede
  verificar entero.

**Y el NTFS no hace falta (25-09).** Los juegos de GOG viven en el volumen
NTFS de Windows 11, y BMO-X lee FAT32 y ESTRATOS, no NTFS. Para el camino A
basta con COPIAR, desde Windows, el fichero de datos (`doom2.wad`, `pak0.pak`)
a la particion de datos de BMO-X, como ya se hace con `doom1.wad`; y un juego
del camino B se queda en Windows, que es donde corre. Un lector de NTFS en
BMO-X es mucho codigo para verificar y no desbloquea nada de esto: no se
parte el disco por esto.

---

## 9. Cyberpunk 2077, el JEFE FINAL -- y la escalera hasta el

El propietario (25-09): *"Cyberpunk 2077 que compre es DRM FREE: vamos a
aplicar el metodo DOOM generic con sus datos; es DirectX 12, y NAGA ya lo
tengo en BMO-externo. Seria mi JEFE FINAL"*.

**Por que el metodo DOOM no le sirve (todavia), dicho claro:**

```text
   DOOM        el MOTOR es abierto (id lo libero en 1997, GPL): se compila a
               .bex y lee el WAD que compraste. Datos + motor abierto = juego
   Cyberpunk   SIN DRM quiere decir sin proteccion de copia, NO motor abierto.
               Sus datos (.archive) se pueden LEER -- la comunidad de mods los
               documenta --, pero el REDengine 4 que los dibuja es cerrado. Sin
               motor, los datos son un disco sin tocadiscos
   NAGA        traduce sombreadores entre WGSL, SPIR-V y GLSL; de HLSL solo
               ESCRIBE. Cyberpunk trae sus sombreadores ya compilados en DXIL,
               y eso Naga no lo lee (en Proton lo hace dxil-spirv). Y DirectX
               12 no son solo sombreadores: listas de ordenes, montones de
               descriptores, barreras, estados de pipeline -- vkd3d-proton
               entero, encima de un Vulkan
```

**La escalera: cada jefe deja una pieza para el siguiente.** Todos por el
camino A (motor abierto + los datos que compraste; comprobar en TU
biblioteca de GOG que los tienes):

```text
   1  DOOM, DOOM II       doomgeneric, CPU                  [HECHO: corre]
   2  Heretic, Hexen      el mismo linaje (Chocolate Doom)  casi gratis
   3  Quake               WinQuake, su RENDERIZADOR DE SOFTWARE: el primer 3D
                          de verdad, en la CPU; pide la coma flotante de BMO C
                          y ~40.000 lineas de C
   4  Quake II            Yamagi, tambien con renderizador de software
   5  Quake con la 3060   el primer juego que dibuja la TARJETA: una capa
                          chica de "GL" sobre AMPERE_B -- la profundidad (T2b),
                          las texturas (T3) y muchos triangulos. Todo lo que
                          ya sale en `gpu raster` y `gpu color`, hecho juego
   6  un juego con sombreadores   motor abierto que pide programas de la
                          tarjeta: el BSF emitiendo SASS (M5d) y VERRANO
   ...
   FINAL  Cyberpunk 2077  hoy: por el camino B (streaming desde el PC donde
                          corre). Nativo: pediria la API de Windows (Wine), D3D12
                          (vkd3d) y un Vulkan conforme para la 3060 -- anios, y
                          fuera de la identidad de BMO-X (seccion 0). Es el jefe
                          que marca la DIRECCION: cada escalon de abajo es una
                          pieza de la que haria falta
```

**Lo que el propietario TIENE (25-09, su Steam y su escritorio), juego a
juego.** La pregunta es siempre la misma: hay un MOTOR ABIERTO que lea sus
datos?

```text
   Half-Life (1998)          SI. Su motor (GoldSrc) viene de Quake, y Xash3D
                             FWGS es un motor ABIERTO y compatible, con
                             renderizador de SOFTWARE y de GL; la logica del
                             juego sale de hlsdk-portable (el SDK de Valve,
                             que su licencia deja usar con Half-Life). Se
                             copian TUS datos (la carpeta `valve/`). Es el
                             camino A con un juego que YA compraste
   Half-Life 2, HL2 DM,      NO. Source y Source 2 son cerrados; el codigo
   HL DM Source, Black Mesa, filtrado de Source (2020) NO se usa: no es de uso
   Garry's Mod, L4D2, CS2,   legal. Camino B (streaming)
   Dota 2
   Cyberpunk 2077 (GOG)      NO por A: REDengine 4 cerrado. Camino B
   Warframe, Zenless Zone    NO: servicios en linea con antitrampas; ni por A
   Zero, Apex, Call of Duty  ni por B tiene sentido en BMO-X
   el resto de la lista      motores cerrados (Unreal, Unity, RE Engine...):
                             camino B
```

**Y dos jefes que NO hay que comprar**, gratis y legales:

```text
   Freedoom        un DOOM entero, libre (BSD): el mismo .bex de DOOM, otro WAD
   Quake shareware el episodio 1 de Quake, que id dejo repartir libremente:
                   su `id1/pak0.pak` basta para el jefe 3
```

La escalera, con lo que hay de verdad: **DOOM** (hecho) -> **Freedoom** ->
**Quake shareware** por software -> **Half-Life** (Xash3D, primero por
software) -> **Half-Life con la 3060** (su renderizador GL sobre AMPERE_B) ->
... -> **Cyberpunk 2077**.

- [ ] **L0 -- Freedoom.** `freedoom1.wad` y `freedoom2.wad` en
      `BMO-externo\doom\`, y el build (`build/ejemplos.ps1`, preparado el
      25-09) los deja en `apps\` como `freedm1.wad` y `freedm2.wad`: el FAT32
      de BMO-X busca por nombre 8.3. Falta que el port (`doomgeneric_bmo.c`,
      fuera del arbol) abra ese WAD en vez de `doom1.wad`. **Como se sabe:**
      el primer mapa de Freedoom se juega en el Ryzen con el mismo `.bex`.
- [ ] **L3 -- el jefe que ya compraste: Half-Life por software.** Xash3D
      FWGS (su renderizador de software) y hlsdk-portable, compilados para
      BMO-X, con la carpeta `valve/` copiada del Half-Life de Steam. Pide lo
      de Quake (coma flotante de BMO C) y C++ para el SDK. Antes de empezar,
      las licencias leidas y apuntadas aqui. **Como se sabe:** el tren del
      principio (`c0a0`) llega a Black Mesa en el Ryzen.
- [ ] **L1 -- el jefe 2: Heretic o Hexen.** Chocolate Heretic/Hexen
      compilado a `.bex` como DOOM, con `heretic.wad` o `hexen.wad` de GOG.
      **Como se sabe:** el primer nivel se juega en el Ryzen.
- [ ] **L2 -- el jefe 3: Quake por software.** WinQuake con BMO C, y
      `id1/pak0.pak` (el del shareware basta: id lo dejo repartir). Paso 1,
      la coma flotante de BMO C, en marcha el 25-09: la SONDA DE QUAKE
      (`toolchain/lang/c/emisor-x86_64/src/tests/sonda_quake.rs`) ejecuta lo
      que pide su `mathlib.c` -- `vec3_t`, `DotProduct`, `CrossProduct`,
      structs, globales, ternarios, asignaciones compuestas -- y encontro
      CINCO fallos del compilador, todos arreglados: el ternario flotante
      tumbaba el compilador, un `float` local con `{...}` quedaba a cero,
      `t[i] *= 2` y `p->x += y` daban cero, los globales flotantes no se
      escribian, y `float f = 1;` global valia 1.4e-45. Paso 2, `math.h`, lo
      EXACTO hecho el 25-09: `sqrt` (la instruccion `sqrtsd`), `trunc`,
      `floor`, `ceil` (con el signo del cero). 17 de 18 casos; faltan
      `printf("%f")` y las series (`sin`, `cos`, `atan2`, `pow`), que van con
      la MISMA tabla que el emisor de SPIR-V (`math::table`). **Como se sabe:** `start.bsp` se recorre en el
      Ryzen, y el `[perf]` dice los fps de la CPU.

---

## 10. StarCraft, StarCraft II y "Vulkan puro" (25-09)

El propietario: *"analizas StarCraft, el original con Brood War, y
StarCraft 2? Y podria enfocar en Vulkan puro para BMO-X"*. Con la pregunta
de siempre -- hay un motor ABIERTO que lea tus datos? -- y lo que no esta
comprobado, dicho:

```text
   StarCraft + Brood War   CANDIDATO al camino A. Es 2D: sprites de 8 bits
   (1998)                  en 640x480 con paleta, o sea lo que BMO-X ya
                           pinta con la CPU (como DOOM, sin la 3060). Hay
                           una reimplementacion abierta de su motor, OpenBW
                           (C++), que lee los ficheros de datos ORIGINALES.
                           Y Blizzard regalo el clasico en 2017. POR
                           COMPROBAR antes de prometer nada: la licencia de
                           OpenBW, como se consiguen hoy los datos del
                           clasico, y en que formato vienen (MPQ el viejo,
                           CASC el Remastered). Pide C++ (como Half-Life) y
                           un lector de MPQ
   StarCraft II            NO por A: motor cerrado, DirectX 9/11 en Windows,
                           y ninguna reimplementacion. Es gratis desde 2017,
                           pero eso no abre el motor. Camino B (streaming)
```

**"Vulkan puro" tiene sentido, con UNA condicion: que sea el Vulkan DE ESTA
CASA, no el de todos.** Son dos metas (`PLAN_VULKAN.md`, "las dos metas"):

```text
   un Vulkan CONFORME   el que pasa el CTS de Khronos y arranca juegos de
                        otros: cada juego pide SU lista de caracteristicas
                        (descriptorIndexing, timelineSemaphore, dynamic
                        Rendering...) y sin una no arranca. NVK tardo ~2
                        anios con varios expertos. Y aun asi, un juego de
                        Windows seguiria pidiendo Win32 encima
   un Vulkan de la CASA un SUBCONJUNTO sobre AMPERE_B -- el que ya dibuja
                        `gpu raster` y `gpu color` -- con SPIR-V como
                        entrada, que es justo lo que ya leen el lector de
                        SPIR-V y el BSF. Rechaza con motivo lo que no tiene
                        (como el juez de PLAN_EL_SOMBREADOR)
```

Y el puente entre los dos tiene nombre: **vkQuake**, Quake (GPL) con un
renderizador de Vulkan. Seria el jefe 5 por Vulkan en vez de por "GL": el
MISMO juego del jefe 3, ahora dibujado por la 3060 con SPIR-V. POR
COMPROBAR: que version de Vulkan y que extensiones pide la version de
vkQuake que se elija (las nuevas piden mas); el subconjunto se dibuja a
partir de esa lista medida, no al reves.

- [ ] **L4 -- la lista de vkQuake.** Leer (sin compilar) que pide vkQuake a
      Vulkan: version, extensiones y `VkPhysicalDeviceFeatures`, y apuntarla
      aqui como la primera lista del Vulkan de la casa. **Como se sabe:** la
      tabla en esta seccion, con la version de vkQuake y cada funcion
      `vk*` que llama.
- [ ] **L5 -- StarCraft, comprobado.** La licencia de OpenBW, de donde salen
      hoy los datos del clasico y en que formato; y `rayosx` sobre el
      `StarCraft.exe` clasico para comparar. **Como se sabe:** las tres
      respuestas apuntadas en esta seccion, con su fuente.
