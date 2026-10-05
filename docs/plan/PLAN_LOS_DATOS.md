# PLAN LOS DATOS -- ESTRATOS ya es la base de datos; TITAN++ la administra

> Abierto el **2026-10-05**. El propietario, despues de preguntar que pasaria
> con SQL y SQLite en BMO-X: *"podemos redefinir el SQL por completo no el
> original sino MUCHO MEJOR propios para esto"*. Y el mismo dia, al leer la
> primera version de este plan, la vuelta que lo cambia: *"el SQL propio es
> ESTRATOS que tiene control y verificar y limpiar eso SIN GC [...] TITAN++
> tendra algo simple para administrar el ESTRATOS porque es YA SQL [...] el C
> bueno es que trae el SQLite pero consultas, pero ya no lo necesito"*. Y
> pidio que ESTRATOS se vea en **nodos animados unicos**.
>
> El primer cliente tiene nombre: **ULTRACRAFT**, el mundo de cubos
> (seccion 8). Va despues de `docs/plan/PLAN_LISTAS_Y_MAPAS.md`: lo que se
> pregunta son listas y mapas.

---

## 0. La respuesta corta

```text
   la vuelta        no hace falta UNA base de datos encima del disco: el
                    sistema de ficheros de BMO-X YA LO ES. Lo que faltaba
                    era verlo, y un idioma para administrarlo
   ESTRATOS da      el commit atomico, la suma de cada bloque, la historia,
                    volver, marcar lo permanente -- y todo con PUERTA desde
                    Ring 3 desde agosto (seccion 2)
   TITAN++ da       el idioma: preguntar con 0 palabras nuevas
                    ([e for e in list("juegos") if e.bytes > n]), y los
                    verbos de administrar como funciones de biblioteca,
                    comprobados AL COMPILAR
   limpiar          SIN recolector de fondo: COMPACTAR cuando una persona o
                    un programa lo pide, LISTANDO antes lo que se suelta
                    (decision del 19-08, confirmada el 01-10 y hoy)
   SQLite y C       fuera: el propietario no los necesita (D5)
   la cara          los estratos como NODOS ANIMADOS, cada uno con la forma
                    que le da su propio hash (seccion 6)
```

---

## 1. SQL contra ESTRATOS, palabra por palabra

Lo que el propietario vio, escrito para que se pueda comprobar:

| en SQL | en ESTRATOS | donde esta |
|---|---|---|
| una TABLA | una CARPETA | `carpeta.rs` en `platform/drivers/storage/estratos/src/` |
| una FILA | un NODO | `objects.rs` en la misma carpeta |
| las COLUMNAS | los ATRIBUTOS del nodo (`:datos`, `:firma`, `:manifiesto`...) | `platform/drivers/storage/estratos/ESTRATOS.md`, seccion 4 |
| `COMMIT` | publicar un ESTRATO: bloques, FLUSH, superbloque alterno, FLUSH | `Ultra_kernel_x86-64/kernel/src/ring0/fsys/estratos/escribir.rs` |
| `ROLLBACK` | `volver(n)`: UN estrato nuevo que apunta a la raiz de antes; lo de en medio no se pierde | `ES_GESTO_VOLVER` |
| `SAVEPOINT` | `marcar(nombre)`: la version con nombre es PERMANENTE | `ES_GESTO_MARCAR` |
| `PRAGMA integrity_check` | `verificar`: el BLAKE3 de cada bloque | `ES_NODO_VERIFICAR` |
| `VACUUM` | COMPACTAR: copiar lo vivo hacia adelante y bajar `log_head` | `ESTRATOS.md`, tramo 4 y C3 (por hacer) |
| copias de seguridad | ya son la historia: cada estrato apunta a su padre | `Ultra_kernel_x86-64/kernel/src/ring0/fsys/estratos/historia.rs` |
| `SELECT ... WHERE` | una lista POR COMPRENSION de TITAN++ sobre lo que se lista (Q1, Q5) | por hacer |

**Lo que ESTRATOS NO tiene de una base de datos, dicho antes de que se
descubra:** un INDICE sobre lo que hay DENTRO de los ficheros. Buscar "los
nodos cuyo contenido dice X" es recorrerlos. El indice, cuando haga falta, es
un mapa de TITAN++ guardado como un nodo mas (Q3 y Q6): se escribe y se usa a
la vista, nada lo mantiene solo.

## 2. Las puertas que YA estan (y la correccion a la primera version)

