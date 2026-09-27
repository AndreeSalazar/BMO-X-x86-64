/* crt.c -- el .exe de P4e de PROTON-X: lo que un CRT de verdad pide al
 * arrancar y en marcha (malloc/free, el VirtualAlloc de sus asignadores, su
 * nombre, su linea de ordenes y su entorno).
 *
 *    el monton     GetProcessHeap (y el PEB lo dice igual), HeapAlloc a 16,
 *                  HeapSize exacto, HEAP_ZERO_MEMORY sobre memoria ya sucia,
 *                  HeapReAlloc que conserva, IN_PLACE_ONLY que no puede, dos
 *                  mil bloques mezclados sin pisarse, y 8 MiB despues de
 *                  soltarlo todo; HeapCreate/HeapDestroy
 *    VirtualAlloc  reservar 1 MiB a 64 KiB, hacer las TRES paginas que toca
 *                  un rango torcido, VirtualQuery de cada tirada, DECOMMIT,
 *                  RELEASE con medida (87) y sin ella; COMMIT|RESERVE a
 *                  ceros y VirtualProtect; GetSystemInfo
 *    el proceso    GetModuleFileNameW/A (y con bufer corto: 122),
 *                  GetCommandLineW/A, y el entorno: poner, leer sin
 *                  mayusculas que cuenten, el bloque, quitar, 203
 *
 * Sin CRT, justo para poder mirar lo que el CRT usaria. Sale con el numero
 * de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int codigo);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA HANDLE WINAPI GetProcessHeap(void);
IMPORTA void *WINAPI HeapAlloc(HANDLE h, DWORD banderas, U64 n);
IMPORTA int WINAPI HeapFree(HANDLE h, DWORD banderas, void *p);
IMPORTA void *WINAPI HeapReAlloc(HANDLE h, DWORD banderas, void *p, U64 n);
IMPORTA U64 WINAPI HeapSize(HANDLE h, DWORD banderas, const void *p);
IMPORTA HANDLE WINAPI HeapCreate(DWORD banderas, U64 inicial, U64 maximo);
IMPORTA int WINAPI HeapDestroy(HANDLE h);
IMPORTA void *WINAPI VirtualAlloc(void *dir, U64 n, DWORD tipo, DWORD prot);
IMPORTA int WINAPI VirtualFree(void *dir, U64 n, DWORD tipo);
IMPORTA U64 WINAPI VirtualQuery(const void *dir, void *mbi, U64 n);
IMPORTA int WINAPI VirtualProtect(void *dir, U64 n, DWORD prot, DWORD *antes);
IMPORTA void WINAPI GetSystemInfo(void *si);
IMPORTA DWORD WINAPI GetModuleFileNameW(HANDLE m, WCHAR *b, DWORD n);
IMPORTA DWORD WINAPI GetModuleFileNameA(HANDLE m, char *b, DWORD n);
IMPORTA WCHAR *WINAPI GetCommandLineW(void);
IMPORTA char *WINAPI GetCommandLineA(void);
IMPORTA DWORD WINAPI GetEnvironmentVariableW(const WCHAR *n, WCHAR *b, DWORD tam);
IMPORTA int WINAPI SetEnvironmentVariableW(const WCHAR *n, const WCHAR *v);
IMPORTA WCHAR *WINAPI GetEnvironmentStringsW(void);
IMPORTA int WINAPI FreeEnvironmentStringsW(WCHAR *b);

#define HEAP_ZERO_MEMORY 0x8
#define HEAP_REALLOC_IN_PLACE_ONLY 0x10
#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define MEM_DECOMMIT 0x4000
#define MEM_RELEASE 0x8000
#define PAGE_READONLY 2
#define PAGE_READWRITE 4

/* MEMORY_BASIC_INFORMATION de x64. */
typedef struct {
    U64 BaseAddress, AllocationBase;
    DWORD AllocationProtect, relleno;
    U64 RegionSize;
    DWORD State, Protect, Type, relleno2;
} MBI;

typedef struct {
    unsigned short arquitectura, reservado;
    DWORD pagina;
    U64 minimo, maximo, mascara;
    DWORD procesadores, tipo, grano;
    unsigned short nivel, revision;
} SYSINFO;

static HANDLE salida;
static unsigned fallos;

