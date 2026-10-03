      * La LIBRERIA de BANK CAT (COPY CABDATOS + CABLIBRO), a mano:
      * abrir, cobrar y pagar 3 x 19.99, con el saldo con mascara.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BANKCAT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY CABDATOS.
       01  VER        PIC -(13)9.99.
       PROCEDURE DIVISION.
           MOVE 1250.00 TO CAB-IMPORTE.
           PERFORM CAB-ABRIR.
           MOVE 50.00 TO CAB-IMPORTE.
           PERFORM CAB-COBRAR.
           MOVE 3 TO CAB-VECES.
           MOVE 19.99 TO CAB-IMPORTE.
           PERFORM CAB-PAGAR.
           MOVE CAB-SALDO TO VER.
           DISPLAY VER.
           MOVE 99999.00 TO CAB-IMPORTE.
           PERFORM CAB-PAGAR.
           DISPLAY CAB-ESTADO.
           PERFORM CAB-CUADRAR.
           DISPLAY CAB-ESTADO.
           STOP RUN.
           COPY CABLIBRO.
