/* tanda2.c -- el .exe de la TANDA 2 de Cyberpunk (29-09): lo que hay debajo
 * de std::mutex, std::condition_variable, std::thread, std::call_once y
 * <chrono> en la biblioteca de C++ de MSVC, importado de msvcp140.dll.
 *
 *    _Mtx_*    un mutex recursivo: lock dos veces, current_owns, unlock;
 *              uno normal: el mismo hilo vuelve a entrar (msvcp140 lo deja)
 *    _Thrd_*   dos hilos que suman con el mutex, y join con su resultado
 *    _Cnd_*    productor y consumidor; timedwait que vence
 *    y mas     _Execute_once una sola vez, los relojes, _Strcoll,
 *              _Strxfrm, _Mbrtowc, _Winerror_map, _Syserror_map
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;
typedef void *HANDLE;
typedef unsigned long DWORD;
#define IMPORTA __declspec(dllimport)
IMPORTA void __stdcall ExitProcess(unsigned c);
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);

typedef struct { U64 m[10]; } MTX; /* 80 bytes, como _Mtx_internal_imp_t */
typedef struct { U64 c[9]; } CND;  /* 72 bytes */
typedef struct { HANDLE h; unsigned id; } THRD;
typedef struct { I64 s; long ns; } TS;
IMPORTA void __cdecl _Mtx_init_in_situ(MTX *m, int tipo);
IMPORTA void __cdecl _Mtx_destroy_in_situ(MTX *m);
IMPORTA int __cdecl _Mtx_lock(MTX *m);
IMPORTA int __cdecl _Mtx_trylock(MTX *m);
IMPORTA int __cdecl _Mtx_unlock(MTX *m);
IMPORTA int __cdecl _Mtx_current_owns(MTX *m);
IMPORTA void __cdecl _Cnd_init_in_situ(CND *c);
IMPORTA void __cdecl _Cnd_destroy_in_situ(CND *c);
IMPORTA int __cdecl _Cnd_wait(CND *c, MTX *m);
IMPORTA int __cdecl _Cnd_timedwait(CND *c, MTX *m, const TS *t);
IMPORTA int __cdecl _Cnd_signal(CND *c);
IMPORTA int __cdecl _Cnd_broadcast(CND *c);
IMPORTA int __cdecl _Thrd_start(THRD *t, int (*f)(void *), void *a);
IMPORTA int __cdecl _Thrd_join(THRD t, int *r);
IMPORTA unsigned __cdecl _Thrd_id(void);
IMPORTA void __cdecl _Thrd_yield(void);
IMPORTA I64 __cdecl _Xtime_get_ticks(void);
IMPORTA I64 __cdecl _Query_perf_counter(void);
IMPORTA I64 __cdecl _Query_perf_frequency(void);
IMPORTA int __cdecl _Strcoll(const char *a1, const char *b1, const char *a2, const char *b2, const void *c);
IMPORTA U64 __cdecl _Strxfrm(char *a1, char *b1, const char *a2, const char *b2, const void *c);
IMPORTA int __cdecl _Mbrtowc(WCHAR *w, const char *s, U64 n, void *st, const void *c);
typedef int (__cdecl *UNAVEZ)(void *, void *, void **);
/* Los de nombre de C++: por su nombre decorado. */
IMPORTA int __cdecl execute_once(void **flag, UNAVEZ f, void *pv) __asm__("?_Execute_once@std@@YAHAEAUonce_flag@1@P6AHPEAX1PEAPEAX@Z1@Z");
IMPORTA int __cdecl winerror_map(int e) __asm__("?_Winerror_map@std@@YAHH@Z");
IMPORTA const char *__cdecl syserror_map(int e) __asm__("?_Syserror_map@std@@YAPEBDH@Z");

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

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static const char A[] = "abc", B[] = "abd", H[] = "hola";
/* El _Cvtvec de msvcp140 y un mbstate_t. */
static struct { unsigned pagina, maximo; int es_c; unsigned char lider[32]; } cvt;
static U64 estado;
static MTX mtx, rec;
static CND cnd;
static volatile int suma, listo, tomado, veces;

static int sumador(void *a) {
    int i;
    for (i = 0; i < 1000; i++) {
        _Mtx_lock(&mtx);
        suma += (int)(U64)a;
        _Mtx_unlock(&mtx);
        if (i % 100 == 0)
            _Thrd_yield();
    }
    return (int)(U64)a * 10;
}

static int consumidor(void *a) {
    (void)a;
    _Mtx_lock(&mtx);
    while (!listo)
        _Cnd_wait(&cnd, &mtx);
    tomado = listo;
    _Mtx_unlock(&mtx);
    return 0;
}

