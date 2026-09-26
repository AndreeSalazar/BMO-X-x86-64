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
