/* tanda9.c -- el .exe de la TANDA 9 de Cyberpunk (30-09): lo que quedaba
 * de kernel32. Fibras, SuspendThread, QueueUserAPC, TerminateThread,
 * Toolhelp, psapi (K32*), el procesador, la pila (RtlCaptureStackBackTrace,
 * RtlPcToFileHeader), discos y lo demas. Solo RELACIONES (el id propio, la
 * base propia, "cabe o no cabe"), nada que dependa de la maquina: cualquier
 * Windows dice lo mismo que la casa.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA HANDLE W GetCurrentProcess(void);
IMPORTA HANDLE W GetCurrentThread(void);
IMPORTA DWORD W GetCurrentProcessId(void);
IMPORTA DWORD W GetCurrentThreadId(void);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA HANDLE W GetProcessHeap(void);
IMPORTA HANDLE W CreateThread(void *a, U64 pila, DWORD(W *f)(void *), void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD W ResumeThread(HANDLE h);
IMPORTA DWORD W SuspendThread(HANDLE h);
IMPORTA int W TerminateThread(HANDLE h, DWORD c);
IMPORTA DWORD W WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int W GetExitCodeThread(HANDLE h, DWORD *c);
IMPORTA void W Sleep(DWORD ms);
IMPORTA DWORD W SleepEx(DWORD ms, int alertable);
IMPORTA DWORD W QueueUserAPC(void(W *f)(U64), HANDLE h, U64 dato);
IMPORTA void *W ConvertThreadToFiber(void *dato);
IMPORTA int W ConvertFiberToThread(void);
IMPORTA int W IsThreadAFiber(void);
IMPORTA void *W CreateFiber(U64 pila, void(W *f)(void *), void *dato);
IMPORTA void W SwitchToFiber(void *f);
IMPORTA void W DeleteFiber(void *f);
IMPORTA HANDLE W CreateToolhelp32Snapshot(DWORD banderas, DWORD pid);
IMPORTA int W Thread32First(HANDLE h, void *e);
IMPORTA int W Thread32Next(HANDLE h, void *e);
IMPORTA int W Process32FirstW(HANDLE h, void *e);
IMPORTA int W Process32NextW(HANDLE h, void *e);
IMPORTA int W Module32FirstW(HANDLE h, void *e);
IMPORTA int W K32EnumProcessModules(HANDLE p, HANDLE *m, DWORD cb, DWORD *hace_falta);
IMPORTA int W K32GetModuleInformation(HANDLE p, HANDLE m, void *mi, DWORD cb);
IMPORTA int W GetLogicalProcessorInformation(void *b, DWORD *largo);
IMPORTA int W GetLogicalProcessorInformationEx(DWORD rel, void *b, DWORD *largo);
IMPORTA unsigned short W RtlCaptureStackBackTrace(DWORD salta, DWORD n, void **marcos, DWORD *hash);
IMPORTA void *W RtlPcToFileHeader(void *pc, void **base);
IMPORTA int W GetDiskFreeSpaceExW(const WCHAR *d, U64 *mio, U64 *total, U64 *libre);
IMPORTA unsigned W GetDriveTypeW(const WCHAR *d);
IMPORTA int W GetFileTime(HANDLE h, U64 *c, U64 *a, U64 *e);
IMPORTA int W HeapQueryInformation(HANDLE h, int clase, void *b, U64 largo, U64 *devuelto);
IMPORTA int W CancelSynchronousIo(HANDLE h);
/* de api-ms-win-core-processthreads-l1-1-0 (tanda9_hilos.def) */
IMPORTA int W OpenProcessToken(HANDLE p, DWORD acceso, HANDLE *t);
IMPORTA int W OpenThreadToken(HANDLE h, DWORD acceso, int propio, HANDLE *t);

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

static U64 fiber_data(void) {
    U64 v;
    __asm__ volatile("movq %%gs:0x20, %0" : "=r"(v));
    return v;
}

/* -- la fibra: cuenta, y vuelve a la principal */
static void *principal, *otra;
static int vueltas;
static U64 vista_dentro;

