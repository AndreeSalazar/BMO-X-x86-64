/* static local que recuerda, enums, const, y sizeof. */
#include <stdio.h>
enum color { ROJO, VERDE = 5, AZUL };
static int contador(void) { static int n = 0; return ++n; }
static const int tabla[] = {2, 3, 5, 7, 11, 13};
int main(void) {
    contador(); contador();
    printf("%d\n", contador());
    printf("%d %d %d\n", ROJO, VERDE, AZUL);
    printf("%d\n", (int)(sizeof(tabla) / sizeof(tabla[0])));
    printf("%d\n", tabla[4]);
    return 0;
}
