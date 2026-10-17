# GSP-CONTROL-BMO -- etapa 1: que la GPU deje evidencia

`bmo-nvidia-gsp-control` es `no_std`, sin dependencias y con `unsafe` prohibido.
Su consumidor real es Director: `gpu control` y el capitulo GPU de `save`.
El diagnostico conserva datos ausentes y muestra los valores crudos que lo
respaldan. No crea un segundo propietario del aparato.

## Propietarios actuales (GCB0a)

Las rutas del kernel parten de `Ultra_kernel_x86-64/kernel/src/ring0/dev/`.
Los cambios de estado y las escrituras siguen en esos propietarios:

| Recurso o accion | Propietario que ejecuta | Papel del supervisor |
| --- | --- | --- |
| Preparar LIBOS, RMARGS y comprobar prestamos | `gpu_libos.rs` | Recibir resultado, sin construir otro LIBOS |
| Firmware GSP, huella y prestamos | `gpu_gsp.rs` | Identificar la imagen; no cargarla |
| Dominio DMA, paginas y candados | `gpu_prestamo.rs` y la IOMMU del kernel | Exigir evidencia de aislamiento para revisar recuperacion |
| Arranque, booter SEC2, autopsia | `gpu_despertar.rs` | Leer el informe existente; no resetear |
| Secuenciador y mensajes INIT_DONE | Kernel `gpu_despertar.rs::secuenciar`; Director `gspinit.rs` consume el mensaje | Distinguir arranque de inicializacion observada |
| Canales y envios de trabajo | `gpu_trabajo/`, incluido `pase_nv.rs` | No emitir ni intervenir en las colas |
| Apagado y descarga | `gpu_apagar.rs` | Conservar resultado, sin descargar |
| Reintento con sus candados | `gpu_reintento.rs` | Ninguna autorizacion nueva |

Director coordina solicitudes; solo el kernel ejecuta operaciones sobre el
aparato. Los formatos puros de GA10x describen datos. VERRANO conserva su API
neutra. La consulta nueva usa exclusivamente `bmo::info` y el resultado local
de `gspinit`; esas INFO ya existentes pueden leer registros y actualizar
las caches de diagnostico del kernel. No incorpora nuevas escrituras GPU.

## Contrato de evidencia (GCB0b)

`Identity` exige tarjeta, huella de firmware, build e intento. `None` es dato
no disponible, incluso si el resto existe. El valor cero presente no es ausencia.
Los identificadores se suministran desde el propietario; no se fabrican con
un reloj. Una sesion del modelo necesita los cuatro componentes. Secuencia
y tiempo monotono pertenecen a esa misma sesion. Una identidad distinta,
un numero repetido con contenido distinto o un tiempo anterior se rechazan.
Un duplicado exacto del ultimo evento aceptado es idempotente.

Hoy la ABI no entrega build e intento fiables. Director lo imprime y utiliza
solo `diagnose(Snapshot)`: una foto derivada, sin inventar una historia de
transiciones. Dos lecturas iguales delimitan la consulta, pero no prueban
atomicidad ni excluyen cambios intermedios. El INIT_DONE es un hecho local
observado por Director; no acredita que el GSP siga sano en este instante.
Un resultado ausente de init conserva `None`, en lugar de asumir que fallo.
`Operativo observado` requiere LIBOS comprobado, arranque registrado y
INIT_DONE recibido, sin motivo de fallo. Dato desconocido o contradictorio
no se anuncia operativo. Un motivo de fallo tiene prioridad sobre el exito.
El motivo del kernel y SEC2 MAILBOX0 se imprimen separados: son dominios distintos.

## Modelo y decisiones (GCB1)

Estados: SinPreparar, Preparado, Arrancando, Operativo, Fallido y
RecuperacionBloqueada. El recorrido normal exige Prepared -> Start -> InitDone.
Start recibe un plazo absoluto calculado por el adaptador segun su protocolo;
el modelo no inventa duraciones ni modifica las actuales. A partir del plazo,
Tick, Fault o InitDone conservan Timeout como primer fallo de esa sesion.
La deteccion necesita una observacion: esta biblioteca no tiene temporizador.

Toda recuperacion exige evidencia conocida y verdadera de motores parados,
DMA aislado y memoria liberada. Falta o falso bloquean. Tres verdaderos solo
recomiendan revision del propietario: no limpian el fallo ni reinician una
sesion bloqueada. Las decisiones son datos, no permisos de escritura.
El primer fallo persiste aunque lleguen nuevos fallos o un InitDone tardio.
El adaptador nativo aun no alimenta este modelo con eventos completos ni
conecta recuperacion; consume el diagnostico de foto. Ese enlace requiere
OBS-BUILD e identificacion real de intento antes de administrar acciones.

## Verificacion y siguiente etapa

Ocho pruebas Rust: orden, identidad completa, cuatro tipos de identidad ajena,
duplicados, datos antiguos, limite exacto del plazo, exito tardio,
27 combinaciones de recuperacion, bloqueo persistente y 108 combinaciones
de fotos con valores conocidos/ausentes, estabilidad y fallo.
Guardian `la-3060` S8 impide dependencias, build scripts, fuentes desviadas
y acceso de aparato en el supervisor; verifica la consulta sin ordenes.
Rechaza tambien punteros crudos, includes, modulos con path externo y
permisos locales para unsafe. Recorre fuentes bajo cfg aunque esa variante
no este activa; Rust aplica forbid(unsafe_code) a todos los modulos compilados.
Catorce pruebas del guardian incluyen fugas bajo feature y target_os; S8
las ejecuta automaticamente en cada --check, tambien desde el build.
S8 se ejecuta desde `la_3060.py --check`, ya conectado a
`Ultra_kernel_x86-64/build/guardianes.ps1`: build normal y ventana VERIFICAR
usan esa lista compartida. Cargo directo aplica forbid, pero no corre Python.
Sin Python, este guardian obligatorio rechaza el build en vez de aprobar
sin comprobar. El alcance nuevo es este supervisor NVIDIA y su consulta, no todos los unsafe
del kernel. Cualquier futuro adaptador que necesite unsafe requiere frontera
de autoridad, prestamos y vida de memoria propios y su guardian antes de entrar.
Sus patrones no sustituyen la revision del flujo, la IOMMU ni las pruebas.

Validacion del 16-10-2026: ocho pruebas Rust y catorce del guardian pasan;
Director release x86-64 compila. Pila 45984/65536 bytes, marco mayor 19552,
sin aumento. Capas, contrato, censo modular, ISA, enlaces, planes y ASCII
verificados. Una copia de prueba con unsafe bajo feature compila con la
variante inactiva y es rechazada por Rust al activarla; S8 rechaza la fuente
aunque este inactiva. La copia de prueba queda en target, fuera del commit.

Siguiente: GCB2, contrastar el protocolo inicial completo y preparar una
variante coherente para G0-MODO. Esta etapa no cambia reset, energia,
firmware, esperas ni corrige por si misma el 0x15. No requiere otra tanda de
reinicios para demostrar el contrato puro. El nuevo resumen nativo queda
pendiente de observar en metal cuando el propietario use su proximo build.
