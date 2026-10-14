# PLAN LA LENTE BINARIA -- CABINA y LENTE hablan el binario, y lo dicen en castellano

> Escrito el **2026-10-14**. El propietario: *"mi kernel como es exclusivo en
> x86-64 ese mismo se convierte el REAL que habla binario y castellano; si
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
castellano, hasta la linea de quien lo escribio. Para la 3060 y para el Ryzen.

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

> 14-10, el propietario: *"mi BMO-X Lente ENTIENDE el idioma solo por ahora
> en AMD; si llega INTEL lo hago, pero ahora es solo con AMD"*. La lente es
> del Zen 3 (familia 19h, el Ryzen 5 5600X): sus tablas salen del APM de AMD
> (las instrucciones) y del PPR de la familia 19h (los contadores y los
> MSR). Intel, cuando haya un Intel en la mesa: sus tablas seran OTRAS,
> medidas en SU metal, y no se mezclan con estas.

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
y la LENTE los dice en castellano: "este bucle falla la L1 una de cada 3".

- primero MEDIR que contador cuenta que en este Ryzen (el PPR dice, el metal
  confirma), como se hizo con las caches (CPUID 0x8000001D)
- solo lectura y solo en el kernel: un contador mal puesto no rompe nada,
  pero un numero mal leido burla

### [ ] LB5 -- INTI exprime con evidencia

Con LB4, la fila "lo que hay y no se coge" (AVX2, FMA, BMI1...) deja de ser
una lista y pasa a tener NUMEROS: donde un programa de INTI o TITAN++ pierde
tiempo y que extension lo cambiaria. La ley 0 de
`../maestro/OPTIMIZACION_MAESTRO.md`: correcto, medido, y solo entonces
rapido, y solo donde el numero diga.

### [ ] LB6 -- la cabecera del `.bex` dice EXACTAMENTE que instrucciones usa

Hoy BEF2 declara `xcr0` (los estados XSAVE que el programa usa) y la puerta
(`bmo-bex-gate`) rechaza lo que el kernel no preserva. El paso siguiente: un
anexo de REQUISITOS preciso -- las extensiones de AMD que el codigo usa, una
por bit de CPUID (SSE2, AVX2, FMA, BMI1, POPCNT...) --, escrito por quien
emite el codigo:

```text
   INTI / TITAN++   cada instruccion que emite sabe de que extension es:
                    el anexo sale SOLO, sin que nadie lo escriba a mano
   el enlazador     junta los de cada `.bo` (la union)
   la puerta        lo compara con el CPUID del Ryzen: lo que no tenga, NO
                    se admite, con su nombre (como `xcr0` hoy)
```

### [ ] LB7 -- la lente comprueba que la cabecera no miente (el juez de x86)

Como el juez de la 3060: al admitir un `.bex`, la lente DECODIFICA su region
de CODIGO con las tablas del APM (LB3) y comprueba que cada instruccion es de
una extension que la cabecera declara. Una instruccion que no sabe leer, o
de una extension no declarada: NO, con su direccion y sus bytes.

- empieza por los `.bex` de INTI y TITAN++ (su subconjunto lo conoce: LB3);
  un `.bex` con codigo de fuera de la tabla se dice "fuera de la tabla",
  sin adivinar
- el x86 es de largo variable: decodificar desde la ENTRADA y los saltos
  conocidos, no a ciegas desde el byte 0 (los datos metidos en el codigo
  burlarian a una lectura lineal); lo que no se alcance, se dice
- coste: una vez por `.bex` al admitirlo (y recordado por su hash), nunca
  en el camino caliente

---

## 3. Verificar una vez, y apartarse

> 14-10, el propietario: *"que hable con mi Kernel que primero verifica con
> respeto, luego se aparta y le habla a mi CPU (AMD) en binarios precisos"*.

