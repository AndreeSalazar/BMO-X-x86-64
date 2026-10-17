# Capturas GSP v1

Estado AMARILLO: pruebas anfitrion y compilacion no equivalen a ejecucion RTX.
El significado interno de SEC2 0x15 sigue sin verificar. No se traduce con
NV_STATUS, Xid, errores de firma ni la orden FWSEC-FRTS del mismo numero.

## Propietarios

`capture.rs` define registros y diario puro, sin dependencias ni unsafe.
`dev/gpu_capture.rs` obtiene lecturas de la 3060 ya autorizada. Director exporta
archivos privados. El ejemplo Rust `ga10x/examples/captura.rs` verifica y compara.
No agrega resets, plazos, escrituras MMIO, firmas modificadas ni otra CPU:
BMO-X sigue siendo x86-64; las herramientas estudian firmware de la GPU.

RAM fija: 32 slots de 296 bytes, dos contadores y un cierre atomico. El escritor publica
con Release al terminar; Acquire impide leer slots sin publicar. Los anteriores
son inmutables. Un escritor ocupado o un diario lleno declara perdida, no gira
ni recicla slots. Esto protege la publicacion en RAM, no vuelve atomico el MMIO.

La sesion combina fecha RTC almacenada y TSC al comenzar la primera orden FRTS.
Sobrevive a un reinicio de Director. Es identidad observacional, no UUID
criptografico ni unicidad garantizada; RTC cero indica fecha ausente. Una ruta
existente se rechaza, incluso parcial. No hay una transaccion de disco ni
creacion exclusiva entre procesos: se apoya en el propietario serializado
Director y en el sistema de archivos existente.

## Fases y validez

| Fase | Punto observado |
| --- | --- |
| 1 | Antes de comprobar/cargar FRTS; no demuestra que se ejecutara |
| 2 | Primera parada FWSEC observada, antes de la autopsia antigua |
| 3 | Antes de STARTCPU SEC2, tras las fotos previas existentes |
| 4 | Primera parada SEC2 observada para ese intento |
| 5 | Director agoto su espera existente de 5 s y pidio la foto |

Fases 1/2 llevan el siguiente intento de booter previsto; fase 3 lleva INTENTOS
incrementado. Una fase ausente no se rellena con otra de un intento previo.
La primera terminal 4/5 se conserva; una parada tardia no borra el timeout.
La consulta de vencimiento exige BOOTER_TSC y 5 s transcurridos; no cambia el plazo.

Cada foto: sesion, build OBS-BUILD con marca de sucio, intento, fase, ventana TSC,
mascaras presencia/PRI y secuencia. Campos: buzones/controles GSP y SEC2, WPR2,
BSI/GFW, HWCFG2, DMA/ENGINE SEC2, ENGINE GSP, ECC ya inventariado, PUT si hay
reserva, eventos IOMMU, G0 y BOOT_0. Direcciones de constantes GA10x existentes;
no se adivinan registros ni se lee DMEM/TRACE.

- Presencia cero: ausencia, nunca cero medido.
- Presencia uno/PRI cero: valor leido, incluido cero.
- Presencia uno/PRI uno: patron PRI con crudo conservado; no mensaje firmware.

Los PUT son u64 de RAM: no se clasifican como PRI por sus bits altos. Reserva
ausente no produce PUT medido cero. IOMMU sin incremento no prueba ausencia de
todo problema. El coste de las nuevas lecturas queda en la ventana TSC; no se
comparan duraciones antiguas como si usaran el mismo instrumental, ni se localiza
una instruccion interna a partir del intervalo CPU.

## Conjunto asociado

Al acabar `despertar`, Director copia logs una vez, comprobando reserva e intento
antes/despues. Guarda PUT en ambos extremos: su igualdad no demuestra atomicidad
DMA. Solo exporta con numero de booter y ultima foto de ese intento. Un fallo
anterior puede dejar fases solo en RAM; no se inventa un intento arrancado.
Error de exportacion queda separado del resultado GPU como falta de conjunto.

```text
informe/gpu/RTC_ALTO/RTC_BAJO/TSC_ALTO/TSC_BAJO/INTENTO/
    captura.bin
    gsplog.bin
    informe.txt
    listo.bin
```

Componentes numericos: ocho hexadecimales, compatibles con FAT32 8.3. La fila
`captura GSP` imprime la ruta real y advierte si es de un intento anterior.
Reemplaza el antiguo guardado compartido `informe/gpu/gsplog.bin`.

`captura.bin` contiene los slots publicados hasta el cierre; el lector selecciona
el intento declarado. `gsplog.bin` conserva los 196608 bytes crudos; sin reserva
no se crea y el manifiesto declara longitud cero. `informe.txt` identifica el
conjunto y sus fases/mascaras. El lector Rust muestra todos los valores en hex.
`listo.bin` se escribe ULTIMO, tras cerrar los otros archivos. Vincula identidad,
builds kernel/Director, cuenta/perdidas, ventana de copia, PUT antes/despues y
BLAKE3 de los tres archivos. Reutiliza bmo-hash; no es una firma NVIDIA.

