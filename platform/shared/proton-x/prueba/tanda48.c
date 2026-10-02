/* tanda48.c -- el .exe de la TANDA 48 (02-10): la base de PROTON-X, llena.
 * Lo que la investigacion de como Proton domo a Cyberpunk (vkd3d-proton)
 * dejo en codigo: cada hueco de las 17 interfaces es algo que la casa hace o
 * una FALLA DOCUMENTADA (el HRESULT de Windows sin esa funcion, el puntero de
 * salida a NULL). Aqui, como en Windows: OpenExistingHeapFromFileMapping sin
 * fichero falla y deja NULL; GetResourceAllocationInfo crece con las mips y
 * con el formato (BC1 menos que RGBA8); una textura de 4 MiB no cabe en un
 * monton de 1 MiB (E_INVALIDARG) y una de 256 KiB si, y no entra en uno
 * que solo admite destinos (ALLOW_ONLY_RT_DS_TEXTURES); la lista es tambien
 * GraphicsCommandList2 y 4: WriteBufferImmediate escribe sus valores y
 * BeginRenderPass con CLEAR limpia el destino de rojo (leido con
 * CopyTextureRegion tras la valla); GetCreationFlags de la valla (Fence1);
 * GetDesc1 de un bufer (Resource2); y Register/UnregisterAdaptersChangedEvent
 * (Factory7). No depende de que tarjeta haya.
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
IMPORTA HANDLE W CreateEventW(void *atributos, int manual, int inicial, const unsigned short *nombre);
IMPORTA void W ExitProcess(unsigned c);
typedef struct { DWORD a; unsigned short b, c; unsigned char d[8]; } GUID;
IMPORTA HRESULT W D3D12CreateDevice(void *adaptador, int nivel, const GUID *riid, void **pp);
IMPORTA HRESULT W CreateDXGIFactory2(UINT banderas, const GUID *riid, void **pp);

static const GUID IID_Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_Device3 = {0x81dadc15, 0x2bad, 0x4392, {0x93, 0xc5, 0x10, 0x13, 0x45, 0xc4, 0xaa, 0x98}};
static const GUID IID_Queue = {0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID IID_Allocator = {0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID IID_List = {0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID IID_List2 = {0x38c3e585, 0xff17, 0x412c, {0x91, 0x50, 0x4f, 0xc6, 0xf9, 0xd7, 0x2a, 0x28}};
static const GUID IID_List4 = {0x8754318e, 0xd3a9, 0x4541, {0x98, 0xcf, 0x64, 0x5b, 0x50, 0xdc, 0x48, 0x74}};
static const GUID IID_DescHeap = {0x8efb471d, 0x616c, 0x4f49, {0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51}};
static const GUID IID_Heap = {0x6b3b2502, 0x6e51, 0x45b3, {0x90, 0xee, 0x98, 0x84, 0x26, 0x5e, 0x8d, 0xf3}};
static const GUID IID_Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID IID_Resource2 = {0xbe36ec3b, 0xea85, 0x4aeb, {0xa4, 0x5a, 0xe9, 0xd7, 0x64, 0x04, 0xa4, 0x95}};
static const GUID IID_Fence = {0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID IID_Fence1 = {0x433685fe, 0xe22b, 0x4ca0, {0xa8, 0xdb, 0xb5, 0xb4, 0xf4, 0xdd, 0x0e, 0x4a}};
static const GUID IID_Factory7 = {0xa4966eed, 0x76db, 0x44da, {0x84, 0xc1, 0xee, 0x9a, 0x7a, 0xfb, 0x20, 0xa8}};

typedef HRESULT (W *Pide)(void *t, const GUID *riid, void **pp);
typedef HRESULT (W *Crear3)(void *t, const void *d, const GUID *riid, void **pp);
typedef HRESULT (W *CrearAsig)(void *t, int tipo, const GUID *riid, void **pp);
typedef HRESULT (W *CrearLista)(void *t, UINT m, int tipo, void *a, void *pso, const GUID *riid, void **pp);
typedef HRESULT (W *Comprometer)(void *t, const void *props, DWORD f, const void *d, int e, const void *c, const GUID *riid, void **pp);
typedef HRESULT (W *Colocar)(void *t, void *h, U64 desde, const void *d, int e, const void *c, const GUID *riid, void **pp);
typedef HRESULT (W *DeFichero)(void *t, HANDLE h, const GUID *riid, void **pp);
typedef HRESULT (W *CrearValla)(void *t, U64 v, int f, const GUID *riid, void **pp);
typedef U64 *(W *Asignacion)(void *t, U64 *ret, UINT mascara, UINT n, const void *d);
typedef void *(W *Oculto)(void *t, void *ret);
typedef void (W *Vista)(void *t, void *r, const void *d, U64 h);
typedef HRESULT (W *Mapear)(void *t, UINT sub, const void *r, void **pp);
typedef U64 (W *Direccion)(void *t);
typedef void (W *Inmediato)(void *t, UINT n, const void *p, const int *modos);
typedef void (W *Pase)(void *t, UINT n, const void *destinos, const void *profundidad, int banderas);
typedef void (W *Fin)(void *t);
typedef void (W *Barrera)(void *t, UINT n, const void *b);
typedef void (W *CopiaTextura)(void *t, const void *d, UINT x, UINT y, UINT z, const void *s, const void *caja);
typedef HRESULT (W *Cerrar)(void *t);
typedef void (W *Ejecutar)(void *t, UINT n, void *const *l);
typedef HRESULT (W *Signal)(void *t, void *v, U64 x);
typedef HRESULT (W *AlLlegar)(void *t, U64 x, HANDLE e);
typedef int (W *Banderas)(void *t);
typedef HRESULT (W *Registrar)(void *t, HANDLE e, DWORD *cookie);
typedef HRESULT (W *Quitar)(void *t, DWORD cookie);

#define E_INVALIDARG ((HRESULT)0x80070057)

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

/* Como `mira`, y si sale MAL dice tambien el HRESULT (para leerlo en
 * Windows sin depurador). */
