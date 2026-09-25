/* switch que cae, goto, do-while, break y continue anidados, y la coma. */
#include <stdio.h>
int main(void) {
    int t = 0;
    for (int i = 0; i < 6; i++) {
        switch (i) {
        case 0: t += 1;
        case 1: t += 10; break;
        case 3: continue;
        default: t += 100;
        }
        t += 1000;
    }
    printf("switch %d\n", t);
    int n = 0;
    do { n += 3; } while (n < 10);
    printf("do %d\n", n);
    int k = 0;
    for (int i = 0; i < 10; i++) {
        for (int j = 0; j < 10; j++) {
            if (j == 3) break;
            if ((i + j) % 2) continue;
            k++;
        }
    }
    printf("anidado %d\n", k);
    int c = (k++, k * 2);
    printf("coma %d\n", c);
    int g = 0;
otra:
    g++;
    if (g < 5) goto otra;
    printf("goto %d\n", g);
    return 0;
}
