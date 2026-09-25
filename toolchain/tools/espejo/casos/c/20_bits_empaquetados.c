/* Lo que MIDE una estructura con campos de bits. GCC y Clang los empaquetan en
 * una palabra (4 bytes aqui); BMO les da su tipo entero a cada uno (16) y lo
 * dice en BRECHA.md: el VALOR ya es el de C (`04_uniones_bits.c`), la
 * DISPOSICION no. Importa para leer un formato binario ajeno. */
#include <stdio.h>
struct banderas { unsigned int a : 3; unsigned int b : 5; unsigned int c : 8; unsigned int d : 16; };
int main(void) {
    printf("medida %d\n", (int)sizeof(struct banderas));
    return 0;
}
