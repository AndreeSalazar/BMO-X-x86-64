# Laboratorio NVIDIA: recetas y camino a TITAN++

17-10-2026. Primera base operativa para copias del anfitrion. El inventario,
las sesiones y las regiones RISC-V tienen salidas JSON versionadas. El
lector ELF/LIBOS sigue siendo el existente; no se reescribe en Python.
No hay todavia una receta completa para sustituir el firmware de GA106.

Antes de otro ensayo: [revision de los partes existentes](REVISION_EVIDENCIA.md),
incluida la unidad de observacion y las comparaciones que faltan.

## 1. Elementos y propietarios

| Elemento | Herramienta / propietario | Resultado y limite |
| --- | --- | --- |
| VBIOS, booter, GSP, diccionario, logs, informe | `inventario.py` | Bytes y SHA-256; el papel es declarado |
| Sesion buena o fallida | `sesion.py` | Identidad, contexto, diferencias y ausencias; no causa |
| ELF y anillos LIBOS | ejemplo Rust `libos` de GA10x | Formatos ya comprobados; exige diccionario de la misma build |
| Region RISC-V identificada | `riscv.py` + Capstone | Bytes, instrucciones, version del decoder y cola sin interpretar |
| Falcon / HS / BROM | estudio separado | No se pasa al decoder RISC-V; variante y acceso pendientes |
| Shader SM86 / FIFO | inspectores GA10x existentes | Otro lenguaje y otro procesador, no firmware GSP |
| Captura MMIO/DMA | kernel y puertas existentes | Un lector anfitrion no adquiere esa responsabilidad |
| Vista nativa TITAN++ | siguiente etapa | Contratos tipados sobre copias; adaptador aun no implementado |

