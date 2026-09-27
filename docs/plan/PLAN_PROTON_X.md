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
- [x] **P1b -- el `kernel32` de la casa.** HECHO el 27-09:
      `Ultra_userspace/apps/proton-x/src/kernel32.rs`, las tres de
      `hola.exe` (`GetStdHandle`, `WriteFile`, `ExitProcess`) en Rust
      `no_std` como `extern "win64"`, sobre la consola y la salida del
      proceso. `WriteFile` no manda el `\r` de un `\r\n` (la consola de
      BMO-X es de lineas) y lo cuenta como escrito. Una que no esta en la
      tabla no se rellena: el cargador dice cual falta. **Como se sabe:**
      `hola.exe` CORRE en la CPU del anfitrion (`bmo-proton-x`,
      `tests/corre.rs`): partido, colocado en una base que no es la suya,
      resuelto contra tres `extern "win64"` de prueba, el codigo R+X y los
      datos sin X, y saltado a su entrada con la convencion de Windows. Dice
      `hola desde un .exe de Windows\r\n` y `ExitProcess(0)`, estable en
      cinco corridas. Un fallo del cargador ahi no es un "distinto": es un
      fallo de pagina.
- [x] **P1c -- `hola.exe` en el Ryzen.** HECHO EN EL METAL el 27-09. El codigo:
      `Ultra_userspace/apps/proton-x` -> `sys/proton-x.bex` (el build lo
      enlaza y copia `hola.exe` a `apps/`). Lee el `.exe`, lo PARTE
      (`bmo_proton_x::partir`: cabeceras y codigo delante, datos detras) en
      dos bloques que el kernel pone SEGUIDOS -- se comprueba, no se supone
      --, coloca en la base del bloque, resuelve contra P1b, SELLA el codigo
      (`MEM_OP_SELLAR`) y salta. Cuenta con el tope del kernel de cuatro
      bloques vivos: fichero y monton, se suelta el fichero, codigo y datos.
      `hola.exe` no toca `gs:` (sus 19 instrucciones), asi que no espera a
      P1d. `bex-link` lo enlaza y `bmo-bex-gate` lo ADMITE. **Como se
      sabe:** `run sys/proton-x.bex apps/hola.exe` dice en BMO-X la frase
      de `hola.exe` en Windows y `el .exe salio con 0`. Visto en el Ryzen el
      27-09: `apps/hola.exe: 2560 B, PE32+ x86-64; en 0xe0103000 (el
      enlazador queria 0x140000000); 3 funcion(es) de la casa; codigo 8 KiB
      SELLADO, datos 8 KiB sin X; monton 19472 B`, luego `hola desde un .exe
      de Windows` y `el .exe salio con 0`, sin un fallo de Ring 3. Movido de
      base (las relocalizaciones trabajaron) y con el W^X de la casa.
- [x] **P1d -- la decision del GS.** HECHO EN EL METAL el 27-09:
      la salida **a** de la seccion 3, escrita como concesion en
      [`LA_COMPATIBILIDAD.md`](../identidad/LA_COMPATIBILIDAD.md) 4.2.
      El kernel guarda un GS de Ring 3 POR HILO (`Task::gs_usuario`) y el
      relevo lo pone en `KERNEL_GS_BASE` solo si CAMBIA: los hilos que no lo
      piden pagan una comparacion. `TASK_OP_PON_GS` (0x36) lo pone y contesta
      los ciclos del `wrmsr`. PROTON-X arma un TEB y un PEB de Windows x64
      (`bmo_proton_x::teb`: Self, pila, ClientId, PEB, LastError; base de la
      imagen en el PEB) y pone el GS antes de saltar. El `kernel32` de la casa
      suma las cuatro que viven en el TEB (`SetLastError`, `GetLastError`,
      `GetCurrentProcessId`, `GetCurrentThreadId`) y las lee por `gs:`, como
      Windows. De paso el cargador aprendio la regla exacta de las
      relocalizaciones: sin `.reloc` se mueve igual salvo `RELOCS_STRIPPED`.
      El `.exe` del banco es `prueba/teb.exe` (3072 B, reproducible): lee
      `gs:` como el CRT de Microsoft y dice `bien` o `MAL` en seis cosas.
      **Como se sabe:** en el anfitrion `teb.exe` CORRE con su TEB en el GS
      (`arch_prctl`, `tests/corre.rs`): seis `bien` y sale con 0, estable en
      cinco corridas. **Y en el Ryzen el 27-09:** `run sys/proton-x.bex
      apps/teb.exe` dijo los seis `bien` (TEB en 0xe0007000, PEB en
      0xe0009000, imagen movida a 0xe0104000, pid 3 y tid 8 en el ClientId,
      LastError 0x1234 en `gs:[0x68]`) y `el .exe salio con 0`, sin un fallo
      de Ring 3; `hola.exe` sigue igual con el GS puesto.

      **Lo que cuesta, medido:** el `wrmsr` de `KERNEL_GS_BASE` costo **148 y
      185 ciclos** (dos corridas), unos 40-50 ns a 3,7 GHz; pedido otra vez
      con el mismo valor, **0**: no se toca. Eso es lo que paga un relevo
      entre un hilo de PROTON-X y otro con distinto GS, y nada mas; al lado
      de los ~720 de una puerta y los miles de un cambio con `xsave`, es
      ruido. Los que no lo piden pagan una comparacion.
