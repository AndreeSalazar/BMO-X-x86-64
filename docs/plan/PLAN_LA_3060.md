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

### [x] E2 -- el VBLANK por INTERRUPCION: la primera escritura (visto en metal, 24-09 04:01)

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

**E2 en codigo (24-09), por el camino M0: detras de un CANDADO.** El Bus
Master de la 3060 se enciende SOLO si en ese instante la IOMMU esta encendida y
la entrada de la 3060 es BLOQUEADA, releida y de ESE BDF (M0e); si no, `NO:
EL CANDADO`. Y al cerrar igual: `iommu apagar` y `gpu ver` apagan E2 ANTES de
quitarle la venda, asi que nunca hay Bus Master con la 3060 viendo la RAM. Los
registros salen de nouveau (v6.10) para el GA106 (`nv176`): el arbol del VFN
en `0xB80000` (la pantalla es la hoja 4, bit 26), el rearme del MSI en
`0x088704`, y la pantalla de Volta (`0x611EC0` quien aviso, `0x611800` el
evento, `0x611CC0`/`0x611D80` la mascara y el encendido del VBLANK). El ORDEN
vive en `platform/drivers/gpu/ga10x/src/vblank.rs` (10 pruebas contra un banco
de registros de mentira); el kernel es el pegamento
(`Ultra_kernel_x86-64/kernel/src/ring0/dev/vblank.rs`): vector 50 instalado AL
ARRANCAR (la IDT no se alcanza desde un syscall), MSI al LAPIC del BSP, el Bus
Master por `pci::enable_mem_bus_master` (el portero la adopta), y desde la
interrupcion solo MMIO: mas de 1.000 avisos en un segundo es una TORMENTA y la
cima se calla sola. `INFO_GPU_VBLANK` (0xAC) cuenta, `INFO_GPU_E2` (0xAD) es la
ESCALERA (vector, ciega, MSI, BME, aviso, evento, hoja, cima): en el metal, el
primer peldano en `-` dice donde se quedo el aviso.

```text
   gpu vblank        encender (save antes, FLUSH del disco) y contar 500 ms
   gpu vblank off    quitar el aviso y retirar el Bus Master
   save mode         iommu -> gpu -> e2, cada uno con su save; e2 escucha
                     300 ms y si no llega ningun VBLANK el paso NO salio
```

**E2 en el metal (24-09, 04:01): la 3060 AVISO.** `e2  4866 VBLANKs por
interrupcion, 4866 entradas al vector 50` -- cada entrada fue un VBLANK de la
cabeza 0, ni un aviso ajeno, ni tormenta, ni otra cabeza (~81 s a 60 Hz).
`iommu`: eventos 0 con el Bus Master encendido, o sea que la 3060 no intento ni
un DMA, y el MSI paso la entrada BLOQUEADA con IV=0 como decia M0e. Despues
`gpu vblank off`: `BME- aviso- hoja- cima-` y `(apagado por orden)`. El
`evento+` que queda tras apagar es el bit del VBLANK que la cabeza sigue
levantando y ya nadie limpia: sin `aviso` no llega a la hoja.

Un solo ruido: `[!] pci MSI armado en un aparato SIN maestro de bus =104` en
CABINA -- el orden era MSI y despues Bus Master. Desde el 24-09 el Bus Master va
PRIMERO (con la cima callada, el MSI apagado y la 3060 ciega no abre nada) y el
aviso ya no sale en una orden que sale bien.

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

** Elegido por el propietario el 23-09 (antes que un BME sin IOMMU). Por
pasos, cada uno visto en el metal antes del siguiente:

```text
   M0a  PREGUNTAR (solo lee): el IVHD elegido, a quien atiende (el mayor BDF
        mide la tabla), alias, IOAPIC/HPET, los IVMD, y los registros --
        la dejo el firmware encendida? que sabe (EFR)?      [en codigo]
   M0b  las tablas en RAM, armadas y probadas en el anfitrion: tabla de
        dispositivos, cola de ordenes, registro de eventos. Sin encender
                                              [VISTO en metal, 24-09 01:45]
   M0c  ENCENDER sin traducir: toda entrada valida y de paso (TV + IR +
        IW, modo 0). No cambia nada para los aparatos; prueba que la cola
        de ordenes da la vuelta (COMPLETION_WAIT) y que el registro de
        eventos queda a 0. POR ORDEN (`iommu encender`), no al arrancar:
        save automatico antes, FLUSH del disco, y si el dato no vuelve en
        10 ms se apaga sola                  [VISTO en metal, 24-09 02:12]
   M0d  TRADUCIR por aparato: cada uno ve SOLO lo que se le presta. Un
        fallo = un EVENTO con su BDF, no memoria pisada. Por la 3060
        primero, que es el camino del GSP (24-09):
          M0d1  las tablas de pagina (formato v1 de Linux) y el ORACULO
                que las recorre como la IOMMU, en el anfitrion   [en codigo]
          M0d2  `gpu traducir`: la 3060 TRADUCIDA por su dominio, vacio;
                `gpu prestar`: una pagina de prueba, solo lectura, en
                0x10000000, releida por el oraculo; y la fila `event`
                de `iommu`                                      [en codigo]
          M0d3  LA PRUEBA DE FUEGO: el DMA de un falcon de la 3060 LEE la
                pagina prestada (y una NO prestada sale como FALLO DE
                PAGINA con su BDF). Es el primer ladrillo de L0
                                              [VISTO en metal, 24-09 04:43]
          M0d4  disco, USB y red traducidos: ven solo lo que el juez de
                DMA (R-DMA) les presto; los IVMD, en identidad
   M0e  la 3060 CIEGA: su entrada BLOQUEADA (V + TV, sin IR ni IW), las
        palabras de interrupcion conservadas -- el MSI pasa, el DMA no.
        `gpu cegar` / `gpu ver`, o `save mode`             [en codigo]
        Y entonces su Bus Master, y E2
```

M0a vive en `platform/shared/bmo-firmware/src/ivrs.rs` (el IVRS entero,
5 pruebas), `platform/drivers/iommu/amdvi` (los registros, 6 pruebas),
`Ultra_kernel_x86-64/kernel/src/ring0/plat/iommu.rs` e `INFO_IOMMU_*`
(0xA0-0xA7), con la orden `iommu` y su cuadro en el capitulo 2 del `save`.
De paso arreglo un fallo latente: `leer_ivrs` contaba los IVMD como IOMMUs.

**M0a en el metal (24-09, 01:08):** APAGADA por el firmware (nada que
heredar), IVHD 0x11 en `0xFD500000` con BDF `00:00.2`, 6 niveles de pagina,
NX, GT, PPR, INVALIDAR-TODO y SIN GA (el remapeo ira con el formato de 32
bits); todo el bus (tabla de 2 MiB); IOAPIC `0x0D` en `00:14.0` (banderas
`0xD7`), IOAPIC `0x0E` en `00:00.1`, HPET en `00:14.0`; ningun IVMD.
Una nota vieja (07-09) decia `0x10 en 0xFEB80000`; `placa` del 24-09 dice
`0xFD500000` igual que `iommu`: los dos bloques coinciden y la nota era de
otro firmware. Y `placa` desde el escritorio tumbaba el kernel (`c16756afe`).

**M0b en el metal (24-09, 01:45):** `ours  tabla de 2048 KiB en 0x02829000,
todo DE PASO; 1 con banderas del IVHD; colas de 512 en 0x02A29000  releida
igual, SIN ENTREGAR`. La entrada con banderas es la del IOAPIC `0x0D`
(`00:14.0`, `0xD7`).

**M0c en el metal (24-09, 02:12):** `live  ENCENDIDA; el COMPLETION_WAIT
volvio en 6 us; eventos 0 (1 intento)`, control `0x...1405` (EN + eventos +
ordenes sobre lo del firmware). DOOM a 70 fps, el audio sin un tiron, el
teclado, el raton y el disco igual que antes: con todo de paso, ningun
aparato noto nada. Y el MSI del disco (vector 49) siguio entrando: con IV=0
las interrupciones pasan sin remapear.

