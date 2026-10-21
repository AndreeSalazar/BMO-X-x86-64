# Mapa de opciones GPU y PROTON-X, 21-10-2026

Inventario pedido por el propietario: trabajo aplicado, evidencia y rutas
posibles. No activa experimentos, compra hardware ni agrega casillas a los
planes. Los costes relativos son una evaluacion de alcance, no plazos.
Los relatos antiguos conservan su fecha; manda el
[foco vigente](../plan/EL_FOCO.md) y el informe identificado por build.

## Punto de partida

- Ultimo metal analizado: AA25256BE, 13:55. Tres conjuntos validos,
  argumentos SEC2 correctos antes de STARTCPU, despues retorno 0x15 y
  LOGINIT cero. [Parte](HS_ARGUMENTOS_METAL_2026-10-21_AA25256BE.md).
- El build 4A1437648 tuvo un recorrido booter cero, RISC-V activo y
  LOGINIT 69; publico GSP_INIT sin confirmacion en 30 s. Otros tres dieron
  0x15. Ese buen recorrido no se traslada a versiones posteriores.
- Paquete mas reciente: a4c34f5530bc, auditoria WPR; 490 pruebas Rust,
  36 Python aprobadas y cinco omitidas, build completo y guardianes
  aprobados. Todavia no hay informe de metal analizado de este paquete.
- No hay Cyberpunk operativo por RTX demostrado en estas tandas. El
  [plan PROTON-X](../plan/PLAN_PROTON_X.md) registra intro por CPU el 13-10,
  aproximadamente 16 s/fotograma, y cero lotes del juego dibujados por RTX.

## Lo que ya existe o se aplico

Esta tabla incluye la base previa del arbol; no atribuye todo a una sesion.
Implementado no equivale a metal satisfactorio ni a causa de 0x15 resuelta.

| Pieza | Trabajo existente/aplicado | Estado y limite |
| --- | --- | --- |
| PCI y perfil GA106 | Identidad de la RTX, BAR, bus master y guardas de la puerta | Base existente; no perfil universal de NVIDIA |
| IOMMU | Prestamo de paginas, permisos, oraculo y eventos | Base existente; cero eventos no demuestra toda coherencia DMA |
| VBIOS/FWSEC | Extraer descriptor, parchear FRTS, elegir firma y ejecutar | FWSEC termina en los ultimos metales |
| WPR/radix3 | Reparto VRAM, bootloader, firma, imagen y metadata V1 | Constructor y comprobacion host; no equivale a aceptacion del booter |
| Versiones | 570.144 y 615.78.08 identificadas; cuatro archivos emparejados | 615 sigue dando 0x15; cambiar version no basto |
| Referencia ULTRA | Estudio de Linux Reconstruido y su NVIDIA 615 | Solo lectura; referencia de arranque real, no port completo |
| Referencia Windows | Estudio del 610.74 instalado | NVML no acredito GSP; no se presume el mismo camino que Linux |
| Modos G0 | riscv, seguido, reset, ptimer y junto; receta openrm por defecto | No bastaron para hacer G0 fiable; no repetirlos como novedades |
| GFW/MMU | Espera acotada, PLM/PRI, limites MMU y datos al vencer | Implementado; no atribuir cada 0x15 a falta de espera |
| Contrato DMA | FULL/IDLE, intervalos, alineacion, TCM y fusible PRI | Correcciones y pruebas aplicadas |
| Regresion propia | Se elimino el rechazo incorrecto a DMEM SEC=1 de FWSEC | cd4018237; el metal posterior vuelve a alcanzar el booter |
| Cache/vigia | wbinvd y huellas de datos prestados | Base previa; no certifica transacciones reales de GPU |
| Capturas | Fases, sesiones/builds, perdidas, cierres y BLAKE3 | Lectores verifican conjuntos; historial no es otro intento |
| Lente HS | 21 registros, PRI, tiempos, entradas y hash de copia CPU | Instrumental observado; no ve internos protegidos de HS |
| Argumentos | Foto despues de escribir BOOTVEC/buzones y antes de STARTCPU | Tres metales con valores correctos y luego 0x15 |
| Transporte 615 | MCTP/NVDM, GMCAPI/NVKV, INIT y EXEC_HS_BINARY acotado | Implementacion inicial; respuesta INIT no acreditada |
| Lente de colas | TX/RX, cabeceras RAM, hash INIT y fotos hasta 29 s | Se ejecuta solo cuando el booter permite llegar; pendiente metal |
| Auditoria WPR | Pagina propia, traducciones de extremos y 256 bytes contra copia escrita | a4c34f553; pendiente metal. Fallo previo bloquea STARTCPU |
| Traduccion de informes | lente.py y meta.py | JSON con campos, validez, diferencias y limites explicitos |
| Supervisor GSP | Estado, fallos, diagnostico y recuperacion implementados por partes | Gobierno util y recuperacion siguen requiriendo prueba real |
| QEMU | UEFI, etapas y shell sin USB en WHPX/TCG | Falta GPT/datos y MMIO alto xHCI; no hay RTX asignada |
| PROTON-X | Carga PE/DLL, partes Win32/D3D12, ruta CPU y backend SM86 | Parcial; la matriz del juego conserva sus rechazos |
| PP3 CPU | MSR 0x49, lista blanca, emulador, INTI y pruebas | ba044154d; independiente del desbloqueo de la GPU |

