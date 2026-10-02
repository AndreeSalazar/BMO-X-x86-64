/* vueltas.c -- el hilo que ESPERA DANDO VUELTAS (02-10).
 *
 * Lo que hace un motor como el de Cyberpunk: el principal despierta a un
 * trabajador y lo espera mirando el reloj en un bucle, sin llamar a ninguna
 * espera de Windows. En Windows el reloj del sistema le quita el turno y el
 * trabajador corre. Con los hilos cooperativos de la casa, antes del 02-10,
 * el trabajador no corria nunca: el bucle solo acababa por su plazo (2 s) y
 * decia MAL. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef long long LONGLONG;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W CreateThread(void *a, unsigned long long pila, DWORD(W *f)(void *), void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD W WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int W QueryPerformanceCounter(LONGLONG *v);
IMPORTA int W QueryPerformanceFrequency(LONGLONG *v);

static unsigned fallos;
static volatile int listo;
static volatile unsigned cuenta;

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

static DWORD W trabajador(void *p) {
    unsigned i;
    (void)p;
    for (i = 0; i < 1000; i++)
        cuenta++;
    listo = 1;
    return 7;
}

/* Espera a que `listo` se encienda mirando el reloj; 1 si llego antes del
 * plazo de dos segundos. */
static int esperar_dando_vueltas(void) {
    LONGLONG f, t0, t;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&t0);
    do {
        if (listo)
            return 1;
        QueryPerformanceCounter(&t);
    } while (t - t0 < 2 * f);
    return listo;
}

void inicio(void) {
    HANDLE h = CreateThread(0, 0, trabajador, 0, 0, 0);
    mira(h != 0, "CreateThread: el trabajador");
    mira(esperar_dando_vueltas(), "el principal lo espera DANDO VUELTAS con QueryPerformanceCounter, y el trabajador corre");
    mira(cuenta == 1000, "el trabajador hizo su trabajo entero");
    mira(WaitForSingleObject(h, 1000) == 0, "y acabo: WaitForSingleObject da WAIT_OBJECT_0");
    di("vueltas.exe: un hilo que espera dando vueltas deja correr a los demas\r\n");
    ExitProcess(fallos);
}