static void di(const char *s) {
    DWORD n = 0, e;
    while (s[n]) n++;
    WriteFile(salida, s, n, &e, 0);
}

static void hex(U64 v) {
    char b[19];
    int i;
    b[0] = '0';
    b[1] = 'x';
    for (i = 0; i < 16; i++) {
        unsigned d = (unsigned)(v >> (60 - 4 * i)) & 15;
        b[2 + i] = (char)(d < 10 ? '0' + d : 'a' + d - 10);
    }
    b[18] = 0;
    di(b);
}

static void mira(int bien, const char *que, U64 valor) {
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di(" ");
    hex(valor);
    di("\r\n");
    if (!bien) fallos++;
}

static U64 gs64(unsigned long o) {
    U64 v;
    __asm__ volatile("movq %%gs:(%1), %0" : "=r"(v) : "r"((U64)o));
    return v;
}

/* Rellenar y comprobar por un puntero volatil: que el compilador no ponga
 * un memset (no hay CRT) ni se salte nada. */
static void llenar(void *p, U64 n, unsigned char v) {
    volatile unsigned char *b = (volatile unsigned char *)p;
    U64 i;
    for (i = 0; i < n; i++) b[i] = (unsigned char)(v + i * 7);
}
static int lleno(const void *p, U64 n, unsigned char v) {
    const volatile unsigned char *b = (const volatile unsigned char *)p;
    U64 i;
    for (i = 0; i < n; i++)
        if (b[i] != (unsigned char)(v + i * 7)) return 0;
    return 1;
}
static int ceros(const void *p, U64 n) {
    const volatile unsigned char *b = (const volatile unsigned char *)p;
    U64 i;
    for (i = 0; i < n; i++)
        if (b[i]) return 0;
    return 1;
}

static WCHAR minus(WCHAR c) {
    return c >= 'A' && c <= 'Z' ? (WCHAR)(c + 32) : c;
}
/* `a` (cualquier caso) contiene `b` (en minusculas). */
static int contiene(const WCHAR *a, const char *b) {
    int i, j;
    for (i = 0; a[i]; i++) {
        for (j = 0; b[j] && a[i + j] && minus(a[i + j]) == (WCHAR)b[j]; j++) {
        }
        if (!b[j]) return 1;
    }
    return 0;
}
static int iguales(const WCHAR *a, const char *b) {
    int i;
    for (i = 0; b[i]; i++)
        if (a[i] != (WCHAR)b[i]) return 0;
    return a[i] == 0;
}

#define N 2000
static void *vivos[N];
static U64 medidas[N];

