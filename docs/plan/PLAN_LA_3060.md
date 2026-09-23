# PLAN LA 3060 -- la grafica que ya hay, de la sonda al GSP

> Escrito el **2026-09-23**, el dia que el Ryzen contesto `SE PUEDE`: la linea que
> barre la RTX 3060 (GA106) se lee por MMIO SIN firmware (`gpu`, 15:22).
>
> El propietario: *"no quiero gastar dinero en GPU que ya tengo"*, y el orden lo
> decidio el: **Early** sin firmware, **Mid** con la GPU leyendo memoria (y
> antes la IOMMU), **Late** el GSP y solo detras de la IOMMU.
>
> El porque de cada decision --lo legal, lo que dejo FastOS, la soberania-- esta
> en [`../maestro/GPU_NVIDIA_MAESTRO.md`](../maestro/GPU_NVIDIA_MAESTRO.md),
> seccion 6b. Aqui solo las casillas: que falta, que lo bloquea, como se sabe.

---

## 0. La regla de las tres etapas

```text
   EARLY   la CPU LEE y escribe registros. La GPU no toca la RAM del PC.
           No hace falta IOMMU: no hay DMA de la tarjeta
   MID     la GPU LEE memoria por su cuenta (DMA): pushbuffers, tablas,
           copias. ANTES, la IOMMU -- o todo dentro de la VRAM (BAR1)
   LATE    el firmware CERRADO del GSP (~69 MB, firmado). Detras de la IOMMU,
           como fila NEUTRO declarada. Nunca en Ring 0
```

La frontera entre etapas no es de dificultad: es **quien lee la memoria de
quien**. Por eso el orden no se salta.

---

## 1. EARLY -- sin firmware, sin DMA

### [x] E0 -- la SONDA: identidad, modo y la linea que barre (2026-09-23)

`platform/drivers/gpu/ga10x` (puro, 9 pruebas) + `Ultra_kernel_x86-64/kernel/src/ring0/dev/gpu.rs`
(solo lee) + `INFO_GPU_*` 0x9A-0x9E + la orden `gpu` y el capitulo 2 del `save`.

**Como se sabe:** el `save` del 23-09 a las 15:22: `GA106 (Ampere) rev A1`,
`dicho 60.000 Hz / MEDIDO 60.001 Hz`, `verdict SE PUEDE`. Y el metal corrigio
una linea de la sonda (`9a0eadeb0`: el registro de inicio del borrado es la
ultima VISIBLE).

### [ ] E1 -- volcar DETRAS del rayo (2026-09-23, en codigo)

`Ultra_userspace/userland/src/sin_gpu/rayo.rs` + `INFO_GPU_ESPERA` (0x9F): antes
de copiar cada caja, el kernel dice cuanto falta para que el rayo no la barre a
medio copiar (`bmo_gpu_ga10x::Modo::espera`); lo que cuesta copiar una fila lo
mide quien copia. Mas de 1 ms se duerme por el latido; menos, se gira.

**Como se sabe:** la fila `compose` de `gpu`: cajas preguntadas, cuantas
esperaron y cuanto, y `NO CABEN` -- las que ni esperando se libran (esas solo
las arregla M2). Y a ojo: arrastrar una ventana sin ver el cuadro partido.

** Lo que dijo el metal (23-09, 15:52 y 16:05), y lo que cambio:

- La fila `compose` sale y el rayo se lee en vivo. Pero con la primera cuenta
  el 16 % de las cajas esperaba (4,2 ms de media, 8,9 s parados de 74).
- Medido POR PIXEL (`00b3aea34`): 3.371-5.685 ps. Una fila de 1920 son
  6,5-10,9 us y el rayo barre una linea en 14,8: **la copia es mas rapida
  que el rayo**. La pantalla entera son 7-12 ms: NO cabe en el VBLANK (666
  us), pero no hace falta -- empezando detras del VBLANK le gana la carrera.
- E1b: `Modo::espera` pide al rayo VENTAJA, no que la copia acabe antes de
  que llegue; y quien copia vuelve a preguntar tras cada espera. Falta verlo:
  `NO CABEN` a 0 y muchas menos esperas.

### [ ] E2 -- el VBLANK por INTERRUPCION: la primera escritura

Encender el aviso de VBLANK de la cabeza (`0x611d80 + 4*cabeza`, bit 2, de
`nvkm/engine/disp/gv100.c`) y el MSI de la GPU, y atenderlo en `0x611800`. Con
eso el compositor DUERME con `WAIT` hasta el VBLANK en vez de preguntar la
linea. Es la **primera escritura** en la tarjeta: se decide aparte.

