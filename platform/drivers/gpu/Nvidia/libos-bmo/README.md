# bmo-nvidia-libos

Lectores puros de **copias** LIBOS para BMO-X. Extraidos del commit
`f153f3db64763a9cd8ba8c95e0fa14fcb0bc57aa`: conservan el anillo y el
diccionario existentes, ahora reutilizables sin depender del driver GA10x.
`no_std`, sin asignador, sin dependencias y con `forbid(unsafe_code)`.
El guardian `la_3060.py --check` incluye esta frontera en S9.

| Pieza | Responsabilidad |
| --- | --- |
| `anillo` | PUT, wrap, registros y parada si falta diccionario. |
| `diccionario` | ELF64 LE, VA a offset, metadata y cadenas acotadas. |
| `Fuente` | Obtener bytes por offset; la implementacion para slices no toca dispositivos. |
| GA10x / kernel | Capturar, sincronizar y acreditar las copias y su procedencia. |
| `gsp-control-bmo` | Interpretar observaciones dentro del contrato de arranque. |

Los consumidores existentes conservan `bmo_gpu_ga10x::inspeccion::libos`
y `bmo_gpu_ga10x::elf::Fuente`. Un consumidor nuevo puede depender solo de
`bmo-nvidia-libos`. Ninguna ruta nueva recibe punteros del dispositivo.

## Versiones: lo comprobado

La referencia publica estudiada es NVIDIA OpenRM **570.144**, commit
`8ec351aeb96a93a4bb69ccc12a542bf8a8df2b6f`, directorio
[`libos-v3.1.0`](https://github.com/NVIDIA/open-gpu-kernel-modules/tree/570.144/src/common/uproc/os/libos-v3.1.0).
Expone auxiliares de ELF, DWARF y logs; ese directorio no contiene todo el
sistema operativo LIBOS. La estructura
[`libos_v3_crashcat.h`](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/uproc/os/common/include/libos_v3_crashcat.h)
usa la firma `LIBOS3.1`, con mayor, menor y changelist. Esto **no demuestra**
una version 3.1.10 ni identifica por si solo el firmware de la RTX del usuario.
La version `0.1.0` de este crate es propia de BMO-X.

RISC-V es una ISA con bases RV32/RV64 y extensiones; su apertura no implica
que todas las implementaciones o su firmware sean abiertos. El decoder de
referencia comenta el ABI RISC-V LP64/LP64F/LP64D. Eso no basta para afirmar
el conjunto exacto de extensiones ni los privilegios del GSP de una GA106.
Los bytes del ELF y el build correspondiente siguen siendo necesarios.

## Verificacion y siguiente paso

```text
cargo test -p bmo-nvidia-libos
cargo test -p bmo-gpu-ga10x --test libos_compat
python3 toolchain/tools/la-3060/la_3060.py --check
```

El banco usa bytes sinteticos: vacio, wrap, PUT extremo, errores que no
avanzan, metadata extendida, ELF hostil, BSS, solapamientos y fuentes acotadas.
La compatibilidad se comprueba con las rutas publicas antiguas.

Validacion de esta extraccion (17-10-2026): biblioteca `no_std` compilada
directamente con rustc 1.75; nueve pruebas de este crate y cuatro del lector
`ga10x/src/arranque/elf.rs` pasaron con ese compilador. Tambien pasaron los
guardianes de la 3060 (incluidas 18 pruebas negativas), capas y ASCII.
La compilacion completa de GA10x con 1.75 se detiene en APIs preexistentes
(`Option::is_none_or` y `f32::abs` en `no_std`). Falta ejecutar el banco Cargo
y `libos_compat` con el nightly del proyecto. No se ha comprobado identidad
binaria de los artefactos BMO-X antes/despues (L6d), ni arranque en hardware.
Por estos pendientes, el cambio se entrega como borrador.

La siguiente ampliacion debe partir de una captura identificada: firmware,
build, diccionario exacto y momento de lectura. Un PUT cero es evidencia de
esa copia, no una explicacion automatica de `0x15`. CrashCat requiere un
lector y pruebas propios antes de anunciar soporte. Este crate no cambia
HS, BROM, firmas, firmware ni la secuencia de arranque.
