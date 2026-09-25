// La cabecera de C++ de printf: <cstdio>. Los demas casos usan <stdio.h>, que
// tambien es C++, para medir el LENGUAJE; este mide la cabecera.
#include <cstdio>
int main() {
    std::printf("%d\n", 42);
    return 0;
}
