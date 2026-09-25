/* `...` y <stdarg.h>: DOOM y Quake hacen asi su Sys_Error y su Con_Printf. */
#include <stdio.h>
#include <stdarg.h>
static int suma(int n, ...) {
    va_list ap;
    va_start(ap, n);
    int t = 0;
    for (int i = 0; i < n; i++) t += va_arg(ap, int);
    va_end(ap);
    return t;
}
int main(void) {
    printf("%d %d %d\n", suma(0), suma(3, 1, 2, 3), suma(6, 10, 20, 30, 40, 50, 60));
    return 0;
}
