/* Structs: copia, anidados, arrays de struct, por valor y devueltos. */
#include <stdio.h>
struct punto { int x, y; };
struct caja { struct punto a, b; char nombre[8]; };
static struct punto suma(struct punto p, struct punto q) {
    struct punto r;
    r.x = p.x + q.x;
    r.y = p.y + q.y;
    return r;
}
static int area(const struct caja *c) { return (c->b.x - c->a.x) * (c->b.y - c->a.y); }
int main(void) {
    struct punto p = {3, 4}, q = {10, 20};
    struct punto r = suma(p, q);
    printf("suma %d %d\n", r.x, r.y);
    struct caja c = {{1, 2}, {6, 9}, "caja"};
    struct caja d = c;
    d.a.x = 0;
    printf("area %d %d %s\n", area(&c), area(&d), d.nombre);
    struct punto arr[3];
    for (int i = 0; i < 3; i++) { arr[i].x = i; arr[i].y = i * i; }
    printf("arr %d %d\n", arr[2].x, arr[2].y);
    printf("medida %d\n", (int)sizeof(struct caja));
    return 0;
}
