# VERRANO -- saber por que no se dibujo (16-10)

## Lente de bytes preparada

`gpu verrano ordenes [offset decimal]` agrega la plantilla NVIDIA de un
triangulo: paquetes, metodos, categorias y motivos; paginas de 32 bytes.
La lectura empieza desde cero para conservar contexto aunque se muestre
otra pagina. [Separacion y limites](../../drivers/gpu/ga10x/INSPECCION_NVIDIA.md).

`gpu verrano bytes vs|ps|bmox-vs|bmox-ps [offset decimal]` muestra paginas
de 64 bytes, hex y bits, sin abrir ni despertar la tarjeta. Los primeros
128 bytes son SPH; despues se muestran los campos SM86 reconocidos. La
salida identifica PREPARADO CPU: no es lectura de VRAM ni prueba de envio.
[Uso, aislamiento, guardianes y pendientes](../bmo-lente-gpu/README.md).

Un juego visible no demuestra que lo dibujo la GPU. MUNDO puede usar el
backend CPU de la misma API. Tampoco un `Vertices` demuestra que la tarjeta
rechazo algo: el escritor del paquete puede negarlo antes de la syscall.

## Consulta del escritorio

`gpu laminas` muestra la ultima app observada: tid, medida, si sigue activa,
fotogramas GPU (prestados y copiados), CPU de respaldo y su motivo. Los
contadores se reinician al cambiar de app; se conservan al cerrarla hasta
observar otra. No abre el aparato ni cambia su estado. La misma consulta
entra automaticamente en el informe maestro de `save`.

Incluye FPS de ventana medidos sobre intervalos entre dibujos nuevos
aceptados (GPU+CPU), medida en ms y antiguedad de la muestra. Necesita un
segundo de actividad; una pausa de mas de un segundo reinicia el intervalo.
No es el refresco del monitor ni el limite teorico del aparato.

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

El arreglo 640x360/Z ya produjo 325 fotogramas GPU prestados y cero CPU en
la cuarta tanda del 16-10. El juez difirio en 4/230.400 pixeles. La quinta
tanda, 20:36, volvio a CPU porque el GSP no arranco: 1.141 fotogramas CPU,
cero GPU, muestra 19,60 FPS sobre 20 intervalos/1.020 ms. El informe
conserva la antiguedad; no es media de la partida ni rendimiento de RTX.

La prueba siguiente necesita kernel y DIRECTOR con la foto GSP ANTES del
booter. Uso normal de MUNDO, `gpu laminas` y `save`, salga bien o mal G0.
Si la GPU esta disponible, conservar tambien Q0a3; los contadores no
sustituyen la comparacion de pixeles. No insistir con reintentar cuando
el descargador ya dejo WPR2 protegida. No se piden reinicios en serie.
[Parte y limites](../../../docs/metal/METAL_2026-10-16.md).

## Verificacion del anfitrion en la implementacion inicial

459 pruebas pasan entre VERRANO (25), su trama (44) y el driver (390);
tambien las 9 de MUNDO. El caso de dimensiones enormes devuelve `Image`
sin tocar los pixeles. DIRECTOR compila en release y pasa la pila de Ring 3:
45.984 de 65.536 bytes. Pasan ASCII, enlaces, capas, censo modular y la
separacion del adaptador GPU. Es el banco de esa implementacion, no un
recuento actualizado de todo el proyecto. El diagnostico GPU/CPU y FPS
ya aparece en los informes del 16-10; la foto previa del booter es posterior
y conserva su prueba de metal pendiente.

[Plan y metas](../../../docs/plan/PLAN_VERRANO.md).
