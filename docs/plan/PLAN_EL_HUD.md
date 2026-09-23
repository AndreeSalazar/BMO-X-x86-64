# PLAN EL HUD -- el escritorio como Hyprland, con UN motivo por pieza

> Abierto el **2026-09-22** a peticion del propietario: *"mejorar todo el
> DIRECTOR, el escritorio HUD como Hyprland... elegante... la barra lateral...
> interfaz en tiempo real"*, y la condicion que ordena todo lo de abajo:
> *"TODO MEZCLADO pero UN MOTIVO, UN MOTIVO; practico, simple y adictivo, ULTRA
> SUPER COMODO"*.
>
> Estudio previo: el de ese mismo dia sobre Windows, Mac y Linux desde el
> origen (en el chat; resumen en la memoria del proyecto). De ahi salen la
> tecla del gestor (Super, y luego Ctrl), Fitts y el borde de foco.

---

# 0. LA REGLA DEL PLAN: una pieza, un motivo

Cada pieza existe por UNA razon que se dice en una linea. Si una pieza no
cabe en una linea, no es una pieza: son dos, o es decoracion.

```text
   pieza                    el motivo
   H1  la tecla Ctrl        el gestor tiene tecla PROPIA y no le quita ninguna a nadie
   H2  borde + huecos       ver de un vistazo a DONDE van las teclas
   H3  la barra lateral     lo que hace la maquina, SIN abrir nada
   H4  el mosaico           ninguna ventana TAPA a otra, y no se ordena a mano
   H5  el panel             todo lo que no es una ventana, en UN solo sitio
```

**Lo que NO entra, y por que** (el precio de lo que Hyprland hace con GPU):

| no | por que |
|---|---|
| desenfoque, transparencia | el alfa aqui es de 1 bit y no hay GPU; el desenfoque de un degradado es el degradado |
| animaciones | cada fotograma animado es latencia; aqui se pinta y se vuelca en la misma vuelta |
| escritorios numerados 1..5 | ya rechazado con motivo en `scene/barra.rs`: cinco numeros que no hacen nada |

---

# 1. LAS PIEZAS

## [ ] H1 -- LA TECLA DEL GESTOR: CTRL (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** el gestor tiene tecla propia y no le quita ninguna a nadie.

Fue **Super** una tarde (`e866a4a3`). Tras el metal de las 16:56 el
propietario la cambio: *"reemplaza con control + (cualquier atajo) porque
BMO-X va a vivir como el estilo de Windows"*. Ctrl ya era del escritorio
(`keys/app.rs::del_escritorio` no se lo da a ninguna app), asi que el motivo
sigue en pie: no se le quita una tecla a nadie. Todo Alt era del escritorio, y
eso tenia un precio escrito: a DOOM no le llegaba el ladeo (Alt+flechas).

```text
   Ctrl + flechas           encajar: mitad izquierda / derecha, arriba maximiza,
                            abajo deshace (el Win+flechas de Windows 7)
   Ctrl + Shift + flechas   mover 24 px
   Ctrl + Q                 cerrar lo de delante (la X y Alt+F4: UN cierre)
   Ctrl + Enter             Ejecutar, delante y con el teclado
   Ctrl + F                 pantalla completa (lo mismo que Alt+Enter)
   Ctrl + Tab               el modo del foco (Alt+Tab elige ventana)
   Ctrl + B / Ctrl + T      barra lateral / mosaico
   Alt                      es de la APP, menos Alt+Tab, Alt+F4 y Alt+Enter
```

**Los choques, y como se resolvieron** (el kernel cuece Ctrl+letra en su
codigo de control, `keyboard::feed_full`, asi que se compara con el codigo):

| choque | que se hizo |
|---|---|
| Ctrl+Alt ES AltGr (`@ # [ ]`) | los atajos exigen Ctrl SIN Alt; el toque de Ctrl+Alt sigue escondiendo Ejecutar |
| Ctrl+M es el byte de Enter, Ctrl+I el de Tab | el modo del foco va con Ctrl+Tab, no con M |
| Ctrl+W ya borra una palabra en Ejecutar | cerrar es Ctrl+Q |
| Ctrl+C frena la corrida / limpia la linea | se queda; COPIAR pasa a Ctrl+Shift+C (el de Windows Terminal) |
| Ctrl+arriba/abajo copiaban y pegaban | ahora encajan; pegar sigue en Ctrl+V |

