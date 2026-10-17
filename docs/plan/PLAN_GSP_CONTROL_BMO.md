# PLAN GSP-CONTROL-BMO -- administrar y conocer la RTX desde BMO-X

Pedido del propietario, 16-10-2026: un crate propio en carpeta `Nvidia`,
que crezca entendiendo el interior de SU RTX 3060 12G, con anatomia,
carriles, motivos y guardianes. GCB0 y GCB1 ya tienen contrato, modelo probado
y diagnostico nativo. El control de acciones aun conserva los propietarios
actuales; no hay firmware BMO ejecutandose en el GSP.

## 1. Objetivo y prioridad

Construir un supervisor NVIDIA que conozca cada vez mejor los recursos,
estados y protocolos que BMO-X puede observar y administrar. Cada capacidad
debe decir que sabe, de donde lo sabe y que sigue inaccesible. El primer
beneficio buscado es localizar G0/0x15 y hacer fiable el arranque.

Orden: evidencia y contrato -> supervisor probado en anfitrion -> prueba
controlada de arranque -> captura real correlacionada -> cobertura interior
progresiva -> optimizacion medida. El estudio de firmware puede avanzar
sobre copias mientras el GSP no arranca. No exige reinicios en serie.

La expansion tiene DOS resultados distintos:

| Alcance | Donde ejecuta | Resultado esperado |
| --- | --- | --- |
| Supervisor GSP-CONTROL-BMO | CPU x86-64 de BMO-X | Administrar nuestra secuencia, memoria concedida, mensajes, estados y recuperacion permitida. |
| Estudio del firmware y su ISA | Analisis de copias en la CPU | Interpretar regiones y funciones conocidas del GSP, con cobertura y desconocidos visibles. |
| Mini-kernel propio dentro del GSP | Hipotetico firmware del microcontrolador | Investigacion condicional de viabilidad; no se promete que esta RTX admita sustituir su firmware. |

El GSP administrativo y los SM que ejecutan SASS SM86 son piezas distintas.
Conocer RISC-V no proporciona por si solo los protocolos o registros de
NVIDIA, ni sustituye la ruta grafica y de computo de VERRANO/PROTON-X.

## 2. Punto de partida comprobado

| Evidencia | Limite actual |
| --- | --- |
| RTX GA106 A1, PCI 10DE:2504, 12288 MiB | Perfil de esta tarjeta; no generalizar a otra NVIDIA. |
| MUNDO: 325 fotogramas GPU prestados en una corrida | C1 observado; Q0a3 sigue abierto por cuatro pixeles distintos. |
| Sesion 23:23: LIBOS esperado 3C000000 intacto; CPUCTL/BCR 12/1 antes del booter | Los argumentos eran legibles en esa foto; no prueba toda su semantica. |
| Despues: booter 0x15, buzones BADF1002 y CPUCTL BADF5620 | El cambio queda entre dos mediciones; causa sin demostrar. |
| Lentes vistas: 64 B de shader y 32 B de plantilla | Copias CPU; no captura de VRAM ni del canal real. |
| Constructor y comprobacion de LIBOS, colas y permisos IOMMU existentes | Ya son control propio del lado anfitrion; evitar duplicarlos. |

Fuentes internas: [parte del metal](../metal/METAL_2026-10-16.md),
[anatomia](../../platform/drivers/gpu/ga10x/ANATOMIA.md),
[estudio del protocolo](../../platform/drivers/gpu/ga10x/ESTUDIO_RTX3060_PROTON_X.md)
y [lector NVIDIA](../../platform/drivers/gpu/ga10x/INSPECCION_NVIDIA.md).

## 3. Ubicacion y direccion de dependencias

La carpeta elegida por el propietario es
[`platform/drivers/gpu/Nvidia`](../../platform/drivers/gpu/Nvidia/README.md).
Crate implementado: `bmo-nvidia-gsp-control`, en
`platform/drivers/gpu/Nvidia/gsp-control-bmo/`. Director consume su diagnostico
por `gpu control` y `save`; la conexion de eventos completos al aparato sigue pendiente.

```text
VERRANO / apps -> ABI y resumen comun, sin fabricante ni acceso a MMIO
kernel        -> adaptador de autoridad NVIDIA -> GSP-CONTROL-BMO
supervisor    -> contratos y observaciones; produce una decision explicita
GA10x actual  -> interpreta campos y construye los protocolos existentes
lente comun   -> conserva bytes y procedencia, sin saber de ninguna ISA
```

El crate propuesto sera puro, no_std y sin unsafe: una maquina de estados
y contratos, ejecutada en el anfitrion y en la CPU de BMO-X. No enlazara el
kernel ni recibira punteros MMIO. El adaptador ROJO del kernel ejecutara
solo las acciones admitidas, con la autoridad existente y sin una segunda
ruta para controlar la tarjeta. La ISA del binario de BMO-X sigue x86-64.