static void W fibra(void *dato) {
    for (;;) {
        vueltas += (int)(U64)dato;
        vista_dentro = fiber_data();
        SwitchToFiber(principal);
    }
}

/* -- los hilos */
static volatile int corrio;

static DWORD W hilo_que_cuenta(void *p) {
    corrio = (int)(U64)p;
    return 7;
}

static DWORD W hilo_que_duerme(void *p) {
    (void)p;
    for (;;)
        Sleep(50);
}

static U64 apc_dato;

static void W apc(U64 d) {
    apc_dato = d;
}

__declspec(noinline) static unsigned short marcos(void **m) {
    return RtlCaptureStackBackTrace(0, 8, m, 0);
}

void inicio(void);

void inicio(void) {
    static unsigned char e[1080];
    static unsigned char grande[65536];
    static HANDLE mods[64];
    static void *m[8];
    U64 base, u[3], mi[3], v;
    DWORD n, id, codigo;
    HANDLE h, t, yo = GetModuleHandleW(0);
    void *b;
    int hallado;

    /* -- fibras */
    mira(IsThreadAFiber() == 0, "IsThreadAFiber: un hilo, antes, no es fibra");
    principal = ConvertThreadToFiber((void *)0x1234);
    mira(principal != 0 && IsThreadAFiber() == 1 && fiber_data() == (U64)principal && *(void **)principal == (void *)0x1234, "ConvertThreadToFiber: el TEB la apunta, y su dato primero");
    otra = CreateFiber(0, fibra, (void *)5);
    mira(otra != 0 && *(void **)otra == (void *)5, "CreateFiber: su dato primero");
    SwitchToFiber(otra);
    SwitchToFiber(otra);
    mira(vueltas == 10 && vista_dentro == (U64)otra && fiber_data() == (U64)principal, "SwitchToFiber: ida y vuelta dos veces, cada una con su TEB");
    DeleteFiber(otra);
    mira(ConvertFiberToThread() == 1 && IsThreadAFiber() == 0, "ConvertFiberToThread");
    SetLastError(0);
    mira(ConvertFiberToThread() == 0 && GetLastError() == 1281, "ConvertFiberToThread otra vez: ERROR_ALREADY_THREAD");

    /* -- SuspendThread y TerminateThread */
    h = CreateThread(0, 0, hilo_que_cuenta, (void *)3, 4, &id);
    mira(h && SuspendThread(h) == 1 && ResumeThread(h) == 2 && ResumeThread(h) == 1, "SuspendThread: las cuentas de antes, con CREATE_SUSPENDED");
    mira(WaitForSingleObject(h, 5000) == 0 && corrio == 3 && GetExitCodeThread(h, &codigo) && codigo == 7, "y reanudado del todo, corre");
    CloseHandle(h);
    h = CreateThread(0, 0, hilo_que_duerme, 0, 0, &id);
    Sleep(20);
    mira(h && TerminateThread(h, 42) && WaitForSingleObject(h, 5000) == 0 && GetExitCodeThread(h, &codigo) && codigo == 42, "TerminateThread: acaba con ese codigo");
    CloseHandle(h);

    /* -- QueueUserAPC */
    mira(QueueUserAPC(apc, GetCurrentThread(), 77) != 0 && apc_dato == 0, "QueueUserAPC: se pone, no corre todavia");
    mira(SleepEx(5000, 1) == 0xC0 && apc_dato == 77, "SleepEx alertable: la corre y dice WAIT_IO_COMPLETION");
    mira(SleepEx(0, 1) == 0, "SleepEx alertable sin APC: 0");

    /* -- Toolhelp */
    t = CreateToolhelp32Snapshot(0x4, 0);
    hallado = 0;
    *(DWORD *)e = 28;
    if (t != (HANDLE)-1 && Thread32First(t, e))
        do
            hallado |= *(DWORD *)(e + 8) == GetCurrentThreadId() && *(DWORD *)(e + 12) == GetCurrentProcessId();
        while (Thread32Next(t, e));
    mira(hallado && GetLastError() == 18 && CloseHandle(t), "Thread32First/Next: el hilo propio esta, y luego ERROR_NO_MORE_FILES");
    t = CreateToolhelp32Snapshot(0x2, 0);
    hallado = 0;
    *(DWORD *)e = 568;
    if (t != (HANDLE)-1 && Process32FirstW(t, e))
        do
            hallado |= *(DWORD *)(e + 8) == GetCurrentProcessId();
        while (Process32NextW(t, e));
    mira(hallado && CloseHandle(t), "Process32FirstW/NextW: el proceso propio esta");
    t = CreateToolhelp32Snapshot(0x8, 0);
    *(DWORD *)e = 1080;
    mira(t != (HANDLE)-1 && Module32FirstW(t, e) && *(DWORD *)(e + 8) == GetCurrentProcessId() && *(HANDLE *)(e + 24) == yo && *(HANDLE *)(e + 40) == yo && CloseHandle(t), "Module32FirstW: el .exe primero");

    /* -- psapi */
    mira(K32EnumProcessModules(GetCurrentProcess(), mods, sizeof mods, &n) && n >= 8 && n % 8 == 0 && mods[0] == yo, "K32EnumProcessModules: el .exe primero");
    mira(K32GetModuleInformation(GetCurrentProcess(), yo, mi, 24) && mi[0] == (U64)yo && mi[1] > 0 && mi[2] == (U64)inicio, "K32GetModuleInformation: la base, y la entrada es inicio");

    /* -- el procesador */
    n = 0;
    mira(!GetLogicalProcessorInformation(0, &n) && GetLastError() == 122 && n > 0 && n % 32 == 0, "GetLogicalProcessorInformation: lo que hace falta, en entradas de 32");
    hallado = 0;
    if (n <= sizeof grande && GetLogicalProcessorInformation(grande, &n))
        for (id = 0; id < n; id += 32)
            hallado |= *(DWORD *)(grande + id + 8) == 0;
    mira(hallado, "GetLogicalProcessorInformation: al menos un nucleo");
    n = 0;
    mira(!GetLogicalProcessorInformationEx(0xFFFF, 0, &n) && GetLastError() == 122 && n > 0 && n <= sizeof grande && GetLogicalProcessorInformationEx(0xFFFF, grande, &n), "GetLogicalProcessorInformationEx: lo que hace falta, y cabe");

    /* -- la pila */
    n = marcos(m);
    mira(n >= 1 && RtlPcToFileHeader(m[0], &b) == yo && b == yo, "RtlCaptureStackBackTrace: vuelve a este .exe");
    mira(RtlPcToFileHeader((void *)inicio, &b) == yo && b == yo, "RtlPcToFileHeader: inicio es de este .exe");
    mira(RtlPcToFileHeader((void *)16, &b) == 0 && b == 0, "RtlPcToFileHeader: de nadie, NULL");

    /* -- discos y ficheros */
    mira(GetDriveTypeW(L"C:\\") == 3, "GetDriveTypeW C:, un disco fijo");
    mira(GetDiskFreeSpaceExW(L"C:\\", &u[0], &u[1], &u[2]) && u[1] > 0 && u[0] <= u[1] && u[2] <= u[1], "GetDiskFreeSpaceExW: lo libre cabe en el total");
    SetLastError(0);
    mira(!GetFileTime((HANDLE)0x1234, &u[0], &u[1], &u[2]) && GetLastError() == 6, "GetFileTime de un handle que no es: ERROR_INVALID_HANDLE");

    /* -- y lo demas */
    v = 0;
    mira(HeapQueryInformation(GetProcessHeap(), 0, &v, 4, &base) && v == 2 && base == 4, "HeapQueryInformation: el heap del proceso es LFH");
    mira(!CancelSynchronousIo(GetCurrentThread()) && GetLastError() == 1168, "CancelSynchronousIo: nada que cancelar");
    t = 0;
    mira(OpenProcessToken(GetCurrentProcess(), 8, &t) && t != 0, "OpenProcessToken: TOKEN_QUERY del propio");
    mira(!OpenThreadToken(GetCurrentThread(), 8, 1, &t) && GetLastError() == 1008, "OpenThreadToken: el hilo no se hace pasar por nadie");
    di("tanda9.exe: lo que quedaba de kernel32 es lo de Windows\r\n");
    ExitProcess(fallos);
}
