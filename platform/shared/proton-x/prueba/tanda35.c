/* tanda35.c -- el .exe de la TANDA 35 (01-10): IDXGIAdapter::GetDesc.
 * Por Cyberpunk: tras Streamline, el juego crea la fabrica, toma el
 * adaptador 0 y pide su descripcion con GetDesc (el hueco 8, el de antes
 * del 1). La casa solo tenia GetDesc1 y el .exe salio. Aqui: la fabrica,
 * el adaptador, GetDesc y GetDesc1 contestan S_OK y dicen LO MISMO (el
 * nombre, el fabricante, el dispositivo, las memorias y el LUID), y GetDesc
 * sin donde escribir es E_INVALIDARG. No depende de que tarjeta haya.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef long HRESULT;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
typedef struct { DWORD a; unsigned short b, c; unsigned char d[8]; } GUID;
IMPORTA HRESULT W CreateDXGIFactory1(const GUID *riid, void **pp);

#define E_INVALIDARG ((HRESULT)0x80070057)

typedef HRESULT (W *Enum1)(void *this_, unsigned i, void **pp);
typedef HRESULT (W *Desc)(void *this_, void *desc);
typedef unsigned long (W *Soltar)(void *this_);

static const GUID IID_IDXGIFactory1 = {0x770aae78, 0xf26f, 0x4dba, {0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87}};

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

static void *hueco(void *obj, int k) {
    return (*(void ***)obj)[k];
}

void inicio(void);

void inicio(void) {
    static unsigned char d[312], d1[312];
    void *f = 0, *a = 0;
    HRESULT r;
    int i, igual = 1;

    r = CreateDXGIFactory1(&IID_IDXGIFactory1, &f);
    mira(r == 0 && f != 0, "CreateDXGIFactory1: la fabrica");
    r = f ? ((Enum1)hueco(f, 12))(f, 0, &a) : -1;
    mira(r == 0 && a != 0, "EnumAdapters1(0): el primer adaptador");
    for (i = 0; i < 312; i++)
        d[i] = d1[i] = 0xAB;
    r = a ? ((Desc)hueco(a, 8))(a, d) : -1;
    mira(r == 0, "GetDesc (hueco 8): S_OK");
    r = a ? ((Desc)hueco(a, 10))(a, d1) : -1;
    mira(r == 0, "GetDesc1 (hueco 10): S_OK");
    for (i = 0; i < 304; i++)
        igual &= d[i] == d1[i];
    mira(igual, "dicen lo mismo: nombre, ids, memorias y LUID");
    mira(d[0] != 0xAB && (d[0] | d[1]) != 0, "y el nombre no esta vacio");
    mira(d[304] == 0xAB && d[311] == 0xAB, "GetDesc no escribe mas alla de sus 304 bytes");
    r = a ? ((Desc)hueco(a, 8))(a, 0) : 0;
    mira(r == E_INVALIDARG, "GetDesc sin donde escribir: E_INVALIDARG");
    if (a)
        ((Soltar)hueco(a, 2))(a);
    if (f)
        ((Soltar)hueco(f, 2))(f);
    di("tanda35.exe: IDXGIAdapter::GetDesc, lo mismo que GetDesc1\r\n");
    ExitProcess(fallos);
}
