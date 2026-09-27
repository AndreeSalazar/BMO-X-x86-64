/* texto.c -- el .exe de P4f de PROTON-X: el texto, la consola y los modulos,
 * lo primero que la `std` de Rust (y cualquier CRT) toca de Windows.
 *
 *    el texto     MultiByteToWideChar / WideCharToMultiByte en UTF-8: lo
 *                 que hace falta, la conversion, el byte malo (U+FFFD, o
 *                 ERROR_NO_UNICODE_TRANSLATION si se pide), el bufer corto;
 *                 CompareStringOrdinal con y sin mayusculas; lstrlenW
 *    la consola   GetConsoleMode y WriteConsoleW (o, si la salida no es una
 *                 consola, ERROR_INVALID_HANDLE); GetConsoleOutputCP
 *    los modulos  GetModuleHandleW con y sin ".dll", LoadLibraryW de
 *                 d3d12.dll y GetProcAddress de D3D12CreateDevice (como un
 *                 juego que la carga "si esta"), GetProcAddress de
 *                 GetLastError LLAMADA por su puntero, y los NO: 126 y 127
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
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
IMPORTA void WINAPI SetLastError(DWORD e);
IMPORTA HANDLE WINAPI GetModuleHandleW(const WCHAR *n);
IMPORTA int WINAPI MultiByteToWideChar(unsigned cp, DWORD banderas, const char *s, int n, WCHAR *d, int m);
IMPORTA int WINAPI WideCharToMultiByte(unsigned cp, DWORD banderas, const WCHAR *s, int n, char *d, int m, const char *def, int *usado);
IMPORTA int WINAPI CompareStringOrdinal(const WCHAR *a, int na, const WCHAR *b, int nb, int sin_mayusculas);
IMPORTA int WINAPI lstrlenW(const WCHAR *s);
IMPORTA int WINAPI GetConsoleMode(HANDLE h, DWORD *modo);
IMPORTA int WINAPI WriteConsoleW(HANDLE h, const WCHAR *b, DWORD n, DWORD *escritos, void *r);
IMPORTA unsigned WINAPI GetConsoleOutputCP(void);
IMPORTA HANDLE WINAPI LoadLibraryW(const WCHAR *n);
IMPORTA void *WINAPI GetProcAddress(HANDLE m, const char *n);
IMPORTA int WINAPI FreeLibrary(HANDLE m);

#define CP_UTF8 65001
#define MB_ERR_INVALID_CHARS 0x8
#define WC_ERR_INVALID_CHARS 0x80

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

static int iguales_w(const WCHAR *a, const WCHAR *b, int n) {
    int i;
    for (i = 0; i < n; i++)
        if (a[i] != b[i]) return 0;
    return 1;
}
static int iguales_b(const char *a, const char *b, int n) {
    int i;
    for (i = 0; i < n; i++)
        if (a[i] != b[i]) return 0;
    return 1;
}

static void el_texto(void) {
    /* "a", n con tilde, el euro y la cara sonriente, en UTF-8, y su 0. */
    static const char U8[] = "a\xC3\xB1\xE2\x82\xAC\xF0\x9F\x98\x80";
    static const WCHAR U16[] = {0x61, 0xF1, 0x20AC, 0xD83D, 0xDE00, 0};
    WCHAR w[16];
    char b[16];
    int n;

    n = MultiByteToWideChar(CP_UTF8, 0, U8, -1, 0, 0);
    mira(n == 6, "MultiByteToWideChar: lo que hace falta (cinco y el 0)", n);
    n = MultiByteToWideChar(CP_UTF8, 0, U8, -1, w, 16);
    mira(n == 6 && iguales_w(w, U16, 6), "y la conversion: a, n con tilde, euro, y un par sustituto", n);
    n = MultiByteToWideChar(CP_UTF8, 0, "a\xFF" "b", 3, w, 16);
    mira(n == 3 && w[1] == 0xFFFD, "un byte malo: U+FFFD", w[1]);
    n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, "a\xFF" "b", 3, w, 16);
    mira(n == 0 && GetLastError() == 1113, "con MB_ERR_INVALID_CHARS: ERROR_NO_UNICODE_TRANSLATION", GetLastError());
    n = MultiByteToWideChar(CP_UTF8, 0, U8, -1, w, 2);
    mira(n == 0 && GetLastError() == 122, "bufer corto: ERROR_INSUFFICIENT_BUFFER", GetLastError());

    n = WideCharToMultiByte(CP_UTF8, 0, U16, -1, 0, 0, 0, 0);
    mira(n == 11, "WideCharToMultiByte: lo que hace falta (diez y el 0)", n);
    n = WideCharToMultiByte(CP_UTF8, 0, U16, -1, b, 16, 0, 0);
    mira(n == 11 && iguales_b(b, U8, 11), "y la conversion, byte a byte", n);
    {
        static const WCHAR SUELTO[] = {0x41, 0xD800, 0x42};
        n = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, SUELTO, 3, b, 16, 0, 0);
        mira(n == 0 && GetLastError() == 1113, "un sustituto suelto con WC_ERR_INVALID_CHARS: 1113", GetLastError());
    }

    mira(CompareStringOrdinal(L"abc", -1, L"ABC", -1, 1) == 2, "CompareStringOrdinal sin mayusculas: igual (2)", 2);
    mira(CompareStringOrdinal(L"abc", -1, L"ABC", -1, 0) == 3, "con mayusculas: 'a' va despues de 'A' (3)", 3);
    mira(CompareStringOrdinal(L"ab", 2, L"abc", 3, 0) == 1, "mas corto va antes (1)", 1);
    mira(lstrlenW(L"hola") == 4 && lstrlenW(0) == 0, "lstrlenW", 4);
}