**M0e y `save mode` (24-09, en codigo):** la entrada de la 3060 (el BDF lo da
la sonda de `dev/gpu.rs`, y el kernel vuelve a mirar que ahi haya un `10DE`
antes de tocarla) pasa a BLOQUEADA con la palabra 0 escrita la ULTIMA,
`INVALIDATE_DEVTAB_ENTRY` + `INVALIDATE_ALL` + `COMPLETION_WAIT` (cada uno
con su propio dato: `plat/iommu.rs::mandar`), y se relee. Si la invalidacion
no vuelve, la entrada vuelve atras. Por peticion del propietario, **`save
mode`** es la VERIFICACION TOTAL (`director/src/commands/verificar.rs`):
todos los pasos en orden -- hoy `iommu` y `gpu` --, un `save` antes de cada
uno, los que ya estan no se repiten, uno que pide otro no se intenta, y al
final NOTAS Y CONSEJOS. `save mode -gpu` quita ese paso; E2 sera una fila
mas.

**El modo ARMADO y el CONSEJERO (24-09, `b4da05862`, en codigo).** El
propietario pidio que la verificacion se automatice "en caso que la PC se
reinicie o kernel fault". `save mode [-paso]` corre y queda ARMADO en
`datos/modo.txt`; antes de cada paso se escribe `en curso: X` (y el kernel
vacia el disco antes de escribir en la IOMMU), y al acabar se borra. El
arranque del escritorio lo lee: si encuentra un `en curso`, ese paso TUMBO la
maquina, se quita solo, queda `tumbo: X` y se repite el resto. Repetir a
ciegas seria un bucle de caidas; con la memoria, cada caida quita un paso.
`save mode off` lo desarma. El CONSEJERO (Ctrl+Alt, al arrancar y tras cada
`save mode`) dice UNA cosa: el paso que tumbo, el siguiente sin hacer o
"todo verificado -> E2", y si el modo esta armado.

**M0d1 y M0d2 (24-09, en codigo).** `platform/drivers/iommu/amdvi/src/paginas.rs`
(11 pruebas): directorio `PR|IR|IW|nivel<<9|tabla`, hoja de 4 KiB
`PR|FC|IR|IW|fisica`, 3 niveles (512 GiB de espacio del aparato), prestar todo o
nada, quitar, y el ORACULO que recorre la tabla como la IOMMU (los permisos son
el AND de los peldanos). Y `INVALIDATE_IOMMU_PAGES` del dominio entero. En el
kernel (`plat/iommu.rs`): un area NEUTRO de 128 paginas para las tablas (fila
IOMMU del censo, x3), el dominio 3 de la 3060, y la entrada TRADUCIDA con las
palabras de interrupcion conservadas -- E2 sigue, y su candado acepta ciega o
traducida. Lo prestado vive en `dev/gpu_prestamo.rs`, la fila GPU del censo
NEUTRO: hoy la pagina de prueba (`PATRON | i` por palabra), luego el firmware
del GSP. `save mode` suma dos pasos, `traducir` y `prestar`. **Como se sabe:**
`iommu` dice `3060 TRADUCIDA`, `domain ... 1 pagina(s) prestada(s)`, sin fila
`event`; y `gpu` con la fila `e2` subiendo igual.

**M0d2 en el metal (24-09, 04:26):** `3060 TRADUCIDA (M0d)`, `domain ... 1
pagina(s) prestada(s) tablas 3 de 128; prueba en 0x10000000 -> 0x022BE000`, sin
fila `event`, y la fila `e2` subiendo con la entrada traducida (2894 -> 4168):
el MSI pasa por la entrada TRADUCIDA igual que por la ciega.

