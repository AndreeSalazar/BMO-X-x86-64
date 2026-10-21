# Contraste y contrato de carga OpenRM 615 -- 21-10-2026

Trabajo posterior a los cuatro fallos del
[metal 0D214540D](G0_2026-10-21_0D214540D.md). Se corrigen diferencias
concretas de preparacion, sin declarar que expliquen el retorno 0x15.
La referencia es OpenRM 615.78.08 para Ampere, no el camino HS de Turing.

## Comparacion y cambios

| Etapa | OpenRM 615 Ampere | BMO antes | Resultado de este cambio |
| --- | --- | --- | --- |
| Reset GSP y argumentos | FWSEC, reset RISC-V, LIBOS, booter SEC2 | El modo por defecto ya aproxima ese orden | Se conserva; no se agrega un reinicio o reintento. |
| Base DMA HS | Esperar FULL falso antes de cambiar BASE | Escribia BASE sin comprobar FULL | Espera acotada y rechazo PRI antes de BASE. |
| Controles DMA | Esperar FULL falso antes de MOFFS/FBOFFS/CMD | Esperaba IDLE despues de cada bloque | Comprueba FULL antes de cada bloque y conserva IDLE despues. |
| Direcciones DMA | Campos BASE/BASE1, MOFFS y FBOFFS con anchos definidos | Admitia argumentos que podian truncarse o desbordarse | Valida alineacion e intervalos completos antes de acceder al aparato. |
| Carga SEC2 | Codigo/datos y firma dentro de las regiones de carga | Validaba fichero y parte de la geometria | Comprueba relleno DMA, TCM real tras reset, motor SEC2 y firma RSA3K completa, tanto al cargar como al descargar. |
| Lectura de fusible | Lectura valida para elegir firma | Un error PRI podia contarse como version cero | Se rechaza PRI; cero valido sigue eligiendo su firma prevista. Tambien se corrige FWSEC. |
| WPR V1 | 256 bytes, offsets de Turing-Ada conservados | Serializer heredado de 570 | Contraste C contra cabeceras 615 y caso Rust con entradas reales 615. No requiere cambiar esos offsets. |
| GSP_INIT | Runtime activo antes de GMCAPI | F2b inicial implementado, aun sin respuesta fisica | Sigue pendiente de metal; esta tanda no llego al intercambio. |

FULL e IDLE son bits distintos (0 y 1). Un valor IDLE con FULL puesto
no autoriza cambiar BASE. Si FULL no baja, no se publica el siguiente
bloque. Los errores internos distinguen cola llena (6), intervalo invalido
(7) y DMA que no acaba (5); PRI conserva su rechazo propio (1).
Son detalles CABINA, no nuevos motivos de syscall ni numeros ABI.

La comprobacion de carga SEC2 aplica un techo del perfil de 64 KiB por
TCM y, despues del reset, las capacidades leidas del motor. Es mas estricta
que aceptar solo que el motor figure entre los bits de una mascara:
el booter de este camino debe pertenecer exactamente al SEC2.
La ROM sigue verificando la firma original; no se sustituye esa validacion.

## Evidencia independiente del serializer

`platform/drivers/gpu/ga10x/tests/openrm615_layout.c` compila contra las
cabeceras oficiales, sin enlazar NVIDIA con el kernel. Comprueba los
offsets WPR V1, offsets RISC-V consumidos, GMCAPI de 40 bytes y peticion
HS de 80 bytes. La estructura RISC-V completa moderna tiene mas campos;
BMO consume el prefijo cuyos offsets se contrastan.

Para reproducir el contraste C, con `ref615` apuntando al directorio de
fuentes OpenRM 615.78.08:

```powershell
clang -std=c11 -fsyntax-only `
  -I "$ref615/src/common/sdk/nvidia/inc" `
  -I "$ref615/src/common/inc" `
  -I "$ref615/src/nvidia/arch/nvalloc/common/inc" `
  -I "$ref615/src/nvidia/interface" `
  -I "$ref615/src/common/inc/swref" `
  platform/drivers/gpu/ga10x/tests/openrm615_layout.c
```

La prueba Rust 615 usa imagen de 0x19BE000 bytes y bootloader de 0x6000:
heap 0x2F6400000..0x2FE400000, WPR desde 0x2F6300000. El reparto cabe
en los ultimos 256 MiB pre-limpiados; no justifica agregar scrubber.
El extremo superior del mapa es exclusivo; el registro WPR2 HI exportado
en metal usa su representacion hardware y no se compara como si fuera
ese extremo exclusivo.

Banco ejecutado: 477 pruebas GA10x/supervisor aprobadas, incluidas ocho
nuevas. La variable BMO_FW_GSP615 se activo apuntando a
Ultra_kernel_x86-64/staging/BMO-DATA/fw/gsp: la prueba adicional leyo el
paquete real 615, cargo ambas firmas de ambos booters en copias privadas
y verifico el descriptor RISC-V. No se ejecuto firmware en el anfitrion.
El contraste C tambien compilo correctamente. Log local del banco:
target/contraste615-tests.log. La compilacion completa y los guardianes
del paquete se ejecutan antes de entregar el resultado.

## Alcance pendiente

Esta entrega mejora el contrato del arranque y su carga HS; no es una
implementacion completa de todas las API de OpenRM. No se declara G0
resuelto ni F2b completo. Los objetos/control de 615 y comandos de runtime
aun no implementados siguen en el plan. El siguiente informe de metal
debe identificar este nuevo build, el resultado del booter y, si lo supera,
la respuesta GSP_INIT. Compilar y simular la cola no prueban el arranque RTX.

## Fuentes primarias consultadas

- [Carga HS GA102 y s_dmaTransfer_GA102](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/615.78.08/src/nvidia/src/kernel/gpu/gsp/arch/ampere/kernel_gsp_falcon_ga102.c).
- [Orden de bootstrap y poblacion WPR](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/615.78.08/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c).
- [Contrato WPR V1](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/615.78.08/src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta_v1.h).

Se leyeron las copias locales de estudio bajo
target/gsp-investigacion/615.78.08-auditoria. Linux Reconstruido se conserva
como referencia de funcionamiento; no se modifico ese repositorio.