Fuentes de esta tanda: [carga 615](CONTRATO_OPENRM_615_2026-10-21.md),
[lente HS](LENTE_HS_2026-10-21.md),
[GMCAPI](F2B_615_INICIAL_2026-10-20.md),
[argumentos](HS_ARGUMENTOS_ANTES_STARTCPU_2026-10-21.md),
[WPR](WPR_AUDITORIA_2026-10-21.md) y
[QEMU](QEMU_OPENRM_2026-10-20.md).

## Opciones tecnicas para la RTX actual

| Opcion | Que permitiria decidir | Situacion |
| --- | --- | --- |
| Probar WPR nueva | Si pagina, permisos o bytes difieren antes de arrancar | Instrumento listo; siguiente dato de metal |
| Contrastar buen/mal intento del mismo build | Primera diferencia externa con firmware/modo/instrumental constantes | HSA3 abierto; no fabricar el par con builds distintos |
| Contrastar entrega con OpenRM 615 | Incumplimiento concreto de orden, descriptor, memoria o vida de prestamo | HSA1/HSA2 abiertos por partes |
| Revisar DMA/coherencia y tablas/TLB | Diferenciar copia CPU correcta de lectura GPU efectiva | Requiere evidencia nueva; no abrir toda RAM para probar |
| Revisar WPR contra limites MMU/VGA | Detectar reparto incompatible, no solo ver registros legibles | Instrumental existe; falta demostrar divergencia causal |
| Estudiar margen y recuperacion WPR | Evaluar F4a3 con layout, liberacion y postcondiciones | Planificado, no aplicado; no prueba VRAM defectuosa |
| Comparacion controlada con 570 | Separar efectos de version conservando cada paquete/protocolo | Reserva existente; no mezclar booters y runtime |
| Observacion equivalente en ULTRA | Un control del mismo hardware con driver funcional | Referencia existente; captura sincronizada mas completa seria trabajo nuevo |
| Caso minimo del booter | Reducir variables y preparar un caso reproducible para estudiar | Posible banco nuevo; no sustituye el contrato del sistema completo |
| Identificar rama estatica del firmware | Dar contexto real al retorno si el binario/modelo lo permite | Investigacion; el desensamblado bruto no lo ha demostrado |
| Completar GSP_INIT 615 | Estudiar TX consumido, RX, eventos y respuesta correlacionada | Solo despues de superar el booter |
| Portar operaciones posteriores 615 | Recursos, objetos, consultas, canales y trabajo real | F2b no termina al construir INIT; esta parte sigue incompleta |
| Recuperacion y apagado | Revocar, recuperar y reutilizar recursos con estado conocido | Pruebas de llegada pendientes; no son otro reset al azar |

