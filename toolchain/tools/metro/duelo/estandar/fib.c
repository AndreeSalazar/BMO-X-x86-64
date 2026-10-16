#include <stdio.h>
static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main(void) {
    int n = 0;
    if (scanf("%d", &n) != 1) return 1;
    printf("%d\n", fib(n));
    return 0;
}
