# BMO ABI -- Especificacion v2.0

> **Estado**: vivo, en evolucion.
> **Ultima revision**: v2.0 (post-Opus).
> **Mantenedor**: proyecto BMO.

---

## 0. Que es BMO ABI?

**BMO ABI** es la **interfaz binaria y de programacion** que define como un
programa (compilado a BEF) interactua con BMO.

Reemplaza:

- el **C ABI** (cdecl/stdcall/Win64/SysV AMD64) y
- la **C standard library** (`<stdint.h>`, `<stddef.h>`, `<string.h>`,
  `<errno.h>`, `<time.h>`, `<stdio.h>`, etc).

**No** es un reemplazo de los lenguajes: es un **contrato** que **todos los
lenguajes pueden usar** para hablar con BMO.

Frontends como C y COBOL deben vivir fuera del kernel (por ejemplo en
`crates_Personal/Lenguajes/`) y generar BEF offline. El kernel solo carga BEF,
resuelve imports y ofrece syscalls BMO; no compila lenguajes en Ring 0.

---

## 1. Principios de esquema

| # | Principio | Significado |
|---|-----------|-------------|
| 1 | **Modular** | Cada sub-modulo es autocontenido. Una app puede importar solo lo que necesita. |
| 2 | **Sin VM obligatoria** | AOT es preferido, pero se permite runtime modular por lenguaje. |
| 3 | **BEF es el formato canonico** | Todo programa compilado a BEF puede cargarse. |
| 4 | **ABI explicito, no implicito** | Tamanos, alineaciones, layouts documentados + `static_assert!` en codigo. |
| 5 | **Manejo de errores unificado** | `BmoStatus` de 16 bytes en RAX:RDX. Sin TLS, sin errno. |
| 6 | **Zero-copy donde sea posible** | IPC, surfaces, strings: pasar `(ptr, len)`, no copiar. |
| 7 | **Handles opacos** | `BmoHandle(0xABCD)` con tag + generation + index. |
| 8 | **Determinismo** | `BmoInstant` es monotonico RDTSC-backed, no afectado por NTP. |

---

## 1.1 La maquina: una, y MEDIDA

BMO-X es x86-64 y nada mas (`toolchain/tools/isa`). La maquina es la del
banco, y su perfil no se elige al compilar: se MIDE al arrancar
(`Ultra_kernel_x86-64/kernel/src/ring0/cpu_vendor/`) y se escribe en
`PERFIL/CPU.txt`, con dos columnas por fila -- lo esperado (hoja de AMD) y lo
visto (foto del Ryzen) -- y un guardian (`toolchain/tools/perfil-campos`).

| Propiedad | Hoy |
|---|---|
| Arquitectura | x86-64, little-endian, punteros de 64 bits, paginas de 4 KiB |
| Maquina | AMD Ryzen 5 5600X (Zen 3 Vermeer), 6 nucleos / 12 hilos, 1 CCX |
| Caches | `PERFIL/CPU.txt`, tabla de caches (medida con CPUID 0x8000001D) |
| ISA que BMO USA | la columna `Yes` de `cpu_vendor/features/usage.rs`: SSE2, RDRAND, XSAVE/OSXSAVE/XSAVEOPT, ERMS, RDTSCP, TSC invariante, MONITORX, NX, SMEP, SMAP, UMIP |

** Hasta el 2026-09-19 esta seccion decia que la ISA requerida era SSE4.2,
AVX, AVX2, FMA, BMI1/2, AES, PCLMULQDQ; que habia un "perfil alternativo AMD
EPYC Zen 3" elegible con una feature de Cargo; y que el loader validaba el
perfil con CPUID. Nada de eso lo hacia nadie, y se borro con `cpu_profiles/`.
Una extension que BMO use y el silicio no tenga la pinta `ext` como CONFLICTO.

---

## 2. Estructura del ABI

** Reescrita el 2026-09-19: aqui se describian `values/`, `runtime/`, `fs/`,
`windowing/`, un cargador con TLS y veinte tipos mas. Ninguno tenia usuario
vivo y se borraron. Lo que queda:

```
bmo_abi/
+-- fundamentals/
|   +-- primitives/     bx_u8..u64, bx_i*, bx_f*, bx_bool
|   +-- status/         BmoStatus (16 B) y el texto de cada codigo
|   +-- handle/         BmoHandle (64 bits), HandleKind
|   +-- sync/           BmoSpinLock y atomicos
+-- types/              convencion de llamada + regla de disposicion
+-- syscalls/           INVOKE (0x00), WAIT (0x02) + syscall0..syscall6
+-- bef2/               EL FORMATO: cabecera de 64 B con cuatro regiones en
|                       sitio fijo, anexos, un reloc, firma; escritor, juez
|                       (lector), objeto (.bo) y paquete (recursos)
+-- bef/                lo que viaja DENTRO de los anexos: katanas, recursos,
|                       requisitos, simbolos, blake3
+-- dynobj/             texto, lista, tabla (runtime de INTI)
```

