# VERRANO -- saber por que no se dibujo (16-10)

Un juego visible no demuestra que lo dibujo la GPU. MUNDO puede usar el
backend CPU de la misma API. Tampoco un `Vertices` demuestra que la tarjeta
rechazo algo: el escritor del paquete puede negarlo antes de la syscall.

## Consulta del escritorio

`gpu laminas` muestra la ultima app observada: tid, medida, si sigue activa,
fotogramas GPU (prestados y copiados), CPU de respaldo y su motivo. Los
contadores se reinician al cambiar de app; se conservan al cerrarla hasta
observar otra. No abre el aparato ni cambia su estado. La misma consulta
entra automaticamente en el informe maestro de `save`.

El ultimo rechazo se conserva aunque un intento posterior funcione. Dice
la fase y el error. Para `Device`, el texto del motivo se captura al fallar;
el informe no vuelve a consultar un motivo global que pudo cambiar.

## Fases y limites

- Antes del kernel: entrada, contrato BSF, destino, alineacion o paquete.
  La GPU todavia no recibio ese dibujo. Se revisa el contrato de BMO-X.
- Kernel: envio o lectura. Se conserva el codigo y el motivo de la puerta;
  no se interpreta automaticamente como un fallo del silicio.
- GPU: la escalera de finalizacion quedo incompleta. Sus etapas y los
  avisos del GSP son la evidencia siguiente; no se recomienda un reinicio
  solo por el nombre de esta fase.
- Respaldo CPU: cabecera, vertices, medida o memoria. Un fallo repetido
  igual se dice una vez; un motivo distinto vuelve a mostrarse. Una ranura
  ocupada o sin publicacion sigue siendo espera normal, no un error.

Los detalles de la tarjeta permanecen en el adaptador del escritorio
`gspcubo/sm86.rs`. No se cambia `Error` ni `Backend` ni el ABI de la lamina.
`check` calcula la medida con multiplicacion comprobada: una medida que
desborda devuelve `Image` antes de acceder o limpiar el bufer.

## Prueba siguiente en el metal

Reconstruir el kernel con el arreglo 640x360/Z y el DIRECTOR con este
diagnostico. Abrir MUNDO, mover la camara y ejecutar `gpu laminas`, luego
`save`. Buscar fotogramas GPU y el juicio Q0a3: los contadores no sustituyen
la comparacion de pixeles. Una tanda basta para observar el resultado.
El despertar intermitente G0/0x15 sigue abierto por separado.

## Verificacion del anfitrion

459 pruebas pasan entre VERRANO (25), su trama (44) y el driver (390);
tambien las 9 de MUNDO. El caso de dimensiones enormes devuelve `Image`
sin tocar los pixeles. DIRECTOR compila en release y pasa la pila de Ring 3:
45.984 de 65.536 bytes. Pasan ASCII, enlaces, capas, censo modular y la
separacion del adaptador GPU. Falta ver los mensajes y contadores en metal;
no se genero una imagen de arranque ni se acredita aceleracion nueva.

[Plan y metas](../../../docs/plan/PLAN_VERRANO.md).
