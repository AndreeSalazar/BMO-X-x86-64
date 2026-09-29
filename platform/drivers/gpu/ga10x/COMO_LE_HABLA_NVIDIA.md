# COMO LE HABLA NVIDIA -- la RTX 3060 12G vista desde el driver de Windows, capa por capa

> Pedido por el propietario (28-09), tras medir el muestreo de la 3060 con CUDA:
> *"investigar como le comunica con mi RTX 3060 12G? TODO E TODO Y PORQUE en
> ANATOMIA ABSOLUTA ATOMICA, para poder tener informacion asi en BMO-X pueda
> hablar con mi PROTON-X ... UN PERFIL ESPECIFICO RTX 3060 12G"*.
>
> [`ANATOMIA.md`](ANATOMIA.md) cuenta la tarjeta **como la ve BMO-X**. Esto es
> la otra mitad: **como le habla NVIDIA**, medido en el Windows del propietario
> con la MISMA tarjeta, para saber que hace cada capa, por que existe y cual le
> toca a BMO-X. El perfil declarativo sale de aqui y vive en
> [`PERFIL/GPU_3060.txt`](../../../../PERFIL/GPU_3060.txt).
>
> La regla de siempre, con tres etiquetas en cada dato:
>
> ```text
>    MEDIDO     lo dio la sonda o una herramienta en ESTA maquina (28-09)
>    FUENTE     lo dice un documento publico citado (WDK, open-gpu-doc, nouveau)
>    DEDUCIDO   se sigue de lo medido, pero nadie lo ha visto pasar
> ```
>
> Las medidas crudas: [`docs/metal/sonda_cuda/`](../../../../docs/metal/sonda_cuda/)
> (`sonda3060.cu`, `SALIDA.TXT`, `HERRAMIENTAS.TXT`), y el muestreo en la rama
> `metal/tex-cuda` (`docs/metal/tex_cuda/`). Un rango ("7-9 us") es el de TRES
> corridas de la sonda el 28-09; `SALIDA.TXT` guarda la ultima.

---

## 0. La respuesta corta, en un dibujo

```text
        WINDOWS + NVIDIA 610.74                         BMO-X
   ------------------------------------      ------------------------------------
   tu programa      tex3060.exe | juego       app | juego por PROTON-X
   runtime          cudart (dentro del exe)   VERRANO (Ring 3)
                    | d3d12.dll
   driver de        nvcuda64.dll   32 MB      --  (no hay: VERRANO arma los
   usuario (UMD)    nvwgf2umx.dll  90 MB          metodos el mismo)
   la puerta        D3DKMT* -> win32u         INVOKE + WAIT (los 2 syscalls)
                    -> dxgkrnl.sys (WDDM)
   driver de        nvlddmkm.sys  120 MB      ga10x + dev/gpu* (Ring 0)
   kernel (KMD)     *** EL RM CORRE AQUI,
                    EN LA CPU ***
   el RM            (dentro del KMD)          *** EL GSP-RM 570.144, EN LA
                    GSP: N/A                  TARJETA ***, por RPC (rm/)
   ---------------------------------- PCIe Gen3 x16 ----------------------------
   BAR0 0xFB000000  16 MiB  registros         el mismo
   BAR1 0xD0000000 256 MiB  ventana a VRAM    el mismo (sin ReBAR)
   BAR3 0xE0000000  32 MiB                    el mismo
   interrupcion     MSI, 1 mensaje            MSI (E2, visto en metal)
   ---------------------------------- la 3060 ----------------------------------
   HOST/PBDMA lee el GPFIFO -> metodos -> FE -> GR (28 SM) | COPY | NVDEC ...
```

**La diferencia que lo explica casi todo (MEDIDO):** `nvidia-smi -q` dice
`GSP Firmware Version : N/A` con `Driver Model : WDDM`. En Windows, en esta
GeForce, **el Resource Manager de NVIDIA corre en la CPU, dentro de
`nvlddmkm.sys`**, aunque el driver trae `gsp_ga10x.bin` (84.293.784 B) en su
carpeta. BMO-X hace lo contrario: arranca el GSP-RM en el RISC-V de la tarjeta
y le habla por RPC. Por eso las dos casas pagan cosas distintas:

