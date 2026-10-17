# GCB2b -- lo que recibe el booter, campo a campo, contra NVIDIA y nova-core

> GCB2b de [PLAN_GSP_CONTROL_BMO](../../../../docs/plan/PLAN_GSP_CONTROL_BMO.md),
> 17-10-2026. Pregunta: **que le da BMO-X al booter, y en que orden, que no le
> den OpenRM 570.144 (el driver de NVIDIA de ESTA version del firmware) y
> nova-core (el driver en Rust de Linux).** Solo comparacion: no cambia nada
> del arranque. La prueba sale en GCB2c.

Fuentes, descargadas el 17-10-2026:

- OpenRM fijado al tag **570.144**: `kernel_gsp_tu102.c` (`kgspCalculateFbLayout_TU102`,
  `kgspBootstrap_TU102`), `kernel_gsp_booter_tu102.c`, `kernel_gsp.c`
  (`kgspGetWprEndMargin_IMPL`, el bucle de `bootAttempts`), `kernel_falcon_ga102.c`
  (`kflcnResetIntoRiscv_GA102`) y `g_kernel_gsp_nvoc.h`.
- nova-core en `master` (referencia mutable, sin SHA): `gsp/hal/tu102.rs`,
  `gsp/hal/ga102.rs`, `gsp/boot.rs`, `gsp/fw.rs` (`GspFwWprMeta::from_ranges`),
  `fb.rs`, `falcon.rs`, `falcon/hal/ga102.rs`.
- BMO-X: `src/arranque/wpr.rs`, `src/arranque/falcon.rs` y
  `kernel/src/ring0/dev/gpu_despertar.rs`; el parte del metal del 17-10
  (`gpu control` y `save`, build `89E4313C14499A`).

## 1. Los DATOS: la WPR meta (256 bytes) y su reparto de la VRAM

| campo | BMO-X (`wpr_meta`) | nova-core | OpenRM 570.144 |
| --- | --- | --- | --- |
| magic, revision | `0xDC3AAE21371A60B3`, 1 | igual | igual |
| radix3, bootloader, firma (direccion y medida) | IOVA prestadas por la IOMMU | DMA de Linux | fisicas (`AT_GPU`) |
| offsets del bootloader (codigo, datos, manifiesto) | del `RM_RISCV_UCODE_DESC` | igual | igual |
| `fbSize` | VRAM entera (12288 MiB) | `vidmem_size` | `kmemsysGetUsableFbSize` |
| vga | `0x625F04`: fbSize - 1 MiB | misma regla | misma regla (si es menor, fbSize - 128 KiB) |
| `gspFwWprEnd` | vga bajada a 128 KiB | igual | igual **menos el margen de recuperacion** (0 en el arranque normal) |
| frts, boot, elf, heap | 1 MiB / 4 KiB / 64 KiB / 1 MiB, de arriba abajo | igual | igual |
| heap | LIBOS3: 128 MiB con 12 GiB | igual | `_kgspCalculateFwHeapSize` (no recalculado aqui) |
| `gspFwWprStart` | heap - 1 MiB | igual | heap - `align_up(256, 1 MiB)` = heap - 1 MiB |
| no-WPR | 1 MiB debajo | igual | `kgspGetNonWprHeapSize` alineado a 1 MiB |
| `flags` | 0 | 0 | `CLOCK_BOOST` si el registro no lo apaga |
| cola de CrashCat | 0 | 0 | su direccion si existe |
| bloqueo MMU de la VBIOS | no se mira | no se mira | `min(mmuLockLo, vga)` si es valido |
| `bootCount`, `verified` | 0 | 0 | 0 |

**Lo que dice la tabla.** La WPR meta de BMO-X es la de nova-core campo a
campo. Contra OpenRM difiere en lo opcional: `CLOCK_BOOST`, la cola de
CrashCat y el bloqueo MMU de la VBIOS. Ninguno es obligatorio para nova-core.

**Y lo decisivo: el dato no cambia entre un arranque bueno y uno malo.** El
vigia del booter da la misma huella en los malos de hoy y en los de
septiembre (`0x908311CBC2BAB6E7`, LIBOS igual), el reparto `cuadra` en todos
y los buenos del 25-09 al 29-09 usaron este mismo reparto. Un dato fijo no
explica un fallo que va y viene.

Lo que pide la region limpia: OpenRM solo usa el "scrubber" si `fbSize -
gspFwRsvdStart` pasa de los 256 MiB que limpia el firmware al encender
(`kgspGetPrescrubbedTopFbSize`). Aqui son `0x300000000 - 0x2F3F00000` =
**193 MiB**: cabe. No hace falta scrubber.

## 2. El PROCEDIMIENTO: el orden, paso a paso

| paso | OpenRM 570.144 | nova-core | BMO-X |
| --- | --- | --- | --- |
| 1 FRTS | `kflcnReset` y FWSEC-FRTS | reset y FWSEC-FRTS | paso `fwsec` |
| 2 el falcon del GSP | **`kflcnResetIntoRiscv`**: reset y `BCR_CTRL` = RISC-V + VALID + BRFETCH | `reset()`: nucleo **FALCON**, `RM` = BOOT_0 | `fa::resetear`: nucleo FALCON, `RM` = BOOT_0 (como nova-core) |
| 3 los argumentos de LIBOS | se escriben en MAILBOX0/1 **y no se arranca nada** | MAILBOX0/1 y **STARTCPU** del falcon vacio, esperar a que se pare | MAILBOX0/1 y STARTCPU (como nova-core) |
| 4 scrubber | si hace falta (aqui no) | -- | -- |
| 5 el booter en el SEC2 | reset, cargar, MAILBOX0/1 = WPR meta, arrancar, esperar, MAILBOX0 debe ser 0 | igual | igual (`booter()`) |
| tiempo de 1 a 5 | seguido, en la misma llamada | seguido, en la misma llamada | **533 ms**, en pasos de Ring 3; el GSP espera parado **120 ms** |
| si falla | reintenta SOLO si hubo error de ECC, moviendo la WPR (margen de recuperacion) | descarga y error | autopsia; `gpu reintentar` |

