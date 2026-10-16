# PLAN ECCE -- escribir en BMO-X: guiones al nivel de Final Draft, y todo lo demas

> El propietario (15-10): *"puedes anotar el "Final Draft" como inspiracion
> TOTAL + Window + TODO los elementos que sean asi para escribir? [...] ese
> mismo se convierte un GENERALISTA TOTAL para construir y escribir guiones
> profesionales pero al nivel REAL del Final Draft"*.
>
> **Es un plan LARGO y no es para ahora**, como
> [`PLAN_CRAFTPHO.md`](PLAN_CRAFTPHO.md) y [`PLAN_ILLAPA.md`](PLAN_ILLAPA.md):
> se empieza el dia que el propietario lo diga.
>
> **El nombre (15-10, D1 contestada):** ECCE, de *Ecce Homo* ("he aqui el
> hombre"), el libro en que Nietzsche se escribe a si mismo. El propietario
> queria algo como Zaratustra pero mas corto: un bloc personal que, como
> aquel libro, sirve para escribir quien es uno. Se llamo `LA PLUMA` mientras
> no tenia nombre.

---

## 0. La respuesta corta

```text
   que es        la app de ESCRIBIR de BMO-X: ECCE. Un solo motor de texto con
                 MODOS: guion de cine y de serie (Final Draft), documento
                 (Word), novela por capitulos y escenas (Scrivener), obra de
                 teatro, comic y notas
   el corazon    el texto es una lista de PARRAFOS con un ELEMENTO cada uno
                 (encabezado de escena, accion, personaje, dialogo...); el
                 modo dice que elementos hay, como se ve cada uno y que
                 tecla lleva al siguiente. Final Draft es eso mismo
   quien hace    TITAN++ administra la app (la ventana, las teclas, los
   que           paneles); el calculo de paginas, el corrector y los
                 formatos de fichero son codigo puro, probado en el
                 anfitrion; el dibujo del texto va por el director (y la
                 3060 cuando C3 este en el Ryzen)
   la regla      el PAGINADO es el juez: la misma escena da las MISMAS
                 paginas aqui que en Final Draft (una pagina ~ un minuto
                 de pantalla). Si eso falla, no es un guion profesional
```

---

## 1. Lo que hace a Final Draft ser Final Draft

Escrito de memoria el 15-10: **se comprueba contra la fuente** (la ayuda de
Final Draft y la especificacion de Fountain) el dia que se escriba la
primera linea.

```text
   LOS ELEMENTOS         encabezado de escena (INT./EXT., lugar, momento),
                         accion, personaje, parentesis, dialogo, transicion
                         (CORTE A:), plano y general. Cada uno con su sangria
                         y su ancho fijos
   TAB Y ENTER           el flujo: tras el personaje, Enter da dialogo; tras
                         el dialogo, Enter da accion y Tab da personaje. Se
                         escribe sin tocar el raton
   SMARTTYPE             completa nombres de personajes, lugares y momentos
                         (DIA, NOCHE) con lo ya escrito en el guion
   LA PAGINA             Courier de 12 puntos, margenes y sangrias del oficio;
                         el (MAS) y el (CONT.) cuando un dialogo cruza de
                         pagina; no dejar un encabezado solo al pie
   PRODUCCION            numerar escenas; BLOQUEAR paginas (las A: 12, 12A,
                         13); REVISIONES por color (blanca, azul, rosa,
                         amarilla, verde, dorada...) con la marca * al margen
   ESTRUCTURA            tablero de beats, mapa de la historia, esquema y
                         fichas de escena (lo de Scrivener tambien)
   INFORMES              escenas, personajes (cuanto habla cada uno),
                         lugares; el desglose de produccion (etiquetas de
                         utileria, vestuario, extras)
   DIALOGO DOBLE         dos personajes hablando a la vez, en dos columnas
   PLANTILLAS            cine, serie de una hora, comedia de media hora,
                         teatro, comic
   FORMATOS              .fdx (XML, el suyo), PDF para entregar, y Fountain
```

**Fountain** (2012, John August y Stu Maschwitz) es la llave: un guion
escrito en TEXTO PLANO con reglas simples (`INT. CASA - NOCHE` es un
encabezado, un nombre en mayusculas solo en su linea es un personaje). Es
abierto, sin duenio de licencia, cabe en `git` y Final Draft lo abre y lo
guarda. **El formato propio de ECCE es Fountain**: no se inventa uno.

---

## 2. El GENERALISTA: los modos sobre el mismo motor

```text
   MODO          INSPIRADO EN       SUS ELEMENTOS
   guion         Final Draft        los de la seccion 1
   serie         Final Draft        + acto, teaser, etiqueta de cierre
   teatro        Final Draft        personaje centrado, acotacion, acto/escena
   comic         Final Draft        pagina, vineta, globo, cartela, onomatopeya
   documento     Word               titulo 1..3, parrafo, lista, cita, tabla,
                                    nota al pie, imagen
   novela        Scrivener          libro > parte > capitulo > escena, cada
                                    una con su ficha y su estado
   notas         (el bloc)          texto libre, con enlaces entre notas
```

Lo que comparten todos: deshacer sin limite, buscar y reemplazar, el
corrector (castellano e ingles), contar palabras y paginas, la meta del dia,
el modo sin distracciones (solo el texto, pantalla completa) y las versiones
guardadas como ESTRATOS (como la `.obra` de
[`PLAN_LA_TINTA.md`](PLAN_LA_TINTA.md)).

---

## 3. Lo que falta abajo (y de quien es)

```text
   UNA LETRA MONOESPACIO      el guion pide una tipo Courier. Se dibuja en
   PROPIA                     la casa (o se toma una abierta: Courier Prime
                              es OFL) -- decision D3
   LETRAS CON ANCHO           el modo documento pide letra proporcional,
   VARIABLE Y KERNING         negrita y cursiva: lo de `PLAN_LA_TINTA.md`
   ESCRIBIR PDF               el guion se ENTREGA en PDF: un escritor de PDF
                              puro (paginas, texto con su letra incrustada)
   LEER Y ESCRIBIR XML        para el .fdx (y el .docx el dia que se pida)
   EL CORTAPAPELES            copiar y pegar entre apps (el director)
   IMPRIMIR                   no existe en BMO-X; el PDF lo cubre (se
                              imprime desde otra maquina)
```

---

## 4. La escalera (de lo mas chico a lo mas grande)

- [ ] **PL0 -- las decisiones de la seccion 7**, contestadas por el propietario (D1 si: ECCE).
- [x] **PL1 -- el motor de texto:** parrafos con elemento, cursor, seleccion,
      deshacer, buscar. Puro, en el anfitrion. **Como se sabe:** mil
      ediciones al azar y su deshacer vuelven al texto de partida, byte a byte.
      **15-10, la primera mitad:** `Ultra_userspace/apps/ecce` (TITAN++),
      `apps/ecce.bex` con icono (desde el 15-10, `apps/ecce/nucleo.bex`: una
      app CARPETA, KA1 de PLAN_EL_KERNEL_DE_LA_APP; su cara, un SVG). El texto es una `[byte]` Latin-1; escribir,
      Enter, Tab, Retroceso, Supr, flechas, Inicio, Fin, RePag y AvPag; las
      lineas que SE VEN se parten en el ultimo espacio que cabe. Guarda SOLO
      `datos/notas.txt` (Ctrl es del escritorio: no hay Ctrl+S), al cerrar y
      un segundo y medio despues de la ultima tecla. Las pruebas:
      `emisor-x86_64/tests/ecce.rs`, 8 (romper el corte de lineas las pone
      rojas).
      **15-10, la segunda mitad: el DESHACER.** Alt+Z deshace y Alt+Y rehace,
      por palabras (lo que se escribe o se borra seguido es un paso), sin
      limite; escribir otra cosa tira lo deshecho. Alt y no Ctrl: Ctrl es del
      escritorio, y la app lleva la cuenta de Alt con sus eventos crudos.
      **La prueba de las mil ediciones** (`a_thousand_random_edits_undo_to_
      the_start`): letras, Enter, Retroceso, Supr y flechas al azar; deshacerlo
      todo da el texto de partida byte a byte, rehacerlo todo el de despues.
      Restaurar un byte equivocado al deshacer la pone roja. PL1 HECHO en el
      anfitrion (los elementos por parrafo son de PL3 en adelante).
      **15-10, la tercera mitad: SELECCIONAR, COPIAR Y PEGAR.** Mayus +
      flechas (e Inicio / Fin, RePag / AvPag) seleccionan; Alt+C copia y
      Alt+X corta (sin seleccion, la linea entera); Alt+V pega; Alt+A lo
      selecciona todo; escribir, Retroceso o Supr con algo seleccionado lo
      reemplazan; todo con su deshacer (un corte entero vuelve con UN Alt+Z).
      Alt y no Ctrl: Ctrl+C es del escritorio (cierra la app de delante o
      frena el comando, `keys/windows.rs`), y una app no puede quitarselo.
      Pruebas: copiar y pegar, cortar y deshacer, escribir encima de lo
      seleccionado. **Falta:** el portapapeles es de ECCE; uno de la CASA,
      compartido entre apps, pide al director (PL16).
- [ ] **PL2 -- Fountain, leer y escribir.** **Como se sabe:** los ejemplos
      de la especificacion de Fountain dan los elementos que ella dice, y
      leer-escribir-leer no cambia nada.
      **15-10, LA MAYOR PARTE** (`apps/ecce/src/guion.titan`): lo que es
      cada linea sale de COMO esta escrita, como manda Fountain; el texto no
      se toca, asi que leer-escribir-leer es el mismo byte a byte. Alt+G
      cambia de NOTAS a GUION, y ECCE se abre en guion si la nota tiene un
      INT. o un EXT. Los elementos, uno a uno:
      - [x] escena: INT. / EXT. / EST. / INT./EXT. / I/E con blanco antes, y
        forzada con "." (en oro y en mayusculas)
      - [x] accion (y forzada con "!")
      - [x] personaje: en MAYUSCULAS con blanco antes y algo debajo, con su
        extension -- "ANA (V.O.)" -- y forzado con "@"
      - [x] dialogo, y el parentesis "(asi)" dentro de el
      - [x] transicion: MAYUSCULAS que acaban en "TO:" o en " A:" (CORTE
        A:), y forzada con ">"
      - [x] centrado: ">asi<"
      - [x] seccion "# ACTO I" (los ACTOS de la mesa) y sinopsis "= ..."
      - [~] salto de pagina "===": se reconoce, pero la pagina aun no salta
        ahi (PL4)
      - [ ] la PAGINA DE TITULO (Title:, Credit:, Author:, Draft date:...)
      - [ ] las notas "[[...]]" y el boneyard "/* ... */"
      - [ ] el dialogo DOBLE ("^" detras del segundo personaje, PL15)
      - [ ] las letras de cancion ("~")
      - [ ] el enfasis: *cursiva*, **negrita**, _subrayado_ (pide la letra
        en cursiva y negrita: la tinta)
      - [ ] no pintar los forzadores (".", "@", ">", "!") al imprimir
      - [ ] el JUEZ: los ejemplos de fountain.io, leidos aqui, dan los
        elementos que dice la especificacion
- [x] **PL3 -- la ventana en TITAN++:** (CERRADA en el anfitrion el 15-10;
      falta verla en el Ryzen: hoja 6b, FD, FOU y RATON) (la ventana de NOTAS ya esta, 15-10:
      ver PL1; falta el modo guion encima) escribir un guion con Tab y Enter,
      cada elemento con su sangria, y guardarlo como `.fountain`.
      - [x] cada elemento con su sangria, en la ventana (15-10): accion y
        escena a la izquierda, personaje a 22 columnas, dialogo a 10 (35 de
        ancho), parentesis a 16, transicion pegada a la derecha; el pie dice
        la escena, la pagina y el ELEMENTO donde esta el cursor (prueba
        `the_screenplay_elements_sit_in_their_columns`)
      - [x] Tab y Enter de Final Draft (15-10): Tab cambia lo que es la
        linea -- accion > personaje > transicion > escena > accion; en el
        dialogo, dialogo <> parentesis; una linea en blanco pasa a personaje
        (o a parentesis dentro de un dialogo, con el cursor entre los dos) --
        reescribiendola en Fountain (`guion.como`: "@", ">", ".", "!",
        mayusculas, parentesis), y Alt+Z la deja como estaba. Enter tras un
        personaje o un parentesis baja al dialogo; tras lo demas deja la
        linea en blanco de Fountain; Mayus+Enter, un salto solo. La ultima
        linea vacia bajo un personaje ya es su dialogo (el cursor a 10). Las
        marcas se pintan en tenue (pruebas
        `tab_and_enter_write_a_screenplay_like_final_draft` y
        `tab_in_the_dialogue_is_a_parenthetical_and_cycles_the_rest`)
      - [x] guardarlo como Fountain y abrir otros ficheros (15-10): ECCE abre
        el fichero que le pasan (`director.argumento()`, nuevo en TITAN++:
        lo de detras de la ruta al lanzar) y guarda ALLI; el explorador abre
        un `.fou` (`.fountain` no cabe en 8.3; ESTRATOS lo trae largo) con
        `apps/ecce/nucleo.bex`. Alt+F guarda como guion en `datos/guion.fou`
        y sigue en el. La cabecera dice el fichero (prueba
        `it_opens_the_file_it_is_given_and_alt_f_saves_a_fountain`). Lo que
        no hay: elegir el NOMBRE al guardar (pide un dialogo de fichero)
      - [x] el raton en la hoja (15-10): un clic pone el cursor en la letra
        mas cercana, arrastrar selecciona, Mayus + clic alarga; la cabecera
        y el pie no mueven nada, y el clic que abre una hoja desde la mesa no
        cuenta otra vez (prueba `a_click_places_the_cursor_and_a_drag_selects`)
- [ ] **PL4 -- EL PAGINADO del oficio:** margenes, (MAS) y (CONT.), no
      cortar donde no se corta. **Como se sabe (el juez):** guiones de
      prueba paginados en Final Draft dan las MISMAS paginas y los mismos
      cortes aqui.
      - [x] los margenes del oficio, en columnas (15-10)
      - [x] la pagina de 55 lineas con sus reglas (16-10, `pagina.titan`):
        el blanco de arriba no cuenta, acto y sinopsis no ocupan, "==="
        salta, la escena nunca sola al final (con dos lineas mas), el
        dialogo se parte con el nombre y dos lineas arriba y dos abajo --
        "(MAS)" abajo y "NOMBRE (CONT.)" arriba de la siguiente, nunca un
        parentesis al final --, la accion se parte dejando dos a cada lado,
        la transicion no se parte; un dialogo de mas de una pagina se parte
        las veces que haga falta. En la ventana, donde acaba cada pagina:
        el (MAS), la raya con "pag N" y el (CONT.); el pie dice "pag N de
        M" y la mesa del guion muestra las paginas DE VERDAD (pruebas
        `tests/ecce_pagina.rs`, 7, y
        `the_page_break_shows_more_the_rule_and_cont`)
      - [ ] partir la accion y el dialogo por el final de una FRASE (hoy
        por la linea que cabe)
      - [ ] el juez contra Final Draft (pide guiones paginados en Final
        Draft: los da el propietario, en PDF o `.fdx`, PL7)
