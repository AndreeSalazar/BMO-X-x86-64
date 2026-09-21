# METAS -- lo que BMO-X quiere ser, por categoria: lo abierto y lo cerrado CON MOTIVO

> Pedido por Eddi el **2026-09-20**: *"organizar las metas que faltan cumplir
> en categoria, en metas que estan abiertas y otras que estan cerradas con
> motivo"*.
>
> Este fichero es la vista por CATEGORIA. Los otros dos de la carpeta son:
> [`plan/ABIERTO.md`](plan/ABIERTO.md), que se GENERA y cuenta las casillas
> (el build lo comprueba), y [`plan/EL_ORDEN.md`](plan/EL_ORDEN.md), que dice
> el CRITERIO de que va primero (10-09; su lista de 22 planes es de esa fecha,
> su criterio sigue en pie). Este no cuenta casillas: dice **que meta es, en
> que estado esta, y por que**.
>
> ## La regla de esta pagina
>
> ```text
>    ABIERTA     hay trabajo de codigo o de metal por delante, y se dice cual
>    CERRADA     y el motivo es UNO de estos cuatro, siempre escrito:
>                  HECHA      esta en el arbol y (si toca) en el Ryzen, con fecha
>                  SUPERADA   otra decision la dejo sin sentido: se cita cual
>                  APARCADA   no se hace AHORA y se dice que la retoma
>                  ESPERA     una decision del dueno, y se dice cual
> ```
>
> Un plan cerrado NO se mueve de `plan/`: sigue siendo la razon por la que algo
> se hizo asi, y eso se consulta mas que la casilla. Lo que se le pone es una
> linea `> Estado: **PALABRA** -- motivo` en su cabecera, y
> `toolchain/tools/planes` la lee: sus casillas sueltas dejan de contar como
> deuda. Un estado sin motivo pone el build en rojo.

---

## 0. La meta de todas: en una frase

**Un orquestador de metal que ejecuta programas de sus propios compiladores,
en su propio formato, y que dice siempre quien, cuanto y por que.** Hoy (20-09)
arranca en un Ryzen 5 5600X real, corre COBOL, C, C++, Ada e INTI en BEF2, juega
DOOM, y cada capa que gasta o que niega lo confiesa (`save`, CABINA, los 23
guardianes del build). Lo que sigue son las metas por categoria.

---

## 1. EL FORMATO Y LA CARGA -- que es un programa, y como entra

| meta | estado | motivo / lo que falta |
|---|---|---|
| Un formato PROPIO, sin ELF (BEF2) | **HECHA** 19/20-09 | `plan/PLAN_BEF_NATIVO.md` B0-B8: cabecera de 64 B, cuatro regiones en sitio fijo, anexos, un reloc. BEF1 muerto (`4bc3ef75`). **Arranco en el Ryzen el 20-09** (d.bex y DOOM) |
| La firma es del INDICE y de todo (B7) | **HECHA** 20-09 | `85e6e6aa`: cabecera + tabla + cada region + cada anexo; el kernel comprueba el indice antes de reservar. Falta VERLA en el Ryzen (hoja del metal 18-09, 3c) |
| Ring 0 mapea por REGION, no por bandera (B8) | **HECHA** 20-09 | `6a7c4bdf`: el permiso lo da el hueco. Codigo RX, constantes R, datos RW |
| Reflejar en vez de copiar (B9, paginas) | ABIERTA | no es un flag: el cargador COPIA (el HBA escribe en el marco). Reflejar pide trabajo del kernel (`identidad/LA_RAM.md` parte IX) y se mide con DOOM despues |
| La llave DECIDE lo que se concede (niveles) | **ESPERA** | `identidad/EL_CONTRATO_DE_CARGA.md` 2b.3/2c contra `task/autoridad.rs` ("no debe ser una jerarquia"): es una decision de Eddi. Hoy un firmado y un extranjero reciben lo mismo |
| Un `.bex` firmado que arranque, y `exige_firma() = true` | ABIERTA | `plan/PLAN_SEGURIDAD.md` S-FIRMA-4/5: firmar uno con `bmo-firmar`, verlo arrancar, y solo despues exigirlo |
| El paquete: recursos DENTRO y leidos en ejecucion | ABIERTA | `bmo-pack` los mete y la firma los cubre; leerlos desde la app por el ABI es lo que falta (`PLAN_REX.md`) |
| El programa DECLARA y el kernel COMPRUEBA (requisitos) | **HECHA** | `bmo-carga-juicio`: el `.bex` trae lo que necesita con su motivo y el kernel niega antes del primer marco |

