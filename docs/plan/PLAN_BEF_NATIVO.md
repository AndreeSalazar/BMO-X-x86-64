# PLAN: BEF nativo -- un formato de BMO-X x86-64, no un ELF con otro nombre

> Pedido por Eddi el 2026-09-19: *"investigar BEF: no quiero que sea ELF como
> tipico, empieza por completo BEF"*.
>
> Estado: **HECHO el 2026-09-19, B0-B6** -- *"BEF reemplaza el ELF
> maestro, DALE"*: el formato se llama **BEF2**, es PROPIO, y **BEF1 murio**
> (un formato, no dos). La regla congelada esta reescrita en
> `platform/abi/bmo-abi/src/bef/BEF_EXTENSIONES.md`. Las secciones 1 y 4 de abajo
> describen lo que HABIA y lo que se propuso; la implementacion es
> `platform/abi/bmo-abi/src/bef2/`. **B8 y B7 HECHOS el 2026-09-20**: Ring
> 0 lee REGIONES, la "pintura al reves" de B3 se fue, y LA FIRMA ES DEL
> INDICE (y de cada anexo). Queda B9 (medir paginas), y el METAL para todo.
> El 20-09 el Ryzen arranco con BEF2 (B0-B6 + B8) y DOOM se jugo.

---

## 1. Lo que BEF es hoy, medido

