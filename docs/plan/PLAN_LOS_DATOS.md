# PLAN LOS DATOS -- SQL redefinido por la casa: preguntar y guardar sin SQL

> Abierto el **2026-10-05**. El propietario, despues de preguntar que pasaria
> con SQL y SQLite en BMO-X: *"podemos redefinir el SQL por completo no el
> original sino MUCHO MEJOR propios para esto [...] el SQL puede ser que mi
> kernel busque de golpe en todo, el SQL lo traiga C y el TITAN++ =
> simplemente lo redefine por completo"*. Y el primer cliente tiene nombre:
> **ULTRACRAFT**, el mundo de cubos (seccion 7).
>
> Va despues de `docs/plan/PLAN_LISTAS_Y_MAPAS.md`: una tabla de una base de
> datos es una lista que sobrevive al apagado, y un indice es un mapa.

---

## 0. La respuesta corta

```text
   SQL son tres     el IDIOMA      decir QUE se quiere, no COMO buscarlo
   cosas juntas     el MOTOR       indices, recorrer, juntar, ordenar
                    el SUELO       que un "guardado" sobreviva a un apagon
   en BMO-X         el SUELO       YA ESTA: ESTRATOS es copy-on-write, con
                                   suma en cada bloque, commit atomico por
                                   superbloque alterno y la historia entera
                                   (lo dificil de SQLite, hecho en el FS)
                    el IDIOMA      TITAN++, comprobado AL COMPILAR, con 0
                                   palabras nuevas: [x for x in l if ...]
                    el MOTOR       las listas y mapas del nivel 13 + el
                                   indice por hash (L9); lo caliente, INTI
   lo que NO        el kernel no ejecuta preguntas (seccion 2)
   lo que se gana   ver la tabla 4: lo que SQL no puede dar porque nacio
                    texto, aparte del lenguaje y encima de un disco que
                    sobreescribe
```

---

## 1. Lo que ya esta, y no se reinventa

| pieza | donde | que da |
|---|---|---|
| copy-on-write con historia | `platform/drivers/storage/estratos/ESTRATOS.md` (secciones 1, 2, 4) | nunca se sobreescribe un bloque vivo; un corte deja el estado ANTERIOR entero |
| el commit en el disco | `Ultra_kernel_x86-64/kernel/src/ring0/fsys/estratos/escribir.rs` | escribir los bloques, FLUSH, superbloque alterno, FLUSH: el orden que no pierde datos |
| volver atras | `volver` en el mismo fichero, y `Ultra_kernel_x86-64/kernel/src/ring0/fsys/estratos/historia.rs` | la cadena de versiones ya se recorre y se restablece |
| leer de ESTRATOS | `Ultra_kernel_x86-64/kernel/src/ring0/obj/file.rs` (`leer_rango`) | un programa ya LEE ficheros de ESTRATOS por `ARCH_OP_*` |
| la cache del disco | `docs/plan/PLAN_EXPRIMIR_EL_DISCO.md` (P0, D5) | el perfil ya dijo que la cache esta encendida: un OK es "aceptado", no "guardado" -- por eso el FLUSH del commit |
| listas y mapas | `toolchain/lang/titan/src/calc/coleccion.rs`, `toolchain/lang/titan/emisor-x86_64/src/e1/coleccion.rs` | `[T]` y `{K: V}` en E0 y E1, con el oraculo E0 == E1 |
| sin null | `Opcion[T]` con `Hay`/`NoHay` (`toolchain/lang/titan/src/prelude.rs`) | lo que puede faltar es un caso que el `match` mira |
| SQLite medido | `docs/identidad/QUE_DESBLOQUEA.md` (fila SQLite) | portarlo por la cadena de C: semanas, sobre `ARCH_OP_*` |

[!] **El hueco de hoy, dicho exacto:** un programa LEE de ESTRATOS, pero lo que
ESCRIBE por `ARCH_OP_ESCRIBIR` acaba en FAT32 (`guardar_en` en
`Ultra_kernel_x86-64/kernel/src/ring0/obj/file.rs`), entero y al cerrar. Las
escrituras con historia solo las hace hoy la ventana de ESTRATOS. Unir esas
dos mitades es el escalon Q4, y es lo que hace que guardar sea un commit.

## 2. Por que el kernel NO busca (y que si hace "de golpe")

La idea del propietario era que el kernel buscara en todo de golpe. Se queda
la mitad buena de esa idea:

```text
   el kernel SI    trae MUCHOS bloques de una vez (NCQ: 32 ordenes a la vez,
                   docs/plan/PLAN_EXPRIMIR_EL_DISCO.md D2), confirma un
                   commit de MUCHOS bloques como uno, y guarda la historia
   el kernel NO    recorre filas, compara claves, junta tablas
```

**Por que no:** una pregunta mal escrita que corre en el anillo 0 cuelga la
maquina entera, y lo que decide como buscar es un cerebro. La regla de la
casa ya lo dice: *contratos y formatos, nunca cerebros*
(`platform/abi/bmo-abi/src/bef/BEF_EXTENSIONES.md`). El kernel da el SUELO
deprisa; buscar es del programa, y lo que haya que buscar muy deprisa se lo
lleva INTI (Q8), que es quien habla por registro.

