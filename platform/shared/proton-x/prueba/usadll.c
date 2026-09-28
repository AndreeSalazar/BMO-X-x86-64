/* usadll.c -- el .exe de P5a de PROTON-X: usa una DLL que NO es de Windows
 * (saludo.dll, junto a el), como un juego usa las suyas.
 *
 *    importaciones   suma, frase y visto_attach de saludo.dll, resueltas al
 *                    cargar; su DllMain corrio ANTES que esta entrada y leyo
 *                    el mismo id de proceso
 *    en marcha       LoadLibraryA y GetModuleHandleA dan la MISMA base (ya
 *                    esta cargada); GetProcAddress por nombre y por ORDINAL
 *                    (#7) dan la misma funcion que la importacion; uno que no
 *                    esta: ERROR_PROC_NOT_FOUND
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;

#define IMPORTA __declspec(dllimport)
#define WINAPI __stdcall
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int c);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA DWORD WINAPI GetCurrentProcessId(void);
IMPORTA HANDLE WINAPI LoadLibraryA(const char *n);
IMPORTA HANDLE WINAPI GetModuleHandleA(const char *n);
IMPORTA void *WINAPI GetProcAddress(HANDLE m, const char *n);
/* saludo.dll */
IMPORTA int suma(int a, int b);
IMPORTA const char *frase(void);
IMPORTA int visto_attach(DWORD *pid);

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

static int igual(const char *a, const char *b) {
    while (*a && *a == *b) a++, b++;
    return *a == *b;
}

void inicio(void) {
    DWORD pid = 0;
    HANDLE h, g;
    int (*por_nombre)(int, int), (*por_ordinal)(int, int);
    salida = GetStdHandle((DWORD)-11);
    di("usadll.exe: una DLL propia, como las de un juego\r\n");

    mira(suma(40, 2) == 42, "suma(40, 2) de saludo.dll: 42", (U64)suma(40, 2));
    mira(igual(frase(), "hola desde saludo.dll"), "frase(): un texto de los datos de la DLL", 0);
    mira(visto_attach(&pid) == 1 && pid == GetCurrentProcessId(), "su DllMain corrio UNA vez antes que esta entrada, y en este proceso", pid);

    h = LoadLibraryA("saludo.dll");
    g = GetModuleHandleA("SALUDO");
    mira(h != 0 && h == g, "LoadLibraryA y GetModuleHandleA: la misma base (ya estaba cargada)", (U64)h);
    por_nombre = (int (*)(int, int))GetProcAddress(h, "suma");
    por_ordinal = (int (*)(int, int))GetProcAddress(h, (const char *)(U64)7);
    mira(por_nombre && por_nombre == por_ordinal && por_nombre(1, 2) == 3, "GetProcAddress por nombre y por ordinal (#7): la misma suma", (U64)(por_nombre == por_ordinal));
    mira(por_nombre == suma, "y es la misma que la importada", 0);
    mira(!GetProcAddress(h, "no_existe_px") && GetLastError() == 127, "una que no esta: ERROR_PROC_NOT_FOUND", GetLastError());
    mira(visto_attach(&pid) == 1, "y LoadLibraryA no volvio a llamar a DllMain", 1);

    di(fallos ? "usadll.exe: ALGO NO es como en Windows\r\n" : "usadll.exe: la DLL propia es como en Windows\r\n");
    ExitProcess(fallos);
}
