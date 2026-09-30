/* tanda25.c -- el .exe de la TANDA 25 de Cyberpunk (30-09):
 * SystemFunction036 (RtlGenRandom). En el metal el juego se rindio con
 * abort() en su arranque: un constructor global crea un std::random_device,
 * que llama a rand_s, y la UCRT (enlazada dentro del .exe) busca
 * SystemFunction036 como lo hace aqui: LoadLibraryExW del API set
 * "api-ms-win-security-systemfunctions-l1-1-0" (o de "advapi32") con
 * LOAD_LIBRARY_SEARCH_SYSTEM32, y GetProcAddress. Si no la encuentra,
 * abort().
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned char BYTE;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W LoadLibraryExW(const unsigned short *n, HANDLE f, DWORD b);
IMPORTA void *W GetProcAddress(HANDLE h, const char *n);

typedef BYTE(W *Azar)(void *b, DWORD n);

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

static int iguales(const BYTE *a, const BYTE *b, unsigned n) {
    unsigned i;
    for (i = 0; i < n; i++)
        if (a[i] != b[i])
            return 0;
    return 1;
}

static int ceros(const BYTE *a, unsigned n) {
    unsigned i;
    for (i = 0; i < n; i++)
        if (a[i])
            return 0;
    return 1;
}

void inicio(void);

void inicio(void) {
    static const unsigned short api[] = {'a', 'p', 'i', '-', 'm', 's', '-', 'w', 'i', 'n', '-', 's', 'e', 'c', 'u', 'r', 'i', 't', 'y', '-', 's', 'y', 's', 't', 'e', 'm', 'f', 'u', 'n', 'c', 't', 'i', 'o', 'n', 's', '-', 'l', '1', '-', '1', '-', '0', 0};
    static const unsigned short adv[] = {'a', 'd', 'v', 'a', 'p', 'i', '3', '2', 0};
    BYTE a[64], b[64], c[80];
    HANDLE h1 = LoadLibraryExW(api, 0, 0x800);
    HANDLE h2 = LoadLibraryExW(adv, 0, 0x800);
    Azar f1 = h1 ? (Azar)GetProcAddress(h1, "SystemFunction036") : 0;
    Azar f2 = h2 ? (Azar)GetProcAddress(h2, "SystemFunction036") : 0;
    unsigned i;
    mira(h1 != 0, "LoadLibraryExW(\"api-ms-win-security-systemfunctions-l1-1-0\", SEARCH_SYSTEM32)");
    mira(h2 != 0, "LoadLibraryExW(\"advapi32\", SEARCH_SYSTEM32): sin .dll");
    mira(f1 != 0, "GetProcAddress(API set, \"SystemFunction036\")");
    mira(f2 != 0, "GetProcAddress(advapi32, \"SystemFunction036\")");
    if (!f1)
        f1 = f2;
    if (!f1) {
        di("tanda25.exe: sin SystemFunction036 no hay mas que mirar\r\n");
        ExitProcess(fallos + 4);
    }
    for (i = 0; i < 64; i++)
        a[i] = b[i] = 0;
    for (i = 0; i < 80; i++)
        c[i] = 0xAB;
    mira(f1(a, 64) != 0, "SystemFunction036(64 bytes): TRUE");
    mira(!ceros(a, 64), "y no son 64 ceros");
    mira(f1(b, 64) != 0 && !iguales(a, b, 64), "otra vez: otros 64 bytes");
    mira(f1(c + 8, 3) != 0 && c[7] == 0xAB && c[11] == 0xAB, "3 bytes: no escribe ni antes ni despues");
    mira(f1(0, 0) != 0, "0 bytes: TRUE");
    di("tanda25.exe: SystemFunction036 (el azar de rand_s) es el de Windows\r\n");
    ExitProcess(fallos);
}
