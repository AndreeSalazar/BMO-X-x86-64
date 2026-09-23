# GPU NVIDIA MAESTRO -- traducir en vez de reconstruir?

> Escrito el **2026-09-07**. Sigue el metodo de `SMP_MAESTRO.md`: **que copiar
> del mundo y que seria un error copiar**, antes de una sola linea de codigo.
>
> La pregunta la trajo el propietario con su motivo economico dicho de frente:
>
> > *"en vez de reconstruir todo PERFIL a Nvidia, sino TRADUCIR a mi BMO-X, eso
> > es posible? Para no tener que gastar mucho esfuerzo... con RTX 3060 12G por
> > mi economia no podre comprar la GPU AMD"*
>
> Su hermano mayor es `platform/drivers/gpu/rdna4/PLAN_VULKAN.md`, que da por
> supuesta una RX 9060 XT. **Este documento no lo reemplaza: le pregunta cuanto
> de el sobrevive si la tarjeta es otra.** La respuesta corta es: casi todo,
> menos lo unico que parecia el trabajo.

---

# 0. ★★ LA RESPUESTA, ANTES DE LOS DETALLES

```text
   traducir el DRIVER de AMD a Nvidia          IMPOSIBLE, y no por lo legal
   traducir el PERFIL, el plan y la FORMA      YA ESTA HECHO. Es casi todo
   necesitar la GPU para lo que te frena HOY   NO. Tu cuello es el blit
```

**La palabra "traducir" tiene dos significados aqui y solo uno funciona.**

---

# 1. Lo legal: tienes razon, y NO es lo que te bloquea

El propietario trajo el dato bien: **tres a cero**. Los casos son reales y esa es la
cuenta.

| caso | anio | quien gano |
|---|---|---|
| Sega v. Accolade | 1992 | **Accolade** -- copiar para entender es uso legitimo |
| Sony v. Connectix | 2000 | **Connectix** -- emular la BIOS de PSX, legal |
| Sony v. Bleem | 2000 | **Bleem** -- capturas comparativas, uso legitimo |

★ **Y en Europa es todavia mas claro**, que es lo que aplica aqui. La Directiva
2009/24/CE, articulo 6, permite **explicitamente** descompilar para conseguir
interoperabilidad. No es una defensa que haya que ganar en un juicio como el
*fair use* americano: es un derecho escrito, con sus tres condiciones (que lo
haga alguien con licencia de uso, que la informacion no este ya disponible, y
que se limite a las partes necesarias).

## ⚠ Pero esto NO es lo que te frena, y creerlo cuesta meses

**Para Nvidia casi no hay que hacer ingenieria inversa.** Desde mayo de 2022
Nvidia publica el codigo del lado del kernel:

```text
   open-gpu-kernel-modules     MIT / GPLv2, FUENTE PUBLICADA
   soporta Turing en adelante  -> tu RTX 3060 (GA106, Ampere) ENTRA
   trae cabeceras de registros -> no es una caja negra
```

Asi que el permiso legal que estabas buscando **ya no hace falta pedirlo para lo
principal**: no hay que descompilar lo que viene con licencia MIT.

> El muro no es un abogado. Es un microcontrolador con firma.

---

# 2. Por que el DRIVER no se puede traducir

Esta es la parte que hay que decir sin adornos, porque es la que ahorra el
esfuerzo mal gastado.

**RDNA y Ampere no comparten nada donde importa.** No es como traducir de un
dialecto a otro; es que no hay dos textos que emparejar:

```text
   el juego de instrucciones del sombreador    distinto entero
   el formato de los paquetes de comandos      distinto entero
   los registros de configuracion              distintos, y sin correspondencia
   el motor de display                         distinto entero
   como se arranca un canal de trabajo         distinto entero
```

** No existe una capa intermedia porque **no hay nada que interpretar**: un
paquete PM4 de AMD y un metodo de un canal de Nvidia no son dos formas de decir
lo mismo, son dos maquinas distintas. Escribir un traductor seria escribir los
dos drivers y ademas el traductor.