La extraccion `bmo-nvidia-libos` esta propuesta en
[PR #64](https://github.com/AndreeSalazar/BMO-X-x86-64/pull/64); hasta integrarla
se conserva la ruta publica GA10x. La herramienta de sesiones es independiente.

## 2. Registrar un intento sin perder los fallos

Conservar la carpeta de cada captura completa que entregue `save`, incluidos
informe y `gpu/gsplog.bin`. No renombrar un firmware de otra release para
hacerlo parecer el del intento. Estos comandos usan rutas de ejemplo.

```powershell
python toolchain/tools/nvidia-hs/sesion.py registrar --case G0-A --outcome bad --boot cold --point after-booter --variable baseline --build BUILD_DEL_INFORME --attempt 1 --pci-id 10de:2504 --firmware-release 570.144 --artefacto "gsp=C:/copias/A/gsp_ga10x.bin" --artefacto "booter=C:/copias/A/boot_ld.bin" --artefacto "logs=C:/copias/A/gsplog.bin" --artefacto "report=C:/copias/A/informe.txt" > sesion-A.json
python toolchain/tools/nvidia-hs/sesion.py comparar sesion-A.json sesion-B.json > diferencias.json
```

Cada etiqueta de contexto es declarada: `good`, `bad`, `unknown`; `cold`,
`warm`, `unknown`. `point` nombra el momento de LA COPIA, no una fase que se
supone alcanzada. `variable` dice que factor se pretendio variar; no garantiza
que fuera el unico. `unknown` conserva ausencias. Build/intento no se extraen
automaticamente del texto: no hay todavia un parser estable del informe.

El comparador separa cambios del contexto, identidad y huellas; enumera los
datos faltantes. No calcula una probabilidad ni culpa al ultimo shader.
Todas las entradas son archivos locales; la salida es stdout. Usar una salida
distinta de las entradas y comprobar exit code: 0 terminado, 2 error.
Una sesion valida o una region legible no significan arranque correcto.

## 3. Interpretar LIBOS con su lector

```powershell
cargo run -p bmo-gpu-ga10x --example libos -- C:/copias/A/gsp_ga10x.bin C:/copias/A/gsplog.bin C:/copias/A/task.elf
```

Omitir `task.elf` si no hay un diccionario exacto: se conserva el inventario y
cabeceras crudas. Un PUT cero describe esa copia, no identifica la causa de
`0x15`. Referencia estudiada: OpenRM 570.144, directorio `libos-v3.1.0`;
eso no acredita la version exacta que ejecuto el hardware.

## 4. Desensamblar una region identificada

Capstone es opcional y SOLO del anfitrion. Entorno reproducible probado:

```powershell
python -m venv .venv-gpu
.venv-gpu/Scripts/python -m pip install -r toolchain/tools/nvidia-hs/requirements-host.txt
.venv-gpu/Scripts/python toolchain/tools/nvidia-hs/riscv.py C:/copias/code.bin --isa rv64 --offset 0 --length 64 --address 0x1000 > region.json
.venv-gpu/Scripts/python -m unittest discover -s toolchain/tools/nvidia-hs -p "test_*.py" -v
```

En Unix el interprete es `.venv-gpu/bin/python`. El offset, longitud y VA del
ejemplo NO son direcciones descubiertas del GSP. Deben salir del mapeo de la
imagen estudiada. La ventana se limita a 64 KiB y al archivo. Elegir RV32/RV64
explicitamente; `--compressed` habilita C y alineacion de dos bytes. No se
conoce por ello el conjunto exacto de extensiones de la RTX.

La huella es de LA REGION, no de todo el archivo; asociarla al inventario
original. Sin skip-data: ante bytes desconocidos o truncados conserva la cola
cruda. Exit 0 tambien puede incluir `unrecognized_or_truncated`: mirar `stop`
y `consumed`. Un listado plausible no prueba que esos bytes sean codigo.

Inspiracion verificable: [Capstone Python](https://www.capstone-engine.org/lang_python.html)
y [Capstone RISC-V](https://github.com/capstone-engine/capstone).
Para un proyecto navegable, referencias y decompilacion, estudiar
[PyGhidra](https://github.com/NationalSecurityAgency/ghidra/blob/master/Ghidra/Features/PyGhidra/README.md).
No se agrega Ghidra ni sus dependencias al kernel. Falcon requiere su propio
backend y validar la cobertura de la variante; SASS requiere herramientas SM86.

## 5. Como evolucionar TITAN++ sin inventarle capacidades

Revisado: `toolchain/lang/titan/src/gpu.rs`, `manifest.rs`, `OBJETOS.md` y
`platform/shared/titan-lector`. Hoy `gpu fn` describe trabajos para shaders;
no significa acceso a firmware GSP. El lector de paquetes TITAN tampoco es
un lector de ELF NVIDIA. Sus contratos existentes se conservan.

Esquema propuesto, NO API implementada: biblioteca `gpu_inspeccion` con valores
`Copia`, `Region`, `Instruccion`, `RegistroLibos` y `Problema`. Empezar por
datos tipados y renderizado, luego lectura de una copia autorizada en Ring 3.
Un SHA-256, una VA o PUT sin representacion numerica exacta se conservan como
bytes/hex; no se convierten a f32 ni a un entero con perdida. Los campos
desconocidos siguen crudos y cada resultado lleva origen y version de lector.

La primera integracion debe decidir como enlazar el crate Rust puro desde la
biblioteca nativa de TITAN. No hay FFI Rust/TITAN ni lector JSON universal
demostrado aqui. Primero un adaptador acotado, pruebas de limites y rechazo;
despues igualar el resultado con el banco anfitrion. Sin palabras nuevas del
lenguaje hasta comprobar que composicion y traits no bastan.

## 6. Orden de trabajo y criterio para avanzar

| Etapa | Entrega | Evidencia necesaria |
| --- | --- | --- |
| Base anfitrion | Inventario, sesiones, region RISC-V | Vectores sinteticos y errores comprobados |
| Capturas reales | Un bueno y un malo con identidad | Firmware, informe, logs, build/intento y tipo de arranque |
| Mapa de imagen | Procesador, formato, VA, offset, modo | Referencia exacta; no desensamblar contenedores a ciegas |
| LIBOS ampliado | Diccionario exacto; futuro CrashCat | Fixture, fuente del formato y resultado reproducible |
| Diagnostico G0 | Primera divergencia repetible | Auditar entrega/DMA/coherencia sin atribuir `0x15` por intuicion |
| TITAN++ nativo | Vista y consulta sobre copias | Adaptador probado, contrato y paridad con el lector Rust |
| Control propio | Evaluar servicios y cadena de carga | Acceso compatible demostrado y plan de recuperacion concreto |
| Juegos AAA | Carga minima fallida y vuelta al juego | Finalizacion, recursos, canal y shader identificados |

La velocidad de BMO-X puede abaratar cada ensayo; no sustituye estas pruebas.
Cada receta nueva conserva entradas, herramienta/version, resultado esperado,
resultado observado y lo que la haria quedar refutada.
