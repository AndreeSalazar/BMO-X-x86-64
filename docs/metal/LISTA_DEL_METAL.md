# La lista del metal

Lo que el propietario corre en la 3060 en cada corrida, EN ESTE ORDEN. Se
actualiza cuando cambia; lo que ya se midio se tacha abajo, con la fecha.

## Cada corrida (en orden)

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

## Lo medido

- 13-10 23:24: E1 en el metal: `preparar` de 11.130 us a 79..234 us; el
  kernel 1,3 ms por lote; 16 lotes por la 3060, `NO a 0`, el vigia corrigio
  0; el PRIMER computo del juego por la 3060 (1.050 us). El fotograma, 35 s.
  El censo: la puerta [1] (texturas de float) para 13 de 16 lotes.
  La lente (paso 2) NO se corrio.
