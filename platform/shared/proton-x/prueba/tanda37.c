/* tanda37.c -- el .exe de la TANDA 37 (01-10): las FECHAS de un fichero.
 * Por Cyberpunk: mira su final.redscripts SOLO con GetFileAttributesExW y,
 * con las fechas a 0 (un fichero de 1601), dice "corrupted or missing
 * scripts file". Aqui el .exe se mira a si mismo por los cuatro caminos de
 * Windows -- GetFileAttributesExW, FindFirstFileW, GetFileTime y
 * GetFileInformationByHandleEx(FileBasicInfo) -- y exige que la fecha de
 * escritura sea de despues del 2000 y la MISMA por los cuatro, y que no sea
 * una carpeta. No depende de cuando se copio el .exe.
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
IMPORTA DWORD W GetModuleFileNameW(HANDLE m, WCHAR *b, DWORD n);
IMPORTA int W GetFileAttributesExW(const WCHAR *n, int nivel, void *d);
IMPORTA HANDLE W FindFirstFileW(const WCHAR *n, void *d);
IMPORTA int W FindClose(HANDLE h);
IMPORTA HANDLE W CreateFileW(const WCHAR *n, DWORD acc, DWORD comp, void *seg, DWORD disp, DWORD band, HANDLE plantilla);
IMPORTA int W GetFileTime(HANDLE h, U64 *c, U64 *l, U64 *e);
IMPORTA int W GetFileInformationByHandleEx(HANDLE h, int clase, void *b, DWORD n);
IMPORTA int W CloseHandle(HANDLE h);

/* 2000-01-01 en FILETIME */
#define DOSMIL 125911584000000000ULL

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

static U64 u64_en(const unsigned char *b) {
    U64 v = 0;
    int i;
    for (i = 7; i >= 0; i--)
        v = v << 8 | b[i];
    return v;
}

void inicio(void);

void inicio(void) {
    static WCHAR ruta[520];
    static unsigned char atr[36], hallado[592], basica[40];
    U64 escrito_atr = 0, escrito_find = 0, escrito_h = 0, c = 0, l = 0;
    HANDLE h;
    DWORD atributos;

    GetModuleFileNameW(0, ruta, 520);
    mira(GetFileAttributesExW(ruta, 0, atr) != 0, "GetFileAttributesExW del propio .exe");
    atributos = atr[0] | atr[1] << 8 | atr[2] << 16 | (DWORD)atr[3] << 24;
    escrito_atr = u64_en(atr + 20);
    mira((atributos & 0x10) == 0 && atributos != 0, "sus atributos: un fichero, no una carpeta");
    mira(escrito_atr > DOSMIL, "su fecha de escritura es de despues del 2000, no de 1601");
    mira(u64_en(atr + 4) > DOSMIL, "y la de creacion tambien");

    h = FindFirstFileW(ruta, hallado);
    mira(h != (HANDLE)-1, "FindFirstFileW lo encuentra");
    if (h != (HANDLE)-1) {
        escrito_find = u64_en(hallado + 20);
        FindClose(h);
    }
    mira(escrito_find == escrito_atr, "FindFirstFileW da la misma fecha de escritura");

    h = CreateFileW(ruta, 0x80000000, 1, 0, 3, 0, 0);
    mira(h != (HANDLE)-1, "CreateFileW lo abre para leer");
    if (h != (HANDLE)-1) {
        mira(GetFileTime(h, &c, &l, &escrito_h) && escrito_h == escrito_atr, "GetFileTime da la misma fecha de escritura");
        mira(GetFileInformationByHandleEx(h, 0, basica, 40) && u64_en(basica + 16) == escrito_atr, "FileBasicInfo da la misma fecha de escritura");
        CloseHandle(h);
    }
    di("tanda37.exe: las fechas de un fichero, las mismas por los cuatro caminos\r\n");
    ExitProcess(fallos);
}
