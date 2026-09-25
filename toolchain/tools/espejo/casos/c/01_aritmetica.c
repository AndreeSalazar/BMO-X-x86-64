/* Enteros: division y resto con negativos, desplazamientos, la vuelta del
 * sin signo, y el `char` con signo de x86-64 (System V). */
#include <stdio.h>
int main(void) {
    int a = -17, b = 5;
    printf("div %d mod %d\n", a / b, a % b);
    printf("div %d mod %d\n", 17 / -5, 17 % -5);
    unsigned int u = 0;
    u = u - 1u;
    printf("vuelta %u\n", u);
    printf("desp %d %d\n", 1 << 20, -64 >> 3);
    printf("hex %x %X\n", 48879u, 51966u);
    char c = (char)200;
    printf("char %d\n", (int)c);
    unsigned char uc = (unsigned char)300;
    printf("uchar %u\n", (unsigned int)uc);
    short s = (short)40000;
    printf("short %d\n", (int)s);
    printf("prec %d\n", 2 + 3 * 4 - 8 / 2 % 3);
    return 0;
}
