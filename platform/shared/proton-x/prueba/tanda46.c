/* tanda46.c -- el .exe de la TANDA 46 (02-10): un monton sobre memoria del .exe.
 * Por Cyberpunk: tras dos VirtualAlloc pide ID3D12Device3::
 * OpenExistingHeapFromAddress (el hueco 48) y la casa no lo tenia: salia
 * con 0xC0DE0030. Aqui, como en Windows: un VirtualAlloc de 1 MiB hecho
 * entero se abre como ID3D12Heap, que mide 1 MiB y es CUSTOM; un bufer
 * colocado en el ve lo que se escribe por el puntero de VirtualAlloc, y al
 * reves; una direccion que no es la base de su region, o una region hecha a
 * medias, son E_INVALIDARG. No depende de que tarjeta haya.
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
IMPORTA void *W VirtualAlloc(void *dir, U64 n, DWORD tipo, DWORD prot);
typedef struct { DWORD a; unsigned short b, c; unsigned char d[8]; } GUID;
IMPORTA HRESULT W D3D12CreateDevice(void *adaptador, int nivel, const GUID *riid, void **pp);

#define E_INVALIDARG ((HRESULT)0x80070057)
#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define PAGE_READWRITE 4

typedef HRESULT (W *Abrir)(void *this_, const void *dir, const GUID *riid, void **pp);
typedef void *(W *DescMonton)(void *this_, void *ret);
typedef HRESULT (W *Colocar)(void *this_, void *monton, U64 desde, const void *desc, int estado, const void *clear, const GUID *riid, void **pp);
typedef HRESULT (W *Mapear)(void *this_, unsigned sub, const void *leer, void **pp);

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

/* Escribe `hr` en hexadecimal (para leerlo en Windows sin depurador). */
static void di_hr(HRESULT hr) {
    static char t[] = "HRESULT 0x00000000";
    int i;
    for (i = 0; i < 8; i++)
        t[10 + i] = "0123456789ABCDEF"[((unsigned long)hr >> (28 - 4 * i)) & 0xF];
    di(t);
}

/* Como `mira`, y si sale MAL dice tambien el HRESULT. */
static void mira_hr(int bien, const char *que, HRESULT hr) {
    mira(bien, que);
    if (!bien) {
        di("        (");
        di_hr(hr);
        di(")\r\n");
    }
}

static void *hueco(void *obj, int k) {
    return (*(void ***)obj)[k];
}

void inicio(void);

void inicio(void) {
    static unsigned char vuelta[48], bd[56];
    void *dev = 0, *h = 0, *nada = 0, *b = 0;
    volatile DWORD *v = 0, *m = 0, *medio = 0;
    HRESULT r;
    int i;

    r = D3D12CreateDevice(0, 0xb000, &IID_ID3D12Device, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice en el nivel 11_0");
    v = VirtualAlloc(0, 1 << 20, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    mira(v != 0, "VirtualAlloc de 1 MiB, hecho entero");

    r = dev && v ? ((Abrir)hueco(dev, 48))(dev, (const void *)v, &IID_ID3D12Heap, &h) : -1;
    mira(r == 0 && h != 0, "OpenExistingHeapFromAddress: un ID3D12Heap sobre esa memoria");
    if (h)
        ((DescMonton)hueco(h, 8))(h, vuelta);
    mira(h && *(U64 *)vuelta == 1 << 20 && *(DWORD *)(vuelta + 8) == 4, "GetDesc: mide la region (1 MiB) y es CUSTOM");

    /* Un bufer de 64 KiB en +64 KiB. */
    for (i = 0; i < 56; i++)
        bd[i] = 0;
    *(DWORD *)bd = 1;
    *(U64 *)(bd + 16) = 0x10000;
    *(DWORD *)(bd + 24) = 1;
    *(unsigned short *)(bd + 28) = 1;
    *(unsigned short *)(bd + 30) = 1;
    *(DWORD *)(bd + 36) = 1;
    *(DWORD *)(bd + 44) = 1;
    /* Sin ALLOW_CROSS_ADAPTER: el monton es SHARED_CROSS_ADAPTER. Lo que
     * Windows conteste se apunta (nota), no se juzga: la casa aun no lo
     * exige, hasta verlo en Windows. */
    r = h ? ((Colocar)hueco(dev, 29))(dev, h, 0x20000, bd, 0, 0, &IID_ID3D12Resource, &nada) : -1;
    di("  nota  sin ALLOW_CROSS_ADAPTER, CreatePlacedResource dice ");
    di_hr(r);
    di("\r\n");
    /* Con D3D12_RESOURCE_FLAG_ALLOW_CROSS_ADAPTER (+48 = 0x10), como pide un
     * monton SHARED_CROSS_ADAPTER (la leccion de Windows, 02-10). */
    *(DWORD *)(bd + 48) = 0x10;
    r = h ? ((Colocar)hueco(dev, 29))(dev, h, 0x10000, bd, 0, 0, &IID_ID3D12Resource, &b) : -1;
    mira_hr(r == 0 && b != 0, "CreatePlacedResource con ALLOW_CROSS_ADAPTER: un bufer de 64 KiB", r);
    if (r == 0 && b)
        r = ((Mapear)hueco(b, 8))(b, 0, 0, (void **)&m);
    mira_hr(r == 0 && m != 0, "Map del bufer colocado", r);
    if (r == 0 && m) {
        v[0x4000 + 3] = 0x4646;
        m[5] = 0x46464646;
    }
    mira(r == 0 && m && m[3] == 0x4646 && v[0x4000 + 5] == 0x46464646, "un bufer colocado en el ES esa memoria: se ven los dos");
    nada = 0;

    r = dev && v ? ((Abrir)hueco(dev, 48))(dev, (const void *)(v + 1024), &IID_ID3D12Heap, &nada) : 0;
    mira(r == E_INVALIDARG && nada == 0, "una direccion que no es la base de su region: E_INVALIDARG");
    medio = VirtualAlloc(0, 2 << 20, MEM_RESERVE, PAGE_READWRITE);
    if (medio)
        VirtualAlloc((void *)medio, 1 << 20, MEM_COMMIT, PAGE_READWRITE);
    r = dev && medio ? ((Abrir)hueco(dev, 48))(dev, (const void *)medio, &IID_ID3D12Heap, &nada) : 0;
    mira(r == E_INVALIDARG && nada == 0, "una region hecha a medias: E_INVALIDARG");

    di("tanda46.exe: un monton sobre memoria del .exe\r\n");
    ExitProcess(fallos);
}