---

# 3. ★★ Lo que SI se traduce, y es casi todo lo que ya tienes escrito

Aqui esta la buena noticia, y es grande. **Lo que costo escribir de
`PLAN_VULKAN.md` no eran los registros: era el pensamiento.** Y ese es
independiente del fabricante.

| lo que ya esta escrito | sobrevive al cambio de tarjeta? |
|---|---|
| **Meta A vs Meta B** (acelerar el compositor / correr juegos) | ★ **entera**. Es la separacion mas valiosa del documento |
| La ruta B1: **Vulkan por software** | ★ **entera, y ni se entera**: no toca la GPU |
| Que Vulkan abarata y que no (las tres partes) | entera |
| Que un juego pide una LISTA de caracteristicas, no una version | entera |
| Que Vulkan es el 30% de lo que toca un juego | entera |
| ⚠ **El muro del firmware firmado** | entera -- y es el mismo muro, ver abajo |
| Los nombres de registros de RDNA | **no**, y son la parte chica |

★ Y lo mismo el resto de la casa: la enumeracion de PCI, el juez de la cesion de
MMIO (`bmo-mmio-juicio`), el veto `PisaRam`, la forma de un anillo de comandos
con su timbre, el perfilado por LEY 24. **Nada de eso sabe de que fabricante es
la tarjeta.**

> El PERFIL no se reconstruye porque el PERFIL no era de AMD. Era de BMO-X.

---

# 4. La asimetria real AMD / Nvidia, y donde esta el muro de cada uno

| | AMD RDNA | Nvidia Ampere |
|---|---|---|
| Especificaciones publicas | **si**, documentos completos de registros e ISA | **no** |
| Codigo del kernel publicado | si (amdgpu, GPL) | **si** (`open-gpu-kernel-modules`, MIT/GPL) |
| Cabeceras de registros | en los PDF | **dentro de ese codigo** |
| El muro | **el PSP** y su firmware firmado | **el GSP** y su firmware firmado |
| Cuanto pasa por el muro | parte | ★ **casi todo, y crece con cada generacion** |

## ⚠ El GSP, dicho claro

Desde Turing, Nvidia mete un procesador propio dentro de la GPU --el **GSP**, GPU
System Processor-- y le pasa el trabajo de gestionar el chip. El modulo abierto
de Nvidia es, en buena parte, **un mensajero que habla con ese firmware**.

```text
   el firmware del GSP viene FIRMADO
   no se puede sustituir por uno propio
   se puede redistribuir, pero no auditar ni cambiar
```

★ **Y esto no es peor que AMD, es la misma clase de muro** -- `PLAN_VULKAN.md`
ya lo tiene escrito como *"el muro real: el PSP y el firmware"*. La diferencia
es de grado: en Nvidia pasa mas cosa por el, y en AMD hay mas documentacion
publica para lo que queda fuera.

---

# 5. ★★ Y AHORA LA PREGUNTA QUE DE VERDAD DECIDE: para que quieres la GPU?

Esto es lo que hay que contestar antes que nada de lo anterior, y esta medido en
esta casa, no estimado:

```text
   [MEDIDO]  DOOM a 640x400 ya va a tope
   [MEDIDO]  a 1600x1000 el deficit ENTERO es el blit: ~300 MB/s al framebuffer
   [DATO]    no hay V-Sync ni VBlank. Volcar la pantalla son 27,6 ms
             contra los 16,7 de un fotograma
```

** Ninguna de esas tres cosas necesita sombreadores, ni CUDA, ni Vulkan. Lo que
te falta de una GPU, HOY, son tres piezas mucho mas chicas:

```text
   1. un MOTOR DE COPIA (DMA) que mueva los pixeles en vez del CPU
   2. la INTERRUPCION DE VBLANK, para que el compositor deje de pintar a ciegas
   3. cambiar de modo de video, y ni eso: el framebuffer del UEFI ya sirve
```

