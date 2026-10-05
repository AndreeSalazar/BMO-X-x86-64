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

**PENDIENTES.** Cada una con su recomendada.

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
- [ ] R2 -- la MEZCLA PURA en `bmo-estratos`: base, A y B como listas de (ruta, nodo) -> lo que sale y los choques, con la tabla de la seccion 2 como pruebas en el anfitrion
- [ ] R3 -- las decisiones D1-D3 del propietario, escritas aqui (seccion 3)
- [ ] R4 -- el gesto MEZCLAR en el kernel (`ES_GESTO_*`): construir el arbol de lo que sale con los nodos ya escritos (no copia bytes) y publicarlo como UN estrato; con D2 (a), formato v2 en imagenes antes que en F:
- [ ] R5 -- la MEZCLA en F1: las dos cadenas que se juntan en la solapa HISTORIA, y cada choque como un nodo partido que se pulsa (D3)
- [ ] R6 -- PLANTILLAS: la carpeta `plantillas/`, su LEEME por plantilla, la guia que las cuenta, y "usar" = copiar la carpeta (pide C4 de `platform/drivers/storage/estratos/ESTRATOS.md`)
- [ ] R7 -- del PROPIETARIO: las leyes de la mezcla (que nunca se pierde un nodo sin que una persona lo vea) con `--sellar`

## 6. Lo que este plan NO es

- **No es Git encima de ESTRATOS.** No hay `.git/` ni un segundo almacen: la
  historia es la del volumen.
- **No mezcla lineas.** Seccion 2: por nodos, entero, siempre.
- **No toca el formato sin escribirlo antes.** D1 (b) y D2 (a) son formato; se
  deciden aqui y se escriben en `ESTRATOS.md` antes de tocar un sector.
