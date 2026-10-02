/* tanda47.c -- el .exe de la TANDA 47 (02-10): lo que vkd3d-proton tiene y
 * la casa no tenia.
 * Por el inventario de la casa contra vkd3d-proton (en vez de un muro por
 * cada vez que se corre Cyberpunk): lo que un motor llama al montar y en
 * cada fotograma. Aqui, como en Windows: SetName en el dispositivo, la cola y
 * un bufer, y GetPrivateData lo devuelve; GetDevice de la cola y de la valla
 * dan el dispositivo; GetParent del adaptador da la fabrica; IsCurrent;
 * EnumAdapterByLuid con el LUID del dispositivo; CheckInterfaceSupport
 * (IDXGIDevice) da la version del driver; GetDesc de la cola y del monton de
 * descriptores; un CBV copiado con CopyDescriptorsSimple; GetCustomHeap
 * Properties de UPLOAD; GetHeapProperties de un bufer READBACK; GetType de
 * la lista; y una lista que copia con CopyBufferRegion y CopyResource y pone
 * dos sellos de tiempo (EndQuery) resueltos a un bufer (ResolveQueryData),
 * leido tras la valla. No depende de que tarjeta haya.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned UINT;
typedef long HRESULT;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
typedef struct { DWORD a; unsigned short b, c; unsigned char d[8]; } GUID;
IMPORTA HRESULT W D3D12CreateDevice(void *adaptador, int nivel, const GUID *riid, void **pp);
IMPORTA HRESULT W CreateDXGIFactory2(UINT banderas, const GUID *riid, void **pp);

static const GUID IID_Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_Queue = {0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID IID_Allocator = {0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID IID_List = {0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID IID_DescHeap = {0x8efb471d, 0x616c, 0x4f49, {0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51}};
static const GUID IID_Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID IID_Fence = {0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID IID_QueryHeap = {0x0d9658ae, 0xed45, 0x469e, {0xa6, 0x1d, 0x97, 0x0e, 0xc5, 0x83, 0xca, 0xb4}};
static const GUID IID_Factory4 = {0x1bc6ea02, 0xef36, 0x464f, {0xbf, 0x0c, 0x21, 0xca, 0x39, 0xe5, 0x16, 0x8a}};
static const GUID IID_Factory = {0x7b7166ec, 0x21c7, 0x44ae, {0xb2, 0x1a, 0xc9, 0xae, 0x32, 0x1a, 0xe3, 0x69}};
static const GUID IID_Adapter = {0x2411e7e1, 0x12ac, 0x4ccf, {0xbd, 0x14, 0x97, 0x98, 0xe8, 0x53, 0x4d, 0xc0}};
static const GUID IID_DXGIDevice = {0x54ec77fa, 0x1377, 0x44e6, {0x8c, 0x32, 0x88, 0xfd, 0x5f, 0x44, 0xc8, 0x4c}};
static const GUID NOMBRE = {0x4cca5fd8, 0x921f, 0x42c8, {0x85, 0x66, 0x70, 0xca, 0xf2, 0xa9, 0xc6, 0xe5}};

typedef HRESULT (W *Pide)(void *t, const GUID *riid, void **pp);
typedef unsigned long (W *Soltar)(void *t);
typedef HRESULT (W *Nombrar)(void *t, const unsigned short *n);
typedef HRESULT (W *Privado)(void *t, const GUID *g, UINT *n, void *d);
typedef HRESULT (W *Crear3)(void *t, const void *d, const GUID *riid, void **pp);
typedef HRESULT (W *CrearAsig)(void *t, int tipo, const GUID *riid, void **pp);
typedef HRESULT (W *CrearLista)(void *t, UINT m, int tipo, void *a, void *pso, const GUID *riid, void **pp);
typedef HRESULT (W *Comprometer)(void *t, const void *props, DWORD f, const void *d, int e, const void *c, const GUID *riid, void **pp);
typedef HRESULT (W *CrearValla)(void *t, U64 v, int f, const GUID *riid, void **pp);
typedef void *(W *Oculto)(void *t, void *ret);
typedef void *(W *Oculto2)(void *t, void *ret, UINT a, UINT b);
typedef void (W *VistaCbv)(void *t, const void *d, U64 h);
typedef void (W *CopiaSimple)(void *t, UINT n, U64 dst, U64 src, int tipo);
typedef HRESULT (W *Mapear)(void *t, UINT sub, const void *r, void **pp);
typedef U64 (W *Direccion)(void *t);
typedef HRESULT (W *Props)(void *t, void *p, DWORD *f);
typedef int (W *Tipo)(void *t);
typedef void (W *CopiaBufer)(void *t, void *d, U64 dd, void *s, U64 sd, U64 n);
typedef void (W *CopiaRecurso)(void *t, void *d, void *s);
typedef void (W *Consulta)(void *t, void *h, int tipo, UINT i);
typedef void (W *Resolver)(void *t, void *h, int tipo, UINT a, UINT n, void *b, U64 off);
typedef HRESULT (W *Cerrar)(void *t);
typedef void (W *Ejecutar)(void *t, UINT n, void *const *l);
typedef HRESULT (W *Signal)(void *t, void *v, U64 x);
typedef HRESULT (W *AlLlegar)(void *t, U64 x, HANDLE e);
typedef U64 (W *Completado)(void *t);
typedef HRESULT (W *Frecuencia)(void *t, U64 *f);
typedef int (W *Booleano)(void *t);
typedef HRESULT (W *PorLuid)(void *t, U64 luid, const GUID *riid, void **pp);
typedef HRESULT (W *Enumerar)(void *t, UINT i, void **pp);
typedef HRESULT (W *Interfaz)(void *t, const GUID *g, U64 *v);

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

static const unsigned short NOMBRE_BUFER[] = {'t', 'a', 'n', 'd', 'a', '4', '7', 0};

/* El nombre de `o` es "tanda47"? */
static int se_llama(void *o) {
    static unsigned short d[32];
    UINT n = sizeof d, i;
    if (((Privado)hueco(o, 3))(o, &NOMBRE, &n, d) != 0 || n < 14)
        return 0;
    for (i = 0; i < 7; i++)
        if (d[i] != NOMBRE_BUFER[i])
            return 0;
    return 1;
}

