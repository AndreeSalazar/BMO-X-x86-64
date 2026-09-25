/* Uniones y campos de bits: el mismo sitio visto de dos formas, y bits
 * empaquetados que se leen igual que se escribieron. */
#include <stdio.h>
union u { unsigned int n; unsigned char b[4]; };
struct banderas { unsigned int a : 3; unsigned int b : 5; unsigned int c : 8; unsigned int d : 16; };
int main(void) {
    union u x;
    x.n = 0x11223344u;
    printf("bytes %x %x %x %x\n", x.b[0], x.b[1], x.b[2], x.b[3]);
    struct banderas f;
    f.a = 5; f.b = 17; f.c = 200; f.d = 40000;
    printf("bits %u %u %u %u\n", f.a, f.b, f.c, f.d);
    f.a = 9;
    printf("corta %u\n", f.a);
    printf("medida %d\n", (int)sizeof(struct banderas));
    return 0;
}
