# EL REINO FUNGI -- BMO-X se inspira en el HONGO, y el kernel es su LENTE

> **BMO-X toma su inspiracion del reino de los HONGOS (Fungi).** No como
> adorno: como una forma de mirar cada pieza y preguntarle si esta bien hecha.
> Esta hoja dice por que el hongo, donde se parece BMO-X con un mecanismo al
> lado (como pide [`EL_ORQUESTAL.md`](EL_ORQUESTAL.md)), donde NO se parece, y
> la regla que sale de aqui: **el kernel es el lente de lo que falla**.
>
> Decision del propietario, **2026-10-13**: *"BMO-X se va a inspirar todo en
> el reino del FUNGI, del HONGO; eso es para mejorar las piezas, y mi kernel
> se convierte en mi lente para facilitar todo lo que FALLA; y si no lee lo
> mismo que nosotros, que es castellano, algo le falta educar mas."*

---

## 1. POR QUE UN REINO, Y NO UNA CAJA

Durante siglos los hongos se clasificaron como PLANTAS: no se mueven, crecen
del suelo. No lo son. No hacen fotosintesis, no comen como un animal, y
tienen quitina en la pared como los insectos. En **1969** (Whittaker) se les
dio un **reino propio**, porque meterlos en una caja que ya existia hacia
mentir a la caja.

Es lo que BMO-X dice de si mismo desde el 06-09: **no es un SO, ni un RTOS,
ni ninguna otra caja prestada** -- es BMO, Bare Metal Orquestal. Una
categoria es una promesa que se hace sola; el hongo es el ejemplo de la
naturaleza de algo que tuvo que REDEFINIR la clasificacion para que se le
viera como es. Eso es lo primero que se toma del reino: **no pedir una caja
prestada; describirse por lo que se HACE.**

---

## 2. CINCO COSAS QUE EL HONGO YA HACE, Y DONDE ESTAN EN EL ARBOL

### 2.1 Presta, no regala (la micorriza)

Casi todas las plantas viven atadas a una red de hongos en sus raices: la
**micorriza**. El hongo le da fosforo y agua; la planta, azucar. No es un
regalo: es un intercambio, y se vigila. Los experimentos (Kiers y otros,
*Science*, 2011) mostraron que **los dos lados premian al socio que cumple y
le dan menos al que hace trampa**.

Es el celo de [`EL_ORQUESTAL.md`](EL_ORQUESTAL.md): *aqui no se da nada, se
presta, y el prestamo se acaba cuando el titular quiere*. Con mecanismo:

```text
   la IOMMU        amdvi::Dominio::prestar_marcos -- la 3060 recibe la RAM
                   de una app PAGINA A PAGINA, todo o nada
                   (platform/drivers/iommu/amdvi/src/paginas.rs)
   el UAV          uav.rs de PROTON-X (LI2f, 12-10): cada direccion se
                   SUJETA al tope de lo prestado; un indice enorme no toma
                   ni un byte mas (platform/shared/proton-x-sm86/src/uav.rs)
   el que falla    un canal de la 3060 que tomo un Xid queda MUERTO
                   (`Porque::CanalMuerto`): al socio que rompe, nada mas
```

### 2.2 Digiere AFUERA, y absorbe lo limpio

Un hongo no traga. Suelta sus enzimas FUERA de su cuerpo, descompone lo que
encuentra, y solo despues absorbe lo que ya es suyo.

Es PROTON-X con un juego de Windows: el DXIL del juego se descompone AFUERA,
en el Programa de la casa (el que la CPU ya corre); de ahi sale el SASS de la
3060, y el JUEZ del kernel lo mira antes de dejarlo entrar
(`bmo_gpu_ga10x::sass::juez`). **Nada ajeno entra entero**: entra lo que la
casa ya reescribio con sus palabras.

### 2.3 Crece por la PUNTA, y todo lo demas es red

Una hifa crece solo por su punta; detras queda la red, que no se mueve. Por
eso un hongo puede ser enorme sin un centro que lo mande.

BMO-X tiene **dos puertas** con el kernel, `INVOKE` y `WAIT`, congeladas
([`ARQUITECTURA.md`](../../ARQUITECTURA.md)). La superficie no crece; lo que
crece esta detras, en crates PUROS y pequenos (cada uno dice su `generacion`
y su `capa` en la cabecera) que se pasan cosas sin un centro que lo haga
todo.

### 2.4 Hace SUELO con lo que ya paso

Los hongos son los que convierten lo muerto en capas de tierra de las que vive
lo nuevo. Nada se tira: se descompone y se guarda en capas.

