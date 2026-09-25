# PLAN LA 3060 AFINADA -- aislar mas, medir, y solo entonces optimizar

> Escrito el **2026-09-25**, el dia despues de que la 3060 funcionara entera por
> primera vez: `save mode` 51 de 51, el compositor por GPU (`volcado` en 649 us
> la pantalla entera, contra ~27 ms de la CPU) y DOOM a 70 fps con 19866 tandas
> de la 3060 debajo.
>
> El propietario: *"analizar la GPU pero aislar bien y mejorar, y que faltarian
> OPTIMIZAR TODO ... porque eso es en base sin optimizacion, pero es mejor
> aislar MAS con CARRIL y obligando a que mis guardianes PROTEJAN de GPU para
> optimizacion, el porque y todo eso"*.
>
> Y el porque de las reglas, desde la historia de NVIDIA (el NV1, las firmas de
> Maxwell 2, el GSP de Turing, el 0x2504): [`../maestro/NVIDIA_HISTORIA.md`](../maestro/NVIDIA_HISTORIA.md).
>
> Las casillas de la 3060 que ya existian siguen en
> [`PLAN_LA_3060.md`](PLAN_LA_3060.md). Aqui va lo que sale de estudiarla ENTERA
> con un objetivo: que optimizarla no la afloje.

---

## 0. La regla que ordena todo esto

Es la ley 0 de [`../maestro/OPTIMIZACION_MAESTRO.md`](../maestro/OPTIMIZACION_MAESTRO.md):

```text
   1. CORRECTO   hace lo que dice, y hay una prueba que lo EJECUTA
   2. MEDIDO     hay un numero, y se sabe con que instrumento salio
   3. RAPIDO     y solo entonces, y solo donde el numero diga
```

Y de ahi el orden de este plan: **primero AISLAR** (que lo que se toque deprisa
no pueda romper lo que no se toca), **despues MEDIR** (cada fotograma partido
en sus trozos), **y lo ultimo OPTIMIZAR** -- cada casilla con quien pone su
presupuesto y el numero que dira que salio.

---

## 1. Lo que se estudio: la 3060 entera, en numeros

| donde | que | lineas |
|---|---|---|
| `Ultra_kernel_x86-64/kernel/src/ring0/dev/gpu*.rs`, `gpu_trabajo/`, `vblank.rs` | el driver en Ring 0: 10 ficheros, **9 ROJO y 1 AMARILLO** | ~5.570 |
| `platform/drivers/gpu/ga10x/` | el crate sin `unsafe` de registros, mensajes y recetas: 37 modulos, 197 pruebas | ~13.290 |
| `Ultra_userspace/services/director/src/commands/gsp*.rs`, `gpu.rs`, `iommu.rs` | las ordenes y sus filas en el escritorio | ~8.000 |
| `syscall/ops.rs` + `op_maquina.rs` | **65 ordenes** por UNA puerta (`OP_IOMMU`) y **83 motivos** de NO | -- |

Lo que se vio, en cinco frases:

1. **La frontera de memoria es buena.** La 3060 solo ve la RAM que BMO-X le
   PRESTA por la IOMMU (M0), el GSP corre en su procesador y no en Ring 0, y
   cada RPC que sale pasa por la lista blanca del contrato
   (`platform/drivers/gpu/ga10x/src/contrato.rs`).
2. **La puerta del sistema tenia un agujero** (seccion 2): miraba quien tiene
   la PANTALLA, y la pantalla se presta.
3. **Todo es sincrono.** Cada orden espera a la 3060 dentro del syscall: 11
   `spin_loop()` en Ring 0 giran hasta 20 ms antes de ceder
   (`dev/gpu_trabajo.rs::esperando`).
4. **Los empujes se escriben por PRAMIN**, la ventana de 1 MiB de la VRAM vista
   por MMIO (`platform/drivers/gpu/ga10x/src/vram.rs`): cada palabra es una
   escritura sin cache sobre el PCIe.
5. **Un fotograma de `gpu pantalla` tarda 4,0 ms y la 3060 solo 0,866** (metal
   25-09, `docs/metal/METAL_2026-09-25.md`). El resto es, sobre todo, la CPU
   COMPROBANDO: 1024 muestras por fotograma, cada una con `clflush`, una
   lectura de VRAM por el PCIe y el fractal recalculado en la CPU para
   compararlo (`dev/gpu_trabajo/pantalla.rs`).

---

## 2. El agujero que aparecio al estudiarla, y como se cerro

