/* stdio.c -- el .exe de P4f5 de PROTON-X: el printf del CRT de MSVC, como
 * lo llaman las cabeceras del UCRT (printf, snprintf, _vsnprintf, swprintf
 * son funciones EN LINEA que acaban en __stdio_common_*, con sus banderas).
 *
 *    snprintf     (STANDARD_SNPRINTF_BEHAVIOR): %d %5.2f %s %x, %e con dos
 *                 cifras de exponente, %g, %p en 16 cifras; cortar dice el
 *                 largo entero; (NULL, 0) cuenta
 *    _vsnprintf   (legado): lo que no cabe da -1 y no pone el 0
 *    swprintf     ancho: %s es ancho (LEGACY_WIDE_SPECIFIERS)
 *    stdout       printf, puts, fputs a stderr, fwrite, fflush: en modo
 *                 texto ("\n" sale "\r\n")
 *
 * Importa de api-ms-win-crt-stdio-l1-1-0.dll. Sale con el numero de fallos.
 * En Windows dice lo mismo. */
#include <stdarg.h>

typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;
typedef struct { void *p; } FILE;

#define IMPORTA __declspec(dllimport)
IMPORTA void __stdcall ExitProcess(unsigned int c);
/* api-ms-win-crt-stdio-l1-1-0.dll */
IMPORTA FILE *__cdecl __acrt_iob_func(unsigned i);
IMPORTA int __cdecl __stdio_common_vfprintf(U64 op, FILE *f, const char *fmt, void *loc, va_list a);
IMPORTA int __cdecl __stdio_common_vsprintf(U64 op, char *b, U64 n, const char *fmt, void *loc, va_list a);
IMPORTA int __cdecl __stdio_common_vswprintf(U64 op, WCHAR *b, U64 n, const WCHAR *fmt, void *loc, va_list a);
IMPORTA int __cdecl puts(const char *s);
IMPORTA int __cdecl fputs(const char *s, FILE *f);
IMPORTA U64 __cdecl fwrite(const void *p, U64 m, U64 n, FILE *f);
IMPORTA int __cdecl fflush(FILE *f);

/* Las banderas de corecrt_stdio_config.h. */
#define NULO_LEGADO 1ull
#define SNPRINTF_ESTANDAR 2ull
#define ANCHOS_LEGADOS 4ull

static unsigned fallos;
/* Lo pide el compilador en cuanto hay un double; lo pone el CRT ESTATICO, y
 * aqui no hay: se define (como el CRT, a 0). */
int _fltused = 0;

static int mi_printf(const char *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vfprintf(ANCHOS_LEGADOS, __acrt_iob_func(1), fmt, 0, a);
    va_end(a);
    return r;
}

static int mi_snprintf(char *b, U64 n, const char *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vsprintf(ANCHOS_LEGADOS | SNPRINTF_ESTANDAR, b, n, fmt, 0, a);
    va_end(a);
    return r;
}

static int mi_vsnprintf_legado(char *b, U64 n, const char *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vsprintf(ANCHOS_LEGADOS, b, n, fmt, 0, a);
    va_end(a);
    return r;
}

static int mi_swprintf(WCHAR *b, U64 n, const WCHAR *fmt, ...) {
    va_list a;
    int r;
    va_start(a, fmt);
    r = __stdio_common_vswprintf(ANCHOS_LEGADOS | SNPRINTF_ESTANDAR, b, n, fmt, 0, a);
    va_end(a);
    return r;
}

static int igual(const char *a, const char *b) {
    while (*a && *a == *b) a++, b++;
    return *a == *b;
}

static void mira(int bien, const char *que, int valor) {
    mi_printf("%s%s %d\n", bien ? "  bien  " : "  MAL   ", que, valor);
    if (!bien) fallos++;
}

void inicio(void) {
    char b[64];
    WCHAR w[16];
    int n;

    mi_printf("stdio.exe: el printf del CRT de MSVC\n");
    n = mi_snprintf(b, 64, "%d|%5.2f|%s|%x", 42, 3.14159, "hola", 255);
    mira(igual(b, "42| 3.14|hola|ff"), "snprintf: %d|%5.2f|%s|%x", n);
    mi_snprintf(b, 64, "%e %.3g %g %g", 123456.789, 3.14159, 0.0001, 0.00001);
    mira(igual(b, "1.234568e+05 3.14 0.0001 1e-05"), "%e con dos cifras de exponente, y %g", 0);
    mi_snprintf(b, 64, "%p|%-4d|%+d|%05.1f", (void *)0xABCD, 7, 7, -2.26);
    mira(igual(b, "000000000000ABCD|7   |+7|-02.3"), "%p en 16 cifras, %-4d, %+d, %05.1f", 0);
    n = mi_snprintf(b, 5, "%s", "abcdefgh");
    mira(n == 8 && igual(b, "abcd"), "snprintf corta con su 0 y dice el largo entero", n);
    n = mi_snprintf(0, 0, "%d-%d", 123, 45);
    mira(n == 6, "(NULL, 0) cuenta lo que haria falta", n);
    b[5] = 'Z';
    n = mi_vsnprintf_legado(b, 5, "%s", "abcdefgh");
    mira(n == -1 && b[4] == 'e' && b[5] == 'Z', "_vsnprintf legado: -1, y sin el 0", n);
    n = mi_swprintf(w, 16, L"%s=%d", L"xy", 7);
    mira(n == 4 && w[0] == 'x' && w[1] == 'y' && w[2] == '=' && w[3] == '7' && w[4] == 0, "swprintf: %s es ancho", n);
    mi_snprintf(b, 64, "%ls|%hs", L"abc", "def");
    mira(igual(b, "abc|def"), "%ls y %hs en el estrecho", 0);

    n = mi_printf("  bien  printf a stdout (%s)\n", "esta linea");
    mira(n == 37, "printf dijo cuantos bytes (antes del \\r)", n);
    puts("  bien  puts (esta linea)");
    fputs("  bien  fputs a stderr (esta linea)\n", __acrt_iob_func(2));
    n = (int)fwrite("  bien  fwrite (esta linea)\n", 1, 28, __acrt_iob_func(1));
    mira(n == 28 && fflush(__acrt_iob_func(1)) == 0, "fwrite y fflush", n);

    mi_printf(fallos ? "stdio.exe: ALGO NO es como en Windows\n" : "stdio.exe: el printf es el de Windows\n");
    ExitProcess(fallos);
}
