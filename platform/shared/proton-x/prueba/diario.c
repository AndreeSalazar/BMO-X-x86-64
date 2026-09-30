/* diario.c -- el .exe del DIARIO (P0.3 de PLAN_LAS_TRES_GRANDES, 30-09).
 *
 * Llama a funciones de Windows en un orden conocido, con los casos que un
 * trampolin podria romper: un double en xmm0 y xmm1 (pow), siete argumentos
 * con tres en la pila (CreateFileA), GetProcAddress (dos veces: el mismo
 * puntero) y funciones repetidas. En Windows, y en la casa sin diario, dice
 * lo mismo; con el diario encendido, el banco mira ademas que el fichero
 * tenga cada funcion UNA vez, en el orden de su primera llamada. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA void *W GetProcAddress(HANDLE m, const char *n);
IMPORTA DWORD W GetCurrentProcessId(void);
IMPORTA HANDLE W CreateFileA(const char *n, DWORD a, DWORD c, void *s, DWORD d, DWORD f, HANDLE t);
IMPORTA DWORD W GetLastError(void);
IMPORTA double pow(double a, double b);

/* Lo pide el compilador en cuanto hay un double; sin CRT, se pone aqui. */
int _fltused = 0;

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

typedef DWORD(W *Tick)(void);

static volatile double dos = 2.0, diez = 10.0;

void inicio(void) {
    mira(pow(dos, diez) == 1024.0 && pow(diez, dos) == 100.0, "pow(2, 10) y pow(10, 2): los double pasan en xmm0 y xmm1");
    {
        HANDLE k = GetModuleHandleW(L"kernel32.dll");
        Tick a = (Tick)GetProcAddress(k, "GetTickCount");
        Tick b = (Tick)GetProcAddress(k, "GetTickCount");
        mira(k && a && a == b && a() <= b() + 1000, "GetProcAddress dos veces: el mismo puntero, y se llama");
    }
    mira(GetCurrentProcessId() == GetCurrentProcessId() && GetCurrentProcessId() != 0, "GetCurrentProcessId tres veces: lo mismo");
    {
        HANDLE f = CreateFileA("no_esta_diario.txt", 0x80000000, 1, 0, 3, 0x80, 0);
        mira(f == (HANDLE)(long long)-1 && GetLastError() == 2, "CreateFileA de uno que no esta: siete argumentos, tres en la pila");
    }
    di("diario.exe: los trampolines no se notan\r\n");
    ExitProcess(fallos);
}