`fn iommu_` (`syscall/op_maquina.rs`) dejaba pasar a **quien tuviera la
pantalla** (`obj::fb::owner()`). Y el escritorio PRESTA la pantalla: DOOM la
reclama mientras juega. Con DOOM delante, DOOM pasaba la puerta de las 65
ordenes -- prestarle RAM del PC a la 3060, despertar su GSP, cegarla, apagar
la IOMMU.

**No hizo falta un mecanismo nuevo.** BMO-X ya tenia la pieza: la AUTORIDAD
(`task/autoridad.rs`), que se fija al nacer, solo desde Ring 0, y **no viaja a
los hijos**. Ya guardaba `REINICIAR`, `LANZAR` y `RED`. Ahora tambien
`MAQUINA`, y `DE_SISTEMA` la incluye: el escritorio la tiene porque lo arranco
el kernel; DOOM no, porque lo lanzo el escritorio.

```text
   antes    la PANTALLA           quien pinta ahora mismo       (se presta)
   ahora    la AUTORIDAD MAQUINA  quien arranco Ring 0          (no se presta)
            Y la PANTALLA         el volcado es de quien pinta
```

Por que no una capability: la 3060 SI es un objeto, pero un handle se pasa, y
lo que aqui se concede no debe poder pasarse a un hijo. Es la misma respuesta
que dio `RED`.

---

## 3. AISLAR MAS

### [x] A1 -- la autoridad `MAQUINA` en la puerta de la IOMMU y la 3060 (2026-09-25)

`task/autoridad.rs` (`MAQUINA`, dentro de `DE_SISTEMA`) y `fn iommu_` de
`syscall/op_maquina.rs`, que la pide ANTES que la pantalla. La biblioteca de
userland (`Ultra_userspace/userland/src/pantalla/roja.rs`) ya caia sola a la
CPU si el volcado por la 3060 le decia NO, asi que una app no pierde nada.

**Como se sabe:** `la_3060.py --check` falla si la llave desaparece (probado
quitandola: `P: fn iommu_ ya no pide autoridad::MAQUINA`). En el metal: con
DOOM delante, el klog diria `negado al pid` si DOOM lo intentara.

### [x] A2 -- el guardian `la-3060` (2026-09-25)

`toolchain/tools/la-3060/la_3060.py`, en el build detras de `escritores`. Cinco
reglas, cuatro ESTRICTAS porque hoy se cumplen enteras:

```text
   P  la PUERTA    `fn iommu_` pide autoridad::MAQUINA antes de la orden
   R  REGISTROS    solo dev/gpu*, dev/gpu_trabajo/ y dev/vblank.rs tocan
                   `Bar0(` o `gpu::bar0()`
   O  ORDENES      cada IOMMU_OP_* vale igual en kernel, ABI y userland;
                   ninguna repite valor; cada una tiene nombre y brazo
   M  MOTIVOS      cada IOMMU_NO_* vale igual en los tres; dos nombres no
                   comparten numero; cada uno tiene texto en el escritorio
   E  ESPERAS      los spin_loop() de la 3060 en Ring 0: TRINQUETE (11)
```

Por que estas y no otras: son exactamente lo que se rompe al optimizar
deprisa. Mover un registro a un sitio "mas comodo" (R), copiar una orden nueva
en dos de los tres sitios (O -- el 0x149 que choco el 25-09 era esto mismo en
los motivos), o agregar "una espera rapida" que gira (E).

**Como se sabe:** `python toolchain/tools/la-3060/la_3060.py --check` dice
`clean: ... 65 ordenes y 83 motivos iguales en los tres sitios; 11 esperas
girando`.

### [x] A7 -- la 3060 12G, y SOLO ella (2026-09-25)

`platform/drivers/gpu/ga10x/src/identidad.rs`: `10DE:2503` o `10DE:2504` (la
del propietario), GA106 (chipset 0x176) y 12288 MiB. La sonda
(`dev/gpu.rs::sondear`) deja SIN BAR0 cualquier otra: ni una lectura mas, ni un
VBLANK, ni una orden, y la fila `chip` lo dice (`GPU_LA_3060_12G`, bit 61 de
`INFO_GPU_CHIP`). La VRAM se mira cuando el GFW acabo, antes de FWSEC
(`dev/gpu_prestamo.rs`, motivo 84 `IOMMU_NO_OTRA_TARJETA`). El cargador solo
reinicia esa misma tarjeta (`s1_cpu/src/gpu_reinicio.rs`).

**Por que es aislar Y optimizar:** cada direccion de VRAM de este driver (el
tramo, las tablas, FRTS) se midio en esa tarjeta; en otra caerian en otro
sitio. Y lo que un driver general pregunta en cada arranque, aqui es una
constante. **Como se sabe:** `la_3060.py` regla I (las dos listas iguales y las
dos llamadas en su sitio) y las dos pruebas de `identidad.rs`.

### [x] A8 -- `opt-level = 3` con nombre, y vigilado (2026-09-25)

Ya lo era por herencia; ahora es `[profile.release.package."bmo-gpu-ga10x"]`
en `Ultra_kernel_x86-64/Cargo.toml` y `Ultra_userspace/Cargo.toml`, y la regla
O3 de `la_3060.py` exige que siga. Dicho claro: optimiza el codigo de la CPU
que habla con la 3060; lo que corre DENTRO de la 3060 son sus sombreadores,
que ya van en codigo maquina (`pantalla.rs::CODIGO`).

### [ ] A3 -- esperar a la 3060 con INTERRUPCION, y el trinquete E a cero

Hoy el nucleo que pide un trabajo gira hasta 20 ms (`GIRANDO_US` en
`dev/gpu_trabajo.rs`) y despues cede en cada vuelta. La pieza para no girar YA
EXISTE: el vector MSI de E2 (`dev/vblank.rs`, el 50 en la IDT) y la cola de
eventos del GSP. El camino: el empuje termina con un semaforo y un
NON_STALL_INTERRUPT; su interrupcion despierta al que espera por `WAIT`, como
el teclado.

**Por que es AISLAR y no solo rapido:** un nucleo girando dentro de un syscall
es un nucleo que el planificador no puede dar a nadie; el 24-09 un trabajo que
no volvia dejo el bus USB sin correr 1 s (la cabecera de `esperando` lo
cuenta). **Como se sabe:** `la_3060.py` baja su linea base
(`toolchain/tools/la-3060/LINEA_BASE.txt`) de 11 hacia 0, y el klog deja de
decir `el CPU lo tuvo ... durante` con la 3060 trabajando.

### [ ] A4 -- los CARRILES de la 3060: una carpeta que no mezcla

Nueve de diez ficheros son ROJO de arriba abajo, y dentro hay masas que no lo
son: `dev/gpu.rs` (AMARILLO) mezcla la sonda con lecturas puras (`info_*`,
VERDE), y `dev/gpu_libos.rs` (1116 lineas) mezcla la cola de RPC (ROJO) con
las filas que la describen. La ley R9/R10 de
[`../../FUERO/META-KERNEL_HARD.md`](../../FUERO/META-KERNEL_HARD.md) ya dice
como: `dev/gpu/` con fachada (`mod.rs` re-exporta, fuera no cambia nada) y
`roja.rs` (escribe registros, presta memoria, toca timbres), `amarilla.rs`
(instrumentos: salud, fotos al llegar, diagnosticos que si mienten mandan a
buscar al sitio que no es) y `verde.rs` (lecturas para el panel).

**Por que:** optimizar es tocar; el color dice que se arriesga al tocar. Un
fichero entero ROJO obliga a leerlo entero para cambiar una fila del panel.
**Como se sabe:** R10 en verde con la carpeta nueva, y `la_3060.py` con R
apuntando a `dev/gpu/roja.rs` y a nadie mas.

### [ ] A5 -- cada prestamo a la 3060 dice quien lo devuelve

`gpu_libos.rs` presta marcos con `escribible` y el volcado los suelta con
`suelta_si_es_de` (`dev/gpu_trabajo/volcado.rs`); el censo NEUTRO
(`toolchain/tools/censo-neutro/`) cuenta los marcos, pero no que cada prestamo
tenga su vuelta. Como `escritores` con los `static mut` del USB: una etiqueta
`[devuelve] <quien>` en la linea de cada prestamo, y el guardian que la exige.

**Por que:** una fuga de prestamo es la 3060 viendo RAM que el PC ya dio a
otro. **Como se sabe:** `la_3060.py` con una regla D y cero prestamos sin
etiqueta.

### [ ] A6 -- el driver sale de Ring 0 (lo grande, y va ultimo)

Hoy ~5.570 lineas ROJO de la 3060 corren en Ring 0: un fallo ahi es la
pantalla amarilla; en Ring 3 seria un servicio que se reinicia. La forma es la
de un microkernel: el kernel se queda con TRES cosas -- una ventana de BAR0
acotada por una lista de rangos, los prestamos de la IOMMU y la interrupcion
convertida en `WAIT` -- y el resto (recetas, colas de RPC, empujes) vive en un
proceso con la autoridad `MAQUINA`.

**Bloquea:** A3 (sin interrupcion, un proceso de Ring 3 tendria que girar) y
A4 (se muda lo que ya esta partido). **Como se sabe:** `dev/gpu*` en Ring 0
baja de ~5.570 lineas a la ventana, los prestamos y la interrupcion, y `save
mode` sigue en 51 de 51.

---

## 4. MEDIR ANTES

### [ ] B1 -- el fotograma de `gpu pantalla`, partido en sus trozos

**En codigo (25-09), falta verlo en el metal.** La tanda de
`commands/gspcomputo/pantalla.rs` suma lo que tardo la CPU en comprobar
(`cpu_us`, que el kernel ya empaquetaba) y cuantas muestras miro; la fila
`pantalla` y el panel de `gpu pantalla` dicen `partido: X us = 3060 Y +
comprobar Z (N muestras) + preparar y syscall W`, y `datos/` los anota.

Ya se mide el total (250 fps = 4,0 ms) y la 3060 (866 us), y el kernel ya
empaqueta `cpu_us`, lo que tarda en comprobar (`dev/gpu_trabajo/pantalla.rs`).
Falta decirlo: la fila `pantalla` de
`Ultra_userspace/services/director/src/commands/gspcomputo/pantalla.rs` con
**preparar / 3060 / comprobar / el resto**.

**Como se sabe:** los cuatro trozos suman el total, +-5 %.

### [ ] B2 -- el volcado por tanda: cajas, bytes y us de la 3060

**En codigo (25-09), falta verlo en el metal.** El kernel cuenta los bytes de
cada tanda enviada y las veces que la CPU tuvo que esperar a la anterior
(`dev/gpu_trabajo/volcado.rs`, selectores `COMO_VA_BYTES` y `COMO_VA_ESPERAS`
de `ga10x/src/volcado.rs`); la fila `volcado` dice MB/s contra el techo del bus
(`salud::mb_por_segundo`, con su prueba) y `cada foto` KiB por tanda y esperas.

`gspvolcado.rs` cuenta tandas; falta cuanto se copio y cuanto tardo cada una,
para saber si el presupuesto lo pone el escaner (16,7 ms a 60 Hz) o la 3060.

**Como se sabe:** la fila `cada foto` de `gpu volcado` dice bytes por tanda y
us, y DOOM a pantalla completa da un numero estable.

### [ ] B3 -- los relojes a los que va la 3060

La fila `pstate` de `commands/gspsalud.rs` ya pregunta el P-state; falta el
MHz del GR y de la memoria. Sin esto, C2 no se puede medir.

**Como se sabe:** `gpu salud` dice MHz en reposo y con `gpu pantalla` corriendo.

---

## 5. OPTIMIZAR -- cada una con quien pone el presupuesto

### [ ] C1 -- comprobar por MUESTREO, no las 1024 en cada fotograma

**En codigo (25-09), falta el numero del metal.** `pantalla::POCAS` (16) y
`muestra_de_paso` (rotan 7 por fotograma: en 64 pasan por las 1024, con su
prueba) en `ga10x/src/pantalla.rs`; el bit `PANTALLA_POCAS` (57) lo pide el
escritorio en los fotogramas de paso; el primero de cada tanda, todo `save
mode` y el que CARGA siguen comprobando las 1024.

**Presupuesto:** la 3060 (866 us por fotograma). **Lo que se gana:** la
mayor parte de los ~3,1 ms que no son de la 3060 (B1 dira cuanto exactamente).
Las 1024 muestras con el fractal recalculado en la CPU van en el PRIMER
fotograma de cada tanda y en `save mode`; en los demas, 16 repartidas.

**Lo que NO se hace:** quitar la comprobacion. Es el `[eje] CORRECCION` de
`dev/gpu_trabajo.rs`, y una 3060 que pinta mal y nadie mira es la que un dia
deja de pintar en silencio (regla 8 de OPTIMIZACION). **Como se sabe:** B1:
`comprobar` baja de milisegundos a decenas de us, y `gpu pantalla` pasa de 250
fps hacia los ~1000 que permite la 3060 a relojes de arranque.

### [ ] C2 -- L2: subir los relojes de la 3060

**Presupuesto:** el silicio. Hoy la 3060 va a relojes de ARRANQUE: nadie le
pidio mas. Es la casilla L2 de [`PLAN_LA_3060.md`](PLAN_LA_3060.md), por RPC al
GSP-RM y por tanto por la lista blanca del contrato. Es la mejora mas grande
de todas para el computo y el 3D, y no toca ni una linea del camino caliente.

**Como se sabe:** B3 dice MHz por encima de los de arranque, y el mismo
fractal baja de 866 us.

### [ ] C3 -- los empujes en RAM del PC prestada, no por PRAMIN

**Presupuesto:** el PCIe. Cada palabra de un empuje escrita por PRAMIN es una
escritura sin cache sobre el bus, y mover la ventana cuesta otra. En RAM
prestada (lo que ya se hace con el lienzo del volcado: IOVA de solo lectura
para la 3060) la CPU escribe en su cache y la 3060 lo trae de una vez.

**Lo que obliga:** cambiar donde vive una region es cambiar un CONTRATO (ley
12): cada lector de `EMPUJE` (`copia.rs`, `volcado.rs`, `computo.rs`,
`sombreador.rs` en `platform/drivers/gpu/ga10x/src/`) se mira antes. **Como se
sabe:** B1 `preparar` baja, y las 197 pruebas del crate siguen verdes.

### [ ] C4 -- dos tandas en vuelo: la CPU nunca espera a la 3060

**Presupuesto:** el que sea mayor de los dos, CPU o 3060, en vez de la suma.
Hoy un trabajo va de uno en uno (`BLUR_EN_MARCHA` y sus colegas en
`dev/gpu_trabajo.rs`). Con dos empujes alternos y la valla por numero de
secuencia (lo que ya hace el volcado, `dev/gpu_trabajo/volcado.rs`), la CPU
prepara la tanda N+1 mientras la 3060 hace la N.

**Bloquea:** A3 (esperar la N sin girar). **Como se sabe:** el total de B1 se
acerca al MAYOR de CPU y 3060, no a su suma.

### [ ] C5 -- M2: el page flip, cero copias

**Presupuesto:** el escaner. Es la casilla M2 de
[`PLAN_LA_3060.md`](PLAN_LA_3060.md): cambiar la superficie que lee el monitor
en el VBLANK en vez de copiar. Con ella se borra
`Ultra_userspace/userland/src/sin_gpu/`.

**Como se sabe:** `volcado` mueve 0 bytes por cuadro.

### [ ] C6 -- el escalado de DOOM por la 3060

**Presupuesto:** la CPU. El 80 % del blit de DOOM es `expansion` (M3 de
[`PLAN_LA_3060.md`](PLAN_LA_3060.md)). Con el motor de copia ya funcionando y
el sombreador de computo probado, escalar es un trabajo de la 3060.

**Como se sabe:** el `[perf]` de DOOM da `expansion` cerca de 0 us de CPU.

### El volcado: 649 us, y cuanto MAS se puede bajar

La cuenta, con lo medido el 25-09:

```text
   1920 x 1080 x 4 B          8.294.400 B
   en 649 us                  ~12.780 MB/s
   el techo de Gen3 x16       15.760 MB/s (antes de cabeceras)   -> ~81 %
```

**Ya va casi al techo del CABLE**: con las cabeceras de cada paquete PCIe, lo
que queda por ganar copiando desde la RAM del PC es poco. Asi que bajarlo no es
"copiar mas rapido", es la regla 3 de OPTIMIZACION: **hacer menos**.

```text
   que                                   hasta cuanto   casilla
   el enlace a Gen4 (la 3060 lo sabe)    ~325 us        lo sube el RM (L2)
   el lienzo DENTRO de la VRAM           ~50 us         M1 (BAR1) + C3
   no copiar: cambiar lo que mira        0 us           C5 (M2, page flip)
   y en cada fotograma: solo lo SUCIO    ya se hace     las cajas de la tanda
```

B2 dira el porcentaje real en la fila `volcado`, y cuantas veces la CPU tuvo
que esperar (la promesa es cero).

---

## 5b. VERIFICAR MAS FUERTE (D) -- lo que una sola vez no dice

El propietario (25-09): *"la propuesta para verificar mas en mi GPU MAS
FUERTE"*. `save mode` da cada trabajo UNA vez y lo juzga por muestras. Lo
que se escapa asi tiene tres formas, y cada D tapa una:

```text
   el tiempo      sale una vez y falla a la 500 (el anillo del GPFIFO ya lo
                  hizo, 24-09), o con la tarjeta caliente       -> D1
   el muestreo    1024 o 16 muestras de 2 millones de pixeles  -> D2
   lo de al lado  el trabajo sale bien pero la 3060 toco algo que no era
                  suyo, o el GSP-RM se quejo en silencio       -> D3, D4
```

### [ ] D1 -- `gpu aguante [minutos]`: la 3060 bajo carga larga

En codigo el 25-09 (`commands/gspaguante.rs`). Repite vuelta tras vuelta
TODOS los trabajos que ya salieron en ese arranque (fractal, triangulo,
escena, raster, color, giro, pantalla), cada uno con su juez, de 1 a 30
minutos (2 sin numero), cediendo el CPU entre trabajo y trabajo (el bus USB
sigue vivo). Se para en el PRIMER fallo y lo nombra, con la vuelta; la fila
dice por trabajo cuantos bien, la media y la peor vez, y la temperatura
al empezar, lo mas alta y al acabar; debajo, los avisos del GSP.
**Como se sabe:** `aguante: N vueltas en 120 s, sin un fallo`, y la
temperatura sube y se queda (si sube sin parar, el ventilador no lo mueve
el RM a nuestra carga: se anota).

### [ ] D2 -- la pantalla ENTERA, bit a bit, uno de cada N fotogramas

En codigo el 25-09 para `pantalla` (el video, despues): el bit 58
(`PANTALLA_ENTERA`) hace que el kernel rehaga y compare TODA la pantalla,
cediendo el CPU en cada linea; los comprobados y los malos quedan en
`DIAG_3D` 6 y 7. `gpu pantalla` lo pide uno de cada 64 fotogramas y `save
mode` en el ultimo de sus 8; ese tiempo de CPU sale de los fps.


`gpu pantalla` y `gpu video` miran 1024, 256 o 16 pixeles. Una vez cada 64
fotogramas, el kernel compara los 2 millones (1920x1080) con la cuenta de la
CPU: unos 30 ms de CPU, que en 64 fotogramas son ~0,5 ms por fotograma. Un
bit nuevo en el argumento (`TODA`) y el recuento en la fila. **Como se
sabe:** `pantalla: ... 2073600 de 2073600 en los fotogramas enteros`.

### [ ] D3 -- la frontera, DESPUES de todo

En codigo el 25-09 dentro de `gpu aguante`: un evento NUEVO de la IOMMU tras
un trabajo es un FALLO de esa vuelta, aunque su juez dijera bien.


Hoy la IOMMU dice los fallos de pagina de la 3060, y el unico esperado es
el de la prueba de frontera (0x2000_0000, en cada arranque). Al acabar
`save mode` y `gpu aguante`: el recuento de eventos de la 3060 tiene que
seguir en 1. Un fallo mas es la 3060 leyendo o escribiendo donde no se le
presto, aunque el trabajo saliera bien. **Como se sabe:** la fila `event`
dice `1 pendiente` y la fila `aguante` lo repite como `frontera: 1 (la de
la prueba)`.

### [ ] D4 -- el GSP-RM, callado

En codigo el 25-09 dentro de `gpu aguante` (`gspcola::contar_avisos`): un
aviso de fallo NUEVO en la cola del GSP es un FALLO de esa vuelta.


Tras cada trabajo, cero `RC_TRIGGERED` y cero `MMU_FAULT` nuevos en su cola
(ya se leen: `gspcola::avisos`). D1 ya los muestra al acabar; falta que
cuenten como FALLO de la vuelta en la que llegaron.

### [ ] D5 -- el programa, ejecutado en el anfitrion

Cada SASS se comprueba hoy con `nvdisasm` (que lee bien) y en el metal (que
corre bien). Falta el medio: un interprete de las ~40 instrucciones SM86 que
usamos (IMAD, IADD3, LOP3, SHF, ISETP, SEL, LDG, STG, BRA, EXIT...) en
`cargo test`, que corra el programa de `pantalla` o `video` sobre una
memoria de mentira y lo compare con la cuenta de la CPU. Un fallo de un bit
en una codificacion a mano se veria ANTES de arrancar el Ryzen.

---

## 5c. EL PASE DE LA 3060 (P) -- la burocracia una vez, y el radar en cada barrido

El propietario (25-09): *"la GPU puede tener lo mismo una sola vez la
burocracia y le viene el radar eso para exprimir con todo?"* -- **si**, y la
pieza ya tiene forma: `platform/shared/bmo-pase-gpu` (puro, 23 pruebas), la
misma ruta de `docs/identidad/LA_RUTA.md` que el GATE RED
(`platform/shared/bmo-puerta-red`), con UNA diferencia: **el latido es el
VBLANK** (E2, `Ultra_kernel_x86-64/kernel/src/ring0/dev/vblank.rs`), no 4 ms.
La pantalla se barre 60 veces por segundo; lo que se copie entre dos barridos
no se ve antes.

```text
   hoy (volcador ARMADO)                     con el pase
   ARMAR: el lienzo prestado una vez    ->   igual: ya era "pagar una vez"
   CAJA x N: un syscall por caja sucia  ->   N cajas en el buzon, 0 syscalls
   la ultima toca el timbre             ->   el latido del VBLANK lo toca
   ESPERAR: la CPU GIRA hasta 50 ms     ->   nadie gira: el latido mira el semaforo
   la comprobacion, cuando se pide      ->   el radar: muestras 1 de cada 16 barridos
   sin nadie mirando entre llamadas     ->   colgada (6 barridos), semaforo raro,
                                              muestras malas (3 seguidas): REVOCA
```

**Lo que se gana, sin inflar:** los syscalls son ~1000 ciclos cada uno
(`syscall/mod.rs`): 20 cajas son ~5 us, casi nada. Lo que se gana es lo que hoy
GIRA (la valla), el RITMO (uno por barrido: ni fotogramas que nadie ve ni
tearing) y sacar la comprobacion del camino del fotograma sin quitarla (regla
8). Es A3 sin esperar a la interrupcion de la 3060: la interrupcion que hace
falta ya existe, es la del monitor.

**Revocar no apaga la pantalla:** el lienzo se devuelve, el motivo queda en el
buzon, y el escritorio vuelve a `CAJA`/`ESPERAR`, que siguen ahi.

- [x] **P0 -- la politica, pura y con banco.** `bmo-pase-gpu`: `pase::juzgar`
      (8 preguntas en orden, la pantalla primero y el VBLANK como pregunta: sin
      latido no hay radar, y sin radar no hay pase), `buzon::Lado::leer` (copia
      una vez, numero que salta o caja fuera = mentira, mas de media pantalla
      sucia = UNA copia entera) y `radar::Radar::mirar`. **Como se sabe:**
      `cargo test -p bmo-pase-gpu`, 23 pruebas.
- [ ] **P1 -- el kernel: `GPU_PASE` y el buzon mapeado.** **En codigo
      (25-09), falta verlo en el metal.** Partido en DOS para que NVIDIA
      quede aislada por completo (el propietario: *"por si voy a tener mi
      GPU alternativos"*):

      ```text
         dev/pase_gpu.rs            NEUTRO: quien pide, el lienzo, el buzon
                                    (un marco, VA 0x2_0010_0000), la LAPIDA,
                                    las estaciones (soltar, morir, apagado) y
                                    el rasgo `Motor`
         gpu_trabajo/pase_nv.rs     el Motor de la 3060: lo que sabe del
                                    aparato, la medida, prestar y devolver
                                    (la IOVA y las tablas del volcador)
         la puerta (op 0x44)        elige el motor: `pase_nv::motor()`; otra
                                    tarjeta seria `.or_else(su_motor)`
      ```

      El guardian `la-3060` gana la regla **N (NEUTRO)**: `dev/pase_gpu.rs` y
      el crate no nombran ni un modulo de NVIDIA ni un registro (probado
      metiendo un `gpu_libos` a proposito: FAIL). El pase y el volcador se
      excluyen (misma IOVA), y el volcado de una vez tambien dice que no con
      el pase abierto. Motivo 86 (`IOMMU_NO_PASE`); el por que exacto, en
      ESTADO. **Como se sabe:** `gpu vblank` y despues `gpu pase` dice cinco
      `si` (ABRIR, el buzon, CERRAR, la lapida, ESTADO), y `gpu volcado`
      sigue verde despues.
- [ ] **P2 -- el latido en el VBLANK.** En el aviso de E2 (o un hilo que
      despierta con el): `Lado::leer` -> empuje del motor de copia -> ENVIADO
      y PAGADO al buzon -> `Radar::mirar`; muestras cuando `toca_mirar`.
      Revocar = `buzon::revocar` + `devolver_gpu`. **Como se sabe:** la fila
      `volcado` dice `esperas 0` y `tandas == barridos` con el escritorio
      quieto moviendo el raton.
- [ ] **P3 -- el escritorio usa el buzon.** `buzon::escribir` en vez de un
      `CAJA` por caja; si ESTADO != 0, vuelve al camino de siempre y lo dice
      en CABINA. **Como se sabe:** DOOM a 60/70 fps con 0 syscalls de volcado
      por fotograma (FRAPS-X lo pinta encima).
- [ ] **P4 -- el mismo pase para el video.** Hoy `gpu video` presta y devuelve
      el fotograma EN CADA llamada (~50 us de IOMMU para 85 paginas) y gira
      esperando; con el pase, un anillo de fotogramas prestado una vez y el
      mismo radar. **Como se sabe:** `video` sin `prestar_gpu` por fotograma
      en el klog.

### Lo que MAS falta optimizar, en orden (25-09)

```text
   1  C2  relojes de la 3060 (L2)      la mayor de todas, y no toca el camino caliente
   2  P   el pase (arriba)             la CPU deja de girar; un fotograma por barrido
   3  C1  muestreo en pantalla         ya en codigo, falta el numero del metal
   4  C4  dos tandas en vuelo          con el pase, casi sale sola (ENVIADO/PAGADO)
   5  C5  el page flip (M2)            0 bytes por fotograma: lo ultimo que queda
```

---

## 6. Las REGLAS ESTRICTAS de la 3060, juntas

El propietario: *"vamos a PONER REGLAS ESTRICTAS en GPU para respetar al GPU
MAXIMO"*. Las que vigila una maquina, y las que salen de la historia:

```text
   la-3060 (build)   P  la puerta pide la autoridad MAQUINA
                     R  los registros solo en dev/gpu*, gpu_trabajo/, vblank.rs
                     O  las 65 ordenes iguales en kernel, ABI y userland
                     M  los motivos iguales, sin choques, con su texto
                     E  las esperas girando, trinquete (11 -> 0)
                     I  la 3060 12G y solo ella, igual en crate y cargador
                     O3 bmo-gpu-ga10x a opt-level = 3, con nombre
   el contrato       cada RPC al GSP, por lista blanca (ga10x/src/contrato.rs)
   la historia       H1..H6 de ../maestro/NVIDIA_HISTORIA.md
   la ley 0          correcto, medido, rapido: nada se acelera sin su numero
```

---

## 7. Lo que NO se hace, aunque parezca mas rapido

- **Optimizar sin el numero de B1..B3 delante.** Es la ley 0.
- **Quitar una comprobacion entera** en vez de muestrear (C1): la correccion
  es un eje, no un coste.
- **Write-combining sin `sfence`** (ley 10 de `BITACORA.md`), ni cambiar el tipo
  de memoria de una region sin buscar a todos sus lectores (ley 12).
- **Firmware de terceros en Ring 0**, ni para ir mas rapido. El GSP sigue en
  su procesador y detras de la IOMMU (seccion 4 de
  [`PLAN_LA_3060.md`](PLAN_LA_3060.md)).
- **Una puerta que mire la pantalla para decidir quien manda.** La pantalla
  se presta; la autoridad no (A1).

---

## 8. El orden, y por que ese

```text
   hecho    A1 la puerta con MAQUINA      A2 el guardian la-3060
            A7 la 3060 12G y solo ella    A8 opt-level 3 con nombre
   en codigo  B1 B2 y C1 (25-09): falta el numero del metal
   1        B3         los MHz: sin esto, C2 no se puede medir
   2        C1         verlo en el metal: de 250 fps hacia ~1000
   3        C2         los relojes: la mejora mas grande sin tocar el camino
   4        A4         partir por carriles ANTES de tocar lo caliente
   5        A3 -> C4   interrupcion, y entonces dos tandas en vuelo
   6        C3 C5 C6   empujes en RAM, page flip, escalado de DOOM
   7        A5 A6      los prestamos con su vuelta, y el driver a Ring 3
   junto a todo  D1 en cada tanda de cambios (2 minutos); D2..D5 cuando se
                 toque la pantalla, el video o un programa nuevo
```

Medir primero porque C1 y C2 dependen de B1 y B3 para saber si salieron. A4
antes que A3 y C4 porque son los cambios mas grandes en codigo ROJO, y es
mejor hacerlos sobre una carpeta ya partida por colores. A6 va el ultimo:
es lo que mas aisla, y solo es posible cuando la 3060 ya no gira en un syscall.
