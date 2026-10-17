# Lente de GPU -- bytes precisos, procedencia y ventanas

Pregunta: que bytes preparo BMO-X, que bytes pudo recuperar y como
estudiarlos sin confundir el volcado con una prueba de ejecucion.
Implementacion inicial del 16-10-2026, solicitada por el propietario.

## Uso en BMO-X

Con DIRECTOR nuevo, estas consultas no necesitan GSP ni inicializan GR:

```text
gpu verrano bytes vs
gpu verrano bytes vs 64
gpu verrano bytes vs 128
gpu verrano bytes ps 128
gpu verrano bytes bmox-vs 128
gpu verrano bytes bmox-ps 128
```

Cada consulta muestra hasta 64 bytes y el comando para continuar. El offset
es decimal, relativo al programa completo y multiplo de 16. `vs` y `ps`
leen los programas del BSF del cubo; `bmox-vs` y `bmox-ps`, los programas
traducidos por PROTON-X que usa el adaptador. No es una captura del shader
de la app activa. La SPH ocupa los primeros 128 bytes y se muestra cruda;
la interpretacion SASS empieza despues. Offset fuera del recurso o argumentos
invalidos producen un rechazo, sin dibujar otra escena por accidente.

Ejemplo de la vista cruda (los bytes no se invierten):

```text
PREPARADO CPU; RECURSO base=0000000000000000
VENTANA; offset=0 capturados=4 total=16
+0000000000000000  00 01 80 ff | 00000000 00000001 10000000 11111111
SIGUIENTE 4
```

Los campos NVIDIA van despues, separados de esos bytes. `LEIDA` significa
que el decodificador local reconoce la palabra; no significa que el juez
aprobo el programa ni que la tarjeta lo ejecuto. Una palabra desconocida
conserva todos sus bytes; una cortada no se rellena con ceros.

## Estudiar un fichero completo en el anfitrion

Desde la raiz del repositorio:

```powershell
cargo run -q -p bmo-gpu-ga10x --example lente -- platform/drivers/gpu/ga10x/sombreadores/bmox12_vs.sm86 --header 128
```

La salida se puede redirigir a un fichero nuevo para conservarla. Se lee
por bloques, sin cargar el fichero entero en RAM. `--header 128` declara
una SPH grafica; sin esa opcion se interpreta como SASS sin cabecera.
No pasar un BSF entero como si fuera codigo SASS: primero debe extraerse
su programa. La herramienta no adivina el contenedor o la cabecera.

Solo una terminacion correcta con `TOTAL leidos/medida` acredita haber
recorrido los bytes del fichero. Se detecta lectura corta o cambio de
medida; no se garantiza una captura atomica frente a cambios de contenido
del mismo largo. Para un estudio reproducible, usar una copia estable.

## Bibliotecas y aislamiento

Ampliacion NVIDIA: `gpu verrano ordenes [offset decimal]` muestra una
plantilla de ordenes con categoria y motivo; el contexto se reconstruye
desde el inicio. [Carriles, fuentes Mesa y limites](../../drivers/gpu/ga10x/INSPECCION_NVIDIA.md).
No representa el ultimo dibujo de una app ni una captura del canal real.

```text
VERRANO::lens  -> bmo-lente-gpu: copias, procedencia, hex, bits, comparacion
adaptador     -> ga10x::sass::lente: campos SM86 del decodificador del juez
kernel        -> conserva la autoridad sobre memoria, canales y registros
```

La lente comun es `no_std`, sin dependencias, monton ni `unsafe`. VERRANO
la publica como `bmo_verrano::lens`; no importa NVIDIA. El lector SM86
permanece en `platform/drivers/gpu/ga10x/src/sass/lente.rs` y no adquiere
un trait de registros. El comando nativo vive bajo el modulo `sm86`,
en `sm86_bytes.rs`. No se agregan syscalls ni un segundo kernel.

Otro fabricante necesita su lector e interpretacion propios, comprobados
con su hardware. Comparte las reglas de bytes y ventanas; no el decoder
SM86, sus direcciones o su firmware. No se agrega un backend AMD ficticio.

`Snapshot` distingue:

| Dato | Contrato |
| --- | --- |
| Procedencia | Preparado CPU, copia recuperada o registros capturados; la declara quien hizo la captura. |
| Espacio | Offset de recurso, VA CPU, VA GPU o registros; la base es un rotulo, no un puntero que la lente sigue. |
| Medida | Total declarado del recurso, offset y bytes realmente disponibles. |
| Estado | Completo solo si existen todos los bytes desde cero; una ventana final no acredita las paginas previas. |
| Inaccesible | No se presenta como una captura vacia correcta. |
| Comparacion | Exige misma ventana y largo; devuelve diferencias y primer offset distinto. El llamador identifica el recurso. |

La API admite una copia de lectura de vuelta, pero **esta version no
adquiere VRAM ni registros**. Tampoco presenta direcciones planeadas como
direcciones observadas. Un `&[u8]` debe ser una copia estable entregada por
quien conserva la autoridad, no memoria que cambia por DMA durante la vista.

## Guardian y pruebas

`la-3060`, S6, exige que la lente comun siga sin dependencias ni scripts de
build, y revisa recursivamente sus fuentes. Rechaza nombres de fabricante,
ISA, `unsafe`, ensamblador y accesos volatiles. S2 ahora revisa tambien
subcarpetas de la API y nombres AMD/NVIDIA. La consulta preparada no admite
las primitivas de acceso al aparato que vigila S6 y debe conservar su
procedencia CPU y espacio de recurso.

El guardian es una inspeccion estatica de patrones, no una demostracion
universal contra macros, alias o llamadas indirectas. Lo complementan el
compilador (`forbid(unsafe_code)`), las capas y la revision de las puertas.

Las pruebas negativas del guardian introducen fabricantes, dependencias
con alias/target, acceso directo y cambios de procedencia en fixtures
temporales; deben ser rechazados. No se afloja una linea base para pasar.
Las pruebas Rust reconstruyen los 256 valores de byte, EOF y ventanas
parciales, direcciones extremas, errores de salida y comparaciones. El
banco integra los dos shaders BMOX-12 y conserva una palabra EXIT literal.

Validacion de esta implementacion: 592 pruebas Rust aprobadas entre lente,
VERRANO, driver, emisor y codificador; ocho pruebas del guardian.
El CLI reconstruyo los 1.696 bytes del shader VS desde ambas columnas y
conservo una palabra desconocida y una cola de un byte. DIRECTOR release
enlazado; pila medida 45.984/65.536 B, marco mayor 19.552 B. Kernel comprobado
para su target. Pasan capas, contrato, censo modular, ASCII, enlaces y planes.

## Lo que sigue pendiente

| Etapa | Estado y evidencia necesaria |
| --- | --- |
| Bytes preparados y lector SM86 | Implementados y probados en anfitrion; DIRECTOR enlazado. |
| Consulta en BMO-X | Vista a las 23:23 del 16-10: 64 B de bmox-vs y 32 B de plantilla de ordenes; otras paginas/rechazos pendientes. |
| Copia recuperada del dispositivo | Pendiente una captura autorizada y acotada, despues de finalizacion y antes de reutilizar el recurso. |
| Identidad de la captura | Pendiente asociar recurso, build y numero de trabajo a preparado/recuperado para compararlos sin mezclar generaciones. |
| Registros del aparato | Pendiente lista permitida con efectos de lectura conocidos, captura en la puerta autorizada y errores de acceso conservados. |
| Otras ISA | Pendiente hardware y fuentes de cada adaptador. |

La captura futura debera usar las puertas existentes y respetar propiedad,
finalizacion, coherencia y devolucion de buffers. No habilitar lectura
arbitraria de MMIO/VRAM desde una app. El administrador de trabajos sigue
siendo parte de BMO-X; un programa de computo GPU no sustituye al GSP firmado.

Esta lente ayuda a localizar diferencias, pero no cierra G0/0x15 ni Q0a3.
Contexto: [estudio RTX / PROTON-X](../../drivers/gpu/ga10x/ESTUDIO_RTX3060_PROTON_X.md)
y [diagnostico VERRANO](../verrano/DIAGNOSTICO.md).
