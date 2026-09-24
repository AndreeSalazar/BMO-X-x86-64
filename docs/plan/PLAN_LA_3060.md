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

**M5 bajo el GSP-RM (24-09): el contexto de oro, como nouveau.** Con el
GSP-RM vivo, FECS/GPCCS los carga el RM: M5a y M5b pasan a ser pedirle el
contexto de GR, la receta de `r535_gr_oneinit` / `r570_gr_get_ctxbufs_and_zcull_info`:

```text
   G0  preguntar los buferes: INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO
       (0x20800A32, 1664 B a cero) sobre las asas INTERNAS del RM (las de
       L1a); ocho buferes -- MAIN (+64 cabeceras de subcontexto), PATCH,
       BUNDLE_CB, PAGEPOOL, ATTRIBUTE_CB, RTV_CB_GLOBAL, FECS_EVENT y
       PRIV_ACCESS_MAP -- con la medida, pagina y alineacion de nouveau
       (`gpu gr`, paso `gr`)                         [VISTO 24-09 15:22]
   G1  un canal en GR0 (lista 0, la de la tabla), como el de L1d2b:
       `canal::GR` (chid 2, instancia y GPFIFO en las paginas 5 y 6 del
       tramo, USERD en el hueco 2, metodos en 0x3A01_0000), BIND GR0 y
       SCHEDULE (`gpu canalgr`, pasos `canalgr` y `encendergr`)
                                                     [VISTO 24-09 15:22]
   G2  los buferes en VRAM propia, mapeados en nuestro espacio: desde
       VRAM 0x0800_0000, en la VA 0x3_0000_0000 (la entrada 24 de la PD1
       del tramo), con una PD0 y hasta 16 PT en 0x0430_0000; los que llena
       el RM PRIMERO y a cero; paginas de 4 KiB (`gpu grmem`, paso `grmem`)
                                                     [VISTO 24-09 15:37]
   G3  PROMOTE_CTX (0x2080012B) con cada uno: MAIN 0, PATCH 2, BUNDLE_CB 3,
       PAGEPOOL 4, ATTRIBUTE_CB 5, RTV 6, FECS_EVENT 9, PRIV_ACCESS_MAP 10
       (no mapeado) y UNRESTRICTED_PRIV_ACCESS_MAP 11 con su memoria
       (`gpu oro`, paso `promover`)                  [VISTO 24-09 15:51]
   G4  AMPERE_B (0xC797) en ese canal: el RM hace el contexto de ORO
       (`gpu oro`, paso `oro`)                       [VISTO 24-09 15:51]
```

**G0 y G1 en el metal (24-09, 15:22): 34 de 34 pasos.** `gr: 8 buferes para
el contexto de oro de GR0, 26048 KiB`: MAIN 936 KiB (694016 B + 64 cabeceras),
PATCH 16, BUNDLE_CB 12, PAGEPOOL 128, ATTRIBUTE_CB 8517 KiB (alineado a 16
MiB), RTV_CB_GLOBAL 512, FECS_EVENT 64, PRIV_ACCESS_MAP 512. `canal gr:
0xF1F00002 (chid 2, GR0): NV_OK`, `atado gr: NV_OK`, `en lista gr: NV_OK`. Y
`copia` en verde con la comprobacion arreglada. G2 se probo en el anfitrion
con ESAS medidas: el reparto cabe en 25 MiB, sin solapes y alineado en VRAM
y en VA.

G3 (siguiente), `NV2080_CTRL_GPU_PROMOTE_CTX_PARAMS` de la 570.144:
`engineType, hClient, ChID, hChanClient, hObject, hVirtMemory, virtAddress,
size, entryCount` y `promoteEntry[]` de `{gpuPhysAddr, gpuVirtAddr, size,
physAttr, bufferId (u16), bInitialize (u8), bNonmapped (u8)}`; nouveau pone
`engineType 1`, el cliente y el canal, y `physAttr 4` en los que inicializa.

Los ids de PROMOTE son los de la r570 (`nvrm/gpu.h`), comprobados: una
primera version los tenia corridos en uno.

**G2 en el metal (24-09, 15:37): 35 de 35.** `gr memoria: 6382 de 6382
entradas releidas; 384 paginas a cero`. Fue la version de OCHO buferes; la de
nueve (4cba718) suma UNRESTRICTED_PRIV_ACCESS_MAP y ~512 KiB. El evento de la
IOMMU en 0x20000000 es el de `frontera`, el de siempre: no es de G2.

**G3 y G4, estudiados linea a linea en nouveau (`rm/r535/gr.c` y `fifo.c`)
y escritos (24-09):**

- **Donde va la orden**: PROMOTE_CTX sobre NUESTRO subdispositivo (el de
  `vmm->rm.device`), no el interno de G0. `engineType 1`, `hChanClient` =
  nuestro cliente, `hObject` = el canal; lo demas de la cabecera a cero.
- **Las entradas**, en el orden de la tabla (no el del reparto): `bufferId`;
  `gpuVirtAddr` en todas menos PRIV_ACCESS_MAP (`bNonmapped`: la copia
  UNRESTRICTED SI va mapeada); y en las que llena el RM, `gpuPhysAddr`,
  `size` (la de MAIN con sus 64 cabeceras) y `physAttr 4`.
- **El mapeo es PRIV** (`gf100_vmm_map_v0 { .priv = 1 }`): las PTE de G2
  llevan ahora el bit 5. Son buferes del FECS, no de un sombreador.
- **El kernel no guarda la respuesta de G0**: el escritorio le pasa las
  ocho medidas (op 0x29, una por llamada, como `TASK_OP_RUTA`), el kernel
  rehace tabla y reparto con la MISMA cuenta (`gr::desde_medidas`) y exige
  que salga lo mismo que mapeo G2; si no, no sale nada (motivo 69).
- **El contrato** (`gr::promover_permitida`) no sabe de G0 y mira la FORMA:
  nuestras asas, las nueve entradas con sus banderas, y cada direccion
  DENTRO de la region de G2, fisica y virtual a la misma distancia.
- **G4**: `nvkm_gsp_rm_alloc(chan, THREED, 0xC797, 0)` -- SIN parametros. Al
  crearlo el RM corre el contexto de oro. nouveau suelta el objeto y el canal
  (el RM guarda el oro); nosotros los dejamos: son los del triangulo.
- **Si G4 dice que no**, lo primero a mirar: nouveau pide el canal de oro
  PRIVILEGIADO (`PRIVILEGED_CHANNEL TRUE`, `internalFlags` ADMIN) y en un chid
  reservado; el nuestro es de USUARIO. Y nouveau no le hace BIND ni SCHEDULE
  antes; el nuestro ya los tiene (G1). Cambiarlo es otra forma de canal, no
  un arreglo de G3.

**EL CONTEXTO DE ORO, en el metal (24-09, 15:51): 37 de 37.** `gr: 9
buferes, 26560 KiB`; `gr memoria: 6382 de 6382 releidas; 512 paginas a
cero`; `gr oro: PROMOTE_CTX de 9 buferes: NV_OK en 10 ms` y `AMPERE_B
0xC7970002 en el canal de GR0: NV_OK en 3 ms, 0 mensajes mas del GSP`. El
canal de USUARIO basto: no hizo falta el privilegiado de nouveau. LOGRM
paso de 636 a 1036593 B en ese arranque: el RM escribio mucho en su log
durante el oro; `datos/gsplog.bin` lo tiene y se mira si S3 no sale.

S1..S3 en codigo (24-09): `bmo_gpu_ga10x::computo`, `gpu computo`, pasos
`computo`, `fichagr` y `trabajogr`. El trabajo lo paga un semaforo de
INFORME (`SET_REPORT_SEMAPHORE_*` de `clc7c0.h`), que ejecuta el FE del GR
y no el PBDMA: si se paga, el motor grafico corrio NUESTRO GPFIFO.

**S1..S3 en el metal (24-09, 16:06): 40 de 40.** `computo: 0xC7C00002: NV_OK`;
`ficha gr: 0x00000002; GR0 en la lista 0`; `gr trabajo: EL MOTOR GRAFICO
CORRIO: semaforo de informe PAGADO (0x306006C0), GP_GET 1; timbre 0x2 ... en
45 us`. El GR ejecuta NUESTRO GPFIFO con el contexto de oro.

**S4..S6, el primer sombreador, en codigo (24-09):** `bmo_gpu_ga10x::
sombreador`, `gpu sombreo`, paso `sombreo`. El SASS NO se escribio a mano:
salio de `ptxas -arch=sm_86` (CUDA 12.9, de PyPI) y se leyo con `nvdisasm -b
SM86` (13.4). Dos instrucciones de CUDA (la pila en c[0x0][0x28] y el
descriptor en c[0x0][0x118]) leen un bufer de constantes que aqui no hay:
quedaron en NOP con sus bits de planificacion, y el STG no usa descriptor (el
bit 101 a 0, comprobado cambiandolo). El QMD V03_00 campo a campo de
`clc7c0qmd.h`, con el semaforo RELEASE0 al acabar la rejilla. Cuando el BSF
emita SASS (kind 2), este programa es su primer oraculo: los mismos 160 B.

**EL PRIMER SOMBREADOR, en el metal (24-09, 16:21): 41 de 41.** `sombreo: EL
PRIMER SOMBREADOR CORRIO: 32 de 32 hilos escribieron lo suyo; semaforo del
QMD PAGADO, de informe PAGADO, GP_GET 2 en 1 us`. El SASS de `ptxas` con las
dos lecturas de CUDA en NOP corre tal cual en la 3060.

**M5d L, el lienzo, en codigo (24-09):** lo primero que se VE. 64 KiB de RAM
del PC (16 marcos NEUTRO) prestados ESCRIBIBLES en la IOVA 0x3A02_0000 y
mapeados en la VA 0x2_0001_0000 (entradas 16..31 de la PT del tramo, PTE de
SISTEMA coherente y VOL); un programa de 128 x 128 hilos (de `ptxas`, igual
que el primero) pinta un degradado; la CPU lo comprueba pixel a pixel y
`gpu lienzo` lo muestra, 4 veces mas grande, arriba a la derecha.
`bmo_gpu_ga10x::lienzo`, paso `lienzo`. El QMD y las ordenes del primer
sombreador, generalizados (`qmd_con`, `ordenes_con`).

**M5d B, el blur, en codigo (24-09):** la media de 7 x 7 de cada pixel del
lienzo (bordes repetidos), por 128 x 128 hilos, en OTRA salida de 64 KiB
(IOVA 0x3A03_0000, VA 0x2_0002_0000, entradas 32..47 de la PT del tramo). El
SASS de `ptxas` con `.pragma "nounroll"`: desenrollado eran 8.8 KiB y no cabe
en una pagina; con bucles, 71 instrucciones y 16 registros, y la division
entre 49 que emite es EXACTA. El kernel compara cada pixel con la MISMA
cuenta en la CPU (`blur::desenfocar`). En `save mode` desenfoca el degradado;
`gpu blur` sube antes un trozo de 128 x 128 de la pantalla (dos pixeles por
llamada, `blur::subir`) y muestra el antes y el despues. Se puede repetir:
cada vez, la siguiente entrada del GPFIFO de GR.

**EL LIENZO, en el metal (24-09, 16:34): 42 de 42.** `lienzo: LA 3060 PINTO EN
LA RAM DEL PC: 16384 de 16384 pixeles como tocan; semaforo del QMD PAGADO,
de informe PAGADO, GP_GET 3; IOVA 0x3A020000`. `domain` sube de 15793 a
15809 paginas prestadas: las 16 del lienzo. Y `pcie` vuelve a Gen3.

