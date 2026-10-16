#include <stdio.h>
#include <stdlib.h>
int main(void) {
    int n = 0;
    if (scanf("%d", &n) != 1) return 1;
    int *a = malloc(n * sizeof(int));
    for (int i = 0; i < n; i++) a[i] = (i * 7919) % 1000;
    for (int i = 1; i < n; i++) {
        int v = a[i], j = i - 1;
        while (j >= 0 && a[j] > v) { a[j + 1] = a[j]; j--; }
        a[j + 1] = v;
    }
    printf("%d %d\n", a[0], a[n - 1]);
    free(a);
    return 0;
}
