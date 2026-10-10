# PLAN INTI SAMURAI -- INTI corta para la CPU; las apps se van a TITAN++

> Abierto el **2026-10-04** a peticion del propietario: *"en INTI vamos a
> eliminar las influencias que es para crear app ... su unico punto ES que
> EMITA al CPU pero nivel EXTREMO, que CORTE sin piedad como samurai; y el
> TITAN++ solo enfocara en GPU al extremo, y tendra que controlar al VERRANO"*.
>
> **Esto es el INVENTARIO y el plan de corte, nada mas.** Ni una linea de INTI
> se ha borrado todavia: cada corte rompe algo que hoy corre, y la lista de lo
> que cae la decide el propietario (seccion 3). El reparto con TITAN++ esta en
> [`TITAN_MAESTRO.md`](../maestro/TITAN_MAESTRO.md) seccion 7.

---

# 0. LA IDEA EN UNA LINEA

```text
   INTI       el SAMURAI de la CPU: registros, AVX2, la instruccion exacta,
              sin comportamiento indefinido. Nada mas
   TITAN++    el CENTAURO de la GPU: computo en la 3060, IA, y manda a
              VERRANO; las apps se construyen en el
```

INTI ya tenia la palabra: sus **katanas** (`emisor-x86_64/tests/katanas.rs`,
S1) son la tabla de reglas que cada binario declara, con el sitio exacto donde
corta cada una. El samurai no es un cambio de caracter: es quitarle lo que no
es suyo.

---

# 1. EL INVENTARIO: que tiene INTI hoy, y de que lado cae

Medido el 04-10 sobre el arbol (lineas de Rust o de `.inti`).

## 1.1 Lo que SE QUEDA: la CPU

| pieza | donde | por que es del samurai |
|---|---|---|
| el frontend sin UB | `toolchain/lang/inti/src/` (lexico 1535, sintaxis 2445, arbol 530, nombres 1084, tipos 680, ir 2893, disposicion 2065, perfil 1364...) | las doce reglas que sustituyen al UB de C: lo que hace que cortar rapido no sea cortar a ciegas |
| el emisor x86-64 | `toolchain/lang/inti/emisor-x86_64/` (marco, operaciones, puerta, metal, funcion) | es el unico que nombra la maquina |
| la maquina entera | `sem-asm/tables/arch/x86_64/inti.toml`, `intrinsics.toml` (AVX2 incluido) | los 16 registros con su rol, y el SIMD por intrinsecos de tabla (INTI_MAESTRO 13.7) |
| el perfil `llano` | `biblioteca.toml` `[llano]`, `comun.toml` `[ambos]` | sin monton, todo en la pila o estatico, y `crudo`: la velocidad de C |
| `crudo` | `modulos.toml` `[crudo]` | la valvula: la instruccion exacta cuando hace falta |
| la matematica | `runtime/matematica/` (exp 146, potencias 97) | calculo de CPU |
| las katanas | `tests/katanas.rs`, `bmo-abi::bef::katanas` | donde corta cada regla, declarado en el binario |
| las sondas de sistema | `sondas/cpu.inti`, `sondas/pulso.inti` | hablan con la CPU y con el kernel; `pulso` es trabajo de verdad en el metro (667 750 instrucciones) |

## 1.2 Lo que es de APP: los candidatos a salir