BEF1 es la idea central de ELF: **una cabecera y una tabla de secciones
tipadas**, y la regla de oro de ELF escrita tal cual en
`platform/abi/bmo-abi/src/bef/BEF_EXTENSIONES.md` (*"una seccion desconocida se salta: es lo que ha
mantenido vivo a ELF treinta anios"*).

### La cabecera (48 B) -- que lee de verdad alguien

| Offset | Campo | Quien lo lee | Veredicto para un BMO-X solo x86-64 |
|---|---|---|---|
| 0 | `magic` `BEF1` | puerta | se queda |
| 4 | `version_major/minor` (2 x u16) | puerta | una version basta, y puede ir en el magic |
| 8 | `flags` (u32) | puerta, validador | de 16 bits definidos, VIVEN 5 (abajo) |
| 12 | `arch` | puerta | siempre `0x01`; existe para rechazar ARM "por su nombre" |
| 13 | `endianness` | puerta | siempre `0`; se justifico "por si PowerPC" |
| 14 | `cpu_features` (u16) | puerta | **se queda y crece**: es lo mas x86 del formato |
| 16 | `abi_version` (2 x u8) | puerta | tercera version del mismo fichero |
| 18 | `_reserved` (6 B) | nadie | -- |
| 24 | `entry_offset` | puerta | se queda |
| 32 | `section_table_offset` | puerta | ver seccion 4 |
| 40 | `section_count` | puerta | ver seccion 4 |
| 44 | `total_size` (u32) | puerta | techo de 4 GiB |

**Banderas.** Vivas: `EXECUTABLE`, `OBJECT`, `SIGNED`, `HAS_MANIFEST`,
`WANTS_SCREEN`. Sin consumidor: `PIE` (siempre puesta), `USES_BAREX` (siempre
puesta), `PROVENANCE_PE/ELF` ("devorar" no existe), `SHARED_LIBRARY` (retirada
el 17-09). Rechazadas: `COMPRESSED`, `HOT_RELOADABLE`, y desde el corte 0
`HAS_TLS` y `HAS_SHADERS`. **No se pueden quitar sin romper los `.bex` que
existen**: todos llevan `PIE | USES_BAREX` y el validador rechaza un bit que no
conoce. Por eso esto es un formato NUEVO y no una limpieza.

### La entrada de seccion (48 B) = `Elf64_Shdr` y `Elf64_Phdr` fundidos

| Campo | Quien lo lee |
|---|---|
| `kind`, `flags`, `file_offset`, `file_size`, `mem_size`, `alignment` | puerta y cargador |
| `virt_addr` | **nadie** (en una seccion; el de `Symbol` si) |
| `hash_index` | lo escribe `writer`, **no lo lee nadie** |
| `_pad` (3 B), `_reserved` (4 B) | nadie |

Banderas de seccion: `READ`, `WRITE`, `EXEC`, y `COMPRESSED`, `PAGE_ALIGNED`,
`HUGE_ALIGNED`, `LAZY`, `HASHED`, `SYNTHETIC` sin consumidor.

### *** El hallazgo que no es de formato: NO HAY PAGINAS DE SOLO LECTURA

`admitir.rs:606`: `let writable = s.flags & SECTION_FLAG_EXEC == 0;` y
`vmm/amarilla.rs::map_page_tipo` pone NX solo si la pagina es escribible.
El kernel tiene **dos** estados: RX (codigo) y RW+NX (todo lo demas).
**`RoData` se mapea escribible**: la constante de un programa se puede pisar,
y un literal de C modificado no da `#PF`, da otro programa. Es herencia
directa de decidir el permiso por una BANDERA y no por lo que la seccion ES.

### Simbolos y relocs: el vocabulario de ELF

- `Symbol`: `Local/Global/Weak`, visibilidad, `File`, `Tls` -- el `st_info` de
  ELF. Solo lo necesitan los objetos (`.bo`) al enlazar; un ejecutable no.
- `Relocation`: `Abs64` (`R_X86_64_64`), `Rel32` (`R_X86_64_PC32`), `Got64`
  (`R_X86_64_GLOB_DAT`: enlazado dinamico, ya REHUSADO en `objeto.rs`) y
  `SeccionAbs64`, la unica que BMO invento porque la necesitaba.

---

## 2. Lo que un `.bex` NECESITA en BMO-X, y nada mas

Lo que hace el cargador del kernel, en orden:

1. saber que es un BEX de esta maquina y de este ABI;
2. saber cuanto estado de CPU tiene que preservar (`cpu_features`);
3. mapear **cuatro regiones**: codigo (RX), constantes (R), datos (RW), ceros (RW);
4. aplicar relocaciones de **un solo tipo** en un ejecutable estatico: "aqui
   va la direccion de region+offset" (`SeccionAbs64`);
5. comprobar hashes/firma y requisitos;
6. saltar al punto de entrada.

Todo lo demas (manifiesto, recursos, katanas, simbolos de depuracion) es para
OTRO, y el kernel no lo abre.

---

## 3. Corte 0 -- HECHO el 2026-09-19 (`31602e7f`)

Dentro de BEF1, sin romper ningun `.bex`: la puerta y el validador RECHAZAN
imports, exports y TLS (`Falta::EnlazadoDinamico`) y las banderas `HAS_TLS` y
`HAS_SHADERS`; fuera once tipos de seccion sin productor y `BefMagic` (la
deteccion de `\x7FELF` y `MZ`). Censo: 49 imagenes, 0 afectadas.

---

## 4. La propuesta: BEX2, disenado desde lo que BMO-X hace

```text
   cabecera fija (64 B, alineada a 64 = una linea de cache)
     0   magic        "BEX2"                  version en el magic: una, no tres
     4   abi          u8   (2)
     5   banderas     u8   EJECUTABLE | OBJETO | FIRMADO | QUIERE_PANTALLA
     6   reservado    u16  = 0, rechazado si no
     8   xcr0         u64  los componentes XSAVE que el programa usa (SSE,
                           AVX, AVX-512...) -- el mapa EXACTO que el kernel
                           necesita para preservar su estado. x86 puro.
    16   entrada      u32  offset dentro de CODIGO
    20   anexos       u32  cuantos anexos hay (tabla detras de la cabecera)
    24   CODIGO       {offset u32, bytes u32}          RX   siempre
    32   CONSTANTES   {offset u32, bytes u32}          R+NX siempre
    40   DATOS        {offset u32, bytes u32}          RW+NX siempre
    48   CEROS        {bytes u32}                      RW+NX siempre
    52   total        u32  bytes del fichero
    56   hash_cab     u64  primeros 8 B del BLAKE3 de la cabecera + anexos
   64..  anexos: {tipo u8, pad, bytes u32, offset u32} x N   (relocs, firma,
                  requisitos, recursos, manifiesto, katanas...)
```

Lo que cambia, y por que es BMO y no ELF:

- **El permiso lo da el HUECO, no una bandera.** Las cuatro regiones tienen
  sitio fijo en la cabecera: no hay forma de escribir un `.bex` con codigo
  escribible ni constantes escribibles. Cierra por construccion el hallazgo
  de la seccion 1.
- **Sin tabla de secciones para lo que se carga.** El kernel lee 64 bytes y
  sabe todo lo que tiene que mapear. Los anexos son la unica lista, y son
  todos para OTRO salvo tres (relocs, firma, requisitos).
- **`xcr0` en vez de `cpu_features`.** El dato que Ring 0 necesita para
  `XSAVE/XRSTOR` es literalmente la mascara de XCR0. Declararla es un contrato
  que el kernel comprueba contra el XCR0 de la maquina de un vistazo.
- **Un solo tipo de reloc en un ejecutable** (region + offset -> 64 bits). Los
  `.bo` conservan simbolos y `Rel32`, que `bmo-enlazar` consume y no pasa al
  `.bex`.
- **Sin `arch` ni `endianness`**: el magic ya dice "BMO-X x86-64". Un binario
  de otro repositorio tendria otro magic, y se rechaza por el primero.
- **Cabecera de 64 B alineada a 64**: una linea de cache, un solo acceso.

---

## 5. Lo que tiene que decidir Eddi antes de escribir una linea

1. **Corte limpio o transicion.** BEX2 rompe todos los `.bex`: los cinco
   payloads del kernel (se regeneran con el build), DOOM y las apps del
   Kingston (hay que recompilar y desplegar). Propuesta: corte limpio, el
   kernel solo acepta BEX2 -- un formato, no dos.
2. **Paginas alineadas en el fichero, o compacto.** Si cada region empieza en
   un multiplo de 4 KiB del fichero, el cargador puede REFLEJAR paginas en vez
   de copiar (`bmo-ram-quirofano`: "reflejar en vez de copiar"). Cuesta
   relleno: el 07-08 se quito a proposito para encoger los `.bex`. Medir DOOM
   con y sin antes de elegir.
3. **La regla congelada** (`BEF_EXTENSIONES.md`): sus secciones 3 y 4 (los
   campos `endianness` y `cpu_features`, la tabla de lo que el kernel lee) se
   REESCRIBEN. La regla 2 (lo que no me incumbe se salta) sigue valiendo para
   los anexos.
4. **Primero el hallazgo de solo lectura, dentro de BEF1.** Se puede arreglar
   hoy sin esperar a BEX2: tres estados de pagina (RX, R+NX, RW+NX) decididos
   por el TIPO de seccion. Es Ring 0 y pide metal.

## 6. La escalera

- [x] **B0 -- lo que no era de BMO-X, fuera de BEF1. HECHO el 2026-09-19**
  (`31602e7f`): `bmo-bex-gate` y `bef/validator.rs` rechazan imports, exports
  y TLS (`Falta::EnlazadoDinamico`); prueba
  `gate_y_validador_no_se_separan::los_dos_rechazan_el_enlazado_dinamico`.
- [x] **B1 -- PAGINAS DE SOLO LECTURA. HECHO el 2026-09-19** en
  `vmm/amarilla.rs` (`PermisoImagen`: Codigo RX, Constantes R+NX, Datos RW+NX,
  por el TIPO de seccion) y `task/admitir.rs`. El emulador de `bmo-lower`
  (`emu/paginas.rs`) protege codigo y rodata igual, y
  `cargador.rs::escribir_en_una_cadena_literal_es_un_fallo_de_pagina` dice que
  muerde; el banco de C (609) y el metro (30 programas) pasan con la
  proteccion puesta. **Falta el metal**: DOOM no corre en el emulador.
- [x] **B2 -- BEF2 en `bmo-abi`. HECHO el 2026-09-19** (`cd23a873`):
  `platform/abi/bmo-abi/src/bef2/` -- cabecera de 64 B con las cuatro
  regiones en sitio fijo, anexos, `xcr0`, un reloc, firma obligatoria; 15
  filas con una mutacion por cada una de las 20 faltas. Prologo 112 B contra
  los 384 de BEF1. Nadie lo usa todavia.
- [x] **B3 -- la PUERTA lee BEF2. HECHO el 2026-09-19** (`5ac020aa`) **y
  REDEFINIDO por B8 el 2026-09-20.** La primera B3 era un adaptador:
  `bmo-bex-gate/src/bef2.rs` leia BEF2 y se lo presentaba al kernel "como
  secciones" (tipo `CODE`/`RODATA`/`DATA`/`BSS`/`RELOCS`/`SIGNATURE` e indice
  de hash) para que `task/*` no cambiara ni una linea. Sirvio para un dia: con
  ella se pudo hacer B4-B6 sin tocar Ring 0. Pero era BEF1 al reves, y mintio
  una vez (los relocs, ver B6). **Hoy B3 es esto**: la puerta lee BEF2 y
  habla de lo que BEF2 tiene, `Cual::{Codigo, Constantes, Datos, Ceros}` y
  `Anexo {tipo, que}`, y el kernel mapea por `Cual`.
