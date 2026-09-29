/* tanda3b.c -- el .exe de la TANDA 3 de Cyberpunk, paso 4a (29-09): el pool
 * de hilos de kernel32 (trabajos, relojes, esperas y
 * RegisterWaitForSingleObject), InitOnce y las SList.
 *
 * Los callbacks corren en OTROS hilos: lo que esperan se mira con un tope
 * (hasta 2 s, de 5 en 5 ms), no con la suerte del orden.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long long I64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA void W Sleep(DWORD ms);
IMPORTA HANDLE W CreateEventW(void *a, int manual, int inicial, const void *n);
IMPORTA int W SetEvent(HANDLE h);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA int W InitOnceExecuteOnce(void **io, int(W *f)(void **, void *, void **), void *p, void **ctx);
IMPORTA int W InitOnceBeginInitialize(void **io, DWORD f, int *pendiente, void **ctx);
IMPORTA int W InitOnceComplete(void **io, DWORD f, void *ctx);
IMPORTA void W InitializeSListHead(void *h);
IMPORTA void *W InterlockedPushEntrySList(void *h, void *e);
IMPORTA void *W InterlockedPopEntrySList(void *h);
IMPORTA void *W InterlockedFlushSList(void *h);
IMPORTA unsigned short W QueryDepthSList(void *h);
IMPORTA void *W CreateThreadpoolWork(void(W *f)(void *, void *, void *), void *ctx, void *entorno);
IMPORTA void W SubmitThreadpoolWork(void *w);
IMPORTA void W WaitForThreadpoolWorkCallbacks(void *w, int cancelar);
IMPORTA void W CloseThreadpoolWork(void *w);
IMPORTA void *W CreateThreadpoolTimer(void(W *f)(void *, void *, void *), void *ctx, void *entorno);
IMPORTA void W SetThreadpoolTimer(void *t, I64 *vence, DWORD periodo, DWORD ventana);
IMPORTA int W IsThreadpoolTimerSet(void *t);
IMPORTA void W WaitForThreadpoolTimerCallbacks(void *t, int cancelar);
IMPORTA void W CloseThreadpoolTimer(void *t);
IMPORTA void *W CreateThreadpoolWait(void(W *f)(void *, void *, void *, DWORD), void *ctx, void *entorno);
IMPORTA void W SetThreadpoolWait(void *w, HANDLE h, I64 *plazo);
IMPORTA void W WaitForThreadpoolWaitCallbacks(void *w, int cancelar);
IMPORTA void W CloseThreadpoolWait(void *w);
IMPORTA int W RegisterWaitForSingleObject(HANDLE *nuevo, HANDLE h, void(W *f)(void *, unsigned char), void *ctx, DWORD ms, DWORD banderas);
IMPORTA int W UnregisterWait(HANDLE h);
IMPORTA int W UnregisterWaitEx(HANDLE h, HANDLE evento);

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

/* Lo que tocan los callbacks: de otros hilos. */
static volatile int cuenta, veces_init, ultimo;
static volatile int registrada, vencio;
static int marca;

static int leer(volatile int *v) {
    return __atomic_load_n(v, __ATOMIC_SEQ_CST);
}

static void sumar(volatile int *v) {
    __atomic_fetch_add(v, 1, __ATOMIC_SEQ_CST);
}

/* Esperar (hasta 2 s) a que *v llegue a n. */
static int hasta(volatile int *v, int n) {
    int k;
    for (k = 0; k < 400 && leer(v) < n; k++)
        Sleep(5);
    return leer(v) >= n;
}

static int W iniciar(void **io, void *p, void **ctx) {
    (void)io;
    sumar(&veces_init);
    *ctx = p;
    return 1;
}

static int W no_iniciar(void **io, void *p, void **ctx) {
    (void)io, (void)p, (void)ctx;
    return 0;
}

static void W trabajo(void *inst, void *ctx, void *w) {
    (void)inst, (void)w;
    if (ctx == &marca)
        sumar(&cuenta);
}

static void W reloj(void *inst, void *ctx, void *t) {
    (void)inst, (void)t;
    if (ctx == &marca)
        sumar(&cuenta);
}

static void W espera(void *inst, void *ctx, void *w, DWORD resultado) {
    (void)inst, (void)w;
    if (ctx == &marca) {
        ultimo = (int)resultado;
        sumar(&cuenta);
    }
}

static void W al_cumplirse(void *ctx, unsigned char fue_el_plazo) {
    if (ctx == &marca) {
        vencio = fue_el_plazo;
        sumar(&registrada);
    }
}

