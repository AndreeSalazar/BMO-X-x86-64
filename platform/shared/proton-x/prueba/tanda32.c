/* tanda32.c -- el .exe de la TANDA 32 (01-10): las rutas de Boost.Filesystem.
 * Por Cyberpunk: Galaxy (REDGalaxy64.dll) fallaba su Init con
 * "boost::filesystem::path codecvt to string: error". Boost pasa las rutas
 * de ancho a estrecho con WideCharToMultiByte(CP_ACP o CP_OEMCP,
 * WC_NO_BEST_FIT_CHARS) y de vuelta con MultiByteToWideChar(CP_ACP,
 * MB_PRECOMPOSED). Con la pagina del SISTEMA esas banderas son validas; con
 * CP_UTF8 explicito, Windows las rechaza (ERROR_INVALID_FLAGS) y no deja
 * pasar lpUsedDefaultChar (ERROR_INVALID_PARAMETER). Solo ASCII: en una
 * pagina 1252 y en UTF-8 da lo mismo.
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
IMPORTA int W WideCharToMultiByte(unsigned cp, DWORD f, const WCHAR *w, int nw, char *b, int nb, const char *def, int *usado);
IMPORTA int W MultiByteToWideChar(unsigned cp, DWORD f, const char *b, int nb, WCHAR *w, int nw);
IMPORTA int W AreFileApisANSI(void);

#define CP_ACP 0
#define CP_OEMCP 1
#define CP_UTF8 65001
#define WC_NO_BEST_FIT_CHARS 0x400
#define MB_PRECOMPOSED 1
#define ERROR_INVALID_FLAGS 1004
#define ERROR_INVALID_PARAMETER 87

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

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int igual_w(const WCHAR *a, const WCHAR *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

void inicio(void);

void inicio(void) {
    static const WCHAR ruta[] = L"D:\\Cyberpunk 2077\\bin\\x64";
    static char b[64];
    static WCHAR w[64];
    int n, usado = 7;

    mira(AreFileApisANSI() != 0, "AreFileApisANSI: si (la pagina de Boost es CP_ACP)");
    n = WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS, ruta, -1, 0, 0, 0, 0);
    mira(n == 26, "WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS) mide: 26 con el 0");
    n = WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS, ruta, -1, b, 64, 0, 0);
    mira(n == 26 && igual(b, "D:\\Cyberpunk 2077\\bin\\x64"), "y convierte la ruta igual");
    n = WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS, ruta, 25, b, 64, 0, &usado);
    mira(n == 25 && usado == 0, "sin el 0, con lpUsedDefaultChar: 25 y usado = FALSE");
    n = WideCharToMultiByte(CP_OEMCP, WC_NO_BEST_FIT_CHARS, ruta, -1, b, 64, 0, 0);
    mira(n == 26, "WideCharToMultiByte(CP_OEMCP, WC_NO_BEST_FIT_CHARS): 26");
    n = MultiByteToWideChar(CP_ACP, MB_PRECOMPOSED, "D:\\juego\\x64", -1, w, 64);
    mira(n == 13 && igual_w(w, L"D:\\juego\\x64"), "MultiByteToWideChar(CP_ACP, MB_PRECOMPOSED): 13 y la ruta");
    SetLastError(0);
    n = WideCharToMultiByte(CP_UTF8, WC_NO_BEST_FIT_CHARS, ruta, -1, b, 64, 0, 0);
    mira(n == 0 && GetLastError() == ERROR_INVALID_FLAGS, "con CP_UTF8, WC_NO_BEST_FIT_CHARS: ERROR_INVALID_FLAGS");
    SetLastError(0);
    n = WideCharToMultiByte(CP_UTF8, 0, ruta, -1, b, 64, 0, &usado);
    mira(n == 0 && GetLastError() == ERROR_INVALID_PARAMETER, "con CP_UTF8, lpUsedDefaultChar: ERROR_INVALID_PARAMETER");
    SetLastError(0);
    n = MultiByteToWideChar(CP_UTF8, MB_PRECOMPOSED, "D:\\juego", -1, w, 64);
    mira(n == 0 && GetLastError() == ERROR_INVALID_FLAGS, "con CP_UTF8, MB_PRECOMPOSED: ERROR_INVALID_FLAGS");
    n = WideCharToMultiByte(CP_UTF8, 0, ruta, -1, b, 64, 0, 0);
    mira(n == 26 && igual(b, "D:\\Cyberpunk 2077\\bin\\x64"), "con CP_UTF8 y sin banderas: la ruta");
    di("tanda32.exe: las rutas de Boost.Filesystem, de ancho a estrecho y vuelta\r\n");
    ExitProcess(fallos);
}
