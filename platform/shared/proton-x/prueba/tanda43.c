/* tanda43.c -- el .exe de la TANDA 43 (02-10): montones de memoria.
 * Por Cyberpunk: con lo grafico ya montado pide ID3D12Device::CreateHeap
 * (el hueco 28) y la casa no lo tenia: salia con 0xC0DE001C. Un motor
 * grande pide montones y COLOCA los recursos dentro (CreatePlacedResource),
 * y pregunta CheckFeatureSupport OPTIONS2 a OPTIONS7. Aqui, como en Windows:
 * un monton DEFAULT de 1 MiB se crea y es un ID3D12Heap, GetDesc dice su
 * medida y su tipo, sin donde dejarlo es S_FALSE (solo se pregunta), con un
 * alineado que no existe es E_INVALIDARG; en un monton UPLOAD se coloca un
 * bufer de 64 KiB, se mapea, se escribe y se lee, y tiene direccion de GPU;
 * OPTIONS2..7 contestan con su medida y con otra, E_INVALIDARG. No depende
 * de que tarjeta haya.
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
IMPORTA HRESULT W D3D12CreateDevice(void *adaptador, int nivel, const GUID *riid, void **pp);

#define S_FALSE ((HRESULT)1)
#define E_INVALIDARG ((HRESULT)0x80070057)

typedef HRESULT (W *Pide)(void *this_, const GUID *riid, void **pp);
typedef HRESULT (W *Capacidad)(void *this_, int que, void *datos, unsigned medida);
typedef HRESULT (W *Monton)(void *this_, const void *desc, const GUID *riid, void **pp);
typedef void *(W *DescMonton)(void *this_, void *ret);
typedef HRESULT (W *Colocar)(void *this_, void *monton, U64 desde, const void *desc, int estado, const void *clear, const GUID *riid, void **pp);
typedef HRESULT (W *Mapear)(void *this_, unsigned sub, const void *leer, void **pp);
typedef U64 (W *DireccionGpu)(void *this_);
typedef unsigned long (W *Soltar)(void *this_);

static const GUID IID_ID3D12Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_ID3D12Heap = {0x6b3b2502, 0x6e51, 0x45b3, {0x90, 0xee, 0x98, 0x84, 0x26, 0x5e, 0x8d, 0xf3}};
static const GUID IID_ID3D12Pageable = {0x63ee58fb, 0x1268, 0x4835, {0x86, 0xda, 0xf0, 0x08, 0xce, 0x62, 0xf0, 0xd6}};
static const GUID IID_ID3D12Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};

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

/* D3D12_HEAP_DESC: SizeInBytes +0, Properties.Type +8, Alignment +32,
 * Flags +40 (ALLOW_ONLY_BUFFERS = 0xC0, que vale en todo tier). */
static void desc_monton(unsigned char *d, U64 medida, DWORD tipo, U64 alineado) {
    int i;
    for (i = 0; i < 48; i++)
        d[i] = 0;
    *(U64 *)d = medida;
    *(DWORD *)(d + 8) = tipo;
    *(U64 *)(d + 32) = alineado;
    *(DWORD *)(d + 40) = 0xC0;
}

void inicio(void);

void inicio(void) {
    static unsigned char hd[48], vuelta[48], bd[56], opc[24];
    static const struct { int que; unsigned medida; } opciones[6] = {{18, 8}, {21, 20}, {23, 12}, {27, 12}, {30, 20}, {32, 8}};
    void *dev = 0, *h = 0, *pg = 0, *nada = 0, *hs = 0, *b = 0;
    volatile DWORD *p = 0;
    HRESULT r;
    int i, todas = 1, malas = 1;

    r = D3D12CreateDevice(0, 0xb000, &IID_ID3D12Device, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice en el nivel 11_0");

    desc_monton(hd, 1 << 20, 1, 0);
    r = dev ? ((Monton)hueco(dev, 28))(dev, hd, &IID_ID3D12Heap, &h) : -1;
    mira(r == 0 && h != 0, "CreateHeap: un monton DEFAULT de 1 MiB");
    mira(h && ((Pide)hueco(h, 0))(h, &IID_ID3D12Pageable, &pg) == 0 && pg == h, "es tambien ID3D12Pageable");
    if (h)
        ((DescMonton)hueco(h, 8))(h, vuelta);
    mira(h && *(U64 *)vuelta == 1 << 20 && *(DWORD *)(vuelta + 8) == 1, "ID3D12Heap::GetDesc: su medida y su tipo");
    r = dev ? ((Monton)hueco(dev, 28))(dev, hd, &IID_ID3D12Heap, 0) : 0;
    mira(r == S_FALSE, "CreateHeap sin donde dejarlo: S_FALSE");
    desc_monton(hd, 1 << 20, 1, 12345);
    r = dev ? ((Monton)hueco(dev, 28))(dev, hd, &IID_ID3D12Heap, &nada) : 0;
    mira(r == E_INVALIDARG && nada == 0, "CreateHeap con un alineado que no existe: E_INVALIDARG");

    desc_monton(hd, 1 << 20, 2, 0);
    r = dev ? ((Monton)hueco(dev, 28))(dev, hd, &IID_ID3D12Heap, &hs) : -1;
    mira(r == 0 && hs != 0, "CreateHeap: un monton UPLOAD");
    /* D3D12_RESOURCE_DESC de un bufer de 64 KiB: Dimension 1, Width +16,
     * Height 1, DepthOrArraySize 1, MipLevels 1, SampleDesc.Count 1,
     * Layout ROW_MAJOR (1). */
    *(DWORD *)bd = 1;
    *(U64 *)(bd + 16) = 0x10000;
    *(DWORD *)(bd + 24) = 1;
    *(unsigned short *)(bd + 28) = 1;
    *(unsigned short *)(bd + 30) = 1;
    *(DWORD *)(bd + 36) = 1;
    *(DWORD *)(bd + 44) = 1;
    r = hs ? ((Colocar)hueco(dev, 29))(dev, hs, 0x10000, bd, 0xAC3, 0, &IID_ID3D12Resource, &b) : -1;
    mira(r == 0 && b != 0, "CreatePlacedResource: un bufer de 64 KiB en el monton");
    r = b ? ((Mapear)hueco(b, 8))(b, 0, 0, (void **)&p) : -1;
    if (r == 0 && p) {
        p[0] = 0x43;
        p[0x3fff] = 0x4343;
    }
    mira(r == 0 && p && p[0] == 0x43 && p[0x3fff] == 0x4343, "Map: se escribe y se lee de punta a punta");
    mira(b && ((DireccionGpu)hueco(b, 11))(b) != 0, "GetGPUVirtualAddress: tiene direccion");

    for (i = 0; i < 6; i++) {
        todas &= dev && ((Capacidad)hueco(dev, 13))(dev, opciones[i].que, opc, opciones[i].medida) == 0;
        malas &= dev && ((Capacidad)hueco(dev, 13))(dev, opciones[i].que, opc, opciones[i].medida + 4) == E_INVALIDARG;
    }
    mira(todas, "CheckFeatureSupport OPTIONS2 a OPTIONS7 contestan");
    mira(malas, "OPTIONS2 a OPTIONS7 con otra medida: E_INVALIDARG");

    if (b) ((Soltar)hueco(b, 2))(b);
    if (hs) ((Soltar)hueco(hs, 2))(hs);
    if (pg) ((Soltar)hueco(pg, 2))(pg);
    if (h) ((Soltar)hueco(h, 2))(h);
    if (dev) ((Soltar)hueco(dev, 2))(dev);
    di("tanda43.exe: montones de memoria\r\n");
    ExitProcess(fallos);
}
