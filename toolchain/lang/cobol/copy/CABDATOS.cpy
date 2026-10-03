      * CABDATOS -- los DATOS de BANK CAT (va en WORKING-STORAGE).
      *
      * La moneda CAB en CENTIMOS ENTEROS: COMP-3, como los datos de un
      * banco. Nada de coma flotante: 19.99 x 3 es 59.97 y no 59.969999.
      *
      * CAB-SIN-DISCO (5): el movimiento se hizo en memoria pero el libro
      * no se pudo guardar (BC4). Se dice: no se calla.
      *
      * Es la "firma" de la libreria: el programa pone el importe en
      * CAB-IMPORTE (y si hace falta las veces en CAB-VECES), hace PERFORM
      * de un parrafo de CABLIBRO y mira CAB-ESTADO. Sin CALL todavia
      * (6.2 de PLAN_BANCA), los datos compartidos SON los parametros.
       01  CAB-SALDO          PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-INICIAL        PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-IMPORTE        PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-DEBE           PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-HABER          PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-PRUEBA         PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-VECES          PIC S9(5) VALUE 1.
       01  CAB-ASIENTOS       PIC S9(7) VALUE 0.
      * Los TEMPORALES de un asiento: se calcula todo aqui y solo se
      * apunta si TODO cupo (o entra entero, o no entra).
       01  CAB-T-HABER        PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-T-DEBE         PIC S9(13)V99 COMP-3 VALUE 0.
       01  CAB-T-ASIENTOS     PIC S9(7) VALUE 0.
       01  CAB-ESTADO         PIC 9 VALUE 0.
           88  CAB-HECHO          VALUE 0.
           88  CAB-NO-CABE        VALUE 1.
           88  CAB-SIN-SALDO      VALUE 2.
           88  CAB-IMPORTE-MALO   VALUE 3.
           88  CAB-DESCUADRE      VALUE 4.
           88  CAB-SIN-DISCO      VALUE 5.
