# HS en la RTX 3060 12G -- investigar el reemplazo con evidencia

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
