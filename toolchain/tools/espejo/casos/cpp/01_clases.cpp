// Una clase con constructor, miembros y metodos const.
#include <stdio.h>
class Cuenta {
    int saldo;
public:
    Cuenta(int inicial) : saldo(inicial) {}
    void meter(int x) { saldo += x; }
    int ver() const { return saldo; }
};
int main() {
    Cuenta c(100);
    c.meter(25);
    c.meter(-40);
    printf("%d\n", c.ver());
    return 0;
}
