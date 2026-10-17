# MUNDO nativo -- TITAN++ y VERRANO

Juego propio de bloques de BMO-X. Abrir `run apps/mundo.bex` desde Ejecutar
en el escritorio. No usa Java ni archivos de Minecraft.

| Tecla | Accion |
| --- | --- |
| W / S / A / D | Avanzar, retroceder, desplazarse |
| Q / E | Girar 15 grados |
| I / K | Mirar arriba / abajo, hasta 75 grados |
| R / F | Subir y bajar |
| X / C | Quitar el bloque apuntado / poner tablas |
| G | Guardar los bloques |
| Raton | Mirada relativa; el puntero se oculta mientras esta capturado |
| Clic dentro | Recapturar la mirada tras soltarla o recuperar el foco |
| Primer Esc | Soltar el puntero |
| Otro Esc, con el puntero libre | Guardar cambios y cerrar; si falla, sigue abierto |

El mundo mide 16 x 8 x 16. Aun se vuela: faltan gravedad, salto, colision
del cuerpo, resaltado del bloque y texturas. Hay un limite de
80 caras visibles; los mundos mayores necesitan vertices fuera de la pila.
La mira central se publica junto a la escena y el rayo para quitar/poner
sigue tanto el giro como la inclinacion. La camara presta su componente de
orientacion a `camera.turn`, usando O1 de TITAN++ en el juego real.

`src/guardado.titan` conserva los bloques en `datos/mundo.bmv`, formato BMW1:
ocho bytes `42 4d 57 31 10 08 10 00`, despues 2048 bytes de tipos 0..6.
Se comprueba todo antes de cargar. Un fichero invalido abre el mundo inicial;
salir sin editarlo ni pulsar G conserva el fichero anterior. La camara se
reinicia al abrir. No se promete escritura atomica ante un corte electrico.

El escritorio intenta dibujar por GPU. Si la puerta rechaza el dibujo,
informa el error y usa el backend CPU de VERRANO. El mensaje `[VERRANO CPU]`
identifica ese respaldo; no demuestra aceleracion GPU ni un rendimiento dado.
`gpu laminas` y `save` separan GPU/CPU, conservan el motivo capturado al
fallar y muestran FPS con intervalo y antiguedad de la muestra. No vuelven a
atribuir un fallo pasado a un motivo global leido mas tarde.

Pruebas del anfitrion: `cargo test -p bmo-titan-x86-64 --test mundo` desde la
raiz. Ejecutan el binario TITAN++, comparan imagenes, guardan y recargan bloques,
rechazan ficheros corruptos y simulan un fallo de escritura al salir.
El propietario confirma jugar y verlo funcionar en el Ryzen el 16-10-2026.
En la segunda tanda (11:08), la consola acredita el backend CPU y el
propietario confirma que cumplen las pruebas solicitadas, incluido el
guardado. En la cuarta tanda hubo 325 fotogramas GPU prestados, cero CPU;
el juez encontro 4 pixeles distintos y esa igualdad sigue abierta. La
quinta tanda tuvo 1.141 fotogramas CPU, cero GPU por el fallo G0/0x15 y
una muestra de 19,60 FPS (20 intervalos/1.020 ms). No es una medida de RTX.
El propietario confirma los controles nuevos; los casos especificos de
limites, alcance y errores conservan sus verificaciones propias.

Control completo: [CONTROL](CONTROL.md). Datos y limites:
[parte del 16-10](../../../docs/metal/METAL_2026-10-16.md).
Siguiente orden: arranque fiable de GPU, igualdad de imagen, resaltado y
fisica. Dos capturas PNG 1080p ya se guardaron en la quinta tanda.

Orden pendiente: [PLAN_MUNDO](../../../docs/plan/PLAN_MUNDO.md).

Referencia local: [estudio de Minecraft 26.3](ESTUDIO_MINECRAFT.md), con
inventario de recursos y firmas de clases; aun no importa recursos al juego.

Reutilizacion del codigo: [objetos, composicion y traits de TITAN++](../../../toolchain/lang/titan/OBJETOS.md),
con un ejemplo de bloques comprobado en el emisor nativo.