void inicio(void) {
    /* -- InitOnce */
    {
        static void *io, *io2, *io3;
        void *c1 = 0, *c2 = 0;
        int r1 = InitOnceExecuteOnce(&io, iniciar, &marca, &c1);
        int r2 = InitOnceExecuteOnce(&io, iniciar, 0, &c2);
        mira(r1 && r2 && veces_init == 1 && c1 == &marca && c2 == &marca, "InitOnceExecuteOnce: una vez, y el contexto las dos");
        mira(!InitOnceExecuteOnce(&io2, no_iniciar, 0, 0) && InitOnceExecuteOnce(&io2, iniciar, &marca, &c1) && veces_init == 2, "InitOnceExecuteOnce que falla: queda sin hacer");
        {
            int pendiente = 0;
            void *c = 0;
            int b1 = InitOnceBeginInitialize(&io3, 1, &pendiente, &c);
            int b2 = InitOnceBeginInitialize(&io3, 0, &pendiente, 0);
            int p2 = pendiente;
            int bc = InitOnceComplete(&io3, 0, (void *)0x1000);
            int b3 = InitOnceBeginInitialize(&io3, 1, &pendiente, &c);
            mira(!b1 && b2 && p2 && bc && b3 && !pendiente && c == (void *)0x1000, "InitOnceBeginInitialize y InitOnceComplete");
        }
    }
    /* -- SList */
    {
        static __declspec(align(16)) U64 cabeza[2];
        static __declspec(align(16)) U64 a[2], b[2], c[2];
        InitializeSListHead(cabeza);
        InterlockedPushEntrySList(cabeza, a);
        InterlockedPushEntrySList(cabeza, b);
        mira(InterlockedPushEntrySList(cabeza, c) == b && QueryDepthSList(cabeza) == 3, "InterlockedPushEntrySList y QueryDepthSList");
        mira(InterlockedPopEntrySList(cabeza) == c && QueryDepthSList(cabeza) == 2, "InterlockedPopEntrySList: el ultimo que entro");
        mira(InterlockedFlushSList(cabeza) == b && b[0] == (U64)a && QueryDepthSList(cabeza) == 0 && !InterlockedPopEntrySList(cabeza), "InterlockedFlushSList: la cadena entera, y la lista vacia");
    }
    /* -- trabajos */
    {
        void *w = CreateThreadpoolWork(trabajo, &marca, 0);
        cuenta = 0;
        SubmitThreadpoolWork(w);
        SubmitThreadpoolWork(w);
        SubmitThreadpoolWork(w);
        WaitForThreadpoolWorkCallbacks(w, 0);
        mira(w && leer(&cuenta) == 3, "SubmitThreadpoolWork tres veces y WaitForThreadpoolWorkCallbacks");
        CloseThreadpoolWork(w);
    }
    /* -- relojes */
    {
        void *t = CreateThreadpoolTimer(reloj, &marca, 0);
        I64 vence = -200000; /* 20 ms, relativo */
        int puesto;
        cuenta = 0;
        SetThreadpoolTimer(t, &vence, 0, 0);
        puesto = IsThreadpoolTimerSet(t);
        mira(t && puesto && hasta(&cuenta, 1), "SetThreadpoolTimer de 20 ms e IsThreadpoolTimerSet");
        cuenta = 0;
        vence = -100000;
        SetThreadpoolTimer(t, &vence, 10, 0);
        mira(hasta(&cuenta, 3), "SetThreadpoolTimer cada 10 ms: tres veces");
        SetThreadpoolTimer(t, 0, 0, 0);
        WaitForThreadpoolTimerCallbacks(t, 1);
        {
            int ahora = leer(&cuenta);
            Sleep(60);
            mira(!IsThreadpoolTimerSet(t) && leer(&cuenta) == ahora, "SetThreadpoolTimer NULL: se para");
        }
        CloseThreadpoolTimer(t);
    }
    /* -- esperas */
    {
        HANDLE e = CreateEventW(0, 1, 0, 0), nunca = CreateEventW(0, 1, 0, 0);
        void *w = CreateThreadpoolWait(espera, &marca, 0);
        I64 plazo = -200000;
        cuenta = 0;
        ultimo = -1;
        SetThreadpoolWait(w, e, 0);
        SetEvent(e);
        mira(w && hasta(&cuenta, 1) && ultimo == 0, "SetThreadpoolWait: el evento se cumple (WAIT_OBJECT_0)");
        SetThreadpoolWait(w, nunca, &plazo);
        mira(hasta(&cuenta, 2) && ultimo == 0x102, "SetThreadpoolWait con plazo: WAIT_TIMEOUT");
        WaitForThreadpoolWaitCallbacks(w, 1);
        CloseThreadpoolWait(w);
        CloseHandle(e);
        CloseHandle(nunca);
    }
    /* -- RegisterWaitForSingleObject */
    {
        HANDLE e = CreateEventW(0, 0, 0, 0), nunca = CreateEventW(0, 1, 0, 0), h = 0, h2 = 0;
        int r = RegisterWaitForSingleObject(&h, e, al_cumplirse, &marca, 0xFFFFFFFF, 0);
        registrada = 0;
        vencio = -1;
        SetEvent(e);
        mira(r && h && hasta(&registrada, 1) && vencio == 0, "RegisterWaitForSingleObject: se cumple");
        SetEvent(e);
        mira(hasta(&registrada, 2), "RegisterWaitForSingleObject: y otra vez (sin WT_EXECUTEONLYONCE)");
        mira(UnregisterWaitEx(h, (HANDLE)-1), "UnregisterWaitEx con INVALID_HANDLE_VALUE");
        SetEvent(e);
        Sleep(60);
        mira(leer(&registrada) == 2, "tras UnregisterWaitEx ya no se llama");
        registrada = 0;
        r = RegisterWaitForSingleObject(&h2, nunca, al_cumplirse, &marca, 20, 8);
        mira(r && hasta(&registrada, 1) && vencio == 1, "RegisterWaitForSingleObject con plazo: vence (TRUE)");
        Sleep(60);
        mira(leer(&registrada) == 1 && UnregisterWait(h2), "WT_EXECUTEONLYONCE: una vez; y UnregisterWait");
        CloseHandle(e);
        CloseHandle(nunca);
    }
    di("tanda3b.exe: el pool de hilos dice lo de Windows\r\n");
    ExitProcess(fallos);
}
