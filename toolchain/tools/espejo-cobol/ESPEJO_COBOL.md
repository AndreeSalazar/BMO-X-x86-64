# ESPEJO de COBOL -- el ultimo informe

> Lo escribe `cargo run -p bmo-espejo-cobol -- --informe` (CM0 de
> `docs/plan/PLAN_COBOL_MAESTRO.md`). BMO COBOL, ejecutado en el emulador de
> x86-64, contra GnuCOBOL. No se edita a mano.

**20 programas: 6 IGUAL, 6 SOLO DISPLAY, 3 DISTINTO, 3 BMO NO, 2 NO ESTANDAR, 0 NO JUZGA**

| veredicto | programa | detalle |
|---|---|---|
| BMO NO | `toolchain/tools/espejo-cobol/casos/aritmetica.cob` | BMO no lo compila: linea 18: DIVIDE requires `BY` |
| IGUAL | `toolchain/tools/espejo-cobol/casos/bankcat.cob` |  |
| IGUAL | `toolchain/tools/espejo-cobol/casos/decision.cob` |  |
| BMO NO | `toolchain/tools/espejo-cobol/casos/desborde.cob` | BMO no lo compila: linea 11: 'ON' es palabra reservada COBOL (COBOL74); aun sin soporte como sentencia: ON SIZE ERROR DISPLAY "NO CABE" |
| IGUAL | `toolchain/tools/espejo-cobol/casos/tablas.cob` |  |
| BMO NO | `toolchain/tools/espejo-cobol/casos/texto.cob` | BMO no lo compila: linea 11: STRING: '" "' no es UNA fuente. Cada una lleva su `DELIMITED BY SIZE` detras |
| IGUAL | `toolchain/lang/cobol/examples/1-basico/hola.cob` |  |
| SOLO DISPLAY | `toolchain/lang/cobol/examples/2-decimal/banco.cob` | linea 6: GnuCOBOL `00059.97` / BMO `59.97` |
| SOLO DISPLAY | `toolchain/lang/cobol/examples/2-decimal/calc.cob` | linea 5: GnuCOBOL `+0000022.99` / BMO `22.99` |
| SOLO DISPLAY | `toolchain/lang/cobol/examples/2-decimal/calcgui.cob` | linea 2: GnuCOBOL `+000003750.00` / BMO `3750.00` |
| IGUAL | `toolchain/lang/cobol/examples/2-decimal/hola_COBOL.cob` |  |
| IGUAL | `toolchain/lang/cobol/examples/3-presentacion/extracto.cob` |  |
| DISTINTO | `toolchain/lang/cobol/examples/4-ficheros/batch.cob` | linea 3: GnuCOBOL `$22,528.22` / BMO ` $1,135.00` |
| NO ESTANDAR | `toolchain/lang/cobol/examples/5-tablas/conceptos.cob` | GnuCOBOL: conceptos.cob:65: error: syntax error, unexpected END-READ |
| NO ESTANDAR | `toolchain/lang/cobol/examples/6-condiciones/cartera.cob` | GnuCOBOL: cartera.cob:57: error: 'DEVOLS' is not a numeric name |
| DISTINTO | `toolchain/lang/cobol/examples/7-empaquetado/cuentas.cob` | linea 5: GnuCOBOL `345` / BMO `12345` |
| DISTINTO | `toolchain/lang/cobol/examples/8-parrafos/cierre.cob` | linea 4: GnuCOBOL `00006` / BMO `4` |
| SOLO DISPLAY | `toolchain/lang/cobol/examples/9-decision/comision.cob` | linea 27: GnuCOBOL `+00000.10` / BMO `0.10` |
| SOLO DISPLAY | `toolchain/lang/cobol/examples/10-binario/maestro.cob` | linea 18: GnuCOBOL `00003` / BMO `3` |
| SOLO DISPLAY | `toolchain/lang/cobol/examples/11-bankcat/libro.cob` | linea 2: GnuCOBOL `+0000000001250.00` / BMO `1250.00` |
