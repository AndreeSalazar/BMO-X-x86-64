# PLAN GSP-CONTROL-BMO -- gobernar el GSP, no reemplazarlo

Pedido del propietario, 16-10-2026: un crate propio en carpeta `Nvidia`,
que crezca entendiendo SU RTX 3060 12G. **Redefinido el 17-10-2026** por el
propietario: *"no se trata de tomar todo, el GSP ya hace bien lo que hace
bien; es quien lo gobierna, y eso es lo que quiero saber para gobernar bien"*.

El nombre del crate (`bmo-nvidia-gsp-control`) y de la orden (`gpu control`)
se conservan. Lo que cambia es la PREGUNTA: ya no es "que hay dentro del GSP"
sino "como lo gobierna BMO-X".

## 1. La idea en una frase

**El GSP ejecuta; BMO-X gobierna.** Gobernar es dar posesion, conceder,
pedir, escuchar y revocar. No es hacer el trabajo del GSP.

El GSP-RM (el firmware firmado de NVIDIA, 570.144) administra por dentro la
tarjeta: motores, canales, energia, relojes, errores. Lo hace bien y no hay
que reescribirlo. En Windows y Linux quien lo gobierna es el driver del
anfitrion (el "cliente" de OpenRM, o nova-core): le habla por RPC sobre colas
en memoria compartida. **En BMO-X ese papel ya lo hace el kernel**: es el
mismo trato de la casa -- el programa DECLARA, el sistema CONCEDE, el kernel
COMPRUEBA -- con el GSP como un miembro mas de la orquesta.

## 2. Las cinco palancas, y lo que ya existe de cada una

El gobierno ya existe, pero esta REPARTIDO e implicito. El inventario:

| Palanca | Que se gobierna | Lo que ya hay en el arbol |
| --- | --- | --- |
| 1. Dar posesion | La cadena FWSEC -> booter (SEC2) -> GSP-RM. Por dentro no se toca; se gobiernan sus ENTRADAS: FRTS/WPR2, argumentos de LIBOS, memoria prestada, orden y esperas | `ga10x/src/arranque/` (fwsec, booter, wpr, libos, secuenciador); kernel `gpu_despertar.rs`, `gpu_libos.rs` |
| 2. Conceder | Memoria (heap LIBOS, colas, 3 logs), paginas por IOMMU, claves de registro, informacion del sistema | `rm/orden.rs` (`GSP_SET_SYSTEM_INFO`, `SET_REGISTRY`); kernel `gpu_prestamo.rs` y la IOMMU |
| 3. Pedir | Objetos con asa (cliente -> dispositivo -> subdispositivo -> canal) y ordenes de control | `rm/objeto.rs` (`GSP_RM_ALLOC`), `rm/control.rs` (`GSP_RM_CONTROL`: P-state, `PERF_BOOST`, `PROMOTE_CTX`...), `rm/contrato.rs` (la lista CERRADA de lo que sale) |
| 4. Escuchar | Eventos de la cola del GSP y sus logs propios | `rm/rpc.rs` (nombra `POST_EVENT`, `RC_TRIGGERED`, `MMU_FAULT_QUEUED`, `OS_ERROR_LOG`, `GSP_POST_NOCAT_RECORD`), `rm/rc.rs` (el parte del canal muerto); Director guarda los 3 logs CRUDOS en `informe/gpu/gsplog.bin` sin decodificar |
| 5. Recuperar y despedir | Que hacer ante un Xid sin reiniciar la PC, el reintento, el apagado en orden | `rm/liberar.rs` (E7b), `rm/vigilante.rs` (el TDR de la casa), kernel `gpu_reintento.rs`, `gpu_apagar.rs` |

Una asa de `GSP_RM_ALLOC` es exactamente una capacidad de BMO-X: la
autoridad sobre la tarjeta es funcional, nunca heredada. Una app (por
VERRANO) jamas habla con el GSP; pide al kernel, y el kernel decide.