**M5d F, el fractal y el panel, en codigo (24-09):** el propietario pidio
"algo MAS fuerte que el blur" y una pantalla de configuracion como la de
NVIDIA. Mandelbrot de 512 x 512, hasta 256 vueltas por pixel, 262144 hilos
(512 bloques de 512), en 1 MiB de RAM del PC (IOVA 0x3A10_0000, VA
0x2_0003_0000, entradas 48..303 de la PT del tramo). Aritmetica ENTERA Q4.28
para que la CPU rehaga la cuenta EXACTA (con coma flotante la FMA de la 3060
y la CPU redondean distinto); el kernel cronometra la 3060 (timbre ->
semaforo) y la CPU haciendo lo mismo. `gpu fractal` (o `gpu panel`): el
panel a pantalla completa, el fractal al doble y los tiempos. SASS de
`ptxas`, 44 instrucciones, 15 registros, el bucle con su `BSSY/BSYNC`.
Un panel de CONFIGURACION de verdad (resolucion, refresco) es M4: cambiar
de modo, que hoy no hace falta y no se toca.

**EL FRACTAL, en el metal (24-09, 16:49), con fotos:** `fractal: 262144 de
262144 pixeles iguales a la CPU; la 3060 en 429 us, la CPU en 18454 us
(x43)`; la primera vez 986 us (x18): relojes subiendo y caches frias, lo
normal. `blur: 16384 de 16384` -- pero las fotos mostraron dos fallos del
ESCRITORIO, no de la 3060: (1) el panel quedaba tapado por la ventana de
Ejecutar y el panel de la izquierda, que se seguian pintando; ahora el
panel cuenta como pantalla completa (`fs` en `desktop/paint.rs`) y la
primera tecla lo cierra y devuelve el escritorio entero; (2) el blur subia
el trozo (32, 200), que es el panel de la izquierda, casi NEGRO: la 3060
desenfoco bien un cuadro negro. Ahora sube el centro del fondo.

**EL BLUR y el PANEL, en el metal (24-09, 16:56), con fotos para el README:**
el gato del fondo y, al lado, desenfocado por la 3060 (16384 de 16384); el
fractal a pantalla completa, limpio: `la 3060 en 176 us, la CPU en 18203 us
(x103)`. 44 de 44.

**M5d T0, el triangulo POR COMPUTO, en codigo (24-09):** 262144 hilos, uno por
pixel de 512 x 512, con las tres FUNCIONES DE ARISTA (lo que hace un
rasterizador) y el color de los tres vertices mezclado por su peso; entero
y exacto (las aristas suman siempre el area doble, 172800), comparado bit a
bit con la CPU. `gpu triangulo`, paso `triangulo`, el mismo panel a pantalla
completa. En el MISMO MiB que el fractal.

**EL TRIANGULO, en el metal (24-09, 17:08), con foto:** 262144 de 262144; la
3060 en 791 us y la CPU en 631: aqui gana la CPU, porque la cuenta por pixel
es ligera y lo que cuesta es llevar 1 MiB a la RAM por PCIe (Gen1 ese
arranque). El panel lo dice asi, no "0 veces". Fractal: x216.

**T1a en codigo (24-09):** la clase 3D (AMPERE_B) limpia el MiB del fractal
como destino de render de 512 x 512 (PITCH, A8R8G8B8) con `CLEAR_SURFACE`,
sin programas, y paga su semaforo tras TODAS las escrituras. Cada metodo,
de `clc797.h`. `gpu 3d`, paso `limpio3d`: "EL PIPELINE 3D ESCRIBE: SI/NO".

**M5d E, la escena 3D con luz, en codigo (24-09):** pedida por el
propietario ("3D y elementos tipicos, luz; la GPU dibuja, no la CPU"). Una
esfera con luz difusa y brillo especular, un suelo en perspectiva con
cuadros y sombra, y cielo: 262144 hilos, uno por pixel, todo ENTERO (la raiz
digito a digito, las divisiones truncando) para que la CPU rehaga la cuenta y
compare. SASS de `ptxas`, 158 instrucciones. `gpu escena`, paso `escena`.

**El mapa de SM86 (24-09):** 4096 codigos probados contra `nvdisasm`: 333
existen. Entre ellos `IPA` (0x326), `OUT` (0x324), `ISBERD` (0x923),
`PIXLD` (0x925), `KILL` (0x95b). `ALD`/`AST` (atributos) NO salen con sus
campos a cero: encontrar sus campos es lo primero de T1b.

**T1, el triangulo por el PIPELINE 3D (estudio, 24-09):** `ptxas` NO compila
programas de vertice ni de pixel (solo computo). Pero `nvdisasm -b SM86` SI
conoce sus instrucciones (probado: `IPA.PASS R0, P0, a[0x0]`), asi que se
pueden codificar a mano y comprobar contra el, como se comprobo el bit del
descriptor del STG. Falta: (1) los dos programas en SASS con su cabecera
(SPH, 20 palabras cada uno); (2) el estado 3D de AMPERE_B (destino de render
en memoria, ventana, recorte, mezcla, los programas atados); (3) los
vertices (o sacados del numero de vertice en el propio programa) y el
dibujo (BEGIN/END). Es el nivel mas grande de M5.

Tambien en ese save: `pcie: Gen1 x16 (2.5 GT/s) de Gen3 x16`, cuando los
anteriores decian Gen3. Es el enlace en reposo que el RM baja: no cambia nada
de lo que se probo, y se mira si algun dia la copia o el lienzo van lentos.

**Del oro al primer sombreador (M5d, contado paso a paso, 24-09).** Cada
fila es un paso de `save mode`, con su prueba en el anfitrion y su fila en
`gpu`; ninguno se junta con otro, como la copia (L1d):

```text
   S1  computo   AMPERE_COMPUTE_B (0xC7C0) en el canal de GR0, otro subcanal;
                 sin parametros como AMPERE_B
   S2  fichagr   GET_WORK_SUBMIT_TOKEN del canal de GR0; el timbre con la
                 lista 0 de la tabla (GR0): 0x0000_0002
   S3  vacio     el primer trabajo EN EL MOTOR GRAFICO: SET_OBJECT del
                 computo y un semaforo, nada mas. Si el semaforo se paga, el
                 GR corre NUESTRO GPFIFO con el contexto de oro (lo mismo que
                 `copia` probo para COPY2)
   S4  codigo    el programa en VRAM propia: el mas corto que escribe algo --
                 S2R del id del hilo, STG de una constante, EXIT -- en SASS de
                 SM86 (instrucciones de 128 bits). A mano y comprobado contra
                 el oraculo la primera vez; despues lo emite el BSF (kind 2)
   S5  qmd       el QMD v3 (256 B, el de NVK): direccion del programa, un
                 bloque de 32 hilos, la cb0 con la direccion del bufer, la
                 memoria local (`SET_SHADER_LOCAL_MEMORY_*`); lanzado con
                 `SEND_PCAS_A` (QMD >> 8) y `SEND_SIGNALING_PCAS2_B`, y un
                 semaforo detras
   S6  sombreo   leer el bufer por PRAMIN: 32 palabras con el valor. ES el
                 primer sombreador de BMO-X en la 3060
```

Seis pasos despues de `oro`: el primero NUEVO de verdad es S4 (el SASS); los
demas son la receta de la copia con otra clase. Despues, el blur (S7: el
mismo QMD con un programa que lee y escribe una imagen del marco copiada
por el canal de copia) y el triangulo (T1..: AMPERE_B con su estado 3D,
vertices, rasterizador y un RT en VRAM).

**Despues de G4 (M5d, estudio):** AMPERE_COMPUTE_B (0xC7C0) en el MISMO canal
(otro subcanal), un QMD v3 (el de NVK/`nvk_cmd_dispatch`) con el codigo SASS
en VRAM propia, `SET_SHADER_LOCAL_MEMORY*` y el lanzamiento por
`SEND_PCAS_A/B`. El primer sombreador: escribir una constante en un bufer
(se comprueba por PRAMIN como la copia); luego el blur; luego el triangulo
(AMPERE_B: vertices, rasterizador y un RT en VRAM, copiado al marco por el
canal de copia).

**`save mode`, refinado (24-09).** El propietario pidio automatizarlo del
todo. Ahora, al acabar: una linea de RESUMEN (cuantos bien, en cual se paro y
por que); las NOTAS solo de lo que no salio y el consejo del ultimo que si
(antes, los 32 consejos en cada arranque); el tiempo de cada paso en su fila;
un segundo intento para los pasos que solo PREGUNTAN (`estatica`, `objetos`,
`salud`, `espacio`, `motores`, `ficha`, `gr`); y `datos/pasos.txt`, una linea
por paso, para pegar ESE fichero en vez del informe entero.

**Por donde entra el BSF (24-09).** El sobre de `toolchain/lang/spirv/bsf`
ya esta hecho para esto: "una maquina nueva es un numero nuevo; el formato no
cambia" (`bsf::kind`). El SASS de SM86 sera un `kind` nuevo (hoy solo existe
`X86_64_SCALAR = 1`), con su `abi` (lo que el QMD y el despacho esperan en
vez de `init/main` de System V). M5d emite ESE objetivo EN EL ANFITRION, como
S4 emite x86-64; en el Ryzen, VERRANO toma el codigo del anexo `0x09` ya
traducido, comprobado por sus cinco capas, lo copia a la VRAM por el canal
(L1d3) y lo despacha. La 3060 no traduce nada al arrancar: lo que el
propietario pidio el 23-09 ("la GPU no pierde tiempo"), igual que BEF2 con
la CPU. Y la misma interfaz (`ModuleView::check`, las ranuras en el orden
del codigo) sirve para atar los buffers del QMD sin abrir el SPIR-V.

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
        queda en datos/vbios.rom                [VISTO en metal, 24-09 04:58]
   L0b  CORRER FWSEC-FRTS en el falcon del GSP: su ucode PRESTADO por la
        IOMMU (como la pagina de M0d3), la orden cambiada a FRTS, la firma
        del fusible puesta, IMEM y DMEM por DMA, BROM, arrancar, y MAILBOX0
        a 0. Como se sabe: la fila `wpr2` dice YA montada
                                                [VISTO en metal, 24-09 05:19]
   L0c0 el FIRMWARE en el disco: los cuatro de linux-firmware 570.144
        (la GA106 usa los de GA102) los baja el build UNA vez, por SHA-256,
        a BMO-externo\firmware\ y los deja en fw\gsp\ del volumen de
        datos                                   [en el build, 24-09]
   L0c1 PREGUNTAR (solo lectura): los cuatro leidos y entendidos, la firma
        del booter para el fusible del SEC2, y la VRAM repartida como
        nova-core (heap, elf, boot, wpr2). `gpu gsp` y el paso `gsp` de
        `save mode`                             [VISTO en metal, 24-09 06:02]
   L0c2 PRESTAR: el GSP-RM (15.513 paginas) y su radix3, el bootloader, la
        firma y la WPR meta, por la IOMMU; y RELEERLO entero por la radix3,
        como lo recorrera el GSP, con su BLAKE3. Sin arrancar nada. `gpu
        radix` y el paso `radix` de `save mode`  [VISTO en metal, 24-09 06:34]
   L0c3a PRESTAR PARA ESCRIBIR lo que el GSP escribe: los argumentos de
        LIBOS, sus tres logs, `rmargs`, sus dos colas y la pagina de
        vaciado (180 paginas), y seguir cada puntero por la IOMMU. Sin
        arrancar nada. `gpu libos` y el paso `libos`  [VISTO en metal, 24-09 06:51]
   L0c3b el GSP con sus argumentos en el buzon, el booter en el SEC2 con la
        WPR meta en el suyo, y el RISC-V del GSP despierta
        (`is_riscv_active`); sus logs dicen como fue. `gpu despertar` y el
        paso `despertar`                         [VISTO en metal, 24-09 07:10]
   L0c4a LEER lo que el GSP ya dijo: los mensajes de su cola, sin contestar
        ni mover un puntero. `gpu cola` y el paso `cola`  [VISTO en metal, 24-09 07:34]
   L0c4b1 VACIAR la cola del GSP: leer cada mensaje, decir lo que traen
        los NOCAT y mover el puntero de lectura de la CPU, para que el GSP
        pueda seguir hablando -- y ver que dice cuando ya cabe. `gpu
        vaciar` y el paso `vaciar`               [VISTO en metal, 24-09 08:14]
   L0c4b2 contestarle hasta su GSP_INIT_DONE, como nova-core:
     L0c4b2a ESCRIBIR en la cola de la CPU: SetSystemInfo y SetRegistry,
        ANTES de despertar (como nouveau y OpenRM). `gpu sistema` y el
        paso `sistema`                           [VISTO en metal, 24-09 08:34]
     L0c4b2b LEER el secuenciador que pide el GSP: sus ordenes, dichas
        una a una, sin correr ninguna. `gpu secuenciador` y el paso
        `secuenciador`                          [VISTO en metal, 24-09 08:59]
     L0c4b2c CORRERLO (RegWrite/Modify/Poll/Delay/Store y CORE_RESUME)
        y esperar GSP_INIT_DONE. `gpu init`, A MANO
                                                [VISTO en metal, 24-09 09:54]
     L0c4b3 la PANTALLA tras GSP_INIT_DONE: se quedo quieta (BAR1?)
     L0c4b3a devolverle BAR1 a la pantalla: la del GOP, apuntada antes
        del secuenciador. `gpu init` solo, y `gpu bar1`
                                                [VISTO en metal, 24-09 10:13]
   L0c4 las colas de mensajes y GSP_INIT_DONE: el GSP-RM contesta