**M0d3 (24-09, en codigo): LA PRUEBA DE FUEGO.** El falcon del GSP (`0x110000`)
se resetea como nova-core (`ENGINE`, borrado de memoria, nucleo FALCON, BOOT_0
en `RM`), su FBIF va a la RAM del PC en fisico y sin contexto, y 16 trozos de
`DMATRF` de 256 B traen `0x10000000` -- la direccion del APARATO, la que
traduce la IOMMU -- a su DMEM, que se lee por PIO y se compara con el patron.
Sin arrancar su procesador ni cargar nada firmado, y al acabar se resetea otra
vez. `gpu fuego` (HECHO = 1024 de 1024 y ningun evento) y `gpu frontera` (lo
mismo desde `0x20000000`, NO prestada: HECHO = un evento con el BDF de la 3060 y
esa direccion). El ORDEN vive en `platform/drivers/gpu/ga10x/src/falcon.rs`, con
6 pruebas contra un falcon de mentira que copia de verdad; los numeros son los
de nova-core (Linux 6.17) y nouveau (6.10). Pide la 3060 TRADUCIDA, la pagina
prestada y el Bus Master de E2 -- no lo enciende: tiene un solo propietario. Son los
fallos 1, 2 y 3 de FastOS, cada uno en su linea. `save mode` suma `fuego` y
`frontera`.

**M0d3 en el metal (24-09, 04:43): LA 3060 LEYO LA RAM DEL PC, Y SOLO LO
PRESTADO.** `fuego  la 3060 LEYO la pagina prestada: 1024 de 1024 palabras, por
el DMA del falcon del GSP   DMEM 64 KiB, seguridad 3, DMA 48 us, eventos nuevos
0` y `frontera  AGUANTA ... tipo del evento 2`, con `iommu`: `event  FALLO de
pagina de un aparato   BDF 29:00.0 direccion 0x0000000020000000`. Los cuatro
pasos que tumbaron a FastOS en el SEC2 --reset, FBIF, direcciones, firma--
pasaron aqui en el falcon del GSP, sin firma porque no se ejecuto nada, y detras
de la IOMMU. Nada se cayo, y E2 siguio contando (6250 VBLANKs).

**M0b** (`platform/drivers/iommu/amdvi/src/tablas.rs`, 7 pruebas; y
`bmo_firmware::ivrs::por_entrada`): la entrada en sus tres formas (bloqueada,
de paso, traducida), las banderas del IVHD con la errata 63, los registros
que las entregan, COMPLETION_WAIT / INVALIDATE_DEVTAB_ENTRY /
INVALIDATE_IOMMU_ALL, el anillo con el hueco de 0x20 de Linux, y los eventos
con nombre. El kernel las ARMA al arrancar (`plat/iommu.rs::armar`: 2 MiB
contiguos NEUTRO + 16 KiB de colas, todo de paso, releido) y NO toca un
registro; la fila `ours` de `iommu` dice si quedaron iguales. La IOMMU entra
en el censo del NEUTRO como cuarto aparato: lee sus tablas y escribe sus
eventos por DMA.

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

** Por escalones (24-09), cada uno visto en el metal antes del siguiente:

```text
   L0a  PREGUNTAR (solo lectura): la VBIOS entera por la ventana PROM
        (BAR0 + 0x300000), de 8 en 8 bytes desde el escritorio; sus
        imagenes, el BIT, la tabla de la PMU, el descriptor v3 de FWSEC,
        la DMEMMAPPER (donde va la orden FRTS), el fusible del motor y la
        firma que pide, la VRAM, donde ira FRTS y si ya hay WPR2. La ROM
        queda en datos/vbios.rom                               [en codigo]
   L0b  CORRER FWSEC-FRTS en el falcon del GSP: su ucode PRESTADO por la
        IOMMU (como la pagina de M0d3), la orden cambiada a FRTS, la firma
        del fusible puesta, IMEM y DMEM por DMA, BROM, arrancar, y MAILBOX0
        a 0. Como se sabe: la fila `wpr2` dice YA montada
   L0c  el booter en el SEC2 y el GSP-RM (los ~69 MB), por el mismo camino
```

L0a vive en `platform/drivers/gpu/ga10x/src/vbios.rs` (9 pruebas contra una
ROM de mentira armada byte a byte, y bytes hostiles que nunca la revientan),
`dev/gpu.rs` (`INFO_GPU_ROM/FUSIBLE/FB/VGA/WPR2`, todo lectura) y
`director/src/commands/vbios.rs` (`gpu vbios`, y el paso `vbios` de `save mode`).

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
