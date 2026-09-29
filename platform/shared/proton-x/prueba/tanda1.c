/* tanda1.c -- el .exe de la TANDA 1 de Cyberpunk (29-09): el C runtime que
 * piden Cyberpunk2077.exe y sus DLL, importado de las MISMAS DLL (los
 * api-ms-win-crt-*, msvcrt.dll y vcruntime140.dll).
 *
 *    cadenas     strchr strrchr strstr strspn strcspn strpbrk strncpy
 *                strncat _strdup _stricmp _strnicmp _memicmp las _s
 *    anchas      wcschr wcsrchr wcsstr wcscmp _wcsicmp wcscpy_s _wcslwr_s
 *    caracteres  isalpha isdigit isspace tolower __pctype_func
 *    numeros     strtol strtoul strtoull atoi _wtoi strtod atof _wtof _itoa_s
 *    mates       sqrt sin cos tan asin acos atan atan2 log pow fmod modf
 *                frexp trunc round roundf ceilf floorf _dclass
 *    utilidad    qsort bsearch div
 *    printf _s   __stdio_common_vsprintf_s / vsnprintf_s, swprintf_s,
 *                _vsnwprintf; scanf: __stdio_common_vsscanf, swscanf_s
 *    ficheros    fopen fputs fclose fread (modo texto: \r\n) fgets fgetc
 *                ungetc fseek ftell feof; _open _read _lseeki64
 *                _filelengthi64 _close; _stat64 _access _errno
 *    rutas       _fullpath _splitpath_s _wmakepath_s
 *    y mas       _time64 _gmtime64 getenv _wgetenv_s _dupenv_s setlocale
 *                localeconv _beginthreadex strerror __std_exception_copy
 *                _o__stricmp (api-ms-win-crt-private)
 *    invalidos   con _set_invalid_parameter_handler, las _s cortas vuelven
 *                con su error (sin el, Windows mata el proceso)
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
#include <stdarg.h>

typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;
typedef struct { void *p; } FILE;

#define IMPORTA __declspec(dllimport)
IMPORTA void __stdcall ExitProcess(unsigned int c);
IMPORTA DWORD __stdcall WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int __stdcall CloseHandle(HANDLE h);
IMPORTA int __stdcall SetEnvironmentVariableW(const WCHAR *n, const WCHAR *v);

/* string */
IMPORTA char *__cdecl strchr(const char *s, int c);
IMPORTA char *__cdecl strrchr(const char *s, int c);
IMPORTA char *__cdecl strstr(const char *h, const char *n);
IMPORTA U64 __cdecl strspn(const char *s, const char *set);
IMPORTA U64 __cdecl strcspn(const char *s, const char *set);
IMPORTA char *__cdecl strpbrk(const char *s, const char *set);
IMPORTA U64 __cdecl strnlen(const char *s, U64 n);
IMPORTA char *__cdecl strncpy(char *d, const char *s, U64 n);
IMPORTA char *__cdecl strncat(char *d, const char *s, U64 n);
IMPORTA char *__cdecl _strdup(const char *s);
IMPORTA int __cdecl _stricmp(const char *a, const char *b);
IMPORTA int __cdecl _strnicmp(const char *a, const char *b, U64 n);
IMPORTA int __cdecl _memicmp(const void *a, const void *b, U64 n);
IMPORTA int __cdecl strcpy_s(char *d, U64 n, const char *s);
IMPORTA int __cdecl strcat_s(char *d, U64 n, const char *s);
IMPORTA int __cdecl strncpy_s(char *d, U64 n, const char *s, U64 c);
IMPORTA int __cdecl memcpy_s(void *d, U64 n, const void *s, U64 c);
IMPORTA WCHAR *__cdecl wcschr(const WCHAR *s, WCHAR c);
IMPORTA WCHAR *__cdecl wcsrchr(const WCHAR *s, WCHAR c);
IMPORTA WCHAR *__cdecl wcsstr(const WCHAR *h, const WCHAR *n);
IMPORTA int __cdecl wcscmp(const WCHAR *a, const WCHAR *b);
IMPORTA int __cdecl _wcsicmp(const WCHAR *a, const WCHAR *b);
IMPORTA int __cdecl wcscpy_s(WCHAR *d, U64 n, const WCHAR *s);
IMPORTA int __cdecl _wcslwr_s(WCHAR *s, U64 n);
IMPORTA int __cdecl isalpha(int c);
IMPORTA int __cdecl isdigit(int c);
IMPORTA int __cdecl isspace(int c);
IMPORTA int __cdecl tolower(int c);
IMPORTA WCHAR __cdecl towlower(WCHAR c);
IMPORTA const unsigned short *__cdecl __pctype_func(void);
/* convert */
IMPORTA long __cdecl strtol(const char *s, char **fin, int base);
IMPORTA unsigned long __cdecl strtoul(const char *s, char **fin, int base);
IMPORTA U64 __cdecl strtoull(const char *s, char **fin, int base);
IMPORTA int __cdecl atoi(const char *s);
IMPORTA int __cdecl _wtoi(const WCHAR *s);
IMPORTA double __cdecl strtod(const char *s, char **fin);
IMPORTA double __cdecl atof(const char *s);
IMPORTA double __cdecl _wtof(const WCHAR *s);
IMPORTA int __cdecl _itoa_s(int v, char *b, U64 n, int base);
/* math */
IMPORTA double __cdecl sqrt(double x);
IMPORTA double __cdecl sin(double x);
IMPORTA double __cdecl cos(double x);
IMPORTA double __cdecl tan(double x);
IMPORTA double __cdecl asin(double x);
IMPORTA double __cdecl acos(double x);
IMPORTA double __cdecl atan(double x);
IMPORTA double __cdecl atan2(double y, double x);
IMPORTA double __cdecl log(double x);
IMPORTA double __cdecl pow(double x, double y);
IMPORTA double __cdecl fmod(double x, double y);
IMPORTA double __cdecl modf(double x, double *i);
IMPORTA double __cdecl frexp(double x, int *e);
IMPORTA double __cdecl trunc(double x);
IMPORTA double __cdecl round(double x);
IMPORTA float __cdecl roundf(float x);
IMPORTA float __cdecl ceilf(float x);
IMPORTA float __cdecl floorf(float x);
IMPORTA short __cdecl _dclass(double x);
/* utility */
IMPORTA void __cdecl qsort(void *b, U64 n, U64 t, int(__cdecl *c)(const void *, const void *));
IMPORTA void *__cdecl bsearch(const void *k, const void *b, U64 n, U64 t, int(__cdecl *c)(const void *, const void *));
typedef struct { int quot, rem; } DIV;
IMPORTA DIV __cdecl div(int a, int b);
/* stdio */
IMPORTA FILE *__cdecl __acrt_iob_func(unsigned i);
IMPORTA int __cdecl __stdio_common_vfprintf(U64 op, FILE *f, const char *fmt, void *loc, va_list a);
IMPORTA int __cdecl __stdio_common_vsprintf_s(U64 op, char *b, U64 n, const char *fmt, void *loc, va_list a);
IMPORTA int __cdecl __stdio_common_vsnprintf_s(U64 op, char *b, U64 n, U64 c, const char *fmt, void *loc, va_list a);
IMPORTA int __cdecl __stdio_common_vsscanf(U64 op, const char *b, U64 n, const char *fmt, void *loc, va_list a);
IMPORTA FILE *__cdecl fopen(const char *r, const char *m);
IMPORTA int __cdecl fclose(FILE *f);
IMPORTA int __cdecl fputs(const char *s, FILE *f);
IMPORTA U64 __cdecl fread(void *p, U64 t, U64 n, FILE *f);
IMPORTA char *__cdecl fgets(char *b, int n, FILE *f);
IMPORTA int __cdecl fgetc(FILE *f);
IMPORTA int __cdecl ungetc(int c, FILE *f);
IMPORTA int __cdecl fseek(FILE *f, long o, int d);
IMPORTA long __cdecl ftell(FILE *f);
IMPORTA int __cdecl feof(FILE *f);
IMPORTA int __cdecl _open(const char *r, int f, ...);
IMPORTA int __cdecl _read(int fd, void *b, unsigned n);
IMPORTA I64 __cdecl _lseeki64(int fd, I64 o, int d);
IMPORTA int __cdecl _close(int fd);
IMPORTA I64 __cdecl _filelengthi64(int fd);
/* filesystem */
typedef struct { unsigned dev; unsigned short ino, mode; short nlink, uid, gid; unsigned rdev; I64 size, atime, mtime, ctime; } STAT64;
IMPORTA int __cdecl _stat64(const char *r, STAT64 *s);
IMPORTA int __cdecl _access(const char *r, int m);
IMPORTA char *__cdecl _fullpath(char *a, const char *r, U64 n);
IMPORTA int __cdecl _splitpath_s(const char *p, char *u, U64 un, char *d, U64 dn, char *f, U64 fn, char *e, U64 en);
IMPORTA int __cdecl _wmakepath_s(WCHAR *b, U64 n, const WCHAR *u, const WCHAR *d, const WCHAR *f, const WCHAR *e);
/* runtime, time, environment, locale, heap, private */
IMPORTA int *__cdecl _errno(void);
IMPORTA int __cdecl _initialize_narrow_environment(void);
IMPORTA int __cdecl _initialize_wide_environment(void);
typedef void(__cdecl *INVALIDO)(const WCHAR *, const WCHAR *, const WCHAR *, unsigned, U64);
IMPORTA INVALIDO __cdecl _set_invalid_parameter_handler(INVALIDO h);
IMPORTA U64 __cdecl _beginthreadex(void *s, unsigned p, unsigned(__stdcall *f)(void *), void *a, unsigned fl, unsigned *id);
IMPORTA char *__cdecl strerror(int e);
IMPORTA I64 __cdecl _time64(I64 *t);
IMPORTA int *__cdecl _gmtime64(const I64 *t);
IMPORTA char *__cdecl getenv(const char *n);
IMPORTA int __cdecl _wgetenv_s(U64 *req, WCHAR *b, U64 n, const WCHAR *nombre);
IMPORTA int __cdecl _dupenv_s(char **b, U64 *n, const char *nombre);
IMPORTA char *__cdecl setlocale(int c, const char *l);
IMPORTA char **__cdecl localeconv(void);
IMPORTA void __cdecl free(void *p);
IMPORTA int __cdecl _o__stricmp(const char *a, const char *b);
/* msvcrt */
IMPORTA int __cdecl swprintf_s(WCHAR *b, U64 n, const WCHAR *fmt, ...);
IMPORTA int __cdecl swscanf_s(const WCHAR *b, const WCHAR *fmt, ...);
IMPORTA int __cdecl _vsnwprintf(WCHAR *b, U64 n, const WCHAR *fmt, va_list a);
/* vcruntime140 */
typedef struct { const char *que; char soltar; } EXCD;
IMPORTA void __cdecl __std_exception_copy(const EXCD *de, EXCD *a);
IMPORTA void __cdecl __std_exception_destroy(EXCD *d);
IMPORTA int __cdecl memcmp(const void *a, const void *b, U64 n);

