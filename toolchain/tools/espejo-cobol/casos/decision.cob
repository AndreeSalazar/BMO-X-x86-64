      * EVALUATE, niveles 88 y PERFORM VARYING: el escalado de un banco.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DECISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  I          PIC 9(3).
       01  SALDO      PIC S9(5)V99.
       01  ESTADO     PIC 9 VALUE 0.
           88  ACTIVO     VALUE 1.
           88  CERRADO    VALUE 2.
       01  VER        PIC Z9.
       PROCEDURE DIVISION.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4
               COMPUTE SALDO = I * 400
               EVALUATE TRUE
                   WHEN SALDO > 1000
                       DISPLAY "ALTO"
                   WHEN SALDO > 500
                       DISPLAY "MEDIO"
                   WHEN OTHER
                       DISPLAY "BAJO"
               END-EVALUATE
           END-PERFORM.
           MOVE 2 TO ESTADO.
           IF CERRADO
               DISPLAY "CERRADO"
           END-IF.
           MOVE I TO VER.
           DISPLAY VER.
           STOP RUN.
