# Inspeccion NVIDIA -- ordenes, campos y motivos, sin ejecutar

Pregunta del propietario (16-10-2026): aplicar lo aprendido de Mesa, con
NVIDIA aislado, carriles y motivos propios. Esta ampliacion interpreta
copias preparadas. No agrega un kernel paralelo ni cambia firmware o resets.

## Carriles y por que existen

| Pieza | Carril | Motivo de su separacion |
| --- | --- | --- |
| `bmo-lente-gpu` | VERDE, comun | Procedencia, limites y bytes no dependen de una ISA. |
| `src/sass/lente.rs` | VERDE, NVIDIA SM86 | Campos de instrucciones; no confundir cabecera SPH con codigo. |
| `src/inspeccion/empuje.rs` | VERDE, NVIDIA FIFO_DMA | Conocer que palabra es cabecera y cual es dato requiere recorrer desde un limite de paquete conocido. |
| `src/inspeccion/metodos.rs` | VERDE, NVIDIA AMPERE_B | Un numero de metodo necesita contexto de clase y subcanal antes de darle nombre. |
| `src/inspeccion/mod.rs` | VERDE, consulta | Plantilla reproducible y explicacion; no recibe autoridad de aparato. |
| `gspcubo/sm86_ordenes.rs` | VERDE, adaptador NVIDIA | Presentar una pagina no debe inicializar GR ni enviar el ejemplo. |
| Motores y puerta del kernel | Carriles existentes | Conservan autoridad, propiedad y sincronizacion; el visor no los reemplaza. |

`ESTADO`, `MEMORIA`, `SINCRONIA` y `DIBUJO` describen el motivo del metodo
que se esta estudiando; no son permisos ni nuevos colores de riesgo. El
inspector es VERDE aunque describa una orden que al ejecutarse escribe VRAM.
Lo desconocido conserva numero y valor, con `SIN INTERPRETAR`.

## Consulta nativa

Con DIRECTOR nuevo:

```text
gpu verrano ordenes
gpu verrano ordenes 32
gpu verrano bytes bmox-vs 128
```

`ordenes` muestra una ventana de 32 bytes, en hex y bits, seguida de los
metodos cuyos datos estan en esa ventana. Cada uno conserva el offset de
su cabecera, modo, subcanal, metodo, valor, categoria y motivo. Se ofrece
el offset decimal siguiente. El desplazamiento debe ser multiplo de cuatro.
Aunque se muestre una pagina intermedia, el contexto se reconstruye desde
el comienzo de la copia; no se trata su primera palabra como una cabecera.

**Es una PLANTILLA:** usa el constructor real de VERRANO V0, un triangulo,
1280x720 y sin escalera. Contiene direcciones planeadas del ejemplo; no
es una captura del ultimo dibujo, de MUNDO o del canal activo. No llama al
GSP ni toca registros. Se puede estudiar aunque G0 impida arrancar la RTX.

La lente SASS ahora conserva tambien `enc` (los 12 bits de codificacion),
`base` de nueve bits, `forma`, guarda, bit `yield` y mascara `reuse` de
cuatro bits. Estos dos ultimos son campos crudos SM86; no prueban cambios
de cache, concurrencia ni rendimiento. No se modifica el planificador.

## Uso en el anfitrion

```powershell
cargo run -q -p bmo-gpu-ga10x --example ordenes
cargo run -q -p bmo-gpu-ga10x --example ordenes -- COPIA_FIFO.bin
```

Sin fichero, la misma plantilla que el comando nativo. Con fichero, una
copia de un segmento contiguo FIFO en little-endian, de hasta 1 MiB. Ese
techo es del visor anfitrion, no de la GPU. No pasar SPIR-V, SASS, un BSF,
una receta VRN1/VRN2 ni una entrada GPFIFO como si fueran este flujo.

Se conserva primero la vista cruda. Un segmento estructuralmente legible
termina con `TOTAL`, numero de paquetes y metodos. Eso no es aprobacion de
metodos ni evidencia de envio o ejecucion. Los nombres solo se aplican
despues de una declaracion `SET_OBJECT` compatible en la copia; una clase
distinta invalida el contexto previo. No se observa el runlist real.

## Formatos admitidos y rechazo preciso

El lector admite NOP, incremento, no incremento, dato inmediato e incremento
una vez. Para los modos con datos separados, COUNT=0 no consume datos.
Un inmediato de valor cero si contiene un metodo. Los datos pueden tener
los mismos bits que una cabecera: no se vuelven a interpretar como tal.

