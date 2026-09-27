/* carpetas.c -- el .exe de P4f3 de PROTON-X: las carpetas y lo que se
 * pregunta de un fichero, como lo usan la std de Rust y un juego que busca
 * sus datos.
 *
 *    buscar       FindFirstFileW("pq?.txt") encuentra los dos que crea, con
 *                 su medida; FindNextFileW acaba con ERROR_NO_MORE_FILES; lo
 *                 que no hay: 2 (fichero) y 3 (carpeta)
 *    atributos    "." es una carpeta, un fichero no; GetFileAttributesExW da
 *                 la medida; SetFileAttributesW(NORMAL)
 *    rutas        GetCurrentDirectoryW, GetFullPathNameW (con su parte final,
 *                 y con "sub\.." por medio), SetCurrentDirectoryW a ".." y de
 *                 vuelta, y a un fichero: ERROR_DIRECTORY; GetTempPathW acaba
 *                 en '\'; GetLogicalDrives tiene C:
 *    una carpeta  abierta con FILE_FLAG_BACKUP_SEMANTICS (lo que hace la std
 *                 de Rust para metadata), y sin la bandera: acceso denegado
 *    un handle    GetFileInformationByHandle(Ex), SetFileInformationByHandle
 *                 (la medida), GetFinalPathNameByHandleW, LockFileEx
 *    copiar       CopyFileW pisando, y sin pisar: ERROR_FILE_EXISTS
 *    lo que falta DeleteFileW, RemoveDirectoryW y MoveFileExW de algo que no
 *                 esta: 2; CreateDirectoryW(".") : ERROR_ALREADY_EXISTS
 *
 * Crea pqa.txt, pqb.txt y pzc.txt en su directorio actual y los deja (se
 * puede correr otra vez: CREATE_ALWAYS y copiar pisando). Sale con el numero
 * de fallos. En Windows dice lo mismo. */
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
IMPORTA DWORD WINAPI GetFileAttributesW(const WCHAR *nombre);
IMPORTA HANDLE WINAPI FindFirstFileW(const WCHAR *patron, void *datos);
IMPORTA int WINAPI FindNextFileW(HANDLE h, void *datos);
IMPORTA int WINAPI FindClose(HANDLE h);
IMPORTA int WINAPI GetFileAttributesExW(const WCHAR *nombre, int nivel, void *datos);
IMPORTA int WINAPI SetFileAttributesW(const WCHAR *nombre, DWORD atr);
IMPORTA DWORD WINAPI GetCurrentDirectoryW(DWORD n, WCHAR *b);
IMPORTA int WINAPI SetCurrentDirectoryW(const WCHAR *d);
IMPORTA DWORD WINAPI GetFullPathNameW(const WCHAR *nombre, DWORD n, WCHAR *b, WCHAR **parte);
IMPORTA DWORD WINAPI GetTempPathW(DWORD n, WCHAR *b);
IMPORTA DWORD WINAPI GetLogicalDrives(void);
IMPORTA int WINAPI GetFileInformationByHandle(HANDLE h, void *info);
IMPORTA int WINAPI GetFileInformationByHandleEx(HANDLE h, int clase, void *b, DWORD n);
IMPORTA int WINAPI SetFileInformationByHandle(HANDLE h, int clase, void *b, DWORD n);
IMPORTA DWORD WINAPI GetFinalPathNameByHandleW(HANDLE h, WCHAR *b, DWORD n, DWORD banderas);
IMPORTA int WINAPI LockFileEx(HANDLE h, DWORD banderas, DWORD r, DWORD bajo, DWORD alto, void *ov);
IMPORTA int WINAPI UnlockFileEx(HANDLE h, DWORD r, DWORD bajo, DWORD alto, void *ov);
IMPORTA int WINAPI CopyFileW(const WCHAR *origen, const WCHAR *destino, int no_pisar);
IMPORTA int WINAPI DeleteFileW(const WCHAR *n);
IMPORTA int WINAPI RemoveDirectoryW(const WCHAR *n);
IMPORTA int WINAPI MoveFileExW(const WCHAR *a, const WCHAR *b, DWORD banderas);
IMPORTA int WINAPI CreateDirectoryW(const WCHAR *n, void *seg);

#define GENERIC_READ 0x80000000
#define GENERIC_WRITE 0x40000000
#define CREATE_ALWAYS 2
#define OPEN_EXISTING 3
#define FILE_ATTRIBUTE_DIRECTORY 0x10
#define FILE_ATTRIBUTE_NORMAL 0x80
#define FILE_FLAG_BACKUP_SEMANTICS 0x02000000
#define INVALID_HANDLE_VALUE ((HANDLE)(I64)-1)
#define LOCKFILE_EXCLUSIVE_LOCK 2

/* WIN32_FIND_DATAW: lo que se mira. */
typedef struct {
    DWORD atributos;
    DWORD fechas[6];
    DWORD alto, bajo;
    DWORD r0, r1;
    WCHAR nombre[260];
    WCHAR corto[14];
} HALLAZGO;

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

