/* tanda23.c -- el .exe de la TANDA 23 de Cyberpunk (30-09): la memoria EN
 * MARCHA. El juego, recien saltado a su entrada, pregunto GlobalMemoryStatus
 * y pidio con VirtualAlloc mas de 64 MiB de una vez; la casa solo sabia dar
 * bloques de 64 MiB (P0.4c: la RESERVA del kernel). Aqui: reservar mucho sin
 * gastar, hacer un trozo en medio, deshacer y rehacer (a cero otra vez),
 * soltar; 200 MiB hechos de una vez; un HeapAlloc de 100 MiB; y reservar
 * 1 GiB. Solo RELACIONES: cualquier Windows de 64 bits dice lo mismo.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA int W GlobalMemoryStatusEx(void *m);
IMPORTA void *W VirtualAlloc(void *d, U64 n, DWORD tipo, DWORD prot);
IMPORTA int W VirtualFree(void *d, U64 n, DWORD tipo);
IMPORTA U64 W VirtualQuery(const void *d, void *mbi, U64 n);
IMPORTA HANDLE W GetProcessHeap(void);
IMPORTA void *W HeapAlloc(HANDLE h, DWORD banderas, U64 n);
IMPORTA int W HeapFree(HANDLE h, DWORD banderas, void *p);

#define MIB (1024ull * 1024)
#define COMMIT 0x1000
#define RESERVE 0x2000
#define DECOMMIT 0x4000
#define RELEASE 0x8000
#define RW 0x04

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

/* MEMORY_BASIC_INFORMATION de x64: 48 bytes. */
typedef struct {
    U64 base, base_region;
    DWORD prot_inicial, relleno1;
    U64 tam;
    DWORD estado, prot, tipo, relleno2;
} Mbi;

void inicio(void);

void inicio(void) {
    U64 m[8];
    Mbi q;
    unsigned char *p, *h, *g;
    HANDLE heap = GetProcessHeap();

    /* -- lo que pregunto el juego */
    m[0] = 64;
    mira(GlobalMemoryStatusEx(m) && m[1] > 0 && m[2] <= m[1], "GlobalMemoryStatusEx: hay RAM, y la libre cabe en la total");

    /* -- reservar 256 MiB no gasta; hacer 1 MiB en medio */
    p = VirtualAlloc(0, 256 * MIB, RESERVE, RW);
    mira(p != 0 && ((U64)p & 0xFFFF) == 0, "VirtualAlloc RESERVE 256 MiB: a 64 KiB");
    mira(p && VirtualQuery(p, &q, sizeof q) == 48 && q.estado == RESERVE && q.tam == 256 * MIB, "VirtualQuery: 256 MiB solo reservados");
    h = p + 100 * MIB;
    mira(p && VirtualAlloc(h, MIB, COMMIT, RW) == h, "VirtualAlloc COMMIT de 1 MiB en medio: esa direccion");
    if (p) {
        h[0] = 1;
        h[MIB - 1] = 2;
    }
    mira(p && h[1] == 0 && h[0] == 1 && h[MIB - 1] == 2, "lo hecho viene a cero, y se escribe");
    mira(p && VirtualQuery(h, &q, sizeof q) == 48 && q.estado == COMMIT && q.tam == MIB && q.prot == RW, "VirtualQuery: ese MiB hecho, RW");

    /* -- deshacer y rehacer: a cero otra vez */
    mira(p && VirtualFree(h, MIB, DECOMMIT), "VirtualFree DECOMMIT");
    mira(p && VirtualQuery(h, &q, sizeof q) == 48 && q.estado == RESERVE, "VirtualQuery: otra vez solo reservado");
    mira(p && VirtualAlloc(h, MIB, COMMIT, RW) == h && h[0] == 0 && h[MIB - 1] == 0, "rehecho: a cero otra vez");
    mira(p && VirtualFree(p, 0, RELEASE), "VirtualFree RELEASE de la region entera");

    /* -- 200 MiB hechos de una vez (lo que el juego pidio: mas de 64) */
    g = VirtualAlloc(0, 200 * MIB, RESERVE | COMMIT, RW);
    if (g) {
        g[0] = 3;
        g[200 * MIB - 1] = 4;
    }
    mira(g != 0 && g[0] == 3 && g[200 * MIB - 1] == 4 && g[100 * MIB] == 0, "VirtualAlloc RESERVE|COMMIT 200 MiB: se escribe entero");
    mira(g && VirtualFree(g, 0, RELEASE), "y se suelta");

    /* -- un HeapAlloc de 100 MiB */
    g = HeapAlloc(heap, 0, 100 * MIB);
    if (g) {
        g[0] = 5;
        g[100 * MIB - 1] = 6;
    }
    mira(g != 0 && g[0] == 5 && g[100 * MIB - 1] == 6, "HeapAlloc de 100 MiB: se escribe entero");
    mira(g && HeapFree(heap, 0, g), "HeapFree de esos 100 MiB");

    /* -- reservar 1 GiB: solo direcciones */
    g = VirtualAlloc(0, 1024 * MIB, RESERVE, RW);
    mira(g != 0 && VirtualQuery(g, &q, sizeof q) == 48 && q.estado == RESERVE && q.tam == 1024 * MIB, "VirtualAlloc RESERVE 1 GiB: solo direcciones");
    mira(g && VirtualFree(g, 0, RELEASE), "y se suelta");
    di("tanda23.exe: la memoria en marcha es la de Windows\r\n");
    ExitProcess(fallos);
}