static void el_monton(void) {
    HANDLE h = GetProcessHeap(), otro;
    unsigned char *a, *b, *c;
    U64 x = 0x2545F4914F6CDD1Dull, i, bien = 1;

    mira(h != 0 && gs64(0x60) && *(U64 *)(gs64(0x60) + 0x30) == (U64)h, "GetProcessHeap, y el PEB+0x30 dice el mismo", (U64)h);
    a = HeapAlloc(h, 0, 100);
    mira(a && ((U64)a & 15) == 0 && HeapSize(h, 0, a) == 100, "HeapAlloc(100): alineado a 16 y HeapSize 100", HeapSize(h, 0, a));
    llenar(a, 100, 1);
    b = HeapAlloc(h, 0, 256);
    llenar(b, 256, 0xAB);
    HeapFree(h, 0, b);
    c = HeapAlloc(h, HEAP_ZERO_MEMORY, 256);
    mira(c && ceros(c, 256), "HEAP_ZERO_MEMORY a ceros, aunque la memoria ya se uso", (U64)c);
    HeapFree(h, 0, c);
    a = HeapReAlloc(h, HEAP_ZERO_MEMORY, a, 5000);
    mira(a && lleno(a, 100, 1) && ceros(a + 100, 4900) && HeapSize(h, 0, a) == 5000, "HeapReAlloc a 5000 conserva lo de dentro, y el resto a ceros", HeapSize(h, 0, a));
    mira(!HeapReAlloc(h, HEAP_REALLOC_IN_PLACE_ONLY, a, 1ull << 30) && HeapSize(h, 0, a) == 5000 && lleno(a, 100, 1), "IN_PLACE_ONLY a 1 GiB: no, y nada tocado", 0);
    mira(HeapFree(h, 0, a) && HeapFree(h, 0, 0), "HeapFree, y HeapFree(NULL) tambien es exito", 1);

    /* Dos mil bloques de 1 a 3000 bytes, cada uno con su sello; se sueltan
     * la mitad, se piden otra vez, y se miran todos. */
    for (i = 0; i < N; i++) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        medidas[i] = 1 + x % 3000;
        vivos[i] = HeapAlloc(h, 0, medidas[i]);
        if (!vivos[i]) bien = 0;
        else llenar(vivos[i], medidas[i], (unsigned char)i);
    }
    for (i = 0; i < N; i += 2) {
        if (!lleno(vivos[i], medidas[i], (unsigned char)i)) bien = 0;
        HeapFree(h, 0, vivos[i]);
        vivos[i] = HeapAlloc(h, 0, medidas[i] / 2 + 1);
        medidas[i] = medidas[i] / 2 + 1;
        if (!vivos[i]) bien = 0;
        else llenar(vivos[i], medidas[i], (unsigned char)(i + 3));
    }
    for (i = 0; i < N; i++) {
        if (!lleno(vivos[i], medidas[i], (unsigned char)(i % 2 ? i : i + 3)) || HeapSize(h, 0, vivos[i]) != medidas[i]) bien = 0;
        HeapFree(h, 0, vivos[i]);
    }
    mira(bien, "dos mil bloques mezclados: ni un byte pisado, cada HeapSize exacto", N);
    a = HeapAlloc(h, 0, 8u << 20);
    mira(a != 0, "y despues de soltarlo todo, 8 MiB de una vez", (U64)a);
    if (a) {
        llenar(a, 8u << 20, 9);
        mira(lleno(a, 8u << 20, 9), "los 8 MiB se escriben y se leen enteros", 8u << 20);
        HeapFree(h, 0, a);
    }
    otro = HeapCreate(0, 0, 0);
    a = otro ? HeapAlloc(otro, 0, 64) : 0;
    b = otro ? HeapAlloc(otro, 0, 64) : 0;
    mira(otro && a && b && HeapDestroy(otro), "HeapCreate, dos bloques, y HeapDestroy se los lleva", (U64)otro);
}

