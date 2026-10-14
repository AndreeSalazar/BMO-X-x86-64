# La lista del metal

Lo que el propietario corre en la 3060 en cada corrida, EN ESTE ORDEN. Se
actualiza cuando cambia; lo que ya se midio se tacha abajo, con la fecha.

## Cada corrida (en orden)

Donde queda cada cosa: lo que se TECLEA y su respuesta (`gpu verrano ...`)
va al capitulo 1 de `save` (`informe/SESION.TXT`), y cada `save` guarda solo
lo de despues del `save` anterior; lo que ESCRIBE un programa (`proton-x
--prueba-computo`, el juego) va a `informe/ESCRITO.TXT`, y ahi queda solo el
ULTIMO programa. Por eso: un `save` justo despues de los pasos 1-3, ANTES de
abrir el juego. Las copias de cada `save`, con fecha, en
`informe/historial/`. Dos `save` en el mismo minuto ya no se pisan (14-10):
el segundo va a `..._HH-MM_b.txt`. El 14-10 07:14 el `save` a mano (con
las lentes) y el automatico del `reboot` cayeron en el mismo minuto y el
automatico se lo comio: esas lentes hay que correrlas otra vez.

0. `smp all`, ANTES de nada (con un nucleo, el juego y el escritorio se
   pelean el mismo).
1. `gpu verrano textura`: los 96 puntos IGUAL (que nada nuevo rompio lo de
   antes).
2. `gpu verrano derivadas`: LA LENTE. Que la gruesa diga `ddx(x)` y `ddy(y)`
   1, y lo cruzado 0; y el signo de FSWZADD. Pendiente: NUNCA se ha corrido
   (lo pidio la seccion 13 de `METAL_2026-10-13.md`). De ella depende que el
   `Sample` con mips mire los vecinos buenos del cuadro.
3. `proton-x --prueba-computo`: PERFECTO.
4. Cyberpunk con `personal 3060`, sin escalera: la salida y `informe/`.
   Mirar: `preparar` en us, cuantos lotes dibuja la 3060, `el kernel dijo
   NO a 0`, el vigia `corrigio 0`, el censo (que puerta para mas lotes), los
   computos por la CPU y la imagen.
5. Si el canal muere: `gpu aviso`, despues `gpu revivir`, y el parte entero.
6. `save`, y traer tambien `proton-x/cyberpunk2077/rechazados/` de ESTRATOS.
7. (Opcional, para VER la 3060 sola) Cyberpunk con `personal solo-gpu`: lo
   que la 3060 no sabe NO lo pinta la CPU, se SALTA. La imagen sale
   incompleta. Mirar el fotograma (`[registro] ... fotograma N ms`) y la
   linea del censo `SOLO GPU: N lote(s) y M computo(s) SALTADOS`: eso es lo
   que falta, contado.

## Lo que falta para que la 3060 lo haga TODO (14-10)

Por que tarda: el 14-10 09:37, de 33,6 s de fotograma, 33,4 s eran la CPU
pintando los POCOS lotes que no van a la 3060 (pases de pantalla entera con
sombreadores pesados, pixel a pixel). La 3060 hace los suyos en ms. En
orden de lo que mas da:

1. EL BACK BUFFER (los 2 lotes que bloquean la pantalla directa): el `Load`
   sin muestreador ya va (14-10); si sigue "s0 sin atar", ese pase muestrea
   y la casa no encuentra su muestreador.
2. LOS ULTIMOS LOTES DEL MENU: mas de 64 registros vivos, instancias, la
   mezcla con triangulos que se pisan, el render target de 16x1 (filas que
   no van de 128 en 128 B). Una pieza cada uno.
3. LOS COMPUTOS QUE NI CORREN (~13 en la corrida de las 09:37): referencia
   hacia delante fuera de un phi, arrays de structs, conversiones de 16/64
   bits, RawBuffer de 16/64 bits, un puntero sin load; y dos PS con un grafo
   que no se estructura. Sus Dispatch NO se hacen (ni la CPU): la imagen
   puede salir incompleta o negra. Se hacen contra su DXIL (`rechazados/`).
4. JUGANDO (no en el menu): miles de lotes por fotograma. Cada uno paga
   ~2..4 ms de puerta y kernel: el ENVIO RAPIDO (muchos lotes por llamada) y
   las imagenes en la VRAM en vez de en la RAM.

`personal solo-gpu` (14-10) es el modo de VER lo que la 3060 ya hace sola:
no arregla nada de lo de arriba, lo salta. El juez de la 3060 es el mismo.

## Que mirar en la corrida siguiente (14-10, tarde)

- La textura HDR: la ranura de textura mide 32 MiB (era 16). "no cabe en
  su ranura" ya no tiene que salir para la de 1280x720 con 5 mips.