static WCHAR minus(WCHAR c) {
    return c >= 'A' && c <= 'Z' ? (WCHAR)(c + 32) : c;
}
/* `a` es `b` (en minusculas, sin mayusculas que cuenten). */
static int es(const WCHAR *a, const char *b) {
    int i;
    for (i = 0; b[i]; i++)
        if (minus(a[i]) != (WCHAR)b[i]) return 0;
    return a[i] == 0;
}
/* `a` acaba en `b`. */
static int acaba(const WCHAR *a, const char *b) {
    int n = 0, m = 0;
    while (a[n]) n++;
    while (b[m]) m++;
    return n >= m && es(a + n - m, b);
}
static int largo(const WCHAR *a) {
    int n = 0;
    while (a[n]) n++;
    return n;
}

static void crear(const WCHAR *n, const char *texto, DWORD k) {
    DWORD e;
    HANDLE h = CreateFileW(n, GENERIC_WRITE, 0, 0, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    WriteFile(h, texto, k, &e, 0);
    CloseHandle(h);
}

static void buscar(void) {
    HALLAZGO d;
    HANDLE h;
    int a = 0, b = 0, otros = 0;
    DWORD e;

    crear(L"pqa.txt", "hola!", 5);
    crear(L"pqb.txt", "abc", 3);
    h = FindFirstFileW(L"pq?.txt", &d);
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if (es(d.nombre, "pqa.txt") && d.bajo == 5 && !(d.atributos & FILE_ATTRIBUTE_DIRECTORY)) a++;
            else if (es(d.nombre, "pqb.txt") && d.bajo == 3) b++;
            else otros++;
        } while (FindNextFileW(h, &d));
        e = GetLastError();
        FindClose(h);
    } else {
        e = GetLastError();
    }
    mira(a == 1 && b == 1 && otros == 0, "FindFirstFileW(pq?.txt): los dos, con su medida", (U64)(a * 100 + b * 10 + otros));
    mira(e == 18, "y FindNextFileW acaba con ERROR_NO_MORE_FILES", e);
    h = FindFirstFileW(L"no_hay_px_*.zzz", &d);
    mira(h == INVALID_HANDLE_VALUE && GetLastError() == 2, "un patron sin nada: ERROR_FILE_NOT_FOUND", GetLastError());
    h = FindFirstFileW(L"no_existe_dir_px\\*", &d);
    mira(h == INVALID_HANDLE_VALUE && GetLastError() == 3, "en una carpeta que no esta: ERROR_PATH_NOT_FOUND", GetLastError());
}

static void atributos_y_rutas(void) {
    WCHAR cur[260], otra[260], full[260], *parte = 0;
    DWORD n, atr, datos[9];
    int i, prefijo;

    atr = GetFileAttributesW(L".");
    mira(atr != 0xFFFFFFFF && (atr & FILE_ATTRIBUTE_DIRECTORY), "GetFileAttributesW(.): una carpeta", atr);
    atr = GetFileAttributesW(L"pqa.txt");
    mira(atr != 0xFFFFFFFF && !(atr & FILE_ATTRIBUTE_DIRECTORY), "y pqa.txt, un fichero", atr);
    mira(GetFileAttributesExW(L"pqa.txt", 0, datos) && datos[8] == 5, "GetFileAttributesExW: su medida", datos[8]);
    mira(SetFileAttributesW(L"pqa.txt", FILE_ATTRIBUTE_NORMAL), "SetFileAttributesW(NORMAL)", 1);

    n = GetCurrentDirectoryW(260, cur);
    mira(n >= 3 && n == (DWORD)largo(cur) && cur[1] == ':' && cur[2] == '\\', "GetCurrentDirectoryW: X:\\...", n);
    n = GetFullPathNameW(L"pqa.txt", 260, full, &parte);
    for (i = 0, prefijo = 1; i < largo(cur); i++)
        if (minus(full[i]) != minus(cur[i])) prefijo = 0;
    mira(n == (DWORD)largo(full) && prefijo && acaba(full, "\\pqa.txt") && parte && es(parte, "pqa.txt"), "GetFullPathNameW: el actual y su nombre, con la parte final", n);
    n = GetFullPathNameW(L"sub\\..\\pqa.txt", 260, otra, 0);
    mira(n == (DWORD)largo(full) && es(otra, "") == 0 && acaba(otra, "\\pqa.txt") && n == (DWORD)largo(otra), "con sub\\.. por medio: la misma", n);
    mira(GetFullPathNameW(L"pqa.txt", 3, full, 0) > 3, "bufer corto: lo que hace falta", 0);

    mira(SetCurrentDirectoryW(L".."), "SetCurrentDirectoryW(..)", 1);
    n = GetCurrentDirectoryW(260, otra);
    for (i = 0, prefijo = 1; i < (int)n && i < 2; i++)
        if (minus(otra[i]) != minus(cur[i])) prefijo = 0;
    mira(n < (DWORD)largo(cur) && prefijo, "y el actual es mas corto", n);
    mira(SetCurrentDirectoryW(cur) && GetCurrentDirectoryW(260, otra) == (DWORD)largo(cur), "y de vuelta", 1);
    mira(!SetCurrentDirectoryW(L"pqa.txt") && GetLastError() == 267, "a un fichero: ERROR_DIRECTORY", GetLastError());

    n = GetTempPathW(260, otra);
    mira(n > 3 && otra[n - 1] == '\\', "GetTempPathW acaba en '\\'", n);
    mira((GetLogicalDrives() & 4) != 0, "GetLogicalDrives tiene C:", GetLogicalDrives());
}