## 3. Lo que NO es este plan, con motivo

- **No es un segundo driver.** Los propietarios del aparato siguen siendo
  los de la tabla del [README del crate](../../platform/drivers/gpu/Nvidia/gsp-control-bmo/README.md)
  (GCB0a). El crate es puro:
  `no_std`, sin `unsafe`, sin MMIO, vigilado por S8 de `la-3060`.
- **APARCADO -- el estudio del RISC-V del GSP** (antes GCB4). Motivo: no
  ayuda a gobernar. Lo que se necesita del firmware son sus MENSAJES, no su
  ISA. Lo retoma: una pregunta de G0 que solo se conteste leyendo su codigo.
- **APARCADO -- un mini-kernel BMO dentro del GSP** (antes seccion 6).
  Motivo: el booter y el GSP-RM van firmados por NVIDIA y la cadena de
  confianza los verifica; no hay ruta demostrada para firmware propio en una
  RTX de consumo, y gobernar no lo necesita. Lo retoma: una ruta de carga
  demostrada y documentada por una fuente primaria. Sin eso, no se abre.
- **No es un porcentaje de "conocer toda la GPU".** Cada medida declara su
  denominador (RPC catalogados, eventos decodificados, transiciones probadas).

## 4. Etapas con criterio de llegada

El orden es el de la prioridad de la casa: G0 primero.

### GCB0 -- inventario de propietarios (HECHO)

- [x] GCB0a: inventariar propietarios actuales de LIBOS, arranque, canales,
      memoria y recuperacion; declarar quien es el unico que ejecuta cada accion.
- [x] GCB0b: escribir contrato de observacion/decision con identificacion de
      tarjeta, firmware, build e intento; conservar desconocido y dato ausente.

### GCB1 -- palanca 1, capitulo uno: el ciclo de vida del arranque (HECHO)

- [x] GCB1a: crear el crate con estados SinPreparar, Preparado, Arrancando,
      Operativo, Fallido y RecuperacionBloqueada; transiciones explicitas.
- [x] GCB1b: probar eventos fuera de orden, duplicados, observaciones antiguas,
      tiempos agotados y recuperacion no permitida; ningun fallo se anuncia operativo.

Implementado el 16-10-2026: [contrato e inventario](../../platform/drivers/gpu/Nvidia/gsp-control-bmo/README.md),
crate puro y `gpu control` incluido en `save`. Hoy solo usa la foto
(`diagnose`): la ABI no entrega build ni intento, y sin ellos el modelo con
historia no se abre. Eso lo cierra GCB2a.
El 17-10 se conecta la autoprueba negativa de S8 a cada check/build; el
portico NVIDIA es obligatorio, y sin Python no aprueba. La medicion y el
reparto del banco se documentan en [VERIFICAR](../../toolchain/tools/verificar/README.md).

### GCB2 -- palanca 1: dar posesion bien (G0, PRIORIDAD)

- [x] GCB2a: entregar al supervisor build e intento reales (OBS-BUILD de
      EL_FOCO y un contador de intentos del kernel) para que cada arranque,
      bueno o malo, quede como una sesion con historia y no solo como una foto.

Hecho en el anfitrion el 17-10, falta verlo en el metal. `INFO_BUILD`
(0xDB): el commit con que se compilo el kernel, armado por `kernel/build.rs`
(`Ultra_kernel_x86-64/build/obs_build.rs`) en cada build; bit SUCIO si habia
cambios sin commit, y 0 si git no contesto -- nunca un commit inventado.
`INFO_GPU_INTENTO` (0xDC): cuantas veces se solto el booter desde que
arranco la maquina; el reintento suma y `rearmar` no lo borra. `gpu control`
arma con ellos una identidad completa, y la cabecera de cada `save` dice el
build del kernel y el del DIRECTOR y si salieron del mismo commit. Lo que
NO hace todavia: alimentar el modelo con eventos (eso es GCB5b); con la
identidad completa, esa historia ya se puede abrir sin inventarla.
**Visto en el metal el 17-10** (build `89E4313C14499A`, intento 1):
[parte](../metal/METAL_2026-10-17.md).
- [x] GCB2b: comparar TODAS las entradas que recibe el booter (FRTS, WPR2,
      argumentos de LIBOS, RMARGS, orden y esperas) contra OpenRM 570.144 y
      nova-core, campo a campo, con la diferencia escrita aunque parezca inocua.

