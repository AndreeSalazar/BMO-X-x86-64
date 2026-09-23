/* sello_C.bex -- el programa que ESTRENA el sello: escribir codigo, sellarlo,
 * ejecutarlo, y comprobar que ya no se puede escribir.
 *
 * Es la prueba de `MEM_OP_SELLAR` (W^X con transicion, 2026-09-23). Toda la
 * memoria que el kernel da nace escribible y NO ejecutable; sellar un bloque
 * la pasa a ejecutable y NO escribible, de una vez y para siempre.
 *
 * == Las cuatro pruebas, y cada una puede fallar sola ==
 *
 *   1. hay bloque       -- `bmo_codigo_pedir` devuelve un bloque aparte del
 *                          monton (sellar el monton dejaria a `malloc` sin
 *                          donde escribir).
 *   2. se sella         -- se escriben seis bytes, `mov eax, 42 ; ret`, y el
 *                          kernel contesta HECHO.
 *   3. se ejecuta       -- se llama como una funcion y devuelve 42. Si el sello
 *                          no hubiera puesto X, esto seria un fallo de pagina
 *                          por NX, no un numero.
 *   4. no se repite     -- sellar otra vez contesta YA_SELLADO (2), no exito.
 *
 * Y la QUINTA, que es la que importa y mata al programa a proposito:
 *
 *   5. no se escribe    -- se escribe un byte en el codigo sellado. Lo correcto
 *                          es un fallo de pagina en Ring 3 y la app muerta. Si
 *                          sale la linea "W^X NO SE CUMPLE", el sello mintio.
 *
 * == Lo que se espera ver ==
 *
 *   sello_C: bloque de codigo en 0x...
 *   sello_C: sellado
 *   sello_C: el codigo sellado devolvio 42
 *   sello_C: sellar otra vez contesta 2 (YA_SELLADO)
 *   sello_C: ahora escribo en el codigo sellado; lo correcto es morir aqui
 *   (y el fallo de pagina de Ring 3 en CABINA)
 *
 * == Como se lanza ==
 *
 *   c/sello.bex     desde la caja Ejecutar del escritorio.
 *
 * Compilar:
 *   cargo run -p bmo-c-x86-64 -- toolchain/lang/c/examples/sello_C.c \
 *       -o sello.bex
 */
#include <bmo/bmo.h>
#include <bmo/codigo.h>

int main() {
    BMO_CODIGO c;
    int (*f)();
    unsigned long long r;
    int v;

    if (bmo_codigo_pedir(&c, 4096) == 0) {
        printf("sello_C: el kernel no dio bloque de codigo\n");
        return 1;
    }
    printf("sello_C: bloque de codigo en 0x%llx\n", (unsigned long long)c.bytes_ptr);

    /* mov eax, 42 ; ret */
    c.bytes_ptr[0] = 0xB8;
    c.bytes_ptr[1] = 42;
    c.bytes_ptr[2] = 0;
    c.bytes_ptr[3] = 0;
    c.bytes_ptr[4] = 0;
    c.bytes_ptr[5] = 0xC3;

    r = bmo_codigo_sellar(&c);
    if (r != BMO_SELLAR_HECHO) {
        printf("sello_C: el kernel NO sello (motivo %llu)\n", r);
        return 2;
    }
    printf("sello_C: sellado\n");

    f = c.bytes_ptr;
    v = f();
    printf("sello_C: el codigo sellado devolvio %d\n", v);
    if (v != 42) {
        printf("sello_C: esperaba 42\n");
        return 3;
    }

    r = bmo_codigo_sellar(&c);
    printf("sello_C: sellar otra vez contesta %llu (YA_SELLADO)\n", r);
    if (r != BMO_SELLAR_YA_SELLADO) {
        printf("sello_C: esperaba %d\n", BMO_SELLAR_YA_SELLADO);
        return 4;
    }

    printf("sello_C: ahora escribo en el codigo sellado; lo correcto es morir aqui\n");
    c.bytes_ptr[0] = 0x90;
    printf("sello_C: W^X NO SE CUMPLE -- se escribio en codigo sellado\n");
    return 5;
}