```text
   Windows    el RM decide solo los relojes: la 3060 paso de 210 MHz (P8) a
              1.782-1.802 MHz al crear el contexto CUDA, y a 1.935-1.971 MHz
              tras 1,5 s de carga, sin que nadie lo pidiera           MEDIDO
   BMO-X      el GSP-RM no sube nada si no se le pide: PERF_BOOST, 60 s,
              x5,7 medido el 26-09 (PERFIL/GPU.txt)                   MEDIDO
```

---

## 1. Quien es, segun el driver de Windows (MEDIDO)

Coincide con lo que BMO-X lee en metal (`ANATOMIA.md` seccion 1), y agrega lo
que Windows sabe y BMO-X aun no pregunta:

| | valor | de donde |
|---|---|---|
| PCI | `10DE:2504`, subsistema `1462:397D`, rev A1 | `nvidia-smi -q`, PnP |
| pieza | `GPU Part Number 2504-302-A1` | `nvidia-smi -q` |
| VBIOS | `94.06.2f.00.9a` | `nvidia-smi -q` |
| InfoROM | `G001.0000.03.03` | `nvidia-smi -q` |
| VRAM | 12.288 MiB; el driver se reserva 173 MiB | `nvidia-smi -q` |
| BAR1 | **256 MiB** (sin Resizable BAR) | `nvidia-smi -q`, WMI |
| PCIe | la tarjeta Gen4, la placa Gen3 -> x16 Gen3; **en reposo baja a Gen1** | `nvidia-smi -q` |
| potencia | 170 W de tope (100 minimo); ~17-18 W en reposo | `nvidia-smi -q` |
| temperatura | objetivo 83 C, maxima 93, frena en 95, se apaga en 98 | `nvidia-smi -q` |
| relojes | maximo SM 2.130 MHz, memoria 7.501 MHz, video 1.950; P8 = 210/405 | `nvidia-smi -q` |
| driver | 610.74 (`32.0.16.1074`), `oem16.inf`, WDDM, CUDA UMD 13.3 | PnP, `nvidia-smi` |
| planificacion | HAGS encendida (`HwSchMode = 2`) | registro |
| interrupcion | MSI, `MessageNumberLimit = 1` | registro |

---

## 2. Lo que el silicio contesta por CUDA (MEDIDO, 159 atributos)

Los 159 estan en `SALIDA.TXT`. Los que cambian lo que BMO-X puede suponer:

```text
   SM                       28       (y %nsmid = 28)
   hilos por SM             1.536    = 48 warps (y %nwarpid = 48)
   bloques por SM           16
   registros                65.536 por SM y por bloque
   memoria compartida       100 KiB por SM; 48 KiB por bloque, 99 KiB pidiendola
                            (101.376 B); 1 KiB reservado por bloque
   constantes               64 KiB
   L2                       2.359.296 B segun CUDA (2,25 MiB); 1,55 MiB persistible
   bus de memoria           192 bits a 7.501 MHz -> 360 GB/s teoricos
   capacidad                8.6 (sm_86)
   reloj nominal            1.807 MHz (CLOCK_RATE)
   motores de copia         ** 1 ** para CUDA (ASYNC_ENGINE_COUNT) -- la tarjeta
                            tiene CINCO (GET_ENGINES_V2, ANATOMIA.md seccion 2)
   texturas                 alineacion 512 B, paso de fila alineado a 32 B,
                            2D hasta 131.072 x 65.536, lineal hasta paso 2.097.120
   vigilante                KERNEL_EXEC_TIMEOUT = 1: el TDR de WDDM mata un
                            trabajo que tarde demasiado (hay pantalla colgada)
   preempcion de computo    si
   memoria gestionada       si, pero CONCURRENT_MANAGED_ACCESS = 0 (WDDM)
```

**Por que el "1 motor de copia" importa:** no es el silicio, es la politica del
driver GeForce en WDDM. BMO-X ya usa COPY2 (la primera LCE asincrona; COPY0/1
son GRCE). Que CUDA vea una no dice nada de lo que BMO-X puede usar.

---

## 3. El camino de CUDA, paso a paso

### 3a. Cargar: quien entra en el proceso (MEDIDO)

`System32\nvcuda.dll` pesa 4.714.728 B, **lo mismo que
`nvcuda_loader64.dll`**: es un cargador. El CUDA de verdad es
`nvcuda64.dll` (32 MB) del `DriverStore`. Tras `cuInit` y el contexto, el
proceso tiene ademas:

```text
   dxcore.dll            el enumerador de adaptadores de Windows
   gdi32 / win32u        donde viven las D3DKMT* (win32u hace el syscall)
   nvapi64 + _impl       la API privada de NVIDIA
   nvdxgdmal64.dll       (DEDUCIDO por el nombre: el lado DXG del driver)
   nvobjectloader64.dll  14 MB
   nvcudart_hybrid64.dll
```

Coste: `cuInit` **17-19 ms**; crear el contexto primario **70-94 ms**.

### 3b. La puerta: lo que CUDA le pide a Windows (MEDIDO + FUENTE)

`nvcuda64.dll` casi no importa nada: `KERNEL32` (con `DeviceIoControl` y
`LoadLibrary*`), `GDI32` (solo `CreateDCA`/`DeleteDC`), `USER32`
(`EnumDisplayDevicesA`), `ADVAPI32`, `SHLWAPI`, `VERSION`. **Las 25 funciones
del kernel grafico las busca en marcha** por su nombre:

```text
   encontrar la 3060    EnumAdapters2/3, OpenAdapterFromDeviceName/FromHdc,
                        QueryAdapterInfo, CloseAdapter
   un dispositivo       CreateDevice, DestroyDevice
   memoria (WDDM 2)     CreateAllocation(2), DestroyAllocation, Lock(2),
                        Unlock(2), OpenResource, QueryResourceInfo,
                        GetSharedPrimaryHandle
   residencia           CreatePagingQueue, MakeResident, SetAllocationPriority,
                        QueryAllocationResidency
   mandar y esperar     Render, WaitForSynchronizationObjectFromCpu
   el tunel privado     Escape
```

Lo que dice cada grupo (FUENTE: WDK, `d3dkmthk.h`): la memoria la reparte el
**gestor de video de Windows** (VidMm, dentro de `dxgkrnl.sys`), no NVIDIA: por
eso hay colas de paginado y "residencia" -- Windows puede sacar una reserva de
la VRAM si otro proceso la necesita. `Escape` es el tunel por el que un UMD le
manda al KMD lo que WDDM no sabe decir: **DEDUCIDO**, por ahi viajan las
peticiones al RM de `nvlddmkm` (crear canales, clases, objetos).

**Lo que NO esta**, y tambien es un dato: ni `SubmitCommand`, ni
`SubmitCommandToHwQueue`, ni `CreateContextVirtual`, ni `CreateHwQueue`.
**DEDUCIDO:** CUDA no manda cada lanzamiento por las colas de WDDM; lo mas
probable es que escriba su GPFIFO y toque el timbre desde el proceso, como en
Linux. **Sin medir**: se ve con una traza ETW de `DxgKrnl` (pide
administrador). Es lo primero de la seccion 8.

Lo que si se mide es el precio:

```text
   encolar un kernel vacio        4,2 us (mediana; min 3,1; p99 27-54)
   lanzar + esperar (ida/vuelta)  8,5 us (mediana; min 7,6; p99 42-50)
   10.000 vacios seguidos         5,9-6,4 us cada uno EN LA GPU (eventos)
   cudaMalloc de 1 MiB            55-59 us (la reserva pasa por VidMm)
   crear una textura              7-9 us
```

### 3c. El kernel compilado: lo que el driver lee para lanzarlo (MEDIDO)

El `.cubin` de `tex3060` (ELF de 64 bits, `sm=86`, `toolkit=13.4`):

```text
   .text.muestrear            0x200 B   el SASS
   .nv.constant0.muestrear    0x184 B   = 0x160 del driver + 0x24 de parametros
   .nv.info                   REGCOUNT 14, FRAME_SIZE 0, MIN_STACK_SIZE 0
   .nv.info.muestrear         PARAM_CBANK 0x240160  -> parametros en el banco 0,
                                                       desde 0x160, 0x24 bytes
                              KPARAM_INFO x5: offsets 0x0 0x8 0x10 0x18 0x20
                              MAXREG_COUNT 0xff
   .nv.rel.action             R_CUDA_CONST_FIELD22_37
```

### 3d. El banco constante 0: el mapa que CUDA espera (MEDIDO)

Sale del SASS de `especiales` (cada registro especial de PTX, leido en el
kernel) y de los valores que devolvio con `<<<(3,2,1),(5,1,1),96 B>>>`:

