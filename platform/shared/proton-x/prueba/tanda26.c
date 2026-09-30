/* tanda26.c -- el .exe de la TANDA 26 de Cyberpunk (30-09):
 * GetModuleHandleExW con GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS. En el metal,
 * pasada la memoria, el juego lo pidio dos veces justo antes de crear su
 * primer hilo, y la casa decia que no sabia que modulo tenia la direccion.
 * Aqui se pregunta por direcciones del propio .exe (su codigo, sus datos, su
 * cabecera, su ultimo byte) y por una que no es de nadie.
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
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA int W GetModuleHandleExW(DWORD banderas, const WCHAR *n, HANDLE *h);

#define DESDE_DIRECCION 4 /* GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS */
#define SIN_CUENTA 2      /* GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT */
#define ERROR_MOD_NOT_FOUND 126

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

static int dato = 5;

void inicio(void);

static HANDLE de(const void *p, DWORD banderas, int *ok) {
    HANDLE h = (HANDLE)1;
    *ok = GetModuleHandleExW(banderas, (const WCHAR *)p, &h);
    return h;
}

void inicio(void) {
    HANDLE yo = GetModuleHandleW(0);
    const unsigned char *b = (const unsigned char *)yo;
    DWORD medida = *(const DWORD *)(b + *(const DWORD *)(b + 0x3C) + 0x50);
    int ok;
    HANDLE h;
    h = de((const void *)inicio, DESDE_DIRECCION | SIN_CUENTA, &ok);
    mira(ok && h == yo, "desde su propio codigo (inicio): el .exe");
    h = de((const void *)&dato, DESDE_DIRECCION | SIN_CUENTA, &ok);
    mira(ok && h == yo, "desde sus datos: el .exe");
    h = de(b, DESDE_DIRECCION | SIN_CUENTA, &ok);
    mira(ok && h == yo, "desde su cabecera (la base misma): el .exe");
    h = de(b + medida - 1, DESDE_DIRECCION | SIN_CUENTA, &ok);
    mira(ok && h == yo, "desde su ultimo byte (SizeOfImage - 1): el .exe");
    h = de((const void *)inicio, DESDE_DIRECCION, &ok);
    mira(ok && h == yo, "sin UNCHANGED_REFCOUNT: el .exe igual");
    SetLastError(0);
    h = de((const void *)0x10, DESDE_DIRECCION | SIN_CUENTA, &ok);
    mira(!ok && h == 0 && GetLastError() == ERROR_MOD_NOT_FOUND, "desde 0x10: FALSE, NULL y ERROR_MOD_NOT_FOUND");
    di("tanda26.exe: GetModuleHandleExW desde una direccion es el de Windows\r\n");
    ExitProcess(fallos);
}
