/* Recursion, punteros a funcion, un array de ellos y una retrollamada. */
#include <stdio.h>
static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static int ack(int m, int n) {
    if (m == 0) return n + 1;
    if (n == 0) return ack(m - 1, 1);
    return ack(m - 1, ack(m, n - 1));
}
static int doble(int x) { return 2 * x; }
static int cuadrado(int x) { return x * x; }
static int aplicar(int (*f)(int), int x) { return f(x); }
int main(void) {
    printf("fib %d\n", fib(20));
    printf("ack %d\n", ack(2, 3));
    int (*tabla[2])(int) = {doble, cuadrado};
    printf("tabla %d %d\n", tabla[0](7), tabla[1](7));
    printf("aplicar %d\n", aplicar(cuadrado, 12));
    printf("vuelve %d\n", fib(10) & 0x7F);
    return 0;
}