## 2. LOS LENGUAJES Y EL TOOLCHAIN -- con que se escribe

| meta | estado | motivo / lo que falta |
|---|---|---|
| Cinco compiladores propios, sin LLVM ni GCC | **HECHA** | C, C++, COBOL, Ada, INTI: frontend agnostico + `emisor-x86_64/` cada uno |
| La lista de lenguajes | **CERRADA** 17/18-09 | decision de Eddi. Python: solo interprete (AOT quitado, es INTI). Java: analizado el 20-09 y NO entra por el lenguaje sino por la GPU y el GC; solo cabria "LLANO" para banca, y es decision suya |
| Compilacion separada: `.bo` + `bmo-enlazar` | **HECHA** para C, C++ e INTI | `plan/PLAN_EL_ENLAZADOR.md` E2, E5e, E8 (20-09). `42 42 7`: un `main` de C, INTI y C++ en un `.bex` |
| COBOL y Ada emiten objetos (E6, E7) | ABIERTA | codegen propio, 0 relocs hoy; es trabajo, no bloquea la banca |
| INTI declara funciones AJENAS (`externo`) | **ESPERA** | INTI es hoy biblioteca de C, no al reves: falta una palabra en su gramatica, y la gramatica es de Eddi |
| El enlazador junta requisitos y katanas de un `.bo` (E9) | ABIERTA | un modulo de INTI enlazado desde C no declara su pantalla en el `.bex` final |
| El emisor de C: menos instrucciones por lo mismo | **HECHA** 18/19-09 | `plan/PLAN_EL_TROQUEL.md` C1-C5: -59 % en el metro, convencion hibrida. **Verificado en metal el 20-09 (DOOM)**. `PLAN_EL_CODEGEN.md` queda SUPERADO por este |
| INTI en registros (I1, I2) | **HECHA** 20-09 | `5f9fba2c`: locales en registro, accesos -34 % en el metro. Quedan I3 (residencia) e I4 (reenvio), abiertas y ordenadas en el troquel |
| INTI: el techo de `crudo`, las katanas del silicio, negar en vez de callar | ABIERTA | `plan/PLAN_EL_SILICIO.md` S6-S8 |
| NUNCA ADIVINA: el compilador no supone lo que no sabe | ABIERTA | `plan/PLAN_NUNCA_ADIVINA.md` A4-A5: la tabla del UB de C y las suposiciones de disposicion |
| El codegen de C, cortado en piezas de una pregunta | ABIERTA | `plan/PLAN_CODEGEN.md`: `emit_program`, `emit_stmt`, el preprocesador (1.204 lineas) |
| Ada A2-A7 (ZFP, ACATS como matriz) | ABIERTA | `toolchain/lang/ada/PLAN_ADA.md`; A1 hecha el 17-09 |
| La banca en COBOL | ABIERTA | `toolchain/lang/cobol/PLAN_BANCA.md`; el objetivo de la hoja de ruta desde el principio |
| C++: plantillas (paso 6) | ABIERTA | `toolchain/lang/cpp/BRECHA.md`; pasos 0-5 hechos (clases, RAII, sobrecarga, herencia, vtables). Sin excepciones ni RTTI a proposito |
| Autohospedaje: compilar SOBRE BMO-X | **APARCADA** | `plan/PLAN_AUTOHOSPEDAJE.md`: no bloquea nada; pide Ada `no_std` y `PLAN_ESTRUCTURA` (F1) antes |
| El taller en F1 (`estructura.bex`) | ABIERTA | `plan/PLAN_ESTRUCTURA.md`: la ventana vacia es la primera casilla |
| El asistente de IA dentro de BMO-X | **APARCADA** | decision de Eddi (10-09): es el ultimo. `plan/PLAN_EL_ASISTENTE.md` |

