/* tanda45.c -- el .exe de la TANDA 45 (02-10): ID3D12Device1 a 10.
 * Por Cyberpunk: con el dispositivo hecho pide ID3D12Device1, 4, 8 y 10
 * (QueryInterface), y la casa decia que no. Aqui, como en un Windows 11 con
 * ID3D12Device10: D3D12CreateDevice la da directamente, y la 1 a la 9 son
 * el MISMO objeto; un bufer y una textura de 16 MiB con
 * CreateCommittedResource1; un monton con
 * CreateHeap1 donde CreatePlacedResource1 y 2 colocan dos buferes en el
 * mismo sitio (y se ven); GetResourceAllocationInfo1 y 2 con dos buferes
 * (cada uno a 64 KiB, uno detras de otro); CreateCommandQueue1, y
 * CreateCommandList1, que nace CERRADA (Reset y Close: S_OK); un PSO por
 * CreatePipelineState, con un FLUJO de subobjetos (el HLSL de cubo12);
 * EnqueueMakeResident lleva la valla a su valor; SetBackgroundProcessingMode
 * y GetDeviceRemovedReason, S_OK; y CreatePipelineLibrary da una libreria o
 * DXGI_ERROR_UNSUPPORTED (las dos cosas son de Windows: depende del driver).
 * No depende de que tarjeta haya.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
#include "cubo12_hlsl.h"

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
IMPORTA HRESULT W D3D12SerializeRootSignature(const void *desc, int version, void **blob, void **error);
IMPORTA HRESULT W D3DCompile(const void *src, U64 n, const char *nombre, const void *macros, void *include, const char *entrada,
                             const char *perfil, UINT f1, UINT f2, void **codigo, void **errores);

#define DXGI_ERROR_UNSUPPORTED ((HRESULT)0x887A0004)
#define LAYOUT_UNDEFINED 0xFFFFFFFFu

typedef HRESULT (W *Pide)(void *this_, const GUID *riid, void **pp);
typedef HRESULT (W *Comprometer1)(void *this_, const void *props, DWORD banderas, const void *desc, int estado, const void *clear, void *sesion, const GUID *riid, void **pp);
typedef HRESULT (W *Monton1)(void *this_, const void *desc, void *sesion, const GUID *riid, void **pp);
typedef HRESULT (W *Colocar1)(void *this_, void *monton, U64 desde, const void *desc, int estado, const void *clear, const GUID *riid, void **pp);
typedef HRESULT (W *Colocar2)(void *this_, void *monton, U64 desde, const void *desc, UINT layout, const void *clear, UINT n, const UINT *formatos, const GUID *riid, void **pp);
typedef U64 *(W *Asignacion1)(void *this_, U64 *ret, UINT mascara, UINT n, const void *descs, U64 *info1);
typedef HRESULT (W *Cola1)(void *this_, const void *desc, const GUID *creador, const GUID *riid, void **pp);
typedef HRESULT (W *Lista1)(void *this_, UINT mascara, int tipo, int banderas, const GUID *riid, void **pp);
typedef HRESULT (W *Asignador)(void *this_, int tipo, const GUID *riid, void **pp);
typedef HRESULT (W *Reiniciar)(void *this_, void *asignador, void *pso);
typedef HRESULT (W *Cerrar)(void *this_);
typedef HRESULT (W *Raiz)(void *this_, UINT nodo, const void *blob, U64 n, const GUID *riid, void **pp);
typedef HRESULT (W *Flujo)(void *this_, const void *desc, const GUID *riid, void **pp);
typedef HRESULT (W *Valla)(void *this_, U64 inicial, int banderas, const GUID *riid, void **pp);
typedef HRESULT (W *Residente)(void *this_, int banderas, UINT n, void *const *objetos, void *valla, U64 valor);
typedef U64 (W *Completado)(void *this_);
typedef HRESULT (W *AlLlegar)(void *this_, U64 valor, HANDLE evento);
typedef HRESULT (W *Fondo)(void *this_, int modo, int accion, HANDLE evento, int *mas);
typedef HRESULT (W *Motivo)(void *this_);
typedef HRESULT (W *Libreria)(void *this_, const void *blob, U64 n, const GUID *riid, void **pp);
typedef HRESULT (W *Mapear)(void *this_, UINT sub, const void *leer, void **pp);
typedef void *(W *Puntero)(void *this_);
typedef U64 (W *Medida)(void *this_);

static const GUID IID_Device10 = {0x517f8718, 0xaa66, 0x49f9, {0xb0, 0x2b, 0xa7, 0xab, 0x89, 0xc0, 0x60, 0x31}};
static const GUID IID_Devices[9] = {
    {0x77acce80, 0x638e, 0x4e65, {0x88, 0x95, 0xc1, 0xf2, 0x33, 0x86, 0x86, 0x3e}},
    {0x30baa41e, 0xb15b, 0x475c, {0xa0, 0xbb, 0x1a, 0xf5, 0xc5, 0xb6, 0x43, 0x28}},
    {0x81dadc15, 0x2bad, 0x4392, {0x93, 0xc5, 0x10, 0x13, 0x45, 0xc4, 0xaa, 0x98}},
    {0xe865df17, 0xa9ee, 0x46f9, {0xa4, 0x63, 0x30, 0x98, 0x31, 0x5a, 0xa2, 0xe5}},
    {0x8b4f173b, 0x2fea, 0x4b80, {0x8f, 0x58, 0x43, 0x07, 0x19, 0x1a, 0xb9, 0x5d}},
    {0xc70b221b, 0x40e4, 0x4a17, {0x89, 0xaf, 0x02, 0x5a, 0x07, 0x27, 0xa6, 0xdc}},
    {0x5c014b53, 0x68a1, 0x4b9b, {0x8b, 0xd1, 0xdd, 0x60, 0x46, 0xb9, 0x35, 0x8b}},
    {0x9218e6bb, 0xf944, 0x4f7e, {0xa7, 0x5c, 0xb1, 0xb2, 0xc7, 0xb7, 0x01, 0xf3}},
    {0x4c80e962, 0xf032, 0x4f60, {0xbc, 0x9e, 0xeb, 0xc2, 0xcf, 0xa1, 0xd8, 0x3c}},
};
static const GUID IID_Heap = {0x6b3b2502, 0x6e51, 0x45b3, {0x90, 0xee, 0x98, 0x84, 0x26, 0x5e, 0x8d, 0xf3}};
static const GUID IID_Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID IID_Queue = {0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID IID_Allocator = {0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID IID_List = {0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID IID_RootSignature = {0xc54a6b66, 0x72df, 0x4ee8, {0x8b, 0xe5, 0xa9, 0x46, 0xa1, 0x42, 0x92, 0x14}};
static const GUID IID_PipelineState = {0x765a30f3, 0xf624, 0x4c6f, {0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45}};
static const GUID IID_Fence = {0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID IID_PipelineLibrary = {0xc64226a8, 0x9201, 0x46af, {0xb4, 0xcc, 0x53, 0xfb, 0x9f, 0xf7, 0x41, 0x4f}};

/* La root signature de cubo12: un CBV en b0, con input layout. */
typedef struct {
    int ParameterType;
    union {
        struct { UINT ShaderRegister, RegisterSpace; } Descriptor;
        struct { UINT NumDescriptorRanges; const void *pDescriptorRanges; } Tabla;
    } u;
    int ShaderVisibility;
} ROOT_PARAMETER;
typedef struct {
    UINT NumParameters;
    const ROOT_PARAMETER *pParameters;
    UINT NumStaticSamplers;
    const void *pStaticSamplers;
    int Flags;
} ROOT_SIGNATURE_DESC;
typedef struct {
    const char *SemanticName;
    UINT SemanticIndex, Format, InputSlot, AlignedByteOffset;
    int InputSlotClass;
    UINT InstanceDataStepRate;
} INPUT_ELEMENT_DESC;
static const INPUT_ELEMENT_DESC LAYOUT[3] = {
    {"POSITION", 0, 6, 0, 0, 0, 0},
    {"NORMAL", 0, 6, 0, 12, 0, 0},
    {"COLOR", 0, 2, 0, 24, 0, 0},
};

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

