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
> del propietario, para siempre). Cada casilla se abre cuando el propietario
> la decide (seccion 4). PP3 autorizado e integrado el 18-10; falta metal.

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
  - **Estado 18-10:** autorizado por el propietario; ABI, kernel, emulador,
    INTI y sonda integrados y probados en anfitrion. Falta la sonda en metal.
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

# 5b. PP3, relevo aplicado -- falta la prueba en metal (18-10)

**Estado actual:** el propietario autorizo aplicar las ocho piezas de este
relevo, incluido Ring 0 y CODEOWNERS. Ya estan integradas: `0x49` lee solo
la lista blanca tras comprobar la CPU del perfil; el emulador devuelve
`0x454D_5500_0000_0000 | msr`, explicitamente MODELADO. No hay escritura.
El kernel release compila; pasan los cuatro bancos indicados abajo, incluido
el espejo de INTI y las tres pruebas nuevas de `tests/msr.rs` (lectura,
rechazos con motivo y sonda completa). La sonda `sondas/msr.inti` lista el
microcodigo, MPERF/APERF, los limites/estado y los ocho P-states; comprueba
el codigo antes de pedir el valor. **PP3 sigue abierta hasta probarla en el
Ryzen; QEMU con otra CPU debe rechazar los MSR permitidos.**

Lo que sigue conserva el contexto del relevo original y el mapa de las
ocho piezas ya aplicadas; no es una lista de cambios todavia pendientes.

> El propietario: *"mejor poner eso en datos para que la otra IA haga ese
> esfuerzo"*. Lo de abajo es lo que falta para cerrar PP3, escrito para que
> quien lo tome no tenga que volver a estudiarlo. Se aparco originalmente
> aqui; el relevo ya se aplico con la autorizacion posterior del propietario.

**Hecho (commits `25a0921`, `bfc42a5`):** `platform/shared/bmo-msr-lista`, la
decision sin dependencias: `BLANCA` (PATCH_LEVEL `0x8B`, MPERF `0xE7`, APERF
`0xE8`, PStateCurLim `0xC001_0061`, PStateStat `0xC001_0063`, PStateDef0..7
`0xC001_0064`..`006B`), `NEGRA` con su motivo, `CONTADORES`
(`0xC001_0200`..`020B`), `No { FueraDeLaLista = 1, OtraCpu = 2 }` y
`decidir(msr: u64, cpu_del_perfil: bool) -> Result<&Fila, No>`. Banco: 3 ok.

