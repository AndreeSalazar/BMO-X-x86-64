// Referencias, const y sobrecarga.
#include <stdio.h>
static void cambiar(int &x) { x *= 3; }
static int f(int x) { return x + 1; }
static int f(int x, int y) { return x * y; }
int main() {
    int a = 7;
    int &r = a;
    cambiar(r);
    const int &c = a;
    printf("%d %d %d %d\n", a, c, f(4), f(4, 5));
    return 0;
}
