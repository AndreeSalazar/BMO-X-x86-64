# Contraste ULTRA LinuX / BMO-X -- G0, 20-10-2026

Pedido del propietario: usar Linux Reconstruido como referencia manteniendo
la arquitectura y esencia propias de BMO-X. El otro repositorio se consulta
en solo lectura; las modificaciones de este trabajo pertenecen a BMO.

## Referencia que funciona

Repositorio local: C:/Users/Salazar/Documents/Linux Reconstruido,
HEAD observado e055cc4e44818e1b6b935cbf71035be08c235435.
Su evidencia del 19-10, no una prueba nueva del 20-10:
`evidencias/pc-real-w3h/informes/gpu-7.3.0-rc7-pc-g32c52ec87d62-nvidia-hyprland.txt`.
Registra RTX 3060, PCI 29:00.0, VBIOS 94.06.2f.00.9a, modulo abierto y
GPU Firmware 615.78.08; sesion NVIDIA/Hyprland, salida 0. El nombre de
version coincidente no demuestra igualdad de todos los binarios o argumentos.
No se infiere rendimiento de juegos de ese informe.

`src/rootfs/bin/ultra-preparar-nvidia` crea nodos GPU/control/modeset con
nvidia-modprobe. Eso habilita clientes Linux; no contiene el procedimiento
de reset/FWSEC/booter que falta contrastar. BMO no necesita reproducir /dev,
udev o DRM para inicializar su GSP; conserva sus capacidades, IOMMU,
prestamos, ABI y Director. La leccion transferible es verificar etapas y
resultados con el firmware y driver emparejados.

## Contraste con OpenRM 615.78.08

Se consultaron los fuentes oficiales de la version que declara la evidencia:

- [kernel_gsp_tu102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/615.78.08/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c):
  `kgspBootstrap_TU102` ejecuta FWSEC, selecciona RISC-V, programa LIBOS,
  ejecuta booter y comprueba actividad antes de la inicializacion del runtime.
- [kernel_falcon_ga102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/615.78.08/src/nvidia/src/kernel/gpu/falcon/arch/ampere/kernel_falcon_ga102.c):
  seleccion BCR no equivale a actividad; la conmutacion de vuelta a Falcon
  libera priv lockdown. Un error PRI en un registro Falcon tras seleccionar
  RISC-V no basta para diagnosticar un reset fallido. La heuristica de OpenRM
  sobre BCR inaccesible tampoco acredita actividad del runtime en BMO.
- [kernel_gsp_booter_tu102.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/615.78.08/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_booter_tu102.c):
  un buzon de retorno del booter distinto de cero es fallo; no da aqui un
  significado especifico de 0x15. No atribuirlo a firma ni a shader.

La receta BMO openrm aproxima este orden. Los cinco conjuntos del
[metal](G0_2026-10-20_05A482C51.md) siguen fallando; el nombre del modo
no certifica equivalencia de toda la implementacion.

## Cambios aplicados en BMO

1. Foto de solo lectura con BCR, RISCV_CPUCTL, FALCON_CPUCTL y ambos buzones.
   Conserva los crudos y separa seleccion valida, actividad valida y argumentos
   legibles. PRI no se convierte en activo, parado ni direccion medida.
2. CABINA toma esa foto tras FWSEC, tras reset/seleccion BCR y tras programar
   LIBOS. Cada foto incluye TSC antes/despues; son lecturas secuenciales.
   La mascara documenta BCR valido/seleccion RISC-V (bits 0/1), control RISC-V
   valido/activo/halt (2/3/4), argumentos validos/iguales a IOVA_LIBOS (5/6).
   Igualdad de buzones comprueba lo leido por CPU, no consumo interno del GSP.
3. MMU_LOCK guarda SIEMPRE ambos limites crudos y decodificados, incluso si
   VGA no aporta una base valida. En el informe anterior solo habia `hasta`
   en ese caso: no daba base para aplicar `min(MMU_LO, VGA)` a un nuevo layout.

No se agregan resets, escritura de protecciones, reintentos ni otro firmware.
No se aplica aun una correccion causal de 0x15. Las lecturas y entradas de
bitacora agregadas pueden variar ligeramente el tiempo del procedimiento;
comparar builds distintos no aisla por si solo una causa.

Dos regresiones anfitrion verifican que la foto no escribe, que seleccion
RISC-V con Falcon inaccesible puede coexistir con RISC-V parado o activo,
y que PRI/BCR no valido se conservan como dato desconocido.

Verificado: 459 pruebas GA10x/supervisor aprobadas, kernel release
x86_64-unknown-none compilado y guardianes la-3060, contrato, codeowners,
ascii-sweep, planes y diff --check correctos. Sin nueva prueba fisica.

## Proxima observacion de metal

Conservar las cuatro capturas binarias ya existentes y el informe CABINA
con las tres fotos nuevas. El orden permite acotar entre FWSEC, seleccion
y argumentos. Comparar una captura buena de firmware/build/modo emparejados
si se obtiene; no contar varios save de una misma sesion como varios intentos.
Los limites MMU completos permitiran decidir si se necesita corregir layout.
No desplazarlo a ciegas por ver 0x15 o un PRI. F2b sigue siendo trabajo aparte
una vez que G0 arranque.