static void a_cero(void *p, U64 n) {
    volatile unsigned char *b = p;
    while (n--)
        *b++ = 0;
}

/* Un D3D12_RESOURCE_DESC1 (64 B: el de siempre, 56, y 12 de sampler
 * feedback a cero) de un bufer: Dimension 1, Width +16, Height 1,
 * DepthOrArraySize 1, MipLevels 1, SampleDesc.Count 1, Layout ROW_MAJOR. */
static void desc_bufer(unsigned char *d, U64 ancho) {
    a_cero(d, 64);
    *(DWORD *)d = 1;
    *(U64 *)(d + 16) = ancho;
    *(DWORD *)(d + 24) = 1;
    *(unsigned short *)(d + 28) = 1;
    *(unsigned short *)(d + 30) = 1;
    *(DWORD *)(d + 36) = 1;
    *(DWORD *)(d + 44) = 1;
}

/* El flujo de subobjetos: cada uno a 8 (alignas(void*) de d3dx12), su tipo
 * y lo de dentro a 8 si lleva punteros (`alinea`) o a 4. */
static unsigned char flujo[512];
static U64 lleno;
static void sub(UINT tipo, UINT alinea, const void *dentro, UINT n) {
    const unsigned char *q = dentro;
    UINT i;
    *(UINT *)(flujo + lleno) = tipo;
    lleno += alinea;
    for (i = 0; i < n; i++)
        flujo[lleno++] = q[i];
    lleno = (lleno + 7) & ~(U64)7;
}