> [!] **Encontrado el 23-09: E2 por MSI NO es Early.** Un MSI es una
> ESCRITURA que hace la tarjeta (a `0xFEE.....`), y para eso el aparato
> necesita el Bus Master (BME, bit 2 del Command). La 3060 lo tiene
> APAGADO: el portero (`dev/portero/roja.rs`) solo perdona a los puentes y
> ella no sale entre los `ajenos`. Encenderlo es darle DMA a TODA la RAM --
> lo que la seccion 0 pone en MID y detras de la IOMMU. Los caminos:
>
> ```text
>    MSI con BME, sin IOMMU     rompe la regla de las etapas; los motores
>                               estan parados, pero la puerta queda abierta
>    INTx (la patilla)          sin BME, pero su ruta al IOAPIC la da el
>                               _PRT, que es AML: y AML en Ring 0, NUNCA
>    sin interrupcion           lo de hoy: dormir por el latido y girar el
>                               ultimo milisegundo. Cero escrituras
>    M0 primero                 la IOMMU confina el DMA Y remapea el MSI;
>                               entonces E2 es seguro
> ```
>
> Lo decide el propietario. Mientras, E1b ya no necesita E2 para no partir.

**Bloquea:** E1 visto en el metal, y la decision de arriba. **Como se sabe:**
`gpu` cuenta VBLANKs por interrupcion y suben ~60 por segundo.

### [ ] E3 -- el compositor al compas de la pantalla

Pintar un cuadro por VBLANK y no mas: hoy el escritorio da ~375 vueltas por
segundo y pintan 8-19 (`save`, 23-09). La entrada sigue a su ritmo; lo que se
ata a la pantalla es PINTAR.

**Bloquea:** E2. **Como se sabe:** `pintan /s` en el consumo del `save` no pasa
de 60, y los vatios en reposo no suben.

---

## 2. MID -- la GPU lee memoria (DMA)

### [ ] M0 -- la IOMMU (AMD-Vi) encendida: el NEUTRO pasa de censo a frontera

Sin ella, cualquier aparato que haga DMA ve TODA la RAM (`NEUTRO/CENSO.txt`, la
fila `GPU+PSP`). Se enciende con tablas propias y cada aparato ve solo lo que
BMO-X le presta -- y protege tambien del disco, el USB y la red, no solo de la
grafica. `placa` ya lee el IVRS (`Ultra_kernel_x86-64/kernel/src/ring0/plat/placa.rs`).

**Como se sabe:** `placa` dice la IOMMU ACTIVA y el censo NEUTRO marca cada
aparato confinado.

> [!] Hay un atajo medido para no bloquear M2 en M0: si los pushbuffers y las
> superficies viven en la VRAM (por BAR1), la GPU no lee la RAM del PC. Se
> decide al llegar, con el coste de cada camino escrito.

### [ ] M1 -- BAR1: la VRAM vista desde la CPU

BAR1 es de 64 bits y esta por encima de 4 GiB (FastOS la leyo MEDIA: su T13 de
12/15). Mapearla fuera del physmap, que solo cubre 0..16 GiB. Es la puerta a
poner cosas DENTRO de la tarjeta.

**Como se sabe:** `gpu` dice la base de BAR1 entera y su medida, y una escritura
de prueba en VRAM se lee de vuelta.

### [ ] M2 -- PAGE FLIP: cero copias, y `sin_gpu/` se BORRA

Los canales de pantalla (core y window, `nvkm/engine/disp/ga102.c`): cambiar la
superficie que lee el escaner en el VBLANK. Es el escalon 8 de
`docs/identidad/LA_RAM.md`, y el dia que funcione se borra
`Ultra_userspace/userland/src/sin_gpu/` entera -- es la regla de esa carpeta.

**Bloquea:** M0 o el atajo de VRAM, y M1. **Como se sabe:** `volcado` mueve 0
bytes por cuadro y `NO CABEN` desaparece: sin copia no hay cuadro partido.

### [ ] M3 -- el MOTOR DE COPIA (CE)

Tablas de paginas de la GPU (GMMU), instancia, runlist, canal y los metodos de
copia (`nvkm/engine/fifo/ga102.c`, `nvkm/engine/ce/ga102.c`, sin firmware). La
GPU mueve los pixeles: el escalado de DOOM (`expansion`, el 80 % del blit) deja
de ser trabajo del CPU.

**Bloquea:** M0. **Como se sabe:** el `[perf]` de DOOM da `expansion` cerca de
0 us del CPU, y el `save` cuenta las copias hechas por el CE.

### [ ] M5 -- el motor 3D y el de COMPUTO, SIN el GSP (por ACR, a relojes de arranque)

