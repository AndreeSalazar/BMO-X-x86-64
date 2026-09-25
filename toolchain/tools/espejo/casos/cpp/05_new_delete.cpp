// new, delete y el orden de los destructores.
#include <stdio.h>
static int orden = 0;
struct Pieza {
    int id;
    Pieza(int i) : id(i) { printf("nace %d\n", id); }
    ~Pieza() { printf("muere %d en %d\n", id, ++orden); }
};
int main() {
    Pieza *p = new Pieza(1);
    {
        Pieza a(2);
        Pieza b(3);
    }
    delete p;
    int *v = new int[4];
    for (int i = 0; i < 4; i++) v[i] = i * i;
    printf("%d\n", v[3]);
    delete[] v;
    return 0;
}
