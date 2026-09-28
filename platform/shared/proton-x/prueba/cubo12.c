/* cubo12.c -- el .exe de P3c4 de PROTON-X: el cubo de cubo.c por el CAMINO
 * de BMOX-12 (estudio_d3d12 de EPICX, `cubo_d3d12.rs`), lo que ese programa
 * pide y cubo.c no:
 *
 *    CreateDXGIFactory2(IDXGIFactory6)   EnumAdapterByGpuPreference(0, ALTO
 *                                        RENDIMIENTO) -> IDXGIAdapter1,
 *                                        GetDesc1, y D3D12CreateDevice CON el
 *                                        adaptador; CheckFeatureSupport del
 *                                        tearing
 *    la cadena, como IDXGISwapChain3     GetCurrentBackBufferIndex en cada
 *                                        fotograma
 *    D3DCompile                          el HLSL de BMOX-12 (SM5: vs_5_0 y
 *                                        ps_5_0), en marcha
 *    la PROFUNDIDAD                      un D32 DEFAULT (GetDesc,
 *                                        GetResourceAllocationInfo), su DSV,
 *                                        ClearDepthStencilView a 1.0 y un PSO
 *                                        con DepthEnable, LESS
 *    la CAPTURA (su --fotograma)         GetCopyableFootprints, un bufer
 *                                        READBACK, y en cada fotograma
 *                                        CopyTextureRegion del back buffer;
 *                                        el .exe saca la HUELLA (FNV-1a de
 *                                        B, G, R, 0: bmo_cubo::referencia)
 *                                        y la compara con la de la 3060 en
 *                                        los fotogramas 0, 30 y 60
 *
 * Todo lo demas es cubo.c (los datos de X1 en cubo_datos.h, y sus
 * sombreadores DXIL no se usan). En Windows dibuja el cubo con el d3dcompiler
 * de ese Windows; en BMO-X, D3DCompile da los .cso de window/sombras (P3c2)
 * y la casa corre ese SM5 (P3c3): la misma imagen, las huellas de la 3060.
 *
 *    una letra   30 fotogramas mas     q o ESC   cierra
 *
 * Sale con el numero de Present que se hicieron, mas 0x100 por cada huella
 * que NO es la de la 3060 (3 es todo bien); un fallo, con 0xE1xx/0xE2xx. */
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

/* Sin CRT: MSVC (y lld-link) piden este simbolo a quien pasa un float a una
 * funcion (el 1.0f de ClearDepthStencilView). Lo define el CRT; aqui, a mano. */
int _fltused = 0;

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

/* D3D12_TEXTURE_COPY_LOCATION (48 B): el recurso, el tipo, y la huella de un
 * subrecurso en un bufer (D3D12_PLACED_SUBRESOURCE_FOOTPRINT) o su indice. */
typedef struct { U64 Offset; UINT Format, Width, Height, Depth, RowPitch; } HUELLA_EN_BUFER;
typedef struct {
    void *pResource;
    int Type;
    union { HUELLA_EN_BUFER Huella; UINT Indice; } u;
} COPY_LOCATION;
_Static_assert(sizeof(HUELLA_EN_BUFER) == 32 && sizeof(COPY_LOCATION) == 48 && DESDE(COPY_LOCATION, u) == 16, "COPY_LOCATION");

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
#define DIMENSION_TEXTURE2D 3
#define D32_FLOAT 40
#define HEAP_TYPE_DEFAULT 1
#define HEAP_TYPE_READBACK 3
#define STATE_COPY_DEST 0x400
#define STATE_COPY_SOURCE 0x800
#define COPY_PLACED_FOOTPRINT 1
#define STATE_DEPTH_WRITE 0x10
#define ALLOW_DEPTH_STENCIL 2
#define COMPARISON_LESS 2
#define DEPTH_WRITE_ALL 1
#define CLEAR_DEPTH 1
#define GPU_ALTO_RENDIMIENTO 1
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
IMPORTA HRESULT WINAPI D3DCompile(const void *src, U64 n, const char *nombre, const void *macros, void *include, const char *entrada,
                                  const char *perfil, UINT f1, UINT f2, void **codigo, void **errores);