- [x] **B4 -- las herramientas en BEF2. HECHO el 2026-09-19**: `bmo-enlazar`
  (el ejecutable sale en BEF2; los `.bo` que entran siguen en BEF1),
  `bmo-pack` (`bef2::paquete`, misma API), `bmo-firmar` (firmar es REABRIR
  y reescribir: `Escritor::de_imagen` + `ed25519`, y se relee con
  `bmo-firma` antes de dar el fichero por bueno), `bmo-verify`, `bex-link`
  (el DIRECTOR), `hello-bex` y `rpc-demo`; el emulador (`emu/cargar.rs`) y el
  metro leen BEF2 por el juez. ** Y dos fallos que salieron al hacerlo:
  `Escritor::de_imagen` TIRABA los requisitos declarados (un `.ibx` que
  pedia pantalla la perdia al pasar por `bmo-pack` para llevarse el icono),
  y la pasada hostil cazo que una region VACIA con offset fuera del fichero
  pasaba el juez y luego `region()` panicaba -- ahora los dos jueces la
  rechazan con nombre, y un anexo vacio tambien.
- [x] **B5 -- los emisores. HECHO el 2026-09-19**: C, C++, COBOL, Ada e
  INTI escriben por `bef2::Escritor`; los relocs nombran regiones
  (`Region::de_seccion_de_emisor` es la unica traduccion); simbolos,
  manifiesto, katanas y recursos son anexos. Los 41 ejecutables del build
  son BEF2 y el metro dice las MISMAS instrucciones y salidas (857.700).
