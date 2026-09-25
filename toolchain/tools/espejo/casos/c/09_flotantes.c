/* float y double: la cuenta, las conversiones y el redondeo hacia cero. Se
 * imprimen como enteros (x1000): lo que se compara es la CUENTA, no %f. */
#include <stdio.h>
int main(void) {
    double a = 1.5, b = 2.25;
    float f = 3.75f;
    printf("%d\n", (int)((a * b) * 1000.0));
    printf("%d\n", (int)((a / b) * 1000000.0));
    printf("%d\n", (int)(f * 1000.0f));
    printf("%d %d\n", (int)-2.9, (int)2.9);
    double s = 0.0;
    for (int i = 1; i <= 100; i++) s += 1.0 / i;
    printf("%d\n", (int)(s * 1000.0));
    printf("%d\n", a < b);
    float g = (float)7 / 2;
    printf("%d\n", (int)(g * 10));
    return 0;
}
