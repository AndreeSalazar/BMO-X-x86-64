/* El ABI de x86-64: cuanto mide cada tipo. System V (Linux) es LP64 y Win64
 * es LLP64: `long` mide 8 en uno y 4 en el otro. Este caso dice cual es BMO. */
#include <stdio.h>
int main(void) {
    printf("char %d short %d int %d long %d llong %d ptr %d\n",
        (int)sizeof(char), (int)sizeof(short), (int)sizeof(int),
        (int)sizeof(long), (int)sizeof(long long), (int)sizeof(void *));
    printf("float %d double %d\n", (int)sizeof(float), (int)sizeof(double));
    return 0;
}
