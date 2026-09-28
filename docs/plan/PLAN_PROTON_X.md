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
- [x] **P2 -- la ventana Win32.** HECHO y VISTO obedecer en el metal (27-09). Un `.exe` de
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
      **Y en el Ryzen, con el arreglo (27-09, 06:13):** la ventana SALIO en
      el escritorio de BMO-X -- `[ventana] tid 7 322x229`, `App 1` en la
      barra, el degradado y el marco dorado de `ventana.exe`, pintados por su
      StretchDIBits. **Y obedecio (27-09, 11:23):** las letras cambian el
      tinte, el clic deja el cuadrado y `q` la cierra; salio con 1793 (siete
      letras y un clic) y en otra corrida con 41984 (164 letras) -- lo que
      cuenta `ventana.exe`, igual que en Windows.
- [ ] **P3 -- el cubo D3D12.** Partido en tres el 27-09, porque el `.exe`
      de X1 (`BMOX-12`, Rust con `std` de Windows) trae su runtime entero
      --~110 importaciones, hilos, TLS y excepciones--, que es P4:
- [x] **P3a -- D3D12 y DXGI limpian la ventana.** HECHO y VISTO obedecer en el metal (27-09).
      `prueba/limpia.exe` (4,5 KiB, reproducible): el esqueleto de todo
      programa D3D12 -- D3D12CreateDevice, CreateDXGIFactory2, la cola, la
      cadena de intercambio sobre la ventana de P2, el monton de RTV, el
      asignador, la lista, la valla y su evento -- y cada fotograma barrera,
      ClearRenderTargetView, Close, ExecuteCommandLists, Present, Signal.
      Sin d3d12.h: cada metodo por su HUECO en la vtabla. La casa suma
      `com.rs` (la forma de un objeto COM: vtabla en el orden de las
      cabeceras de Windows, QueryInterface por IID, y un hueco que no esta
      dice "ID3D12Device::CreateRootSignature (hueco 16) no esta en la
      casa" y sale con 0xC0DE0000 | interfaz << 8 | hueco), `d3d12.rs` y
      `dxgi.rs`: la cola es SINCRONA y de la CPU (las ordenes se hacen en
      ExecuteCommandLists, la valla ya esta cumplida al volver) y Present
      copia el back buffer a la superficie de la ventana. **Como se sabe:**
      en el anfitrion `limpia.exe` CORRE con las DLL de la casa: dos Present,
      los 64.000 pixeles del color exacto (0.75, 0.25, 0.0 -> 0xFFBF4000) y
      sale con 2, ni un aviso; y un hueco de P3b llamado de verdad dice su
      nombre y sale con 0xC0DE0010. **Y en el Ryzen (27-09, 06:30):** la
      ventana salio AZUL -- (0.0, 0.25, 0.75), R 0 G 64 B 191, el primer
      color de `limpia.exe`, limpiado por el ClearRenderTargetView de la casa
      y puesto por su Present; el wrmsr del GS, 185 ciclos. **Y obedecio
      (27-09, 11:23):** cada letra cambia el color, `q` la cierra, y sale con
      88: los Present que hizo, como en Windows.
- [ ] **P3b -- el cubo con sus sombreadores.** Root signature, PSO,
      buferes de vertices, DrawInstanced, y lo dificil: los sombreadores.
      Decidido el 27-09: **DXIL, no DXBC**. DXBC (SM5, `fxc`) es mas facil
      de leer, pero es lo viejo; lo que traen Cyberpunk y todo juego D3D12
      de hoy es DXIL (SM6, `dxc`): bitcode de LLVM dentro de un contenedor
      `DXBC`. Un desvio por DXBC seria un lector que el jefe final no usa.
      En cuatro escalones, cada uno con su banco:
      P3b1 [HECHO el 27-09] el contenedor y el bitcode:
           `bmo_proton_x::dxil` (puro, sin `unsafe`) lee el `DXBC` (sus
           partes, las firmas ISG1/OSG1 y la cabecera del programa: etapa y
           modelo), el flujo de bits de LLVM entero (`dxil/bits.rs`: bloques,
           BLOCKINFO, abreviaturas, Array, Char6, Blob) y el modulo: las
           funciones con su nombre de la tabla de simbolos, cuales son
           `dx.op.*`, cuantas instrucciones tiene cada cuerpo, y quien lo
           hizo (el `llvm.ident` de los metadatos: en 3.7 no hay bloque
           IDENTIFICATION). Los sombreadores del banco son los del cubo de
           X1 en HLSL (`prueba/cubo.hlsl`), compilados por `dxc` 1.8 (el
           oficial, version Linux) a `cubo_vs.dxil` y `cubo_ps.dxil`,
           reproducibles, con su testigo `dxc -dumpbin` al lado. **Como se
           sabe:** lo que el lector encuentra solo es lo que dice el testigo
           -- vertice: POSITION/NORMAL/COLOR -> SV_Position/NORMAL/COLOR,
           `vertice` con 80 instrucciones y 5 operaciones de D3D; pixel:
           `pixel` con 31 y 6, SV_Target (valor de sistema 64); productor
           `dxc(private) 1.8.0.4662` --, y un bitcode cortado dice en que
           bit se quedo. Compila para `x86_64-unknown-none`. Y un dato para
           P3b3: dxc funde `mul` + `add` en FMad; la igualdad bit a bit con
           D3D12 en la 3060 tendra que decidir si se funde o no.
      P3b2 [HECHO el 27-09, VISTO en el metal] la tuberia:
           `bmo_proton_x::dxbc` (el contenedor y su HUELLA: el MD5 con el
           final de Microsoft, IGUAL en los tres blobs de dxc) y
           `bmo_proton_x::raiz` (la root signature 1.0: leerla y escribirla,
           byte a byte la de `dxc -T rootsig_1_0`, `prueba/raiz.rts`). En la
           casa, `tuberia.rs`: D3D12SerializeRootSignature, CreateRootSignature,
           CreateGraphicsPipelineState (los dos DXIL leidos, cada uno en SU
           hueco, y el input layout CRUZADO con la firma de entrada del
           vertice: una semantica que falta es E_INVALIDARG y lo dice),
           CreateCommittedResource de BUFERES alineados a 256, Map,
           GetGPUVirtualAddress; y en la lista, IASetPrimitiveTopology,
           IASetVertexBuffers, IASetIndexBuffer, RSSetViewports,
           RSSetScissorRects, OMSetRenderTargets, SetPipelineState,
           SetGraphicsRootSignature, SetGraphicsRootConstantBufferView,
           DrawInstanced y DrawIndexedInstanced. Un Draw se APUNTA con el
           estado de ese momento y, al ejecutarse, lee los buferes como la
           GPU: cada direccion se RESUELVE contra los buferes de la casa (una
           ajena se dice, no se lee), comprueba PSO, root signature igual a la
           del PSO y formato del destino, y CAPTURA lo que el dibujo veria.
           El `.exe` es `prueba/cubo.c`: un programa D3D12 entero y correcto
           (estructuras de d3d12.h con sus desplazamientos comprobados al
           compilar), con los datos y los DXIL de `cubo_datos.h` (FABRICADO
           desde bmo-cubo). **Como se sabe:** `cubo.exe` corre en el
           anfitrion con la casa, y los dibujos de los fotogramas 0, 30 y 60
           ven, BIT A BIT, los vertices, los indices y las constantes de X1
           -- lo que X4 subio a la 3060 --, con viewport 1280x720, descarte
           de detras y la ventana limpia con el fondo de X1. En Windows el
           mismo `.exe` dibuja el cubo: ese es el juez de que el programa es
           D3D12 de verdad (el propietario lo vio dibujar el cubo). En el
           Ryzen, el 27-09 09:48: la ventana, el fondo de X1 y un solo aviso,
           el de P3b3; ni un hueco que falte ni un fallo de Ring 3.
      P3b3 [HECHO el 27-09, VISTO en el metal] los sombreadores
           EN LA CPU. `bmo_proton_x::dxil::programa` lee el CUERPO del DXIL
           (tipos, constantes del modulo y de la funcion, operandos
           relativos, llamadas a `dx.op.*`) y lo compila a un programa de
           registros: LoadInput, StoreOutput, CreateHandle,
           CBufferLoadLegacy, fmul/fadd/fsub/fdiv, extractvalue, FMad,
           Dot2/3/4, Rsqrt, Sqrt, Saturate, FAbs, FMin, FMax; lo demas se
           dice por su nombre al crear el PSO. `bmo_proton_x::trama` es el
           rasterizador: las cuatro reglas del juez (centro del pixel, 1/256
           con empates al par, viewport, top-left), descarte y cara de
           delante, viewport + tijera + destino, y atributos con perspectiva
           (uno igual en los tres vertices es ESE, exacto; si al sombreador
           de pixeles le entra lo mismo que al pixel anterior, su color se
           reusa: una vez por cara en el cubo). Va en `bmo_proton_x` y no en
           VERRANO porque VERRANO V0 toma un color por triangulo, y D3D12
           pide atributos por pixel. Las decisiones de numeros: FMad SIN
           fundir y Dot de izquierda a derecha (asi cuenta el juez), Rsqrt =
           1 / la raiz EXACTA (en enteros: igual con soft-float). **Como se
           sabe:** el sombreador de vertices corrido da BIT A BIT `wvp * pos`
           y la normal del juez; y `cubo.exe` entero, en el anfitrion con la
           casa, presenta en cada fotograma (0, 30, 60) la HUELLA de lo que
           D3D12 dibujo en la 3060 bajo Windows -- sin el juez de por medio:
           los DXIL de dxc, corridos. Lo que falta, contado y dicho: recorte
           (w <= 0 o fuera de 0..w no se pinta), profundidad, mezcla,
           texturas, SV_Position en el de pixeles, saltos en el DXIL.
           En el Ryzen, el 27-09 10:11: `run sys/proton-x.bex apps/cubo.exe`
           dibujo el cubo -- el mismo que el propietario vio en Windows 11 a
           las 10:13, cara verde y tapa magenta en el mismo sitio --, sin un
           aviso y sin un fallo de Ring 3. La primera imagen de un `.exe` de
           D3D12 dibujada por BMO-X, con los sombreadores de dxc corridos.
           Y de paso, el hueco que vio el propietario al cerrar `cubo.exe`
           con ^C (`no devuelto: sigue PRESTADO a otro`, 3.5 MiB): el
           kernel apuntaba el bloque retenido en ningun sitio y, al soltarlo
           el escritorio, esos marcos no volvian nunca. Ahora
           `obj/memory.rs` los apunta (`RETENIDOS`) y los libera cuando se
           suelta el ultimo prestamo que los sujetaba.
      P3b3b [HECHO el 27-09, VISTO en el metal] los sombreadores
           NATIVOS en la CPU, con INTI. El Rust de Ring 3 no puede usar SSE
           (el target es soft-float; encenderlo lo retira rustc, issue
           #162235, y tumba a LLVM con SIGILL: comprobado), y el interprete
           hacia cada `fmul` con una rutina de software. INTI si emite SSE en
           Ring 3, y con la MISMA regla de numeros a la que llego P3b3 por su
           cuenta (`acumula` = dos redondeos, no FMA; el orden del juez).
           `bmo_proton_x::nativo` traduce el `Programa` de cada sombreador a
           x86-64 escalar con SSE (movss/mulss/addss/subss/divss/sqrtss;
           Saturate, FMin y FMax con el NaN de D3D; el MXCSR de quien llama
           guardado y el de D3D puesto), independiente de donde caiga. La casa
           (`nativo.rs`) lo traduce al crear el PSO y lo pone en UN bloque
           SELLADO (`MEM_OP_SELLAR`, que se escribio para esto: "la pieza que
           un JIT necesita"); con cada PSO nuevo, un bloque nuevo con todo y
           el viejo SOLTADO (`MEM_OP_SOLTAR`): vivo, siempre uno. El limite no
           era cuatro sino OCHO bloques vivos (`MAX_PETICIONES`, desde el
           20-09): proton-x gasta monton, codigo, datos, la ventana y este.
           Sin bloque, se interpreta y se dice (dan lo mismo). El ejecutor es
           el de la casa (`nativo::dibujar`, por la costura del lote).
           **Como se sabe:** el JUEZ es el interprete -- los dos sombreadores
           del cubo en los 360 fotogramas, cada operacion con NaN, infinitos,
           -0 y subnormales, y 20.000 valores cualesquiera, BIT A BIT
           (`tests/nativo.rs`; cambiar el orden de FMin da 132 distintos,
           comprobado); `cubo.exe` en el anfitrion sigue dando las huellas de
           la 3060 con el codigo nativo (romper `mulss` las pierde,
           comprobado); y los opcodes son los de las filas SSE de la tabla de
           INTI (`intrinsics.toml`), con FMad como su `acumula` y ni una FMA.
           En el Ryzen, el 27-09 10:58: `cubo.exe` dibujo el mismo cubo con
           los sombreadores nativos en un bloque sellado, sin un aviso.
      P3b4 PAGAR UNA VEZ (4a): DXIL -> SPIR-V -> SASS con su juez, guardado
           en `.bsf` por la huella del sombreador; la segunda vez no se
           traduce nada, y el cubo lo dibuja la 3060 (el criterio de X5).
- [ ] **P3c -- el BMOX-12 de EPICX-FRAMEWORK, sin tocar.** Pide P4 (el
      runtime de Rust para Windows). **Como se sabe:** `proton-x cubo.exe`
      dibuja IGUAL, bit a bit, que D3D12 en Windows -- el criterio de X5.
      **MEDIDO el 27-09, sin Windows.** BMOX-12 es `estudio_d3d12` de la rama
      `estudio-d3d` de EPICX-FRAMEWORK (su `CENSO.md`: 112 funciones de 15
      DLL). Compilado aqui para `x86_64-pc-windows-msvc` con `lld-link`,
      `/entry:main` y `/force:unresolved` (sin el CRT estatico de Microsoft
      no corre, pero su tabla de importaciones es la de verdad: cuadra con el
      censo), y cruzada nombre a nombre con la tabla de la casa:
```text
   importa 78 nombres de 9 DLL (+ los del CRT estatico); a la casa le faltaban
   P3c1  PeekMessageW, AdjustWindowRect, LoadCursorW, SetWindowTextW,
         LoadLibraryExA, GetErrorInfo, SysStringLen, SysFreeString,
         RoOriginateErrorW (el crate `windows` para sus HRESULT), ceil,
         _register_thread_local_exe_atexit_callback, terminate  [HECHO 27-09]
   P3c2  [HECHO en el banco 28-09; falta Windows y el metal]
         D3DCompile: el cubo COMPILA su HLSL en marcha (FXC, Shader Model
         5.0: vs_5_0 / ps_5_0). En BMO-X no hay compilador de HLSL; lo
         honesto es PAGAR UNA VEZ: compilarlo en Windows (d3dcompiler_47,
         el mismo) a un fichero por (fuente, entrada, perfil) y que la casa
         lo devuelva, y si no esta, que lo diga con su huella
   P3c3  el bytecode de SM5 (DXBC con SHEX): la casa ejecuta DXIL (SM6, el
         de dxc); SM5 es OTRO lenguaje de maquina virtual (registros r#, v#,
         o#, cb#[]) y pide su lector y su interprete hacia el mismo lote
   P4c   AddVectoredExceptionHandler, RtlCaptureContext,
         RtlLookupFunctionEntry, RtlVirtualUnwind, y lo de C++ del CRT
```
      Lo que la tabla NO ve son los metodos COM (por vtabla): su
      diccionario (DICCIONARIO de la rama estudio-d3d) los lista, y hay que cruzarlos con los huecos de
      `com.rs` de la casa. P3c1 **Como se sabe:** `prueba/peek.exe` dice
      `bien` doce veces (poner `ceil` a redondear hacia abajo da MAL,
      comprobado). P3c2 **Como se sabe:** `bmo_proton_x::sombras` (la
      huella FNV-1a de 32 bits de fuente, entrada, perfil, banderas y macros,
      y el `.ent`), `compilador.rs` de la casa (D3DCompile sobre
      `window/sombras`, carpeta que crea el build) y el obrero de Windows
      `obrero/sombras.exe`, que compila lo pendiente con el d3dcompiler_47 de
      ese Windows. `prueba/compila.exe` en el anfitrion, dos veces: sin `.cso`
      dice E_FAIL y deja la fuente y el pedido; con ellos, da sus bytes. En
      Windows compila de verdad (tambien `bien`). Falta: correr `sombras.exe`
      en Windows sobre lo que deje el Ryzen. Para repetir la medida:
```text
   cd estudio-d3d
   RUSTFLAGS="-C linker=lld-link -C link-arg=/force:unresolved -C link-arg=/entry:main"
     cargo build --release -p estudio_d3d12 --target x86_64-pc-windows-msvc
   llvm-readobj --coff-imports target/x86_64-pc-windows-msvc/release/estudio_d3d12.exe
```

- [ ] **P4 -- la semantica dificil, con banco.** Hilos y TLS, excepciones
      (SEH y el desenrollado de x64), COM (las vtables: X1 midio que casi
      todo D3D12 va por ahi y ninguna tabla de importaciones lo ve), ficheros
      mapeados. Un `.exe` de pruebas tuyo por cada una. **Como se sabe:** el
      mismo `.exe` da la misma salida en Windows y en BMO-X.
      P4a [HECHO el 27-09, VISTO en el metal] LOS HILOS. BMO-X no
           tiene hilos de Ring 3 (una tarea, un hilo; el FUERO los deja para
           cuando haya SMP), asi que PROTON-X se los da al `.exe` DENTRO de
           su tarea, COOPERATIVOS (el modelo M:1): cada hilo con su pila (del
           monton, con un canario al fondo que se mira en cada relevo), su
           TEB y su GS, su copia del TLS estatico y sus ranuras de TlsAlloc;
           se cede el turno cuando uno ESPERA. `bmo_proton_x::hilos` decide
           (puro, con banco: eventos, semaforos, hilos, secciones criticas
           recursivas, SRW con lectores, condiciones en orden de llegada,
           plazos, y el BLOQUEO MUTUO, que se dice y sale con 0xDEAD10CC en
           vez de colgarse). `bmo_proton_x::tls` lee el directorio 9 del PE;
           el cargador deja el bloque del hilo principal en `TEB+0x58` y
           llama a los callbacks con PROCESS_ATTACH antes de la entrada (y
           con THREAD_ATTACH/DETACH en cada hilo, en ESE hilo). La casa
           (`hilos.rs`) hace el relevo en ensamblador guardando lo que Windows
           x64 da por conservado -- rbx rbp rdi rsi r12..r15, xmm6..xmm15, el
           MXCSR y la x87 -- porque en Ring 3 la casa es soft-float y el
           compilador no sabe que los xmm existen; y 46 nombres de kernel32
           (CreateThread, WaitFor*, Sleep, Tls*, *CriticalSection*, *SRWLock*,
           *ConditionVariable*, semaforos, eventos, GetTickCount*,
           QueryPerformance*). La valla de D3D12 ya enciende eventos que
           llegan DESPUES (un Signal de otro hilo). **Como se sabe:**
           `prueba/hilos.exe` (sin CRT, con su `_tls_used`) dice `bien`
           diecinueve veces en el anfitrion con la casa: hilos y su codigo de
           salida, CREATE_SUSPENDED, TEB y pila propios, TLS dinamico y
           estatico, los callbacks, 4 x 10000 sumas en una seccion critica con
           el turno cedido DENTRO, productor y consumidor con semaforos, una
           cola con SRW y condicion, un plazo, y xmm6..xmm15 y el MXCSR
           intactos aunque otro hilo los ensucie. Lo que no depende del orden
           de los hilos, que es lo que Windows tambien tiene que decir. [!] En
           el anfitrion los xmm los guarda ya el compilador (con SSE): el
           banco los ve solo por el MXCSR (quitar su `ldmxcsr` del relevo da
           MAL, comprobado); el juez de los xmm es `hilos.exe` en el Ryzen.
           En el Ryzen, el 27-09 10:58: `hilos.exe` dijo `bien` diecinueve
           veces y salio con 0 -- incluida la linea de xmm6..xmm15, la que el
           anfitrion no podia juzgar: el relevo de la casa los guarda de
           verdad con la casa en soft-float.
           Lo que falta, dicho: un hilo que da vueltas sin esperar no suelta
           el turno (no hay reloj que se lo quite).
      P4b [HECHO el 27-09] LA COSTURA con quien dibuja: `bmo_proton_x::lote`.
           La casa traduce D3D12 (punteros, descriptores, root signature) a
           un LOTE neutro -- sombreadores cosidos, bytes de vertices, ids,
           topologia, cb, reglas -- y lo dibuja el `Ejecutor` que pone la
           plataforma (`Plataforma::dibujar`). Hoy `lote::en_cpu` (el que dio
           las huellas de la 3060, y el banco lo prueba EN la costura); con
           P3b4, el de VERRANO con la 3060, y `en_cpu` de juez. La casa no
           cambia para eso.
      P4c [BASE el 27-09] LAS EXCEPCIONES (SEH): `bmo_proton_x::desenrollar`
           (.pdata/.xdata, un marco como RtlVirtualUnwind) y `prueba/seh.c`.
           Falta el despachador de la casa (RaiseException,
           __C_specific_handler, RtlUnwindEx): pendiente.
      P4d [HECHO el 27-09, VISTO en el metal] LOS FICHEROS. La
           casa sirve CreateFileW/A, ReadFile, WriteFile, SetFilePointer(Ex),
           GetFileSize(Ex), GetFileType, FlushFileBuffers, GetFileAttributesW
           y CloseHandle PAGANDO UNA VEZ: al abrir, el fichero ENTERO a
           memoria (en BMO-X `Archivo::leer_de` + un bloque + `leer_en`: un
           viaje), cada ReadFile una copia, y al cerrar uno escrito sale
           entero (`Archivo::create` + `write`). `bmo_proton_x::ficheros`
           (puro) pasa la ruta de Windows a la del volumen: la unidad se
           quita, las barras se enderezan, `..` se resuelve y uno que saldria
           del volumen se RECHAZA; una relativa va desde el directorio del
           `.exe` (su directorio actual: `apps`). **Como se sabe:**
           `prueba/ficheros.exe` dice `bien` dieciseis veces en el anfitrion
           (crear, escribir, leer, moverse desde el fin y el principio, los
           errores 2, 80, 131 y 183 de Windows, y CREATE_ALWAYS vaciando), y
           en el volumen queda lo ultimo que escribio. Lo que no hay, dicho:
           dos handles que escriben el mismo fichero (cada uno tiene su
           copia). [Corregido el 27-09, P4f3: el "hasta 4 KiB" no era verdad
           -- el bufer del kernel crece; lo caro era `Archivo::write`, siete
           bytes por llamada. Ahora sale de UNA con `escribir_de`.]
           En el Ryzen, el 27-09 11:23: `ficheros.exe` dijo `bien` dieciseis
           veces y salio con 0.
      P4e [HECHO el 27-09, en el banco; falta el metal] LO QUE PIDE UN CRT.
           `bmo_proton_x::monton` es el monton de Windows del `.exe`, el suyo
           y no el del cargador (que solo avanza): cabeceras DENTRO del
           bloque, lista de libres, y soltar FUSIONA con los dos vecinos --
           un CRT que hace malloc/free un millon de veces no se come nada, y
           el monton no pide memoria para apuntar. Un PROPIETARIO de 16 bits por
           bloque dice de que monton es (el del proceso, uno de HeapCreate,
           VirtualAlloc); HeapDestroy los suelta de una pasada. Las arenas
           las da `Plataforma::memoria` (en BMO-X, un bloque del kernel de 64
           MiB al primer HeapAlloc). `bmo_proton_x::regiones` cuenta las
           paginas de VirtualAlloc (reservar a 64 KiB, hacer a CERO las que
           toca un rango, deshacer, soltar, consultar, proteger) y
           `bmo_proton_x::proceso` da el nombre (`C:\window\x.exe`), la linea
           de ordenes (`run sys/proton-x.bex window/x.exe lo de detras`) y el
           entorno (OS, PATH, TEMP, TMP, NUMBER_OF_PROCESSORS=1...). El PEB
           dice su ProcessHeap (+0x30). La casa suma GetProcessHeap, Heap*
           (Alloc, Free, ReAlloc, Size, Validate, Create, Destroy,
           SetInformation), Virtual* (Alloc, Free, Query, Protect),
           GetSystemInfo, GetModuleFileNameW/A, GetCommandLineW/A y el
           entorno W. **Como se sabe:** el banco puro da seis mil pedidas y
           sueltas al azar sin pisar un byte y la arena entera al final
           (`Monton::comprobar` la recorre de punta a punta), y
           `prueba/crt.exe` dice `bien` treinta y cinco veces en el anfitrion
           (sabotear HEAP_ZERO_MEMORY da MAL, comprobado). **En Windows (el
           propietario, 27-09):** 34 de 35; la MAL era de la PRUEBA -- miraba
           que GetEnvironmentVariableW con bufer corto no lo tocara, y su
           documentacion dice que lo de dentro queda indefinido (Windows lo
           toca). Arreglada: solo se mira lo que devuelve. Lo que no es
           Windows, dicho: reservar ya gasta memoria (el kernel da bloques
           hechos), no se reserva en una direccion fija, no se ejecuta lo
           pedido en marcha (PAGE_EXECUTE_*: W^X), READONLY/NOACCESS se
           apuntan pero la pagina sigue RW, VirtualQuery solo sabe de lo
           suyo, y una pedida de mas de 64 MiB de una vez no cabe.
      P4f [MEDIDO el 27-09; P4f1 HECHO en el banco] LO QUE PIDE LA `std`
           DE RUST PARA WINDOWS, que es lo que trae BMOX-12 (P3c). Medido sin
           Windows: un programa de Rust con hilos, ficheros, entorno y hora,
           compilado para `x86_64-pc-windows-msvc` y enlazado con bibliotecas
           de importacion VACIAS -- `lld-link` dice cada nombre que falta. Son
           157; la casa tenia 36. Los 121 que faltaban, en el orden en que los
           toca un programa al arrancar:
```text
   P4f1  texto, consola      MultiByteToWideChar, WideCharToMultiByte,
         y modulos           CompareStringOrdinal, lstrlenW, GetConsoleMode,
                             WriteConsoleW, GetConsoleOutputCP, LoadLibraryA,
                             GetModuleHandleA/ExW, GetProcAddress   [HECHO]
   P4f2  hilos y esperas     WaitOnAddress, WakeByAddressSingle/All, Fls*,  [HECHO]
                             CreateMutexA, ReleaseMutex, IsThreadAFiber,
                             SetThreadStackGuarantee, GetCurrentProcess,
                             DuplicateHandle, SetHandleInformation, SleepEx,
                             WaitForSingleObjectEx, los temporizadores
                             esperables, GetSystemTimePreciseAsFileTime
   P4f3  ficheros y          CreateDirectoryW, RemoveDirectoryW, DeleteFileW,  [HECHO*]
         directorios         MoveFileExW, CopyFileExW, FindFirstFileExW,
                             FindNextFileW, FindClose, GetFullPathNameW,
                             Get/SetCurrentDirectoryW, GetFileInformation*,
                             SetFileInformationByHandle, SetFileAttributesW,
                             SetFileTime, LockFileEx, UnlockFile, GetTempPathW,
                             GetFinalPathNameByHandleW, y los Nt*File de ntdll
   P4f4  lo que existe y     CreateProcessW, CreatePipe, TerminateProcess, la  [HECHO]
         dice NO             red (ws2_32: WSAStartup, socket...), dbghelp:
                             estar, para que el .exe CARGUE, y contestar el
                             fallo de Windows si se llama
   P4c   las excepciones     AddVectoredExceptionHandler, RtlCaptureContext,
                             RtlLookupFunctionEntry, RtlVirtualUnwind
   P4f5  el CRT de MSVC      vcruntime140.dll (memcpy, memset, memcmp,  [HECHO*]
                             __CxxFrameHandler3, __C_specific_handler...) y
                             los api-ms-win-crt-*.dll (el arranque del CRT)
```
           P4f1 **Como se sabe:** `prueba/texto.exe` dice `bien` veintiuna
           veces en el anfitrion: UTF-8 <-> UTF-16 con el byte malo y el
           bufer corto de Windows, WriteConsoleW escribe su linea, y
           LoadLibraryW(d3d12.dll) + GetProcAddress(D3D12CreateDevice) sale de
           la MISMA tabla que resolvio las importaciones. Lo que no es
           Windows, dicho: una pagina de codigos (UTF-8), sin mayusculas que
           cuenten solo en ASCII, sin ordinales en GetProcAddress, y ninguna
           DLL de verdad del disco (P5).
           P4f2 [HECHO en el banco 27-09] **Como se sabe:**
           `prueba/esperas.exe` dice `bien` veinticuatro veces en el
           anfitrion, estable en cinco corridas: mutex con recursion,
           ERROR_NOT_OWNER y WAIT_ABANDONED de un hilo que acaba sin
           soltarlo; un temporizador manual a 20 ms y uno automatico con
           periodo; WaitOnAddress importado de su API set
           (`api-ms-win-core-synch-l1-2-0.dll`, como la `std` de Rust -- la
           casa resuelve los `api-ms-win-core-*` y `kernelbase.dll` con la
           tabla de kernel32); FLS con su callback al acabar el hilo y en
           FlsFree (quitarlo da MAL, comprobado); DuplicateHandle; y la hora
           del dia de la placa (`INFO_FECHA`, `bmo_proton_x::hora`). El
           planificador puro suma Objeto::Mutex y Objeto::Temporizador, y un
           hilo que espera un temporizador ya no cuenta como bloqueo mutuo.
           **hilos.exe en Windows (28-09):** se callaba tras la seccion
           critica: la prueba leia el TEB de un hilo que YA habia acabado, y
           Windows lo libera (la casa no, y por eso aqui pasaba). Cada hilo
           mira ahora su TEB estando vivo. Falta verlo otra vez en Windows.
           Lo que no es Windows, dicho: sin APC, un handle duplicado es el
           mismo numero con una copia mas, un fichero no se duplica, y la
           hora de la placa se toma como UTC.
           P4f3 [HECHO en el banco 27-09] LAS CARPETAS. Lo que el FAT32 de
           BMO-X da a Ring 3 hoy es leer, crear o reemplazar un fichero
           entero y LISTAR una carpeta; con eso son de verdad FindFirstFileW
           (comodines de Windows, `.` y `..` fuera de la raiz), los
           atributos de carpeta sin leer el fichero, abrir una CARPETA con
           FILE_FLAG_BACKUP_SEMANTICS (la `std` de Rust lo hace para
           `metadata`), GetFullPathNameW, Get/SetCurrentDirectoryW,
           GetTempPathW, la informacion por handle (FileBasicInfo,
           FileStandardInfo, FileAttributeTagInfo), cambiar la medida
           (SetFileInformationByHandle, SetEndOfFile), GetFinalPathNameByHandleW
           y CopyFileW/ExW. `Plataforma::listar` (en BMO-X, `bmo::Directorio`)
           y `bmo_proton_x::ficheros` (`comodin`, `partir_patron`,
           `ruta_o_raiz`). **Como se sabe:** `prueba/carpetas.exe` dice
           `bien` treinta y cuatro veces en el anfitrion, dos veces seguidas
           (sabotear el comodin da MAL, comprobado). *Lo que NO es: el FAT32
           de BMO-X no BORRA, no crea CARPETAS y no RENOMBRA desde Ring 3 --
           no hay la operacion en el kernel. DeleteFileW, RemoveDirectoryW,
           CreateDirectoryW y MoveFileExW contestan el error de Windows si lo
           nombrado no esta, y ERROR_ACCESS_DENIED dicho por la consola si
           esta: es lo que el kernel tiene que aprender (tres operaciones de
           FAT32 en Ring 0, fuera de PROTON-X). Tampoco: las fechas de los
           ficheros (0), atributos que no sean NORMAL/DIRECTORY, SetFileTime,
           y la rutina de progreso de CopyFileExW.
           P4f4 [HECHO en el banco 27-09] LO DEMAS. Dos cosas que la medida
           no vio y leer la `std` si: `File::read`/`write` de Rust van por
           **NtReadFile/NtWriteFile** de ntdll (hechas DE VERDAD, con su
           IO_STATUS_BLOCK y STATUS_END_OF_FILE), y el azar de `HashMap` es
           **ProcessPrng** de `bcryptprimitives.dll`, importada por
           raw-dylib (no salia en la lista del enlazador; hecha con RDRAND).
           Ademas: ReadFile/WriteFile con OVERLAPPED en un handle sincrono y
           GetOverlappedResult, SetStdHandle, FormatMessageW (los mensajes de
           sistema en castellano, `bmo_proton_x::mensajes`) y LocalFree,
           RtlNtStatusToDosError, GetWindows/SystemDirectoryW,
           GetUserProfileDirectoryW (USERPROFILE: la carpeta del `.exe`),
           QueryDosDeviceW, y del propio proceso GetProcessId,
           GetExitCodeProcess y TerminateProcess. Y lo que EXISTE y dice NO
           como Windows sin eso: ws2_32 entera (WSAStartup da
           WSASYSNOTREADY, cada socket WSANOTINITIALISED), CreateProcessW
           (2 si el `.exe` no esta; si esta, dicho), CreatePipe,
           Nt{Create,Open}File, NtCreateNamedPipeFile, NtSetInformationFile,
           DeviceIoControl, ReadFileEx/WriteFileEx (sin APC), ReadConsoleW
           (no hay entrada de consola) y los enlaces (el FAT32 no los
           tiene). Las DLL `ntdll`, `kernelbase`, `ws2_32`, `userenv` y
           `bcryptprimitives` se cargan por nombre (GetModuleHandle +
           GetProcAddress). **Como se sabe:** `prueba/sistema.exe` dice `bien`
           veintidos veces en el anfitrion y sale por TerminateProcess -- y
           en Windows (el propietario, 27-09) cumple igual --
           (sabotear STATUS_END_OF_FILE da MAL, comprobado). **La cuenta:** de
           los 157 nombres de la `std` de Rust (y ProcessPrng) faltan 18: los
           4 de P4c y los 14 del CRT de MSVC (P4f5). `C:\Windows` es un
           nombre que se da, no una carpeta del volumen.
           P4f5 [HECHO en el banco 27-09, *sin lo de C++] EL CRT DE MSVC. La
           casa (`crt.rs`) resuelve `ucrtbase.dll`, `vcruntime140.dll` y los
           API set `api-ms-win-crt-*`, que es de donde importa un `.exe` de
           Visual C++ con el CRT dinamico: el arranque (_configure_narrow/
           wide_argv, __p___argc/argv/wargv con las reglas de argv del CRT,
           el entorno, _initterm y _initterm_e), la salida (exit corre lo de
           _crt_atexit al reves; _exit no; las tablas de onexit), el monton
           (malloc, calloc, realloc, free: el de Windows del proceso, P4e) y
           memoria y cadenas (memcpy, memmove, memset, memcmp, memchr,
           strlen, wcslen, strcmp, strncmp). **Como se sabe:**
           `prueba/ucrt.exe` dice `bien` diecinueve veces en el anfitrion, y
           su ultima linea la escribe una funcion de _crt_atexit (quitar esa
           llamada de exit la calla, comprobado). **En Windows (27-09):** se
           CALLO despues de _initterm_e -- la prueba pasaba una tabla de
           onexit con basura, y el UCRT de verdad da por iniciada una tabla
           cuyo primero no es su fin (no la toca) y registrar en ella tumba el
           proceso. La casa perdonaba lo que Windows no: ahora hace lo mismo,
           y la prueba pasa la tabla a ceros como el CRT. Y otra vez en
           Windows (27-09): diecinueve `bien`, la ultima desde _crt_atexit. **Y el printf (27-09):** `bmo_proton_x::formato` es
           el `printf` de C sin punteros (banderas, ancho, precision, los
           largos de Microsoft, %d %u %o %x %c %s %ls %p %e %f %g; `%n` y `%a`
           tal cual), probado contra C en veinticinco casos; la casa da
           __acrt_iob_func, los __stdio_common_v(f)(s)(w)printf con las
           banderas de las cabeceras del UCRT (snprintf estandar, _vsnprintf
           legado, %s ancho en swprintf), puts, fputs, fputc, putchar,
           fwrite y fflush, con stdout y stderr en modo texto. **Como se
           sabe:** en Windows (27-09) trece de catorce: la funcion CRUDA
           __stdio_common_vsprintf da -2 cuando no cabe (es la en linea
           `_vsnprintf` la que lo vuelve -1); la casa y la prueba ya dicen
           -2. `prueba/stdio.exe` dice `bien` quince veces (quitar las
           dos cifras del exponente de %e da MAL, comprobado). *Lo que falta, dicho: lo del
           mecanismo de excepciones de C++ (__CxxFrameHandler3,
           _CxxThrowException) va con P4c; `fopen` y los FILE de ficheros
           no estan (los tres estandar si); y lo que un `.exe` de MSVC enlaza
           ESTATICO (mainCRTStartup, __chkstk, _fltused, _tls_index, atexit)
           va dentro de su imagen, pero sus objetos piden alguna funcion mas
           del CRT -- la lista exacta la dara `rayosx` sobre BMOX-12 (P3c).

## 3b. EL ORDEN, escrito (27-09): lo que sigue y por que

El propietario pidio un orden preciso, "para no chocar". Sale de las casillas
de arriba y de lo que cada una PIDE, no de gustos:

```text
   1  P2 y P3a en el metal   [HECHO 27-09 11:23] obedecen: letras, clic, q
   2  P4d en el metal        [HECHO 27-09 11:23] ficheros.exe, 16 bien
   3  P4c, el despachador    SEH: RaiseException, __C_specific_handler,
                             RtlUnwindEx (la base ya esta). PENDIENTE: se
                             salto el 27-09 para no parar el orden; P3c lo
                             PIDE, asi que va antes que P3c
   4  P4e                    [HECHO en el banco 27-09] HeapAlloc,
                             VirtualAlloc, GetModuleFileNameW, la linea de
                             ordenes, el entorno. Falta: run sys/proton-x.bex
                             window/crt.exe en el Ryzen (35 bien)
   5  P3c                    el BMOX-12 de EPICX-FRAMEWORK sin tocar.
                             MEDIDO (27-09): le faltan P3c2 (D3DCompile,
                             pagando una vez), P3c3 (el bytecode de SM5) y
                             P4c; P3c1 hecho
   6  VERRANO V2 a V4        PLAN_VERRANO: profundidad, constantes, y el
                             emisor SPIR-V a SM86. P3b4 los PIDE
   7  P3b4                   el lote de PROTON-X lo dibuja la 3060
   8  P5, P6, F              un juego de tu GOG, los rayos, Cyberpunk
```

**La carpeta `window` (27-09, lo pidio el propietario):** desde hoy los `.exe`
de Windows viven en `window/` del volumen de datos, no en `apps/` (que es de
las aplicaciones de BMO-X). La crea el build (`ejemplos.ps1`) con su
`LEEME.TXT`, y copia ahi los de prueba. Se lanzan con `run sys/proton-x.bex
window/x.exe`, y su directorio actual es `window`. Lo de arriba que dice
`apps/x.exe` es la historia de cuando se vio.

P3b4 NO va antes que VERRANO V4: la 3060 de hoy corre los programas FIJOS
del `.bsf` del cubo, no un DXIL cualquiera; traducir uno pide el emisor de
V4. Por eso, aunque P3b4 este antes en la lista de arriba, se hace despues.

- [ ] **P5 -- un juego chico de verdad.** Uno de TU biblioteca de GOG, 64
      bits, D3D11 o D3D12, sin antitrampas, elegido por `rayosx` (el de
      MENOS importaciones que diga DENTRO). **Como se sabe:** su primer nivel
      se juega en el Ryzen.
- [ ] **P6 -- los rayos (DXR).** Lo que Cyberpunk pide y ningun escalon de
      abajo: estructuras de aceleracion y los sombreadores de rayos, hacia
      las unidades RT de la 3060. Es el escalon mas lejano de la escalera de
      VERRANO (Quake II RTX, `PLAN_VERRANO.md` 2d). **Como se sabe:** un
      `.exe` DXR tuyo dibuja igual que en Windows.
**Medidas pendientes con `rayosx` (27-09).** `rayosx` sin nada detras solo
muestra su ayuda; hay que darle el `.exe` o la carpeta:

```text
   python toolchain\tools\rayosx\rayosx.py ruta\a\BMOX-12.exe          P3c: lo que
                                                                    le falta de verdad
   python toolchain\tools\rayosx\rayosx.py "D:\...\Cyberpunk 2077"      F: otra vez,
                                                                    ya en el disco D:
```

El propietario borro Cyberpunk de C: y lo esta pasando a D:; la medida del
25-09 (Ludoteca, seccion 9) sigue valiendo hasta repetirla alli.

**Donde instalar Cyberpunk (27-09, el propietario: "PERSONAL D ese podria
entrar en mi Cyberpunk 2077").** `Personal (D:)` tiene 111 GB libres y el
juego pide unos 70 GB (Ludoteca, seccion 9): cabe, con margen para Phantom
Liberty y parches. Instalarlo ahi PARA WINDOWS (con GOG, en NTFS) es gratis y
no estorba a nada. Lo que se dice para cuando llegue F: BMO-X no lee NTFS
(Ludoteca, seccion 8), y su FAT32 busca por nombres 8.3 y no guarda ficheros
de mas de 4 GiB -- Cyberpunk tiene nombres largos (`basegame_4_gamedata.archive`)
y ficheros grandes. Para que BMO-X lo lea hara falta, entonces, copiarlo a
un volumen que BMO-X entienda con nombres largos y ficheros grandes (ESTRATOS
ya los guarda; le falta copiar una carpeta entera, E2 de la Ludoteca), o un
lector de NTFS de solo lectura. No se decide hoy: F es el ultimo escalon.

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