Lo que ya hacen otros, y lo que BMO-X ya hace con la 3060: eBPF en Linux (un
verificador antes de cargar, y luego a toda velocidad), Native Client de
Google (un subconjunto de x86 validado, y luego directo), y el juez de la
3060 (R0..R7 una vez, y la receta en caliente).

### [ ] LB8 -- lo DEMOSTRADO al admitir se apunta, y se paga una vez

Lo que LB7 demuestra de un `.bex` (que extensiones usa, que su codigo no
salta fuera de su region, que no toca lo que no declara) se apunta con su
hash. La siguiente vez que se lance, el kernel lo sabe sin volver a leerlo:
verifica con respeto UNA vez, y luego se aparta. Ningun coste en el camino
caliente.

### [ ] LB9 -- propiedades que se pueden demostrar en el binario

Una lente que lee el binario puede comprobar lo que casi nadie comprueba.
Cada una, una pieza con su banco, y solo cuando un programa la PIDA en su
cabecera:

```text
   tiempo constante  ningun salto ni acceso depende de un secreto (bmo-cripto:
                     AES, GCM, SHA-256; lo que evita robar claves midiendo)
   tiempo real       un bucle sin caminos que se disparen (el audio, que no
                     llegue tarde una trama)
   sin escapes       nunca se salta fuera del codigo propio
```

---

## 4. El perfil del Ryzen como LIBRERIA

> 14-10, el propietario: *"mi PERFIL de Ryzen 5 5600X se convierte en
> libreria 'CPU: ryzen 5 5600X' ... datos que emitio el CPU, y el kernel ya
> sabra que hacer porque es su biblioteca"*.

Ya hay dos mitades: `PERFIL/CPU.txt` (lo declarado; su tabla de EXTENSIONES
ya la lee una maquina, y `perfil-campos` la compara con el codigo) y
`cpu_vendor/ryzen_5_5600x/` (lo operativo, en el kernel). La regla de
`PERFIL/README.md` sigue mandando: lo que se le PUEDE preguntar al silicio
(CPUID) se pregunta; lo que NO, se escribe.

### [ ] LB10 -- una sola fuente para el build, el kernel y la lente

El perfil, como datos que leen TRES: INTI y TITAN++ al emitir (que
instruccion usar y cual cuesta menos en el Zen 3), la puerta al admitir (LB6:
que extensiones hay) y la lente (LB3/LB4: como se lee y que cuenta cada
contador). Lo que hoy esta en dos sitios y lo vigila un guardian pasa a
salir de uno.

### [ ] LB11 -- lo MEDIDO vuelve al perfil

Dos clases de dato, y no se mezclan:

```text
   DECLARADO   del APM y el PPR de AMD, y de las tablas publicas de
               latencias del Zen 3: lo que el fabricante dice
   MEDIDO      lo que este Ryzen dijo (las caches por CPUID, el TSC, los
               contadores de LB4): con su fecha y su instrumento
```

El kernel guarda lo medido en el disco (como ya guarda el informe); el build
lo lee la siguiente vez. Un numero medido que contradiga al declarado se
dice, no se esconde.

### [ ] LB12 -- el perfil se COMPRUEBA al arrancar

Un perfil es una suposicion: si la CPU que hay debajo no es la del perfil (otra
placa, otro Ryzen), el kernel lo dice y usa lo GENERICO de x86-64. Nunca
instrucciones de un perfil que no es el de la maquina: un `.bex` hecho para el
Zen 3 en otra CPU lo para la puerta (LB6), no un fallo a mitad de camino.
Cuando haya otro perfil (otro AMD, o un Intel), es OTRO fichero con sus
tablas, medido en su metal.

---

## 5. Lo que NO se hace

- una lente que adivine: si una codificacion no esta en la tabla, se dice
  "no la conozco" con sus bytes en crudo
- leer contadores o la instruccion de OTRO proceso sin permiso: la lente es
  del kernel y lo que dice va al informe del propietario
- convertir la lente en un camino caliente: lee cuando algo pasa (un fallo,
  un distinto, una medida pedida), no en cada fotograma
