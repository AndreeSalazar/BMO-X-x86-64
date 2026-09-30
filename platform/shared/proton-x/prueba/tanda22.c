/* tanda22.c -- el .exe de la TANDA 22 de Cyberpunk (30-09): el TLS de una
 * DLL. libxess_fg.dll del juego tiene __declspec(thread) y un callback de
 * TLS; en el metal su DllMain leyo 0+0x8: la casa solo daba TLS al .exe. En
 * Windows cada modulo con TLS tiene su indice (el .exe el 0, las DLL de
 * arranque detras), su bloque en cada hilo, y sus callbacks antes de su
 * DllMain. Este .exe tiene TLS propio (asi la DLL es el indice 1, como en
 * el juego) e importa tanda22d.dll, que va en su misma carpeta.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef unsigned long long U64;
typedef unsigned long DWORD;
typedef void *HANDLE;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
typedef void(W *CB)(void *, DWORD, void *);
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W CreateThread(void *a, U64 pila, DWORD(W *f)(void *), void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD W WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA int leer(void);
IMPORTA void poner(int v);
IMPORTA int en_main(void);
IMPORTA int antes_de_main(void);
IMPORTA int callbacks_hilo(void);

#pragma section(".tls", read, write)
#pragma section(".tls$ZZZ", read, write)
#pragma section(".CRT$XLA", read)
#pragma section(".CRT$XLZ", read)
__declspec(allocate(".tls")) char _tls_start = 0;
__declspec(allocate(".tls$ZZZ")) char _tls_end = 0;
DWORD _tls_index = 0;
__declspec(allocate(".CRT$XLA")) CB __xl_a = 0;
__declspec(allocate(".CRT$XLZ")) CB __xl_z = 0;
typedef struct {
    U64 ini, fin, indice, callbacks;
    DWORD ceros, caracteristicas;
} DirTls;
const DirTls _tls_used = {(U64)&_tls_start, (U64)&_tls_end, (U64)&_tls_index, (U64)(&__xl_a + 1), 0, 0};

static __declspec(thread) int del_exe = 5;

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

static volatile int en_hilo_dll = -1, en_hilo_exe = -1;

static DWORD W hilo(void *p) {
    (void)p;
    en_hilo_dll = leer();
    en_hilo_exe = del_exe;
    poner(55);
    del_exe = 66;
    return 0;
}

void inicio(void);

void inicio(void) {
    HANDLE h;
    mira(antes_de_main() == 1, "el callback de TLS de la DLL corre ANTES de su DllMain");
    mira(en_main() == 42, "su DllMain ve su variable de hilo con su valor inicial");
    mira(leer() == 7, "lo que su DllMain escribio, lo ve el hilo principal");
    mira(del_exe == 5, "el .exe tiene su propio TLS, aparte");
    poner(100);
    del_exe = 200;
    mira(leer() == 100 && del_exe == 200, "cada modulo escribe en lo suyo");
    h = CreateThread(0, 0, hilo, 0, 0, 0);
    mira(h != 0 && WaitForSingleObject(h, 5000) == 0, "un hilo nuevo corre y acaba");
    mira(en_hilo_dll == 42 && en_hilo_exe == 5, "el hilo nuevo tiene bloques NUEVOS, con los valores iniciales");
    mira(leer() == 100 && del_exe == 200, "lo que escribio el hilo no toca los del principal");
    mira(callbacks_hilo() >= 1, "el callback de TLS de la DLL vio el THREAD_ATTACH");
    CloseHandle(h);
    di("tanda22.exe: el TLS de una DLL es el de Windows\r\n");
    ExitProcess(fallos);
}
