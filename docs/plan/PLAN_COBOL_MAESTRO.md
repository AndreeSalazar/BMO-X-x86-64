# PLAN COBOL MAESTRO -- un FRONTEND bien hecho, y el x86-64 aparte

> Escrito el **2026-10-03**. El propietario: *"apuntar el plan con el COBOL
> MAESTRO TOTAL, pero FRONTEND bien hecho y backend en emitir x86-64
> separado, por si voy a otra arquitectura"*. Y antes: *"el ESPEJO de COBOL"*,
> para saber de verdad en que parte esta, y si un banco miraria esto.
>
> Va encima de `toolchain/lang/cobol/PLAN_BANCA.md` (las tareas del lenguaje,
> numeradas 0.x a 8.x) y de `PLAN_BANK_CAT.md` (el que lo usa). Este plan dice
> COMO tiene que estar hecho el compilador por dentro; aquel, QUE tiene que
> saber.

---

# 0. LA RESPUESTA CORTA

```text
   fuente .cob
     |  COPY             src/copia.rs (03-10)                     FRONTEND
     |  lexer            src/lexer.rs                             agnostico:
     |  PARSER           hoy por LINEAS (parser.rs); luego el     no nombra
     |                   de TOKENS (tparser.rs, 0.2)              ninguna CPU
     |  SEMANTICA        nombres, PIC -> tipos, 88, OCCURS, y lo
     |                   que NO es COBOL dicho con su motivo      (nuevo)
     v
   IR COBOL              operaciones de decimal con su escala, el
                         redondeo y el ON SIZE ERROR EXPLICITOS,
                         y el flujo de control. Un interprete en el
                         anfitrion lo EJECUTA: es el oraculo       (nuevo)
     |
     v  BACKEND           emisor-x86_64/: solo BAJA el IR a x86-64
   .bex                   (bmo-lower). Otra CPU = otro emisor-<cpu>/
                          al lado, con el MISMO IR. Lo vigila `isa`
```

**El juez de todo**, desde hoy: el **ESPEJO de COBOL**
(`toolchain/tools/espejo-cobol`), que corre cada programa con BMO COBOL en el
emulador y con **GnuCOBOL**, y compara lo impreso. Su ultimo informe esta en
`toolchain/tools/espejo-cobol/ESPEJO_COBOL.md`.

---

# 1. DONDE ESTA HOY, MEDIDO (03-10)

El ESPEJO, sobre los 11 ejemplos de la casa y 6 casos escritos en COBOL
estandar puro:

```text
   20 programas: 6 IGUAL, 6 SOLO DISPLAY, 3 DISTINTO, 3 BMO NO, 2 NO ESTANDAR
```

| que | cuantos | que dice | a donde va |
|---|---|---|---|
| IGUAL | 6 | byte a byte con GnuCOBOL, incluida la LIBRERIA de BANK CAT (`casos/bankcat.cob`) | -- |
| SOLO DISPLAY | 6 | el VALOR es el mismo; difiere como se escribe un numero sin mascara: GnuCOBOL `00059.97` (el estandar), BMO `59.97` | CM6 (1.5) |
| BMO NO | 3 | el parser por LINEAS: `DIVIDE ... INTO`, un `ON SIZE ERROR` en la linea de abajo, un `STRING` en dos lineas | CM2 |
| DISTINTO | 3 | `cuentas.cob`: `MOVE 12345` a un `PIC 9(3)` normal debe dar `345` y BMO deja `12345` (FALLO de BMO). `cierre.cob` y `batch.cob`: leen un fichero de TEXTO en un registro `COMP-3`; el estandar lo lee como bytes y BMO convierte el texto (EXTENSION de BMO) | CM1, CM7 |
| NO ESTANDAR | 2 | GnuCOBOL los rechaza y BMO los acepta: `conceptos.cob` (`END-READ` donde no va) y `cartera.cob` (`DEVOLS` no es numerico) | CM3 |

**En una frase:** como CALCULADORA de dinero, BMO COBOL ya da lo mismo que
el COBOL de fuera; como LECTOR de COBOL ajeno, le falta el parser de tokens
y una semantica que diga que no a lo que no es COBOL.

---

# 2. POR QUE ASI: lo que esta mal repartido hoy

- **El parser es por LINEAS** (`parser.rs`, ~2000 lineas): una sentencia
  tiene que caber en su linea, y el COBOL de un banco no se escribe asi. El
  de tokens (`tparser.rs`) existe y no es el principal (0.2 de PLAN_BANCA).
- **El backend hace trabajo del frontend.** `codegen.rs` (~2900 lineas)
  trocea las expresiones de `COMPUTE` (`tokenize_expr`), decide las escalas
  y resuelve nombres. El 03-10 eso dio un fallo de verdad: `COMPUTE` partia
  `CAB-SALDO` en `CAB` menos `SALDO`. **Un fallo de LENGUAJE no puede vivir
  en el emisor de una CPU**: con otro emisor habria que arreglarlo dos veces.
