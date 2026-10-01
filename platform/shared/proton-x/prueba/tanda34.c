/* tanda34.c -- el .exe de la TANDA 34 (01-10): un HMODULE se puede LEER.
 * Por Cyberpunk: sl.interposer.dll (NVIDIA Streamline) pide dxgi.dll y lee
 * su cabecera PE en memoria (handle + 0x3C, el e_lfanew) para recorrer sus
 * exportaciones. En la casa el handle era un numero sin nada detras y el
 * juego caia con un fallo de pagina. Aqui lo mismo que hace un recorredor:
 * MZ, PE, x64, DLL, el directorio de exportaciones, su nombre, buscar una
 * funcion por la tabla y que sea la MISMA que da GetProcAddress (o, si es
 * un reenvio, que lo diga), y llamarla.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
typedef unsigned char BYTE;
typedef long long (__stdcall *PROC)(void);
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA HANDLE W LoadLibraryW(const WCHAR *n);
IMPORTA PROC W GetProcAddress(HANDLE m, const char *n);
IMPORTA DWORD W GetCurrentProcessId(void);

typedef DWORD (W *Pid)(void);

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

static unsigned short u16(const BYTE *p) { return (unsigned short)(p[0] | p[1] << 8); }
static DWORD u32(const BYTE *p) { return (DWORD)p[0] | (DWORD)p[1] << 8 | (DWORD)p[2] << 16 | (DWORD)p[3] << 24; }

static int igual_sin_mayusculas(const char *a, const char *b) {
    while (*a && *b) {
        char x = *a, y = *b;
        if (x >= 'A' && x <= 'Z') x += 32;
        if (y >= 'A' && y <= 'Z') y += 32;
        if (x != y)
            return 0;
        a++, b++;
    }
    return *a == *b;
}

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

/* Las cabeceras de la imagen `m`; el RVA y la medida de sus exportaciones. */
static int cabeceras(const BYTE *m, DWORD *exp, DWORD *exp_n) {
    const BYTE *nt;
    if (!m || m[0] != 'M' || m[1] != 'Z')
        return 0;
    nt = m + u32(m + 0x3C);
    if (nt[0] != 'P' || nt[1] != 'E' || nt[2] || nt[3])
        return 0;
    if (u16(nt + 4) != 0x8664 || !(u16(nt + 22) & 0x2000) || u16(nt + 24) != 0x20B || u32(nt + 24 + 108) < 1)
        return 0;
    *exp = u32(nt + 24 + 112);
    *exp_n = u32(nt + 24 + 116);
    return *exp != 0;
}

/* La direccion de `nombre` recorriendo la tabla; *reenvio si lo es. */
static const BYTE *buscar(const BYTE *m, DWORD exp, DWORD exp_n, const char *nombre, int *reenvio) {
    const BYTE *d = m + exp;
    DWORD n = u32(d + 24), i;
    const BYTE *nombres = m + u32(d + 32), *ords = m + u32(d + 36), *funcs = m + u32(d + 28);
    for (i = 0; i < n; i++) {
        if (igual((const char *)(m + u32(nombres + 4 * i)), nombre)) {
            DWORD rva = u32(funcs + 4 * u16(ords + 2 * i));
            *reenvio = rva >= exp && rva < exp + exp_n;
            return m + rva;
        }
    }
    return 0;
}

void inicio(void);

void inicio(void) {
    const BYTE *k = (const BYTE *)GetModuleHandleW(L"kernel32.dll");
    const BYTE *x, *f;
    DWORD exp = 0, exp_n = 0;
    int reenvio = 0;

    mira(k != 0, "GetModuleHandleW(\"kernel32.dll\"): un modulo");
    mira(cabeceras(k, &exp, &exp_n), "kernel32: MZ, PE, x64, DLL, PE32+ y exportaciones");
    mira(exp && igual_sin_mayusculas((const char *)(k + u32(k + exp + 12)), "kernel32.dll"), "su directorio de exportaciones se llama kernel32.dll");
    f = exp ? buscar(k, exp, exp_n, "GetCurrentProcessId", &reenvio) : 0;
    mira(f != 0, "GetCurrentProcessId, encontrada recorriendo la tabla");
    mira(f && (reenvio || ((Pid)(void *)f)() == GetCurrentProcessId()), "y llamada por ahi da el mismo pid (o es un reenvio)");

    x = (const BYTE *)LoadLibraryW(L"dxgi.dll");
    mira(x != 0 && x == (const BYTE *)GetModuleHandleW(L"dxgi.dll"), "LoadLibraryW(\"dxgi.dll\"): el mismo modulo que GetModuleHandleW");
    mira(cabeceras(x, &exp, &exp_n), "dxgi: MZ, PE, x64, DLL, PE32+ y exportaciones (lo que lee Streamline)");
    f = exp ? buscar(x, exp, exp_n, "CreateDXGIFactory1", &reenvio) : 0;
    mira(f != 0 && !reenvio, "CreateDXGIFactory1, encontrada recorriendo la tabla");
    mira(f && f == (const BYTE *)GetProcAddress((HANDLE)x, "CreateDXGIFactory1"), "y es la misma direccion que da GetProcAddress");
    di("tanda34.exe: un HMODULE es una imagen PE que se puede leer\r\n");
    ExitProcess(fallos);
}
