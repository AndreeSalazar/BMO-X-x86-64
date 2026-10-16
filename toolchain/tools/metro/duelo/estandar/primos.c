#include <stdio.h>
int main(void) {
    int n = 0;
    if (scanf("%d", &n) != 1) return 1;
    int c = 0;
    for (int k = 2; k < n; k++) {
        int p = 1;
        for (int d = 2; d * d <= k; d++) {
            if (k % d == 0) { p = 0; break; }
        }
        c += p;
    }
    printf("%d\n", c);
    return 0;
}