| pieza | donde | lineas | lo usa |
|---|---|---|---|
| el perfil `pleno` | `comun.toml` `[pleno]` (escribe, lee, listas, textos, ficheros...) y la comprobacion en `src/perfil` | ~60 nombres | todo programa `perfil pleno` |
| los objetos | `runtime/objetos/` texto, lista, tabla, contador | 608 | `pleno` |
| el monton | `runtime/monton/` origen, reparto | 176 | los objetos |
| la superficie | `runtime/superficie/` dibujo, roja, amarilla; `modulos.toml` `[superficie]` | 452 | `sondas/ventana.inti`, `navegar` |
| la lamina | `runtime/lamina/lamina.inti`; `[lamina]` | 358 | `navegar` |
| VERRANO | `runtime/verrano.inti`; `[verrano]` | 101 | `ejemplos/cubo.inti` -- **pasa a TITAN++**: el propietario quiere que VERRANO lo mande el centauro |
| la entrada | `runtime/entrada/buzon.inti`; `[entrada]` | 50 | las apps con teclado y raton |
| la letra | `runtime/fuente/datos.inti` (los glifos, generado por `bmo-fontgen`) | 177 | pintar texto en una superficie |
| los recursos del `.bex` | `modulos.toml` `[paquete]` | -- | las apps que traen datos dentro |
| el sonido | `modulos.toml` `[sonido]` | -- | `ejemplos/musica.inti` |
| sus pruebas | `emisor-x86_64/src/pruebas/objetos.rs` (22), `monton.rs` (11), `tabla.rs` (7); `tests/navegar.rs`, `ventana.rs`, `musica.rs` | -- | -- |

## 1.3 Lo que NO esta claro: lo decide el propietario

| pieza | lineas | a favor de quedarse | a favor de salir |
|---|---|---|---|
| `numero` decimal exacto (`runtime/decimal/`) | 193 | es aritmetica de CPU, y COBOL la heredo a la casa | TITAN++ trae `dec` (2b.1) para el dinero de las apps |
| los ficheros (`modulos.toml` `[archivo]`) | -- | `bico.inti` le gana a C leyendo ficheros de OTRO: es seguridad de sistema | leer y guardar es de app |
| `ejemplos/bico.inti`, `png.inti` | -- | herramientas, no apps; `bico` es donde INTI le gana a libpng | usan ficheros y texto |

---

# 2. LO QUE SE ROMPE, dicho antes de cortar

*"Ya no importa si se rompe"* (el propietario). Bien -- pero que se rompa
SABIENDO que:

```text
   el build       Ultra_kernel_x86-64/build/ejemplos.ps1 despliega siete .ibx:
                  cpu, pulso (se quedan), ventana, bico, musica, cubo, navegar
   la app         Ultra_userspace/apps/navegar/navegar.inti deja de compilar
                  (pide superficie, lamina, texto y entrada). PLAN_NAVEGAR.md
                  queda sin cara hasta que exista en TITAN++
   el metro       seis filas de INTI en LINEA_BASE.txt; ventana, musica, cubo
                  y navegar salen del banco (con su motivo escrito, o el
                  trinquete lo para: "un programa que falta se caza")
   las pruebas    las de objetos, monton, tabla, navegar, ventana y musica
   los docs       INTI_MAESTRO (los dos perfiles, el modo Python), el README
                  de INTI ("dos perfiles, un lenguaje"), GRAMATICA de INTI
```

**Y lo que NO se rompe, porque ya esta dicho**: `cpu.inti` y `pulso.inti`
corren en el Ryzen (22-08 y 12-09) y son `llano`. El samurai conserva lo que
ya demostro en el metal.

---

# 3. LO QUE DECIDE EL PROPIETARIO, antes del primer corte

```text
   D1  navegar.inti: se APARCA (queda en el arbol, fuera del build y del
       metro, hasta portarla a TITAN++) o se BORRA
   D2  el decimal exacto: se queda en INTI (CPU) o se va a TITAN++ (dec)
   D3  los ficheros, bico y png: se quedan (seguridad de sistema) o salen
   D4  como se corta: CONGELAR primero (un `perfil pleno` da un NO con su
       motivo, y el codigo sigue ahi) y borrar despues; o borrar de una vez
```

La recomendacion era **congelar primero** (D4).

## 3.1 Lo que DECIDIO el propietario (2026-10-04)

> *"con INTI en navegar le trasladamos a TITAN++, que sepa manejar eso ...
> INTI solo ira UNICAMENTE en CPU puro, para x86-64 preciso ... el decimal
> exacto es para TITAN++ ... los ficheros, bico y png se quedan, y perfil
> pleno se va por TITAN++"*

