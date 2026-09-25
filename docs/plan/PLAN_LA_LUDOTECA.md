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

- [x] **L4 -- la lista de vkQuake.** HECHO el 25-09, leido del codigo sin
      compilar: la tabla y la lista entera en la seccion 12. **Como se sabe:**
      la tabla de la seccion 12, con la version (0.50, commit del
      2016-08-07, y la de `master` del 2026-09-23) y cada funcion `vk*`.
- [ ] **L5 -- StarCraft, comprobado.** La licencia de OpenBW, de donde salen
      hoy los datos del clasico y en que formato; y `rayosx` sobre el
      `StarCraft.exe` clasico para comparar. **Como se sabe:** las tres
      respuestas apuntadas en esta seccion, con su fuente.
      **Medido el 25-09** (clon de `github.com/openbw/openbw`, ultimo commit
      2026-08-13): el repositorio **NO trae licencia** -- ni `LICENSE` ni
      cabecera con permiso; su README manda a `OpenBW/bwapi`, que es LGPL-3
      pero es la API de los bots, no el motor. Sin licencia, lo que rige es
      "todos los derechos reservados": **no se puede compilar y repartir con
      BMO-X sin permiso de sus autores**. El motor son ~36.000 lineas de C++
      en cabeceras (plantillas, `<memory>`, `<functional>`,
      `<unordered_map>`) y la cara ~3.400 sobre SDL2; lee los MPQ el mismo
      (`data_loading.h`, sin StormLib) y pide TRES ficheros del clasico:
      `StarDat.mpq`, `BrooDat.mpq` y `Patch_rt.mpq`. Falta: de donde salen
      hoy esos tres (el Remastered los trae en CASC, no en MPQ) y `rayosx`
      sobre el `StarCraft.exe` clasico.

## 11. El efecto domino: los clasicos con motor abierto, en orden (25-09)