El GA10x, el codificador SM86 y su emisor existentes permanecen separados
por responsabilidad. Antes de moverlos se inventarian consumidores y
guardianes; un traslado sin cambio funcional requiere la prueba L6d.
No se crea un segundo fabricante que tenga que pasar por el supervisor NVIDIA.

## 4. Anatomia con cobertura, no una promesa de leerlo todo

El catalogo debe distinguir el mapa conocido de lo realmente leido.

| Dominio | Pregunta que responde | Primer modo de estudio |
| --- | --- | --- |
| Identidad, PCI, BAR y VBIOS | Que tarjeta y version tenemos | Datos existentes y copia estable de ROM. |
| Arranque: FWSEC, SEC2, GSP, WPR | En que etapa cambia el estado | Fotografias permitidas y protocolo versionado. |
| LIBOS, RMARGS, logs y colas | Que se concede y que contesta el firmware | Formatos, limites, permisos y mensajes observados. |
| VRAM, tablas, IOVA y VA | Quien puede ver cada rango | Mapa de recursos, propiedad y traducciones concedidas. |
| Canales, GR, copia y computo | Que trabajo fue preparado y cual termino | Paquetes, identificadores y vallas correlacionadas. |
| SASS, SPH, texturas y destinos | Como se representa un dibujo concreto | Copias y decodificadores NVIDIA; contraste independiente. |
| Energia, relojes y sensores | Que solicitud se hizo y que estado se midio | Consultas documentadas, validez y respuesta; medir bajo carga comparable. |
| Firmware GSP y su ISA | Que regiones e instrucciones se pueden interpretar | Analisis fuera de linea; datos desconocidos conservados. |

Cada entrada tendra: tarjeta/version aplicable, fuente y revision, espacio
de direccion, rango y unidad, propietario, condicion de acceso, efectos
conocidos de lectura, valor crudo, interpretacion, momento y validez.
Estados de conocimiento: documentado, observado, inferido, desconocido,
inaccesible y no aplicable. Un valor BADF no se convierte en un dato del
registro; un rango no concedido no se muestra como ceros.

Un volcado completo solo significa completo para el recurso/rango declarado.
No se anuncia cobertura de toda la GPU ni se barre MMIO para descubrirlo:
primero se conoce la semantica de la lectura. Una lectura puede tener efectos.

Aprender progresivamente significa incorporar una pregunta, fuente, captura,
lector, prueba negativa y contraste independiente antes de ampliar el
catalogo. No significa que el supervisor se conceda permisos nuevos o cambie
su codigo por lo que encuentre en un volcado. Las ampliaciones se revisan
y versionan; un formato desconocido conserva su estado hasta comprobarlo.

## 5. Etapas con criterio de llegada

### GCB0 -- especificar antes de controlar

- [x] GCB0a: inventariar propietarios actuales de LIBOS, arranque, canales,
      memoria y recuperacion; declarar quien es el unico que ejecuta cada accion.
- [x] GCB0b: escribir contrato de observacion/decision con identificacion de
      tarjeta, firmware, build e intento; conservar desconocido y dato ausente.

Salida: mapa de dependencias y contratos revisables, sin escrituras nuevas.
El numero de build enlaza con OBS-BUILD de EL_FOCO, no lo cuenta como cumplido.

### GCB1 -- supervisor chico y comprobable

- [x] GCB1a: crear el crate propuesto con estados SinPreparar, Preparado,
      Arrancando, Operativo, Fallido y RecuperacionBloqueada; transiciones explicitas.
- [x] GCB1b: probar eventos fuera de orden, duplicados, observaciones antiguas,
      tiempos agotados y recuperacion no permitida; ningun fallo se anuncia operativo.

Salida: decisiones puras (esperar, siguiente paso permitido, conservar fallo),
sin ejecutar el efecto. Cada decision lleva motivo y evidencia requerida.
Los plazos proceden del protocolo existente y su fuente; no se alargan para
ocultar un fallo. El adaptador conserva propiedad, cerrojos y coste real.

Implementado el 16-10-2026: [contrato e inventario](../../platform/drivers/gpu/Nvidia/gsp-control-bmo/README.md),
crate puro y consulta `gpu control` incluida en `save`. El consumidor nativo
usa diagnostico de foto; el modelo con historia exige identidad completa.
OBS-BUILD e intento no estan disponibles: no se fabrican ni se cierra OBS-BUILD.
Recuperacion y alimentacion de eventos desde metal siguen pendientes.

### GCB2 -- resolver el protocolo de arranque, prioridad G0

