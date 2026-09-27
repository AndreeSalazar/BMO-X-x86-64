/* sistema.c -- el .exe de P4f4 de PROTON-X: lo demas que pide un runtime de
 * Windows (la std de Rust) de ntdll, bcryptprimitives, userenv, ws2_32 y
 * kernel32.
 *
 *    ntdll        NtWriteFile y NtReadFile sobre un fichero, con su
 *                 IO_STATUS_BLOCK, un desplazamiento y STATUS_END_OF_FILE al
 *                 final (asi lee y escribe ficheros la std de Rust);
 *                 RtlNtStatusToDosError
 *    el azar      ProcessPrng (el de HashMap)
 *    OVERLAPPED   ReadFile con un desplazamiento en un handle sincrono, y
 *                 GetOverlappedResult
 *    mensajes     FormatMessageW(FROM_SYSTEM) acaba en "\r\n"; con
 *                 ALLOCATE_BUFFER y LocalFree; uno que no hay: 317
 *    SetStdHandle la salida a un fichero y de vuelta
 *    nombres      GetWindowsDirectoryW, GetSystemDirectoryW,
 *                 GetUserProfileDirectoryW, QueryDosDeviceW(C:)
 *    la red       WSAStartup: o hay red (0) o no esta lista (10091), y sin
 *                 ella un socket no se crea (10093)
 *    procesos     GetProcessId y GetExitCodeProcess del propio; CreateProcessW
 *                 de un .exe que no esta: 2; la lista de atributos dice su
 *                 medida; y se sale con TerminateProcess
 *
 * Deja pnt.txt en su directorio actual. Sale con el numero de fallos. En
 * Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;
typedef long NTSTATUS;

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI SetStdHandle(DWORD n, HANDLE h);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA int WINAPI ReadFile(HANDLE h, void *b, DWORD n, DWORD *leidos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int codigo);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA HANDLE WINAPI CreateFileW(const WCHAR *nombre, DWORD acceso, DWORD compartir, void *seg, DWORD disposicion, DWORD banderas, HANDLE plantilla);
IMPORTA int WINAPI CloseHandle(HANDLE h);
IMPORTA int WINAPI GetOverlappedResult(HANDLE h, void *ov, DWORD *n, int esperar);
IMPORTA DWORD WINAPI FormatMessageW(DWORD banderas, void *fuente, DWORD id, DWORD idioma, WCHAR *b, DWORD n, void *args);
IMPORTA void *WINAPI LocalFree(void *p);
IMPORTA unsigned WINAPI GetWindowsDirectoryW(WCHAR *b, unsigned n);
IMPORTA unsigned WINAPI GetSystemDirectoryW(WCHAR *b, unsigned n);
IMPORTA DWORD WINAPI QueryDosDeviceW(const WCHAR *n, WCHAR *b, DWORD m);
IMPORTA HANDLE WINAPI GetCurrentProcess(void);
IMPORTA DWORD WINAPI GetCurrentProcessId(void);
IMPORTA DWORD WINAPI GetProcessId(HANDLE h);
IMPORTA int WINAPI GetExitCodeProcess(HANDLE h, DWORD *c);
IMPORTA int WINAPI TerminateProcess(HANDLE h, unsigned c);
IMPORTA int WINAPI CreateProcessW(const WCHAR *app, WCHAR *linea, void *a, void *b, int hereda, DWORD banderas, void *entorno, const WCHAR *dir, void *si, void *pi);
IMPORTA int WINAPI InitializeProcThreadAttributeList(void *l, DWORD n, DWORD banderas, U64 *medida);
/* ntdll.dll */
IMPORTA NTSTATUS WINAPI NtWriteFile(HANDLE h, HANDLE ev, void *apc, void *ctx, U64 *iosb, const void *b, DWORD n, I64 *off, DWORD *clave);
IMPORTA NTSTATUS WINAPI NtReadFile(HANDLE h, HANDLE ev, void *apc, void *ctx, U64 *iosb, void *b, DWORD n, I64 *off, DWORD *clave);
IMPORTA DWORD WINAPI RtlNtStatusToDosError(NTSTATUS s);
/* bcryptprimitives.dll */
IMPORTA int WINAPI ProcessPrng(unsigned char *b, U64 n);
/* userenv.dll */
IMPORTA int WINAPI GetUserProfileDirectoryW(HANDLE token, WCHAR *b, DWORD *n);
/* ws2_32.dll */
IMPORTA int WINAPI WSAStartup(unsigned short v, void *datos);
IMPORTA int WINAPI WSACleanup(void);
IMPORTA int WINAPI WSAGetLastError(void);
IMPORTA U64 WINAPI socket(int af, int tipo, int proto);
IMPORTA int WINAPI closesocket(U64 s);

