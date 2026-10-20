# PLAN GPU COMO LINUX -- la 3060 con el firmware que ya arranca en ULTRA LinuX

> Pedido por el propietario (2026-10-20). ULTRA LinuX es SU esquema
> (`AndreeSalazar/ULTRA-LinuX`: el kernel oficial, destripado y mejorado
> por el), y en el la RTX 3060 arranca normal. Sus palabras: *"es una forma
> de confirmar que mi GPU FUNCIONA normal, entonces ese firmware lo traemos y
> que mi BMO-X arranque normal, pero tienen que inspirarse como se hizo mi
> [esquema]"*. Y la regla: **ULTRA se LEE, no se toca**; lo que se trae es lo
> aprendido.
>
> **Lo que afirma**: que BMO-X, con el firmware del GSP **615.78.08** -- el
> mismo con el que la 3060 arranca en ULTRA -- y con el esquema de ULTRA
> (version emparejada, firmware fijo, informe por arranque), despierta su
> GPU sin la pelea del `0x15`.
>
> **Como se cae**: si con 615.78.08 el booter sigue dando `0x15` en frio; o
> si se cambia de version sin saber que mensajes del GSP cambiaron.
>
> Este plan SUPERA a [`PLAN_LA_3060.md`](PLAN_LA_3060.md),
> [`PLAN_LA_3060_AFINADA.md`](PLAN_LA_3060_AFINADA.md) y
> [`PLAN_GSP_CONTROL_BMO.md`](PLAN_GSP_CONTROL_BMO.md): se quedan como
> historia (lo estudiado del `0x15` sirve si 615 no basta).

---

# 1. Lo que ULTRA LinuX demostro, leido de su arbol

| | En ULTRA (19-10) | Donde se lee |
|---|---|---|
| quien despierta el GSP | el driver de NVIDIA, `nvidia-open` 615.78.08, compilado contra su propio kernel 7.3-rc7 | `scripts/build-baseline.sh` |
| el firmware | `GPU Firmware: 615.78.08`: el de la MISMA version que el driver, desde una ruta fija de solo lectura (`firmware_class.path=/sistema/usr/lib/firmware`) | `evidencias/medicion-w3e/pc-real/gpu-*.txt` |
| la 3060 | GeForce RTX 3060, 12288 MiB, VBIOS 94.06.2f.00.9a, en P8 a 53 C y 19 W | `evidencias/pc-real/nvidia-2026-10-19/` |
| el orden | primero la pantalla del firmware (`simpledrm`), luego `nvidia` (1,37 s), el arbitro VGA le pasa la tarjeta, `nvidia-drm` listo a 3,62 s | el dmesg de esa carpeta |
| sin errores | ni una linea de error `NVRM` en el dmesg guardado | id. |

[!] **Lo que no dice**: cuantas veces de cuantas. Hay pocos arranques con
NVIDIA grabados en ULTRA. Es una confirmacion de que la tarjeta y la placa
sirven, no una estadistica.

# 2. Lo que se trae del esquema de ULTRA (inspiracion, no codigo)

- **La version EMPAREJADA.** En ULTRA, driver y firmware son la misma
  version o no hay GPU. En BMO-X: el driver sabe con que version habla, y un
  firmware de otra version es ROJO antes de tocar la tarjeta (hoy ya se fija
  el BLAKE3 de `gsp-570.144.bin`, `ring0/dev/gpu_gsp.rs:91`; se extiende a la
  version declarada de los cuatro ficheros).
- **El firmware FIJO.** Una ruta de solo lectura y su huella, como
  `/sistema/usr/lib/firmware`: en BMO-X, `fw/gsp/` (`gsp.bin`,
  `bootldr.bin`, `boot_ld.bin`, `boot_ul.bin`) con las huellas de 615.
- **El ORDEN.** La pantalla del firmware primero, la GPU despues, y cada
  paso con su tiempo.
