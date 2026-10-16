/* EL DUELO (PLAN_TITAN_AFINADO): contar los primos de 2 a 20000 por division: bucles, `%` y `break`.
 * El MISMO programa que primos.titan, por el emisor de C de la casa. */
#include <stdio.h>

int main(void) {
    int n = 20000, c = 0;
    for (int k = 2; k < n; k++) {
        int p = 1;
        for (int d = 2; d * d <= k; d++) {
            if (k % d == 0) {
                p = 0;
                break;
            }
        }
        c += p;
    }
    printf("%d\n", c);
    return 0;
}
