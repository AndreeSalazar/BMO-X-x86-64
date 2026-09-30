/* tanda27.c -- el .exe de la TANDA 27 de Cyberpunk (30-09):
 * PEB->ProcessParameters. En el metal el primer hilo del juego (de
 * _beginthreadex) murio nada mas empezar: la UCRT, en
 * __acrt_get_begin_thread_init_policy, lee gs:[0x60] (el PEB), +0x20
 * (ProcessParameters) y el Flags de +0x08, y la casa tenia ese puntero a
 * cero. Aqui se lee igual, desde el hilo principal y desde un hilo de
 * CreateThread, como lo hace la UCRT.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W CreateThread(void *a, unsigned long long pila, DWORD(W *f)(void *), void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD W WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int W GetExitCodeThread(HANDLE h, DWORD *c);

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

static unsigned long long peb(void) {
    unsigned long long p;
    __asm__ volatile("movq %%gs:0x60, %0" : "=r"(p));
    return p;
}

static unsigned long long parametros(void) {
    return *(volatile unsigned long long *)(peb() + 0x20);
}

static volatile unsigned long long del_hilo, peb_del_hilo;

static DWORD W hilo(void *arg) {
    (void)arg;
    peb_del_hilo = peb();
    del_hilo = parametros();
    /* Lo que hace la UCRT: el Flags, y su bit 31. */
    return del_hilo && (int)*(volatile DWORD *)(del_hilo + 8) >= 0 ? 7 : 1;
}

void inicio(void);

void inicio(void) {
    unsigned long long p = parametros();
    DWORD banderas = p ? *(volatile DWORD *)(p + 8) : 0;
    HANDLE h;
    DWORD codigo = 0;
    mira(p != 0, "PEB->ProcessParameters no es NULL");
    mira(p != 0 && (banderas & 0x80000000u) == 0, "su Flags: el bit 31 (proceso seguro) a 0");
    mira(p != 0 && (banderas & 1) != 0, "su Flags: NORMALIZED (bit 0)");
    h = CreateThread(0, 0, hilo, 0, 0, 0);
    mira(h != 0, "CreateThread");
    mira(h != 0 && WaitForSingleObject(h, 5000) == 0, "el hilo acaba");
    mira(h != 0 && GetExitCodeThread(h, &codigo) && codigo == 7, "el hilo lee su Flags como la UCRT (sale con 7)");
    mira(peb_del_hilo == peb(), "el hilo ve el MISMO PEB");
    mira(del_hilo == p, "y los mismos ProcessParameters");
    di("tanda27.exe: PEB->ProcessParameters es el de Windows\r\n");
    ExitProcess(fallos);
}
