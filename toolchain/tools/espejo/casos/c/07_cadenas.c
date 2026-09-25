/* <string.h>: lo que pide cualquier juego para sus nombres y buferes. */
#include <stdio.h>
#include <string.h>
int main(void) {
    char a[32];
    strcpy(a, "hola");
    strcat(a, " mundo");
    printf("%s %d\n", a, (int)strlen(a));
    printf("cmp %d %d\n", strcmp("abc", "abc") == 0, strcmp("abc", "abd") < 0);
    char b[8];
    memset(b, 'x', 7);
    b[7] = 0;
    printf("%s\n", b);
    char c[8];
    memcpy(c, "espejo", 7);
    printf("%s %c\n", c, c[2]);
    printf("ncmp %d\n", strncmp("vkquake", "vkquack", 5) == 0);
    return 0;
}
