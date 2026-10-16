#include <iostream>
#include <vector>
int main() {
    int n = 0;
    if (!(std::cin >> n)) return 1;
    std::vector<int> a;
    for (int i = 0; i < n; i++) a.push_back((i * 7919) % 1000);
    for (int i = 1; i < n; i++) {
        int v = a[i], j = i - 1;
        while (j >= 0 && a[j] > v) { a[j + 1] = a[j]; j--; }
        a[j + 1] = v;
    }
    std::cout << a[0] << ' ' << a[n - 1] << '\n';
}