## 3. EL METAL Y EL KERNEL -- lo que corre en Ring 0

| meta | estado | motivo / lo que falta |
|---|---|---|
| Una arquitectura, un repositorio: x86-64 | **CERRADA** 18-09 | decision de Eddi; guardian `isa`. ARM/RISC-V = OTRO repo |
| BMO-X como aparato guardian (placa RISC-V) | **SUPERADA** | por la decision de arriba. `plan/PLAN_EL_GUARDIAN.md` se conserva para ese otro arbol |
| Ring 0 cerrado a externos | **CERRADA** 17-09 | para siempre, `.github/CODEOWNERS`, guardian `codeowners`. Motivo: xz |
| El USB en su propio nucleo (A0-A2) | ABIERTA | `plan/PLAN_EL_BUS_APARTE.md`: A0 y A1 hechos (el triple fallo era el selector de 16 bits + sin TSS, confirmado en el Ryzen); A2 residente en `bmo-orquesta` |
| El COMPAS: el quantum se retira, el turno se concede | ABIERTA | `plan/PLAN_EL_COMPAS.md` E0-E11, 0 hechas. Es el fantasma del planificador (`bmo-planificador-suelo`) |
| El PLAZO: V-Sync, VBlank, un numero contra el que decidir | ABIERTA | `plan/PLAN_EL_PLAZO.md` P2.2 (reschedule forzado) es la que el `WAIT` del compositor espera |
| La puerta se parte: dividir lo que no se puede abaratar | ABIERTA | `plan/PLAN_LA_PUERTA_SE_PARTE.md` M0b-2, M0c, M1b; 8 hechas |
| El NEUTRO vigilado: el DMA con plazo medido (LEY 24) | ABIERTA | `plan/PLAN_EL_NEUTRO_VIGILADO.md` N5b: el numero sale de varios arranques con `mudo=`, no se elige |
| La RAM sale del kernel (bmo-marcos) | ABIERTA | `plan/PLAN_LA_RAM_SALE_DEL_KERNEL.md`: un arranque verde primero |
| La vida util de la memoria: liberar EN CALIENTE sin recolector | ABIERTA | `plan/PLAN_LA_VIDA_UTIL.md`: 0-2 y 4 HECHOS el 21-09 (`request` prestada con `Drop` / `residente` dicha; el visor devuelve 8 MiB al cerrar; guardian `esperable`; `KIND_ARCHIVO` derogado). 7 HECHO tambien (WAIT sobre un bloque PRESTADO: `soltar -> par -> WAIT -> soltar`, `Memoria::soltar_esperando`). Falta 5 (censo), 6 (entregar en cero: pide el numero de soltar en el Ryzen), 8 (`invlpg` del otro nucleo) |
| La pila de kernel de cada tarea CABE lo que corre encima | **HECHA** 21-09 | 4 -> 8 paginas y guardian `pila` (mide el ELF en cada build). El PD del escritorio a cero era esto, no el asignador |
| La pila huerfana (matar Ring 3 y volver) | ABIERTA | `plan/PLAN_LA_PILA_HUERFANA.md`: reproducir, el juez en su crate, `reap` pregunta |
| Autocuracion: de informar a ACTUAR | ABIERTA | `plan/PLAN_AUTOCURACION.md`, 17 casillas, 0 hechas: contar lo que queda del muerto tras revocar |
| La deuda medida: `static mut`, el codegen de COBOL | ABIERTA | `plan/PLAN_LA_DEUDA.md` D2a (trinquete de `static mut`), D3a |
| Los vatios: gastar poco y ACUSAR quien | ABIERTA | `plan/PLAN_VATIOS.md`: W0-W4 piden el vatimetro en el Ryzen; MWAIT solo llega a C1 |
| El repartir la pila de disco | **HECHA** | `plan/PLAN_ALMACENAMIENTO.md`, cumplido |
| El perfil total de la maquina (8 escalones) | **HECHA** | `plan/PLAN_EL_PERFIL_TOTAL.md`, cumplido; `PERFIL/` con guardianes |
| El suelo de Ring 3 (las tres piezas) | **HECHA** | `plan/PLAN_SUELO_RING3.md`, cumplido |
| El semaforo completo (cada fichero dice su color) | ABIERTA | `plan/PLAN_EL_SEMAFORO_COMPLETO.md` S4-S6: 38 drivers, `platform/shared`, `platform/abi` |
| La GPU (Vulkan, RDNA4) | **APARCADA** | `platform/drivers/gpu/rdna4/PLAN_VULKAN.md`: con plan escrito, no descartada. Es lo que Minecraft y "lo tipico" piden de verdad |

