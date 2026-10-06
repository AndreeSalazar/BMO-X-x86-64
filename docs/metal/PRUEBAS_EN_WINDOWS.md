# LAS PRUEBAS EN WINDOWS -- la lista larga (06-10)

El propietario: *"dime en lista larga para probar en Windows porque BMO-X se
refleja bien en PROTON-X en Windows"*. Cada juez de
`platform/shared/proton-x/prueba/` se hizo en la nube, y lo que pide (cuantos
`bien`, que bits) se escribio a mano o se saco de las reglas de D3D12. El
banco solo prueba que PROTON-X dice lo mismo que el juez. **Que el juez tenga
razon lo dice Windows**, y solo tu Windows puede decirlo. Por eso esta lista
va ANTES que la del Ryzen (`PRUEBAS_DX12_EN_EL_RYZEN.md`):

    Windows dice lo que pide el juez   ->  el juez es bueno
    el banco dice lo que pide el juez  ->  PROTON-X es Windows (en Linux)
    el Ryzen dice lo que pide el juez  ->  BMO-X es Windows (en el metal)

Si Windows dice otra cosa, el que esta mal es el juez (y con el, lo que el
banco creia probado): esa linea `MAL` es lo mas valioso que puedes mandar.

## 0. Lo que hace falta

- La carpeta `platform/shared/proton-x/prueba/` tal cual sale del repo (los
  `.exe`, `saludo.dll` y `tanda22d.dll` juntos). En `C:`, no en `D:`.
- Windows 10 u 11 de 64 bits, con la 3060 y su driver. Ni Visual Studio ni
  el SDK. Los de MSVC (`tanda4m`, `tanda19m`) piden el runtime de Visual C++
  2015-2022 (`vcruntime140.dll`, `msvcp140.dll`): lo tiene cualquier PC con
  juegos; si falta, salen con `0xC0000135` y el guion lo dice.

## 1. Los 90 de consola: UN guion

En una PowerShell normal, dentro de la carpeta:

    powershell -ExecutionPolicy Bypass -File .\correr_en_windows.ps1

