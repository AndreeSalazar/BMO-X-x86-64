// Herencia y funciones virtuales: el puntero a la base llama a la derivada.
#include <stdio.h>
struct Forma {
    virtual int area() const { return 0; }
    virtual ~Forma() {}
};
struct Rect : Forma {
    int w, h;
    Rect(int a, int b) : w(a), h(b) {}
    int area() const override { return w * h; }
};
struct Cuadrado : Rect {
    Cuadrado(int l) : Rect(l, l) {}
};
int main() {
    Rect r(3, 4);
    Cuadrado c(5);
    Forma *f[2] = {&r, &c};
    printf("%d %d\n", f[0]->area(), f[1]->area());
    return 0;
}