## 3. Las decisiones del propietario

**PENDIENTES.** Cada una con su recomendada; se escriben aqui al decidirse.

### D1 -- como se escribe una pregunta

```text
   (a) RECOMENDADA  con lo que ya hay: una lista o un mapa POR COMPRENSION
                      [c.nombre for c in clientes if c.edad > 30]
                      [(p.id, c.nombre) for p in pedidos for c in clientes
                                        if p.cliente == c.id]
                      {c.id: c for c in clientes if c.activo}
                    for, in e if ya son palabras: 0 nuevas (siguen 25).
                    SELECT es lo de delante, FROM es el for, WHERE es el
                    if, y un JOIN son dos for
   (b)              palabras de SQL: from/where/select (3 nuevas). Se lee
                    como SQL, pero son dos maneras de decir un for
```

### D2 -- que se guarda en el disco

```text
   (a) RECOMENDADA  el VALOR de TITAN++ tal cual: una lista o un mapa de
                    registros, con la FORMA de su tipo escrita delante.
                      guardar(mundo, "mundo")
                      let m: {Sitio: Trozo} = ...  cargar("mundo") da
                      Hay(m) si la forma del fichero es la del tipo, NoHay
                      si no esta o es otra
                    la fila ES el tipo: sin traducir de tablas a objetos
   (b)              un fichero .db de SQLite: lo leen todas las
                    herramientas de fuera, pero el tipo de TITAN++ y la
                    tabla de SQLite serian dos cosas que hay que casar
```

### D3 -- que es un guardado

```text
   (a) RECOMENDADA  UN guardado = UN estrato. Atomico (o todo o lo de
                    antes), con suma, y con nombre si se le pone. Volver al
                    mundo de hace diez minutos es leer un estrato viejo
   (b)              guardar sin historia, sobreescribiendo: mas sitio libre
                    en el disco, pero se tira lo unico que SQLite no tiene
```

### D4 -- los nombres de la biblioteca

```text
   (a) RECOMENDADA  en ingles corto, como push/put/get (D1 del nivel 13):
                    save(v, "ruta") y load("ruta")
   (b)              en castellano: guardar/cargar (en este plan se usan asi
                    para leerlo, pero el nivel 13 ya eligio ingles)
```

### D5 -- SQLite

```text
   (a) RECOMENDADA  DESPUES, y solo como PUERTA: el SQLite de verdad por la
                    cadena de C para EXPORTAR e IMPORTAR (que los datos de
                    BMO-X se abran en Windows y Linux, y al reves)
   (b)              nunca: todo de la casa y nada sale
   (c)              primero: SQLite como motor y TITAN++ encima. Es lo mas
                    rapido de tener, pero mete en medio un idioma de texto
                    que se comprueba al correr, y un diario propio encima
                    de un FS que ya es transaccional (el doble de escrituras)
```

## 4. Por que es MEJOR, y donde no lo es

| SQL de siempre | aqui | de donde sale |
|---|---|---|
| la pregunta es TEXTO: se comprueba al correr | se comprueba AL COMPILAR: una columna que no existe o un tipo que no casa es un codigo T antes de correr | el comprobador de TITAN++ |
| inyeccion de SQL | no existe: no hay texto que pegar | lo mismo |
| NULL y la logica de tres valores (`NULL = NULL` no es verdad) | no hay NULL: lo que falta es `NoHay`, y el `match` obliga a mirarlo | nivel 8 y nivel 13 |
| tablas por un lado, objetos por otro (el "ORM") | la fila ES un `type`; la tabla ES `[Fila]` o `{K: Fila}` | D2 (a) |
| un diario o WAL propio encima del FS | el commit ES el del FS: una escritura, no dos | ESTRATOS |
| volver a ayer: copias de seguridad | volver a ayer: leer un estrato viejo | ESTRATOS, `volver` |
| la corrupcion la nota quien lee | cada bloque lleva su suma: se nota al leer, y es un FAULT | ESTRATOS, principio 2 |
| el resultado puede cambiar entre maquinas (orden sin ORDER BY) | orden de entrada siempre, y E0 == E1 lo exige | D4 del nivel 13 |

**Donde NO es mejor, dicho antes de que se descubra:**

- **No hay preguntas tecleadas al correr.** Una pregunta es codigo y se
  compila; una consola tipo `sqlite3` para preguntar lo que se le ocurra a una
  persona seria otro escalon (un interprete), no este plan.
- **No hay planificador por costes.** SQLite elige solo que indice usar; aqui
  el indice es un mapa que el programa escribe y usa. Mas claro, menos magico.
- **No hay compatibilidad de fuera** hasta D5.
- **Treinta anios de pruebas** no se copian: las de SQLite son cientos de
  veces su codigo. Por eso el SUELO (lo que pierde datos si falla) NO se
  reescribe: es ESTRATOS, que ya tiene su 1.0.

## 5. Los escalones

