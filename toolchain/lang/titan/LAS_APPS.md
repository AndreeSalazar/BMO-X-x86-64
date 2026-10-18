# LAS APPS DEL PROPIETARIO -- lo que cada una le pide a TITAN++

18-10-2026. El propietario: *"poner en todo la lista de las APP que son mis
proyectos, HASTA ILLAPA, que pide, para poder construir un HOUDINI PERSONAL"*.
Leido de los planes de `docs/plan/` y del arbol, sin adivinar; cada fila nombra
su plan. Lo que TITAN++ YA da se marca HECHO; lo que falta dice de quien es:
del lenguaje, del kernel, del METAL (el Ryzen) o una DECISION del propietario.

## Lo que TITAN++ ya da (y sirve a todas)

| Pieza | Desde | Para que |
|---|---|---|
| ventana, teclado, raton, letra (`use director`) | F1-F3 de EL_FOCO, 10-10 | toda app con cara |
| leer y guardar ficheros (`director.fichero`, guardar) | TA4 de LA_TINTA | obras, mundos, notas |
| `byte`, listas y mapas que crecen, `lamina(n)` | niveles 13-15 | pixeles, mallas, datos grandes |
| `gpu fn` con bucles, que dibuja, la CPU de reserva | LB4-LB7 de LAS_LIBRERIAS | computo y dibujo por VERRANO |
| composicion, `trait`, prestamo de componentes (O1) | nivel 10, OBJETOS | piezas reutilizables |
| **`div` / `rem`** de Euclides | 18-10 | rejillas, coordenadas negativas, angulos |
| **la casa**: `use casa.medida`, `cifras`, `colores`, `letrero` | 18-10 | dejar de copiar entre apps |
| **cuerpos por defecto en un trait** (O3) | 18-10 | el comportamiento comun, una vez |
| **listas y mapas de un trait** (O4) | 18-10 | redes de nodos, escenas, widgets: tipos distintos en una lista |
| **`gpu fn` con VECINAS** (LB9): `campo: [f32]` entera, `celda()`, fuera 0 | 18-10 | filtros, rellenos, densidades: un hilo que mira a los de al lado |
| **`piso(x)`** y la REJILLA de vecinos (IL2) | 18-10 | ordenar por celda, buscar, mirar solo lo cercano |
| **las reducciones**: `use casa.suma` (total, punto, mayor, menor) | 18-10 | totales, productos escalares, el gradiente conjugado: la CPU no suma f32 |

## La lista

| App | Que es | Hoy | Plan | Lo que le pide a TITAN++ |
|---|---|---|---|---|
| **MUNDO** | el Minecraft propio | TITAN++, jugable | `PLAN_MUNDO` | vertices en listas del monton (MC4 (a), HECHO: de 80 a 126 caras); subir los 768 vertices del DIRECTOR; fisica con `div` (HECHO), resaltado y teclas mantenidas (app) |
| **ECCE** | escribir: guiones al nivel de Final Draft | TITAN++, 7 modulos | `PLAN_ECCE` | trabajar con texto por trozos (hoy todo es `[byte]` a mano); la casa para sus piezas comunes |
| **LA TINTA** | Adobe + Clip Studio, libros y manga | TITAN++, lienzo minimo | `PLAN_LA_TINTA` | Wacom con presion (driver, METAL), capas y teselas en ESTRATOS, `gpu fn` con VECINOS (**LB9**, HECHO 18-10) para los filtros |
| **CRAFTPHO** | pintar y retocar con capas, en la 3060 | plan, 0 de 14 | `PLAN_CRAFTPHO` | los 12 modos de mezcla como `match` en `gpu fn` (ya cabe); mosaicos 64 x 64; rellenar y varita necesitan VECINOS (**LB9**, HECHO 18-10) |
| **RESOLUCION** | elegir la medida de las ventanas | TITAN++ | EL_FOCO R1 | nada nuevo: ya usa la casa |
| **NAVEGAR** | la app que navega sin ser navegador | TITAN++ | `PLAN_NAVEGAR` | leer formatos de texto (HTML/JSON) sin escribirlo byte a byte: **una casa de texto** |
| **PNG, BICO, MUSICA** | herramientas y la musica | TITAN++ | -- | migrar su `div` a mano (pendiente) |
| **TALLER (F1)** | los modulos como nodos y cables | Rust (Ring 3) | `PLAN_TALLER` | ya dibuja `use casa.X`, y su lectura rapida no se rompe con los traits con cuerpo; que ILLAPA reuse sus nodos (HO1) |
| **LOS DATOS** | ESTRATOS como base de datos | plan | `PLAN_LOS_DATOS` | **listas por comprension** (`[e for e in l if ...]`, Q1): el SELECT de la casa, sin palabras nuevas |
| **HERMES** | dos BMO-X que se hablan, sin servidor | Rust | `PLAN_HERMES` | hoy en Rust; pasarla a TITAN++ pediria la red desde el lenguaje (un nodo como `director`) |
| **BANK CAT** | la cartera: la cara en Rust, el dinero en COBOL | Rust + COBOL | `PLAN_BANK_CAT` | nada: el dinero se queda en COBOL por decision |
| **MEDIOS / EL VIDEO** | VLC como objetivo, video por la 3060 | plan | `PLAN_MEDIOS`, `PLAN_EL_VIDEO` | V3 de EL_VIDEO: el reproductor en TITAN++ (cabeceras y muestras); los decodificadores en Rust |
| **LA LUDOTECA** | los juegos que compraste | Rust | `PLAN_LA_LUDOTECA` | nada: es Rust por decision |
| **BMO ATENTO** | el sistema que se adelanta | plan | `PLAN_BMO_ATENTO` | "el cerebro es de TITAN++": cuentas y decisiones, cabe en lo de hoy |
| **ILLAPA** | motor grafico + Houdini propio | plan, IL0 sin contestar | `PLAN_ILLAPA` | la seccion de abajo |

