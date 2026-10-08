# D3D12 MAESTRO -- el ABI de DirectX 12 en la casa, medido y no recordado

> Escrito el **2026-10-03**, con Cyberpunk 2077 vivo hasta los 47 s en el
> metal: ventana, sonido, 1065 PSO, y todavia sin un fotograma. El
> propietario pidio "estudiar todo DX12 como se hizo para construir
> PROTON-X, que tenga ABI maestro". Este documento es eso, con una regla:
> **la tabla de la seccion 2 la escribe una prueba, no la memoria.**

---

# 1. LAS TRES CAPAS DE "SABER D3D12"

Un juego D3D12 le pide a la casa tres cosas distintas, y fallar en cada una
se ve distinto:

```text
   capa            que es                              si falta
   ABI             cada hueco de cada vtabla COM       el .exe salta a una trampa
                   (ID3D12Device, la lista, DXGI...)   y sale (0xC0DE....)
   semantica       lo que ese hueco HACE de verdad     el dibujo sale a medias o
                   (un Draw con instancias, un         no sale, y lo DICE
                   Dispatch, ExecuteIndirect)          (`aviso`, una vez)
   sombreadores    el DXIL/SM5 que el PSO trae         el PSO existe pero cada
                   (las operaciones de D3D)            Draw con el se salta
```

La capa ABI esta CERRADA: ningun hueco de las 28 interfaces que la casa da
sale por `falta` (seccion 2). Lo que queda esta en las otras dos (secciones
3 y 4), y lo dice cada corrida del metal.

---

# 2. EL CENSO DEL ABI (lo escribe `proton-x-casa/tests/abi.rs`)

La prueba crea un objeto de cada interfaz por las puertas de Windows, como
un `.exe` (D3D12CreateDevice, CreateCommandQueue... CreateDXGIFactory2,
EnumAdapters1, EnumOutputs; CoCreateInstance del MMDeviceEnumerator,
Activate, GetService), y de cada vtabla dice que es cada hueco:

```text
   hace               la casa tiene codigo para el (ver la seccion 3: no
                      siempre es TODO lo de Windows)
   falla documentada  el HRESULT de Windows en una maquina sin esa funcion,
                      con el puntero de salida a NULL, y dicho (`fallas.rs`):
                      rayos, malla, tiles, recursos compartidos o protegidos
   falta              un `falta`: dice cual y SALE
```

Para regenerarla (y la prueba falla si la linea TOTAL de aqui no es la de
hoy):

```text
   cargo test -p bmo-proton-x-casa --test abi -- --nocapture
```

La cadena de intercambio (`IDXGISwapChain3`) no entra en el censo: pide una
ventana de verdad. La prueban `tests/corre.rs` (cubo12.exe) y el metal.

#### ID3D12Device -- 79 metodos: 64 hace, 15 falla documentada, 0 falta

- falla documentada: 30 CreateReservedResource, 31 CreateSharedHandle, 32 OpenSharedHandle, 33 OpenSharedHandleByName, 49 OpenExistingHeapFromFileMapping, 52 CreateProtectedResourceSession, 55 CreateReservedResource1, 57 CreateLifetimeTracker, 60 EnumerateMetaCommandParameters, 61 CreateMetaCommand, 62 CreateStateObject, 66 AddToStateObject, 67 CreateProtectedResourceSession1, 71 CreateSamplerFeedbackUnorderedAccessView, 78 CreateReservedResource2

#### ID3D12CommandQueue -- 19 metodos: 17 hace, 2 falla documentada, 0 falta

- falla documentada: 8 UpdateTileMappings, 9 CopyTileMappings

#### ID3D12CommandAllocator -- 9 metodos: 9 hace, 0 falla documentada, 0 falta


#### ID3D12GraphicsCommandList -- 86 metodos: 75 hace, 11 falla documentada, 0 falta

- falla documentada: 18 CopyTiles, 70 InitializeMetaCommand, 71 ExecuteMetaCommand, 72 BuildRaytracingAccelerationStructure, 73 EmitRaytracingAccelerationStructurePostbuildInfo, 74 CopyRaytracingAccelerationStructure, 75 SetPipelineState1, 76 DispatchRays, 79 DispatchMesh, 84 SetProgram, 85 DispatchGraph

#### ID3D12DescriptorHeap -- 11 metodos: 11 hace, 0 falla documentada, 0 falta


#### ID3D12Resource -- 17 metodos: 16 hace, 1 falla documentada, 0 falta