Con Ctrl (sin Alt) un caracter imprimible no se escribe: un Ctrl+1 sin atajo
se tira, para que el dia que lo tenga no haya escrito un `1` antes. Super
vuelve a ser de la app.

| que | afirma | como se cae |
|---|---|---|
| Ctrl+flechas con Datos delante | encaja a una mitad, con huecos | no se mueve: la flecha no llega con `MOD_CTRL` |
| DOOM en ventana, Alt+flechas | DOOM ladea | se mueve la ventana: la regla de `keys/app.rs` no cambio |
| Ctrl+Q con una app delante | se cierra | no hace nada: no llego el 0x11 |
| escribir `@` con AltGr y con Ctrl+Alt | sale la arroba | salta un atajo: la guarda de Alt no esta |
| Ctrl+Shift+C y luego Ctrl+V en Ejecutar | la linea se duplica | limpia la linea: el Shift no se mira |

## [ ] H2 -- EL BORDE DE FOCO Y LOS HUECOS (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ver de un vistazo a donde van las teclas.

* El marco de la ventana con foco lleva el borde del ACENTO; las demas, el
  color de su ventana (que dice CUAL es). El `col.active_border` de Hyprland.
  Lo pone UN sitio, `desktop/foco.rs`, cuando el foco cambia, y repinta las del
  sistema de la mas vieja a la de delante.
* HUECOS de 8 px (`scene/chrome.rs::HUECO`): encajar y maximizar miden en
  `area_util`, la pantalla menos la barra y menos los huecos. Una ventana
  pegada a otra se lee como una; con aire, dos.
* Y como se pinta cada ventana vive en UN sitio (`paint::pintar_ventana`): era
  un cierre dentro de `keys::edges`, y el borde era la segunda copia.

| que | afirma | como se cae |
|---|---|---|
| clic en CABINA con Datos abierta | CABINA con borde azul, Datos con el suyo | los dos azules: `seguir` no corre |
| Ctrl+izquierda y Ctrl+derecha en dos ventanas | 8 px entre ellas y con los bordes | pegadas: `snap` no mide en `area_util` |

## [ ] H3 -- LA BARRA LATERAL EN VIVO (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ver lo que hace la maquina, sin abrir nada.

> Esa misma tarde crecio: la barra de arriba se fundio en ella (H5), y las
> fichas, la luz y el reloj viven ahora aqui. Lo de abajo es como nacio.

Al escribirla el motivo se afino: "lo abierto" ya lo dicen las fichas de
arriba, y ponerlo otra vez aqui era dos sitios para lo mismo. La barra es el
HUD EN TIEMPO REAL: una columna de 112 px con cinco instrumentos --cpu,
memoria, vatios, pulso (vueltas del escritorio) y sonido (el medidor del
maestro)--, cada uno con su cifra y su grafica de los ultimos 11 s (44
muestras a 4 por segundo). Ctrl+B la esconde.

* `scene/lateral.rs`: su caja, su color para `scene_color`, la historia y el
  pintado. Las mismas cuentas que la barra de arriba.
* La columna es RESERVADA (la `exclusive zone` de Hyprland): `area_util`, los
  topes del arrastre, de las flechas y de `fit`, y la rejilla de iconos leen
  `lateral::margen()`. Por eso su repintado de 4 Hz nunca pinta encima de una
  ventana.

| que | afirma | como se cae |
|---|---|---|
| arrancar el escritorio | la columna a la izquierda, con las cinco graficas moviendose | vacia: `latido` no corre o `will_paint` no llega |
| DOOM sonando | la grafica de sonido sube en verde/ambar | quieta: el medidor del maestro no se lee |
| arrastrar una ventana a la izquierda | se para en el borde de la columna | la tapa: un tope no lee `margen()` |
| Ctrl+B | se va, y la rejilla y las ventanas ocupan su sitio | queda un trozo pintado: `repintar_escritorio` no la borra |

## [ ] H4 -- EL MOSAICO (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ninguna ventana tapa a otra, y no se ordena a mano.

Ctrl+T lo enciende y lo apaga (y lo dice en la linea de estado). Al
escribirlo se eligio MAESTRO Y PILA (dwm, el `master` de Hyprland) y no
`dwindle`: es el que se predice sin mirar. Una ventana: el area util entera;
dos o mas: la primera a la izquierda, las demas apiladas a la derecha, con
huecos. La primera es la app si hay una, si no Ejecutar. Solo recoloca
cuando CAMBIA que ventanas hay (`desktop/mosaico.rs`), asi que arrastrar una
no se pelea con la mano.

