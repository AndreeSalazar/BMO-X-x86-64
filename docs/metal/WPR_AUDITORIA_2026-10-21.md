# WPR: la memoria apuntada por el buzon, antes del booter

El [metal AA25256BE](HS_ARGUMENTOS_METAL_2026-10-21_AA25256BE.md) confirmo
BOOTVEC y buzones correctos antes de STARTCPU en tres recorridos con 0x15.
La siguiente frontera es la pagina WPR de 256 bytes apuntada por MAILBOX0.

## Comprobacion aplicada

Antes de soltar el SEC2, el kernel recorre sus tablas IOMMU con el oraculo
existente ve_la_gpu para el primer y ultimo byte de la meta. Exige que
ambos lleven a la pagina NEUTRO propia, con lectura y escritura como ya
establece su prestamo. La pagina debe estar inicializada y prestada.
No cambia tablas, permisos ni el estado del aparato para efectuar la prueba.

Solo cuando la traduccion coincide, lee las 32 palabras de 64 bits de esa
pagina propia y las compara con META_ESCRITA, la copia conservada al
construirla. No dereferencia la fisica desconocida de una traduccion
incorrecta. Una diferencia de cualquier bit, incluida la palabra 31,
queda indicada en la mascara de diferencias.

Un fallo de traduccion/permisos detiene el intento antes de STARTCPU con
el motivo existente 33; contenido distinto usa el motivo existente 27.
El estado concreto de la auditoria se registra aparte como HOST, no como
MAILBOX0 ni como un nuevo retorno de NVIDIA. No se agrega ABI o selector.
La carga HS puede estar preparada; el booter no se suelta si falla esta
comprobacion. No hay nuevos resets, DMA o reintentos automaticos.

La autopsia toma una segunda foto, sin rechazar cambios posteriores:
bootCount y verified pueden variar legitimamente. Dos fotos detalladas
como maximo por arranque; su id es local y no es el intento del booter.
Una repeticion en la misma sesion puede agotar ese detalle, pero conserva
la comprobacion previa. La fase 2 es el momento de la autopsia, que puede
pedirse tambien al vencer una espera; no acredita por si sola una parada.

## Como leer el informe

`WPR foto:` conserva fase (1 previa, 2 autopsia), tiempos TSC, IOVA,
fisicas esperada/inicio/fin, permisos, estado y las palabras esperadas y
observadas. Una traduccion ausente conserva MAX y su bit de presencia
apagado; no se inventa una copia de ceros. Las fotos incluyen cierre por id.

Permisos: bit 0 inicio presente, 1 inicio escribible, 2 fin presente,
3 fin escribible. El oraculo solo devuelve traducciones legibles.
Los estados host son:

| Estado | Significado |
| --- | --- |
| 0 | Copia coincide con la guardada, con enlace coherente |
| 1 | Falta traduccion legible de algun extremo |
| 2 | La traduccion lleva a otra direccion |
| 3 | Falta el permiso de escritura previsto |
| 4 | Sin copia observada |
| 5 | Contenido distinto al guardado |
| 6 | Base propia ausente, no alineada o no representable |

Traductor anfitrion, sin aparato ni red:

```powershell
python toolchain/tools/nvidia-hs/meta.py C:/copias/CAIDA.TXT > C:/copias/wpr.json
```

El JSON nombra los campos WPR V1 segun el contrato C existente y conserva
valores hexadecimales de 64 bits. Valida lista/orden, campos duplicados,
cierre, tiempos, permisos y coherencia entre estado y diferencias.
Una foto incompleta queda marcada; un informe antiguo devuelve cero fotos.
El hash SHA-256 identifica el archivo de texto, no atestigua procedencia.

## Alcance y prueba

Esto comprueba tablas en RAM y una copia leida por CPU. No verifica el
TLB de la IOMMU, una transaccion de DMA, cache/coherencia efectiva, firma
HS ni la rama interna que produce 0x15. Tampoco es una segunda validacion
independiente del serializer: compara contra su salida guardada, cuyos
offsets ya tienen el contrato OpenRM 615. No lee memoria protegida de GPU.
Las fotos son secuenciales y agregan tiempo antes del booter.

490 pruebas Rust GA10x/supervisor aprobadas con firmware 615 y VBIOS reales
activados. Las tres nuevas cubren traducciones, limites, permisos, lector
no invocado ante enlace invalido y cambios en los 64 bits de cada palabra.
Log local: target/wpr-auditoria-tests.log. Banco Python HS: 41 pruebas,
36 aprobadas y cinco omitidas por entorno; seis nuevas del lector WPR.
Log local: target/wpr-auditoria-python.log. Kernel release aprobado en 30 s
(target/wpr-auditoria-kernel.log). El paquete se construye con todos los
guardianes antes de entregar; falta la prueba en metal. Las explicaciones
del Director para motivos 27/33 incluyen ahora esta frontera WPR.

Siguiente informe: si la comprobacion previa falla, identificar estado y
campo sin atribuirlo al firmware; si pasa y vuelve 0x15, esta frontera de
tablas/copia CPU quedara comprobada en esa corrida. Conservar ambas fotos
y los conjuntos cerrados, sin confundir historicos o builds. No se declara
cerrado G0 ni acceso HS propio por haber implementado el instrumento.
