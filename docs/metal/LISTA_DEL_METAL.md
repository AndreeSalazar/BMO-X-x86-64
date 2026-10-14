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

## Que mirar en la corrida siguiente (14-10, tarde)

- La textura HDR: la ranura de textura mide 32 MiB (era 16). "no cabe en
  su ranura" ya no tiene que salir para la de 1280x720 con 5 mips.
- El censo dice ahora `la puerta paro N lote(s) al prepararlos`, con cada
  motivo y sus veces: los que antes faltaban entre "irian" y "de verdad".
  Lo que salga mas arriba es lo siguiente que hay que abrir.
- `iommu` en el escritorio: `tablas N de 256` (eran 128).

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
