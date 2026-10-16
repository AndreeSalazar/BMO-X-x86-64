/* EL DUELO (PLAN_TITAN_AFINADO): llenar 200000 bytes y sumarlos: una lista de bytes.
 * El MISMO programa que bytes.titan, por el emisor de C de la casa. */
#include <stdio.h>

static unsigned char a[200000];

int main(void) {
    int n = 200000;
    for (int i = 0; i < n; i++)
        a[i] = (unsigned char)(i * 7);
    long s = 0;
    for (int i = 0; i < n; i++)
        s += a[i];
    printf("%ld\n", s);
    return 0;
}
