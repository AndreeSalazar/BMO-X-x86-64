/* EL DUELO (PLAN_TITAN_AFINADO): fib(22) recursivo: llamadas.
 * El MISMO programa que fib.titan, por el emisor de C de la casa. */
#include <stdio.h>

int fib(int n) {
    return n < 2 ? n : fib(n - 1) + fib(n - 2);
}

int main(void) {
    printf("%d\n", fib(22));
    return 0;
}
