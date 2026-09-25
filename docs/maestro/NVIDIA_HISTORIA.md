# NVIDIA, SU HISTORIA -- por que la 3060 es como es, y que le toca a BMO-X

> Escrito el **2026-09-25**, a peticion del propietario: *"me gustaria que
> estudies el pasado de Nvidia y su historia ... porque?"*. No es un museo: cada
> etapa de abajo explica una pieza que BMO-X ya se encontro en su 3060 (el GSP,
> las firmas, los relojes de arranque, el 0x2504) y termina en una regla.
>
> El legal y el "traducir o reconstruir" estan en
> [`GPU_NVIDIA_MAESTRO.md`](GPU_NVIDIA_MAESTRO.md); las casillas, en
> [`../plan/PLAN_LA_3060.md`](../plan/PLAN_LA_3060.md) y
> [`../plan/PLAN_LA_3060_AFINADA.md`](../plan/PLAN_LA_3060_AFINADA.md).

---

## 1. La linea del tiempo, con lo que dejo en tu tarjeta

```text
   1993  nace NVIDIA (Jensen Huang, Chris Malachowsky, Curtis Priem)
   1995  NV1: superficies CUADRATICAS en vez de triangulos. Fracaso
   1997  RIVA 128: triangulos, como pedia Direct3D. La salva
   1999  GeForce 256: "la primera GPU" -- transformar e iluminar en la tarjeta
   2001  GeForce 3: sombreadores PROGRAMABLES (y la Xbox)
   2006  G80 (GeForce 8800): sombreadores UNIFICADOS, los SM de hoy
   2007  CUDA: la GPU deja de ser solo para pintar
   2010  Fermi   2012 Kepler   2014 Maxwell   2016 Pascal
   2014+ Maxwell 2: el firmware de los falcon tiene que ir FIRMADO
   2017  Volta: los nucleos tensor
   2018  Turing: RT, y el GSP -- un RISC-V dentro de la tarjeta
   2020  Ampere (GA10x)
   2021  RTX 3060 12 GB (GA106), y la LHR: 10DE:2504, LA TUYA
   2022  NVIDIA publica sus modulos de kernel ABIERTOS (Turing y despues)
   2023+ nouveau con GSP; NVK (Vulkan libre); nova-core, en Rust
```

### NV1 (1995): la leccion que costo casi la empresa

NVIDIA aposto por dibujar con superficies cuadraticas mientras el mundo --y
Microsoft con Direct3D-- se iba a los triangulos. El NV1 no tenia a quien
venderle. El RIVA 128 hizo lo contrario: lo que el estandar pedia, bien hecho.

**Lo que dejo:** una GPU es una maquina de triangulos y de sombreadores sobre
interfaces publicadas (clases, metodos, colas). **Regla para BMO-X:** se habla
con la 3060 por SUS interfaces --las clases de `clc797.h`, el GPFIFO, las RPC
del GSP--, nunca por atajos inventados.

### G80 y CUDA (2006-2007): por que tu 3060 es un monton de SM

Los sombreadores dejaron de ser de vertices o de pixeles: todos iguales,
agrupados en SM, programables en C. Es la razon por la que BMO-X pudo pintar
un fractal a pantalla completa con un programa de 62 instrucciones
(`platform/drivers/gpu/ga10x/src/pantalla.rs`): el motor de COMPUTO es el
mismo silicio que pinta.

**Regla:** lo que la 3060 hace bien es MUCHO trabajo igual a la vez. Un
trabajo por syscall, esperado a mano, la desperdicia (A3 y C4 del plan
afinado).

### Maxwell 2 (2014-2015): la pared de las FIRMAS

Desde aqui, los microcontroladores de la tarjeta (los *falcon*: PMU, SEC2,
GR...) solo aceptan firmware firmado por NVIDIA. El proyecto nouveau --que
desde 2006 hacia el driver libre por ingenieria inversa-- se quedo sin poder
SUBIR LOS RELOJES: la tarjeta arrancaba a sus relojes de arranque y ahi se
quedaba. Durante casi ocho anios, una NVIDIA moderna con driver libre iba a
una fraccion de su velocidad.

