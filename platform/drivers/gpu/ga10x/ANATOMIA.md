# ANATOMIA -- la RTX 3060 12G de MSI del propietario, por dentro

> Pedido por el propietario (26-09), tras el quinto 0x15 del booter: *"AISLAR
> BRUTALMENTE los archivos por completo y poner guias con reglas ... estudiar
> que esta hecho en GPU RTX 3060 12G de MSI ... que explican que son en todo la
> anatomia ... no quiero sorpresas"*.
>
> Regla de este documento: **solo lo MEDIDO por BMO-X en esta tarjeta, o lo
> que dicen fuentes publicas citadas**. Lo que no se sabe dice "sin medir".
> No se inventa nada: la tarjeta ya nos sorprendio cinco veces por suponer.

---

## 1. Quien es (medido)

| | valor | de donde |
|---|---|---|
| PCI | `10DE:2504`, subsistema `1462:397D` (1462 = MSI) | espacio de configuracion PCI |
| chip | GA106-A, revision A1, `BOOT_0 = 0xB76000A1` (Ampere) | `lectura/identidad.rs` |
| VRAM | 12288 MiB GDDR6, bus de 192 bits | `0x1183A4` (lo escribe el GFW) y el GSP-RM |
| bus | PCIe Gen4 x16 de capacidad; va en Gen3 x16 en esta placa (A320) | capacidad PCI Express |
| pantalla | 4 cabezas, pinta la 0; 1920x1080 a 60,000 Hz medidos | `lectura/vblank.rs` y `dev/gpu.rs` |
| VBIOS | 546 KiB en 4 imagenes: PCI-AT (63K), EFI (82K, el GOP), FWSEC (21K) y FWSEC (379K) | `arranque/vbios.rs` |
| fusibles | FWSEC: `0x8241E0 = 3` -> firma version 2; booter: `0x824148 = 1` -> firma 0 de 2 | `arranque/vbios.rs`, `arranque/booter.rs` |

**El modelo comercial exacto de MSI** (Ventus, Gaming X...) NO sale del
subsistema: no se afirma. Para el software lo que cuenta es la fila de
arriba; el disipador y la alimentacion de la placa solo se ven como
temperatura y P-state.

## 2. Lo que tiene dentro (medido por `GET_ENGINES_V2`)

```text
   GR0          el motor grafico: 3D (AMPERE_B) y computo (AMPERE_COMPUTE_B)
   COPY0..4     cinco copiadores; BMO-X usa COPY2 (el del canal)
   NVDEC0       decodificador de video       -- sin usar
   NVENC0       codificador de video          -- sin usar (Ludoteca 14)
   OFA0         flujo optico                  -- sin usar
   SEC2         el falcon de seguridad: corre el BOOTER
   SW0          metodos de software del RM
```

Y los procesadores que no son motores:

```text
   el falcon del GSP  primero corre FWSEC (de la VBIOS), despues es un RISC-V
                      que corre el GSP-RM (570.144): el "driver" de NVIDIA
                      DENTRO de la tarjeta
   el SEC2            un falcon en modo SEGURO: solo corre codigo firmado
```

## 3. La cadena de confianza (por que nada se salta)

```text
   fusibles (de fabrica)       dicen QUE VERSION de firma vale
        |
   la ROM de cada falcon       comprueba la firma antes de correr nada
        |
   FWSEC-FRTS  (VBIOS)         monta la WPR2 en la cima de la VRAM
        |                      -- solo corre con la WPR2 VACIA
   el BOOTER   (fw de NVIDIA)  en el SEC2: EXTIENDE la WPR2 a todo el GSP-RM,
        |                      lo copia dentro y arranca el RISC-V del GSP
   el GSP-RM                   arranca, pide el secuenciador, dice INIT_DONE
        |
   BMO-X                       habla con el por RPC; nunca entra en la WPR2
```

**Lo que BMO-X NO puede hacer, y es a proposito:** leer o escribir la WPR2,
correr codigo sin firma en un falcon seguro, o saber por que un firmware
firmado dijo que no -- el 0x15 del booter es eso: un NO sin explicacion
publica.

## 4. Los dominios de estado: que sobrevive a que

Es la pregunta que cinco 0x15 dejaron abierta. El propietario de estas direcciones
es `src/lectura/aon.rs`, y su cabecera lleva la tabla viva.

