# PLAN GPU COMO LINUX -- aprender de como arranca la 3060 en Linux, sin traer Linux

> Pedido por el propietario (2026-10-20), despues de que ULTRA LinuX
> (`AndreeSalazar/ULTRA-LinuX`, su otro repositorio) arrancara la RTX 3060
> "normal": *"inspirarte como se hizo y aislar ... empieza con eso algo
> simple que pueda mejorar ... y en consecuencia ya no me peleo con 0x15"*.
>
> **Lo que afirma**: que la misma tarjeta, en la misma placa, arranca su GSP
> con el driver de NVIDIA, y que lo que hace NVIDIA se puede GRABAR en esa
> maquina y comparar con lo que hace BMO-X, hasta encontrar la diferencia que
> decide el `0x15` del booter (G0 de [`PLAN_LA_3060.md`](PLAN_LA_3060.md)).
>
> **Como se cae**: si Linux tambien falla en frio (entonces no es la
> secuencia); si la traza no llega al booter; o si se copia una secuencia
> entera "porque en Linux va" sin saber cual de sus pasos es el que cuenta.

---

# 1. Lo que hizo ULTRA LinuX, leido de su arbol

| | ULTRA LinuX (19-10) | BMO-X |
|---|---|---|
| quien despierta el GSP | el driver de NVIDIA, `nvidia-open` **615.78.08** (sus modulos abiertos), compilado contra el kernel 7.3-rc7 propio (`scripts/build-baseline.sh`) | su propio driver (`platform/drivers/gpu/ga10x`) |
| el firmware | `gsp.bin` de la MISMA version, 615.78.08, en `firmware_class.path` | GSP-RM y booter **570.144** (`GSP_570144_EVIDENCIA.md`) |
| la linea del kernel | `nvidia-drm.modeset=1 nvidia-drm.fbdev=1` | -- |
| lo que quedo medido | `GPU Firmware: 615.78.08`, la 3060 con 12288 MiB, P8 a 53 C y 19 W, sin errores NVRM en el dmesg (`evidencias/pc-real/nvidia-2026-10-19/`) | el `0x15` en frio, 1 de 13 buenos desde el 29-09 (`DESPERTAR_FIABLE.md`) |

[!] **Lo que todavia NO dice ULTRA**: cuantas veces de cuantas. Hay pocos
arranques con NVIDIA grabados; "arranca normal" es una corrida, no diez.

# 2. La regla del aislamiento

**De Linux entra DATO, nunca codigo.** Una traza de la tarjeta del
propietario, una tabla, un numero. BMO-X no enlaza nada de Linux ni de
NVIDIA, y sus pasos se escriben con sus propias palabras y sus propias
pruebas. OpenRM (MIT) se LEE como referencia, como hasta hoy.

# 3. Las casillas

- [ ] **GL0 -- LA LINEA BASE en ULTRA LinuX (del propietario, sin codigo de
  BMO-X).** Diez arranques EN FRIO (apagar y esperar 15-30 s; no reiniciar),
  y en cada uno `dmesg | grep NVRM` y
  `cat /proc/driver/nvidia/gpus/*/information`.
  - **Como se sabe:** la tabla de los diez aqui. Si los diez arrancan: la
    placa, la fuente, ErP y Windows quedan descartados, y el `0x15` es de la
    secuencia de BMO-X. Si alguno falla: el problema no es de BMO-X solo, y
    se mira el `NVRM:` de ese.
- [ ] **GL1 -- LA TRAZA (del propietario, en ULTRA).** `mmiotrace` del kernel
  graba cada lectura y escritura del driver en la tarjeta:
  ```text
     el kernel     CONFIG_FTRACE=y, CONFIG_MMIOTRACE=y (hoy solo esta
                   HAVE_MMIOTRACE_SUPPORT); y que `nvidia` NO se cargue solo
                   al arrancar
     antes         echo 262144 > /sys/kernel/tracing/buffer_size_kb
                   echo mmiotrace > /sys/kernel/tracing/current_tracer
                   cat /sys/kernel/tracing/trace_pipe > /ruta/traza.txt &
     el driver     modprobe nvidia; nvidia-smi   (para que despierte el GSP)
     despues       echo nop > /sys/kernel/tracing/current_tracer
  ```
  `mmiotrace` deja un solo nucleo encendido mientras graba: es lento, y es
  lo que hace la traza exacta. En frio, como GL0.
  - **Como se sabe:** `traza-gpu receta traza.txt` dice "hasta que el SEC2
    arranca (el booter)".