| palabra de `c[0x0]` | que es | valor medido |
|---|---|---|
| `0x000` `0x004` `0x008` | `ntid.x/y/z` (el bloque) | 5 1 1 |
| `0x00C` `0x010` `0x014` | `nctaid.x/y/z` (la rejilla) | 3 2 1 |
| `0x028` | la PILA: el primer `MOV R1, c[0x0][0x28]` de TODO kernel | sin leer |
| `0x02C` | `dynamic_smem_size` | 96 |
| `0x030` `0x034` | `gridid` (64 bits) | 1, 2: +1 por lanzamiento |
| `0x088` .. `0x104` | `envreg0` .. `envreg31`, de 4 en 4 | `envreg0 = 0x035C32E6`, `envreg6 = 1`, el resto 0 |
| `0x10C` | `nsmid` | 28 |
| `0x114` | se RESTA a `SR_SMEMSZ` para dar `total_smem_size` | 128 con 96 dinamicos |
| `0x118` | 64 bits: el descriptor de memoria de TODO `LDG`/`STG` (`ULDC.64 UR4`) | sin leer |
| `0x160` .. | los parametros del kernel | los del `.nv.info` |

Y lo que NO sale del banco 0 sino del silicio: `smid` (`SR_VIRTUALSMID`),
`warpid` (bits 14..8 de `SR_VIRTID`), `laneid`, `clock64` (`SR_CLOCKLO`) y
`globaltimer` (`SR_GLOBALTIMERLO`, que avanza **de 1.024 en 1.024 ns**).
`nwarpid` ni se lee: el compilador escribe 48.

**Por que le importa a BMO-X:** `computo.rs` llega hoy hasta S3 (el semaforo
de informe); **aun no lanza un kernel** (no hay QMD). El dia que corra un SASS
que salio de `ptxas` -- y el corpus de oro sale de ahi --, ese SASS lee la pila
en `0x28` y el descriptor de memoria en `0x118` en su primera linea. Un banco 0
a cero no da error: da un kernel que escribe donde no debe.

### 3e. Las texturas: el asa ES un indice (MEDIDO + DEDUCIDO)

`cudaTextureObject_t`, tal como sale de `cudaCreateTextureObject`:

```text
   A Point Wrap   0x1     A Linear Wrap   0x5     otra imagen B      0x9
   A Point Mirror 0x2     A Linear Mirror 0x6     A repetida         0xA
   A Point Clamp  0x3     A Linear Clamp  0x7     tras destruir la 1 0xB (no reusa)
   A Point Border 0x4     A Linear Border 0x8     una SUPERFICIE     0xC
   200 mas: de 0xD a 0xD4, seguidas
```

**Lo que se sigue de ahi:** los bits 20..39 son siempre 0, y aun asi las ocho
combinaciones MUESTREAN DISTINTO (`SALIDA.TXT` de `tex_cuda`). Si el asa fuera
`tic | tsc << 20` con muestreadores independientes (lo que supone T1, como
NVK), las ocho usarian el muestreador 0. **DEDUCIDO: CUDA usa UN indice para las
dos piscinas**: la entrada `i` de las cabeceras (TIC) y la entrada `i` de los
muestreadores (TSC). FUENTE a comprobar: el campo `SAMPLER_INDEX`
(`INDEPENDENTLY` / `VIA_HEADER_INDEX`) de la QMD en `clc7c0qmd.h`, y
`SET_SAMPLER_BINDING` en la clase 3D.

**Por que CUDA elige eso y PROTON-X no puede:** en CUDA una "textura" es imagen
+ muestreador juntos, asi que un indice basta. En D3D12 las imagenes (SRV) y los
muestreadores viven en **heaps separados** y un sombreador combina cualquiera
con cualquiera: PROTON-X necesita el modo independiente, que es el de T1. Los
dos modos existen en la misma tarjeta; lo que no vale es mezclarlos.

Y el `TEX` cambia de forma segun de donde venga el asa (MEDIDO, dos binarios):

```text
   asa como parametro del kernel   TEX.SCR.LZ   R10, R8, R2, R5, 0x0, 0x58, 2D
                                   (0x58 palabras = c[0x0][0x160]: el asa se
                                   lee del banco constante)            tex3060
   asa en un registro (de un LDG)  TEX.SCR.B.LZ R6, R4, R4, R0, 2D     T1
```

El juez de T1 conoce solo la segunda. La primera es la que emite `nvcc` en
cuanto el asa es un argumento.

### 3f. Lo que la 3060 muestrea (MEDIDO, rama `metal/tex-cuda`)

96 muestras (Point/Linear x Wrap/Mirror/Clamp/Border x 12 puntos) de una 4x4
RGBA8 conocida. Tres cosas que un emulador no adivina:

```text
   el borde se CUANTIZA al formato   0,5 pedido -> 0,498039216 = 127/255
   (u,v) = (1,0; 1,0) con Point      Wrap -> texel 0; Mirror y Clamp -> texel 3;
                                     Border -> el borde
   pitch contra cudaArray            96 de 96 IGUALES bit a bit (sonda_cuda)
```

La tercera es la que convierte esto en juez: el TIC **pitch** de T0 de BMO-X
tiene que dar exactamente los numeros de `SALIDA.TXT` de `tex_cuda`.

### 3g. Los numeros del cable y del reloj (MEDIDO)

```text
   RAM fijada <-> VRAM      13,05 / 13,18 GB/s   (Gen3 x16 = 15,75 teoricos: 83 %)
   RAM paginable <-> VRAM   10,6-12,6 GB/s       (tres corridas; DEDUCIDO: CUDA
                                                  la pasa antes por una fijada)
   VRAM -> VRAM             331 GB/s de bus      (360 teoricos: 92 %)
   reloj del SM             1.782-1.802 MHz al empezar, 1.935-1.971 tras 1,5 s
                            (clock64 contra globaltimer: 20 M ciclos)
   globaltimer              paso de 1.024 ns
```

**El metodo del reloj vale para BMO-X tal cual** (B3 de
`PLAN_LA_3060_AFINADA.md`): un kernel de tres instrucciones que cuenta ciclos
del SM contra el `globaltimer`. No hace falta saber como lee NVIDIA sus MHz;
basta con que corra un kernel.

---

## 4. El camino de D3D12 (el de PROTON-X), contra el de CUDA

`nvwgf2umx.dll` (90 MB, el UMD de D3D12) solo nombra **5** funciones D3DKMT
(`Escape`, `SignalSynchronizationObject(2)`,
`SignalSynchronizationObjectFromGpu2`, `WaitForSynchronizationObject2`)
(MEDIDO). El resto lo recibe **como llamadas de vuelta del runtime de D3D**
(`D3DDDI_DEVICECALLBACKS`: reservar, mandar, paginar) (FUENTE: WDK). O sea:

```text
   CUDA     nvcuda64 -----------------------> D3DKMT -> dxgkrnl -> nvlddmkm
   D3D12    d3d12.dll -> nvwgf2umx -> pfn*Cb -> dxgkrnl -> nvlddmkm
```

Los dos acaban en el MISMO RM (el de `nvlddmkm`, en la CPU) y en los mismos
canales de la tarjeta. **Lo que BMO-X sustituye con PROTON-X es todo el tramo
del medio**: `d3d12.dll` + UMD + `dxgkrnl` + KMD se vuelven VERRANO + `ga10x` +
el GSP-RM. Lo que no cambia es el final: GPFIFO, metodos de `AMPERE_B`, SASS.

---

## 5. Capa por capa: NVIDIA en Windows contra BMO-X

| capa | Windows + 610.74 | BMO-X | estado en BMO-X |
|---|---|---|---|
| saber que tarjeta es | `EnumAdapters` + nvapi | `lectura/identidad.rs` (10DE:2504 + BOOT_0) | hecho |
| arrancar el firmware | lo hizo el GFW al encender; el RM en la CPU | `arranque/`: FWSEC-FRTS, booter en SEC2, GSP-RM | hecho (el 0x15, en `EL_0x15.md`) |
| el RM | dentro de `nvlddmkm.sys` | el GSP-RM, por RPC (`rm/`) | hecho |
| objetos | `Escape` (DEDUCIDO) | `GSP_RM_ALLOC`: ROOT, DEVICE, SUBDEVICE, VASPACE | hecho |
| espacio de direcciones | VidMm de Windows (WDDM 2) | `memoria/mmu.rs`, VASPACE de fuera | hecho |
| reservar memoria | `CreateAllocation`, 55 us/MiB | `memoria/vram.rs` + prestamos por la IOMMU | hecho |
| canal | el RM de la CPU | `motores/canal.rs`, `0xC56F`, 368 B | hecho |
| mandar trabajo | GPFIFO + timbre (DEDUCIDO) | GP_PUT + timbre `0xBB0090` | hecho |
| esperar | cerca vigilada (`WaitFor...FromCpu`) | semaforo de informe + latido | hecho; A3 (interrupcion) pendiente |
| copiar | 1 motor visible | COPY2 | hecho |
| 3D | `nvwgf2umx` -> `AMPERE_B` | `motores/tresde.rs` + VERRANO | hecho (el cubo, bit a bit con D3D12) |
| computo | QMD + el banco 0 de la seccion 3d | `motores/computo.rs` S1-S3, **sin QMD** | FALTA |
| texturas | un indice para TIC y TSC | `trabajos/texturas.rs` T0-T1, modo independiente | T2-T3 pendientes |
| relojes | el RM los sube solo | PERF_BOOST pedido al GSP-RM | hecho a mano |
| interrupciones | MSI, 1 mensaje | MSI (E2 VBLANK visto) | parcial |
| vigilante | TDR de WDDM | juez + plazos | otro esquema a proposito |

