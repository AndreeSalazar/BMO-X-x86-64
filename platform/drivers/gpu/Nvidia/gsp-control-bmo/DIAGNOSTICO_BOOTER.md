# Booter: hacer explicito el retorno sin inventar su significado

17-10-2026. Problema: un `0x15` aislado no identifica el componente que lo
produjo ni explica por que fallo. `gpu control` ya mostraba motivo y buzon;
esta ampliacion interpreta su disponibilidad y conserva sus dominios.
No cambia la secuencia de arranque ni hace una nueva prueba en la RTX.

## 1. Tres dominios que no deben mezclarse

| Dato | Origen comprobado en BMO-X | Significado respaldado |
| --- | --- | --- |
| SEC2 MAILBOX0 `0x15` | `gpu_despertar::acabar`, despues de observar SEC2 parado | Retorno no cero del booter; causa interna aun no verificada |
| Motivo BMO `44` / `0x2C` | `IOMMU_NO_BOOTER_MAL`, rama `m0 != 0` de `acabar` | El kernel registro ese rechazo; no es el codigo NVIDIA 0x15 |
| GSP MAILBOX0 `0xBADF1002` | Lectura cruda guardada en `autopsiar` | Patron de error PRI segun `ga10x::es_error_pri`; no es un mensaje GSP |

Referencia externa fijada: [OpenRM 570.144, s_executeBooterUcode_TU102](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/8ec351aeb96a93a4bb69ccc12a542bf8a8df2b6f/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_booter_tu102.c).
La funcion distingue el status del ejecutor y los dos buzones. Si el ejecutor
acaba sin error pero MAILBOX0 no es cero, devuelve `NV_ERR_GENERIC`.
Esa rama no traduce 0x15 a un motivo interno. No usar una tabla de NV_STATUS,
Xid o errores de shaders para darle nombre por coincidencia numerica.

## 2. Diagnostico agregado

`booter::diagnose` recibe hechos, sin acceder al dispositivo. Distingue:
datos cambiantes o ausentes; arranque no registrado; parada no observada;
lectura no interpretable; contradiccion; buzon final cero o no cero.
La disponibilidad PRI la aporta el adaptador con el lector GA10x existente.

Antes de interpretar el retorno se exige parada observada. Cero no equivale
a INIT_DONE ni a salud actual. Si el kernel conserva rechazo y la lectura
viva ahora es cero, se muestra contradiccion: no se borra el fallo.
El 0x15 conserva su numero y "significado interno no verificado".

Director delimita la consulta con `INFO_GPU_DESPIERTO` y ahora tambien con
`INFO_GPU_INTENTO`. Palabras iguales con intento distinto ya no pasan por
foto estable. Es una comprobacion parcial, no atomicidad: cambios intermedios
pueden no observarse y no se crea una historia de eventos.

## 3. Autopsia sin sello de intento

`gpu_despertar::autopsiar` retorna si `AUTOPSIA[1]` ya es no cero: conserva
la primera parada observada del arranque. `INTENTOS`, en cambio, aumenta
cuando se suelta otro booter. La foto no contiene ese contador.

Se puede atribuir la foto al intento 1 si ese sigue siendo el actual y la
consulta no cambio. Con contador mayor, ausente o cero, la relacion queda
SIN ATRIBUIR; no se combinan los registros de esa foto con el retorno vivo
como si fueran el mismo instante. No se conoce necesariamente en que intento
se tomo la primera foto si la consulta inicial ocurrio mas tarde.

Esta es una mejora de la evidencia, no una explicacion causal del fallo GPU.
El siguiente cambio de captura, si hace falta, debera sellar cada foto desde
el propietario Ring 0 y definir su publicacion/coherencia. Esta entrega no
modifica esa estructura ni agrega selectores a la ABI.

## 4. El reloj no localiza una instruccion del firmware

En `soltar`, `BOOTER_TSC` se escribe antes de incrementar el intento, tomar
la foto previa y `fotografiar_honda`, y antes de llamar a `arrancar_con`.
En `autopsiar`, los microsegundos se calculan cuando la CPU observa parada.
El intervalo contiene trabajo previo al STARTCPU y demora de observacion.
Ademas se limita a 31 bits al publicarlo. No es tiempo interno exclusivo de
ejecucion del booter ni marca exacta de su instruccion final.

Por eso el informe lo llama intervalo CPU observado. Una diferencia de unos
0,7 ms entre sesiones no localiza un chequeo de firma, WPR o liberacion GSP.
Para localizarlo hace falta evidencia independiente que distinga esos pasos.

## 5. Verificacion y limite de entrega

```text
cargo test -p bmo-nvidia-gsp-control
python3 toolchain/tools/la-3060/la_3060.py --check
```

Validacion anfitrion con rustc 1.75: 16 pruebas del crate y seis del adaptador
real de Director incluidas en `tests/informe_host.rs`. Ese banco sustituye
la puerta y la salida por datos sinteticos y comprueba las constantes contra
el runtime. No ejecuta syscalls, GPU ni Director completo. Tambien se compila
la biblioteca `no_std` sin dependencias. Los guardianes conservan S8.

La compilacion completa con el nightly del proyecto, el coste/pila del
Director resultante y la observacion de las filas en metal siguen pendientes.
Se entrega como PR de borrador. Para la prueba fisica usar el build revisado,
mantener el modo de arranque, y guardar cada `gpu control`/`save` con identidad.
El campo nuevo no promete eliminar 0x15; hace explicito lo conocido y lo que
la captura actual no puede decidir.