static unsigned fallos;
int _fltused = 0;

static void di(const char *fmt, ...) {
    va_list a;
    va_start(a, fmt);
    __stdio_common_vfprintf(4, __acrt_iob_func(1), fmt, 0, a);
    va_end(a);
}

static void mira(int bien, const char *que) {
    if (!bien)
        fallos++;
    di("  %s  %s\n", bien ? "bien" : "MAL ", que);
}

static int cerca(double a, double b) {
    double d = a - b;
    return (d < 0 ? -d : d) < 1e-12;
}

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int igual_w(const WCHAR *a, const WCHAR *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int __cdecl menor(const void *a, const void *b) {
    int x = *(const int *)a, y = *(const int *)b;
    return x < y ? -1 : x > y;
}

static int s_printf_s(char *b, U64 n, const char *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vsprintf_s(0, b, n, fmt, 0, a);
    va_end(a);
    return r;
}

static int sn_printf_s(char *b, U64 n, U64 c, const char *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vsnprintf_s(0, b, n, c, fmt, 0, a);
    va_end(a);
    return r;
}

static int s_scanf(const char *b, const char *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vsscanf(0, b, (U64)-1, fmt, 0, a);
    va_end(a);
    return r;
}

static int vw(WCHAR *b, U64 n, const WCHAR *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = _vsnwprintf(b, n, fmt, a);
    va_end(a);
    return r;
}

static volatile int del_hilo;

/* Una funcion `_s` con un bufer corto llama al manejador de parametros
 * invalidos; el de serie MATA el proceso. Los juegos ponen el suyo, y con el
 * la funcion vuelve con su error: es lo que se prueba aqui. */
static volatile int invalidos;
static void __cdecl manejador(const WCHAR *e, const WCHAR *f, const WCHAR *fi, unsigned l, U64 r) {
    (void)e, (void)f, (void)fi, (void)l, (void)r;
    invalidos++;
}
static unsigned __stdcall hilo(void *a) {
    del_hilo = (int)(U64)a;
    return 7;
}

void inicio(void) {
    char b[64], *fin;
    WCHAR w[64];
    int i, e;
    double x;

    _set_invalid_parameter_handler(manejador);
    /* -- cadenas */
    {
        const char *s = "cyberpunk 2077";
        mira(strchr(s, 'p') == s + 5 && strrchr(s, '7') == s + 13 && strstr(s, "2077") == s + 10 && !strstr(s, "night"), "strchr strrchr strstr");
        mira(strspn(s, "cyber") == 5 && strcspn(s, " ") == 9 && strpbrk(s, "0123456789") == s + 10 && strnlen(s, 4) == 4, "strspn strcspn strpbrk strnlen");
        for (i = 0; i < 8; i++)
            b[i] = 'x';
        strncpy(b, "ab", 6);
        mira(b[0] == 'a' && b[2] == 0 && b[5] == 0 && b[6] == 'x', "strncpy rellena con ceros");
        strncat(b, "cdef", 2);
        mira(igual(b, "abcd"), "strncat");
        fin = _strdup("Night City");
        mira(fin && igual(fin, "Night City") && _stricmp(fin, "NIGHT city") == 0 && _strnicmp(fin, "nightXX", 5) == 0 && _memicmp("ABC", "abd", 3) < 0, "_strdup _stricmp _strnicmp _memicmp");
        free(fin);
        mira(strcpy_s(b, 5, "1234") == 0 && strcat_s(b, 8, "567") == 0 && igual(b, "1234567"), "strcpy_s strcat_s");
        mira(strcpy_s(b, 4, "12345") == 34 && b[0] == 0, "strcpy_s que no cabe: ERANGE y vacio");
        mira(strncpy_s(b, 4, "abcdef", (U64)-1) == 80 && igual(b, "abc"), "strncpy_s con _TRUNCATE: STRUNCATE");
        mira(memcpy_s(b, 2, "xyz", 3) == 34 && memcpy_s(b, 4, "xyz", 3) == 0 && memcmp(b, "xyz", 3) == 0, "memcpy_s");
        mira(_o__stricmp("CYBER", "cyber") == 0, "_o__stricmp de api-ms-win-crt-private");
    }
    /* -- anchas */
    {
        const WCHAR *s = L"Johnny Silverhand";
        mira(wcschr(s, L'S') == s + 7 && wcsrchr(s, L'n') == s + 15 && wcsstr(s, L"hand") == s + 13, "wcschr wcsrchr wcsstr");
        mira(wcscmp(s, L"Johnny") > 0 && _wcsicmp(s, L"JOHNNY SILVERHAND") == 0, "wcscmp _wcsicmp");
        mira(wcscpy_s(w, 64, L"V MERC") == 0 && _wcslwr_s(w, 64) == 0 && igual_w(w, L"v merc") && towlower(L'Q') == L'q', "wcscpy_s _wcslwr_s towlower");
    }
    /* -- caracteres */
    mira(isalpha('x') && !isalpha('1') && isdigit('7') && isspace('\t') && tolower('K') == 'k', "isalpha isdigit isspace tolower");
    /* La tabla del locale "C": sin el bit 0x100 (C1_ALPHA) en las ASCII. */
    mira(__pctype_func()['A'] == 0x81 && __pctype_func()['z'] == 0x02 && __pctype_func()['7'] == 0x84 && __pctype_func()[' '] == 0x48, "__pctype_func: 'A' 0x81, 'z' 0x02, '7' 0x84, ' ' 0x48");
    mira(__pctype_func()[-1] == 0, "__pctype_func: la entrada de EOF (-1) a cero");
    /* -- numeros */
    {
        long v = strtol("  -0x1Fz", &fin, 0);
        mira(v == -31 && *fin == 'z', "strtol en base 0 con 0x");
        mira(strtoul("-1", 0, 10) == 0xFFFFFFFFul && strtoull("18446744073709551615", 0, 10) == 0xFFFFFFFFFFFFFFFFull, "strtoul strtoull");
        *_errno() = 0;
        v = strtol("99999999999", 0, 10);
        mira(v == 0x7FFFFFFF && *_errno() == 34, "strtol que se pasa: LONG_MAX y ERANGE");
        mira(atoi("  42abc") == 42 && _wtoi(L"-7") == -7, "atoi _wtoi");
        x = strtod("3.25e2x", &fin);
        mira(x == 325.0 && *fin == 'x' && atof("0.5") == 0.5 && _wtof(L"-1e-3") == -0.001, "strtod atof _wtof");
        mira(_itoa_s(-255, b, 16, 10) == 0 && igual(b, "-255") && _itoa_s(255, b, 16, 16) == 0 && igual(b, "ff") && _itoa_s(12345, b, 3, 10) == 34, "_itoa_s");
    }
    /* -- mates */
    {
        double ent;
        mira(cerca(sqrt(2.0) * sqrt(2.0), 2.0) && cerca(sin(0.5) * sin(0.5) + cos(0.5) * cos(0.5), 1.0) && cerca(tan(0.25), sin(0.25) / cos(0.25)), "sqrt sin cos tan");
        mira(cerca(atan2(1.0, 1.0) * 4.0, 3.141592653589793) && cerca(atan(1.0), atan2(1.0, 1.0)) && cerca(asin(0.5) * 6.0, 3.141592653589793) && cerca(acos(-1.0), 3.141592653589793), "atan atan2 asin acos");
        mira(cerca(log(2.718281828459045), 1.0) && pow(2.0, 10.0) == 1024.0 && pow(-2.0, 3.0) == -8.0 && pow(-8.0, 1.0 / 3.0) != pow(-8.0, 1.0 / 3.0), "log pow (y pow negativo no entero: NaN)");
        mira(fmod(7.5, 2.0) == 1.5 && fmod(-7.5, 2.0) == -1.5 && modf(-3.75, &ent) == -0.75 && ent == -3.0, "fmod modf");
        mira(frexp(8.0, &e) == 0.5 && e == 4 && trunc(-1.7) == -1.0 && round(2.5) == 3.0 && round(-2.5) == -3.0, "frexp trunc round");
        mira(roundf(-2.5f) == -3.0f && ceilf(1.2f) == 2.0f && floorf(-1.2f) == -2.0f && _dclass(0.0) == 0 && _dclass(ent) == -1, "roundf ceilf floorf _dclass");
    }
    /* -- utilidad */
    {
        int v[] = {5, -1, 9, 3, 3, 0, 7, 100, -50, 2}, k = 7;
        DIV d = div(-7, 2);
        qsort(v, 10, sizeof(int), menor);
        mira(v[0] == -50 && v[3] == 2 && v[9] == 100 && bsearch(&k, v, 10, sizeof(int), menor) == &v[7], "qsort bsearch");
        mira(d.quot == -3 && d.rem == -1, "div");
    }
    /* -- printf _s y scanf */
    {
        int n = 0;
        double r = 0;
        char s[16];
        WCHAR ws[16];
        mira(s_printf_s(b, 8, "%d-%s", 12, "ab") == 5 && igual(b, "12-ab") && s_printf_s(b, 4, "%s", "demasiado") == -1 && b[0] == 0, "__stdio_common_vsprintf_s");
        mira(sn_printf_s(b, 16, (U64)-1, "%s", "0123456789abcdefXYZ") == -1 && igual(b, "0123456789abcde") && sn_printf_s(b, 16, 3, "%d", 12345) == -1 && igual(b, "123"), "__stdio_common_vsnprintf_s");
        mira(swprintf_s(w, 64, L"%d-%s", 7, L"ab") == 4 && igual_w(w, L"7-ab"), "swprintf_s (msvcrt, variadica)");
        mira(vw(w, 3, L"%s", L"largo") == -1 && w[0] == L'l' && w[2] == L'r', "_vsnwprintf: sin cero si no cabe, y -1");
        mira(s_scanf("  42 2.5 hola,mundo", "%d %lf %[^,],%s", &n, &r, s, b) == 4 && n == 42 && r == 2.5 && igual(s, "hola") && igual(b, "mundo"), "__stdio_common_vsscanf");
        mira(swscanf_s(L"9 arasaka", L"%d %s", &n, ws, 16) == 2 && n == 9 && igual_w(ws, L"arasaka"), "swscanf_s (msvcrt, variadica, con medida)");
    }
    /* -- ficheros */
    {
        FILE *f = fopen("tanda1.txt", "w");
        STAT64 st;
        int fd;
        mira(f && fputs("uno\ndos\n", f) >= 0 && fclose(f) == 0, "fopen w, fputs, fclose");
        f = fopen("tanda1.txt", "rb");
        mira(f && fread(b, 1, 64, f) == 10 && memcmp(b, "uno\r\ndos\r\n", 10) == 0 && feof(f), "fread en binario: el texto salio con \\r\\n");
        fclose(f);
        f = fopen("tanda1.txt", "r");
        mira(f && fgets(b, 64, f) && igual(b, "uno\n") && ftell(f) == 5, "fgets en texto: \\r\\n llega \\n");
        mira(fseek(f, 0, 0) == 0 && fgetc(f) == 'u' && ungetc('U', f) == 'U' && fgetc(f) == 'U' && fgetc(f) == 'n', "fseek fgetc ungetc");
        fclose(f);
        mira(_stat64("tanda1.txt", &st) == 0 && st.size == 10 && (st.mode & 0x8000) && _access("tanda1.txt", 0) == 0, "_stat64 _access");
        *_errno() = 0;
        mira(_access("no_esta.txt", 0) == -1 && *_errno() == 2 && fopen("no_esta.txt", "r") == 0 && igual(strerror(2), "No such file or directory"), "lo que no esta: ENOENT");
        fd = _open("tanda1.txt", 0x8000);
        mira(fd > 2 && _filelengthi64(fd) == 10 && _lseeki64(fd, 5, 0) == 5 && _read(fd, b, 3) == 3 && memcmp(b, "dos", 3) == 0 && _close(fd) == 0, "_open _filelengthi64 _lseeki64 _read _close");
    }
    /* -- rutas */
    {
        char u[4], d[32], n[16], x2[8];
        mira(_splitpath_s("C:\\juegos\\bin\\Cyberpunk2077.exe", u, 4, d, 32, n, 16, x2, 8) == 0 && igual(u, "C:") && igual(d, "\\juegos\\bin\\") && igual(n, "Cyberpunk2077") && igual(x2, ".exe"), "_splitpath_s");
        mira(_wmakepath_s(w, 64, L"D", L"juegos", L"cp", L"exe") == 0 && igual_w(w, L"D:juegos\\cp.exe"), "_wmakepath_s");
        fin = _fullpath(0, "tanda1.txt", 0);
        mira(fin && strstr(fin, "tanda1.txt") && fin[1] == ':', "_fullpath");
        free(fin);
    }
    /* -- la hora, el entorno, el locale */
    {
        I64 t = _time64(0);
        int *tm = _gmtime64(&t);
        U64 req = 1;
        char *p = 0;
        mira(t > 1700000000 && tm && tm[5] >= 123 && tm[4] < 12 && tm[3] >= 1, "_time64 _gmtime64");
        /* El CRT tiene su COPIA del entorno, hecha al cargarse (antes que
         * nada del .exe): lo que se ponga despues con
         * SetEnvironmentVariableW no la cambia. OS=Windows_NT esta desde el
         * arranque, en Windows y en la casa. */
        SetEnvironmentVariableW(L"BMO_TANDA", L"uno");
        mira(getenv("OS") && igual(getenv("OS"), "Windows_NT") && getenv("NO_HAY_TAL") == 0, "getenv de una del arranque");
        mira(getenv("BMO_TANDA") == 0, "getenv lee la copia del CRT: SetEnvironmentVariableW de despues no la cambia");
        mira(_wgetenv_s(&req, w, 64, L"OS") == 0 && req == 11 && igual_w(w, L"Windows_NT") && _dupenv_s(&p, &req, "OS") == 0 && p && igual(p, "Windows_NT"), "_wgetenv_s _dupenv_s");
        free(p);
        /* "" es el del usuario (en tu Windows, el tuyo). */
        mira(setlocale(0, "") != 0 && setlocale(0, "klingon") == 0 && igual(setlocale(0, "C"), "C") && localeconv()[0][0] == '.', "setlocale (\"\", uno que no hay, \"C\") y localeconv");
    }
    /* -- un hilo y lo de C++ */
    {
        unsigned id = 0;
        U64 h = _beginthreadex(0, 0, hilo, (void *)77, 0, &id);
        EXCD de = {"se acabo la bateria", 1}, a = {0, 0};
        mira(h && WaitForSingleObject((HANDLE)h, 5000) == 0 && del_hilo == 77 && CloseHandle((HANDLE)h), "_beginthreadex");
        __std_exception_copy(&de, &a);
        mira(a.soltar && a.que != de.que && igual(a.que, de.que), "__std_exception_copy copia el texto");
        __std_exception_destroy(&a);
        mira(a.que == 0 && !a.soltar, "__std_exception_destroy");
    }
    /* strcpy_s, memcpy_s, _itoa_s y __stdio_common_vsprintf_s cortos. */
    mira(invalidos == 4, "el manejador de parametros invalidos, 4 veces");
    di("tanda1.exe: el C runtime de Cyberpunk es el de Windows\n");
    ExitProcess(fallos);
}