**Por que se paro entonces:** el clasificador de permisos de aquella sesion nego dos veces
tocar Ring 0 y `.github/CODEOWNERS` ("Modify Shared Resources", "Security
Weaken"). Quien lo tome necesita esas rutas permitidas, o que el propietario
aplique a mano.

**Lo que se sabia antes de empezar** (no repetir el estudio):
- el kernel YA lee MSR solo en la CPU del perfil:
  `ring0/cpu_vendor/profile.rs::extensiones().0` (patron de
  `cpu_vendor/ryzen_5_5600x/contadores.rs::se_pueden`);
- MPERF/APERF y la energia acumulada YA salen por `TASK_OP_INFO`
  (`INFO_CPU_MPERF` `0x6B`, `INFO_CPU_APERF` `0x6C`, `INFO_CPU_UJ_*`);
- la ultima op es `TASK_OP_CONTAR = 0x48`: la nueva es `0x49`;
- el NO del despachador es `BmoStatus::negado(motivo, valor)`, codigo
  `ERROR_NEGADO = 11` (`syscall/ops.rs`), con el motivo en las banderas;
- la base del kernel compila con
  `cargo +nightly build --release --target x86_64-unknown-none` desde
  `Ultra_kernel_x86-64/kernel` (~1 min).

**Las piezas aplicadas, en orden:**

1. `platform/abi/bmo-abi/src/syscalls/surface/tarea.rs`, detras de
   `CONTAR_PARA`: `TASK_OP_LEE_MSR: u64 = 0x49`, `LEE_MSR_FUERA: u32 = 1`,
   `LEE_MSR_OTRA_CPU: u32 = 2` y `ERROR_NEGADO: u32 = 11` (el ABI no lo tenia),
   con su comentario: lee de ESTE nucleo, nunca escribe.
2. `Ultra_kernel_x86-64/kernel/src/ring0/syscall/ops.rs`, detras de
   `CONTAR_PARA`: `pub(crate) const TASK_OP_LEE_MSR: u64 = 0x49;` (espejo).
3. `.../ring0/syscall/mod.rs`: en el despachador, detras de
   `TASK_OP_CONTAR => contar(arg0, arg1),`, el brazo
   `TASK_OP_LEE_MSR => lee_msr(arg0),`; y junto a `fn contar`:
   ```text
   fn lee_msr(msr: u64) -> BmoStatus {
       let del_perfil = crate::ring0::cpu_vendor::profile::extensiones().0;
       match bmo_msr_lista::decidir(msr, del_perfil) {
           Ok(fila) => BmoStatus::ok_value(crate::ring0::cpu_vendor::ryzen_5_5600x::msr::leer(fila)),
           Err(no) => BmoStatus::negado(no as u32, 0),
       }
   }
   ```
4. `.../ring0/cpu_vendor/ryzen_5_5600x/msr.rs` nuevo, y `pub mod msr;` en su
   `mod.rs` detras de `pub mod presupuesto;`: `pub fn leer(fila:
   &bmo_msr_lista::Fila) -> u64` con un `rdmsr` de `fila.msr`
   (`options(nomem, nostack)`). Recibe una FILA, nunca un numero: lo de fuera
   de la lista no tiene por donde entrar.
5. `Ultra_kernel_x86-64/kernel/Cargo.toml`, detras de `bmo-contadores`:
   `bmo-msr-lista = { path = "../../platform/shared/bmo-msr-lista" }`.
6. `.github/CODEOWNERS`, detras de `bmo-contadores`:
   `/platform/shared/bmo-msr-lista/ @AndreeSalazar` (el guardian
   `codeowners` falla sin ella en cuanto el kernel lo enlaza).
7. El emulador, `toolchain/forge/bmo-lower/src/emu/sistema.rs`, junto al brazo
   de `TASK_OP_CONTAR`: el mismo `decidir` con la CPU del perfil MODELADA, y un
   valor que se vea modelado (por ejemplo `0x454D_5500_0000_0000 | msr`); el
   NO con `fallar_syscall(ERROR_NEGADO | (motivo << 32))`. Y la dependencia en
   `toolchain/forge/bmo-lower/Cargo.toml`.
8. INTI: en `toolchain/forge/sem-asm/tables/lang/inti/modulos.toml`,
   `op_lee_msr = "0x49"` (y los dos motivos), con sus filas en
   `toolchain/lang/inti/tests/espejo_del_kernel.rs` (la prueba lee el fuente
   del kernel); una prueba como `emisor-x86_64/tests/contar.rs` que pide
   PATCH_LEVEL (valor modelado) y EFER (`0xC000_0080`: NO, motivo 1); y la
   sonda `sondas/msr.inti`, que lista la blanca en el Ryzen.

**Como se sabe:** el kernel compila; `cargo test -p bmo-msr-lista`,
`-p bmo-lower`, `-p bmo-inti-x86-64`, `-p bmo-inti-front` (el espejo); los
guardianes `codeowners`, `ascii-sweep --check`, `planes --check`; y en el
Ryzen, la sonda: el microcodigo y los ocho P-states. En QEMU sin `-cpu host`
solo se vera el NO de otra CPU (motivo 2), y eso tambien es la prueba.

# 6. Lo que este plan NO es

- **No es dar Ring 0 a una app.** Ninguna puerta deja escribir un MSR, un
  registro de control o una tabla del sistema.
- **El kernel es del propietario.** La integracion PP3 del 18-10 fue
  autorizada por el; PP1 y PP2 conservan sus decisiones pendientes.
- **No hace de TALK-CPU parte de BMO-X.** Es otra casa, con su propio camino
  al metal; aqui se usa como laboratorio, y lo que se mida alli se cita.
