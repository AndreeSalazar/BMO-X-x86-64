# PLAN: LA VIDA UTIL

**Cuanto tiempo es tuya la memoria que pediste, y quien lo dice.**

Abierto el 2026-09-20. Lo pidio el dueno con estas palabras: *"en inspiracion
de GC pero que es liberar la memoria cuando ya entra pero tiene que salir, en
tiempo real que libere fuerte para que no sufra por eso"*. Y a la vuelta
siguiente: *"pero que hay del WAIT?"*.

La segunda pregunta es la buena, y este documento existe sobre todo por ella.

---

## 1. Lo primero: eso NO es un recolector de basura

*"Entra pero tiene que salir"* es **propiedad y alcance**. Un recolector es un
mecanismo para **ENCONTRAR** lo que ya nadie usa; aqui no hay nada que
encontrar, porque quien lo pide ya sabe cuando sale.

Y un trazador de verdad no cabe en esta casa, por tres motivos que son de
BMO-X y no de un libro:

1. **Tiene que encontrar punteros.** Hay CINCO frontends --C, C++, COBOL, Ada,
   INTI-- y los dos primeros no pueden dar mapas precisos de donde hay un
   puntero. La alternativa es escanear conservador, y el objeto grande de este
   sistema es un doble bufer de **8 MB de pixeles**: cada pixel que por
   casualidad parezca una direccion retiene un bloque que nadie usa. El
   escaner se ahoga justo en la memoria que mas se usa.
2. **Tiene que parar a los mutadores.** El presupuesto de un fotograma son
   16,7 ms. Una pausa ahi es literalmente lo que el dueno llamo *sufrir*.
3. **El kernel ya decidio no ser asignador**, y `obj/memory.rs` lo argumenta:
   DOOM trae su `Z_Zone`. Un recolector en Ring 0 seria escribir un asignador
   que ningun programa usa, para llamarlo por un syscall.

---

## 2. Las DOS mitades, y entre las dos no queda hueco para un GC

```text
   UNA parte    ->  propiedad (Drop)   coste CERO, sin syscall, deterministico
   DOS partes   ->  secuencia (WAIT)   un syscall que DUERME; el kernel juzga
```

Meter `WAIT` en el caso local seria pagar una puerta por una decision que el
compilador ya sabia. Meter `Drop` en el caso compartido seria liberar lo que
otro esta leyendo. **Cada mitad en su sitio, y no sobra trabajo para un
recolector.** Esa es la respuesta entera a la pregunta del dueno.

### 2a. La mitad local: la vida util se DECLARA al pedir

Hoy todo bloque es residente **por accidente**: `Memoria` no lleva `Drop`, asi
que salir del alcance es fugarse. Y nadie lo dijo -- `scene/fondo.rs` llevaba
escrito *"el fichero se suelta al acabar"* sobre un valor que solo se caia del
alcance. Eso costo que el escritorio gastara sus cuatro peticiones antes de
abrir nada, y que el visor de imagenes no tuviera cupo.

Lo que se propone es un **formato, no un cerebro**:

```text
   Memoria::request(bytes)     PRESTADA    lleva Drop. Sale del alcance, vuelve
   Memoria::residente(bytes)   PERMANENTE  sin Drop. Hay que DECIRLO
```

** Y `residente` tiene que existir de verdad, no ser una excepcion vergonzante:
`Pantalla::activar_doble_bufer` pide ~8 MB, se queda la direccion y deja caer
el `Memoria` **porque ese bloque tiene que vivir lo que viva el proceso**. Con
un `Drop` ciego, el escritorio se queda sin lienzo en la linea siguiente. Un
`Drop` automatico seria correcto en el 80 % de los sitios y mortal en el otro
20, y eso no es una politica: es una trampa.

Regalo: **el censo de lo permanente**. Contar los `residente` en el build como
trinquete (solo puede bajar) contesta una pregunta que hoy nadie puede
contestar -- *cuanta memoria de este sistema es permanente a proposito*.