### Tipos repr(C) y medidas verificados

| Tipo | Tamano | Area |
|------|--------|------|
| cabecera BEF2 | 64 B | bef2 |
| entrada de anexo | 16 B | bef2 |
| `Reloc` | 16 B | bef2 |
| `objeto::Enlace` | 24 B | bef2 |
| `Symbol` | 32 B | bef |
| `SectionHash` | 40 B | bef |
| `SignatureHeader` | 8 B | bef |
| `BmoStatus` | 16 B | fundamentals |

Los fija `tests/abi_layout.rs::static_assert_sizes`.

---

## 3. BMO ABI v2: dos syscalls

| Numero | Nombre | Responsabilidad |
|--------|--------|-----------------|
| 0x00 | `BMO_INVOKE` | Control sincrono sobre una capability |
| 0x01 | *reservado* | Era `CHANNEL_KICK`; hoy es `CHANNEL_OP_KICK` sobre el canal. Contesta `ERROR_UNSUPPORTED` y no se reutiliza |
| 0x02 | `BMO_WAIT` | Bloquear hasta cambio de secuencia o deadline |

Filesystem, red, audio, input, compositor y GPU son servicios accesibles por
capabilities y BMO Channel. No agregan nuevas entradas privilegiadas.

### La tabla v1 (0x100..=0x1FF) YA NO EXISTE (2026-09-19)

Eran 109 nombres (`bmo_mem_alloc` 0x190, `bmo_exit` 0x181...). El kernel no
los despachaba -- nunca hubo el "adaptador temporal" que esta seccion
prometia -- y cualquiera de ellos contestaba `rax = 10`. Como 10 no es cero,
un `malloc` que los usaba escribia en la direccion `0xA`. Se quito la tabla y
todo lo que la leia (frontends de C y COBOL, `bmo-rt`, `stdlib/heap`): un
nombre v1 es ahora un error de compilacion.


### Convencion de syscall (x86_64)

```
RAX = syscall number
RDI = arg0
RSI = arg1
RDX = arg2
R10 = arg3
R8  = arg4
R9  = arg5
RAX = status code (0 = OK)
RDX = value (handle, contador, etc.)
```

Wrappers: `syscall0()` .. `syscall6()` en `syscalls/` (inline asm, `no_std`).

---

## 4. BEF2 (el formato de un programa de BMO-X)

** BEF1 --cabecera de 48 B con tabla de secciones tipadas, la idea de ELF con
otro nombre-- **murio el 2026-09-19**. Ver `docs/plan/PLAN_BEF_NATIVO.md`.

```
+--------------------------------------------------+
| cabecera (64 B, una linea de cache)              |
|    0  magic "BEF2"    4 abi=2    5 banderas      |
|    8  xcr0            16 entrada  20 anexos      |
|   24  codigo {off, bytes}          R+X           |
|   32  constantes {off, bytes}      R+NX          |
|   40  datos {off, bytes}           R+W+NX        |
|   48  ceros bytes                  R+W+NX        |
|   52  total                                      |
+--------------------------------------------------+
| tabla de anexos: {tipo, off, bytes} x N (16 B)   |
|   RELOCS FIRMA REQUISITOS  <- los abre el kernel |
|   RECURSOS MANIFIESTO KATANAS SIMBOLOS ENLACE    |
+--------------------------------------------------+
| regiones (cada una empieza en un sector)         |
| anexos                                           |
+--------------------------------------------------+
```

- **El permiso lo da el HUECO**: no hay campo para un codigo escribible.
- **Un reloc** (`donde`+`offset` <- `destino`+`addend`, 16 B), y solo en
  ejecutables; un `.bo` lleva `ENLACE` (`Rel32`, `Abs64`, `Region`) que
  resuelve `bmo-enlazar` y nunca llega al kernel.
- **`xcr0`** en vez de "extensiones": la mascara literal de XSAVE.
- **Firma obligatoria**: BLAKE3 por region y por anexo que el kernel lee;
  Ed25519 opcional encima (`bmo-firmar`).
- **Sin `arch` ni `endianness`**: el magic ya dice BMO-X x86-64.

### Escritor, juez, y la puerta del kernel

| Componente | Archivo | Funcion |
|------------|---------|---------|
| Escritor | `bef2/escritor.rs` | `Escritor` -> `Vec<u8>` BEF2; `de_imagen` reabre para `bmo-pack` y `bmo-firmar` |
| Juez | `bef2/lector.rs` | `leer()` -- dice SI o NO con `Falta`; comprueba cada hash |
| Objeto | `bef2/objeto.rs` | `read()` -- el contrato del `.bo` para `bmo-enlazar` |
| Puerta de carga | `platform/abi/bmo-bex-gate` | `revisar()` -- lo que corre en Ring 0 antes de mapear nada; atada al juez por `tests/gate_y_validador_no_se_separan.rs` |