static void lo_virtual(void) {
    MBI m;
    SYSINFO si;
    DWORD antes = 0;
    unsigned char *base, *p;

    GetSystemInfo(&si);
    mira(si.arquitectura == 9 && si.pagina == 4096 && si.grano == 65536 && si.procesadores >= 1, "GetSystemInfo: AMD64, paginas de 4 KiB, regiones a 64 KiB", si.grano);

    base = VirtualAlloc(0, 1 << 20, MEM_RESERVE, PAGE_READWRITE);
    mira(base && ((U64)base & 0xFFFF) == 0, "VirtualAlloc(MEM_RESERVE, 1 MiB): a 64 KiB", (U64)base);
    mira(VirtualQuery(base, &m, sizeof m) == 48 && m.State == MEM_RESERVE && m.RegionSize == (1 << 20) && m.AllocationBase == (U64)base, "VirtualQuery: 1 MiB reservado", m.RegionSize);
    p = VirtualAlloc(base + 0x10000 + 10, 0x2000, MEM_COMMIT, PAGE_READWRITE);
    mira(p == base + 0x10000, "MEM_COMMIT de un rango torcido: desde su pagina", (U64)(p - base));
    mira(VirtualQuery(base + 0x10000 + 77, &m, sizeof m) && m.State == MEM_COMMIT && m.RegionSize == 0x3000 && m.Protect == PAGE_READWRITE && m.BaseAddress == (U64)p, "las TRES paginas que toca, hechas y RW", m.RegionSize);
    mira(ceros(p, 0x3000), "y a ceros", 0x3000);
    llenar(p, 0x3000, 5);
    mira(VirtualQuery(base, &m, sizeof m) && m.State == MEM_RESERVE && m.RegionSize == 0x10000, "delante, 64 KiB siguen solo reservados", m.RegionSize);
    mira(VirtualFree(p, 0x1000, MEM_DECOMMIT) && VirtualQuery(p, &m, sizeof m) && m.State == MEM_RESERVE, "MEM_DECOMMIT de una pagina: vuelve a reservada", m.State);
    mira(!VirtualFree(base, 0x1000, MEM_RELEASE) && GetLastError() == 87, "MEM_RELEASE con medida: ERROR_INVALID_PARAMETER", GetLastError());
    mira(VirtualFree(base, 0, MEM_RELEASE), "MEM_RELEASE de la region entera", 1);

    p = VirtualAlloc(0, 10000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    mira(p && ceros(p, 10000), "MEM_COMMIT|MEM_RESERVE: a ceros", (U64)p);
    if (p) {
        llenar(p, 10000, 3);
        mira(VirtualProtect(p, 4096, PAGE_READONLY, &antes) && antes == PAGE_READWRITE, "VirtualProtect a PAGE_READONLY: antes era PAGE_READWRITE", antes);
        mira(VirtualQuery(p, &m, sizeof m) && m.Protect == PAGE_READONLY && m.RegionSize == 0x1000, "VirtualQuery lo dice, pagina a pagina", m.Protect);
        VirtualProtect(p, 4096, PAGE_READWRITE, &antes);
        mira(lleno(p, 10000, 3) && VirtualFree(p, 0, MEM_RELEASE), "lo de dentro sigue, y se suelta", 1);
    }
}

static void el_proceso(void) {
    WCHAR b[260], *l, *e;
    char a[260], *la;
    DWORD n, na, i, bien;

    n = GetModuleFileNameW(0, b, 260);
    mira(n > 8 && b[n] == 0 && b[1] == ':' && b[2] == '\\' && contiene(b, "\\crt.exe"), "GetModuleFileNameW: X:\\...\\crt.exe", n);
    na = GetModuleFileNameA(0, a, 260);
    for (i = 0, bien = na == n; bien && i < n; i++) bien = (WCHAR)a[i] == b[i];
    mira(bien, "GetModuleFileNameA: lo mismo en bytes", na);
    b[4] = 0x7777;
    mira(GetModuleFileNameW(0, b, 5) == 5 && b[4] == 0 && GetLastError() == 122, "con un bufer de 5: 4 y el 0, devuelve 5 y ERROR_INSUFFICIENT_BUFFER", GetLastError());

    l = GetCommandLineW();
    la = GetCommandLineA();
    mira(l && contiene(l, "crt"), "GetCommandLineW nombra el .exe", 0);
    for (i = 0, bien = la != 0; bien && l[i]; i++) bien = (WCHAR)la[i] == l[i];
    mira(bien && la[i] == 0, "GetCommandLineA: la misma linea", i);

    mira(GetEnvironmentVariableW(L"PATH", 0, 0) > 1, "PATH esta en el entorno", 0);
    mira(SetEnvironmentVariableW(L"PXPRUEBA", L"hola"), "SetEnvironmentVariableW(PXPRUEBA=hola)", 1);
    n = GetEnvironmentVariableW(L"pxprueba", b, 64);
    mira(n == 4 && iguales(b, "hola"), "GetEnvironmentVariableW sin mayusculas que cuenten: 4, hola", n);
    /* Con el bufer corto Windows dice lo que hace falta; lo que queda DENTRO
     * del bufer "no esta definido" (su documentacion) y Windows lo toca: no
     * se mira (el 27-09 en Windows, esta linea decia MAL por mirarlo). */
    n = GetEnvironmentVariableW(L"PXPRUEBA", b, 2);
    mira(n == 5, "bufer corto: lo que hace falta CON el 0", n);
    e = GetEnvironmentStringsW();
    for (l = e, bien = 0; l && *l; l += i + 1) {
        for (i = 0; l[i]; i++) {
        }
        if (iguales(l, "PXPRUEBA=hola")) bien = 1;
    }
    mira(bien && FreeEnvironmentStringsW(e), "el bloque de GetEnvironmentStringsW la trae, y se suelta", 0);
    SetEnvironmentVariableW(L"PXPRUEBA", 0);
    mira(GetEnvironmentVariableW(L"PXPRUEBA", b, 64) == 0 && GetLastError() == 203, "quitada: ERROR_ENVVAR_NOT_FOUND", GetLastError());
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    di("crt.exe: lo que pide un CRT de Windows\r\n");
    el_monton();
    lo_virtual();
    el_proceso();
    di(fallos ? "crt.exe: ALGO NO es como en Windows\r\n" : "crt.exe: la memoria y el proceso son los de Windows\r\n");
    ExitProcess(fallos);
}