```text
   D1  navegar se PORTA a TITAN++. Hasta que TITAN++ pueda (tipos, tablas,
       la superficie), navegar.inti se queda donde esta y sigue en el build:
       el escritorio la lanza desde el ANTENISTA (`apps/navegar.ibx`)
   D2  el decimal exacto es de TITAN++ (`dec`, 2b.1). INTI aporta su
       precision sin el: enteros de medida exacta que ATRAPAN al desbordar,
       flotantes IEEE dichos por escrito y ni un comportamiento indefinido.
       El decimal es aritmetica de software para el dinero de las apps
   D3  los ficheros, bico.inti y png.inti SE QUEDAN: leer lo que escribio
       otro sin desbordar un bufer es seguridad de sistema. **REESCRITA el
       10-10:** salen tambien, con su relevo en TITAN++ (1d de EL_FOCO)
   D4  `perfil pleno` se va a TITAN++: CONGELADO hoy (E0077), borrado en el
       escalon 4-5
```

[!] **Por que ventana, musica y cubo no salen del build HOY** (y el escalon 2
se movio detras del 4): las tres son `perfil llano` y **siguen compilando**, y
hoy son las UNICAS pruebas en el metal del audio USB (`musica`), de la
ventana (`ventana`) y de VERRANO (`cubo`). Quitarlas antes de que su runtime
se vaya solo perderia esas pruebas sin ganar nada. Salen el dia del corte 4,
con su relevo en TITAN++.

> **09-10 (LB7 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md)):** el relevo
> del CUBO ya existe en TITAN++, en el anfitrion: `nivel11/cubo_gira` cuenta
> cada fotograma con sus gpu fn y lo publica en la lamina de VERRANO
> (`director.lamina`, `director.publica`); el lector de VERRANO lo lee con los
> bits de `bmo_cubo` (`emisor-x86_64/tests/lamina.rs`). `cubo.inti` sigue en el
> build hasta que la app de TITAN++ se vea en el metal: hoy la prueba de
> VERRANO en el metal es la suya.
>
> **09-10, 18:24: visto en el metal**, y `cubo.inti` SALIO el mismo dia (el
> corte 4a, seccion 3.3): `run titan/cubogira.bex` y `gpu verrano banco inti`
> dieron 360 y 3600 fotogramas por la lamina, 0 rotos, `IGUAL al juez, bit a
> bit` ([`METAL_2026-10-08.md`](../metal/METAL_2026-10-08.md), seccion 4).

## 3.3 Lo que DECIDIO el propietario (2026-10-09): INTI pierde todo lo de app, poco a poco

> *"INTI tiene que perder todo, se degradan poco a poco para que TITAN++
> lleve administracion por completo; el TITAN++ es el que conecta CPU (por
> INTI) y GPU (por VERRANO)"* -- y del corte 4: *"hazlo"*.

Es la seccion 7 de [`TITAN_MAESTRO.md`](../maestro/TITAN_MAESTRO.md) dicha por
el propietario con otras palabras: TITAN++ ADMINISTRA -- las apps, los
juegos, la IA se construyen en el --, y llega a la maquina por dos puertas:

```text
   TITAN++  --(la CPU, lo caliente)-->  INTI      el samurai: registros, AVX2,
                                                  la instruccion exacta; TITAN++
                                                  le llama por `.bo` +
                                                  bmo-enlazar (E2 de 7.3)
            --(la GPU, dibujar)------>  VERRANO   la lamina y la puerta de la
                                                  3060 (E4 de 7.3: HECHO el
                                                  09-10, `director.publica`)
```

**Poco a poco** quiere decir lo que ya decia 3.1: cada pieza de app sale de
INTI el dia que su relevo en TITAN++ existe, y no antes -- quitarla antes
solo pierde la unica prueba del metal que tiene. El orden, con lo que pide
cada relevo:

```text
   pieza de INTI               su prueba hoy        su relevo en TITAN++        estado
   el cubo de VERRANO          cubo.inti            titan/cubogira.bex (LB7)    FUERA, 09-10 (4a)
   (runtime/verrano.inti,
   [verrano] de modulos.toml)
   la ventana (superficie,     sondas/ventana.inti  `director.ventana` y los    FUERA, 10-10 (4c):
   entrada, letra)                                  suyos (F1-F3 de EL_FOCO):   la sonda y NAVEGAR;
                                                    titan/ventana.bex           su runtime, en 4b
   el sonido ([sonido])        ejemplos/musica.inti el sonido en TITAN++        espera su relevo
                                                    (no existe: ni la palabra)
   NAVEGAR (lamina de          apps/navegar.inti    apps/navegar/ en TITAN++    FUERA, 10-10 (4c):
   navegar, texto, monton,                          (F4 de EL_FOCO: los mismos  los mismos pixeles
   objetos)                                         pixeles)
   el perfil `pleno`           congelado (E0077)    los niveles 6-13 de         lo usan las sondas
                                                    TITAN++ (dec, tablas,       del censo: paso 5
                                                    listas, mapas)
   las herramientas de         ejemplos/bico.inti   apps/bico y apps/png en     FUERA, 10-10 (4e):
   imagen (bico, png)          ejemplos/png.inti    TITAN++ (`director.crea`,   los mismos bytes y
                                                    `escribe`, `cierra`)        los ficheros rotos
```

**Lo que NO pierde**, y no es una contradiccion con *"perder todo"*: lo de
CPU es lo que hace de INTI la puerta de TITAN++ a la CPU. `cpu.inti` y
`pulso.inti` (llano, vistos en el Ryzen) y el emisor se quedan. `bico.inti`
y `png.inti` se quedaban por D3 (leer lo que escribio otro sin desbordar un
bufer es seguridad de sistema). **10-10, D3 REESCRITA** (el propietario:
*"se pueden degradar mas? [...] png, bico y otros (no quites el CPU)"*):
salen tambien, con su relevo en TITAN++ -- que juzga cada indice igual que
INTI: la seguridad de D3 viaja con ellas --, y no antes. Ver 1d de
[`EL_FOCO.md`](EL_FOCO.md).

## 3.2 PROPUESTA (04-10): el decimal de Grace Hopper, al estilo del Ryzen

El propietario: *"lo mejor es que lleve el estilo de COBOL que Grace Hopper hizo,
para que la CPU calcule sin problemas en decimal ... la CPU ES ULTRA PRECISA"*.

La idea es buena, con un matiz de SILICIO:

```text
   COBOL en un mainframe   decimal EN HARDWARE (z/Architecture): por eso alli
                           el BCD es rapido
   el Ryzen (x86-64)       NO tiene decimal en hardware; el modo de 64 bits
                           incluso quito DAA/DAS. Un BCD empaquetado aqui es
                           software -- mas lento que un entero
   PUNTO FIJO              un ENTERO con la escala dicha al compilar:
                           `decimal(7,2)` se guarda en centimos, cabe en un
                           registro, se suma con UNA instruccion y es EXACTO.
                           La semantica de COBOL (digitos declarados, nunca se
                           redondea a escondidas) a la velocidad del entero
   float (IEEE)            rapido (SSE/AVX) pero inexacto en decimales
                           (0.1 + 0.2): para la GPU de TITAN++ (f32), no para
                           la precision del samurai
```

**La propuesta**: el decimal de INTI es de PUNTO FIJO, con la escala en el
tipo; el decimal de runtime (coeficiente y escala que cambian, 16 bytes) se fue
a TITAN++ con `pleno` (D2). No choca con D2: son dos cosas distintas. Queda
escrita para que el propietario diga si; no se ha tocado codigo.

---

# 4. LA ESCALERA