- falla documentada: 15 GetProtectedResourceSession

#### ID3D12Fence -- 12 metodos: 12 hace, 0 falla documentada, 0 falta


#### IDXGIFactory6 -- 32 metodos: 31 hace, 1 falla documentada, 0 falta

- falla documentada: 17 GetSharedResourceAdapterLuid

#### ID3D12RootSignature -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


#### ID3D12PipelineState -- 9 metodos: 9 hace, 0 falla documentada, 0 falta


#### ID3DBlob -- 5 metodos: 5 hace, 0 falla documentada, 0 falta


#### IDXGIAdapter4 -- 19 metodos: 19 hace, 0 falla documentada, 0 falta


#### IDXGIOutput6 -- 29 metodos: 23 hace, 6 falla documentada, 0 falta

- falla documentada: 13 GetGammaControlCapabilities, 14 SetGammaControl, 15 GetGammaControl, 16 SetDisplaySurface, 17 GetDisplaySurfaceData, 21 GetDisplaySurfaceData1

#### ID3D12Heap -- 10 metodos: 9 hace, 1 falla documentada, 0 falta

- falla documentada: 9 GetProtectedResourceSession

#### ID3D12QueryHeap -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


#### ID3D12CommandSignature -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


#### IMMDeviceEnumerator -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


#### IMMDeviceCollection -- 5 metodos: 5 hace, 0 falla documentada, 0 falta


#### IMMDevice -- 7 metodos: 7 hace, 0 falla documentada, 0 falta


#### IMMEndpoint -- 4 metodos: 4 hace, 0 falla documentada, 0 falta


#### IPropertyStore -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


#### IAudioClient3 -- 21 metodos: 21 hace, 0 falla documentada, 0 falta


#### IAudioRenderClient -- 5 metodos: 5 hace, 0 falla documentada, 0 falta


#### IAudioClock -- 6 metodos: 6 hace, 0 falla documentada, 0 falta


#### ISimpleAudioVolume -- 7 metodos: 7 hace, 0 falla documentada, 0 falta


#### IAudioSessionControl2 -- 17 metodos: 17 hace, 0 falla documentada, 0 falta


#### IAudioStreamVolume -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


#### IChannelAudioVolume -- 8 metodos: 8 hace, 0 falla documentada, 0 falta


TOTAL: 28 interfaces, 465 huecos: 428 hace, 37 falla documentada, 0 falta

---

# 3. LA SEMANTICA A MEDIAS (lo que "hace" y no es todo)

Cada fila es un `aviso` de la casa, con la casilla del plan
(`docs/plan/PLAN_LAS_TRES_GRANDES.md`). Ordenadas por lo que pesan en
Cyberpunk:

```text
   Dispatch (el COMPUTO): corre en la CPU desde el     N5.5  la luz, las sombras
   05-10 (E2.3a), con memoria compartida, barreras           y el post-proceso
   y UAV de bufer, y traducido a x86-64 (E2.3b);
   la 3060 aun no
   ExecuteIndirect: desde el 05-10 (E2.4), con el      N5.17 el culling de
   contador de los UAV; sin rayos ni malla                   la GPU
   las consultas de OCLUSION y SetPredication:         D5.5  el culling por
   desde el 05-10 (E2.7), contadas en la CPU; las            oclusion
   de estadisticas dan ceros
   el sombreador de GEOMETRIA: en la CPU desde el      N5.18 particulas,
   05-10 (E2.3b); sin vertice calculado, adyacencia,         siluetas
   puntos o lineas de salida ni stream output
   un UAV en el de PIXELES, y los de textura 3D o de   N5.3c
   array: el sombreador no los ve (en el computo, los
   de textura 2D, con tipo y en la raiz, desde el 05-10)
   render targets de float (N5.16 y N5.16b, 05-10):    N5.16b la 3060 en
   en float en la CPU; un R32_UINT/SINT de destino,          float
   todavia no
   stencil y AlphaToCoverage: se apuntan, no se usan   (nueva)
   topologias que no son triangulos (lineas, puntos)   (nueva) con un GS, ya
   sin un GS que las haga triangulos                        (E2.3b)
   Clear* con rectangulos: limpia el recurso entero
   SetEventOnMultipleFenceCompletion de varias vallas
   D3D12SerializeRootSignature: solo la 1.0;
   el deserializador no lee firmas
   CreateSwapChainForHwnd / ResizeBuffers: solo
   R8G8B8A8 y B8G8R8A8
```