```

**L0c0 (24-09).** La VBIOS traia FWSEC; el resto no. `build\firmware.ps1` baja de
linux-firmware (`nvidia/ga102/gsp/`, que es a donde apunta `nvidia/ga106/gsp`
en su WHENCE) y comprueba contra el SHA-256 escrito en el guion:

```text
   fw\gsp\boot_ld.bin   booter_load     61304 B   SEC2 ucode 3, 2 firmas de 384 B
   fw\gsp\boot_ul.bin   booter_unload   41080 B   el camino de vuelta
   fw\gsp\bootldr.bin   bootloader      24684 B   RISC-V v5, ucode 0x6000 B
   fw\gsp\gsp.bin       gsp          63571696 B   ELF RISC-V: .fwimage 0x3C99000 B
                                                  y .fwsignature_ga10x (4 KiB)
```

** **Por que la 570.144 y no la 535.113.01** (cambiado el mismo 24-09): la 535
era la `FIRMWARE_VERSION` de Linux 6.17, que solo llegaba hasta FWSEC. La WPR
meta, el heap del GSP y las colas de mensajes CAMBIAN con la version, y el
unico camino publicado entero --nova-core de Linux 7.3, `gsp/fw/r570_144`-- es
el de la 570.144. Seguir otro seria inventar la mitad.

No van al repo: son 63 MB (mas que el `.git` entero) y su licencia es la de
NVIDIA. Sin red el build sigue y dice donde dejarlos a mano.

**L0c1 (24-09, en codigo).** Tres modulos puros nuevos en `bmo_gpu_ga10x`, 16
pruebas, y las de los ficheros de verdad si `BMO_FW_GSP` apunta a ellos:

- `booter.rs`: `BinHdr` + `HsHeaderV2` + `HsLoadHeaderV2` (el booter) y
  `RmRiscvUCodeDesc` (el bootloader). [!] La firma del booter se elige
  RESTANDO (`fuse_ver` - version del fusible; fusible a 0 = la ultima), no
  contando bits como la de FWSEC.
- `elf.rs`: las secciones del GSP-RM por una `Fuente`, sin traerse el fichero:
  contra el de verdad lee menos de 8 KiB de los 63 MB.
- `wpr.rs`: el reparto de `FbRanges` y los 256 bytes de `GspFwWprMeta` (r570).
  Para esta 3060: heap 128 MiB en `0x2F4100000`, elf en `0x2FC160000`, boot en
  `0x2FFDFA000`, wpr2 `0x2F4000000..0x2FFF00000`; y 15.546 paginas de radix3.

El escritorio lee los tres chicos enteros y el GSP-RM con `Archivo::reflejar`
(la ventana de 64 KiB, sin el camino asincrono que se trae el fichero ENTERO).
El fusible del SEC2 (`0x824148`) ya se podia leer desde L0a. No cambia el
kernel. **Como se sabe:** `gpu gsp` dice cuantas paginas habra que prestar y
la fila `cuadra` dice que el frts del reparto es donde FWSEC monto la WPR2.

**L0c1 en el metal (24-09, 06:02).** `booter boot_ld 61304 B: motor 0x0001
ucode 3, 2 firmas de 384 B; IMEM 0x8900 B desde +0x0100, DMEM 0x6200 B, la
firma en DMEM+0x010`, `fusible 0x824148 = 0x00000001 -> version 1 (el fichero
dice 1); la firma buena del booter es la 0 de 2`, `bootldr ... codigo +0x1800,
datos +0x0800`, `gsp-rm .fwimage 60 MiB (0x3C99000 B) = 15513 paginas + 33 de
radix3; firma ga10x 4096 B (leidos 1315 B de el)`, `mapa wpr2
0x2F4000000..0x2FFF00000 (191 MiB)` y `cuadra ... es donde FWSEC monto la WPR2:
el reparto cuadra`. Los diez pasos de `save mode` verificados; el dominio sigue
en 33 paginas y el unico evento es el de la frontera (M0d3). Lo que dijo el
metal es lo que dijeron los ficheros en el anfitrion, byte a byte.

**L0c2 (24-09, en codigo).** `dev/gpu_gsp.rs` (fila GPU nueva del censo del
NEUTRO, x3), cuatro ordenes como FWSEC. [!] Los ficheros los abre
`syscall/op_gsp.rs` y se los da como un `Fichero`: `dev` esta DEBAJO de `fsys`
(el disco es un aparato), y la primera version, que abria fw/gsp/ desde `dev`,
la paro el guardian de capas (L8b) en el build del propietario:

```text
   PREPARAR     el kernel abre fw/gsp/ POR SU CUENTA: el bootloader por una
                ventana de 32 KiB, el ELF con `bmo_gpu_ga10x::elf` por la misma
                ventana (dos vueltas del cursor de FAT32, no una por seccion);
                pide 31 bloques de 2 MiB, 48 paginas de radix3 y 16 auxiliares
   TROZO(k)     512 KiB del .fwimage del disco a sus marcos, en orden, y su
                BLAKE3; 122 trozos, un syscall cada uno
   PRESTAR      la radix3 (`wpr::Radix3::palabra`), la WPR meta y el prestamo:
                0x3E000000 bootloader + firma + meta (la unica escribible),
                0x3F000000 la radix3, 0x40000000 el .fwimage; 0x20000000 sigue
                sin prestar (la frontera)
   COMPROBAR(k) cada pagina, nivel 0 -> 1 -> 2 -> imagen, CADA SALTO por la
                IOMMU (`iommu::ve_la_gpu`, el oraculo), y solo si la 3060 NO
                puede escribirla; y su BLAKE3
```

**Como se sabe:** tres BLAKE3 iguales -- lo copiado, lo visto por la radix3 y
el del `.fwimage` de la 570.144 calculado en el anfitrion con el mismo
`bmo-hash` (`e7856ee2b387917b...`). La fila `radix` dice `PRESTADO y la radix3
lleva al GSP-RM de la 570.144 entero`, y `iommu` ~15.600 paginas sin eventos
nuevos.

**L0c2 en el metal (24-09, 06:34).** `radix PRESTADO y la radix3 lleva al GSP-RM
de la 570.144 entero   15554 paginas; 122 de 122 trozos releidos; blake3
E7856EE2B387917B` -- el mismo BLAKE3 que el anfitrion. `domain 15587 pagina(s)
prestada(s)   tablas 38 de 128` (las 33 de antes + 15.554), y el unico evento
sigue siendo el de la frontera. La RAM usada paso de 34 a 97 MiB: los 61 MB del
GSP-RM, en marcos NEUTRO. E2 siguio contando y nada se quejo en el anillo.

**L0c3a (24-09, en codigo).** `bmo_gpu_ga10x::libos` arma los bytes de r570.144
(6 pruebas): `LibosMemoryRegionInitArgument` (32 B: `id8` = el nombre al reves,
IOVA, medida, contiguo, en la RAM del PC), la tabla de cada log desde +8,
`GSP_ARGUMENTS_CACHED` (72 B: GspMem, 129 paginas, colas en 0x1000 y 0x41000
CONTADOS TRAS la pagina de tabla, `bDmemStack` 1) y la cabecera de la cola del
CPU. `dev/gpu_libos.rs` (fila GPU del censo, x1) lo presta ESCRIBIBLE -- como
nova-core, que los hace `Coherent` --:

```text
   0x3C000000  argumentos de LIBOS    0x3C001000  rmargs    0x3C002000  vaciado
   0x3C100000  LOGINIT, LOGINTR, LOGRM (3 x 16)
   0x3D000000  GspMem: su tabla y las dos colas (129)
```

**Como se sabe:** desde los argumentos -- lo unico que el GSP recibira por su
buzon -- se sigue CADA puntero por la IOMMU: nombre, IOVA y cada pagina de cada
region, la tabla dentro de cada log, `rmargs`, las 129 entradas de GspMem y la
cabecera de la cola. Todos llevan a los marcos pedidos y la 3060 puede
escribirlos; la fila `libos` dice cuantos punteros se siguieron. La pagina de
vaciado se presta ya y se registra en L0c3b.

**L0c3a en el metal (24-09, 06:51).** `libos PRESTADO para escribir, y cada
puntero del GSP lleva a lo suyo   180 paginas (argumentos, rmargs, vaciado, 3
logs, 2 colas); 234 punteros seguidos`, `domain 15767 pagina(s) prestada(s)
tablas 40 de 128`, y el unico evento sigue siendo el de la frontera. Los doce
pasos de `save mode` verificados. Lo que cuesta todo L0 a la maquina: 97 MiB
de RAM usada (eran 34), y nada corriendo -- la 3060 todavia no ejecuta nada;
lo unico que late es E2, ~60 interrupciones por segundo.

**L0c3b (24-09, en codigo).** `dev/gpu_despertar.rs` (fila GPU del censo, x1)
sigue `gsp/hal/tu102.rs::boot` y `gsp/boot.rs` de nova-core en su orden, en
TRES ordenes -- el escritorio espera entre ellas cediendo el turno, porque
nova-core da 2 s a cada falcon y 2 s dentro de un syscall son 500 latidos del
bus USB --:

```text
   DESPERTAR  la pagina de vaciado (0x100C40 / 0x100C10, y releida); el GSP
              reseteado, MAILBOX0/1 = 0x3C000000 (sus argumentos de LIBOS),
              STARTCPU. Se para solo: aun no tiene codigo
   BOOTER     boot_ld.bin firmado con la firma que pide el fusible del SEC2
              (la 0), prestado SOLO LECTURA en 0x3B000000; SEC2 reseteado,
              FBIF, la app 0 a la IMEM SEGURA con su ETIQUETA (+0x100), los
              datos a la DMEM, el BROM, BOOTVEC 0x100, MAILBOX0/1 = la WPR meta
              (0x3E007000), STARTCPU
   ACABAR     el SEC2 parado con MAILBOX0 = 0; el registro OS del GSP = la
              `app_version` del bootloader