```text
   [x] 1  D1-D4 escritas por el propietario en este fichero (3.1, 04-10)
   [x] 3  `perfil pleno` da un NO con codigo y motivo (el CONGELADO de D4)
          HECHO el 04-10: E0077 en `src/perfil`, lo primero que se dice;
          las 13 sondas del censo que necesitaban `pleno` declaran E0077 (y
          dicen lo que esperaban antes), las otras 14 pasaron a `llano` y
          cumplen lo mismo; 243 + 269 pruebas en verde
   [ ] 2  el build y el metro: ventana, musica, cubo y navegar fuera de
          ejemplos.ps1 y del banco del metro, cada uno con su motivo --
          JUNTO con el 4, cuando TITAN++ tenga su relevo (3.1). El cubo,
          HECHO el 09-10 (4a): fuera del build y del metro, con su motivo
          en `metro/src/main.rs`; su relevo (`nivel11/cubo_gira`) entro en
          el banco del metro
   [x] 4c NAVEGAR y la sonda de la VENTANA salen de INTI (10-10, F5 de
          EL_FOCO): `apps/navegar/navegar.inti`, `sondas/ventana.inti` y
          sus pruebas; el build, el metro, la medida y el ANTENISTA dicen
          `apps/navegar.bex` (TITAN++, los mismos pixeles) y
          `titan/ventana.bex`
   [x] 4a VERRANO sale de INTI (09-10, 3.3): `ejemplos/cubo.inti`,
          `runtime/verrano.inti`, `[verrano]` y las constantes `verrano_*`
          de modulos.toml, sus diez filas del espejo del kernel, sus cuatro
          pruebas (`pruebas/simd.rs`) y la dependencia de `bmo-verrano`;
          el DIRECTOR nombra solo `titan/cubogira.bex`. INTI: 243 + 265
          pruebas en verde; el metro, limpio con una fila menos y la del
          relevo
   [x] 4b la superficie, la lamina, la entrada y la letra salen de INTI
          (10-10, el propietario: "quita todo el INTI por completo que INTI
          HACE, ventana..."): `runtime/superficie/`, `lamina/`, `entrada/` y
          `fuente/`, `[entrada]`, `[superficie]`, `[lamina]` y las
          constantes `sup_*`, `evento_*`, `estado_*` y `vista_*` de
          modulos.toml con sus filas del espejo, `tests/fuente.rs`, los
          gemelos de C (`gemelos_inti.rs`) y la cuarta salida de fontgen. Su
          relevo: F1-F3 de EL_FOCO (TITAN++ abre ventanas, lee su buzon y
          escribe la letra)
   [ ] 4  fuera el resto del runtime de app (objetos y monton) -- con su
          relevo (3.3): `png.inti` y `cpu.inti` usan el monton, y se
          quedan mientras sean de CPU
   [ ] 5  el emisor pierde los caminos del monton y de los objetos; los docs
          de INTI dicen UN perfil
   [ ] 6  EL PRIMER CORTE DE VELOCIDAD: asignacion de registros (INTI_MAESTRO
          13.6, mecanismo 1: 2-4x en codigo aritmetico), medida en el metro
   [ ] 7  meter en linea y borrar la comprobacion demostrada (13.6, 2 y 3)
   [ ] 8  el liston: >= 85% de ASM a mano sin SIMD en el banco del metro
```

| escalon | si esta bien | si falla |
|---|---|---|
| 2 | `metro --check` limpio con el banco nuevo; el build despliega cpu, pulso y lo que diga D3 | el metro caza "un programa que falta": se quito sin escribir su motivo |
| 3 | `perfil pleno` da el NO con su codigo y su linea | compila: el perfil no se mira |
| 4 | las pruebas que quedan pasan; `cpu.inti` y `pulso.inti` dan la misma salida (la huella del metro no cambia) | cambia la huella de `pulso`: se corto algo del samurai |
| 6 | `pasos` y `accesos` de `pulso.inti` bajan en el metro, la salida igual | baja `pasos` y cambia la salida: no es una optimizacion |
| 8 | el numero sale del metro, no de la memoria | se dice "ya va a velocidad de ASM" sin una fila que lo muestre |

---

Ver [`TITAN_MAESTRO.md`](../maestro/TITAN_MAESTRO.md) seccion 7 (el reparto),
[`INTI_MAESTRO.md`](../maestro/INTI_MAESTRO.md) 13.6-13.10 (los tres
mecanismos, el trato y por que el ensamblador es control),
[`PLAN_NAVEGAR.md`](PLAN_NAVEGAR.md) (la app que D1 decide) y
[`PLAN_LA_LENGUA_DE_LA_3060.md`](PLAN_LA_LENGUA_DE_LA_3060.md) (la 3060, que es
donde va el centauro).