---

## 6. Lo que esto cambia para BMO-X, en orden

1. **T3 ya tiene juez.** `docs/metal/tex_cuda/SALIDA.TXT` es la 3060 misma
   muestreando, y pitch = array bit a bit: `gpu verrano textura` tiene que dar
   esos 96 grupos de bits, no "algo parecido".
2. **El modo del muestreador se decide y se escribe.** PROTON-X pide
   independiente (heaps separados de D3D12); CUDA usa indice compartido. Los
   dos corren en esta tarjeta: el que se elija va en `SET_SAMPLER_BINDING` (3D)
   o en `SAMPLER_INDEX` (QMD), y el asa se empaqueta de acuerdo con el.
3. **El juez del SASS tiene que conocer `TEX.SCR.LZ` con el asa en el banco
   constante**, o decir NO a proposito: es la forma que emite `nvcc` en cuanto
   el asa es un argumento.
4. **Cuando llegue la QMD, el banco 0 de la seccion 3d es el contrato** para
   correr SASS de NVIDIA. Lo minimo: `0x28` (pila) y `0x118` (descriptor de
   memoria) validos, y los parametros en `0x160`.
5. **Los relojes de referencia**: con carga, 1,94-1,97 GHz; en reposo, 210 MHz.
   Una medida de BMO-X que salga 10-20 veces mas lenta que Windows mira primero
   el P-state, no el codigo.
6. **Lo que NO se copia**: VidMm, la residencia, el TDR. Existen porque en
   Windows la 3060 la comparten 20+ procesos a la vez (`nvidia-smi` los lista
   todos como `C+G`). En BMO-X esa pregunta la contesta el orquestador.

---

## 7. Lo que esto NO afirma

- Que CUDA toque el timbre desde el proceso: es lo mas probable por lo que
  falta en sus importaciones, pero **no se ha visto**.
- Los bytes de los TIC/TSC que escribe CUDA: viven en la VRAM de su contexto y
  la API no los muestra (ver seccion 8).
- Que significa `envreg0 = 0x035C32E6`.
- Por que CUDA cuenta 2,25 MiB de L2.
- Nada del GSP de Windows: en esta tarjeta, con este driver, no corre.

---

## 8. Lo que falta medir, en orden

1. **Traza ETW de `DxgKrnl` + el proveedor de CUDA** (`nvcudaETW.xml` viene en
   el driver) durante `sonda3060.exe`: si por cada lanzamiento hay un paquete
   DMA de WDDM o no. Pide una consola de administrador.
2. **Los TIC/TSC de CUDA, leidos**: con `cuMemGetAddressRange` no se llega; un
   camino es la API de depuracion (`nvcudadebugger.dll`, tambien en el driver).
3. **`c[0x0][0x28]` y `c[0x0][0x118]` con valor**: un kernel en PTX no los
   puede leer; en SASS escrito a mano si.

---

## 9. Como repetirlo

```text
   pip install nvidia-cuda-nvcc nvidia-cuda-crt nvidia-nvvm nvidia-cuda-runtime
               nvidia-cuda-cuobjdump nvidia-cuda-nvdisasm           (13.4.92)
   MSVC 14.44 + SDK 10.0.26100 con PATH/INCLUDE/LIB a mano (vcvars64 falla aqui)
   python atributos.py <cu13>/include/cuda.h > atributos.h
   nvcc -arch=sm_86 -O2 -std=c++17 -o sonda3060.exe sonda3060.cu -lcuda -lpsapi
   sonda3060.exe > SALIDA.TXT
   cuobjdump -sass -fun especiales sonda3060.exe
```

Las herramientas y lo que dijeron: `docs/metal/sonda_cuda/HERRAMIENTAS.TXT`.
