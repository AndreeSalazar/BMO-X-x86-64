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

La recomendacion es **congelar primero** (D4): un `perfil pleno` que diga *"INTI
ya no hace apps: eso es TITAN++"* con su codigo de error es un corte limpio que
se ve, y el borrado viene despues, cuando nada lo use.

---

# 4. LA ESCALERA

```text
   [ ] 1  D1-D4 escritas por el propietario en este fichero
   [ ] 2  el build y el metro: ventana, musica, cubo y navegar fuera de
          ejemplos.ps1 y del banco del metro, cada uno con su motivo
   [ ] 3  `perfil pleno` da un NO con codigo y motivo (el CONGELADO de D4)
   [ ] 4  fuera el runtime de app (objetos, monton, superficie, lamina,
          entrada, letra, verrano) y sus secciones de modulos.toml, con sus
          pruebas (la de `tests/fuente.rs` incluida: bmo-fontgen deja de
          emitir la copia INTI)
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
