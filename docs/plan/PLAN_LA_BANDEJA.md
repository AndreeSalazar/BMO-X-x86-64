# PLAN LA BANDEJA -- F2 donde viven TODOS los archivos, F12 el volumen, F1 todo nodos

> Abierto el **2026-10-05**, el mismo dia que `docs/plan/PLAN_LOS_DATOS.md`
> y saliendo de el. El propietario: *"el F2 se convierte donde viven TODOS los
> archivos pero (aqui tienen que ver que SSD o NVMe que mi kernel lee y que
> deja listo en bandeja de oro o diamante para mi :3) y el F12 se convierte en
> donde alli puedes verificar [...] volumen control total"*. Y antes: *"esos
> archivos SON SIEMPRE independiente [...] cuando buildea ese resultado
> siempre vive independiente para no tener choques"*.
>
> Y de paso, dos cosas del escritorio: los ICONOS se van (F2 los sustituye
> entero) y el globo del puntero lo anuncia.

---

## 0. La respuesta corta

```text
   F2    LOS ARCHIVOS    todos, de todos los discos, en un sitio: juegos,
                         apps, musica, fuentes y lo que sale de construir.
                         Arriba, LA BANDEJA: los discos que el kernel lee,
                         cada uno con lo que el kernel sabe de el
   F12   EL VOLUMEN      control total de ESTRATOS: verificar, la historia,
                         marcar, volver, limpiar -- y sus nodos animados
   F1    EL TALLER       TODO es un nodo, y no solo TITAN++: C entra ya,
                         lo demas poco a poco
   la regla que lo une   lo que sale de construir es SIEMPRE un nodo NUEVO
                         e independiente, que dice de donde salio
                         (`:origen`). Nunca pisa nada: no hay choques
```

---

## 1. Las decisiones del propietario (05-10)

```text
   E1   F2 = donde viven TODOS los archivos, con la bandeja de discos
        F12 = el volumen, control total (verificar y lo demas)
   E2   en F1 entran TITAN++ y C; lo demas, poco a poco
   E3   la frontera de ESTRATOS: los verbos que LEEN, para cualquier
        programa; los que ESCRIBEN, para lo que se lanza desde F1 o F2
        (escrito en `docs/plan/PLAN_LOS_DATOS.md`, D7)
   y    F1: TODOS son nodos
   y    los iconos del escritorio se eliminan; F2 los compensa entero
   y    el globo dice donde se administran los archivos: F2, MIAU
```

## 2. Lo que ya esta, y no se reinventa

| pieza | donde | que da |
|---|---|---|
| `:origen` en el formato | `platform/drivers/storage/estratos/src/objects.rs` (`ATTR_ORIGEN`) | el sitio para decir *de que fuente salio, con que compilador, cuando* (`ESTRATOS.md`, seccion 4) -- existe y nadie lo escribe todavia |
| la ventana de Datos | `Ultra_userspace/services/director/src/scene/data/mod.rs` | F12 HOY: el centro de control de ESTRATOS, con solapas |
| el explorador del TALLER | `Ultra_userspace/apps/taller/src/explorer.rs`, `Ultra_userspace/apps/taller/src/store.rs` | arbol, renombrar, mover, crear -- sobre ESTRATOS, a la manera del propietario |
| el disco ajeno | `Ultra_kernel_x86-64/kernel/src/ring0/dev/disk/ajeno.rs` | el segundo SSD SATA (Personal D:, NTFS), SOLO LECTURA con dos cerrojos |
| el perfil del disco | `docs/plan/PLAN_EXPRIMIR_EL_DISCO.md` (P0) | lo que el aparato contesto: 32 ranuras, NCQ, Gen3, la cache encendida |
| las teclas | `Ultra_userspace/services/director/src/desktop/keys/editor.rs` | HOY F2 escribe `info`; se sigue pudiendo teclear |
| el globo | `Ultra_userspace/services/director/src/desktop/globo.rs` | la rueda de `SABIAS`: cada linea tiene que ser VERDAD, y caber en 64 letras |
| los iconos | `Ultra_userspace/services/director/src/desktop/mouse/iconos.rs` y la rejilla de `Ultra_userspace/services/director/src/main.rs` | lo que se va, despues de que F2 lo haga todo |

