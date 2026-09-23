# PLAN: EXPRIMIR EL DISCO -- por el PERFIL, no por la caja

> Escrito el 2026-09-23. Mismo formato que `PLAN_ALMACENAMIENTO.md`: casillas
> ordenadas, cada una con **que la bloquea** y **como se sabe que quedo hecha**.

El propietario: *"exprimir TODA la velocidad del disco... a BASE de PERFIL, que
pregunta por completo a nivel de atomos que es y aplica por eso: ese es el
motivo PARA EXPRIMIR CON TODO"*.

La primera respuesta a esa frase empezo por *"el fabricante dice 500 MB/s"*, y
eso es justo lo que prohibe la LEY 24: **una estimacion generica es la
estimacion de OTRO proyecto**. Este plan empieza preguntandole al aparato que
es, y cada paso cita el atomo con el que decide.

---

## 0. Lo que el hardware YA contesto (y donde estaba)

| Atomo | Registro / palabra | Valor en el Ryzen | Dice |
|---|---|---|---|
| NCS | `CAP` 12:8 | 31 | **32 ranuras** de comando en el HBA |
| SNCQ | `CAP` 30 | 1 | la controladora **sabe encolar** |
| ISS | `CAP` 23:20 | 3 | hasta **Gen3**, 6 Gb/s = 600 MB/s de datos (8b/10b) |
| S64A | `CAP` 31 | 1 | su DMA alcanza toda la RAM |
| NP | `CAP` 4:0 | 7 | 8 puertos |
| cola, NCQ | IDENTIFY 75, 76 | (en `disco`) | lo que el DISCO admite |
| enlace | IDENTIFY 76, 77 | (en `disco`) | lo que el DISCO negocio |
| sector fisico | IDENTIFY 106, 209 | (en `disco`) | la frontera de alineado |

`cap = 0xEF36FF27` vivia **en un comentario** de
`platform/drivers/storage/ahci/src/arranque.rs`. Y el contraste ya decia el
hallazgo sin suponer nada: **la controladora ofrece 32 ordenes a la vez y el
driver usa una** (`RANURAS_EN_USO = 1`, la ranura 0, esperando girando).

---

## 1. LAS CASILLAS

### [ ] P0 -- COMPLETAR LA PREGUNTA (codigo HECHO 2026-09-23; se cierra con el metal)

Lo que faltaba preguntar, y ya se pregunta:

- el `CAP` del HBA como **dato**, no como comentario: `AhciController::cap`
  (`platform/drivers/storage/ahci/src/controller.rs`);
- el `PxSSTS` del puerto del disco: `bmo_ahci::port_ssts`. Su `SPD` es la
  generacion que negocio **el HBA**; la palabra 77 es la que dice **el disco**.
  Son los dos extremos del mismo cable, y el informe marca si discrepan;
- la **cache de escritura** (palabras 82-85): `padre::Cache` en
  `platform/drivers/storage/identify/src/padre.rs`, con la guarda de la 83 y
  cuatro pruebas (sin guarda no se afirma nada: un `FFFFh` diria "si" a todo).

Salen por `INFO_DISCO_HBA` (0x92) e `INFO_DISCO_CACHE` (0x93), crudos y
descifrados, y los pinta la tabla de `disco` y `informe/DISCO.TXT` del `save`.

**Como se sabe:** el `save` del Ryzen trae en DISCO.TXT las filas `hba`
(`32 ranuras, NCQ, hasta Gen3, DMA 64 bits   CAP 0xEF36FF27`), `port` y `cache`.

### [ ] D0 -- EL METRO (codigo HECHO 2026-09-23; se cierra con el primer numero)

`disco banda [MiB]` (`Ultra_kernel_x86-64/kernel/src/ring0/dev/disk/banda.rs`,
`DISCO_OP_BANDA` 0x03): lee N MiB (64 si no se dice, techo 1024) de la
particion de datos por el camino de verdad del driver, cronometra **solo las
ordenes**, y deja en `INFO_DISCO_BANDA` / `_ORDEN` los MB/s, la orden mas rapida
y la mas lenta, y **cuantos sectores traian datos**.

Tres decisiones, con su motivo:

1. **Solo lee.** La cifra del perfil (`sostenido_mb_s`) es la ESCRITURA despues
   de agotar la cache SLC: decenas de GB escritos, desgaste y un sitio donde
   escribirlos. Eso lo decide el propietario, no un metro que se lanza con una
   orden. Por eso esta medida **no** convierte `Cifra::catalogo(450)` en
   medida: es otra cifra.
2. **El % de sectores con datos va al lado.** Un SSD contesta un sector que
   nunca se escribio desde su mapa, sin leer la NAND. Una banda sobre ceros no
   es la del disco, y el informe lo dice en rojo por debajo del 90 %.
3. **La orden mas lenta contra la mas rapida.** La media esconde a un disco
   que se para a medio camino (sin DRAM, el mapa del FTL); la distancia no.

**Como se sabe:** `disco banda` en el Ryzen da un numero, `cable` dice que
parte del techo Gen3 usa, y ese numero se pega aqui abajo como el ANTES de D1.

### [ ] D1 -- ASINCRONO + WAIT: cero congelones

Emitir, volver, y que la IRQ despierte a quien espera. No depende de un atomo:
es lo que quita la sordera. Las piezas existen (`bmo_ahci::emitir`,
`bmo_ahci::sondear`, `dev/disk/irq.rs`). **Bloquea:** D0 medido (sin el ANTES no
se sabe si D1 gano o solo cambio de sitio el tiempo).

### [ ] D3 -- DMA directo al bloque prestado + PRD multiples

Una orden de hoy pide como mucho 4 MiB (una entrada de PRDT, `DBC` de 22 bits);
el protocolo admite 32 MiB (`READ DMA EXT`, 65.536 sectores). **Como se sabe:**
`disco banda` con la orden de 8 MiB contra la de 4 MiB.

### [ ] D2 -- NCQ: la cola que decide el orquestador

`cola = min(NCS del HBA, profundidad del disco, la parte que el orquestador le
da a quien tiene el foco)`. Tres respuestas, y el kernel decide con las tres:
eso es orquestar. **Bloquea:** D1. Y el TRIM **encolado** es el que tiene
historial de corrupcion (`NO_NCQ_TRIM` de Linux): el TRIM se queda fuera de la
cola hasta que el metal diga otra cosa.

### [ ] D4 -- Tramos alineados al sector fisico (palabra 106)

Si el fisico es de 4 KiB, todo tramo que no empiece y acabe en frontera hace
que el disco reescriba de mas. **Como se sabe:** `sector` en la tabla de `disco`
dice el fisico; si es 512, esta casilla se cierra sin codigo.

### [ ] D5 -- FLUSH solo donde hace falta (palabra 85)

Con la cache **apagada** un WRITE que vuelve OK ya esta en la NAND y la barrera
no compra nada; **encendida** y sin condensadores (el perfil dice `false`), la
barrera es lo unico. Que se pierde si se va la luz **lo decide el propietario**,
no este plan. **Bloquea:** P0 visto en metal (la fila `cache`).

---

## 2. LA REGLA DE ESTE PLAN

Cada decision del disco cita su atomo. Si no hay atomo, se pregunta ANTES de
decidir -- y si el aparato no lo contesta (el bloque de borrado, el TBW), va al
perfil de `platform/shared/bmo-disco-juicio/src/perfil.rs` con su `Origen`.