- [ ] **PL5 -- SmartType:** completar personajes, lugares y momentos.
- [x] **PL6 -- el PDF:** el guion entregable. **HECHO en el anfitrion el
      16-10** (`pdf.titan`, Alt+P): PDF 1.4 con la Courier de 12 puntos, una
      de las catorce que todo lector trae (no se mete la letra); las
      paginas de PL4 con su (MAS), su NOMBRE (CONT.) y el numero desde la
      segunda; sin actos, sinopsis ni marcas de Fountain. Lo validan poppler
      (`pdftotext`) y `pypdf` en modo estricto, y las pruebas miran su
      indice por dentro (`alt_p_writes_the_screenplay_as_a_pdf`,
      `a_long_screenplay_has_its_pages_numbers_more_and_cont`). Para
      escribirlo, TITAN++ aprendio `push(mut bytes, "texto")`. Falta: el
      nombre del fichero (hoy `datos/guion.pdf`, PL17), la portada, y verlo
      en el Ryzen. **Como se sabe:** abre en
      cualquier lector de PDF y se ve igual que en pantalla.
- [ ] **PL7 -- .fdx:** abrir y guardar el formato de Final Draft. **Como se
      sabe:** un `.fdx` hecho en Final Draft entra, sale y Final Draft lo
      abre sin quejarse.