static void una_carpeta_y_un_handle(void) {
    HANDLE h;
    DWORD info[13], std_[6], n, ov[8] = {0};
    I64 medida = 0, m2 = 2;
    WCHAR b[520];
    char t[8];

    h = CreateFileW(L".", GENERIC_READ, 7, 0, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, 0);
    mira(h != INVALID_HANDLE_VALUE && GetFileInformationByHandle(h, info) && (info[0] & FILE_ATTRIBUTE_DIRECTORY), "una carpeta abierta con FILE_FLAG_BACKUP_SEMANTICS", info[0]);
    CloseHandle(h);
    h = CreateFileW(L".", GENERIC_READ, 7, 0, OPEN_EXISTING, 0, 0);
    mira(h == INVALID_HANDLE_VALUE && GetLastError() == 5, "sin la bandera: ERROR_ACCESS_DENIED", GetLastError());

    h = CreateFileW(L"pqa.txt", GENERIC_READ | GENERIC_WRITE, 0, 0, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0);
    mira(GetFileInformationByHandle(h, info) && info[9] == 5 && info[10] == 1 && !(info[0] & FILE_ATTRIBUTE_DIRECTORY), "GetFileInformationByHandle: 5 bytes, un enlace", info[9]);
    mira(GetFileInformationByHandleEx(h, 1, std_, sizeof std_) && std_[2] == 5 && ((unsigned char *)std_)[21] == 0, "GetFileInformationByHandleEx(FileStandardInfo)", std_[2]);
    n = GetFinalPathNameByHandleW(h, b, 520, 0);
    mira(n > 8 && b[0] == '\\' && b[1] == '\\' && b[2] == '?' && acaba(b, "\\pqa.txt"), "GetFinalPathNameByHandleW: \\\\?\\...\\pqa.txt", n);
    mira(LockFileEx(h, LOCKFILE_EXCLUSIVE_LOCK, 0, 1, 0, ov) && UnlockFileEx(h, 0, 1, 0, ov), "LockFileEx y UnlockFileEx", 1);
    mira(SetFileInformationByHandle(h, 6, &m2, 8) && GetFileSizeEx(h, &medida) && medida == 2, "SetFileInformationByHandle(FileEndOfFileInfo): 2 bytes", (U64)medida);
    CloseHandle(h);
    h = CreateFileW(L"pqa.txt", GENERIC_READ, 1, 0, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0);
    mira(ReadFile(h, t, 8, &n, 0) && n == 2 && t[0] == 'h' && t[1] == 'o', "y en el volumen queda 'ho'", n);
    CloseHandle(h);
}

static void copiar_y_lo_que_falta(void) {
    HANDLE h;
    char t[8];
    DWORD n = 0;

    mira(CopyFileW(L"pqb.txt", L"pzc.txt", 0), "CopyFileW pisando", 1);
    mira(!CopyFileW(L"pqb.txt", L"pzc.txt", 1) && GetLastError() == 80, "y sin pisar: ERROR_FILE_EXISTS", GetLastError());
    h = CreateFileW(L"pzc.txt", GENERIC_READ, 1, 0, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0);
    mira(ReadFile(h, t, 8, &n, 0) && n == 3 && t[0] == 'a' && t[2] == 'c', "la copia dice 'abc'", n);
    CloseHandle(h);
    mira(!CopyFileW(L"no_existe_px.bin", L"pzd.txt", 0) && GetLastError() == 2, "copiar lo que no esta: 2", GetLastError());
    mira(!DeleteFileW(L"no_existe_px.bin") && GetLastError() == 2, "DeleteFileW de lo que no esta: 2", GetLastError());
    mira(!RemoveDirectoryW(L"no_existe_dir_px") && GetLastError() == 2, "RemoveDirectoryW de lo que no esta: 2", GetLastError());
    mira(!MoveFileExW(L"no_existe_px.bin", L"pzz.bin", 0) && GetLastError() == 2, "MoveFileExW de lo que no esta: 2", GetLastError());
    mira(!CreateDirectoryW(L".", 0) && GetLastError() == 183, "CreateDirectoryW(.): ERROR_ALREADY_EXISTS", GetLastError());
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    di("carpetas.exe: las carpetas y lo que se pregunta de un fichero\r\n");
    buscar();
    atributos_y_rutas();
    una_carpeta_y_un_handle();
    copiar_y_lo_que_falta();
    di(fallos ? "carpetas.exe: ALGO NO es como en Windows\r\n" : "carpetas.exe: las carpetas son las de Windows\r\n");
    ExitProcess(fallos);
}