### 2b. La mitad compartida: el hueco que dejo `MEM_OP_SOLTAR`

`MEM_OP_SOLTAR` (20-09) contesta **0** cuando el bloque sigue PRESTADO a otro.
Es honesto y es la politica correcta -- devolver memoria que otro esta leyendo
es peor que negarse. Pero deja al dueno **sin paso siguiente**: reintentar
cuando? Girar preguntando? Girar es exactamente lo que un compositor no puede
hacer.

Lo que falta es que **un bloque prestado sea ESPERABLE**:

```text
   soltar                              -> 0  (sigue prestado, y aqui va la
                                              secuencia que viste)
   WAIT(bloque, esa secuencia, plazo)  -> DUERME. No gira
   soltar                              -> 1
```

** Y no cuesta un syscall nuevo. `WAIT` esta congelado y esto **no es una
llamada mas: es una CLAVE mas**, como ya lo son `KIND_ENDPOINT`,
`KIND_LATIDO`, `KIND_RED` y `KIND_CHANNEL`. El patron existe cuatro veces.

[!] **`WAIT` dice CUANDO volver a preguntar. El kernel dice SI se puede.** Si
el kernel liberara *porque* una secuencia se movio, se estaria creyendo a Ring
3 en una decision de seguridad, y liberar pronto no falla: corrompe. El juez ya
existe y es `loan::hay_prestado_en`. `WAIT` no lo sustituye -- lo despierta.

---

## 3. WAIT: por que existe, y lo que le falta

El dueno: *"WAIT aunque no uso tanto TIENEN que tener por que"*. Lo tiene, y la
historia esta en el arbol.

```text
   WAIT(waitable, observed_sequence, timeout_ns)
```

Bloquea hasta que la secuencia del esperable pase de `observed_sequence`, o
venza el plazo. **La comparacion ocurre bajo el cerrojo del planificador**, asi
que un aviso entre que el llamante lee y llama no se puede perder. Eso no es un
`sleep` con esperanza: es la unica primitiva de este sistema que permite
esperar sin girar.

### 3a. Lo que WAIT sabe hacer hoy

| `KIND_` | quien lo concede | quien lo espera |
|---|---|---|
| `ENDPOINT` | `obj/endpoint.rs` | `endpoint::wait_for` |
| `LATIDO` | `obj/latido.rs` | secuencia de `latido::cuenta` |
| `RED` | `op_maquina.rs` | secuencia de `puerta::secuencia` |
| `CHANNEL` | `obj/cap.rs` | `channel::complete_seq` |
| `ARCHIVO` | `obj/file.rs` | **NADIE** |

### 3b. *** LA PROMESA ROTA: `KIND_ARCHIVO`

`obj/file.rs`, sobre `abrir_asinc`, dice literalmente:

> *"entre trozo y trozo puede **dormirse** sobre su propio handle -- el `wait`
> sabe hacerlo -- mientras el resto del sistema corre. [...] El handle sale con
> `RIGHT_WAIT` ademas de lectura, y **es lo que le da sentido**: sin ese
> derecho el unico modo de esperar seria preguntar en un bucle, que es
> exactamente lo que se estaba quitando."*

**El `wait` NO sabe hacerlo.** El despachador de `syscall/mod.rs` conoce
ENDPOINT, LATIDO, RED y CHANNEL; un `KIND_ARCHIVO` cae en `Ok(_) =>
unsupported()`.

O sea que la lectura asincrona de ficheros --el escalon 4 entero-- concede un
derecho que no se puede ejercer, y el unico modo de esperar **vuelve a ser el
bucle que ese comentario dice que se estaba quitando**. Es la misma familia que
el *"se suelta al acabar"* de `fondo.rs`: una promesa escrita contra un
mecanismo que no existe, y ninguna prueba que cruce las dos mitades.