```

Y el escritorio espera al RISC-V ACTIVO (`0x111388` bit 7), hasta 5 s. Salga
como salga, los tres logs del GSP se guardan crudos en `datos/gsplog.bin`
(`INFO_GPU_GSP_MEM`, de 8 en 8 bytes, como la ROM). `bmo_gpu_ga10x::falcon`
gano `copiar_etiquetado` (la IMEM del booter se etiqueta con su origen, no con
0: arranca en 0x100), `arrancar_con` (los buzones antes de STARTCPU) y
`riscv`; 3 pruebas.

**Como se sabe:** la fila `despierto` dice `el RISC-V del GSP esta ACTIVO` con
sus seis pasos en `+`, MAILBOX0 del SEC2 a 0, y la fila `gsplog` dice si el GSP
escribio en sus logs o en su cola. Si el RISC-V no despierta, lo que el GSP
alcanzo a escribir esta en `datos/gsplog.bin`.

**L0c3b en el metal (24-09, 07:10): EL GSP DE LA 3060 DESPIERTO, A LA PRIMERA.**
`despierto el RISC-V del GSP esta ACTIVO: el GSP-RM de la 570.144 corre en tu
3060 ... +booter +sec2 +os +riscv   firma 0, MAILBOX0 del SEC2 0x00000000` y
`gsplog el GSP ESCRIBIO en tu RAM: LOGINIT 5579, LOGINTR 0, LOGRM 195; su cola
62 mensajes`. Y dos pruebas de que el booter leyo NUESTRA WPR meta, sin ser
fila de nada: la WPR2 paso de `0x2FFE00000` (FWSEC) a `0x2F4000000..0x2FFEE0000`
-- exactamente la `wpr2` del reparto de L0c1 --; y el dominio subio a 15783
paginas (las 16 del booter) sin un evento nuevo: ni el booter ni el GSP-RM
tocaron nada que no se les prestara.

[!] Tres filas mintieron por haber salido bien, y se arreglaron el mismo dia:
`wpr2 ... NO donde se pidio` (el booter la EXTIENDE; ahora `cuadra` lo compara
con la `wpr2` del reparto), `frts ARRANCADO y todavia corriendo` (leia en vivo
un falcon del GSP que ya no era de FWSEC; ahora contesta la foto de antes) y
`-gsp` en `despierto` (el GSP se paro, y luego el booter lo arranco como RISC-V;
ahora queda dicho).

**La cola con 62 mensajes es lo siguiente.** Son 62 de los 63 huecos: el GSP-RM
arranco, hablo, y espera a que alguien le lea y le conteste -- nova-core le
manda SetSystemInfo y SetRegistry, corre su secuenciador y espera GSP_INIT_DONE.
Eso es L0c4.

**L0c4a (24-09, en codigo).** Sin tocar el kernel: `INFO_GPU_GSP_MEM` (L0c3b)
ya deja leer GspMem. `bmo_gpu_ga10x::rpc` (4 pruebas) entiende un mensaje de
r570 -- 48 B de elemento (`checkSum`, `seqNum`, `elemCount`) y 32 de RPC
(`header_version` 3.0, "VRPC", `length`, `function`, `rpc_result`) --, su suma
(la de nova-core: cada byte rotado 8 x (posicion mod 8), XOR, y las mitades
con XOR: un mensaje entero da 0) y el nombre de cada tipo. `commands/gspcola.rs`
recorre la cola del GSP desde el `readPtr` de la CPU hasta el `writePtr` del
GSP, mensaje a mensaje por su `elemCount`, SIN mover ningun puntero, y la
guarda cruda en `datos/gspcola.bin`.

**Como se sabe:** la fila `cola` dice cuantos mensajes y que todos traen VRPC y
su suma en 0; las filas `dijo`, cuantos de cada tipo -- se espera al menos un
`GSP_RUN_CPU_SEQUENCER` (0x1002), lo que el GSP pide a la CPU antes de seguir --;
y `init`, si ya esta el `GSP_INIT_DONE`.

**L0c4a en el metal (24-09, 07:23): la cola salio VACIA, y era el reloj.**
`cola 0 mensajes del GSP en las paginas 0..0` mientras `gsplog`, en el MISMO
save, decia `su cola 62 mensajes`: `save mode` la leyo justo detras de
`despertar`, y el RISC-V se enciende antes de que el GSP-RM hable. Ahora se
espera a que el `writePtr` del GSP se quede quieto medio segundo (5 s como
mucho) antes de recorrerla, y una cola vacia lo dice asi, no como "0 mensajes".
Las tres filas arregladas en L0c3b ya salieron bien: `wpr2 EXTENDIDA por el
booter`, `frts CORRIO` (la foto), `+gsp`, y `cuadra` con la wpr2 del reparto.

**L0c4a en el metal (24-09, 07:34): EL GSP HABLA, y su cola esta LLENA.** `cola
62 mensajes del GSP en las paginas 0..62 de su cola; secuencia 0..61` -- todos
con VRPC y la suma en 0 -- y `dijo GSP_POST_NOCAT_RECORD (0x1020) x62`. Los 62
son registros NOCAT (diagnostico del GSP-RM), y 62 de 63 huecos es la cola
LLENA: lo que el GSP diga despues -- su secuenciador, su GSP_INIT_DONE -- no
cabe hasta que alguien lea. nova-core no los descifra: los consume y sigue
esperando el suyo (`receive_msg`: "Messages with non-matching function codes are
silently consumed"). Por eso L0c4b se parte: primero VACIAR y ver que dicen los
NOCAT y que viene detras; despues, contestar.

**L0c4b1 (24-09, en codigo).** El kernel mueve el `readPtr` de la CPU
(`IOMMU_OP_GSP_LEIDO`, `dev/gpu_libos.rs::mover_lectura`: solo si el GSP
desperto en este arranque y a un hueco 0..62, con barrera antes y despues,
como `advance_cpu_read_ptr` de nova-core). `rpc::informativo` dice que se
puede consumir sin contestar -- NOCAT (0x1020), `LIBOS_PRINT` (0x100C) y el
registro de errores (0x1006) --, y `rpc::textos` saca lo legible de sus datos
(6 pruebas nuevas en el anfitrion). `commands/gspvaciar.rs` recorre la cola
desde el `readPtr`: cada informativo con su firma y su suma se cuenta, se copia
crudo a `datos/gspnocat.bin` y SOLO DESPUES se devuelve su hueco; el primero que
pide algo se queda sin tocar. Vaciada, espera hasta 3 s a que el GSP diga mas
(10 s en total) y sigue.

**L0c3b en el metal (24-09, 07:48): el booter devolvio 0x15.** MAILBOX0 del
SEC2 = 0x15 donde tres veces antes dio 0, tras REINICIAR sin cortar la
corriente con el GSP-RM de antes corriendo (nunca se apago con `booter_unload`).
FWSEC si corrio y el booter llego a extender la WPR2: fallo despues. Se anadio
MAILBOX1 a la fila `despierto`. Tras APAGAR del todo (08:14), el booter dio 0 y 0
y el GSP desperto: la regla, de momento, es apagar entre pruebas.

**Otra vez 0x15 (24-09, 13:52), y ahora se ve ANTES.** Mismo cuadro que a las
07:48: `despierto: +sec2 -os -riscv, MAILBOX0 del SEC2 0x00000015`, y la
3060 con pistas de venir caliente del arranque anterior (el de 13:35, con el
GSP-RM y el canal vivos): el enlace PCIe ya en Gen3 SIN RM en este arranque
(en frio sale en Gen1 y lo sube el RM) y el sensor termico sin su bit de
validez. La repeticion de `save mode` puso encima el motivo 45 y tapo el
0x15. Tres arreglos: `despertar` compara la WPR2 con la de FWSEC-FRTS y, si
ya empieza mas abajo (solo lo hace un booter), NO gasta el booter y dice
`IOMMU_NO_GPU_CALIENTE` (67): APAGA y corta la corriente; el 45 ya no pisa
el motivo de antes; y las filas `despierto` (0x15) y `pcie` (subida sin RM)
lo explican. La regla sigue: entre pruebas, APAGAR, no reiniciar.

**Y una foto en frio (24-09).** El propietario SI apago 15 s antes del 13:52,
asi que "reiniciaste" era una suposicion. Para no suponer: `gpu::sondear`
guarda la WPR2 y el enlace en el instante del arranque, antes de que BMO-X
toque la tarjeta (`INFO_GPU_SALUD` selectores 2 y 3), y la fila `al llegar`
lo dice. En frio NO hay WPR2 (la monta FWSEC-FRTS, que corre BMO-X). Si la
hay, la tarjeta no perdio la corriente -- un apagado con la fuente aun
enchufada puede dejar la placa alimentada --, y `despertar` no gasta el
booter (67). Si `al llegar` dice FRIO y aun asi sale 0x15, la hipotesis
estaba mal y el 0x15 es otra cosa: se mira por ahi.

**Y la foto MINTIO (24-09, 14:09): la tarjeta NO venia caliente.** Al sondear,
la WPR2 cruda ya "tenia techo" y el kernel no gasto el booter (67). Pero en
ESE MISMO arranque FWSEC-FRTS corrio y monto la WPR2 limpia en `0x2FFE00000`
-- y FWSEC solo corre con la WPR2 vacia (`IOMMU_NO_WPR2_YA`). Al sondear, el
firmware de arranque de la tarjeta aun no ha acabado y esos registros no
dicen nada. Y lo mismo vale para las 13:52: `frts CORRIO` alli tambien, asi
que la tarjeta llego limpia y "venia caliente" era FALSO; el Gen3 al llegar
tampoco era prueba (14:09 lo trae igual, y FWSEC corrio). Arreglado: la foto
queda cruda y sin veredicto, y `despertar` solo se niega si la WPR2 ya esta
EXTENDIDA antes de nuestro booter. **El 0x15 del booter queda sin causa
conocida**: dos veces (07:48 y 13:52), y el arranque siguiente fue bien las
dos.

**L0c4b1 en el metal (24-09, 08:14): 835 NOCAT, y DETRAS EL SECUENCIADOR.**
`vacia 835 consumidos; la CPU lee ahora en la pagina 16: GSP_POST_NOCAT_RECORD
x835` y `pide GSP_RUN_CPU_SEQUENCER (0x1002) numero 835`: lo que se esperaba.
Devolverle los huecos funciono -- el GSP siguio escribiendo 773 mas. Los NOCAT
traen en claro `ASSERT` y `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05_0_GFW_BOOT_P...`
(el registro del progreso del arranque de la VBIOS, `0x118234`); el resto era
ruido binario, y ahora se filtra (6 caracteres o mas, hasta 96). Sin descifrar
todavia: los crudos estan en `datos/gspnocat.bin`. La fila `despierto` decia en
rojo "el RISC-V YA NO esta activo, PARADO": es lo que toca. El GSP-RM se para
SOLO tras pedir el secuenciador, y la ultima orden de este, `CORE_RESUME`,
resetea el GSP, le vuelve a dar sus argumentos, arranca el SEC2-RTOS y espera a
que el GSP-RM vuelva (`gsp/sequencer.rs` de nova-core). Ahora la fila lo dice en
verde. Y nova-core manda SetSystemInfo y SetRegistry ANTES de eso, al ver el
RISC-V activo: BMO-X no los mando, y el GSP siguio sin ellos -- quiza los 835
ASSERT sean eso. Por eso L0c4b2 se parte en tres.

**L0c4b2a (24-09, en codigo): LA PRIMERA VEZ QUE LA CPU LE ESCRIBE AL GSP.**
`bmo_gpu_ga10x::orden` (3 pruebas) arma `GSP_SET_SYSTEM_INFO` (72, los 928 B
de `GspSystemInfo` con los campos que llena nova-core: BAR0, BAR1 y BAR3
fisicas, el BDF, los ID del PCI, el espejo del PCI en 0x88000 y `maxUserVa`) y
`SET_REGISTRY` (73, las tres claves de nova-core a 1: `RMSecBusResetEnable`,
`RMForcePcieConfigSave`, `RMDevidCheckIgnore`), con su `seqNum`, `rpc_result` a
0xFFFFFFFF y la suma que da 0. Los arma el KERNEL (`IOMMU_OP_GSP_SISTEMA`,
`dev/gpu_libos.rs::escribir_sistema`) directamente en las paginas 0 y 1 de la
cola de la CPU, con lo que el mismo lee del PCI, y mueve su `writePtr` a 2: el
escritorio no manda bytes. Van ANTES de despertar, como nouveau
(`r535_gsp_oneinit`) y OpenRM (`kgspInitRm`); nova-core los manda despues con el
RISC-V activo y toca el timbre `0x110C00` -- aqui no hace falta, `despertar`
resetea el falcon del GSP despues. En `save mode`, `sistema` va entre `libos` y
`despertar`.

** De paso, un error de L0c4a: el `length` del RPC CUENTA los 32 B de su
cabecera (nova-core, `payload_length`). La suma cubria 32 bytes de mas y dio 0
en los 835 del metal solo porque detras habia ceros; ahora `Mensaje::datos`.

**L0c4b2a en el metal (24-09, 08:34): LOS LEYO, y se acabaron los ASSERT.**
`sistema GSP_SET_SYSTEM_INFO (928 B, suma 0) y SET_REGISTRY (117 B, suma 0)`,
`sysinfo BAR0 0x0FB000000, BAR1 0x00D0000000, BAR3 0x00E0000000; PCI 29:00.0
10DE:2504 sub 1462:397D rev A1` y `leyo el GSP LOS LEYO: su puntero sobre la
cola de la CPU esta en 2`. Y lo que cambio detras: la cola del GSP trae UN
mensaje, `GSP_RUN_CPU_SEQUENCER` con secuencia 0 -- ni un NOCAT, donde antes
hubo 835 --, y los logs bajaron de LOGINIT 76267 / LOGRM 1767 a 203 / 92. Los
835 ASSERT eran el GSP-RM sin su SetSystemInfo. Lo que queda es el
secuenciador (L0c4b2b).

**L0c4b2b (24-09, en codigo).** `bmo_gpu_ga10x::secuenciador` (3 pruebas) lee
`rpc_run_cpu_sequencer_v17_00`: `bufferSizeDWord`, `cmdIndex` (cuantas
ordenes, como el `total_cmds` de nova-core), `regSaveArea[8]` y detras las
ordenes PEGADAS -- un opcode y solo su carga: REG_WRITE (addr, val),
REG_MODIFY (addr, mask, val), REG_POLL (addr, mask, val, timeout, error),
DELAY_US, REG_STORE (addr, index) y las cuatro del nucleo sin carga
(CORE_RESET, CORE_START, CORE_WAIT_FOR_HALT, CORE_RESUME). Se para en la
primera que no entiende. `commands/gspsecuencia.rs` lee el mensaje donde lo
dejo `vaciar`, comprueba su suma, lo guarda crudo en `datos/gspsec.bin` y dice
cada orden con la unidad del registro (PMC, PFB, falcon GSP, PGC6/BSI, falcon
SEC2). No corre nada ni mueve un puntero.

**L0c4b2b en el metal (24-09, 08:48): 420 ordenes, y `cmdIndex` son PALABRAS.**
`secuen 420 ordenes (cmdIndex 1564, buffer 16354 palabras): ESCRIBIR x312
ESPERAR x104 CORE_RESET x1 CORE_START x1 CORE_WAIT_FOR_HALT x1 CORE_RESUME x1`
-- y un "SE PARO" que era de BMO-X: 312 x 3 + 104 x 6 + 4 x 1 = 1564, las
palabras exactas. `cmdIndex` no cuenta ordenes (nova-core lo llama `total_cmds`
pero se para antes, donde acaban los datos). El lector ahora se para donde
acaban las `cmdIndex` palabras, con una prueba que rehace el del metal. Las 48
primeras ordenes: esperar el bit 31 de MAILBOX0 del GSP (`0x110040`), ponerlo
a 0, CORE_RESET, `0x110600 <- 0x114`, y luego una y otra vez el DMA del
falcon del GSP -- base `0x110110 <- 0x02F3D430` (VRAM 0x2F3D43000, justo bajo
la WPR2), y bloques de 256 B con `0x110114`/`0x11011C` y la orden `0x614` en
`0x110118`, esperando su bit 0. Ahora la fila `toca` cuenta que unidades toca
todo el secuenciador y `nucleo` donde caen las cuatro del nucleo: con eso se
decide que deja escribir el kernel en L0c4b2c.

**L0c4b2b en el metal (24-09, 08:59): `toca falcon GSP x416`.** Las 416
ordenes con registro, TODAS en el falcon del GSP (0x110000..0x111FFF);
`nucleo CORE_RESET en la 3, CORE_START en la 418, CORE_WAIT_FOR_HALT en la
419, CORE_RESUME en la 420`. Las ultimas: el DMA a IMEM acaba, y la BROM
(`0x111210` PARAADDR, `0x11119C` ENGIDMASK, `0x111198` UCODE_ID, `0x111180`
MOD_SEL), MAILBOX0 `<- 0xFE` y BOOTVEC `<- 0x100`: el GSP-RM carga un ucode
firmado en el falcon del GSP, lo arranca, espera a que se pare y pide volver.

**L0c4b2c (24-09, en codigo): CORRERLO.** `bmo_gpu_ga10x::correr` (5 pruebas,
con una 3060 de mentira) hace cada orden como nova-core (`gsp/sequencer.rs`),
pero por TRAMOS: cada llamada corre hasta 1 ms y dice donde se quedo; una
espera que no llega no bloquea, sigue contando su plazo en la llamada
siguiente (REG_POLL 4 s sin plazo, la parada 2 s, la vuelta 2 s). Antes del
primer bit, `validar` mira TODAS: solo el falcon del GSP, alineado a 4 -- una
sola fuera y no se escribe nada. CORE_RESUME en tres fases: reset del GSP y
LIBOS a sus buzones y arrancar el SEC2; esperar el bit 26 de
`NV_PGC6_BSI_SECURE_SCRATCH_14` (0x1180F8); MAILBOX0 del SEC2 a 0, el OS del
GSP, y su RISC-V activo. El kernel (`IOMMU_OP_GSP_SECUENCIAR`,
`dev/gpu_despertar.rs::secuenciar`) lee el mensaje EL MISMO de la cola del
GSP, con su suma, y al acabar devuelve sus huecos; una vez por arranque. El
escritorio (`commands/gspinit.rs`) llama tramo a tramo y luego consume lo
que diga el GSP-RM hasta su `GSP_INIT_DONE` (10 s).

** Lo que puede salir: la orden 1 espera el bit 31 de MAILBOX0 del GSP, y la
fila `gsplog` de 08:59 lo leyo a 0. Si no se pone, la orden 1 acaba en su
plazo (4 s) SIN haber escrito nada, y la fila `corrio` dice lo ultimo leido.

**Repasado contra OpenRM 570.144 antes del metal (24-09).** El driver de
NVIDIA de ESTA version (`kernel_gsp.c::kgspExecuteSequencerBuffer`,
`kernel_gsp_tu102.c::kgspExecuteSequencerCommand_TU102`) confirma que
`cmdIndex` son palabras y que cada orden es como aqui. Tres cosas cambiaron:
CORE_RESUME resetea el GSP PARA EL RISC-V (`kflcnResetIntoRiscv_GA102`: el
reset y `BCR_CTRL` = RISC-V, valido y BRFETCH, sin pasar por FALCON ni `RM`;
nova-core resetea a FALCON), `falcon::resetear_en_riscv`; REG_STORE solo a
los huecos 0..7 de `regSaveArea` y `cmdIndex < bufferSizeDWord`, como OpenRM;
y un `gpu init` sin el mensaje en la cola ya no deja el paso roto para todo
el arranque (no se habia escrito nada). De paso: `datos/gspsec.bin` se
escribe de una vez (`escribir_de`), no de 7 en 7 bytes -- sospechoso de los
43-46 ms de `latido tarde` de 08:48 y 08:59 --, las filas `init` y `pide`
dicen si el secuenciador ya se corrio, y los consejos de `save mode` nombran
el paso siguiente de verdad.

**L0c4b2c en el metal (24-09, 09:54): EL GSP-RM ARRANCO.** `corrio 420 de 420
ordenes CORRIDAS en 5 tramos de 1 ms; CORE_RESUME: el GSP-RM volvio`, `listo
GSP_INIT_DONE LLEGO (197 ms tras CORE_RESUME); dijo: GSP_POST_NOCAT_RECORD x1
UCODE_LIBOS_PRINT x2 GSP_INIT_DONE x1`, y `despierto el RISC-V del GSP esta
ACTIVO`. La orden 1 (MAILBOX0 bit 31) SI llego: el miedo de L0c4b2c no se
cumplio.

** Y la pantalla se QUEDO QUIETA: la CPU viva, el texto de `save mode` en
pantalla y sin panel; el propietario apago a los 10-20 s. La pista, en el
mismo INFORME: `compose ... copia 381 ps/pixel`, donde todos los de antes
decian 2600-5300. El GOP se pinta por BAR1 (0xD0000000, la fila `sysinfo`), y
el GSP-RM, al arrancar, se queda con BAR1 y la pone VIRTUAL: lo que pinta la
CPU deja de caer donde mira la pantalla. Sin probar. Por eso (L0c4b3):

- `init` SALE de `save mode`: con el modo armado, cada arranque congelaba la
  pantalla. La cadena acaba en `secuenciador` (el GSP parado, esperando) y
  `gpu init` se da A MANO, con un save antes y otro despues.
- `INFO_GPU_DESPIERTO_BUZON` selector 3 lee `BAR1_BLOCK` y `BAR2_BLOCK`
  (0xB80F40/48, nouveau `tu102_bar`); la fila `bar1` dice antes y despues, y
  si el GSP-RM la hizo virtual.
- Si es eso, lo siguiente es pintar sin el GOP por BAR1: pedirle al GSP-RM un
  hueco de BAR1 para la superficie de la pantalla (L1), o devolverle a BAR1 su
  modo fisico si el GSP-RM no la usa.

**L0c4b3a (24-09, en codigo).** El kernel apunta `BAR1_BLOCK` (0xB80F40) en
la primera llamada del secuenciador, antes de CORE_RESUME, y
`IOMMU_OP_GSP_BAR1` le devuelve ESE valor --solo ese: nada que venga del
escritorio-- y espera el enlace como nouveau (`tu102_bar_bar1_wait`, los bits
0..1 de 0xB80F50, 2 ms). `gpu init` lo hace solo si ve BAR1 cambiada tras
`GSP_INIT_DONE`, y repinta el escritorio entero; `gpu bar1` lo hace a mano. Es
un puente: el GSP-RM, parado esperando RPC, no usa BAR1. Lo de verdad es
pedirle en L1 su propio hueco de BAR1 para la superficie de la pantalla, y
luego que pinte la GPU (el canal y el motor de copia).

**Como se sabe (L0c4b3a):** tras `gpu init` la pantalla sigue viva, con el
panel y la luz del GSP en verde; la fila `bar1` dice si hubo que devolverla;
y la copia al GOP vuelve a sus ~3000 ps/pixel.

**Como se sabe (L0c4b2c):** `corrio 420 de 420 ordenes CORRIDAS`; `listo
GSP_INIT_DONE LLEGO`; y `despierto` otra vez con el RISC-V ACTIVO.

**Como se sabe (L0c4b2b):** la fila `secuen` dice cuantas ordenes y de que tipo
(se espera que acabe en CORE_RESUME), y cada fila `orden` que registro toca y
con que. Con esa lista delante se decide que deja correr el kernel en L0c4b2c.

**Como se sabe (L0c4b2a):** la fila `sistema` relee los dos de la cola, con VRPC
y la suma en 0; `sysinfo` dice las BAR, el BDF y los ID que se le dieron; y
`leyo`, tras `despertar`, que el puntero con el que el GSP lee la cola de la
CPU paso de 0 a 2. Y se espera que la fila `nocat` cambie: si los 835 ASSERT
eran por no tener esto, bajan.

**Como se sabe:** la fila `vacia` dice cuantos consumidos y de que tipo, y en
que pagina lee ahora la CPU; `nocat`, los textos en claro de los NOCAT; y
`pide`, el primer mensaje que espera respuesta -- se espera
`GSP_RUN_CPU_SEQUENCER` (0x1002), que es donde empieza L0c4b2.

**L0a en el metal (24-09, 04:58):** `vbios 546 KiB en 4 imagenes: PCI-AT(63K)
EFI(82K) FWSEC(21K) FWSEC(379K)`, `fwsec v3 en 0x41210: IMEM 57856 B, DMEM 2048
B, motor 0x0400 ucode 9, 3 firmas (versiones 0x0007)`, `DMAP en DMEM+0x0560; la
orden va en DMEM+0x07C0 (64 B)`, `fusible 0x8241E0 = 0x00000003 -> version 2; la
firma buena es la 2 de 3`, `vram 12288 MiB; FRTS iria en
0x2FFE00000..0x2FFF00000; el firmware de arranque ACABO`, `wpr2 NO hay`.

**L0b (24-09, en codigo).** `bmo_gpu_ga10x::fwsec` cambia la orden a FRTS (los 44
bytes empaquetados de nova-core: ReadVbios + FrtsRegion en paginas, tipo VRAM) y
pone la firma del fusible en DMEM + `pkc_data_offset`; `falcon.rs` gana el DMA a
la IMEM en modo seguro, el BROM (`+0x1180/0x1198/0x119C/0x1210`) y el arranque
(`MAILBOX0`, STARTCPU por el alias). El kernel (`dev/gpu_prestamo.rs`, la fila
GPU del censo pasa a x2) lo hace en tres ordenes -- PREPARAR relee y juzga el
descriptor por su cuenta, TROZO copia 4 KiB de la ROM por syscall, CORRER
parchea, firma, presta 32 paginas SOLO LECTURA en `0x11000000`, resetea, carga y
arranca -- y el escritorio espera a que el falcon se pare cediendo el turno, 3 s
como mucho. Exito = MAILBOX0 0, el codigo de FRTS (`0x1438`, bits 16..31) 0 y la
WPR2 donde se pidio. `gpu fwsec`, la fila `frts`, y el paso `fwsec` de `save
mode` (hecho = hay WPR2, que sobrevive hasta que la 3060 se reinicia).

**L0b en el metal (24-09, 05:19): EL PRIMER FIRMWARE QUE BMO-X EJECUTA EN LA
3060.** `frts CORRIO: el falcon se paro con MAILBOX0 = 0 y la WPR2 montada   firma
2, MAILBOX0 0x00000000, codigo FRTS 0x0000` y `wpr2 YA montada:
0x2FFE00000..0x2FFEE0000 -- donde se pidio` (896 KiB de la MiB pedida: FRTS usa
lo que necesita). `domain 33 paginas prestadas` (la de prueba y las 32 de FWSEC),
sin eventos nuevos: el ucode firmado se leyo por la IOMMU y no toco nada mas. Y
E2 siguio contando. Es exactamente donde se para nova-core en Linux 6.17
("GPU instance built"), y aqui detras de una IOMMU que traduce.

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

```text
   L1a  la PRIMERA RPC: GET_GSP_STATIC_INFO -- el nombre de la 3060 segun
        el GSP-RM, su VRAM, sus regiones y las ASAS del RM. `gpu estatica`,
        y sola tras `gpu init`                  [VISTO 24-09 10:35]
   L1b  NUESTROS objetos en el RM (GSP_RM_ALLOC): cliente, dispositivo y
        subdispositivo propios. `gpu objetos`, y solo tras `gpu init`
                                                [en codigo, 24-09]
   L1c  colgando de ellos: un espacio de direcciones (VASPACE), memoria
        de VRAM y un hueco de BAR1 para la pantalla (y soltar el puente
        de L0c4b3a)
          L1c1  FERMI_VASPACE_A "de fuera" (`espacio`)   [VISTO 24-09 11:56]
          L1c2  la CPU escribe en la VRAM, PRAMIN (`vram`) [VISTO 24-09 11:56]
          L1c3  la raiz PD3 en la VRAM y SET_PAGE_DIRECTORY (`directorio`)
                                                [VISTO 24-09 12:25]
   L1d  un canal y el motor de COPIA: que la GPU mueva los pixeles
          L1d0  leer la raiz tras el RM (fila `raiz`)  [VISTO 24-09 12:37]
          L1d1  mapear 16 paginas propias bajo ella (`tramo`)
                                                [VISTO 24-09 12:47]
          L1d2  el canal AMPERE_CHANNEL_GPFIFO_A, su USERD y su GPFIFO
                L1d2a  que motores hay y cuanto mide el bufer de
                       metodos (`motores`)      [VISTO 24-09 13:19]
                L1d2b  el canal: su ALLOC con instancia, USERD y
                       GPFIFO en el tramo, y el bufer de metodos
                       (`canal`)                [VISTO 24-09 13:35]
                L1d2c  BIND a COPY2 y GPFIFO_SCHEDULE (`encender`)
                                                [VISTO 24-09 13:35]
                L1d2d  la ficha (`ficha`)       [VISTO 24-09 13:35]
                       y el timbre: la primera entrada del GPFIFO
                       (`copia`)                [VISTO 24-09 14:55]
          L1d3  AMPERE_DMA_COPY_B en el canal: la GPU copia VRAM a VRAM
                (`copiador` y `copia`)          [VISTO 24-09 14:55]
