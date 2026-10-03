      * ARITMETICA EXACTA: lo que un banco suma, multiplica y divide.
      * La salida va con MASCARA (como un extracto), para juzgar el VALOR.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ARITMETICA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A          PIC S9(7)V99 VALUE 19.99.
       01  B          PIC S9(7)V99 VALUE 0.
       01  C          PIC S9(7)V99 VALUE 10.00.
       01  VER        PIC -(7)9.99.
       PROCEDURE DIVISION.
           MULTIPLY 3 BY A GIVING B.
           MOVE B TO VER.
           DISPLAY VER.
           ADD 0.01 TO B.
           MOVE B TO VER.
           DISPLAY VER.
           DIVIDE 3 INTO C ROUNDED.
           MOVE C TO VER.
           DISPLAY VER.
           COMPUTE B = (A + 0.03) * 2 - 100.
           MOVE B TO VER.
           DISPLAY VER.
           SUBTRACT 1000 FROM C.
           MOVE C TO VER.
           DISPLAY VER.
           STOP RUN.