/* Como InitOnceExecuteOnce: (la bandera, el parametro, el contexto). */
static int __cdecl una_vez(void *bandera, void *pv, void **ctx) {
    (void)bandera, (void)ctx;
    veces += (int)(U64)pv;
    return 1;
}

void inicio(void) {
    THRD a, b, c;
    int r1 = 0, r2 = 0;
    char buf[8];
    WCHAR w = 0;
    void *flag = 0;
    TS pasado;

    _Mtx_init_in_situ(&mtx, 1);
    _Mtx_init_in_situ(&rec, 1 | 0x100);
    _Cnd_init_in_situ(&cnd);

    mira(_Mtx_lock(&rec) == 0 && _Mtx_lock(&rec) == 0 && _Mtx_current_owns(&rec) && _Mtx_unlock(&rec) == 0 && _Mtx_current_owns(&rec) && _Mtx_unlock(&rec) == 0 && !_Mtx_current_owns(&rec), "_Mtx recursivo: dos veces, y suelta a la segunda");
    mira(_Mtx_lock(&mtx) == 0 && _Mtx_trylock(&mtx) == 0 && _Mtx_unlock(&mtx) == 0 && _Mtx_current_owns(&mtx) && _Mtx_unlock(&mtx) == 0 && !_Mtx_current_owns(&mtx), "_Mtx normal: el mismo hilo vuelve a entrar (como msvcp140) y suelta a la segunda");

    mira(_Thrd_start(&a, sumador, (void *)1) == 0 && _Thrd_start(&b, sumador, (void *)2) == 0 && a.id != b.id && a.id != _Thrd_id(), "_Thrd_start: dos hilos, cada uno con su id");
    mira(_Thrd_join(a, &r1) == 0 && _Thrd_join(b, &r2) == 0 && r1 == 10 && r2 == 20 && suma == 3000, "_Thrd_join con su resultado, y el mutex no perdio ni una suma");

    _Thrd_start(&c, consumidor, 0);
    _Thrd_yield();
    _Mtx_lock(&mtx);
    listo = 7;
    _Cnd_signal(&cnd);
    _Mtx_unlock(&mtx);
    mira(_Thrd_join(c, 0) == 0 && tomado == 7, "_Cnd_wait y _Cnd_signal: el consumidor ve lo del productor");

    pasado.s = _Xtime_get_ticks() / 10000000 - 1;
    pasado.ns = 0;
    _Mtx_lock(&mtx);
    mira(_Cnd_timedwait(&cnd, &mtx, &pasado) == 2 && _Mtx_current_owns(&mtx) && _Cnd_broadcast(&cnd) == 0, "_Cnd_timedwait con la hora pasada: timedout, y el mutex sigue cogido");
    _Mtx_unlock(&mtx);

    mira(execute_once(&flag, una_vez, (void *)5) && execute_once(&flag, una_vez, (void *)5) && veces == 5, "_Execute_once: una sola vez");
    /* La frecuencia es la de la maquina (la casa cuenta ns). */
    mira(_Xtime_get_ticks() > 17000000000000000ll && _Query_perf_frequency() > 0 && _Query_perf_counter() > 0, "_Xtime_get_ticks y los contadores");
    mira(_Strcoll(A, A + 3, B, B + 3, 0) < 0 && _Strcoll(A, A + 2, B, B + 2, 0) == 0, "_Strcoll");
    mira(_Strxfrm(buf, buf + 8, H, H + 4, 0) == 4 && buf[0] == 'h' && buf[3] == 'a', "_Strxfrm");
    /* msvcp140 lee la tabla de conversion: la del locale "C". */
    cvt.pagina = 0;
    cvt.maximo = 1;
    cvt.es_c = 1;
    mira(_Mbrtowc(&w, "Z", 1, &estado, &cvt) == 1 && w == 'Z' && _Mbrtowc(&w, "Z", 0, &estado, &cvt) == -2, "_Mbrtowc");
    mira(winerror_map(2) == 2 && winerror_map(5) == 13 && winerror_map(12345) == 0 && igual(syserror_map(2), "no such file or directory"), "_Winerror_map y _Syserror_map");

    _Cnd_destroy_in_situ(&cnd);
    _Mtx_destroy_in_situ(&mtx);
    _Mtx_destroy_in_situ(&rec);
    di("tanda2.exe: los hilos de la biblioteca de C++ son los de Windows\r\n");
    ExitProcess(fallos);
}
