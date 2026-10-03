      * ON SIZE ERROR: lo que no cabe NO entra, y se dice.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DESBORDE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A          PIC 9(3) VALUE 999.
       01  D          PIC 9(3) VALUE 10.
       01  VER        PIC ZZ9.
       PROCEDURE DIVISION.
           ADD 1 TO A
               ON SIZE ERROR DISPLAY "NO CABE"
               NOT ON SIZE ERROR DISPLAY "CABE"
           END-ADD.
           MOVE A TO VER.
           DISPLAY VER.
           DIVIDE 0 INTO D
               ON SIZE ERROR DISPLAY "DIVISION POR CERO"
           END-DIVIDE.
           MOVE D TO VER.
           DISPLAY VER.
           STOP RUN.
