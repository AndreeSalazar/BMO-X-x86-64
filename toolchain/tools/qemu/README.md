# Arranque UEFI en QEMU

Prueba aislada de la EFI construida de BMO. Requiere Python, QEMU con
qemu-img y EDK2 (incluidos en la instalacion Windows usada aqui).

Desde la raiz, despues del build:

```powershell
python toolchain/tools/qemu/arranque.py --staging Ultra_kernel_x86-64/staging --out target/qemu-mi-prueba --seconds 45
```

El directorio de salida debe ser nuevo. Se copian EFI y BMO-DATA; la
carpeta copiada respalda un disco qcow2 con escritura local. Las variables
UEFI tambien son una copia. No conecta discos fisicos ni red.

Configuracion: pc, WHPX, CPU host, 1 vCPU, 2 GiB, SATA AHCI y VGA estandar.
`--accel tcg` usa CPU max sin icount; `--usb` incluye xHCI para diagnostico.
No habilitar USB por defecto: la prueba del 20-10 encontro un fallo al
acceder a su BAR alto. El teclado PS/2 emulado queda disponible.

Recoge serial.log, qemu.stderr.log, pantalla.png y resultado.json por QMP;
cierra solamente el proceso creado por esta ejecucion. El codigo de salida
0 significa evidencia recogida, no arranque aprobado. Revisar captura y
logs: `query-status: running` tampoco acredita que el sistema funcione.
`boot_success` queda sin decidir y `rtx_tested` siempre es false.

El volumen FAT virtual de QEMU sirve para cargar UEFI; no reproduce la
GPT con ESP y datos de BMO. Esta prueba no acredita escritorio, montaje
de datos, juegos ni RTX/GSP. [Resultado del 20-10](../../../docs/metal/QEMU_OPENRM_2026-10-20.md).
