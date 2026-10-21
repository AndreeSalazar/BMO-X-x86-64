# Regresion FWSEC del build 43DB9E231 -- 21-10-2026

Informe maestro A:/informe, 11:30. Kernel y Director 43DB9E231D520A,
mismo commit limpio. Copia privada preservada en
BMO-externo/evidencias/gpu-20261021-43db9e231.

## Lo observado

- Despertar termina con motivo 25. CABINA detalla fallo al cargar FWSEC,
  motivo interno 7: DmaFuera.
- FRTS no arranco, WPR2 ausente y sin STARTCPU del booter SEC2.
- Solo una fase de captura publicada, sin conjunto cerrado ni listo.bin.
  No hay nuevos conjuntos binarios completos que validar o contar como
  retornos 0x15. Las repeticiones de CABINA no prueban sesiones distintas.
- Cola e INIT no se intentaron. No hay respuesta GMCAPI que evaluar.
- El paso fwsec HECHO de pasos.txt corresponde aqui a preparar FWSEC:
  con el modo junto, su ejecucion se realiza dentro de despertar.

## Causa comprobable y correccion

El cambio 43db9e231 agrego en copiar_etiquetado una condicion que rechazaba
`seguro && !imem`. firmar_cargar_arrancar, usado por FWSEC-FRTS y FWSEC-SB,
ya carga IMEM y DMEM con SEC=1. La segunda transferencia satisface esa
condicion y devuelve DmaFuera antes de sus escrituras DMA. El arranque
queda interrumpido antes de STARTCPU de FWSEC, sin llegar al booter.

Fue una restriccion introducida por esta implementacion, no una condicion
de OpenRM demostrada para FWSEC. Se elimina SOLO esa restriccion. Se
conservan comprobaciones FULL/IDLE, alineacion, anchos de direccion,
intervalos, capacidades SEC2 y rechazo PRI al elegir firmas.

La nueva prueba sintetica reproduce las dos llamadas y comprueba comandos
IMEM 0x614 y DMEM 0x604, ademas de los datos transferidos. Otra prueba toma
la VBIOS real mediante BMO_FWSEC_ROM: analiza su descriptor y reproduce
las dos transferencias con las mismas medidas, destinos y SEC del kernel.
La VBIOS aportada contiene IMEM de 57856 bytes y DMEM de 2048 bytes;
la simulacion comprueba contenido y ausencia de accesos fuera de la memoria
prestada. No ejecuta firmware ni comprueba el comportamiento electrico.

Banco con BMO_FWSEC_ROM y BMO_FW_GSP615 activadas: 479 pruebas aprobadas,
incluido el paquete real 615 y las dos regresiones nuevas. Log local:
target/fwsec-sec-regresion-tests.log. Se reconstruye el paquete completo
del commit corregido antes de entregar; no se copia a la unidad A:.

Esta correccion elimina el rechazo determinista agregado a FWSEC. No
demuestra resuelto el retorno 0x15 anterior: esta tanda ni siquiera llego
a ejecutar el booter que lo devuelve. G0 y la respuesta GSP_INIT siguen
pendientes de validacion en metal.
