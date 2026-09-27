/* ucrt.c -- el .exe de P4f5 de PROTON-X: el CRT de MSVC (UCRT y
 * vcruntime140), importado de las MISMAS DLL que un .exe de Visual C++ con
 * el CRT dinamico (api-ms-win-crt-*-l1-1-0.dll y vcruntime140.dll).
 *
 *    el arranque  lo que hace mainCRTStartup: _set_app_type,
 *                 _configure_narrow_argv, __p___argc / __p___argv,
 *                 _initialize_narrow_environment y su entorno, _initterm y
 *                 _initterm_e (que para en el primero que no da 0)
 *    la salida    una tabla de onexit (_register / _execute, al reves), y
 *                 _crt_atexit: la ULTIMA linea la escribe una funcion
 *                 registrada asi, cuando exit() la llama
 *    el monton    malloc (a 16), calloc (a ceros), realloc (conserva), free
 *    memoria      memcpy, memmove con solape, memset, memcmp, memchr
 *    cadenas      strlen, wcslen, strcmp, strncmp
 *
 * Sale con el numero de fallos, por exit(). En Windows dice lo mismo (pide
 * vcruntime140.dll: el redistribuible de Visual C++, que traen los juegos). */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;
typedef void(__cdecl *PVFV)(void);
typedef int(__cdecl *PIFV)(void);
typedef struct { PVFV *primero, *ultimo, *fin; } ONEXIT;

#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
/* api-ms-win-crt-runtime-l1-1-0.dll */
IMPORTA void __cdecl _set_app_type(int t);
IMPORTA int __cdecl _configure_narrow_argv(int modo);
IMPORTA int *__cdecl __p___argc(void);
IMPORTA char ***__cdecl __p___argv(void);
IMPORTA int __cdecl _initialize_narrow_environment(void);
IMPORTA char **__cdecl _get_initial_narrow_environment(void);
IMPORTA void __cdecl _initterm(PVFV *a, PVFV *b);
IMPORTA int __cdecl _initterm_e(PIFV *a, PIFV *b);
IMPORTA int __cdecl _crt_atexit(PVFV f);
IMPORTA int __cdecl _initialize_onexit_table(ONEXIT *t);
IMPORTA int __cdecl _register_onexit_function(ONEXIT *t, PVFV f);
IMPORTA int __cdecl _execute_onexit_table(ONEXIT *t);
IMPORTA void __cdecl exit(int c);
/* api-ms-win-crt-heap-l1-1-0.dll */
IMPORTA void *__cdecl malloc(U64 n);
IMPORTA void *__cdecl calloc(U64 n, U64 m);
IMPORTA void *__cdecl realloc(void *p, U64 n);
IMPORTA void __cdecl free(void *p);
/* api-ms-win-crt-string-l1-1-0.dll */
IMPORTA U64 __cdecl strlen(const char *s);
IMPORTA U64 __cdecl wcslen(const WCHAR *s);
IMPORTA int __cdecl strcmp(const char *a, const char *b);
IMPORTA int __cdecl strncmp(const char *a, const char *b, U64 n);
/* vcruntime140.dll */
IMPORTA void *__cdecl memcpy(void *d, const void *s, U64 n);
IMPORTA void *__cdecl memmove(void *d, const void *s, U64 n);
IMPORTA void *__cdecl memset(void *d, int c, U64 n);
IMPORTA int __cdecl memcmp(const void *a, const void *b, U64 n);
IMPORTA void *__cdecl memchr(const void *p, int c, U64 n);

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

/* El orden en que se llama a cada cosa. */
static char orden[16];
static int n_orden;
static void __cdecl uno(void) { orden[n_orden++] = '1'; }
static void __cdecl dos(void) { orden[n_orden++] = '2'; }
static void __cdecl tres(void) { orden[n_orden++] = '3'; }
static int __cdecl da0(void) { orden[n_orden++] = 'a'; return 0; }
static int __cdecl da7(void) { orden[n_orden++] = 'b'; return 7; }
static int __cdecl nunca(void) { orden[n_orden++] = 'X'; return 0; }

static int empieza(const char *s, const char *p) {
    for (; *p; s++, p++) {
        char a = (*s >= 'A' && *s <= 'Z') ? (char)(*s + 32) : *s;
        if (a != *p) return 0;
    }
    return 1;
}