Medido en el metal el 17-10, igual que el 16-10 a las 23:23: **GSP ANTES**
`MAILBOX0/1 = 0x3C000000/0`, `CPUCTL = 0x12` (parado tras arrancar),
`BCR_CTRL = 0x1` (VALID, nucleo FALCON). Es exactamente el estado del paso 3
de nova-core, y no el de OpenRM.

Tres diferencias de procedimiento, y su estado:

- **D1, G0-MODO**: el falcon del GSP en nucleo FALCON y arrancado vacio
  (BMO-X, nova-core) contra nucleo RISC-V sin arrancar (OpenRM 570.144).
  Nunca probada. [ESTUDIO_RTX3060_PROTON_X](ESTUDIO_RTX3060_PROTON_X.md), seccion 3.
- **D2, el tiempo**: 533 ms entre FRTS y el booter y 120 ms del GSP parado,
  contra milisegundos en los dos drivers. Probada a medias: con FRTS justo
  antes de `despertar` (717 ms) siguio el 0x15 (29-09 14:37). El hueco
  `despertar` -> booter (H4 de [EL_0x15](EL_0x15.md)) nunca se quito.
- **D3, el reintento de NVIDIA**: no aplica a esta 3060 (sin ECC); solo
  confirma que NVIDIA cuenta con que el arranque PUEDE fallar.

## 3. Lo que dice la tabla de arranques, y que cambia el orden

[`EL_0x15_ARRANQUES.csv`](EL_0x15_ARRANQUES.csv), solo los arranques sin
`fuego` ni `frontera` (la causa vieja, ya quitada):

```text
   hasta el 29-09 06:25    7 de 7 buenos   (25-09 noche .. 29-09 06:25)
   desde el 29-09 10:09    1 de 13 buenos  (el 10:24 del mismo 29-09)
```

Si buenos y malos se repartieran al azar entre esos 20, que los siete
primeros salgan todos buenos tiene una probabilidad de **1 entre 9.700**
(hipergeometrica: 8 buenos de 20, 7 de 7). **No es suerte: algo cambio el
29-09 entre las 06:25 y las 10:09.**

Y ese algo **no es D1**: los siete buenos usaron el mismo nucleo FALCON y el
mismo arranque vacio que los malos de hoy. G0-MODO puede seguir siendo una
mejora, pero no explica el cambio de regimen.

Lo que entro en el arbol en esa ventana (hora de Lima, `git log`):

| commit | que toca | toca el camino del booter? |
| --- | --- | --- |
| `c02c40fdb` | lecturas ANTES de FRTS: el falcon del GSP, la VGA, el espacio de configuracion PCI (FLR, capacidades) | si, solo lecturas |
| `55981cc10`, `c76507ad0` | N0 y **N1a**: el disco Personal (D:) en NTFS, montado al arrancar, otro puerto del mismo SATA | no directamente; mas DMA y mas puertos vivos |
| `3d8212504`, `db9a8951e`, `21d9fbdd7` | Z1, Z3, Z4a: el fotograma de una app en la VRAM | despues del GSP |
| `18d0be2cb`, `840e8e171` | recetas en caliente, `run` de PROTON-X | despues del GSP |

Dos limites, dichos: no hay registro de arranques entre el 26-09 06:46 y el
29-09 06:25 (los siete buenos se juntan en dos dias), y el bueno de las
10:24 ya llevaba todo eso. El cambio sube la probabilidad del 0x15; no la
hace 1.

## 4. Lo que se propone para GCB2c, en orden

Una variable cada vez, y el propietario decide cuantos arranques gasta.

1. **E1, el build como variable** (decide entre CODIGO y FUERA): desplegar
   el build del 29-09 anterior a las 06:25 (`a2408875a`, el merge de las
   06:17) en la misma maquina, sin cambiar nada mas. Que ese fuera el build
   desplegado aquel dia a esa hora no quedo escrito -- es lo que OBS-BUILD
   arregla desde hoy --; es el ultimo commit antes del arranque bueno.
   Tres buenos seguidos (con 1 de 13 de ahora, la probabilidad de eso por azar es menor de 1 en
   2.000) culpan al codigo de la ventana, y se parte por la mitad hasta el
   commit. Un 0x15 lo exculpa y apunta fuera (placa, Windows antes, la
   tarjeta). Riesgo: ese build no lee lo que las apps de hoy dejaron en el
   disco; se despliega en otro volumen de arranque o se vuelve al de hoy
   justo despues.
2. **E2, N1a apagado**: el build de hoy sin montar D: (una sola condicion en
   `disk::ajeno::buscar`). Si E1 culpa al codigo, este es el primer
   sospechoso: es el cambio mas grande de la ventana.
3. **E3, D1 / G0-MODO**: el arranque de OpenRM 570.144 tal cual
   (`resetear_en_riscv`, buzones sin STARTCPU, el booter sin exigir el
   falcon parado), detras de un modo que se elige, con el de hoy como
   referencia. Va despues de E1 porque no explica el cambio de regimen.
4. **E4, D2**: `despertar` y el booter en la misma llamada, sin hueco.

En todas: kernel y DIRECTOR del mismo build, `save` al final, salga bien o
mal, y una fila nueva en el CSV con su build (OBS-BUILD ya lo escribe).
