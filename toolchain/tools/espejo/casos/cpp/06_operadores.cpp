// Operadores sobrecargados.
#include <stdio.h>
struct V2 {
    int x, y;
    V2 operator+(const V2 &o) const { return V2{x + o.x, y + o.y}; }
    V2 operator*(int k) const { return V2{x * k, y * k}; }
    bool operator==(const V2 &o) const { return x == o.x && y == o.y; }
};
int main() {
    V2 a{1, 2}, b{10, 20};
    V2 c = (a + b) * 3;
    printf("%d %d %d\n", c.x, c.y, (int)(c == V2{33, 66}));
    return 0;
}
