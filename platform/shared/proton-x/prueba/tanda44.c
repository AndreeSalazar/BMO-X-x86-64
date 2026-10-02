/* tanda44.c -- el .exe de la TANDA 44 (02-10): buferes grandes y montones
 * de verdad.
 * Por Cyberpunk: ya con CreateHeap, pidio un bufer de 192 MiB y el cargador
 * de PROTON-X entro en panico ("memory allocation of 201326592 bytes
 * failed"): cada bufer era un Vec del monton del cargador, que mide 48 MiB.
 * Aqui, como en Windows: un bufer UPLOAD comprometido de 192 MiB se crea, se
 * mapea y se escribe de punta a punta; en un monton UPLOAD de 256 MiB, dos
 * buferes colocados en el mismo sitio comparten la memoria (lo que uno
 * escribe, el otro lo lee) y la direccion de GPU; uno que no cabe desde su
 * desplazamiento es E_INVALIDARG, y el ultimo que cabe se escribe entero.
 * No depende de que tarjeta haya.
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

#define E_INVALIDARG ((HRESULT)0x80070057)
#define MIB ((U64)1 << 20)

typedef HRESULT (W *Comprometer)(void *this_, const void *props, DWORD banderas, const void *desc, int estado, const void *clear, const GUID *riid, void **pp);
typedef HRESULT (W *Monton)(void *this_, const void *desc, const GUID *riid, void **pp);
typedef HRESULT (W *Colocar)(void *this_, void *monton, U64 desde, const void *desc, int estado, const void *clear, const GUID *riid, void **pp);
typedef HRESULT (W *Mapear)(void *this_, unsigned sub, const void *leer, void **pp);
typedef U64 (W *DireccionGpu)(void *this_);
typedef unsigned long (W *Soltar)(void *this_);

static const GUID IID_ID3D12Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_ID3D12Heap = {0x6b3b2502, 0x6e51, 0x45b3, {0x90, 0xee, 0x98, 0x84, 0x26, 0x5e, 0x8d, 0xf3}};
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

/* D3D12_RESOURCE_DESC de un bufer: Dimension 1, Width +16, Height 1,
 * DepthOrArraySize 1, MipLevels 1, SampleDesc.Count 1, Layout ROW_MAJOR. */
static void desc_bufer(unsigned char *d, U64 ancho) {
    int i;
    for (i = 0; i < 56; i++)
        d[i] = 0;
    *(DWORD *)d = 1;
    *(U64 *)(d + 16) = ancho;
    *(DWORD *)(d + 24) = 1;
    *(unsigned short *)(d + 28) = 1;
    *(unsigned short *)(d + 30) = 1;
    *(DWORD *)(d + 36) = 1;
    *(DWORD *)(d + 44) = 1;
}

static void *colocar(void *dev, void *h, U64 desde, U64 ancho, HRESULT *r) {
    static unsigned char bd[56];
    void *b = 0;
    desc_bufer(bd, ancho);
    *r = ((Colocar)hueco(dev, 29))(dev, h, desde, bd, 0xAC3, 0, &IID_ID3D12Resource, &b);
    return b;
}

static volatile DWORD *mapear(void *b) {
    void *p = 0;
    if (!b || ((Mapear)hueco(b, 8))(b, 0, 0, &p) != 0)
        return 0;
    return (volatile DWORD *)p;
}

void inicio(void);

void inicio(void) {
    /* D3D12_HEAP_PROPERTIES: Type UPLOAD (2), nodos 1 y 1. */
    static const DWORD props[5] = {2, 0, 0, 1, 1};
    static unsigned char bd[56], hd[48];
    void *dev = 0, *grande = 0, *h = 0, *a = 0, *b = 0, *nada = 0, *fin = 0;
    volatile DWORD *p, *q;
    U64 n;
    HRESULT r;

    r = D3D12CreateDevice(0, 0xb000, &IID_ID3D12Device, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice en el nivel 11_0");

    desc_bufer(bd, 192 * MIB);
    r = dev ? ((Comprometer)hueco(dev, 27))(dev, props, 0, bd, 0xAC3, 0, &IID_ID3D12Resource, &grande) : -1;
    mira(r == 0 && grande != 0, "CreateCommittedResource: un bufer UPLOAD de 192 MiB");
    p = mapear(grande);
    n = 192 * MIB / 4;
    if (p) {
        p[0] = 0x44;
        p[n / 2] = 0x4444;
        p[n - 1] = 0x444444;
    }
    mira(p && p[0] == 0x44 && p[n / 2] == 0x4444 && p[n - 1] == 0x444444, "Map: se escribe y se lee de punta a punta");

    /* D3D12_HEAP_DESC: 256 MiB, UPLOAD, alineado 0, ALLOW_ONLY_BUFFERS. */
    *(U64 *)hd = 256 * MIB;
    *(DWORD *)(hd + 8) = 2;
    *(DWORD *)(hd + 40) = 0xC0;
    r = dev ? ((Monton)hueco(dev, 28))(dev, hd, &IID_ID3D12Heap, &h) : -1;
    mira(r == 0 && h != 0, "CreateHeap: un monton UPLOAD de 256 MiB");

    a = h ? colocar(dev, h, MIB, 0x10000, &r) : 0;
    mira(a != 0, "un bufer de 64 KiB colocado en +1 MiB");
    b = h ? colocar(dev, h, MIB, 0x10000, &r) : 0;
    mira(b != 0, "y otro en el MISMO sitio");
    p = mapear(a);
    q = mapear(b);
    if (p)
        p[7] = 0x4400ABCD;
    mira(p && q && q[7] == 0x4400ABCD, "lo que uno escribe, el otro lo lee: comparten la memoria");
    mira(a && b && ((DireccionGpu)hueco(a, 11))(a) == ((DireccionGpu)hueco(b, 11))(b) && ((DireccionGpu)hueco(a, 11))(a) != 0, "y la misma direccion de GPU");

    nada = h ? colocar(dev, h, 256 * MIB - 0x10000, 0x20000, &r) : 0;
    mira(h && r == E_INVALIDARG && nada == 0, "uno que no cabe desde su desplazamiento: E_INVALIDARG");
    fin = h ? colocar(dev, h, 256 * MIB - 0x10000, 0x10000, &r) : 0;
    p = mapear(fin);
    if (p) {
        p[0] = 0x44;
        p[0x3fff] = 0x4444;
    }
    mira(p && p[0] == 0x44 && p[0x3fff] == 0x4444, "el ultimo que cabe se escribe entero");

    if (fin) ((Soltar)hueco(fin, 2))(fin);
    if (b) ((Soltar)hueco(b, 2))(b);
    if (a) ((Soltar)hueco(a, 2))(a);
    if (h) ((Soltar)hueco(h, 2))(h);
    if (grande) ((Soltar)hueco(grande, 2))(grande);
    if (dev) ((Soltar)hueco(dev, 2))(dev);
    di("tanda44.exe: buferes grandes y montones de verdad\r\n");
    ExitProcess(fallos);
}