- [ ] **PL8 -- produccion:** numerar escenas, bloquear paginas, revisiones
      por color con su `*`.
- [ ] **PL9 -- la estructura:** fichas de escena, esquema y tablero de
      beats, que mueven el texto de verdad (mover una ficha mueve la escena).
- [x] **PL9b -- LA MESA: la hoja de cerca, la historia de lejos** (el
      propietario, 15-10: *"la hoja cuando entro es zoom siempre pero si
      retrocedo con control + rueda del raton [...] las hojas se ven de lejos
      como nodos"*). La maqueta: `docs/arte/maqueta_ecce.html`. Es un ZOOM
      SEMANTICO: un solo lienzo con las hojas puestas por acto (una fila por
      acto, una columna por escena); al entrar, siempre la hoja a medida de
      lectura; Ctrl + rueda aleja alrededor del puntero y, por debajo de ~25 %,
      cada hoja se vuelve un NODO (su numero, su cabecera, de que va y quien
      sale) unido a la siguiente; un clic en un nodo vuelve a su hoja. Tab y
      Enter como Final Draft (Tab cambia el elemento; tras un personaje, su
      dialogo). Lo que pide abajo, dicho:
      - **Ctrl es del escritorio** (`keys/app.rs`): CONTESTADA por el
        propietario el 15-10, la mesa usa **Alt + rueda** (como el deshacer
        usa Alt + Z). Es la regla que la casa ya tiene ("de quien es una
        tecla: un orden, nunca una heuristica"): Ctrl, las F y Alt+Tab /
        Alt+F4 / Alt+Enter son del escritorio; lo demas, de la ventana.
      - [x] **La RUEDA llega a una app** (15-10): `SUP_EV_RUEDA` (bit 60;
        las muescas con signo en el byte bajo, Alt en el bit 16), a la app
        que esta DEBAJO del puntero, como un clic; Ctrl + rueda no llega. En
        TITAN++, `director.evento()` dice 6 y `director.rueda()` da las
        muescas (prueba `the_wheel`). En C, `BMO_SUP_EV_RUEDA`.
      - Pintar texto ESCALADO: hoy la letra de TITAN++ es de una medida; de
        lejos basta con rayas grises (lo que hace la maqueta por debajo de
        ~45 %), de cerca hace falta la letra a varias medidas (VERRANO).
      **Como se sabe:** en el Ryzen, un guion de 8 escenas se aleja y se
      acerca a 60 fps, y el clic en un nodo deja el cursor en esa escena.
      **15-10, LA PRIMERA MESA, HECHA EN ECCE** (`apps/ecce/src/mesa.titan`;
      el banco del emulador en verde): tres vistas -- la HOJA (escribir),
      la MESA (cada hoja de 54 lineas en chico, una raya gris por linea, la
      elegida con marco de oro) y los NODOS (una tarjeta por hoja con su
      numero y su primera linea, unidas en orden por un hilo de oro). Alt +
      rueda aleja y acerca; acercando se abre la hoja elegida. En la mesa,
      las flechas eligen, Enter o un CLIC abren y Esc vuelve; la rueda sola,
      en la hoja, mueve el cursor tres lineas. Pruebas: Alt + rueda dos
      veces y un clic en el nodo 2 escriben al principio de la linea 55; los
      pixeles de la mesa (papel, raya, marco) y de los nodos (tarjeta,
      hilo). **Lo que falta, dicho:** el zoom es por PASOS, no continuo (la
      letra de TITAN++ es de una medida); los nodos son HOJAS, no escenas
      ni actos (eso pide el modo GUION, PL2..PL4); y verlo en el Ryzen (hoja
      6b, fila MESA).
      **15-10, Y EN GUION LOS NODOS SON ESCENAS:** una tarjeta por escena
      ("esc N" y su cabecera), cada ACTO ("# ACTO I") una fila con su titulo
      en oro encima, y el hilo de una escena a la siguiente entrando por la
      izquierda (no tacha el titulo). Un clic en una escena la abre con el
      cursor en su cabecera (prueba `in_a_screenplay_the_nodes_are_scenes_by_
      act`). Es la maqueta, de verdad. Lo que queda va en PL9c.
- [ ] **PL9c -- la mesa, entera:** el zoom CONTINUO alrededor del puntero
      (pide la letra de BMO-X a varias medidas -- VERRANO --, o pintar la
      hoja en una lamina y escalarla), mover una tarjeta y que se mueva la
      escena (PL9), y en cada nodo quien sale y de que va (la sinopsis "=").
- [ ] **PL10 -- los informes y el desglose:** escenas, personajes, lugares
      y etiquetas de produccion.
- [ ] **PL11 -- los otros modos:** serie, teatro y comic sobre el mismo motor.
- [ ] **PL12 -- el modo DOCUMENTO** (lo de Word): estilos, listas, tablas,
      notas al pie e imagenes, con letra proporcional (pide la tinta).
- [ ] **PL13 -- el modo NOVELA** (lo de Scrivener): el arbol de partes,
      capitulos y escenas, con su ficha, su estado y la meta de palabras.
- [ ] **PL14 -- el corrector** en castellano y en ingles (un diccionario
      abierto: Hunspell tiene los dos).
- [ ] **PL15 -- dialogo doble** y el modo sin distracciones.
- [ ] **PL16 -- el portapapeles de la CASA:** copiar en ECCE y pegar en otra
      app (y en la linea de Ejecutar). Lo guarda el director; las apps lo
      piden por su buzon. Hoy cada app tiene el suyo.
- [ ] **PL17 -- elegir el FICHERO:** abrir y "guardar como" con un dialogo
      de ficheros (hoy ECCE abre el que le pasan y Alt+F guarda en
      `datos/guion.fou`, un nombre fijo). Pide a TITAN++ textos que se
      construyen letra a letra (un `[byte]` a `text`), o el dialogo del
      director.

- [ ] **PL18 -- LA LETRA ELEGANTE** (el propietario, 16-10: *"ECCE puede
      tener letras elegantes? [...] que todo el INTERFAZ se vea mucho mas
      elegante"*, y *"no creo que letras sean dificiles [...] eso va en
      RING 3"*). La casa YA tiene su letra: `platform/shared/bmo-letra`
      (proporcional, de borde suave, trazos propios), la de las apps de
      Rust y del escritorio. Lo que no la tenia era TITAN++: `director.texto`
      pinta la de 8 x 16 del kernel, sin grises. Y la regla del propietario
      del 03-10 sigue: *"no integrar objetos terceros, sino tomar la
      inspiracion"* -- ninguna letra de otros entra al arbol.
      - [x] LE1 -- `toolchain/tools/bmo-atlas` (16-10): el ATLAS de una
        letra -- cada una de Latin-1 ya dibujada con su alfa (formato BLET)
        --, de la letra de la casa o de un TrueType que el propietario ELIJA
        (lee `glyf` con curvas cuadraticas y glifos COMPUESTOS, como la "a"
        con tilde; el fichero no entra al arbol). El juez: la tinta de una
        curva es su AREA, a cada medida (pruebas `tests/atlas.rs`, con un
        TrueType hecho a mano dentro de la prueba)
      - [x] LE2 -- ECCE escribe su CABEZA y su PIE con la letra de la casa
        (16-10, `letra.titan`): el build deja `apps/ecce/letra.blt` (14 px)
        y `titulo.blt` (26 px, negrita) junto al nucleo, ECCE los lee y
        pinta cada letra mezclando su borde con la barra. Sin ellos, la de
        8 x 16 de siempre (prueba `the_house_letter_writes_the_header_and_the_footer`).
        Para que fuera rapido, E1 ya no COPIA una lista que una fn solo lee
        (PRESTADO PARA LEER, `e1/mod.rs`): pintar la cabecera bajo de mas de
        100 millones de pasos a menos de 120, y todo ECCE copia menos
      - [ ] LE3 -- LA HOJA con una letra de ancho fijo y borde suave: el
        guion necesita columnas exactas (diez letras por pulgada). La de la
        casa es proporcional: o se le hace una version de ancho fijo (trazos
        propios), o el propietario elige un TrueType de ancho fijo para SU
        maquina (`bmo-atlas fuente.ttf`). **Lo decide el propietario**
      - [ ] LE4 -- la letra de la casa en `director.texto` para TODA app de
        TITAN++ (el atlas en el director, no en cada app), y la mesa de
        ECCE con ella
      - [ ] LE5 -- OpenType con CFF (curvas cubicas), el kerning y las
        ligaduras, para los TrueType que el propietario elija
---

## 5. Que hace cada lado

```text
   TITAN++         la app: la ventana, las teclas, los paneles, los modos
   codigo puro     el motor de texto, Fountain, el paginado, el PDF, el
   (Rust, probado  .fdx y el corrector: TODO se prueba en el anfitrion antes
   en el           de tocar la ventana -- el paginado es el que mas pruebas
   anfitrion)      pide, porque es el que un productor mira
   el director     dibujar el texto en la ventana (y la 3060, por C3)
```

---

## 6. Lo que este plan NO hace

* **No trae Final Draft ni Word.** Se aprenden sus ideas y el formato del
  oficio, que es publico; ni su codigo ni su nombre. Final Draft es de
  Cast & Crew; `.fdx` se lee porque es el formato con el que se trabaja.
* **No trae la colaboracion en red** (dos personas en el mismo guion) al
  principio: pide la red de BMO-X (`PLAN_LA_RED_SOLA.md`).
* **No hace el `.docx` al principio.** Primero Fountain, PDF y `.fdx`; el
  `.docx` (un ZIP con XML) entra si el modo documento lo pide.

---

## 7. Las decisiones del propietario

```text
   D1  EL NOMBRE        CONTESTADA (15-10): ECCE
   D2  DONDE VIVE       una app propia en la rejilla, o un modo del bloc
                        de texto que ya existe (`c/texto.bex`)
   D3  LA LETRA         una monoespacio dibujada en la casa, o Courier
                        Prime (licencia OFL, libre)
   D4  QUE VA PRIMERO   el guion (PL1..PL8) o el documento (PL12). El
                        propietario (15-10): *"es para tener mi bloc de
                        notas personales"* -- apunta a las NOTAS primero
                        (PL1, PL3 y el modo notas), y el guion encima
```

---

## 8. De donde sale lo de arriba

* Final Draft (Cast & Crew): su ayuda en linea y la lista de funciones de
  la version 13 (SmartType, Beat Board, revisiones, informes, dialogo
  doble). Citado de memoria: se lee antes de escribir la primera linea.
* Fountain: https://fountain.io/syntax (la especificacion abierta).
* El formato de pagina del guion de cine (Courier 12, una pagina por
  minuto) es oficio comun; las medidas exactas se toman de la ayuda de
  Final Draft y se prueban con PL4.
* Scrivener (Literature & Latte): el arbol de escenas y las fichas, como
  idea.
* Hunspell: los diccionarios abiertos de castellano e ingles (LibreOffice
  los usa).