static const GUID IID_Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_Queue = {0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID IID_Allocator = {0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID IID_List = {0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID IID_Heap = {0x8efb471d, 0x616c, 0x4f49, {0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51}};
static const GUID IID_Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID IID_Fence = {0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID IID_RootSignature = {0xc54a6b66, 0x72df, 0x4ee8, {0x8b, 0xe5, 0xa9, 0x46, 0xa1, 0x42, 0x92, 0x14}};
static const GUID IID_PipelineState = {0x765a30f3, 0xf624, 0x4c6f, {0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45}};
static const GUID IID_Factory6 = {0xc1b6694f, 0xff09, 0x44a9, {0xb0, 0x3c, 0x77, 0x90, 0x0a, 0x0a, 0x1d, 0x17}};
static const GUID IID_Adapter1 = {0x29038f61, 0x3839, 0x4626, {0x91, 0xfd, 0x08, 0x68, 0x79, 0x01, 0x1a, 0x05}};
static const GUID IID_SwapChain3 = {0x94d99bdb, 0xf1f8, 0x4ab0, {0xb2, 0x36, 0x7d, 0xa0, 0x17, 0x0e, 0xda, 0xb1}};

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
typedef HRESULT(__stdcall *F_preferencia)(void *, UINT, int, const GUID *, void **);
typedef HRESULT(__stdcall *F_desc1)(void *, void *);
typedef HRESULT(__stdcall *F_soporta)(void *, int, void *, UINT);
typedef HRESULT(__stdcall *F_qi)(void *, const GUID *, void **);
typedef UINT(__stdcall *F_indice)(void *);
typedef void(__stdcall *F_dsv)(void *, void *, const void *, U64);
typedef void(__stdcall *F_limpiar_z)(void *, U64, int, float, unsigned char, UINT, const void *);
typedef RESOURCE_DESC *(__stdcall *F_getdesc)(void *, RESOURCE_DESC *);
typedef void(__stdcall *F_huellas)(void *, const RESOURCE_DESC *, UINT, UINT, U64, HUELLA_EN_BUFER *, UINT *, U64 *, U64 *);
typedef void(__stdcall *F_copiar_region)(void *, const COPY_LOCATION *, UINT, UINT, UINT, const COPY_LOCATION *, const void *);
typedef U64 *(__stdcall *F_asignacion)(void *, U64 *, UINT, UINT, const RESOURCE_DESC *);

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
 *   IDXGIFactory6       15 CreateSwapChainForHwnd  28 CheckFeatureSupport
 *                       29 EnumAdapterByGpuPreference
 *   IDXGIAdapter1       10 GetDesc1
 *   IDXGISwapChain3     0 QueryInterface  8 Present  9 GetBuffer
 *                       36 GetCurrentBackBufferIndex
 *   y lo de P3c4        device 21 CreateDepthStencilView
 *                       25 GetResourceAllocationInfo; resource 10 GetDesc;
 *                       lista 47 ClearDepthStencilView
 *   la captura          device 38 GetCopyableFootprints; lista 16
 *                       CopyTextureRegion                                 */

#define BUFFERS 2

static void *disp, *cola, *asig, *lista, *cadena, *valla, *buffers[BUFFERS];
static void *raiz, *pso, *vb, *ib, *cb;
static unsigned int *constantes;
static U64 rtv[BUFFERS], dsv;
static void *profundidad;
/* La captura: el bufer READBACK (mapeado), su huella, y las cuentas. */
static void *leido;
static unsigned char *mapa_leido;
static COPY_LOCATION copia_destino, copia_origen;
static UINT huellas_malas, huellas_vistas;

/* bmo_cubo::referencia::HUELLAS: lo que D3D12 dibujo en la 3060. */
static const U64 HUELLA_3060[3] = {0xab7afc663a345885ULL, 0x2b3985e93e1a6574ULL, 0x8dc7ef10f691548eULL};
static HANDLE evento;
static U64 valor;
static UINT presentados, fotograma_actual;

/* Las estructuras grandes, fuera de la pila y a cero (sin CRT no hay memset). */
static PSO_DESC desc;
static ROOT_PARAMETER parametro;
static ROOT_SIGNATURE_DESC firma;
static VERTEX_BUFFER_VIEW vista_vb;
static INDEX_BUFFER_VIEW vista_ib;
static unsigned char desc_adaptador[312];

/* El HLSL de BMOX-12 (estudio-d3d/d3d12/src/cubo.hlsl de EPICX), byte a byte:
 * su huella es la de prueba/sombras/f3ef42a0 y 4d67f5e4 (lo que D3DCompile
 * busca en window/sombras). */
#include "cubo12_hlsl.h"

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
    void *blob, *error, *mapa, *vs, *ps;
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
    /* D3DCompile, como BMOX-12: sin macros, sin include, banderas 0. */
    if (D3DCompile(CUBO12_HLSL, sizeof CUBO12_HLSL, "cubo.hlsl", 0, 0, "VSMain", "vs_5_0", 0, 0, &vs, &error) < 0) ExitProcess(0xE220);
    if (D3DCompile(CUBO12_HLSL, sizeof CUBO12_HLSL, "cubo.hlsl", 0, 0, "PSMain", "ps_5_0", 0, 0, &ps, &error) < 0) ExitProcess(0xE221);
    desc.VS.pShaderBytecode = ((F_ptr)HUECO(vs, 3))(vs);
    desc.VS.BytecodeLength = ((F_u64)HUECO(vs, 4))(vs);
    desc.PS.pShaderBytecode = ((F_ptr)HUECO(ps, 3))(ps);
    desc.PS.BytecodeLength = ((F_u64)HUECO(ps, 4))(ps);
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
    desc.DepthStencilState.DepthEnable = 1;
    desc.DepthStencilState.DepthWriteMask = DEPTH_WRITE_ALL;
    desc.DepthStencilState.DepthFunc = COMPARISON_LESS;
    desc.DSVFormat = D32_FLOAT;
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

/* La huella de lo copiado (la valla ya paso): FNV-1a sobre los 4 bytes de
 * 0x00RRGGBB de cada pixel (B, G, R, 0), como bmo_cubo::referencia::huella. */
static void comparar_huella(void) {
    U64 h = 0xcbf29ce484222325ULL;
    UINT x, y, k = fotograma_actual / PASO;
    for (y = 0; y < CUBO_ALTO; y++) {
        const unsigned char *fila = mapa_leido + (U64)y * copia_destino.u.Huella.RowPitch;
        for (x = 0; x < CUBO_ANCHO; x++) {
            const unsigned char *p = fila + 4 * x; /* R8G8B8A8: R, G, B, A */
            h = (h ^ p[2]) * 0x100000001b3ULL;
            h = (h ^ p[1]) * 0x100000001b3ULL;
            h = (h ^ p[0]) * 0x100000001b3ULL;
            h = h * 0x100000001b3ULL; /* el cuarto byte, el ?? de 0x??RRGGBB: 0 */
        }
    }
    if (fotograma_actual % PASO == 0 && k < 3) {
        huellas_vistas++;
        if (h != HUELLA_3060[k]) huellas_malas++;
    }
}

/* Un fotograma: el cubo en su angulo, presentado, y esperar a la valla. */
static void fotograma(void) {
    UINT i = ((F_indice)HUECO(cadena, 36))(cadena);
    copiar(constantes, CUBO_CONSTANTES[fotograma_actual % FOTOGRAMAS], sizeof CUBO_CONSTANTES[0]);
    ((F_0)HUECO(asig, 8))(asig);
    ((F_reset)HUECO(lista, 10))(lista, asig, pso);
    ((F_poner)HUECO(lista, 30))(lista, raiz);
    ((F_cbv)HUECO(lista, 38))(lista, 0, va(cb));
    ((F_viewports)HUECO(lista, 21))(lista, 1, &VISTA);
    ((F_tijeras)HUECO(lista, 22))(lista, 1, &TIJERA);
    barrera(buffers[i], STATE_PRESENT, STATE_RENDER_TARGET);
    if (i >= BUFFERS) ExitProcess(0xE230);
    ((F_destinos)HUECO(lista, 46))(lista, 1, &rtv[i], 0, &dsv);
    ((F_limpiar)HUECO(lista, 48))(lista, rtv[i], FONDO, 0, 0);
    ((F_limpiar_z)HUECO(lista, 47))(lista, dsv, CLEAR_DEPTH, 1.0f, 0, 0, 0);
    ((F_uint)HUECO(lista, 20))(lista, TRIANGLELIST);
    ((F_vb)HUECO(lista, 44))(lista, 0, 1, &vista_vb);
    ((F_ib)HUECO(lista, 43))(lista, &vista_ib);
    ((F_dibujar)HUECO(lista, 13))(lista, 36, 1, 0, 0, 0);
    /* Como el --fotograma de BMOX-12: el back buffer al bufer READBACK. */
    barrera(buffers[i], STATE_RENDER_TARGET, STATE_COPY_SOURCE);
    copia_origen.pResource = buffers[i];
    ((F_copiar_region)HUECO(lista, 16))(lista, &copia_destino, 0, 0, 0, &copia_origen, 0);
    barrera(buffers[i], STATE_COPY_SOURCE, STATE_PRESENT);
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
    comparar_huella();
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
        PostQuitMessage((int)(presentados + 0x100 * (huellas_malas + 3 - (huellas_vistas < 3 ? huellas_vistas : 3))));
        return 0;
    }
    return DefWindowProcW(h, m, w, l);
}

static const WCHAR CLASE[] = L"BMOXCubo12";
static const WCHAR TITULO[] = L"PROTON-X P3c4";

void inicio(void) {
    WNDCLASSEXW wc;
    MSG msg;
    QUEUE_DESC qd;
    HEAP_DESC hd;
    SWAP_DESC1 sd;
    void *fabrica, *monton, *adaptador, *cadena1, *monton_z;
    int tearing;
    HEAP_PROPERTIES hp_z = {HEAP_TYPE_DEFAULT, 0, 0, 1, 1};
    RESOURCE_DESC rz, visto;
    U64 asignado[2];
    struct { UINT Format; float Depth; unsigned char Stencil; } limpio = {D32_FLOAT, 1.0f, 0};
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

    /* Como BMOX-12: la fabrica 6, el adaptador de alto rendimiento, su
     * descripcion, y el dispositivo SOBRE ese adaptador. */
    if (CreateDXGIFactory2(0, &IID_Factory6, &fabrica) < 0) ExitProcess(0xE102);
    if (((F_preferencia)HUECO(fabrica, 29))(fabrica, 0, GPU_ALTO_RENDIMIENTO, &IID_Adapter1, &adaptador) < 0) ExitProcess(0xE110);
    if (((F_desc1)HUECO(adaptador, 10))(adaptador, desc_adaptador) < 0 || desc_adaptador[0] == 0) ExitProcess(0xE111);
    if (D3D12CreateDevice(adaptador, 0xb000, &IID_Device, &disp) < 0) ExitProcess(0xE101);
    tearing = 7;
    if (((F_soporta)HUECO(fabrica, 28))(fabrica, 0, &tearing, sizeof tearing) < 0 || (tearing != 0 && tearing != 1)) ExitProcess(0xE112);
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
    if (((F_cadena)HUECO(fabrica, 15))(fabrica, cola, h, &sd, 0, 0, &cadena1) < 0) ExitProcess(0xE104);
    /* El `.cast()` de BMOX-12: la misma cadena, como IDXGISwapChain3. */
    if (((F_qi)HUECO(cadena1, 0))(cadena1, &IID_SwapChain3, &cadena) < 0) ExitProcess(0xE113);

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
    /* La captura: la huella del back buffer en un bufer, y el bufer READBACK. */
    {
        HEAP_PROPERTIES hp_l = {HEAP_TYPE_READBACK, 0, 0, 1, 1};
        RESOURCE_DESC rl;
        RANGE nada = {0, 0};
        U64 total = 0;
        ((F_getdesc)HUECO(buffers[0], 10))(buffers[0], &visto);
        ((F_huellas)HUECO(disp, 38))(disp, &visto, 0, 1, 0, &copia_destino.u.Huella, 0, 0, &total);
        if (total < (U64)CUBO_ANCHO * CUBO_ALTO * 4 || copia_destino.u.Huella.RowPitch < CUBO_ANCHO * 4) ExitProcess(0xE118);
        rl.Dimension = DIMENSION_BUFFER;
        rl.Alignment = 0;
        rl.Width = total;
        rl.Height = 1;
        rl.DepthOrArraySize = 1;
        rl.MipLevels = 1;
        rl.Format = 0;
        rl.SampleCount = 1;
        rl.SampleQuality = 0;
        rl.Layout = LAYOUT_ROW_MAJOR;
        rl.Flags = 0;
        if (((F_crear_recurso)HUECO(disp, 27))(disp, &hp_l, 0, &rl, STATE_COPY_DEST, 0, &IID_Resource, &leido) < 0) ExitProcess(0xE119);
        if (((F_map)HUECO(leido, 8))(leido, 0, &nada, (void **)&mapa_leido) < 0) ExitProcess(0xE11A);
        copia_destino.pResource = leido;
        copia_destino.Type = COPY_PLACED_FOOTPRINT;
        copia_origen.Type = 0; /* SUBRESOURCE_INDEX 0 */
        copia_origen.u.Indice = 0;
    }
    /* La profundidad: D32 de la medida de la cadena, su monton DSV y su vista. */
    rz.Dimension = DIMENSION_TEXTURE2D;
    rz.Alignment = 0;
    rz.Width = CUBO_ANCHO;
    rz.Height = CUBO_ALTO;
    rz.DepthOrArraySize = 1;
    rz.MipLevels = 1;
    rz.Format = D32_FLOAT;
    rz.SampleCount = 1;
    rz.SampleQuality = 0;
    rz.Layout = 0;
    rz.Flags = ALLOW_DEPTH_STENCIL;
    if (((F_crear_recurso)HUECO(disp, 27))(disp, &hp_z, 0, &rz, STATE_DEPTH_WRITE, &limpio, &IID_Resource, &profundidad) < 0) ExitProcess(0xE114);
    ((F_getdesc)HUECO(profundidad, 10))(profundidad, &visto);
    if (visto.Dimension != DIMENSION_TEXTURE2D || visto.Width != CUBO_ANCHO || visto.Height != CUBO_ALTO || visto.Format != D32_FLOAT) ExitProcess(0xE115);
    ((F_asignacion)HUECO(disp, 25))(disp, asignado, 0, 1, &rz);
    if (asignado[0] < (U64)CUBO_ANCHO * CUBO_ALTO * 4 || asignado[1] == 0) ExitProcess(0xE116);
    hd.Type = 3; /* D3D12_DESCRIPTOR_HEAP_TYPE_DSV */
    hd.NumDescriptors = 1;
    if (((F_crear4)HUECO(disp, 14))(disp, &hd, &IID_Heap, &monton_z) < 0) ExitProcess(0xE117);
    ((F_inicio)HUECO(monton_z, 9))(monton_z, &dsv);
    ((F_dsv)HUECO(disp, 21))(disp, profundidad, 0, dsv);
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
