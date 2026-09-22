# PLAN EL HUD -- el escritorio como Hyprland, con UN motivo por pieza

> Abierto el **2026-09-22** a peticion del propietario: *"mejorar todo el
> DIRECTOR, el escritorio HUD como Hyprland... elegante... la barra lateral...
> interfaz en tiempo real"*, y la condicion que ordena todo lo de abajo:
> *"TODO MEZCLADO pero UN MOTIVO, UN MOTIVO; practico, simple y adictivo, ULTRA
> SUPER COMODO"*.
>
> Estudio previo: el de ese mismo dia sobre Windows, Mac y Linux desde el
> origen (en el chat; resumen en la memoria del proyecto). De ahi salen la
> tecla Super, Fitts y el borde de foco.

---

# 0. LA REGLA DEL PLAN: una pieza, un motivo

Cada pieza existe por UNA razon que se dice en una linea. Si una pieza no
cabe en una linea, no es una pieza: son dos, o es decoracion.

```text
   pieza                    el motivo
   H1  la tecla Super       el gestor tiene tecla PROPIA y no le quita ninguna a nadie
   H2  borde + huecos       ver de un vistazo a DONDE van las teclas
   H3  la barra lateral     lo que hace la maquina, SIN abrir nada
   H4  el mosaico           ninguna ventana TAPA a otra, y no se ordena a mano
```

**Lo que NO entra, y por que** (el precio de lo que Hyprland hace con GPU):

| no | por que |
|---|---|
| desenfoque, transparencia | el alfa aqui es de 1 bit y no hay GPU; el desenfoque de un degradado es el degradado |
| animaciones | cada fotograma animado es latencia; aqui se pinta y se vuelca en la misma vuelta |
| escritorios numerados 1..5 | ya rechazado con motivo en `scene/barra.rs`: cinco numeros que no hacen nada |

---

# 1. LAS PIEZAS

## [ ] H1 -- LA TECLA SUPER (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** el gestor tiene tecla propia y no le quita ninguna a nadie.

La tecla Windows llegaba al escritorio desde el 01-09 (`MOD_GUI`, `badfb3b9`)
y nadie la usaba. Todo Alt era del escritorio, y eso tenia un precio escrito:
a DOOM no le llegaba el ladeo (Alt+flechas).

```text
   Super + flechas          encajar: mitad izquierda / derecha, arriba maximiza,
                            abajo deshace (Windows 7, Win+flechas)
   Super + Shift + flechas  mover 24 px
   Super + Q                cerrar lo de delante (la X y Alt+F4: UN cierre)
   Super + Enter            Ejecutar, delante y con el teclado
   Super + F                pantalla completa (lo mismo que Alt+Enter)
   Super + M                el modo del foco (antes Alt+M)
   Alt                      es de la APP, menos Alt+Tab, Alt+F4 y Alt+Enter
```

Con Super pulsado nada se escribe: un Super+X sin atajo se tira, para que el
dia que lo tenga no haya escrito una `x` antes.

| que | afirma | como se cae |
|---|---|---|
| Super+flechas con Datos delante | encaja a una mitad, con huecos | no se mueve: el kernel no marca `MOD_GUI` en esa tecla |
| DOOM en ventana, Alt+flechas | DOOM ladea | se mueve la ventana: la regla de `keys/app.rs` no cambio |
| Super+Q con una app delante | se cierra | no hace nada: la `q` no llego cocida con Super |

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
| Super+izquierda y Super+derecha en dos ventanas | 8 px entre ellas y con los bordes | pegadas: `snap` no mide en `area_util` |

## [ ] H3 -- LA BARRA LATERAL EN VIVO (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ver lo que hace la maquina, sin abrir nada.

Al escribirla el motivo se afino: "lo abierto" ya lo dicen las fichas de
arriba, y ponerlo otra vez aqui era dos sitios para lo mismo. La barra es el
HUD EN TIEMPO REAL: una columna de 112 px con cinco instrumentos --cpu,
memoria, vatios, pulso (vueltas del escritorio) y sonido (el medidor del
maestro)--, cada uno con su cifra y su grafica de los ultimos 11 s (44
muestras a 4 por segundo). Super+B la esconde.

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
| Super+B | se va, y la rejilla y las ventanas ocupan su sitio | queda un trozo pintado: `repintar_escritorio` no la borra |

## [ ] H4 -- EL MOSAICO (2026-09-22)

> Codigo hecho; se cierra cuando el metal diga la tabla.

**Motivo:** ninguna ventana tapa a otra, y no se ordena a mano.

Super+T lo enciende y lo apaga (y lo dice en la linea de estado). Al
escribirlo se eligio MAESTRO Y PILA (dwm, el `master` de Hyprland) y no
`dwindle`: es el que se predice sin mirar. Una ventana: el area util entera;
dos o mas: la primera a la izquierda, las demas apiladas a la derecha, con
huecos. La primera es la app si hay una, si no Ejecutar. Solo recoloca
cuando CAMBIA que ventanas hay (`desktop/mosaico.rs`), asi que arrastrar una
no se pelea con la mano.

| que | afirma | como se cae |
|---|---|---|
| Super+T con Ejecutar y Datos abiertas | Ejecutar a la izquierda, Datos a la derecha, sin taparse | nada se mueve: `seguir` no corre o la firma no cambia |
| abrir CABINA (F11) con el mosaico puesto | la pila de la derecha se parte en dos | se abre encima: la firma no ve la ventana nueva |
| con DOOM en ventana | DOOM a la izquierda, lo demas apilado | DOOM no se mueve: su marco no se coloca |
| lo que se dice | una ventana con minimo mayor que su hueco (Ejecutar) asoma: es a proposito | -- |

---

Ver [`PLAN_DIRECTOR.md`](PLAN_DIRECTOR.md) (el compositor) y
[`PLAN_EL_PIXEL.md`](PLAN_EL_PIXEL.md) (por que no hay animaciones).
