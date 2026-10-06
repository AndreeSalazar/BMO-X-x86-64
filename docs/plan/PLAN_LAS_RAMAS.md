# PLAN LAS RAMAS -- ESTRATOS como Git: guias, plantillas, ramas y mezcla por NODOS

> Abierto el **2026-10-05**. El propietario, con la solapa ESTRATOS de F1 ya
> en pie: *"en ESTRATOS sean posible para tener guias simples [...] plantillas,
> guias y porque motivos todo que existen cuantos existen y se pueden usar
> creatividad [...] ESTRATOS tienen que tener algo simple por completo en
> general sobre control de nodos y merge, y eso como estilo de Git pero tambien
> TODOS los nodos son independiente :3"*.
>
> Sigue a `docs/plan/PLAN_LA_BANDEJA.md` (B8, la cara de F1) y a
> `docs/plan/PLAN_LOS_DATOS.md` (ESTRATOS es la base de datos).

---

## 0. La respuesta corta

```text
   la guia       HECHA (R1): F1 cuenta las puertas de ESTRATOS que EXISTEN --
                 39, en 4 familias -- y dice QUE hace cada una y POR QUE,
                 copiado del contrato del ABI; el build dice NO si mienten
   Git           ESTRATOS YA es casi Git: commit, log, tag, revert y objetos
                 por hash estan (seccion 1). Le faltan dos cosas: RAMAS que
                 vivan a la vez, y MEZCLAR
   la mezcla     por NODOS, nunca por lineas: cada nodo es independiente y
                 entero, asi que mezclar es elegir NODOS (seccion 2). No hay
                 marcas de conflicto dentro de un fichero, nunca
   plantillas    una carpeta que se COPIA: en copy-on-write copiar es una
                 entrada nueva que apunta a los mismos nodos -- cuesta un
                 bloque, no los bytes (seccion 4)
```

---

## 1. Lo que ESTRATOS ya es de Git, y lo que no

| Git | ESTRATOS | donde |
|---|---|---|
| objetos por su hash | bloques, nodos y estratos con BLAKE3 | `platform/drivers/storage/estratos/ESTRATOS.md`, secciones 1 y 4 |
| `commit` | cada gesto publica un ESTRATO | `ES_GESTO_*` |
| `log` | la historia: la cadena de padres | `ES_HIST_*`, la solapa ESTRATOS de F1 |
| `tag` | `marcar`: la version con nombre es PERMANENTE | `ES_GESTO_MARCAR` |
| `revert` | `volver(n)`: un estrato nuevo, lo de en medio se queda | `ES_GESTO_VOLVER` |
| `checkout` de una vieja | montar un estrato viejo ES montar | `ESTRATOS.md`, seccion 4 |
| `fsck` | la suma de cada bloque, `verificar` | `ES_NODO_VERIFICAR` |
| **ramas a la vez** | **NO**: el superbloque apunta a UNA punta | `Estrato.padre` en `platform/drivers/storage/estratos/src/lib.rs` |
| **`merge`** | **NO**: un estrato tiene UN padre | el mismo |

La diferencia que importa, y la que pidio el propietario: Git mezcla TEXTO
linea a linea, y cuando no puede escribe `<<<<<<<` dentro del fichero. Aqui
cada nodo es un objeto entero e independiente, asi que **se mezcla por nodos**.

## 2. Mezclar por NODOS

Tres arboles: la BASE (el ultimo estrato que las dos ramas comparten), la
rama A y la rama B. Para cada ruta, se compara QUE NODO hay en cada uno (su
identidad, no su texto):

```text
   base   A      B      sale
   x      x      x      x            nadie lo toco
   x      a      x      a            solo A lo cambio
   x      x      b      b            solo B lo cambio
   x      a      a      a            los dos hicieron lo mismo
   x      a      b      CHOQUE       los dos lo cambiaron distinto: una
                                     PERSONA elige a o b, ENTERO
   -      a      -      a            A lo creo
   x      -      x      -            A lo quito y B no lo toco
   x      -      b      CHOQUE       A lo quito y B lo cambio
```

**Por que por nodos es mejor, y no solo mas facil:**

- Un choque NUNCA deja un fichero a medias: se elige un nodo entero, y el
  que se elige ya era valido (tenia su suma).
- Un binario, una imagen, un `.bex` se mezclan igual que un texto: son nodos.
- La mezcla es un estrato mas: se puede `volver` de ella como de cualquier
  otra cosa, y la base y las dos ramas siguen enteras en la historia.