## 3. LA BANDEJA: los discos, como el kernel los deja servidos

Lo que el kernel lee HOY, medido en el codigo y no supuesto:

```text
   el SSD de BMO-X     SATA, AHCI     FAT32 (arranque) + ESTRATOS (F:)
   el SSD Personal     SATA, AHCI     NTFS (D:), SOLO LECTURA: dos cerrojos
   el NVMe (C:)        NO SE MIRA     no hay driver de NVMe, y es el Windows
                                      del propietario: no tocarlo es a
                                      proposito (`ajeno.rs`, `ESTRATOS.md`)
```

Cada disco sale en la bandeja con lo que el kernel SABE de el (modelo, serie,
enlace, cola, cache: el perfil de P0) y con su RANGO. El rango no es la
velocidad: es **lo que te garantiza**, y por eso se puede ganar:

```text
   DIAMANTE   ESTRATOS     escribe SIN pisar, suma en cada bloque, historia
   ORO        FAT32        escribe, pero PISA: lo de antes no vuelve
   PLATA      NTFS (D:)    se lee entero; escribir, no (los cerrojos)
   CERRADO    el NVMe      no se mira; el dia que haya driver, entra
                           como PLATA (solo lectura) y no antes
```

[!] Un disco que el kernel no ha leido NO sale "vacio": sale CERRADO y con el
motivo. Pintar un disco que no se leyo como si no tuviera nada seria mentir
con la cara.

## 4. Independientes: lo que sale de construir

```text
   construir    un .titan o un .c  ->  un .bex
   hoy          el resultado es un fichero mas: el siguiente lo PISA, y nada
                dice de donde salio
   aqui         el resultado es un NODO NUEVO de ESTRATOS, con
                  :datos     el .bex
                  :origen    las fuentes (por su hash), el compilador y cuando
                el de ayer sigue entero en la historia; dos construcciones
                no chocan porque no comparten nodo
```

F2 pinta el CABLE de cada resultado a sus fuentes leyendo `:origen` -- y al
reves: pulsar una fuente dice que salio de ella. No hay base de datos aparte
que se desincronice: el cable vive en el propio nodo.

**Lo que se parece fuera, dicho:** Nix y Guix en Linux guardan cada
construccion por su hash en una carpeta aparte (`/nix/store`). Aqui no es una
carpeta encima del disco: es el sistema de ficheros, con su historia y su
suma. Eso es lo que solo tiene BMO-X.

## 4b. "ESTA ABIERTO": el cuadrito que Windows hace mal (06-10)

El propietario: *"Microsoft es muy pesimo para lock file [...] mi caso seria
mostrar el cuadrito y dice 'esta abierto no se puede eliminar a menos:' 2
opciones: mostrar la ventana ESPECIFICA [...] o forzar el cierre y eliminar, y
el ultimo es aceptar como 'okey'"*. Y despues: *"esta bien aplicar, por algo el
kernel mi orquestador es basicamente el mejor mayordomo del mundo"*.

** EN ESTRATOS BORRAR NO FALLA NUNCA, y por eso el cuadrito casi no sale.
Borrar es DEJAR DE NOMBRAR: un `Archivo` abierto lee de SU nodo, no del
nombre (`Ultra_kernel_x86-64/kernel/src/ring0/obj/estratos.rs`), asi que el
nombre se va y el programa sigue viendo su version hasta que la cierre. Ahi
basta una linea: *"ship.titan esta abierto en TALLER: seguira viendo su
version hasta cerrarlo"*.

El cuadrito sale donde algo de verdad esta AGARRADO:

```text
   D: (FAT32)          sobreescribe: borrar con alguien leyendo SI rompe
   expulsar un disco   desde la BANDEJA, con un fichero suyo abierto
   el recolector       quiere soltar una version que alguien tiene abierta
```

Y sus tres botones, mejor que en Windows porque el kernel SABE quien:

| boton | que hace | por que se puede aqui |
|---|---|---|
| **MOSTRAR LA VENTANA** | lleva a la ventana EXACTA que lo tiene | el permiso de abrirlo es una capability en la tabla del kernel, con el pid de quien la tiene: no hay que adivinar como el Monitor de recursos |
| **FORZAR** | le QUITA el permiso a ese programa (revoca la capability) y despues borra; el programa sigue vivo y se entera al leer | Windows mata el proceso entero; aqui se quita una cosa, no un programa |
| **OKEY** | no hace nada | -- |

[!] FORZAR lo pulsa una PERSONA, nunca un programa: es la misma regla que
`marcar` y que elegir un choque (D3 de `docs/plan/PLAN_LAS_RAMAS.md`).

## 5. F1: TODOS son nodos

Hoy el TALLER es un editor de nodos de TITAN++ (`docs/plan/PLAN_TALLER.md`,
seccion 8). Pasa a ser de TODO lo que se construye, por tipos de nodo:

```text
   fuente TITAN++    como hoy: el cable es un `use`
   fuente C          un .c o un .h; el cable es un #include
   resultado         un .bex, con su cable de `:origen` a las fuentes
   recurso           una imagen, un sonido: se cuelga de quien lo usa
```

[!] **Mostrar y construir no son lo mismo, y se dice:** F1 puede MOSTRAR y
organizar un .c en cuanto sepa leer sus #include; CONSTRUIRLO DENTRO de BMO-X
pide que el compilador de C viva dentro del .bex (el escalon 6 de
`docs/plan/PLAN_TALLER.md`, el autohospedaje). TITAN++ es el que esta mas
cerca. Hasta entonces, un resultado construido fuera entra con su `:origen`
igual: dice de donde salio aunque no saliera de F1.