★ Eso es **exactamente la Meta A** de `PLAN_VULKAN.md`, que aquel documento ya
califica de *"alcanzable y bien planificada"* y del medida *"del driver de
AHCI"*. **La Meta A no exige AMD.** Exige un motor de copia, y eso lo tiene
cualquier GPU de los ultimos veinte anios.

---

# 6. Lo que NO se sabe, y hay que averiguar antes de prometer nada

Se escribe aparte porque es la unica parte de este documento que no es un hecho
comprobado. **Un maestro que mezcla lo que sabe con lo que supone es el que
manda a alguien a perder tres meses.**

```text
   [SIN COMPROBAR]  se puede mover un motor de copia de Ampere SOLO por MMIO,
                    sin el firmware del GSP?  <- ESTA es la pregunta que decide
   [SIN COMPROBAR]  el cambio de modo de video, igual
   [SIN COMPROBAR]  cuanto del modulo abierto es utilizable fuera de Linux --
                    depende de las APIs del kernel de Linux por todas partes
```

★ **La primera es la unica que importa.** Si la respuesta es *"si"*, la Meta A
sobre la 3060 es un proyecto del medida del AHCI y merece la pena. Si es *"no,
todo pasa por el GSP"*, entonces la Meta A sobre Nvidia cuesta arrancar y hablar
con un firmware firmado, que es otro proyecto entero -- y en ese caso la
respuesta correcta **no es comprar otra tarjeta**: es la ruta B1 y seguir con el
framebuffer del UEFI.

---

# 6b. ★★ LA RESPUESTA A LA SECCION 6 (2026-09-23)

**Si: el motor de copia y la pantalla de Ampere se mueven SIN el firmware del
GSP.** nouveau tiene ese camino escrito y publicado (MIT), y sin firmware:

```text
   nvkm/engine/fifo/ga102.c   los canales de comandos (runlist, doorbell)
   nvkm/engine/ce/ga102.c     el motor de COPIA
   nvkm/engine/disp/ga102.c   la PANTALLA: modo, VBLANK, flip
   ---------------------------------------------------------------
   lo UNICO que pide firmware firmado es el 3D (GR): FECS/GPCCS por ACR,
   que corre en SEC2 -- y ni eso pasa por el GSP
```

★ Asi que la Meta A sobre la 3060 **es del medida del AHCI**, como la seccion 6
decia que seria si la respuesta era si. Y el motivo del documento cambio por el
camino: el blit ya no es el cuello (DOOM a 70 fps, blit 258 us, 23-09). Lo que
de verdad falta hoy es el **VBLANK** -- saber cuando la tarjeta acaba de barrer
la pantalla para que el compositor deje de pintar a ciegas.

## ★ Lo que dejo FastOS (abril-mayo 2026), para no perderlo otra vez

FastOS, el antecesor de esta casa, intento ARRANCAR EL GSP de esta misma
tarjeta. Su codigo vivio en `kernel/src/drivers/gsp/` (~2.300 lineas) y en
`Driver_Canon GA106/`, y se borro el 09-05 (`0e43d7f34`); sigue en el git. Su
`gputest` dio 12/15 y **los tres FAIL no eran muros del hardware**:

| test | que paso de verdad |
|---|---|
| T05 `PMC_ENABLE = 0x40000000` | en Ampere ese registro ya no enciende los motores; el test esperaba lo de antes |
| T10 PRIV ring `TIMEOUT` | FastOS RESETEABA el anillo PRIV que la VBIOS ya habia dejado vivo: eso lo rompe, y de ahi salen despues los `0xBADF....` |
| T13 BAR1 = 0 | BAR1 es de 64 bits y esta por encima de 4 GiB: se leyo media |

Y la cadena del GSP llego mas lejos de lo que parecia:

```text
   1. FWSEC-FRTS (de la VBIOS) monta la WPR2        LOGRADO: err=0, WPR2 SET
   2. SEC2 corre booter_load (firmado)              aqui murio: SEC2 DMA Timeout
   3. booter carga GSP-RM y arranca su RISC-V       no llego
   4. RPC por colas en memoria compartida           no llego
```