- [ ] GCB2a: contrastar la preparacion inicial completa con OpenRM 570.144 y
      la ruta nova usada como antecedente; comparar LIBOS/WPR, nucleo y esperas.
- [ ] GCB2b: preparar una variante coherente y reversible para G0-MODO,
      probar su secuencia en banco y definir evidencia de cada etapa en metal.
- [ ] GCB2c: validar el cambio candidato contra el criterio de G0 del plan
      de GPU, conservando resultados buenos y malos y regresiones de dibujo.

Salida: causa acotada y correccion reproducible, o hipotesis debilitada con
resultado registrado. No basta cambiar resetear por resetear_en_riscv:
el arranque vacio y la espera de Falcon parado dependen del protocolo actual.
Un arranque bueno aislado no cierra fiabilidad. Las pruebas fisicas se
agrupan con usos normales; no se pide repetir siete o diez reinicios.

### GCB3 -- del dato preparado al trabajo observado

- [ ] GCB3a: catalogar lecturas permitidas y producir capturas estables con
      build, intento, recurso, generacion, instante y resultado de acceso.
- [ ] GCB3b: correlacionar preparado, enviado, finalizado y copia recuperada
      de un trabajo acotado; comprobar coherencia y vida del recurso antes de compararlo.
- [ ] GCB3c: publicar un resumen neutro para VERRANO/save; los bytes y campos
      NVIDIA quedan en su adaptador y una captura parcial conserva ese estado.

Salida: primer trabajo real explicable de extremo a extremo. Una valla
terminada no basta para demostrar igualdad de imagen; Q0a3 conserva su juez.
No leer un buffer mientras el dispositivo lo cambia ni despues de devolverlo.

### GCB4 -- aprender el interior y el RISC-V concreto del GSP

- [ ] GCB4a: identificar artefactos de firmware, version, hashes, contenedores,
      segmentos de codigo/datos y direccion de carga conocida; mantener desconocidos.
- [ ] GCB4b: determinar anchura, extensiones y particularidades respaldadas por
      fuentes o binarios del GSP; no deducir todo el perfil de una etiqueta RISC-V.
- [ ] GCB4c: ampliar un lector fuera de linea, instruccion/familia por vez,
      con vectores independientes y rechazo de instrucciones cortadas o no soportadas.
- [ ] GCB4d: registrar funciones y acceso a recursos solo donde haya evidencia;
      separar flujo estatico posible de una traza realmente observada.

Salida: informe de cobertura por version e instrucciones, con bytes sin
interpretar visibles. Se comienza con el subconjunto necesario para una
pregunta concreta de G0 o de mensajes; no con un emulador universal.
La especificacion estandar no describe por si sola los perifericos NVIDIA.
La longitud de instruccion se determina segun el perfil comprobado: no se
asumen palabras fijas de 32 bits si hay instrucciones comprimidas. CSR,
privilegios o instrucciones particulares requieren evidencia propia;
desconocido no autoriza inventar la longitud ni convertir datos en codigo.
No se presupone acceso de depuracion, lectura de memoria protegida o una
traza de ejecucion interna. Un modelo simulado no acredita esos accesos.

El lector procesa datos de un coprocesador, ejecutandose como programa
x86-64. Se debera comprobar el alcance del guardian isa y conservar sus
rechazos a targets, cfg y emisores para otra CPU. No abrir un backend de
kernel RISC-V para estudiar un fichero ni crear rutas prohibidas por nombre.
Si hace falta ajustar el clasificador para datos de dispositivo, sera una
regla explicita con pruebas negativas, sin rebajar la arquitectura del repo.

### GCB5 -- administracion y rendimiento medidos

- [ ] GCB5a: integrar recuperacion solo desde estados y condiciones demostrados;
      probar timeout, fallo de descargador y repeticion sin perder el diagnostico original.
- [ ] GCB5b: medir una mejora por vez (esperas, envios, copias, presion de registros
      o politica de energia admitida), con misma carga y correccion de resultado.

Salida: menos trabajo o menor tiempo medido, con intervalo, backend, estado
y condiciones registrados. No se promete un porcentaje de FPS ni que dominar
el GSP habilite todas las funciones graficas. Mantener separado el tiempo de
CPU, de GPU, de espera y de presentacion.

## 6. Horizonte: mini-kernel BMO dentro del GSP

Esta rama es INVESTIGACION CONDICIONAL, no trabajo de implementacion inmediata.
Primero debe responder: que imagen acepta el arranque de esta revision,
que se puede cargar/ejecutar, con que privilegios y memoria, y que parte del
contrato con RM y los motores habria que conservar. Registrar fuentes,
dependencias y resultado reproducible antes de trazar un kernel interno.