## 4. EL ESCRITORIO Y LAS APPS -- lo que el dueno ve

| meta | estado | motivo / lo que falta |
|---|---|---|
| La ventana sale en metal (DIRECTOR, superficies, buzon) | **HECHA** 12-09 | DOOM 58 fps en ventana, cubo, la LETRA por el buzon |
| DOOM se juega | **HECHA** 20-09 | `plan/PLAN_DOOM.md` CERRADO. Quedan NUMEROS (hoja 18-09 3b: `[perf]`, `blit`, `ciclos`) |
| `save` maestro: la maquina se redacta entera | **HECHA** 20-09 | `feb1ef9d`: 7 capitulos + la ficha BEF2 de cada programa. Visto en el Ryzen el mismo dia |
| El DIRECTOR: de compositor a administrador | ABIERTA | `plan/PLAN_DIRECTOR.md`: el hueco por buzon, reemplazar superficie, DOOM elige escala |
| El DIRECTOR por dentro: `system.rs`, `disco.rs`, `consola.rs` cortados | ABIERTA | `plan/PLAN_DIRECTOR_CENSO.md`, 4 sueltas |
| El PIXEL: `bInterval` del raton hasta Ring 3, `BUS_PERIOD_MS` medido | ABIERTA | `plan/PLAN_EL_PIXEL.md` Y1.1, Y1.2 |
| La MAQUETA: el escritorio como texto, ficheros dorados | ABIERTA (1) | `plan/PLAN_MAQUETA.md`: el oraculo dorado de `calc` |
| La cara VIAJA (la maquetacion como dato en el `.bex`) | ABIERTA | `plan/PLAN_LA_CARA_VIAJA.md` 3-5: el lector, desde fichero, en el anexo |
| NAVEGAR: la app que navega sin ser navegador | ABIERTA | `plan/PLAN_NAVEGAR.md` N4 (lamina viva), N5 (historial), AA0 (la app Android en el repo) |
| Los DOCUMENTOS: el escritorio lista lo que abres (`.datex`) | **ESPERA** | idea de Eddi sin decidir; `CLASE_PANTALLA` ya existe en los requisitos |
| REX: `<bmo/latido.h>`, `<bmo/corriente.h>` | ABIERTA (2) | `plan/PLAN_REX.md` 5b, 5c; 15 hechas |
| El explorador de ESTRATOS (5 pasos) | ABIERTA | `bmo-explorador-plan`: pintar navegaba |
| Ver imagenes: BMP, QOI, BICO, **PNG, JPEG** | **HECHA** 20-09 | `bmo-imagen`: inflate propio, IDCT entera, 40 filas contra Pillow/libjpeg; el visor abre `.png` y `.jpg`. Falta VERLO en el Ryzen (`datos/inti.png`, `datos/arranque.jpg`) |
| MP3 | ABIERTA | `plan/PLAN_MEDIOS.md` 7.2: el decodificador es codigo (~2.500 lineas Rust propias); OIRLO pide A1 (`TUBO ABIERTO`), que el Ryzen nego el 25-08 y no se ha vuelto a arrancar |
| MP4 | **SUPERADA** por MPEG-1 | `PLAN_MEDIOS.md` 7.3: H.264+AAC sin FFmpeg ni GPU es un proyecto por codec; lo que cabe es `pl_mpeg` en ventana y convertir fuera (o la antena). VLC sigue siendo "no" con motivo |
| Audio en metal (A1 SET_INTERFACE) | ABIERTA (2) | `plan/PLAN_AUDIO.md`: el Ryzen lo nego el 25-08, corregido el 26-08, sin ejecutar desde entonces |

## 5. LA RED Y LA ANTENA -- lo que entra de fuera