| estado | que es | reinicio del PC (sin cortar) | cortar la corriente |
|---|---|---|---|
| VOLATIL | registros del chip (falcons, pantalla, motores) | se borran | se borran |
| RAM | lo que BMO-X presta en la RAM del PC (GSP-RM, LIBOS) | lo rehace BMO-X | lo rehace BMO-X |
| VRAM | la memoria de la tarjeta | **SOBREVIVE** (lo dice la WPR2 viva) | se pierde |
| WPR | la region protegida (0x1FA824/28) | **SOBREVIVE** (metal 24-09 07:48) | cae |
| AON | PGC6/BSI, 0x118000..0x118FFF | sin medir; el bit 26 de BSI_14 estaba ABAJO antes del booter en el 0x15 de 25-09 19:59 | cae |
| FUSIBLE | la version de firma, la identidad | fijo | fijo |
| ROM | la VBIOS | fija | fija |

**La regla que sale de la tabla:** lo que sobrevive a un reinicio puede
hacer que un fallo aparezca en OTRA sesion. Por eso: BMO-X no escribe en WPR
ni en AON, nunca (regla A3); y entre pruebas del arranque del GSP, APAGAR y
cortar la corriente 30 s.

## 5. Las carpetas: un carril cada una

`src/` esta partido por lo que cada fichero ARRIESGA. Cada carpeta tiene su
`mod.rs` con su carril, y los `pub use` de `lib.rs` dejan los caminos de
siempre (`bmo_gpu_ga10x::falcon`).

| carpeta | carril | que hay | que estado toca |
|---|---|---|---|
| `lectura/` | VERDE | identidad, salud, vblank, **aon** | lee; la unica escritura es el aviso del VBLANK |
| `arranque/` | ROJO | falcon, vbios, fwsec, booter, elf, wpr, libos, secuenciador, correr, descarga | firmware, WPR, AON: **un error aqui se ve en la SIGUIENTE sesion** |
| `rm/` | AMARILLO | rpc, orden, estatica, objeto, control, contrato | lo que sale al GSP-RM, por la lista cerrada |
| `memoria/` | ROJO | vram, mmu | tablas de la MMU: una PTE mala da memoria ajena |
| `motores/` | AMARILLO | canal, copia, gr, computo, sombreador, tresde | canales y clases, pagados por semaforo |
| `trabajos/` | VERDE | lienzo, blur, fractal, triangulo, raster, color3d, giro, pantalla, video, volcado, escena, cubo, tuberia | bytes y jueces; tocan la tarjeta por `motores` y `memoria` |
| `sass/` | VERDE | **juez** (el juez del SASS: `TOMA TU BODRIO` o `PERFECTO Y PRECISO`), corpus (lo que ya corrio en el metal) | nada: lee programas y dice SI o NO, antes de que la 3060 los vea |

Cada fichero de `arranque/` y `lectura/` declara en su cabecera:

```text
//! [estado]  AON WPR       el motivo, detras de DOS espacios
```

con el vocabulario cerrado `VOLATIL VRAM WPR AON FUSIBLE ROM RAM`.

## 6. Las reglas (las comprueba el build: `toolchain/tools/la-3060`)

| | regla | por que existe |
|---|---|---|
| P | la puerta de las ordenes pide la autoridad `MAQUINA` | un juego con la pantalla prestada mandaba en la 3060 |
| R | solo `dev/gpu*`, `dev/gpu_trabajo/` y `dev/vblank.rs` tocan registros | que un registro mal escrito tenga pocos sospechosos |
| O | cada orden vale lo mismo en kernel, ABI y userland, con nombre y brazo | un numero con dos significados |
| M | igual con cada motivo del NO, y con su texto | un NO sin porque |
| E | los giros de la CPU esperando a la 3060 solo pueden bajar | cada uno es un nucleo perdido |
| I | la 3060 12G y SOLO ella | todo se probo contra UNA tarjeta |
| O3 | el crate va con `opt-level = 3` | lo caliente, optimizado a proposito |
| N | el pase de la GPU es NEUTRO: ni un nombre de NVIDIA | para la tarjeta alternativa |
| **A** | **lo que SOBREVIVE tiene UN propietario (`aon.rs`) y nadie lo escribe; cada fichero de `arranque/` y `lectura/` declara su `[estado]`** | **el 0x15: un fallo que viene de otra sesion** |

