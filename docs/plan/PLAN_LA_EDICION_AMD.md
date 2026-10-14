# PLAN LA EDICION AMD -- BMO-X x86-64 AMD EXCLUSIVE: el kernel con la CPU y la GPU

> Escrito el **2026-10-14**. El propietario: *"si mi kernel tiene EXPANSION
> en AMD con GPU y CPU, entonces mi kernel administra TODO en GPU y CPU por
> completo, y puede empezar a tomar en serio dividir por completo las
> tareas: la CPU le guia en como tendra la logica y la GPU simplemente
> DIBUJA todo lo que la CPU imagina en tiempo real"*.
>
> Es el modelo de las consolas: PS5 y Xbox Series son CPU AMD y GPU AMD, con
> un sistema que conoce las dos enteras.

---

## 0. Por que AMD, y por que una EDICION y no un repositorio aparte

**Por que AMD.** En la 3060 el kernel le habla al GSP (el firmware de NVIDIA
corriendo en la tarjeta) y el GSP gestiona la GPU: desde Maxwell 2 los
falcons solo corren firmware firmado por NVIDIA, y sin el GSP no hay
relojes ni contextos. En AMD no hay un RM que lo haga todo: el driver
programa colas, memoria y contextos casi directamente, los registros estan
documentados (las cabeceras de amdgpu, los manuales de la ISA de RDNA) y el
driver de Linux es abierto entero. Siguen habiendo microcontroladores con
firmware firmado (SMU energia, MES y RLC las colas, PSP la seguridad), pero
con tareas acotadas: lo demas lo manda el kernel.

**Por que una edicion.** Un repositorio copiado son dos bases desde el
primer dia, y cada arreglo hay que llevarlo a mano. La edicion es la MISMA
base con un build que solo junta lo de AMD, y un guardian que lo hace
cumplir:

```text
   la base comun       kernel, ESTRATOS, director, TITAN++, PROTON-X,
                       VERRANO (la API), el juez, PROMETEO, la lente
   la edicion NVIDIA   drivers/gpu/ga10x (la 3060, con su GSP)
   la edicion AMD      drivers/gpu/rdna4 (la ranura ya existe), el perfil
                       del Zen, la lente de AMD (PLAN_LA_LENTE_BINARIA.md)
```

Un repositorio propio, solo el dia que el kernel de la edicion tenga que
decidir algo INCOMPATIBLE con la base (un planificador solo para los CCX
del Zen, la memoria unica de una APU). Para entonces se sabra por que.

---

## 1. Las piezas

### [ ] EA0 -- el guardian de la edicion

Un build `edicion amd` y su guardian: en ese binario no entra NADA de
NVIDIA (ni `ga10x`, ni sus clases, ni su firmware). Como `isa` con las
CPU: la pureza no se promete, se comprueba en cada build.

### [ ] EA1 -- la CPU (la mitad que ya esta)

El Ryzen 5 5600X: su perfil (`PERFIL/CPU.txt`, `cpu_vendor/ryzen_5_5600x`)
y la lente de AMD (LB3..LB12 de `PLAN_LA_LENTE_BINARIA.md`). Se hace en la
maquina de hoy.

### [ ] EA2 -- la GPU AMD en la mesa

Pide la tarjeta (la ranura de `drivers/gpu/rdna4` apunta a una RDNA 4, la
RX 9060 XT de `PLAN_VULKAN.md`). Lo que no esta, se mide: sin la tarjeta
no se escribe una linea que hable con ella. Lo publico se trae: las
cabeceras de registros y los manuales de la ISA de AMD; el firmware de
`linux-firmware`, tal cual (no es nuestro y no se toca).

### [ ] EA3 -- el kernel administra las dos: CPU piensa, GPU dibuja

El reparto del propietario, hecho regla del kernel:

```text
   la CPU    la logica: el juego, la fisica, la IA, que hay en la escena
   la GPU    todo lo que se dibuja, y el calculo en masa (PROMETEO: el
             mismo Programa en las dos, con el mismo resultado)
   el kernel decide donde va cada trabajo por lo que MIDE (la lente), no
             por lo que supone
```

Con el renderizado dirigido por la GPU (la GPU elige que se ve y se arma
sus propios dibujos, los dibujos indirectos de Vulkan y DX12) la CPU sube
la escena UNA vez y no paga un peaje por lote.

### [ ] EA4 -- la memoria compartida

En AMD, con la BAR grande (Smart Access Memory), la CPU ve TODA la VRAM:
la escena se escribe una vez donde la GPU la lee, sin copias. Medir antes
de fiarse: el ancho por el PCIe, en el metal.

---

## 2. Lo que NO se hace

- copiar el repositorio "para empezar limpio": dos bases se separan solas
- escribir el driver de la GPU AMD sin la tarjeta: se adivinaria
- tocar el firmware de AMD: se carga como viene, firmado
