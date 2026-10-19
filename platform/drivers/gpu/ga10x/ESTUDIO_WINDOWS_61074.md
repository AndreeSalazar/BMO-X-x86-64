# Windows 610.74 frente a BMO -- referencia real antes de copiar el arranque

18-10-2026. Pedido del propietario: estudiar el driver oficial de Windows
que gobierna su RTX 3060 12G y aplicar lo aprendido al arranque BMO-X.
Este corte identifica y examina copias; no reconstruye todavia la secuencia
MMIO ejecutada por Windows ni cambia el arranque del kernel.

## 1. Lo observado en esta maquina

[MEDIDO en Windows] `nvidia-smi` devuelve RTX 3060, driver/KMD 610.74,
PCI device `0x250410DE`, VBIOS `94.06.2f.00.9a`, modelo WDDM y
`GSP Firmware Version: N/A`. La consulta directa de NVML inicializa bien,
obtiene la GPU 0 y devuelve estado 3, `Not Supported`, tanto para modo GSP
como para version. Los campos rechazados se guardan `null`, nunca `false`.

[CITADO] La [referencia NVML de NVIDIA](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html)
documenta estas consultas y ese rechazo cuando GSP no esta habilitado.
El [manual Linux 570.144](https://download.nvidia.com/XFree86/Linux-x86_64/570.144/README/gsp.html)
tambien distingue version activa de N/A; su parametro de modulo es de Linux,
no una orden aplicable a Windows o BMO.

[INFERIDO] Esta sesion no acredita GSP-RM activo en Windows. Es compatible
con gestion desde el host, pero no permite reconstruir todas las operaciones
internas ni excluir todo uso temporal de microcodigo GSP/HS. **Windows
funcionando no prueba que ejecute el mismo booter/runtime que BMO.**

## 2. Copias de referencia y procedencia

La [pagina oficial 610.74](https://www.nvidia.com/en-us/geforce/drivers/details/274188/)
identifica esa release. La fuente medida es el paquete YA instalado en
DriverStore, no un instalador nuevo descargado. `nvmdi.inf` declara
`DriverVer = 07/02/2026, 32.0.16.1074`; la version del SYS coincide.
`Get-AuthenticodeSignature` devuelve `Valid`, firmante Microsoft Windows
Hardware Compatibility Publisher. Esto verifica el archivo Windows por
Authenticode, no la firma BROM del firmware ni cada dato instalado.

Copias locales fuera del repositorio:
`BMO-externo/firmware/nvidia/windows-610.74/`. Se cotejo el SHA-256 de cada
origen con su copia; iguales. El manifiesto `estudio.json` conserva origen,
versiones, medidas, huellas, consultas y resultados de comparacion.

| Copia | Bytes | SHA-256 |
| --- | ---: | --- |
| nvlddmkm.sys | 120313064 | 61cb92adf51e915510926dff6533cbe92591861316142de30df5b4a5b1eba769 |
| gsp_ga10x.bin | 84293784 | ba5667d90467581ce43f1e7320e7f59a000a5278779f8fc35952309adad514a2 |
| nvmdi.inf | 371405 | a1edbb3c95cc520e9ebb44a660bf0ccdf3809c96479cd96336864dca915f95bc |

No se incluyen binarios NVIDIA en el commit. El SYS tiene formato PE AMD64,
32 secciones y registro CodeView GUID `1AC6FAB1-9D8A-4277-950F-E47B6F36A07B`,
age 1. El nombre PDB apunta a r610; no se obtuvo ese PDB ni se atribuyen
nombres privados a direcciones de funciones sin simbolos comprobados.

## 3. Firmware distribuido y firmware ejecutado son preguntas distintas

[MEDIDO en copias] El lector ELF/LIBOS de BMO acepta ambos GSP:

| Campo | Copia Windows | Copia BMO |
| --- | --- | --- |
| Release del paquete/uso | Windows 610.74; uso GSP no acreditado | 570.144 del proyecto |
| `.fwversion` literal | `610.53` y NUL | `570.144` y NUL |
| `.fwimage` offset / bytes | 0x40 / 84242432 | 0x40 / 63541248 |
| `.fwsignature_ga10x` offset / bytes | 0x0506206b / 4096 | 0x03c9f06c / 4096 |
| Secciones `.fwlogging*` | 0 | 0 |

La cadena interna 610.53 se conserva tal cual; no se renombra 610.74 ni se
declara corrupcion por diferir del numero del paquete. No se encontro el
diccionario de logs direccionable en ninguna de estas dos copias. Una firma
presente en ELF tampoco demuestra que BROM la haya aceptado.

La busqueda exacta de los cuatro ficheros completos 570.144 (load, unload,
bootloader y GSP) en el SYS y GSP Windows no encontro coincidencias contiguas.
Eso solo excluye esos ficheros completos identicos en claro; no excluye
componentes equivalentes, fragmentados o comprimidos. No se desensambla el
contenido completo del SYS como si todo fueran instrucciones ejecutables.

## 4. Primer contrato HS de BMO extraido con su propio lector

El ejemplo `contrato_hs` reutiliza `arranque::booter`, sin otro parser.
Entrada: booter_load-570.144.bin, SHA-256
`4497e3eff7e95c774b8a569d17b27c08c9650158d10b229d2be81cdcad9a085b`.

| Campo | Valor del fichero | Propietario/limite |
| --- | --- | --- |
| Ucode | offset 0x378, bytes 0xec00 | lector BinHdr |
| Firmas | offset 0x3c, 2 de 0x180 bytes | seleccion segun fusible, no fijar indice sin lectura |
| Parametros BROM | fuse_ver 1, engine mask 1, ucode id 3 | `Booter` y `falcon::brom` |
| IMEM | origen 0x100, destino 0, bytes 0x8900 | ucode; DMA seguro etiquetado |
| DMEM | origen 0x8a00, destino 0, bytes 0x6200 | ucode; firma insertada en copia |
| Firma / BROM_PARAADDR | parche ucode 0x8a10 / DMEM 0x10 | offsets con origen distinto |
| Arranque | 0x100 | no confundir con offset 0x478 del fichero |
| Fusible real / indice usado | ausentes en esta inspeccion | necesitan captura del intento BMO |
| IOVA, paginas, vida y postcondiciones | no salen del fichero | completar con kernel/captura HSA1 |

[CITADO en kernel] La entrega estatica declara IOVA_BOOTER `0x3b000000`,
32 paginas NEUTRO de 4096 bytes reservadas, primeras 16 prestadas solo
lectura al aparato y ultimas 16 para el fichero original. WPR meta se
direcciona en `0x3e007000` y LIBOS en `0x3c000000`. Son constantes del codigo,
no direcciones fisicas ni una fotografia de las traducciones de este intento.
`BOOTER_PRESTADO` impide un segundo prestamo; la reserva se conserva.
Falta contrastar esa vida en reintento/descarga y las traducciones reales.

### Diferencia de carga que merece auditoria

La funcion publica [s_prepareHsFalconDirect de OpenRM 570.144](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/8ec351aeb96a93a4bb69ccc12a542bf8a8df2b6f/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_falcon_tu102.c)
escribe IMEM/DMEM por registros, con regiones NS/HS y etiquetas. BMO usa
DMA etiquetado en SEC2, con FBIF configurado como memoria de sistema
coherente. El WBINVD de `soltar_booter` ocurre despues de esas copias DMA,
antes de STARTCPU. Ese orden es un hecho del codigo, no prueba de cache
incorrecta: falta comprobar el contrato de coherencia de todo el camino.

La referencia Direct usa un layout NS/HS y BOOTVEC propios; BMO interpreta
su contenedor con entrada etiquetada 0x100. Antes de proponer PIO como
experimento se debe identificar que variante de carga corresponde al
booter GA106 usado y conservar destinos, etiquetas y firma equivalentes.
La existencia de la funcion Direct no demuestra que sea la llamada en
Windows 610.74. **Diferencia concreta encontrada; causa 0x15 no demostrada.**

[CITADO en codigo local] `gpu_despertar::despertar` usa esa firma, tramos,
DMA, BROM y buzones antes de STARTCPU. `falcon::como_va` devuelve HALT y
dos buzones; no un certificado independiente de todos los chequeos HS.
`info_despierto` conserva por separado parada SEC2 y RISC-V activo.

OpenRM 570.144 `kgspExecuteHsFalcon_TU102` prepara, escribe buzones,
arranca y espera HALT; el wrapper examina despues MAILBOX0. Por tanto, su
estado del ejecutor tampoco debe inventarse como un codigo detallado de
autenticacion BROM. Fuente fijada en el
[estudio GSP existente](GSP_570144_EVIDENCIA.md).

## 5. Consecuencia para gobernar la tarjeta

Se abren dos comparaciones con evidencia distinta:

1. Windows 610.74: determinar el camino realmente usado antes de atribuirle
   el protocolo GSP. Se dispone de identidad, copias y consultas; faltan
   simbolos/flujo validado o traza de inicializacion. No se hace pasar un
   inventario estatico por esa traza.
2. OpenRM 570.144: referencia publica comparable al firmware BMO. Completar
   HSA1 de memoria/vida/barreras y HSA2 de resultados. Luego contrastar los
   conjuntos bueno/fallo; E3 RISC-V ya fallo y no se anuncia como arreglo.

Una ruta desde host con menos dependencia de GSP es una investigacion
posible, pero sustituir la gestion RM exige inventariar servicios y autoridad;
no consiste en quitar una llamada al booter del kernel actual. Tampoco se
propone intercambiar el GSP Windows 610.53 con el 570.144 de BMO a ciegas.

**Resultado de este corte:** referencia Windows real identificada, archivos
preservados, diferencias comprobadas y contrato de fichero HS extraido.
**Sigue pendiente:** causa 0x15, secuencia Windows reconstruida y prueba G0.

## 6. Reproduccion en el anfitrion

Desde la raiz BMO, con las copias locales indicadas:

```powershell
nvidia-smi --query-gpu=name,driver_version,pci.device_id,vbios_version --format=csv,noheader
python toolchain/tools/nvidia-hs/consulta_windows.py
cargo run -p bmo-gpu-ga10x --example libos -- ../BMO-externo/firmware/nvidia/windows-610.74/gsp_ga10x.bin
cargo run -p bmo-gpu-ga10x --example contrato_hs -- ../BMO-externo/firmware/nvidia/ga102/gsp/booter_load-570.144.bin
```

`contrato_hs` acepta opcionalmente un fusible hexadecimal DECLARADO; no lo
lee del aparato. Sin dato no elige firma. Las salidas derivadas originales
estan en `target/gsp-investigacion/`; no se modifica el firmware original.