```

**L1d2a en el metal (24-09, 13:19): LOS MOTORES.** `11: GR0 COPY0 COPY1 COPY2
COPY3 COPY4 NVDEC0 NVENC0 SW0 SEC20 OFA0` por `GET_ENGINES_V2` en 1 ms, y el
bufer de metodos `20480 B (0x05000)`, lo que `canal::METODOS` presta. La fila
decia `el de copia del canal: COPY0`: tomaba la PRIMERA COPY, y COPY0 y COPY1
son GRCE en GA10x (atadas al 3D). Arreglado: nombra `canal::MOTOR`, COPY2, el
unico que el contrato deja atar.

**L1d2b, L1d2c y la ficha (24-09, en codigo).** Tres pasos de `save mode`
detras de `motores`, y `gpu canal` los da en orden, parando en el primero que
no sale:

```text
   canal     IOMMU_OP_GPU_CANAL (0x21): las paginas 0..2 del tramo a cero por
             PRAMIN, 5 marcos NEUTRO a cero y prestados ESCRIBIBLES en
             0x3A000000 (el bufer de metodos, releidos por la IOMMU), y el
             GSP_RM_ALLOC de `canal::pedir` (368 B que el contrato compara
             uno a uno). Una vez por arranque, con el tramo puesto. Antes, el
             escritorio exige que L1d2a dijo COPY2 y 20480 B
   encender  IOMMU_OP_GPU_CANAL_ORDEN (0x22): BIND a COPY2 y despues
             GPFIFO_SCHEDULE, como `r535_chan_start`; solo con el canal pedido
   ficha     GET_WORK_SUBMIT_TOKEN, una pregunta por IOMMU_OP_GSP_CONTROL