- [x] **B6 -- BEF1 MUERTO. HECHO el 2026-09-19.** Fuera `bef::{header,
  sections, relocations, writer, validator, objeto, paquete, signing}`,
  `bex.rs`, `bef-bootstrap`, la mitad BEF1 de `bmo-bex-gate` (que ahora
  rechaza `BEF1` por el magic como a un ELF), el camino BEF1 del emulador,
  del metro y de `hashes_del_disco`, y todas las filas que construian BEF1.
  Los OBJETOS `.bo` son BEF2 con la bandera `OBJETO`: simbolos en el anexo
  `SIMBOLOS` (nombran REGIONES) y un anexo `ENLACE` (`Rel32`, `Abs64`,
  `Region`; 24 B) que solo consume `bmo-enlazar` y que la puerta rechaza en
  un ejecutable. De rebote, un puntero al `bss` ya se puede nombrar
  (`BssNoSeSabeNombrar` se fue). Lo que queda de `bef/` son los CONTENIDOS
  de anexos (katanas, recursos, requisitos, simbolos, blake3).
  *** Y LO QUE SALIO AL HACERLO, y es de metal: **B3 mentia en los relocs**.
  El kernel descodificaba el registro de 24 B de BEF1 (secciones 0 code /
  1 data / 2 rodata) sobre el anexo de 16 B de BEF2 (regiones 0 codigo / 1
  constantes / 2 datos / 3 ceros): en el Ryzen, todo programa con un puntero
  en sus datos --DOOM, INTI con su monton-- habria muerto en "relocation
  fuera de su seccion". Ahora `bex::leer_reloc` y `gate::reloc` son BEF2 y
  `el_kernel_lee_los_relocs_de_un_bef2` lo ata con el codigo del kernel
  copiado. Dos filas mas que estaban VERDES midiendo nada: `codigo_de` de
  `bmo-enlazar` leia la tabla de BEF1 sobre un BEF2, y la fila del sector de
  INTI no encontraba ninguna seccion. **Y las regiones vuelven a empezar en
  un SECTOR** (512): BEF1 lo hacia desde el 10-08 para que el HBA escriba
  sectores enteros en el marco del proceso, la primera version de BEF2 lo
  pego a 16 y cada region empezaba con una cabeza rebotada. +24 KB en los 41
  ejecutables, y `ram.rs` mide ahora "empieza en pagina?" como unica
  pregunta (B9).
