#include <stdio.h>
#include <stdlib.h>
int main(void) {
    int n = 0;
    if (scanf("%d", &n) != 1) return 1;
    unsigned char *a = malloc(n);
    for (int i = 0; i < n; i++) a[i] = (unsigned char)(i * 7);
    long s = 0;
    for (int i = 0; i < n; i++) s += a[i];
    printf("%ld\n", s);
    free(a);
    return 0;
}
