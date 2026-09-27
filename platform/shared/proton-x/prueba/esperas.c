/* esperas.c -- el .exe de P4f2 de PROTON-X: las esperas que la `std` de Rust
 * y un CRT piden ademas de las de hilos.exe.
 *
 *    mutex          CreateMutexW con propietario, recursion, ReleaseMutex de
 *                   mas (ERROR_NOT_OWNER), otro hilo que espera, y uno que
 *                   acaba sin soltarlo: WAIT_ABANDONED
 *    temporizadores CreateWaitableTimerW manual a 20 ms, y uno automatico
 *                   con periodo de 10 ms tres veces; CancelWaitableTimer
 *    WaitOnAddress  importado de api-ms-win-core-synch-l1-2-0.dll, como lo
 *                   importa la std de Rust: distinto vuelve en el acto, igual
 *                   duerme hasta WakeByAddressSingle o el plazo
 *    FLS            el callback corre al acabar el hilo y en FlsFree
 *    handles        DuplicateHandle del hilo actual (un handle de verdad) y
 *                   de un evento (cerrar uno no cierra el otro)
 *    la hora        GetSystemTimePreciseAsFileTime: despues de 2020, y no
 *                   va hacia atras
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef long LONG;
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int codigo);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA HANDLE WINAPI CreateThread(void *attr, U64 pila, DWORD(WINAPI *f)(void *), void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD WINAPI WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int WINAPI CloseHandle(HANDLE h);
IMPORTA void WINAPI Sleep(DWORD ms);
IMPORTA int WINAPI GetExitCodeThread(HANDLE h, DWORD *codigo);
IMPORTA HANDLE WINAPI CreateEventW(void *attr, int manual, int inicial, const WCHAR *n);
IMPORTA int WINAPI SetEvent(HANDLE h);
IMPORTA int WINAPI QueryPerformanceCounter(I64 *v);
IMPORTA int WINAPI QueryPerformanceFrequency(I64 *v);
IMPORTA HANDLE WINAPI CreateMutexW(void *attr, int inicial, const WCHAR *n);
IMPORTA int WINAPI ReleaseMutex(HANDLE h);
IMPORTA HANDLE WINAPI CreateWaitableTimerW(void *attr, int manual, const WCHAR *n);
IMPORTA HANDLE WINAPI CreateWaitableTimerExW(void *attr, const WCHAR *n, DWORD banderas, DWORD acceso);
IMPORTA int WINAPI SetWaitableTimer(HANDLE h, const I64 *vence, LONG periodo, void *rutina, void *arg, int reanudar);
IMPORTA int WINAPI CancelWaitableTimer(HANDLE h);
IMPORTA int WINAPI WaitOnAddress(volatile void *dir, void *cmp, U64 n, DWORD ms);
IMPORTA void WINAPI WakeByAddressSingle(void *dir);
IMPORTA DWORD WINAPI FlsAlloc(void(WINAPI *cb)(void *));
IMPORTA int WINAPI FlsFree(DWORD i);
IMPORTA void *WINAPI FlsGetValue(DWORD i);
IMPORTA int WINAPI FlsSetValue(DWORD i, void *v);
IMPORTA HANDLE WINAPI GetCurrentProcess(void);
IMPORTA HANDLE WINAPI GetCurrentThread(void);
IMPORTA int WINAPI DuplicateHandle(HANDLE p1, HANDLE h, HANDLE p2, HANDLE *d, DWORD acceso, int hereda, DWORD opciones);
IMPORTA int WINAPI IsThreadAFiber(void);
IMPORTA int WINAPI SetThreadStackGuarantee(unsigned long *n);
IMPORTA void WINAPI GetSystemTimePreciseAsFileTime(U64 *ft);

#define INFINITE 0xFFFFFFFF
#define WAIT_TIMEOUT 0x102
#define WAIT_ABANDONED 0x80
#define DUPLICATE_SAME_ACCESS 2
#define TIMER_ALL_ACCESS 0x1F0003

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

static I64 frecuencia;
static I64 ms_desde(I64 t0) {
    I64 t;
    QueryPerformanceCounter(&t);
    return (t - t0) * 1000 / frecuencia;
}

/* -- mutex ------------------------------------------------------------------ */

