# PLAN PROTON-X -- un `.exe` de Windows en BMO-X, SOLO x86-64, y medido

> Pedido por el propietario (2026-09-27), tras investigar como se portea
> Cyberpunk 2077: *"si, aplicar, y fijate eso en PROTON-X. Es exclusivo en
> x86-64, por motivos PARA BMO"*.
>
> Nace de la seccion 16 de [`PLAN_LA_LUDOTECA.md`](PLAN_LA_LUDOTECA.md) (el
> RHI y PROTON-X, 25-09), donde ya estan HECHOS X1 a X5: el cubo de D3D12
> medido en Windows y dibujado IGUAL, bit a bit, por la 3060 sin Windows.
> Aqui vive desde hoy lo que sigue.

---

## 0. La respuesta corta

```text
   Proton     Wine (la API de Windows entera, ~30 anios) + DXVK + vkd3d-proton,
              sobre un kernel Linux y un Vulkan conforme. Sirve a CUALQUIER
              programa: por si acaso
   PROTON-X   una LIBRERIA de Ring 3 que carga UN .exe x86-64 que compraste y
              le da SOLO lo que ese .exe importa (lo mide `rayosx`). Encima de
              INVOKE, con D3D12 hacia VERRANO y los sombreadores hacia el SASS
              de la 3060 con juez. Una funcion que falta: NO arranca, y dice
              CUAL -- nunca un stub callado
```