### 3c. Y WAIT ya mintio una vez, con fecha

`01c09d94` -- **"WAIT no habia bloqueado NUNCA -- el despachador tenia UN solo
brazo, y el compilador llevaba diciendolo"**. El syscall congelado numero dos
del sistema estuvo sin funcionar hasta que alguien lo miro.

** Eso es el argumento entero para el guardian de 3d: en esta casa, lo que no
tiene juez se rompe callado, y WAIT ya demostro que no es una excepcion.

### 3d. Lo que hay que redefinir, y no es la firma

La firma esta bien. Lo que falta es **que nadie pueda conceder `RIGHT_WAIT`
sobre un `KIND_` que el despachador no sabe esperar**. Eso es un guardian de
contrato como los 23 que ya corren: cruzar los `grant(..., RIGHT_WAIT, ...)`
del arbol contra los brazos del `match` de `wait()`, y romper el build si
sobra uno.

---

## 4. El numero incomodo: liberar NO es gratis hoy

`soltar` de un bloque de 8 MB son **2.048 paginas**, y cada una paga:

```text
   zero_frame    write_bytes de 4 KiB      <- esto es lo caro
   free_frame    el mapa de bits
   invlpg        una por pagina
```

Son **8 MB de memset dentro de un syscall**. Por aritmetica sobre el codigo,
del orden de 2-3 ms; con 64 MiB, mas de un fotograma entero. Mientras soltar
era un caso raro no importaba. **Si la liberacion se vuelve automatica y
frecuente, pasa a ser el problema de latencia.**

[!] Ese 2-3 ms es una CUENTA, no una medida. El numero bueno lo da el Ryzen.

### La salida: UNA regla en vez de dos

Hoy el asignador **no limpia al entregar**, y por eso hay que limpiar al
devolver (lo dicen `destroy_address_space` y `process_died`, cada uno por su
lado). Si el asignador **entrega siempre en cero**:

- el borrado se paga donde el que llama **ya esta esperando memoria**, no donde
  se compone un fotograma;
- el trabajo total es el mismo;
- y queda **una invariante en lugar de dos que hay que mantener de acuerdo** --
  que es como se desacoplan (`caminable` vs `zero_frame`, 30-08).

---

## 5. La mina: `invlpg` es de UN solo nucleo

`unmap_page` invalida con `invlpg`, y `invlpg` invalida **en el nucleo que lo
ejecuta**. Con AXION residente hay un segundo nucleo.

Hoy no se sabe si ese nucleo corre alguna vez con el CR3 de un proceso de
Ring 3 -- `crew.rs` dice de si mismo que no planifica-- pero la liberacion
frecuente convierte esto de teorico en *cada pocos segundos*: una entrada
rancia en el TLB del otro nucleo es un proceso escribiendo en un marco que ya
es de otro, y **eso no falla: corrompe**.

O se cierra, o se declara con fecha y alcance. No se deja sin decir.

---

## 6. Y los 969 ciclos estan SUPERADOS

`docs/componente/LA_PUERTA_POR_DENTRO.md` fecha su tanda el **2026-08-17** y
dice, con todas las letras, que todo numero suyo o esta medido o lo declara.
Desde entonces la puerta cambio:

```text
   70ea8db5  M0b: FUERA el cerrojo que pagaba TODA puerta   -147 ticks
   01c09d94  WAIT no habia bloqueado nunca
   289d8340  ~6.300 lineas de bmo-abi fuera
             syscall/ + cap.rs: 2.314 lineas anadidas, 576 quitadas
```

M0b sola son **147 ticks = ~179 ciclos**, el **18 %** de los 969, y su propio
mensaje lo demuestra con tres hechos del arbol. Asi que:

- la direccion es **hacia abajo**, y
- **el numero exacto no lo sabe nadie ahora mismo.**

