# GSP 570.144: lo comprobado y las preguntas que siguen abiertas

17-10-2026. GA106, RTX 3060 12G. Investigacion del estado de `d042729c2`
y lector agregado despues, sin modificar la secuencia de arranque.

Etiquetas: **[CITADO]** fuente primaria indicada, o artefacto local identificado;
**[INFERIDO]** conclusion contrastable; **[NO LO SE]** falta evidencia. Ningun
codigo privado del booter se presenta como conocido.

## 1. Que significa SEC2 MAILBOX0 = 0x15

**[CITADO]** OpenRM 570.144, `kernel_gsp_booter_tu102.c`,
`s_executeBooterUcode_TU102`: separa el resultado del ejecutor HS del valor
del buzon. Si el ejecutor termina bien pero MAILBOX0 no es cero, devuelve
`NV_ERR_GENERIC`. `kgspExecuteBooterLoad_TU102` entrega la direccion fisica
de los argumentos dividida entre MAILBOX0/1. [Fuente B](#fuentes).

| Valor/espacio | Interpretacion demostrada |
| --- | --- |
| [CITADO] MAILBOX0 de salida = 0 | El wrapper acepta el resultado del booter. No demuestra por si solo INIT_DONE. [B, T] |
| [CITADO] MAILBOX0 de salida distinto de 0 | Error del booter, sin traduccion especifica en ese wrapper. [B] |
| [CITADO] 0xFF/0xFF | Argumentos centinela del **unloader**, no errores decodificados. [B] |
| [CITADO] 0xDEADDEAD | Argumento de descarga para entrada GC6, no un error de carga. [B] |
| [NO LO SE] 0x15 y otros errores privados | No encontre enum ni tabla del booter en el tag consultado. Hace falta el enum de esa build o identificar una rama del firmware mediante un experimento. |

**[CITADO]** `nvstatuscodes.h` asigna 0x15 a `NV_ERR_I2C_SPEED_TOO_HIGH` en
**otro espacio**, NV_STATUS. [Fuente V](#fuentes). **[INFERIDO]** traducir el
buzon como ese error I2C seria mezclar dos contratos sin evidencia.

## 2. Que chequea y que hipotesis encaja

**[CITADO]** Hay chequeos visibles del **host**, anteriores a ejecutar el
booter: `s_patchBooterUcodeSignature` selecciona firma segun el fusible SEC2,
rechaza version sin firma y valida limites de destino; `s_imemCopyTo_TU102` exige
alineaciones del destino y de la medida. No son una lista de las ramas del
firmware firmado. [Fuentes P y H](#fuentes).

**[NO LO SE]** No puedo enumerar el orden interno de comprobaciones de
WPR2, FRTS, revocacion, GSP, firmas o metadatos que conducen a 0x15.
Un PC sin simbolos tampoco lo demuestra. Lo resolverian codigo/simbolos de
esa build o una divergencia reproducible al variar un solo factor.

**[CITADO]** El `A:/informe/salida.txt` leido hoy registra E2 `sin-d`,
intento 1, build `665438EE9AA456`, FRTS correcto, GSP previo en Falcon y
0x15; WPR2 pasa de ausente al llegar a extendida despues. El informe no
contiene aun la traza nueva del commit `3c76c493d`. No se incorporan sus
datos crudos al repositorio.

**[INFERIDO]** Esa extension indica progreso de la cadena, pero no prueba
que cada byte se haya consumido bien, ni que el booter haya pasado todas
las firmas. E2 debilita la hipotesis del montaje D como causa suficiente.
El caso con HI=0 al llegar debilita la explicacion universal "WPR2 heredada".

**[INFERIDO]** Un cambio de build/protocolo y una ventana temporal merecen
prioridad. Metadatos constantes no excluyen problemas de entrega, estado
interno o traduccion de memoria. Los 7/7 frente a 1/13 justifican investigar
un cambio de regimen; no identifican su causa. No valido la probabilidad
1/9700 sin especificar independencia, seleccion de la fecha e hipotesis nula.

## 3. Falcon frente a RISC-V con BRFETCH

**[CITADO]** `kgspBootstrap_TU102`, camino NORMAL: FWSEC-FRTS cuando procede,
`kflcnResetIntoRiscv_HAL`, argumentos LIBOS, scrubber si existe, booter,
programacion de OS y comprobacion de RISC-V activo. [Fuente T](#fuentes).

**[CITADO]** `kflcnResetIntoRiscv_GA102` espera, resetea, espera finalizacion
y llama a `kflcnRiscvProgramBcr_GA102(true)`: RISCV, VALID y BRFETCH. Esa
rutina no da STARTCPU. [Fuente F](#fuentes).

**[CITADO]** La revision nova fijada abajo usa `Tu102::boot`: reset del
falcon GSP, `boot` con argumentos LIBOS y booter en SEC2. [Fuente N](#fuentes).
**[NO LO SE]** Que la precondicion Falcon sea universalmente valida para
este booter GA106. Nova es un contraste, no la especificacion privada.
**[INFERIDO]** E3 compara dos protocolos completos; no se debe cambiar BCR
y conservar una espera de HALT que solo tenia sentido en el camino Falcon.

## 4. BADF1002, BADF5620 y recuperacion

**[CITADO]** `kflcnIsRiscvSelected_GA102` reconoce el patron BADFxxxx como
lectura privada fallida y contempla bloqueo de lectura por PLM. No afirma
que todo BADF pruebe que el firmware esta funcionando. [F]

**[CITADO]** `dev_pri_ringstation_sys.h` de gm107 define el campo 31:8
BADF10 como FECS_PRI_TIMEOUT, BADF11 decode y BADF12 reset. [Fuente D](#fuentes).
**[INFERIDO]** BADF1002 es compatible con esa familia timeout. La cabecera
gm107 no basta para asignar el sufijo 02 a un origen concreto de GA106.
**[NO LO SE]** La decodificacion exacta de BADF5620 en GA106. No lo llamo
"lockdown demostrado". Medir acceso a PGSP_FALCON_ENGINE, BCR, control SEC2
y errores PRI del mismo intento distingue accesibilidad parcial de reset;
para la causa exacta falta la tabla de retorno PRI de esa generacion.

**[CITADO]** `kgspResetHw_TU102`: FALCON_ENGINE.RESET=true, lecturas de
propagacion, false y mas lecturas. Esa funcion no alterna PMC_ENABLE. [T]
**[CITADO]** `kflcnSwitchToFalcon_GA102` documenta liberar priv lockdown y
advierte no invocarla dentro de reset. [F]

**[CITADO]** Nova `Sec2UnloadBundle::run` intenta FWSEC-SB, luego unloader
SEC2 si WPR2 existe, verifica buzon y desaparicion de WPR2. OpenRM
`kgspExecuteBooterUnloadIfNeeded_TU102` tambien comprueba WPR2 y el resultado
de descarga. [N, B]
**[NO LO SE]** Si esa secuencia recupera tu fallo sin cortar corriente.
Los informes historicos incluyen descarga con 0x15. **[INFERIDO]** No se
debe repetir reset/unload a ciegas ni escribir WPR2 desde el host: conservar
la autopsia y exigir postcondiciones antes de volver a prestar memoria.

## 5. Estado residual y foto anterior a FWSEC

**[CITADO]** `kgspIsWpr2Up_TU102` extrae HI.VAL y comprueba que no sea
cero; no basta con `LO != 0`. Los registros son LO=0x001FA824,
HI=0x001FA828, campo VAL=31:4, alineacion de 12 bits. [T, R]

**[CITADO]** Nova `Tu102::run_fwsec_frts` rechaza WPR2 preexistente con
EBUSY antes de ejecutar FWSEC. [N] **[NO LO SE]** No encuentro evidencia
para afirmar que el booter siempre devuelve 0x15 en ese caso o que OpenRM
simplemente salte FWSEC cuando detecta una WPR2 heredada.

**[INFERIDO]** Foto minima, con build/intento y orden temporal: WPR2 LO/HI
crudos, GSP FALCON_ENGINE, CPUCTL, BCR, ambos buzones, BSI_14 y progreso
GFW; luego la misma foto tras FRTS y tras booter. Si una lectura es BADF,
marcarla ilegible antes de extraer campos. LO=1FFFFE00, HI=0, como en la
muestra actual, significa que ese indicador no encuentra WPR2 activa.
Las constantes y propietarios de BSI/GFW siguen en `lectura/aon.rs`.

## 6. Cinco experimentos, un factor por comparacion

**[INFERIDO]** Orden por informacion/coste con las herramientas ya presentes.
No mezclar modificaciones ni considerar una unica victoria una correccion.
Usar el mismo firmware, metodo de llegada y receta; registrar build de kernel
y Director, intento, tiempos, buzon, WPR2, autopsia y traza SEC2.

| Orden | Factor unico / hipotesis | Si es cierta | Dato que discrimina |
| --- | --- | --- | --- |
| [INFERIDO] 1 | E3, `gpu g0 riscv`: protocolo previo GSP | Cambia la tasa 0x15 y hay progreso posterior | BCR legible antes del booter con RISCV/VALID/BRFETCH; SEC2 MAILBOX0=0, RISC-V activo e INIT_DONE. Un buzon 0 aislado no basta. |
| [INFERIDO] 2 | E4, `gpu g0 seguido`: ventana entre FRTS y booter | La incidencia depende de acortar esa ventana | Tiempo FRTS->booter, misma precondicion BCR, misma WPR meta y diferencia de buzon/traza. Si E3 cambio el modo, volver primero al control. |
| [INFERIDO] 3 | E1, build `a2408875a` frente al control | El comportamiento sigue a la imagen del sistema | Identidad/hashes de las imagenes usadas y buzon. Es cribado de una regresion, no identifica una linea; si mejora, bisectar. |
| [INFERIDO] 4 | Tipo de llegada: frio frente a reinicio caliente, misma imagen | La diferencia sigue al estado heredado | WPR2 HI.VAL y BSI/BCR **antes** de FWSEC, despues buzon. HI=0 en ambos refuta solo el indicador WPR2, no todo estado residual. |
| [INFERIDO] 5 | Descarga ordenada antes del reinicio caliente, frente a reinicio caliente sin ella | El residuo evitable desaparece tras descarga correcta | Buzon del unloader=0 y WPR2 HI.VAL=0 antes de reiniciar; nueva foto al llegar. Si descarga falla, el experimento queda inconcluso. |

**[CITADO]** E2 `sin-d` ya produjo 0x15 en el informe actual. No pide otro
reinicio para repetir exactamente ese dato. E3 sigue pendiente de metal.

## 7. Que permite TRACEPC sin codigo del booter

**[CITADO]** Nouveau `gm200_flcn_tracepc`: lee SCTL+0x240, TRACEIDX+0x148,
extrae cantidad de bits 16..23, escribe cada indice en +0x148 y lee PC en
+0x14C. No es una captura de todas las instrucciones ni una tabla de
errores. Consultar la traza escribe el selector; preservar la foto inicial.
[Fuente C](#fuentes).

**[NO LO SE]** No puedo asignar PCs desconocidos a firma, WPR o espera
del GSP. Hace falta correspondencia PC->rutina de la misma imagen, o una
referencia de fallo provocado y aislado. **[INFERIDO]** Comparar rutas y
duracion entre exito/fallo puede localizar una divergencia; un PC repetido
solo sugiere una espera si la semantica y cobertura de la traza lo apoyan.
Un bucle interno que termina en error no equivale al timeout del host.

**[CITADO: codigo local]** La sonda nueva de BMO-X conserva como maximo 32
PCs (`gpu_despertar.rs::TRAZA_PCS`), ademas de SCTL/TRACEIDX. Si el contador
del hardware es mayor, la muestra es parcial; no llamarla traza completa.

**[CITADO]** `kgspExecuteHsFalcon_TU102` espera HALT y devuelve ese estado
separado de los buzones. **[INFERIDO]** Clasificar primero: HALT observado
con buzon no cero, o plazo del host agotado. No llamar "timeout GSP" al
primero sin prueba adicional. [H]

## 8. LIBOS: donde estan las cadenas y que significa un log vacio

**[CITADO: medicion local]** Se descargo el paquete oficial sin ejecutarlo:
`NVIDIA-Linux-x86_64-570.144.run`, 375742470 bytes. MD5 de los bytes a partir
de la cuarta linea, segun su cabecera: `7fbffdb4ab51ec70175a2aaed9d0eb44`,
coincide. Esto comprueba integridad frente a la cabecera, no una firma nueva.
Se leyo el tar Zstandard y solo se extrajo `firmware/gsp_ga10x.bin`.
[Paquete oficial](https://download.nvidia.com/XFree86/Linux-x86_64/570.144/).

**[CITADO: medicion local]** El binario extraido y el de BMO-externo miden
63571696 bytes y tienen el mismo SHA-256:
`a8c3ebeed280323aedb51c061f321e73379cce7a9ae643a33dd03915df027f7f`.
Inventario ELF: 14 secciones, cero `.fwlogging*`, `.fwimage` en offset 0x40,
63541248 bytes; firma GA10x en 0x03C9F06C, 4096 bytes. `.fwimage` empieza
en instrucciones, no en una cabecera ELF anidada. No se afirma que carezca
de toda cadena interna: falta el diccionario direccionable de logging.

**[CITADO]** OpenRM `libos_log.h` explica que metadata y formatos se emiten
en `.logging` y se retiran en el paso final. `_kgspInitLibosLogDecoder`
acepta `pLogElf` separado, verifica version y extrae `.fwlogging`; las
tareas usan `.fwlogging_init`, `.fwlogging_rm` y la seccion kernel GA10x.
No es correcto asumir que el `.run` publico conserva ese material. [L, G]

**[CITADO]** `LibosElfMapVirtual`: con PHDR, offset = VA - p_vaddr +
p_offset; si no, busca SHDR y usa VA - sh_addr + sh_offset. La VA es del
**ELF de la tarea de logging**, no una direccion fisica de VRAM ni un
offset directo en `.fwimage`. [Fuente E](#fuentes).

**[CITADO]** `libosExtractLog_ReadRecord` recorre hacia atras timestamp,
metadata y argumentos; su numero depende del diccionario. El marcador
extendido incluye referencia a otra tarea. [Fuente A](#fuentes).
**[NO LO SE]** Donde conseguir legalmente el ELF de logs exacto de esta
build publica. La descarga comprobada no lo aporta. No sustituirlo por
metadata de otra version, ni traducir 0x15 usando textos sin relacion.

**[CITADO]** `kgspSetupLibosInitArgs_IMPL` requiere LOGINIT primero para
logging temprano; el host suspende el sondeo de logs mientras ejecuta el
booter. Suspender sondeo no impide que el GSP escriba. [G, B]
**[NO LO SE]** Si el GSP llega a ejecutar logging antes del retorno de
este booter concreto. Medir PUT antes/despues y relacionarlo con HALT
resuelve actividad publicada; la ausencia no localiza la causa del fallo.

**[CITADO: medicion local]** `A:/informe/gpu/gsplog.bin`, fecha 17-10
07:09:44, 196608 bytes, SHA-256
`f310a8ec5ce020ededce9514ad9073bd5bdf45c45b2a9b6f861812e8cdecc4fa`:
los tres PUT valen cero. Hay 141 bytes no nulos, exactamente las entradas
de paginas iniciales a partir de +8 de cada log, que el kernel prepara en
`gpu_libos.rs::libos`. **[INFERIDO]** No hay registros publicados por
esos PUT en esta copia; esos bytes no son mensajes del GSP. No demuestra
que el GSP nunca ejecutara ni escribiera parcialmente. El archivo no
incluye garantia temporal de coherencia ni identidad propia de build.

## Lector incorporado y limites de entrega

**[CITADO: codigo local]** `../Nvidia/libos-bmo/src/anillo.rs` lee copias del anillo sin
unsafe: limites, wrap, cabecera cruda, argc maximo 20 y parada ante
diccionario ausente/truncado. `../Nvidia/libos-bmo/src/diccionario.rs` valida ELF64 LE,
tablas acotadas, VA a offset, metadata normal/extendida y cadenas acotadas;
rechaza BSS y mapeos ambiguos. No accede a MMIO ni tiene autoridad de reset.

**[CITADO: codigo local]** Director usa ese lector para el inventario real
de `gpu gsp`/`save` y resumen de LOGINIT, y diferencia reserva ausente de PUT
cero. El kernel ya entrega la evidencia DMA por su puerta existente. No
se agregan punteros, unsafe ni un segundo propietario del hardware.

**[CITADO: codigo local]** Ejemplo de anfitrion, desde la raiz:

```powershell
cargo run -p bmo-gpu-ga10x --example libos -- target/gsp-investigacion/570.144/gsp_ga10x.bin A:/informe/gpu/gsplog.bin
```

**[CITADO: codigo local]** Un tercer argumento opcional recibe el ELF
**de la tarea y build correcta**. Emite formato literal escapado y
argumentos separados; no implementa todo printf, simbolos remotos ni
traduccion universal. El llamador sigue siendo responsable de acreditar
esa identidad: este primer lector no autentica un diccionario externo.
Sin el tercer argumento solo inventario, PUT y ultima cabecera cruda.

**[INFERIDO]** Esto abre GCB3b, no lo cierra: faltan diccionario real,
formatos completos y muestras con registros. G0 y E3 siguen abiertos.
No se puede "ver toda la GPU" a partir de un ELF firmado sin simbolos.

## Fuentes

Todas NVIDIA fijadas al tag **570.144**. Los nombres de funcion de cada
apartado permiten contrastar sin depender de una numeracion de lineas.

- B: [kernel_gsp_booter_tu102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_booter_tu102.c)
- T: [kernel_gsp_tu102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c)
- F: [kernel_falcon_ga102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/falcon/arch/ampere/kernel_falcon_ga102.c)
- P: [kernel_gsp_booter.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/gsp/kernel_gsp_booter.c)
- H: [kernel_gsp_falcon_tu102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_falcon_tu102.c)
- V: [nvstatuscodes.h](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/sdk/nvidia/inc/nvstatuscodes.h)
- D: [dev_pri_ringstation_sys.h (gm107)](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/inc/swref/published/maxwell/gm107/dev_pri_ringstation_sys.h)
- R: [dev_fb.h (tu102)](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/inc/swref/published/turing/tu102/dev_fb.h)
- G: [kernel_gsp.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c)
- L: [libos_log.h](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/uproc/os/common/include/libos_log.h)
- A: [liblogdecode.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/uproc/os/libos-v3.1.0/lib/liblogdecode.c)
- E: [libelf.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/common/uproc/os/libos-v3.1.0/lib/libelf.c)
- N: [nova-core tu102.rs, revision 91645a52ebf2e22f0bd37c142b79238617d38758](https://github.com/torvalds/linux/blob/91645a52ebf2e22f0bd37c142b79238617d38758/drivers/gpu/nova-core/gsp/hal/tu102.rs)
- C: [nouveau gm200.c, misma revision](https://github.com/torvalds/linux/blob/91645a52ebf2e22f0bd37c142b79238617d38758/drivers/gpu/drm/nouveau/nvkm/falcon/gm200.c)

Los paquetes descargados, dependencias de extraccion y fuentes ajenas viven
en `target/gsp-investigacion/570.144/` (ignorado). Ningun firmware, instalador
ni informe crudo entra en el commit.

**[CITADO: validacion local]** Ocho pruebas nuevas del lector incluidas en 417 pruebas
aprobadas del paquete grafico; Director release x86-64 bare-metal construido,
pila 45984/65536 bytes. Esto no es una nueva prueba fisica de E3 ni de G0.
`verificar.ps1 -Rapido` llego a TODO VERIFICADO en 72,5 s: contrato y
guardianes, omitiendo explicitamente el banco global. No se presenta esa
corrida como las pruebas completas de todo el workspace.
