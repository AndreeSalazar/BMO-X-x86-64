/* `c ? a : b` vale el tipo COMUN de las dos ramas, no el de la primera. Con
 * `0u` delante y un valor de 64 bits detras, BMO lo daba por `unsigned int` y
 * quien lo sumaba recortaba a 32. Lo encontro el espejo el 25-09 reduciendo el
 * programa al azar 38. */
#include <stdio.h>
int main(void) {
    unsigned long long big = 15796813855956853689ull;
    printf("%llu\n", (0u > 0u ? 0u : big) + 0u);
    printf("%llu\n", (1 ? 0u : big) + 1u);
    printf("%llu\n", (big > 5ull ? 7u : big) + 0u);
    return 0;
}
