/* 64 bits: producto, division y los limites. */
#include <stdio.h>
int main(void) {
    unsigned long long x = 0xFFFFFFFFull;
    x = x * x;
    printf("%llu\n", x);
    long long m = -9000000000000000000ll;
    printf("%lld\n", m / 7);
    printf("%lld\n", m % 1000000007ll);
    unsigned long long d = 18446744073709551615ull;
    printf("%llu %llu\n", d / 3ull, d % 1000ull);
    long long s = 1;
    for (int i = 0; i < 62; i++) s *= 2;
    printf("%lld\n", s);
    return 0;
}