- [ ] **P2 -- la ventana Win32.** El CODIGO esta (27-09). Un `.exe` de
      manual (`prueba/ventana.exe`, 4 KiB, reproducible): RegisterClassExW,
      CreateWindowExW, ShowWindow, UpdateWindow, el bucle de GetMessageW /
      TranslateMessage / DispatchMessageW, y WM_PAINT con StretchDIBits de un
      bufer pintado por la CPU. Pide 16 funciones de tres DLL. Las DLL de la
      casa se mudaron a su crate, `platform/shared/proton-x-casa`
      (`kernel32`, `user32`, `gdi32` como `extern "win64"`), sobre una
      PLATAFORMA que pone quien carga: en el Ryzen, la puerta de BMO-X
      (`apps/proton-x/src/plataforma.rs`: la consola, una SUPERFICIE del
      escritorio con su buzon -- la misma de VERRANO y DOOM); en el banco,
      una pantalla de mentira. Lo que se dice sin punteros -- la cola en el
      orden de Windows (lo llegado, WM_QUIT, WM_PAINT sintetizado), cada
      evento de BMO-X como WM_CHAR, WM_KEYDOWN/UP con su VK o
      WM_LBUTTONDOWN, y la copia de un DIB de 32 bits de arriba abajo o de
      abajo arriba -- es `bmo_proton_x::ventanas`, puro. Lo que no sabe
      todavia (estirar, un trozo del DIB, filtrar GetMessage) contesta el
      fallo de Windows y lo dice por la consola. Lo que no es Windows, dicho:
      el marco lo pinta el escritorio (ancho x alto son el area de cliente) y
      cerrar con su X mata el proceso. **Como se sabe:** en el anfitrion
      `ventana.exe` CORRE con las DLL de la casa de verdad (`tests/corre.rs`
      de la casa): una letra, un clic en (10, 20) y `q`; la ventana se
      ofrece UNA vez, se pinta tres veces (el ultimo WM_PAINT lo cancela
      DestroyWindow, como en Windows), el pixel del clic es blanco, el del
      tinte es 0xFF80804F, y sale con 0x101; ni un aviso, estable en cinco
      corridas. **Primer intento en el Ryzen (27-09): fallo de Ring 3**
      nada mas saltar. El motivo: el kernel arranca Ring 3 con la pila
      alineada a 16 al ENTRAR en `_start`, y Rust da por hecho la de despues
      de un `call` (16 + 8), asi que la app le pasaba al `.exe` la pila
      torcida 8 bytes; el compilador de Microsoft guarda xmm6 con `movaps`
      (xmm6..15 no son volatiles en Windows x64) y eso es un #GP.
      `hola.exe` y `teb.exe` no tienen ningun `movaps` y no se enteraron; el
      banco tampoco, porque su trampolin ya alineaba. Arreglo: `proton-x.bex`
      salta a la entrada con `and rsp, -16` y la sombra, como Windows.
      Falta el metal otra vez: `run sys/proton-x.bex apps/ventana.exe`
      abre su ventana en el escritorio de BMO-X, las letras cambian el
      tinte, el clic deja el cuadrado y `q` la cierra.
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

