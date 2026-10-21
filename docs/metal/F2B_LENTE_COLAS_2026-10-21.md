# F2b 615: observar el intervalo entre envio y respuesta

El [metal 4A1437648](LENTE_HS_METAL_2026-10-21_4A1437648.md) dejo un
recorrido con booter cero, RISC-V activo y LOGINIT 69. Publico GSP_INIT,
pero no confirmo respuesta en 30 s. Los otros tres conjuntos terminaron
en 0x15 antes de intentar este intercambio. Son dos fronteras distintas.

## Cambio

Se agrega observacion de solo lectura dentro de la inicializacion 615
existente, serializada por OCUPADO. CABINA/CAIDA conserva fotos con prefijo
`F2b foto:` y huella del mensaje construido con prefijo `F2b TX:`.

- Una foto previa a publicar TX_HEAD y otra al observar la publicacion.
- Fotos a partir de 100 ms, 1 s, 5 s, 15 s y 29 s de espera. Se toma la
  primera oportunidad disponible; los tiempos impresos son los reales.
  Si una llamada salta plazos no se inventan muestras retrospectivas.
- Una foto adicional cuando se valida la respuesta INIT. Maximo ocho
  fotos por intercambio. No hay nuevas esperas ni un sondeo autonomo.
- Ocho registros cerrados: TX_HEAD/TAIL, RX_HEAD/TAIL y BCR_CTRL,
  RISCV_CPUCTL, MAILBOX0/1 del GSP. Cada foto incluye mascara PRI y TSC
  antes/despues de leer, antes de escribir el detalle al registro CABINA.
- Cuatro palabras RAM de 64 bits de cada cabecera de cola. Indices 0..3
  pertenecen a TX, 4..7 a RX; se conservan incluso cuando son cero.
- BLAKE3 completo, como cuatro palabras little endian, de los bytes
  exactos de GSP_INIT construidos y validados antes de publicar.

La apertura de foto empaqueta id en bits 31:0 y fase en 63:32 (0 previa,
1 espera, 2 respuesta). Los registros empaquetan direccion en bits 31:0 y
crudo en 63:32. El cierre conserva id. Los identificadores son locales al
arranque; no son UUID ni numero de intento del booter. La lectura de cada
foto es secuencial, no atomica.

Pendientes TX y disponibles RX usan resta monotonica y limite de 63 huecos.
Un PRI o distancia imposible queda como u64::MAX, nunca como cola vacia.
No se interpreta un TX sin pendientes como prueba de ejecucion correcta.

## Que distinguir en el siguiente informe

| Observacion | Lo que permite estudiar |
| --- | --- |
| TX permanece con un pendiente | No se observa avance del consumidor; contrastar estado del GSP y cabeceras |
| TX deja de tener pendientes y RX sigue vacia | El contador cambio; buscar eventos/estado sin asumir respuesta INIT |
| Cabecera RX sigue cero | Diferenciar falta de publicacion de una cabecera publicada pero invalida |
| RX avanza | Comparar disponibilidad con el parser y los comandos recibidos |
| PRI o cambio de RISC-V | Conservar fallo de acceso/estado, sin llamarlo silencio del protocolo |

No se altera el constructor INIT, la seleccion de firma, reset, DMA,
permisos, numero de mensajes ni plazo de 30 s. El instrumental agrega
lecturas, hash CPU y escritura en CABINA: cambia tiempos y no demuestra
una causa del 0x15. Las fotos solo existen si el booter permite llegar a
GSP_INIT; no representan una apertura de HS ni un arreglo de G0.

## Validacion y siguiente metal

484 pruebas Rust GA10x/supervisor aprobadas con BMO_FW_GSP615 y BMO_FWSEC_ROM
activadas. Las dos nuevas pruebas comprueban lista cerrada sin escrituras,
PRI/distancia invalida, pendientes y limite temporal con saltos. Log local:
target/f2b-observacion-tests.log. Compilacion completa y guardianes aprobados
en 296 s (target/f2b-observacion-build.log). Tras guardar el commit se
regenera el paquete para que el informe identifique esta revision limpia.

Usar el paquete construido y el recorrido habitual de save mode. Conservar
el informe completo, incluidos CAIDA historicos y conjuntos cerrados. Si
sale 0x15 no se acredita probado F2b; si alcanza cola, contrastar la linea
temporal hasta el vencimiento o respuesta. No hace falta modificar QEMU
para esta medida: su VGA emulada no ejecuta el GSP de la RTX.