## 7. Las sorpresas que ya dio (y lo que mostro cada una)

| cuando | que | la leccion |
|---|---|---|
| 24-09 07:48 | primer 0x15 del booter | la WPR2 sobrevive a un reinicio |
| 24-09 14:09 | la foto "al llegar" dijo WPR2 con techo, y FWSEC la encontro vacia | al sondear, el firmware de arranque de la tarjeta aun no acabo: una lectura temprana miente |
| 24-09 | el enlace en Gen3 "sin RM" | no era prueba de nada: sale igual en frio |
| 25-09 | el GSP apagado en orden y despues `gpu raster` decia NO | tras la despedida no hay motor grafico: el kernel ya lo dice al instante |
| 25-09 13:35 | D2: 2 s con las interrupciones cerradas | un syscall no cede: la comparacion va fila a fila |
| 25-09 | el color a 8 bits trunca a 12 bits | es del silicio (X5c), no de Windows |
| 25-09 19:38 y 19:59 | 0x15 con la tarjeta fria segun la WPR2, y con BSI abajo | la WPR2 no es todo el estado; el GSP no se dejaba leer al pararse el SEC2 |
| 25-09 ~21:30 | el primer arranque en frio SIN `fuego`/`frontera` levanta el GSP | la causa del 0x15 era nuestra: el falcon del GSP tocado antes del booter (3 de 3; fuera de `save mode`). El caso: [`EL_0x15.md`](EL_0x15.md) |

## 8. Lo que falta medir (sin esto, seguira habiendo sorpresas)

- Las casillas "sin medir" de la seccion 4: un arranque de cada tipo
  (reiniciar, reinicio por el bus, cortar la corriente) con las filas `al
  llegar` y `autopsia`.
- **La autopsia de un arranque BUENO**, para comparar con los malos: que
  tardo el booter, que dicen los CPUCTL crudos y los buzones del GSP.
- El reinicio por el bus del cargador (`s1_cpu::gpu_reinicio`): si limpia la
  WPR2 y el dominio AON, o solo los registros.

Las fuentes publicas usadas: nova-core (Linux, `drivers/gpu/nova-core`),
nouveau (`nvkm/subdev/gsp`) y el driver abierto de NVIDIA 570.144
(`kernel_gsp_tu102.c`).

## 9. Lo que BMO-X le entrega al booter, dato por dato (26-09)

El 0x15 es un NO del booter. Un NO tiene que venir de algo que se le dio, del
estado de la tarjeta, o del momento. Esto es TODO lo que se le da, con de
donde sale cada numero y si puede cambiar de un arranque a otro. **Un dato
que varia entre arranques es un sospechoso; uno fijo, no.**

### 9a. Los registros, en orden (`dev/gpu_despertar.rs`)

| paso | que se escribe | valor | varia? |
|---|---|---|---|
| 0 | la pagina de VACIADO del GSP (`VACIADO`, `VACIADO_HI`), releida | IOVA `0x3C002000` | fijo |
| 1 | reset del falcon del GSP; sus MAILBOX0/1 = los argumentos de LIBOS; arranca y se para solo | IOVA `0x3C000000` | fijo |
| 2 | reset del SEC2; FBIF en fisico coherente (`TRANSCFG`), `DMACTL` 0 | constantes | fijo |
| 2 | IMEM y DMEM del SEC2, copiadas por DMA desde el booter prestado | IOVA `0x3B000000`, firma elegida por el fusible (la 0 de 2) | fijo |
| 2 | los parametros de la ROM (`brom`): offset de PKC, mascara de motor, ucode id, RSA-3K | del propio `boot_ld` | fijo |
| 2 | BOOTVEC, MAILBOX0/1 del SEC2 = la IOVA de la WPR meta, STARTCPU | IOVA `0x3E007000` | fijo |

### 9b. La WPR meta: los 256 bytes que el booter lee (`arranque/wpr.rs`)

La forma es `GspFwWprMeta` (nova-core y el driver abierto de NVIDIA). Valores
de la 3060 del propietario (fila `mapa` del informe):

