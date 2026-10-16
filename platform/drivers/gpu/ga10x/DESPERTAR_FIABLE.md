# Despertar fiable de la RTX 3060 12G -- investigacion del 16-10

**Actualizacion 16-10, 11:07:** ya hay un parte malo concreto: llegada FRIA
registrada, FRTS correcto, booter SEC2 MAILBOX0=0x15, sin GSP activo. IOMMU
sin eventos nuevos y RAM del firmware intacta. MUNDO funciona por VERRANO
CPU. [Resultado y limites](../../../../docs/metal/METAL_2026-10-16.md).
Se aplica la correccion de clasificacion descrita abajo: `caliente()` exige
motivo 67; 0x15 solo no demuestra estado residual de Windows. G0 sigue abierto.

Pregunta: por que el propietario tiene que reiniciar siete o diez veces
si la misma tarjeta ya ha ejecutado 3D. Investigacion de commits y codigo;
sin nueva corrida del metal y sin cambiar registros ni firmware.

## Conclusion y limite

El 3D funciona en las corridas documentadas. La fiabilidad del arranque
GSP sigue abierta (G0). No esta demostrada una causa electrica o ACPI
para el fallo ACTUAL: el nuevo parte acredita un 0x15 en frio, no su causa.
Un error conocido del
booter y un resumen incorrecto del escritorio pueden parecer el mismo
"no desperto", pero requieren respuestas distintas.

## Lo que dicen los commits, por fecha

| commit | hallazgo | lo que no prueba |
|---|---|---|
| `1cce3821f`, 29-09 | `gpu reintentar` intenta deshacer un arranque GSP fallido sin reiniciar | que el descargador consiga limpiar siempre |
| `0aa3f2750`, 29-09 | FRTS se acerca al booter; el reintento puede saltar SB si el falcon no acepta cargarlo | que el momento sea la causa: el 0x15 volvio |
| `9bc0294dd`, 10-10 | segundo arranque bueno de G0, tras los cambios de Windows/apagado | diez de diez, ni una solucion definitiva |
| `a3c3ea11b` y `89eb717cc`, 13-10 | `gpu revivir` reconstruye el canal GR y corrige su promocion | recuperar un GSP cuyo firmware nunca arranco |
| `2598b7e40`, 15-10 | un segundo `gpu init` pisaba el resumen bueno; ya conserva INIT_DONE | corregir el booter de firmware |
| `2b52d25dd`, 15-10 | los reinicios repetidos se relacionan con 0x15 incluso EN FRIO; G0 abierto | que apagar o desactivar inicio rapido lo resuelva por si solo |

El caso [`EL_0x15.md`](EL_0x15.md) conserva una conclusion del 25-09 al
principio (tres buenos sin fuego/frontera), pero su seccion 10 documenta la
reaparicion del 29-09 SIN esas pruebas. No leer la cabecera como cierre
actual. La tabla G0 de PLAN_LA_3060 tiene dos buenos; no invalida los malos
posteriores del 11-10 y la aclaracion del 15-10 en EL_FOCO.

## Energia, firmware y canal: tres cosas distintas

- [MEDIDO EN EL REPOSITORIO, 28-09] `PERFIL/GPU_3060.txt` y
  `COMO_LE_HABLA_NVIDIA.md`: Windows WDDM con GSP N/A, RM en la CPU;
  BMO-X con GSP-RM 570.144 en la tarjeta. Es el perfil de esa tarde, no
  una afirmacion sobre todas las versiones de Windows o NVIDIA.
- [CODIGO] `gspreposo.rs` pide `PERF_AGGRESSIVE_PSTATE_NOTIFY` al GSP-RM
  y mira P0/P8. Es control de consumo DESPUES de tener al GSP contestando;
  no es el mismo paso que cargarlo por primera vez.
- [FUENTE EXTERNA] PCI permite gestion de energia nativa y por firmware
  de plataforma, como ACPI. D3cold implica retirar alimentacion; no se
  identifica simplemente con un P-state. Ver
  [PCI Power Management](https://kernel.org/doc/html/latest/power/pci.html).
- [FUENTE EXTERNA] NVIDIA describe el GSP como procesador al que puede
  delegar inicializacion y gestion de GPU. Ver
  [GSP Firmware, rama 570](https://download.nvidia.com/XFree86/Linux-x86_64/570.86.16/README/gsp.html).
- [LIMITE] "APIC o algo asi" no identifica la etapa que fallo. No se
  deduce una averia del gestor de energia ni una falta de AML de esa frase.
  BMO-X conserva su contrato: tablas ACPI estaticas, sin interprete AML.

## Un diagnostico del escritorio que aun mezcla causas

Antes de la correccion del 16-10, en `commands/gsp.rs`, `caliente()` devolvia
true si el motivo era 67 O si
SEC2 arranco y MAILBOX0 vale 0x15. El segundo caso no demuestra que la
tarjeta llegara caliente: esta documentado tambien en frio.

Esa funcion se usa en el fin de `save mode`, el arranque del escritorio y
el globo. `verificar::avisar_fin` recomienda apagar 15 s. Es un problema
concreto de CLASIFICACION: puede inducir reinicios sin aportar una medida
nueva. No demuestra que ese texto sea la causa del fallo del firmware.

## Como reducir las pruebas necesarias

Primero usar el informe ya guardado de UN arranque malo, no pedir diez
reinicios nuevos. Buscar en su bloque PARA PEGAR:

1. `cargador` / `al llegar`: estado previo y WPR2, no una suposicion.
2. `despierto` y `autopsia`: motivo, SEC2 MAILBOX0, GSP MAILBOX0/1,
   WPR2, tiempos y huellas antes/despues.
3. `init`, RPC y objetos: detectar un resumen incompatible con un GSP vivo.
4. `reintento`, si se ejecuto: si bajo WPR2 o si el descargador tambien dio
   0x15. El 29-09 dio 0x15 tambien: NO prometer que este mando lo arregla.
5. Xid/canal GR: solo si el GSP SI arranco. Ese es el caso de `gpu revivir`.

No indicar `gpu revivir` para un GSP que nunca arranco ni reintentar un
GSP vivo. No activar reset PCIe permanente: el ensayo `siempre` del
28-09 rompio la cadena posterior de FWSEC/WPR2 (PLAN_LA_3060).

Siguiente cambio razonable: separar el aviso de 0x15 del de llegada
caliente y conservar un parte por intento. Luego elegir UNA diferencia
medible del arranque malo frente a uno bueno. Sin esa diferencia no se
declara causa ni se cambian tiempos, voltajes o secuencias a ciegas.
