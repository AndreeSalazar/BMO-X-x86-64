/* Etiquetas APILADAS: varios `case` seguidos para el mismo codigo. Lo encontro
 * el espejo el 25-09, reduciendo el programa al azar 4: BMO mandaba el 1 y el
 * 2 al `default`. Quake y DOOM lo usan en cada tabla de teclas y de estados. */
#include <stdio.h>
int main(void) {
    int r = 0, s = 0;
    for (unsigned int i = 0; i < 5u; i++) {
        switch (i) {
        case 0:
        case 1:
        case 2:
            s += 10;
            break;
        default:
            r++;
        }
    }
    printf("r %d s %d\n", r, s);
    return 0;
}
