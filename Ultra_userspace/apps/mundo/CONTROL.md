# MUNDO -- mirada libre y medida de FPS (16-10)

## Jugar

Al abrirse con foco MUNDO solicita capturar el raton. El DIRECTOR oculta
el cursor y entrega desplazamientos HID sin recorte al borde del panel.
La mira blanca del juego sigue visible. Esc suelta el puntero; otro Esc
guarda los cambios y sale. Clic izquierdo dentro vuelve a solicitar captura.
Alt+Tab, minimizar, cerrar, abrir el conmutador o iniciar un recorte revocan
la captura; al recuperar el foco hace falta otro clic. Los atajos del
escritorio conservan su prioridad. No se entrega el dispositivo fisico.

La captura retira tambien el cursor YA pintado, incluso sin mover el raton:
no basta con dejar de dibujarlo en el siguiente fotograma. Al salir vuelve
el cursor normal; sus botones se siguen contabilizando durante la captura
para no inventar un clic al liberarla.

## La biblioteca chica

`src/pointer.titan` es un modulo reutilizable por `mod pointer`: begin,
release, active y motion. No requiere clases base ni un runtime nuevo.
Se puede llevar a otra app de TITAN++ que use ventana y eventos del DIRECTOR.
Dos primitivas nuevas conectan la solicitud y su concesion:
`director.raton_captura(bool)` y `director.raton_capturado()`. Una solicitud correcta no significa
que el escritorio ya la concedio.

Evento 7 entrega dx/dy con signo por `director.raton_x/y`. La sensibilidad
es 0,3 grados por cuenta HID; Y hacia arriba eleva la vista. `camera.titan`
guarda orientacion en decimas de grado, interpola su tabla de senos y limita
pitch a +/-75 grados. Teclado y rayo usan esa misma orientacion. Q/E e I/K
conservan los saltos de 15 grados para teclado.

## FPS que se pueden afirmar

El informe de las 19:37 cuenta 325 fotogramas GPU pero no su duracion:
no permite calcular FPS. `gpu laminas` y `save` incorporan ahora una muestra
de intervalos entre fotogramas nuevos aceptados por VERRANO, con su tiempo
real y antiguedad. No es el refresco del monitor ni el maximo del hardware.
Sin un segundo de actividad suficiente lo dice; no inventa 60 FPS. Una
pausa de mas de un segundo reinicia la ventana de medida y conserva la
ultima muestra con su antiguedad. El reloj se consulta al kernel una vez;
por fotograma solo se lee el TSC y se cuentan intervalos.
FRAPS-X toma ahora la secuencia de la fuente VERRANO de la ventana cuando
existe: la cabecera BSUP no cambia al publicar una lamina, y contar solo esa
cabecera ocultaba los fotogramas del juego.

## Comprobar en el Ryzen

Reconstruir kernel, DIRECTOR y MUNDO: el kernel agrega la lectura 0x07 de
contadores HID, que no consume ni cambia el cursor. Mover el raton durante
varios segundos y contra el borde del panel; no debe frenarse el giro.
Probar Esc, clic, Alt+Tab, cierre y recorte: el puntero debe reaparecer.
Consultar `gpu laminas` despues de mover y traer `save` y una captura.
Tanda 16-10 20:36: el propietario confirma los controles; el informe mide
19,60 FPS en respaldo CPU (20 intervalos/1.020 ms). La RTX no arranco en esa
sesion. Dos PNG 1080p guardados. Esto no mide la velocidad del camino GPU.

## Verificacion del anfitrion de esta implementacion

Pasan las suites de TITAN++ (frente y emisor), ABI, entrada, reparto del foco
y VERRANO; en particular 12 casos de entrada y 10 del BEX de MUNDO.
El metro de 72 programas conserva 26.203.892 instrucciones, 4.943.814 accesos
y 459.777 bytes de codigo. El decodificador relativo solo se emite para
programas que usan las primitivas de captura; las otras apps no lo pagan.
Kernel pasa cargo check y MUNDO genera un BEX de 77.598 bytes. DIRECTOR
compila en release. En la quinta tanda el propietario confirma los controles
y el informe registra sus FPS por CPU; no es una validacion de todas las
variantes de foco ni del rendimiento de la RTX.

El banco del anfitrion prueba movimiento con signo, solicitud/concesion,
liberacion inmediata, conserva bits del DIRECTOR y ejecuta el BEX de MUNDO:
mirada ida/vuelta, mira central y Esc antes de cerrar. El reparto puro prueba
cambio de foco, vuelta de contadores y buzon lleno sin perder deltas.

Nota L6d: read_u8_pub pasa del despachador del emulador a paginas.rs sin
cambiar sus bytes; SHA256 b80ae81baa56997cffa5dad2fa678e7b8b2df5994f642129c0ef5e93a0c30f7f.
El DIRECTOR de prueba tambien corrige TOMADA: va en el estado alto del
buzon, no en el campo de coordenadas. La concesion de raton se habilita
explicitamente en los casos que la necesitan.

[Metas](../../../docs/plan/PLAN_MUNDO.md).
