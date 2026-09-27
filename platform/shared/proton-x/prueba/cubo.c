/* cubo.c -- el .exe de P3b2 de PROTON-X: el cubo de X1 con la tuberia
 * ENTERA de D3D12.
 *
 * Lo de limpia.c (dispositivo, cola, cadena de intercambio, RTV, lista,
 * valla) y encima todo lo que un programa D3D12 de verdad hace para dibujar:
 *
 *    D3D12SerializeRootSignature   un parametro: el CBV de b0, en la raiz
 *    CreateRootSignature
 *    CreateGraphicsPipelineState   los dos DXIL de dxc (cubo_datos.h), el input
 *                                  layout POSITION, NORMAL, COLOR (40 bytes),
 *                                  descarte de las caras de detras, sin
 *                                  profundidad, un render target R8G8B8A8
 *    CreateCommittedResource x3    buferes UPLOAD: vertices, indices y
 *                                  constantes, mapeados y rellenos
 *    cada fotograma                Reset con el PSO, root signature, CBV,
 *                                  viewport, tijera, barrera, destino,
 *                                  limpiar, topologia, vertices, indices,
 *                                  DrawIndexedInstanced(36), barrera,
 *                                  Close, Execute, Present, valla
 *
 * Los datos son los de X1 (bmo-cubo) en bits, fabricados en cubo_datos.h: el
 * .exe no calcula ni un seno. Las constantes de cada fotograma son las que X4
 * subio a la 3060 para sus huellas.
 *
 * Las estructuras son las de d3d12.h, campo a campo, y cada desplazamiento
 * que cuenta se COMPRUEBA al compilar (los _Static_assert): con clang o con
 * MSVC, en Windows este .exe dibuja el cubo. En BMO-X lo dibuja la casa
 * (P3b3): los mismos DXIL, corridos en la CPU, y la misma imagen.
 *
 *    una letra   30 fotogramas mas (el cubo gira 30 grados)
 *    q o ESC     cierra
 *
 * Sale con el numero de Present que se hicieron. */
typedef unsigned short WCHAR;
typedef unsigned int UINT;
typedef unsigned long DWORD;
typedef long LONG;
typedef long HRESULT;
typedef unsigned long long U64;
typedef long long I64;
typedef void *HANDLE;
typedef I64 (*WNDPROC)(HANDLE, UINT, U64, I64);
typedef struct { DWORD d1; unsigned short d2, d3; unsigned char d4[8]; } GUID;

#include "cubo_datos.h"

typedef struct {
    UINT cbSize, style;
    WNDPROC lpfnWndProc;
    int cbClsExtra, cbWndExtra;
    HANDLE hInstance, hIcon, hCursor, hbrBackground;
    const WCHAR *lpszMenuName, *lpszClassName;
    HANDLE hIconSm;
} WNDCLASSEXW;

typedef struct {
    HANDLE hwnd;
    UINT message;
    U64 wParam;
    I64 lParam;
    DWORD time;
    LONG x, y;
    DWORD lPrivate;
} MSG;

typedef struct { int Type, Priority, Flags; UINT NodeMask; } QUEUE_DESC;
typedef struct { int Type; UINT NumDescriptors; int Flags; UINT NodeMask; } HEAP_DESC;
typedef struct {
    UINT Width, Height, Format;
    int Stereo;
    UINT Count, Quality;
    UINT BufferUsage, BufferCount;
    int Scaling, SwapEffect, AlphaMode;
    UINT Flags;
} SWAP_DESC1;
/* D3D12_RESOURCE_BARRIER de transicion: 32 bytes. */
typedef struct {
    int Type, Flags;
    void *pResource;
    UINT Subresource;
    int StateBefore, StateAfter;
    int relleno;
} BARRIER;