static void mira_hr(int bien, const char *que, HRESULT hr) {
    static char t[] = " (HRESULT 0x00000000)";
    int i;
    mira(bien, que);
    if (bien)
        return;
    for (i = 0; i < 8; i++)
        t[11 + i] = "0123456789ABCDEF"[((unsigned long)hr >> (28 - 4 * i)) & 0xF];
    di("        ");
    di(t + 1);
    di("\r\n");
}

static void *hueco(void *obj, int k) {
    return (*(void ***)obj)[k];
}

static void ceros(void *p, int n) {
    int i;
    for (i = 0; i < n; i++)
        ((unsigned char *)p)[i] = 0;
}

/* Un D3D12_RESOURCE_DESC de textura 2D: Dimension 3, Width +16, Height
 * +24, DepthOrArraySize +28, MipLevels +30, Format +32, SampleDesc.Count
 * +36, Flags +48. */
static void textura(unsigned char *d, U64 lado, unsigned short mips, DWORD formato, DWORD banderas) {
    ceros(d, 56);
    *(DWORD *)d = 3;
    *(U64 *)(d + 16) = lado;
    *(DWORD *)(d + 24) = (DWORD)lado;
    *(unsigned short *)(d + 28) = 1;
    *(unsigned short *)(d + 30) = mips;
    *(DWORD *)(d + 32) = formato;
    *(DWORD *)(d + 36) = 1;
    *(DWORD *)(d + 48) = banderas;
}

/* Lo que mide segun GetResourceAllocationInfo. */
static U64 mide(void *dev, U64 lado, unsigned short mips, DWORD formato) {
    static unsigned char d[56];
    U64 ret[2] = {0, 0};
    textura(d, lado, mips, formato, 0);
    ((Asignacion)hueco(dev, 25))(dev, ret, 0, 1, d);
    return ret[0];
}