static HANDLE mutex;
static volatile LONG cogido;

static DWORD WINAPI coge_y_suelta(void *a) {
    (void)a;
    WaitForSingleObject(mutex, INFINITE);
    cogido = 1;
    ReleaseMutex(mutex);
    return 0;
}

static DWORD WINAPI coge_y_se_va(void *a) {
    (void)a;
    WaitForSingleObject(mutex, INFINITE);
    return 0;
}

static void los_mutex(void) {
    HANDLE h;
    DWORD r;
    mutex = CreateMutexW(0, 1, 0);
    mira(mutex && WaitForSingleObject(mutex, 0) == 0, "CreateMutexW con propietario, y otra vez: recursion", 0);
    mira(ReleaseMutex(mutex) && ReleaseMutex(mutex), "ReleaseMutex dos veces", 1);
    mira(!ReleaseMutex(mutex) && GetLastError() == 288, "una tercera: ERROR_NOT_OWNER", GetLastError());

    WaitForSingleObject(mutex, INFINITE);
    h = CreateThread(0, 0, coge_y_suelta, 0, 0, 0);
    Sleep(20);
    mira(cogido == 0, "otro hilo espera el mutex mientras es mio", cogido);
    ReleaseMutex(mutex);
    WaitForSingleObject(h, INFINITE);
    CloseHandle(h);
    mira(cogido == 1, "y al soltarlo, lo coge", cogido);

    h = CreateThread(0, 0, coge_y_se_va, 0, 0, 0);
    WaitForSingleObject(h, INFINITE);
    CloseHandle(h);
    r = WaitForSingleObject(mutex, 1000);
    mira(r == WAIT_ABANDONED, "un hilo acaba sin soltarlo: WAIT_ABANDONED", r);
    mira(ReleaseMutex(mutex) && WaitForSingleObject(mutex, 0) == 0, "y se coge normal despues", 0);
    ReleaseMutex(mutex);
    CloseHandle(mutex);
}

/* -- temporizadores ------------------------------------------------------------ */

static void los_temporizadores(void) {
    I64 t0, vence = -200000; /* 20 ms, relativo, en centenas de ns */
    HANDLE t = CreateWaitableTimerW(0, 1, 0);
    DWORD r, i, bien = 1;

    QueryPerformanceCounter(&t0);
    mira(t && SetWaitableTimer(t, &vence, 0, 0, 0, 0), "CreateWaitableTimerW manual y SetWaitableTimer a 20 ms", 0);
    mira(WaitForSingleObject(t, 0) == WAIT_TIMEOUT, "antes de su hora: WAIT_TIMEOUT", 0);
    r = WaitForSingleObject(t, 2000);
    mira(r == 0 && ms_desde(t0) >= 15, "a su hora: se enciende (y no antes)", (U64)ms_desde(t0));
    mira(WaitForSingleObject(t, 0) == 0, "manual: sigue encendido", 0);
    CloseHandle(t);

    t = CreateWaitableTimerExW(0, 0, 0, TIMER_ALL_ACCESS);
    vence = -100000;
    QueryPerformanceCounter(&t0);
    SetWaitableTimer(t, &vence, 10, 0, 0, 0);
    for (i = 0; i < 3; i++)
        if (WaitForSingleObject(t, 2000) != 0) bien = 0;
    mira(bien && ms_desde(t0) >= 25, "automatico con periodo de 10 ms: tres veces", (U64)ms_desde(t0));
    CancelWaitableTimer(t);
    Sleep(15);
    WaitForSingleObject(t, 0); /* lo que ya vencio antes de cancelar */
    mira(WaitForSingleObject(t, 40) == WAIT_TIMEOUT, "CancelWaitableTimer: ya no vuelve", 0);
    CloseHandle(t);
}

/* -- WaitOnAddress -------------------------------------------------------------- */

static volatile LONG valor;

static DWORD WINAPI cambia_y_despierta(void *a) {
    (void)a;
    Sleep(10);
    valor = 1;
    WakeByAddressSingle((void *)&valor);
    return 0;
}

