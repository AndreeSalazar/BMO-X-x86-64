/* sonido.h -- el SONIDO, en C.
 *
 * == Lo que esta capability entrega, y lo que NO ==
 *
 * Entrega el DERECHO a hacer ruido. No entrega un motor de audio: no hay
 * mezclador, no hay canales, no hay formato de muestras. El driver de HD Audio
 * --codec, DMA, anillo de buffers-- es la casilla 5.1 de `docs/plan/PLAN_DOOM.md` y
 * todavia no existe.
 *
 * El contrato va ANTES que el driver a proposito. Escribir el motor primero y
 * preguntarse despues quien tiene derecho a usarlo es como se acaba con un
 * sistema donde cualquier programa pita encima de cualquier otro. La pantalla
 * ya aprendio esto: `KIND_FRAMEBUFFER` existia antes que el compositor.
 *
 * == Reclamarlo es EXCLUSIVO ==
 *
 * Un solo proceso lo tiene a la vez, igual que la pantalla y por el mismo
 * motivo: dos propietarios escribiendo en el mismo aparato no es mezclar, es ruido.
 * Mezclar es un trabajo con nombre y le toca a Ring 3.
 *
 * Si otro lo tiene, `bmo_sonido_reclamar()` devuelve 0. Hay que comprobarlo:
 * un programa que da por hecho que lo tiene pita al vacio y parece un altavoz
 * roto.
 *
 * == Lo que suena hoy, dicho sin adornos ==
 *
 * El altavoz del PC, y puede que ni eso. El puerto que lo controla existe en
 * todo x86; el zumbador fisico, no -- muchas placas modernas traen el cabezal
 * SPKR sin nada conectado, y desde el kernel no hay forma de saberlo. Por eso
 * `bmo_sonido_aparatos()` dice que hay CAMINO, no que se vaya a oir algo.
 *
 * == Y PITAR BLOQUEA ==
 *
 * Mientras dura el tono, el nucleo no hace otra cosa: el altavoz del PC no
 * tiene interrupcion que avise de que acabo. El kernel recorta a
 * BMO_SONIDO_MAX_MS, asi que pedir mas no cuelga la maquina -- pero tampoco
 * suena mas. Es una propiedad del altavoz, no del contrato: con HDA se llena un
 * anillo y el DMA lo consume solo.
 *
 * -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  AMARILLO     reclamar es EXCLUSIVO, igual que la pantalla y por el
 *                        mismo motivo: dos propietarios en el mismo aparato no es
 *                        mezclar, es ruido
 * [cuesta]  APARATO      un proceso que reclama y muere sin soltar deja el
 *                        altavoz sin propietario hasta que alguien reinicie
 * [riesgo]  SILENCIO     `aparatos()` dice que hay CAMINO, no que se oiga: hay
 *                        placas con el cabezal SPKR sin nada conectado y desde
 *                        aqui no se sabe
 */
#ifndef BMO_SONIDO_H
#define BMO_SONIDO_H

#include <bmo/bmo.h>

/* -- Operaciones sobre el handle de sonido ----------------------------- */
/* Que aparatos hay. Mapa de bits, ver BMO_APARATO_*. */
#define BMO_SONIDO_APARATO 0x01
/* Pitar: a0 = Hz, a1 = ms. Devuelve los ms que de verdad sonaron. */
#define BMO_SONIDO_PITAR 0x02
/* Volumen 0..100. En el altavoz del PC son DOS escalones, no cien: el volumen
 * se consigue cambiando el modo del temporizador y no hay mas modos. */
#define BMO_SONIDO_VOLUMEN 0x03
/* Callar ahora mismo. */
#define BMO_SONIDO_CALLAR 0x04
/* **EL TUBO ISOCRONO**, o sea lo unico que de verdad SUENA en esta maquina
 * (2026-09-22). `a0` es el campo (BMO_TUBO_*) y `a1` el dato cuando lo lleva.
 *
 * *** SIN ESTO, UN PROGRAMA DE C NO PODIA HACER RUIDO. Esta cabecera es de
 * antes de que el tubo existiera: tenia pitar, volumen y callar, y el altavoz
 * del PC de esta placa no tiene zumbador conectado. O sea que un programa de C
 * --DOOM incluido-- solo podia pitar al vacio. El kernel ofrece el contrato
 * entero desde A4 y `musica.inti` lo usa; lo que faltaba era decirlo AQUI. */
#define BMO_SONIDO_TUBO 0x05

/* -- Lo que puede contestar BMO_SONIDO_APARATO ------------------------- */
#define BMO_APARATO_ALTAVOZ 1
#define BMO_APARATO_HDA 2
/* Audifono USB Audio con control de volumen. **En esta maquina es el unico
 * aparato que suena de verdad**: la placa no trae zumbador. */
#define BMO_APARATO_USB 4

/* -- Los campos del TUBO (BMO_SONIDO_TUBO) ----------------------------- */
/* Hay tubo? Es la PRIMERA pregunta de cualquiera que quiera sonar. */
#define BMO_TUBO_ABIERTO 0
/* Armar: empezar a empujar tramas (silencio si no hay nada). Es TRAFICO --250
 * latidos por segundo-- y por eso no se enciende solo. */
#define BMO_TUBO_ARMAR 1
#define BMO_TUBO_CALLAR 2
/* Bytes por milisegundo. **De aqui salen los CANALES, y no hay otra forma**:
 * a 48.000 Hz, 192 = 48 muestras x 2 canales x 2 bytes. Suponer estereo es
 * justo el error que este numero existe para evitar. */