La ruta de entrega sigue: G0 fiable -> GSP_INIT confirmado -> trabajo pequeno
con resultado -> canal sano -> cliente real. Las tareas HSA/HSG viven en
[HS_INVESTIGACION](../../platform/drivers/gpu/Nvidia/HS_INVESTIGACION.md).
El criterio operativo de G0 conserva diez arranques consecutivos registrados;
no es una garantia estadistica de fallo imposible ni una orden de reiniciar
en serie antes de tener una prediccion que probar.

## Opciones generales y arquitectonicas

| Ruta | Beneficio posible | Coste/limite respecto a BMO actual |
| --- | --- | --- |
| Continuar BMO nativo con cadena firmada | Conserva la arquitectura y trabajo existente | Depende de resolver G0 y completar runtime |
| Avanzar PROTON-X en CPU | Semantica DXIL, recursos, Win32, casos pequenos y regresiones | Sirve para desarrollo; rendimiento del juego no resuelto |
| Banco comparativo en Linux | Comparar resultados con drivers que ya funcionan alli | Exito en Linux no acredita ejecucion en BMO |
| Inspirarse en Nova/Nouveau | Otra referencia de inicializacion Ampere | No sustituye firmware ni abre HS automaticamente |
| Estudiar port/adaptacion mayor de OpenRM | Reutilizar mas del controlador publicado | Adaptacion de interfaces/servicios del SO; proyecto grande |
| Estudiar NVK/Mesa como backend | Vulkan y compilador abiertos como referencia/alternativa | NVK requiere Linux en su entorno documentado; port BMO no existe |
| Usar arquitectura D3D12 sobre Vulkan | Referencia vkd3d-proton y pruebas de compatibilidad | Exige backend Vulkan suficiente; cambia la ruta nativa actual |
| GPU virtual Virtio-GPU/Venus | Desarrollar un camino acelerado bajo un anfitrion | Driver/protocolo invitados nuevos; no valida GSP nativo GA106 |
| Asignacion PCI mediante VFIO | Una VM recibe acceso al aparato real | Preparacion de host, IOMMU/reset y soporte invitado; no probado aqui |
| Mantener QEMU como banco del SO | Reproducir arranque, discos, fallos y ABI | VGA emulada actual no reproduce HS de la RTX |
| Backend de laboratorio apoyado en otro SO | Separar pruebas del juego de arranque propio de GPU | Arquitectura nueva; no equivale a Cyberpunk nativo en BMO |
| Desarrollar AMD RDNA4 | Una segunda familia de GPU y otra referencia de driver | Perfil reservado y modelo PSP; aun sin PCI IDs ni driver 3D real |
| Evaluar otro hardware | Elegir por soporte demostrable y documentacion | Necesita auditoria previa del perfil exacto; ninguna compra garantiza el port |
| Aparcar NVIDIA temporalmente | Seguir el SO, compiladores y compatibilidad del juego | El hito GPU del juego queda pendiente; lo guardado es retomable |