Por que murio en el paso 2 (leido en su `loader.rs`, no supuesto):

1. **El reset del SEC2 estaba mal**: al GSP lo reseteaba por el registro de
   MOTOR (`0x1103C0`) y esperaba el borrado de memoria; al SEC2 solo por
   `0x840094`, con vueltas de espera y sin mirar nada. `CPUCTL = 0xBADF5620`
   = el motor no contestaba.
2. **No programaba el FBIF**, que es lo que le dice a la DMA del falcon que lea
   de la RAM del PC. Sin eso la DMA mira la memoria de la tarjeta.
3. **Pasaba punteros virtuales como fisicos** a la DMA (`booter.as_ptr()`).
4. **Elegia la firma por el FICHERO y no por el FUSIBLE** del chip
   (`FUSE_OPT_FPF_SEC2_UCODE1_VERSION`): nouveau y nova-core le preguntan al
   hardware. Es LEY 24 otra vez.

## ⚠ El GSP y la soberania, dicho una vez

El GSP-RM son ~69 MB **cerrados**, sacados de `linux-firmware` o del driver de
Windows. Y la ironia: el GSP **ya es RISC-V** (`ELF64 RISC-V`); lo que lo
cierra no es la arquitectura, es la **firma** -- la ROM de la tarjeta solo
acepta codigo firmado por Nvidia en SEC2 y en el GSP. **En esta tarjeta no se
podra reemplazar nunca**, sea el CPU x86 o RISC-V. La soberania de verdad en la
GPU es silicio sin candado (la vision DPU), no esta.

Y aislarlo tiene nombre en esta casa --el NEUTRO, fila `GPU+PSP`-- pero HOY es
solo una palabra: sin la **IOMMU** encendida, el GSP ve TODA la RAM por DMA.

## ★ El orden, decidido por el propietario el 23-09

```text
   1. VBLANK y (si hace falta) el motor de copia, SIN firmware   <- ahora
   2. la IOMMU (AMD-Vi) encendida: el NEUTRO pasa de censo a frontera
      -- y protege tambien del disco, el USB y la red
   3. solo entonces, si algun dia se quiere 3D o computo: el GSP,
      DETRAS de la IOMMU y como fila NEUTRO declarada. Nunca antes
```

Las casillas de ese orden --Early, Mid y Late, con lo que bloquea cada una y
como se sabe que quedo hecha-- viven en
[`../plan/PLAN_LA_3060.md`](../plan/PLAN_LA_3060.md).

---

# 7. El orden, y el primer paso ya esta dado

```text
   [x] 0. que BMO-X SEPA que la tarjeta esta ahi
          hecho el 07-09: `dev/portero.rs` censa el bus y dice
          "hay una GRAFICA NVIDIA y BMO-X no tiene codigo para ella"
   [ ] 1. leer su identidad: BAR0 y el registro de arranque que da el chip
          una lectura de MMIO. Confirma GA106 sin tocar nada
   [x] 2. contestar la pregunta de la seccion 6 LEYENDO el codigo publicado
          hecho el 23-09: SI, sin firmware (seccion 6b)
   [x] 3. decidir: Meta A sobre Nvidia, empezando por el VBLANK (6b)
   [ ] 4. el VBLANK, primero PREGUNTADO: la linea que barre la cabeza y la
          geometria del modo, en solo lectura -- "se puede" lo dice el metal
```

## ⚠ Y el paso que NO va aqui

**Comprar la AMD no es el paso 4.** Si la Meta A resulta cara sobre Nvidia, lo
que gana no es otra tarjeta: es que el cuello de hoy --el blit-- tiene otras
respuestas mas baratas que ya estan escritas en
`EL_COMPOSITOR_Y_EL_ESCANER.md`, y ninguna cuesta dinero.

> La tarjeta que tienes no bloquea nada de lo que estas construyendo. Bloquea
> una meta que ya estaba aparcada por escrito, y por otras razones.
