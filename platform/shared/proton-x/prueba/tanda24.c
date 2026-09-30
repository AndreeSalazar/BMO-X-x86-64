/* tanda24.c -- el .exe de la TANDA 24 de Cyberpunk (30-09):
 * IsProcessorFeaturePresent. En el metal el juego lo pregunto justo antes
 * de rendirse (UnhandledExceptionFilter), y la casa solo decia SSE, SSE2 y
 * NX. Aqui cada respuesta se compara con lo que dice el propio CPUID (y
 * XGETBV para AVX, como mira Windows): cualquier maquina, cualquier Windows.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA int W IsProcessorFeaturePresent(DWORD f);

static unsigned fallos;

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle((DWORD)-11), t, n, &k, 0);
}

static void mira(int bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

static void cpuid(unsigned hoja, unsigned sub, unsigned r[4]) {
    __asm__ volatile("cpuid" : "=a"(r[0]), "=b"(r[1]), "=c"(r[2]), "=d"(r[3]) : "a"(hoja), "c"(sub));
}

void inicio(void);

void inicio(void) {
    unsigned h1[4], h7[4];
    unsigned long long xcr0 = 0;
    int osxsave, avx;
    cpuid(1, 0, h1);
    cpuid(7, 0, h7);
    osxsave = (h1[2] >> 27) & 1;
    if (osxsave) {
        unsigned lo, hi;
        __asm__ volatile("xgetbv" : "=a"(lo), "=d"(hi) : "c"(0));
        xcr0 = ((unsigned long long)hi << 32) | lo;
    }
    avx = ((h1[2] >> 28) & 1) && (xcr0 & 6) == 6;
#define PF(n, v, t) mira(!!IsProcessorFeaturePresent(n) == !!(v), t)
    PF(6, (h1[3] >> 25) & 1, "PF_XMMI (SSE) = CPUID");
    PF(10, (h1[3] >> 26) & 1, "PF_XMMI64 (SSE2) = CPUID");
    PF(13, h1[2] & 1, "PF_SSE3 = CPUID");
    PF(14, (h1[2] >> 13) & 1, "PF_COMPARE_EXCHANGE128 = CPUID");
    PF(36, (h1[2] >> 9) & 1, "PF_SSSE3 = CPUID");
    PF(37, (h1[2] >> 19) & 1, "PF_SSE4_1 = CPUID");
    PF(38, (h1[2] >> 20) & 1, "PF_SSE4_2 = CPUID");
    PF(39, avx, "PF_AVX = CPUID y XCR0 (el estado habilitado)");
    PF(40, avx && ((h7[1] >> 5) & 1), "PF_AVX2 = CPUID y XCR0");
    PF(17, osxsave, "PF_XSAVE_ENABLED = CPUID.OSXSAVE");
    PF(8, (h1[3] >> 4) & 1, "PF_RDTSC = CPUID");
    PF(12, 1, "PF_NX_ENABLED: si");
    di("tanda24.exe: IsProcessorFeaturePresent es el de Windows\r\n");
    ExitProcess(fallos);
}