**Y cada nodo, UNICO** (el propietario: *"en NODOS por completo tienen que
tener representacion unicas en mi TALLER"*): sea del tipo que sea, lleva un
sello sacado del hash de lo que representa, como los astros de la solapa
ESPACIO (`Ultra_userspace/apps/taller/src/astros.rs`). Dos nodos distintos no
se pintan igual nunca, y uno que cambia de contenido cambia de cara. Asi se ve
en la maqueta: `docs/arte/maqueta_taller_estratos.html`.

El propietario ya lo probo y espera bugs de sorpresa: se anotan en la seccion
8, con lo que se vio, y se arreglan antes de crecer.

## 6. Los escalones

- [x] B0 -- las decisiones E1-E3 del propietario, escritas aqui (seccion 1)
- [ ] B1 -- F2 abre LOS ARCHIVOS: una app propia como el TALLER (`sys/archivos.bex`), que el DIRECTOR lanza en F2 como lanza F1; `info` se sigue tecleando. Es el explorador de ESTRATOS que la frontera ya preve (`VALKYRIE-ABI/FRONTERA.txt`, fila `ES_`)
- [ ] B2 -- el GLOBO, en el MISMO commit que B1 (una linea de `SABIAS` tiene que ser verdad el dia que sale): `administrar archivos (juegos, apps, musica...)? F2 MIAU :3` -- 58 letras, cabe en las 64
- [ ] B3 -- LA BANDEJA (seccion 3): los discos que el kernel lee, con su perfil y su rango; el NVMe CERRADO con su motivo
- [ ] B4 -- TODOS los archivos: ESTRATOS, FAT32 y D: en un arbol, cada uno diciendo de que disco es; abrir hace lo que toca (un .bex corre, un .exe por PROTON-X, la musica suena, una imagen se ve)
- [ ] B5 -- los ICONOS se van, y no antes de B4: primero el inventario de lo que lanza cada icono de la rejilla, y cada uno con su sitio en F2; despues se quita la rejilla (`Ultra_userspace/services/director/src/main.rs`, `Ultra_userspace/services/director/src/desktop/mouse/iconos.rs`)
- [ ] B6 -- INDEPENDIENTES (seccion 4): construir escribe un nodo NUEVO con `:origen`; F2 pinta el cable de un resultado a sus fuentes y al reves
- [ ] B7 -- F12, el VOLUMEN con control total: verificar el volumen entero, la historia, marcar, volver, y limpiar con su lista (Q7 y C3), con los nodos animados unicos de `docs/plan/PLAN_LOS_DATOS.md` (Q8)
- [ ] B8 -- F1, TODOS nodos: el tipo de nodo C (sus #include como cables), el resultado y el recurso, junto a los de TITAN++
- [x] B8a -- la MAQUETA de F1 con ESTRATOS servido, C entre TITAN++, el resultado con su `:origen` y cada nodo con su sello unico: `docs/arte/maqueta_taller_estratos.html` (05-10)
- [ ] B8 por partes, en el orden de la casa (lo que no toca nada, primero), decidido el 05-10 al pedir el propietario *"a aplicar, empezando por el orden"*:
  - [x] T1 -- HECHO el 05-10: cada nodo del GRAFO lleva su SELLO en la cabecera (`seal` en `Ultra_userspace/apps/taller/src/astros.rs`), de su nombre y la suma de sus bytes (`FileEntry::sum` en `platform/shared/titan-lector/src/package.rs`, con su prueba: tocar un fichero mueve solo su suma); visto con `cara-taller`
  - [x] T2 -- HECHO el 05-10: la solapa ESTRATOS (`Ultra_userspace/apps/taller/src/strata.rs`), tercera de la tira junto a GRAFO y ESPACIO: cada version un nodo con su sello, en cadena de la mas vieja a AHORA; las marcadas brillan con su nombre; clic o flechas eligen; ENTER dos veces RESTABLECE (`volver(n)`, sin perder lo de en medio), Esc retira la pregunta. Lee la historia al abrir la solapa y cuando la generacion se movio, nunca por fotograma; vive en un bloque propio (`pila.py --ring3`: 41.456 de 65.536). Fotos con `cara-taller`: `estratos.png`, `estratos_pregunta.png`
  - [ ] T2b -- lo que la solapa todavia NO hace, dicho: el sello sale de cuando, quien y el nombre porque `ES_HIST_*` no da la SUMA de cada estrato (una puerta mas, y el sello pasa a ser el BLAKE3); VERIFICAR por version y el PLAN DE LIMPIEZA piden C3 de `platform/drivers/storage/estratos/ESTRATOS.md`; MARCAR pide escribir un nombre en F1
  - [ ] T3 -- `use estratos` como nodo SERVIDO en el GRAFO, con sus verbos como pines
  - [ ] T4 -- los nodos de C, con sus `#include` como cables
  - [ ] T5 -- los cables de `:origen` de un resultado a sus fuentes (pide B6)
- [ ] B8b -- el ESPEJO de esa maqueta: `cara-taller` (`toolchain/tools/espejo-cara/src/bin/cara_taller.rs`) pinta con el codigo de verdad lo que la maqueta dibuja, y `foto.js` hace la foto de la regla
- [ ] B10 -- EL CUADRITO "ESTA ABIERTO" (seccion 4b): la pregunta al kernel "quien tiene abierto esto" (las capabilities con su pid), la linea de ESTRATOS que no bloquea, y el cuadrito de tres botones en D:, al expulsar y ante el recolector; FORZAR revoca la capability, no mata el programa
- [ ] B9 -- los bugs de sorpresa que vea el propietario al probar F1 (seccion 8), cada uno arreglado con su prueba

## 7. Lo que este plan NO es

- **No es otro gestor de ficheros encima de ESTRATOS.** F2 muestra lo que hay
  y pide los gestos que ESTRATOS ya sabe; no guarda nada por su cuenta.
- **No escribe en el NVMe ni en D:.** La bandeja los MUESTRA; los cerrojos de
  `ajeno.rs` siguen mandando.
- **No compila C dentro de BMO-X todavia.** Seccion 5.
- **No quita los iconos antes de tiempo.** B5 espera a que F2 lance todo lo
  que ellos lanzaban: un escritorio sin iconos y sin F2 completo deja cosas
  sin puerta.

## 8. Los bugs que se vean

*(vacio: se anota cada uno con la fecha, lo que se hizo y lo que se vio)*
