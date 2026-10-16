# MUNDO nativo -- TITAN++ y VERRANO

Juego propio de bloques de BMO-X. Abrir `run apps/mundo.bex` desde Ejecutar
en el escritorio. No usa Java ni archivos de Minecraft.

| Tecla | Accion |
| --- | --- |
| W / S / A / D | Avanzar, retroceder, desplazarse |
| Q / E | Girar 15 grados |
| R / F | Subir y bajar |
| X / C | Quitar el bloque apuntado / poner tablas |
| G | Guardar los bloques |
| Esc | Guardar cambios y cerrar; si falla, sigue abierto |

El mundo mide 16 x 8 x 16. Aun se vuela: faltan gravedad, salto, colision
del cuerpo, raton relativo, mirar arriba/abajo y texturas. Hay un limite de
80 caras visibles; los mundos mayores necesitan vertices fuera de la pila.

`src/guardado.titan` conserva los bloques en `datos/mundo.bmv`, formato BMW1:
ocho bytes `42 4d 57 31 10 08 10 00`, despues 2048 bytes de tipos 0..6.
Se comprueba todo antes de cargar. Un fichero invalido abre el mundo inicial;
salir sin editarlo ni pulsar G conserva el fichero anterior. La camara se
reinicia al abrir. No se promete escritura atomica ante un corte electrico.

El escritorio intenta dibujar por GPU. Si la puerta rechaza el dibujo,
informa el error y usa el backend CPU de VERRANO. El mensaje `[VERRANO CPU]`
identifica ese respaldo; no demuestra aceleracion GPU ni un rendimiento dado.
El ultimo motivo del kernel es global: es diagnostico, no prueba por si solo
de la causa de un fotograma si otro trabajo pudo actualizarlo.

Pruebas del anfitrion: `cargo test -p bmo-titan-x86-64 --test mundo` desde la
raiz. Ejecutan el binario TITAN++, comparan imagenes, guardan y recargan bloques,
rechazan ficheros corruptos y simulan un fallo de escritura al salir.
El propietario confirma jugar y verlo funcionar en el Ryzen el 16-10-2026.
No identifico el backend usado; falta comprobar persistencia al cerrar/abrir
y el camino GPU con su diagnostico.

Orden pendiente: [PLAN_MUNDO](../../../docs/plan/PLAN_MUNDO.md).
