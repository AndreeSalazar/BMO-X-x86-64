/* Punteros: aritmetica, diferencia, puntero a puntero, arrays 2D. */
#include <stdio.h>
int main(void) {
    int v[6] = {10, 20, 30, 40, 50, 60};
    int *p = v + 2;
    int **pp = &p;
    printf("%d %d %d\n", *p, *(p + 3), p[-1]);
    printf("dif %d\n", (int)(&v[5] - p));
    **pp = 99;
    printf("v2 %d\n", v[2]);
    int m[3][4];
    for (int i = 0; i < 3; i++)
        for (int j = 0; j < 4; j++)
            m[i][j] = i * 10 + j;
    int *q = &m[0][0];
    printf("m %d %d %d\n", m[2][3], q[7], *(*(m + 1) + 2));
    const char *t = "espejo";
    const char *e = t;
    while (*e) e++;
    printf("largo %d\n", (int)(e - t));
    return 0;
}