Repetir la tanda no es trabajo nuevo: la herramienta existe.

```text
   .\bmo.ps1 -Metro        kernel con `--features metro_puerta` (a otro
                           --target-dir, para que no pise al normal)
   sys\precio.bex          el testigo de Ring 3
```

[!] El metro **anade ~112 ciclos por puerta** (los dos `rdtsc` de `dispatch`),
y eso esta escrito en el build. La cifra se corrige, no se olvida.

---

## 7. EL PLAN

- [ ] **0. EL AGUJERO PRIMERO.** No se toca nada de aqui hasta cerrar el
      agujero del doble bufer del escritorio. Motivo, y es el que manda: un
      bloque liberado demasiado pronto y una pagina que se esfuma sola
      **producen la misma pantalla**. Empezar por aqui seria enterrar el fallo
      que estamos a un arranque de cerrar, con un mecanismo que produce su
      mismo sintoma.

- [ ] **1. EL GUARDIAN DE `RIGHT_WAIT`.** Cruzar los `grant(..., RIGHT_WAIT,
      ...)` contra los brazos del `match` de `wait()` y romper el build si
      sobra uno. Hoy sobra `KIND_ARCHIVO`. Va PRIMERO porque es el unico paso
      que no depende de ninguna decision: una promesa que no se puede ejercer
      es un fallo, se decida lo que se decida despues.

- [ ] **2. `KIND_ARCHIVO` ESPERABLE**, o quitarle `RIGHT_WAIT` y decir por que.
      Las dos cierran el paso 1; la primera cumple lo que el fichero promete y
      la segunda deroga la promesa. Lo que no vale es dejarlo como esta.

- [ ] **3. LA TANDA DEL METRO.** `-Metro` + `precio.bex` en el Ryzen, y
      reescribir `LA_PUERTA_POR_DENTRO.md` con la fecha nueva. Hasta entonces,
      **ningun documento cita 969 sin decir que es del 17-08**.

- [ ] **4. `residente` CONTRA `request`.** Partir la peticion en dos por vida
      util, `Drop` en la prestada, y `activar_doble_bufer` pidiendo
      `residente` con su motivo escrito.

- [ ] **5. EL CENSO DE LO PERMANENTE.** Trinquete de `residente` en el build:
      cuantos bloques de este sistema son permanentes a proposito, y solo
      puede bajar.

- [ ] **6. ENTREGAR EN CERO.** Mover el borrado del devolver al entregar, UNA
      invariante, y medir el antes y el despues en el mismo sitio donde duele.

- [ ] **7. LA CLAVE NUEVA DE WAIT.** Secuencia sobre el bloque prestado, que
      se mueve cuando el prestatario suelta, y `soltar` devolviendo la
      secuencia que vio. Con eso el bucle correcto es `soltar -> WAIT ->
      soltar`, y **nunca** `WAIT -> dar por hecho`: lo que WAIT devuelve es una
      secuencia, no un veredicto.

- [ ] **8. EL `invlpg` DEL OTRO NUCLEO.** Cerrarlo o declararlo con fecha,
      alcance y motivo. Ver la seccion 5.

---

## 8. Que TUMBARIA este plan

- **Que el agujero del escritorio resulte ser una liberacion temprana.** Si el
  paso 0 descubre que alguien ya esta soltando memoria de mas, la mitad 2a
  cambia de sentido: no faltaria `Drop`, sobraria una llamada.
- **Que la tanda del metro diga que la puerta sigue cerca de 969.** Entonces el
  paso 7 --un syscall para esperar-- hay que pesarlo otra vez contra girar unas
  pocas vueltas, y el numero decide, no la elegancia.
- **Que `entregar en cero` salga mas caro medido que limpiar al devolver.** Es
  el mismo trabajo en otro sitio; si el sitio importa mas de lo que parece, la
  seccion 4 se cae y queda solo la cola de limpieza, que es mas maquinaria.
