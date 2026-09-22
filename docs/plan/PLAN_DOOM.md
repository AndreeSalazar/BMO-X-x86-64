# El plan largo: de "BMO C compila 69 de 81" a "DOOM se juega en el Ryzen"

> Estado: **CERRADO** -- hecho el 2026-09-20: DOOM se juega en el Ryzen sobre BEF2 y el emisor de C de septiembre. Lo que queda son numeros de la hoja del metal (`docs/metal/METAL_2026-09-18.md`, 3b), no casillas de DOOM.

> Escrito el **2026-08-08**, el dia que la sonda paso de 0 a 69 ficheros sueltos
> y el unity build empezo a parsear las 56.465 lineas enteras.
>
> ★★★★ **CERRADO EL 2026-09-20: DOOM SE JUEGA EN EL RYZEN.** Ese dia se jugo
> hasta que el personaje murio, sobre el formato BEF2 recien estrenado
> (`PLAN_BEF_NATIVO.md`, 1.285 relocs de 16 B, hashes al aterrizar) y sobre el
> emisor de C de septiembre (`PLAN_EL_TROQUEL.md`: -59 % de instrucciones,
> convencion de llamada hibrida, residencia en registros, reenvio `jmp`), que
> ningun CPU habia ejecutado antes. 742 KB de `.bex` (eran 912 en agosto), 58
> fps en ventana. Lo que queda de DOOM ya no es de DOOM: es del escalado por
> CPU (`bmo-doom-rendimiento`) y de la GPU, que esta aparcada con plan. Todo
> lo de abajo es la historia de como se llego, y se deja entera.
>
> ★★★ **AL DIA EL 2026-08-13. Si vienes a saber por que DOOM no se juega, salta
> directo a [DONDE MUERE DOOM HOY](#-donde-muere-doom-hoy----2026-08-13-y-ya-no-es-una-teoria)**,
> al final. Lo de aqui arriba es el plan y su historia; la respuesta esta abajo
> y son tres lineas de `codegen/mod.rs`.
>
> ★★ **ACTUALIZADO EL 2026-08-09: LA FASE 1 ESTA HECHA.** El unity build ya no
> se para: con un backend de plataforma vacio, las 56.465 lineas salen en un
> `.bex` de **1.299.512 bytes**. Lo que queda para verlo correr es la FASE 2 --
> seis funciones-- y ya no hay nada desconocido delante.
>
> `docs/identidad/QUE_DESBLOQUEA.md` dice **que falta y por que**. Este dice **en que
> orden, que bloquea a que, y como se sabe que una casilla esta hecha.** Es el
> mismo trato que `toolchain/lang/cobol/PLAN_BANCA.md` tiene con la banca.
>
> Esta hecho para avanzar **poco a poco**: cada casilla se puede entregar sola,
> con su prueba, sin dejar el compilador roto entre medias.

## Como se lee esto

```
[ ]  pendiente        [~]  a medias, y se dice cuanto        [x]  hecho, con fecha
★    la pieza que decide su fase
⛔   BLOQUEADO, y por QUE -- comprobado en el codigo, no supuesto
⚠    tiene una decision dentro que hay que tomar antes de escribir codigo
```

Medida: **S** una sesion - **M** dos o tres - **L** una semana de verdad -
**XL** la pieza grande de su fase.

## La regla que no se negocia

**Nada entra sin su fila en el banco de pruebas, y la fila EJECUTA el programa.**
Hoy son 311 en `bmo-c-front`. Las dos cosas que este proyecto encontro el 08-08
--`p->x++` que no hacia nada y `*p` que leia ocho bytes-- **no dan error**: dan
un numero. Solo se ven ejecutando.

---

# FASE 0 -- Donde estamos, medido

No es una fase de trabajo: es la linea de salida, para que las de abajo se lean
contra algo.

```
   ficheros sueltos:  0 -> 7 -> 27 -> 35 -> 41 -> 47 -> 55 -> 61 -> 67 -> 69
   unity build:       parsea (08-08) -> ** COMPILA A .bex ** (08-09)
                      1.299.512 bytes, con backend de plataforma vacio
```

| | |
|---|---|
| Lenguaje | 32/32 sondas, y siete tandas de arreglos el 08-08 |
| Relocations | las **tres** caras: cadena, funcion y global |
| Ficheros | `fopen`/`fread`/`fseek`/`fclose`, `ARCH_OP_LEER_EN` |
| Memoria | `KIND_MEMORIA`, y DOOM pide **un solo bloque** de 6 MiB |
| Pantalla | framebuffer con doble bufer, ya probado con `ray.bex` |
| Entrada | teclado USB con su ESC (que hasta el 08-08 **no existia**) |
| Tiempo | `INFO_TICKS` y el TSC medido |

★ Y los dos hallazgos que cambian la estimacion, comprobados en el codigo de
DOOM y no supuestos:

- **El renderer no necesita coma flotante.** El unico `atan()` esta dentro de un
  `#if 0` que dice *"UNUSED - now getting from tables.c"*.
- **El tope de 4 `malloc` por proceso no lo bloquea.** `I_ZoneBase` pide UN
  bloque y `Z_Malloc` reparte desde dentro.

---

# FASE 1 -- Que el unity build llegue al final

Es la fase que decide todo lo demas: mientras no salga un `.bex`, las fases de
abajo no se pueden ni empezar a probar.

| # | Casilla | Tam | Estado |
|---|---|---|---|
| 1.0 | ★ **`printf` con formato en tiempo de ejecucion** | XL | **[x] 2026-08-09** -- camino A |
| 1.1 | `sprintf` / `snprintf` sobre el mismo formateador | M | **[x] 2026-08-09** |
| 1.2 | `fprintf` -- 64 llamadas en DOOM | S | **[x] 2026-08-09** (van a consola) |
| 1.3 | Las ~20 triviales | M | **[x] 2026-08-09** |
| 1.4 | `system` `mkdir` `getenv` `remove` `rename` -- apuntaladas con su motivo | S | **[x] 2026-08-09** |
| 1.5 | Que el `.bex` quepa en `MAX_BEX` | S | **[x] 2026-08-09** -- ver abajo |

**Y cuatro cosas que no estaban en la lista y hubo que hacer**, porque no se
sabian hasta intentarlo:

| | Que era | Por que no estaba previsto |
|---|---|---|
| 1.6 | ★ **`__va_list()`** -- el `va_list` pasa a ser un PUNTERO | `__va_arg(i)` es un INDICE, y un indice no sobrevive a pasarlo a otra funcion: describe una posicion en el marco de quien pregunta. Sin esto no hay familia `v*`, y `M_vsnprintf` es exactamente eso |
| 1.7 | ★ **El `#elif` compilaba las dos ramas** | Ep. 36 de la bitacora. `i_swap.h` definia `SYS_LITTLE_ENDIAN` **y** `SYS_BIG_ENDIAN` |
| 1.8 | ★ **`double` como PARAMETRO** | Lo pedia `fabs`. El motivo escrito era *"falta la ABI de xmm"* y resulto que no hace falta ninguna: aqui los argumentos van por la pila |
| 1.9 | `sscanf` -- 8 llamadas, con `%i` de base automatica | Estaba contada como trivial y no lo es: `M_StrToInt` distingue `0x`, `0` y decimal con el formato |

## 1.5 -- El tope, y quien manda sobre el

`MAX_BEX` estaba en **1 MiB** y la imagen mide **1.299.512 bytes**: no cabia
por 248.936. Se subio a **4 MiB**, y la regla que se aplico queda escrita aqui
porque va a volver a hacer falta:

> **El programa ajeno manda sobre el tope, no al reves.** DOOM es de 1993 y es
> el codigo mas apretado que se va a portar aqui en mucho tiempo. Si no cabe, el
> que esta mal medido es el bufer.

Lo que cuesta: `.bss` del kernel, dentro del hueco de **16 MiB** que el cargador
UEFI ya reserva y pone a cero en `0x400000`. **El `.bin` no crece** --se midio:
909.696 B antes y despues-- porque `.bss` no viaja en la imagen.

## ★ 1.0 -- El formateador en ejecucion, que es la pieza de verdad

Hoy `printf` se emite **en linea desde un literal**: el compilador lee el
formato al compilar y escribe las llamadas. `I_Error(fmt, ...)` y `M_snprintf`
reciben el formato **como argumento**, y ahi no hay literal que leer.

Lo que hace falta es un `__bmo_vprintf(fmt, args)` que recorra la cadena al
vuelo. Las piezas ya estan casi todas:

- los formateadores sintetizados (`__bmo_fmt_i64`, `_u64_hex`, `_cstr`...),
- `__va_arg(i)`, que da el variadico `i` porque BMO pasa los variadicos por la
  pila detras de los nombrados,
- y `console::write_const` para lo literal.

⚠ **La decision que hay que tomar antes de escribir codigo**: donde vive.

| Camino | A favor | En contra |
|---|---|---|
| **A** -- en C, dentro del unity | se escribe una vez, se lee, se prueba con el resto | hay que interceptar `printf` en el codegen para que NO lo emita en linea |
| **B** -- como funcion sintetizada | no toca el codegen de llamadas | el mecanismo de sintetizadas recibe `&mut Vec<u8>` y **no sabe de etiquetas**: un bucle con saltos no cabe hoy |

Hoy A es mas barato, y B pide primero ensanchar la tabla de sintetizadas para
que acepte emisores con etiquetas -- que es un cambio de la TABLA, no de las
funciones. Ver la cabecera de `codegen/sintetizadas.rs`.

**Como se sabe que 1.0 esta hecha**: `printf(fmt, 42)` con `fmt` en una variable
imprime `42`, y la fila lo EJECUTA.

### [x] HECHO el 2026-08-09 -- se tomo el CAMINO A, y con un matiz

El formateador vive en **`toolchain/forge/sem-asm/tables/standards/C/stdio.h`**, escrito en
C, y el codegen desvia ahi el `printf` cuyo formato no es literal. De la tabla
de sintetizadas solo hizo falta **una** entrada nueva, `bmo_escribir`, que saca
a la consola un bufer que no existia al compilar -- su cuerpo es
`console::write_buffer`, que ya estaba escrito y solo alcanzaba el codegen.

★ **El detalle que hizo el desvio corto**: en BMO los argumentos se empujan por
la pila, asi que **empujarlos en orden inverso deja en memoria un `va_list`
tal cual**, y lo que se le pasa al formateador es `rsp`. No hay area de
argumentos que construir.

★ **Y una cosa que el camino B no habria dado**: este formateador **aplica la
anchura**. El de linea lee el `7` de `%7i` y lo tira --lo dice en su propio
comentario-- porque sus conversores escriben directo a la consola y para
rellenar hay que saber cuanto ocupa el numero ANTES. Aqui se arma en un array
primero, asi que una tabla alineada sale alineada.

Filas que lo ejecutan, en `tests/libc.rs`: el formato en una variable, la
anchura y las banderas, el `va_list` que viaja a otra funcion, el truncado de
`snprintf` sin desbordar, el hexadecimal con el bit alto puesto, y una
conversion desconocida que **no se come el argumento**.

---

# FASE 1.5 -- Lo que la lista NO tenia, y bloqueaba de verdad

Escrito el **2026-08-09**, tarde. Ninguna de las tres estaba en el plan y las
tres impiden que DOOM arranque. Salieron de contar en vez de suponer.

| # | Que era | Como se supo |
|---|---|---|
| 1.10 | ★ **El tope de 4 `malloc`** | El plan decia que no bloqueaba porque `I_ZoneBase` pide UN bloque. Contando los sitios: el arranque llama a `malloc` **una docena de veces** -- solo `I_AtExit` son siete. **[x]** `<bmo/monton.h>`, un asignador de Ring 3 |
| 1.11 | ★★ **El teclado no tenia SOLTAR** | `INPUT_OP_TECLA` entrega un CARACTER, y un caracter no tiene "solto". Quien echa a andar no para nunca; y Shift/Ctrl/Alt no producen caracter, asi que ni salian. **[x]** `INPUT_OP_EVENTO_TECLA` |
| 1.12 | **`fseek` ignoraba el origen** | `M_FileLength` mide el WAD con `SEEK_END`. Con el origen ignorado, **el WAD media cero bytes** sin una sola linea de error. **[x]** -- y de paso salio que `feof` daba EOF pasada la mitad de cualquier fichero |

★ La leccion, que vale mas que las tres: **el plan daba por bloqueado lo que
era visible (el lenguaje) y por resuelto lo que no lo era (la superficie del
sistema).** Las tres se encontraron mirando el codigo de DOOM y el del kernel a
la vez, no compilando.

Y una que no bloquea pero se llevaba media imagen: **el 90,3% de la seccion
`data` de DOOM eran ceros** que viajaban en el fichero. Ver `docs/identidad/LA_RAM.md`.

---

# FASE 2 -- La capa de plataforma: seis funciones

DOOM (doomgeneric) habla con el sistema por **seis funciones**, y las seis ya
tienen con que hacerse. Es `doomgeneric_bmo.c`, y es el fichero que hay que
escribir de cero.

## [x] ESCRITA ENTERA el 2026-08-09 -- `doomgeneric_bmo.c`

| # | Casilla | Con que se hizo |
|---|---|---|
| 2.0 | `DG_Init` | `PANTALLA_RECLAMAR` + `ENTRADA_RECLAMAR`, `FB_BASE`/`DIMS`/`STRIDE`, centrado |
| 2.1 | `DG_DrawFrame` | `memcpy` fila a fila -- **por el stride**, ver abajo |
| 2.2 | `DG_GetTicksMs` | **TSC**, no `INFO_TICKS` |
| 2.3 | `DG_SleepMs` | girar sobre el TSC **cediendo** |
| 2.4 | `DG_GetKey` | `INPUT_OP_EVENTO_TECLA` + tabla de scancode a `doomkeys.h` |
| 2.5 | `DG_SetWindowTitle` | a consola: DOOM tiene la pantalla entera |

**El `.bex`: 812.736 bytes.** Vive en `BMO-externo/doom/doomgeneric/`, fuera del
repo, porque es GPL.

⚠ **La decision de 2.1 se resolvio sola, y para bien**: DOOM **ya entrega 32
bits**. `I_FinishUpdate` llama a `cmap_to_fb` y deja `DG_ScreenBuffer` con
640x400 pixeles listos -- la expansion por paleta la hace DOOM, no nosotros.
Aqui solo queda el blit. Lo que si hubo que hacer es copiar **fila a fila**: el
framebuffer tiene stride, y un solo `memcpy` de corrido funciona en el panel
donde stride == ancho y sale torcido en el primero donde no.

★ **2.2 NO usa `INFO_TICKS`**, y el motivo importa: el tick del LAPIC se calibra
en el arranque y **su frecuencia no esta declarada en ninguna constante que un
programa pueda leer**. La del TSC si (`INFO_TSC_HZ`, medida por el kernel), asi
que el reloj sale de `__rdtsc()` dividido por ciclos-por-milisegundo. Un reloj
que no se puede convertir a milisegundos no es un reloj.

★ **2.3 tiene una trampa que se paga cara**: girar sobre el reloj sin ceder no
solo quema el quantum -- deja al resto del sistema sin turno, **incluido el bus
USB, que se sondea desde dentro de un syscall**. O sea que un bucle de espera
mal escrito aqui apaga el teclado de este mismo programa.

**Como se sabe que la fase esta hecha**: sale el menu de DOOM en el Ryzen, con
foto. **Todavia no ha corrido.**

---

# FASE 3 -- El WAD, que es donde vive el juego

| # | Casilla | Tam | Nota |
|---|---|---|---|
| 3.0 | Leer `doom1.wad` (4.196.020 B) con la cadena de ficheros | S | **[x] escrito** -- `-iwad apps/doom1.wad` por `myargv` |
| 3.1 | ⚠ Que quepa: el WAD son 4 MiB | M | **no hace falta**: DOOM NO lo carga entero, ver abajo |
| 3.2 | `W_CacheLumpName` sobre el zone allocator | S | es codigo de DOOM, no de BMO |

★ **3.1 se cayo sola al mirarlo**: `w_file_stdc.c` **no slurpea el WAD**. Lee el
directorio de lumps al abrir y luego cada lump por `fseek`+`fread` cuando hace
falta, a memoria de la zona. Nunca hay 4 MiB en vuelo. Lo que si hizo falta fue
que `fseek` entendiera `SEEK_END`, porque el WAD se MIDE con el (fase 1.12).

★ **El WAD se nombra, no se busca.** `d_iwad.c` sabe rebuscar en directorios
estandar y en variables de entorno, y aqui no hay ni lo uno ni lo otro --
`getenv` contesta que no hay y lo dice. Se le pasa la ruta por `myargv`, que es
un camino que DOOM ya tiene y que no obliga a inventarse un sistema de ficheros
que BMO-X no promete.

### ★★ 3.1 SI hacia falta, y no por el motivo escrito -- 2026-08-11

DOOM no cargaba el WAD entero. **BMO se lo cargaba por el.** `archivo::open`
pedia los 4.196.020 bytes en marcos CONTIGUOS y los leia de golpe, justo despues
de que DOOM se llevara sus 12 MiB de zona:

```text
   M_LoadDefaults: Load system defaults.
   Unknown configuration variable: 'use_joystick'
   <- y aqui se acaba. Lo siguiente de `D_DoomMain` es `W_Init: Init WADfiles`
```

O sea que la casilla 3.1 estaba bien contada por el lado de DOOM --que no lo
slurpea-- y no se miro el lado de BMO, que si. **[x] Arreglado**: un archivo
abierto para leer ya no se trae, se **refleja** -- un cursor de FAT32, una
ventana de 64 KiB para las lecturas de siete bytes, y cada `fread` trayendo su
rango del disco al bloque del programa. Ver `docs/identidad/LA_RAM.md`, seccion del 08-11.

Y la nota vieja de esta fila --*"el camino de lectura copia por un bufer de
rebote del kernel... si duele, lo arregla el DMA al bufer del llamante"*-- queda
cerrada de paso: `ARCH_OP_LEER_EN` escribe por el espejo fisico del bloque, asi
que el HBA deja los sectores enteros **dentro de la zona de DOOM**. Lo que se
mide sigue siendo `disk::cuentas_dma()`, y ahora ademas `archivo::cuentas()`.

---

# FASE 4 -- Que se pueda jugar de verdad

| # | Casilla | Tam |
|---|---|---|
| 4.0 | Guardar partida (`fwrite` + modo I-O) | M |
| 4.1 | El menu y los cheats (dependen de `M_snprintf`, o sea de 1.0) | S |
| 4.2 | Medir fotogramas por segundo y decirlo | S |

---

# FASE 5 -- SONIDO

> **Estado real, comprobado**: `platform/drivers/audio/` existe y son **109
> lineas de altavoz de PC** -- `outb` a un puerto y un retardo por TSC. Sirve
> para un pitido, no para DOOM. Y **no lo llama nadie**: es uno de los crates
> huerfanos de la auditoria de deuda tecnica.

O sea que el audio de verdad **empieza de cero**, y por eso va al final: DOOM se
juega entero sin sonido, y ninguna de las fases de arriba lo necesita.

## ★★ LA FASE ESTABA MIRANDO AL SITIO EQUIVOCADO (2026-08-10)

El 10 de agosto Vivaldi corrio en el Ryzen y no se oyo. El log dijo por que, y
de paso reordeno esta fase entera:

```
   info uaudio  el aparato guardo OTRO volumen =35
```

`=35` es el eco piano de la pieza, y **el audifono USB lo guardo**. O sea que la
cadena del volumen funciona de punta a punta hasta el aparato de verdad. Lo que
no llega es la NOTA: `AUDIO_OP_VOLUME` va al altavoz del PC **y** al audifono,
y `AUDIO_OP_BEEP` va **solo** al altavoz -- que en esta placa no tiene zumbador.

** Asi que el camino corto no es HD Audio, es USB, y lo caro ya esta pagado:

```
   xHCI                      HECHO       enumerar el aparato    HECHO
   leer sus descriptores     HECHO       control transfers      HECHO
   transferencias ISOCRONAS  <- FALTA, y es lo unico
```

`platform/drivers/usb/uaudio` lo dice en su primera linea. `bmo-xhci` tiene
`queue_interrupt_in` y le falta su equivalente isocrono de salida.

| # | Casilla | Tam | Nota |
|---|---|---|---|
| 5.0b | ★ Isocronas de salida en `bmo-xhci` | L | ✅ **HECHO**: `queue_isoch_out`, y el tubo se abre solo al reclamar |
| 5.0 | ~~Decidir el aparato~~ | M | ~~HD Audio o AC'97~~ **Contestado por el metal: USB** |
| 5.1 | Enumerar el aparato y abrir un stream de salida | XL | ✅ **CONFIRMADO en el Ryzen 2026-09-22, 00:21**: `1B3F:2008` reclamado, ranura 3, `tubo 1`, 48.000 Hz, 192 B por trama |
| 5.2 | `KIND_AUDIO` como capability | M | ✅ **HECHO**: un propietario a la vez, y se recupera solo si muere |
| 5.3 | ★ El modulo de sonido de DOOM, y su mezclador | L | ✅ **ESCRITO el 2026-09-22**: `bmo_sonido.c`, y DOOM compila con el. Falta el metal |
| 5.4 | Musica MUS -> MIDI (`mus2mid.c` ya compila) | XL | ✅ **ESCRITA el 2026-09-22** sin MIDI: FM propia sobre el GENMIDI del WAD (5.4 abajo). Falta el metal |

★ **La linea honesta**: 5.0 a 5.3 son "DOOM con efectos". 5.4 es "DOOM con
musica", y eso pide un sintetizador. **Se paran en 5.3 y se dice.**

## 5.3 -- POR QUE DOOM NO SONO EL 22-09, y no es del tubo (2026-09-22)

El propietario jugo con el audifono ya reclamado y el tubo abierto, y no oyo nada.
La causa no esta en el bus: **este DOOM esta compilado SIN sonido**. En
`i_sound.c`:

```c
static sound_module_t *sound_modules[] = {
    #ifdef FEATURE_SOUND
    &DG_sound_module,
    #endif
    NULL,
};
```

`FEATURE_SOUND` no se define en la construccion de BMO-X, y
`doom-port/unity.py` salta `i_sdlsound.c` e `i_allegrosound.c` (los dos unicos
ficheros que definen `DG_sound_module`, y los dos arrastran SDL). O sea que la
lista tiene un solo elemento, `NULL`; `InitSfxModule` no encuentra modulo,
`sound_module` se queda en `NULL`, y **cada `I_StartSound` es un `if` que no
hace nada**. DOOM no es que no suene: es que **no lo pide**.

Asi que la respuesta a *"se necesitan todo eso?"* es NO. De las cinco
casillas de la fase 5, cuatro ya estan pagadas por el metal. Queda UNA, y vive
en Ring 3:

```text
   DOOM da                         el tubo quiere
   -------                         --------------
   11.025 Hz                       48.000 Hz
   8 bits sin signo                16 bits con signo
   mono                            2 canales (192 B/ms = 48 x 2 x 2)
   hasta 8 canales a la vez        UNO ya mezclado
   por canal: volumen y `sep`      la mezcla decide izquierda/derecha
```

Entre las dos columnas hay un **mezclador**, y es aritmetica entera: remuestrear
x4,3537 (48.000/11.025), escalar por volumen, repartir por `sep` en dos canales,
sumar los ocho con saturacion, y escribir en el bloque PRESTADO que el aparato
lee por DMA (A4) -- el mismo que `musica.inti` ya usa. Lo que hay que escribir
es `bmo_sonido.c` en el puerto: los ocho verbos de `sound_module_t`
(`Init`, `Shutdown`, `GetSfxLumpNum`, `Update`, `UpdateSoundParams`,
`StartSound`, `StopSound`, `SoundIsPlaying`, `CacheSounds`) y el mezclador
detras. Sin SDL, sin libsamplerate y sin tocar el kernel.

**El orden**: antes de esto, `musica` tiene que SONAR. Es un stream, ya a la
frecuencia del aparato, sin mezclar ni remuestrear: si eso no se oye, el
mezclador de DOOM solo pondria ocho veces el mismo silencio.

## [X] 5.3 -- HECHO el 2026-09-22 (sin metal): `bmo_sonido.c`

`musica` sono a las 08:23, asi que el orden se cumplio y le toco a esto. El
modulo vive en `BMO-externo/doom/doomgeneric/doomgeneric/bmo_sonido.c` --DOOM
es GPL-2.0 y este arbol es Apache-2.0-- y tiene cuatro piezas:

| pieza | que hace |
|---|---|
| el lector de DMX | la cabecera de 24 B del lump: formato, frecuencia y cuantas muestras. **Toma el menor entre lo declarado y lo que hay**: un WAD tocado que declare de mas seria leer memoria de otro y mandarla por el altavoz |
| el mezclador | ocho canales, remuestreo en coma fija 16.16 (de 11.025 a 48.000 el paso es 15.059), `vol` y `sep` de DOOM a dos ganancias, **suma en 32 bits** y recorte al salir |
| el tubo | reclama el sonido, lee `BYTES_MS` y `FRECUENCIA` del aparato --**los canales se CALCULAN**: 192/(48x2) = 2, no se suponen--, presta un bloque de 4 MiB y arma |
| el modulo de musica | un armazon que contesta `false` y lo DICE: sin sintetizador MIDI no hay musica (5.4) |

Y tres cosas que hubo que quitar de en medio, todas dichas donde pasan:

1. **`sonido.h` no conocia el tubo.** La cabecera de C tenia pitar, volumen y
   callar, y el altavoz de esta placa no tiene zumbador: un programa de C solo
   podia pitar al vacio. Ahora trae `BMO_SONIDO_TUBO` con sus catorce campos y
   `bmo_tubo()`. **Eso esta en el repo** (`tables/bmo/sonido.h`) y vale para
   cualquier tercero, no solo para DOOM.
2. **`i_sound.c` incluye `<SDL_mixer.h>` bajo `FEATURE_SOUND` y no usa ni un
   simbolo suyo** (comprobado con grep: las tres apariciones son comentarios).
   Un armazon vacio en `doom-port/include` en vez de tocar DOOM.
3. **`use_libsamplerate` y `libsamplerate_scale`** los definia `i_sdlsound.c`,
   que se salta. Van en `doomgeneric_bmo.c` **antes** del agregado, porque BMO
   C resuelve los nombres en el orden en que los lee.

**El precio que esto tenia, y que ya NO tiene** (ver 5.3c): el bufer prestado
se usaba como si fuera lineal, con 4 MiB y un re-ofrecer cada 21,8 segundos.
Era un rodeo, este fichero lo decia, y el propietario lo mando quitar: *"que no
sea parche, cambiar piezas"*. Se cambio la pieza en el kernel.

DOOM compila con el: **740.232 B**, +8.440 sobre la version muda (+1,2 %).

| que | afirma | como se cae |
|---|---|---|
| `run apps/doom.bex` con el audifono | `[bmo] sonido: tubo USB a 48000 Hz, 2 canales, 192 B/ms`, y **se oyen los disparos** | `no hay tubo`: el audifono no entro en ese arranque; `lo tiene otro programa`: algo no solto el sonido |
| el `save` despues | `encoladas` sube, `tarde 0`, `huecos` unas pocas decenas | `huecos` en cientos: la ventaja de 100 ms no basta y hay que subirla |
| la musica | no suena, **y lo dice al arrancar** | -- |

## [X] 5.3b -- EL METAL DIJO QUE NO, y el fallo era de ESQUEMA (2026-09-22, 09:42)

El mismo arranque dijo las dos cosas, y las dos eran verdad:

```text
   DOOM:  [bmo] sonido: no hay tubo (audifono USB) -- DOOM en silencio
   save:  tubo 1      frecuencia 48000 Hz     bytes por ms 192
```

## Lo PRIMERO fue descartar al compilador, con bytes

Antes de tocar nada se desensamblo la llamada, que es la regla de esta casa
(*"desensamblar con llvm-objdump ANTES de sospechar del metal"*). `--map` del
compilador da el offset de cada funcion, y la cabecera BEF2 dice donde empieza
la region de codigo (byte 24):

```text
   bmo_tubo:
     48 8b ca               mov rcx, rdx     ; dato  -> a1
     48 8b d6               mov rdx, rsi     ; campo -> a0   (en ESTE orden)
     48 c7 c6 05 00 00 00   mov rsi, 5       ; op = BMO_SONIDO_TUBO
     49 c7 c0 00 00 00 00   mov r8, 0        ; a2
     e9 05 fe ff ff         jmp bmo_valor    ; rdi = cap, intacto

   bmo_valor:
     4c 8b d1               mov r10, rcx     ; a1 al registro que `syscall` no pisa
     48 c7 c0 00 00 00 00   mov rax, 0       ; NR_INVOKE
     0f 05                  syscall
     48 89 d0               mov rax, rdx     ; el VALOR vuelve en rdx
     c3                     ret
```

**Correcto byte a byte**, incluido el detalle que podia haberlo roto: `rcx` se
carga desde `rdx` ANTES de que `rdx` se pise. La emision a x86-64 esta limpia,
y la sospecha era del ayudante, no del codigo del propietario.

## El fallo, entonces: un tubo NO es una constante del arranque

El audifono de esta casa **falla su primer intento de enumeracion** --su ficha
sale `sin papeles`-- y entonces el puerto entra en la escalera de descanso del
bus: **5, 10, 20 y 40 segundos** (`uhid/puertos.rs`, `MAX_DOBLADOS`). Su tubo
puede abrirse medio minuto despues del arranque. DOOM pregunto UNA vez, a los
tres segundos, y se rindio para siempre.

Y habia una segunda mitad, peor: al rendirse **se quedaba la capability del
sonido**. El `save` lo dijo con el pid: `[!] audio el propietario del sonido
MURIO ... =2`. O sea que DOOM, ademas de no sonar, **dejaba al resto de la
maquina sin poder sonar** mientras corria.

## El arreglo

| era | es |
|---|---|
| preguntar una vez en `Init` | `bmo_snd_abrir()`, que se puede llamar muchas veces |
| rendirse para siempre | `Update` vuelve a mirar **dos veces por segundo** (`BMO_SND_MIRAR_CADA` 18 de ~35 vueltas/s) |
| `Init` devuelve `false` si no hay tubo | devuelve `true`: si devolviera `false`, `i_sound.c` dejaria `sound_module` a nulo y **`I_UpdateSound` no se llamaria nunca**, o sea que no quedaria quien reintentara |
| quedarse la capability al fallar | **se suelta** en todas las salidas |
| `ya va sobrado` | el motivo dicho: escribir de mas es RETRASO entre el disparo y el ruido, que es lo unico que el jugador nota |
| `no hay tubo` a secas | `(intento N)` al entrar, y `nunca hubo tubo (N miradas)` al salir |

DOOM: **740.784 B**. Sin metal todavia.

| que | afirma | como se cae |
|---|---|---|
| `run apps/doom.bex` y esperar | `aun no hay tubo; se sigue mirando` y, unos segundos despues, `tubo USB a 48000 Hz, 2 canales, 192 B/ms (intento N)`; y se OYE | `nunca hubo tubo (N miradas)` al salir: el audifono no entro en ese arranque -- el `save` lo confirma con `audifono 0 ranura` |
| el `intento N` | dice si entro a la primera o por la escalera del bus | -- |
| el `save` despues | ya NO sale `el propietario del sonido MURIO` con el pid de DOOM | si sale, queda una salida sin soltar |

## [X] 5.3c -- EL BUFER PRESTADO ES UN ANILLO DE VERDAD (2026-09-22)

El propietario, al leer el arreglo de los 4 MiB: *"fijate que no sea parche... NO
PARCHE, si CAMBIAR piezas y madurar"*. Tenia razon, y la pieza estaba en el
kernel.

### Lo que habia: un circulo de palabra

`dev/usb/audio.rs` decia en un comentario que *"el bufer es circular por acuerdo
con la app, que reinicia su `escrito` al mismo tiempo"*. Pero:

```rust
let hay = p.escrito.checked_sub(p.leido)?;   // <- el atasco
```

**En cuanto la app daba la vuelta**, `escrito` quedaba por DEBAJO de `leido`, la
resta devolvia `None` y el tubo se quedaba sin nada que mandar **hasta que
`leido` llegara al final** -- que no llegaba, porque `leido` solo avanza cuando
hay algo que mandar. Un punto muerto.

Por eso todos los productores acabaron inventandose el mismo rodeo: volver a
OFRECER el bloque para poner los dos indices a cero. `musica.inti` lo hace en
`vacia()`; el modulo de DOOM lo hacia en `bmo_snd_sitio`. **Dos programas con
el mismo parche para el mismo agujero es la prueba de que el agujero es de la
pieza, no de los programas.**

Y habia una segunda mitad: el bufer daba la vuelta en `bytes`, y `bytes` no
tiene por que ser multiplo de una trama (4.096 entre 192 son 21 y sobran 64).
El corte caia en mitad de una muestra, y **la app no tenia forma de saber
donde**, porque nadie se lo decia.

### Lo que hay: un anillo con su medida dicha

| pieza | que cambia |
|---|---|
| `Prestado.anillo` | `bytes` redondeado hacia abajo a tramas enteras. Una trama **no cruza nunca** el final |
| `hay_en()` | cuenta la vuelta: `escrito >= leido` es la resta; si no, `(anillo - leido) + escrito` |
| `escrito()` | dar la vuelta es LEGAL, y el tope es el anillo, no `bytes` |
| `siguiente_trama()` | usa `hay_en` y envuelve en `anillo` |
| `BMO_TUBO_ANILLO` (campo 14) | **la app pregunta donde dar la vuelta en vez de adivinarlo**, en C y en Rust |

Y lo que se cae solo detras:

* DOOM pasa de **4 MiB a 256 KiB** --dieciseis veces menos memoria-- y
  `bmo_snd_sitio` desaparece. Ya no hay silencios cada 21,8 s.
* `musica.inti` deja de poder escribir mas alla del anillo: su `medida_pcm`
  separa el bloque (4 MiB) de lo escribible, con el margen de una trama larga
  (1 KiB) porque esa funcion no tiene el handle para preguntar y en INTI no hay
  globales donde guardarlo. Su `vacia()` sigue valiendo: re-ofrecer no esta
  prohibido, solo ha dejado de ser obligatorio.

### Y lo del compilador: la comprobacion a mano pasa a ser una FILA

Antes de tocar nada se desensamblo la llamada y salio **correcta** (ver 5.3b).
Pero una comprobacion a mano no vuelve a correr sola, y el banco **no podia
hacerla**: `ObservedSyscall` guardaba tres de los cinco argumentos de la
puerta, asi que **ninguna prueba podia mirar `a1`** -- que es justo por donde
viaja el `dato` de `bmo_tubo`.

Ahora el emulador observa los cinco, y `convencion.rs` tiene la fila
`el_reenvio_de_dos_parametros_llega_a_r10`: dos parametros reenviados a las
ranuras tercera y cuarta, que es la forma exacta con la que se pide el tubo. Si
alguien pisa `rdx` antes de tiempo, `a1` llegaria con el valor de `a0` y la
fila cae.

DOOM: **740.098 B**. Sin metal todavia.

## [X] 5.3d -- *"ESCUCHO TODO, PERO ESTA RARO"*: el latido servia el DOBLE (2026-09-22)

DOOM sono a la primera (`intento 1`) y el propietario lo oyo todo, raro. El `save`
lo decia con un numero: `encoladas 77.192` en ~38 s son **~2.030 por segundo**,
y un aparato Full Speed come **1.000**. El latido del kernel encolaba ocho
tramas cada 4 ms sin preguntar cuantas habia servido el xHC: el bufer de DOOM
se consumia al doble (efectos acelerados y con saltos) y el productor no
llegaba (`huecos 7.540`). **No era de este modulo: era de la pieza de abajo**,
y `musica.inti` lo tenia igual desde el primer dia.

El latido ahora se acompasa a **`MFINDEX`**, el reloj con el que el propio xHC
sirve los isocronos, y repone solo lo servido. Detalle y tabla en
[`METAL_2026-09-18.md`](../metal/METAL_2026-09-18.md) 3e-sexies.

## [X] 5.3e -- LOS EFECTOS SUBIAN A 48 kHz EN ESCALONES (2026-09-22)

Con el maestro a +24 dB el propietario oyo *"pelea y tirones"*. Parte era el
limite (ver S4c de [`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md)), y parte era
esto: `bmo_snd_mezclar` tomaba la muestra de `pos >> 16` a secas, asi que cada
muestra de 11.025 Hz se repetia ~4,35 veces. Un escalon son agudos que el
sonido no tenia -- un rechinar que +24 dB sube igual que todo.

Ahora va la RECTA entre la muestra y la siguiente (la ultima, hacia el
silencio). El mezclador crece 141 bytes (0x3A5 -> 0x432 en el `--map`); el
`.bex` no cambia de medida porque sus regiones van redondeadas.

**Y la palanca de volumen limpia la tiene DOOM**: su volumen de efectos viene
a 8 de 15. En su menu (Options -> Sound Volume) a 15 son **+5,5 dB sin tocar
el limite**, que es lo que el maestro no puede dar sin aplastar.

## [ ] 5.3f -- EL SONIDO VA CON EL RELOJ, NO CON EL FOTOGRAMA (2026-09-22)

> Codigo hecho; se cierra cuando `tirones` salga en 0 (o casi) en el metal.

El contador que separo los tirones del arranque contesto a las 14:26, y los
tirones **eran de verdad**:

```text
   huecos 4.854 | en marcha 3.626 | tirones 29 | el mas largo 1.420 ms | tarde 0
```

`tarde 0`: el aparato y el kernel sirvieron todo a su hora. El que no llegaba
era el MEZCLADOR de DOOM: solo se rellenaba desde `I_UpdateSound`, una vez por
fotograma, y DOOM tiene bucles que no vuelven al fotograma. El grande es la
pantalla que se derrite entre mapa y mapa: `D_Display` hace
`do { ... } while (!done)` durante ~1,5 s --el `1.420 ms`--. Con 100 ms de
ventaja, todo lo que pase de 100 ms es un corte.

**La pieza.** Un port con SDL mezcla en un hilo de audio que no depende del
juego; BMO-X no le da hilos a una app. Pero todos esos bucles hacen algo que
el fotograma no: **preguntar la hora** (`wipe_ScreenWipe` en cada vuelta,
`TryRunTics` mientras espera). Asi que el relleno (`bmo_snd_rellenar`) se
cuelga del RELOJ: `DG_GetTicksMs` llama a `bmo_snd_reloj`, que rellena como
mucho cada 4 ms, y solo con el juego en marcha (`main_loop_started`) para que
la carga inicial no vuelva a contar como tiron. `I_UpdateSound` sigue
rellenando tambien.

| que | afirma | como se cae |
|---|---|---|
| `tirones` tras jugar y cambiar de mapa | 0, o muy pocos | siguen: hay otro bucle sin reloj |
| `el mas largo` | por debajo de 100 ms | cientos de ms: CARGAR un mapa (`P_SetupLevel`) no mira la hora, y es la siguiente pieza |
| el derretido entre mapas | suena entero, sin corte | se corta: el reloj no llega a la pieza |

> **5.3f queda SUPERADA por 5.3g sin llegar al metal.** Colgar el relleno del
> reloj curaba un sintoma: seguia siendo DOOM quien marcaba el tiempo.

## [ ] 5.3g -- LAS VOCES DEL ORQUESTADOR: DOOM declara, el kernel toca (2026-09-22)

> Codigo hecho y banco verde (3.046 filas); se cierra cuando el metal diga
> la tabla de abajo.

El propietario, tras tres arreglos seguidos a los tirones: *"mas elegante, por
completo: por algo es BMO, Bare Metal ORQUESTADOR"*. Lo que estaba mal era el
REPARTO: con el anillo PCM, DOOM tenia que ir SIEMPRE 100 ms por delante del
aparato, y cualquier bucle suyo que no volviera al fotograma (el derretido,
cargar un mapa) era un corte.

Ahora DOOM hace lo que hacia un juego con una tarjeta de voces (la GUS, la
AWE32 --que DOOM ya nombra en su lista de aparatos--, y en PC DirectSound con
sus bufer estaticos, u OpenAL con buffers y sources):

```text
   DOOM presta UN banco (2 MiB) con los efectos     bmo_voz_banco
   dice "toca este, a este volumen, a este lado"    bmo_voz_tocar
   mueve el lado cuando el monstruo anda            bmo_voz_ajustar
   y se olvida                                      -- lo mezcla el kernel cada trama
```

| pieza | donde |
|---|---|
| el mezclador (16 voces, recta entre muestras, paneo, sin coma flotante) | `platform/shared/bmo-amplificador/src/voces.rs`, 8 pruebas |
| el banco, la cola de ordenes y la mezcla en el hilo del bus | `Ultra_kernel_x86-64/kernel/src/ring0/dev/usb/voces.rs` |
| anillo + voces sumados en 32 bits, y por el MAESTRO al cable | `dev/usb/maestro.rs`, `componer` |
| la puerta: `AUDIO_OP_VOZ` (0x06), seis verbos por `arg0` | `bmo-abi` `objetos.rs`, `obj/audio.rs` |
| C: `bmo_voz_banco/tocar/ajustar/callar/suena` | `toolchain/forge/sem-asm/tables/bmo/sonido.h`, 2 filas en `toolchain/lang/c/emisor-x86_64/src/tests/voces.rs` |
| el modulo de DOOM, que ya NO mezcla | `BMO-externo/doom/doomgeneric/doomgeneric/bmo_sonido.c` (fuera del arbol, GPL) |
| el `save` | `voces sonando`, `banco`, `banco de`, `tocadas`, `rechazadas`, `perdidas` |

Y el mismo dia se cerro un hueco que existia desde A4: `memory::soltar`
devolvia al asignador un bloque que el TUBO (o ahora el banco) seguia
leyendo, porque solo preguntaba por los prestamos de `loan`. Ahora avisa al
audio antes (`audio::block_returned`).

**Lo que se gana:** si DOOM se atasca, lo que sonaba SIGUE sonando; del
disparo al ruido pasa de hasta 100 ms a lo que tarde una trama; y
`I_UpdateSound` ya no hace nada con el tubo abierto.

| que | afirma | como se cae |
|---|---|---|
| al arrancar DOOM | `[bmo] sonido: VOCES del orquestador, banco de 2048 KiB, tubo a 48000 Hz` | `el kernel no acepto el banco`: `cabina fallos` dice el motivo |
| disparar, puertas, monstruos | se oyen, con su lado | silencio con `tocadas` subiendo: la mezcla no llega al cable |
| el derretido entre mapas | los efectos que sonaban acaban enteros | se cortan: algo sigue dependiendo de DOOM |
| `tirones` en el `save` | 0 (el anillo ya no se usa en DOOM) | sube: mirar si otro programa usa el anillo a la vez |
| `rechazadas` y `perdidas` | 0 | >0: el motivo en `cabina fallos` |
| el panel F10 | el medidor se mueve con los disparos | quieto: las voces no pasan por el maestro |

**Lo que NO hace:** musica (5.4), sonido posicional de verdad (`sep` es un
paneo de dos canales) ni pistas para LA MESA (van a 0 hasta M3).

## [ ] 5.4 -- LA MUSICA: FM propia, la cancion entera en el banco (2026-09-22)

> Codigo hecho; el sintetizador comprobado en Windows y en el emulador; se
> cierra cuando se OIGA en el Ryzen.

El propietario eligio, con el coste delante, la musica **autentica**: la de la
Sound Blaster de 1993. DOOM guarda sus canciones como MUS (partitura a 140
tics por segundo) y sus instrumentos en el lump GENMIDI (175 instrumentos FM
de dos operadores). No hace falta MIDI ni un sintetizador de terceros: hace
falta **el chip**.

| pieza | donde | que es |
|---|---|---|
| el chip FM (YM3812, 18 canales) | `BMO-externo/doom/doomgeneric/doomgeneric/bmo_opl.c` | escrito desde la hoja del chip; solo enteros |
| sus tablas | `bmo_opl_tablas.c`, de `BMO-externo/doom-port/tablas_fm.py` | seno, ganancia, 2^x, KSL: BMO-X no tiene libm |
| la partitura sobre el chip | `bmo_mus.c` | MUS + GENMIDI -> registros, 18 voces |
| DOOM | `bmo_musica.c` | `music_module_t`: renderiza y la entrega a una VOZ |
| el bucle | `bmo_voz_tocar_bucle`, bit 56 de `AUDIO_OP_VOZ` | la voz vuelve al principio y no se calla sola |

**El reparto es el de los efectos llevado al extremo:** al pedir la cancion se
renderizan 6 s en el banco (24 kHz, S16 mono, de 2 a 16 MiB), se toca como UNA
voz en bucle en el canal 15, y el resto se renderiza un trozo de 150 ms en cada
`Poll`. El orquestador marca el tiempo; DOOM solo tiene que ir por delante, y
si alguna vez no llega lo DICE (`la voz ALCANZO al render`).

**Lo medido antes del metal** (`BMO-externo/doom-port/musica_host.c`, los MISMOS
ficheros compilados con cl):

| que | numero |
|---|---|
| las 13 canciones de doom1.wad | de 7 a 272 s (D_E1M3); 29,8 min en total |
| notas a la vez | hasta 15 (D_INTRO); con 18 voces, **0 robadas** en las que suenan |
| afinacion | la tecla 69 da 436-444 Hz en 8 instrumentos (el paso de la medida a 24 kHz) |
| picos | el mas alto 30.534 de 32.767 con la ganancia x1,75; **0 sujetadas** |
| en Windows (cl /O2) | 250-500 veces el tiempo real |
| ★ BMO C contra cl | **el mismo hash** en 2 s de D_E1M1 (48.000 muestras) en el emulador |
| en BMO C | ~47 M instrucciones por segundo de musica: "del orden de 5 ms en el Ryzen" -- **FALSO**, el metal dijo ~14 ms por 150 ms de musica (ver 16:09 abajo) |

★ Y lo que se cazo al medir: **DMX numera las notas una octava por ENCIMA del
MIDI**. La primera fila de su tabla de frecuencias es un F-Number de 0x133
(14,56 Hz), que solo cuadra asi, y 72 de las 128 portadoras del GENMIDI lo
compensan con multiplo x0,5. Con el LA en la 69 toda la musica habria sonado
una octava grave; va en la 57.

**Lo que es propio y NO de DMX, dicho:** 18 voces (DMX tenia 9 y robaba), la
curva de volumen de General MIDI (40 log10) y no la de DMX, el envolvente
aproximado (el ataque es exponencial con el tiempo de la hoja del chip).
Paneo, modulacion y pedal se ignoran; pausar baja la voz a 0 y la cancion
sigue corriendo callada.

| que | afirma | como se cae |
|---|---|---|
| al arrancar | `[bmo] musica: FM de 18 voces con el GENMIDI del WAD, 24000 Hz` | `este WAD no trae GENMIDI` |
| en el titulo y en E1M1 | se oye la musica, en bucle | silencio: `cabina fallos` y `voces sonando` en el `save` |
| en la consola, al rato | `[bmo] musica: 96 s renderizados en N ms (sujetadas 0, notas sin voz 0)` | nunca sale: `Poll` no se llama |
| el derretido y cargar un mapa | la musica sigue, sin corte | `la voz ALCANZO al render`: el adelanto de 6 s no basto |
| Options -> Music Volume | sube y baja | no cambia: `ajustar` no llega |

### 16:09 del 22-09: el metal contesta -- SUENA, y la cuenta del coste era FALSA

El `save` de las 16:09: `tocadas 594`, `rechazadas 0`, `perdidas 0`, `tarde 0`,
`tirones 0`. Los efectos por voces y la musica suenan. Pero el `[perf]` de
DOOM (`datos/APPSDOOM.TXT`) dice otra cosa que nadie pregunto:

```text
   antes de cambiar de mapa   57 fps   fotograma 17,4 ms   blit ~1,7 ms
   despues (casi un minuto)   31 fps   fotograma ~31 ms    blit ~1,3 ms
```

Empieza justo detras de `[heap] cepo ARMADO`, que lo arma `P_SetupLevel`: el
mapa nuevo trae cancion nueva, y `Poll` renderizaba 150 ms de musica por
fotograma. **Eso costaba ~14 ms en el Ryzen** -- no el ~1 ms de la tabla de
arriba, que salia de contar instrucciones en el emulador con una cancion
todavia floja (el principio de D_E1M1) y suponer un ritmo de CPU que BMO C no
da. Dos arreglos, los dos medidos:

| que | antes | ahora |
|---|---|---|
| el sintetizador: ondas en tabla, sin llamadas por muestra, envolvente y LFO cada 4 muestras | 95 M instrucciones por 2 s de D_E1M1 | **37,6 M** (x2,5), y el MISMO hash en BMO C y en cl |
| `Poll` | 150 ms de musica, cueste lo que cueste | **~2 ms de reloj** por vuelta; 8 si la voz esta a menos de 2 s de lo hecho |

Y el numero que ya no se adivina: al acabar cada cancion DOOM escribe
`[bmo] musica: N s renderizados en M ms = X us por segundo de musica`. Esa
linea es el juez; la estimacion de arriba queda como lo que fue.

---

# La cuenta, para poder repartir

Actualizada el **2026-08-13**.

| Fase | Casillas | Faltan | Estado |
|---|---|---|---|
| 1 -- el unity termina | 6 | 0 | **[x]** compila a `.bex` |
| 1.5 -- lo que no estaba en la lista | 3 | 0 | **[x]** monton, tecla cruda, `fseek` |
| 2 -- la plataforma | 6 | 0 | **[x]** escrita, `doomgeneric_bmo.c` |
| 3 -- el WAD | 3 | 0 | **[x]** escrito -- `-iwad apps/doom1.wad` |
| 4 -- jugable | 3 | 2 | guardar partida pide `fwrite`, que devuelve 0 |
| 5 -- sonido | 6 | **0** | 5.0b/5.0/5.1/5.2 en metal el 22-09; 5.3 (efectos, por VOCES: 5.3g) y 5.4 (musica FM) escritos el mismo dia, sin metal |

★★ **NO QUEDA NINGUNA CASILLA POR ESCRIBIR.** Lo que queda es **un defecto del
compilador**, localizado el 2026-08-13 y con reproduccion en el emulador.

---

# ★★★ DONDE MUERE DOOM HOY -- 2026-08-13, y ya no es una teoria

**`&c->defaults[i]` vale CERO.** Tres lineas de `codegen/mod.rs`.

## Lo que se vio en el Ryzen

DOOM arranca, toma pantalla y raton, imprime nueve lineas suyas y **se muere
solo**, sin fallo del kernel:

```text
   Doom Generic 0.1
   Z_Init: Init zone memory allocation daemon.
   zone memory: e00fa050, 600000 allocated for zone
   V_Init: allocate screens.
   M_LoadDefaults: Load system defaults.
   Unknown configuration variable: 'use_joystick'      <- LA ULTIMA
```

```text
   103 WARN fb:    el propietario de la pantalla MURIO
   104 WARN gui:   murio sin decir una sola linea
   105 INFO ring3: proceso termino por su cuenta (EXIT)
```

[!] **Esa ultima linea de DOOM no es un aviso: es la causa de muerte.**
`m_config.c:1954` la emite con `I_Error`, que imprime y llama a `exit`. Leerla
como ruido es lo que hizo perder un dia.

## El camino, entero

```
   I_BindJoystickVariables            i_joystick.c:343
     -> M_BindVariable("use_joystick")
        -> GetDefaultForName          m_config.c:1937
           -> SearchCollection        m_config.c:1567
              -> return &collection->defaults[i];    <-- AQUI
```

`SearchCollection` **encuentra** la entrada --el `strcmp` acierta-- y devuelve
su direccion con `&collection->defaults[i]`. Esa expresion es
`AddrOf(IndexPtr(..))`, y el brazo `Expr::AddrOf` de `codegen/mod.rs:2886` sabe
emitir tres formas:

```rust
Expr::Var(..)        => lea de la variable
Expr::Subscript(..)  => emit_subscript_addr
Expr::Deref(..)      => la direccion apuntada
_                    => self.emit_xor_eax(),      // <-- y aqui cae la de DOOM
```

O sea que el compilador emite `xor eax,eax`, **la direccion pedida sale CERO sin
un solo aviso**, `GetDefaultForName` devuelve `NULL` y DOOM se mata a si mismo a
56.465 lineas del sitio donde esta el fallo.

** Es la tercera vez que el mismo patron cobra: un `_ =>` que rellena de ceros
lo que no sabe traducir. Las otras dos fueron el `char *mapa` del raycaster
(`2bc13367`) y las relocations que no existian (`46506e51`).

## La reproduccion, en el emulador y sin encender la maquina

`toolchain/lang/c/emisor-x86_64/src/tests/tabla_de_config.rs`. Seis tests verdes que
**descartan** la tabla, la cuenta, el operador `#`, la escala de 200 punteros y
`strcmp`; y cuatro `#[ignore]` que reproducen el defecto:

```powershell
cargo test -p bmo-c-x86-64 tabla_de_config -- --ignored
```

El reparto entre ellos ES el diagnostico:

| forma | |
|---|---|
| `&c->campo[i]` | **ROJO** -- la de DOOM |
| `c->campo + i` | **ROJO** -- misma familia, aritmetica en vez de `&` |
| `p = c->campo; &p[i]` | VERDE -- copiar a un local lo arregla |
| `&global[i]` | VERDE -- sin campo en medio no pasa |

## ✅ EL ARREGLO -- HECHO el 2026-08-13, y salieron TRES defectos

**1. Tres brazos nuevos en `Expr::AddrOf`**: `IndexPtr`, `Field` y `Arrow`. Son
la version SIN CARGA de los que ya existian mas abajo -- calcular la direccion
es lo mismo que leer el valor menos el ultimo paso.

★ Y ahi aparecio el segundo: **`&s.campo` y `&p->campo` tambien valian CERO**.
Eso es C de todos los dias --pasar un campo por referencia-- y estaba roto
desde siempre; no se habia notado porque ningun ejemplo de BMO lo hacia.

**2. El `_ =>` ya no rellena de ceros: acumula un error con la expresion
dentro.** Es lo que de verdad cierra esto -- mientras devolviera cero en
silencio, el siguiente hueco costaba otro dia de fotos.

**3. Y el tercero, que es el peor por lo general**: `pointer_scale` media con
`TypeSpec::stack_size()`, que contesta **0** para un `StructRef` porque desde el
AST no hay tabla de medidas. Con `0`, la funcion decidia *"esto no es un
puntero"* y no escalaba: **`p + 1` sobre un `struct T *` avanzaba UN BYTE**. No
es un caso raro de DOOM -- es cualquier recorrido de una tabla de structs con
aritmetica en vez de subindice. Ahora mide con `type_stack_size`, que es la
misma cuenta con la tabla delante, la que ya usaba el subindice. **El subindice
acertaba y la suma no, siendo la misma direccion escrita de dos formas.**

**Lo que se comprobo antes de dar esto por bueno:**

| | |
|---|---|
| Suite de C | **397 verdes, 0 rojos, 0 ignorados** |
| Los cuatro tests que reproducian el fallo | verdes, y se quedan de guarda |
| DOOM recompila | si -- 816.904 B (crecio 2 KB: son las direcciones que antes eran `xor`) |
| Los 12 ejemplos de C recompilan | si, ninguno dispara el error nuevo |

⚠ **PENDIENTE, y es de metal**: `build.ps1 -Flash` para desplegar los `.bex`
nuevos y volver a lanzar DOOM. Toca el codegen, o sea **todos** los `.bex` de C:
si algo que arrancaba deja de arrancar, es esto.

⚠ Y lo que este arreglo **no** promete: que DOOM sea jugable. Era el primer
rechazo despues de las tres puertas del sistema; puede haber mas detras. Lo que
ya no habra es una muerte muda.

## Lo que este fallo NO es, y esta descartado con pruebas

- **No es el cargador ni FAT32.** `bytes DIRECTOS del disco al marco = 813.568`
  frente a `el fichero mide = 815.496` **no es una lectura corta**: los 1.928 de
  diferencia son la seccion `Resources` (el icono), que `admitir_por_rangos` no
  se trae a proposito.
- **No es el WAD.** Esta en el disco y con su medida exacto --`A:\apps\doom1.wad`,
  4.196.020 B-- y en las fotos **no aparece ni una linea `arch` con ese numero**:
  DOOM muere antes de `W_Init`, o sea antes de abrirlo.
- **No es la pantalla.** DOOM la reclama (`fb: pantalla cedida a Ring 3`) y muere
  sin escribir un pixel.

## ⛔ EL BLOQUEANTE DEL 2026-08-09 -- CERRADO, historico

```
   83 WARN proc:   el .bex de disco no paso la admision =4
```

Era la **relocation partida entre dos paginas** (`a3fbe9fe`, 08-11), y detras
habia dos mas: el `+ part_lba` que faltaba en FAT32 (`ea7ad1e0`) y las tablas
out-of-band leidas antes que el codigo (`60dd6ddd`). Las tres puertas del
sistema estan cerradas desde el 08-11.

Se conserva la leccion, que sigue valiendo: **el siguiente paso no era tocar
codigo, era MEDIR** -- que `lanzar` dijera bytes traidos frente a bytes del
fichero. Esos dos numeros son los que hoy descartan la lectura corta de un
vistazo.

## Lo que puede salir mal DESPUES, para no volver a escribirlo

| Sintoma | Sospechoso |
|---|---|
| **Muere tras `M_LoadDefaults` sin decir mas** | **`&c->defaults[i]` = 0. Arriba** |
| `DOOM: no hay pantalla` | HASTA EL 11-09. Desde entonces con la pantalla ocupada DOOM pide una VENTANA; si sale `DOOM: ni pantalla ni ventana`, nadie compone y la pantalla es de otro |
| `W_AddFile: doom1.wad no encontrado` | la ruta del WAD, o FAT32 no monta |
| Se para y **no sale `W_Init`** | el WAD. Era `archivo::open` tragandoselo entero (arreglado el 08-11); si vuelve, mirar `arch` en CABINA |
| Arranca y muere sin pintar | el monton: 12 MiB CONTIGUOS en fisico. CABINA dice si el kernel los nego |
| Pinta y no responde | `DOOM: sin teclado` en la consola lo dice antes |
| Anda solo y no para | la cola cruda no llega: el `soltar` se perdio |
| Va a tirones | el blit, o `DG_SleepMs` cediendo mal |

## [ ] DOOM EN UNA VENTANA -- escrito el 2026-09-11, sin metal todavia

DOOM solo conocia `<bmo/pantalla.h>`: la pantalla entera o nada. Por eso *"se
ejecuta sin nada"*: mientras corre no hay escritorio, y al morir no vuelve a
nadie. El modelo de VENTANA existia desde el 23-08 (`<bmo/superficie.h>`,
`ray.bex` en una caja con teclas por buzon); DOOM no lo pedia.

Ahora `DG_Init` pregunta por la pantalla y, si la tiene el DIRECTOR, pide una
superficie de 960x600 con buzon: `DG_DrawFrame` expande en esa memoria y sube
la secuencia, `DG_GetKey` lee el buzon, `DG_SleepMs` duerme en vez de ceder.
Con la pantalla libre --shell de Ring 0 o `presta`-- nada cambia.

*** Y el cambio real fue del COMPILADOR: `WANTS_SCREEN` se deducia de
`PANTALLA_RECLAMAR` y con ella el DIRECTOR se apartaba ANTES de lanzar, asi
que DOOM siempre habria encontrado la pantalla libre. Un programa que ademas
sabe componerse (`MI_PADRE`) ya no la lleva; `tests/bandera_de_pantalla.rs`
son las primeras filas que esa bandera tuvo en el banco. Prueba en metal: V1 de
`../metal/METAL_2026-09-10.md`.

Lo que se paga, dicho: escala fija x3 (Bloq Despl no aplica), F12 no aplica,
Alt es del escritorio (el ladeo con Alt+flecha no llega), y el coste de que el
DIRECTOR pegue 576.000 pixeles por fotograma NO ESTA MEDIDO.

## ** LA PANTALLA "BUGEADA" NO ES DE DOOM

Cuando DOOM muere ahi, **no ha pintado un solo pixel**. Lo que queda en el
monitor son los restos de tres pintores encima del mismo framebuffer: la ventana
de consola con las nueve lineas, el panel del kernel --que vuelve al morir el
propietario-- y el repintado del compositor al recuperarla.

O sea que la pantalla rota **es el sintoma de que DOOM no llego a dibujar**, no
un fallo del blit ni del troceado por cajas sucias (`4ea125c7`), que no toca a
DOOM: DOOM pinta con su propio blit.

[!] El camino de recuperacion (`main.rs:2294`) repinta fondo, lanzador, barra,
caja y salida -- **pero no las ventanas que estuvieran abiertas**. Con F11 o F12
abiertas al lanzar, esos rectangulos se quedan con lo que hubiera debajo. Es un
defecto propio y chico, y se ve exactamente igual que el otro.

## Lo que SI se vio, y no es poco

El escritorio arranco con **el icono de DOOM y su nombre debajo**: listar
`apps/`, abrir el `.bex`, encontrar `Resources`, leer el indice `BRES`, sacar el
recurso `icono`, descifrar `BICO` y pintarlo -- siete pasos y dos formatos
leidos a mano, ninguno fallo.

[!] El icono salio **blanco** y deberia ser una cara roja. La silueta es la
correcta --el recorte transparente esta bien-- asi que lo que llego mal es el
color, no el dibujo. Abierto, y distinguible a ojo del otro caso: si fuera el
icono por defecto seria un cuadro macizo con una `D`.

---

Ver [`QUE_DESBLOQUEA.md`](../identidad/QUE_DESBLOQUEA.md) para el censo, `AVANCES.md` para el
estado y `BMO-externo/doom-port/` (fuera del repo) para la sonda y el unity.


---

# ★★★ EL PLAN DE 2026-08-23 -- Y LA MUERTE DE DOOM, LOCALIZADA EN UNA LINEA

> La pregunta del propietario: *"entonces el plan para arrancar a DOOM no es por abrir
> o compilar sino por sondas rojas, no?"*. **Si.** Y son tres puertas
> independientes, que es lo que hacia falta separar.

```text
   ABRIR      hecho en codigo el 23-08 (el doble clic).  Falta UN ARRANQUE.
   COMPILAR   `doom.bex` compila hoy: 880.250 B, bandera puesta.  El que NO
              pasa es el build de la IMAGEN, y lo para L6a -- que es de INTI y
              no de DOOM.
   JUGAR      las sondas rojas.  Y a partir de hoy ya no son una investigacion:
              son una linea con nombre y numero.
```

## 1 -- ✅ LOCALIZADO: `resolve_arrow_expr_offset` acaba en `.unwrap_or(0)`

`toolchain/lang/c/src/parser/types.rs`:

```rust
    pub(super) fn resolve_arrow_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        self.resolve_expr_type(expr)
            .and_then(|t| Self::pointee_struct_of(&t).map(str::to_string))
            .and_then(|s| self.get_field_offset(&s, field))
            .unwrap_or(0)          //  <-- AQUI
    }
```

**Cuando el tipo de la base no se deduce, el offset del campo pasa a ser CERO en
silencio.** Ni error ni aviso: la escritura cae en el primer campo del struct.

Y `tope - 1` es una binaria, que `resolve_expr_type` no sabe tipar. Asi que:

```c
   (tope - 1)->next = &unsorted;    // escribe en `prev`, que es el campo 0
```

`next` conserva `ds + 1` --una posicion mas alla del final--, el recorrido se
va del array, y en `+0x2c` (`scale`) revienta. **Es `R_SortVisSprites+0x2c6`
exacto, sin un cabo suelto.**

[!] Y tiene hermano en el mismo camino: `field_type_via_pointer` termina en
`.unwrap_or(TypeSpec::Long)`, o sea que el **ancho** de la escritura tambien se
inventa: 8 bytes.

### Como se llego, en cinco casillas y una vuelta

`toolchain/lang/c/emisor-x86_64/src/tests/sonda_resta_de_punteros.rs`:

```text
   `tope - 1` calcula la direccion            VERDE   560 = 7 x 80
   con el puntero en una VARIABLE, escribe    VERDE
   EN LINEA, escribe                          ROJA
   y donde cae                                `1 0`  -> al campo 0, no al suyo
```

★★ **La casilla que nombra al culpable es la ultima**, y por eso se conserva: un
`ROJA` dice que algo falla; `1 0` dice **que** falla. La diferencia entre las dos
verdes --variable intermedia si, en linea no-- es lo que apunta al TIPO y no a
la aritmetica ni al `->`.

## 2 -- ⚠ LA BIFURCACION, que es de esquema y no de teclado

El arreglo no es cambiar el `0` por otro numero. Son dos cosas y **hay que hacer
las dos**:

```text
   A. MOSTRAR a `resolve_expr_type` a tipar una binaria de puntero:
      `p - n`, `p + n` y `&arr[i]` conservan el tipo de `p`.
      Eso es lo que arregla a DOOM.

   B. Y que lo NO deducible deje de valer cero:
      un offset que no se sabe es un ERROR de compilacion, no un 0.
```

⚠ **B tiene radio de explosion y hay que decirlo por delante**: hoy el `0`
acierta **por casualidad** cada vez que el campo pedido resulta ser el primero
del struct. Al convertirlo en error, cualquier sitio que estuviera viviendo de
esa casualidad deja de compilar -- y eso incluye codigo de C del mundo, no solo
DOOM. Es la ley de la casa (*nada que compile y no haga lo que dice*) contra el
riesgo de que el arreglo se lleve por delante lo que hoy anda.

★ El orden que lo hace barato: **A primero y solo**, con el banco entero
delante. Si A pone verdes las tres casillas rojas y no rompe ninguna de las 449,
DOOM se recompila y se prueba. **B va despues y por su cuenta**, porque su
trabajo no es arreglar DOOM: es que el proximo fallo de esta familia se vea.

## 3 -- LA SEGUNDA SONDA ROJA, y la prediccion de que es la misma familia

```text
   la_resta_al_reves_sale_negativa   `arr - &arr[5]`  ->  -679168, esperado -5
```

Su propia cabecera ya dice *"falla lo que se le da a restar"*, no la division. Y
lo que se le da a restar es un array **decaido** en el lado izquierdo, o sea otra
vez **un operando cuyo tipo hay que deducir**.

> ★ **PREDICCION, escrita antes de tocar nada:** el paso A de arriba pone esta
> casilla verde tambien, o la deja a un pelo. Si despues de A sigue dando
> -679168, entonces son dos bugs y no uno, y esta es de `pointer_scale`.

## 4 -- ★★★ Y LA PREDICCION GRANDE: el destrozo del MONTON puede ser el MISMO bug

Del 14-08, la firma del bloque roto de la zona de DOOM:

```text
   BLOQUE 1336 en +1889056: dice 0, hasta el siguiente hay 672 | tag 1 id 1d4a11
```

Lo que se dedujo entonces, y sigue siendo cierto: **no es un desbordamiento**
--`tag` e `id` estan intactos-- sino *"un almacenamiento suelto del ancho de un
puntero, con valor 0"*, en el **offset 0** de la cabecera. Y en `memblock_t` el
campo del offset 0 es `size`.

★★★ Ahora hay que leer esa frase otra vez con el bug de la seccion 1 al lado:

```text
   offset 0        <- lo que devuelve `resolve_arrow_expr_offset` cuando falla
   8 bytes         <- lo que mide `TypeSpec::Long`, el tipo que se inventa
                      `field_type_via_pointer` cuando falla
```

**Las dos mitades de la firma son exactamente las dos mitades del fallo.**

> ★ **PREDICCION:** en `z_zone.c` hay al menos una escritura por flecha sobre
> una base calculada, y el paso A la arregla. Si tras A el monton sale SANO del
> arranque, eran el mismo bug y DOOM pierde sus dos muertes de una vez.
>
> **Y si no**, tambien vale: querra decir que el destrozo del monton es otra
> cosa, y habra costado un arranque saberlo en vez de una semana.

## 5 -- EL ORDEN, con lo que aprueba cada peldano

```text
   [x] 1  localizar                sonda que dice `1 0`, no solo ROJA
   [ ] 2  A: tipar la binaria      las 3 casillas nuevas en verde, y 449 sin
                                   una roja nueva
   [ ] 3  recompilar `doom.bex`    y comparar el medida contra 880.250
   [ ] 4  ARRANQUE                 pasa del primer fotograma dibujado?  y el
                                   monton, sale sano?  (la prediccion de 4)
   [ ] 5  B: el cero deja de ser   por su cuenta, con el radio medido: cuantos
          una respuesta            sitios del arbol vivian de la casualidad
```

[!] El peldano 3 no puede llegar al disco mientras L6a pare el build. **No es
una dependencia de DOOM: es la puerta de al lado**, y esta escrita en la seccion
0 de `../metal/METAL_2026-08-23.md`.