## ILLAPA: lo que pide para un Houdini personal

ILLAPA es UN programa con dos modos: PAUSA (el taller de efectos, lo de
Houdini) y PLAY (el juego a 60 fps). Su corazon es UN solucionador de
particulas (XPBD / PBF: sangre, tela, carne, escombros, todo chocando en el
mismo paso), y cada paso es "haz esto con cada particula": una `gpu fn`.
El VEX de Houdini es eso mismo.

### Lo que pide del LENGUAJE, en orden de bloqueo

| # | Pieza | Por que ILLAPA no anda sin ella | De quien | Estado |
|---|---|---|---|---|
| 1 | **VECINOS en una `gpu fn`**: leer celdas de OTRA tabla por indice | una gota mira a las de al lado (IL1, IL2: la rejilla de vecinos) | lenguaje + un asa del kernel (**LB9**) | **HECHO 18-10** en el anfitrion: el asa es la del UAV, que R7 ya acotaba; la primera densidad en el banco |
| 2 | **la puerta de computo de una app** | que la 3060 corra el solucionador de verdad | kernel + METAL (**LB8**) | abierto |
| 3 | **reducciones** en `gpu fn` (sumar por bloques) | gradiente conjugado de la carne (HO4), densidades | lenguaje | **HECHO 18-10** como casa (`use casa.suma`: total, punto, mayor, menor), sin palabras nuevas; sin memoria compartida, un paso por bloque de 64 |
| 4 | **listas de un trait** (O4): `[Nodo]` con nodos distintos | la red de HO1 es crear, transformar, copiar, unir: nodos de tipos distintos recorridos en orden | lenguaje | **HECHO 18-10**, con la primera red de HO1 en el banco |
| 5 | **tablas grandes fuera de la pila** | millones de particulas; hoy una tabla `f32` vive en la pila de 64 KiB | emisor E1 (**MC4** de MUNDO) | **HECHO 18-10**: gpu fn y `publica` sobre listas `[f32]` del monton; MUNDO ya las usa |
| 6 | **`f32` en la CPU** | rotaciones, cuaterniones, IK y curvas de animacion (IL16) | **DECISION D5** del propietario, una ley con `--sellar` | sin contestar |
| 7 | **leer glTF** (y despues escribirlo) | el personaje de Blender (IL5, IL20) | una casa de texto/JSON + binario | abierto |
| 8 | **llamada con punto** (`p.mueve(dt)`) | comodidad al escribir cientos de nodos | lenguaje | propuesta |

Lo que NO pide del lenguaje, porque ya esta: la `gpu fn` con bucles y la que
dibuja, `f32` dentro de la 3060, la CPU como juez bit a bit, los trait con
cuerpos por defecto (la base de un nodo "que sabe hacer" algo) y la casa.

### Lo que pide de VERRANO y del sistema (no del lenguaje)

- profundidad, constantes, texturas, mezcla alfa (V2, V3, M3 de `PLAN_VERRANO`)
- pantallas HDR y varias a la vez (G-buffer), mipmaps y mapas de cubo (LZ1-LZ9)
- la escena como nodo de ESTRATOS con versiones y ramas (IL9)
- G0: que la 3060 arranque fiable (el 0x15 sigue abierto)

### Las decisiones que esperan al propietario (IL0, seccion 8 de ILLAPA)

```text
   D2  donde vive ILLAPA: una tecla nueva, o un modo de F1
   D3  la escena del primer CGI (la sangre que choca con un personaje)
   D4  traer los personajes de Blender como glTF
   D5  el f32 en la CPU, o toda la animacion en gpu fn
```

### Un orden que acerca ILLAPA sin esperar al metal

1. **O4, listas de un trait** -- HECHO el 18-10, con HO1 como primer uso:
   [red_de_nodos](ejemplos/nivel13/red_de_nodos.titan) crea, mueve, escala y
   copia 48 puntos; rehecha por partes da la misma huella. Sin la 3060.
2. **MC4, tablas fuera de la pila** -- HECHO el 18-10 (camino a): listas
   `[f32]` en las gpu fn y en `publica`; MUNDO ya dibuja asi.
3. **Una casa de texto** (`casa.texto`: partir, buscar, numeros escritos),
   que piden NAVEGAR, ECCE y el lector de glTF.
4. **LB9, vecinos** -- HECHO el 18-10, antes que la casa de texto (el
   propietario: *"dale con ILLAPA y los vecinos"*): el paso que convierte la
   `gpu fn` en un solucionador.
   [densidad](ejemplos/nivel11/densidad/src/main.titan) suma las vecinas de
   cada gota.
   **IL2, la rejilla** -- HECHO el 18-10:
   [rejilla](ejemplos/nivel11/rejilla/src/main.titan) ordena las gotas por
   celda (un orden bitonico) y cada una mira solo sus nueve celdas. Lo que
   sigue es IL3, la sangre (PBF) -- HECHA el 18-10 en 2D:
   [sangre](ejemplos/nivel11/sangre/src/main.titan), una presa que cae y
   conserva su densidad.
   **Las reducciones** (pieza 3) -- HECHAS el 18-10 como casa:
   [sumas](ejemplos/nivel11/sumas/src/main.titan), y la rejilla y la
   densidad ya suman sus totales en la GPU.
5. Con LB8 y G0 en el metal: IL3, la primera sangre en la 3060.

[!] Esta lista es el estudio y el orden propuesto: no cambia ninguna casilla
de los planes que nombra. Cada peldanio se cierra en su plan, con su prueba.
