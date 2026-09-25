/* La CAPA 3 de vkQuake 0.50: un puntero a funcion que devuelve float. El valor
 * vuelve en xmm0, no en rax. */
#include <stdio.h>
static float mitad(float x) { return x * 0.5f; }
static double tercio(double x) { return x / 3.0; }
int main(void) {
    float (*f)(float) = mitad;
    double (*g)(double) = tercio;
    printf("%d %d\n", (int)(f(9.0f) * 100.0f), (int)(g(10.0) * 1000.0));
    return 0;
}