/* Un bufer comprometido de `n` bytes en el monton `tipo`. */
static void *bufer(void *dev, int tipo, U64 n, int estado) {
    static unsigned char d[56];
    DWORD props[5];
    void *b = 0;
    int i;
    props[0] = tipo;
    props[1] = props[2] = 0;
    props[3] = props[4] = 1;
    for (i = 0; i < 56; i++)
        d[i] = 0;
    *(DWORD *)d = 1;
    *(U64 *)(d + 16) = n;
    *(DWORD *)(d + 24) = 1;
    *(unsigned short *)(d + 28) = 1;
    *(unsigned short *)(d + 30) = 1;
    *(DWORD *)(d + 36) = 1;
    *(DWORD *)(d + 44) = 1;
    if (((Comprometer)hueco(dev, 27))(dev, props, 0, d, estado, 0, &IID_Resource, &b) != 0)
        return 0;
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
    static DWORD cola_desc[4] = {0, 0, 0, 0}, monton_desc[4] = {0, 4, 0, 0}, consultas_desc[3] = {1, 2, 0};
    static DWORD ret[8], props[5], vuelta[4];
    static U64 cbv[2], luid;
    void *dev = 0, *cola = 0, *otro = 0, *f = 0, *a = 0, *a2 = 0, *padre = 0, *mon = 0, *asig = 0, *lista = 0;
    void *sube = 0, *lee = 0, *lee2 = 0, *cons = 0, *valla = 0;
    volatile DWORD *p, *q;
    volatile U64 *t;
    DWORD flags = 7;
    U64 freq = 0, ver = 0, base;
    HRESULT r;
    int i, bien;

    r = D3D12CreateDevice(0, 0xb000, &IID_Device, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice en el nivel 11_0");
    r = dev ? ((Crear3)hueco(dev, 8))(dev, cola_desc, &IID_Queue, &cola) : -1;
    sube = dev ? bufer(dev, 2, 256, 0xAC3) : 0;
    lee = dev ? bufer(dev, 3, 256, 0x400) : 0;
    lee2 = dev ? bufer(dev, 3, 256, 0x400) : 0;
    mira(r == 0 && cola && sube && lee && lee2, "una cola y tres buferes (UPLOAD y dos READBACK)");

    bien = dev && cola && sube;
    if (bien) {
        ((Nombrar)hueco(dev, 6))(dev, NOMBRE_BUFER);
        ((Nombrar)hueco(cola, 6))(cola, NOMBRE_BUFER);
        ((Nombrar)hueco(sube, 6))(sube, NOMBRE_BUFER);
    }
    mira(bien && se_llama(dev) && se_llama(cola) && se_llama(sube), "SetName en el dispositivo, la cola y un bufer; GetPrivateData lo devuelve");
    r = cola ? ((Pide)hueco(cola, 7))(cola, &IID_Device, &otro) : -1;
    mira(r == 0 && otro == dev, "GetDevice de la cola: el dispositivo");
    r = dev ? ((CrearValla)hueco(dev, 36))(dev, 0, 0, &IID_Fence, &valla) : -1;
    otro = 0;
    if (r == 0)
        r = ((Pide)hueco(valla, 7))(valla, &IID_Device, &otro);
    mira(r == 0 && otro == dev, "GetDevice de la valla: el dispositivo");

    r = CreateDXGIFactory2(0, &IID_Factory4, &f);
    if (r == 0)
        r = ((Enumerar)hueco(f, 12))(f, 0, &a);
    if (r == 0)
        r = ((Pide)hueco(a, 6))(a, &IID_Factory, &padre);
    mira(r == 0 && padre != 0, "GetParent del adaptador: la fabrica");
    mira(f && ((Booleano)hueco(f, 13))(f) == 1, "IsCurrent: la fabrica esta al dia");
    if (dev)
        ((Oculto)hueco(dev, 43))(dev, &luid);
    r = f ? ((PorLuid)hueco(f, 26))(f, luid, &IID_Adapter, &a2) : -1;
    mira(r == 0 && a2 != 0, "EnumAdapterByLuid con el LUID del dispositivo");
    r = a ? ((Interfaz)hueco(a, 9))(a, &IID_DXGIDevice, &ver) : -1;
    mira(r == 0 && ver != 0, "CheckInterfaceSupport(IDXGIDevice): la version del driver");

    if (cola)
        ((Oculto)hueco(cola, 18))(cola, vuelta);
    mira(cola && vuelta[0] == 0, "GetDesc de la cola: DIRECT");
    r = dev ? ((Crear3)hueco(dev, 14))(dev, monton_desc, &IID_DescHeap, &mon) : -1;
    if (r == 0)
        ((Oculto)hueco(mon, 8))(mon, ret);
    mira(r == 0 && ret[0] == 0 && ret[1] == 4, "GetDesc del monton de descriptores: CBV_SRV_UAV, 4");
    if (mon) {
        ((Oculto)hueco(mon, 9))(mon, &base);
        cbv[0] = sube ? ((Direccion)hueco(sube, 11))(sube) : 0;
        cbv[1] = 256;
        ((VistaCbv)hueco(dev, 17))(dev, cbv, base);
        ((CopiaSimple)hueco(dev, 24))(dev, 1, base + 2 * ((UINT(W *)(void *, int))hueco(dev, 15))(dev, 0), base, 0);
    }
    mira(mon != 0, "CreateConstantBufferView y CopyDescriptorsSimple");

    if (dev)
        ((Oculto2)hueco(dev, 26))(dev, props, 0, 2);
    mira(dev && props[0] == 4 && props[1] == 2 && props[2] == 1, "GetCustomHeapProperties(UPLOAD): CUSTOM, WRITE_COMBINE, L0");
    r = lee ? ((Props)hueco(lee, 14))(lee, props, &flags) : -1;
    mira(r == 0 && props[0] == 3, "GetHeapProperties de un bufer READBACK: READBACK");

    r = dev ? ((CrearAsig)hueco(dev, 9))(dev, 0, &IID_Allocator, &asig) : -1;
    if (r == 0)
        r = ((CrearLista)hueco(dev, 12))(dev, 0, 0, asig, 0, &IID_List, &lista);
    mira(r == 0 && ((Tipo)hueco(lista, 8))(lista) == 0, "GetType de la lista: DIRECT");
    r = dev ? ((Crear3)hueco(dev, 39))(dev, consultas_desc, &IID_QueryHeap, &cons) : -1;
    mira(r == 0 && cons != 0, "CreateQueryHeap: dos sellos de tiempo");

    p = mapear(sube);
    if (p)
        for (i = 0; i < 64; i++)
            p[i] = 0x47000000u + i;
    r = (lista && cons && p && lee && lee2 && cola && valla) ? 0 : -1;
    if (r == 0) {
        ((Consulta)hueco(lista, 53))(lista, cons, 2, 0);
        ((CopiaBufer)hueco(lista, 15))(lista, lee, 0, sube, 16, 128);
        ((CopiaRecurso)hueco(lista, 17))(lista, lee2, sube);
        ((Consulta)hueco(lista, 53))(lista, cons, 2, 1);
        ((Resolver)hueco(lista, 54))(lista, cons, 2, 0, 2, lee, 128);
        r = ((Cerrar)hueco(lista, 9))(lista);
    }
    if (r == 0) {
        ((Ejecutar)hueco(cola, 10))(cola, 1, &lista);
        r = ((Signal)hueco(cola, 14))(cola, valla, 1);
    }
    if (r == 0)
        r = ((AlLlegar)hueco(valla, 9))(valla, 1, 0);
    q = mapear(lee);
    mira(r == 0 && q && q[0] == 0x47000004u && q[31] == 0x47000023u, "CopyBufferRegion: 128 bytes desde +16, tras la valla");
    p = mapear(lee2);
    mira(p && p[0] == 0x47000000u && p[63] == 0x4700003Fu, "CopyResource: el bufer entero");
    t = q ? (volatile U64 *)(q + 32) : 0;
    r = cola ? ((Frecuencia)hueco(cola, 16))(cola, &freq) : -1;
    mira(r == 0 && freq != 0 && t && t[0] != 0 && t[1] >= t[0], "EndQuery y ResolveQueryData: dos sellos de tiempo, en orden");

    di("tanda47.exe: lo que vkd3d-proton tiene y la casa no tenia\r\n");
    ExitProcess(fallos);
}