static void las_direcciones(void) {
    LONG cmp = 7;
    HANDLE h;
    mira(WaitOnAddress(&valor, &cmp, 4, INFINITE), "WaitOnAddress con otro valor: vuelve en el acto", 1);
    cmp = 0;
    mira(!WaitOnAddress(&valor, &cmp, 4, 10) && GetLastError() == 1460, "con el mismo y 10 ms: ERROR_TIMEOUT", GetLastError());
    h = CreateThread(0, 0, cambia_y_despierta, 0, 0, 0);
    while (valor == 0) WaitOnAddress(&valor, &cmp, 4, INFINITE);
    mira(valor == 1, "otro hilo lo cambia y WakeByAddressSingle lo despierta", valor);
    WaitForSingleObject(h, INFINITE);
    CloseHandle(h);
}

/* -- FLS ---------------------------------------------------------------------------- */

static DWORD fls;
static volatile LONG llamadas;
static volatile U64 suma;

static void WINAPI al_soltar(void *v) {
    llamadas++;
    suma += (U64)v;
}

static DWORD WINAPI pone_fls(void *a) {
    (void)a;
    FlsSetValue(fls, (void *)0x77);
    return 0;
}

static void el_fls(void) {
    HANDLE h;
    fls = FlsAlloc(al_soltar);
    mira(fls != 0xFFFFFFFF && FlsGetValue(fls) == 0, "FlsAlloc: una ranura, a 0", fls);
    h = CreateThread(0, 0, pone_fls, 0, 0, 0);
    WaitForSingleObject(h, INFINITE);
    CloseHandle(h);
    mira(llamadas == 1 && suma == 0x77, "el callback corre al acabar el hilo, con SU valor", suma);
    mira(FlsSetValue(fls, (void *)5) && FlsGetValue(fls) == (void *)5, "FlsSetValue / FlsGetValue", 5);
    mira(FlsFree(fls) && llamadas == 2 && suma == 0x77 + 5, "FlsFree llama al callback con lo que quedaba", suma);
}

/* -- handles y lo demas ---------------------------------------------------------------- */

static void los_handles(void) {
    HANDLE yo = 0, ev, otro = 0;
    DWORD codigo = 0;
    unsigned long garantia = 0;
    U64 a = 0, b = 0;

    mira(DuplicateHandle(GetCurrentProcess(), GetCurrentThread(), GetCurrentProcess(), &yo, 0, 0, DUPLICATE_SAME_ACCESS) && GetExitCodeThread(yo, &codigo) && codigo == 259,
         "DuplicateHandle del hilo actual: un handle de verdad (STILL_ACTIVE)", codigo);
    CloseHandle(yo);
    ev = CreateEventW(0, 1, 0, 0);
    DuplicateHandle(GetCurrentProcess(), ev, GetCurrentProcess(), &otro, 0, 0, DUPLICATE_SAME_ACCESS);
    CloseHandle(ev);
    mira(otro && SetEvent(otro) && WaitForSingleObject(otro, 0) == 0, "un evento duplicado: cerrar uno no cierra el otro", 0);
    CloseHandle(otro);
    mira(!IsThreadAFiber() && SetThreadStackGuarantee(&garantia), "IsThreadAFiber no, SetThreadStackGuarantee si", garantia);
    GetSystemTimePreciseAsFileTime(&a);
    Sleep(2);
    GetSystemTimePreciseAsFileTime(&b);
    mira(a > 132223104000000000ull && b >= a, "GetSystemTimePreciseAsFileTime: despues de 2020, y no va hacia atras", a);
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    QueryPerformanceFrequency(&frecuencia);
    di("esperas.exe: mutex, temporizadores, WaitOnAddress, FLS y handles\r\n");
    los_mutex();
    los_temporizadores();
    las_direcciones();
    el_fls();
    los_handles();
    di(fallos ? "esperas.exe: ALGO NO es como en Windows\r\n" : "esperas.exe: las esperas son las de Windows\r\n");
    ExitProcess(fallos);
}