| meta | estado | motivo / lo que falta |
|---|---|---|
| La RTL8168 recibe en metal; la red es familia propia | **HECHA** 13-09 | `ring0/red`, 5 -> 14 tramas, malas 0 |
| Transmitir con el DMA contado (RED TX E0-E3) | **HECHA** en codigo | `plan/PLAN_RED_TX.md`; falta la foto del ARP contestado |
| El muro IOMMU, el ping que contesta, firmas sobre UDP (E4-E6) | ABIERTA | `plan/PLAN_RED_TX.md`; `bmo-pila` espera encima |
| G5: TCP propio contra la antena (`red hola`) | ABIERTA (metal) | en codigo desde el 18-09; espera el Ryzen |
| CLOUD LOCAL: el movil es la antena, BMO-X la pantalla | ABIERTA | `plan/PLAN_CLOUD_LOCAL.md`, 45 sueltas, el plan mas largo. La antena Android esta TERMINADA (HONOR navega solo); lo que falta es BMO-X: S1 (pl_mpeg), N3, P0 |
| Windows como CONSOLA (pixeles MPEG-1 + entrada de vuelta) | ABIERTA | `PLAN_CLOUD_LOCAL.md` seccion 13, S6 ESPEJO: es como Minecraft llega a esta pantalla |
| Privacidad: ni MAC ni IP en el repo publico | **HECHA** 13/18-09 | guardian en el build + `--msg` en el hook de commit |

## 6. LA SEGURIDAD -- decir que no con nombre

| meta | estado | motivo / lo que falta |
|---|---|---|
| Cuatro pasadas de superficie de ataque, seis reales cerrados | **HECHA** 17-09 | `bmo-hostile` en `bmo.ps1`; falta el metal (C8e) |
| El metal de la seguridad (C8e, S-FIRMA-4) | ABIERTA | `plan/PLAN_SEGURIDAD.md`: 6 sueltas, 26 hechas |
| Quien firma: `bmo-firmar`, el ancla, la privada fuera del repo | **HECHA** 10-09 | `task/confianza.rs` |
| La cadena que se firma es la que el kernel comprueba | **HECHA** 20-09 | cazado antes del metal: era mi fallo de B4 |
| Los NIVELES por llave (extranjero / socio / dueno) | **ESPERA** | ver categoria 1: decision de Eddi |
| SHA-256 (NIST), Ed25519, BLAKE3 | **HECHA** | `bmo-cripto`; abre HTTPS y la firma con la misma llave |

## 7. LA COMUNIDAD Y EL TRABAJO -- lo que no es codigo

| meta | estado | motivo / lo que falta |
|---|---|---|
| Apache-2.0, repo publico | **HECHA** 05-09 | Techne/Simbiosis ya no existe; se conserva el razonamiento |
| El lanzamiento: r/osdev antes que Show HN | ABIERTA | `bmo-lanzamiento-comunidad`; el material privado vive en PREPARACION |
| El video del Ryzen (bloquea la busqueda de trabajo) | ABIERTA | `eddi-busqueda-de-trabajo`; DOOM jugado el 20-09 es la escena |
| README al dia | **HECHA** 20-09 | cinco compiladores, BEF2, los tres que enlazan, tamanos de hoy |

---

## 8. Lo que el Ryzen tiene que contestar antes de cerrar mas

Todo lo de la columna "HECHA" que dice *falta el metal* cabe en UNA tanda:
[`metal/METAL_2026-09-18.md`](metal/METAL_2026-09-18.md), secciones 3b (los
numeros del emisor) y 3c (BEF2: `save` con 7 capitulos, `cuadran` 4 en d.bex y
5 en DOOM, `s/hash` 0, la firma del indice). Cuando esa hoja tenga sus fotos,
esta pagina cambia tres filas de "falta el metal" a "vista".

## 9. Como se mantiene esta pagina

- Cuando un plan se cierra, se le pone la linea `> Estado:` **con motivo** y
  se regenera `plan/ABIERTO.md` (`python toolchain/tools/planes/planes.py
  --apply`). El build lo exige.
- Esta pagina se toca a mano cuando una META cambia de estado, no cuando una
  casilla se marca: las casillas las cuenta la herramienta, las metas las
  escribe una persona. Si las dos discrepan, manda el plan.
