/* La CAPA 2 de vkQuake 0.50 (q_stdinc.h): la comprobacion al compilar con un
 * array de medida -1 si falla. Para 70 de sus 82 ficheros. */
#include <stdio.h>
#define COMPILE_TIME_ASSERT(name, x) typedef int dummy_##name[(x) * 2 - 1]
COMPILE_TIME_ASSERT(char, sizeof(char) == 1);
COMPILE_TIME_ASSERT(int, sizeof(int) == 4);
COMPILE_TIME_ASSERT(ptr, sizeof(void *) == 8);
int main(void) {
    printf("%d\n", (int)sizeof(dummy_char));
    return 0;
}