| que | afirma | como se cae |
|---|---|---|
| Ctrl+T con Ejecutar y Datos abiertas | Ejecutar a la izquierda, Datos a la derecha, sin taparse | nada se mueve: `seguir` no corre o la firma no cambia |
| abrir CABINA (F11) con el mosaico puesto | la pila de la derecha se parte en dos | se abre encima: la firma no ve la ventana nueva |
| con DOOM en ventana | DOOM a la izquierda, lo demas apilado | DOOM no se mueve: su marco no se coloca |
| lo que se dice | una ventana con minimo mayor que su hueco (Ejecutar) asoma: es a proposito | -- |

## [ ] H5 -- EL PANEL: LA BARRA DE ARRIBA SE FUNDE EN LA LATERAL (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** todo lo que no es una ventana, en un solo sitio.

El propietario, tras el metal de las 16:56: *"la barra de arriba vamos a
quitar y mejorar eso, para que sea mucho mas elegante... ya cumplio su
parte"*. De tres formas propuestas (fundir en la lateral, isla centrada, tira
de 28 px) eligio **fundir**. El motivo de fondo: eran dos barras para lo
mismo --cpu, memoria y vatios salian en las dos-- y la de arriba se habia
llenado de instrumentos de los dias de cazar averias (~1.100 px de numeros).

```text
   arriba del panel   la marca BMO-X
                      las fichas, en vertical (Ejecutar, ESTRATOS, CABINA, apps)
                      (aire: las fichas crecen hacia abajo)
   abajo del panel    la luz del bus (el testigo)
                      cpu / memoria / vatios / pulso CON SU AGUJA / sonido
                      el reloj y el dia; el vol (clic = el maestro)
   a CABINA           el reparto del pulso, el volcado y la entrada, en una
                      linea encima del pie, 4 Hz y SOLO con CABINA delante
```

* `scene/barra.rs` se borro; `TASKBAR_H` tambien. Las ventanas ganan 40 px de
  alto: `area_util`, los topes y el centrado de una ventana nueva miden desde
  arriba y a la DERECHA del panel. La rejilla de iconos empieza en `y = 24`.
* El panel mide 160 px (la grafica pasa a 68 muestras, 17 s) y sigue siendo
  columna RESERVADA. Las opciones de la barra en `sys/director.cfg` son ahora
  las suyas: `barra_flotante`, `barra_hueco`, y `cpu`/`memoria`/`vatios`/`reloj`
  encienden cada instrumento (el pulso y el sonido no se apagan).
* **Escondido (Ctrl+B) queda una TIRA de 6 px con la luz del bus, y un clic
  lo trae.** La ficha de CABINA estaba siempre en la barra porque *"un panel de
  diagnostico al que solo se llega con el aparato que puede estar roto no es un
  panel de diagnostico"*: esconder el panel no puede dejar al raton sin camino.
* Cambiar el panel en el editor de aspecto recoloca las ventanas por el mismo
  camino que Ctrl+B (`desktop::lateral_cambio`).
* De paso: el numero del testigo (`REPARADO x3`) se ponia en `tx + ancho`, y
  `texto` devuelve donde ACABA: caia el doble de lejos.

| que | afirma | como se cae |
|---|---|---|
| arrancar el escritorio | sin barra arriba; el panel con la marca, Ejecutar y CABINA, la luz verde, las graficas, la hora y el vol | una tira oscura arriba: algo sigue pintando `TASKBAR` |
| abrir DOOM y minimizarlo | su ficha aparece en el panel, apagada; un clic lo trae | no vuelve: `ficha_en` no casa con la fila pintada |
| clic en el vol | el maestro se abre al lado del panel, con el pie a la altura del vol | se abre arriba a la derecha: `junto_a_la_barra` no se cambio |
| Ctrl+B y clic en la tira | se va dejando la luz; el clic lo trae y las ventanas se corren | la tira no contesta: `en_la_tira` no se mira |
| CABINA delante | una linea `latido .../s pinta .. cuerpo .. puerta ..  volcado ..  entrada ..` encima del pie | vacia: `instrumentos` no corre o no cabe en el ancho |
| la aguja del pulso | gira cuatro veces por segundo | quieta: `dictamen` no se llama o no gira |

---

Ver [`PLAN_DIRECTOR.md`](PLAN_DIRECTOR.md) (el compositor) y
[`PLAN_EL_PIXEL.md`](PLAN_EL_PIXEL.md) (por que no hay animaciones).
