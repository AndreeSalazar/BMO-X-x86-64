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
      P3b4 LA 3060 DIBUJA EL LOTE (rehecho el 28-09, tras E5 en el metal:
           ya no pasa por SPIR-V; el Programa de la casa va DIRECTO a SASS).
           La CPU dirige -- traduce una vez, copia los datos, lanza --; la
           3060 dibuja.
           4a  [HECHO en el banco 28-09] PAGAR UNA VEZ:
               `bmo_proton_x_sm86::pso` -- el Enlace del PSO y el input layout
               del juego a los dos programas (emisor, pegamento, JUEZ), en un
               Almacen por PSO y paso. El pegamento lee el vertice TAL CUAL
               lo da el juego (cada elemento en su byte; lo que no trae, el
               0/1 de D3D). **Como se sabe:** `bmox12.exe` sin tocar en el
               banco: 39 lotes, UNA traduccion, y en cada lote su de vertice
               con los registros que el pegamento cargaria de los DATOS da los
               bits de la casa en los 24 vertices (936). DATOS de 1104 B.
           4b  [x] EL KERNEL -- HECHO Y VISTO EN EL METAL el 28-09 15:53, las
               cuatro piezas: `gpu verrano` (tramo "16 paginas y 16 de DATOS:
               36 de 36"), `gpu verrano bmox12 0/30/60` (12 triangulos en UN
               dibujo, con indices y descarte por la 3060, IGUAL a D3D12:
               0xab7afc663a345885, 0x2b3985e93e1a6574, 0x8dc7ef10f691548e),
               `gpu verrano banco anillo` (1581 fps de pared, la 3060 48 us,
               preparar 27 us, IGUAL) y `gpu verrano bmox12 30 enram` (la 3060
               pinto en la RAM de la app; leido en 0 ms, IGUAL). Antes, el
               mismo dia, el primer build cayo: el `draw` de V0 del director
               pedia 384 KiB de pila de Ring 3 y la autopsia del kernel volvio
               a fallar leyendola. Arreglado cambiando piezas (autopsia por la
               fisica, fallo anidado dicho, marco de la sonda en la pantalla,
               `pila.py` que ahora SI cuenta la sonda y mide Ring 3). El
               detalle de cada pieza:
               EL KERNEL (Ring 0, dicho que si el 28-09): (1) dibujar en
               la ventana de la app -- su memoria prestada a la 3060 por la
               IOMMU, como `lienzo` --, no en un recuadro fijo [HECHO en el
               banco 28-09: `destino` (VA 0x7_0000_0000, 4 PT, PTE de sistema
               hacia la IOVA 0x5800_0000; el camino de `volcado`); VRN1 lleva
               el destino (la VA de la app, fila, medida); el kernel exige un
               bloque ESCRIBIBLE de quien pide, lo presta SOLO mientras dibuja
               y lo devuelve siempre; sin limpiar (la app limpia su back
               buffer). Prueba en el metal: `gpu verrano bmox12 30 enram`, la
               3060 dibuja en RAM del escritorio y se compara leyendo de ahi.
               Hoy solo 1280x720]; (2) DATOS en un
               tramo propio de VRAM (64 KiB; hoy 1 KiB y BMOX-12 pide 1104 B)
               [HECHO en el banco 28-09: `vram::DATOS`, 16 paginas detras del
               tramo, en las entradas 320..335 de su PT; el anillo, 1 KiB por
               ranura (`anillo::cabe`). FALTA el metal];
               (3) el dibujo CON INDICES por el hardware; (4) el descarte de
               caras por el hardware (hoy lo hace la CPU con la tanda)
               [(3) y (4) HECHOS en el banco 28-09: el paquete VRN1 (cabecera
               de 64 B: el byte de los indices, el descarte y el giro de
               delante, cuantos vertices hay), cada indice validado; las
               ordenes con OGL_SET_FRONT_FACE/CULL_FACE/CULL y el bufer de
               indices (SET_INDEX_BUFFER_*, DRAW_INDEX_BUFFER), metodos de
               clc797.h de NVIDIA. `gpu verrano bmox12` ya manda los 24
               vertices y los 36 indices; la CPU no escoge caras. FALTA el
               metal (y si el cubo sale del reves: `antihorario`)].
           4c  [ ] la puerta de la 3060 en la app `proton-x` (su ejecutor del
               lote, `Plataforma::dibujar`), y en el Ryzen: `run
               sys/proton-x.bex window/bmox12.exe` dibujado por la 3060, con
               EL REGISTRO contra los 36-38 fps de la CPU. Partido en piezas
               el 28-09 (el propietario eligio la PUERTA ESTRECHA, A):
               4c.1 [HECHO banco] el juez R7: el CUERPO que manda una app es
                    lista blanca (FADD FMUL FFMA FMNMX MOV MUFU, sin c[][],
                    EXIT al final, registros suyos). R0..R6 dicen si un
                    programa esta bien hecho; no a QUE memoria va: por eso la
                    app manda el cuerpo y el pegamento lo pone el KERNEL.
               4c.2 [HECHO banco] la RECETA (VRN2, `receta`): los cuerpos,
                    sus cargas, el input layout y los DATOS; el kernel pega
                    (su pegamento), comprueba que cada lectura cae en los
                    DATOS, juzga y dibuja. PEGADA DA LOS MISMOS DOS PROGRAMAS,
                    byte a byte, que los del metal. De paso: el pegamento no
                    comprobaba que el destino de una carga fuera del cuerpo
                    (una carga en `D` cambiaria el puntero de los datos por
                    un valor de la app): ahora si (`cargas_propias`).
               4c.3 [HECHO banco, FALTA el metal] la PROFUNDIDAD por la 3060
                    (`profundidad`: ZF32 bloque-lineal en VRAM 0x0A00_0000,
                    kind GENERIC_MEMORY, VA 0x8_0000_0000; SET_ZT_*, la regla
                    de D3D tal cual en SET_DEPTH_FUNC, la limpieza). BMOX-12
                    dibuja con DepthEnable/LESS: sin esto su cubo solo sale
                    bien por ser convexo. PRUEBA en el Ryzen:
                    `gpu verrano bmox12 30 z` (sin descarte, con Z) tiene que
                    dar IGUAL; `gpu verrano bmox12 30 ambas` (sin descarte ni
                    Z), DISTINTO. [!] Lo que se prueba ademas: si la 3060
                    acepta el color LINEAL (pitch) con la Z bloque-lineal. Si
                    no, el color va a una sombra bloque-lineal en VRAM y el
                    motor de copia la lleva a la RAM de la app.
               4c.4 [HECHO, FALTA el metal] la puerta en el kernel:
                    `IOMMU_OP_GPU_DIBUJAR` (0x48), desviada en
                    `op_maquina::iommu` ANTES de las dos llaves y solo hacia
                    `gpu_trabajo::receta` (regla P2 de `la-3060`, probada
                    ensanchandola: la para). La ficha del GR la pone el
                    kernel (la del lienzo del escritorio: sin `gpu verrano`
                    antes, NO). El taller del pegado, estatico.
               4c.5 [HECHO banco, FALTA el metal] el ejecutor en la app
                    (`apps/proton-x/src/la3060.rs`) y `proton-x-sm86::puerta`
                    (cuerpos por PSO, pegado de prueba una vez, la receta de
                    cada lote, la Z coherente). El banco: los 39 lotes de
                    `bmox12.exe` sin tocar pasan por la puerta con los
                    programas del metal y la Z limpiada en cada fotograma. El
                    monton de la app alinea a pagina lo de 64 KiB o mas (el
                    back buffer es el destino).
                    EN EL RYZEN, en este orden: `gpu verrano bmox12 30 z`
                    (IGUAL) y `... ambas` (DISTINTO); luego `run
                    sys/proton-x.bex window/bmox12.exe`: tiene que decir
                    "PROTON-X: la 3060 dibuja los lotes" y EL REGISTRO sus
                    fps (contra los 36-38 de la CPU). Si dice "este lote va
                    por la CPU: ...", el porque viene en la misma linea.
               4c.6 [HECHO banco] la CPU DEJA DE LIMPIAR (29-09): la casa
                    APUNTA el ClearRenderTargetView y el de la Z en vez de
                    llenar 3,6 MB cada uno por fotograma; los hace quien
                    dibuje (la 3060 en su dibujo: `Dibujo.color`, +20/+24 de
                    la receta, floats byte/255 que la 3060 redondea al mismo
                    byte; la CPU al empezar el suyo), o quien lea antes
                    (Present, CopyTextureRegion). Las huellas de la casa, las
                    mismas. Y el pegado ya no copia programas por valor: el
                    kernel pega en su taller (`vertice_en`/`pixel_en`), 336 B
                    de marco en vez de 13.056.
               4c.6b [EL METAL DIJO NO, 28-09 20:31] `gpu verrano bmox12
                    30 z`: el estado entero (34 de 34) y los vertices
                    pagados, el dibujo NO: Xid 69 (error de clase del motor
                    grafico) y el canal GR MUERTO hasta reiniciar; `ambas`
                    despues, 0 de 34 (ya muerto). Y `bmox12.exe` esperaba
                    1 s ENTERO por lote (el tope del kernel): 0 fps. **Por
                    que:** la 3060 no dibuja con una Z en bloque sobre un
                    color PITCH -- nouveau apaga la Z si el color es lineal
                    (`nvc0_validate_fb`: `zsbuf && !cbuf_is_linear`) y NVK
                    dibuja en una SOMBRA en bloque y la copia
                    (`nvk_rendering_linear`: "Depth and stencil are never
                    linear"; `nvk_linear_render_copy`, con el motor de
                    copia). Nuestro destino es el back buffer de la app, en
                    su RAM: pitch. **Lo hecho (Ring 3):** la puerta de la
                    app no manda un lote con Z (`puerta::Z_CON_COLOR_PITCH`)
                    y dice por que; un dibujo NO pagado apaga la 3060 a la
                    PRIMERA con la escalera dicha (no un segundo por lote).
                    El banco sigue comprobando la receta con Z (la que el
                    kernel recibira). **Lo que falta (Ring 0, por aprobar):**
                    la SOMBRA -- el color en VRAM en bloque, junto a la Z, y
                    una copia bloque->pitch al back buffer con un objeto de
                    copia (C7B5) EN el canal GR, tras un semaforo; y que el
                    kernel, al oir un RC_TRIGGERED, marque el canal muerto y
                    diga NO al instante (y el juez niegue Z sin sombra).
               4c.6d [HECHO banco, Ring 0 aprobado 29-09] EL CANAL MUERTO:
                    el kernel lee la cola del GSP sin moverla
                    (`rpc::rc_del_canal`); un RC_TRIGGERED del canal de GR --
                    o dos dibujos seguidos esperados sin pagar -- lo marca
                    MUERTO, la cabina dice su Xid UNA vez, y todo trabajo del
                    GR dice NO al instante (`IOMMU_NO_CANAL_MUERTO` 90): ni un
                    segundo mas por lote (el metal del 28-09: 1041 ms por
                    fotograma). La app, al oirlo, deja la 3060 y lo dice.
               4c.6c [VISTO EN EL METAL 28-09 22:36] `gpu verrano bmox12 30
                    z`: IGUAL (huella 0x2b3985e93e1a6574, la de D3D12), la
                    3060 en 1176 us; `ambas`: DISTINTO (74648 pixeles), como
                    tocaba. `run sys/proton-x.bex window/bmox12.exe`: "la 3060
                    dibuja los lotes", 59-90 fps (dibujar 10-15 ms) contra
                    34-36 de la CPU. El cuello NO es la 3060 (1 ms por
                    fotograma): es el camino -- un syscall que ESPERA cada
                    lote, el prestamo del back buffer (900 paginas) y su
                    devolucion, la sombra (3,6 MB) copiada por un PCIe en
                    Gen1 (lo que el GSP-RM deja al arrancar) y el RM en P5.
                    Lo siguiente es MEDIRLO por partes.
               4c.6d [ANALISIS 29-09, por medir] EL CUELLO: ~90 fps por
                    la receta contra los ~28000 del cubo con la GPU al
                    maximo. NO es la misma carrera:
                    - los 28000 son el ANILLO (`en_anillo`): las ordenes se
                      arman UNA vez, el juez juzga UNA vez (huella), no hay
                      prestamo por fotograma y la 3060 se come el anillo
                      sola; la CPU casi no entra
                    - la receta de una app (`cubo::receta`) va SIEMPRE por
                      `en_frio`, un syscall que ESPERA el dibujo, y cada
                      lote paga: leer y validar la receta, `pegar` (juntar
                      programas) y el juez otra vez, prestar el back buffer
                      (~900 paginas por la IOMMU) y devolverlo (con su
                      invalidacion), escribir las ordenes en la VRAM, la
                      copia de la sombra (3,6 MB) si hay Z, y esperar el
                      semaforo. La 3060 dibuja en ~1 ms; lo demas es camino
                    - LA MEDIDA: la linea `[3060]` de `la3060.rs` (una por
                      segundo en `run sys/proton-x.bex window/bmox12.exe`)
                      parte el lote en puerta / 3060 / preparar / sombra /
                      resto. SIN esa linea del metal no se toca nada.
                    - los arreglos, en orden de lo que se espera que pese
                      (por confirmar con la linea): (1) prestar el back
                      buffer UNA vez y dejarlo prestado mientras la app viva
                      (va en `resto`); (2) pegado y juez por huella, como
                      el anillo (va en `resto`); (3) la receta caliente: si
                      la huella de lo fijo no cambia, solo vertices y
                      constantes (va en `preparar`); (4) no esperar: el
                      syscall vuelve con el timbre y el siguiente espera
                      al anterior (doble buffer; esconde `3060`); (5) el
                      PCIe en Gen1 (va en `sombra`).
               4c.9 [INVESTIGADO 29-09, pedido por el propietario: "que
                    podemos hacer CERO COPIAS por completo, que solo sea
                    verificado automatico por algo"] **EL CAMINO DE UN
                    FOTOGRAMA, COPIA A COPIA** (BMOX-12, 1280x720, con Z;
                    leido en el codigo, no adivinado):
                    ```text
   paso                          quien        bytes / fotograma   donde en el codigo
   ----------------------------  -----------  ------------------  ---------------------------
   1 la receta a la caja         CPU (app)    KiB (vertices,      puerta::preparar
                                              constantes)
   2 leerla                      nadie: SE LEE EN SU SITIO (physmap)  cubo::receta, rc::leer
   3 pegar + el juez             CPU (kernel) KiB, CADA lote      rc::pegar (sin huella)
   4 prestar el back buffer      IOMMU        ~900 paginas y su   prestar_destino /
     y devolverlo                             invalidacion, CADA  devolver_gpu
                                              lote
   5 ordenes y datos a la VRAM   CPU -> PCIe  KiB, TODO el estado  preparar_con: "con
                                              fijo cada vez       destino, siempre en frio"
   6 DIBUJAR                     la 3060      en la VRAM (sombra)  ~1 ms (medido 28-09)
   7 sombra -> back buffer       copia GPU    3,6 MB por el PCIe   volcado::copia_de_sombra
                                              en Gen1, BAJANDO
   8 Present: back buffer ->     CPU (app)    3,6 MB, pixel a      dxgi::present
     superficie                               pixel (y R<->B si
                                              es RGBA)
   9 el escritorio: superficie   CPU          3,6 MB, pixel a      director scene/surface.rs
     -> pantalla                 (director)   pixel con            `compose`
                                              read_volatile, y la
                                              pantalla esta en la
                                              VRAM: SUBIENDO por
                                              el PCIe otra vez
                    ```
                    O sea: el fotograma NACE en la VRAM (6), BAJA a la RAM
                    (7), la CPU lo copia DOS veces (8, 9) y SUBE a la VRAM
                    otra vez (9), donde ya estaba. Lo que la 3060 hace en 1
                    ms, el camino lo paga en 3 viajes de 3,6 MB. Y el
                    "siempre en frio" (5) viene del prestamo (4): con
                    destino, la receta caliente esta apagada.

                    **CERO COPIAS DE VERDAD = el fotograma NO SALE de la
                    VRAM.** La pantalla del GOP YA esta en la VRAM de la
                    3060 (la BAR1), y el kernel YA sabe copiar la sombra a
                    "la ventana de la pantalla" (lo hace `gpu verrano`). Lo
                    que falta es que PROTON-X lo pida. Los escalones, cada
                    uno con lo que lo VERIFICA SOLO (sin que nadie mire):
                    Z0 [por hacer, solo lectura] MEDIR lo que ya se dice:
                       las lineas `[3060]` (pasos 1-7) y `[registro]` (el
                       fotograma, `dibujar` y `presentar`: el 8) de `run
                       sys/proton-x.bex window/bmox12.exe`. El paso 9 no
                       tiene linea: su `ritmo` del escritorio. Sin esto no
                       se sabe cuanto da cada Z; el ORDEN de abajo no
                       depende de ello, la medida de la ganancia si.
                       **Metal 29-09 06:40:** la primera medida NO midio
                       la 3060: el GSP-RM estaba arrancado (`save mode
                       init`) pero nadie dio los pasos del motor grafico
                       hasta `lienzo`; el kernel contesto el motivo 78
                       ("el escritorio no la preparo") tres veces y
                       PROTON-X se paso a la CPU: 22-35 fps, `dibujar`
                       27-42 ms, `presentar` 1-2 ms, la CPU al 100 %. Es
                       el techo de la CPU, no el de la 3060. Arreglo
                       (director, `editor.rs`): `run` de `proton-x.bex` da
                       solo lo que falte hasta `lienzo` si el GSP-RM ya
                       corre (`init`), y si no corre lo dice.
                       **Metal 29-09 07:02, LA MEDIDA DE VERDAD** (dos
                       corridas, ~30 lineas, BMOX-12 por la 3060): 83-98
                       fps, fotograma ~11 ms, `presentar` 1 ms; por lote la
                       puerta 1 us y el kernel ~10,3 ms =
                       ```text
   3060      ~1,1 ms   11 %   el dibujo de verdad
   preparar  ~5,4 ms   52 %   SIEMPRE en frio: tres paginas a cero y
                              RELEIDAS por el PCIe, y los programas, la
                              tabla, los vertices y las ordenes releidos
   sombra    ~1,15 ms  11 %   3,6 MB por el PCIe en Gen1 (~3,2 GB/s)
   resto     ~2,6 ms   25 %   prestar/devolver el back buffer, pegar y el
                              juez, la receta
                       ```
                       O sea: el cuello NO era la copia (Z1) sino la
                       BUROCRACIA de preparar: releer cada palabra para
                       saber que llego. Por eso Z2 va ANTES que Z1.
                    Z1 [VISTO EN EL METAL 29-09 07:50] BMOX-12: **3749
                       fps** (antes de Z2, 83-98; con Z2, 130-250), en la
                       pantalla 3750 de 3750 y en caliente todos; por lote
                       la puerta 1 us y el kernel **264 us** = 3060 36 +
                       preparar 12 + sombra 92 (VRAM a VRAM) + resto 124;
                       `presentar` 0 ms. El PCIe salio en Gen3 x16 en esa
                       sesion; la 3060, en P8. Lo que queda: `resto`
                       (pegar y el juez en cada lote: Z3), la espera
                       sincrona (Z4) y los relojes (P8 -> P0). Visto por el
                       propietario: "se pinto ENCIMA de mi escritorio" --
                       el cubo sale en la ventana fija del centro de la
                       pantalla, no donde esta la ventana de la app: falta
                       confirmar si el resto quedo en negro (pantalla
                       completa) o si el escritorio siguio asomando. Y al
                       cortar con ^C, `cabina`: "mem no devuelto: sigue
                       PRESTADO a otro =3690496" (la superficie, que el
                       escritorio aun tenia tomada; por mirar si pasaba
                       tambien antes de Z1). Asi quedo, pieza a pieza:
                       - la receta: +88 bit 0 = "el destino es un back
                         buffer de la cadena" (`Dibujo::cadena`, lo pone la
                         casa de DXGI). La receta NUNCA dice "a la
                         pantalla": eso lo decide el kernel
                       - el escritorio: una superficie con el formato
                         `SUP_LA_3060_DIRECTA` (2) se pone a pantalla
                         completa, en negro, no se compone y le da la
                         pantalla al tid de su app con la orden nueva
                         `IOMMU_OP_GPU_PANTALLA_PARA` (0x49), que pasa por
                         las dos llaves de la IOMMU: SOLO el escritorio.
                         Al morir la app, o si deja de pedirla, se le quita
                       - el kernel (`cubo::receta`): si el pid es el que el
                         escritorio dijo, el destino es de la cadena y mide
                         1280x720, `Dibujo::pantalla` -- las ordenes van a
                         la ventana de la pantalla (sin la limpieza de
                         VERRANO, con la de la app), la sombra se copia a
                         la pantalla VRAM a VRAM, y el back buffer NO se
                         presta. El `Ok` lleva el bit 62 (`cu::A_PANTALLA`)
                       - PROTON-X: tras el primer lote por la 3060 pide la
                         pantalla (formato 2 en su cabecera); con el bit 62
                         la casa marca el back buffer `en_pantalla` y su
                         `Present` NO copia nada. Un lote que vaya por la
                         CPU con la pantalla pedida la SUELTA (formato 0):
                         nunca se muestra un fotograma a medias entre la
                         pantalla y la RAM
                       - la linea `[3060]` dice `en la pantalla P de N`
                       Bancos: la huella con y sin pantalla, las ordenes a
                       la ventana (`z1_el_back_buffer_va_a_la_pantalla`),
                       el plan de la sombra, la receta con +88 (y que la
                       app no se pone en la pantalla sola), el bit 62 del
                       `Ok`, y la casa: `cubo12.exe` con un ejecutor que
                       dice `en_pantalla` deja la superficie a CERO
                       (`z1_un_fotograma_en_la_pantalla_no_se_copia_a_la_superficie`).
                       Pila 29662 de 40960, igual. La idea de antes:
                    Z1 PRESENTAR POR LA 3060, A PANTALLA COMPLETA (como D2c
                       de `PLAN_VERRANO.md` con DOOM, y un juego va a
                       pantalla completa igual): los lotes dibujan en la
                       SOMBRA y NADA baja a la RAM; `Present` pide al
                       kernel "sombra -> pantalla" por el motor de copia,
                       VRAM a VRAM (~15 us para 1080p, contra los 3 viajes).
                       Se van los pasos 4, 7, 8 y 9 enteros. Lo que lo
                       verifica solo:
                       - el KERNEL, no la app, sabe si el fotograma esta
                         entero: cuenta los lotes pagados por la 3060 desde
                         el ultimo Present; si UNO fue por la CPU, ese
                         fotograma vuelve al camino de hoy (nunca se ve un
                         fotograma a medias)
                       - el rectangulo de la pantalla lo pone el
                         ESCRITORIO (su tabla), no el `.exe`: una app no
                         puede escribir fuera de lo que se le dio
                       - el escritorio NO pinta encima (como D2c: sin
                         cursor, sin volcar) mientras la 3060 manda
                       - el banco del metal, como `gpu verrano bmox12 z`:
                         el primer fotograma se relee de la pantalla y su
                         huella tiene que ser la de la CPU (IGUAL), y
                         despues uno cada N segundos; si da DISTINTO, la
                         app vuelve a la CPU y se dice por que
                    Z2 [HECHO en codigo 29-09, falta el metal] LA RECETA
                       CALIENTE CON DESTINO. No hacia falta esperar a Z1:
                       el prestamo del back buffer no toca la VRAM (las
                       ordenes apuntan a `destino::VA`, fija; cambia la
                       IOMMU detras). Lo que lo apagaba era la huella:
                       llevaba la DIRECCION del destino, y los dos back
                       buffers de un juego daban dos huellas alternas. Ahora
                       lleva sus MEDIDAS (`tuberia::huella_fija`, con su
                       banco) y el kernel ya no exige "sin destino". Lo
                       verifica SOLO, sin releer: la huella (los mismos
                       programas juzgados, las mismas ordenes), la entrada
                       del GR (nadie lanzo nada entre medias), menos de 100
                       ms desde el ultimo, y que ese ultimo se PAGO entero;
                       si algo falla, el siguiente va en frio. La linea
                       `[3060]` dice `en caliente H de N`. **VISTO EN EL
                       METAL 29-09 07:21:** en caliente 235 de 236 y luego
                       TODOS; `preparar` de ~5,4 ms a **11 us**; el lote
                       de ~10,3 a ~4,6 ms (3060 ~1,2 + sombra ~1,16 +
                       resto ~2,2); **130-250 fps** (antes 83-98). Lo que
                       queda es camino: la sombra por el PCIe y el
                       prestamo del back buffer, que es Z1. (Los primeros
                       segundos la 3060 y la sombra iban al DOBLE de
                       rapido, ~0,6 ms cada una, y luego bajaron: la 3060
                       arranca en P8, reposo; subirla es otro escalon)
                    Z3 [VISTO EN EL METAL 29-09 08:18] PEGAR UNA VEZ:
                       ~4000 fps (antes 3749), el lote de 264 a 247 us,
                       `resto` de 124 a 105 us. Pegar y juzgar costaban
                       ~20 us, no los 124: el resto de `resto` es otra cosa
                       (leer la receta, la huella de lo fijo, las lecturas
                       por el PCIe al esperar) y hay que partirlo antes de
                       tocarlo. NO por huella (una app podria fabricar una
                       colision de 64 bits y colar un cuerpo sin juzgar):
                       `receta::clave` pone en bytes TODO lo que `pegar`
                       lee (los dos cuerpos, cargas, elementos, genericos
                       y los numeros, cada trozo con su medida delante) y
                       `receta::ya_pegada` la compara BYTE A BYTE con la de
                       la ultima receta que el juez aprobo, guardada en el
                       taller del kernel (no en la pila). Igual: el taller
                       ya tiene esos programas pegados y juzgados. Un solo
                       bit distinto, u otro pegado que fallo, y se pega y
                       se juzga entero. Bancos: la misma se reusa; un bit
                       del cuerpo, otra carga u otros registros, no; los
                       DATOS no cuentan; un pegado fallido olvida lo
                       aprobado. Lo esperado: `resto` de ~124 us a unas
                       decenas. La idea de antes:
                    Z3 PEGAR Y JUZGAR POR HUELLA (el paso 3): los programas
                       ya juzgados y ya en la VRAM se reusan si su huella
                       no cambia; la 3060 corre la copia de la VRAM, que
                       es la juzgada, asi que lo que la app toque despues
                       no llega. Lo verifica: la huella de los bytes de la
                       app en cada lote (leer, no copiar), y el juez la
                       primera vez
                    Z4 NO ESPERAR en el syscall (el paso 6): el timbre y
                       volver; el siguiente lote espera al anterior. Lo
                       verifica el semaforo, que ya se lee.
                       **Z4a [HECHO en codigo 29-09, falta el metal]: la
                       copia a la pantalla, lanzada y NO esperada.** Sin
                       nada nuevo de la 3060 (el mismo canal, las mismas
                       ordenes, el mismo semaforo): con `pantalla`, la
                       sombra -> la pantalla se lanza y queda EN VUELO
                       (`volcado::lanzar_copia_de_sombra`); la espera quien
                       vaya a tocar la sombra (`esperar_copia_en_vuelo`,
                       antes del dibujo siguiente o de otra copia). La
                       parte `sombra` de `[3060]` pasa a ser lo que la CPU
                       ESPERO, no lo que tardo la copia. A la RAM de la app
                       (sin pantalla) se sigue esperando: la app la lee.
                       **Z4b [por hacer, con su prueba en el metal ANTES]:
                       que la 3060 encadene sola** el dibujo y la copia con
                       los semaforos del HOST (clase C56F, sacados de
                       `clc56f.h` de NVIDIA 570.144, no de memoria):
                       SEM_ADDR_LO 0x5c (bits 31:2), SEM_ADDR_HI 0x60 (7:0),
                       SEM_PAYLOAD_LO 0x64, SEM_PAYLOAD_HI 0x68, SEM_EXECUTE
                       0x6c con OPERATION (2:0) ACQUIRE 0, RELEASE 1,
                       ACQ_STRICT_GEQ 2, ACQ_CIRC_GEQ 3; ACQUIRE_SWITCH_TSG
                       bit 12; RELEASE_WFI bit 20; PAYLOAD_SIZE bit 24 (0 =
                       32 bits). La prueba primero (`gpu verrano espera`): el
                       canal de COPIA hace ACQUIRE sobre un semaforo que
                       suelta el GR, y la copia sale DESPUES del dibujo sin
                       que la CPU mire. Solo con eso visto, el dibujo tambien
                       se lanza sin esperar.
                       **El esquema, escrito (29-09), para cuando Z3 este
                       medido.** El propietario: *"que se pague una vez y
                       el cocinero tenga todas las mesas listas"*. Es el
                       ANILLO de VERRANO (V1b, `en_anillo`) llevado a la
                       receta: (1) el kernel escribe el lote, toca el
                       timbre y VUELVE con `cu::EN_VUELO`; (2) la copia de
                       la sombra a la pantalla la encadena la PROPIA 3060
                       (el canal de copia espera el semaforo del GR con un
                       SEMAPHORE_ACQUIRE en sus ordenes), no la CPU; (3) el
                       lote siguiente, antes de pisar DATOS y la entrada del
                       GPFIFO, espera la valla del anterior (dos ranuras de
                       DATOS: mientras la 3060 lee una, la CPU escribe la
                       otra); (4) las texturas prestadas se devuelven al
                       pagarse su valla, no al volver; (5) un Xid se sabe en
                       la valla siguiente (el canal muerto ya dice NO al
                       instante). Con la 3060 a ~128 us por fotograma
                       (dibujo + copia), el techo es ~7800 fps en BMOX-12;
                       en un juego de verdad lo que gana es que la CPU del
                       juego y la 3060 trabajen A LA VEZ
                    Lo que NO se hace: que el `.exe` lea su back buffer
                    (casi ningun juego lo hace; si uno lo hace, ese
                    fotograma baja a la RAM, dicho), ni cambiar la
                    pantalla de la UEFI (modeset): la pantalla sigue siendo
                    la del GOP, y el fotograma se COPIA dentro de la VRAM.
                    Ir de "copia en la VRAM" a "cero bytes" pediria que la
                    pantalla lea la sombra (flip), y eso es modeset propio:
                    otro escalon, mas adelante.
               4c.6c [HECHO banco] LA SOMBRA (29-09, Ring 0
                    dicho que si por el propietario): `bmo_gpu_ga10x::sombra`
                    -- el color A8R8G8B8 BLOQUE-LINEAL en VRAM 0x0A40_0000
                    (tras la Z), VA 0x9_0000_0000, kind generico 0x06, el
                    mismo bloque que la Z (16 GOBs). Con Z, `ordenes_dibujo`
                    pone el destino de color en la sombra ANTES de encender
                    la Z, y toda limpieza va a ella, nunca al pitch. El
                    kernel (`en_frio`): (1) si nadie limpia el color, el
                    destino -> la sombra; (2) el dibujo; (3) pagado, la
                    sombra -> el destino (la ventana de la pantalla o el back
                    buffer de la app). Las copias van por el canal de COPIA
                    (COPY2, el del volcado: `volcado::copia_de_sombra`, con
                    su cerrojo y su GPFIFO, ordenes en +0xC00 y semaforo en
                    +0x200), `SET_SRC/DST_BLOCK_SIZE` 0x1040 y el ORIGIN X/Y
                    de Pascal+. Si la copia final no se paga, el dibujo se
                    dice NO pagado (`cubo::sin_dibujo`). La puerta de la app
                    manda ya la Z (`puerta::Z_EN_LA_SOMBRA`). Sin un giro
                    nuevo (trinquete E: 11); pila 29662 de 40960.
                    EN EL RYZEN, en este orden: `gpu verrano bmox12 30 z`
                    tiene que dar IGUAL (y `ambas`, DISTINTO); luego `run
                    sys/proton-x.bex window/bmox12.exe`, sin la linea "este
                    lote va por la CPU" y con EL REGISTRO contra los 34-36
                    fps de la CPU. Si la copia falla, la cabina dice
                    "P3b4c.6b: la 3060 no pago la copia de la sombra".
               4c.8 LAS TEXTURAS POR LA 3060 (TIC/TSC), en escalones
                    (29-09, pedido por el propietario):
                    T0 [HECHO banco] los DESCRIPTORES: `bmo_gpu_ga10x::
                       texturas` -- el TIC de una textura 2D pitch de 8 bits
                       (A8B8G8R8 UNORM, BGRA cruzando X y Z, TWO_D_NO_MIPMAP,
                       coordenadas normalizadas), el TSC de un muestreador
                       de D3D12 (punto/lineal, los cinco modos, borde) y las
                       ordenes de las piscinas (SET_TEX_HEADER_POOL 0x1574,
                       SET_TEX_SAMPLER_POOL 0x155c, y las dos invalidaciones
                       0x1330/0x1334). `proton-x-sm86::muestreo` lleva el
                       Muestreador y la textura de la casa a ellos. [!] Los
                       campos son los de `gm107_texture.xml.h` de nouveau; el
                       banco comprueba sus bits, NO que la 3060 los entienda.
                    T1 [HECHO banco, 29-09] el `TEX` en SASS, SACADO de un
                       binario y no de memoria: `ptxas -arch=sm_86` (CUDA
                       12.9, de PyPI) de un `tex.level.2d` con el asa en un
                       registro -> `TEX.SCR.B.LZ R6, R4, R4, R0, 2D`
                       (0x3800000004047361 / 0x004f4400009e0f06), y cada
                       campo movido y releido con `nvdisasm -b SM86` 13.4
                       (`platform/drivers/gpu/ga10x/sombreadores/tex_bindless.md`). `texturas::tex` da
                       esos bits y cinco combinaciones mas que nvdisasm leyo
                       como se pidieron; `texturas::asa` = tic | tsc << 20
                       (NVK; por comprobar). El juez conoce SOLO esa forma
                       (2D, .LZ, los cuatro canales seguidos, pares
                       alineados): una desacoplada con su barrera (R1 si se
                       lee antes); cualquier otra, R0. FALTA la regla R8: en
                       el cuerpo de una APP el asa la pone el kernel.
                    T2 [HECHO banco, 29-09, Ring 0 pedido por el
                       propietario] EL KERNEL DE LAS TEXTURAS:
                       - donde viven (`texturas`): los texeles son la RAM de
                         la APP, prestada SOLO LECTURA y solo mientras dibuja
                         (IOVA 0x5900_0000, una ranura de 1 MiB por textura,
                         hasta 4); VA 0xA_0000_0000 con PTE de sistema, y
                         detras la pagina de las PISCINAS en VRAM 0x04A0_4000
                         (TIC en +0, TSC en +0x800; la textura k usa los k)
                       - la receta VRN2 lleva sus texturas: +84 cuantas, 48 B
                         cada una detras de los genericos (`DeApp`: su VA,
                         medidas, fila, BGRA y el muestreador), y una carga
                         nueva, el ASA (`2, textura, 0, reg`), solo en el de
                         pixel. `receta` + 0 texturas = la de siempre.
                       - el pegamento pone el asa con un MOV; el juez R7
                         (`juzgar_cuerpo_con_asas`) deja un TEX en el cuerpo
                         SOLO con un asa del kernel y sin pisarla: la app
                         elige coordenadas y registros, NO que TIC ni TSC
                       - las ordenes: SET_TEX_HEADER_POOL / SAMPLER_POOL y
                         las invalidaciones, si el dibujo tiene texturas
                       - el kernel (`receta`): presta cada textura (un
                         bloque de quien manda la receta, `fisica_de`),
                         escribe y relee los TIC/TSC, dibuja y DEVUELVE
                         siempre. Pila 29662 de 40960; sin giros nuevos.
                    T2b [HECHO banco, 29-09] LA APP: el emisor saca
                       `Op::Muestra` como un TEX (2D, nivel 0): las
                       coordenadas en un par alineado que no se devuelve, los
                       cuatro canales en un bloque alineado con UNA barrera
                       (`Clase::Tex`, el planificador la espera en cualquiera
                       de los cuatro) y el asa como precarga `Asa { tN, sM }`
                       fija todo el programa. La pareja (tN, sM) es la
                       textura k de la receta (`pso::texturas_de`); la
                       puerta la saca de los recursos del lote (su RAM, su
                       muestreador) o dice por que no. El simulador corre el
                       TEX con el muestreo de la casa (que iguala a la 3060):
                       el pixel de HelloTexture da los bits del interprete,
                       punto y lineal. `tests/textura.rs`: su PSO es UN TEX
                       de t0/s0 que el juez R7 aprueba con el asa del kernel.
                    T3 [HECHO EN EL METAL 29-09 06:25] `gpu verrano
                       textura`: **96 de 96, IGUAL a la 3060 bajo CUDA
                       (Windows), bit a bit**, los ocho muestreadores 12/12
                       (Point y Linear x Wrap, Mirror, Clamp, Border), las 8
                       en 18586 us, y el barrido Point Clamp `00 00 00 11 11
                       11 22 22 22 33 33 33` como la casa. FALTA: HelloTexture
                       por PROTON-X en la 3060 (T4). La historia:
                       - el TEX CORRE: 8 recetas por la 3060 sin Xid, el
                         juez R7 y el kernel las aceptan, los TIC/TSC se
                         releen bien (T0..T2b valen en el metal)
                       - pero 0/96: TODAS las muestras corridas +1 texel en
                         x y en y (nearest y linear por igual). El barrido
                         (m=8) lo dejo claro: la 3060 dio
                         `11 11 11 22 22 22 33 33 33 -- -- --` donde la
                         casa espera `00 00 00 11 11 11 22 22 22 33 33 33`
                       - la causa: el TIC palabra 4, BORDER_SIZE (31:29) en
                         0 = BORDER_SIZE_ONE: la 3060 cree que la imagen
                         trae un texel de borde GUARDADO en memoria y se lo
                         salta. Arreglo (29-09): `texturas::
                         BORDE_DEL_MUESTREADOR` = 7 << 29
                         (BORDER_SIZE_SAMPLER_COLOR, lo de nouveau): el borde
                         lo da el TSC. El metal: 96/96 (arriba).
                       - el muestreo de la casa YA iguala al hardware bit a
                         bit (`tests/metal_textura.rs`, 96/96 contra CUDA en
                         `docs/metal/tex_cuda/SALIDA.TXT`): si el metal da
                         otra cosa tras el arreglo, es el TIC/TSC, no la
                         cuenta.
               4c.7 [HECHO banco] TEXTURAS, el camino de D3D12HelloTexture
                    (29-09): `bmo_proton_x::textura` muestrea (punto y
                    bilineal con fraccion de 8 bits, los cinco modos de
                    direccion de D3D12, color de borde); `Op::Muestra` lo
                    corre el interprete; DXIL (dx.op.sample, handles SRV y
                    sampler) y SM5 (`sample`, dcl_resource texture2d,
                    dcl_sampler) lo traducen. La casa: texturas 2D
                    RGBA/BGRA comprometidas, subida buffer->textura por
                    CopyTextureRegion, CreateShaderResourceView,
                    CreateSampler, samplers estaticos de la firma (1.0 y
                    1.1), SetGraphicsRootDescriptorTable,
                    CheckFeatureSupport(ROOT_SIGNATURE),
                    D3DCompileFromFile. **Como se sabe:**
                    `tests/textura.rs` hace lo que hace HelloTexture por las
                    vtables de la casa y los 4096 pixeles del quad son los
                    texeles esperados, con firma 1.0 y 1.1. Lo que FALTA,
                    dicho: la 3060 aun no muestrea (TEX, descriptores
                    TIC/TSC): un PSO con texturas no va a nativo ni a la
                    3060, se interpreta en la CPU y la casa lo avisa.
               De paso (28-09): el cerrojo del GR se quedaba TOMADO si un
               destino malo salia por un `?` (`verrano` en el kernel): la 3060
               decia "uno en marcha" hasta reiniciar. Ahora toda salida lo
               suelta.
