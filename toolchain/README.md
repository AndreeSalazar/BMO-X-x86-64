# toolchain/ -- de codigo fuente a BEF2

El pipeline de BMO-X: lenguajes -> objeto (`.bo`) -> enlazado -> `.bex` (BEF2)
-> verificacion -> el kernel. Organizado en **tres carpetas con un rol claro**.

```
toolchain/
+-- lang/     <- FRONTENDS (la esencia, individual por lenguaje)
|   +-- c/        bmo-c-front       (+ emisor-x86_64/: bmo-c-x86-64)
|   +-- cpp/      bmo-cpp-front     (+ emisor-x86_64/: bmo-cpp-x86-64, con el codegen de C)
|   +-- cobol/    bmo-cobol-front   (todos los dialectos; NO nombra una maquina)
|   |   +-- emisor-x86_64/  bmo-cobol-x86-64  (el unico sitio de COBOL que emite)
|   +-- ada/      bmo-ada-front     (+ emisor-x86_64/: bmo-ada-x86-64)
|   +-- inti/     bmo-inti-front    (+ emisor-x86_64/: bmo-inti-x86-64)
|   +-- base/     stdlib base (bmo/core, pci, lib/printf) -- datos, no crate
|
+-- forge/    <- PIPELINE compartido (librerias OPCIONALES, nunca embudos)
|   +-- sem-asm/    bmo-sem-asm   codificacion: tablas TOML -> bytes; las tablas
|   |                             de la maquina (`tables/arch/x86_64/`) y REX
|   +-- bmo-lower/  bmo-lower     el emulador de x86-64 del banco (no del metal)
|   +-- bmo-verify/ bmo-verify    el juez del BEF2 en el anfitrion
|   +-- bmo-mods/   bmo-mods      donde estan las tablas (`$BMO_MODS`)
|
+-- tools/    <- HERRAMIENTAS y GUARDIANES del build
    +-- bmo-enlazar/    junta `.bo` de C, C++ e INTI en un `.bex`; estatico; poda
    +-- bmo-pack/       mete los recursos (icono, datos) DENTRO del `.bex`
    +-- bmo-firmar/     firma Ed25519 sobre la cadena de hashes de un `.bex`
    +-- bex-link/       el DIRECTOR (Rust) -> `.bex`
    +-- metro/          el metro de los emisores: instrucciones, accesos, bytes
    +-- hello-bex/, rpc-demo/   los payloads que el kernel embebe
    +-- fontgen/        genera font16_data.rs (tabla de glifos)
    +-- ... y los guardianes (capas, isa, contrato, privacidad, tamano, planes,
        codeowners, ambitos): cada uno una regla, y `bmo.ps1` los corre todos
```

## Reglas de organizacion

- **`lang/` = esencia.** Cada lenguaje es un pipeline COMPLETO y privado
  (parser, AST, su propio descenso). Nadie lo toca. La duplicacion de trabajo
  mecanico se compensa con las librerias de `forge/`, que el frontend
  **elige** enlazar. Lo UNICO agnostico es el frontend: cada lenguaje tiene su
  `emisor-x86_64/`, y no hay otra maquina en este repositorio.
- **`forge/` = contratos y librerias, NUNCA cerebros.** Nada es un embudo
  obligatorio. Ver `forge/README.md` y `lang/cobol/cobol.md` para la teoria.
- **`tools/` = herramientas y guardianes.** Se invocan con `cargo run -p
  <nombre>` (independiente de la ruta) o desde `bmo.ps1`.

## Flujo

```
fuente (C / C++ / INTI)  -> [frontend] -> emisor-x86_64 -> .bo   (OBJETO: BEF2 + SIMBOLOS + ENLACE)
                                                            |
                                        [bmo-enlazar] <-----+  junta las unidades, poda lo que
                                              |                nadie llama, escribe el .bex
fuente (COBOL / Ada)     -> [frontend] -> emisor-x86_64 ---+-> .bex  (EJECUTABLE: BEF2 con firma)
                                                            |
                                              [bmo-pack]    +-> recursos dentro   (opcional)
                                              [bmo-firmar]  +-> Ed25519           (opcional)
                                              [bmo-verify]  -> el juez del anfitrion
                                                            -> el kernel: la puerta (bmo-bex-gate)
```

**Los tres que enlazan (2026-09-20).** C, C++ e INTI salen como el MISMO `.bo`:
un BEF2 con la bandera `OBJETO`, sus funciones como simbolos, sus tablas como
regiones con enlace, y lo que llaman y no traen como simbolos indefinidos. La
convencion de llamada (`platform/abi/bmo-abi/src/types/convention.rs`) la
importan los tres emisores, asi que un `long long` de C, un `entero64` de INTI
y un `long long` de C++ viajan por el mismo registro. La prueba esta en
`tools/bmo-enlazar/src/pruebas.rs::un_programa_de_c_llama_a_inti_y_a_cpp`: un
`main` de C, una funcion de INTI y una clase de C++ con `extern "C"`, un solo
`.bex`, y sale `42 42 7`. Lo que NO cruza son los objetos de INTI (`texto`,
`lista`, `tabla`): llevan cabecera propia. COBOL y Ada no emiten simbolos
todavia (E6/E7 en `docs/plan/PLAN_EL_ENLAZADOR.md`).

```
   cargo run -p bmo-inti-x86-64 -- mates.inti --objeto       -> mates.bo
   cargo run -p bmo-c-x86-64    -- principal.c --objeto      -> principal.bo
   cargo run -p bmo-enlazar     -- principal.bo mates.bo -o app.bex
```

## El formato

BEF2 (`platform/abi/bmo-abi/src/bef2/`): cabecera de 64 B, cuatro regiones
en sitio fijo (codigo RX, constantes R, datos RW, ceros), anexos, un solo
reloc, y una firma que cubre el INDICE, cada region y cada anexo. La regla
congelada esta en `platform/abi/bmo-abi/src/bef/BEF_EXTENSIONES.md` y el plan
que lo trajo en `docs/plan/PLAN_BEF_NATIVO.md`. No hay ELF en este arbol.
