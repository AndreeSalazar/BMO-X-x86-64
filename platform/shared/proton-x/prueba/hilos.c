/* hilos.c -- el .exe de P4 de PROTON-X: hilos, TLS y sincronizacion de
 * Windows, como los usa un juego.
 *
 *    CreateThread / WaitForMultipleObjects / GetExitCodeThread
 *    CREATE_SUSPENDED y ResumeThread
 *    un TEB por hilo (gs:[0x30] distinto) y su pila entre StackLimit y StackBase
 *    TLS dinamico: TlsAlloc, TlsSetValue, TlsGetValue (y LastError a 0)
 *    TLS estatico: __declspec(thread), una copia de la plantilla por hilo
 *    los callbacks del TLS: PROCESS_ATTACH antes de la entrada, THREAD_ATTACH
 *        en cada hilo nuevo, en ESE hilo
 *    seccion critica con espera de verdad (se cede el turno dentro)
 *    semaforos: productor y consumidor con un anillo de 4
 *    SRW y variable de condicion: una cola
 *    un evento que nadie enciende: WaitForSingleObject(10 ms) da WAIT_TIMEOUT
 *    xmm6..xmm15 y el MXCSR sobreviven a que OTRO hilo los ensucie mientras
 *        este espera (Windows x64 los da por conservados)
 *
 * Sin CRT: `_tls_used` (el directorio de TLS que el enlazador busca por su
 * nombre) va definido aqui, como lo define `tlssup.c` del CRT de Microsoft.
 *
 * Todo lo que dice es INDEPENDIENTE del orden en que corran los hilos: sale
 * igual en Windows (hilos de verdad, en varios nucleos) y en BMO-X (hilos
 * cooperativos dentro de PROTON-X). Sale con el numero de fallos. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long LONG;
typedef void(__stdcall *TLS_CB)(void *, DWORD, void *);
typedef DWORD(__stdcall *PROC)(void *);
typedef struct { void *a, *b, *c, *d, *e; } CRITICAL_SECTION; /* 40 bytes en x64 */
typedef struct { void *p; } SRWLOCK;
typedef struct { void *p; } CONDITION_VARIABLE;

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int codigo);
IMPORTA DWORD WINAPI GetLastError(void);
IMPORTA void WINAPI SetLastError(DWORD e);
IMPORTA DWORD WINAPI GetCurrentThreadId(void);
IMPORTA HANDLE WINAPI CreateThread(void *attr, U64 pila, PROC f, void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD WINAPI ResumeThread(HANDLE h);
IMPORTA int WINAPI GetExitCodeThread(HANDLE h, DWORD *codigo);
IMPORTA DWORD WINAPI WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA DWORD WINAPI WaitForMultipleObjects(DWORD n, const HANDLE *h, int todos, DWORD ms);
IMPORTA int WINAPI CloseHandle(HANDLE h);
IMPORTA void WINAPI Sleep(DWORD ms);
IMPORTA int WINAPI SwitchToThread(void);
IMPORTA HANDLE WINAPI CreateEventW(void *attr, int manual, int inicial, const void *nombre);
IMPORTA int WINAPI SetEvent(HANDLE h);
IMPORTA HANDLE WINAPI CreateSemaphoreW(void *attr, LONG inicial, LONG max, const void *nombre);
IMPORTA int WINAPI ReleaseSemaphore(HANDLE h, LONG n, LONG *antes);
IMPORTA DWORD WINAPI TlsAlloc(void);
IMPORTA int WINAPI TlsFree(DWORD i);
IMPORTA void *WINAPI TlsGetValue(DWORD i);
IMPORTA int WINAPI TlsSetValue(DWORD i, void *v);
IMPORTA void WINAPI InitializeCriticalSection(CRITICAL_SECTION *cs);
IMPORTA void WINAPI EnterCriticalSection(CRITICAL_SECTION *cs);
IMPORTA void WINAPI LeaveCriticalSection(CRITICAL_SECTION *cs);
IMPORTA void WINAPI DeleteCriticalSection(CRITICAL_SECTION *cs);
IMPORTA void WINAPI InitializeSRWLock(SRWLOCK *l);
IMPORTA void WINAPI AcquireSRWLockExclusive(SRWLOCK *l);
IMPORTA void WINAPI ReleaseSRWLockExclusive(SRWLOCK *l);
IMPORTA void WINAPI InitializeConditionVariable(CONDITION_VARIABLE *c);
IMPORTA int WINAPI SleepConditionVariableSRW(CONDITION_VARIABLE *c, SRWLOCK *l, DWORD ms, DWORD banderas);
IMPORTA void WINAPI WakeConditionVariable(CONDITION_VARIABLE *c);

#define INFINITE 0xFFFFFFFF
#define WAIT_OBJECT_0 0
#define WAIT_TIMEOUT 0x102
#define CREATE_SUSPENDED 4
#define STILL_ACTIVE 259
#define DLL_PROCESS_ATTACH 1
#define DLL_THREAD_ATTACH 2
#define HILOS 4

/* -- El directorio de TLS, sin CRT (lo que hace tlssup.c) -------------------- */
typedef struct {
    U64 StartAddressOfRawData, EndAddressOfRawData, AddressOfIndex, AddressOfCallBacks;
    DWORD SizeOfZeroFill, Characteristics;
} IMAGE_TLS_DIRECTORY64;

#pragma section(".tls", read, write)
#pragma section(".tls$ZZZ", read, write)
#pragma section(".CRT$XLA", long, read)
#pragma section(".CRT$XLB", long, read)
#pragma section(".CRT$XLZ", long, read)
__declspec(allocate(".tls")) char _tls_start = 0;
__declspec(allocate(".tls$ZZZ")) char _tls_end = 0;
__declspec(allocate(".CRT$XLA")) TLS_CB __xl_a = 0;
__declspec(allocate(".CRT$XLZ")) TLS_CB __xl_z = 0;
DWORD _tls_index = 0;
const IMAGE_TLS_DIRECTORY64 _tls_used = {(U64)&_tls_start, (U64)&_tls_end, (U64)&_tls_index, (U64)(&__xl_a + 1), 0, 0};

/* Las variables POR HILO: cada hilo nace con una copia de estos valores. */
__declspec(thread) int t_marca = 7;
__declspec(thread) int t_vio_attach = 0;

static volatile int proceso_attach, attach_antes_de_la_entrada, en_la_entrada;

static void __stdcall callback(void *base, DWORD motivo, void *nada) {
    (void)base;
    (void)nada;
    if (motivo == DLL_PROCESS_ATTACH) {
        proceso_attach++;
        attach_antes_de_la_entrada = !en_la_entrada;
    }
    /* En ESE hilo, con SU copia: si el TLS no fuera por hilo, se veria en el
     * principal. */
    if (motivo == DLL_THREAD_ATTACH) t_vio_attach = 1;
}
__declspec(allocate(".CRT$XLB")) TLS_CB mi_callback = callback;

/* -- Decir --------------------------------------------------------------------- */
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

static U64 gs64(unsigned long o) {
    U64 v;
    __asm__ volatile("movq %%gs:(%1), %0" : "=r"(v) : "r"((U64)o));
    return v;
}

/* -- Lo que hace cada hilo ------------------------------------------------------ */
static DWORD tls_dinamico;
static CRITICAL_SECTION cs;
static volatile long contador;
static struct {
    U64 teb, tid;
    int propia_pila, marca_inicial, marca_final, vio_attach, tls_ok;
} visto[HILOS];

static DWORD WINAPI trabajador(void *arg) {
    int i = (int)(U64)arg, k;
    U64 teb = gs64(0x30), local = (U64)&k;
    visto[i].teb = teb;
    visto[i].tid = GetCurrentThreadId();
    visto[i].propia_pila = *(U64 *)(teb + 0x10) < local && local < *(U64 *)(teb + 0x08);
    visto[i].marca_inicial = t_marca;
    visto[i].vio_attach = t_vio_attach;
    t_marca = 100 + i;
    TlsSetValue(tls_dinamico, (void *)(U64)(i + 1));
    /* La seccion critica, con el turno cedido DENTRO: los otros tienen que
     * esperar de verdad a que salga. */
    for (k = 0; k < 10000; k++) {
        EnterCriticalSection(&cs);
        contador++;
        if (k % 1000 == 0) SwitchToThread();
        LeaveCriticalSection(&cs);
    }
    Sleep(1);
    SetLastError(77);
    visto[i].tls_ok = TlsGetValue(tls_dinamico) == (void *)(U64)(i + 1) && GetLastError() == 0;
    visto[i].marca_final = t_marca;
    return 1000 + i;
}

/* Productor y consumidor con semaforos: un anillo de 4. */
static HANDLE vacios, llenos;
static int anillo[4];
static U64 suma_semaforos;

static DWORD WINAPI productor(void *arg) {
    int n;
    (void)arg;
    for (n = 1; n <= 100; n++) {
        WaitForSingleObject(vacios, INFINITE);
        anillo[n % 4] = n;
        ReleaseSemaphore(llenos, 1, 0);
    }
    return 0;
}

static DWORD WINAPI consumidor(void *arg) {
    int n;
    (void)arg;
    for (n = 1; n <= 100; n++) {
        WaitForSingleObject(llenos, INFINITE);
        suma_semaforos += (U64)anillo[n % 4];
        ReleaseSemaphore(vacios, 1, 0);
    }
    return 0;
}

/* Una cola con SRW y variable de condicion. */
static SRWLOCK cerrojo;
static CONDITION_VARIABLE hay;
static int cola[16], cabeza, cuantos_en_cola;
static U64 suma_cola;

static DWORD WINAPI lector_de_cola(void *arg) {
    int n;
    (void)arg;
    for (n = 0; n < 10; n++) {
        AcquireSRWLockExclusive(&cerrojo);
        while (cuantos_en_cola == 0) SleepConditionVariableSRW(&hay, &cerrojo, INFINITE, 0);
        suma_cola += (U64)cola[cabeza++ % 16];
        cuantos_en_cola--;
        ReleaseSRWLockExclusive(&cerrojo);
    }
    return 0;
}

static volatile int dormido_corrio;
static DWORD WINAPI dormido(void *arg) {
    (void)arg;
    dormido_corrio = 1;
    return 5;
}

/* xmm6..xmm15 (10 x 16 bytes) y el MXCSR detras (+160). */
static __declspec(align(16)) U64 xmm_antes[21], xmm_despues[21];
static __declspec(align(16)) U64 basura[21];

static DWORD WINAPI ensucia(void *arg) {
    (void)arg;
    /* Otros valores en xmm6..15 y el redondeo hacia cero en el MXCSR. */
    __asm__ volatile("movdqu (%0), %%xmm6\n movdqu 16(%0), %%xmm7\n movdqu 32(%0), %%xmm8\n movdqu 48(%0), %%xmm9\n"
                     "movdqu 64(%0), %%xmm10\n movdqu 80(%0), %%xmm11\n movdqu 96(%0), %%xmm12\n movdqu 112(%0), %%xmm13\n"
                     "movdqu 128(%0), %%xmm14\n movdqu 144(%0), %%xmm15\n ldmxcsr 160(%0)"
                     :
                     : "r"(basura)
                     : "xmm6", "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15", "memory");
    return 0;
}

/* Carga xmm6..15, espera a `h` (el otro hilo corre y los ensucia) y los
 * guarda: todo en un bloque, para que el compilador no los toque en medio. */
static void espera_con_xmm(HANDLE h) {
    __asm__ volatile("stmxcsr 160(%[a])\n"
                     "movdqu (%[a]), %%xmm6\n movdqu 16(%[a]), %%xmm7\n movdqu 32(%[a]), %%xmm8\n movdqu 48(%[a]), %%xmm9\n"
                     "movdqu 64(%[a]), %%xmm10\n movdqu 80(%[a]), %%xmm11\n movdqu 96(%[a]), %%xmm12\n movdqu 112(%[a]), %%xmm13\n"
                     "movdqu 128(%[a]), %%xmm14\n movdqu 144(%[a]), %%xmm15\n"
                     "mov %%rsp, %%rbx\n and $-16, %%rsp\n sub $32, %%rsp\n"
                     "mov %[h], %%rcx\n mov $0xFFFFFFFF, %%edx\n call *%[w]\n"
                     "mov %%rbx, %%rsp\n"
                     "movdqu %%xmm6, (%[d])\n movdqu %%xmm7, 16(%[d])\n movdqu %%xmm8, 32(%[d])\n movdqu %%xmm9, 48(%[d])\n"
                     "movdqu %%xmm10, 64(%[d])\n movdqu %%xmm11, 80(%[d])\n movdqu %%xmm12, 96(%[d])\n movdqu %%xmm13, 112(%[d])\n"
                     "movdqu %%xmm14, 128(%[d])\n movdqu %%xmm15, 144(%[d])\n stmxcsr 160(%[d])"
                     :
                     : [a] "r"(xmm_antes), [d] "r"(xmm_despues), [h] "r"(h), [w] "r"(WaitForSingleObject)
                     : "rax", "rbx", "rcx", "rdx", "r8", "r9", "r10", "r11", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6",
                       "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15", "memory", "cc");
}

void inicio(void) {
    HANDLE h[HILOS], p[3], s, nadie;
    DWORD codigo, antes, id;
    int i, todo_bien, distintos;
    en_la_entrada = 1;
    salida = GetStdHandle((DWORD)-11);
    di("hilos.exe: hilos, TLS y sincronizacion de Windows\r\n");

    mira(proceso_attach == 1 && attach_antes_de_la_entrada, "el callback del TLS vio PROCESS_ATTACH antes de la entrada", proceso_attach);
    mira(t_marca == 7 && t_vio_attach == 0, "__declspec(thread) del principal: la plantilla, sin THREAD_ATTACH", (U64)t_marca);

    tls_dinamico = TlsAlloc();
    mira(tls_dinamico != 0xFFFFFFFF, "TlsAlloc da un indice", tls_dinamico);
    InitializeCriticalSection(&cs);
    for (i = 0; i < HILOS; i++) h[i] = CreateThread(0, 0, trabajador, (void *)(U64)i, 0, &id);
    mira(WaitForMultipleObjects(HILOS, h, 1, INFINITE) == WAIT_OBJECT_0, "WaitForMultipleObjects espera a los cuatro", HILOS);

    todo_bien = 1;
    for (i = 0; i < HILOS; i++) todo_bien &= GetExitCodeThread(h[i], &codigo) && codigo == (DWORD)(1000 + i);
    mira(todo_bien, "GetExitCodeThread da lo que devolvio cada uno (1000 + i)", 1000);
    mira(contador == HILOS * 10000, "la seccion critica: 4 x 10000 sumas, ninguna perdida", (U64)contador);

    todo_bien = 1;
    distintos = 1;
    for (i = 0; i < HILOS; i++) {
        todo_bien &= visto[i].teb != gs64(0x30) && *(U64 *)(visto[i].teb + 0x30) == visto[i].teb && visto[i].propia_pila;
        distintos &= visto[i].tid != GetCurrentThreadId() && (i == 0 || visto[i].tid != visto[i - 1].tid);
    }
    mira(todo_bien, "cada hilo tiene su TEB en gs:[0x30] y su pila entre StackLimit y StackBase", HILOS);
    mira(distintos, "GetCurrentThreadId es distinto en cada hilo", HILOS);

    todo_bien = 1;
    for (i = 0; i < HILOS; i++) todo_bien &= visto[i].marca_inicial == 7 && visto[i].marca_final == 100 + i && visto[i].vio_attach == 1;
    mira(todo_bien, "__declspec(thread): cada hilo nace con 7, se queda con lo suyo y vio THREAD_ATTACH", 7);
    mira(t_marca == 7, "y el principal sigue con su 7", (U64)t_marca);

    todo_bien = 1;
    for (i = 0; i < HILOS; i++) todo_bien &= visto[i].tls_ok;
    mira(todo_bien, "TlsGetValue: el valor de cada hilo, y LastError a 0", HILOS);
    mira(TlsGetValue(tls_dinamico) == 0 && TlsFree(tls_dinamico), "el principal no puso nada: 0; y TlsFree", 0);
    for (i = 0; i < HILOS; i++) CloseHandle(h[i]);
    DeleteCriticalSection(&cs);

    vacios = CreateSemaphoreW(0, 4, 4, 0);
    llenos = CreateSemaphoreW(0, 0, 4, 0);
    p[0] = CreateThread(0, 0, consumidor, 0, 0, &id);
    p[1] = CreateThread(0, 0, productor, 0, 0, &id);
    WaitForMultipleObjects(2, p, 1, INFINITE);
    mira(suma_semaforos == 5050, "semaforos: 1..100 por un anillo de 4 suman 5050", suma_semaforos);

    InitializeSRWLock(&cerrojo);
    InitializeConditionVariable(&hay);
    p[2] = CreateThread(0, 0, lector_de_cola, 0, 0, &id);
    for (i = 1; i <= 10; i++) {
        AcquireSRWLockExclusive(&cerrojo);
        cola[(cabeza + cuantos_en_cola++) % 16] = i;
        ReleaseSRWLockExclusive(&cerrojo);
        WakeConditionVariable(&hay);
        if (i % 3 == 0) Sleep(2);
    }
    WaitForSingleObject(p[2], INFINITE);
    mira(suma_cola == 55, "SRW y variable de condicion: una cola de 1..10 suma 55", suma_cola);

    s = CreateThread(0, 0, dormido, 0, CREATE_SUSPENDED, &id);
    Sleep(20);
    GetExitCodeThread(s, &codigo);
    mira(!dormido_corrio && codigo == STILL_ACTIVE, "CREATE_SUSPENDED: no corre hasta ResumeThread (STILL_ACTIVE)", codigo);
    antes = ResumeThread(s);
    WaitForSingleObject(s, INFINITE);
    GetExitCodeThread(s, &codigo);
    mira(antes == 1 && dormido_corrio && codigo == 5, "ResumeThread da 1, y el hilo corre y acaba", codigo);

    nadie = CreateEventW(0, 1, 0, 0);
    mira(WaitForSingleObject(nadie, 10) == WAIT_TIMEOUT, "un evento que nadie enciende: WAIT_TIMEOUT a los 10 ms", WAIT_TIMEOUT);
    SetEvent(nadie);
    mira(WaitForSingleObject(nadie, 0) == WAIT_OBJECT_0 && WaitForSingleObject(nadie, 0) == WAIT_OBJECT_0, "y encendido, de reinicio manual, sigue encendido", 0);

    for (i = 0; i < 20; i++) {
        xmm_antes[i] = 0x0123456789ABCDEFull * (U64)(i + 1);
        basura[i] = ~xmm_antes[i];
    }
    basura[20] = 0x7F80; /* MXCSR: redondeo hacia cero, excepciones tapadas */
    s = CreateThread(0, 0, ensucia, 0, 0, &id);
    espera_con_xmm(s);
    todo_bien = 1;
    for (i = 0; i < 20; i++) todo_bien &= xmm_despues[i] == xmm_antes[i];
    mira(todo_bien && (DWORD)xmm_despues[20] == (DWORD)xmm_antes[20], "xmm6..xmm15 y el MXCSR sobreviven a que otro hilo los ensucie", (DWORD)xmm_despues[20]);

    di(fallos ? "hilos.exe: ALGO NO es como en Windows\r\n" : "hilos.exe: los hilos son los de Windows\r\n");
    ExitProcess(fallos);
}
