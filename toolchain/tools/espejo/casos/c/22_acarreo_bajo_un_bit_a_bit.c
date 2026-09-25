/* Una suma de 32 bits que se pasa tiene que PERDER el acarreo, tambien cuando
 * lo siguiente es un `|`, un `&` o un `%`. El `&` de dentro no tenia tipo, asi
 * que la suma tampoco, y el bit 33 llegaba vivo al `|` y de ahi a un `%` hecho
 * en 64. Lo encontro el espejo el 25-09 (el mismo programa al azar 38). */
#include <stdio.h>
static unsigned int h(void) { return 3033899948u; }
int main(void) {
    unsigned int l0 = 1623127984u;
    unsigned int v = 3033899948u;
    printf("%u\n", (h() + (l0 & 1734225868u)) | 1u);
    printf("%u\n", (v + (l0 & 1734225868u)) | 1u);
    printf("%u\n", (v + (l0 < 5u)) ^ 1u);
    printf("%u\n", 1000000007u % ((v + (l0 & 1734225868u)) | 1u));
    return 0;
}
