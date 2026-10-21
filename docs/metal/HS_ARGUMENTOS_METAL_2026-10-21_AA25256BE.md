# Argumentos HS en metal: AA25256BE, 21-10-2026

Informe maestro de las 13:55: kernel y Director AA25256BE4ABA0, mismo
commit limpio. Copia privada en BMO-externo/evidencias/gpu-20261021-aa25256be.

Tres conjuntos cerrados validos, cuatro fases y cero perdidas por conjunto:
150D3627/A9965DFB, 150D3705/870F0ABC y 150D3722/34A7A082. Todos pertenecen
a este build y declaran intento 1 en su sesion. Los tres terminan con
SEC2 MAILBOX0=0x15 y LOGINIT PUT=0. No se alcanza GSP_INIT.

El [punto de observacion corregido](HS_ARGUMENTOS_ANTES_STARTCPU_2026-10-21.md)
si se alcanza. Las fotos de argumentos puestos del SEC2 muestran:

| Registro antes de STARTCPU | Crudo observado | Solicitado |
| --- | --- | --- |
| BOOTVEC | 0x00000100 | 0x00000100 |
| MAILBOX0 | 0x3E007000 | 0x3E007000 |
| MAILBOX1 | 0x00000000 | 0x00000000 |

Mascara 460551 decimal = 0x070707: los tres campos pedidos, legibles e
iguales. FWSEC registra 197379 = 0x030303: BOOTVEC y MAILBOX0 pedidos,
legibles e iguales; MAILBOX1 no pedido. Las fotos se decodifican con lente.py
y sus JSON se conservan en la copia privada. El historial repetido de los
CAIDA no cuenta como nuevos intentos.

Conclusion acotada: estos tres registros tienen los valores solicitados
en el punto observado, despues de escribirlos y antes de STARTCPU. No son
los ceros previos de la foto antigua. Esto no verifica los bytes que la
GPU consume por DMA, la traduccion efectiva de sus lecturas, autenticacion
HS, ni el contenido de la memoria apuntada por MAILBOX0. La foto no revela
la comprobacion interna que devuelve 0x15. G0 sigue abierto; no corresponde
repetir esta prueba como si faltara una lectura de argumentos puestos.

No hay cambio de kernel ni paquete nuevo por esta anotacion.
