# PLAN LA LENTE BINARIA -- CABINA y LENTE hablan el binario, y lo dicen en espanol

> Escrito el **2026-10-14**. El propietario: *"mi kernel como es exclusivo en
> x86-64 ese mismo se convierte el REAL que habla binario y espanol; si
> quieres hablar con la CPU es el INTI y ese mismo te puede exprimir MAS, y
> CABINA e LENTE se convierten tus lentes en binarios"*.
>
> Y antes, el mismo dia: *"CABINA + LENTE ambos se fusionan y entiende el
> idioma BINARIO no en ASM"*.

---

## 0. La idea, y la regla que la sostiene

El ensamblador es una vista para personas. La maquina habla en bits, y el
kernel ya los lee sin pasar por texto:

```text
   el juez          las palabras de 128 bits de la 3060, bit a bit (R0..R7)
   el secuenciador  las 420 ordenes del GSP, de los bytes de gspsec.bin
   el parte         el Xid, el tipo de fallo y la VA, de los bits del GSP-RM
```

Lo que falta es FUSIONAR: que CABINA guarde lo CRUDO cuando algo pasa (la
instruccion, los registros, los contadores) y la LENTE lo explique en
espanol, hasta la linea de quien lo escribio. Para la 3060 y para el Ryzen.

La regla es la de siempre: las tablas que entienden el binario salen de lo
PUBLICO y lo VERIFICADO (open-gpu-doc, NIL/Mesa, ptxas/nvdisasm; para x86,
el SDM de Intel y el APM y el PPR de AMD) o de lo MEDIDO en el metal. Una
tabla adivinada es una lente que miente: peor que no tener lente.

---

## 1. La 3060

### [ ] LB1 -- el MAPA: de la instruccion de la 3060 a la linea del DXIL

El emisor (`bmo-proton-x-sm86`) apunta, por cada instruccion que escribe, de
que operacion del Programa salio; y el Programa, de que instruccion del DXIL.
Con eso, un canal muerto se explica entero:

```text
   el canal murio en +0x1a0: un TEX con el asa de R6, de la operacion 37,
   que es el `Sample` de la instruccion %112 del PS 9e....
```

- el mapa viaja con el `.bsf` (A9), no en la receta: el kernel no lo necesita
- por MEDIR antes: si el parte del GSP-RM (o el ESR del SM) trae el PC de la
  instruccion que fallo. Sin PC, el mapa sirve para el vigia (que lote, que
  operacion dio distinto), no para el canal muerto
- banco: un sombreador de `dxc` con un fallo puesto a proposito, y el mapa
  tiene que llevar a su linea

### [ ] LB2 -- el vigia dice DONDE, no solo QUE

Hoy el vigia dice "corrigio N". Con LB1, cuando la 3060 y la CPU den
distinto: la primera salida distinta, su operacion y su instruccion DXIL. Es
lo que convierte un "distinto" en una pieza con nombre.

---

## 2. El Ryzen (x86-64, el unico: BMO-X no tiene otra CPU)

### [ ] LB3 -- la autopsia de Ring 3 lee la instruccion que fallo

Hoy la autopsia (capitulo 7 de `save`) dice el fallo y la direccion. Con la
lente: los bytes en RIP DECODIFICADOS (prefijos, opcode, ModRM, SIB,
desplazamiento) y explicados: que instruccion, que registro tenia la
direccion mala, y de que programa y que funcion era.

- empezar por el SUBCONJUNTO que emiten INTI, TITAN++ y el enlazador de
  BMO-X: sus codificaciones ya las escribe la casa, y se verifican contra
  las suyas. Lo demas se dice "fuera de la tabla", no se adivina
- las tablas: el SDM de Intel (vol. 2) y el APM de AMD (vol. 3), publicos
- banco: cada codificacion del subconjunto, ida (el emisor) y vuelta (la
  lente) dan lo mismo

### [ ] LB4 -- los contadores del Zen 3, explicados

El informe dice hoy `sin MPERF/APERF`. El Zen 3 tiene contadores de
rendimiento (fallos de cache, saltos mal predichos, instrucciones retiradas)
documentados en el PPR de la familia 19h de AMD. CABINA los lee por programa
y la LENTE los dice en espanol: "este bucle falla la L1 una de cada 3".

- primero MEDIR que contador cuenta que en este Ryzen (el PPR dice, el metal
  confirma), como se hizo con las caches (CPUID 0x8000001D)
- solo lectura y solo en el kernel: un contador mal puesto no rompe nada,
  pero un numero mal leido engana

### [ ] LB5 -- INTI exprime con evidencia

Con LB4, la fila "lo que hay y no se coge" (AVX2, FMA, BMI1...) deja de ser
una lista y pasa a tener NUMEROS: donde un programa de INTI o TITAN++ pierde
tiempo y que extension lo cambiaria. La ley 0 de
`../maestro/OPTIMIZACION_MAESTRO.md`: correcto, medido, y solo entonces
rapido, y solo donde el numero diga.

---

## 3. Lo que NO se hace

- una lente que adivine: si una codificacion no esta en la tabla, se dice
  "no la conozco" con sus bytes en crudo
- leer contadores o la instruccion de OTRO proceso sin permiso: la lente es
  del kernel y lo que dice va al informe del propietario
- convertir la lente en un camino caliente: lee cuando algo pasa (un fallo,
  un distinto, una medida pedida), no en cada fotograma