El lector rechaza falta de cierre, medidas incorrectas, huellas distintas,
registros de otra sesion/build y terminales duplicados. `salida.txt` puede contener
observaciones posteriores (INIT_DONE): enlaza el conjunto mediante la fila,
pero no convierte todo el informe general en una instantanea del mismo momento.

Formato little-endian: 37 u64 por slot; 0 magia BMOGCAP1, 1/2 sesion, 3 build,
4 intento, 5 fase, 6/7 ventana TSC, 8 presencia, 9 PRI, 10 secuencia,
11..36 campos de `capture::NAMES`. Manifiesto: 256 bytes, 20 u64 y tres hashes
de 32 bytes (captura/logs/informe). Banderas: reserva antes/despues, PUT iguales
con reserva, intento estable, cuenta/sesion estable. No son una concesion DMA.
INFO existente: 64 cabecera, 65 slot, 66 vencimiento y 67 logs acotados. Cada
mitad de palabra viaja con TAG BMO C437CA01: una consulta fallida/ausente (cero)
rechaza la exportacion, incluso si el dato esperado era cero. No es codigo
NVIDIA. El selector 67 solo lee los tres logs, con alineacion y presencia de
marcos comprobadas; no acepta direcciones fisicas del consumidor.
No se agrega syscall ni autoridad de escritura.

## Recoger bueno y fallo con la misma build

1. Partir del commit de esta rama y comprobar que esta salida quede vacia:

   ```powershell
   git status --porcelain --untracked-files=no
   ```

   Compilar/desplegar kernel Y Director juntos UNA vez mediante COMPILAR. Si A:
   es tu unidad BMO de arranque y datos, el comando equivalente es:

   ```powershell
   .\Ultra_kernel_x86-64\build.ps1 -Todo -Drive A -SinDoom
   ```

2. En BMO-X guardar solo modo normal para el siguiente arranque:

   ```text
   gpu g0 normal
   ```

   Mantener el mismo modo de apagado/encendido entre casos. No activar riscv,
   seguido, reset o ptimer ni probar juegos para esta recogida.

3. En cada arranque nuevo dejar correr el `save mode` habitual UNA vez. Si no
   corre automaticamente, ejecutarlo una sola vez. Despues consultar/guardar:

   ```text
   save mode
   gpu control
   save
   ```

   `despertar` exporta automaticamente, bueno o malo. No repetirlo para buscar
   otra foto antes de copiar el conjunto. Verificar builds iguales y limpios,
   G0 normal, intento 1 y conjunto cerrado. Bueno: INIT_DONE observado en
   `gpu control`; cero SEC2 solo no basta. Fallo: SEC2 parado con retorno crudo
   0x15, separado del motivo BMO-X 44. No contar varios save como varios fallos.

4. En Windows localizar y copiar TODA la carpeta de cada caso a rutas privadas:

   ```powershell
   Get-ChildItem A:\informe\gpu -Recurse -Filter listo.bin |
       Select-Object -ExpandProperty DirectoryName
   ```

   Guardar bueno y fallo separados; no intercambiar sus archivos. Repetir en
   otro arranque con los MISMOS artefactos, sin recompilar entre los dos.

5. Desde la raiz del repo, usando las rutas reales de las dos copias:

   ```powershell
   cargo run -p bmo-gpu-ga10x --example captura -- C:\privado\bueno C:\privado\fallo
   ```

   Omitir la segunda ruta para inspeccionar uno. Se informa primera diferencia
   por orden de fase/campo observado, no primer evento fisico ni causa. Builds
   distintos/sucios y perdidas se declaran antes de comparar. Sin diferencias
   tampoco se demuestra igualdad de todos los estados internos.

   El lector LIBOS existente puede inventariar ELF sin diccionario supuesto:

   ```powershell
   cargo run -p bmo-gpu-ga10x --example libos -- A:\fw\gsp\gsp.bin C:\privado\fallo\gsplog.bin
   ```

   PUT cero no demuestra ausencia absoluta de ejecucion. Mantener carpetas y
   resultados locales; no adjuntarlos a una PR publica.

## Verificacion

```text
cargo test -p bmo-nvidia-gsp-control -p bmo-nvidia-libos -p bmo-gpu-ga10x
cargo test -p bmo-gpu-ga10x --example captura
./Ultra_kernel_x86-64/build/verificar.ps1 -Rapido
```

El banco prueba publicacion concurrente, reintentos, capacidad/perdidas, timeout,
ausencia/cero/PRI, exportador Director real con puerta/archivos sinteticos y
lector real con mezclas, truncaciones y diferencias. Falta probar esta
instrumentacion en la RTX con un bueno y un fallo de la misma build.

Validacion anfitrion de esta entrega: 464 casos Rust aprobados (459 entre
los tres crates y cinco del lector CLI). Incluye consulta de transporte
fallida en cabecera, slot y copia de logs, y exportacion con 32 fotos sin
truncar el informe. Kernel y Director enlazaron en release x86-64; el kernel
se construye desde su carpeta, como hace build.ps1, para cargar su linker.
El primer intento desde la raiz del workspace no cargo ese guion y fallo
al enlazar; no fue un fallo de GPU. Los 13 avisos del kernel son anteriores.
Verificar -Rapido aprobo todos los guardianes; omite el banco global de apps.
No hubo despliegue, arranque fisico, publicacion de datos ni merge.