| Resultado de investigacion | Continuacion |
| --- | --- |
| No se demuestra una ruta compatible para firmware propio | Conservar el supervisor y el analisis; documentar el limite. Es un resultado valido. |
| Se demuestra una via y sus condiciones en este hardware | Plan separado para programa minimo interno, evidencia de arranque y retorno conocido antes de administrar recursos. |
| Solo se ejecuta un shader SM86 | Clasificarlo como computo en SM; no como kernel RISC-V del GSP. |

Controlar entradas, memoria concedida y mensajes no equivale a poseer el
codigo o los mecanismos de autenticacion internos del firmware. La viabilidad
no se deduce de ser propietario de la tarjeta. No se afirma que el 0x15
sea un rechazo de firma, electrico, de LIBOS o de nucleo sin evidencia.

Un port de BMO-X para una CPU principal RISC-V sigue siendo otro proyecto;
este plan conserva el kernel x86-64 y la frontera del dispositivo.

## 7. Guardianes previstos y por que

| Frontera | Guardian y prueba exigidos |
| --- | --- |
| API y lente comun sin fabricante | Conservar S2/S6 y sus fixtures negativos AMD/NVIDIA y dependencias escondidas. |
| Inspeccion sin aparato | Conservar S7: interpretar paquetes no concede MMIO, firmware ni ejecucion. |
| Nuevo supervisor puro | Regla nueva: sin unsafe, kernel, registro o acceso volatile; probar rechazo de una fuga. |
| Un solo propietario de efectos | Inventario y chequeo de rutas para reset, prestamo, envio y descarga; evitar un atajo del supervisor. |
| Observaciones versionadas | Rechazar mezcla de intentos, generaciones, firmware y recursos; ausente nunca significa correcto. |
| Arquitectura de BMO-X | Conservar isa: target y binarios del sistema x86-64; analisis de copias no es otro kernel. |
| Medida de exito | Jueces de estado, bytes y pixeles separados; no sustituirlos por un contador de comandos preparados. |

Cada fase trae su prueba negativa antes de anunciar que la regla protege.
Se ejecutan las pruebas que corresponden al cambio, kernel/DIRECTOR cuando
se enlacen piezas nuevas y pila cuando cambie su camino. No se aflojan
guardianes o baselines para que una implementacion pase.

## 8. Potencial y dificultad que hay que resolver

| Potencial | Que lo habilita | Lo que aun puede impedirlo |
| --- | --- | --- |
| Arranque explicable y mas fiable | Estado y protocolo correctos | Firmware y transiciones aun no comprendidos. |
| Menos reinicios de diagnostico | Capturas utiles y recuperacion demostrada | Un fallo puede dejar WPR protegida sin recuperacion conocida. |
| Mejor autopsia de un dibujo | Identidad y capturas correlacionadas | Memoria mutable, acceso restringido o falta de sincronizacion. |
| Mayor cobertura de la anatomia | Catalogo versionado y lectores independientes | Regiones inaccesibles y extensiones sin documentar. |
| Mejor rendimiento | Cuello de botella medido y cambio comprobado | GSP operativo no resuelve automaticamente shaders o costes del juego. |
| Firmware BMO interno | Viabilidad especifica de arranque demostrada | No hay actualmente esa demostracion. |

La medida de avance sera: dominios catalogados, registros con semantica de
lectura conocida, bytes interpretados frente a bytes capturados, transiciones
probadas y trabajos correlacionados. Cada porcentaje debera declarar su
denominador; no se inventa un porcentaje de conocimiento de toda la GPU.

## 9. Fuentes de estudio

- [NVIDIA OpenRM 570.144](https://github.com/NVIDIA/open-gpu-kernel-modules/tree/570.144):
  protocolo y estructuras de esa version. Su README exige componentes de
  firmware correspondientes; no mezclar versiones como experimento incidental.
- [Manual RISC-V 20260120](https://docs.riscv.org/reference/isa/v20260120/unpriv/unpriv-index.html):
  base y extensiones estandar, no evidencia del perfil implementado por este GSP.
- [Fuentes y pruebas de Mesa/NAK](https://docs.mesa3d.org/drivers/nvk/external_hardware_docs.html):
  distinguir codificacion contrastada de comportamiento comprobado en hardware.
- [Informe NAK fijado a commit](../metal/mesa_nak/INFORME_NAK.md):
  referencia SASS; no es especificacion del firmware RISC-V.
- [Aislamiento existente](PLAN_EL_AISLAMIENTO.md) y
  [plan GPU](PLAN_LA_3060.md): conservar sus contratos y evidencia.

Fuentes externas consultadas el 16-10-2026. La rama de mini-kernel interno
permanece sin ruta de ejecucion demostrada; las etapas anteriores ya tienen
valor sin depender de que esa rama resulte viable.
