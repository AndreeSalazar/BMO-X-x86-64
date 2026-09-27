/* peek.c -- el .exe de P3c1 de PROTON-X: lo chico que le faltaba a BMOX-12
 * (el estudio_d3d12 de EPICX-FRAMEWORK), medido en su tabla de
 * importaciones el 27-09.
 *
 *    user32     PeekMessageW: sin nada, 0; tras PostQuitMessage(5), WM_QUIT
 *               con PM_NOREMOVE (y sigue ahi), y con PM_REMOVE (y ya no);
 *               AdjustWindowRect no achica; LoadCursorW(IDC_ARROW)
 *    kernel32   LoadLibraryExA
 *    oleaut32   GetErrorInfo sin error: S_FALSE y NULL; SysStringLen(NULL);
 *               SysFreeString(NULL)
 *    winrt      RoOriginateErrorW (se llama; lo que devuelve depende de si
 *               alguien escucha)
 *    el CRT     ceil y floor, con el double en xmm0
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;
typedef struct { HANDLE hwnd; unsigned mensaje; U64 w, l; DWORD t; long x, y; DWORD privado; } MSG;
typedef struct { long izq, arriba, der, abajo; } RECT;

#define IMPORTA __declspec(dllimport)
#define WINAPI __stdcall
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int c);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA HANDLE WINAPI LoadLibraryExA(const char *n, HANDLE f, DWORD banderas);
/* user32.dll */
IMPORTA int WINAPI PeekMessageW(MSG *m, HANDLE h, unsigned min, unsigned max, unsigned quitar);
IMPORTA void WINAPI PostQuitMessage(int c);
IMPORTA int WINAPI AdjustWindowRect(RECT *r, DWORD estilo, int menu);
IMPORTA HANDLE WINAPI LoadCursorW(HANDLE inst, const WCHAR *id);
/* oleaut32.dll */
IMPORTA long WINAPI GetErrorInfo(DWORD r, void **info);
IMPORTA unsigned WINAPI SysStringLen(WCHAR *b);
IMPORTA void WINAPI SysFreeString(WCHAR *b);
/* api-ms-win-core-winrt-error-l1-1-0.dll */
IMPORTA int WINAPI RoOriginateErrorW(long hr, unsigned n, const WCHAR *texto);
/* api-ms-win-crt-math-l1-1-0.dll */
IMPORTA double __cdecl ceil(double x);
IMPORTA double __cdecl floor(double x);

#define WM_QUIT 0x12
#define PM_NOREMOVE 0
#define PM_REMOVE 1
#define WS_OVERLAPPEDWINDOW 0x00CF0000

int _fltused = 0;
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

/* Que el compilador no calcule ceil por su cuenta: el valor llega de fuera. */
static volatile double dos_tres = 2.3, menos_dos_tres = -2.3;

void inicio(void) {
    MSG m;
    RECT r = {0, 0, 640, 480};
    void *info = (void *)1;
    salida = GetStdHandle((DWORD)-11);
    di("peek.exe: lo chico que le faltaba a BMOX-12\r\n");

    mira(PeekMessageW(&m, 0, 0, 0, PM_REMOVE) == 0, "PeekMessageW sin nada: 0, y no espera", 0);
    PostQuitMessage(5);
    mira(PeekMessageW(&m, 0, 0, 0, PM_NOREMOVE) == 1 && m.mensaje == WM_QUIT && m.w == 5, "tras PostQuitMessage(5): WM_QUIT con PM_NOREMOVE", m.mensaje);
    mira(PeekMessageW(&m, 0, 0, 0, PM_REMOVE) == 1 && m.mensaje == WM_QUIT, "sigue ahi, y con PM_REMOVE sale", m.w);
    mira(PeekMessageW(&m, 0, 0, 0, PM_REMOVE) == 0, "y ya no esta", 0);
    mira(AdjustWindowRect(&r, WS_OVERLAPPEDWINDOW, 0) && r.der - r.izq >= 640 && r.abajo - r.arriba >= 480, "AdjustWindowRect no achica", (U64)(r.der - r.izq));
    mira(LoadCursorW(0, (const WCHAR *)(U64)32512) != 0, "LoadCursorW(IDC_ARROW)", 0);
    mira(LoadLibraryExA("kernel32.dll", 0, 0) != 0, "LoadLibraryExA(kernel32.dll)", 0);

    mira(GetErrorInfo(0, &info) == 1 && info == 0, "GetErrorInfo sin error: S_FALSE y NULL", 0);
    SysFreeString(0);
    mira(SysStringLen(0) == 0, "SysStringLen(NULL) y SysFreeString(NULL)", 0);
    RoOriginateErrorW((long)0x80004005, 0, 0);
    mira(1, "RoOriginateErrorW se deja llamar", 0);

    mira(ceil(dos_tres) == 3.0 && ceil(menos_dos_tres) == -2.0, "ceil: 3 y -2 (el double en xmm0)", 0);
    mira(floor(dos_tres) == 2.0 && floor(menos_dos_tres) == -3.0, "floor: 2 y -3", 0);

    di(fallos ? "peek.exe: ALGO NO es como en Windows\r\n" : "peek.exe: lo chico de BMOX-12 es lo de Windows\r\n");
    ExitProcess(fallos);
}
