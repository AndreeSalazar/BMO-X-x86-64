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
- [ ] **F0b -- SU ESTUDIO ESTATICO.** El BLAKE3 de cada uno (el del
  `.fwimage` del GSP, como `ring0/dev/gpu_gsp.rs:91` para 570) y sus
  cabeceras leidas como en
  `platform/drivers/gpu/ga10x/ESTUDIO_BOOTER_LOAD_570144.md` (BinHdr,
  HsHeaderV2, regiones), sin ejecutar nada.
  - **Como se sabe:** las cabeceras de 615 en una tabla al lado de las de 570.
- [ ] **F1 -- EL IDIOMA que cambia de 570.144 a 615.78.08.** Lo que BMO-X
  habla con el GSP, mensaje a mensaje, en las cabeceras de OpenRM de las dos
  versiones: los `_v03_00`, `_v17_00` y `_v17_02` que usa el driver, la
  meta de la WPR (`GSP_FW_WPR_META`), los argumentos de LIBOS, el registro y
  las ordenes del secuenciador (`arranque/secuenciador.rs`). Una tabla:
  igual, cambiado (y como), nuevo.
  - **Bloquea:** F0.
  - **Como se sabe:** cada fila con el fichero y la linea de OpenRM de cada
    version. Son 36 ficheros de Rust del driver y del kernel los que nombran
    570.144: la tabla dice cuales cambian de verdad.
- [ ] **F2 -- LAS DOS VERSIONES lado a lado (Ring 0, del propietario).** El
  driver elige por la huella del firmware: 615.78.08 con su idioma de F1, y
  570.144 como reserva hasta que 615 pase F4. Con el guardian de la version
  emparejada (seccion 2).
  - **Bloquea:** F1.
  - **Como se sabe:** el kernel compila; con un firmware de otra version, el
    ROJO antes de tocar la tarjeta; y lo de F3 en verde.
- [ ] **F3 -- EL BANCO sin metal.** Lo que se puede probar en el anfitrion:
  leer las cabeceras de los ficheros de 615, armar los mensajes nuevos y
  leer las respuestas de ejemplo, contra lo que dicen las fuentes de OpenRM.
  - **Bloquea:** F1.
- [ ] **F4 -- EL METAL: diez arranques en frio con 615.78.08.** Apagar y
  esperar, no reiniciar, como pedia G0. El GSP-RM arriba en los diez, con
  su informe.
  - **Bloquea:** F2.
  - **Como se sabe:** la tabla de los diez aqui. Con diez de diez, el `0x15`
    deja de ser una pelea y se retira la reserva 570.

# 4. Si 615 no basta

Si el booter de 615.78.08 tambien da `0x15` en frio, no es la version: se
vuelve a lo ya estudiado, que sigue en los planes superados y en
`platform/drivers/gpu/ga10x/` (`DESPERTAR_FIABLE.md`, `EL_0x15.md`), y en
primer lugar a la hipotesis G0-MODO (BMO-X prepara el GSP como Falcon y
OpenRM como RISC-V).

# 5. Lo que este plan NO es

- No toca ULTRA LinuX: se lee.
- No trae codigo de Linux ni de NVIDIA al kernel: trae el firmware (como ya
  se trae el de 570) y lo aprendido de las fuentes abiertas, escrito con las
  palabras y las pruebas de BMO-X.
- No cambia el driver hasta F2, y F2 es del propietario.