#define GENERIC_READ 0x80000000
#define GENERIC_WRITE 0x40000000
#define CREATE_ALWAYS 2
#define FILE_ATTRIBUTE_NORMAL 0x80
#define STATUS_END_OF_FILE ((NTSTATUS)0xC0000011)
#define FORMAT_MESSAGE_ALLOCATE_BUFFER 0x100
#define FORMAT_MESSAGE_IGNORE_INSERTS 0x200
#define FORMAT_MESSAGE_FROM_SYSTEM 0x1000

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

static int igual(const char *a, const char *b, int n) {
    int i;
    for (i = 0; i < n; i++)
        if (a[i] != b[i]) return 0;
    return 1;
}

static int largo(const WCHAR *a) {
    int n = 0;
    while (a[n]) n++;
    return n;
}

static void el_azar(void) {
    unsigned char a[32], b[32];
    int i, iguales = 1, ceros = 1;
    mira(ProcessPrng(a, 32) && ProcessPrng(b, 32), "ProcessPrng: dos veces", 1);
    for (i = 0; i < 32; i++) {
        if (a[i] != b[i]) iguales = 0;
        if (a[i]) ceros = 0;
    }
    mira(!iguales && !ceros, "y no salen iguales ni a ceros", a[0]);
}

static void ntdll_y_overlapped(void) {
    U64 iosb[2] = {0, 0};
    I64 off;
    char b[16];
    DWORD n = 0, ov[8] = {0};
    NTSTATUS s;
    HANDLE h = CreateFileW(L"pnt.txt", GENERIC_READ | GENERIC_WRITE, 0, 0, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);

    s = NtWriteFile(h, 0, 0, 0, iosb, "0123456789", 10, 0, 0);
    mira(s == 0 && iosb[1] == 10, "NtWriteFile: 10 bytes, y el IO_STATUS_BLOCK lo dice", iosb[1]);
    off = 3;
    s = NtReadFile(h, 0, 0, 0, iosb, b, 4, &off, 0);
    mira(s == 0 && iosb[1] == 4 && igual(b, "3456", 4), "NtReadFile desde 3: '3456'", iosb[1]);
    off = 100;
    s = NtReadFile(h, 0, 0, 0, iosb, b, 4, &off, 0);
    mira(s == STATUS_END_OF_FILE && iosb[1] == 0, "pasado el final: STATUS_END_OF_FILE", (U64)(DWORD)s);
    mira(RtlNtStatusToDosError(STATUS_END_OF_FILE) == 38 && RtlNtStatusToDosError((NTSTATUS)0xC0000034) == 2, "RtlNtStatusToDosError: 38 y 2", 38);

    ov[4] = 5; /* Offset */
    mira(ReadFile(h, b, 3, &n, ov) && n == 3 && igual(b, "567", 3), "ReadFile con OVERLAPPED en 5: '567'", n);
    n = 0;
    mira(GetOverlappedResult(h, ov, &n, 0) && n == 3, "GetOverlappedResult: 3", n);

    mira(SetStdHandle((DWORD)-11, h) && GetStdHandle((DWORD)-11) == h, "SetStdHandle: la salida es el fichero", 1);
    SetStdHandle((DWORD)-11, salida);
    mira(GetStdHandle((DWORD)-11) == salida, "y de vuelta a la consola", 1);
    CloseHandle(h);
}

