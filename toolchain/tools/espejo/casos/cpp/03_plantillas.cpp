// Plantillas de funcion y de clase.
#include <stdio.h>
template <typename T> T mayor(T a, T b) { return a > b ? a : b; }
template <typename T, int N> struct Pila {
    T datos[N];
    int n = 0;
    void poner(T x) { datos[n++] = x; }
    T quitar() { return datos[--n]; }
};
int main() {
    printf("%d %d\n", mayor(3, 9), (int)mayor('a', 'z'));
    Pila<int, 8> p;
    p.poner(1); p.poner(2); p.poner(3);
    // Uno detras de otro: dos llamadas en los argumentos de printf no tienen
    // orden fijo (GCC y Clang lo hacen al reves).
    int a = p.quitar();
    int b = p.quitar();
    printf("%d %d\n", a, b);
    return 0;
}
