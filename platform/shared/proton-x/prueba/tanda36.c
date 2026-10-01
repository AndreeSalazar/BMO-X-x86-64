/* tanda36.c -- el .exe de la TANDA 36 (01-10): el idioma de un recurso.
 * Por Cyberpunk: su aviso salio en arabe, porque la casa tomaba el PRIMER
 * idioma del STRINGTABLE y el arabe (0x401) va antes que el ingles (0x409).
 * Windows elige por idioma: el del usuario y, si no esta, el ingles de
 * EE. UU.; solo si no hay ninguno de esos, el que haya. Aqui: la cadena 7
 * esta en arabe y en ingles (sale la inglesa, entera y con su 0). Windows
 * elige el BLOQUE de 16 cadenas, no cada una: la 9 solo esta en el bloque
 * arabe, y en el ingles (el de la 7) esta vacia: 0. La 20, de otro bloque
 * que solo esta en arabe, sale la arabe. Con una tabla sin el idioma de esta
 * maquina, en Windows en ingles o en castellano dice lo mismo.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA int W LoadStringW(HANDLE inst, unsigned id, WCHAR *b, int n);

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

static int igual_w(const WCHAR *a, const char *b) {
    while (*b)
        if (*a++ != (WCHAR)*b++)
            return 0;
    return *a == 0;
}

void inicio(void);

void inicio(void) {
    static WCHAR b[64];
    WCHAR *p = 0;
    HANDLE inst = GetModuleHandleW(0);
    int x;

    x = LoadStringW(inst, 7, b, 64);
    mira(x == 10 && igual_w(b, "Night City"), "LoadStringW 7: la inglesa, no la arabe que va antes");
    x = LoadStringW(inst, 7, (WCHAR *)&p, 0);
    mira(x == 10 && p && p[0] == 'N', "LoadStringW 7 con 0: el puntero a la inglesa");
    x = LoadStringW(0, 7, b, 64);
    mira(x == 10 && b[0] == 'N', "LoadStringW 7 sin instancia: la del .exe, la inglesa");
    b[0] = 'x';
    x = LoadStringW(inst, 9, b, 64);
    mira(x == 0 && b[0] == 0, "LoadStringW 9: en el bloque ingles esta vacia, 0");
    x = LoadStringW(inst, 20, b, 64);
    mira(x == 4 && b[0] == 0x0634 && b[4] == 0, "LoadStringW 20: su bloque solo esta en arabe, sale la arabe");
    b[0] = 'x';
    x = LoadStringW(inst, 8, b, 64);
    mira(x == 0 && b[0] == 0, "LoadStringW 8: no esta en ningun idioma, 0 y vacia");

    di("tanda36.exe: LoadStringW elige el ingles, no el primer idioma\r\n");
    ExitProcess(fallos);
}
