#include <iostream>
static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main() {
    int n = 0;
    if (!(std::cin >> n)) return 1;
    std::cout << fib(n) << '\n';
}
