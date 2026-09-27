/* limpia.c -- el .exe de P3a de PROTON-X: D3D12 y DXGI LIMPIAN una ventana.
 *
 * El esqueleto de todo programa D3D12, sin sombreadores: D3D12CreateDevice,
 * CreateDXGIFactory2, la cola, la cadena de intercambio sobre la ventana de
 * P2, un monton de RTV con un descriptor por back buffer, el asignador, la
 * lista, la valla y su evento. Cada fotograma: barrera PRESENT ->
 * RENDER_TARGET, ClearRenderTargetView, barrera de vuelta, Close,
 * ExecuteCommandLists, Present, Signal y esperar a la valla.
 *
 * Sin d3d12.h: los metodos se llaman por su HUECO en la vtabla, que es lo que
 * el codigo compilado contra d3d12.h hace de verdad. Los numeros son los del
 * orden de la cabecera de Windows (ver la tabla de abajo).
 *
 *    una letra   el color siguiente (se limpia y se presenta otra vez)
 *    q o ESC     cierra
 *
 * Sale con el numero de Present que se hicieron. En Windows corre igual. */
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

#define WM_DESTROY 0x0002
#define WM_KEYDOWN 0x0100
#define WM_CHAR 0x0102
#define VK_ESCAPE 0x1B
#define WS_OVERLAPPEDWINDOW 0x00CF0000
#define SW_SHOW 5
#define INFINITE 0xFFFFFFFF
#define R8G8B8A8_UNORM 28
#define USAGE_RENDER_TARGET_OUTPUT 0x20
#define SWAP_EFFECT_FLIP_DISCARD 4
#define STATE_PRESENT 0
#define STATE_RENDER_TARGET 4

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
IMPORTA HRESULT WINAPI CreateDXGIFactory2(UINT banderas, const GUID *riid, void **pp);

static const GUID IID_Device = {0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID IID_Queue = {0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID IID_Allocator = {0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID IID_List = {0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID IID_Heap = {0x8efb471d, 0x616c, 0x4f49, {0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51}};
static const GUID IID_Resource = {0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID IID_Fence = {0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID IID_Factory2 = {0x50c83a1c, 0xe072, 0x4c48, {0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0}};

/* El metodo del hueco `h` de un objeto COM. */
#define HUECO(obj, h) ((*(void ***)(obj))[h])
typedef HRESULT(__stdcall *F_crear4)(void *, const void *, const GUID *, void **);
typedef HRESULT(__stdcall *F_crear_asig)(void *, int, const GUID *, void **);
typedef HRESULT(__stdcall *F_crear_lista)(void *, UINT, int, void *, void *, const GUID *, void **);
typedef UINT(__stdcall *F_incremento)(void *, int);
typedef void(__stdcall *F_rtv)(void *, void *, const void *, U64);
typedef HRESULT(__stdcall *F_valla)(void *, U64, int, const GUID *, void **);
typedef U64 *(__stdcall *F_inicio)(void *, U64 *);
typedef HRESULT(__stdcall *F_cadena)(void *, void *, HANDLE, const SWAP_DESC1 *, const void *, void *, void **);
typedef HRESULT(__stdcall *F_buffer)(void *, UINT, const GUID *, void **);
typedef HRESULT(__stdcall *F_0)(void *);
typedef HRESULT(__stdcall *F_reset)(void *, void *, void *);
typedef void(__stdcall *F_barrera)(void *, UINT, const BARRIER *);
typedef void(__stdcall *F_limpiar)(void *, U64, const float *, UINT, const void *);
typedef void(__stdcall *F_ejecutar)(void *, UINT, void **);
typedef HRESULT(__stdcall *F_signal)(void *, void *, U64);
typedef U64(__stdcall *F_valor)(void *);
typedef HRESULT(__stdcall *F_evento)(void *, U64, HANDLE);
typedef HRESULT(__stdcall *F_present)(void *, UINT, UINT);

/*   ID3D12Device        8 CreateCommandQueue   9 CreateCommandAllocator
 *                       12 CreateCommandList   14 CreateDescriptorHeap
 *                       15 GetDescriptorHandleIncrementSize
 *                       20 CreateRenderTargetView   36 CreateFence
 *   ID3D12DescriptorHeap 9 GetCPUDescriptorHandleForHeapStart
 *   ID3D12CommandAllocator 8 Reset
 *   ID3D12GraphicsCommandList 9 Close 10 Reset 26 ResourceBarrier
 *                             48 ClearRenderTargetView
 *   ID3D12CommandQueue  10 ExecuteCommandLists  14 Signal
 *   ID3D12Fence         8 GetCompletedValue  9 SetEventOnCompletion
 *   IDXGIFactory2       15 CreateSwapChainForHwnd
 *   IDXGISwapChain1     8 Present  9 GetBuffer                            */

#define ANCHO 320
#define ALTO 200
#define BUFFERS 2

static void *cola, *asig, *lista, *cadena, *valla, *buffers[BUFFERS];
static U64 rtv[BUFFERS];
static HANDLE evento;
static U64 valor;
static UINT presentados, color;

static const float COLORES[3][4] = {
    {0.0f, 0.25f, 0.75f, 1.0f},
    {0.75f, 0.25f, 0.0f, 1.0f},
    {0.25f, 0.75f, 0.25f, 1.0f},
};

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

/* Un fotograma: limpiar el back buffer actual y presentarlo, y esperar. */
static void fotograma(void) {
    UINT i = presentados % BUFFERS;
    ((F_0)HUECO(asig, 8))(asig);
    ((F_reset)HUECO(lista, 10))(lista, asig, 0);
    barrera(buffers[i], STATE_PRESENT, STATE_RENDER_TARGET);
    ((F_limpiar)HUECO(lista, 48))(lista, rtv[i], COLORES[color % 3], 0, 0);
    barrera(buffers[i], STATE_RENDER_TARGET, STATE_PRESENT);
    ((F_0)HUECO(lista, 9))(lista);
    ((F_ejecutar)HUECO(cola, 10))(cola, 1, &lista);
    ((F_present)HUECO(cadena, 8))(cadena, 1, 0);
    presentados++;
    valor++;
    ((F_signal)HUECO(cola, 14))(cola, valla, valor);
    if (((F_valor)HUECO(valla, 8))(valla) < valor) {
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
        color++;
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

static const WCHAR CLASE[] = L"BMOXLimpia";
static const WCHAR TITULO[] = L"PROTON-X P3a";

void inicio(void) {
    WNDCLASSEXW wc;
    MSG msg;
    QUEUE_DESC qd;
    HEAP_DESC hd;
    SWAP_DESC1 sd;
    void *disp, *fabrica, *monton;
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
    h = CreateWindowExW(0, CLASE, TITULO, WS_OVERLAPPEDWINDOW, 0, 0, ANCHO, ALTO, 0, 0, inst, 0);
    if (!h) ExitProcess(0xE002);

    if (D3D12CreateDevice(0, 0xb000, &IID_Device, &disp) < 0) ExitProcess(0xE101);
    if (CreateDXGIFactory2(0, &IID_Factory2, &fabrica) < 0) ExitProcess(0xE102);
    qd.Type = 0;
    qd.Priority = 0;
    qd.Flags = 0;
    qd.NodeMask = 0;
    if (((F_crear4)HUECO(disp, 8))(disp, &qd, &IID_Queue, &cola) < 0) ExitProcess(0xE103);

    sd.Width = ANCHO;
    sd.Height = ALTO;
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
    if (((F_crear_lista)HUECO(disp, 12))(disp, 0, 0, asig, 0, &IID_List, &lista) < 0) ExitProcess(0xE108);
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
