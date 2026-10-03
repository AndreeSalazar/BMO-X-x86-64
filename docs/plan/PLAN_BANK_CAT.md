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
- [x] **BC2 -- el WRAPPER** (`platform/shared/bmo-bankcat`, 03-10): las
      ordenes y las respuestas, puras, en centimos `i64` (ni un `f64`), y el
      saldo en castellano (`1.240,03`); 7 pruebas. Y la que importa:
      `el_wrapper_y_el_motor_hablan_el_mismo_idioma` (en `bmo-cobol-x86-64`)
      escribe las ordenes con el wrapper, las EJECUTA el motor COBOL en el
      emulador y las respuestas las lee otra vez el wrapper: cuadra al centimo.
- [~] **BC3 -- la CARA** (`Ultra_userspace/apps/bankcat`, F5, 03-10): el
      gato hucha de la maqueta (monedas en los ojos y cayendo en la ranura al
      cobrar, lagrima al pagar, la pata que llama), el saldo grande en
      castellano, los tres botones, el libro de la sesion y las tarjetas de
      lo que aun no hay (Mover, Mercado, Cajero) dichas sin fingir.
      ** El motor es del ESCRITORIO (`desktop/bankcat.rs`): una app nace sin
      la autoridad de lanzar (`task/autoridad.rs`), asi que la app PIDE
      (`0x1E bankcat pagar 19.99`, `desktop/pide.rs`), el escritorio le
      habla al motor con el wrapper y le devuelve `0x1E bank <estado>
      <centimos>`. El motor vive lo que vive el escritorio y abre el libro
      con la BIENVENIDA (1.250,00 CAB de juego). Pintado en el anfitrion con
      el arnes (1200x720 y 900x600). **Como se sabe en el Ryzen:** F5, el
      saldo dice 1.250,00; "3 x 19,99" lo deja en 1.190,03 y el gato llora;
      cerrar y volver a abrir con F5 sigue en 1.190,03. [!] NO probado en el
      Ryzen.
- [~] **BC4 -- el libro en el DISCO** (03-10). El motor CARGA
      `bankcat.dat` al nacer (cinco cifras: inicial, haber, debe, saldo,
      asientos) y lo REESCRIBE entero tras cada movimiento hecho: sin `OPEN
      EXTEND` ni `I-O` (3.1 y 3.2 de PLAN_BANCA), es lo que hay, y el libro
      es chico. Abrir con un libro que ya existe no lo pisa (contesta su
      saldo). Si el disco no guarda, el estado es **5**: hecho en memoria,
      NO guardado, y la cara lo dice. Probado en el emulador
      (`el_libro_sobrevive_al_reinicio`, `si_el_disco_no_guarda_se_dice`).
      [!] Reescribir un fichero que ya existe es 3.0 de PLAN_BANCA (FAT32
      que REEMPLAZA), escrito y NO probado en el Ryzen. **Como se sabe en el
      Ryzen:** F5, pagar 19,99, reiniciar la maquina, F5: el saldo sigue.
- [x] **BC4b -- PASA EL JUEZ** (03-10). La libreria (`CABDATOS`,
      `CABLIBRO`) y el motor pasan `cobol --juez`, el juez de nivel banco de
      PLAN_COBOL_MAESTRO: toda aritmetica con ON SIZE ERROR, y cada
      movimiento calculado ENTERO en temporales (`CAB-PRUEBA`, `CAB-T-*`)
      y apuntado solo si todo cupo.
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
