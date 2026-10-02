/* tanda42.c -- el .exe de la TANDA 42 (02-10): lo que la tarjeta dice de si.
 * Por Cyberpunk: crea el dispositivo y pregunta en seguida
 * CheckFeatureSupport(FEATURE_LEVELS) y (OPTIONS), y pide IDXGIAdapter2; la
 * casa contestaba E_INVALIDARG y E_NOINTERFACE, y ademas se presentaba como
 * un adaptador por SOFTWARE sin memoria. El juego no monto lo grafico y cayo
 * con cero colas en su fin de fotograma. Aqui, como en Windows con una
 * tarjeta de verdad: el adaptador 0 es tambien IDXGIAdapter2, 3 y 4, no es
 * de software, tiene fabricante y memoria, GetDesc2 dice lo mismo que
 * GetDesc1, hay presupuesto de VRAM; el dispositivo dice un nivel de la
 * lista pedida, unas OPTIONS con sus tiers en rango (y con una medida mala,
 * E_INVALIDARG), un modelo de sombreador entre 5.1 y el pedido, que
 * R8G8B8A8 sirve de textura y de destino, su arquitectura, y el mismo LUID
 * que el adaptador. No depende de que tarjeta haya.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef long HRESULT;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
typedef struct { DWORD a; unsigned short b, c; unsigned char d[8]; } GUID;
IMPORTA HRESULT W CreateDXGIFactory1(const GUID *riid, void **pp);
IMPORTA HRESULT W D3D12CreateDevice(void *adaptador, int nivel, const GUID *riid, void **pp);

#define E_INVALIDARG ((HRESULT)0x80070057)

typedef HRESULT (W *Enum)(void *this_, unsigned i, void **pp);
typedef HRESULT (W *Pide)(void *this_, const GUID *riid, void **pp);
typedef HRESULT (W *Desc)(void *this_, void *desc);
typedef HRESULT (W *Memoria)(void *this_, unsigned nodo, unsigned grupo, U64 *info);
typedef HRESULT (W *Capacidad)(void *this_, int que, void *datos, unsigned medida);
typedef U64 *(W *Luid)(void *this_, U64 *ret);
typedef unsigned long (W *Soltar)(void *this_);

static const GUID IID_IDXGIFactory1 = {0x770aae78, 0xf26f, 0x4dba, {0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87}};
static const GUID IID_IDXGIAdapter2 = {0x0aa1ae0a, 0xfa0e, 0x4b84, {0x86, 0x44, 0xe0, 0x5f, 0xf8, 0xe5, 0xac, 0xb5}};
static const GUID IID_IDXGIAdapter3 = {0x645967a4, 0x1392, 0x4310, {0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd}};
static const GUID IID_IDXGIAdapter4 = {0x3c8d99d1, 0x4fbf, 0x4181, {0xa8, 0x2c, 0xaf, 0x66, 0xbf, 0x7b, 0xd2, 0x4e}};
static const GUID IID_ID3D12Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};

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

static DWORD d32(const unsigned char *p) { return *(const DWORD *)p; }
static U64 d64(const unsigned char *p) { return *(const U64 *)p; }

void inicio(void);

void inicio(void) {
    static unsigned char d1[312], d2[320], o[60], fl[24];
    static const int niveles[4] = {0xb000, 0xb100, 0xc000, 0xc100};
    void *f = 0, *a = 0, *a2 = 0, *a3 = 0, *a4 = 0, *dev = 0;
    U64 mem[4] = {0, 0, 0, 0}, luid = 0;
    DWORD sm[1], fs[3], ar[4];
    HRESULT r;
    int i, igual = 1;

    r = CreateDXGIFactory1(&IID_IDXGIFactory1, &f);
    r = f ? ((Enum)hueco(f, 12))(f, 0, &a) : -1;
    mira(r == 0 && a != 0, "la fabrica y el adaptador 0");
    mira(a && ((Pide)hueco(a, 0))(a, &IID_IDXGIAdapter2, &a2) == 0 && ((Pide)hueco(a, 0))(a, &IID_IDXGIAdapter3, &a3) == 0 && ((Pide)hueco(a, 0))(a, &IID_IDXGIAdapter4, &a4) == 0, "es IDXGIAdapter2, 3 y 4");
    r = a ? ((Desc)hueco(a, 10))(a, d1) : -1;
    mira(r == 0 && (d32(d1 + 304) & 2) == 0 && d32(d1 + 256) != 0 && d64(d1 + 272) > 0, "GetDesc1: no es de software, tiene fabricante y VRAM");
    r = a2 ? ((Desc)hueco(a2, 11))(a2, d2) : -1;
    for (i = 0; i < 304; i++)
        igual &= d1[i] == d2[i];
    mira(r == 0 && igual, "GetDesc2 dice lo mismo que GetDesc1");
    r = a3 ? ((Memoria)hueco(a3, 14))(a3, 0, 0, mem) : -1;
    mira(r == 0 && mem[0] > 0, "QueryVideoMemoryInfo: hay presupuesto de VRAM");

    r = D3D12CreateDevice(a, 0xb000, &IID_ID3D12Device, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice en el nivel 11_0");
    *(DWORD *)fl = 4;
    *(const int **)(fl + 8) = niveles;
    r = dev ? ((Capacidad)hueco(dev, 13))(dev, 2, fl, 24) : -1;
    mira(r == 0 && d32(fl + 16) >= 0xb000 && d32(fl + 16) <= 0xc100, "FEATURE_LEVELS: uno de la lista pedida");
    r = dev ? ((Capacidad)hueco(dev, 13))(dev, 0, o, 60) : -1;
    mira(r == 0 && d32(o + 16) >= 1 && d32(o + 16) <= 3 && d32(o + 56) >= 1 && d32(o + 56) <= 2, "OPTIONS: ResourceBinding 1..3 y ResourceHeap 1..2");
    r = dev ? ((Capacidad)hueco(dev, 13))(dev, 0, o, 59) : 0;
    mira(r == E_INVALIDARG, "OPTIONS con una medida mala: E_INVALIDARG");
    sm[0] = 0x60;
    r = dev ? ((Capacidad)hueco(dev, 13))(dev, 7, sm, 4) : -1;
    mira(r == 0 && sm[0] >= 0x51 && sm[0] <= 0x60, "SHADER_MODEL: entre 5.1 y el pedido");
    fs[0] = 28; /* DXGI_FORMAT_R8G8B8A8_UNORM */
    r = dev ? ((Capacidad)hueco(dev, 13))(dev, 3, fs, 12) : -1;
    mira(r == 0 && (fs[1] & 0x20) && (fs[1] & 0x4000), "FORMAT_SUPPORT: R8G8B8A8 es textura 2D y destino de dibujo");
    ar[0] = 0;
    r = dev ? ((Capacidad)hueco(dev, 13))(dev, 1, ar, 16) : -1;
    mira(r == 0, "ARCHITECTURE del nodo 0");
    if (dev)
        ((Luid)hueco(dev, 43))(dev, &luid);
    mira(dev && luid == d64(d1 + 296), "GetAdapterLuid: el mismo que el adaptador");

    if (dev) ((Soltar)hueco(dev, 2))(dev);
    if (a4) ((Soltar)hueco(a4, 2))(a4);
    if (a3) ((Soltar)hueco(a3, 2))(a3);
    if (a2) ((Soltar)hueco(a2, 2))(a2);
    if (a) ((Soltar)hueco(a, 2))(a);
    if (f) ((Soltar)hueco(f, 2))(f);
    di("tanda42.exe: lo que la tarjeta dice de si\r\n");
    ExitProcess(fallos);
}
