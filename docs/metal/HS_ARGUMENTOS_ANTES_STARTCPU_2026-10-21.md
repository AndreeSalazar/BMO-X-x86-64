# HS: leer argumentos escritos antes de STARTCPU

Tras la [tanda 5A1C419F9](G0_2026-10-21_5A1C419F9.md), las cuatro pruebas
nuevas se detuvieron en 0x15 antes de llegar al lente de colas. El siguiente
cambio se coloca en el arranque del firmware, que si se alcanza.

## Hueco encontrado

La foto HS anterior se tomaba antes de llamar a falcon::arrancar_con. Esa
funcion escribia BOOTVEC, MAILBOX0 y MAILBOX1, y acto seguido STARTCPU.
La foto conservaba los ceros anteriores, no una lectura de los argumentos
recien escritos. La entrada solicitada se registraba aparte, pero no se
podia contrastar con una lectura posterior a esas escrituras.

Al comparar CAIDA002 del build 4A1437648 (booter cero) con CAIDA002 del
5A1C419F9 (0x15), las fotos HS previas de GSP/SEC2 tienen los mismos 21
crudos; la foto de parada SEC2 difiere en MAILBOX0. Esto no cubre los
tiempos, otros registros o todos los posibles estados internos. Tampoco
convierte los argumentos no escritos aun en una comprobacion de entrega.

## Cambio aplicado

falcon::arrancar_con_observador conserva las escrituras originales:
BOOTVEC si se pide, MAILBOX0 si se pide, MAILBOX1 si se pide, y STARTCPU
por CPUCTL o su alias segun el mismo bit existente. Ejecuta un observador
entre los argumentos y la lectura de CPUCTL que precede a STARTCPU.
arrancar_con conserva su interfaz y usa un observador vacio.

El observador HS del kernel toma la misma foto externa de 21 registros en
ese punto. Se aplica a FWSEC FRTS/SB, BOOTER_LOAD y BOOTER_UNLOAD.
No se agregan escrituras de aparato ni se repiten las existentes.
EXEC_HS_BINARY del corredor 615 conserva su instrumental anterior.

El marcador nuevo es `HS foto externa argumentos puestos antes STARTCPU`.
No debe confundirse con `HS foto externa previa a STARTCPU` de builds
anteriores: la posicion respecto a los argumentos es distinta. El lector
lente.py conserva el marcador completo como fase y acepta ambos.

Despues del cierre de foto se registran los valores esperados de los
campos pedidos, y las mascaras `HS arranque: pedidos | legibles << 8 |
iguales << 16`. En cada mascara: bit 0 BOOTVEC, 1 MAILBOX0, 2 MAILBOX1.
Un campo omitido no se compara con cero. Un PRI no cuenta como legible
ni igual, aunque coincida numericamente con el esperado. Para BOOTER_LOAD,
el caso de lectura legible e igual de los tres campos es 0x070707.

Es un diagnostico, no una nueva condicion de rechazo. Una discrepancia
conserva el dato crudo y la ejecucion original; PRI al leer CPUCTL sigue
produciendo el mismo rechazo antes de STARTCPU. No se convierte un fallo
del host en un retorno 0x15 inventado. La foto tampoco valida consumo DMA,
firma ni contenido de IMEM/DMEM protegidas.

El intervalo TSC de la operacion observada incluye ahora la foto y su
registro en CABINA: no mide exclusivamente la escritura STARTCPU. Las
lecturas son secuenciales, y el cambio de posicion tambien cambia tiempos.
No se atribuye una mejora o empeoramiento causal a una unica corrida.

## Validacion y siguiente prueba

Pruebas de regresion: argumentos visibles antes de STARTCPU en GSP y SEC2,
con y sin alias; exactamente las escrituras previstas, sin repetirlas;
escrituras ignoradas no se declaran iguales; CPUCTL PRI impide STARTCPU;
campo omitido, cero valido y PRI quedan separados. El lector Python acepta
la fase nueva y la distingue de la anterior.

Banco Rust: 487 pruebas aprobadas con firmware 615 y VBIOS reales activados
(target/hs-argumentos-tests.log). Banco Python HS: 35 ejecutadas, 30
aprobadas y cinco omitidas por entorno (target/hs-argumentos-python.log).
Kernel release aprobado en 44 s (target/hs-argumentos-kernel.log).
ASCII, CODEOWNERS y capas pasan. El paquete completo del commit se construye
con todos los guardianes antes de entregarlo; el metal sigue pendiente.

Usar el paquete del nuevo commit y save mode habitual. Comparar las fotos
de argumentos puestos y su mascara con la parada del mismo id de operacion
y build. Si todos coinciden pero vuelve 0x15, se habra comprobado esta
frontera externa; seguira pendiente identificar la rama interna. Si alguno
no coincide o da PRI, el informe identificara cual antes de STARTCPU.
No se declara arreglado G0 ni abierta la proteccion HS por este cambio.
