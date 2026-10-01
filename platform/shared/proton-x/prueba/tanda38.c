/* tanda38.c -- el .exe de la TANDA 38 (01-10): IDXGIAdapter::EnumOutputs.
 * Por Cyberpunk: con el dispositivo ya creado, el juego pregunta al
 * adaptador "que monitores tienes" (EnumOutputs, el hueco 7) y la casa no
 * lo tenia: el .exe salio con 0xC0DE0C07. Aqui: la salida 0 existe, esta
 * en el escritorio con un rectangulo de verdad, un nombre \\.\ y un
 * HMONITOR; tiene modos en R8G8B8A8 (con medida, hercios y ese formato), y
 * con sitio para uno solo es DXGI_ERROR_MORE_DATA; la salida 64 no existe
 * (NOT_FOUND y el puntero a nulo); es tambien IDXGIOutput1 y 6, su padre
 * es el adaptador, y GetDesc1 dice el mismo nombre con 8 bits o mas.
 * No depende de que monitor haya.
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

#define DXGI_ERROR_NOT_FOUND ((HRESULT)0x887A0002)
#define DXGI_ERROR_MORE_DATA ((HRESULT)0x887A0003)
#define R8G8B8A8 28

typedef HRESULT (W *Enum)(void *this_, unsigned i, void **pp);
typedef HRESULT (W *Pide)(void *this_, const GUID *riid, void **pp);
typedef HRESULT (W *Desc)(void *this_, void *desc);
typedef HRESULT (W *Modos)(void *this_, unsigned formato, unsigned banderas, unsigned *n, void *lista);
typedef unsigned long (W *Soltar)(void *this_);

static const GUID IID_IDXGIFactory1 = {0x770aae78, 0xf26f, 0x4dba, {0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87}};
static const GUID IID_IDXGIAdapter = {0x2411e7e1, 0x12ac, 0x4ccf, {0xbd, 0x14, 0x97, 0x98, 0xe8, 0x53, 0x4d, 0xc0}};
static const GUID IID_IDXGIOutput1 = {0x00cddea8, 0x939b, 0x4b83, {0xa3, 0x40, 0xa6, 0x85, 0x22, 0x66, 0x66, 0xcc}};
static const GUID IID_IDXGIOutput6 = {0x068346e8, 0xaaec, 0x4b84, {0xad, 0xd7, 0x13, 0x7f, 0x51, 0x3f, 0x77, 0xa1}};

/* DXGI_MODE_DESC: medida, hercios (n/d), formato, barrido, escala */
typedef struct { unsigned w, h, hn, hd, formato, barrido, escala; } Modo;

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

static void suelta(void *o) {
    if (o)
        ((Soltar)hueco(o, 2))(o);
}

void inicio(void);

void inicio(void) {
    static unsigned char d[96], d1[152];
    static Modo lista[512];
    void *f = 0, *a = 0, *s = 0, *nada = (void *)1, *s1 = 0, *s6 = 0, *padre = 0;
    unsigned n = 0, uno = 1, i;
    int r, bien;
    unsigned short *nom = (unsigned short *)d, *nom1 = (unsigned short *)d1;
    int *rect = (int *)(d + 64);

    r = CreateDXGIFactory1(&IID_IDXGIFactory1, &f);
    mira(r == 0 && f != 0, "CreateDXGIFactory1: la fabrica");
    r = f ? ((Enum)hueco(f, 12))(f, 0, &a) : -1;
    mira(r == 0 && a != 0, "EnumAdapters1(0): el primer adaptador");
    r = a ? ((Enum)hueco(a, 7))(a, 0, &s) : -1;
    mira(r == 0 && s != 0, "EnumOutputs(0) (hueco 7): la primera salida");
    r = s ? ((Desc)hueco(s, 7))(s, d) : -1;
    mira(r == 0 && *(int *)(d + 80) != 0 && rect[2] > rect[0] && rect[3] > rect[1], "GetDesc: en el escritorio y con medida");
    mira(nom[0] == '\\' && nom[1] == '\\' && nom[2] == '.' && nom[3] == '\\' && *(void **)(d + 88) != 0, "su nombre es \\\\.\\... y tiene HMONITOR");
    r = s ? ((Modos)hueco(s, 8))(s, R8G8B8A8, 0, &n, 0) : -1;
    mira(r == 0 && n > 0 && n <= 512, "GetDisplayModeList (hueco 8): cuantos modos en R8G8B8A8");
    r = s && n <= 512 ? ((Modos)hueco(s, 8))(s, R8G8B8A8, 0, &n, lista) : -1;
    bien = r == 0 && n > 0;
    for (i = 0; bien && i < n; i++)
        bien = lista[i].w > 0 && lista[i].h > 0 && lista[i].hn > 0 && lista[i].hd > 0 && lista[i].formato == R8G8B8A8;
    mira(bien, "y la lista: medida, hercios y el formato pedido");
    r = s && n > 1 ? ((Modos)hueco(s, 8))(s, R8G8B8A8, 0, &uno, lista) : DXGI_ERROR_MORE_DATA;
    mira(r == DXGI_ERROR_MORE_DATA, "con sitio para uno solo: DXGI_ERROR_MORE_DATA");
    r = a ? ((Enum)hueco(a, 7))(a, 64, &nada) : -1;
    mira(r == DXGI_ERROR_NOT_FOUND && nada == 0, "EnumOutputs(64): NOT_FOUND y el puntero a nulo");
    r = s ? ((Pide)hueco(s, 0))(s, &IID_IDXGIOutput1, &s1) : -1;
    mira(r == 0 && s1 != 0, "es IDXGIOutput1");
    r = s ? ((Pide)hueco(s, 6))(s, &IID_IDXGIAdapter, &padre) : -1;
    mira(r == 0 && padre != 0, "GetParent: el adaptador");
    r = s ? ((Pide)hueco(s, 0))(s, &IID_IDXGIOutput6, &s6) : -1;
    mira(r == 0 && s6 != 0, "es IDXGIOutput6");
    r = s6 ? ((Desc)hueco(s6, 27))(s6, d1) : -1;
    bien = r == 0 && *(unsigned *)(d1 + 96) >= 8;
    for (i = 0; bien && i < 32; i++)
        bien = nom[i] == nom1[i];
    mira(bien, "GetDesc1 (hueco 27): el mismo nombre y 8 bits o mas");
    suelta(s6);
    suelta(s1);
    suelta(padre);
    suelta(s);
    suelta(a);
    suelta(f);
    di("tanda38.exe: IDXGIAdapter::EnumOutputs, el monitor\r\n");
    ExitProcess(fallos);
}