```

**Como se sabe:** `gpu` dice `canal` NV_OK, `atado` COPY2 NV_OK, `en lista`
NV_OK y `ficha` un numero; `iommu` 5 paginas mas en `domain` y sin eventos
nuevos. Si el RM NIEGA el canal, su NV_STATUS sale en la fila. Cotejado con
`r535_chan_alloc`/`r535_chan_start` de nouveau (Linux master, 24-09): la misma
memoria (instancia, USERD y RAMFC en VRAM con `addressSpace` 2, el bufer de
metodos en `dma_alloc_coherent` con 1 -- la IOVA, como aqui), las mismas
`internalFlags` y el mismo orden: BIND y despues SCHEDULE con `bEnable 1`.

**L1d2b, L1d2c y la ficha en el metal (24-09, 13:35): EL CANAL VIVE.**
`canal: 0xF1F00001 (clase 0xC56F, chid 1): NV_OK` en 1 ms; `atado: COPY2
(tipo 0x0B): NV_OK`; `en lista: NV_OK`; `ficha: 0x00000001`. `iommu`: 15788
paginas (las 5 del bufer de metodos) y ningun evento nuevo. 29 de 29 pasos
de `save mode`.

**L1d2d y L1d3 (24-09, en codigo): EL PRIMER TRABAJO.** `bmo_gpu_ga10x::copia`
(5 pruebas) y dos pasos mas de `save mode`:

```text
   copiador  IOMMU_OP_GPU_COPIADOR (0x23): GSP_RM_ALLOC de AMPERE_DMA_COPY_B
             (0xC7B5) colgado del canal, { version 1, engineType COPY2 }
             (`r535_ce_alloc`). El contrato compara sus 8 B
   copia     IOMMU_OP_GPU_COPIA (0x24, `arg1` = la ficha, que tiene que
             decir el chid de NUESTRO canal): por PRAMIN el ORIGEN (pagina 8
             del tramo) con el patron de L1c2, DESTINO (12) y SEMAFORO (4) a
             cero, 17 palabras de ordenes en la pagina 3 (SET_OBJECT 0xC7B5
             en el subcanal 4, origen/destino/pitch, SET_SEMAPHORE y
             LAUNCH_DMA 0x18E) y la entrada 0 del GPFIFO; todo releido.
             Despues GP_PUT = 1 en el USERD (+0x8C) y la ficha en el TIMBRE,
             BAR0 0xBB0090 (`ga100_vfn` 0xB80000 + `user` 0x30000 + 0x90,
             `tu102_chan_start`). Hasta 100 ms esperando el semaforo