- La parte que decide es PURA: tres listas de (ruta, nodo) dentro, una lista
  de lo que sale y de los choques fuera. Se prueba en el anfitrion sin disco
  (R2), como todo lo puro de `bmo-estratos`.

[!] Lo que NO hace, dicho: no junta dos ediciones del MISMO fichero en una.
Si los dos tocaron `mundo.titan`, se elige uno de los dos. Para que eso pase
poco, los paquetes de TITAN++ ya estan partidos en modulos chicos: cada
modulo es su nodo.

## 3. Las decisiones del propietario

**DECIDIDAS el 05-10, las tres (a)** (el propietario: *"D1 A, D2 A, D3 A, me
encanta la A"*): una rama es una MARCA; un estrato de mezcla guarda DOS
padres (formato v2, escrito antes en `ESTRATOS.md` y probado en imagenes);
un choque lo resuelve una PERSONA en F1. Y D4 (seccion 4b), tambien (a), el
mismo dia: *"D4 es A"*.

### D1 -- que es una rama

```text
   (a) RECOMENDADA  una MARCA, como hoy: `marcar("pruebas")` deja una version
                    con nombre, y trabajar en ella es `volver` a ella. Una
                    punta sola en el superbloque, sin formato nuevo
   (b)              ramas que viven a la vez: una TABLA de puntas en el
                    volumen. Mas parecido a Git, pero es formato nuevo en el
                    sitio mas delicado del disco
```

### D2 -- que guarda un estrato de mezcla

```text
   (a) RECOMENDADA  DOS padres: el estrato de mezcla apunta a A y a B, y la
                    historia lo PINTA como dos cadenas que se juntan. Es un
                    campo mas en el estrato: formato v2, escrito antes en
                    `ESTRATOS.md` y probado en imagenes antes que en F:
   (b)              un padre, y "mezcla de B" escrito en su motivo: sin
                    formato nuevo, pero la historia pierde el cable de B
```

### D3 -- quien resuelve un choque

```text
   (a) RECOMENDADA  una PERSONA, en F1: el choque se ve como un nodo partido
                    en dos (a y b, cada uno con su sello) y se pulsa uno
   (b)              una regla fija ("gana A"): mas rapido, pero pierde
                    trabajo sin que nadie lo vea
```

### D5 -- donde vive la PUNTA de cada rama (PENDIENTE, encontrado el 06-10)

Al ir a escribir el gesto se vio un hueco en D1 (a), y se dice antes de
construir encima. Si una rama es una MARCA y "cambiar de rama" es `volver` a
ella, `volver` publica un estrato cuyo padre es la punta de AHORA (es un
revert): la rama que se deja pasa a ser ANTEPASADO de la nueva. Ejemplo:

```text
   T1 "main" --volver a X--> T2 (raiz de X, padre T1) --trabajo--> T3
   mezclar "main" (T1) en T3: T1 YA es antepasado de T3 -> "nada que
   mezclar", y lo de main que T2 deshizo no vuelve nunca
```

Y no se puede arreglar mirando solo la historia: una marca solo se
encuentra recorriendo desde la punta, y una punta que se deja de seguir no
se encuentra mas.

```text
   (a) RECOMENDADA  una TABLA DE RAMAS fuera de la historia, como las refs de
                    Git: el superbloque tiene 360 bytes LIBRES (120..480, a
                    cero) y en ellos cabe un puntero a un objeto chico
                    "nombre -> punta". Cambiar de rama es que el superbloque
                    siga OTRA punta (sin estrato nuevo y sin hacer a nadie
                    antepasado de nadie); las marcas siguen siendo versiones
                    permanentes. [!] Un kernel VIEJO que haga commit
                    reescribe el superbloque con ceros ahi y perderia la
                    tabla: se sube `version` del superbloque para que no lo
                    haga sin enterarse
   (b)              ramas dentro de la historia, como hoy: no hace falta
                    formato, pero mezclar una rama de la que se volvio no
                    funciona (el ejemplo de arriba)
   (c)              un fichero `.ramas` en el arbol: sin formato, pero se
                    versiona y se MEZCLA consigo mismo -- la lista de ramas
                    tendria choques
```

## 4b. Nodos ULTRA independientes: independizar es una DECISION

El propietario, al decidir D1-D3: *"esos nodos sean independiente [...] ese
nodo que aun hereda, se corte por completo la herencia [...] otros que nace con
dependencias el ultimo es independiente dependiendo de la decision"*.

Hay DOS independencias, y una ya esta garantizada:

```text
   LO QUE ES       independiente SIEMPRE, ya hoy: nada se sobreescribe, asi
                   que cambiar un nodo NO puede cambiar otro. Un cambio es un
                   nodo nuevo; el de antes sigue entero
   DONDE VIVE      COMPARTIDO a veces: una copia de una plantilla, o dos
                   versiones del mismo fichero, apuntan a los MISMOS bloques
                   hasta que algo cambia. Esto es lo que "aun hereda"
```

**Independizar** corta lo segundo: los bloques del nodo se escriben aparte,
en su sitio propio. El `BlockPtr` cambia su DONDE (`lba`, `off`) y conserva
su QUE (`hash`), asi que para la mezcla (`mezcla.rs`) y para la historia es
el mismo nodo; solo deja de compartir disco con nadie.

```text
   cuesta         los bytes del nodo, otra vez (compartir era gratis)
   da             que un sector malo no se lleve a dos nodos a la vez; que se
                  pueda sacar o mover el nodo sin arrastrar a otro; y que en
                  F1 se VEA: un nodo que comparte lleva un cable fino hasta
                  con quien comparte, y al independizarlo el cable se CORTA
   no da          independencia de LO QUE ES: esa ya la tenia
```

### D4 -- cuando nace un nodo de una copia o de una rama (DECIDIDA: a)

```text
   (a) RECOMENDADA  nace COMPARTIENDO, y se independiza cuando una persona lo
                    decide (en F1, sobre el nodo, o en F12): gratis hasta que
                    importa, y la decision queda a la vista
   (b)              nace YA independiente: cada copia escribe sus bytes. Nunca
                    comparte nada, pero copiar una plantilla de un GiB cuesta
                    un GiB
```

[!] Saber CON QUIEN comparte un nodo es comparar los bloques de su arbol con
los de los otros: ESTRATOS no lleva cuentas de referencias (el espacio es una
resta, `ESTRATOS.md` tramo 4), asi que es una busqueda, no un numero guardado.
Para una carpeta en pantalla es barato; para un volumen entero, otra cosa.

## 4. Plantillas

Una plantilla es una CARPETA de ESTRATOS que se copia para empezar algo: un
paquete de TITAN++, un mundo de ULTRACRAFT, la carpeta de un juego. En
copy-on-write copiar una carpeta es UNA entrada nueva que apunta a los mismos
nodos: **no se copia ni un byte** hasta que se cambia algo, y entonces solo
ese nodo. Es lo que el propietario llama *nodos independientes*: la copia y
la plantilla comparten lo que no cambio y se separan nodo a nodo.

```text
   plantillas/          una carpeta del volumen; cada hija, una plantilla
     <nombre>/
       LEEME.txt        QUE es y POR QUE existe: la guia la lee de aqui
       ...              lo que trae
```

La guia de F1 las CUENTA y las lista igual que las puertas: de lo que hay, no
de una lista escrita. Copiar una carpeta entera es C4 de `ESTRATOS.md`.

## 5. Los escalones

- [x] R1 -- HECHO el 05-10: la GUIA de ESTRATOS en F1 (sub-solapa GUIA de ESTRATOS, `Ultra_userspace/apps/taller/src/strata_guide.rs`): las 39 puertas que existen, en 4 familias, con QUE y POR QUE, generadas del contrato por `toolchain/tools/estratos-guia/guia.py` (`--check` en el build). Tres puertas del contrato ganaron su linea propia para que la guia pudiera explicarlas
- [x] R2 -- HECHO el 05-10: la MEZCLA PURA (`platform/drivers/storage/estratos/src/mezcla.rs`): base, A y B como listas de (ruta, nodo) -> `Queda` o `Choque`, comparando el QUE (el BLAKE3) y no el DONDE, rutas sin distinguir mayusculas como las entradas, y `base()` para el ultimo estrato comun; 6 pruebas en el anfitrion con la tabla de la seccion 2 fila a fila; compila sin `std`
- [x] R3 -- las decisiones D1-D4 del propietario, las cuatro (a), el 05-10 (secciones 3 y 4b)
- [x] R4a -- HECHO el 05-10: el FORMATO v2, el SEGUNDO PADRE (`SegundoPadre`, `Estrato::mezcla` en `platform/drivers/storage/estratos/src/lib.rs`), escrito antes en `platform/drivers/storage/estratos/ESTRATOS.md` ("las ramas y la mezcla"): en los 16 bytes que v1 dejaba a cero, asi que un v1 se lee como v2 sin segundo padre y un kernel v1 lee un v2 entero; 3 pruebas de los dos sentidos; el kernel compila
- [x] R4b -- HECHO el 05-10: `toolchain/tools/estratos-mezcla` construye el arbol MEZCLADO sobre una imagen, en el anfitrion: la base por los DOS padres de cada estrato, aplanar base, A y B, decidir con `mezcla.rs`, preguntar cada choque (D3), escribir SOLO las carpetas (los ficheros apuntan a los nodos que ya estan: D4, 3 bloques para mezclar una raiz) y UN estrato de dos padres, con el orden de `estratos-put`, y releer lo publicado. 5 pruebas sobre imagenes (un choque elegido, no copiar, una rama que ya estaba dentro no escribe ni un byte, la base por el segundo padre, un nombre Latin-1); y `estratos-fmt --verificar` dice OK en las cinco
- [x] R4c-1 -- HECHO el 05-10: la mezcla POR CARPETAS, la forma que puede usar el kernel. `mezcla::por_carpeta` (en `platform/drivers/storage/estratos/src/mezcla.rs`, sin `alloc`): la regla de tres sobre las entradas de UNA carpeta; lo que un solo lado toco entra ENTERO sin leerlo, y solo se baja donde los dos cambiaron. `estratos-mezcla` la usa de verdad (`decide::por_arbol`) y conserva la plana como ORACULO: la prueba al azar compara las dos con las tres respuestas a un choque (2.000 semillas sin una diferencia) y publica cada mezcla; `estratos-fmt --verificar` dice OK en 25 imagenes. El azar encontro que el hash de una carpeta REESCRITA cambia aunque su contenido no: el hash de carpeta es un atajo, nunca la verdad
- [x] R4c-2a -- HECHO el 06-10: EL MOTOR sin `alloc` (`platform/drivers/storage/estratos/src/motor_mezcla.rs`): tablas FIJAS por nivel que presta quien llama (64 entradas por lado, las de `MAX_ENTRIES` del kernel), dos pasadas (CONTAR bloques y choques sin escribir; ESCRIBIR con las mismas respuestas) y la regla unica `mezcla::regla`. `estratos-mezcla` mezcla YA con este motor (el mismo que correra el kernel) y lo compara al publicar con la mezcla de carpetas con `Vec`; a quien elige se le pregunta UNA vez por choque. 2.000 semillas al azar sin diferencia, una carpeta de 50 (lista de dos bloques) bien, una de 70 se DICE sin escribir un byte, y `estratos-fmt --verificar` OK en 25 imagenes
- [ ] R4c-2b -- el gesto en el KERNEL (`ES_GESTO_*`): el motor con sus tablas en `static` (unos 38 KB por nivel de carpetas), los choques en dos tiempos (listar por la puerta, despues elegir) y el commit de siempre. Bloquea: D5
- [ ] R4c-3 -- del PROPIETARIO: la mezcla en F: en el Ryzen, despues de las imagenes
- [ ] R5 -- la MEZCLA en F1: las dos cadenas que se juntan en la solapa HISTORIA, y cada choque como un nodo partido que se pulsa (D3)
- [ ] R6 -- PLANTILLAS: la carpeta `plantillas/`, su LEEME por plantilla, la guia que las cuenta, y "usar" = copiar la carpeta (pide C4 de `platform/drivers/storage/estratos/ESTRATOS.md`)
- [ ] R8 -- INDEPENDIZAR puro (seccion 4b): dados los bloques de dos nodos, cuales comparten; con sus pruebas en el anfitrion
- [ ] R9 -- el gesto INDEPENDIZAR en el kernel: reescribir los bloques compartidos de un nodo en su sitio propio y publicar UN estrato (el QUE no cambia)
- [ ] R10 -- en F1, en tiempo real: el cable fino de "comparte con" y el corte al independizar
- [ ] R7 -- del PROPIETARIO: las leyes de la mezcla (que nunca se pierde un nodo sin que una persona lo vea) con `--sellar`

## 6. Lo que este plan NO es

- **No es Git encima de ESTRATOS.** No hay `.git/` ni un segundo almacen: la
  historia es la del volumen.
- **No mezcla lineas.** Seccion 2: por nodos, entero, siempre.
- **No toca el formato sin escribirlo antes.** D1 (b) y D2 (a) son formato; se
  deciden aqui y se escriben en `ESTRATOS.md` antes de tocar un sector.
