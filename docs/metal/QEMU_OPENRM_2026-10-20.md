# QEMU y referencia Linux Reconstruido -- 20-10-2026

Se reconstruyo BMO completo con BuildOnly/SinDoom y se probo su EFI en
QEMU. Fuente de la EFI: a16d02eabc3d. Build correcto, 201,4 segundos.
BOOTX64.EFI: 2897920 bytes; SHA256:
`bc6f5de8a87ebafb66823446a2e89c63b0363b13ed8f8c1d6452aceda8f41910`.
QEMU 11.1.0, v11.1.0-12130-ge470268ff4, EDK2 de su instalacion Windows.

## Referencia consultada

`C:/Users/Salazar/Documents/Linux Reconstruido`, revision
`27fafba1ea733b67ced86f375427c189121b7b97`, solo lectura.
Su scripts/probar_arranque.py aporta pc + WHPX + CPU host + 1 vCPU + 2 GiB;
el taller usa otra receta TCG/icount. Se reutilizo la primera para BMO,
con su propia EFI y VGA estandar. No se reutilizaron discos ni credenciales.

El informe fisico
`evidencias/pc-real-w3h/informes/gpu-7.3.0-rc7-pc-g32c52ec87d62-nvidia-hyprland.txt`
registra modulo NVIDIA abierto y GPU Firmware 615.78.08. Es una referencia
real para comparar con G0; el arranque de QEMU con graficos software no
demuestra esa cadena en BMO ni prueba Cyberpunk.

## Resultados observados

Evidencia local bajo target/, conservada por intento:

| Directorio | Resultado |
| --- | --- |
| qemu-openrm-20261020 | Disco FAT conectado solo lectura rechazado por QEMU. |
| qemu-openrm-20261020-overlay | q35, TCG, icount auto: a los 45 s la captura queda en s1, cpuOK ebs. |
| qemu-openrm-20261020-whpx | pc/WHPX con xHCI: entra al kernel y falla al leer el BAR de USB. |
| qemu-openrm-20261020-whpx-sin-usb | Captura del shell: BMO-X operativo; escritorio no arranca por falta de datos montados. |
| qemu-openrm-20261020-tcg-sin-usb | Control TCG sin icount: shell operativo; tiempos mostrados incoherentes, no usarlos como rendimiento. |
| qemu-openrm-20261020-whpx-ahci | Perfil final del ejecutor: shell operativo, AHCI detectado, sin volumen de datos montado. |

El fallo xHCI tiene vector 0x0E, lectura no presente desde kernel,
RIP 0x58F026 y CR2 0xFFFF80C000000000. El BAR anunciado fue
0xC000000000; s2 anuncio un physmap alto de 0..16 GiB. La direccion queda
fuera de ese rango. Pendiente resolver el mapeo MMIO de ese dispositivo;
se excluye del perfil de prueba por defecto. No es un fallo de la RTX.

La captura sin USB registra RING 0 y el prompt bmo-x, pero tambien
`NO se pudo cargar la imagen` y `no hay volumen de datos montado`.
El FAT virtual no reproduce las dos particiones GPT de BMO; copiar d.bex
a su raiz no equivale a montar un volumen de datos. Falta una imagen de
disco con ese contrato para acreditar el escritorio. El serial termina
antes que la pantalla porque pasa a una cola; no usar su ultima linea
como prueba de bloqueo del kernel.

La ultima captura con AHCI agrega `ninguna particion tiene un volumen
ESTRATOS`. Su presencia no sustituye la GPT/FAT32 de datos para d.bex.
El build completo y los guardianes planes, codeowners, ascii-sweep y
diff --check terminaron correctamente. Las pruebas QEMU cierran su proceso
con salida 0; esa salida corresponde a la recogida de evidencia.

## Alcance para OpenRM

La prueba acredita carga UEFI, salto s1/s2/kernel y shell en el perfil
sin USB. No hay passthrough NVIDIA: VGA emulada, sin ensayo HS, GFW,
MMU_LOCK ni GSP. El 0x15 sigue pendiente de una nueva tanda en la RTX.
F2b de 615 y la validacion de juegos conservan su estado pendiente.

El [ejecutor reproducible](../../toolchain/tools/qemu/README.md) guarda la
orden completa, hash, inventario PCI y captura, y no declara exito por
el mero hecho de que QEMU permanezca vivo.
