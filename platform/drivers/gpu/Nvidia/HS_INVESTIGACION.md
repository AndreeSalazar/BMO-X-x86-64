# HS en la RTX 3060 12G -- poner la cadena segura al servicio de BMO

## Ampliacion vigente -- investigacion GA106, 21-10-2026

Pedido del propietario: inspirarse en las investigaciones publicas para
obtener mas control de SU GPU. Se incorpora HSG1..HSG5 en el
[plan GPU vigente](../../../../docs/plan/PLAN_GPU_COMO_LINUX.md#4e-investigacion-hs-para-ga106-21-10).
Este bloque actualiza el orden del 18-10: el arranque firmado G0/F2b sigue
siendo la via de entrega; el estudio de otras rutas HS tiene resultados y
criterios propios. La ausencia de una ruta alternativa no bloquea G0.

### Base disponible, sin mezclar conclusiones

- [REGISTRO] GA106 A1, 10DE:2504, subsistema 1462:397D, 12 GiB. Firmware
  emparejado 615.78.08. Booters/bootloader tienen huellas fijadas por el
  build; las capturas buenas y malas existentes conservan su identidad.
- [REGISTRO] cd4018237 corrige el rechazo agregado a DMEM SEC de FWSEC;
  479 pruebas y paquete completo compilados. Pendiente metal de esa
  correccion. No sumar el fallo previo de 43DB9E231 a los retornos 0x15.
- [CITADO] La documentacion general de NVIDIA describe HS autenticado y
  estado interno restringido. Es el modelo documentado, no una prueba de
  que todo firmware este libre de defectos.
- [CITADO, no reproducido por BMO] cmpunlocker documenta una ruta sobre
  CMP/GA100 y reaplica sus cambios al inicializar el driver. Eso no acredita
  compatibilidad GA106 ni persistencia fisica tras quitar alimentacion.
- [CITADO, no reproducido por BMO] GPU- documenta experimentos GA104 y
  afirmaciones iniciales de control de ejecucion; sus conclusiones posteriores
  describen limites de autenticacion y flujo que cerraron la ruta de boot
  que investigaban. Una primera demostracion no acredita el resultado final.
- [SIN DATO] Una ruta HS propia, una apertura PLM util o un cambio permanente
  que funcione en esta GA106 con estos binarios 615. No inferirlo de GA100,
  de una emulacion o de compartir el nombre Ampere.

Fuentes consultadas el 21-10 (ramas mutables; HSG1 exige fijar revision
antes de convertirlas en codigo o experimento):
[modelo NVIDIA](https://nvidia.github.io/open-gpu-doc/Falcon-Security/Falcon-Security.html),
[arquitectura de cmpunlocker](https://github.com/bayley/cmpunlocker/blob/master/docs/ARCHITECTURE.md),
[historial y conclusiones GA104](https://github.com/Cheurteenyt/GPU-/blob/main/README.md).

### Que se transfiere de esas investigaciones

El metodo: identificar el binario exacto, modelar efectos de DMA junto con
las instrucciones, conservar los resultados negativos y comprobar una
prediccion en el hardware correspondiente. Las direcciones, cargas y permisos
de otra tarjeta no constituyen un contrato para GA106. El primer producto
de HSG1/HSG2 es una matriz de compatibilidad que pueda concluir SIN DATO o
NO APLICABLE, con motivo y evidencia.

El modelo anfitrion debe declarar su cobertura: instrucciones realmente
interpretadas, perifericos modelados, DMA, memoria inicial, limites y salidas
desconocidas. Una comprobacion criptografica simulada no demuestra que BROM
acepte una imagen en metal. QEMU sin una RTX asignada tampoco mide esa ROM.

### Que significa entrar y que significa permanecer

| Resultado | Evidencia necesaria en GA106 | Lo que todavia no demuestra |
| --- | --- | --- |
| El firmware firmado entro en HS | Resultado del arranque identificado y postcondicion de esa etapa | Ejecucion arbitraria propia o registro accesible desde CPU. |
| Una operacion privilegiada propia se completo | Efecto acotado previamente definido, lectura independiente, control negativo y fin del ensayo | Apertura de todos los permisos o runtime funcional. |
| El anfitrion obtuvo una capacidad adicional | Acceso concreto comprobado antes/despues, mascara y propietario identificados | Que toda la GPU quede abierta o que se pueda dibujar. |
| El resultado dura la sesion | Estado observado antes/despues de uso y despedida | Supervivencia a reset, recarga de driver o corte electrico. |
| Se reaplica al arrancar | Nuevo intento y nueva ejecucion registrados | Cambio permanente del hardware. |
| Es persistente fisicamente | Estado posterior al corte sin reaplicar, con condiciones documentadas | Hoy no hay ese resultado para esta tarjeta. |

HSG4 solo tiene sentido si HSG2 identifica una precondicion concreta de
esta tarjeta y HSG3 permite comprobarla. El prototipo se delimita a una
operacion reversible y observable, con sus postcondiciones; no basta una
parada, un buzon nuevo o un bucle HS. Debe terminar y permitir contrastar
el arranque normal. La configuracion experimental no se convierte en el
camino por defecto por un resultado aislado.

HSG5 necesita tambien el resultado del GSP_INIT 615 y una operacion GPU
identificada antes de hablar de integracion util. Ejecucion HS, permisos,
protocolo del runtime y Cyberpunk son hitos diferentes. Este trabajo no
requiere reemplazar la arquitectura de capacidades/IOMMU de BMO.

## Orden vigente -- redefinicion completa, 18-10-2026

El propietario pide que Heavy Secure juegue a favor de BMO-X para tener
control de su RTX 3060 12G y eliminar el 0x15. Este bloque sustituye el orden
HS1..HS6 conservado abajo como antecedente. No afirma que el fallo este
corregido ni que ya se pueda ejecutar firmware HS propio.

**Objetivo:** BMO prepara y comprueba el contrato de entrada; FWSEC y el
booter firmados ejecutan las operaciones privilegiadas; BMO comprueba la
salida, toma el gobierno del GSP por RPC y conserva cada fallo explicable.
El primer resultado exigido es G0, no un desensamblado completo del firmware.

### La autoridad, por pieza

| Pieza | Que hace a favor de BMO | Que controla BMO | Evidencia que falta |
| --- | --- | --- | --- |
| BROM | Autentica la entrada HS | Imagen entregada y parametros de carga documentados | Separar resultado del ejecutor de resultado del booter por intento |
| FWSEC en GSP | Preparacion protegida FRTS/WPR | Peticion admitida, datos y secuencia | Postcondiciones observadas y validez, sin suponer que toda WPR correcta implica toda firma correcta |
| Booter HS en SEC2 | Carga/autentica el runtime | Firma seleccionada, tramos, DMA, metadata y buzones de entrada | Resultado de ejecucion HS separado de MAILBOX0 y transicion a runtime |
| GSP-RM | Gestion interna de la tarjeta | Recursos concedidos y RPC admitidos | GSP_INIT en 615 o INIT_DONE en 570, respuesta y primer trabajo finalizado |
| SM86 | Computo y dibujo de las apps | Programa juzgado, recursos y plazo | Validacion de cada cliente; un shader no concede autoridad HS |

[CITADO, revisado 18-10] El modelo general de NVIDIA exige ucode firmado
para HS y limita el acceso externo a su estado interno. Nova documenta la
cadena Ampere BROM -> booter HS SEC2 -> GSP-RM LS y la preparacion FRTS
privilegiada. Estas referencias describen el mecanismo; no prueban cada
estado de esta GA106. Fuentes: [NVIDIA Falcon Security](https://nvidia.github.io/open-gpu-doc/Falcon-Security/Falcon-Security.html),
[Nova Falcon](https://www.kernel.org/doc/html/latest/gpu/nova/core/falcon.html).

### El 0x15 se investiga por contrato y por fase

[CITADO] [OpenRM 570.144, s_executeBooterUcode_TU102](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/8ec351aeb96a93a4bb69ccc12a542bf8a8df2b6f/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_booter_tu102.c)
comprueba primero el estado devuelto por el ejecutor HS y despues el buzon
del booter; un buzon no cero se convierte en NV_ERR_GENERIC. Por tanto,
guardar ambos resultados por separado es una pregunta util. No se traduce
0x15 como error de firma, Xid ni NV_STATUS por coincidencia numerica.

[REGISTRO] Hoy BMO ya selecciona firma por fusible, carga tramos seguros,
programa BROM y arranca SEC2. Los propietarios son `arranque/booter.rs`,
`arranque/falcon.rs` y `ring0/dev/gpu_despertar.rs`. `fa::brom` escribe los
parametros; su llamada por si sola no acredita autenticacion completada.
Las capturas v1 tienen cinco fases, no una traza interna de HS.

### Trabajo nuevo, con salida y rechazo comprobables

Avance 18-10: [Windows 610.74 frente a BMO](../ga10x/ESTUDIO_WINDOWS_61074.md).
Referencia instalada identificada y copiada con huellas; NVML no acredita
GSP activo en Windows. Primer contrato del fichero booter BMO extraido con
`contrato_hs`; faltan datos del intento, memoria/vida y la secuencia Windows.
HSA1/HSA2 permanecen abiertos: este estudio no cierra G0.

- [ ] HSA1 -- Contrato de entrega HS de ESTA GA106. Reunir en una tabla
      imagen/release/SHA-256, fusible crudo e indice de firma, engine mask,
      ucode id, tramos IMEM/DMEM, punto de firma, arranque, metadata, IOVA,
      paginas y vida de cada prestamo. Fuente: lectores existentes y sitios
      que ejecutan el protocolo, con revision. No duplicar parser.
      Llegada: cada campo tiene propietario, unidad, rango y momento de uso;
      los casos de offset, indice y medida invalidos se rechazan en host.
      Falta de dato se escribe ausente; una huella no certifica firma NVIDIA.
- [ ] HSA2 -- Separar las fronteras de fallo. Auditar que observacion publica
      permite distinguir preparacion/DMA, ejecutor BROM-HS, salida del booter,
      traspaso al GSP e INIT_DONE. Primero verificar registros, permisos de
      lectura y significado en fuentes aplicables a GA106; despues decidir
      si basta captura v1 o hace falta una version nueva. Cada resultado
      conserva crudo, validez, intento y fase; si no es observable, desconocido.
      Llegada: pruebas negativas impiden que timeout, PRI, dato ausente o
      MAILBOX0=0 sin runtime se anuncien GSP operativo. Sin inferir una PC
      interna de HS a partir de duraciones o registros inaccesibles.
- [ ] HSA3 -- Comparar una entrega buena y una fallida del mismo build,
      firmware, modo G0 e instrumental. Usar CAPTURAS y su manifiesto:
      sesiones separadas, huellas, fase, reservas y vida de memoria. Revisar
      orden DMA/barreras/accesibilidad con el contrato HSA1; metadata igual
      o cero eventos IOMMU no bastan para absolver toda la entrega.
      Llegada: primera divergencia OBSERVABLE con alcance escrito, o
      comparacion inconclusa con el dato que falta. No fabricar el par.
- [ ] HSA4 -- Una correccion causal del anfitrion. Elegir una divergencia
      de HSA3 o un incumplimiento documentado de HSA1, escribir prediccion,
      cambio minimo, forma de comprobar que la variable cambio y resultado
      que refutaria la hipotesis. Mantener constantes firmware e instrumental
      si no son la variable. Llegada: la correccion elimina el incumplimiento
      y cumple G0; un arranque bueno o cambiar el codigo de error no bastan.
- [ ] HSA5 -- Validar control recuperado. Diez arranques consecutivos de
      validacion con todos los intentos conservados, procedencia Windows/BMO
      y tipo de apagado registrados, booter correcto e inicializacion segun
      version (GSP_INIT en 615, INIT_DONE en 570). No mezclar
      builds ni reciclar los dos buenos historicos; no forzar reinicios en
      serie. Diez de diez es el criterio operativo de G0, no garantia
      estadistica de fallo imposible. Despues: RPC, primer trabajo GPU y
      apagado ordenado comprobados; canal recuperado no equivale a GSP arrancado.
- [ ] HSA6 -- Gobierno util despues de G0. Conectar las politicas GCB4/GCB5:
      concesiones, eventos y recuperacion con precondiciones demostradas.
      Probar Q0a3 y el cliente MUNDO; conectar LB8 de TITAN++ sin confundir
      puerta de computo implementada con uso del compilador o prueba metal.
      Llegada: carga identificada con resultado, backend y fallo explicable.

Orden: HSA1/HSA2 se pueden estudiar con las fuentes disponibles; HSA3 usa
los conjuntos reales que existan; HSA4 depende de una prediccion comprobable;
HSA5 valida la correccion; HSA6 aprovecha la tarjeta. La siguiente accion
es HSA1 y la auditoria HSA2, no probar otro reset por intuicion.

### Relacion con los otros planes y la investigacion propia

G0 sigue en PLAN_LA_3060; GCB2c ejecuta HSA1..HSA5, sin otro banco de
arranques. GCB0/GCB1 y las capturas ya hechas se conservan. GCB3 escucha;
GCB4/GCB5 gobiernan; Q0a3/MUNDO y LB8 son clientes posteriores. El indice
general referencia este trabajo desde GSP-CONTROL-BMO.

HS0 original sigue hecho solo en anfitrion. HS1/HS2 originales alimentan
HSA1; HS3 alimenta HSA2..HSA5; HS4 alimenta HSA6/GCB3. HS5/HS6 originales
conservan la investigacion de firmware propio como via separada: requieren
una ruta de carga demostrada para GA106, funciones a sustituir y prueba
temporal verificable. No bloquean arreglar la entrega al HS firmado actual.
El estudio estatico del booter intacto ya descarta tratar su desensamblado
bruto como una rama 0x15 identificada. TRACEPC a cero ya se observo en bueno
y malo. E2/E3/E5/E6 no dieron una correccion suficiente; E4 inicial no cambio
su variable. No repetirlos como descubrimientos pendientes.

No se ejecuta firmware ni se cambia el protocolo por editar este plan.
El significado interno de 0x15 sigue SIN DATO. Las fuentes mutables de Nova
se revisaron el 18-10; antes de convertirlas en un parche se fijara tambien
su revision de codigo. El wrapper OpenRM se referencia por commit.

## Antecedente del 17-10 -- alcance y preguntas de la investigacion original

Pedido del propietario, 17-10-2026: estudiar BROM, HS y firmware para llegar a
control propio y diagnosticos explicables al ejecutar juegos AAA en BMO-X.
La preparacion inicial es sobre copias en el anfitrion. No cambia firmware,
reset, permisos MMIO, IOMMU, guardianes ni la prioridad G0 del sistema.

## 1. La pregunta y la tarjeta

Que pieza impide observar o controlar una operacion necesaria, que interfaz
ofrece hoy, y que haria falta implementar para sustituir esa pieza?

La [anatomia registrada](../ga10x/ANATOMIA.md) identifica GA106 A1,
PCI `10DE:2504`, subsistema `1462:397D`, 12288 MiB. Son datos del proyecto,
no una nueva medicion de esta entrega. GA100, Tegra y GA106 no se sustituyen
entre si por compartir el nombre NVIDIA o la familia Ampere.

HS es un modo de seguridad Falcon. No es una particion que se formatea.
El booter, FWSEC y GSP-RM son codigo; BROM es la raiz de confianza interna;
VBIOS es otra pieza, almacenada en flash en la tarjeta. Cambiar VBIOS no
equivale a cambiar BROM ni elimina automaticamente la autenticacion.

## 2. Fuentes y lo que permiten afirmar

Etiquetas: **[CITADO]** documentacion/codigo identificado; **[REGISTRO]** parte
del proyecto, no reproducido por este documento; **[HIPOTESIS]** por comprobar;
**[SIN DATO]** falta evidencia. Esta es investigacion, no una regla nueva de
silicio ni un eje de rendimiento anunciado sin metro (L0/L2 de META-KERNEL).

| Pieza | Lo conocido | Limite |
| --- | --- | --- |
| BROM | [CITADO] La documentacion describe ROM inmutable y verificacion de ucode | No se dispone de un volcado ni mapa completo de la BROM GA106 |
| HS | [CITADO] Ejecuta ucode autorizado; restringe observacion desde el host | No hay acceso propio privilegiado demostrado en esta tarjeta |
| FWSEC | [CITADO] Preparacion segura, incluida FRTS/WPR2 | No confundirlo con el runtime GSP-RM |
| Booter SEC2 | [CITADO] Participa en autenticar/cargar GSP-RM | Su wrapper publico no explica cada codigo interno |
| GSP-RM | [CITADO] Inicializacion y gestion, con RPC hacia el host | Reemplazarlo requiere servicios, no solo permiso de ejecutar |
| Shader SM86 | [CITADO] Codigo grafico/computo distinto de ucode Falcon | Ejecutar un shader no acredita control HS |

Referencias primarias consultadas el 17-10-2026:

- [Falcon Security, NVIDIA](https://nvidia.github.io/open-gpu-doc/Falcon-Security/Falcon-Security.html): modelo general publicado en 2014, no manual completo GA106.
- [Falcon, nova-core](https://www.kernel.org/doc/html/latest/gpu/nova/core/falcon.html): cadena BROM/HS/LS y ejemplo Ampere; pagina mutable.
- [FWSEC, DRM](https://dri.freedesktop.org/docs/drm/gpu/nova/core/fwsec.html): preparacion segura; pagina mutable.
- [OpenRM 570.144](https://github.com/NVIDIA/open-gpu-kernel-modules/tree/8ec351aeb96a93a4bb69ccc12a542bf8a8df2b6f): commit de la referencia usada, no `main` de otra release.
- [GSP 570.144](https://download.nvidia.com/XFree86/Linux-x86_64/570.144/README/gsp.html): funcion de GSP y ruta de controlador que permite desactivarlo; no es una opcion ya implementada en BMO-X.
- [Binary Utilities](https://docs.nvidia.com/cuda/cuda-binary-utilities/index.html): herramientas para codigo de GPU/CUDA, no un desensamblador universal de Falcon.
- [Xid](https://docs.nvidia.com/deploy/xid-errors/archive/index.html): categorias de fallo; no traducen MAILBOX0 del booter.

El [estudio existente](../ga10x/GSP_570144_EVIDENCIA.md) conserva referencias
de implementacion, firmas, logs y preguntas abiertas. No se duplica su lector
ELF/metadata ni se inventa un significado de `0x15`.

## 3. Lo observado y lo que sigue abierto

[REGISTRO] [GCB2B_ENTRADAS](../ga10x/GCB2B_ENTRADAS.md) contiene los ensayos:
E2 sin D: fallo; E3 RISC-V fallo; E4 inicial no redujo el hueco pretendido;
E5 reset tuvo resultados mixtos; E6 PTIMER dio un fallo comunicado sin informe.
La traza SEC2 dio ceros en un bueno y un malo. No volver a listar E3 o esa
traza como una prueba todavia no intentada. No se declara cerrada G0.

[CITADO] `s_executeBooterUcode_TU102` de OpenRM 570.144 distingue el estado
del ejecutor HS del MAILBOX0 de salida. Un buzon no cero se convierte en
`NV_ERR_GENERIC`. No se encontro una traduccion especifica de `0x15` ahi.
El [parte vivo](../ga10x/EL_0x15_ARRANQUES.csv) conserva las sesiones.

[SIN DATO] La causa interna. WPR2 extendida indica progreso, no prueba
todas las firmas ni cada byte consumido. Metadatos iguales no prueban
coherencia DMA. Los aproximadamente 0,7 ms entre una duracion buena y las
malas no localizan por si solos una instruccion o chequeo.

## 4. Etapas, cada una con resultado revisable

El [laboratorio y sus recetas](../../../../toolchain/tools/nvidia-hs/LABORATORIO.md)
prepara registro de sesiones, comparacion y regiones RISC-V con Capstone.
Separa la base anfitrion del futuro adaptador TITAN++ y de las pruebas GA106.

- [x] HS0: inventariar copias por papel, medida y SHA-256, con identidad
      declarada y ausencias explicitas; comparacion de conjuntos y pruebas.
      Herramienta: [nvidia-hs](../../../../toolchain/tools/nvidia-hs/README.md).
      Cerrado solo en anfitrion con entradas sinteticas; no se ha leido BROM.
- [ ] HS1: inventariar los binarios reales del propietario y relacionarlos
      con partes identificados por build/intento, sin subir binarios al repo.
      Resultado: conjunto de evidencia con procedencia y huecos escritos.
- [ ] HS2: mapear cada imagen al procesador, modo y fase que le corresponde;
      identificar formatos y regiones interpretables con los lectores existentes.
      Resultado: tabla con offsets y origen de cada interpretacion; lo cifrado
      o sin formato conocido se conserva como desconocido.
- [ ] HS3: auditar la entrega del arranque G0: orden, DMA, coherencia, vida de
      memoria y transiciones. Reutilizar los partes disponibles antes de pedir
      mas pruebas; variar un factor y registrar tambien los fallos.
      Resultado: divergencia reproducible o hipotesis debilitada, no causa supuesta.
- [ ] HS4: conectar canal/envio/shader/recursos con RC_TRIGGERED, OS_ERROR_LOG
      y fallos MMU hasta `save`/CABINA; auditar primero `ga10x/src/rm/rc.rs`.
      Resultado: un fallo minimo deja informe crudo e interpretado; los datos
      ausentes no se sustituyen por cero ni por una frase inventada.
- [ ] HS5: evaluar rutas de control propio especificamente para GA106:
      booter, runtime GSP o mas gestion desde CPU. Cada ruta debe enumerar
      permisos necesarios, funciones a implementar y evidencia de viabilidad.
      Resultado: decision tecnica separada por pieza, con limites demostrados.
- [ ] HS6: cuando exista una ruta demostrada, preparar un prototipo temporal
      con diagnostico explicito, aislamiento y postcondiciones de recuperacion.
      Resultado: prueba identificada en esta tarjeta; no se promete permanencia
      ni compatibilidad AAA por obtener ejecucion privilegiada.

La [herramienta de inventario](../../../../toolchain/tools/nvidia-hs/README.md)
solo fija la entrada de HS1. No acredita estado de la GPU, firma NVIDIA ni
posibilidad de reemplazo. GSP-CONTROL-BMO conserva su autoridad y su modelo.

## 5. Como evaluar una pista de control privilegiado

Hay investigacion publicada sobre TSEC de Tegra y proyectos GA100. Son
antecedentes, no una solucion reproducida en GA106. Para cada pista guardar:

1. Fuente original y revision del codigo/documento.
2. Chip, revision, firmware, firma y precondiciones del resultado.
3. Que control dice obtener: diagnostico, ejecucion, permisos o persistencia.
4. Evidencia publicada y diferencias pendientes frente a esta RTX.
5. Una prueba local acotada que podria confirmar aplicabilidad, si procede.

Pistas iniciales, sin reproducir sus resultados ni adoptar sus afirmaciones
como hechos de BMO-X:

- [Investigacion TSEC](https://vbe0201.github.io/posts/how-not-to-provide-secure-access-to-a-secret/): chip y cadena distintos.
- [Proyecto CMP170HX/GA100](https://github.com/g4bwy/cmpunlocker): declara otro chip, firmware y controlador.
- [Envytools](https://github.com/envytools/envytools): herramientas y bases; comprobar cobertura de variante.
- [Ghidra Falcon](https://github.com/marysaka/ghidra_falcon): soporte parcial, repositorio archivado; una salida plausible no prueba semantica correcta.

No se ha verificado una ruta publica compatible con GA106 para sustituir el
firmware privilegiado. Eso es un limite de esta investigacion, no una prueba
de imposibilidad. Alcanzar privilegios no implementa por si solo el controlador,
la gestion de energia, los contextos, los shaders ni Direct3D de un juego.

## 6. Criterio de utilidad para AAA

El objetivo observable es un fallo explicable: identidad del shader y envio,
canal y generacion, ultimo trabajo finalizado, recursos y permisos, evento
original, campos conocidos y motivo de recuperacion permitida/bloqueada.
Con varios trabajos en vuelo, no culpar automaticamente al ultimo shader.
Una ejecucion serial de diagnostico puede acotar el responsable; no es un
benchmark de rendimiento normal.

Para cada primer lote fallido de PROTON-X: conservarlo, reducirlo a una carga
minima, contrastar emision y recursos, corregir, comprobar finalizacion y
volver al lote original. Esto puede avanzar en sesiones con GSP operativo
mientras la investigacion de control propio sigue abierta.