NVK es un driver Vulkan para NVIDIA y documenta requisitos Linux:
[Mesa NVK](https://docs.mesa3d.org/drivers/nvk.html). Valve Proton usa Wine
para juegos Windows sobre Linux; vkd3d-proton implementa D3D12 sobre Vulkan:
[Proton](https://github.com/ValveSoftware/Proton),
[vkd3d-proton](https://github.com/HansKristian-Work/vkd3d-proton).
Estos componentes no son instaladores para el kernel de BMO.

Venus serializa comandos Vulkan sobre Virtio-GPU; VFIO permite asignar
dispositivos con aislamiento IOMMU. Son rutas distintas y requieren trabajo
propio: [Venus](https://docs.mesa3d.org/drivers/venus.html),
[VFIO](https://docs.kernel.org/driver-api/vfio.html).

El [perfil RDNA4 local](../../platform/drivers/gpu/rdna4/src/lib.rs) tiene
pci_devices vacio y claims siempre falso para cualquier SKU. Su PSP es una
maquina de estados sin MMIO, no un controlador acelerado. Una ruta AMD
necesita firmware/PSP, memoria, colas, sincronizacion y backend de shaders;
el soporte del [driver Linux AMDgpu](https://docs.kernel.org/gpu/amdgpu/index.html)
no se hereda por escribir AMD en un perfil BMO.

## HS propio y persistencia

- Usar la cadena firmada para una operacion admitida: ruta principal actual.
- Corpus y contratos GA106: HSG1/HSG2, con revisiones y resultados negativos.
- Modelo anfitrion de instrucciones/DMA: HSG3, declarando cobertura y desconocidos.
- Capacidad propia concreta: HSG4 solo tras precondiciones y efecto verificables.
- Duracion/reaplicacion frente a reset/corte: HSG5, separada de persistencia fisica.
- Firmware propio NS/LS: investigacion distinta; no concede por si solo HS.
- HS abierto permanentemente en esta GA106/615: sin demostracion propia.

La documentacion de [NVIDIA Falcon Security](https://nvidia.github.io/open-gpu-doc/Falcon-Security/Falcon-Security.html)
describe HS con autenticacion y estado interno restringido; la descripcion
de [Nova Falcon](https://www.kernel.org/doc/html/latest/gpu/nova/core/falcon.html)
aporta el contexto de la cadena. Las referencias de otros chips no prueban
compatibilidad con esta tarjeta ni persistencia fisica.

Aceptar 0x15 como exito, ignorar PRI, simular autenticacion o borrar guardas
no constituye inicializacion ni un trabajo terminado. Cambiar limites o
firmas sin una precondicion comprobada tampoco aporta una prueba causal.
El driver Windows depende de su arquitectura WDDM: estudiarlo es una
referencia, instalar su binario en BMO no es una ruta implementada.
[Microsoft WDDM](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/windows-vista-display-driver-model-design-guide).

## Lo que falta para Cyberpunk despues del arranque

La ruta actual es EXE/DLL -> Win32/D3D12 PROTON-X -> Programa
PROMETEO/VERRANO -> SM86/juez -> recursos y canal RTX. Vulkan es una
alternativa arquitectonica, no una condicion impuesta a esta ruta.

1. Respuesta GSP_INIT y operacion pequena con resultado correcto.
2. Recursos/canales y GR sanos, con recuperacion observada.
3. Cobertura DXIL/shaders de los Draw y Dispatch reales del juego.
4. Descriptores y seleccion dinamica de texturas que hoy rechaza el backend.
5. Vistas TYPELESS, formatos float/HDR y sus combinaciones con profundidad.
6. Texturas restantes: BC6/signados y formas arrays/3D/cubos segun el censo.
7. MRT, stencil, instancias y semanticas de profundidad pendientes.
8. Computo completo: SRV/UAV, atomicos/barreras y primera ejecucion real.
9. Recuperacion de Dispatch parcial sin repetir efectos sobre UAV.
10. Residencia RAM/VRAM, transferencias, envio y Present medidos.
11. APIs/DLL, hilos, streaming, entrada y audio exigidos al llegar a partida.
12. Escena repetible con guardado/recarga y tiempos de fotograma.

La primera validacion conserva configuracion fija, sin atribuir soporte
gratuito a ray tracing, DLSS, FSR o generacion de fotogramas. Se reduce a
un caso pequeno por semantica antes del fotograma y despues a la partida.
Los detalles y propietarios ya estan en el [plan PROTON-X](../plan/PLAN_PROTON_X.md).

## Orden recomendado en este corte

Una prueba del paquete WPR y lectura de su evidencia; contraste causal si
aparece una divergencia; mientras, casos DXIL/recursos en anfitrion. Si
todo lo observable coincide y falta una hipotesis nueva, aparcar el ensayo
fisico es una opcion razonable. Las rutas AMD, Vulkan, virtualizacion o HS
propio se deciden como trabajos separados por su alcance, no como curas
ya disponibles del 0x15. Este inventario no cambia el estado de los planes.