Y lo que la casa hace en la CPU y la 3060 todavia no (N6.1): MRT, mezcla,
discard, SV_Position, SV_Depth, solo profundidad, la matematica, las olas,
los arrays, el Gather, el SampleCmp y contar los pixeles de una consulta de
oclusion (E2.7). Cada uno, cuando la puerta lo niega, va por la CPU (el
interprete), que es el juez de la 3060.

---

# 4. LOS SOMBREADORES (lo que el DXIL pide)

Lo que el metal fue pidiendo, corrida a corrida, y donde se hizo (todo con
un `.hlsl` de `dxc` en `proton-x/prueba/` y su prueba en el banco):

```text
   hecho      la matematica (sin, cos, exp2, log2, acos, atan, cosh...),
              los redondeos y los medios floats (`mates.rs`); discard;
              varios render targets; SV_Position; SV_Depth; las olas con
              un carril y las derivadas a 0 (`dxil/olas.rs`); los arrays
              locales y las tablas globales (`dxil/arreglos.rs`); los
              cbuffers con fila calculada; isnan/isinf/isfinite; las de
              bits (30 a 34); Gather, GatherCmp, SampleCmp y
              SampleCmpLevelZero (`dxil/sombras.rs`); el muestreador
              anisotropico (lineal) y el de comparacion
   falta      el indice dinamico de recursos (bindless, N5.4); los UAV y su
              escritura (N5.3c); el computo (N5.5); "un bucle con mas de
              una salida" (el estructurador); las derivadas de verdad (un
              cuadro de 2x2 en la trama)
```

> **08-10 (LB3b de [`PLAN_LAS_LIBRERIAS.md`](../plan/PLAN_LAS_LIBRERIAS.md)):**
> el interprete de la casa y lo que necesita (`mates`, `textura`, `bufer`,
> `formato_ia`, `bc`, lo que HACEN las olas) viven en PROMETEO,
> `platform/shared/prometeo/src`; PROTON-X los re-exporta en sus rutas de
> siempre, y los traductores de DXIL y de SM5 siguen aqui.

---

# 5. LA ESCALERA: QUE JUEGOS, Y POR QUE (03-10)

El propietario pregunto si DX9 seria un atajo, y por Left 4 Dead 2. La
respuesta, con lo que la casa es hoy:

```text
   DX9           no hay `d3d9.dll` en la casa, y casi todo lo DX9 es de 32
                 bits: PROTON-X carga solo PE32+ (64 bits). Dos proyectos
                 nuevos, no un atajo
   Left 4 Dead 2 32 bits, DX9 (Source) y el DRM de Steam (pide el cliente
                 corriendo): las tres cosas a la vez
   DX11          la casa no tiene `d3d11.dll`; pero sus sombreadores son
                 SM5, que la casa YA lee, y D3D11 se puede montar SOBRE el
                 D3D12 de la casa (como vkd3d al reves): el camino que abre
                 cientos de juegos de 64 bits
```

**Rehecha el 05-10: Cyberpunk deja de ser el escalon 2 y pasa a ser la
VARA DE MEDIR.** La escalera del 03-10 iba de "los DirectX-Graphics-Samples"
a Cyberpunk sin nada en medio, y decia que HelloTriangle y HelloTexture "ya
pasan". Medido en el arbol, no era asi: el triangulo es `cubo12.exe`
(nuestro), y de HelloTexture solo corre su CAMINO llamado a mano desde Rust
(`proton-x-casa/tests/textura.rs`). Ningun `.exe` de Microsoft ha corrido en
PROTON-X. Cada muro de Cyberpunk mezcla cinco capas (Win32, D3D12, DXIL, DLL
de otros, rendimiento) y su codigo es cerrado: un solo nulo (`ffxDispatch`)
costo diez corridas en el metal.

La escalera entera, con sus reglas, sus casillas y la de LAS_TRES_GRANDES
que prueba cada escalon, vive en
[`PLAN_LA_ESCALERA_PROTON_X.md`](../plan/PLAN_LA_ESCALERA_PROTON_X.md). En
corto, de menos a mas, UNA capa nueva por escalon y cada uno con la huella
de su fotograma en Windows como juez:

```text
   E0  lo que ya hay: la base Win32 (hola..seh, tandas 1 a 48), el ABI
       contado y el cubo (`cubo12.exe`) igual que en la 3060
   E1  los Hello de Microsoft, el .exe DE VERDAD (MIT): Window, Triangle,
       Texture, ConstBuffers, FrameBuffering, Bundles
   E2  una casilla abierta, una muestra: Multithreading (H2.7),
       DynamicIndexing (N5.4, el bindless), nBodyGravity (N5.5, el
       COMPUTO), ExecuteIndirect (N5.17), SM6WaveIntrinsics (las olas),
       HDR (N5.16), PredicationQueries (D5.5)
   E3  el jefe intermedio: el ModelViewer de MiniEngine (MIT): un Cyberpunk
       en miniatura con el codigo a la vista
   E4  un motor ABIERTO con D3D12, por elegir con `rayosx`: Godot 4, Wicked
       Engine o RBDOOM-3-BFG
   E5  un juego CERRADO mas chico que Cyberpunk, de los del propietario
   FINAL  Cyberpunk 2077 (GOG, DX12): el norte. Una corrida por escalon
       cerrado, para ver hasta donde llega ya; no una por muro
```

Lo que tiene el propietario, por medir (`rayosx` en Windows dice la API y
los bits antes de intentarlo), y donde cae:

```text
   The Witcher 3 (GOG, 64 bits)     DX11 y, desde la 4.0, DX12. Con DX12, el
                                    candidato de E5 (el mismo estudio)
   Baldur's Gate 3 (GOG, 64 bits)   Vulkan y DX11; sin DX12. Espera a D3D11
                                    sobre la casa (o a Vulkan)
   Alien: Isolation (Epic)          DX11; la tienda de Epic y su lanzador,
                                    por medir
```

La regla de la LUDOTECA sigue: GOG primero (sin DRM), y nada de terceros
dentro de la casa.

# 6. LOS QUE YA LO HICIERON: QUE LEER, PARA QUE CASILLA (03-10)

D3D12 no se aprende solo de la documentacion de Microsoft: otros ya lo
tradujeron a otra cosa, con codigo abierto, y cada traduccion muestra DONDE
esta lo dificil. Se LEE para entender; no se copia codigo (las licencias
mandan, y la casa es nuestra). La regla de siempre: lo que se aprenda de
ahi entra con su prueba en el banco, no de memoria.

```text
   fuente                         que es                       para que casilla
   DirectX-Specs (Microsoft)      la especificacion de D3D12:   todas: es la ley
                                  root signatures, barreras,    (el ESPEJO de la
                                  heaps, ExecuteIndirect...      semantica a medias)
   DXIL.rst y DXC (Microsoft)     el formato DXIL y cada        N5.x: cada
                                  OperacionD3d con su firma     OperacionD3d nueva
   vkd3d-proton (Valve)           D3D12 sobre Vulkan: lo que    N5.4 bindless,
                                  Proton usa para Cyberpunk     N5.17 ExecuteIndirect,
                                  en Linux. Lo mas cercano a    los heaps de
                                  la casa; tiene arreglos por   descriptores; y que
                                  juego (LGPL: leer, no copiar) hace Cyberpunk raro
   dxil-spirv (Valve)             DXIL a SPIR-V, el de          el estructurador
                                  vkd3d-proton: su              ("bucle con mas de
                                  estructurador de bucles       una salida"), olas,
                                                                bindless
   vkd3d / vkd3d-shader (Wine)    DXBC y DXIL leidos, HLSL      sm5.rs, dxbc.rs
   DXVK (D3D9/10/11 sobre Vulkan) lo de antes de DX12           D3D11 para Witcher 3
                                                                y Baldur's Gate 3
   Mesa: Dozen (dzn)              Vulkan SOBRE D3D12: el        que promete D3D12 de
                                  camino al reves; dice que     verdad (lo que un
                                  da D3D12 y que no             juego puede esperar)
   Mesa: el d3d12 de Gallium y    OpenGL sobre D3D12, y su      como se ESCRIBE un
   nir_to_dxil (Microsoft)        compilador NIR -> DXIL        DXIL valido (pruebas)
   Mesa: NVK y NAK                Vulkan para NVIDIA, Ampere    la 3060 (N6): las
                                  incluida; NAK emite SASS      palabras de SM86 de
                                  de SM86                       proton-x-sm86
   open-gpu-doc y                 las clases de la 3060         N6: los metodos de
   open-gpu-kernel-modules        (AMPERE_B, el QMD...)         la clase 3D
   (NVIDIA)
```

El orden en que sirven: para Cyberpunk HOY, vkd3d-proton y dxil-spirv (el
bindless, el estructurador y ExecuteIndirect son exactamente sus
problemas); para N6, NAK y open-gpu-doc; para la escalera DX11, DXVK.