- [x] **B7 -- la CABECERA firmada. HECHO el 2026-09-20.** Una entrada de
  firma mas, la PRIMERA (`FIRMA_INDICE = 0x7F`: los 64 B + la tabla de
  anexos), obligatoria en los dos jueces; el kernel la comprueba con el
  prologo que ya tiene, antes de reservar un marco (`admitir.rs`, "LA FIRMA
  ES DEL INDICE"). Y de paso **la firma cubre CADA anexo**, no solo los tres
  que el kernel lee: un icono o un WAD dentro del paquete ya no se puede
  cambiar sin que `bmo-verify` lo vea, y la firma de autor responde por
  ellos. +40 B por imagen y +40 por anexo antes descubierto (3.200 B en los
  41). *** Y AL HACERLO SALIO UN FALLO DE METAL: `cadena_de_hashes` (lo que
  firma `bmo-firmar`) hasheaba las ENTRADAS de 40 B y el kernel
  (`Firmas::cadena`) los DIGESTS de 32 B -- desde B4 (19-09). El primer
  `.bex` firmado con la clave del ancla habria cuadrado en el anfitrion y
  salido `NoCuadra` en el Ryzen, y una firma que no cuadra NO arranca.
  Ahora es una sola cadena y la fila
  `la_cadena_que_se_firma_es_la_que_el_kernel_comprueba` la ata con el
  codigo del kernel copiado. Los ceros ya no cuentan como "sin hash" en la
  ficha (el 1 en rojo que salio en el Ryzen).
- [x] **B8 -- Ring 0 lee REGIONES. HECHO el 2026-09-20.** Se fue
  `bmo-bex-gate/src/bef2.rs` (309 lineas) y la puerta es UN fichero
  (`lib.rs`, 409 -> 605 con el contrato entero dentro): `Revisada` da
  `region(Cual)`, `regiones()`, `anexo(tipo)`, `anexos()`,
  `hasta_donde_hace_falta()`; no queda `kind`, `flags` ni `indice`.
  Kernel: `task/bex.rs` 428 -> 318 (`BexLoadPlan {regiones: [Region; 4],
  relocs, firma, requisitos: Tramo}`; el plan ya no tiene una lista de
  secciones que recorrer buscando tipos), `task/landing.rs` 253 -> 243
  (`Aterrizaje::abrir(que, ..)` y `Firmas::digest_de(que)` con el byte de
  la firma, sin "indice propio" que excluir porque la firma es un anexo y
  no se nombra a si misma), `task/admitir.rs` 987 -> 969 (`PermisoImagen`
  sale de `s.cual` -- **"el permiso lo da el hueco" vive ahora donde se
  mapea**; las VA se calculan por region en el orden de la cabecera; los
  tres anexos que el kernel lee tienen su tramo con nombre). La fila espejo
  `la_puerta_y_el_juez_ven_las_mismas_regiones` exige campo a campo que la
  puerta y `bef2::leer` digan lo mismo (tramos, `que`, anexos, `xcr0`,
  `lo_lee_el_kernel`), y `la_puerta_dice_cuanto_hay_que_traer_de_un_bef2`
  que 100 KB de recursos detras NO se traigan para ejecutar -- para eso el
  escritor pone los anexos que el kernel lee (relocs, requisitos, firma)
  ANTES que el resto. 25 filas en la puerta, 16 espejo; el kernel compila en
  `x86_64-unknown-none` sin un aviso en `task/`. **Pide metal**: es el
  mismo cargador con otro vocabulario, pero es el cargador.
- [ ] **B9 -- medir la decision 2** (paginas alineadas o compacto) con DOOM
  en el Ryzen antes de elegirla: `alinear_a_pagina()` es la palanca.