## 4a. PAGAR UNA VEZ -- lo que PROTON-X ya hace, y lo que falta (27-09)

El propietario, con P1 hecho: *"PROTON-X solo en x86-64 parece que PURGA,
no? una sola vez y ya [...] se puede optimizar al estilo de mi syscall que
solo paga una vez y se purga por completo, y los servicios (Cyberpunk 2077)
arranquen normal?"*. Si, y es la misma regla de la casa (*elegir uno y pagar
una vez*), aplicada a Windows:

```text
   lo que YA se paga una vez, al cargar       lo que pasa despues, en marcha
   ------------------------------------      -------------------------------
   leer y juzgar el PE (rayosx, P0)           las instrucciones del .exe: EN
   colocar y relocalizar (P1a)                EL RYZEN, TAL CUAL. Nadie las
   resolver CADA importacion (P1a)            mira ni las traduce
   sellar el codigo (W^X)                     una llamada a la casa: `call
   el TEB y el PEB (P1d)                      [rip+x]` a una funcion Rust, sin
                                              tabla, sin busqueda, sin puente
```

Por eso "purga": despues de `resolver`, PROTON-X no esta en medio. La IAT
del `.exe` apunta DIRECTAMENTE a las funciones de la casa, y el codigo que
ejecuta es el suyo. Lo unico que se paga en marcha es lo que la casa HACE
(pintar, leer el buzon), no una capa de traduccion.

Y **los servicios de Windows no se arrancan**: no hay `services.exe`, ni
`svchost`, ni el registro entero. Se da SOLO lo que el `.exe` importa (lo
cuenta `rayosx`), y una funcion que no esta no arranca -- nada mas se carga
por si acaso. Eso es lo que Wine no puede hacer (sirve a CUALQUIER programa)
y aqui es la regla.

Lo que FALTA para pagar una vez de verdad en un juego grande, y es donde se
gana:

```text
   los SOMBREADORES   DXIL -> SPIR-V -> SASS se traduce UNA vez y se guarda
                      en un .bsf con su juez (el sobre que ya existe), como
                      la cache de DXVK pero comprobada: la segunda vez que
                      arranca Cyberpunk no traduce ni uno (P3)
   la IMAGEN          colocada, relocalizada y resuelta una vez, se puede
                      guardar sellada: la segunda vez ni se relocaliza (una
                      "imagen precocinada", como el `.bex`)
   las DLL del juego  PhysX, Bink, Oodle son PE x86-64: se cargan igual, y
                      su resolucion entra en la misma cuenta
```

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

- [x] **P-censo -- TU biblioteca, medida.** HECHO el 27-09 (26-09 por la
      noche en la hora del Ryzen): `rayosx` por la carpeta de cada juego del
      escritorio del propietario, y por las DLL de su motor cuando el `.exe`
      es solo un lanzador. La tabla y lo que dice, en 4b.1. **Como se sabe:**
      la tabla de 4b.1, juego a juego, con la fecha.

### 4b.1 El censo (27-09): lo que `rayosx` dijo de cada juego