- El censo dice ahora `la puerta paro N lote(s) al prepararlos`, con cada
  motivo y sus veces: los que antes faltaban entre "irian" y "de verdad".
  Lo que salga mas arriba es lo siguiente que hay que abrir.
- `iommu` en el escritorio: `tablas N de 256` (eran 128).
- LOS SOMBREADORES QUE NO SE SABEN CORRER (M1): cada uno se guarda en
  ESTRATOS, `proton-x/cyberpunk2077/rechazados/`, como vino del juego
  (`<huella>_cs.dxil`, `_vs`, `_ps`) con su porque (`<huella>.txt`), y
  `INDICE.TXT` los lista TODOS con cada porque y cuantos lo tienen. Traer
  esa carpeta entera: con el DXIL real, `dxc -dumpbin` dice la instruccion
  exacta y cada pieza se hace contra eso, sin adivinar.
- LA PANTALLA DIRECTA (la 3060 a pantalla completa, Z1): ningun metal de
  Cyberpunk la ha pedido nunca. Se pide tras el primer lote del BACK
  BUFFER (1280x720) que dibuja la 3060, y se suelta para siempre si uno del
  back buffer va por la CPU. El censo dice ahora `al back buffer: N
  lote(s), M por la 3060` y por que fue cada uno por la CPU. Si N sale 0,
  el juego no dibuja el back buffer con lotes: lo escribe un computo o una
  copia, y la pieza que falta es esa.

## Que mirar en la corrida siguiente (14-10, noche)

- Antes de abrir el juego: `smp all` (09:37 corrio con `solo el BSP` y la
  CPU al 100 %: el escritorio y el juego en un nucleo). Con el juego
  corriendo, lo tecleado ya es una ORDEN (el juego no lee su consola).
- El back buffer: `al back buffer: N lote(s), M por la 3060`. Si M pasa de
  0, el ultimo pase era un `Load` y ya va; si sigue "t0 con s0 y el lote no
  tiene s0 atado", ese pase MUESTREA y la casa no encuentra su muestreador
  (y la CPU lo pinta a CEROS: la pantalla negra). Eso se mira despues.
- La carpeta `proton-x/cyberpunk2077/rechazados/` de ESTRATOS (NO el
  `informe/INDICE.TXT`, que es el indice del informe): sus `.dxil`, sus
  `.txt` y su `INDICE.TXT`.

## Lo medido el 14-10 09:37

- El `%` de floats (C1) ya no sale: ese CS se sabe correr. El `save` de
  antes del `reboot` fue a `_b` y no piso el bueno.
- 21..29 lotes por la 3060, `NO a 0`, el vigia corrigio 0. Los 2 del back
  buffer, por la CPU: "t0 con s0 sin atar". Fotograma 33,6 s.
- Siguen sin correr: CS con referencia hacia delante fuera de un phi (x3),
  arrays de structs (x2), conversiones de 16/64 bits (x4), RawBuffer de
  16/64 bits (x3), un puntero sin load (x1); dos PS con un grafo que no se
  estructura (guardados en rechazados/).

## Que mirar en la corrida anterior (14-10, F1)

- F1, las texturas de float: en el censo, la puerta "una textura de float"
  ya no tiene que salir (si sale "con un muestreador de BORDE", es la que
  queda). Cuantos lotes mas dibuja la 3060 y que la imagen se vea bien: un
  filtro mal hecho se ve con escalones o manchas, no cuelga.
- El tiempo del fotograma (`[registro] ... fotograma N ms`): con esos lotes
  en la 3060 tiene que bajar mucho de 35 s.

## Lo medido

- 13-10 23:24: E1 en el metal: `preparar` de 11.130 us a 79..234 us; el
  kernel 1,3 ms por lote; 16 lotes por la 3060, `NO a 0`, el vigia corrigio
  0; el PRIMER computo del juego por la 3060 (1.050 us). El fotograma, 35 s.
  El censo: la puerta [1] (texturas de float) para 13 de 16 lotes.
  La lente (paso 2) NO se corrio.
- 14-10 07:14: F1 en el metal: la puerta "textura de float" desaparecio del
  censo; irian 84..90% de los lotes (antes 52%). El fotograma, 35,8 s ->
  27,9 -> 24,7 s. Pero la 3060 dibujo 15 de los 32 que el censo da por
  buenos: los que faltan los para la PUERTA y el censo no los cuenta (una
  HDR de 1280x720 con 5 mips en float, ~20 MiB, "no cabe en su ranura de 16
  MiB"; "t0 con s0 sin atar"). Un lote de 12 ms de 3060 (un pase a pantalla
  entera con el filtro de float). `NO a 0`, el vigia corrigio 0. El primer
  computo, 465 us. La lente, `verrano textura` y `--prueba-computo`, sin
  correr.
