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
   H3  la barra lateral     lo abierto y lo que hace la maquina, SIN abrir nada
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

## [ ] H3 -- LA BARRA LATERAL EN VIVO

**Motivo:** lo abierto y lo que hace la maquina, sin abrir nada.

Una columna a la izquierda. Arriba, las apps (se lanzan con un clic); en
medio, las ventanas abiertas (la del foco resaltada); abajo, lo que la maquina
hace AHORA: CPU, memoria, vatios, el medidor del sonido. Super+B la esconde.
`area_util` le deja su columna: encajar, maximizar y el mosaico no la pisan.

## [ ] H4 -- EL MOSAICO

**Motivo:** ninguna ventana tapa a otra, y no se ordena a mano.

Super+T lo enciende y lo apaga. Con el mosaico puesto, las ventanas abiertas
se reparten `area_util` como en el `dwindle` de Hyprland: la primera entera;
con dos, mitades; cada nueva parte en dos la ultima. Con huecos.

---

Ver [`PLAN_DIRECTOR.md`](PLAN_DIRECTOR.md) (el compositor) y
[`PLAN_EL_PIXEL.md`](PLAN_EL_PIXEL.md) (por que no hay animaciones).