#define BMO_TUBO_BYTES_MS 3
/* La frecuencia que el aparato acepto, en Hz. NO se supone. */
#define BMO_TUBO_FRECUENCIA 4
/* Tramas mandadas al bus. Tiene que SUBIR SOLA mientras algo suena. */
#define BMO_TUBO_ENCOLADAS 5
/* Tramas que el xHC no sirvio en su microtrama: la cifra que separa "suena
 * bien" de "chasquea". Es del BUS. */
#define BMO_TUBO_TARDE 6
#define BMO_TUBO_ARMADO 7
/* Prestar un bloque PROPIO al aparato: `a1` es su direccion. Los bytes los
 * mira el kernel en el bloque; una app no declara la medida de su memoria. */
#define BMO_TUBO_OFRECER 8
/* Hasta donde ha llenado la app, en bytes desde el principio del bloque. Solo
 * puede CRECER: retroceder es pisar lo que el aparato no ha leido, y se oye. */
#define BMO_TUBO_ESCRITO 9
/* Por donde va el APARATO. La distancia con `escrito` es lo que queda. */
#define BMO_TUBO_LEIDO 10
/* Lo escrito y aun no sonado. **Es el mando del productor**: si baja de una
 * trama, el aparato se queda sin nada y eso es un hueco. */
#define BMO_TUBO_PENDIENTES 11
/* Vueltas sin trama que mandar. Tiene que ser cero; si sube, el productor no
 * llega. Es de la APP, al reves que `tarde`. */
#define BMO_TUBO_HUECOS 12
#define BMO_TUBO_SOLTAR 13
/* **La medida del ANILLO**: `bytes` del bloque redondeado hacia abajo a un
 * numero entero de tramas. Es DONDE hay que dar la vuelta.
 *
 * *** SIN ESTE NUMERO EL ANILLO NO ERA UN ANILLO (2026-09-22). El bufer daba
 * la vuelta en el kernel y la app tenia que adivinar donde; y como `bytes` no
 * suele ser multiplo de una trama (4.096 entre 192 son 21 y sobran 64), el
 * corte caia en mitad de una muestra. Ahora las dos partes dan la vuelta en el
 * mismo sitio, y una trama no cruza nunca el final. */
#define BMO_TUBO_ANILLO 14

/* Tope de duracion de un pitido, en ms. El kernel recorta igual; esto solo
 * evita la sorpresa de pedir 5000 y recibir 250. */
#define BMO_SONIDO_MAX_MS 250

/* -- La capability ----------------------------------------------------- */

/* Reclamar el sonido. Devuelve el handle, o 0 si ya lo tiene otro proceso. */
unsigned long long bmo_sonido_reclamar() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_SONIDO_RECLAMAR, 0, 0, 0);
}

/* Soltarlo y seguir vivo. Devuelve 0 si se solto.
 *
 * Existe desde el primer dia por lo que costo que faltara en la pantalla: alli
 * la unica forma de dejar de ser propietario era morir, y el escritorio no podia
 * prestarla ni queriendo. */
unsigned long long bmo_sonido_soltar() {
    return bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_SONIDO_SOLTAR, 0, 0, 0);
}

/* Que aparatos hay. Un bit puesto dice que hay camino, no que se oiga. */
unsigned long long bmo_sonido_aparatos(unsigned long long cap) {
    return bmo_valor(cap, BMO_SONIDO_APARATO, 0, 0, 0);
}

/* Pitar. Devuelve los ms que de verdad sonaron (recortados a MAX_MS). */
unsigned long long bmo_sonido_pitar(unsigned long long cap,
                                    unsigned long long hz,
                                    unsigned long long ms) {
    return bmo_valor(cap, BMO_SONIDO_PITAR, hz, ms, 0);
}

/* Volumen 0..100. Devuelve el que quedo puesto. */
unsigned long long bmo_sonido_volumen(unsigned long long cap,
                                      unsigned long long v) {
    return bmo_valor(cap, BMO_SONIDO_VOLUMEN, v, 0, 0);
}

/* Callar. */
void bmo_sonido_callar(unsigned long long cap) {
    bmo_codigo(cap, BMO_SONIDO_CALLAR, 0, 0, 0);
}

/* -- El TUBO: las muestras de verdad ----------------------------------- */

/* Una pregunta (o una orden) al tubo. `dato` va en las que lo llevan
 * (OFRECER y ESCRITO); en las demas se pone 0.
 *
 * == El camino entero, en siete lineas ==
 *
 *     cap = bmo_sonido_reclamar();
 *     if (!bmo_tubo(cap, BMO_TUBO_ABIERTO, 0)) { no hay audifono }
 *     bytes_ms = bmo_tubo(cap, BMO_TUBO_BYTES_MS, 0);   // 192 -> estereo 16
 *     hz       = bmo_tubo(cap, BMO_TUBO_FRECUENCIA, 0); // 48000
 *     bmo_tubo(cap, BMO_TUBO_OFRECER, (unsigned long long) mi_bloque);
 *     bmo_tubo(cap, BMO_TUBO_ARMAR, 0);
 *     ... escribir muestras y decir hasta donde con BMO_TUBO_ESCRITO ...
 *
 * == Los DOS numeros que cruzan, y por que son dos ==
 *
 *     escrito   hasta donde ha llenado la APP      (solo crece)
 *     leido     por donde va el APARATO            (lo dice el kernel)
 *
 * Con esos dos y un bloque en medio no hace falta una puerta por muestra: la
 * app escribe delante del aparato y el aparato come detras de la app. Cero
 * copias --el controlador lee la memoria de la app por DMA-- y una puerta por
 * vuelta, no por trama. */
unsigned long long bmo_tubo(unsigned long long cap,
                            unsigned long long campo,
                            unsigned long long dato) {
    return bmo_valor(cap, BMO_SONIDO_TUBO, campo, dato, 0);
}

#endif /* BMO_SONIDO_H */