ESTRATOS ([`ESTRATOS.md`](../../platform/drivers/storage/estratos/ESTRATOS.md))
guarda CADA escritura como una version, por capas, y mezcla ramas por nodos
enteros. El nombre ya es de suelo; el hongo es quien trabaja esas capas. Y
la BITACORA es lo mismo para las decisiones: lo que fallo no se borra, queda
como capa de la que aprende lo de despues.

### 2.5 Una red que SABE donde le duele

Un micelio responde donde se le dania: crece hacia el alimento y se retira de
lo que le hace mal, punta por punta, sin un cerebro que lo decida. La red
entera se entera porque cada parte reacciona en su sitio.

Eso pide el lente (abajo): **cada pieza dice donde y por que le duele**, en
su sitio, y el kernel lo junta para que el titular lo lea.

---

## 3. EL KERNEL ES EL LENTE DE LO QUE FALLA

Esta es la regla que sale del reino, y la mas util:

> **Una pieza que falla y no sabe decirlo en palabras -- en castellano, el
> que leemos nosotros -- todavia no esta educada.** Le falta aprender a
> hablar, no solo a funcionar.

El kernel no esconde los fallos detras de un numero: es el **lente** que los
pone delante del titular, con su nombre. Ya hay piezas educadas, y son el
modelo:

```text
   el porque de la 3060   bmo_gpu_ga10x::porque (12-10): 23 porques con su
                          frase, el SUB (cual paso, cual caso) y un detalle;
                          el kernel apunta el ultimo NO y PROTON-X lo lee
                          (INFO_GPU_PORQUE) y lo dice
   el simulador           proton_x_sm86::simula::decir: la instruccion que
                          no supo correr, con su texto
   el respiro             cada motivo por el que no suelta, con su nombre
                          (RESERVA_GPU_VIENDO, ...)
```

Y el caso que lo enseno: hasta el 12-10 el kernel decia `motivo 79` por
**cuarenta caminos distintos**. El metal dio dos NO con ese numero y ninguna
forma de saber cual. Un numero solo es un hongo que no dice donde le duele:
la red no puede reaccionar. Desde que cada camino tiene su frase, el primer
NO del metal se lee y se arregla.

**Como se mide** que una pieza esta educada:

1. Cada `NO` (cada variante de error) tiene su **frase** en castellano, sin
   acentos (lo que se imprime es ASCII), y una prueba que lo comprueba.
2. El numero NO se reusa: un informe viejo se sigue leyendo igual.
3. Lo que falla dice **donde** (el paso, el caso, la instruccion), no solo
   **que**.
4. Lo que el kernel apunta lo puede leer quien lo sufrio (su pid), y nadie
   mas: el lente es del titular, no un escaparate.

Una pieza nueva que solo devuelve un numero o un `false` **no esta
terminada**, aunque funcione.

---

## 4. DONDE LA METAFORA SE ROMPE (dicho, para no mentir con ella)

- **El hongo se extiende a cualquier sitio; BMO-X no, a proposito.** Una sola
  arquitectura (x86-64), y el guardian `isa` rompe el build si crece otra.
  BMO-X es una micorriza **especialista**: un solo huesped (el Ryzen y la
  RTX 3060), conocido hasta el bit.
- **El hongo tolera perder media red; BMO-X exige bits exactos.** Los mismos
  bits que la CPU (DL10), la palabra de oro de `ptxas` en cada instruccion,
  nada adivinado. Ahi BMO-X es mas cristal que hongo, y eso NO se cambia por
  la metafora.
- **La metafora inspira; no decide.** Si una pieza queda mas clara con el
  hongo, bien. Si la imagen pide algo que el metal desmiente, gana el metal.
- **"Fingido" no es "fungi".** En el codigo, *fingido* es "de mentira" (el
  `Metal` fingido del USB, la 3060 de mentira del simulador). Esa palabra
  sigue significando eso; el reino se nombra **fungi** u **hongo**.

---

## 5. COMO SE USA AL HACER UNA PIEZA

No se renombra lo que ya existe (romperia los planes, la bitacora y los .bsf
guardados). Se usa al hacer o rehacer una pieza ENTERA -- *no parches:
cambiar piezas* --, con cinco preguntas:

```text
   1. presta o regala?       lo que da, lo puede recuperar el titular?
   2. digiere afuera?        lo ajeno entra reescrito y juzgado, o entero?
   3. crece por la punta?    agranda la superficie, o crece detras de ella?
   4. deja suelo?            lo que paso queda en capas de las que aprender?
   5. dice donde le duele?   cada NO tiene su frase, en castellano? (el lente)
```

Una pieza que contesta mal a una de las cinco tiene por donde mejorar. Esa
es la razon del reino: **mejorar las piezas**, y que lo que falla se vea.
