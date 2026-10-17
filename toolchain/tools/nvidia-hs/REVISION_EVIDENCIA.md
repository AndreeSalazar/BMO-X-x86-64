# G0: revision del historial antes de nuevos ensayos

17-10-2026. Fuentes locales: [tabla de arranques](../../../platform/drivers/gpu/ga10x/EL_0x15_ARRANQUES.csv)
y [GCB2b, secciones 7 a 10](../../../platform/drivers/gpu/ga10x/GCB2B_ENTRADAS.md).
Se revisan transcripciones existentes; no se anuncian nuevas mediciones.

## Integridad y unidad de observacion

La tabla contiene 42 filas de datos: 14 etiquetadas `bien` y 28 `0x15`.
La ultima (17-10, 10:40-11:00) resume VARIOS fallos comunicados sin informe
individual. Por tanto, 42 filas NO son 42 intentos y 14/42 no es una tasa
valida de exito. Las otras 41 filas describen arranques individuales, sin
garantia de muestreo completo ni independencia entre ellos.

Corregido solo el formato CSV de la nota del 17-10 09:21: `~0,7 ms` tenia
una coma sin comillas que creaba un campo adicional y truncaba `nota` en
`csv.DictReader`. Se conserva todo el texto y no cambia ninguna medicion.
Comprobacion: 42 filas, 16 campos cada una, sin sobrantes ni ausencias de
columnas; los campos vacios originales siguen vacios.

## Pares que ya podemos estudiar

| Evidencia | Lo que aporta | Lo que no demuestra |
| --- | --- | --- |
| 17-10 07:09 / 07:16, build 665438EE9AA456 | Control y E2 sin D:, ambos 0x15 | Que D: sea irrelevante en todas las condiciones |
| 17-10 08:34 / 08:45 / 08:59, build F153F3DB64763A | Controles y E3 RISC-V, todos 0x15 | Que el modo no influya nunca; solo no basto en estos intentos |
| 17-10 09:04, E4 | Se comunico que el hueco siguio en 118 ms | Haber probado quitar el hueco; la manipulacion no se logro |
| 17-10 09:21, build CF9AE7F2E80CCD | Bueno con autopsia; vigia y campos transcritos iguales a malos | Par bueno/malo con el mismo build y archivos identificados |
| 17-10 10:33, build CF3646E2710789 | Bueno E5 reset tras reboot en caliente | Que reset resuelva G0; los fallos agregados carecen de identidad individual |
| E6 PTIMER, texto de GCB2b | Un fallo comunicado sin informe | Comparacion instrumental ni motivo interno del fallo |

Los nombres de build anteriores proceden de las transcripciones. No se
reconstruyen hashes SHA-256 del firmware ni informes originales a partir
de esas etiquetas. `fria registrada` describe estado declarado de la GPU;
no se traduce automaticamente a un corte fisico de corriente del equipo.

## Inferencias que deben seguir abiertas

La diferencia entre 247011 us del bueno y 246200-246500 us de varios malos
no localiza una instruccion ni un chequeo del booter. Es una diferencia de
duracion, no una traza de ejecucion. La frase historica "ultimo tramo"
es una hipotesis que requiere otra evidencia.

Vigia y metadata iguales solo cubren lo que midio cada mecanismo. No
prueban igualdad temporal de todos los bytes consumidos por DMA ni de todos
los estados internos. Una traza SEC2 a cero en buenos y malos no distingue
el fallo y no se propone repetirla como si fuera un instrumento nuevo.

E2 y E3 debilitan explicaciones simples, no excluyen toda interaccion.
E5 mixto no justifica un arreglo declarado. E4 sigue pendiente de un ensayo
que realmente cambie la variable; no se implementa otro reset por intuicion.

## Siguiente lote util

Mantener un build y modo normal identificados; conservar cada `save` en su
propia carpeta, incluido cada fallo, con fecha/intento y origen del arranque.
Inventariar los archivos reales de ese lote con `sesion.py`. Hace falta al
menos un bueno y un malo del mismo build para la primera comparacion de
contexto, huellas y campos; si no aparecen ambos, conservar el resultado
sin cambiar condiciones para fabricar el par. Un par no demuestra causa.

La revision actual dispone del CSV y del estudio, no de las copias binarias
ni los informes originales de esos intentos. No se generan sesiones que
presenten transcripciones como si fueran capturas originales verificadas.

Con las copias: comprobar identidad, firmware/booter, punto de captura,
WPR/metadata, foto previa y PUT de LIBOS, y separar ausente de cero. Solo
entonces elegir una discrepancia comprobable y una variable para el ensayo
siguiente. El [laboratorio](LABORATORIO.md) conserva los comandos y limites.
