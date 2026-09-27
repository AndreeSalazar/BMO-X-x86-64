/* ficheros.c -- el .exe de P4d de PROTON-X: los ficheros de Windows, como
 * los usa un juego para cargar sus datos (y guardar una partida).
 *
 *    CreateFileW   CREATE_ALWAYS escribe; OPEN_EXISTING lee; uno que no esta
 *                  da ERROR_FILE_NOT_FOUND; CREATE_NEW sobre uno que esta,
 *                  ERROR_FILE_EXISTS; OPEN_ALWAYS sobre uno que esta,
 *                  ERROR_ALREADY_EXISTS (y abre)
 *    WriteFile / ReadFile, y el final: exito con 0 leidos
 *    SetFilePointerEx desde el fin, SetFilePointer desde el principio, y
 *                  antes del principio: ERROR_NEGATIVE_SEEK
 *    GetFileSizeEx / GetFileSize / GetFileType / GetFileAttributesW
 *    CREATE_ALWAYS sobre uno que esta lo VACIA
 *
 * `pxtest.txt` va junto al .exe (su directorio actual: en BMO-X, `window`). Sale
 * con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int codigo);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA HANDLE WINAPI CreateFileW(const WCHAR *nombre, DWORD acceso, DWORD compartir, void *seg, DWORD disposicion, DWORD banderas, HANDLE plantilla);
IMPORTA int WINAPI ReadFile(HANDLE h, void *b, DWORD n, DWORD *leidos, void *ov);
IMPORTA int WINAPI CloseHandle(HANDLE h);
IMPORTA int WINAPI GetFileSizeEx(HANDLE h, I64 *medida);
IMPORTA DWORD WINAPI GetFileSize(HANDLE h, DWORD *alto);
IMPORTA int WINAPI SetFilePointerEx(HANDLE h, I64 dist, I64 *nueva, DWORD metodo);
IMPORTA DWORD WINAPI SetFilePointer(HANDLE h, long bajo, long *alto, DWORD metodo);
IMPORTA DWORD WINAPI GetFileAttributesW(const WCHAR *nombre);
IMPORTA DWORD WINAPI GetFileType(HANDLE h);

#define GENERIC_READ 0x80000000
#define GENERIC_WRITE 0x40000000
#define CREATE_NEW 1
#define CREATE_ALWAYS 2
#define OPEN_EXISTING 3
#define OPEN_ALWAYS 4
#define FILE_ATTRIBUTE_NORMAL 0x80
#define INVALID_HANDLE_VALUE ((HANDLE)(I64)-1)
#define FILE_BEGIN 0
#define FILE_CURRENT 1
#define FILE_END 2

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

static int igual(const char *a, const char *b, DWORD n) {
    DWORD i;
    for (i = 0; i < n; i++)
        if (a[i] != b[i]) return 0;
    return 1;
}

static const WCHAR NOMBRE[] = L"pxtest.txt";
static const char TEXTO[] = "hola fichero 0123456789";

void inicio(void) {
    HANDLE h;
    DWORD n = 0, alto = 7;
    I64 medida = 0, pos = 0;
    char b[32];
    salida = GetStdHandle((DWORD)-11);
    di("ficheros.exe: los ficheros de Windows\r\n");

    h = CreateFileW(NOMBRE, GENERIC_WRITE, 0, 0, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    mira(h != INVALID_HANDLE_VALUE && WriteFile(h, TEXTO, 23, &n, 0) && n == 23, "CreateFileW(CREATE_ALWAYS) y WriteFile de 23 bytes", n);
    mira(GetFileType(h) == 1, "GetFileType: un fichero de disco", GetFileType(h));
    mira(CloseHandle(h) != 0, "CloseHandle lo deja en el volumen", 1);
    mira(GetFileAttributesW(NOMBRE) != 0xFFFFFFFF, "GetFileAttributesW lo encuentra", 0);

    h = CreateFileW(NOMBRE, GENERIC_READ, 1, 0, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0);
    mira(h != INVALID_HANDLE_VALUE && GetFileSizeEx(h, &medida) && medida == 23, "OPEN_EXISTING y GetFileSizeEx: 23", (U64)medida);
    mira(GetFileSize(h, &alto) == 23 && alto == 0, "GetFileSize: 23, y la mitad alta 0", alto);
    mira(ReadFile(h, b, 4, &n, 0) && n == 4 && igual(b, "hola", 4), "ReadFile: los 4 primeros", n);
    mira(SetFilePointerEx(h, -10, &pos, FILE_END) && pos == 13, "SetFilePointerEx(-10, FILE_END): 13", (U64)pos);
    mira(ReadFile(h, b, 32, &n, 0) && n == 10 && igual(b, "0123456789", 10), "ReadFile lee lo que queda: 10", n);
    mira(ReadFile(h, b, 32, &n, 0) && n == 0, "al final: exito con 0 leidos", n);
    mira(SetFilePointer(h, 5, 0, FILE_BEGIN) == 5 && ReadFile(h, b, 7, &n, 0) && igual(b, "fichero", 7), "SetFilePointer(5, FILE_BEGIN) y leer 'fichero'", 5);
    mira(!SetFilePointerEx(h, -100, 0, FILE_CURRENT) && GetLastError() == 131, "antes del principio: ERROR_NEGATIVE_SEEK", GetLastError());
    CloseHandle(h);

    h = CreateFileW(L"no_existe_px.bin", GENERIC_READ, 1, 0, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0);
    mira(h == INVALID_HANDLE_VALUE && GetLastError() == 2, "uno que no esta: ERROR_FILE_NOT_FOUND", GetLastError());
    h = CreateFileW(NOMBRE, GENERIC_WRITE, 0, 0, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, 0);
    mira(h == INVALID_HANDLE_VALUE && GetLastError() == 80, "CREATE_NEW sobre uno que esta: ERROR_FILE_EXISTS", GetLastError());
    h = CreateFileW(NOMBRE, GENERIC_READ, 1, 0, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    mira(h != INVALID_HANDLE_VALUE && GetLastError() == 183, "OPEN_ALWAYS sobre uno que esta: abre, y ERROR_ALREADY_EXISTS", GetLastError());
    CloseHandle(h);

    h = CreateFileW(NOMBRE, GENERIC_WRITE, 0, 0, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    WriteFile(h, "corto", 5, &n, 0);
    CloseHandle(h);
    h = CreateFileW(NOMBRE, GENERIC_READ, 1, 0, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0);
    mira(GetFileSizeEx(h, &medida) && medida == 5 && ReadFile(h, b, 32, &n, 0) && n == 5 && igual(b, "corto", 5), "CREATE_ALWAYS sobre uno que esta lo VACIA: 5", (U64)medida);
    CloseHandle(h);

    di(fallos ? "ficheros.exe: ALGO NO es como en Windows\r\n" : "ficheros.exe: los ficheros son los de Windows\r\n");
    ExitProcess(fallos);
}