Hecho el 17-10: [GCB2B_ENTRADAS](../../platform/drivers/gpu/ga10x/GCB2B_ENTRADAS.md).
La WPR meta es la de nova-core campo a campo y no cambia entre arranques
buenos y malos (vigia igual). El procedimiento difiere de OpenRM 570.144 en
el falcon del GSP antes del booter (G0-MODO) y en el tiempo (533 ms contra
milisegundos). La tabla de arranques da un cambio de regimen el 29-09 (7 de
7 buenos antes, 1 de 13 despues; 1 entre 9.700 por azar) que G0-MODO no
explica: GCB2c empieza por E1, el build anterior a ese dia.

- [ ] GCB2c: cambiar UNA variable a la vez donde GCB2b encuentre diferencia,
      validar contra el criterio de G0 de `PLAN_LA_3060.md` y registrar cada
      arranque en `EL_0x15_ARRANQUES.csv`, salga bien o mal.

17-10, preparado en el anfitrion: E2 (`sin-d`), E3 (`riscv`) y E4
(`seguido`) se eligen con `gpu g0` sin recompilar y el `save` dice cual
corrio; E1 es desplegar `a2408875a`. Como se corre cada una:
[GCB2B_ENTRADAS, seccion 5](../../platform/drivers/gpu/ga10x/GCB2B_ENTRADAS.md).
Falta el metal: la casilla se cierra con los arranques, no con el codigo.

17-10, instrumento nuevo: tras un 0x15 la autopsia lee la traza de PC del
SEC2 (SCTL 0x240, TRACEIDX 0x148, TRACEPC 0x14c, como `gm200_flcn_tracepc`
de nouveau) y el `save` la escribe como `TRAZA del SEC2`. Comparar la traza
de un arranque bueno con la de uno malo dice DONDE se para el booter, aunque
el booter sea cerrado. Si el PRI no deja leerla, el informe lo dice.

Salida: causa acotada y correccion reproducible, o hipotesis debilitada con
resultado registrado. Un arranque bueno aislado no cierra fiabilidad. Las
pruebas fisicas se agrupan con usos normales; no se piden reinicios en serie.

### GCB3 -- palanca 4: escuchar al GSP en castellano

- [ ] GCB3a: decodificar cada evento que ya se nombra (`RC_TRIGGERED`,
      `MMU_FAULT_QUEUED`, `OS_ERROR_LOG`, `GSP_POST_NOCAT_RECORD`) a una frase
      con sus campos, en `save` y CABINA; el que no se entienda se dice crudo.
- [ ] GCB3b: decodificar los logs LIBOS (`LOGINIT`, `LOGINTR`, `LOGRM`) que
      hoy van crudos a `gsplog.bin`. Primero confirmar contra OpenRM 570.144
      donde viven sus cadenas de formato y leer el diccionario de la misma
      build si esta disponible, sin meter firmware al arbol.
      Avance 17-10: [paquete original e investigacion](../../platform/drivers/gpu/ga10x/GSP_570144_EVIDENCIA.md)
      confirman SHA-256 identico y diccionario ausente del binario publico.
      Lector puro de anillo/ELF, metadata normal/extendida, ocho pruebas y
      ejemplo anfitrion implementados; Director muestra el inventario real.
      Siguen faltando diccionario de la misma build, formatos completos y
      muestra con registros; no se marca cerrada esta casilla.