static volatile DWORD *mapear(void *b) {
    void *p = 0;
    if (!b || ((Mapear)hueco(b, 8))(b, 0, 0, &p) != 0)
        return 0;
    return (volatile DWORD *)p;
}

void inicio(void);

void inicio(void) {
    /* D3D12_HEAP_PROPERTIES UPLOAD; D3D12_HEAP_DESC de 1 MiB UPLOAD, solo buferes. */
    static const DWORD props[5] = {2, 0, 0, 1, 1}, props_default[5] = {1, 0, 0, 1, 1};
    static unsigned char bd[64], descs[128], hd[48];
    static U64 info1[6], ret[2];
    static ROOT_PARAMETER parametro;
    static ROOT_SIGNATURE_DESC firma;
    static const DWORD cola_desc[4] = {0, 0, 0, 0};
    void *dev = 0, *d = 0, *b = 0, *h = 0, *x = 0, *y = 0, *cola = 0, *lista = 0, *asig = 0;
    void *blob = 0, *error = 0, *raiz = 0, *vs = 0, *ps = 0, *pso = 0, *valla = 0, *lib = 0, *t = 0;
    volatile DWORD *p, *q;
    U64 vsb[2], psb[2], il[2], flujo_desc[2];
    UINT u, rt[9];
    int todas = 1, mas = 1, i;
    HRESULT r;

    r = D3D12CreateDevice(0, 0xb000, &IID_Device10, &dev);
    mira(r == 0 && dev != 0, "D3D12CreateDevice pidiendo ID3D12Device10");
    for (i = 0; i < 9; i++) {
        d = 0;
        todas &= dev && ((Pide)hueco(dev, 0))(dev, &IID_Devices[i], &d) == 0 && d == dev;
    }
    mira(todas, "ID3D12Device1 a 9: el mismo objeto");

    desc_bufer(bd, 0x10000);
    r = dev ? ((Comprometer1)hueco(dev, 53))(dev, props, 0, bd, 0xAC3, 0, 0, &IID_Resource, &b) : -1;
    p = mapear(b);
    if (p)
        p[0x3fff] = 0x45;
    mira(r == 0 && p && p[0x3fff] == 0x45, "CreateCommittedResource1: un bufer, se mapea y se escribe");

    /* Una textura 2D RGBA de 2048x2048 (16 MiB) en un monton DEFAULT. */
    a_cero(bd, 64);
    *(DWORD *)bd = 3;
    *(U64 *)(bd + 16) = 2048;
    *(DWORD *)(bd + 24) = 2048;
    *(unsigned short *)(bd + 28) = 1;
    *(unsigned short *)(bd + 30) = 1;
    *(DWORD *)(bd + 32) = 28;
    *(DWORD *)(bd + 36) = 1;
    r = dev ? ((Comprometer1)hueco(dev, 53))(dev, props_default, 0, bd, 0, 0, 0, &IID_Resource, &t) : -1;
    mira(r == 0 && t != 0, "CreateCommittedResource1: una textura de 2048x2048 (16 MiB)");

    *(U64 *)hd = 1 << 20;
    *(DWORD *)(hd + 8) = 2;
    *(DWORD *)(hd + 40) = 0xC0;
    r = dev ? ((Monton1)hueco(dev, 54))(dev, hd, 0, &IID_Heap, &h) : -1;
    mira(r == 0 && h != 0, "CreateHeap1: un monton UPLOAD de 1 MiB");
    desc_bufer(bd, 0x10000);
    r = h ? ((Colocar1)hueco(dev, 70))(dev, h, 0, bd, 0xAC3, 0, &IID_Resource, &x) : -1;
    if (h && r == 0)
        r = ((Colocar2)hueco(dev, 77))(dev, h, 0, bd, LAYOUT_UNDEFINED, 0, 0, 0, &IID_Resource, &y);
    p = mapear(x);
    q = mapear(y);
    if (p)
        p[9] = 0x4545;
    mira(r == 0 && p && q && q[9] == 0x4545, "CreatePlacedResource1 y 2 en el mismo sitio: se ven");

    /* GetResourceAllocationInfo1 lee D3D12_RESOURCE_DESC (56 B); la 2,
     * D3D12_RESOURCE_DESC1 (64 B). Dos buferes, de 64 KiB y de 100 bytes:
     * cada uno ocupa 64 KiB, el segundo detras del primero. */
    desc_bufer(descs, 0x10000);
    desc_bufer(descs + 56, 100);
    if (dev)
        ((Asignacion1)hueco(dev, 56))(dev, ret, 0, 2, descs, info1);
    mira(dev && ret[0] == 0x20000 && ret[1] == 0x10000 && info1[0] == 0 && info1[2] == 0x10000 && info1[3] == 0x10000 && info1[5] == 0x10000,
         "GetResourceAllocationInfo1: 128 KiB, y cada uno en su sitio");
    desc_bufer(descs, 0x10000);
    desc_bufer(descs + 64, 100);
    ret[0] = ret[1] = 0;
    if (dev)
        ((Asignacion1)hueco(dev, 68))(dev, ret, 0, 2, descs, info1);
    mira(dev && ret[0] == 0x20000 && info1[3] == 0x10000, "GetResourceAllocationInfo2: lo mismo con D3D12_RESOURCE_DESC1");

    r = dev ? ((Cola1)hueco(dev, 75))(dev, cola_desc, &IID_Device10, &IID_Queue, &cola) : -1;
    mira(r == 0 && cola != 0, "CreateCommandQueue1");
    r = dev ? ((Lista1)hueco(dev, 51))(dev, 0, 0, 0, &IID_List, &lista) : -1;
    if (r == 0)
        r = ((Asignador)hueco(dev, 9))(dev, 0, &IID_Allocator, &asig);
    if (r == 0)
        r = ((Reiniciar)hueco(lista, 10))(lista, asig, 0);
    if (r == 0)
        r = ((Cerrar)hueco(lista, 9))(lista);
    mira(r == 0, "CreateCommandList1 nace cerrada: Reset con su asignador y Close");

    parametro.ParameterType = 2; /* CBV */
    firma.NumParameters = 1;
    firma.pParameters = &parametro;
    firma.Flags = 1; /* ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT */
    r = dev ? D3D12SerializeRootSignature(&firma, 1, &blob, &error) : -1;
    if (r == 0)
        r = ((Raiz)hueco(dev, 16))(dev, 0, ((Puntero)hueco(blob, 3))(blob), ((Medida)hueco(blob, 4))(blob), &IID_RootSignature, &raiz);
    if (r == 0)
        r = D3DCompile(CUBO12_HLSL, sizeof CUBO12_HLSL, "cubo.hlsl", 0, 0, "VSMain", "vs_5_0", 0, 0, &vs, &error);
    if (r == 0)
        r = D3DCompile(CUBO12_HLSL, sizeof CUBO12_HLSL, "cubo.hlsl", 0, 0, "PSMain", "ps_5_0", 0, 0, &ps, &error);
    if (r == 0) {
        vsb[0] = (U64)((Puntero)hueco(vs, 3))(vs);
        vsb[1] = ((Medida)hueco(vs, 4))(vs);
        psb[0] = (U64)((Puntero)hueco(ps, 3))(ps);
        psb[1] = ((Medida)hueco(ps, 4))(ps);
        il[0] = (U64)LAYOUT;
        il[1] = 3;
        for (i = 0; i < 9; i++)
            rt[i] = 0;
        rt[0] = 28; /* R8G8B8A8_UNORM */
        rt[8] = 1;
        sub(0, 8, &raiz, 8);
        sub(1, 8, vsb, 16);
        sub(2, 8, psb, 16);
        sub(12, 8, il, 16);
        u = 3; /* TRIANGLE */
        sub(14, 4, &u, 4);
        sub(15, 4, rt, 36);
        u = 40; /* D32_FLOAT */
        sub(16, 4, &u, 4);
        flujo_desc[0] = lleno;
        flujo_desc[1] = (U64)flujo;
        r = ((Flujo)hueco(dev, 47))(dev, flujo_desc, &IID_PipelineState, &pso);
    }
    mira(r == 0 && pso != 0, "CreatePipelineState: un PSO de un flujo de subobjetos");

    r = dev ? ((Valla)hueco(dev, 36))(dev, 0, 0, &IID_Fence, &valla) : -1;
    if (r == 0)
        r = ((Residente)hueco(dev, 50))(dev, 0, 1, &b, valla, 7);
    /* SetEventOnCompletion sin evento: espera ahi (en Windows llega en seguida). */
    if (r == 0)
        r = ((AlLlegar)hueco(valla, 9))(valla, 7, 0);
    mira(r == 0 && ((Completado)hueco(valla, 8))(valla) == 7, "EnqueueMakeResident lleva la valla a su valor");
    r = dev ? ((Fondo)hueco(dev, 65))(dev, 0, 0, 0, &mas) : -1;
    mira(r == 0 && mas == 0, "SetBackgroundProcessingMode: S_OK, no hacen falta mas medidas");
    mira(dev && ((Motivo)hueco(dev, 37))(dev) == 0, "GetDeviceRemovedReason: S_OK, el dispositivo vive");
    r = dev ? ((Libreria)hueco(dev, 44))(dev, 0, 0, &IID_PipelineLibrary, &lib) : -1;
    mira((r == 0 && lib != 0) || (r == DXGI_ERROR_UNSUPPORTED && lib == 0), "CreatePipelineLibrary: una libreria, o DXGI_ERROR_UNSUPPORTED");

    di("tanda45.exe: ID3D12Device1 a 10\r\n");
    ExitProcess(fallos);
}
