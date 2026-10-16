# Lente cruda -- tanda pendiente del 16-10

**Estado actualizado: CONSULTA VISTA en el Ryzen el 16-10; un fallo de
rotulo encontrado.** Evidencia y resultado completo en
[`METAL_2026-10-16.md`](METAL_2026-10-16.md). La nueva autopsia sigue sin
observarse porque no hubo fallo de Ring 3. La lista de abajo conserva lo
que se pidio antes de la corrida.

## Consulta, solo lectura

1. `dentro apps/ecce/nucleo.bex`: ver la region de codigo y copiar la orden
   `codigo crudo: dentro bits ...` que imprime. Ese offset pertenece al
   FICHERO, mientras la entrada mostrada es relativa al CODIGO.
2. `dentro bits 0x0 apps/ecce/nucleo.bex`: cabecera del fichero; 16 filas
   como maximo, cada byte con dos cifras hex y OCHO bits. No se debe
   confundir la cabecera con instrucciones.
3. Repetir la orden `siguiente` que imprime. La segunda pagina empieza
   en 0x40, sin repetir ni saltar un byte. Pedir el comienzo del codigo
   con la orden del paso 1 y compararlo con el MISMO fichero en el host
   (`metro --binario fichero.bex salida.txt`). No comparar con otro build.
4. Con un fichero de medida conocida: pedir exactamente su final (fin,
   cero filas) y una posicion posterior (rechazo, sin lectura). Si su
   ultima pagina es parcial, no debe rellenar la cola con ceros.
5. Fotografiar las paginas: la consulta no las exporta a disco. `save`
   guarda la sesion pero no sustituye esa evidencia visual de los bytes.

## Autopsia

Con una app de prueba que ya tenga un fallo controlado de Ring 3, recoger
la autopsia: RIP, hasta 16 bytes crudos con direcciones crecientes y despues
`instr` y `dice`. No provocar un fallo de Ring 0. Si no se pudieron capturar
todos los bytes, solo se imprimen los disponibles. Comparar el hex con los
bits de la MISMA fila y guardar el informe con `save`.

## Resultado

- Consulta y avance: VISTO 16-10: paginas 0x0 y 0x40, 128 bytes consecutivos,
  hex y bits iguales. El rotulo y `siguiente` recortaban sus offsets a un
  digito; CORREGIDO EN EL ANFITRION, falta observar el rotulo corregido.
- EOF y ultima pagina parcial: PENDIENTE.
- Coincidencia con el MISMO binario del anfitrion: PENDIENTE.
- Autopsia, direcciones y ocho bits por byte: PENDIENTE.

Anotar la fecha real, el build, la foto y el informe al ejecutar la tanda.
Lo que no se observe permanece pendiente.
