# M3a y la lente: estudiar los bytes sin perder su sitio

16-10-2026. Trabajo en el anfitrion; el cambio de la autopsia espera al Ryzen.

**Resultado posterior del Ryzen:** la consulta muestra paginas 0x0 y 0x40
con 128 bytes consecutivos verificados (hex = bits). Se encontro que los
rotulos y la orden siguiente recortaban sus offsets al imprimir un solo
digito; corregido a 16 digitos, aun sin observar esa correccion en el metal.
La autopsia sigue pendiente: no hubo fallo Ring 3. Detalle y datos:
[`METAL_2026-10-16.md`](../../../docs/metal/METAL_2026-10-16.md).

## Lo medido de M3a

| ordenar (600 elementos) | Antes | Ahora |
|---|---:|---:|
| Instrucciones ejecutadas | 1.186.491 | 824.134 |
| Accesos a memoria | 271.528 | 271.528 |
| Bytes de codigo | 3.625 | 3.530 |

Son 362.357 instrucciones menos (-30,54 %), no una medida de segundos.
No cambia el algoritmo de insercion: el emisor recuerda que un indice
con desplazamiento ya fue comprobado. Tras decrementar `j`, el antiguo
`j` es el nuevo `j + 1`; y si el salto demuestra `j >= 0`, tambien se
puede demostrar que `j` esta dentro. Sin ambas cotas, sigue el control.
La prueba compara toda una lista ordenada y mantiene el NO para indices
negativos y para la celda posterior al final.

## Como leer una fila

Primera fila del codigo real de ordenar, generado tras M3a en el anfitrion
(no es una captura del Ryzen):

```text
0000000000000000  48 83 ec 10 | 01001000 10000011 11101100 00010000
```

1. La primera columna es el offset del primer byte (hexadecimal).
2. `48`, `83`, `ec`, `10` son los cuatro bytes en orden de memoria.
3. Cada grupo de ocho bits es EXACTAMENTE el byte de la misma posicion.
4. Decodificados desde ese inicio, esos bytes forman `sub rsp, 16`:
   baja RSP en 16 para reservar espacio de pila. La vista cruda por si
   sola no establece que sea codigo.

Un inmediato de varios bytes conserva su orden de memoria, little-endian.
Su valor numerico se interpreta aparte; los bytes no se reordenan al imprimir.

## Estudiar el codigo real de ordenar

Desde la raiz, con un nombre de salida que no exista:

```powershell
cargo run --release -p bmo-metro -- --caliente toolchain/tools/metro/duelo/ordenar.titan target/estudio/ordenar-m3a.bin
cargo run --release -p bmo-metro -- --binario target/estudio/ordenar-m3a.bin target/estudio/ordenar-m3a.txt
```

Crear `target/estudio` antes si no existe. El primer paso guarda la region
de codigo y el fichero `.cuentas`: offset y veces que se ejecuto esa
direccion. El segundo imprime TODOS los bytes del codigo. Para un `.bex`
entero se puede dar directamente ese fichero al segundo paso: entonces los
offsets incluyen cabecera, regiones y anexos, no solo instrucciones.

El orden de direcciones permite comparar posiciones. El orden de ejecucion
incluye saltos, llamadas y repeticiones: no se deduce leyendo de arriba abajo.
Los datos y los bytes no alcanzados siguen siendo bytes; no se presentan
como instrucciones confirmadas. La lente mantiene su limite explicito:
lo que no reconoce, fuera de la tabla.

## Lo que imprime el kernel

En una autopsia: RIP, las cuatro filas crudas de su ventana de hasta 16
bytes, la instruccion decodificada y su explicacion en castellano. Una
pagina ilegible produce una ventana parcial; no se rellena inventando ceros.
La vista es compartida con el metro; falta observar esa salida en el Ryzen.
No se ha agregado un volcado de ejecutables completos al camino de admision:
la comprobacion sigue siendo una vez y fuera del camino caliente.

## Estudiarlo dentro de BMO-X, sin volver al anfitrion

En la caja de Ejecutar del DIRECTOR:

```text
dentro bits 0x0 apps/ecce/nucleo.bex
dentro bits 0x40 apps/ecce/nucleo.bex
```

La consulta abre el fichero SOLO para leer, muestra hasta 64 bytes (16 filas)
y escribe el comando de la pagina siguiente. OFFSET admite decimal o `0x`.
La ruta es todo lo que va despues del offset, incluidos sus espacios.
Se puede recorrer el fichero entero con memoria acotada: no se carga entero
ni se llena el historial de 200 filas de una sola vez. Cualquier fichero
legible sirve; no hace falta que sea un ejecutable valido.

Los offsets son del FICHERO, no direcciones de RAM ni offsets exclusivos del
codigo. `dentro apps/ecce/nucleo.bex` indica el comando para empezar en la
region de codigo: la cabecera BEF2 tambien contiene bytes, pero no son las
instrucciones de la app. La pagina no ejecuta ni decodifica lo que muestra.

EOF exacto dice fin sin inventar bytes. Despues del final, la consulta se
rechaza. Una lectura corta se muestra como INCOMPLETA, nunca como pagina
completa. Se prueba la particion de 0..257 bytes, las ultimas filas parciales
y posiciones cerca de `u64::MAX`. El DIRECTOR compila y enlaza; su guardian
de pila pasa (45.984 de 65.536 B de camino mayor, marco mayor 19.552 B).
**Falta la consulta real en el Ryzen** (la lista del foco la conserva como
pendiente). Cada consulta abre de nuevo el fichero: para comparar paginas,
usar la misma version, sin recompilarla ni sustituirla entre consultas.

## Lo que sigue faltando, en orden

1. Ver la autopsia cruda y la consulta paginada en el Ryzen: offsets,
   hexadecimal y bits deben coincidir con el mismo fichero en el anfitrion.
2. Dar cobertura a instrucciones FUERA DE LA TABLA cuando aparezcan en un
   binario real. Cada ampliacion necesita su codificacion verificada y su
   prueba; imprimir sus bits no demuestra haberla decodificado.
3. Relacionar los offsets con funcion y linea de origen cuando exista ese
   mapa. Los ejecutables que no lo traen deben decirlo, sin inventarlo.
4. Una traza en el metal para estudiar el ORDEN EJECUTADO, ademas del orden
   del fichero. Los contadores y el volcado por si solos no son esa traza.
5. La lente de la 3060: LB1/LB2 del plan binario (SASS -> Programa -> DXIL
   y la primera salida distinta). Sigue separada de x86-64 y Cyberpunk
   sigue aparcado hasta que se decida retomarlo.