La primera version de este plan decia que un programa solo podia escribir en
FAT32 y pedia tocar el kernel. **Era media verdad:** `ARCH_OP_*` escribe en
FAT32 (`guardar_en` en `Ultra_kernel_x86-64/kernel/src/ring0/obj/file.rs`),
pero ESTRATOS tiene su propia puerta desde Ring 3, y es entera:

| puerta | que hace | contrato |
|---|---|---|
| `TASK_OP_ES_NODO` | el CURSOR: raiz, hijos, entrar, subir, tipo, bytes, atributos, firmado, VERIFICAR, releer | `platform/abi/bmo-abi/src/syscalls/surface/objetos.rs` |
| `TASK_OP_ES_TEXTO` | los nombres (de un hijo, de la ruta, de una version) | el mismo fichero |
| `ES_HIST_*` (por el cursor) | la HISTORIA: cuantas, cuando, quien, con nombre | el mismo fichero |
| `TASK_OP_ES_GESTO` | lo que ESCRIBE: guardar, crear, carpeta, quitar, renombrar, copiar, marcar, volver | `platform/abi/bmo-abi/src/syscalls/surface/disco.rs` |

Y ya hay quien las envuelve en Rust: `Ultra_userspace/userland/src/estratos.rs`
(`guardar_desde`, `marcar`, `volver`, `verificar`, `hist_*`...), que usan la
consola del director y la ventana de Datos. **TITAN++ no necesita ni una
puerta nueva del kernel:** necesita que E1 emita estas mismas llamadas.

## 3. Limpiar SIN recolector, y por que es una ventaja