static void el_arranque(void) {
    int argc, i, path = 0;
    char **argv, **env;
    PVFV tabla[] = {uno, 0, dos};
    PIFV tabla_e[] = {da0, da7, nunca};

    _set_app_type(1);
    mira(_configure_narrow_argv(1) == 0, "_configure_narrow_argv", 0);
    argc = *__p___argc();
    argv = *__p___argv();
    mira(argc >= 1 && argv && argv[argc] == 0 && strlen(argv[0]) > 0, "__p___argc y __p___argv: argv[argc] es NULL", (U64)argc);
    for (i = 0; argv[0][i]; i++) {
        if (empieza(argv[0] + i, "ucrt")) break;
    }
    mira(argv[0][i] != 0, "argv[0] nombra el .exe", (U64)i);
    mira(_initialize_narrow_environment() == 0 && (env = _get_initial_narrow_environment()) != 0, "_initialize_narrow_environment", 0);
    for (i = 0; env[i]; i++)
        if (empieza(env[i], "path=")) path = 1;
    mira(path, "y en su entorno esta PATH", (U64)i);

    n_orden = 0;
    _initterm(tabla, tabla + 3);
    mira(n_orden == 2 && orden[0] == '1' && orden[1] == '2', "_initterm: cada uno en orden, los NULL no", (U64)n_orden);
    n_orden = 0;
    mira(_initterm_e(tabla_e, tabla_e + 3) == 7 && n_orden == 2 && orden[1] == 'b', "_initterm_e: para en el primero que no da 0", (U64)n_orden);
}

static void la_tabla_de_salida(void) {
    /* A CERO, como las tablas estaticas del CRT: el UCRT de Windows solo
     * inicia una tabla cuyo primero es igual a su fin; una con basura la da
     * por iniciada, y registrar en ella tumba el proceso (el 27-09, en
     * Windows, este .exe se callaba aqui por eso). */
    ONEXIT t = {0, 0, 0};
    n_orden = 0;
    mira(_initialize_onexit_table(&t) == 0 && _register_onexit_function(&t, uno) == 0 && _register_onexit_function(&t, dos) == 0 && _register_onexit_function(&t, tres) == 0,
         "una tabla de onexit con tres", 3);
    mira(_execute_onexit_table(&t) == 0 && n_orden == 3 && orden[0] == '3' && orden[2] == '1', "_execute_onexit_table: al reves", (U64)n_orden);
}

static void el_monton(void) {
    char *p = malloc(100), *q;
    int *z = calloc(64, 4), i, ceros = 1;
    mira(p && ((U64)p & 15) == 0, "malloc(100): alineado a 16", (U64)p & 15);
    memset(p, 'x', 100);
    q = realloc(p, 5000);
    mira(q && q[0] == 'x' && q[99] == 'x', "realloc a 5000 conserva lo de dentro", 0);
    for (i = 0; i < 64; i++)
        if (z[i]) ceros = 0;
    mira(z && ceros, "calloc: a ceros", 0);
    free(q);
    free(z);
    free(0);
}

static void memoria_y_cadenas(void) {
    char b[16] = "0123456789";
    char c[16];
    static const WCHAR W[] = {'h', 'o', 'l', 'a', 0};
    memcpy(c, b, 11);
    mira(memcmp(c, b, 11) == 0, "memcpy y memcmp", 0);
    memmove(b + 2, b, 5);
    mira(memcmp(b, "0101234789", 10) == 0, "memmove con solape", 0);
    mira(memcmp("abc", "abd", 3) < 0 && memcmp("b", "a", 1) > 0, "memcmp da el signo", 0);
    mira(memchr(b, '7', 10) == b + 7 && memchr(b, 'z', 10) == 0, "memchr", 0);
    mira(strlen("hola mundo") == 10 && wcslen(W) == 4, "strlen y wcslen", 10);
    mira(strcmp("abc", "abc") == 0 && strcmp("abc", "abd") < 0 && strncmp("abcX", "abcY", 3) == 0, "strcmp y strncmp", 0);
}

/* La ultima: la llama exit(), registrada con _crt_atexit. */
static void __cdecl al_salir(void) {
    di(fallos ? "ucrt.exe: ALGO NO es como en Windows (dicho desde _crt_atexit)\r\n" : "ucrt.exe: el CRT es el de Windows (dicho desde _crt_atexit)\r\n");
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    di("ucrt.exe: el CRT de MSVC, de sus propias DLL\r\n");
    el_arranque();
    la_tabla_de_salida();
    el_monton();
    memoria_y_cadenas();
    mira(_crt_atexit(al_salir) == 0, "_crt_atexit", 0);
    exit((int)fallos);
}