- **No hay una capa que diga "esto no es COBOL".** Por eso pasan
  `conceptos.cob` y `cartera.cob`, que GnuCOBOL rechaza.

---

# 3. LOS ESCALONES

- [x] **CM0 -- el ESPEJO de COBOL** (03-10). `toolchain/tools/espejo-cobol`:
      BMO COBOL (emulador de x86-64) contra GnuCOBOL (`cobc`), con seis
      veredictos (IGUAL, SOLO DISPLAY, DISTINTO, BMO NO, NO ESTANDAR, NO
      JUZGA), `uno <fichero>` para mirar un caso y `--informe`. Sin `cobc`
      no inventa veredictos: lo dice. **Como se sabe:** `cargo run -p
      bmo-espejo-cobol -- --informe` reescribe `ESPEJO_COBOL.md`.
- [ ] **CM1 -- los fallos que el espejo ya muestra.** El `MOVE` que no trunca
      a un `PIC 9(n)` normal (`cuentas.cob`); `DIVIDE ... INTO` y
      `MULTIPLY ... GIVING` de las dos formas. **Como se sabe:**
      `cuentas.cob` y `casos/aritmetica.cob` en IGUAL o SOLO DISPLAY.
- [ ] **CM2 -- el parser de TOKENS como principal** (0.2). Sentencias que
      cruzan lineas, clausulas en la linea de abajo, el formato fijo de
      verdad (columnas 7 y 8-72). **Como se sabe:** `casos/desborde.cob` y
      `casos/texto.cob` dejan de ser BMO NO, y la matriz de 61 filas sigue
      en verde.
- [ ] **CM3 -- la SEMANTICA, en el frontend.** Un crate agnostico que
      resuelve nombres, convierte cada PIC en su tipo (digitos, escala,
      signo, uso), cuelga los 88 y las tablas, y RECHAZA con su motivo lo
      que no es COBOL. **Como se sabe:** `conceptos.cob` y `cartera.cob`
      salen de NO ESTANDAR (se arreglan los ejemplos, o BMO los rechaza con
      el mismo motivo que GnuCOBOL).
- [ ] **CM4 -- el IR COBOL y su INTERPRETE.** Lo que sale de la semantica:
      operaciones de decimal con escala, redondeo y desborde explicitos, y
      el flujo de control. Un interprete en el anfitrion lo ejecuta y
      entra en el ESPEJO como TERCER juez (BMO interpretado, BMO x86-64,
      GnuCOBOL). **Como se sabe:** los tres coinciden en todo lo IGUAL.
- [ ] **CM5 -- el emisor de x86-64 solo BAJA el IR.** `tokenize_expr`, las
      escalas y los nombres salen de `codegen.rs`. Otra CPU seria otro
      `emisor-<cpu>/` con el mismo IR -- este repositorio solo tiene
      x86-64, y el guardian `isa` lo vigila. **Como se sabe:** `codegen.rs`
      no tiene ni una decision de lenguaje (se lee) y el ESPEJO no cambia.
- [ ] **CM6 -- el DISPLAY del estandar** (1.5): `00059.97` con sus ceros y
      su signo, como GnuCOBOL. **Como se sabe:** los SOLO DISPLAY pasan a
      IGUAL. (Ver D1: cambia lo que ya muestran los ejemplos.)
- [ ] **CM7 -- los ficheros como el estandar.** `ORGANIZATION IS LINE
      SEQUENTIAL` para el texto y `SEQUENTIAL` para registros fijos, y la
      conversion de texto a numero del `READ` marcada como EXTENSION de BMO
      (o fuera). Con `EXTEND` e `I-O` (3.1, 3.2) mejora BC4 de BANK CAT, que
      hoy guarda su libro reescribiendolo entero (03-10).
      **Como se sabe:** `cierre.cob` y `batch.cob` en IGUAL.
- [ ] **CM8 -- lo que lleva un banco**, en el orden de PLAN_BANCA: `CALL` y
      `LINKAGE` (6.2, 6.3), `SORT` (5.1), ficheros indexados (fase 4),
      `SYNCPOINT` / `ROLLBACK` (7.2). Cada uno con su caso en el ESPEJO.

---

# 4. LO QUE DECIDE EL PROPIETARIO

- **D1 -- el dialecto por defecto.** ESTRICTO (lo que dice el estandar y
  GnuCOBOL: `DISPLAY` con ceros, el `READ` no convierte texto) o BMO (lo de
  hoy, mas comodo de leer). Lo recomendado: ESTRICTO por defecto, y las
  extensiones de BMO con una bandera que las nombre. Para hablar con un
  banco, lo que cuenta es "da lo mismo que el COBOL que ya usais".
- **D2 -- GnuCOBOL como juez.** Es una herramienta del ANFITRION (como GCC
  y Clang para el ESPEJO de C), nunca entra en BMO-X. En Windows se instala
  aparte; sin ella el espejo compila y corre BMO, pero no juzga.
