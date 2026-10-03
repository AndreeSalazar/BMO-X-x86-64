# PLAN BANK CAT -- la cartera de CAB, llevada por COBOL (F5)

> Escrito el **2026-10-03**. El propietario: *"estudia COBOL, pero me gustaria
> crear algo como libreria o WRAPPER para simplificar por completo, y asi
> preciso en emitir x86-64 a la CPU; es para jugar con BANK CAT. F5 para eso,
> y el F6 lo reemplazo con RED"*. Y antes: *"BANK CAT simplemente es para
> aplicar COBOL honesto y reflejo"*.
>
> La maqueta es `docs/arte/maqueta_bankcat.html` (el GATO HUCHA). El
> compilador es BMO COBOL (`toolchain/lang/cobol`), y su plan de banca es
> `toolchain/lang/cobol/PLAN_BANCA.md`: este plan usa lo que aquel construye.

---

# 0. LA FORMA

```text
   la CARA     una app de Rust, como HERMES: el gato hucha, el saldo, los
               botones. No calcula ni un centimo
   el WRAPPER  `bmo-bankcat` (puro, probado en el anfitrion): escribe las
               ordenes y lee las respuestas en CENTIMOS ENTEROS; en la
               maquina, lanza el motor con su consola y le habla por lineas
   el MOTOR    `cobol/11/libro.bex`, COBOL compilado a x86-64 por la casa:
               lleva el libro con partida doble y COMP-3
   la LIBRERIA los copybooks `CABDATOS` y `CABLIBRO` (`COPY`): lo que
               cualquier otro programa COBOL de BANK CAT trae con una linea
```

Es el patron de la calculadora (`desktop/calc.rs` + `cobol/2/calcgui.bex`):
la cara en Rust, el dinero en COBOL. Asi el dinero lo calcula SIEMPRE el
mismo codigo, el que se puede leer en voz alta.

---

# 1. LOS ESCALONES

- [x] **BC0 -- la maqueta.** `docs/arte/maqueta_bankcat.html` (03-10).
- [x] **BC0b -- las teclas.** F5 lanza `sys/bankcat.bex` (o dice que aun
      no esta); la RED paso a F6 y `smp` se teclea (03-10).
- [~] **BC1 -- la LIBRERIA y el MOTOR en COBOL** (03-10). `COPY` en el
      compilador (2.8 de PLAN_BANCA, sin `REPLACING`), los copybooks
      `copy/CABDATOS.cpy` y `copy/CABLIBRO.cpy` y el motor
      `examples/11-bankcat/libro.cob`, EJECUTADO en el emulador
      (`tests/bankcat.rs`): 1250.00 + 50.00 - 3 x 19.99 = 1240.03, sin saldo
      no se paga, lo que no cabe en S9(13)V99 no entra. Va al disco como
      `cobol/11/libro.bex`. **Como se sabe en el Ryzen:** `run cobol/11/libro.bex`
      y teclear `1`, `1250.00`, `2`, `50.00`, `9`, `0`: contesta `0`/`1250.00`,
      `0`/`1300.00`, `0`/`1300.00`. [!] NO probado en el Ryzen.
- [ ] **BC2 -- el WRAPPER** (`bmo-bankcat`): las ordenes y las respuestas,
      puras y probadas; y una prueba que pone al wrapper a hablar con el
      motor COBOL DE VERDAD, ejecutado en el emulador.
- [ ] **BC3 -- la CARA** (`apps/bankcat`, F5): el gato hucha de la maqueta,
      el saldo y los botones, hablando con el motor por el wrapper.
- [ ] **BC4 -- el libro en el DISCO.** Hoy el motor lo lleva en memoria.
      Guardarlo pide `OPEN EXTEND` o `I-O` (3.1 y 3.2 de PLAN_BANCA), o
      reescribir el fichero entero en cada cierre.
- [ ] **BC5 -- entre amigos, por HERMES** (pide H6): un movimiento firmado
      con tu clave y apuntado en LOS DOS libros.
- [ ] **BC6 -- el CAJERO** (decision D1 del propietario).

---

# 2. LO QUE DECIDE EL PROPIETARIO

- **D1 -- comprar CAB con dinero de verdad.** Si se hace: solo de ENTRADA
  (como el Platinum de Warframe), el pago en la ANTENA y nunca en BMO-X, y
  los CAB nunca vuelven a ser dinero. Sacar dinero ya es dinero electronico:
  licencia, identidad de cada usuario, normas contra el blanqueo.

---

# 3. LOS BANCOS, DICHO CLARO

El propietario: *"estoy pensando en hablar con los bancos, para convertir sus
monedas en nodos [...] y quien sabe convertir CAB en monedas que los bancos
apliquen"*.

Se puede hablar con ellos, y conviene llegar con esto claro:

- **Lo que ya existe en la banca:** los "depositos tokenizados" (dinero de un
  banco apuntado en un libro compartido) los prueban bancos grandes, siempre
  DENTRO de su regulacion y con nodos que ellos controlan o certifican.
- **Lo que un banco va a pedir antes de mirar el codigo:** una empresa
  detras, quien responde si algo falla, auditorias de seguridad hechas por
  terceros, cumplimiento (identidad de clientes, prevencion de blanqueo) y,
  para tocar dinero de verdad, una licencia o un banco socio. Muchos paises
  tienen "sandbox" regulatorio para empezar en chico.
- **Lo que BMO-X puede mostrar YA:** un libro de partida doble en COBOL que
  cuadra al centimo, compilado por un compilador propio a x86-64, en un
  sistema sin dependencias de terceros. Eso es una DEMO honesta y es un buen
  comienzo de conversacion.
- **Lo que no hay que prometer:** que CAB "se convierta en moneda" de un
  banco. Eso lo decide el banco y su regulador, no el codigo.

Hasta que haya un acuerdo de verdad, BANK CAT es un circuito CERRADO: CAB
de juego, sin salida a dinero.
