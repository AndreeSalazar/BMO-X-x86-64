/* && y || cortan: el lado derecho NO se evalua si no hace falta. */
#include <stdio.h>
static int llamadas = 0;
static int marca(int v) { llamadas++; return v; }
int main(void) {
    int r1 = marca(0) && marca(1);
    int r2 = marca(1) || marca(0);
    int r3 = marca(1) && marca(1) && marca(0) && marca(1);
    printf("%d %d %d %d\n", r1, r2, r3, llamadas);
    int x = 5;
    int y = x > 3 ? x < 10 ? 1 : 2 : 3;
    printf("%d %d\n", y, !x);
    return 0;
}
