/* Una cuenta de constantes SIN signo que da la vuelta, comparada. Plegada al
 * compilar salia como inmediato con signo, y `x > (a - b)` con constantes daba
 * lo contrario que con variables. Lo encontro el espejo el 25-09 reduciendo el
 * programa al azar 479. */
#include <stdio.h>
int main(void) {
    unsigned int x = 3000000000u;
    printf("%u\n", (unsigned int)(x > (424671700u - 2218538477u)));
    printf("%u\n", (unsigned int)((x > (424671700u - 2218538477u) ? 0u : 5u) & 3u));
    printf("%u\n", (unsigned int)(x < (1u - 2u)));
    return 0;
}