Lo que PROTON-X NO cambia de la casa: el kernel no se entera de Windows
(`docs/identidad/ENTRAR_EN_SU_ECOSISTEMA.md`, "Y NO, con Windows no es lo
mismo"), INVOKE y WAIT siguen congelados
([`LA_COMPATIBILIDAD.md`](../identidad/LA_COMPATIBILIDAD.md), tabla 1), y
nada de Wine ni de Proton entra por la puerta de atras. Lo que SI cambia,
dicho a la vista: la seccion 4 de la Ludoteca decia "ningun `.exe` en
BMO-X"; desde el 27-09 dice "ningun `.exe` salvo por PROTON-X".

## 1. POR QUE SOLO x86-64 -- la regla, y sus motivos para BMO-X

**La regla:** PROTON-X carga un PE32+ con maquina AMD64 (0x8664) y codigo
MAQUINA. Nada mas. Lo dice `rayosx` para cada `.exe` desde hoy:

```text
   PROTON-X: DENTRO -- PE32+ x86-64 con codigo maquina: sus instrucciones
                       corren tal cual en el Ryzen
   PROTON-X: FUERA  -- x86 de 32 bits (pediria WOW64: un segundo mundo entero)
   PROTON-X: FUERA  -- ARM64 (otras instrucciones: pediria traducirlas, como
                       FEX o Rosetta)
   PROTON-X: FUERA  -- .NET: lleva IL, no instrucciones x86-64 (pediria una CLR)
```

**Los motivos, y son de BMO-X, no de gusto:**

```text
   1  LA CPU NO SE TRADUCE. Las instrucciones de un juego de Windows ya son
      x86-64 y el Ryzen las ejecuta TAL CUAL: "Wine Is Not an Emulator". Lo
      que hay que dar son sus IMPORTACIONES, y eso se cuenta (rayosx). Quien
      NO es x86-64 paga una capa entera: Apple con Rosetta, el Steam Frame
      (ARM) con FEX, que traduce x86 -> IR -> ARM64 con su cache de codigo;
      y el port de Cyberpunk a Switch 2 (ARM) pidio "muchos cambios a nivel
      de motor, y todos los sombreadores en otro lenguaje" (CD Projekt Red,
      IGN). Un traductor de CPU es un JIT: codigo que se escribe en marcha,
      lo contrario de lo que el juez puede verificar
   2  EL REPO ES DE UNA ARQUITECTURA. El guardian `toolchain/tools/isa` lo
      dice desde antes: todo lo que se emite aqui es x86-64. PROTON-X es la
      misma regla mirada desde fuera: todo lo que ENTRA tambien
   3  EL JUEZ COMPARA BIT A BIT EN UNA CPU. El cubo salio IGUAL a D3D12 en la
      3060 (X4, X5) porque la aritmetica es la misma. Con otra CPU debajo, una
      diferencia podria ser del traductor y no del juego: el juez ya no
      sabria de quien es el fallo
   4  NI 32 BITS NI .NET. Un .exe de 32 bits pide WOW64 (el modo compatible,
      otra ABI, otro cargador, otras DLL): un segundo Windows. Un .exe de .NET
      (XNA, muchos indies) lleva IL, un bytecode: pide una CLR entera, que es
      un JIT otra vez. Los dos se dicen FUERA, con su motivo, y no se
      intentan
```

Lo que la regla NO promete: que todo x86-64 entre. Un juego con
antitrampas de kernel o con DRM que se reescribe (Denuvo) es codigo que se
disfraza en marcha, y queda fuera por la seccion 5, no por la CPU.

## 2. Como portea la industria -- investigado el 27-09, con fuentes

Hay TRES caminos, y solo uno esta abierto con un motor cerrado:

```text
   1  PORT NATIVO           con el CODIGO FUENTE      CD Projekt Red (PS5, Xbox,
                                                      Switch 2, Mac), Saber
                                                      (The Witcher 3 en Switch)
   2  CAPA DE COMPATIBILIDAD  solo el .exe            Valve (Proton), CodeWeavers
                                                      (CrossOver), Apple (Game
                                                      Porting Toolkit)
   3  RECOMPILACION ESTATICA  el binario, a C         N64Recomp, XenonRecomp:
                                                      consolas viejas
```

REDengine 4 es cerrado: para Cyberpunk en BMO-X solo queda el 2. Eso es
PROTON-X.

**Cyberpunk 2077 en Mac (WWDC26, "Bringing Cyberpunk 2077 to Mac")** -- la
fuente mas detallada, y el metodo se parece al de la casa:

```text
   antes     el .exe de Windows TRADUCIDO (Game Porting Toolkit) para medir
             donde se iba el fotograma, antes de escribir una linea
   fase 1    el Mac como plataforma de verdad: builds nativos, su cadena de
             datos (archivos, cache de sombreadores) y "anios de supuestos de
             otras CPU", validados con pruebas unitarias
   fase 2    jugable: el backend Metal pieza a pieza -- PRUEBAS UNITARIAS,
             luego ESCENAS QUIETAS, luego ESCENAS EN MOVIMIENTO --; el HLSL a
             Metal con Metal Shader Converter DENTRO del build; y un bucle:
             escenas repetibles, buscar diferencias de luz y materiales,
             arreglar a mano los pocos sombreadores que no salen iguales. Del
             path tracing: "validando que la imagen siguiera siendo la misma
             que en las otras plataformas"
   fase 3    vendible: Metal HUD y su perfilador por hilos (en la ciudad
             densa el limite era la CPU, no la GPU), MetalFX con resolucion
             dinamica (50-80 % de la salida para 60 fps), y lo de la
             plataforma: HDR solo, audio espacial, mandos, no pintar lo que
             no se ve
```

**Y lo demas que dice la investigacion:**

- **Stadia (2020):** Cyberpunk salio con un backend VULKAN ademas de DX12,
  solo para Stadia. El motor ya sabe hablar Vulkan; ese backend nunca se
  publico para PC.
- **Consolas (2020-22):** CD Projekt Red admite que se centro demasiado en el
  PC y que eso hizo mucho mas dificil la consola; Sony lo retiro de su tienda
  de diciembre de 2020 a junio de 2021.
- **Switch 2 (2025):** ARM; DLSS en los Tensor cores y la microSD Express lo
  hicieron posible.
- **The Witcher 3 en Switch (Saber, 12 meses):** empezo a 10 fps, con un 50 %
  mas de memoria de la que habia y 20 GB mas que el cartucho; se recorto
  (sin luces dinamicas con sombra, sin SSAO, un 30 % menos de gente).
- **vkd3d-proton:** D3D12 -> Vulkan, con DXR, y es por donde corre Cyberpunk
  en el Steam Deck. Sus sombreadores DXIL pasan a SPIR-V con **dxil-spirv**
  (licencia MIT), que traduce los montones de descriptores a un arreglo
  "bindless" y guarda lo traducido en cache.
- **DXVK:** D3D9/10/11 -> Vulkan; compila las tuberias AL CARGAR el
  sombreador (`VK_EXT_graphics_pipeline_library`) y no al dibujar: asi se
  quitan los tirones.
- **Wine:** las DLL del lado PE (`kernel32`, `user32`...) llaman a `ntdll` y
  `win32u`, que son las unicas que cruzan al sistema de debajo. Es la
  forma que PROTON-X copia, NO su codigo: en PROTON-X esa frontera es INVOKE.

**Lo que PROTON-X toma de cada uno:**

```text
   de CD Projekt Red   medir primero; unitarias -> escena quieta -> escena en
                       movimiento; la imagen IGUAL como criterio. Es el metodo
                       del juez (X4, X5), dicho por quien porteo el jefe final
   de Wine             UNA frontera: las DLL de la casa hablan con UNA capa
                       (nuestra `ntdll`), y solo esa llama a INVOKE
   de vkd3d-proton     DXIL -> SPIR-V, y el monton de descriptores como un
                       arreglo grande. Aqui el SPIR-V sigue: bmo-spirv-front
                       -> SASS de SM86, con su juez
   de DXVK             compilar al CARGAR, no al dibujar
   de Saber            cuando no cabe, se recorta, y se dice que se recorto
```

## 3. Como es PROTON-X por dentro

```text
   juego.exe (PE32+, AMD64)                  el que compraste, sin tocar
      |
      v
   proton-x.bex (Ring 3)                     el CARGADOR y las DLL de la casa
      1  lee el .exe como fichero; rayosx dice DENTRO o no arranca
      2  cada seccion en su bloque; las de CODIGO se SELLAN (MEM_OP_SELLAR:
         R+X, sin W, irreversible -- el W^X de la casa, 23-09)
      3  las relocalizaciones (.reloc) a la base elegida
      4  las IMPORTACIONES contra la TABLA DE LA CASA: una que no esta ->
         "no arranca: falta kernel32!CreateFileMappingW", y ninguna mas
      5  un TEB y un PEB, y salto a su punto de entrada
      |
      +-- kernel32/ntdll de la casa   hilos, memoria, ficheros, tiempo:
      |                               la UNICA capa que llama a INVOKE
      +-- user32 de la casa           la ventana = una SUPERFICIE del
      |                               director; las teclas, por su buzon
      +-- dxgi + d3d12 de la casa     hacia VERRANO (el RHI); DXIL -> SPIR-V
      |                               -> SASS, cada programa por el juez
      +-- las DLL que TRAE el juego   (PhysX, Bink, Oodle...) tambien son PE
                                      x86-64: se cargan igual, y lo que
                                      ellas importan entra en la misma cuenta
```

**La convencion de llamada.** Windows x64 pasa los argumentos en
rcx/rdx/r8/r9 con 32 bytes de sombra; BMO-X, en Rust, llama como System V.
Las DLL de la casa se escriben `extern "win64"` en Rust `no_std`: el
compilador hace el puente, sin ensamblador a mano.

**LA UNICA COSA QUE PROTON-X LE PEDIRIA AL KERNEL (medido el 27-09).** El
codigo de Windows encuentra su TEB por `gs:[0x30]` y su PEB por `gs:[0x60]`;
lo hace el propio compilador de Microsoft, en cualquier funcion. Y hoy el
kernel da por hecho que el GS de Ring 3 vale SIEMPRE 0
(`Ultra_kernel_x86-64/kernel/src/ring0/task/percpu.rs`: "the user GS is
always 0"). Hay dos salidas, y se decide en P1 con la medida delante, no
antes:

```text
   a  el kernel guarda y restaura un GS de usuario por hilo (y habilita
      FSGSBASE o una operacion para ponerlo). Es tocar el cambio de
      contexto: se escribe en LA_COMPATIBILIDAD como concesion, con su
      compensacion
   b  sin tocar el kernel: no hay. Reescribir los `gs:` del .exe es tocar
      el binario, y el binario no se toca
```

## 4. La escalera -- del hola al jefe final

X1 a X5 (el cubo medido, el diccionario, el juez neutro, el RHI y el cubo
por la 3060) estan HECHOS en la seccion 16 de la Ludoteca. Lo que sigue:

- [x] **P0 -- la regla x86-64, en codigo.** HECHO el 27-09: `rayosx` da el
      veredicto de PROTON-X de cada PE (DENTRO solo PE32+, AMD64 y codigo
      maquina; FUERA con su motivo si es de 32 bits, ARM64, ARM64EC, ARM64X,
      ARM de 32 bits, PE32 o .NET). **Como se sabe:** `python
      toolchain/tools/rayosx/rayosx.py --prueba` en verde, con el PE de
      prueba DENTRO y sus seis mutaciones FUERA, cada una con su motivo.
- [x] **P1a -- el cargador, puro y con un `.exe` de VERDAD.** HECHO el
      27-09: `platform/shared/proton-x` (`bmo-proton-x`, `no_std`): lee el
      PE con el veredicto de `rayosx`, coloca cabeceras y secciones, aplica
      las relocalizaciones DIR64 si lo mueve de base, lista lo que importa
      ranura a ranura y lo resuelve contra una tabla -- o devuelve TODAS las
      que faltan y no escribe nada. Rechaza una seccion que escribe Y
      ejecuta (W^X) y el TLS (hasta P4). El `.exe` del banco es REAL:
      `prueba/hola.exe`, 2560 bytes, fabricado en la nube con clang y
      lld-link (el enlazador de LLVM compatible con el de Microsoft), sin
      CRT, REPRODUCIBLE byte a byte (`prueba/HACER.txt`, con su sha256).
      **Como se sabe:** `cargo test -p bmo-proton-x`: 11 pruebas en verde --
      cada `call [rip+x]` del codigo cae en una ranura de la IAT, movido a
      otra base el puntero absoluto lo sigue y NADA mas cambia, sin
      `WriteFile` en la tabla dice `kernel32.dll!WriteFile` y no arranca,
      y las mutaciones (32 bits, ARM, PE32, .NET, W+X, otra relocalizacion,
      fichero cortado) salen cada una con su motivo. Compila tambien para
      `x86_64-unknown-none`, el blanco de BMO-X.
- [ ] **P1b -- el `kernel32` de la casa.** Las tres de `hola.exe`
      (`GetStdHandle`, `WriteFile`, `ExitProcess`) en Rust `no_std`, como
      `extern "win64"`, sobre INVOKE: la consola y la salida del proceso. Es
      la primera fila de LA TABLA DE LA CASA. **Como se sabe:** su banco las
      llama con la convencion de Windows (rcx, rdx, r8, r9 y 32 bytes de
      sombra) y la consola dice la frase.
- [ ] **P1c -- `hola.exe` en el Ryzen.** `proton-x.bex` en Ring 3: lee el
      `.exe` del volumen, pide los bloques, coloca con `bmo-proton-x`, SELLA
      `.text` (`MEM_OP_SELLAR`), resuelve contra la tabla de P1b y salta a
      la entrada. `hola.exe` no toca `gs:` (su codigo son 19 instrucciones,
      desensambladas el 27-09), asi que P1c NO espera a la decision del GS.
      **Como se sabe:** `run apps/proton-x.bex hola.exe` dice en BMO-X lo
      mismo que `hola.exe` en Windows, y sale con 0.
- [ ] **P1d -- la decision del GS (seccion 3).** Con el primer `.exe` que SI
      lea `gs:[0x30]` (el CRT de Microsoft lo hace), no antes. **Como se
      sabe:** la decision escrita aqui, con lo que cuesta en el cambio de
      contexto medido.
- [ ] **P2 -- la ventana Win32.** Un `.exe` tuyo con `CreateWindowExW`, su
      bucle de mensajes y pixeles pintados por la CPU. La ventana es una
      superficie del director; las teclas llegan por su buzon. **Como se
      sabe:** la ventana sale en BMO-X, y sus pixeles son los de Windows.
- [ ] **P3 -- el cubo D3D12, EL MISMO `.exe`.** El BMOX-12 de
      EPICX-FRAMEWORK (el de X1) cargado sin tocar: `d3d12`/`dxgi` de la casa
      hacia VERRANO, y sus dos sombreadores DXIL por dxil-spirv (MIT, se
      puede traer con su licencia) hasta SASS. El juez ya existe: las huellas
      de D3D12 en la 3060 de X4. **Como se sabe:** `proton-x cubo.exe` dibuja
      IGUAL, bit a bit, que D3D12 en Windows -- el mismo criterio que X5.
- [ ] **P4 -- la semantica dificil, con banco.** Hilos y TLS, excepciones
      (SEH y el desenrollado de x64), COM (las vtables: X1 midio que casi
      todo D3D12 va por ahi y ninguna tabla de importaciones lo ve), ficheros
      mapeados. Un `.exe` de pruebas tuyo por cada una. **Como se sabe:** el
      mismo `.exe` da la misma salida en Windows y en BMO-X.
- [ ] **P5 -- un juego chico de verdad.** Uno de TU biblioteca de GOG, 64
      bits, D3D11 o D3D12, sin antitrampas, elegido por `rayosx` (el de
      MENOS importaciones que diga DENTRO). **Como se sabe:** su primer nivel
      se juega en el Ryzen.
- [ ] **P6 -- los rayos (DXR).** Lo que Cyberpunk pide y ningun escalon de
      abajo: estructuras de aceleracion y los sombreadores de rayos, hacia
      las unidades RT de la 3060. Es el escalon mas lejano de la escalera de
      VERRANO (Quake II RTX, `PLAN_VERRANO.md` 2d). **Como se sabe:** un
      `.exe` DXR tuyo dibuja igual que en Windows.
- [ ] **F -- Cyberpunk 2077 (GOG, sin DRM).** Lo que pide, medido el 25-09
      (Ludoteca, seccion 9): 663 funciones de 36 bibliotecas en la primera
      capa, D3D12 cargado en marcha, y detras PhysX, Bink, Oodle, ICU,
      libcurl, REDGalaxy y los tres reescaladores. Los reescaladores se
      APAGAN (DLSS habla con el driver de NVIDIA: no hay). 12 GB de RAM y 70
      GB de datos, copiados desde Windows (el NTFS no hace falta, Ludoteca
      seccion 8). **Como se sabe:** Night City en el Ryzen, y la imagen
      comparada con la de Windows como hizo CD Projekt Red con el Mac.

## 4b. "La MAYORIA de los juegos?" -- lo que decide, medido y no a ojo

El propietario (27-09), mirando su Steam, su GOG y su Epic: GTA V, Dying
Light 2, Alien: Isolation, Control, Cyberpunk... *"se puede aplicar a la
MAYORIA de juegos, no?"*.

**Por la CPU, casi todos si.** Los juegos de PC de los ultimos quince anios
son PE32+ x86-64: `rayosx` dira DENTRO de casi todos. **Pero la CPU nunca fue
lo que decide.** Deciden cuatro cosas, y `rayosx` ya separa las dos primeras
(familias `tienda` y `antitrampas`, 27-09):

```text
   1  LA TIENDA      un juego que le pregunta al cliente de su tienda si lo
                     compraste (steam_api, EOSSDK de Epic, el lanzador de
                     Rockstar) no arranca sin ese cliente, y PROTON-X no finge
                     ser el cliente. GOG sin DRM, si: por eso va primero
   2  ANTITRAMPAS    BattlEye, EasyAntiCheat...: vigilan el sistema y se
                     disfrazan en marcha. FUERA en la practica, y el modo en
                     linea de un juego asi, siempre
   3  LA API GRAFICA D3D11 (DXVK) es mas cercana que D3D12 (vkd3d); D3D12 con
                     rayos (DXR) es lo ultimo de la escalera (P6)
   4  LO QUE PIDE    cientos de funciones (Cyberpunk: 663 en la primera capa)
                     y las DLL que trae. Cada juego suma SU lista
```

Asi que la respuesta honesta: **la mayoria cabe por la CPU, y cada juego se
decide por su tienda**. El orden sale solo: primero lo de GOG sin DRM (tus
17: Cyberpunk entre ellos), despues lo que `rayosx` diga que no habla con su
tienda. Lo de Steam, Epic y Rockstar que exija su cliente se queda en
Windows (camino B de la Ludoteca, por streaming).

- [ ] **P-censo -- TU biblioteca, medida.** `rayosx` por la carpeta de cada
      juego instalado, en tu Windows, y la tabla aqui: DENTRO/FUERA, tienda,
      antitrampas, API grafica y cuantas funciones. **Como se sabe:** la
      tabla, juego a juego, con la fecha.

## 4c. El banco contra Windows: fps Y vatios

El propietario: *"si BMO-X consume pocos vatios y sin el parasito de
Windows, podria comparar esos juegos contra Windows en benchmarks"*. Es la
pregunta correcta, y la casa ya mide las dos mitades: FRAPS-X los fps y el
HUD de la GPU los vatios de la 3060 (NVML en Windows, el GSP en BMO-X). La
regla, para que la comparacion valga:

```text
   el MISMO .exe, los MISMOS ajustes, la MISMA escena repetible (la demo o
   el banco del propio juego), en la MISMA maquina; y de cada lado: fps
   medios, el 1 % mas lento, y los vatios de la 3060 durante la escena
```

Y dicho antes de medir: una capa de traduccion CUESTA (DXVK y vkd3d pagan
la traduccion de sombreadores y de estados), y BMO-X no tiene hoy un driver
de la madurez del de NVIDIA. Ganar a Windows no es lo esperable al
principio; lo que se mide es CUANTO cuesta, y de donde sale cada vatio.

- [ ] **P-banco -- la primera comparacion.** Con el primer juego que corra
      (P5), la tabla de arriba de los dos lados. **Como se sabe:** fps y
      vatios de Windows y de BMO-X, en la misma escena, con la fecha.

## 5. Lo que NO se hace

- **Nada de 32 bits, ARM ni .NET** (seccion 1): se dicen FUERA y no se
  intentan.
- **Nada de DRM ni antitrampas.** Solo juegos que compraste y que se venden
  SIN DRM (GOG primero, por eso). Nada que se salte una proteccion o las
  condiciones de una tienda. Un juego con antitrampas de kernel se queda en
  Windows (camino B de la Ludoteca, por streaming).
- **Ni un stub callado.** Una funcion que no esta, o que esta a medias, NO
  arranca y lo dice con su nombre. Es la regla del compilador (ver
  `registrar_global`, codegen/bex.rs, 27-09) llevada a Windows.
- **Ni una operacion de Windows en el kernel.** PROTON-X es Ring 3; si la
  decision del GS (P1) pide tocar el cambio de contexto, se escribe como
  concesion con su compensacion, como manda `LA_COMPATIBILIDAD.md`.
- **Ni codigo de Wine, DXVK o vkd3d-proton COPIADO al arbol.** Son LGPL y
  zlib; se ESTUDIAN y se citan. dxil-spirv es MIT: ese si puede entrar, con
  su licencia y su procedencia al lado, como `bmo-cubo`.
- **Ningun fichero de un juego en el repositorio**, como con DOOM.

---

## Fuentes (27-09)

- Apple, WWDC26 -- "Bringing Cyberpunk 2077 to Mac":
  https://developer.apple.com/videos/play/wwdc2026/356/
- HotHardware -- CD Projekt Red sobre portar de x86 a Switch 2:
  https://hothardware.com/news/challenge-of-porting-cyberpunk-2077-to-switch-2
- Nintendo Life -- Cyberpunk y DLSS en Switch 2:
  https://www.nintendolife.com/news/2025/04/cyberpunk-2077-confirmed-as-the-first-switch-2-title-to-use-nvidias-dlss-upscaling
- Wikipedia -- Cyberpunk 2077 (fechas, consolas, lo de Sony):
  https://en.wikipedia.org/wiki/Cyberpunk_2077
- VideoCardz -- Vulkan solo en Stadia:
  https://videocardz.com/newz/cyberpunk-2077-is-playable-on-linux-but-still-needs-work
- EGM -- The Witcher 3 en Switch:
  https://egmnow.com/the-switcher-how-saber-interactive-took-one-of-the-biggest-games-ever-portable/
- vkd3d-proton: https://github.com/HansKristian-Work/vkd3d-proton
- dxil-spirv: https://github.com/HansKristian-Work/dxil-spirv
- Maister, "My personal hell of translating DXIL to SPIR-V":
  https://themaister.net/blog/2021/09/05/my-personal-hell-of-translating-dxil-to-spir-v-part-1/
- DXVK: https://github.com/doitsujin/dxvk
- Wine, la capa de la API: https://deepwiki.com/wine-mirror/wine/1.2-windows-api-implementation-layer
- FEX-Emu: https://github.com/FEX-Emu/FEX
- N64Recomp: https://github.com/N64Recomp/N64Recomp
- GOG, requisitos de Cyberpunk 2077:
  https://support.gog.com/hc/en-us/articles/360015701497-Cyberpunk-2077-System-requirements
