/* Codigo ANTES del primer `case`: en C no se ejecuta nunca (el salto va
 * directo a una etiqueta, o fuera). Lo encontro el reductor del espejo el
 * 25-09: BMO lo ejecutaba. */
#include <stdio.h>
int main(void) {
    int g = 7;
    switch (1) {
        g = 99;
    case 1:
        break;
    }
    int h = 5;
    switch (h & 3) {
        h = 100;
    }
    printf("%d %d\n", g, h);
    return 0;
}