| Caso | Respuesta |
| --- | --- |
| Cola que no completa cuatro bytes | Se conserva el volcado; error de palabra parcial. |
| COUNT mayor que los datos disponibles | Error con offset, cabecera cruda y cantidades. |
| Control no implementado, incluido fin de segmento | Detener lectura; no seguir saltos ni resincronizar inventando un limite. |
| Bit reservado con un metodo presente | No interpretar el paquete. |
| Incremento fuera del campo de metodo | Detener lectura; no envolver a otra direccion silenciosamente. |
| Metodo desconocido o clase distinta | Mantener sus numeros; no tomar prestado un nombre 3D. |

El formato puede dividir datos entre segmentos en hardware. Este lector
requiere una copia contigua completa: no reconstruye segmentos faltantes.
Sus rechazos conservadores no significan que toda variante rechazada sea
ilegal para el silicio. Tampoco valida todos los valores de cada metodo.

## Fuentes y contraste con Mesa

El formato se contrasto con
[NV_FIFO_DMA de NVIDIA](https://raw.githubusercontent.com/NVIDIA/open-gpu-doc/master/manuals/turing/tu104/dev_ram.ref.txt)
y la [guia de ordenes de NVK](https://docs.mesa3d.org/drivers/nvk/hardware_docs.html),
consultadas el 16-10-2026. Son referencias mutables. La direccion de metodo
se convierte de palabras a bytes; los modos de incremento se mantienen
distintos. Los nombres admitidos usan las constantes locales procedentes
de la clase AMPERE_B, no una tabla universal de motores.

Los campos comunes SASS se contrastan con el
[informe NAK conservado](../../../../docs/metal/mesa_nak/INFORME_NAK.md),
que identifica el commit Mesa `9d007d018715131a85e11bee9a91b19b744a4374`.
Esta sesion no recompilo Mesa ni ejecuto su banco en GPU. Tampoco genero
un desensamblado independiente nuevo de NVIDIA.

La [documentacion de pruebas de NAK](https://docs.mesa3d.org/drivers/nvk/external_hardware_docs.html)
separa comprobar codificacion con nvdisasm de comprobar comportamiento en
hardware. Aqui se mantiene esa distincion: las pruebas del lector y del
emisor no acreditan que el dibujo haya terminado correctamente en la RTX.

## Guardianes y comprobacion

S7 de `la-3060` recorre `inspeccion/`, incluidas subcarpetas y pruebas.
Exige las piezas del lector, carril VERDE y consumo NADA; rechaza autoridad
de registros, reloj, memoria volatil, ensamblador, arranque y RM. S6 cubre
las dos consultas preparadas, que conservan procedencia CPU y espacio de
recurso. S2 mantiene VERRANO sin fabricante. No se aflojan lineas base.

Es un guardian de patrones; no una prueba universal frente a macros o
llamadas indirectas. Lo complementan `forbid(unsafe_code)` del crate, las
capas, las pruebas negativas y la revision de llamadas.

El banco cubre modos con vectores literales, NOP/COUNT=0, inmediato cero y
maximo, datos que parecen cabeceras, lectura bytes/palabras, truncamiento,
limites, errores sin resincronizacion, clases y huecos sin nombre. La
plantilla actual recorre 680 bytes, 53 paquetes y 117 metodos. El CLI permite
reconstruir sus bytes desde hex y bits, y rechaza cuatro entradas malformadas
sin emitir un `TOTAL` de exito.

Validacion: 602 pruebas Rust aprobadas (incluidos todos los prefijos de
la plantilla, para detectar cortes), once pruebas del guardian y comprobacion
del kernel para su target. DIRECTOR release enlazado; pila 45.984/65.536 B,
marco mayor 19.552 B. Pasan capas, contrato, censo modular, ASCII, enlaces
y planes. La primera observacion en el equipo llego a las 23:23:
64 bytes de shader y 32 bytes de plantilla, con columnas hex/bits
contrastadas. [Parte y limites](../../../../docs/metal/METAL_2026-10-16.md).

Pendiente: observar otras paginas y rechazos en BMO-X, capturar el envio real con
identidad de recurso/trabajo/build y recuperar resultados tras finalizacion.
No se declara arreglado G0/0x15 ni la diferencia de cuatro pixeles de Q0a3.