- [ ] **P3c -- el BMOX-12 de EPICX-FRAMEWORK, sin tocar.** [VISTO EN EL
      METAL el 28-09 06:41: `run sys/proton-x.bex window/bmox12.exe` en el
      Ryzen abre su ventana (1282x749) y el cubo GIRA, dibujado por la CPU;
      112 funciones de la casa, codigo 256 KiB sellado, TLS con 1 callback,
      sus cifras de memoria impresas, y la autopsia: ningun fallo de Ring 3.
      Queda abierta hasta que el cubo lo dibuje la 3060 (P3b4). Desde ese
      dia EL REGISTRO: `bmo_proton_x::registro`, una linea por segundo
      (`[registro] N fps  fotograma min/medio/max ms  dibujar  presentar`) a
      la consola, y con ella a datos/sysproto.txt. PRIMERA MEDIDA (28-09
      07:01, el Ryzen, un nucleo, SSE2; datos/sysproto.txt): 36-38 fps,
      fotograma 23..36 ms (medio 27), dibujar 25-26 ms, presentar 1 ms,
      estable en 492 fotogramas. El banco da ~7 fps: esta en debug y calcula
      la huella de cada fotograma dentro de presentar; la cifra de
      referencia es la del metal] [DIBUJA en el banco 28-09
      LO QUE DIBUJO LA 3060] Con los .cso de su
      HLSL CRLF (d7e2992c y b50c1000, de sombras.exe en el Windows del
      propietario: bit a bit los mismos que los de LF), bmox12.exe en
      interactivo da en los Present 0, 30 y 60 las huellas de la 3060, sin
      un aviso, y ESC sale con Ok; con `--fotograma 30` guarda por READBACK
      y la `std::fs` de Rust un PNG cuyos pixeles (descomprimidos fuera del
      banco) dan 0x2b3985e93e1a6574, la huella de la 3060. Para eso la casa
      aprendio WS_VISIBLE: una ventana creada visible se muestra YA (sin
      ShowWindow), como en Windows; sin eso sus teclas no le llegaban (el
      banco lo cazo: mil Present sin salir, 0xF00D -- el tope nuevo del
      banco, para que un bucle de juego no lo cuelgue). Saboteado: sin la
      negacion de SM5, el fotograma 0 no cuadra. Antes: [ARRANCA en el
      banco 28-09, hasta D3DCompile] `prueba/bmox12.exe` es estudio_d3d12
      compilado por el propietario en su Windows (rustc 1.97.1, `std` de Rust
      y CRT de MSVC; ver HACER.txt). En la casa le faltaban 9 nombres: el
      arranque del CRT estatico (InitializeSListHead, IsDebuggerPresent,
      IsProcessorFeaturePresent -- SSE, SSE2 y NX si, FASTFAIL no --,
      UnhandledExceptionFilter y _seh_filter_exe) y las excepciones de C++
      por las que va el panic de Rust (_CxxThrowException,
      __CxxFrameHandler3, __current_exception(_context)): esas cuatro, hoy,
      dicen cual y terminan con 0xE06D7363 como en Windows; un programa que
      no entra en panico no las llama. **Como se sabe:** en el banco arranca
      entero -- imprime "GPU: PROTON-X (la CPU de BMO-X)", el tearing y sus
      cifras de memoria -- y llega a D3DCompile: su HLSL va con CRLF (huella
      d7e2992c), no hay .cso, la casa deja el pedido y el programa sale con
      su error (1) sin romperse. Quitar un nombre de la casa: no carga.
      Falta: los .cso de d7e2992c y b50c1000 (sombras.exe en Windows, una
      vez), el cubo entero en el banco, y el Ryzen. Lo de C++ (el panic)
      va aparte, con su banco. Pide P4 (el
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
   P3c3  [HECHO en el banco 28-09] el bytecode de SM5 (DXBC con SHEX): la
         casa ejecutaba DXIL (SM6, el de dxc); SM5 es OTRO lenguaje de
         maquina virtual (registros r#, v#, o#, cb#[]). `bmo_proton_x::sm5`
         lo TRADUCE al mismo `Programa` escalar que sale del DXIL (sin otro
         interprete): el lote, la trama y `nativo` no cambian, y
         `dxil::leer` acepta SHEX/SHDR, asi que CreateGraphicsPipelineState
         de la casa lo toma sin tocarla. Sabe add mul mad div dp2/3/4 rsq
         sqrt min max mov ret, _sat y -x |x| -|x|, sobre r# v# o# l() y
         cb0[n]; lo demas (saltos, texturas, otro cbuffer, indices
         relativos, x#) se DICE con su numero. **Como se sabe:** los dos
         .cso que `sombras.exe` compilo en el Windows del propietario
         (`prueba/sombras/f3ef42a0.cso` y `4d67f5e4.cso`, commit 20ac1b4),
         corridos por el lote de la casa, dan las HUELLAS de la 3060 de los
         fotogramas 0, 30 y 60 bit a bit (`pruebas_sm5.rs`); el vertice da
         los bits del juez, el pixel su color a 8 bits, y traducidos a
         x86-64 (`tests/nativo.rs` de la casa) los bits del interprete en
         360 fotogramas. Saboteado: sin la negacion, sin el _sat o sin el
         swizzle, cae. Lo que no es D3D11, dicho: los subnormales no se
         llevan a cero (tampoco en el DXIL de la casa)
   P4c   [HECHO en el banco 28-09; falta el metal] AddVectoredException-
         Handler, RtlCaptureContext, RtlLookupFunctionEntry, RtlVirtualUnwind
         (y RaiseException, __C_specific_handler, RtlUnwindEx). Lo de C++ del
         CRT (__CxxFrameHandler3/4, _CxxThrowException) NO: sigue pendiente
```
      Lo que la tabla NO ve son los metodos COM (por vtabla). CRUZADOS el
      28-09: las llamadas de `estudio-d3d/d3d12/src/cubo_d3d12.rs` (EPICX,
      solo lectura) contra las vtablas que la casa arma en `com.rs`. De 45
      metodos, 35 estan; lo que faltaba es **P3c4** [HECHO en el banco
      28-09, salvo --fotograma]:

```text
   P3c4  DXGI     IDXGIFactory6 (el cubo la pide a CreateDXGIFactory2):
                  la vtabla de Factory2 + GetCreationFlags (3),
                  EnumAdapterByLuid, EnumWarpAdapter (4), CheckFeatureSupport
                  (5: PRESENT_ALLOW_TEARING) y EnumAdapterByGpuPreference (6);
                  un IDXGIAdapter1 con GetDesc1, que D3D12CreateDevice acepte;
                  IDXGISwapChain3 (el .cast() del cubo) con
                  GetCurrentBackBufferIndex
         device   CreateDepthStencilView, GetResourceAllocationInfo (solo
                  informa: sus cifras de la 3060 estan en el DICCIONARIO)
         lista    ClearDepthStencilView; y la PROFUNDIDAD de verdad en la
                  trama (D32, LESS, borrar a 1.0): el PSO del cubo la
                  enciende. Es la V2 de VERRANO en la CPU
         recurso  GetDesc
         --fotograma (la huella): GetCopyableFootprints,
                  CopyTextureRegion a un bufer READBACK  [HECHO 28-09]
```
      P3c4 **Como se sabe:** `prueba/cubo12.exe` (cubo.c por el camino de
      BMOX-12: Factory6, EnumAdapterByGpuPreference, GetDesc1, el
      dispositivo SOBRE el adaptador, CheckFeatureSupport, SwapChain3 y
      GetCurrentBackBufferIndex, D3DCompile del HLSL de BMOX-12 con sus .cso,
      y la profundidad D32 con LESS) da en el banco las huellas de la 3060 de
      los fotogramas 0, 30 y 60, sin un aviso (`tests/corre.rs`). La trama
      (`trama::Profundidad`, z lineal en pantalla, prueba antes del
      sombreador) lo prueba sola: el cubo SIN descarte y con profundidad da
      la imagen de la 3060 salvo empates de z en la silueta (como mucho 2
      pixeles por fotograma, contados). El float de ClearDepthStencilView
      llega en xmm3 y la casa es soft-float: un trampolin de dos
      instrucciones lo pasa a r9 (el guardian PX7 ya sabe que `sym f` es
      darla). Saboteado: borrar la profundidad a 0 no pinta nada; un indice
      de back buffer malo sale con 0xE230. Lo que no, dicho: el adaptador
      dice SOFTWARE (dibuja la CPU); GetResourceAllocationInfo da lo que
      reserva la casa, no un driver; el stencil se avisa y no se usa.
      --fotograma [HECHO 28-09]: GetCopyableFootprints (filas a 256, el total
      sin el relleno de la ultima, como D3D12) y CopyTextureRegion (un render
      target entero a un bufer; lo demas se dice). cubo12.exe lee cada
      fotograma por READBACK, saca la huella y la compara con la de la 3060:
      en el banco las tres cuadran; un bit cambiado en la copia da las tres
      malas (sale con 0x303).
      VISTO en el Windows del propietario (28-09, la 3060): el cubo gira
      con sus caras de colores, el HLSL compilado por su d3dcompiler. Falta:
      en el Ryzen.
       P3c1 **Como se sabe:** `prueba/peek.exe` dice
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
      P4c [HECHO en el banco 28-09, rama p4c-seh; falta el metal] LAS
           EXCEPCIONES (SEH). Lo que se DECIDE es puro y tiene su banco:
           `bmo_proton_x::desenrollar` (un marco, como RtlVirtualUnwind) y
           `bmo_proton_x::seh` (las cabeceras de una imagen ya colocada y su
           .pdata; subir un marco con su RUNTIME_FUNCTION o por la regla de
           la hoja; la SCOPE_TABLE de __C_specific_handler y sus dos
           pasadas -- que __except mirar, que __finally correr y donde
           parar --; EXCEPTION_RECORD y DISPATCHER_CONTEXT; los
           vectorizados). La casa (`excepciones.rs`) pone lo que no se puede
           decir sin la maquina: la foto de los registros y el salto a un
           CONTEXT, en ensamblador (es soft-float y Rust no nombra rbx ni
           rbp), y las llamadas al `.exe`. Da RaiseException,
           RtlCaptureContext, RtlLookupFunctionEntry, RtlVirtualUnwind,
           RtlUnwindEx, SetUnhandledExceptionFilter y Add/Remove-
           VectoredExceptionHandler (kernel32 y los Rtl* de ntdll), y
           __C_specific_handler (ntdll y vcruntime140). Sin nadie que la
           coja: el filtro de las no manejadas, o el proceso acaba con el
           codigo de la excepcion. **Como se sabe:** `prueba/seh.exe` dice
           `bien` nueve veces en Windows y sale con 0 (28-09). Con la casa:
           en un arnes provisional de Windows (fuera del repo) ocho de nueve
           y sale con 0 -- la del hilo no se puede correr alli: el relevo de
           hilos de la casa es de System V --, y sin el rax del destino o sin
           correr el __finally, cae (comprobado); `tests/corre.rs` lo corre
           entero en el banco de Linux: FALTA verlo alli y en el Ryzen
           (`run sys/proton-x.bex window/seh.exe`). `pruebas_seh.rs`:
           dieciocho pruebas, con UNWIND_INFO escritas a mano y el .pdata de
           verdad de seh.exe (siete __except y un __finally). Lo que NO hace,
           dicho: solo excepciones de SOFTWARE -- un fallo de pagina o una
           division por cero del `.exe` no llegan a Ring 3, eso es del
           kernel --; un marco de la casa corta la pila (una excepcion en una
           WndProc o dentro de un filtro queda sin manejar); ni anidadas ni
           desenrollados que chocan (se dicen y se sigue); ni el desenrollado
           de salida (RtlUnwindEx sin marco); ni las de C++;
           ExceptionAddress es la vuelta de RaiseException; una excepcion
           dentro de un EPILOGO no se detecta (con RaiseException no pasa).
           La huella de seh.exe es la de clang 23 (HACER.txt): la del
           encargo, de clang 18, no sale con el.
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
   P4c   las excepciones     AddVectoredExceptionHandler, RtlCaptureContext,  [HECHO*]
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
           4 de P4c (hechos el 28-09) y los 14 del CRT de MSVC (P4f5). `C:\Windows` es un
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
           _CxxThrowException) no lo trae P4c (28-09: solo las de C) y sigue
           pendiente; `fopen` y los FILE de ficheros
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
   3  P4c, el despachador    [HECHO en el banco 28-09, rama p4c-seh] SEH:
                             RaiseException, __C_specific_handler,
                             RtlUnwindEx. Falta: tests/corre.rs en Linux y
                             window/seh.exe en el Ryzen (nueve bien)
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

**LO QUE FALTA PARA JUGAR, en orden (28-09).** El propietario: *"VE HASTA EL
FINAL QUE PUEDA JUGAR"*. Escrito entero, para que nadie choque:

```text
   AHORA, en su Windows (el propietario, minutos):
   a  correr hilos.exe, stdio.exe, peek.exe, compila.exe (arreglados/nuevos)
   b  obrero\sombras.exe platform\shared\proton-x\prueba\sombras
      -> dos .cso: el SM5 de VERDAD del cubo de BMOX-12, que P3c3 necesita

   LUEGO, aqui:
   1  P3c3  [HECHO en el banco 28-09] el SM5 de FXC -> el mismo lote: los
            .cso de (b) dan las huellas de la 3060
   2  P4c   las excepciones (SEH): [HECHO en el banco 28-09] en la rama
            p4c-seh, de OTRA sesion; se revisa aqui antes de juntarlo
   3  P3c   BMOX-12 en el Ryzen, dibujado por la CPU (P3b4 despues)
   4  VERRANO V2..V4 y P3b4: la 3060 dibuja el lote (E2 pide las palabras
            de `ptxas`/`nvdisasm` de su Windows)
            -- con EL REGISTRO de PLAN_VERRANO (2c): fps, tiempo por
            fotograma y por parte, y los avisos, a consola y a fichero
   5  P5    UN JUEGO CHICO. Lo que un juego pide y un cubo no, medido de
            antemano con `rayosx` (Ludoteca 9):
            5a  cargar SUS DLL. [BANCO HECHO 28-09] `bmo_proton_x::dll`
                (las exportaciones: nombre, ordinal, reenvios), la casa
                registra las DLL propias (su base es su HMODULE, GetProcAddress
                por nombre y ordinal, DllMain una vez antes de la entrada) y el
                banco las carga junto al .exe; usadll.exe + saludo.dll dicen
                `bien` ocho veces (sin DllMain, MAL). FALTA EL METAL: el kernel
                SELLA BLOQUES ENTEROS y da ocho por proceso -- cada imagen pide
                su codigo sellado y sus datos RW, asi que un .exe y dos DLL ya
                los agotan. Pide al kernel sellar un TRAMO de paginas dentro de
                un bloque (MEM_OP_SELLAR con desplazamiento y medida): un bloque
                grande para todas las imagenes, cada codigo sellado en su
                tramo. Es Ring 0 y W^X: se decide con el propietario
            5b  D3D9/D3D11 (la mayoria de los chicos de GOG no son D3D12):
                otra traduccion al MISMO lote
            5c  el sonido (XAudio2 / DirectSound) y el mando (XInput)
            5d  borrar, renombrar y crear carpetas en el FAT32 desde Ring 3
                (las partidas guardadas): codigo del KERNEL
   6  P6, F los rayos (DXR) y Cyberpunk (D: y un volumen que BMO-X lea con
            nombres largos y ficheros de mas de 4 GiB)
```

Lo que YA hay para un juego de verdad (27/28-09): hilos, TLS, esperas,
ficheros y carpetas, memoria (monton y VirtualAlloc), texto y consola, el CRT
de MSVC con su printf, la `std` de Rust para Windows con sus excepciones de C (P4c; las de C++
no), D3D12 y DXGI
por la CPU, D3DCompile pagando una vez, y el guardian `proton-x`.

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
**REPETIDA el 28-09 en D:** (`D:\Cyberpunk 2077\bin\x64\Cyberpunk2077.exe`):
DENTRO (PE32+ x86-64, codigo maquina), 663 funciones de 36 bibliotecas --
la misma cifra --. De Windows: KERNEL32 209, USER32 66, WS2_32 33, ole32 12,
HID 11, SETUPAPI 6, ADVAPI32 4, WINMM 4, OLEAUT32/SHELL32/VERSION 3, ntdll
2, GDI32/SHLWAPI/POWRPROF/dbghelp/XINPUT9_1_0 1. Del JUEGO (van en su
carpeta, las carga P5a, y cada una pide lo suyo de Windows: medirlas con
rayosx una a una): icuuc 89, PhysX3 42, icuin 33, bink2w64 21,
sl.interposer 19 (Streamline de NVIDIA), PxFoundation 16, libcurl 15,
libxess_fg 11, REDGalaxy64 10 (la tienda: GOG sin DRM), libxess 9,
PhysX3Common 8, amd_ags 6, oo2ext 6, libxell 6, ffx_fsr3 y
ffx_backend_dx12 4, redlexer_native 1. Graficos: DXGI, D3D12 (y
D3D12SerializeVersionedRootSignature, D3D12GetDebugInterface) y
D3D11CreateDeviceAndSwapChain. Y abre mas en marcha (LoadLibrary*,
GetProcAddress): la tabla no es todo lo que pide.
**Las 59 de "graficos" NO son dibujar (28-09):** rayosx cuenta asi a
sl.interposer (Streamline, 19), libxess + libxess_fg + libxell (XeSS, 26),
ffx_fsr3 + ffx_backend_dx12 (FSR 3, 8) y amd_ags (6): reescaladores y extras
de fabricante, DLL del propio juego (P5a las carga; con el reescalado
APAGADO casi no trabajan; AGS pregunta si la tarjeta es AMD y la 3060 dice
que no). El propietario juega NATIVO (sin reescalar): se apagan en las
opciones. Lo grueso de los graficos no esta en ninguna tabla: los metodos
COM de D3D12 (BMOX-12 uso 45; los de Cyberpunk se MIDEN corriendolo, la casa
dice cada hueco con su nombre), sus sombreadores DXIL en la 3060 (VERRANO
E3..E5, P3b4) y DXR (P6, que tambien se apaga).

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
