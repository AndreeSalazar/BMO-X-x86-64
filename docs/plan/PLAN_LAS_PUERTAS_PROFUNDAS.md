# PLAN LAS PUERTAS PROFUNDAS -- que la CPU hable mas directo, porque el kernel lo deja

> Pedido por el propietario (2026-10-18): *"que mi kernel ayude por completo
> pero que puedan bypasear MAS profundo con que mi CPU hable gracias a mi
> kernel"*, y despues de ver las dos salidas: *"podria ser ambas porque tengo
> QEMU"*. Y su otro repositorio, donde ya habla con la CPU byte a byte:
> [`AndreeSalazar/TALK-CPU-x86-64`](https://github.com/AndreeSalazar/TALK-CPU-x86-64).
>
> **Lo que afirma**: que un programa de Ring 3 puede usar una instruccion que
> hoy le da `#GP` -- un puerto de E/S, un contador de rendimiento, un MSR --
> SIN que el kernel la ejecute por el cada vez, y sin dejar de estar
> encerrado: el kernel abre UNA puerta concreta a UNA tarea concreta, lo dice
> en el `.bex`, y la CPU hace cumplir el resto.
>
> **Como se cae**: una puerta que se abre para todos (IOPL 3: todos los
> puertos); un puerto que llega a un controlador que el kernel tambien usa
> (el disco, el PIC, el PCI: `0xCF8`/`0xCFC`); un MSR que se escribe; o una
> puerta que sigue abierta cuando la tarea muere.
>
> [!] **Todo esto es Ring 0** (`.github/CODEOWNERS`: lo que el kernel enlaza es
> del propietario, para siempre). Este plan lo ESCRIBE y dice como probarlo;
> no cambia una linea del kernel. Cada casilla se abre cuando el propietario
> la decide (seccion 4).

---

# 1. Lo que hay hoy, medido en el arbol

| | Hoy | Donde |
|---|---|---|
| un puerto desde Ring 3 | `#GP`: el TSS no tiene mapa de E/S (`iomap_base` apunta detras de su limite) | `Ultra_kernel_x86-64/kernel/src/ring0/plat/smp/tss.rs:151` |
| `rdpmc` desde Ring 3 | `#GP`: CR4.PCE no se pone (el kernel solo toca CR4.MCE) | `ring0/plat/porque.rs:157-164` |
| un MSR | solo Ring 0 (`rdmsr` en `mm/vmm/roja.rs`, `amarilla.rs`) | la CPU: `rdmsr` es CPL 0, siempre |
| lo que SI llega a Ring 3 | `invoca(cap, op, ...)`: el kernel comprueba y responde (`TASK_OP_INFO`, 36 ciclos) | `ring0/syscall/ops.rs:240`; `inti/sondas/pulso.inti` |
| lo que INTI dice de esto | *"`crudo` marca: aqui nadie comprueba por ti"*; y la lista de lo que Ring 3 no puede | `toolchain/lang/inti/README.md`, `sondas/cpu.inti` |

O sea: la CPU ya separa lo que una app puede tocar, y BMO-X no le abre nada.
Las tres puertas de abajo son las que el x86-64 tiene para abrir UNA cosa.

---

# 2. Las tres puertas (y la que no existe)

```text
   PUERTA          QUE ABRE                     QUIEN COMPRUEBA DESPUES
   ---------------------------------------------------------------------------
   PP1 el mapa     unos PUERTOS de E/S, a una   la CPU: in/out a un puerto que
       de E/S      tarea (su TSS, su bitmap)    no esta en el mapa es #GP
   PP2 CR4.PCE     rdpmc: leer los contadores   la CPU: solo lee; programarlos
                   de rendimiento               (wrmsr) sigue siendo Ring 0
   PP3 el MSR      leer un MSR de una LISTA     el kernel, en cada llamada:
       por cap     BLANCA, por invoca           una capability, no una puerta
   ---------------------------------------------------------------------------
   (no existe)     escribir un MSR, CR0/CR3,    nadie: eso ES el kernel. Lo
                   lgdt/lidt, cli/hlt           que lo pida es un driver
```

**PP1 -- el mapa de E/S por tarea.** El TSS de x86-64 lleva un bitmap de
65536 bits: un 0 deja el puerto, un 1 da `#GP`. Hoy no hay bitmap. La puerta:
cada tarea con permiso lleva el suyo (8 KiB, o solo hasta el ultimo puerto que
pida), el kernel lo copia al TSS del nucleo al cambiar de tarea y lo borra al
salir. El `.bex` declara los puertos (`necesita puertos 0x60, 0x64`), el
manifiesto los pide (U2) y el juez de carga rechaza los del kernel.
- **Lo que gana**: un `in al, 0x60` cuesta unos pocos ciclos, no una syscall
  (~36 del camino de `OP_INFO`, mas el viaje).
- **Lo que cuesta**: el cambio de tarea copia el bitmap (o cambia el TSS);
  y un puerto compartido es un fallo silencioso. **La lista negra se escribe
  antes que la blanca**: PIC (`0x20`, `0xA0`), PIT, PCI (`0xCF8`-`0xCFF`),
  el disco y lo que use el kernel.

**PP2 -- CR4.PCE.** Con el bit 8 de CR4, `rdpmc` funciona en Ring 3. Es la
puerta del METRO: medir ciclos e instrucciones retiradas de un trozo de codigo
sin syscall. Pero los contadores los PROGRAMA un MSR (`wrmsr`, Ring 0), y lo
que leen dice cosas de OTRAS tareas si no se reinician al cambiar: una fuga
lateral. La puerta va con el kernel guardando y limpiando los contadores en
cada cambio, o solo para la tarea que los pidio.

**PP3 -- leer un MSR por capability.** `rdmsr` no tiene puerta en el
silicio. Lo que si se puede es una `op` nueva: *lee el MSR X*, con X en una
lista blanca del kernel (la temperatura, la energia RAPL, `IA32_APERF` /
`MPERF` para los MHz de verdad). Es lo que `pulso.inti` ya hace por `OP_INFO`,
con nombre de registro en vez de informe. Nunca `wrmsr`.

---

# 3. El laboratorio: TALK-CPU y su Puente al Metal

TALK-CPU tiene lo que BMO-X no puede arriesgar: un camino PROBADO a Ring 0 en
el mismo Ryzen 5 5600X, sin el kernel. Su puente (`Puente_Metal`) carga un
paquete FAGRAW64 (`METAL/INICIO.X86`: firma, longitud, entrada, suma), cierra
BootServices y lo corre con CPL 0 (QEMU/OVMF lo comprobo; el 18-10 dio su
saludo en la MSI A320M-A PRO MAX).

Ahi cada puerta se prueba ANTES de llegar al kernel, en QEMU primero:

```text
   la pregunta                         en el puente (CPL 0)
   ------------------------------------------------------------------------
   PP1  un TSS con bitmap: in/out de   poner un TSS propio, bajar a CPL 3 con
        un puerto dado y uno negado    un iretq, probar los dos y volver por
                                       la #GP (la IDT tambien es del puente)
   PP2  rdpmc con PCE y sin PCE        lo mismo, con CR4.PCE puesto o no
   PP3  que MSR existen y que dicen    rdmsr directo en CPL 0: la lista blanca
        en ESTA placa                  sale de leerlos, no de un manual
```

`toolchain/tools/inti-hex` (18-10) es el puente entre las dos casas: escribe
lo que emite INTI con la sintaxis de TALK-CPU (`; comentario`, `=offset`
comprobado) y hace el paquete FAGRAW64 con su longitud y su suma CALCULADAS.
Su prueba lee el `hola` de TALK-CPU y da los mismos 24 bytes de cabecera que
alli se escribieron a mano (`0x28`, entrada 0, suma `0x0C26`).

[!] Lo que el puente NO tiene todavia, segun su propio LEEME: GDT, IDT ni
paginacion propias; corre con IF=0 y la memoria del firmware. PP1 y PP2 piden
una IDT para atrapar el `#GP` y un camino a CPL 3: son trabajo de TALK-CPU,
no de BMO-X.

---

# 4. Las decisiones del propietario

```text
   PD1  QUE PUERTAS: PP1, PP2, PP3, o ninguna. Recomendado: PP3 primero (no
        cambia el cambio de tarea ni el TSS: es una op mas, como OP_INFO), PP2
        despues (el metro la quiere), PP1 la ultima (es la que mas toca)
   PD2  PP1, QUE PUERTOS NUNCA: la lista negra (seccion 2), escrita en una
        tabla que el juez de carga lee
   PD3  PP2, LOS CONTADORES DE OTROS: limpiarlos en cada cambio de tarea, o
        PCE solo mientras corre la tarea que los pidio
   PD4  DONDE SE PRUEBA PRIMERO: el puente de TALK-CPU en QEMU, despues el
        kernel en QEMU, despues el Ryzen
```

# 5. Las casillas

- [ ] **PP0 -- las decisiones de la seccion 4**, contestadas por el propietario.
- [ ] **PP3 -- un MSR de una lista blanca, por `invoca`.** Una `op` de Ring 0
  que lee UN MSR si esta en la lista, y NO con su motivo si no; la lista sale
  de leer los MSR de la placa en el puente de TALK-CPU.
  - **Bloquea:** PD1.
  - **Como se sabe:** una sonda de INTI que lee la energia y la temperatura
    por la op, y la op dice NO a `IA32_EFER` (`0xC0000080`) y a cualquier
    escritura.
- [ ] **PP2 -- `rdpmc` desde Ring 3 (CR4.PCE).**
  - **Bloquea:** PD1, PD3.
  - **Como se sabe:** el metro cuenta las instrucciones de un trozo de INTI
    con `rdpmc` y da lo mismo que su emulador (`toolchain/tools/metro`); y una
    tarea sin el permiso sigue dando `#GP`.
- [ ] **PP1 -- el mapa de E/S por tarea.**
  - **Bloquea:** PD1, PD2.
  - **Como se sabe:** en QEMU, una tarea con `0x60` lee el teclado PS/2 sin
    syscall; la misma tarea con `0x64` fuera de su mapa da `#GP`; una tarea
    sin permiso, `#GP` en los dos; y al morir, su mapa no queda en el TSS.

# 6. Lo que este plan NO es

- **No es dar Ring 0 a una app.** Ninguna puerta deja escribir un MSR, un
  registro de control o una tabla del sistema.
- **No cambia el kernel.** Lo escribe; el kernel es del propietario.
- **No hace de TALK-CPU parte de BMO-X.** Es otra casa, con su propio camino
  al metal; aqui se usa como laboratorio, y lo que se mida alli se cita.