** Corregido el 23-09: la primera version de este plan ponia el 3D en Late,
detras del GSP. **No es asi.** nouveau acelera Ampere SIN el GSP desde Linux
6.2: el motor grafico (GR) arranca con firmware CHICO firmado por Nvidia
(`acr/ucode_ahesasc`, FECS y GPCCS, de `linux-firmware`), cargado por SEC2 --
el mismo SEC2 donde murio FastOS. Lo que el GSP da y esto NO son los relojes:
sin el, la 3060 va a los de arranque, que son bajos.

Lo que ya hay publicado para no adivinar:

```text
   la clase 3D de Ampere      clc797.h (AMPERE_B, lista el GA106), de Nvidia
   el despacho de computo     las cabeceras QMD, de Nvidia (open-gpu-doc)
   el codigo maquina (SASS)   NAK, el compilador en Rust de NVK (MIT): se LEE
   el frente                  PLAN_EL_SOMBREADOR.md: SPIR-V ya se lee, se
                              juzga y se ejecuta; falta el emisor de SASS
```

Los escalones, cada uno con su prueba:

```text
   M5a  SEC2 arranca el ACR con la firma por FUSIBLE     (el muro de FastOS)
   M5b  GR vivo: FECS y GPCCS cargados, contexto de oro
   M5c  un despacho de COMPUTO: un QMD y un sombreador SASS que suma
   M5d  SPIR-V -> SASS (SM86): el emisor, probado en el anfitrion contra
        codificaciones conocidas, como el de x86-64 contra su oraculo
   M5e  UN TRIANGULO 3D con AMPERE_B en la pantalla
```

**Bloquea:** M0 (el GR hace DMA), M3 (canales). **Como se sabe:** un
sombreador del BSF (`platform/drivers/gpu/` cuando exista) corre EN LA 3060 y
da los mismos bits que el oraculo del SOMBREADOR; y el triangulo se ve.

> [!] Sigue siendo firmware de Nvidia corriendo en SUS microcontroladores:
> chico, firmado, fila NEUTRO. Distinto del GSP en medida (KB contra 69 MB)
> y en lo que ve (el GR, no la tarjeta entera), no en su naturaleza.

### [ ] M4 -- cambiar de modo (resolucion y refresco)

Solo si hace falta: hoy el 1080p a 60 del GOP sirve. Es lo que permitiria el
144 Hz de un monitor que lo tenga.

**Bloquea:** M2. **Como se sabe:** `gpu` dice el modo nuevo con el refresco
MEDIDO, no el dicho.

---

## 3. LATE -- el firmware cerrado, detras de la IOMMU

### [ ] L0 -- SEC2 arranca el booter, y el GSP-RM corre

Los cuatro fallos que tumbaron a FastOS, leidos en su `loader.rs` (sigue en el
git; se borro en `0e43d7f34`) y escritos en `GPU_NVIDIA_MAESTRO.md` 6b: reset
por el registro de MOTOR y esperar el borrado de memoria, programar el FBIF,
direcciones FISICAS a la DMA, y la firma por el FUSIBLE
(`FUSE_OPT_FPF_SEC2_UCODE1_VERSION`), no por el fichero.

**Bloquea:** M0 -- sin IOMMU el GSP ve toda la RAM, y eso no se hace. **Como se
sabe:** el RISC-V del GSP responde en su buzon y las colas de mensajes dan la
vuelta.

### [ ] L1 -- hablar con el GSP-RM (RPC)

Las colas en memoria compartida, los argumentos de libos, el registro y la
informacion del sistema. Atado a UNA version del firmware: el protocolo cambia
con cada una.

**Bloquea:** L0. **Como se sabe:** el GSP contesta una RPC con la version que
dice ser.

### [ ] L2 -- lo que SOLO el GSP da: los RELOJES

El 3D y el computo NO estan aqui: son M5, sin GSP. Lo que solo trae el GSP es
subir la 3060 de los relojes de arranque a los suyos (y la gestion de
energia). **Lo que si son anios**, dicho con su nombre: un Vulkan COMPLETO y
conforme (el CTS de Khronos) que corra juegos de otros -- NVK tardo ~2 anios
con varios expertos. Un Vulkan que corra LO DE ESTA CASA no es eso.

**Bloquea:** L1. **Como se sabe:** `gpu` dice los relojes de la 3060 por
encima de los de arranque, y el mismo sombreador de M5 va mas rapido.

---

## 4. Lo que NO se hace nunca

- Cargar el GSP sin la IOMMU encendida.
- Firmware de terceros en Ring 0 (ver `docs/identidad/` y CODEOWNERS): el GSP
  corre en SU procesador; BMO-X solo le presta memoria y habla por colas.
- Comprar otra tarjeta para esquivar esto: la 3060 no bloquea nada de lo que se
  esta construyendo.