static void la_consola(void) {
    static const WCHAR LINEA[] = L"  bien  WriteConsoleW escribe UTF-16 en la consola\r\n";
    DWORD modo = 0, n = 0;
    if (GetConsoleMode(salida, &modo)) {
        mira(WriteConsoleW(salida, LINEA, lstrlenW(LINEA), &n, 0) && n == (DWORD)lstrlenW(LINEA), "GetConsoleMode: es una consola, y WriteConsoleW dijo todo (la linea de arriba)", n);
    } else {
        mira(GetLastError() == 6, "la salida no es una consola: ERROR_INVALID_HANDLE (y WriteFile es lo que vale)", GetLastError());
        di("  bien  (sin consola, la linea de WriteConsoleW no se escribe)\r\n");
    }
    mira(GetConsoleOutputCP() != 0, "GetConsoleOutputCP da una pagina de codigos", GetConsoleOutputCP());
}

static void los_modulos(void) {
    typedef DWORD(WINAPI * PIDE)(void);
    HANDLE k = GetModuleHandleW(L"kernel32.dll"), d;
    PIDE f;

    mira(k != 0 && GetModuleHandleW(L"KERNEL32") == k, "GetModuleHandleW de kernel32, con y sin .dll: el mismo", (U64)k);
    f = (PIDE)GetProcAddress(k, "GetLastError");
    SetLastError(1234);
    mira(f && f() == 1234, "GetProcAddress(GetLastError), llamada por su puntero", f ? f() : 0);
    mira(!GetProcAddress(k, "NoExisteJamasPx") && GetLastError() == 127, "una que no esta: ERROR_PROC_NOT_FOUND", GetLastError());
    mira(!GetModuleHandleW(L"no_existe_px.dll") && GetLastError() == 126, "un modulo que no esta: ERROR_MOD_NOT_FOUND", GetLastError());
    d = LoadLibraryW(L"d3d12.dll");
    mira(d && GetProcAddress(d, "D3D12CreateDevice") != 0, "LoadLibraryW(d3d12.dll) y GetProcAddress(D3D12CreateDevice)", (U64)d);
    mira(!d || FreeLibrary(d), "FreeLibrary", 1);
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    di("texto.exe: el texto, la consola y los modulos de Windows\r\n");
    el_texto();
    la_consola();
    los_modulos();
    di(fallos ? "texto.exe: ALGO NO es como en Windows\r\n" : "texto.exe: el texto y los modulos son los de Windows\r\n");
    ExitProcess(fallos);
}