static void los_mensajes(void) {
    WCHAR b[256], *p = 0;
    DWORD n;
    n = FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, 0, 2, 0, b, 256, 0);
    mira(n > 4 && b[n - 2] == '\r' && b[n - 1] == '\n' && b[n] == 0, "FormatMessageW(2): un texto que acaba en \\r\\n", n);
    n = FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, 0, 5, 0, (WCHAR *)&p, 0, 0);
    mira(n > 2 && p && p[n] == 0 && LocalFree(p) == 0, "con ALLOCATE_BUFFER, y LocalFree", n);
    n = FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, 0, 0x0DEADBEE, 0, b, 256, 0);
    mira(n == 0 && GetLastError() == 317, "uno que no hay: ERROR_MR_MID_NOT_FOUND", GetLastError());
}

static void los_nombres(void) {
    WCHAR w[260], s[260], u[260], q[260];
    DWORD n = 260;
    unsigned a = GetWindowsDirectoryW(w, 260), b = GetSystemDirectoryW(s, 260);
    mira(a > 3 && w[1] == ':' && b > a, "GetWindowsDirectoryW y GetSystemDirectoryW: X:\\... y mas largo", a);
    mira(GetUserProfileDirectoryW((HANDLE)(I64)-4, u, &n) && u[1] == ':', "GetUserProfileDirectoryW", (U64)largo(u));
    mira(QueryDosDeviceW(L"C:", q, 260) > 0 && q[0] == '\\' && q[1] == 'D', "QueryDosDeviceW(C:): \\Device\\...", q[1]);
}

static void la_red(void) {
    unsigned char wsa[512];
    int r = WSAStartup(0x0202, wsa);
    U64 s;
    if (r == 0) {
        mira(1, "WSAStartup: hay red (0)", 0);
        s = socket(2, 1, 6);
        if (s != (U64)-1) closesocket(s);
        mira(1, "(con red no se prueba mas: aqui no hace falta)", 0);
        WSACleanup();
    } else {
        mira(r == 10091, "WSAStartup: la red no esta lista (WSASYSNOTREADY)", r);
        s = socket(2, 1, 6);
        mira(s == (U64)-1 && WSAGetLastError() == 10093, "y un socket no se crea: WSANOTINITIALISED", WSAGetLastError());
    }
}

static void los_procesos(void) {
    DWORD c = 0;
    U64 medida = 0;
    DWORD si[26] = {104};
    U64 pi[3];
    mira(GetProcessId(GetCurrentProcess()) == GetCurrentProcessId(), "GetProcessId del propio", GetCurrentProcessId());
    mira(GetExitCodeProcess(GetCurrentProcess(), &c) && c == 259, "GetExitCodeProcess: STILL_ACTIVE", c);
    mira(!CreateProcessW(L"no_existe_px.exe", 0, 0, 0, 0, 0, 0, 0, si, pi) && GetLastError() == 2, "CreateProcessW de un .exe que no esta: 2", GetLastError());
    mira(!InitializeProcThreadAttributeList(0, 1, 0, &medida) && GetLastError() == 122 && medida > 0, "InitializeProcThreadAttributeList(NULL): su medida", medida);
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    di("sistema.exe: ntdll, el azar, los mensajes, la red y los procesos\r\n");
    el_azar();
    ntdll_y_overlapped();
    los_mensajes();
    los_nombres();
    la_red();
    los_procesos();
    di(fallos ? "sistema.exe: ALGO NO es como en Windows\r\n" : "sistema.exe: lo demas es lo de Windows\r\n");
    TerminateProcess(GetCurrentProcess(), fallos);
    ExitProcess(0xBAD);
}
