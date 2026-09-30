/* tanda8.c -- el .exe de la TANDA 8 de Cyberpunk (30-09): el locale de
 * kernel32. Comparar y cambiar texto, lo que un locale sabe y los formatos
 * de fecha, hora, numero y moneda. Siempre con "en-US" (0x409) o el
 * invariante, para que CUALQUIER Windows diga lo mismo que la casa.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA int W CompareStringEx(const WCHAR *l, DWORD f, const WCHAR *a, int na, const WCHAR *b, int nb, void *v, void *r, long long p);
IMPORTA int W CompareStringW(DWORD l, DWORD f, const WCHAR *a, int na, const WCHAR *b, int nb);
IMPORTA int W LCMapStringEx(const WCHAR *l, DWORD f, const WCHAR *s, int n, WCHAR *d, int m, void *v, void *r, long long p);
IMPORTA int W LCMapStringW(DWORD l, DWORD f, const WCHAR *s, int n, WCHAR *d, int m);
IMPORTA int W LCMapStringA(DWORD l, DWORD f, const char *s, int n, char *d, int m);
IMPORTA int W GetLocaleInfoEx(const WCHAR *l, DWORD t, WCHAR *b, int n);
IMPORTA int W GetLocaleInfoW(DWORD l, DWORD t, WCHAR *b, int n);
IMPORTA int W GetLocaleInfoA(DWORD l, DWORD t, char *b, int n);
IMPORTA int W GetGeoInfoW(long g, DWORD t, WCHAR *b, int n, WORD lang);
IMPORTA int W GetDateFormatEx(const WCHAR *l, DWORD f, const WORD *st, const WCHAR *p, WCHAR *b, int n, const WCHAR *cal);
IMPORTA int W GetDateFormatW(DWORD l, DWORD f, const WORD *st, const WCHAR *p, WCHAR *b, int n);
IMPORTA int W GetTimeFormatEx(const WCHAR *l, DWORD f, const WORD *st, const WCHAR *p, WCHAR *b, int n);
IMPORTA int W GetTimeFormatW(DWORD l, DWORD f, const WORD *st, const WCHAR *p, WCHAR *b, int n);
IMPORTA int W GetNumberFormatEx(const WCHAR *l, DWORD f, const WCHAR *v, const void *fmt, WCHAR *b, int n);
IMPORTA int W GetCurrencyFormatEx(const WCHAR *l, DWORD f, const WCHAR *v, const void *fmt, WCHAR *b, int n);

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

static int igual_w(const WCHAR *a, const WCHAR *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int menor_bytes(const unsigned char *a, int na, const unsigned char *b, int nb) {
    int i;
    for (i = 0; i < na && i < nb; i++)
        if (a[i] != b[i])
            return a[i] < b[i];
    return na < nb;
}

#define EN L"en-US"
#define LCID 0x409

void inicio(void) {
    static WCHAR b[128];
    static char a[128];
    static unsigned char k1[64], k2[64];
    static WORD st[8] = {2026, 9, 0, 29, 15, 4, 5, 0};
    static DWORD numero;
    int n, n1, n2;

    /* -- comparar */
    mira(CompareStringEx(EN, 1, L"abc", -1, L"ABC", -1, 0, 0, 0) == 2, "CompareStringEx sin mayusculas: igual");
    mira(CompareStringEx(EN, 0, L"a", -1, L"A", -1, 0, 0, 0) == 1, "CompareStringEx: la minuscula antes que la mayuscula");
    mira(CompareStringW(LCID, 0, L"apple", -1, L"Banana", -1) == 1 && CompareStringW(LCID, 0, L"zeta", 2, L"zebra", 2) == 2, "CompareStringW: por letras, y con largo");
    mira(CompareStringEx(EN, 0x800000, L"a", -1, L"b", -1, 0, 0, 0) == 0 && GetLastError() == 1004, "CompareStringEx con una bandera que no hay: ERROR_INVALID_FLAGS");
    /* -- cambiar */
    n = LCMapStringEx(EN, 0x200, L"Hola Night City", -1, b, 128, 0, 0, 0);
    mira(n == 16 && igual_w(b, L"HOLA NIGHT CITY"), "LCMapStringEx: mayusculas, con el 0");
    n = LCMapStringW(LCID, 0x100, L"ARASAKA", 3, b, 128);
    mira(n == 3 && b[0] == 'a' && b[2] == 'a', "LCMapStringW: minusculas, sin el 0");
    mira(LCMapStringW(LCID, 0x100, L"abc", -1, b, 0) == 4, "LCMapStringW con 0: lo que hace falta");
    mira(LCMapStringA(LCID, 0x200, "v", -1, a, 128) == 2 && igual(a, "V"), "LCMapStringA");
    n1 = LCMapStringEx(EN, 0x400, L"a", -1, (WCHAR *)k1, 64, 0, 0, 0);
    n2 = LCMapStringEx(EN, 0x400, L"b", -1, (WCHAR *)k2, 64, 0, 0, 0);
    mira(n1 > 0 && n2 > 0 && menor_bytes(k1, n1, k2, n2), "LCMapStringEx LCMAP_SORTKEY: a antes que b");
    /* -- el locale */
    mira(GetLocaleInfoEx(EN, 0x0E, b, 128) == 2 && igual_w(b, L".") && GetLocaleInfoEx(EN, 0x0F, b, 128) == 2 && igual_w(b, L","), "GetLocaleInfoEx: el punto decimal y el de miles");
    mira(GetLocaleInfoEx(EN, 0x5C, b, 128) == 6 && igual_w(b, L"en-US") && GetLocaleInfoEx(EN, 0x1001, b, 128) > 0 && igual_w(b, L"English"), "GetLocaleInfoEx: el nombre, y el idioma en ingles");
    mira(GetLocaleInfoW(LCID, 0x38 + 8, b, 128) && igual_w(b, L"September") && GetLocaleInfoW(LCID, 0x2A + 1, b, 128) && igual_w(b, L"Tuesday"), "GetLocaleInfoW: el mes 9 y el dia 2 (martes)");
    mira(GetLocaleInfoW(LCID, 0x1F, b, 128) && igual_w(b, L"M/d/yyyy") && GetLocaleInfoW(LCID, 0x1003, b, 128) && igual_w(b, L"h:mm:ss tt"), "GetLocaleInfoW: la fecha corta y la hora");
    mira(GetLocaleInfoW(LCID, 0x20000000 | 0x11, (WCHAR *)&numero, 2) == 2 && numero == 2, "GetLocaleInfoW con LOCALE_RETURN_NUMBER: 2 decimales");
    mira(GetLocaleInfoA(LCID, 0x14, a, 128) == 2 && igual(a, "$") && GetLocaleInfoW(LCID, 0x0E, b, 1) == 0 && GetLastError() == 122, "GetLocaleInfoA, y la W que no cabe");
    mira(GetGeoInfoW(244, 4, b, 128, 0) == 3 && igual_w(b, L"US") && GetGeoInfoW(244, 5, b, 128, 0) == 4 && igual_w(b, L"USA"), "GetGeoInfoW 244: US y USA");
    /* -- fechas y horas */
    mira(GetDateFormatEx(EN, 2, st, 0, b, 128, 0) > 0 && igual_w(b, L"Tuesday, September 29, 2026"), "GetDateFormatEx larga (y el martes lo calcula)");
    mira(GetDateFormatW(LCID, 1, st, 0, b, 128) > 0 && igual_w(b, L"9/29/2026"), "GetDateFormatW corta");
    mira(GetDateFormatEx(EN, 0, st, L"dd 'de' MMM yy", b, 128, 0) > 0 && igual_w(b, L"29 de Sep 26"), "GetDateFormatEx con su picture y un literal");
    mira(GetTimeFormatEx(EN, 0, st, 0, b, 128) > 0 && igual_w(b, L"3:04:05 PM"), "GetTimeFormatEx");
    mira(GetTimeFormatW(LCID, 2, st, 0, b, 128) > 0 && igual_w(b, L"3:04 PM"), "GetTimeFormatW sin segundos");
    mira(GetTimeFormatEx(EN, 0, st, L"HH:mm", b, 128) > 0 && igual_w(b, L"15:04"), "GetTimeFormatEx con su picture de 24 horas");
    /* -- numeros y moneda */
    mira(GetNumberFormatEx(EN, 0, L"1234567.891", 0, b, 128) > 0 && igual_w(b, L"1,234,567.89"), "GetNumberFormatEx");
    mira(GetNumberFormatEx(EN, 0, L"-0.005", 0, b, 128) > 0 && igual_w(b, L"-0.01"), "GetNumberFormatEx redondea lejos del cero");
    mira(GetCurrencyFormatEx(EN, 0, L"1234.5", 0, b, 128) > 0 && igual_w(b, L"$1,234.50"), "GetCurrencyFormatEx");
    di("tanda8.exe: el locale de kernel32 es el de Windows\r\n");
    ExitProcess(fallos);
}
