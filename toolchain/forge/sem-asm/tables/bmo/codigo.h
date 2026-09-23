/* codigo.h -- ESCRIBIR CODIGO Y DESPUES EJECUTARLO, sin tener nunca las dos.
 *
 * == El hueco que tapa ==
 *
 * Hasta el 2026-09-23 una app de BMO-X solo podia ejecutar el codigo que venia
 * dentro de su `.bex`. Toda la memoria que pedia nacia escribible y NO
 * ejecutable (NX), y no habia forma de cambiarlo. Eso cierra la puerta a lo que
 * genera codigo al vuelo: un sombreador SPIR-V pasado a x86-64 (VERRANO), un
 * interprete que compila lo caliente, una expresion regular compilada.
 *
 * La salida facil --dejar pedir memoria escribible Y ejecutable-- es la que
 * este sistema no toma: una pagina con W y X a la vez es donde un fallo de
 * memoria se convierte en ejecutar lo que el atacante escribio.
 *
 * == W^X con transicion: el bloque cambia de estado UNA vez ==
 *
 *     bmo_codigo_pedir(bytes)      bloque nuevo: escribible, NO ejecutable
 *     ... escribir las instrucciones ...
 *     bmo_codigo_sellar(&c)        desde aqui: ejecutable, NO escribible
 *     ... llamarlo ...
 *
 * Sellar es **irreversible**. Para regenerar se pide otro bloque. Y no hay
 * instante con las dos: cada pagina cambia de W a X de un solo golpe.
 *
 * ** No es solo la tabla de paginas. El kernel escribe en bloques ajenos por
 * su cuenta --`fread` sobre el bloque, la copia de la orquesta, un DMA-- y
 * presta bloques con escritura. Con un bloque sellado, los cuatro se niegan.
 *
 * == Por que un bloque PROPIO y no un trozo del monton ==
 *
 * Porque se sella el BLOQUE entero, y el monton es un bloque: sellarlo dejaria
 * a `malloc` escribiendo en paginas de solo lectura. El codigo va aparte, y
 * cuenta en el tope de peticiones por proceso igual que cualquier otro bloque.
 *
 * == Como se usa ==
 *
 *     BMO_CODIGO c;
 *     int (*f)();
 *     if (bmo_codigo_pedir(&c, 4096)) {
 *         c.bytes_ptr[0] = 0xB8; ...          (mov eax, 42 / ret)
 *         if (bmo_codigo_sellar(&c) == BMO_SELLAR_HECHO) {
 *             f = c.bytes_ptr;
 *             f();
 *         }
 *     }
 *
 * Ver `toolchain/lang/c/examples/sello_C.c`.
 *
 * -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  ROJO         salta a bytes que el programa escribio. Una
 *                        instruccion mal codificada no la caza el compilador:
 *                        la caza el procesador, en Ring 3, con la app muerta
 * [cuesta]  TAREA        un fallo mata la app que genero el codigo, y a nadie
 *                        mas: el sello es por proceso y por bloque
 * [riesgo]  SILENCIO     sellar puede decir que NO (bloque prestado, NX
 *                        apagado...). Quien salte sin mirar el motivo salta a
 *                        memoria que sigue sin permiso de ejecucion
 */
#ifndef BMO_CODIGO_H
#define BMO_CODIGO_H

#include <bmo/bmo.h>

/* Pedir un bloque de memoria. Operacion de TAREA. */
#define BMO_OP_MEMORIA_PEDIR 0x15

/* Donde empieza el bloque. Operacion sobre `KIND_MEMORIA`.
 *
 * Se guarda con `#ifndef` por si otra cabecera lo trae antes: las dos
 * definiciones dicen lo mismo, y R13 lo comprueba. */
#ifndef BMO_MEM_BASE
#define BMO_MEM_BASE 0x01
#endif

/* Sellar: de datos a codigo. Operacion sobre `KIND_MEMORIA`. */
#define BMO_MEM_SELLAR 0x06

/* -- Lo que contesta sellar: 0, o POR QUE no --------------------------- */
/* Sellado: desde ahora se ejecuta y no se escribe. */
#define BMO_SELLAR_HECHO 0
/* Ese bloque no es tuyo, o ya lo soltaste. */
#define BMO_SELLAR_NO_ES_SUYO 1
/* Ya estaba sellado. Sellar no se repite ni se deshace. */
#define BMO_SELLAR_YA_SELLADO 2
/* Sigue prestado a otro proceso con escritura. Se suelta el prestamo antes. */
#define BMO_SELLAR_PRESTADO 3
/* El procesador tiene NX apagado: sin NX, sellar no garantizaria nada. */
#define BMO_SELLAR_SIN_NX 4
/* El kernel no pudo rehacer el mapeo; el bloque queda inutilizable. */
#define BMO_SELLAR_NO_REMAPEA 5

/* Un bloque para codigo. */
struct BMO_CODIGO {
    unsigned long long cap;        /* el handle; 0 = no hay bloque */
    unsigned char *bytes_ptr;      /* donde empieza, en TU espacio */
    unsigned long long bytes;      /* lo que se pidio */
};
typedef struct BMO_CODIGO BMO_CODIGO;

/* Pide un bloque de `bytes` para escribir codigo en el. 1 = hay, 0 = no.
 *
 * Nace escribible y NO ejecutable, como toda la memoria que el kernel da. */
int bmo_codigo_pedir(BMO_CODIGO *c, unsigned long long bytes) {
    unsigned long long cap;
    unsigned long long base;

    c->cap = 0;
    c->bytes_ptr = 0;
    c->bytes = 0;
    if (bytes == 0) {
        return 0;
    }
    cap = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_MEMORIA_PEDIR, bytes, 0, 0);
    if (cap == 0) {
        return 0;
    }
    base = bmo_valor(cap, BMO_MEM_BASE, 0, 0, 0);
    if (base == 0) {
        return 0;
    }
    c->cap = cap;
    c->bytes_ptr = (unsigned char *)base;
    c->bytes = bytes;
    return 1;
}

/* Sella el bloque: desde aqui se ejecuta y ya no se escribe.
 *
 * Devuelve `BMO_SELLAR_HECHO` (0) o el motivo. ** El motivo viaja en los 32
 * bits altos de la respuesta, y si el codigo no es exito pero el motivo sale 0
 * --un handle que ni siquiera es de memoria-- se contesta `NO_ES_SUYO`: un
 * "no" nunca puede leerse aqui como "si". */
unsigned long long bmo_codigo_sellar(BMO_CODIGO *c) {
    unsigned long long r;
    unsigned long long motivo;

    r = bmo_codigo(c->cap, BMO_MEM_SELLAR, 0, 0, 0);
    if ((r & 0xFFFFFFFF) == 0) {
        return BMO_SELLAR_HECHO;
    }
    motivo = r >> 32;
    if (motivo == 0) {
        return BMO_SELLAR_NO_ES_SUYO;
    }
    return motivo;
}

#endif /* BMO_CODIGO_H */