---

## 5. Handles

`BmoHandle` es un `u64` opaco con tres campos internos:

```
bit  63      = tag        (0 = recurso, 1 = canal/cola)
bits 62..56  = kind       (7 bits)
bits 55..40  = generation (16 bits, detecta use-after-close)
bits 39..0   = index      (40 bits, slot en la tabla del proceso)
```

(Hasta el 2026-09-19 aqui ponia indice 0..47 y generacion 48..60: no era el
formato de `handle/opaque.rs` ni el del kernel, que `build/contrato.ps1`
compara entre si.)

- `0` = `BmoHandle::NULL`, `0xFFFF_FFFF_FFFF_FFFF` = `BmoHandle::INVALID`.
- El **kind** se almacena en una tabla global del kernel.
- `HandleKind` tiene **34 variantes**: Window, File, Dir, Pipe, Socket, Port,
  Timer, Thread, Process, Semaphore, SharedMem, Surface, GpuBuffer, etc.

---

## 6. Errores

`BmoStatus` (16 B, repr(C)) es el return value universal:

```
[0..3]  code:   u32  -- 0 = OK, >0 = error
[4..7]  flags:  u32  -- StatusFlags (partial, retry, truncated, etc.)
[8..15] value:  u64  -- handle, contador, offset, etc.
```

Por la puerta viaja partido: `rax = code | flags << 32`, `rdx = value`. Los
numeros de `code` los define el kernel (`syscall/ops.rs`, `ERROR_*`) y su texto
esta en `fundamentals/status/error.rs`. (`BmoError`, `error_code/` y
`convert/` eran dos copias mas de lo mismo, sin usuario; se fueron el 19-09.)

---

## 7. Convencion de llamada (BMO Call)

La fuente es `types/convention.rs`, y **los emisores la importan** (C:
`decidir/llamada.rs`; INTI: `emisor-x86_64/src/lib.rs`). Esto la resume:

```
Argumentos (6 GPR): RDI, RSI, RDX, RCX, R8, R9
Del 7o en adelante: pila, de derecha a izquierda; los quita el que llama
Agregados y flotantes (BMO C): pila, por ranuras de 8
Variadicas: TODO por la pila
Retorno: RAX (solo la puerta del kernel devuelve RAX:RDX)
Preservados: RBX, RBP, R12-R15
Pila al hacer `call`: 8 B garantizado (INTI mantiene 16 salvo impares)
Zona roja: NINGUNA
Shadow space: 0 B
```

** Hasta el 2026-09-19 esta seccion decia 7 registros (con R10), retorno en
RAX:RDX, pila a 64 y zona roja de 256. Ningun emisor lo hizo nunca.

---

## 8. Punto de entrada

Todo programa BMO ABI exporta un simbolo `_bmo_start`:

```rust
#[no_mangle]
pub extern "sysv64" fn _bmo_start(argc: u64, argv: *const *const u8) -> !;
```

El loader de BEF salta a `_bmo_start` despues de:
1. Mapear secciones en memoria
2. Aplicar relocalizaciones
3. Resolver imports
4. Preparar la pila (sin zona roja, y sin TLS: BMO-X no tiene puntero de hilo)

---

## 9. Garantias

1. **ABI v2 estable**: las dos puertas (0x00, 0x02) no cambian dentro de v2.x.
2. **Solo ABI 2.x**: el cargador rechaza un BEX 1.0 (2026-09-19). Lo unico
   que un 1.0 sabia llamar era la tabla v1, que el kernel no despacha.
3. **Handles son procesos-locales**: un handle de un proceso no es valido
   en otro (salvo via IPC explicito).
4. **Strings son UTF-8 valido obligatorio**. Una funcion que recibe un
   `BmoStr` puede asumir UTF-8 valido.
5. **Time es monotonico**: `BmoInstant::now()` usa RDTSC, no retrocede.
6. **Todos los tipos repr(C) tienen static_assert!** que verifica su medida
   en compilacion. Si el layout cambia, el build falla.

---

## 10. Glosario

- **ABI**: Application Binary Interface.
- **AOT**: Ahead-Of-Time (compilacion antes de ejecutar).
- **BEF**: BMO Executable Format.
- **BEFCore**: protocolo de mensajes app <-> BMO CORE.
- **BMO**: plataforma nativa y contrato binario de BMO.
- **BLAKE3**: hash criptografico rapido (seccion hashing de BEF).
- **Devour**: traduccion de PE/ELF a BEF nativo.
- **Handle**: referencia opaca a un objeto del kernel.
- **Ring 0**: kernel (CPU privilege level 0).
- **Ring 3**: userland (CPU privilege level 3).
- **Runtime**: codigo que un lenguaje necesita para ejecutarse.
- **Syscall**: llamada al kernel mediante instruccion `syscall`.
- **TypeRegistry**: registro fijo de 256 TypeMeta para reflexion.