- [ ] Q0 -- las decisiones D1-D5 del propietario, escritas aqui (seccion 3)
- [ ] Q1 -- la COMPRENSION en el frontend y el calculo (E0): `[e for x in l if c]`, varios `for` (el JOIN) y `{k: v for ...}`, con su tipo deducido de `e`; un nivel nuevo de TITAN++ (`toolchain/lang/titan/GRAMATICA.md`), con su BIEN y sus NO en los dos bancos
- [ ] Q2 -- la comprension en E1 (`toolchain/lang/titan/emisor-x86_64/src/e1/coleccion.rs`): un bucle que llena una lista o un mapa en el monton; el oraculo E0 == E1 y el azar la cubren como a las listas
- [ ] Q3 -- el INDICE POR HASH: es L9 de `docs/plan/PLAN_LISTAS_Y_MAPAS.md`; sin el, `get` en un mapa grande recorre todo, y un indice que recorre no es un indice
- [ ] Q4 -- en el KERNEL: lo que un programa escribe por `ARCH_OP_*` en el volumen de ESTRATOS se guarda con `crear_fichero`/`aplicar` (`Ultra_kernel_x86-64/kernel/src/ring0/fsys/estratos/escribir.rs`) y no con `guardar_en` de FAT32: cerrar ES commitear. FAT32 se queda para la particion de arranque (principio 6 de ESTRATOS)
- [ ] Q5 -- `save` y `load` en TITAN++ (D2, D4): el valor con la forma de su tipo delante, en E0 (un fichero de prueba) y en E1 (por `ARCH_OP_*`); `load` da `Hay`/`NoHay`, nunca un valor de otra forma
- [ ] Q6 -- guardar POR TROZOS: hoy un fichero se reescribe entero; para un mundo de gigas, una operacion nueva en `platform/abi/bmo-abi/src/syscalls/surface/objetos.rs` que escriba EN una posicion dentro de la misma transaccion y confirme todo junto al cerrar (en copy-on-write solo se copian los bloques tocados). Bloquea: Q4, y medir antes que hace falta (LEY 24: no se estima, se mide)
- [ ] Q7 -- la HISTORIA desde TITAN++: cargar el valor de un estrato anterior (`historia.rs` ya recorre la cadena); el "deshacer" de un mundo entero
- [ ] Q8 -- lo CALIENTE en INTI: recorrer millones de filas con SIMD segun el perfil de la CPU, cuando el metro diga que la comprension de E1 es el cuello
- [ ] Q9 -- SQLite como PUERTA (D5 a): exportar e importar por la cadena de C, el dia que alguien necesite sacar los datos
- [ ] Q10 -- del PROPIETARIO: las leyes nuevas con `--sellar` (`toolchain/tools/titan-leyes/LEYES.txt`)

## 6. La forma que tiene, de punta a punta

```text
   TITAN++     let viejos = [c for c in clientes if c.edad > 30]
                 comprobado al compilar; E0 lo corre si los datos son
                 constantes, E1 lo emite si vienen de fuera
   E1          un bucle sobre las celdas, llenando una lista nueva en el
                 monton (lo de L4); con un indice, un `get` por hash (L9)
   INTI        el mismo bucle con SIMD cuando el metro lo pida (Q8)
   kernel      save: los bloques nuevos, FLUSH, superbloque, FLUSH
                 = UN estrato; load: trae los bloques en tramos grandes
   disco       con la cache encendida, un OK no es guardado: el commit pide el FLUSH
```

## 7. ULTRACRAFT: el primer cliente

El mundo de cubos del propietario es, antes que un juego, una BASE DE DATOS
que se dibuja. Minetest (Luanti), el clon libre de Minecraft, guarda sus
trozos en SQLite, una fila por trozo. Aqui:

```text
   en la memoria     {Sitio: Trozo}        los trozos cargados (nivel 13,
                                           `toolchain/lang/titan/ejemplos/nivel13/mundo.titan`
                                           ya es el primer mundo)
   en el disco       save(mundo, "mundo")  UN guardado = UN estrato
   preguntar         [t for t in trozos if t.cambiado]   lo que hay que
                                           guardar; [s for s in cerca if
                                           not has(mundo, s)] lo que hay
                                           que cargar
   deshacer          un estrato viejo      "el mundo de hace diez minutos",
                                           sin copia de seguridad ni mod
```

Lo demas de ULTRACRAFT (la ventana, los cubos con profundidad y texturas por
VERRANO, lo caliente en INTI, el puente de E2) va en su propio plan. Este solo
le da el SUELO y el IDIOMA para preguntar.

## 8. Lo que este plan NO es

- **No es un servidor de base de datos.** No hay proceso aparte ni red: la
  pregunta es codigo del programa, y el disco es el FS.
- **No es SQL en el kernel.** Seccion 2.
- **No reescribe SQLite.** Si hace falta SQLite, se compila el de verdad (D5).
- **No toca el formato de ESTRATOS.** Usa sus verbos; si Q6 pide uno nuevo,
  se escribe en `platform/drivers/storage/estratos/ESTRATOS.md` antes de
  tocar un sector.