- [ ] GCB3c: decir en el informe, despues de un 0x15, si el GSP llego a
      escribir algo en `LOGINIT` o no. Un log vacio tambien es un dato.
      Avance 17-10: Director distingue reserva ausente de PUT cero y no
      equipara este ultimo con inactividad total. La copia actual contiene
      PUT cero y tablas de paginas iniciales, sin registros publicados;
      sigue faltando identificar/coherenciar la captura por intento.

Salida: el GSP explica sus fallos en la voz del sistema. Es el kernel como
LENTE aplicado a la tarjeta.

### GCB4 -- palancas 2 y 3: la constitucion

- [ ] GCB4a: una tabla de politica por palanca, en codigo: cada RPC que sale
      (de `contrato.rs`) con quien puede pedirlo, para que, con que presupuesto
      y que evidencia deja; cada concesion con medida, propietario y motivo.
- [ ] GCB4b: un guardian que compare esa tabla con `contrato.rs` y con los
      sitios que arman mensajes: un RPC sin fila, o una fila sin RPC, pone el
      build en rojo. Con su prueba negativa antes de anunciar que protege.

Salida: el gobierno deja de estar implicito. Leer la tabla basta para saber
que se le permite pedir al GSP y por que.

### GCB5 -- palanca 5: recuperar y despedir

- [ ] GCB5a: una politica escrita ante cada Xid observado: cuales se
      recuperan liberando el canal (`liberar.rs`), cuales piden despedir el
      GSP en orden y cuales solo un reinicio; con el diagnostico original
      conservado. Es tambien lo que necesita PROTON-X: el canal grafico de
      Cyberpunk murio por un Xid antes del primer lote.
- [ ] GCB5b: conectar el supervisor a esa politica (hoy solo recomienda):
      una recuperacion se ejecuta solo desde estados y condiciones demostrados,
      y el ejecutor sigue siendo el propietario del kernel.

### GCB6 -- medir el gobierno

- [ ] GCB6a: medir una mejora por vez (esperas del arranque, envios, copias,
      politica de energia admitida) con la misma carga, separando tiempo de
      CPU, de GPU, de espera y de presentacion. Sin prometer FPS.

## 5. Guardianes y por que

| Frontera | Guardian |
| --- | --- |
| El supervisor no toca el aparato | S8 de `la-3060` (ya activo): sin unsafe, sin dependencias, sin MMIO |
| La lente comun y la inspeccion sin autoridad | S6 y S7 de `la-3060` (ya activos) |
| Un solo propietario de cada efecto | El inventario GCB0a; ningun atajo del supervisor a reset, envio o descarga |
| La constitucion dice la verdad | GCB4b, nuevo |
| Arquitectura de BMO-X | `isa`: leer mensajes del GSP no abre otro kernel ni otro objetivo |

No se aflojan guardianes ni lineas base para que una implementacion pase.

## 6. Fuentes

- [NVIDIA OpenRM 570.144](https://github.com/NVIDIA/open-gpu-kernel-modules/tree/570.144):
  el protocolo de esa version; no mezclar versiones de firmware.
- [nova-core tu102.rs](https://github.com/torvalds/linux/blob/master/drivers/gpu/nova-core/gsp/hal/tu102.rs):
  otro gobernador del mismo GSP, como contraste del orden de arranque y descarga.
- Internas: [parte del metal](../metal/METAL_2026-10-16.md),
  [anatomia](../../platform/drivers/gpu/ga10x/ANATOMIA.md),
  [estudio del protocolo](../../platform/drivers/gpu/ga10x/ESTUDIO_RTX3060_PROTON_X.md),
  [lector NVIDIA](../../platform/drivers/gpu/ga10x/INSPECCION_NVIDIA.md),
  [aislamiento](PLAN_EL_AISLAMIENTO.md) y [plan GPU](PLAN_LA_3060.md).
