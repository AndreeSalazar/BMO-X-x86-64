/* malloc, una lista enlazada y free: la memoria dinamica de un juego. */
#include <stdio.h>
#include <stdlib.h>
struct nodo { int v; struct nodo *sig; };
int main(void) {
    struct nodo *cabeza = 0;
    for (int i = 1; i <= 10; i++) {
        struct nodo *n = (struct nodo *)malloc(sizeof(struct nodo));
        n->v = i * i;
        n->sig = cabeza;
        cabeza = n;
    }
    int t = 0, k = 0;
    while (cabeza) {
        struct nodo *s = cabeza->sig;
        t += cabeza->v;
        k++;
        free(cabeza);
        cabeza = s;
    }
    printf("%d %d\n", k, t);
    int *a = (int *)calloc(16, sizeof(int));
    printf("%d\n", a[15]);
    free(a);
    return 0;
}