```text
   juego (tienda)     binario que manda        CPU      tienda        antitrampas        API grafica              funciones
   -----------------  -----------------------  -------  ------------  -----------------  -----------------------  ---------------------------
   Cyberpunk 2077     Cyberpunk2077.exe        DENTRO   REDGalaxy64   ninguno            D3D12 (+DXGI, AGS,       663 de 36 bibliotecas;
     (GOG)                                              (10, SDK de   en la tabla ni     Streamline/DLSS, FSR3,   20 las trae el juego (303)
                                                        GOG: el       en la carpeta      XeSS; D3D11 de reserva)
                                                        juego no lo
                                                        exige)
   Dota 2 (Steam)     dota2.exe = lanzador     DENTRO   steam_api64   VAC: va por el     Vulkan o D3D11:          110 el .exe; el motor:
                      (110); el motor es       (las 4)  (22 + 13)     cliente de Steam,  rendersystemvulkan.dll   engine2 903, client 991,
                      engine2.dll + client.dll                        no sale en tabla   y rendersystemdx11.dll   vulkan 466, dx11 447
   Warframe (Steam)   Warframe.x64.exe         DENTRO   steam_api64   ninguno visible    D3D11 y D3D12            679 de 32 bibliotecas;
                                                        (9) + EOSSDK                     (retrasadas) + AGS +     11 las trae el juego (171)
                                                        (38, cuentas                     Aftermath
                                                        de Epic)
   Zenless Zone Zero  ZenlessZoneZero.exe =    DENTRO   su cuenta     HoYoKProtect.sys   Unity; la tabla solo     9 el .exe; UnityPlayer 561,
     (Steam)          lanzador (9); el juego   (las 4)  (mhypbase,    = CONTROLADOR DE   ve wgl*, y D3D/Vulkan    GameAssembly 332,
                      es UnityPlayer.dll +              HoYo SDK)     KERNEL: FUERA      se abren en marcha       mhypbase 449
                      GameAssembly.dll (IL2CPP:                       en la practica     (vulkan-1.dll viene
                      C# ya compilado a x86-64)                       (seccion 5)        en la carpeta)
   Left 4 Dead 2      left4dead2.exe +         FUERA:   steam_api     VAC, como Dota 2   D3D9 (shaderapidx9) o    70 el .exe; engine 443,
     (Steam)          engine.dll               32 bits  (23)                             Vulkan (shaderapivk),    shaderapivk 147
                                                                                         los dos de 32 bits
```

**Lo que dice, en cuatro lineas:**

```text
   1  POR LA CPU CABEN 4 DE 5. Solo Left 4 Dead 2 se queda fuera, y por la
      regla de la seccion 1 (32 bits = WOW64), no por su API
   2  POR LA TIENDA Y EL ANTITRAMPAS, SOLO CYBERPUNK PASA LIMPIO. Es de GOG,
      y su REDGalaxy64 es el SDK de GOG, que el juego no exige. Dota 2 y
      Warframe hablan con Steam (y Warframe tambien con las cuentas de Epic);
      Zenless trae un controlador de kernel. Los tres ultimos son ademas
      servicios EN LINEA: sin sus servidores no hay juego, pase lo que pase
      con PROTON-X
   3  CYBERPUNK ES D3D12, o sea el ultimo tramo de la escalera (P6 con DXR).
      Dota 2 trae un RENDERIZADOR VULKAN propio: el unico de la lista cuya
      grafica no pediria traducir D3D -- pero es de Steam
   4  EL PRIMERO DE LA COLA SIGUE SIENDO CYBERPUNK, como decia la seccion 4b;
      el censo lo confirma con numeros en vez de suponerlo
```

**Y lo que el censo mostro de `rayosx`**, que es trabajo para la herramienta
y no para los juegos:

```text
   a  "EL .exe MAS GRANDE" NO ES SIEMPRE EL JUEGO. En Zenless el mas grande
      era el DESINSTALADOR, y en Dota 2, Zenless y Left 4 Dead 2 el .exe del
      juego es un lanzador de 9-110 funciones: el motor vive en DLL
      (engine2/client, UnityPlayer/GameAssembly, engine). Las DLL se midieron
      a mano, una por una. Arreglo: saltar unins*/crash*/launcher y medir
      tambien las DLL grandes del juego
   b  LA TABLA NO VE LO QUE SE ABRE EN MARCHA. Los cinco usan LoadLibrary:
      el renderizador Vulkan de Dota 2 no importa vulkan-1.dll, y Unity no
      importa D3D. La columna "API grafica" sale de los NOMBRES de las DLL de
      la carpeta, no de las importaciones
   c  UN ANTITRAMPAS DE KERNEL NO ES UNA IMPORTACION. HoYoKProtect.sys no sale
      en ninguna tabla: lo carga el lanzador como controlador. Arreglo: que
      rayosx mire tambien los .sys de la carpeta y los nombres conocidos
```

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