- [x] **GL2 -- LA HERRAMIENTA (20-10).** `toolchain/tools/traza-gpu`: lee
  la traza, encuentra la BAR0 de la NVIDIA (el `PCIDEV` 10de, o el `MAP` de
  16 MiB), nombra cada registro como lo nombra BMO-X (`arranque/falcon.rs`:
  GSP `0x110000`, SEC2 `0x840000`, CPUCTL, MAILBOX0/1, BCR_CTRL,
  RISCV_CPUCTL...), la corta en la escritura que arranca el SEC2 (CPUCTL con
  STARTCPU: ahi suelta NVIDIA al booter) y compara dos recetas -- solo las
  escrituras de los falcons, la BSI, la WPR y los fusibles, sin el tiempo --
  hasta la primera diferencia.
  - **Como se sabe:** 3 pruebas sobre una traza SINTETICA (dicho asi: no es
    de la 3060). La de verdad la da GL1.
  ```text
     traza-gpu receta  traza.txt
     traza-gpu compara buena.txt mala.txt [--lecturas]
  ```
- [ ] **GL3 -- LA COMPARACION con BMO-X.** La receta de NVIDIA, paso a paso,
  contra lo que hace BMO-X hasta soltar al booter (`arranque/booter.rs`,
  `falcon.rs`, `gpu_despertar.rs`), escrita aqui como una lista de
  diferencias. La primera pregunta es **G0-MODO**
  (`ESTUDIO_RTX3060_PROTON_X.md`): BMO-X prepara el GSP como Falcon y OpenRM
  570.144 como RISC-V; en la receta se ve en las escrituras a `BCR_CTRL` y
  `RISCV_CPUCTL` del GSP antes del arranque del SEC2.
  - **Bloquea:** GL1.
  - **Como se sabe:** cada diferencia con su registro, su valor en las dos, y
    si OpenRM la explica.
- [ ] **GL4 -- VERSION O SECUENCIA.** Si GL3 no basta: la misma traza con
  `nvidia-open` 570.144 (la version del firmware de BMO-X), si compila contra
  el kernel de ULTRA. Separa "el booter de 615 arreglo algo" de "la secuencia
  de BMO-X es otra".
- [ ] **GL5 -- EL CAMBIO en BMO-X (Ring 0, del propietario).** UNA diferencia
  de GL3, la que la receta y OpenRM sostienen, en el driver de BMO-X, con su
  prueba. Nunca la secuencia entera.
  - **Bloquea:** GL3.
  - **Como se sabe:** es G0: diez arranques en frio de BMO-X con el GSP-RM
    arriba, en la tabla de `PLAN_LA_3060.md`. Entonces G0 se cierra, y el
    `0x15` deja de ser una pelea.

# 4. Por que asi, y no traer el driver de Linux

- **Traer el driver** (o arrancar Linux y pasarle la tarjeta viva a BMO-X)
  haria de BMO-X un pasajero: un GSP ya arrancado con su WPR2 arriba no se
  entrega sano entre dos sistemas, y BMO-X dejaria de saber su propia 3060.
- **Adivinar** (cambiar tiempos o resets a ciegas) ya costo: el reset PCIe
  permanente del 28-09 rompio FWSEC/WPR2 (`DESPERTAR_FIABLE.md`).
- **Grabar y comparar** es lo que BMO-X ya hace con todo lo demas (las tres
  respuestas de L31): una receta medida en ESTA tarjeta, y una sola
  diferencia cada vez.

# 5. Lo que este plan NO es

- No cambia el driver de BMO-X hasta GL5, y GL5 es del propietario.
- No modifica ULTRA LinuX: dice lo que el propietario haria alli.
- No copia firmware ni codigo de NVIDIA: la traza es de la tarjeta propia, y
  OpenRM se cita, como hasta hoy.
