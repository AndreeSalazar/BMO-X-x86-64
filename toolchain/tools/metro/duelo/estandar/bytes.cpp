#include <iostream>
#include <vector>
#include <cstdint>
int main() {
    int n = 0;
    if (!(std::cin >> n)) return 1;
    std::vector<std::uint8_t> a;
    for (int i = 0; i < n; i++) a.push_back(static_cast<std::uint8_t>(i * 7));
    long s = 0;
    for (auto b : a) s += b;
    std::cout << s << '\n';
}