**Lo que dejo en BMO-X:** es exactamente lo que se ve hoy. La 3060 corre el
fractal a relojes de ARRANQUE porque nadie le pidio mas. **Regla:** los
relojes solo se piden por el GSP (L2), con su firmware, tal cual. Nunca se
intenta esquivar una firma: no se puede, y el intento es tiempo tirado.

### Turing (2018) y el GSP: por que existe tu RISC-V

Turing trajo el GSP: un RISC-V dentro de la tarjeta. Y en 2022 NVIDIA hizo
algo que cambio todo: publico sus modulos de kernel ABIERTOS, pero solo para
Turing y despues. El truco es el GSP: la parte secreta del driver (el
*Resource Manager*) se mudo DENTRO de la tarjeta, como firmware firmado de
~69 MB, y lo que queda en el kernel puede ser abierto.

**Lo que dejo en BMO-X:** todo L0 y L1. El booter, FWSEC, la WPR2, las colas
de RPC, el 0x15 del booter y la "3060 caliente" son la forma de ese reparto.
Y es por lo que BMO-X puede existir: los modulos abiertos (y nouveau y
nova-core, que leen lo mismo) dicen como hablarle al GSP. **Regla:** el GSP
corre en SU procesador y habla por colas; BMO-X le presta memoria por la
IOMMU y nada mas. Nunca firmware de terceros en Ring 0.

### 2021: la RTX 3060 12 GB, y por que la tuya es la 0x2504

La 3060 salio en febrero de 2021 con 12 GB --mas que la 3070--, en plena
fiebre de minado. NVIDIA le puso un limitador de minado (LHR, *Lite Hash
Rate*) y un driver beta lo desactivo por error a las pocas semanas. Las que se
fabricaron despues salieron con otro dispositivo PCI: **0x2504, la LHR**. La
tuya (`10DE:2504`, subsistema `1462:397D`, una MSI) es de esas. Para lo que
BMO-X hace --computo, copias, pintar-- el LHR no cambia nada.

**Regla:** BMO-X maneja la 3060 12G y SOLO ella
(`platform/drivers/gpu/ga10x/src/identidad.rs`): 0x2503 o 0x2504, GA106,
12288 MiB. Cada direccion de VRAM de este driver se midio en esa.

### 2023 en adelante: el camino que BMO-X tiene delante

nouveau aprendio a usar el GSP (con el mismo firmware r535/r570), NVK dio un
Vulkan libre y conforme, y nova-core empezo un driver nuevo en Rust. BMO-X
lee a los tres (`kernel_gsp_tu102.c` de los modulos abiertos, nouveau,
nova-core) y ya lo cita en su codigo. NVK tardo unos dos anios con varios
expertos en ser un Vulkan completo: el "Vulkan de esta casa" de L2 es otra
cosa, mas chica y a proposito.

---

## 2. Las reglas que salen de la historia

| # | regla | de donde sale | quien la hace cumplir |
|---|---|---|---|
| H1 | por SUS interfaces (clases, GPFIFO, RPC), nunca por atajos | NV1 | el contrato de RPC (`ga10x/src/contrato.rs`) |
| H2 | mucho trabajo igual a la vez; ni un syscall por pixel | G80, CUDA | el trinquete E de `la-3060` |
| H3 | los relojes, solo por el GSP; ninguna firma se esquiva | Maxwell 2 | L2, por la lista blanca |
| H4 | el GSP en su procesador, detras de la IOMMU; nada ajeno en Ring 0 | Turing, 2022 | `la-3060` P y R, CODEOWNERS |
| H5 | la 3060 12G y solo ella | 2021, el 0x2504 | `la-3060` I |
| H6 | apagar el GSP en orden, o reiniciar la tarjeta | el reparto del GSP | `reboot`, el cargador (`gpu_reinicio.rs`) |

---

## 3. Lo que este documento NO afirma

Las fechas son las publicas y las que la casa ya uso; lo que NVIDIA no publico
(por que eligio cada cosa por dentro) no se inventa aqui. Donde una regla
depende de un dato, el dato esta en el metal (`docs/metal/`) o en el codigo,
no en esta historia.