El propietario lo decidio el 19-08 (*"no es necesario el GC"*), lo matizo el
01-10 cuando los juegos cambiaron la cuenta (*"no olvides el recolector para
limpiar"*), y hoy lo dice entero: **limpiar si, recolector de fondo no.**
`ESTRATOS.md` (tramo 4 y "el recolector vuelve a la mesa") ya eligio COMO:
**B, COMPACTAR**.

```text
   un GC de fondo           decide solo, cuando quiere, y se come el disco
                            en un momento que nadie eligio
   limpiar en ESTRATOS      1  se PIDE: un verbo, de una persona o de un
                               programa de TITAN++
                            2  se LISTA antes: que versiones se sueltan y
                               cuanto se gana -- y se puede mirar (seccion 6)
                            3  se hace EXACTAMENTE esa lista, nada mas
                            4  lo MARCADO no se suelta jamas
                            5  lo que es cache de verdad se guarda SIN
                               historial: su basura no llega a existir
```

**Las ventajas, medibles:** nada corre de fondo (ni pausas ni discos
ocupados sin motivo); lo que se pierde lo eligio alguien y quedo escrito; y
TITAN++ comprueba al compilar que un programa de limpieza solo puede tocar lo
que lista. El COMPACTAR mismo es C3 de `ESTRATOS.md` y es del kernel: TITAN++
lo PIDE, no lo implementa.

## 4. Las decisiones del propietario

**D1-D4 ACEPTADAS el 05-10** (*"eso es perfecto"*), las cuatro (a):
preguntar por COMPRENSION con 0 palabras nuevas; guardar el VALOR de TITAN++
con la forma de su tipo delante; UN guardado = UN estrato; nombres de
biblioteca en ingles corto, como `push`/`get`. **D5 DECIDIDA el 05-10, (b):**
sin SQLite y sin C en medio (*"ya no lo necesito"*). Si alguna no era asi, se
corrige aqui.

### D1 -- como se escribe una pregunta (ACEPTADA: a)

```text
   (a) ELEGIDA      una lista o un mapa POR COMPRENSION
                      [c.nombre for c in clientes if c.edad > 30]
                      [(p.id, c.nombre) for p in pedidos for c in clientes
                                        if p.cliente == c.id]
                      {c.id: c for c in clientes if c.activo}
                    SELECT es lo de delante, FROM es el for, WHERE es el
                    if, y un JOIN son dos for. Siguen siendo 25 palabras
   (b)              from/where/select: tres palabras para decir un for
```

### D2, D3, D4, D5 -- (ACEPTADAS)

```text
   D2 (a)   se guarda el VALOR: `load` da Hay(v) si la forma del fichero es
            la del tipo pedido, y NoHay si no esta o es otra
   D3 (a)   UN guardado = UN estrato: atomico, con suma, y volver es leer
            uno viejo
   D4 (a)   ingles corto: save, load, list, history, mark, back, verify,
            clean
   D5 (b)   sin SQLite: el idioma es TITAN++ y el suelo es ESTRATOS
```

### D6 -- donde viven los verbos (PENDIENTE)

```text
   (a) RECOMENDADA  en un MODULO de la biblioteca, `estratos`, que el
                    programa pide con `use estratos` (nivel 9). No choca con
                    nada: `remove(mut m, k)` ya es de los mapas (nivel 13), y
                    quitar un fichero es otra cosa. Y se lee de donde sale
                    cada verbo: lo que toca el disco se pide a la vista
   (b)              sueltos, como push y get: mas corto, pero obliga a
                    buscar nombres que no choquen con los de las colecciones
```

## 5. Los verbos de TITAN++ sobre ESTRATOS

La propuesta, cada verbo sobre una puerta que YA existe:

| TITAN++ | da | puerta |
|---|---|---|
| `list(ruta)` | `[Entrada]`: nombre, si es carpeta, bytes, firmado | `ES_NODO_ENTRAR`, `ES_NODO_HIJOS`, `ES_NODO_HIJO_TIPO`, `ES_NODO_HIJO_BYTES`, `ES_NODO_HIJO_FIRMADO`, `ES_TXT_HIJO` |
| `history()` | `[Version]`: cuando, quien, nombre, marcada | `ES_HIST_RELEER`, `ES_HIST_CUANTAS`, `ES_HIST_CUANDO`, `ES_HIST_QUIEN`, `ES_HIST_CON_NOMBRE`, `ES_TXT_HIST_NOMBRE` |
| `verify(ruta)` | un caso: `Cuadra`, `NoCuadra`, `SinSuma`, `NoSeLee` | `ES_NODO_VERIFICAR` |
| `save(v, ruta)` | el estrato nuevo, o `NoHay` | `ES_GESTO_ORIGEN` + `ES_GESTO_GUARDAR` (el valor viaja en un bloque de `KIND_MEMORIA`, dos llamadas para cualquier medida) |
| `load(ruta)` | `Hay(v)` / `NoHay` (D2) | `ARCH_OP_*`, que ya resuelve ESTRATOS antes que FAT32 |
| `mark(nombre)` | la version en curso, PERMANENTE | `ES_GESTO_MARCAR` |
| `back(n)` | volver n pasos, sin perder lo de en medio | `ES_GESTO_VOLVER` |
| `forget(ruta)`, `rename(ruta, nuevo)` | dejar de nombrar (no destruye), renombrar | `ES_GESTO_QUITAR`, `ES_GESTO_RENOMBRAR` |
| `clean_plan()` y `clean(plan)` | la LISTA de lo que se soltaria, y soltar exactamente esa | por hacer: C3 de `ESTRATOS.md` |

Y juntos, un programa de administracion entero:

```text
   use estratos

   fn main()
       let grandes = [e for e in list("proton-x") if e.bytes > 500000000]
       for e in grandes
           print(e.nombre, " ", e.bytes)
       for v in history()
           if v.marcada
               print("para siempre: ", v.nombre)
       match verify("mundo")
           Cuadra
               mark("mundo sano")
           NoCuadra
               print("el mundo no cuadra: vuelvo uno atras")
               back(1)
           SinSuma
               print("sin suma")
           NoSeLee
               print("no se lee")
```

Un nombre de columna mal escrito (`e.byts`) o un caso olvidado en el `match`
son un codigo T AL COMPILAR, como todo en TITAN++. Ningun SQL da eso.

## 6. La cara: ESTRATOS en nodos animados unicos

`ESTRATOS.md` (seccion 0.4) ya lo pedia: *ver los estratos como nodos y
volver a uno*, con el verbo RESTABLECER. Y el TALLER ya sabe dar a cada nodo
una forma propia sacada de un hash (`Ultra_userspace/apps/taller/src/space.rs`
y sus astros). Se juntan:

```text
   cada estrato     un NODO; su forma, su color y su giro salen de SU
                    BLAKE3 -- unico de verdad, no por sorteo: dos versiones
                    distintas no se pintan igual
   el cable         del padre al hijo: la historia es un grafo y no hay
                    que construirlo, solo pintarlo
   marcado          brilla fijo: PERMANENTE, clean no lo toca
   en un plan       los que clean_plan() soltaria se APAGAN poco a poco
   de limpieza      ANTES de hacerlo: lo que se pierde se ve
   verify falla     el nodo se AGRIETA: la suma no cuadra
   el verbo         pulsar un nodo y RESTABLECER = back(n) hasta el
```

Vive en la ventana de Datos (`Ultra_userspace/services/director/src/scene/data/mod.rs`),
junto a lo que queda de la maqueta animada de ESTRATOS (V6 de `ESTRATOS.md`).
Solo se anima con la ventana a la vista y sin tapar, como V5.

## 7. Los escalones

- [x] Q0 -- las decisiones: D1-D4 (a) y D5 (b), el 05-10 (seccion 4). D6 queda pendiente
- [ ] Q1 -- la COMPRENSION en el frontend y el calculo (E0): `[e for x in l if c]`, varios `for` (el JOIN) y `{k: v for ...}`, con su tipo deducido de `e`; un nivel nuevo en `toolchain/lang/titan/GRAMATICA.md`, con su BIEN y sus NO en los dos bancos
- [ ] Q2 -- la comprension en E1 (`toolchain/lang/titan/emisor-x86_64/src/e1/coleccion.rs`): un bucle que llena una lista o un mapa en el monton; el oraculo E0 == E1 y el azar la cubren como a las listas
- [ ] Q3 -- el INDICE POR HASH: es L9 de `docs/plan/PLAN_LISTAS_Y_MAPAS.md`
- [ ] Q4 -- la VARA: el emulador (`toolchain/forge/bmo-lower/src/emu/sistema.rs`) contesta `TASK_OP_ES_NODO`, `TASK_OP_ES_TEXTO` y `TASK_OP_ES_GESTO` sobre un volumen EN MEMORIA hecho con el `bmo-estratos` de verdad (`platform/drivers/storage/estratos/`), el mismo que `toolchain/tools/estratos-fmt` sabe fabricar. Sin esto E1 no tiene contra que medirse
- [ ] Q5 -- los verbos que LEEN (D6): `list`, `history`, `verify` en el frontend y en E1; no conceden nada, como sus puertas
- [ ] Q6 -- los verbos que ESCRIBEN: `save`, `load`, `mark`, `back`, `forget`, `rename`; probados en el emulador contra una imagen, y despues `estratos-fmt --verificar` sobre lo que quedo
- [ ] Q7 -- LIMPIAR (seccion 3): `clean_plan()` y `clean(plan)`, y guardar SIN historial para lo que es cache. Bloquea: C3 de `platform/drivers/storage/estratos/ESTRATOS.md` (el COMPACTAR, primero en imagenes)
- [ ] Q8 -- la CARA (seccion 6): los estratos como nodos unicos por su hash en la ventana de Datos, con el apagado de lo que un plan soltaria y el RESTABLECER
- [ ] Q9 -- guardar POR TROZOS un valor grande (un mundo de gigas): es C2 de `ESTRATOS.md`; `save` lo usa cuando exista, sin cambiar lo que escribe TITAN++
- [ ] Q10 -- lo CALIENTE en INTI: recorrer millones de filas con SIMD segun el perfil de la CPU, cuando el metro diga que la comprension de E1 es el cuello
- [ ] Q11 -- del PROPIETARIO: las leyes nuevas con `--sellar` (`toolchain/tools/titan-leyes/LEYES.txt`)

## 8. ULTRACRAFT: el primer cliente

Un mundo de cubos es, antes que un juego, datos que se dibujan. Minetest
(Luanti), el clon libre de Minecraft, guarda sus trozos en SQLite, una fila
por trozo. Aqui no hace falta SQLite:

```text
   en la memoria     {Sitio: Trozo}        los trozos cargados (nivel 13;
                                           `toolchain/lang/titan/ejemplos/nivel13/mundo.titan`
                                           ya es el primer mundo)
   en el disco       save(mundo, "mundo")  UN guardado = UN estrato
   preguntar         [t for t in trozos if t.cambiado]
   deshacer          back(1), o pulsar un nodo de la seccion 6: "el mundo
                     de hace diez minutos", sin copia ni mod
   la partida buena  mark("antes del dragon"): permanente, clean no la toca
```

Lo demas de ULTRACRAFT (la ventana, los cubos con profundidad y texturas por
VERRANO, lo caliente en INTI, el puente de E2) va en su propio plan.

## 9. Lo que este plan NO es

- **No es un servidor de base de datos.** Ni proceso aparte ni red: la
  pregunta es codigo del programa, y la base es el sistema de ficheros.
- **No es SQL en el kernel.** El kernel no recorre filas ni compara claves:
  da los bloques y confirma los commits (*contratos y formatos, nunca
  cerebros*, `platform/abi/bmo-abi/src/bef/BEF_EXTENSIONES.md`). Una pregunta
  mal escrita en el anillo 0 colgaria la maquina entera.
- **No es un recolector de basura.** Seccion 3.
- **No toca el formato de ESTRATOS.** Usa sus puertas; lo que pida de nuevo
  (C2, C3) se escribe antes en `ESTRATOS.md`, porque ahi equivocarse cuesta
  datos.