```

Los metodos y bits, de `clc7b5.h` y `clc56f.h` de open-gpu-kernel-modules.

**L1d3 en el metal (24-09, 14:19): el GSP desperto y todo hasta el copiador
en verde -- y la copia NO.** `copiador: 0xCE000002 (clase 0xC7B5, COPY2, en el
canal): NV_OK`; `copia: 0 de 1024 ... semaforo SIN PAGAR, GP_GET 0 en 100000
us`; `iommu` sin eventos nuevos. GP_GET 0: la 3060 no llego a leer la entrada
del GPFIFO. El formato de las tablas coincide con `vmmgp100.c` bit a bit, pero
faltaba algo que nouveau hace tras CADA mapeo y BMO-X no hizo tras L1d1:
invalidar la MMU de la GPU para la raiz (`tu102_vmm_flush`: 0xB830A0 la raiz
>> 8, 0xB830A4 0, 0xB830B0 0x80000001, y esperar el bit 31). Ahora `lanzar`
lo hace antes del timbre. Y si la copia vuelve a fallar, la fila `diag` lee
(solo lectura, `IOMMU_OP_GPU_LEER` 0x25, solo el tramo) lo que el RM dejo en
la RAMFC (USERD, GPFIFO, chid, la raiz en +0x200), GP_GET/GP_PUT, la entrada
0 y el semaforo, y barre 1 s la cola del GSP: un canal caido llega como
`RC_TRIGGERED` o `MMU_FAULT_QUEUED`.

**Y con la MMU invalidada (24-09, 14:30): igual, GP_GET 0 -- pero `diag`
hablo.** `gpfifo lo 0x00002000, hi 0x00090002` (la VA del GPFIFO y 512
entradas: el RM escribio la RAMFC), `pdb lo 0x04100C00` (NUESTRA raiz, en
VRAM, formato v2 y pagina grande de 64 KiB), `gp_put 1`, la entrada 0 bien,
`userd 0` y `chid 0`, y el GSP-RM sin decir nada. `userd 0` NO es el fallo: en
GA10x el USERD va en la entrada de la lista de ejecucion
(`NV_RAMRL_ENTRY_CHAN_USERD_PTR`, `kernel_channel_ga100.c`) y la 3060 lo copia
a la RAMFC al CARGAR el canal. Asi que la RAMFC sin tocar dice que la 3060
nunca CARGO el canal: el timbre no lo desperto. La ficha `0x1` cuadra con
`kfifoGenerateWorkSubmitTokenHal_GA100` (lista << 16 | chid: lista 0, chid 1).
Para separar las dos causas que quedan, `diag` lee ahora el reloj de la
ventana del timbre (`NVC361_TIME_0`, BAR0 0xBB0080) antes y despues de 1 s
(fila `timbre`): si corre, el timbre llega a su sitio y el problema es el
canal en la lista; si esta quieto, la ventana no es esa. Y `+010`/`config` de
la RAMFC, y GP_GET otra vez al final.

**14:41: el timbre LLEGA.** `timbre: su ventana VIVE (el reloj de 0xBB0080
corrio)`; `+010 0x0000FACE` (el RM escribio la RAMFC con la marca de nouveau);
GP_GET 0 un segundo despues y el GSP-RM callado. Descartada la direccion del
timbre: queda el canal en su LISTA. La ficha 0x1 dice lista 0; y nouveau
(`r535_fifo_runl_ctor`) se salta las copias GRCE sin GR en su lista ("they
don't appear to function as async copy engines"). Si COPY2 esta en la lista
de GR0, su lista no corre sin el contexto de GR (el "golden context" que
nouveau prepara en `r535_gr_oneinit` antes de ningun canal). Para verlo:
`Control::Dispositivos` (`FIFO_GET_DEVICE_INFO_TABLE`, 0x20801112, 3212 B de
ceros, que el contrato compara sin un bufer de 3 KiB) da la lista de cada
motor y la base de sus registros; y `IOMMU_OP_GPU_LEER` con 63:62 = 01 lee de
esa lista la config de su CHRAM (+0x004), la del timbre (+0x008, el numero
en 31:16) y la entrada de NUESTRO canal en la CHRAM (ENABLE, PENDING, BUSY,
los FAULTED: `dev_runlist.h` de OpenRM), sacando la direccion el kernel. Filas
`listas` y `en la 3060`.

**14:47: LA LISTA.** `listas: GR0=L0 COPY0=L0 COPY1=L0 COPY2=L1 COPY3=L2
COPY4=L8`: COPY2 tiene su PROPIA lista (la 1), no es una GRCE. Pero la ficha
del RM era 0x1 = lista 0 << 16 | chid 1: el timbre llamaba a la lista de GR0,
no a la de nuestro canal. nouveau sobre el GSP-RM en GA1xx (`rm/ga1xx.c`:
`tu102_chan_doorbell_handle`) escribe `lista << 16 | chid` con la lista de la
TABLA de aparatos: para nosotros 0x00010001. Ahora `copiar` pregunta la
tabla antes y usa ESE valor (`copia::timbre_de`); la ficha del RM solo si la
tabla no contesta. Y un fallo nuestro: la fila `en la 3060` dijo "sin leer"
porque `lista_legible` pedia 4 KiB y la base de la lista de COPY2 es
0x00C00400; ahora 64 B.

**L1d3 en el metal (24-09, 14:55): LA 3060 COPIO.** `copia: 1024 de 1024
palabras de VA 0x200008000 a VA 0x20000C000; semaforo PAGADO; timbre
0x00010001 (la lista de COPY2 segun la tabla)`. El primer trabajo que la RTX
3060 ejecuta para BMO-X, por su canal, sus tablas de paginas y su timbre, sin
el driver de NVIDIA. `en la 3060: ... NUESTRO canal en su CHRAM: 0x000000C6 =
ENABLE NEXT ON_PBDMA ON_ENG`; `diag` un segundo despues, `gp_get 1` y el
semaforo `0x3060C0DE`; `iommu` sin eventos nuevos. La causa de GP_GET 0 en los
cuatro intentos de antes: el timbre llevaba la ficha del RM (0x1, la lista
0, la de GR0) y el canal vive en la 1. La fila todavia salio en rojo por una
comprobacion nuestra: `sana` exigia GP_GET 1 y la 3060 lo escribe en el USERD
despues de pagar el semaforo. Arreglado: cuentan el destino y el semaforo,
que solo escribe ella; GP_GET se espera hasta 10 ms y se muestra.

**Como se sabe (L1d3):** la fila `copia` dice `LA 3060 COPIO: 1024 de 1024`,
`semaforo PAGADO` y `GP_GET 1`, y `iommu` sigue sin eventos nuevos. Nada de
eso lo escribe la CPU. Si el semaforo no llega, lo primero a mirar es si
GP_GET avanzo (la 3060 leyo el GPFIFO: el timbre y el USERD van bien) o no.

**L0c4b3a en el metal (24-09, 10:13): LA PANTALLA SOBREVIVE AL GSP-RM.** `bar1
antes 0x002FFF00, despues 0x802F3E90 (BAR2 0xC02F3E91 -> 0xC02F3E91): el GSP-RM
la puso VIRTUAL y se le DEVOLVIO la del GOP`. Con el GSP-RM corriendo (`RISC-V
ACTIVO`), el escritorio siguio vivo: DOOM a 70 fps, el cubo, 16.032 cajas
compuestas y la copia al GOP otra vez en 5393 ps/pixel. El diagnostico de
09:54 era ese.

**L1a (24-09, en codigo).** `bmo_gpu_ga10x::estatica` (2 pruebas) arma la
pregunta --la RPC 65 con los 1656 B de `GspStaticConfigInfo_t` a cero-- y lee
la respuesta en los offsets de las bindings r570.144 (sacados compilandolas en
el anfitrion): `gpuNameString` +0x4EC, `fb_length` +0x4C8, las regiones +0x158,
`bar1PdeBase` +0x600, `hInternalClient/Device/Subdevice` +0x640. El kernel
(`IOMMU_OP_GSP_ESTATICA`, `dev/gpu_libos.rs::preguntar_estatica`) la pone en la
pagina siguiente de la cola de la CPU, mueve su `writePtr` y toca el TIMBRE
(0x110C00, `notify_gsp` de nova-core): el primer registro que se escribe para
hablarle a un GSP-RM vivo. El escritorio (`commands/gsprpc.rs`) espera la
respuesta 5 s, consume lo que llegue antes y la lee.

**Como se sabe (L1a):** la fila `rpc` dice CONTESTADA con `rpc_result 0`;
`nombre` dice lo que el GSP-RM cree que es tu tarjeta; `memoria` sus 12 GiB;
`asas` las tres asas del RM, que son la llave de L1b.

**L1a en el metal (24-09, 10:35): EL GSP-RM CONTESTA.** `GET_GSP_STATIC_INFO
(numero 2) CONTESTADA en 0 ms, rpc_result 0x00000000`; `NVIDIA GeForce RTX
3060 (GA106-A, arranco por UEFI)`; 12288 MiB; 5 regiones de VRAM, 1 usable,
`0x003110000..0x2F06DFFFF` (11989 MiB); asas internas cliente `0xC2000006`,
dispositivo `0xABCD0080`, subdispositivo `0xABCD2080`; BAR1 PDE `0x2F3C2D000`,
BAR2 PDE `0x2F3E92000`. El panel del escritorio, `gsp LISTO` con los 7 nodos
en verde. Lo unico raro: `fb_bus_width` salio 0, `fb_ram_type` 192 y
`l2_cache_size` 0, con offsets iguales a `gsp_static_config.h` de OpenRM
570.144 (comprobado): el GSP-RM no los llena como dicen sus nombres. La fila
`memoria` los muestra ahora CRUDOS, con `fbio_mask` y `fbp_mask`.

**L1b (24-09, en codigo).** `bmo_gpu_ga10x::objeto` (3 pruebas) arma
`GSP_RM_ALLOC` (RPC 103, `rpc_gsp_rm_alloc_v03_00` de 32 B y sus parametros)
para tres objetos de asas FIJAS, las de nouveau (`rm/handles.h`): el cliente
`0xC1D0000B` (NV01_ROOT, `NV0000_ALLOC_PARAMETERS` de 120 B en r570, con
`processID = ~0`), el dispositivo `0xDE1D0000` (NV01_DEVICE_0, 56 B,
`hClientShare` = el cliente) y el subdispositivo `0x5D1D0000`
(NV20_SUBDEVICE_0, 4 B). El kernel (`IOMMU_OP_GSP_OBJETO`, `arg1` = 0, 1 o 2)
los arma con el mismo `enviar` que L1a: el escritorio dice CUAL, nunca manda
bytes. `commands/gspobjeto.rs` los pide en orden y se para en el primero que
no sale; `gpu init` los pide solo, tras la respuesta de L1a.

**L1b en el metal (24-09, 10:52): EL RM TIENE NUESTRO CLIENTE.** `gpu init`
mando los tres (numeros 3, 4 y 5: la siguiente a mano fue la 6, y el escritorio
solo pide el siguiente si el anterior salio, asi que cliente y dispositivo
salieron). `gpu objetos` a mano: `cliente 0xC1D0000B: ya existia (0x19),
rpc_result 0x00000019`: el RM lo tenia. El escritorio se paro ahi por un fallo
NUESTRO: el GSP-RM repite el `NV_STATUS` en el `rpc_result`, y `vale` pedia
`rpc_result 0`. Arreglado: vale si los dos dicen lo mismo; y la numero que CREO
cada objeto se recuerda todo el arranque (`CREADO con NV_OK en la numero N`).

Y la fila `memoria`, cruda: `+0x4D0: 7 0 0 C0 11 0 7`. El bus (192 = 0xC0) y
`RAM_TYPE_GDDR6` (0x11) estan 4 B DESPUES de lo que dice el header de OpenRM:
dos aciertos exactos. Se leen donde los puso el firmware.

**L1b+ (24-09, en codigo): EL CONTRATO, LA PRIMERA ORDEN DE CONTROL Y LA 3060
EN EL PANEL.**

- `bmo_gpu_ga10x::contrato` (2 pruebas): la lista CERRADA de lo que sale hacia
  el GSP-RM -- SetSystemInfo, SetRegistry, GET_GSP_STATIC_INFO, `GSP_RM_ALLOC`
  solo de nuestros tres objetos (asas y clases) y `GSP_RM_CONTROL` solo sobre
  nuestro subdispositivo y solo las ordenes de `control::Control`. El kernel la
  pasa sobre cada mensaje YA armado, antes de mover el `writePtr`: lo que no
  esta no sale y el timbre no suena (motivo 57). Es la segunda llave: la
  primera es que el escritorio nunca manda bytes.
- `bmo_gpu_ga10x::control` (2 pruebas): `GSP_RM_CONTROL` (RPC 76, cabecera de
  24 B) con una orden, `PERF_GET_CURRENT_PSTATE` (0x20802068, 4 B). Kernel
  `IOMMU_OP_GSP_CONTROL` (0x1C, motivo 58); escritorio `gpu salud`, paso
  `salud` de `save mode`, y sola tras `gpu init`.
- `bmo_gpu_ga10x::salud` (2 pruebas) e `INFO_GPU_SALUD` (0xC2): la temperatura
  del sensor `0x020460` (nouveau `gp100_temp_get`, bit 29 valido) y el enlace
  PCIe (Link Status y Link Capabilities), dos lecturas. En el panel, bajo la
  luz del GSP: `3060  45o  P8` y `pcie 4 x16  12G GDDR6`.
- **Los VATIOS de la 3060 no estan, y se dice por que**: la potencia la leen
  sensores de la PMU por I2C y la orden del RM que la da no esta en OpenRM
  570.144 (`ctrl2080pmgr.h`, `thermal`, `clk` y `fan` salen sin ordenes).
- `save mode` VUELVE a llevar `init`, y detras `estatica`, `objetos` y `salud`:
  una sola orden lo da todo. Antes de todo, un save de EMERGENCIA (aunque no
  haya nada que arriesgar); `init` repinta el escritorio; el paso en curso se
  ve en la linea de estado.
- La caja de Ctrl+Alt: el consejero ya no se AGREGA a la salida en cada
  invocacion (la mezclaba): sale en UNA linea, la de estado. Mientras se
  teclea, esa linea SUGIERE ordenes (`commands::sugerencias`, cada una pasada
  por `parse`), y TAB completa ordenes antes que rutas. Una fila en blanco
  separa cada orden de la respuesta de antes.

**L1b+ en el metal (24-09, 11:25): LOS 21 PASOS DE UNA VEZ.** `save mode` dio
todo seguido y el consejero dijo `verificado: los 21 pasos`. `obj cli/disp/sub`
NV_OK (numeros 3, 4 y 5); `pstate P0` por `PERF_GET_CURRENT_PSTATE` en 1 ms
(numero 6, mascara 0x0001): **la primera orden de control a un objeto
nuestro**. `pcie Gen1 x16 de Gen3 x16`: la 3060 arranca en Gen1 y subir el
enlace es del RM (aun no pedido). `temp`: el sensor dio `0xC0003168`, con el
bit 29 CAIDO y el 31 puesto; sus bits 3..16 son 49,4 grados. Ni `open-gpu-doc`
ni `envytools` documentan ese registro en Ampere: se muestra como PROBABLE
(`49o?`), y el panel pinta su HISTORIA para confirmarlo con carga.

El panel se pisaba (`pcie 1/312G6GDDR6` en 17 columnas): ahora el bloque de la
3060 habla el idioma de los instrumentos, una cosa por renglon (`3060 49o?`,
la historia de un minuto con la raya de 83, `pstate P0`, `pcie 1/3 x16`, `vram
12G GDDR6`).

**L1b+ otra vez (24-09, 11:41): EL SENSOR ES DE VERDAD.** Tras un rato de DOOM
(70 fps por la CPU), `temp 52 grados` (`0xC0003408`) donde a las 11:25 marcaba
49: sube en P0, que es lo que hace una 3060 caliente. El "probable" gana peso.

**L1c (24-09, en codigo).** La leccion de nouveau (`r535/vmm.c`): con el GSP,
las tablas de paginas de un cliente NO las lleva el GSP-RM -- las lleva quien
hace de RM de la CPU, EN LA VRAM, y le dice al RM donde estan. Por eso L1c son
tres pasos:

- **L1c1**, `espacio`: `FERMI_VASPACE_A` (0x90F1, asa `0x90F10000`, hijo del
  dispositivo) con `NV_VASPACE_ALLOCATION_PARAMETERS` de 48 B, `index` GPU_NEW y
  `IS_EXTERNALLY_OWNED`. Cuarto objeto de `bmo_gpu_ga10x::objeto` (y del
  contrato); paso propio de `save mode`, para que si falla no tape el P-state.
- **L1c2**, `vram`: `bmo_gpu_ga10x::vram` (3 pruebas) -- la ventana PRAMIN
  (`0x1700` = base >> 16, un MiB en `BAR0 + 0x700000`, como `instmem/nv50.c`).
  Una pagina en UNA direccion fija, 64 MiB (dentro de la usable del GSP-RM,
  sobre el GOP y lejos de la WPR2): guardada, patron escrito y releido,
  devuelta, y la ventana como estaba. Kernel `IOMMU_OP_GPU_VRAM` (0x1D, motivo
  59, FLUSH del disco antes); el escritorio la pide solo si las regiones de L1a
  dicen que esa pagina es usable.
- **L1c3** (lo siguiente): construir el directorio de paginas de la GPU en la
  VRAM (formato de MMU de Ampere, `ver 2`) y `NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY`.

**L1c1 y L1c2 en el metal (24-09, 11:56): LA CPU ESCRIBE EN LA VRAM.** Los 23
pasos de `save mode` de una vez. `obj esp espacio 0x90F10000 (clase 0x90F1):
NV_OK en 1 ms (numero 7)`: el espacio de direcciones es nuestro. `vram PRAMIN
en 0x004000000: 1024 de 1024 palabras escritas y releidas; devueltas 1024;
ventana 0xFFF0 devuelta`: la ventana estaba en `0xFFF0` (la base 0xFFF00000,
la VRAM de lo alto, donde la dejo el firmware) y se le devolvio. Temperatura
48 grados en reposo.

Y la caja, arreglada tras la captura: TAB escribe la sugerencia ENTERA (y la
siguiente con cada TAB; antes solo el prefijo comun, y `gp` se quedaba en
`gpu`); la salida corta en el ultimo espacio y sigue BAJO EL VALOR (antes a
media palabra y en la columna 0: `control 0x000022000000` / `1405`); la
historia de la temperatura en su propia ventana (en escala fija salia plana).

**12:07: la caja, otra vuelta.** Los 23 pasos otra vez (`obj esp` NV_OK,
`vram` 1024 de 1024). El corte nuevo PEGABA palabras (`luzde RECORTE`,
`lodesarma`, `` `gpuinit` ``): el espacio que llegaba justo en el borde movia
la palabra de antes y se perdia el. Arreglado (`Output::saltar`), y la
continuacion cae bajo el ULTIMO CAMPO de la fila (el que empieza tras dos
espacios), no tras la primera palabra: `vetos DMA` y las filas de tres
columnas seguian en la columna 10 o bajo el numero. Las etiquetas de `campo`
y `label` llevan `:` y se pintan apagadas (`Output::etiqueta`), cada seccion
con una fila en blanco delante. Las sugerencias se eligen tambien con las
flechas (tras un TAB) y con un CLIC.

**L1c3 (24-09, en codigo): LA RAIZ.** `control::Control::Directorio`:
`NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY` (0x801813, 32 B) sobre el
DISPOSITIVO, con `physAddress` = `vram::DIRECTORIO` (0x4100000, 65 MiB, 1 MiB
sobre la prueba), `numEntries` 4 (la PD3 de Ampere: `page[0]` de 47 bits de
nouveau, `gp100_vmm_desc_16[4]`, 2 bits), APERTURE VIDMEM y `hVASpace` el
nuestro. El contrato ahora compara los PARAMETROS de cada control byte a byte
con los esperados (otra direccion, u otro objeto: NO). El kernel
(`IOMMU_OP_GPU_DIRECTORIO`, 0x1E, motivo 60) pone la pagina a cero por PRAMIN
y manda la RPC, UNA vez por arranque: despues el RM escribe en esa raiz lo que
se reserve. La op de control (0x1C) solo sirve PREGUNTAS. Paso `directorio`
de `save mode` (24).

**L1c3 en el metal (24-09, 12:25): EL RM ACEPTO NUESTRA RAIZ.** `pd: raiz PD3
en 0x004100000 (4 entradas, a cero): NV_OK   SET_PAGE_DIRECTORY en 0 ms (numero
8)`. Los 24 pasos de una vez; el espacio de direcciones de la GPU tiene
directorio, y es nuestro.

**L1d0 (en codigo): que dejo el RM en la raiz.** Al aceptar un directorio, el
RM "copia las PDE que gestiona el" (`ctrl0080dma.h`): puede haber colgado una
PD2 suya en la entrada 0 (su zona desde 4 GiB). `IOMMU_OP_GPU_RAIZ` (0x1F) lee
cada entrada por PRAMIN, solo lectura, y la fila `raiz` las dice. L1d1 mapea
en las vacias, o bajo la suya sin pisarla. `bmo_gpu_ga10x::mmu` (3 pruebas):
el formato v2 de Pascal/Ampere (PDE: VRAM = 1; PTE: VRAM = 0 y bit de
validez), los cinco niveles y `mapear` de la hoja a la raiz.

**L1d0 en el metal (24-09, 12:37): LA RAIZ ESTA ENTERA VACIA.** `raiz: [0]
vacia; [1] vacia; [2] vacia; [3] vacia`: al aceptar el directorio de un
espacio "de fuera", el RM NO colgo nada suyo. Todo el espacio de 49 bits es
nuestro para mapear.

**L1d1 (en codigo): el tramo.** El MiB de 65 MiB es de las tablas (la raiz y
detras una PD2, PD1, PD0 y PT, `vram::TABLAS`); el de 66 MiB, de las paginas:
16 (64 KiB, `vram::TRAMO`) vistas por la GPU en `vram::TRAMO_VA` = 8 GiB (lejos
de la zona del RM en 4 GiB). `mmu::mapear_tramo` (1 prueba) y
`vram::mapear_tramo` (1 prueba, sobre una VRAM de mentira de 2 MiB): tablas a
cero, las 16 PTE y las 4 PDE de la hoja a la raiz, RELEIDAS, y solo si la
entrada de la raiz sigue vacia. Kernel `IOMMU_OP_GPU_TRAMO` (0x20, motivo 61),
una vez por arranque; paso `tramo` de `save mode` (25), fila `tramo`. Ahi
viviran el GPFIFO, el USERD, el bloque de instancia y los datos de L1d.

**L1d1 en el metal (24-09, 12:47): LA GPU YA VE TU VRAM.** `tramo: VA
0x200000000 -> VRAM 0x004200000, 16 paginas: 20 de 20 entradas releidas`.

**L1d2a (en codigo): lo que el canal necesita saber antes de nacer.** Dos
PREGUNTAS mas en el contrato (`control::Control`), las dos sobre nuestro
subdispositivo y de solo lectura: `Motores` (`GET_ENGINES_V2`, 0x20800170,
340 B: cuantos y sus `NV2080_ENGINE_TYPE_*`) y `Metodos`
(`CE_GET_FAULT_METHOD_BUFFER_SIZE`, 0x20802A08: los bytes del bufer de metodos
que el RM le pide a un canal de copia). El bufer de comparacion del contrato
paso de 64 a 512 B (con su prueba). En el escritorio, `gspsalud::controlar`
sirve a toda pregunta (el P-state, ahora, tambien); `gspmotores`: la orden
`gpu motores`, el paso `motores` de `save mode` (26) y las filas `motores`
(la lista y el de copia elegido, el primer COPYn) y `metodos`.

Y la caja: `buscar` / Ctrl+F sobre Ejecutar (sobre una app sigue siendo su
pantalla completa): la coincidencia en medio de la ventana y resaltada, las
demas en su sombra, cada Enter la anterior; `buscar` no deja eco. Sin `:`
suelto en las filas de continuacion; la sangria colgante no pasa de la 40.

**Como se sabe (L1c3):** la fila `pd` dice `raiz PD3 en 0x004100000 (4
entradas, a cero): NV_OK`.

**Como se sabe (L1c1 y L1c2):** `obj esp` en NV_OK; `vram` dice 1024 de 1024,
devueltas 1024 y la ventana devuelta.

**Como se sabe (L1b):** las filas `obj cli`, `obj disp` y `obj sub` dicen
`NV_OK` (o `ya existia`, si se pidieron antes en el mismo arranque). Un
`parametros de otra medida` (0x3A) es un struct de otra version; `padre
invalido` (0x36), el arbol mal colgado.

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