La idea es correcta y tiene nombre: **cada juego desbloquea UNA pieza que
el siguiente ya da por hecha**. No se salta al jefe final; se tumba la
primera ficha y cada una empuja a la siguiente. El orden no es por fecha ni
por fama: es por **que capacidad pide** cada motor a BMO-X. Ya lo decia
`platform/drivers/gpu/rdna4/PLAN_VULKAN.md` ("para juegos antiguos el
camino es SDL + motores abiertos") y `docs/identidad/QUE_DESBLOQUEA.md`
(OpenTTD y ScummVM piden compilacion separada); esta seccion lo ordena.

Regla de las columnas: **motor** = el codigo que se compila (tiene que ser
abierto); **datos** = lo que dice "gratis" si hay datos libres o shareware
legales, o "comprar" si hace falta el juego original. La licencia va
marcada POR COMPROBAR donde no es GPL limpia (motores sacados por
ingenieria inversa, o licencias propias como la de Build).

### Ficha 1 -- 2D por la CPU (lo que ya hace DOOM: pixeles al framebuffer)

| Motor | Juego | Licencia | Datos | Que desbloquea |
|---|---|---|---|---|
| **Wolf4SDL** | Wolfenstein 3D | GPL (id, 2012) -- POR COMPROBAR la version | gratis (shareware) | **rayos por columna**: el abuelo de DOOM, mas chico -- el censo mas facil |
| **SDLPoP** | Prince of Persia | GPL (desensamblado) -- POR COMPROBAR | comprar | animacion por fotogramas, paletas VGA |
| **Omnispeak** / Commander Genius | Commander Keen 4-6 | GPL (reimplementacion) | Keen 1 shareware gratis | desplazamiento suave por azulejos |
| **Vanilla Conquer** | C&C y Red Alert | GPL (EA, 2020) | **gratis**: EA los libero como freeware en 2007-2008 | ★ estrategia en tiempo real; audio en mezcla; **C++ acotado** |
| **Stratagus + Wargus** | Warcraft II | GPL | comprar (GOG lo vende) | motor RTS generico + **Lua** |
| **fheroes2** | Heroes of Might and Magic II | GPL | la demo sirve | C++ con STL, por turnos (no pide tiempo real) |
| **DevilutionX** | Diablo | POR COMPROBAR (reimplementacion) | la version shareware sirve | lector de MPQ -- la **misma** familia de archivo que StarCraft (seccion 10) |
| **OpenBW** | StarCraft | POR COMPROBAR (L5) | comprar / gratis de Blizzard | ya en la seccion 10 |
| **CorsixTH** | Theme Hospital | MIT | la demo sirve | **Lua embebido** de verdad |
| **OpenXcom** | UFO / X-COM | GPL | comprar | C++ grande con YAML |
| **OpenTTD** | Transport Tycoon Deluxe | GPL | ★ **gratis**: OpenGFX + OpenSFX + OpenMSX | **compilacion separada** (~600k C++, QUE_DESBLOQUEA palanca 2) |
| **ScummVM** | cientos de aventuras | GPL | varias gratis (Beneath a Steel Sky, Flight of the Amazon Queen, Lure of the Temptress) | un **interprete**: un motor, muchos juegos |

### Ficha 2 -- 3D por software (la CPU pinta; float de verdad)

| Motor | Juego | Licencia | Datos | Que desbloquea |
|---|---|---|---|---|
| doomgeneric (**hecho**) | DOOM / Freedoom | GPL | gratis | la ficha que ya cayo |
| Chocolate Heretic / Hexen | Heretic, Hexen | GPL (id/Raven, 2008) | shareware gratis | el mismo motor con inventario: casi gratis despues de DOOM |
| **Quake** (WinQuake) | Quake | GPL | shareware gratis (pak0) | ★ **float + 3D real** -- la ficha de la sonda (seccion 10, jefe 3) |
| **Yamagi Quake II** | Quake II | GPL | demo gratis | renderizador por software `ref_soft` **y** GL en el mismo motor: la bisagra hacia la GPU |
| **EDuke32** | Duke Nukem 3D | GPL + licencia Build -- POR COMPROBAR | shareware gratis | motor Build (sectores, espejos) |
| **NBlood** | Blood | POR COMPROBAR (Build) | comprar | el mismo Build: si cae Duke, Blood viene casi solo |
| **DXX-Rebirth** | Descent 1-2 | mixta (Parallax + GPL) -- POR COMPROBAR | shareware gratis | 6 grados de libertad por software |
| **TheForceEngine** | Dark Forces | GPL | comprar | motor Jedi de LucasArts |
| **Xash3D FWGS** | Half-Life | motor GPL; la DLL del juego es del SDK de Valve -- POR COMPROBAR | **ya lo tienes** (Steam) | tiene renderizador por **software**: Half-Life sin GPU es posible |

### Ficha 3 -- GPU con tuberia fija ("GL de 1999")

| Motor | Juego | Licencia | Datos | Que desbloquea |
|---|---|---|---|---|
| **Yamagi Quake II** (ref_gl1) | Quake II | GPL | demo gratis | el MISMO juego de la ficha 2, ahora por la 3060: se compara pixel a pixel |
| **ioquake3** | Quake III Arena | GPL | demo gratis / comprar | GL con multitextura; red |
| **OpenJK** | Jedi Outcast / Academy | GPL | comprar | idTech3 con mas estado |
| Xash3D (ref_gl) | Half-Life | igual que arriba | ya lo tienes | el Half-Life de la ficha 2, por GPU |

### Ficha 4 -- GPU con sombreadores (SPIR-V, el Vulkan de la casa)

| Motor | Juego | Licencia | Datos | Que desbloquea |
|---|---|---|---|---|
| **vkQuake** | Quake | GPL | shareware gratis | ★ el puente: el Quake de la ficha 2, por Vulkan (L4) |
| **dhewm3** | Doom 3 | GPL (con terminos extra de id) | comprar | sombras por stencil, sombreadores |
| **OpenMW** | Morrowind | GPL | comprar | mundo abierto; pide mucho C++ y OpenSceneGraph -- ficha tardia |

Asi se lee: **cada fila de una ficha reusa lo que dejo la anterior**. Quake
por software deja el float y el lector PAK; Quake II deja el mismo juego
con dos renderizadores (la prueba honesta: software contra GPU, mismos
pixeles); vkQuake deja la primera lista medida de Vulkan. Y los juegos con
**datos gratis** (Wolf3D, Freedoom, Quake, C&C, OpenTTD) son los que se
prueban primero: no dependen de que compres nada, y cualquiera puede
repetir la prueba.

### Vulkan contra Direct3D: por que Vulkan, y no por moda

| | Vulkan | Direct3D 12 |
|---|---|---|
| Especificacion | abierta (Khronos), publica entera | Microsoft publica DirectX-Specs, pero el API es COM de Windows |
| Pruebas de conformidad | **CTS abierto**: BMO-X puede correrlo contra su subconjunto | el HLK es de Windows |
| Sombreadores | **SPIR-V**, abierto -- BMO-X ya lo lee | DXIL (bitcode de LLVM); el compilador DXC es abierto. Microsoft anuncio (2024) que el Shader Model 7 adoptara SPIR-V -- POR COMPROBAR cuando llegue |
| Controladores abiertos para aprender | Mesa **NVK** (NVIDIA) y RADV (AMD), en C, leibles | ninguno: en Linux D3D se **traduce** a Vulkan (DXVK, vkd3d) |
| Encaje con BMO-X | explicito: buffers de comandos, semaforos, barreras -- lo MISMO que ya hacen GPFIFO y los semaforos de `gpu pantalla` | tambien explicito (D3D12 y Vulkan son primos), pero amarrado a DXGI/WDDM |
| Juegos abiertos | vkQuake, ports con Vulkan, y todo lo GL se puede subir despues | casi ninguno: un juego D3D es un juego de Windows |

Conclusion: **no es que D3D12 sea "peor" como esquema** -- en la GPU son casi
lo mismo. Es que Vulkan esta **abierto por los cuatro lados** (especificacion,
pruebas, sombreadores, controladores de referencia) y D3D esta cerrado por
el lado que importa: vive dentro de Windows. Por eso el camino de BMO-X es
el de las fichas: primero CPU, luego un Vulkan de la casa medido juego a
juego. Y un juego D3D (Cyberpunk) sigue siendo el jefe final por la
seccion 9, no por esta tabla.

- [ ] **L6 -- la primera ficha nueva: Wolf4SDL.** Traerlo a `BMO-externo`
      con el shareware, pasar el censo del compilador C (como la sonda de
      Quake) y apuntar que le falta. **Como se sabe:** la lista de fallos
      (o cero) apuntada aqui, con la version de Wolf4SDL.
- [ ] **L7 -- licencias de la tabla, comprobadas.** Para cada fila marcada
      POR COMPROBAR, leer el archivo de licencia del repositorio del motor y
      apuntar el nombre exacto. **Como se sabe:** esta seccion sin ningun
      POR COMPROBAR de licencia, cada fila con su fuente.
- [ ] **L8 -- C&C con datos libres.** Comprobar que Vanilla Conquer arranca
      con los datos freeware de EA (no con el Remastered) y cuanto C++ pide.
      **Como se sabe:** el tamanio del C++ y la lista de rasgos de C++ que
      usa, apuntados aqui.

---

## 12. La escalera por VULKAN, medida (25-09)

El propietario: *"empezar con algo simple, para mejorar, y hasta el final
BOSS"*. Medido en el codigo de vkQuake (GPL-2), sin compilarlo:

| | vkQuake **0.50** (2016-08-07) | vkQuake `master` (2026-09-23) |
|---|---|---|
| Vulkan | 1.0 | 1.0, o 1.1 si la hay |
| funciones `vk*` distintas | **67** | 95 |
| extensiones | solo `surface` y `swapchain` (+ la de su ventana) | ~20: `swapchain` obligada; opcionales `descriptor_indexing`, `push_descriptor`, `float16_int8`, `subgroup_size_control`, `present_wait2`... y **trazado de rayos** (`acceleration_structure`, `ray_query`) |
| sombreadores | **12** (vertices y fragmentos) | 51 (con computo) |
| obligado | una cola grafica y un formato de profundidad `D24_UNORM_S8` o `D32_SFLOAT_S8` | lo mismo |
| funciones de SDL2 | 75 | 187 |
| hilos | no | si (su sistema de tareas) |
| lineas (C) | ~68.000 | ~168.000 |

**El escalon simple es la 0.50**: el mismo Quake, con la tercera parte de lo
que pide la de hoy. Sus 67 funciones, por familia (sin el prefijo `vk`):

```text
   instancia y aparato  10  CreateInstance EnumeratePhysicalDevices CreateDevice
                            GetDeviceQueue GetPhysicalDeviceProperties
                            GetPhysicalDeviceQueueFamilyProperties
                            Enumerate{Instance,Device}ExtensionProperties
                            Get{Instance,Device}ProcAddr
   memoria y buferes     9  AllocateMemory FreeMemory MapMemory FlushMappedMemoryRanges
                            CreateBuffer DestroyBuffer BindBufferMemory
                            GetBufferMemoryRequirements GetPhysicalDeviceMemoryProperties
   imagenes              7  CreateImage DestroyImage CreateImageView DestroyImageView
                            BindImageMemory GetImageMemoryRequirements CreateSampler
   tuberia               7  CreateRenderPass CreateFramebuffer DestroyFramebuffer
                            CreatePipelineLayout CreateGraphicsPipelines
                            CreateShaderModule DestroyShaderModule
   descriptores          5  CreateDescriptorSetLayout CreateDescriptorPool
                            AllocateDescriptorSets UpdateDescriptorSets FreeDescriptorSets
   ordenes              21  CreateCommandPool AllocateCommandBuffers Begin/EndCommandBuffer
                            CmdBeginRenderPass CmdNextSubpass CmdEndRenderPass
                            CmdBindPipeline CmdBindDescriptorSets CmdPushConstants
                            CmdBindVertexBuffers CmdBindIndexBuffer CmdDraw CmdDrawIndexed
                            CmdSetViewport CmdSetScissor CmdSetDepthBias
                            CmdCopyBuffer CmdCopyBufferToImage CmdPipelineBarrier
                            QueueSubmit
   sincronia             5  CreateFence ResetFences WaitForFences CreateSemaphore
                            DeviceWaitIdle
   pantalla              3  QueuePresentKHR y la superficie de su ventana (Win32 o
                            XCB); las del swapchain las busca por puntero
```

Y a que pieza de la 3060 de BMO-X cae cada familia -- lo que ya esta y lo
que falta:

```text
   QueueSubmit, Fence, Semaphore   el GPFIFO y los semaforos de `gpu pantalla`   YA
   CreateShaderModule              SPIR-V: el lector y el BSF (PLAN_EL_SOMBREADOR) EN OBRAS
   CreateGraphicsPipelines, CmdDraw AMPERE_B: `gpu raster` y `gpu color`          YA (1 triangulo)
   CmdCopyBuffer(ToImage)          el motor de copia (COPY2, `gpu copia`)          YA
   CreateImage + Sampler           TEXTURAS: T3                                   FALTA
   el formato D24S8                PROFUNDIDAD: T2b                               FALTA
   miles de CmdDraw por fotograma  A3 + C4 de PLAN_LA_3060_AFINADA                FALTA
   QueuePresentKHR                 el volcado de hoy, o el page flip (M2)         YA / M2
   la ventana, la entrada, el sonido  una capa tipo SDL2 en Ring 3 (75 funciones) FALTA
```

La escalera por Vulkan, cada escalon con su lista medida antes de empezarlo:

```text
   1  vkQuake 0.50      Quake shareware (gratis)    67 vk, 12 sombreadores
   2  Quake3e           Quake III, demo gratis      por medir
   3  vkQuake master    el mismo Quake, mas moderno 95 vk, computo, hilos
   4  RBDOOM-3-BFG      Doom 3 BFG (comprar)        Vulkan 1.2, por medir
   FINAL  Quake II RTX  demo gratis, GPL (NVIDIA)   trazado de rayos: los nucleos
                        RT de la 3060. El jefe final ABIERTO: el mismo objetivo
                        que Cyberpunk (la tarjeta al maximo), verificable entero
```

- [ ] **L9 -- la capa SDL2, medida.** Las 75 funciones `SDL_*` de vkQuake
      0.50, por familia (ventana, entrada, sonido, tiempo, ficheros), y cuales
      ya tienen su pieza en BMO-X. **Como se sabe:** la tabla apuntada aqui.

## 13. ESTRATOS para la Ludoteca: que ya guarda y que le falta (25-09)

El propietario: *"ESTRATOS seria para organizar pero falta, no? puede
guardar TODOS esos elementos?"*. Leido el formato
(`platform/drivers/storage/estratos`) y el kernel
(`Ultra_kernel_x86-64/kernel/src/ring0/fsys/estratos`):

```text
   YA      ficheros de mas de 100 GB (4 niveles de indireccion de 85 punteros),
           nombres de hasta 63 letras (el FAT32 de BMO-X busca por 8.3), cada
           bloque con su BLAKE3, la historia entera (volver a una version),
           crear, carpetas, copiar un fichero desde el FAT32, renombrar, quitar
   FALTA   E1  una carpeta con MAS DE 36 entradas: hoy caben en un bloque
               (ENTRADAS_POR_BLOQUE = 4096 / 112)
           E2  copiar una CARPETA entera desde el FAT32, con sus nombres largos
           E3  el RECOLECTOR: en copia-en-escritura lo borrado no vuelve solo;
               sin el, quitar un juego no libera su sitio
           E4  el MANIFIESTO de cada juego (`:manifiesto` ya existe en el
               formato): las lineas de `bmo-ludoteca` y la suma de cada
               fichero, comprobadas al abrir
```

Lo que eso quiere decir, juego a juego: **los primeros jefes no esperan a
ESTRATOS.** DOOM (1 WAD), Quake (`id1/pak0.pak`), StarCraft (3 MPQ) son
pocos ficheros grandes, y eso ESTRATOS ya lo guarda. E1 y E2 hacen falta con
Half-Life (su `valve/` tiene cientos de ficheros por carpeta); E3 en cuanto
se instalen y quiten juegos grandes; E4 es lo que convierte ESTRATOS en la
caja de la Ludoteca.

- [x] **E1 -- carpetas de mas de 36 entradas.** El bloque de entradas como
      flujo de varios bloques, con el mismo arbol que `:datos`
      (`bmo_estratos::flujo`). **Como se sabe:** `cargo test -p
      bmo-estratos` con una carpeta de 1000 entradas leida entera, y una de
      36 igual que hoy (sin cambiar el formato de las viejas).
      *Hecho 25-09:* `bmo_estratos::carpeta` (examinar, reescribir, buscar),
      9 pruebas: 1000 entradas creadas UNA A UNA por el camino del kernel y
      leidas enteras; 36 byte a byte igual que `entradas_con`; una de 3200
      (dos niveles) republicada; quitar, renombrar y repuntar justo en la
      entrada que cruza de bloque. El kernel reescribe cada nivel de la ruta
      asi y busca a trozos; `estratos-fmt` ya no se niega (401 ficheros en
      una carpeta, `--verificar` OK). Queda el panel de Datos, que pinta las
      64 primeras y avisa. Falta la prueba de metal: crear la entrada 37.
- [ ] **E2 -- copiar una carpeta entera.** De FAT32 a ESTRATOS, recursiva y
      con nombres largos. **Como se sabe:** `valve/` de Half-Life copiada y
      su arbol igual, fichero a fichero, con su suma.


## 14. FRAPS-X, y la grabacion por NVENC (25-09)

El propietario: *"FRAPS + OBS propio + Action!, los 3 fusionados, exclusivo en
BMO-X"*, y despues *"como que no se puede con la 3060? investigar!"*.

**Hecho:** el contador y el banco (`platform/shared/bmo-fraps`, 8 pruebas;
`desktop::fraps` y `scene::fraps` en el DIRECTOR). Mide lo de delante --la app
a pantalla completa, la del foco o el escritorio-- por la secuencia de su
superficie, y se pinta encima de todo, un juego a pantalla completa incluido.
`Ctrl+Shift+F` lo pone y lo mueve de esquina; `Ctrl+Shift+B` hace el banco
(minimo, media, maximo, 1 % y 0,1 % bajos) a `capturas/banNNNNN.csv`. Las
capturas (Impr Pant, PNG propio) ya existian.

### NVENC en la 3060: SI es razonable, y lo que cambio

Lo que decia el 25-09 al principio del dia --*"su interfaz es cerrada"*-- era lo que
sabia de memoria, y **estaba viejo**. Leido hoy:

```text
   open-gpu-doc  classes/video/clc7b7.h     la clase NVENC de Ampere (GA10x):
                                            sus metodos (SET_IN_CUR_PIC,
                                            SET_OUT_BITSTREAM, EXECUTE...)
                 classes/video/nvenc_drv.h  3.830 lineas, licencia MIT,
                                            publicado 2026-06-09: las
                                            estructuras que lee el motor
                                            (pic_setup de H.264 y H.265,
                                            control de ritmo, estado), y
                                            NV_NVENC_DRV_MAGIC 0xC7B70006
                                            para NVENC 7.3 = la 3060
   open-gpu-kernel-modules  resource_list.h  NVC7B7_VIDEO_ENCODER se reserva
                                            DEBAJO DE UN CANAL, sin privilegio:
                                            el mismo camino que ya recorren el
                                            motor de copia y el de computo
   el metal (PLAN_LA_3060)                  GET_ENGINES_V2 ya devolvio NVENC0
```

Lo que el motor pide: la imagen en **NV12** (luma + croma, lineal por bloques o
en teselas de 16x16); el motor escribe la cabecera de cada slice y sus datos, y
la SPS/PPS las escribe el software. H.264 y H.265 en la 3060 (AV1 es de la
serie 40).

Y por que es la buena para BMO-X, que no es Windows: **OBS sin NVIDIA usa x264,
que es la CPU**, y aqui hoy corre un nucleo que tambien es el del juego. NVENC es
silicio aparte: no le quita ni un ciclo a la CPU ni a los nucleos de la GPU. Y
el fotograma YA esta en la VRAM: el volcado lo sube cada fotograma.

- [ ] **N1 -- el motor responde.** Un canal en el runlist de NVENC0, el objeto
      `NVC7B7_VIDEO_ENCODER`, un `NOP` y un semaforo. La lista blanca del
      contrato y `la_3060.py` lo aceptan con su motivo. **Como se sabe:** `gpu
      nvenc` dice que el semaforo llego; si el GSP pide un ucode del motor que
      no trae, se dice con el codigo que devuelve.
- [ ] **N2 -- una imagen.** El escritorio a NV12 por el motor de computo, una
      IDR con QP fijo, la SPS/PPS escritas aqui. **Como se sabe:** un `.h264`
      en `capturas/` que VLC abre en Windows.
- [ ] **N3 -- un video.** P-frames con su referencia y control de ritmo, 60
      por segundo. **Como se sabe:** FRAPS-X graba un minuto de DOOM y el banco
      de ese minuto no baja.
- [ ] **N4 -- el contenedor.** `.mp4` (o `.mkv`) propio, para que lo abra
      cualquiera.

## 15. La BRECHA REAL del C propio: vkQuake 0.50 compilado (25-09)

El propietario: *"tomar lo que mi C tiene y el C de tercero, para madurar"*.
`toolchain/lang/c/BRECHA.md` mide 32 de 32 **sondas**, programas chicos
escritos aqui. Esto es el otro lado: el C de un juego de verdad, fichero a
fichero, con `c -c` y apuntando el PRIMER error de cada uno. Medido hoy:

```text
   capa                                         ficheros que para (de 82)
   1  cabeceras del sistema (sys/types.h...)    78   -> vacias de relleno
   2  sizeof en la medida de un array           70   el COMPILE_TIME_ASSERT de
      (typedef int x[(sizeof(char)==1)*2-1])         q_stdinc.h: TODO el juego
   3  puntero a funcion que devuelve float      70   el propio compilador lo
                                                     dice: leeria rax, no xmm0
   -  #if con macros de SDL, size_t             12   API, no lenguaje
```

Lo que se aprende: **dos rasgos del lenguaje paran el 85 % de un juego entero**,
y ninguna sonda los pedia. Una sonda mide lo que alguien penso; un juego mide lo
que hace falta.

- [x] **C1 -- la brecha real, generada.** `c-gen` pela capa a capa un corpus
      (vkQuake 0.50 primero; Quake, Quake 2 y OpenBW despues) y escribe la
      tabla en `BRECHA.md`, al lado de las sondas. **Como se sabe:** la tabla
      sale sola y sube cuando se arregla una capa.
      *Hecho 25-09, y mas grande:* `toolchain/tools/espejo` (ESPEJO). Ademas
      de pelar un corpus (`espejo corpus`, la misma tabla de arriba en 2 s),
      compila y EJECUTA cada programa por los dos lados --GCC y Clang en el
      anfitrion, BMO en el emulador-- y compara lo impreso; genera programas
      al azar sin comportamiento indefinido, REDUCE un fallo a lo minimo, y
      vigila en tiempo real (`espejo vigilar`). Ver su README.
- [ ] **C3 -- lo que ESPEJO encontro el primer dia.** Cuatro fallos que
      COMPILAN y dan otra cosa, que ninguna sonda veia: las etiquetas
      apiladas de un `switch` (`case 0: case 1: case 2:` manda el 1 y el 2 al
      `default`), el codigo antes del primer `case`, el campo de bits que no
      corta (`f.a = 9` en 3 bits) y el `static` local que no recuerda. Y en
      C++, el literal `ull` por encima del maximo con signo. **Como se sabe:**
      `espejo casos` los da IGUALES (`casos/c/04`, `14`, `18`, `19`).
- [ ] **C2 -- las dos primeras capas del lenguaje:** `sizeof` en una expresion
      constante, y el puntero a funcion que devuelve `float`/`double` (el
      valor en `xmm0`). **Como se sabe:** la capa 2 y la 3 bajan de 70 a 0.

## 16. El RHI de BMO-X y PROTON-X: traducir UNA cosa, medida (25-09)

El propietario: *"la capa RHI traduce las ordenes del juego a la API de la
consola, entonces BMO-X puede tener su RHI ... podria inspirarme en Proton
... ese esta lleno de POSIX, y metio TODO por si acaso ... PROTON-X, propio,
PARA concentrar una cosa"*. Tiene sentido, con dos piezas que van en
direcciones CONTRARIAS y una regla que las ata.

### El RHI: hacia DENTRO (los juegos que se compilan aqui)

Un RHI no es una API publica: es la interfaz INTERNA que un motor usa para no
saber que GPU hay debajo. BMO-X ya tiene la forma, y es de hoy: el rasgo
`Motor` de `Ultra_kernel_x86-64/kernel/src/ring0/dev/pase_gpu.rs` -- el pase
no nombra a NVIDIA, cada tarjeta trae su motor. El RHI es lo mismo para
DIBUJAR:

```text
   el juego (vkQuake, DOOM, tu cubo)
        |  las 67 funciones de vkQuake 0.50 (seccion 12) = la primera lista
        v
   RHI de BMO-X (VERRANO: buferes, imagenes, tuberia, ordenes, vallas)
        |
        +-- backend 3060   AMPERE_B + SASS (lo que ya dibuja `gpu raster`)
        +-- backend CPU    el mismo resultado por software: el JUEZ
        +-- backend otra   la tarjeta alternativa, sin tocar lo de arriba
```

El backend CPU no es el plan B: es el juez de siempre (cada trabajo de la
3060 ya se compara con la CPU). Con un RHI, TODO lo que se dibuje tiene juez.

### PROTON-X: hacia FUERA (un `.exe` de Windows, traducido)

Proton es Wine (la API de Windows ENTERA, ~25 anios) + DXVK/vkd3d-proton
(Direct3D -> Vulkan) + un runtime, todo sobre POSIX y un Vulkan conforme. Es
enorme porque sirve a CUALQUIER programa de Windows: por si acaso.
PROTON-X sirve a UNO, y lo que implementa lo dice `rayosx`, no la intuicion:

```text
   Proton                          PROTON-X
   toda la API, por si acaso       las funciones que el .exe IMPORTA (rayosx)
   sobre POSIX (Wine se escribio   sobre INVOKE, en Ring 3, como libreria
     para Unix)                      (la regla de ENTRAR_EN_SU_ECOSISTEMA:
                                     el kernel no se entera de Windows)
   D3D12 -> Vulkan conforme        D3D12 -> el RHI de la casa
   una funcion que falta: stub     una funcion que falta: NO arranca, y dice
     silencioso, a veces anda        CUAL (el mismo juez que el sombreador)
```

**Lo que NO se ahorra, dicho antes:** lo dificil de Windows no es POSIX, es
su SEMANTICA -- el cargador de PE y sus DLL, hilos y TLS, excepciones (SEH),
COM, el registro, ficheros mapeados. Y lo dificil de D3D12 son sus
sombreadores: DXIL, que habria que llevar a SPIR-V o a SASS. Por eso el
primer cliente de PROTON-X NO es Cyberpunk (663 funciones de 36 bibliotecas,
seccion 9, mas DLL cerradas de terceros): es **tu cubo**.

### La escalera de PROTON-X, del cubo al jefe

```text
   1  cubo.exe tuyo (DX12)      lo escribiste: sabes que llama. rayosx lo
                                cuenta (d3d12, dxgi, user32, kernel32)
   2  cubo.exe en BMO-X         cargador de PE en Ring 3 + esas funciones
                                + D3D12 -> RHI, con DXIL de UN sombreador
   3  un juego D3D9/11 chico    la lista crece medida, no a ciegas
   4  ...                       cada juego suma SU lista; la tabla la escribe rayosx
   F  Cyberpunk                 el jefe final, por la misma escalera
```

**Lo que ya hay (25-09, en el repo EPICX-FRAMEWORK-DirectX12 del
propietario, carpeta `BMOX/`, sin confirmar):** otra IA escribio un cubo por
cada Direct3D -- 9, 10, 11 y 12 -- en Rust (`windows` 0.58), con profundidad,
luz y caras ocultas, medidos en la 3060 (~8700 / ~9200 / ~8900 / ~6000 fps; el
12 pierde con UN cubo, como se espera). Encontro ademas que el antiguo
`cube_dx12.rs` dibujaba por CPU (`softbuffer`): el `BMOX-12` es el primer D3D12
de verdad. [!] El nombre `BMOX` es un envoltorio de la API de Microsoft, no
BMO-X: lo que CRUZA a este repo es lo NEUTRO (la malla, las matrices, el orden
de un fotograma, el diccionario y una imagen de referencia), nunca el codigo
que llama a Windows.

- [ ] **X1 -- el cubo, medido.** El cubo DX12 en Windows (simple, sin motor)
      y `rayosx cubo.exe`. **Como se sabe:** su tabla de importaciones
      apuntada aqui, por DLL, con el numero exacto de funciones.
- [ ] **X2 -- el diccionario.** Cada llamada del cubo (device, cola, lista de
      ordenes, heaps, barreras, fence, present) junto a la pieza de BMO-X que
      ya hace lo mismo (GPFIFO, empuje, semaforo, tablas, page flip).
      **Como se sabe:** la tabla completa, sin una fila "no se".
- [ ] **X3 -- el RHI, escrito como rasgo.** La primera lista (las 67 de
      vkQuake 0.50 y las del cubo, juntas) como interfaz, con el backend CPU
      primero. **Como se sabe:** un crate puro con banco que dibuja el cubo
      por el backend CPU y lo compara con una imagen fija.
