#include <iostream>
int main() {
    int n = 0;
    if (!(std::cin >> n)) return 1;
    int c = 0;
    for (int k = 2; k < n; k++) {
        bool p = true;
        for (int d = 2; d * d <= k; d++)
            if (k % d == 0) { p = false; break; }
        c += p;
    }
    std::cout << c << '\n';
}