| byte | campo | que es | valor | de donde | varia? |
|---|---|---|---|---|---|
| 0 | magic | la firma de la estructura | `0xDC3AAE21371A60B3` | constante | fijo |
| 8 | revision | la version de la forma | 1 | constante | fijo |
| 16 | sysmemAddrOfRadix3Elf | donde esta la tabla radix3 del GSP-RM | IOVA `0x3F000000` | BMO-X | fijo |
| 24 | sizeOfRadix3Elf | los bytes del GSP-RM | `0x3C99000` | el `.fwimage` | fijo |
| 32 | sysmemAddrOfBootloader | el bootloader RISC-V | IOVA `0x3E000000` | BMO-X | fijo |
| 40..64 | medida y offsets de codigo, datos y manifiesto del bootloader | lo que el RISC-V arranca | del `bootldr` | fijo |
| 72, 80 | sysmemAddrOfSignature, sizeOfSignature | la firma ga10x del GSP-RM | IOVA `0x3E006000`, 4096 B | el ELF | fijo |
| 88..104 | gspFwRsvdStart, nonWprHeap | lo de debajo de la WPR2 | `0x2F3F00000`, 1 MiB | `wpr::mapa` | fijo si la VRAM y la VGA lo son |
| 112 | gspFwWprStart | donde EXTIENDE el booter la WPR2 | `0x2F4000000` | `wpr::mapa` | fijo (y el metal lo confirma) |
| 120, 128 | gspFwHeap | el heap del GSP-RM | 128 MiB desde `0x2F4100000` | `wpr::heap` | fijo |
| 136 | gspFwOffset | donde copia el booter el ELF | `0x2FC160000` | `wpr::mapa` | fijo |
| 144 | bootBinOffset | donde va el bootloader | `0x2FFDFA000` | `wpr::mapa` | fijo |
| 152, 160 | frtsOffset, frtsSize | lo que monto FWSEC-FRTS | `0x2FFE00000`, 1 MiB | `vbios::frts` | fijo |
| 168 | gspFwWprEnd | el final de la WPR2 | `0x2FFF00000` (a 128 KiB) | `wpr::mapa` | fijo |
| 176 | fbSize | la VRAM entera | 12 GiB | `0x1183A4` (GFW) | fijo si el GFW acabo |
| 184, 192 | vgaWorkspace | la zona de la pantalla de arranque | desde `0x2FFF00000` | `info_vga` | fijo |
| **200** | **bootCount** | **lo escribe el booter/GSP** | BMO-X pone 0 | -- | **la autopsia lo lee** |
| 208..240 | la union del RPC, particiones VF, flags | sin usar en Ampere sin VF | 0 | -- | fijo |
| 244 | pmuReservedSize | la reserva de la PMU | 0 en Ampere | -- | fijo |
| **248** | **verified** | **lo escribe el booter al comprobar la imagen** | BMO-X pone 0 | -- | **la autopsia lo lee** |

**Lo que dice la tabla:** todo lo que BMO-X entrega es FIJO de un arranque a
otro -- y el metal lo confirma: el `mapa` sale identico en los arranques
buenos y en los malos. Con eso, el 0x15 NO viene de un dato mal calculado
por BMO-X, salvo que el GFW no hubiera acabado (y la fila dice `GFW 0xFF`).
Queda el estado de la tarjeta y el momento.

### 9c. Lo que ahora se apunta solo (save mode y CABINA)

Cada arranque, BUENO O MALO, sin teclear nada:

```text
   al llegar      BSI_14, el progreso del GFW, la WPR2 cruda
   autopsia       antes del booter: BSI_14
                  al pararse el SEC2: MAILBOX0/1 del SEC2 y del GSP (crudos),
                  los CPUCTL de los dos, la WPR2, los us, y
                  QUE PALABRAS DE LA WPR META CAMBIO EL BOOTER, su
                  `verified` y su `bootCount` -- HASTA DONDE LLEGO
   donde          la fila `autopsia` del informe, `datos/DATOS.TXT` (gpu
                  booter ...) y CABINA (la caja negra y el serie)
```

**Como se lee:** en un arranque bueno el booter llega al final y deja su
huella en la meta; en uno malo, si la mascara o `verified` salen distintos,
se sabe EN QUE ETAPA se paro. Eso es lo que convierte un 0x15 sin
explicacion en un "se paro antes (o despues) de verificar la imagen".
