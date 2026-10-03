      * CABLIBRO -- los PARRAFOS de BANK CAT (van DESPUES del STOP RUN).
      *
      *   CAB-ABRIR     el saldo de partida es CAB-IMPORTE
      *   CAB-COBRAR    entra CAB-IMPORTE x CAB-VECES     (al HABER)
      *   CAB-PAGAR     sale  CAB-IMPORTE x CAB-VECES     (al DEBE)
      *   CAB-CUADRAR   INICIAL + HABER - DEBE tiene que ser el SALDO
      *
      * CAB-ESTADO dice como fue: 0 hecho, 1 no cabe, 2 sin saldo,
      * 3 importe malo, 4 descuadre, 5 hecho pero sin guardar.
      *
      * * PASA EL JUEZ (03-10, `cobol --juez`): toda aritmetica lleva
      * ON SIZE ERROR, y un movimiento se calcula ENTERO en los
      * temporales (CAB-PRUEBA, CAB-T-*) y solo se apunta si todo cupo.
       CAB-ABRIR.
           MOVE 0 TO CAB-ESTADO.
           IF CAB-IMPORTE < 0
               MOVE 3 TO CAB-ESTADO
           ELSE
               MOVE CAB-IMPORTE TO CAB-SALDO
               MOVE CAB-IMPORTE TO CAB-INICIAL
               MOVE 0 TO CAB-DEBE
               MOVE 0 TO CAB-HABER
               MOVE 0 TO CAB-ASIENTOS
           END-IF.
       CAB-PREPARAR.
           MOVE 0 TO CAB-ESTADO.
           IF CAB-VECES < 1
               MOVE 3 TO CAB-ESTADO
           END-IF.
           IF CAB-HECHO
               MULTIPLY CAB-VECES BY CAB-IMPORTE ON SIZE ERROR
                   MOVE 1 TO CAB-ESTADO
               END-MULTIPLY
           END-IF.
           MOVE 1 TO CAB-VECES.
           IF CAB-HECHO
               IF CAB-IMPORTE NOT > 0
                   MOVE 3 TO CAB-ESTADO
               END-IF
           END-IF.
           MOVE CAB-SALDO TO CAB-PRUEBA.
           MOVE CAB-HABER TO CAB-T-HABER.
           MOVE CAB-DEBE TO CAB-T-DEBE.
           MOVE CAB-ASIENTOS TO CAB-T-ASIENTOS.
           ADD 1 TO CAB-T-ASIENTOS ON SIZE ERROR
               MOVE 1 TO CAB-ESTADO
           END-ADD.
       CAB-ASENTAR.
           IF CAB-HECHO
               MOVE CAB-PRUEBA TO CAB-SALDO
               MOVE CAB-T-HABER TO CAB-HABER
               MOVE CAB-T-DEBE TO CAB-DEBE
               MOVE CAB-T-ASIENTOS TO CAB-ASIENTOS
           END-IF.
       CAB-COBRAR.
           PERFORM CAB-PREPARAR.
           IF CAB-HECHO
               ADD CAB-IMPORTE TO CAB-PRUEBA ON SIZE ERROR
                   MOVE 1 TO CAB-ESTADO
               END-ADD
           END-IF.
           IF CAB-HECHO
               ADD CAB-IMPORTE TO CAB-T-HABER ON SIZE ERROR
                   MOVE 1 TO CAB-ESTADO
               END-ADD
           END-IF.
           PERFORM CAB-ASENTAR.
       CAB-PAGAR.
           PERFORM CAB-PREPARAR.
           IF CAB-HECHO
               IF CAB-IMPORTE > CAB-SALDO
                   MOVE 2 TO CAB-ESTADO
               END-IF
           END-IF.
           IF CAB-HECHO
               SUBTRACT CAB-IMPORTE FROM CAB-PRUEBA ON SIZE ERROR
                   MOVE 1 TO CAB-ESTADO
               END-SUBTRACT
           END-IF.
           IF CAB-HECHO
               ADD CAB-IMPORTE TO CAB-T-DEBE ON SIZE ERROR
                   MOVE 1 TO CAB-ESTADO
               END-ADD
           END-IF.
           PERFORM CAB-ASENTAR.
       CAB-CUADRAR.
           MOVE 0 TO CAB-ESTADO.
           MOVE CAB-INICIAL TO CAB-PRUEBA.
           ADD CAB-HABER TO CAB-PRUEBA ON SIZE ERROR
               MOVE 4 TO CAB-ESTADO
           END-ADD.
           SUBTRACT CAB-DEBE FROM CAB-PRUEBA ON SIZE ERROR
               MOVE 4 TO CAB-ESTADO
           END-SUBTRACT.
           IF CAB-PRUEBA NOT = CAB-SALDO
               MOVE 4 TO CAB-ESTADO
           END-IF.