/* Un bufer comprometido de `n` bytes en el monton `tipo`. */
static void *bufer(void *dev, int tipo, U64 n, int estado) {
    static unsigned char d[56];
    DWORD props[5];
    void *b = 0;
    props[0] = tipo;
    props[1] = props[2] = 0;
    props[3] = props[4] = 1;
    ceros(d, 56);
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
    static DWORD cola_desc[4] = {0, 0, 0, 0}, rtv_desc[4] = {2, 1, 0, 0};
    static DWORD props[5] = {1, 0, 0, 1, 1};
    /* D3D12_HEAP_DESC: 1 MiB, DEFAULT, ALLOW_ONLY_NON_RT_DS_TEXTURES (0x44). */
    static unsigned char hd[48], td[56], d1[64];
    /* D3D12_WRITEBUFFERIMMEDIATE_PARAMETER: Dest, Value (16 B cada uno). */
    static U64 inmediatos[4];
    /* D3D12_RENDER_PASS_RENDER_TARGET_DESC (88 B): el descriptor, el acceso
     * de entrada CLEAR (2) con R8G8B8A8_UNORM (28) y rojo {1, 0, 0, 1} en
     * sus bits (sin flotantes en el .exe), y el de salida PRESERVE (1). */
    static DWORD pase[22];
    /* D3D12_RESOURCE_BARRIER de RENDER_TARGET a COPY_SOURCE (32 B). */
    static DWORD barrera[8];
    /* D3D12_TEXTURE_COPY_LOCATION (48 B): el destino, una huella de 4x4
     * RGBA8 con filas de 256 bytes; el origen, el subrecurso 0. */
    static DWORD destino[12], origen[12];
    void *dev = 0, *dev3 = 0, *nada = 0, *cola = 0, *asig = 0, *lista = 0, *l2 = 0, *l4 = 0;
    void *monton = 0, *otro = 0, *cabe = 0, *grande = 0, *rt = 0, *rtvs = 0, *lee = 0, *lee_rt = 0;
    void *valla = 0, *valla1 = 0, *r2 = 0, *f = 0;
    volatile DWORD *p, *q;
    U64 una, dos, todas, bc1, rtv = 0;
    DWORD cookie = 0;
    HANDLE evento;
    HRESULT r;

    r = D3D12CreateDevice(0, 0xb000, &IID_Device, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice en el nivel 11_0");

    /* La falla documentada: sin fichero no hay monton, y pp queda NULL. */
    r = dev ? ((Pide)hueco(dev, 0))(dev, &IID_Device3, &dev3) : -1;
    if (r == 0)
        r = ((DeFichero)hueco(dev3, 49))(dev3, 0, &IID_Heap, &nada);
    mira(dev3 && r < 0 && nada == 0, "OpenExistingHeapFromFileMapping sin fichero: falla y deja NULL");

    una = dev ? mide(dev, 1024, 1, 28) : 0;
    dos = dev ? mide(dev, 1024, 2, 28) : 0;
    todas = dev ? mide(dev, 1024, 0, 28) : 0;
    mira(una >= 4 << 20 && una < dos && dos <= todas, "GetResourceAllocationInfo crece con las mips");
    bc1 = dev ? mide(dev, 1024, 1, 71) : 0;
    mira(bc1 != 0 && bc1 < una, "GetResourceAllocationInfo: BC1 mide menos que RGBA8");

    *(U64 *)hd = 1 << 20;
    *(DWORD *)(hd + 8) = 1;
    *(DWORD *)(hd + 40) = 0x44;
    r = dev ? ((Crear3)hueco(dev, 28))(dev, hd, &IID_Heap, &monton) : -1;
    textura(td, 256, 1, 28, 0);
    if (r == 0)
        r = ((Colocar)hueco(dev, 29))(dev, monton, 0, td, 0x400, 0, &IID_Resource, &cabe);
    mira_hr(r == 0 && cabe != 0, "CreatePlacedResource: 256 KiB caben en un monton de 1 MiB", r);
    textura(td, 1024, 1, 28, 0);
    r = monton ? ((Colocar)hueco(dev, 29))(dev, monton, 0, td, 0x400, 0, &IID_Resource, &grande) : 0;
    mira_hr(r == E_INVALIDARG && grande == 0, "CreatePlacedResource: 4 MiB no caben en 1 MiB (E_INVALIDARG)", r);
    /* La leccion de Windows (02-10): 0x84 es ALLOW_ONLY_RT_DS_TEXTURES, y una
     * textura sin RENDER_TARGET no entra aunque quepa. */
    *(DWORD *)(hd + 40) = 0x84;
    otro = 0;
    r = dev ? ((Crear3)hueco(dev, 28))(dev, hd, &IID_Heap, &otro) : -1;
    textura(td, 256, 1, 28, 0);
    grande = 0;
    if (r == 0)
        r = ((Colocar)hueco(dev, 29))(dev, otro, 0, td, 0x400, 0, &IID_Resource, &grande);
    mira_hr(r == E_INVALIDARG && grande == 0, "una textura sin RT en un monton ALLOW_ONLY_RT_DS_TEXTURES: E_INVALIDARG", r);

    r = dev ? ((Crear3)hueco(dev, 8))(dev, cola_desc, &IID_Queue, &cola) : -1;
    if (r == 0)
        r = ((CrearAsig)hueco(dev, 9))(dev, 0, &IID_Allocator, &asig);
    if (r == 0)
        r = ((CrearLista)hueco(dev, 12))(dev, 0, 0, asig, 0, &IID_List, &lista);
    if (r == 0)
        r = ((Pide)hueco(lista, 0))(lista, &IID_List2, &l2);
    if (r == 0)
        r = ((Pide)hueco(lista, 0))(lista, &IID_List4, &l4);
    mira(r == 0 && l2 && l4, "la lista es tambien GraphicsCommandList2 y 4");

    lee = dev ? bufer(dev, 3, 256, 0x400) : 0;
    lee_rt = dev ? bufer(dev, 3, 1024, 0x400) : 0;
    textura(td, 4, 1, 28, 1);
    r = dev ? ((Comprometer)hueco(dev, 27))(dev, props, 0, td, 4, 0, &IID_Resource, &rt) : -1;
    if (r == 0)
        r = ((Crear3)hueco(dev, 14))(dev, rtv_desc, &IID_DescHeap, &rtvs);
    if (r == 0) {
        ((Oculto)hueco(rtvs, 9))(rtvs, &rtv);
        ((Vista)hueco(dev, 20))(dev, rt, 0, rtv);
    }
    r = dev ? ((CrearValla)hueco(dev, 36))(dev, 0, 0, &IID_Fence, &valla) : -1;
    mira(r == 0 && lee && lee_rt && rt && rtvs && valla, "dos buferes READBACK, un destino de 4x4 y su RTV");

    r = (l2 && l4 && lee && lee_rt && rt && valla) ? 0 : -1;
    if (r == 0) {
        inmediatos[0] = ((Direccion)hueco(lee, 11))(lee) + 4;
        inmediatos[1] = 0x48000001u;
        inmediatos[2] = inmediatos[0] + 12;
        inmediatos[3] = 0x48000002u;
        ((Inmediato)hueco(l2, 66))(l2, 2, inmediatos, 0);

        *(U64 *)pase = rtv;
        pase[2] = 2;
        pase[3] = 28;
        pase[4] = 0x3F800000u;
        pase[5] = 0;
        pase[6] = 0;
        pase[7] = 0x3F800000u;
        pase[8] = 1;
        ((Pase)hueco(l4, 68))(l4, 1, pase, 0, 0);
        ((Fin)hueco(l4, 69))(l4);

        *(void **)(barrera + 2) = rt;
        barrera[4] = 0xFFFFFFFF;
        barrera[5] = 4;
        barrera[6] = 0x800;
        ((Barrera)hueco(lista, 26))(lista, 1, barrera);
        *(void **)destino = lee_rt;
        destino[2] = 1;
        destino[6] = 28;
        destino[7] = destino[8] = 4;
        destino[9] = 1;
        destino[10] = 256;
        *(void **)origen = rt;
        ((CopiaTextura)hueco(lista, 16))(lista, destino, 0, 0, 0, origen, 0);
        r = ((Cerrar)hueco(lista, 9))(lista);
    }
    if (r == 0) {
        ((Ejecutar)hueco(cola, 10))(cola, 1, &lista);
        r = ((Signal)hueco(cola, 14))(cola, valla, 1);
    }
    if (r == 0)
        r = ((AlLlegar)hueco(valla, 9))(valla, 1, 0);
    p = r == 0 ? mapear(lee) : 0;
    mira(p && p[1] == 0x48000001u && p[4] == 0x48000002u && p[2] == 0, "WriteBufferImmediate: sus dos valores, tras la valla");
    q = r == 0 ? mapear(lee_rt) : 0;
    mira(q && q[0] == 0xFF0000FFu && q[3] == 0xFF0000FFu && q[64 * 3 + 3] == 0xFF0000FFu, "BeginRenderPass con CLEAR: el destino, rojo");

    r = valla ? ((Pide)hueco(valla, 0))(valla, &IID_Fence1, &valla1) : -1;
    mira(r == 0 && ((Banderas)hueco(valla1, 11))(valla1) == 0, "Fence1::GetCreationFlags: NONE");
    r = lee ? ((Pide)hueco(lee, 0))(lee, &IID_Resource2, &r2) : -1;
    if (r == 0)
        ((Oculto)hueco(r2, 16))(r2, d1);
    mira(r == 0 && *(DWORD *)d1 == 1 && *(U64 *)(d1 + 16) == 256, "Resource2::GetDesc1: un bufer de 256 bytes");

    evento = CreateEventW(0, 0, 0, 0);
    r = CreateDXGIFactory2(0, &IID_Factory7, &f);
    if (r == 0)
        r = ((Registrar)hueco(f, 30))(f, evento, &cookie);
    if (r == 0)
        r = ((Quitar)hueco(f, 31))(f, cookie);
    mira(r == 0 && evento != 0, "Factory7: Register y UnregisterAdaptersChangedEvent");

    di("tanda48.exe: la base de PROTON-X, llena\r\n");
    ExitProcess(fallos);
}