Corre cada uno con un tope de 60 s, cuenta sus `bien`, `MAL` y `nota`, mira
con que sale, y escribe una linea por juez: `bien` si dice lo que pide la
tabla, `DISTINTO` (y por que) si no. Al final: cuantos de cada. Lo que hay que
mandar: `informe_windows\resumen.txt` (las lineas MAL van enteras dentro), o
la carpeta `informe_windows\` entera si algo sale DISTINTO.

Lo que pide cada uno. Desde el 06-10 la tabla es UN fichero,
`prueba/jueces.txt`: la lee este guion y la lleva dentro `sys/jueces.bex`,
el que corre los mismos jueces SOLOS en BMO-X (`run sys/jueces.bex`). Lo
que Windows dio por bueno, alli se pide igual.

### 1a. LOS PRIMEROS: los 14 de D3D12, NINGUNO corrido aun en Windows

Son los que mas cuentan: todo lo de DX12 del 05-10 y el 06-10 se dio por
bueno contra ellos.

| .exe | que juzga | `bien` |
|---|---|---|
| `computo` | memoria compartida y barrera de un CS (E2.3a) | 4 |
| `instancias` | instancias, tres ranuras de vertices, dibujar sin bufer (N5.13) | 3 |
| `vistas` | vistas en la raiz, UAV de textura, RWBuffer con tipo, ClearUAV (N5.3b/c) | 7 |
| `hdr` | render targets RGBA16F y R11G11B10F, leidos como textura (N5.16) | 4 |
| `uavpixel` | UAV escritos desde un pixel, InterlockedAdd, RWBuffer desde el VS (N5.3d) | 4 |
| `flotante1` | R32F y R16F de destino, UAV de RGBA16F, DepthClipEnable = FALSE (N5.16b) | 10 |
| `stencil` | REPLACE/EQUAL, INCR_SAT, mascaras, las dos caras (N5.12b) | 5 |
| `olas` | Wave* y Quad* de 32 carriles (E2.5); el "hasta N" de B puede salir otro; D y E, SV_VertexID con un vertice base (06-10) | 17 |
| `derivadas` | ddx/ddy finas y gruesas, la mip de cada muestreo, CalculateLevelOfDetail (D4.4) | 17 |
| `restos` | destinos de enteros, UAV sin destino y desde el GS, el plano de stencil, SV_StencilRef | 18 |
| `multihilo` | listas de cuatro hilos, colas que esperan a una valla, Reset/Close (E2.1) | 17 |
| `volumen` | UAV de texturas 3D y de arrays, ClearUAV de rebanadas | 10 |
| `firmas` | root signatures 1.1, la de dentro del sombreador, DESC1 serializada | 6 |
| `postpro` | un posproceso por computo: bindless, UAV creado sin descripcion | 3 |
| `tipos` | (06-10) texturas TYPELESS vistas con OTRO formato: UAV, SRV y render targets (D2.7) | 8 |
| `limpieza` | (06-10) ClearUnorderedAccessView en el formato de la vista y con rectangulos (A2) | 8 |
| `escena` | (06-10) una escena 3D DURA contra la imagen de Windows (A11): ver abajo | 3 (o 2 y una nota sin `escena.ref`) |
| `leefirma` | (06-10) los deserializadores de root signatures, y la 1.1 serializada con los bytes de Windows (A3, A4) | 10 |
| `capas1d` | (06-10) los UAV de arrays de UNA dimension (`RWTexture1DArray`): escritos, leidos, contados, sus medidas y su ClearUnorderedAccessView (A5) | 9 |
| `adyacencia` | (06-10) las topologias con ADYACENCIA: que vertices le llegan a un GS `triangleadj` o `lineadj` en listas y tiras, y que se pinta sin GS (A6) | 7 |
| `mapeo` | (06-10) `Map` sobre una TEXTURA de un monton de la CPU (CUSTOM), con WriteToSubresource y ReadFromSubresource; la nota dice el HRESULT de Windows al Map CON puntero (A7) | 7 y una nota |

Si alguno se CUELGA (el guion lo dice), en `multihilo` es su parte B: una
cola que espera y nadie la despierta. Si `restos` dice una `nota` en E, la
3060 no tiene SV_StencilRef: no es fallo (la casa dice que si lo tiene, y se
anota la diferencia).

### 1b. PROTON-X por dentro (P1 a P5, 27-09 y 28-09)

| .exe | que juzga | `bien` |
|---|---|---|
| `hola` | el primer .exe: una frase y sale con 0 | (ninguno) |
| `teb` | gs: como lo lee el CRT de Microsoft | 6 |
| `hilos` | hilos, TLS, `__declspec(thread)` y sus callbacks | 19 |
| `ficheros` | crear, escribir, leer, mover (deja `pxtest.txt`) | 16 |
| `crt` | lo que pide un CRT de Windows al arrancar | 35 |
| `texto` | UTF-16, la consola, WriteConsoleW, los modulos | 21 |
| `esperas` | mutex, temporizadores, WaitOnAddress, FLS (unos 150 ms) | 24 |
| `carpetas` | carpetas y atributos (deja `pqa.txt`, `pqb.txt`, `pzc.txt`) | 34 |
| `sistema` | ntdll, el azar, la red sin cable, procesos (sale por TerminateProcess) | 22 |
| `ucrt` | el CRT de MSVC desde sus DLL, `_crt_atexit` | 19 |
| `stdio` | printf, puts, fputs a stderr | 15 |
| `peek` | PeekMessageW, AdjustWindowRect y lo chico de BMOX-12 | 12 |
| `compila` | D3DCompile de verdad (d3dcompiler_47 de Windows) | (sin cuenta: ningun MAL, sale con 0) |
| `usadll` | una DLL propia con ordinales (con `saludo.dll` al lado) | 8 |
| `seh` | excepciones estructuradas x64, la ultima desde el filtro | 9 |
| `vueltas` | el hilo que espera dando vueltas (02-10) | 4 |

### 1c. Las TANDAS de Cyberpunk (29-09 a 02-10)

Algunas ya las corriste (4m, 13, 19m, 22m, 46, 47, 48): correrlas otra vez
dice si Windows sigue igual tras sus parches.

| .exe | `bien` | | .exe | `bien` | | .exe | `bien` |
|---|---|---|---|---|---|---|---|
| `tanda1` | 55 | | `tanda14` | 27 | | `tanda29` | 18 |
| `tanda2` | 13 | | `tanda14b` | 12 | | `tanda30` | 14 |
| `tanda3` | 25 | | `tanda15` | 11 | | `tanda31` | 30 |
| `tanda3b` | 18 | | `tanda16` | 9 | | `tanda32` | 10 |
| `tanda3c` | 16 | | `tanda17` | 11 | | `tanda33` | 8 |
| `tanda4` | 11 | | `tanda18` | 16 | | `tanda34` | 9 |
| `tanda4m` | 11 | | `tanda19` | 19 | | `tanda35` | 8 |
| `tanda5` | 22 | | `tanda19m` | 15 | | `tanda36` | 6 |
| `tanda6` | 29 | | `tanda20` | 15 | | `tanda37` | 9 |
| `tanda7` | 29 | | `tanda21` | 17 | | `tanda38` | 13 |
| `tanda8` | 25 | | `tanda22` | 9 (*) | | `tanda39` | 8 |
| `tanda9` | 30 | | `tanda23` | 16 | | `tanda41` | 10 |
| `tanda10` | 25 | | `tanda24` | 12 | | `tanda42` | 13 |
| `tanda11` | 35 | | `tanda25` | 9 | | `tanda43` | 12 |
| `tanda12` | 24 | | `tanda26` | 6 | | `tanda44` | 10 |
| `tanda13` | 16 | | `tanda27` | 8 | | `tanda45` | 15 |
| `diario` | 4 | | `tanda28` | 6 | | `tanda46` | 9 y 1 `nota` |
| | | | | | | `tanda47` | 21 |
| | | | | | | `tanda48` | 14 |

(*) `tanda22` es una PREGUNTA ABIERTA (HACER.txt, 30-09): Windows ignoraba el
TLS de la DLL de clang. Si ahora dice 9, la hipotesis (le faltaba importar
algo de kernel32) era cierta; si dice 7 MAL, no, y queda como prueba solo del
anfitrion. Cualquiera de las dos respuestas sirve.

### 1d. La escena 3D DURA: Windows hace la referencia (06-10)

`escena.exe` no se juzga bit a bit (la GPU y la CPU no dan los mismos bits
en los floats): se compara con la imagen que deja TU Windows. No hay que
hacer nada aparte: si no esta, `correr_en_windows.ps1` la hace la primera
vez (`escena.exe guardar`) y lo dice en amarillo al final. A mano, en la
carpeta `prueba\`, seria:

    .\escena.exe guardar

Deja `escena.ref` (la referencia) y `escena.bmp` (para mirarla: un
terreno a cuadros con 64 cubos, sus sombras y un vidrio azul). Manda
`escena.ref`: entra al repo, y desde ahi el banco y BMO-X (`run
sys/jueces.bex`) se comparan con ella. Sin `escena.ref`, su C es una nota.

## 2. Los 15 de ventana: A OJO

No dicen `bien`: se miran. Una foto si algo se ve distinto. Las muestras de
Microsoft leen sus `.cso` de su carpeta: copia el `.exe` dentro de
`muestras\<nombre>\` y lanzalo desde ahi (`hwindow` no lleva ninguno).

| .exe | donde | que tiene que verse |
|---|---|---|
| `ventana` | `prueba\` | 320x200 con un degradado y un marco dorado; una letra cambia el tinte, un clic deja un cuadrado blanco, q o ESC cierran |
| `limpia` | `prueba\` | la ventana limpiada por D3D12 y DXGI |
| `cubo` | `prueba\` | 1280x720 con el cubo de X1 girando |
| `cubo12` | `prueba\` | el mismo cubo por el camino de BMOX-12 (compila su HLSL); con `--fotograma` compara cada fotograma con las huellas de la 3060 |
| `bmox12` | `prueba\` | el BMOX-12 de EPICX sin tocar: el cubo en la 3060 |
| `hwindow` | `prueba\` | 1280x720 entera AZUL `{0, 0.2, 0.4}` |
| `htriang` | `muestras\htriang\` | el triangulo de colores sobre el azul |
| `hframes` | `muestras\hframes\` | el MISMO triangulo |
| `hbundles` | `muestras\hbundles\` | el MISMO triangulo |
| `hcbuffer` | `muestras\hcbuffer\` | el triangulo moviendose de izquierda a derecha |
| `htexture` | `muestras\htexture\` | el triangulo con un tablero de damas, pixeles duros |
| `dynindex` | `muestras\dynindex\` | una ciudad de 15 x 8 copias, cada una con su material |
| `nbody` | `muestras\nbody\` | dos nubes de particulas que se juntan |
| `indirect` | `muestras\indirect\` | 1024 triangulos entrando por la izquierda; en la franja central, lo mismo con culling y con ESPACIO (sin culling) |
| `predica` | `muestras\predica\` | el cuadro blanco lejano NO se dibuja el fotograma despues de que el translucido lo tapara entero |

Esto es lo que BMO-X tiene que ver igual en el Ryzen: si en Windows la
ciudad o las particulas salen distintas a como se describen aqui, la
descripcion esta mal y la del Ryzen tambien.

## 3. Las 3 medidas (la pila B del contador, PLAN_LAS_TRES_GRANDES 7.1)

No son jueces: dicen si a la pila A le FALTAN cosas. Sobre una COPIA del
juego en `C:`, nunca sobre `D:`:

- **D0.1** -- `rayosx` sobre la carpeta del juego: las interfaces y metodos
  de D3D12/DXGI que se importan o se piden por IID.
- **D0.2** -- los DXIL de la cache de sombreadores del juego: sus opcodes,
  su modelo (6.x), sus recursos. Lo que los traductores no sepan entra en A.
- **D0.3** -- el `diario` de P0.3 con el juego, filtrado a D3D12: que
  metodos, cuantas veces, en que orden hasta el primer `Present`.

## 3b. Lo que dijo Windows la PRIMERA vez (06-10, la 3060 del propietario)

78 de 83 como pide la tabla. Los 5 distintos:

- `tanda2`: 13 `bien` y pedia 14. La TABLA estaba mal (el texto de
  HACER.txt dice catorce; el juez tiene trece `mira` y el banco pide 13).
  Arreglada.
- `tanda3`: 1 MAL, `WriteConsoleA`. El guion manda la salida a un fichero, y
  ahi WriteConsoleA falla en Windows. El juez ya solo lo juzga si la salida
  es una consola (como texto.exe): `tanda3.exe` rehecho.
- `restos`: 15 `bien` y una `nota`: la 3060 dice
  PSSpecifiedStencilRefSupported = FALSE (las NVIDIA no tienen
  SV_StencilRef), y E se salta. No es fallo; el guion acepta 18, o 15 con
  una nota. Lo que si dice: E (SV_StencilRef) solo lo juzga el banco.
- `vistas`: 3 MAL, los tres por lo mismo: 0.5 en UNORM de 8 bits es
  127.5, un empate, y la 3060 da 127 donde la casa da 128 (todo lo demas,
  bit a bit igual). Del JUEZ: acepta los dos en ese canal; y sus copias van
  ya a sitios de 512 bytes.
- `olas`: 1 MAL, y este era de PROTON-X: **SV_VertexID no cuenta el
  StartVertexLocation**. Un DrawInstanced(3, 1, 6, 0) en Windows lee los
  vertices 0..2 (media pantalla) y la casa leia 6..8 (un pixel). Arreglado
  (`lote::Lote::base_vertice`); el juez suma D (eso) y E (lo mismo con
  BaseVertexLocation en un dibujo con indices: lo PREGUNTA). Pide 17.

**La SEGUNDA corrida (06-10, 08:49): 83 de 83.** `olas` 17 `bien`: E
confirma que en un dibujo con indices el BaseVertexLocation TAMPOCO cuenta
en SV_VertexID (lo que hace la casa desde hoy). `restos`, 15 y su nota.
Desde aqui, cada juez de esta carpeta es Windows: lo que diga distinto
BMO-X en el Ryzen, es de BMO-X.

**La TERCERA corrida (06-10, 09:55): 84 de 84**, con `tipos.exe` (D2.7)
en 8 `bien` a la primera.

**La CUARTA corrida (06-10, 12:22): 85 de 86.** `escena` hizo su
`escena.ref` y dijo 3 `bien`. `limpieza`, MAL en D y G: un
ClearUnorderedAccessViewUint de ENTEROS satura (0x10001 en 16 bits es
0xFFFF), la casa se quedaba con los bits bajos; arreglado, y con el un NaN
de half que la casa no conservaba.

## 4. Que mandar, en orden

1. `informe_windows\resumen.txt` (seccion 1). Si todo dice `bien`: con esa
   linea basta.
2. Las fotos de lo que en la seccion 2 se vea distinto.
3. Cuando puedas, D0.1 a D0.3.

Con 1 y 2 en la mano, la hoja del Ryzen (`PRUEBAS_DX12_EN_EL_RYZEN.md`)
cuenta de verdad.
