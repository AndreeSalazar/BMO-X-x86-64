/* tanda33.c -- el .exe de la TANDA 33 (01-10): secur32 y su tabla SSPI.
 * Por Cyberpunk: Galaxy (REDGalaxy64.dll), ya con la red local, arranca
 * libcurl en un hilo suyo. Curl mira la carpeta del sistema, carga
 * "<sistema>\secur32.dll" por su ruta entera, pide InitSecurityInterfaceW y
 * exige la tabla; la casa no tenia secur32, curl no arranco y Galaxy aborto
 * (exit 3). Aqui el mismo camino, y lo que curl usa de la tabla: que exista,
 * su version, las entradas de Schannel no nulas, y que un paquete que no
 * existe conteste SEC_E_SECPKG_NOT_FOUND. Nada depende de que paquetes haya.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
typedef long long (__stdcall *PROC)(void);
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA unsigned W GetSystemDirectoryW(WCHAR *b, unsigned n);
IMPORTA HANDLE W LoadLibraryExW(const WCHAR *n, HANDLE f, DWORD b);
IMPORTA HANDLE W LoadLibraryW(const WCHAR *n);
IMPORTA PROC W GetProcAddress(HANDLE m, const char *n);

/* SecurityFunctionTableW de sspi.h: dwVersion y 31 punteros. */
typedef struct {
    unsigned long version;
    void *f[31];
} Tabla;
typedef Tabla *(W *Iniciar)(void);
typedef long (W *Info)(const WCHAR *paquete, void **info);

#define SEC_E_SECPKG_NOT_FOUND ((long)0x80090305)

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

void inicio(void) {
    static WCHAR ruta[300];
    static const WCHAR nombre[] = L"\\secur32.dll";
    /* Las que usa el Schannel de curl: Acquire, FreeCredentials,
     * InitializeSecurityContext, DeleteSecurityContext, QueryContext,
     * FreeContextBuffer, QuerySecurityPackageInfo, Encrypt, Decrypt. */
    static const int usadas[] = {2, 3, 5, 8, 10, 15, 16, 24, 25};
    unsigned n, i, todas;
    HANDLE h;
    Iniciar iw = 0, ia = 0;
    Tabla *t = 0, *ta = 0;
    void *info = (void *)1;
    long r;

    n = GetSystemDirectoryW(ruta, 260);
    mira(n > 0 && n < 260, "GetSystemDirectoryW: la carpeta del sistema");
    for (i = 0; nombre[i]; i++)
        ruta[n + i] = nombre[i];
    ruta[n + i] = 0;
    h = LoadLibraryExW(ruta, 0, 0);
    mira(h != 0, "LoadLibraryExW(\"<sistema>\\secur32.dll\"): cargada");
    mira(h != 0 && LoadLibraryW(L"secur32.dll") == h, "LoadLibraryW(\"secur32.dll\"): el mismo modulo");
    if (h) {
        iw = (Iniciar)GetProcAddress(h, "InitSecurityInterfaceW");
        ia = (Iniciar)GetProcAddress(h, "InitSecurityInterfaceA");
    }
    mira(iw != 0 && ia != 0, "GetProcAddress: InitSecurityInterfaceW y A");
    if (iw)
        t = iw();
    if (ia)
        ta = ia();
    mira(t != 0 && t->version >= 1, "InitSecurityInterfaceW: la tabla, version 1 o mas");
    mira(ta != 0 && ta->version >= 1, "InitSecurityInterfaceA: la suya tambien");
    todas = t != 0;
    for (i = 0; t && i < sizeof usadas / sizeof usadas[0]; i++)
        todas &= t->f[usadas[i]] != 0;
    mira(todas, "las nueve entradas que usa el Schannel de curl: no nulas");
    r = t && t->f[16] ? ((Info)t->f[16])(L"PaqueteQueNoExisteBMOX", &info) : 0;
    mira(r == SEC_E_SECPKG_NOT_FOUND, "QuerySecurityPackageInfoW(\"no existe\"): SEC_E_SECPKG_NOT_FOUND");
    di("tanda33.exe: secur32 por su ruta y la tabla SSPI que pide curl\r\n");
    ExitProcess(fallos);
}
