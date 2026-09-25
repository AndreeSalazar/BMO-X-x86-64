/* El preprocesador: macros con argumentos, # y ##, #if con aritmetica. */
#include <stdio.h>
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define CADENA(x) #x
#define PEGAR(a, b) a##b
#define VERSION 50
int main(void) {
    int PEGAR(val, 1) = 7;
    printf("%d %s\n", MAX(val1, 3), CADENA(espejo));
#if VERSION >= 50 && defined(MAX)
    printf("nuevo\n");
#else
    printf("viejo\n");
#endif
#ifdef NO_EXISTE
    printf("mal\n");
#endif
    printf("%d\n", __LINE__ > 0);
    return 0;
}
