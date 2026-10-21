# Inventario HS -- las copias antes de las hipotesis

Ampliacion: [laboratorio, sesiones, RISC-V y camino TITAN++](LABORATORIO.md).
Instrumental del 21-10: [lente externo HS del kernel](../../../docs/metal/LENTE_HS_2026-10-21.md).
`python toolchain/tools/nvidia-hs/lente.py C:/copias/CAIDA.TXT` traduce sus
fotos externas a JSON con registros hexadecimales y validez. No lee la GPU
ni convierte una lectura PRI en respuesta del firmware. Las fotos parciales
se conservan como incompletas; los informes antiguos no generan fotos falsas.

Pregunta: con QUE bytes se hizo una investigacion de la RTX 3060 12G?
Herramienta anfitrion de Python 3, biblioteca estandar, sin aparato ni red.
Emite JSON en stdout. Las rutas y los binarios no se incluyen en el JSON.

El [plan NVIDIA](../../../platform/drivers/gpu/Nvidia/HS_INVESTIGACION.md)
explica la ruta vigente: contrato HS al servicio de BMO y gobierno del GSP;
el reemplazo de firmware conserva su investigacion separada.
La herramienta no descubre BROM, no lee registros y no verifica firmas NVIDIA.
SHA-256 identifica los bytes leidos de una copia: no prueba identidad de lo
ejecutado, coherencia DMA, atomicidad de captura ni funcionamiento de la GPU.
Build, intento, PCI y release son declarados por el usuario, no medidos aqui.
Si faltan, salen como `null`; no se inventa una identidad.

## Registrar copias existentes

Referencia real del 18-10: [Windows 610.74 frente a BMO](../../../platform/drivers/gpu/ga10x/ESTUDIO_WINDOWS_61074.md).
`python toolchain/tools/nvidia-hs/consulta_windows.py` consulta modo/version
GSP mediante NVML del sistema, sin cambiar configuracion. Conserva codigo de
error y campos `null` si la consulta no responde; no transforma rechazo en
modo deshabilitado confirmado. GPU 0, identificada aparte con `nvidia-smi`.
El ejemplo Rust `contrato_hs` inspecciona el booter con el lector existente,
sin aparato. Estas herramientas no reconstruyen una traza Windows.

Desde la raiz, con archivos que ya tengas copiados en el anfitrion:

```powershell
python toolchain/tools/nvidia-hs/inventario.py inventario --artefacto "gsp=C:/copias/gsp_ga10x.bin" --artefacto "booter=C:/copias/boot_ld.bin" --artefacto "vbios=C:/copias/vbios.rom" --firmware-release 570.144 --pci-id 10de:2504 > inventario.json
```

Papeles: `vbios`, `booter`, `gsp`, `logs`, `log_dictionary`, `report`.
Se declara cada papel una sola vez; el nombre no comprueba el contenido.
`--build` debe venir del informe de ESA captura, no del checkout actual.
`--attempt` es el intento de ese informe, entero positivo. Una copia de firmware
aislada puede inventariarse sin ambos datos. No representa por eso una sesion.

Redirigir stdout crea el archivo de salida por decision del shell: elegir una
ruta distinta de las entradas para no truncarlas ANTES de ejecutar Python.
Algunos shells crean una salida vacia al fallar: revisar el codigo de salida.

## Comparar dos inventarios

```powershell
python toolchain/tools/nvidia-hs/inventario.py comparar antes.json despues.json
```

El resultado distingue artefactos agregados, quitados, con la misma huella o
con huella diferente. Los cambios de identidad declarada salen aparte.
Igualdad de huellas no es igualdad de estado de la GPU ni prueba de causa.
Salida 0: operacion terminada; 2: entrada rechazada o error de lectura.
Una diferencia valida no es un error del CLI y devuelve 0.

La lectura usa bloques de 1 MiB. Se rechaza un cambio observable de medida,
mtime o ctime durante la lectura; eso no equivale a una instantanea atomica.
El comparador limita el JSON a 1 MiB, rechaza claves repetidas, identidad
mal formada, contrato desconocido y huellas/medidas invalidas. Esos limites
pertenecen a esta herramienta, no a la RTX.

## Inspeccionar el formato sin duplicar el lector

El propietario de ELF y LIBOS ya existe en GA10x. Para interpretar secciones
y copias de logs usar su ejemplo, no otro parser Python:

```powershell
cargo run -p bmo-gpu-ga10x --example libos -- C:/copias/gsp_ga10x.bin C:/copias/gsplog.bin
```

Si hay un ELF de logging de la MISMA build, puede darse como tercer archivo.
La procedencia/version debe comprobarse aparte: el inventario no la acredita.
No convertir un PUT cero en prueba de ausencia absoluta de ejecucion.

## Verificacion anfitrion

```powershell
python -m unittest discover -s toolchain/tools/nvidia-hs -p "test_*.py" -v
```

Vectores sinteticos, SHA-256 conocido, lectura multibloque, identidad ausente,
comparacion independiente de bytes/identidad, roles repetidos, archivos vacios,
JSON ambiguo, limites y CLI. No son mediciones en metal ni se conectan al build
del kernel: esta herramienta se ejecuta expresamente sobre copias.
