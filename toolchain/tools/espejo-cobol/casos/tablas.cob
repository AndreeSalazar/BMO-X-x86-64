      * OCCURS: una tabla de conceptos y su total.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TABLAS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  T.
           05  E          PIC S9(5)V99 OCCURS 3 TIMES.
       01  I          PIC 9(3).
       01  TOTAL      PIC S9(7)V99 VALUE 0.
       01  VER        PIC $$$,$$9.99.
       PROCEDURE DIVISION.
           MOVE 10.05 TO E(1).
           MOVE 0.20 TO E(2).
           MOVE 1.75 TO E(3).
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
               ADD E(I) TO TOTAL
           END-PERFORM.
           MOVE TOTAL TO VER.
           DISPLAY VER.
           MOVE E(3) TO VER.
           DISPLAY VER.
           STOP RUN.