/* -- La root signature (d3d12.h) ------------------------------------------ */
typedef struct {
    int ParameterType;
    /* La union: D3D12_ROOT_DESCRIPTOR es la que se usa aqui; la mas grande
     * (D3D12_ROOT_DESCRIPTOR_TABLE: UINT y un puntero) mide 16. */
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

/* -- El PSO (d3d12.h) ----------------------------------------------------- */
typedef struct { const void *pShaderBytecode; U64 BytecodeLength; } SHADER_BYTECODE;
typedef struct {
    const void *pSODeclaration;
    UINT NumEntries;
    const UINT *pBufferStrides;
    UINT NumStrides, RasterizedStream;
} STREAM_OUTPUT_DESC;
typedef struct {
    int BlendEnable, LogicOpEnable;
    int SrcBlend, DestBlend, BlendOp, SrcBlendAlpha, DestBlendAlpha, BlendOpAlpha, LogicOp;
    unsigned char RenderTargetWriteMask;
} RENDER_TARGET_BLEND_DESC;
typedef struct {
    int AlphaToCoverageEnable, IndependentBlendEnable;
    RENDER_TARGET_BLEND_DESC RenderTarget[8];
} BLEND_DESC;
typedef struct {
    int FillMode, CullMode, FrontCounterClockwise, DepthBias;
    float DepthBiasClamp, SlopeScaledDepthBias;
    int DepthClipEnable, MultisampleEnable, AntialiasedLineEnable;
    UINT ForcedSampleCount;
    int ConservativeRaster;
} RASTERIZER_DESC;
typedef struct { int StencilFailOp, StencilDepthFailOp, StencilPassOp, StencilFunc; } DEPTH_STENCILOP_DESC;
typedef struct {
    int DepthEnable, DepthWriteMask, DepthFunc, StencilEnable;
    unsigned char StencilReadMask, StencilWriteMask;
    DEPTH_STENCILOP_DESC FrontFace, BackFace;
} DEPTH_STENCIL_DESC;
typedef struct {
    const char *SemanticName;
    UINT SemanticIndex, Format, InputSlot, AlignedByteOffset;
    int InputSlotClass;
    UINT InstanceDataStepRate;
} INPUT_ELEMENT_DESC;
typedef struct { const INPUT_ELEMENT_DESC *pInputElementDescs; UINT NumElements; } INPUT_LAYOUT_DESC;
typedef struct { const void *pCachedBlob; U64 CachedBlobSizeInBytes; } CACHED_PIPELINE_STATE;
typedef struct {
    void *pRootSignature;
    SHADER_BYTECODE VS, PS, DS, HS, GS;
    STREAM_OUTPUT_DESC StreamOutput;
    BLEND_DESC BlendState;
    UINT SampleMask;
    RASTERIZER_DESC RasterizerState;
    DEPTH_STENCIL_DESC DepthStencilState;
    INPUT_LAYOUT_DESC InputLayout;
    int IBStripCutValue, PrimitiveTopologyType;
    UINT NumRenderTargets;
    UINT RTVFormats[8];
    UINT DSVFormat;
    UINT SampleCount, SampleQuality;
    UINT NodeMask;
    CACHED_PIPELINE_STATE CachedPSO;
    int Flags;
} PSO_DESC;

/* -- Los recursos (d3d12.h) ----------------------------------------------- */
typedef struct { int Type, CPUPageProperty, MemoryPoolPreference; UINT CreationNodeMask, VisibleNodeMask; } HEAP_PROPERTIES;
typedef struct {
    int Dimension;
    U64 Alignment, Width;
    UINT Height;
    unsigned short DepthOrArraySize, MipLevels;
    UINT Format, SampleCount, SampleQuality;
    int Layout, Flags;
} RESOURCE_DESC;
typedef struct { U64 Begin, End; } RANGE;
typedef struct { U64 BufferLocation; UINT SizeInBytes, StrideInBytes; } VERTEX_BUFFER_VIEW;
typedef struct { U64 BufferLocation; UINT SizeInBytes, Format; } INDEX_BUFFER_VIEW;
typedef struct { float TopLeftX, TopLeftY, Width, Height, MinDepth, MaxDepth; } VIEWPORT;
typedef struct { LONG left, top, right, bottom; } RECT;

/* Lo que se midio contra la cabecera de Windows: si un campo se moviera, el
 * .exe no compila (y la casa lee los mismos numeros: tuberia.rs). */
#define DESDE(t, c) __builtin_offsetof(t, c)
_Static_assert(sizeof(ROOT_PARAMETER) == 32 && DESDE(ROOT_PARAMETER, ShaderVisibility) == 24, "ROOT_PARAMETER");
_Static_assert(sizeof(ROOT_SIGNATURE_DESC) == 40 && DESDE(ROOT_SIGNATURE_DESC, Flags) == 32, "ROOT_SIGNATURE_DESC");
_Static_assert(sizeof(INPUT_ELEMENT_DESC) == 32, "INPUT_ELEMENT_DESC");
_Static_assert(sizeof(PSO_DESC) == 656, "PSO_DESC");
_Static_assert(DESDE(PSO_DESC, VS) == 8 && DESDE(PSO_DESC, PS) == 24 && DESDE(PSO_DESC, StreamOutput) == 88, "PSO_DESC sombreadores");
_Static_assert(DESDE(PSO_DESC, BlendState) == 120 && DESDE(PSO_DESC, SampleMask) == 448, "PSO_DESC mezcla");
_Static_assert(DESDE(PSO_DESC, RasterizerState) == 452 && DESDE(PSO_DESC, DepthStencilState) == 496, "PSO_DESC raster");
_Static_assert(DESDE(PSO_DESC, InputLayout) == 552 && DESDE(PSO_DESC, PrimitiveTopologyType) == 572, "PSO_DESC layout");
_Static_assert(DESDE(PSO_DESC, NumRenderTargets) == 576 && DESDE(PSO_DESC, RTVFormats) == 580, "PSO_DESC destinos");
_Static_assert(DESDE(PSO_DESC, DSVFormat) == 612 && DESDE(PSO_DESC, SampleCount) == 616, "PSO_DESC muestras");
_Static_assert(DESDE(PSO_DESC, CachedPSO) == 632 && DESDE(PSO_DESC, Flags) == 648, "PSO_DESC final");
_Static_assert(sizeof(RESOURCE_DESC) == 56 && DESDE(RESOURCE_DESC, Format) == 32 && DESDE(RESOURCE_DESC, Flags) == 48, "RESOURCE_DESC");
_Static_assert(sizeof(HEAP_PROPERTIES) == 20 && sizeof(VIEWPORT) == 24, "HEAP_PROPERTIES, VIEWPORT");
_Static_assert(sizeof(VERTEX_BUFFER_VIEW) == 16 && sizeof(INDEX_BUFFER_VIEW) == 16, "vistas");

#define WM_DESTROY 0x0002
#define WM_KEYDOWN 0x0100
#define WM_CHAR 0x0102
#define VK_ESCAPE 0x1B
#define WS_OVERLAPPEDWINDOW 0x00CF0000
#define SW_SHOW 5
#define INFINITE 0xFFFFFFFF
#define R8G8B8A8_UNORM 28
#define R32G32B32A32_FLOAT 2
#define R32G32B32_FLOAT 6
#define R16_UINT 57
#define USAGE_RENDER_TARGET_OUTPUT 0x20
#define SWAP_EFFECT_FLIP_DISCARD 4
#define STATE_PRESENT 0
#define STATE_RENDER_TARGET 4
#define STATE_GENERIC_READ 0xAC3
#define HEAP_TYPE_UPLOAD 2
#define DIMENSION_BUFFER 1
#define LAYOUT_ROW_MAJOR 1
#define ROOT_PARAMETER_CBV 2
#define ROOT_SIGNATURE_VERSION_1 1
#define ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT 1
#define FILL_SOLID 3
#define CULL_BACK 3
#define BLEND_ZERO 1
#define BLEND_ONE 2
#define BLEND_OP_ADD 1
#define LOGIC_OP_NOOP 4
#define TOPOLOGY_TYPE_TRIANGLE 3
#define TRIANGLELIST 4
#define FOTOGRAMAS 360
#define PASO 30

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetModuleHandleW(const WCHAR *nombre);
IMPORTA void WINAPI ExitProcess(UINT codigo);
IMPORTA HANDLE WINAPI CreateEventW(void *attr, int manual, int inicial, const WCHAR *nombre);
IMPORTA DWORD WINAPI WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int WINAPI CloseHandle(HANDLE h);
IMPORTA unsigned short WINAPI RegisterClassExW(const WNDCLASSEXW *c);
IMPORTA HANDLE WINAPI CreateWindowExW(DWORD ex, const WCHAR *clase, const WCHAR *titulo, DWORD estilo, int x, int y,
                                      int ancho, int alto, HANDLE padre, HANDLE menu, HANDLE inst, void *param);
IMPORTA int WINAPI ShowWindow(HANDLE h, int como);
IMPORTA int WINAPI GetMessageW(MSG *m, HANDLE h, UINT min, UINT max);
IMPORTA int WINAPI TranslateMessage(const MSG *m);
IMPORTA I64 WINAPI DispatchMessageW(const MSG *m);
IMPORTA I64 WINAPI DefWindowProcW(HANDLE h, UINT m, U64 w, I64 l);
IMPORTA void WINAPI PostQuitMessage(int codigo);
IMPORTA int WINAPI DestroyWindow(HANDLE h);
IMPORTA HRESULT WINAPI D3D12CreateDevice(void *adaptador, int nivel, const GUID *riid, void **pp);
IMPORTA HRESULT WINAPI D3D12SerializeRootSignature(const ROOT_SIGNATURE_DESC *desc, int version, void **blob, void **error);
IMPORTA HRESULT WINAPI CreateDXGIFactory2(UINT banderas, const GUID *riid, void **pp);

static const GUID IID_Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_Queue = {0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID IID_Allocator = {0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID IID_List = {0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID IID_Heap = {0x8efb471d, 0x616c, 0x4f49, {0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51}};
static const GUID IID_Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID IID_Fence = {0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID IID_RootSignature = {0xc54a6b66, 0x72df, 0x4ee8, {0x8b, 0xe5, 0xa9, 0x46, 0xa1, 0x42, 0x92, 0x14}};
static const GUID IID_PipelineState = {0x765a30f3, 0xf624, 0x4c6f, {0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45}};
static const GUID IID_Factory2 = {0x50c83a1c, 0xe072, 0x4c48, {0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0}};

/* El metodo del hueco `h` de un objeto COM. */
#define HUECO(obj, h) ((*(void ***)(obj))[h])
typedef HRESULT(__stdcall *F_crear4)(void *, const void *, const GUID *, void **);
typedef HRESULT(__stdcall *F_crear_asig)(void *, int, const GUID *, void **);
typedef HRESULT(__stdcall *F_crear_lista)(void *, UINT, int, void *, void *, const GUID *, void **);
typedef HRESULT(__stdcall *F_crear_raiz)(void *, UINT, const void *, U64, const GUID *, void **);
typedef HRESULT(__stdcall *F_crear_recurso)(void *, const HEAP_PROPERTIES *, int, const RESOURCE_DESC *, int, const void *,
                                            const GUID *, void **);
typedef UINT(__stdcall *F_incremento)(void *, int);
typedef void(__stdcall *F_rtv)(void *, void *, const void *, U64);
typedef HRESULT(__stdcall *F_valla)(void *, U64, int, const GUID *, void **);
typedef U64 *(__stdcall *F_inicio)(void *, U64 *);
typedef HRESULT(__stdcall *F_cadena)(void *, void *, HANDLE, const SWAP_DESC1 *, const void *, void *, void **);
typedef HRESULT(__stdcall *F_buffer)(void *, UINT, const GUID *, void **);
typedef HRESULT(__stdcall *F_0)(void *);
typedef U64(__stdcall *F_u64)(void *);
typedef void *(__stdcall *F_ptr)(void *);
typedef HRESULT(__stdcall *F_map)(void *, UINT, const RANGE *, void **);
typedef HRESULT(__stdcall *F_reset)(void *, void *, void *);
typedef void(__stdcall *F_barrera)(void *, UINT, const BARRIER *);
typedef void(__stdcall *F_limpiar)(void *, U64, const float *, UINT, const void *);
typedef void(__stdcall *F_poner)(void *, void *);
typedef void(__stdcall *F_uint)(void *, UINT);
typedef void(__stdcall *F_cbv)(void *, UINT, U64);
typedef void(__stdcall *F_viewports)(void *, UINT, const VIEWPORT *);
typedef void(__stdcall *F_tijeras)(void *, UINT, const RECT *);
typedef void(__stdcall *F_vb)(void *, UINT, UINT, const VERTEX_BUFFER_VIEW *);
typedef void(__stdcall *F_ib)(void *, const INDEX_BUFFER_VIEW *);
typedef void(__stdcall *F_destinos)(void *, UINT, const U64 *, int, const U64 *);
typedef void(__stdcall *F_dibujar)(void *, UINT, UINT, UINT, int, UINT);
typedef void(__stdcall *F_ejecutar)(void *, UINT, void **);
typedef HRESULT(__stdcall *F_signal)(void *, void *, U64);
typedef HRESULT(__stdcall *F_evento)(void *, U64, HANDLE);
typedef HRESULT(__stdcall *F_present)(void *, UINT, UINT);

/*   ID3D12Device        8 CreateCommandQueue   9 CreateCommandAllocator
 *                       10 CreateGraphicsPipelineState  12 CreateCommandList
 *                       14 CreateDescriptorHeap
 *                       15 GetDescriptorHandleIncrementSize
 *                       16 CreateRootSignature  20 CreateRenderTargetView
 *                       27 CreateCommittedResource  36 CreateFence
 *   ID3D12Resource      8 Map  11 GetGPUVirtualAddress
 *   ID3DBlob            3 GetBufferPointer  4 GetBufferSize
 *   ID3D12DescriptorHeap 9 GetCPUDescriptorHandleForHeapStart
 *   ID3D12CommandAllocator 8 Reset
 *   ID3D12GraphicsCommandList 9 Close 10 Reset 13 DrawIndexedInstanced
 *                       20 IASetPrimitiveTopology 21 RSSetViewports
 *                       22 RSSetScissorRects 25 SetPipelineState
 *                       26 ResourceBarrier 30 SetGraphicsRootSignature
 *                       38 SetGraphicsRootConstantBufferView
 *                       43 IASetIndexBuffer 44 IASetVertexBuffers
 *                       46 OMSetRenderTargets 48 ClearRenderTargetView
 *   ID3D12CommandQueue  10 ExecuteCommandLists  14 Signal
 *   ID3D12Fence         8 GetCompletedValue  9 SetEventOnCompletion
 *   IDXGIFactory2       15 CreateSwapChainForHwnd
 *   IDXGISwapChain1     8 Present  9 GetBuffer                            */

#define BUFFERS 2

static void *disp, *cola, *asig, *lista, *cadena, *valla, *buffers[BUFFERS];
static void *raiz, *pso, *vb, *ib, *cb;
static unsigned int *constantes;
static U64 rtv[BUFFERS];
static HANDLE evento;
static U64 valor;
static UINT presentados, fotograma_actual;

/* Las estructuras grandes, fuera de la pila y a cero (sin CRT no hay memset). */
static PSO_DESC desc;
static ROOT_PARAMETER parametro;
static ROOT_SIGNATURE_DESC firma;
static VERTEX_BUFFER_VIEW vista_vb;
static INDEX_BUFFER_VIEW vista_ib;

/* El fondo de X1 (bmo_cubo::FONDO_F): 16/255, 16/255, 24/255. */
static const float FONDO[4] = {16.0f / 255.0f, 16.0f / 255.0f, 24.0f / 255.0f, 1.0f};
static const VIEWPORT VISTA = {0.0f, 0.0f, (float)CUBO_ANCHO, (float)CUBO_ALTO, 0.0f, 1.0f};
static const RECT TIJERA = {0, 0, CUBO_ANCHO, CUBO_ALTO};

static const INPUT_ELEMENT_DESC LAYOUT[3] = {
    {"POSITION", 0, R32G32B32_FLOAT, 0, 0, 0, 0},
    {"NORMAL", 0, R32G32B32_FLOAT, 0, 12, 0, 0},
    {"COLOR", 0, R32G32B32A32_FLOAT, 0, 24, 0, 0},
};

/* Un bufer UPLOAD de `bytes`, mapeado; lo mapeado va en `*mapa`. */
static void *bufer(U64 bytes, void **mapa, UINT fallo) {
    HEAP_PROPERTIES hp = {HEAP_TYPE_UPLOAD, 0, 0, 1, 1};
    RESOURCE_DESC rd;
    RANGE nada = {0, 0};
    void *r;
    rd.Dimension = DIMENSION_BUFFER;
    rd.Alignment = 0;
    rd.Width = bytes;
    rd.Height = 1;
    rd.DepthOrArraySize = 1;
    rd.MipLevels = 1;
    rd.Format = 0;
    rd.SampleCount = 1;
    rd.SampleQuality = 0;
    rd.Layout = LAYOUT_ROW_MAJOR;
    rd.Flags = 0;
    if (((F_crear_recurso)HUECO(disp, 27))(disp, &hp, 0, &rd, STATE_GENERIC_READ, 0, &IID_Resource, &r) < 0) ExitProcess(fallo);
    if (((F_map)HUECO(r, 8))(r, 0, &nada, mapa) < 0) ExitProcess(fallo + 1);
    return r;
}

static U64 va(void *r) { return ((F_u64)HUECO(r, 11))(r); }

static void copiar(void *a, const void *de, UINT bytes) {
    unsigned char *d = a;
    const unsigned char *s = de;
    UINT i;
    for (i = 0; i < bytes; i++) d[i] = s[i];
}

static void barrera(void *recurso, int antes, int despues) {
    BARRIER b;
    b.Type = 0;
    b.Flags = 0;
    b.pResource = recurso;
    b.Subresource = 0xFFFFFFFF;
    b.StateBefore = antes;
    b.StateAfter = despues;
    b.relleno = 0;
    ((F_barrera)HUECO(lista, 26))(lista, 1, &b);
}

/* La root signature, el PSO y los tres buferes. */
static void tuberia(void) {
    void *blob, *error, *mapa;
    UINT i;

    parametro.ParameterType = ROOT_PARAMETER_CBV;
    parametro.u.Descriptor.ShaderRegister = 0;
    parametro.u.Descriptor.RegisterSpace = 0;
    parametro.ShaderVisibility = 0; /* ALL: la luz la lee el de pixeles */
    firma.NumParameters = 1;
    firma.pParameters = &parametro;
    firma.Flags = ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT;
    if (D3D12SerializeRootSignature(&firma, ROOT_SIGNATURE_VERSION_1, &blob, &error) < 0) ExitProcess(0xE201);
    if (((F_crear_raiz)HUECO(disp, 16))(disp, 0, ((F_ptr)HUECO(blob, 3))(blob), ((F_u64)HUECO(blob, 4))(blob),
                                        &IID_RootSignature, &raiz) < 0)
        ExitProcess(0xE202);

    desc.pRootSignature = raiz;
    desc.VS.pShaderBytecode = CUBO_VS;
    desc.VS.BytecodeLength = sizeof CUBO_VS;
    desc.PS.pShaderBytecode = CUBO_PS;
    desc.PS.BytecodeLength = sizeof CUBO_PS;
    for (i = 0; i < 8; i++) {
        desc.BlendState.RenderTarget[i].SrcBlend = BLEND_ONE;
        desc.BlendState.RenderTarget[i].DestBlend = BLEND_ZERO;
        desc.BlendState.RenderTarget[i].BlendOp = BLEND_OP_ADD;
        desc.BlendState.RenderTarget[i].SrcBlendAlpha = BLEND_ONE;
        desc.BlendState.RenderTarget[i].DestBlendAlpha = BLEND_ZERO;
        desc.BlendState.RenderTarget[i].BlendOpAlpha = BLEND_OP_ADD;
        desc.BlendState.RenderTarget[i].LogicOp = LOGIC_OP_NOOP;
        desc.BlendState.RenderTarget[i].RenderTargetWriteMask = 0xF;
    }
    desc.SampleMask = 0xFFFFFFFF;
    desc.RasterizerState.FillMode = FILL_SOLID;
    desc.RasterizerState.CullMode = CULL_BACK;
    desc.RasterizerState.FrontCounterClockwise = 0;
    desc.RasterizerState.DepthClipEnable = 1;
    desc.DepthStencilState.DepthEnable = 0;
    desc.InputLayout.pInputElementDescs = LAYOUT;
    desc.InputLayout.NumElements = 3;
    desc.PrimitiveTopologyType = TOPOLOGY_TYPE_TRIANGLE;
    desc.NumRenderTargets = 1;
    desc.RTVFormats[0] = R8G8B8A8_UNORM;
    desc.SampleCount = 1;
    if (((F_crear4)HUECO(disp, 10))(disp, &desc, &IID_PipelineState, &pso) < 0) ExitProcess(0xE203);

    vb = bufer(sizeof CUBO_VERTICES, &mapa, 0xE210);
    copiar(mapa, CUBO_VERTICES, sizeof CUBO_VERTICES);
    ib = bufer(sizeof CUBO_INDICES, &mapa, 0xE212);
    copiar(mapa, CUBO_INDICES, sizeof CUBO_INDICES);
    /* 256: lo que D3D12 pide a un bufer de constantes. Se queda mapeado. */
    cb = bufer(256, &mapa, 0xE214);
    constantes = mapa;
    vista_vb.BufferLocation = va(vb);
    vista_vb.SizeInBytes = sizeof CUBO_VERTICES;
    vista_vb.StrideInBytes = 40;
    vista_ib.BufferLocation = va(ib);
    vista_ib.SizeInBytes = sizeof CUBO_INDICES;
    vista_ib.Format = R16_UINT;
}

/* Un fotograma: el cubo en su angulo, presentado, y esperar a la valla. */
static void fotograma(void) {
    UINT i = presentados % BUFFERS;
    copiar(constantes, CUBO_CONSTANTES[fotograma_actual % FOTOGRAMAS], sizeof CUBO_CONSTANTES[0]);
    ((F_0)HUECO(asig, 8))(asig);
    ((F_reset)HUECO(lista, 10))(lista, asig, pso);
    ((F_poner)HUECO(lista, 30))(lista, raiz);
    ((F_cbv)HUECO(lista, 38))(lista, 0, va(cb));
    ((F_viewports)HUECO(lista, 21))(lista, 1, &VISTA);
    ((F_tijeras)HUECO(lista, 22))(lista, 1, &TIJERA);
    barrera(buffers[i], STATE_PRESENT, STATE_RENDER_TARGET);
    ((F_destinos)HUECO(lista, 46))(lista, 1, &rtv[i], 0, 0);
    ((F_limpiar)HUECO(lista, 48))(lista, rtv[i], FONDO, 0, 0);
    ((F_uint)HUECO(lista, 20))(lista, TRIANGLELIST);
    ((F_vb)HUECO(lista, 44))(lista, 0, 1, &vista_vb);
    ((F_ib)HUECO(lista, 43))(lista, &vista_ib);
    ((F_dibujar)HUECO(lista, 13))(lista, 36, 1, 0, 0, 0);
    barrera(buffers[i], STATE_RENDER_TARGET, STATE_PRESENT);
    ((F_0)HUECO(lista, 9))(lista);
    ((F_ejecutar)HUECO(cola, 10))(cola, 1, &lista);
    ((F_present)HUECO(cadena, 8))(cadena, 1, 0);
    presentados++;
    valor++;
    ((F_signal)HUECO(cola, 14))(cola, valla, valor);
    if (((F_u64)HUECO(valla, 8))(valla) < valor) {
        ((F_evento)HUECO(valla, 9))(valla, valor, evento);
        WaitForSingleObject(evento, INFINITE);
    }
}

static I64 __stdcall proc(HANDLE h, UINT m, U64 w, I64 l) {
    switch (m) {
    case WM_CHAR:
        if (w == 'q') {
            DestroyWindow(h);
            return 0;
        }
        fotograma_actual += PASO;
        fotograma();
        return 0;
    case WM_KEYDOWN:
        if (w == VK_ESCAPE) DestroyWindow(h);
        return 0;
    case WM_DESTROY:
        PostQuitMessage((int)presentados);
        return 0;
    }
    return DefWindowProcW(h, m, w, l);
}

static const WCHAR CLASE[] = L"BMOXCubo";
static const WCHAR TITULO[] = L"PROTON-X P3b2";

void inicio(void) {
    WNDCLASSEXW wc;
    MSG msg;
    QUEUE_DESC qd;
    HEAP_DESC hd;
    SWAP_DESC1 sd;
    void *fabrica, *monton;
    U64 base, paso;
    UINT i;
    HANDLE h, inst = GetModuleHandleW(0);

    wc.cbSize = sizeof wc;
    wc.style = 0;
    wc.lpfnWndProc = proc;
    wc.cbClsExtra = 0;
    wc.cbWndExtra = 0;
    wc.hInstance = inst;
    wc.hIcon = 0;
    wc.hCursor = 0;
    wc.hbrBackground = 0;
    wc.lpszMenuName = 0;
    wc.lpszClassName = CLASE;
    wc.hIconSm = 0;
    if (!RegisterClassExW(&wc)) ExitProcess(0xE001);
    h = CreateWindowExW(0, CLASE, TITULO, WS_OVERLAPPEDWINDOW, 0, 0, CUBO_ANCHO, CUBO_ALTO, 0, 0, inst, 0);
    if (!h) ExitProcess(0xE002);

    if (D3D12CreateDevice(0, 0xb000, &IID_Device, &disp) < 0) ExitProcess(0xE101);
    if (CreateDXGIFactory2(0, &IID_Factory2, &fabrica) < 0) ExitProcess(0xE102);
    qd.Type = 0;
    qd.Priority = 0;
    qd.Flags = 0;
    qd.NodeMask = 0;
    if (((F_crear4)HUECO(disp, 8))(disp, &qd, &IID_Queue, &cola) < 0) ExitProcess(0xE103);

    /* La cadena mide lo que las huellas de X4: 1280x720, aunque el area de la
     * ventana sea algo menor (Windows la estira al presentar). */
    sd.Width = CUBO_ANCHO;
    sd.Height = CUBO_ALTO;
    sd.Format = R8G8B8A8_UNORM;
    sd.Stereo = 0;
    sd.Count = 1;
    sd.Quality = 0;
    sd.BufferUsage = USAGE_RENDER_TARGET_OUTPUT;
    sd.BufferCount = BUFFERS;
    sd.Scaling = 0;
    sd.SwapEffect = SWAP_EFFECT_FLIP_DISCARD;
    sd.AlphaMode = 0;
    sd.Flags = 0;
    if (((F_cadena)HUECO(fabrica, 15))(fabrica, cola, h, &sd, 0, 0, &cadena) < 0) ExitProcess(0xE104);

    hd.Type = 2; /* D3D12_DESCRIPTOR_HEAP_TYPE_RTV */
    hd.NumDescriptors = BUFFERS;
    hd.Flags = 0;
    hd.NodeMask = 0;
    if (((F_crear4)HUECO(disp, 14))(disp, &hd, &IID_Heap, &monton) < 0) ExitProcess(0xE105);
    ((F_inicio)HUECO(monton, 9))(monton, &base);
    paso = ((F_incremento)HUECO(disp, 15))(disp, 2);
    for (i = 0; i < BUFFERS; i++) {
        if (((F_buffer)HUECO(cadena, 9))(cadena, i, &IID_Resource, &buffers[i]) < 0) ExitProcess(0xE106);
        rtv[i] = base + i * paso;
        ((F_rtv)HUECO(disp, 20))(disp, buffers[i], 0, rtv[i]);
    }
    if (((F_crear_asig)HUECO(disp, 9))(disp, 0, &IID_Allocator, &asig) < 0) ExitProcess(0xE107);
    tuberia();
    if (((F_crear_lista)HUECO(disp, 12))(disp, 0, 0, asig, pso, &IID_List, &lista) < 0) ExitProcess(0xE108);
    ((F_0)HUECO(lista, 9))(lista);
    if (((F_valla)HUECO(disp, 36))(disp, 0, 0, &IID_Fence, &valla) < 0) ExitProcess(0xE109);
    evento = CreateEventW(0, 0, 0, 0);

    ShowWindow(h, SW_SHOW);
    fotograma();
    msg.wParam = 0;
    while (GetMessageW(&msg, 0, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    CloseHandle(evento);
    ExitProcess((UINT)msg.wParam);
}