- **El INFORME por arranque**, VERDE / AMARILLO / ROJO, y nada se da por
  probado sin su registro (lo que ULTRA guarda en `A:/ULTRA/informe`).

# 3. Las casillas

- [x] **F0 -- LOS CUATRO FICHEROS de 615.78.08.** HECHO el 20-10 en el PC
  del propietario (lo hizo otra IA alli, con este plan como encargo). En
  `BMO-externo\firmware\nvidia\ga102\gsp\`, al lado de los de 570.144, que
  no se tocaron (`sha256sum -c`: los cuatro OK):
  ```text
     fichero                       bytes      SHA-256
     booter_load-615.78.08.bin     61816      ad36e2044f76df6d0715031acb665c20cfcc70b0611748e41391b733a0d1db50
     booter_unload-615.78.08.bin   42616      8f68eb5bc034009f5b874aa0aa5da81ecc1fdad8e892c16afd862d78cf34fdc2
     bootloader-615.78.08.bin      24684      82428f532240727e95bb3083fbaaba9b2cc7b937314323f2d546ce7245f27fad
     gsp-615.78.08.bin             27043992   a20100cafd5a85e22b3e13188339730c6ced94bd5f654984dd2ed274ed58d584
  ```
  - **De donde:** el GSP es el `gsp_ga10x.bin` de ULTRA, leido sin montar
    de `build\pc\sistema-a.erofs` (`/usr/lib/firmware/nvidia/615.78.08/`);
    su SHA-256 es el que registra pacman en esa imagen (`nvidia-utils
    615.78.08-1`). Los booters y el bootloader salen de OpenRM 615.78.08
    (`open-gpu-kernel-modules`, commit `1a82fbfda8f3`) con su
    `extract-firmware-nouveau.py`.
  - **[!] El extractor se PARCHEO**: el de esa etiqueta (y el de `main`) no
    lee los arrays que 615 declara como `NvU64` y se para al empezar. Una
    copia con tres cambios los lee como palabras de 64 bits little-endian;
    las comprobaciones del propio script (medida comprimida, descomprimida
    y gzip) pasan en todos. linux-firmware no publica la 615: no hay un
    fichero oficial con el que comparar los booters.
  - **Lo que dicen los numeros**: el `bootloader` es el MISMO byte a byte
    que el de 570.144 (mismo SHA-256); los dos booters cambian (y el de
    carga mide 512 bytes mas: 61816 contra 61304); el GSP-RM pasa de 63 MB a
    27 MB. El GSP-RM entero es otro: F1 dice que cambio de su idioma.
- [x] **F0b -- SU ESTUDIO ESTATICO.** HECHO el 20-10; la linea del GSP-RM
  salio de `fw-huella` en el PC del propietario.
  - **Los booters y el bootloader, comprobados por segunda vez.** En el
    anfitrion, de las fuentes de OpenRM 615.78.08 con un lector de arrays
    `NvU64` ESCRITO APARTE del parche de F0: salen los tres con el MISMO
    SHA-256 de la tabla de F0. Dos parches independientes que coinciden: el
    `[!]` del extractor queda descartado. (Y los de 570.144, con su
    extractor sin parchear, dan los SHA-256 de `firmware.ps1`.)
  - **Sus cabeceras, leidas por el lector del kernel** (`booter.rs`, con
    `toolchain/tools/fw-huella`):
    ```text
                         570.144                     615.78.08
       booter_load    ucode 0x378+0xec00          ucode 0x378+0xee00
                      2 firmas de 384 B           2 firmas de 384 B
                      parche 0x8a10               parche 0x8c10
                      fuse_ver 1, engine 1, id 3  fuse_ver 1, engine 1, id 3
                      imem 0x100 -> 0, 0x8900     imem 0x100 -> 0, 0x8b00
                      dmem 0x8a00 -> 0, 0x6200    dmem 0x8c00 -> 0, 0x6200
       booter_unload  imem 0x4f00, dmem 0x4d00    imem 0x5500, dmem 0x4d00
       bootloader     RISC-V v5, codigo 0x1800, datos 0x800 (los dos IGUALES)
    ```
    La misma forma, la misma firma pedida (fusible 1, SEC2, ucode 3): solo
    crece el codigo. Ni un numero de 570 esta fijado en el kernel
    (`gpu_despertar` lee los tramos del fichero).
  - **`fw-huella` se valido con la 570.144**: sobre el `gsp-570.144.bin` de
    linux-firmware da `e7856ee2...eea0`, el `HASH_570` de `gpu_gsp.rs` byte a
    byte.
  - **El GSP-RM, en el PC del propietario** (20-10, sobre los cuatro de F0
    con sus SHA-256 comprobados):
    `cargo run --release -p bmo-fw-huella -- <BMO-externo>\firmware\nvidia\ga102\gsp 615.78.08`.
    Los dos booters y el bootloader dan lo mismo que la tabla de arriba, y
    el ELF de 615 trae las dos secciones:
    ```text
       gsp-615.78.08.bin  27043992 B
         .fwimage           0x19be000 B desde 0x40
         .fwsignature_ga10x 0x1000 B desde 0x19c906e
         BLAKE3 .fwimage    aaf3b527c14f0bf61df2fee6b766128abfe49b171fe0cc97d353fe334fdb588f
    ```
    Ese BLAKE3 es el `HASH_615` de `ring0/dev/gpu_gsp.rs`: el kernel ya
    reconoce la 615 por su huella.
- [x] **F1 -- EL IDIOMA que cambia de 570.144 a 615.78.08.** HECHO el
  20-10, leido en las cabeceras y fuentes de OpenRM de las dos etiquetas
  (`open-gpu-kernel-modules` 570.144 y 615.78.08, MIT). En corto: **615 no
  es un cambio de huella, es otro idioma de arranque.** Lo que BMO-X habla
  hoy con el GSP (los mensajes `0x1001`, `0x1002`, `SET_SYSTEM_INFO`,
  `SET_REGISTRY`) 615 lo cambia por una orden nueva, GMCAPI.

  | pieza (donde en BMO-X) | 570.144 | 615.78.08 | |
  |---|---|---|---|
  | meta de la WPR, `GspFwWprMeta` (`arranque/wpr.rs`) | magic `0xdc3aae21371a60b3`, revision 1 | la misma magic y revision; `padding[2]` pasa a `NvU16 pagingConfig`; 256 bytes; sin `GspFwHeapFreeList` | cambiado, poco |
  | regiones de LIBOS, `LibosMemoryRegionInitArgument` (`arranque/libos.rs`) | id8, pa, size, kind, loc | igual byte a byte (`libos_init_args.h`) | igual |
  | `RM_RISCV_UCODE_DESC` (cabecera de `gsp.bin`) | ... `bSignedAsCode` | + `bIsSmp`, `bIsPlicEnabled` al final | cambiado, se lee mas largo |
  | `GSP_ARGUMENTS_CACHED` (`arranque/libos.rs`) | sin magic; `bDmemStack` | magic `'GSP '`, `size`, `flags` (IN_PM_TRANSITION, STACK_IN_DMEM, RECOVERY_MARGIN...) | cambiado |
  | `MESSAGE_QUEUE_INIT_ARGUMENTS` (`arranque/libos.rs`) | offsets `NvLength` | + `flags`, offsets `NvU64`, medidas y alineaciones del elemento, `reserved[8]` | cambiado |
  | elemento de la cola, `GSP_MSG_QUEUE_ELEMENT` (`rm/orden.rs`) | authTag, aad, checkSum, seqNum, elemCount, y la cabecera RPC | MCTP (`"MCTP"`, medida, cabecera MCTP, cabecera NVDM; 16 B), [etiqueta de cifrado 16 B], NVDM (medida + reservado; 8 B), y dentro una RPC vieja **o** una GMCAPI | **otro formato** |
  | cabecera RPC (`rm/rpc.rs`) | `g_rpc-message-header.h` | igual (solo la fecha del copyright) | igual |
  | numeros de funcion y de evento (`rm/rpc.rs`) | ALLOC 103, CONTROL 76, FREE 10, GET_GSP_STATIC_INFO 65, eventos 0x1001.. | los mismos numeros; `GSP_SET_SYSTEM_INFO` (72) y `SET_REGISTRY` (73) marcados *deprecated* | igual de numero |
  | sistema y registro (`rm/estatica.rs`, `rm/contrato.rs`) | `GspSystemInfo` como struct en la RPC 72; el registro empaquetado en la 73; luego esperar `GSP_INIT_DONE` (0x1001) y pedir la 65 | **una sola orden** `GMCAPI_CMD_GSP_INIT` (`0x010001`): entra el sistema como pares clave/valor de 64 bits (`NVGMC_SI_*`, `gmcapi_system_info.h`), con el registro dentro; sale la info estatica tambien en clave/valor (`gmcapi_gsp_config.h`); ya no se espera `INIT_DONE` | **nuevo** |
  | `GspStaticConfigInfo` (`rm/estatica.rs`) | struct fijo | struct distinto (sin ecid, fb_*, l2_cache_size...; con pdi, vbiosRevision...) y ademas llega por clave/valor | **otro formato** |
  | el secuenciador (`arranque/secuenciador.rs`) | evento `0x1002` RUN_CPU_SEQUENCER con REG_WRITE/MODIFY/POLL/DELAY/STORE/CORE_* | `rmgspseq.h` sin opcodes ni `rpc_run_cpu_sequencer`: el GSP pide `EXEC_GENERIC_BOOTLOADER` (`0x010002`) y `EXEC_HS_BINARY` (`0x010003`) por GMCAPI, y la CPU carga el HS (reset, DMA IMEM/DMEM, PKC, BOOTVEC, esperar halt, `CORE_RESUME`) | **nuevo** |
  | `rpc_gsp_rm_control_v03_00` (`rm/control.rs`) | ... paramsSize, `flags`, params a +24 | `rmapiRpcFlags`, `rmctrlFlags`, `rmctrlAccessRight`, `reserved0` (u64), params a **+40** | cambiado (todas las ordenes de control) |
  | `rpc_gsp_rm_alloc_v03_00`, `rpc_free_v03_00` (`rm/objeto.rs`, `rm/liberar.rs`) | | iguales | igual |
  | `rpc_rc_triggered_v17_02` (`rm/rc.rs`) | | + rcChannelInfoType, instblkAddrLo/Hi, instblkAperture al final | cambiado, se lee mas largo |
  | `rpc_os_error_log_v17_00` (`rm/vigilante.rs`) | exceptType, runlistId, chid, errString, preemptive... | `preemptiveRemovalPreviousXid` pasa a segundo: **mismo nombre, otro orden** | cambiado |
  | el booter y FWSEC (`arranque/booter.rs`, `arranque/fwsec.rs`) | | `kernel_gsp_booter_tu102.c` y `_frts_` casi iguales; `kernel_gsp_fwsec.c` igual | igual |
  | el orden de `kgspBootstrap_TU102` | ... esperar `INIT_DONE` | el scrubber antes; si el GSP ya se suspendio para hablar con la CPU, cuenta como arrancado; luego `GSP_INIT` | cambiado |

  - **Lo que dice la tabla para el `0x15`:** el camino del booter (el que
    da el `0x15`) casi no cambia en el lado de la CPU; lo que cambia es el
    booter mismo (los ficheros de F0) y todo lo que viene DESPUES de que el
    GSP-RM arranca. Asi que F4 puede contestar la pregunta del `0x15` con
    solo la parte del booter portada: si con el booter de 615 el GSP-RM
    llega a correr (WPR2 arriba y el RISC-V vivo), el `0x15` era de
    version, aunque el GSP-RM no entienda aun nada mas.
  - **[!] El extractor de F0 y el `0x15`**: los booters de 615 salen de
    un extractor parcheado y sin fichero oficial con que comparar; si
    falla el booter, lo primero es descartar el parche.
- [ ] **F2 -- LAS DOS VERSIONES lado a lado (Ring 0, del propietario).** El
  driver elige por la huella del firmware: 615.78.08 con su idioma de F1, y
  570.144 como reserva hasta que 615 pase F4. Con el guardian de la version
  emparejada (seccion 2). Por F1 se hace en dos pasos:
  - **F2a -- hasta que el GSP-RM corre.** CODIGO HECHO el 20-10 (AMARILLO:
    compila, sus pruebas pasan; sin metal). Lo que cambio:
    - `ring0/dev/gpu_gsp.rs`: `HASH_615` (fijado en F0b), `Version`,
      `GSP_ES_615` (bit 61 de `INFO_GPU_GSP`), y `version()`: la version
      emparejada sale de la huella de lo visto por la radix3.
    - `ga10x/arranque/libos.rs`: `rmargs_615` (184 B, magic `'GSP '`) y la
      cabecera `msgqTxHeader` v2.0, con sus pruebas; los offsets, contra un
      `offsetof` compilado de las cabeceras de OpenRM 615.
    - `ring0/dev/gpu_libos.rs`: escribe y COMPRUEBA `rmargs` y la cola en el
      idioma de la version; sin version conocida, `IOMMU_NO_LIBOS_ORDEN`.
    - `gpu_despertar.rs`: acepta 570 o 615; `secuenciar` y
      `escribir_sistema` dicen `IOMMU_NO_IDIOMA_615` (94) con la 615: son
      del idioma de 570.
    - `build/firmware.ps1`: prepara la 615.78.08 si estan sus cuatro en
      `BMO-externo`, si no la 570.144; nunca mezcla; `version.txt` al lado;
      `$env:BMO_GSP_VERSION` fuerza una. Probado con pwsh: los tres casos.
    - El director dice la version que ve.
    - Sin cambio, porque 615 no lo cambio: la WPR meta (`pagingConfig` a 0
      es el `padding` de antes), LIBOS, el bootloader, el heap de una GA102.
  - **F2b -- hablarle.** La cola MCTP/NVDM, GMCAPI (`GSP_INIT` con clave/
    valor, `EXEC_HS_BINARY` en lugar del secuenciador), el control a +40,
    y los eventos mas largos. Es la parte grande: sin ella, no hay
    pantalla ni juego.
  - **Bloquea:** F1.
  - **Como se sabe:** el kernel compila; con un firmware de otra version, el
    ROJO antes de tocar la tarjeta; y lo de F3 en verde.
- [ ] **F3 -- EL BANCO sin metal.** Lo que se puede probar en el anfitrion:
  leer las cabeceras de los ficheros de 615, armar los mensajes nuevos y
  leer las respuestas de ejemplo, contra lo que dicen las fuentes de OpenRM.
  - **Bloquea:** F1.
- [ ] **F4 -- EL METAL: diez arranques en frio con 615.78.08.** Apagar y
  esperar, no reiniciar, como pedia G0. **F4a** (tras F2a): el booter sin
  `0x15` y el RISC-V vivo en los diez. **F4b** (tras F2b): el GSP-RM
  contesta `GSP_INIT` en los diez, con su informe.
  - **Bloquea:** F2.
  - **Como se sabe:** la tabla de los diez aqui. Con diez de diez, el `0x15`
    deja de ser una pelea y se retira la reserva 570.

# 4. Si 615 no basta

Si el booter de 615.78.08 tambien da `0x15` en frio, no es la version: se
vuelve a lo ya estudiado, que sigue en los planes superados y en
`platform/drivers/gpu/ga10x/` (`DESPERTAR_FIABLE.md`, `EL_0x15.md`), y en
primer lugar a la hipotesis G0-MODO (BMO-X prepara el GSP como Falcon y
OpenRM como RISC-V).

## 4a. Lo que dio el metal con la 615 (19-10, 21:54 a 22:03 hora local)

14 arranques de `save mode` seguidos (reinicios PEDIDOS cada ~30 s, no en
frio), build 120A0DDD4E91B7, GSP-RM 615 verificado por radix3 en los 14:
**2 arrancan, 12 dan `0x15`.** La 615 NO quita el `0x15`: es intermitente, no
de version. En los 2 buenos el RISC-V queda ACTIVO, el booter deja 0 y el
GSP-RM escribe 69 entradas en LOGINIT (sin diccionario en el ELF de 615). Lo
que separa bueno de malo en la captura de la fase 4 (`HALTED`, tras el
booter) es CONSECUENCIA: en los malos todo el bloque del GSP contesta
`0xBADF...` (pri-mask 0x200F); en los buenos solo CPUCTL (0x0004).

## 4b. OpenRM 615 contra BMO-X, paso a paso antes del booter (20-10)

Leido en `kgspBootstrap_TU102`, `kgspExecuteBooterLoad_TU102`,
`kgspExecuteHsFalcon_GA102`, `kflcnResetIntoRiscv_GA102`,
`kflcnReset_TU102`, `ksec2ResetHw_TU102` (OpenRM 615.78.08, MIT):

| | OpenRM (lo que corre en ULTRA) | BMO-X hoy (sin `g0.txt`) | interruptor de BMO-X |
|---|---|---|---|
| el GSP tras FWSEC | `kflcnResetIntoRiscv`: reset, BCR = RISC-V + VALID + BRFETCH, SIN arrancar; LIBOS en los buzones | reset a FALCON y STARTCPU del falcon VACIO; esperar que se pare | `riscv` (E3) |
| el pulso del reset (GSP y SEC2) | 10 LECTURAS del registro por flanco (`FLCN_RESET_PROPAGATION_DELAY_COUNT`) | 10 us de espera en la CPU | `reset` (E5) |
| FWSEC -> booter | en la MISMA funcion: milisegundos | ~522 ms, con Ring 3 y un `save` a disco en medio | ninguno (`seguido`, E4, solo junta despertar y booter) |
| el SEC2 antes del booter | reset, esperar RESET_READY y MEM_SCRUBBING, nucleo FALCON, `RM` = chipId | lo mismo (`resetear_con`) | -- |
| el DMA del booter | sysmem coherente, fisica | sysmem coherente por la IOMMU | -- |
| el scrubber | no-op en GA10x: la VBIOS deja limpios los 256 MB de arriba | no hay | -- (la region del GSP mide ~157 MB: cabe) |

Con la 570, E3 solo dio `0x15` y E5 solo, mixto (17-10). NUNCA se probaron
JUNTOS, ni con la 615, ni con el hueco FWSEC -> booter cerrado.

- [ ] **F4a' -- OpenRM entero, con lo que ya hay.** `informe/estado/g0.txt` =
  `riscv seguido reset`, sin recompilar, y la misma tanda: cuantos de N
  arrancan, contra 2 de 14. Si sube, el `0x15` es del procedimiento.
- [ ] **F4a'' -- cerrar el hueco.** FWSEC dentro de la misma llamada que
  despertar y el booter, sin `save` en medio (un interruptor nuevo de G0).
  Solo si F4a' no basta.

# 5. Lo que este plan NO es

- No toca ULTRA LinuX: se lee.
- No trae codigo de Linux ni de NVIDIA al kernel: trae el firmware (como ya
  se trae el de 570) y lo aprendido de las fuentes abiertas, escrito con las
  palabras y las pruebas de BMO-X.
- No cambia el driver hasta F2, y F2 es del propietario.
