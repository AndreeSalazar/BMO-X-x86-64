/* tanda21.c -- el .exe de la TANDA 21 de Cyberpunk (30-09): LoadLibrary de
 * un API set. El CRT de MSVC del juego, al arrancar en el metal, hizo
 * LoadLibraryExW("api-ms-win-core-synch-l1-2-0", 0, SEARCH_SYSTEM32) para
 * buscar funciones con GetProcAddress, y la casa dijo NULL. En Windows un
 * API set no es un fichero: LoadLibrary devuelve el modulo que lo
 * implementa (kernelbase para los core, ucrtbase para los del CRT). Solo
 * RELACIONES: cualquier Windows 10 u 11 dice lo mismo.
 *
 * Y VirtualProtect sobre la PROPIA imagen: el juego lo hizo dos veces al
 * arrancar y la casa dijo FALSE. Codigo a R+X y datos a RW o R se pueden
 * (y "RW, escribir, lo de antes", como hace el cargador retrasado).
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
IMPORTA HANDLE W LoadLibraryExW(const WCHAR *n, HANDLE f, DWORD banderas);
IMPORTA HANDLE W LoadLibraryW(const WCHAR *n);
IMPORTA void *W GetProcAddress(HANDLE m, const char *n);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA int W VirtualProtect(void *d, unsigned long long n, DWORD prot, DWORD *antes);

#define SISTEMA32 0x800 /* LOAD_LIBRARY_SEARCH_SYSTEM32, lo que pide el CRT */

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

void inicio(void);

static volatile int escrito;
static const volatile int constante = 7;

void inicio(void) {
    HANDLE base = GetModuleHandleW(L"kernelbase.dll");
    HANDLE sincro = LoadLibraryExW(L"api-ms-win-core-synch-l1-2-0", 0, SISTEMA32);
    HANDLE fibras = LoadLibraryExW(L"api-ms-win-core-fibers-l1-1-1", 0, SISTEMA32);
    HANDLE locale = LoadLibraryExW(L"api-ms-win-core-localization-l1-2-1", 0, SISTEMA32);
    HANDLE texto = LoadLibraryExW(L"api-ms-win-core-string-l1-1-0", 0, SISTEMA32);
    HANDLE crt;

    /* -- los core: su anfitrion es kernelbase */
    mira(base != 0, "GetModuleHandleW kernelbase: esta cargada");
    mira(sincro != 0 && sincro == base, "LoadLibraryExW synch-l1-2-0 (sin .dll): kernelbase");
    mira(fibras != 0 && fibras == base, "LoadLibraryExW fibers-l1-1-1: kernelbase");
    mira(locale != 0 && locale == base, "LoadLibraryExW localization-l1-2-1: kernelbase");
    mira(texto != 0 && texto == base, "LoadLibraryExW string-l1-1-0: kernelbase");
    mira(LoadLibraryW(L"API-MS-WIN-CORE-SYNCH-L1-2-0.DLL") == base, "LoadLibraryW con .dll y en mayusculas: kernelbase");

    /* -- y GetProcAddress encuentra lo que el CRT busca en ellos */
    mira(sincro && GetProcAddress(sincro, "InitializeCriticalSectionEx") != 0, "GetProcAddress synch!InitializeCriticalSectionEx");
    mira(fibras && GetProcAddress(fibras, "FlsAlloc") != 0, "GetProcAddress fibers!FlsAlloc");
    mira(locale && GetProcAddress(locale, "LCMapStringEx") != 0, "GetProcAddress localization!LCMapStringEx");
    mira(texto && GetProcAddress(texto, "CompareStringOrdinal") != 0, "GetProcAddress string!CompareStringOrdinal");

    /* -- los del CRT: su anfitrion es ucrtbase */
    crt = LoadLibraryExW(L"api-ms-win-crt-string-l1-1-0", 0, SISTEMA32);
    mira(crt != 0 && crt == GetModuleHandleW(L"ucrtbase.dll"), "LoadLibraryExW crt-string-l1-1-0: ucrtbase");
    mira(crt && GetProcAddress(crt, "strlen") != 0, "GetProcAddress crt-string!strlen");

    /* -- un API set que no existe: NULL y ERROR_MOD_NOT_FOUND */
    SetLastError(0);
    mira(LoadLibraryExW(L"api-ms-win-nadie-l1-1-0", 0, SISTEMA32) == 0 && GetLastError() == 126, "LoadLibraryExW de un API set que no existe: NULL, 126");

    /* -- VirtualProtect sobre la propia imagen */
    {
        DWORD antes = 0, otra = 0;
        escrito = 1;
        mira(VirtualProtect((void *)&escrito, 4, 0x04, &antes) && antes == 0x04, "VirtualProtect .data a RW: si, y antes era RW");
        mira(VirtualProtect((void *)inicio, 1, 0x20, &antes) && antes == 0x20, "VirtualProtect del codigo a R+X: si, y antes era R+X");
        mira(VirtualProtect((void *)&constante, 4, 0x04, &antes), "VirtualProtect de .rdata a RW: si");
        *(volatile int *)&constante = 9;
        mira(constante == 9 && VirtualProtect((void *)&constante, 4, antes, &otra), "se escribe, y vuelve a lo de antes");
    }
    di("tanda21.exe: LoadLibrary de un API set es lo de Windows\r\n");
    ExitProcess(fallos);
}
