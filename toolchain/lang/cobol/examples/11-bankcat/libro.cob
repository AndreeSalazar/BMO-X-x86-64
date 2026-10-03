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
      *             4 descuadre              5 hecho, pero NO se guardo
      *
      * * LA LIBRERIA. Todo lo de dinero viene de dos copybooks de
      * `toolchain/lang/cobol/copy`: CABDATOS (los datos) y CABLIBRO (los
      * parrafos). Este programa solo traduce ordenes en PERFORM.
      *
      * * EL LIBRO EN EL DISCO (BC4, 03-10): `bankcat.dat`, cinco cifras
      * (inicial, haber, debe, saldo, asientos). Se CARGA al nacer y se
      * REESCRIBE entero tras cada movimiento hecho: sin OPEN EXTEND ni I-O
      * (3.1 y 3.2 de PLAN_BANCA), escribir de nuevo es lo que hay, y el
      * libro es chico. ABRIR con un libro que ya existe NO lo pisa:
      * contesta su saldo.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LIBRO.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT LIBRO ASSIGN TO "bankcat.dat" FILE STATUS IS ST.
       DATA DIVISION.
       FILE SECTION.
       FD  LIBRO.
       01  R-CIFRA            PIC 9(13)V99.
       WORKING-STORAGE SECTION.
           COPY CABDATOS.
       01  ST                 PIC XX VALUE "00".
       01  CODIGO             PIC 9.
       01  ENTRA              PIC S9(13)V99.
       01  FIN                PIC 9 VALUE 0.
           88  SE-ACABO           VALUE 1.
       01  CARGADO            PIC 9 VALUE 0.
           88  HAY-LIBRO          VALUE 1.
       PROCEDURE DIVISION.
           PERFORM CARGAR.
           PERFORM UNTIL SE-ACABO
               ACCEPT CODIGO
               ACCEPT ENTRA
               MOVE ENTRA TO CAB-IMPORTE
               EVALUATE CODIGO
                   WHEN 1
                       IF HAY-LIBRO
                           PERFORM CAB-CUADRAR
                       ELSE
                           PERFORM CAB-ABRIR
                           PERFORM GUARDAR
                       END-IF
                   WHEN 2
                       PERFORM CAB-COBRAR
                       PERFORM GUARDAR
                   WHEN 3
                       PERFORM CAB-PAGAR
                       PERFORM GUARDAR
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

      * Cargar el libro, si hay. Sin fichero (35) es un libro nuevo.
       CARGAR.
           OPEN INPUT LIBRO.
           IF ST = "00"
               READ LIBRO
                   AT END MOVE 0 TO R-CIFRA
               END-READ
               MOVE R-CIFRA TO CAB-INICIAL
               READ LIBRO
                   AT END MOVE 0 TO R-CIFRA
               END-READ
               MOVE R-CIFRA TO CAB-HABER
               READ LIBRO
                   AT END MOVE 0 TO R-CIFRA
               END-READ
               MOVE R-CIFRA TO CAB-DEBE
               READ LIBRO
                   AT END MOVE 0 TO R-CIFRA
               END-READ
               MOVE R-CIFRA TO CAB-SALDO
               READ LIBRO
                   AT END MOVE 0 TO R-CIFRA
               END-READ
               MOVE R-CIFRA TO CAB-ASIENTOS
               CLOSE LIBRO
               MOVE 1 TO CARGADO
           END-IF.

      * Guardar el libro ENTERO, solo si el movimiento se hizo. Si el disco
      * dice que no, el estado es 5: hecho en memoria, NO guardado.
       GUARDAR.
           IF CAB-HECHO
               MOVE 1 TO CARGADO
               OPEN OUTPUT LIBRO
               MOVE CAB-INICIAL TO R-CIFRA
               WRITE R-CIFRA
               MOVE CAB-HABER TO R-CIFRA
               WRITE R-CIFRA
               MOVE CAB-DEBE TO R-CIFRA
               WRITE R-CIFRA
               MOVE CAB-SALDO TO R-CIFRA
               WRITE R-CIFRA
               MOVE CAB-ASIENTOS TO R-CIFRA
               WRITE R-CIFRA
               CLOSE LIBRO
               IF ST = "00"
                   MOVE 0 TO CAB-ESTADO
               ELSE
                   MOVE 5 TO CAB-ESTADO
               END-IF
           END-IF.

           COPY CABLIBRO.
