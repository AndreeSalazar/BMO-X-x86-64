/* tanda29.c -- el .exe de la TANDA 29 (01-10): CARPETAS que se crean,
 * ficheros que se renombran, se mueven y se borran. Para el perfil de un
 * juego en ESTRATOS (relevo de PROTON-X, paso 4b): AppData de Cyberpunk y
 * su UserSettings.json, las partidas, los logs. Todo dentro de una carpeta
 * propia (t29) junto al .exe, y al acabar no queda nada.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA int W ReadFile(HANDLE h, void *b, DWORD n, DWORD *e, void *o);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA HANDLE W CreateFileW(const WCHAR *n, DWORD acc, DWORD comp, void *seg, DWORD disp, DWORD band, HANDLE pl);
IMPORTA int W CreateDirectoryW(const WCHAR *n, void *seg);
IMPORTA int W RemoveDirectoryW(const WCHAR *n);
IMPORTA int W DeleteFileW(const WCHAR *n);
IMPORTA int W MoveFileExW(const WCHAR *a, const WCHAR *b, DWORD band);
IMPORTA DWORD W GetFileAttributesW(const WCHAR *n);

#define GENERIC_READ 0x80000000u
#define GENERIC_WRITE 0x40000000u
#define CREATE_ALWAYS 2
#define OPEN_EXISTING 3
#define NO_ATRIBUTOS 0xFFFFFFFFu
#define DIRECTORIO 0x10
#define ERROR_FILE_NOT_FOUND 2
#define ERROR_ALREADY_EXISTS 183
#define ERROR_DIR_NOT_EMPTY 145
#define MOVEFILE_REPLACE_EXISTING 1
#define NO_VALE ((HANDLE)(long long)-1)

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

static int escribir(const WCHAR *n, const char *t, DWORD k) {
    DWORD e = 0;
    HANDLE h = CreateFileW(n, GENERIC_WRITE, 0, 0, CREATE_ALWAYS, 0, 0);
    if (h == NO_VALE)
        return 0;
    WriteFile(h, t, k, &e, 0);
    return CloseHandle(h) && e == k;
}

static int leer_hola(const WCHAR *n) {
    char b[8] = {0};
    DWORD e = 0;
    HANDLE h = CreateFileW(n, GENERIC_READ, 0, 0, OPEN_EXISTING, 0, 0);
    if (h == NO_VALE)
        return 0;
    ReadFile(h, b, 8, &e, 0);
    CloseHandle(h);
    return e == 4 && b[0] == 'h' && b[1] == 'o' && b[2] == 'l' && b[3] == 'a';
}

void inicio(void);

void inicio(void) {
    static const WCHAR dir[] = {'t', '2', '9', 0};
    static const WCHAR sub[] = {'t', '2', '9', '\\', 'A', 'p', 'p', 'D', 'a', 't', 'a', 0};
    static const WCHAR a[] = {'t', '2', '9', '\\', 'A', 'p', 'p', 'D', 'a', 't', 'a', '\\', 'a', '.', 't', 'x', 't', 0};
    static const WCHAR b[] = {'t', '2', '9', '\\', 'A', 'p', 'p', 'D', 'a', 't', 'a', '\\', 'b', '.', 't', 'x', 't', 0};
    static const WCHAR c[] = {'t', '2', '9', '\\', 'c', '.', 't', 'x', 't', 0};
    static const WCHAR huerfano[] = {'n', 'o', '_', 'h', 'a', 'y', '\\', 'x', 0};
    DWORD at;
    mira(CreateDirectoryW(dir, 0), "CreateDirectoryW(t29)");
    at = GetFileAttributesW(dir);
    mira(at != NO_ATRIBUTOS && (at & DIRECTORIO), "y es una carpeta");
    mira(!CreateDirectoryW(dir, 0) && GetLastError() == ERROR_ALREADY_EXISTS, "otra vez: FALSE y ERROR_ALREADY_EXISTS");
    mira(!CreateDirectoryW(huerfano, 0) && GetLastError() == 3, "sin su carpeta de arriba: ERROR_PATH_NOT_FOUND");
    mira(CreateDirectoryW(sub, 0), "una dentro: t29\\AppData");
    mira(escribir(a, "hola", 4), "un fichero dentro: a.txt");
    mira(MoveFileExW(a, b, 0), "MoveFileExW en la misma carpeta (renombrar)");
    mira(GetFileAttributesW(a) == NO_ATRIBUTOS && leer_hola(b), "a.txt ya no esta; b.txt dice hola");
    mira(escribir(c, "xxxx", 4), "otro arriba: c.txt");
    mira(!MoveFileExW(b, c, 0) && GetLastError() == ERROR_ALREADY_EXISTS, "a uno que ya esta, sin REPLACE: no");
    mira(MoveFileExW(b, c, MOVEFILE_REPLACE_EXISTING), "con REPLACE_EXISTING, a otra carpeta: si");
    mira(GetFileAttributesW(b) == NO_ATRIBUTOS && leer_hola(c), "b.txt ya no esta; c.txt dice hola");
    mira(!RemoveDirectoryW(dir) && GetLastError() == ERROR_DIR_NOT_EMPTY, "RemoveDirectoryW de una llena: FALSE y ERROR_DIR_NOT_EMPTY");
    mira(DeleteFileW(c), "DeleteFileW(c.txt)");
    mira(!DeleteFileW(c) && GetLastError() == ERROR_FILE_NOT_FOUND, "otra vez: ERROR_FILE_NOT_FOUND");
    mira(RemoveDirectoryW(sub), "RemoveDirectoryW(t29\\AppData), vacia");
    mira(RemoveDirectoryW(dir), "RemoveDirectoryW(t29), vacia");
    mira(GetFileAttributesW(dir) == NO_ATRIBUTOS, "y ya no esta");
    di("tanda29.exe: carpetas y ficheros que se crean, se mueven y se borran como en Windows\r\n");
    ExitProcess(fallos);
}
