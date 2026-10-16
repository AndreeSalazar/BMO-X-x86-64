/* EL DUELO (PLAN_TITAN_AFINADO): ordenar 600 por insercion: indices y un while con `and`.
 * El MISMO programa que ordenar.titan, por el emisor de C de la casa. */
#include <stdio.h>

int main(void) {
    int a[600];
    int n = 600;
    for (int i = 0; i < n; i++)
        a[i] = (i * 7919) % 1000;
    for (int i = 1; i < n; i++) {
        int v = a[i];
        int j = i - 1;
        while (j >= 0 && a[j] > v) {
            a[j + 1] = a[j];
            j--;
        }
        a[j + 1] = v;
    }
    printf("%d %d\n", a[0], a[n - 1]);
    return 0;
}
