      * CABLIBRO -- los PARRAFOS de BANK CAT (van DESPUES del STOP RUN).
      *
      *   CAB-ABRIR     el saldo de partida es CAB-IMPORTE
      *   CAB-COBRAR    entra CAB-IMPORTE x CAB-VECES     (al HABER)
      *   CAB-PAGAR     sale  CAB-IMPORTE x CAB-VECES     (al DEBE)
      *   CAB-CUADRAR   INICIAL + HABER - DEBE tiene que ser el SALDO
      *
      * CAB-ESTADO dice como fue: 0 hecho, 1 no cabe, 2 sin saldo,
      * 3 importe malo, 4 descuadre. Un movimiento que no se puede hacer
      * NO toca nada: o entra entero, o no entra.
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
       CAB-COBRAR.
           MOVE 0 TO CAB-ESTADO.
           MULTIPLY CAB-VECES BY CAB-IMPORTE ON SIZE ERROR
               MOVE 1 TO CAB-ESTADO
           END-MULTIPLY.
           MOVE 1 TO CAB-VECES.
           IF CAB-HECHO
               IF CAB-IMPORTE > 0
                   MOVE CAB-SALDO TO CAB-PRUEBA
                   ADD CAB-IMPORTE TO CAB-PRUEBA ON SIZE ERROR
                       MOVE 1 TO CAB-ESTADO
                   END-ADD
               ELSE
                   MOVE 3 TO CAB-ESTADO
               END-IF
           END-IF.
           IF CAB-HECHO
               MOVE CAB-PRUEBA TO CAB-SALDO
               ADD CAB-IMPORTE TO CAB-HABER
               ADD 1 TO CAB-ASIENTOS
           END-IF.
       CAB-PAGAR.
           MOVE 0 TO CAB-ESTADO.
           MULTIPLY CAB-VECES BY CAB-IMPORTE ON SIZE ERROR
               MOVE 1 TO CAB-ESTADO
           END-MULTIPLY.
           MOVE 1 TO CAB-VECES.
           IF CAB-HECHO
               IF CAB-IMPORTE > 0
                   IF CAB-IMPORTE > CAB-SALDO
                       MOVE 2 TO CAB-ESTADO
                   END-IF
               ELSE
                   MOVE 3 TO CAB-ESTADO
               END-IF
           END-IF.
           IF CAB-HECHO
               SUBTRACT CAB-IMPORTE FROM CAB-SALDO
               ADD CAB-IMPORTE TO CAB-DEBE
               ADD 1 TO CAB-ASIENTOS
           END-IF.
       CAB-CUADRAR.
           COMPUTE CAB-PRUEBA = CAB-INICIAL + CAB-HABER - CAB-DEBE.
           IF CAB-PRUEBA = CAB-SALDO
               MOVE 0 TO CAB-ESTADO
           ELSE
               MOVE 4 TO CAB-ESTADO
           END-IF.
