/* bits_C.bex -- un programa que PIDE algo a la CPU, y lo dice (A6 del foco).
 *
 * `__popcnt` es POPCNT y `__tzcnt` es BMI1: no son x86-64 base. El compilador
 * lo apunta en el `.bex` (la linea CLASE_INSTRUCCIONES de sus requisitos), y
 * el kernel no lo admite en una CPU que no los tenga: con su nombre, antes de
 * la primera reserva, no con un `#UD` a mitad de camino.
 *
 * Esta en el banco del `metro` para que su oraculo tenga que morder: cada
 * instruccion que corre pide lo que pide, y el `.bex` tiene que declararlo.
 *
 * Compilar:
 *   cargo run -p bmo-c-x86-64 -- toolchain/lang/c/examples/bits_C.c -o bits_C.bex
 */
int main() {
    unsigned total = 0;
    for (unsigned v = 1; v < 64; v = v + 1) {
        total = total + __popcnt(v);
    }
    printf("unos de 1..63 = %d\n", (int)total);
    printf("ceros detras de 40 = %d\n", (int)__tzcnt(40));
    printf("bits termino ok\n");
    return 0;
}
