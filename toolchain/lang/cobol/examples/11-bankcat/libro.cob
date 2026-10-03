      * LIBRO -- el MOTOR de BANK CAT (F5). Sin adornos y sin preguntas.
      *
      * Como `2-decimal/calcgui.cob`: quien lo llama es un programa, no una
      * persona. Lee ordenes de DOS lineas y contesta DOS por orden.
      *
      *     entra:  codigo \n  importe \n      (una y otra vez)
      *     sale:   estado \n  saldo   \n      (por cada orden)
      *
      *     codigo: 1 abrir con ese saldo    2 cobrar    3 pagar
      *             4 las VECES del siguiente (el importe es el numero)
      *             5 cuadrar                9 cerrar (cuadra y acaba)
      *
      *     estado: 0 hecho   1 no cabe   2 sin saldo   3 importe malo
      *             4 descuadre
      *
      * * LA LIBRERIA. Todo lo de dinero viene de dos copybooks de
      * `toolchain/lang/cobol/copy`: CABDATOS (los datos) y CABLIBRO (los
      * parrafos). Este programa solo traduce ordenes en PERFORM.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LIBRO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY CABDATOS.
       01  CODIGO             PIC 9.
       01  ENTRA              PIC S9(13)V99.
       01  FIN                PIC 9 VALUE 0.
           88  SE-ACABO           VALUE 1.
       PROCEDURE DIVISION.
           PERFORM UNTIL SE-ACABO
               ACCEPT CODIGO
               ACCEPT ENTRA
               MOVE ENTRA TO CAB-IMPORTE
               EVALUATE CODIGO
                   WHEN 1
                       PERFORM CAB-ABRIR
                   WHEN 2
                       PERFORM CAB-COBRAR
                   WHEN 3
                       PERFORM CAB-PAGAR
                   WHEN 4
                       MOVE ENTRA TO CAB-VECES
                       MOVE 0 TO CAB-ESTADO
                   WHEN 5
                       PERFORM CAB-CUADRAR
                   WHEN 9
                       PERFORM CAB-CUADRAR
                       MOVE 1 TO FIN
                   WHEN OTHER
                       MOVE 3 TO CAB-ESTADO
               END-EVALUATE
               DISPLAY CAB-ESTADO
               DISPLAY CAB-SALDO
           END-PERFORM.
           STOP RUN.
           COPY CABLIBRO.
