// multihilo.cpp -- el juez de E2.1 de la ESCALERA de PROTON-X (05-10):
// listas de ordenes GRABADAS DESDE VARIOS HILOS, y la superficie de colas y
// vallas que usa Cyberpunk. La muestra de Microsoft (D3D12Multithreading)
// pide SquidRoom.bin (43 MB, que no se mete: 7.1 de la ESCALERA); este es
// NUESTRO, de consola, con los sombreadores de multihilo.hlsl dentro.
//
//   A  CUATRO HILOS (CreateThread), cada uno con SU allocator y SU lista,
//      graban A LA VEZ en el MISMO render target de 64 x 64 (su franja, y
//      una tira comun abajo) y en un mapa de profundidad (un pase de solo Z,
//      el de un mapa de sombras). Se paran a mitad (un evento) para que las
//      cuatro listas esten abiertas a la vez y se graben a turnos; lo que
//      pusieron antes de pararse (la raiz, las constantes) sigue ahi
//      despues. Dos rondas: las seis listas en UN ExecuteCommandLists, y en
//      SEIS, en otro orden.
//   B  Tres COLAS (directa, de computo y de copia) con vallas: la de computo
//      y la de copia ESPERAN en la GPU (Wait) a un valor que la directa da
//      despues; una cola que espera a una valla que marca la CPU, con su
//      lista reiniciada y vuelta a grabar mientras tanto;
//      SetEventOnCompletion sin evento (espera ahi; otro hilo marca); y
//      SetEventOnMultipleFenceCompletion con ALL y ANY.
//   C  Las reglas de las listas y los allocators: Reset de una lista con un
//      allocator que graba otra (E_INVALIDARG), Reset del allocator con su
//      lista abierta (E_FAIL), Reset de una lista abierta (E_FAIL), Close
//      dos veces (E_FAIL), y lo que si se puede: el allocator de una lista
//      cerrada, tras su valla, y una lista ejecutada dos veces.
//
// Lo que tiene que salir, en bits escritos a mano abajo. Donde D3D no da
// orden (los hilos al grabar), el juez no lo pide: cada hilo su franja, y en
// la Z comun gana la menor sea cual sea el orden. Donde si lo da (las listas
// de una cola van en el orden en que se mandan), lo pide: en la tira comun
// gana el color de la ULTIMA lista mandada. Sale con el numero de fallos; en
// Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_cuadro\n vs_cuadro:\n .incbin \"multihilo_vs.dxil\"\n .globl vs_cuadro_fin\n vs_cuadro_fin:\n"
        ".p2align 4\n"
        ".globl ps_color\n ps_color:\n .incbin \"multihilo_ps.dxil\"\n .globl ps_color_fin\n ps_color_fin:\n"
        ".p2align 4\n"
        ".globl cs_doble\n cs_doble:\n .incbin \"multihilo_cs.dxil\"\n .globl cs_doble_fin\n cs_doble_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_cuadro[], vs_cuadro_fin[], ps_color[], ps_color_fin[], cs_doble[], cs_doble_fin[];

static const UINT LADO = 64;
static const UINT HILOS = 4;
// Las franjas de los hilos (x de 16 en 16) van de y = 0 a 40; de 40 a 48
// queda lo limpio; de 48 a 64, la tira que pintan todos.
static const LONG FRANJA = 40, TIRA = 48;
static const UINT N = 64; // los u32 de los buferes de B y C
// Lo que la CPU espera a una valla o a sus eventos, en ms. Con -DPLAZO=0 el
// juez no se queda esperando contra una casa cuyas colas no esperan (asi se
// probo que dice NO: ver HACER.txt); el .exe de verdad lleva 10 s.
#ifndef PLAZO
#define PLAZO 10000
#endif

static volatile LONG fallos = 0;

static void decir(bool bien, const char *que) {
    printf("%s%s\n", bien ? "  bien  " : "  MAL   ", que);
    if (!bien)
        InterlockedIncrement(&fallos);
}

static bool hecho(HRESULT h, const char *que) {
    if (FAILED(h)) {
        char m[200];
        snprintf(m, sizeof m, "%s: HRESULT 0x%08lx", que, (unsigned long)h);
        decir(false, m);
        return false;
    }
    return true;
}

// Lo comun: lo crea main y lo usan los hilos (el dispositivo es libre de
// hilos en D3D12; cada lista y su allocator, de uno solo).
static ID3D12Device *d;
static ID3D12RootSignature *rs;
static ID3D12PipelineState *pso_color, *pso_z;
static D3D12_CPU_DESCRIPTOR_HANDLE rtv, dsv;

// Colores (R, G, B, A) que caben exactos en 8 bits: 0, 1, 0.25 (63.75 ->
// 64) y 0.75 (191.25 -> 191). En memoria R8G8B8A8: R en el byte bajo.
static const float COLOR[2][HILOS][4] = {
    {{1, 0, 0, 1}, {0, 1, 0, 1}, {0, 0, 1, 1}, {1, 1, 0, 1}},
    {{0.25f, 0, 0, 0.25f}, {0, 0.25f, 0, 0.25f}, {0, 0, 0.75f, 0.25f}, {0.75f, 0.75f, 0, 0.25f}},
};
static const UINT PIXEL[2][HILOS] = {
    {0xFF0000FFu, 0xFF00FF00u, 0xFFFF0000u, 0xFF00FFFFu},
    {0x40000040u, 0x40004000u, 0x40BF0000u, 0x4000BFBFu},
};
// Lo limpio: (0.75, 0.25, 0, 1) -> BF 40 00 FF.
static const float LIMPIO[4] = {0.75f, 0.25f, 0, 1};
static const UINT PIXEL_LIMPIO = 0xFF0040BFu;
// La Z de cada hilo: potencias de 2 (un float exacto, y el viewport de 0 a
// 1 la deja igual). En la tira gana la MENOR (LESS): 0.125 y 0.0625.
static const float Z[2][HILOS] = {{0.5f, 0.25f, 0.75f, 0.125f}, {0.375f, 0.0625f, 0.25f, 0.5f}};
static const float Z_TIRA[2] = {0.125f, 0.0625f};

struct Obra {
    UINT i, ronda;
    ID3D12CommandAllocator *a;
    ID3D12GraphicsCommandList *l;
    HANDLE medio, seguir, listo;
};

static void tijera(ID3D12GraphicsCommandList *l, LONG x0, LONG y0, LONG x1, LONG y1) {
    D3D12_RECT r = {x0, y0, x1, y1};
    l->RSSetScissorRects(1, &r);
}

// Un hilo de A: su allocator y su lista (nuevos en la ronda 0; en la 1,
// Reset de los dos tras la valla), la mitad de lo suyo, parar, y el resto.
static DWORD WINAPI grabar(LPVOID p) {
    Obra *o = (Obra *)p;
    char m[120];
    if (o->ronda == 0) {
        snprintf(m, sizeof m, "hilo %u: CreateCommandAllocator", o->i);
        if (!hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&o->a)), m))
            return 1;
        snprintf(m, sizeof m, "hilo %u: CreateCommandList", o->i);
        if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, o->a, pso_color, IID_PPV_ARGS(&o->l)), m))
            return 1;
    } else {
        snprintf(m, sizeof m, "hilo %u: Reset del allocator, cerrada su lista y pasada su valla", o->i);
        hecho(o->a->Reset(), m);
        snprintf(m, sizeof m, "hilo %u: Reset de la lista", o->i);
        hecho(o->l->Reset(o->a, pso_color), m);
    }
    ID3D12GraphicsCommandList *l = o->l;
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    float k[5];
    memcpy(k, COLOR[o->ronda][o->i], 16);
    k[4] = 0.5f;
    l->SetGraphicsRootSignature(rs);
    l->RSSetViewports(1, &vp);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    l->OMSetRenderTargets(1, &rtv, FALSE, nullptr);
    l->SetGraphicsRoot32BitConstants(0, 5, k, 0);
    tijera(l, 16 * o->i, 0, 16 * o->i + 16, FRANJA);
    l->DrawInstanced(6, 1, 0, 0);
    // A mitad: las cuatro listas abiertas a la vez; main dice cuando seguir.
    SetEvent(o->medio);
    WaitForSingleObject(o->seguir, INFINITE);
    // La tira comun, con la raiz y las constantes de ANTES de parar.
    tijera(l, 0, TIRA, LADO, LADO);
    l->DrawInstanced(6, 1, 0, 0);
    // El pase de solo Z (un mapa de sombras): su franja y la tira.
    k[4] = Z[o->ronda][o->i];
    l->SetPipelineState(pso_z);
    l->OMSetRenderTargets(0, nullptr, FALSE, &dsv);
    l->SetGraphicsRoot32BitConstants(0, 5, k, 0);
    tijera(l, 16 * o->i, 0, 16 * o->i + 16, FRANJA);
    l->DrawInstanced(6, 1, 0, 0);
    tijera(l, 0, TIRA, LADO, LADO);
    l->DrawInstanced(6, 1, 0, 0);
    snprintf(m, sizeof m, "hilo %u: Close", o->i);
    hecho(l->Close(), m);
    SetEvent(o->listo);
    return 0;
}

static ID3D12Resource *bufer(D3D12_HEAP_TYPE monton, UINT64 bytes, D3D12_RESOURCE_FLAGS f, D3D12_RESOURCE_STATES e) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    r.Width = bytes;
    r.Height = 1;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.SampleDesc.Count = 1;
    r.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    r.Flags = f;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource de un bufer");
    return b;
}

static ID3D12Resource *textura(DXGI_FORMAT f, D3D12_RESOURCE_FLAGS banderas, D3D12_RESOURCE_STATES e, const D3D12_CLEAR_VALUE *cv) {
    D3D12_HEAP_PROPERTIES hp = {};
    hp.Type = D3D12_HEAP_TYPE_DEFAULT;
    D3D12_RESOURCE_DESC td = {};
    td.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    td.Width = LADO;
    td.Height = LADO;
    td.DepthOrArraySize = 1;
    td.MipLevels = 1;
    td.Format = f;
    td.SampleDesc.Count = 1;
    td.Flags = banderas;
    ID3D12Resource *t = nullptr;
    hecho(d->CreateCommittedResource(&hp, D3D12_HEAP_FLAG_NONE, &td, e, cv, IID_PPV_ARGS(&t)), "CreateCommittedResource de una textura");
    return t;
}

static ID3D12PipelineState *pso(bool color) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS.pShaderBytecode = vs_cuadro;
    p.VS.BytecodeLength = (SIZE_T)(vs_cuadro_fin - vs_cuadro);
    if (color) {
        p.PS.pShaderBytecode = ps_color;
        p.PS.BytecodeLength = (SIZE_T)(ps_color_fin - ps_color);
        p.NumRenderTargets = 1;
        p.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
    } else {
        p.DepthStencilState.DepthEnable = TRUE;
        p.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ALL;
        p.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_LESS;
        p.DSVFormat = DXGI_FORMAT_D32_FLOAT;
    }
    p.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    p.SampleDesc.Count = 1;
    ID3D12PipelineState *x = nullptr;
    hecho(d->CreateGraphicsPipelineState(&p, IID_PPV_ARGS(&x)), "CreateGraphicsPipelineState");
    return x;
}

static void transicion(ID3D12GraphicsCommandList *l, ID3D12Resource *r, D3D12_RESOURCE_STATES de, D3D12_RESOURCE_STATES a) {
    D3D12_RESOURCE_BARRIER b = {};
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
    b.Transition.pResource = r;
    b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
    b.Transition.StateBefore = de;
    b.Transition.StateAfter = a;
    l->ResourceBarrier(1, &b);
}

// Una textura de 64 x 64 a un bufer, con la huella que da el dispositivo.
static void copiar(ID3D12GraphicsCommandList *l, ID3D12Resource *t, ID3D12Resource *b) {
    D3D12_RESOURCE_DESC rd;
    t->GetDesc(&rd);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT h = {};
    d->GetCopyableFootprints(&rd, 0, 1, 0, &h, nullptr, nullptr, nullptr);
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = b;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint = h;
    de.pResource = t;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    de.SubresourceIndex = 0;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
}

// Esperar en la CPU a que `v` llegue a `x` (con su evento).
static void esperar(ID3D12Fence *v, UINT64 x, HANDLE ev) {
    if (v->GetCompletedValue() < x) {
        hecho(v->SetEventOnCompletion(x, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, PLAZO);
    }
}

static const void *mapa(ID3D12Resource *b) {
    void *p = nullptr;
    hecho(b->Map(0, nullptr, &p), "Map");
    return p;
}

static void escribir(ID3D12Resource *b, UINT base) {
    void *p = nullptr;
    D3D12_RANGE nada = {0, 0};
    if (!hecho(b->Map(0, &nada, &p), "Map"))
        return;
    for (UINT i = 0; i < N; i++)
        ((UINT *)p)[i] = base + i;
    b->Unmap(0, nullptr);
}

// Los N u32 de `p` contra `quiero(i)`.
template <typename Q> static void juzgar_u32(const UINT *p, Q quiero, const char *que) {
    UINT malos = 0, primero = 0;
    for (UINT i = 0; i < N; i++)
        if (p[i] != quiero(i) && malos++ == 0)
            primero = i;
    char m[240];
    if (malos == 0)
        snprintf(m, sizeof m, "%s", que);
    else
        snprintf(m, sizeof m, "%s: %u distintos; el %u es %u y tenia que ser %u", que, malos, primero, p[primero], quiero(primero));
    decir(malos == 0, m);
}

// A, una ronda: los cuatro hilos graban, main manda y lee.
static void ronda(UINT r, ID3D12CommandQueue *cola, ID3D12Fence *valla, HANDLE ev, ID3D12CommandAllocator *am, ID3D12GraphicsCommandList **lm, ID3D12GraphicsCommandList **lr, Obra *obras, ID3D12Resource *rt, ID3D12Resource *z, ID3D12Resource *leer_rt, ID3D12Resource *leer_z) {
    HANDLE hilos[HILOS], medios[HILOS], listos[HILOS];
    for (UINT i = 0; i < HILOS; i++) {
        obras[i].i = i;
        obras[i].ronda = r;
        medios[i] = obras[i].medio = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        obras[i].seguir = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        listos[i] = obras[i].listo = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        hilos[i] = CreateThread(nullptr, 0, grabar, &obras[i], 0, nullptr);
    }
    // Mientras, main graba las suyas: limpiar, y leer lo pintado.
    if (r == 0) {
        hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, am, nullptr, IID_PPV_ARGS(lm)), "CreateCommandList de main");
        ID3D12CommandAllocator *al = nullptr;
        hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&al)), "CreateCommandAllocator de leer");
        hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, nullptr, IID_PPV_ARGS(lr)), "CreateCommandList de leer");
        transicion(*lr, rt, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
        transicion(*lr, z, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_COPY_SOURCE);
        copiar(*lr, rt, leer_rt);
        copiar(*lr, z, leer_z);
        transicion(*lr, rt, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET);
        transicion(*lr, z, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE);
        hecho((*lr)->Close(), "Close de leer");
    } else {
        hecho(am->Reset(), "Reset del allocator de main tras la valla");
        hecho((*lm)->Reset(am, nullptr), "Reset de la lista de main");
    }
    (*lm)->ClearRenderTargetView(rtv, LIMPIO, 0, nullptr);
    (*lm)->ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH, 1.0f, 0, 0, nullptr);
    hecho((*lm)->Close(), "Close de main");

    char m[200];
    DWORD w = WaitForMultipleObjects(HILOS, medios, TRUE, 10000);
    snprintf(m, sizeof m, "A%u: los cuatro hilos, a mitad de grabar, con sus cuatro listas abiertas a la vez", r);
    decir(w < WAIT_OBJECT_0 + HILOS, m);
    for (UINT i = HILOS; i-- > 0;)
        SetEvent(obras[i].seguir);
    w = WaitForMultipleObjects(HILOS, listos, TRUE, 10000);
    WaitForMultipleObjects(HILOS, hilos, TRUE, 10000);
    if (w >= WAIT_OBJECT_0 + HILOS) {
        decir(false, "los hilos no acabaron de grabar");
        return;
    }

    // El orden de las listas: ronda 0, todas en UNA llamada (3, 1, 0, 2);
    // ronda 1, una llamada por lista (0, 2, 3, 1). La tira es de la ultima.
    static const UINT ORDEN[2][HILOS] = {{3, 1, 0, 2}, {0, 2, 3, 1}};
    ID3D12CommandList *ls[HILOS + 2];
    ls[0] = *lm;
    for (UINT k = 0; k < HILOS; k++)
        ls[k + 1] = obras[ORDEN[r][k]].l;
    ls[HILOS + 1] = *lr;
    if (r == 0) {
        cola->ExecuteCommandLists(HILOS + 2, ls);
    } else {
        for (UINT k = 0; k < HILOS + 2; k++)
            cola->ExecuteCommandLists(1, &ls[k]);
    }
    hecho(cola->Signal(valla, r + 1), "Signal de A");
    esperar(valla, r + 1, ev);

    const UINT ultimo = ORDEN[r][HILOS - 1];
    const UINT *px = (const UINT *)mapa(leer_rt);
    const float *pz = (const float *)mapa(leer_z);
    if (!px || !pz)
        return;
    UINT malos = 0, primero = 0, quise = 0;
    for (UINT y = 0; y < LADO; y++)
        for (UINT x = 0; x < LADO; x++) {
            UINT q = y < (UINT)FRANJA ? PIXEL[r][x / 16] : y < (UINT)TIRA ? PIXEL_LIMPIO : PIXEL[r][ultimo];
            if (px[y * LADO + x] != q && malos++ == 0) {
                primero = y * LADO + x;
                quise = q;
            }
        }
    if (malos == 0)
        snprintf(m, sizeof m, "A%u: el color, cada hilo su franja y la tira del ultimo mandado (el %u), en %s", r, ultimo, r == 0 ? "UN ExecuteCommandLists" : "seis");
    else
        snprintf(m, sizeof m, "A%u: el color: %u pixeles distintos; el (%u, %u) es %08x y tenia que ser %08x", r, malos, primero % LADO, primero / LADO, px[primero], quise);
    decir(malos == 0, m);
    malos = 0;
    float zq = 0;
    for (UINT y = 0; y < LADO; y++)
        for (UINT x = 0; x < LADO; x++) {
            float q = y < (UINT)FRANJA ? Z[r][x / 16] : y < (UINT)TIRA ? 1.0f : Z_TIRA[r];
            if (memcmp(&pz[y * LADO + x], &q, 4) != 0 && malos++ == 0) {
                primero = y * LADO + x;
                zq = q;
            }
        }
    if (malos == 0)
        snprintf(m, sizeof m, "A%u: la Z (solo profundidad), cada hilo la suya y en la tira la menor, %g, bit a bit", r, Z_TIRA[r]);
    else
        snprintf(m, sizeof m, "A%u: la Z: %u texeles distintos; el (%u, %u) es %g y tenia que ser %g", r, malos, primero % LADO, primero / LADO, pz[primero], zq);
    decir(malos == 0, m);
    leer_rt->Unmap(0, nullptr);
    leer_z->Unmap(0, nullptr);
    for (UINT i = 0; i < HILOS; i++) {
        CloseHandle(hilos[i]);
        CloseHandle(obras[i].medio);
        CloseHandle(obras[i].seguir);
        CloseHandle(obras[i].listo);
    }
}

// B3: otro hilo marca, desde la CPU, la valla a la que espera la cola.
static ID3D12Fence *marcar_k;
static DWORD WINAPI marcar(LPVOID) {
    Sleep(20);
    hecho(marcar_k->Signal(1), "Signal desde otro hilo");
    return 0;
}

static ID3D12GraphicsCommandList *lista(D3D12_COMMAND_LIST_TYPE t, ID3D12CommandAllocator **a, ID3D12PipelineState *p) {
    ID3D12GraphicsCommandList *l = nullptr;
    hecho(d->CreateCommandAllocator(t, IID_PPV_ARGS(a)), "CreateCommandAllocator");
    hecho(d->CreateCommandList(0, t, *a, p, IID_PPV_ARGS(&l)), "CreateCommandList");
    return l;
}

int main() {
    if (!hecho(D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d)), "D3D12CreateDevice"))
        return 1;
    D3D12_COMMAND_QUEUE_DESC qd = {};
    ID3D12CommandQueue *directa = nullptr, *de_computo = nullptr, *de_copia = nullptr;
    qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&directa)), "CreateCommandQueue DIRECT");
    qd.Type = D3D12_COMMAND_LIST_TYPE_COMPUTE;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&de_computo)), "CreateCommandQueue COMPUTE");
    qd.Type = D3D12_COMMAND_LIST_TYPE_COPY;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&de_copia)), "CreateCommandQueue COPY");
    ID3D12Fence *valla = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&valla)), "CreateFence");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);

    // La raiz de dibujo: cinco constantes (b0: el color y la z).
    D3D12_ROOT_PARAMETER pr = {};
    pr.ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    pr.Constants.Num32BitValues = 5;
    pr.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_SIGNATURE_DESC rd = {1, &pr, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    // La de computo: un UAV en la raiz (u0).
    D3D12_ROOT_PARAMETER pu = {};
    pu.ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    pu.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_SIGNATURE_DESC ru = {1, &pu, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3D12RootSignature *rsc = nullptr;
    ID3DBlob *firma = nullptr, *error = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (hecho(D3D12SerializeRootSignature(&ru, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature de computo"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rsc)), "CreateRootSignature de computo");
    if (fallos)
        return fallos;
    pso_color = pso(true);
    pso_z = pso(false);
    D3D12_COMPUTE_PIPELINE_STATE_DESC cd = {};
    cd.pRootSignature = rsc;
    cd.CS.pShaderBytecode = cs_doble;
    cd.CS.BytecodeLength = (SIZE_T)(cs_doble_fin - cs_doble);
    ID3D12PipelineState *pso_cs = nullptr;
    hecho(d->CreateComputePipelineState(&cd, IID_PPV_ARGS(&pso_cs)), "CreateComputePipelineState");

    // -- A: los cuatro hilos ----------------------------------------------
    D3D12_CLEAR_VALUE cz = {};
    cz.Format = DXGI_FORMAT_D32_FLOAT;
    cz.DepthStencil.Depth = 1.0f;
    ID3D12Resource *rt = textura(DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr);
    ID3D12Resource *z = textura(DXGI_FORMAT_D32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, D3D12_RESOURCE_STATE_DEPTH_WRITE, &cz);
    ID3D12Resource *leer_rt = bufer(D3D12_HEAP_TYPE_READBACK, LADO * LADO * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    ID3D12Resource *leer_z = bufer(D3D12_HEAP_TYPE_READBACK, LADO * LADO * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hz = {D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *mr = nullptr, *mz = nullptr;
    hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&mr)), "CreateDescriptorHeap RTV");
    hecho(d->CreateDescriptorHeap(&hz, IID_PPV_ARGS(&mz)), "CreateDescriptorHeap DSV");
    if (fallos)
        return fallos;
    mr->GetCPUDescriptorHandleForHeapStart(&rtv);
    mz->GetCPUDescriptorHandleForHeapStart(&dsv);
    d->CreateRenderTargetView(rt, nullptr, rtv);
    d->CreateDepthStencilView(z, nullptr, dsv);
    ID3D12CommandAllocator *am = nullptr;
    hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&am)), "CreateCommandAllocator de main");
    ID3D12GraphicsCommandList *lm = nullptr, *lr = nullptr;
    static Obra obras[HILOS];
    ronda(0, directa, valla, ev, am, &lm, &lr, obras, rt, z, leer_rt, leer_z);
    ronda(1, directa, valla, ev, am, &lm, &lr, obras, rt, z, leer_rt, leer_z);

    // -- B: las tres colas -------------------------------------------------
    // B1: la de computo espera (en la GPU) a 1 y la de copia a 2; la directa
    // llena el bufer y da el 1 DESPUES. Sale (1000 + i) * 2 + i.
    ID3D12Resource *sube = bufer(D3D12_HEAP_TYPE_UPLOAD, N * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *datos = bufer(D3D12_HEAP_TYPE_DEFAULT, N * 4, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COMMON);
    ID3D12Resource *leer = bufer(D3D12_HEAP_TYPE_READBACK, N * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    ID3D12Fence *v1 = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&v1)), "CreateFence de B1");
    if (!sube || !datos || !leer || !v1)
        return fallos;
    escribir(sube, 1000);
    ID3D12CommandAllocator *a_llena, *a_cs, *a_copia;
    ID3D12GraphicsCommandList *llena = lista(D3D12_COMMAND_LIST_TYPE_DIRECT, &a_llena, nullptr);
    ID3D12GraphicsCommandList *dobla = lista(D3D12_COMMAND_LIST_TYPE_COMPUTE, &a_cs, pso_cs);
    ID3D12GraphicsCommandList *copia = lista(D3D12_COMMAND_LIST_TYPE_COPY, &a_copia, nullptr);
    if (!llena || !dobla || !copia)
        return fallos;
    llena->CopyBufferRegion(datos, 0, sube, 0, N * 4);
    hecho(llena->Close(), "Close de llenar");
    dobla->SetComputeRootSignature(rsc);
    dobla->SetComputeRootUnorderedAccessView(0, datos->GetGPUVirtualAddress());
    dobla->Dispatch(1, 1, 1);
    hecho(dobla->Close(), "Close de doblar");
    copia->CopyBufferRegion(leer, 0, datos, 0, N * 4);
    hecho(copia->Close(), "Close de copiar");
    ID3D12CommandList *x[1] = {dobla};
    hecho(de_computo->Wait(v1, 1), "Wait de la de computo");
    de_computo->ExecuteCommandLists(1, x);
    hecho(de_computo->Signal(v1, 2), "Signal de la de computo");
    hecho(de_copia->Wait(v1, 2), "Wait de la de copia");
    x[0] = copia;
    de_copia->ExecuteCommandLists(1, x);
    hecho(de_copia->Signal(v1, 3), "Signal de la de copia");
    decir(v1->GetCompletedValue() == 0, "B1: con la de computo y la de copia esperando en la GPU, la valla sigue en 0");
    x[0] = llena;
    directa->ExecuteCommandLists(1, x);
    hecho(directa->Signal(v1, 1), "Signal de la directa");
    esperar(v1, 3, ev);
    decir(v1->GetCompletedValue() == 3, "B1: la valla llega a 3: directa, de computo y de copia, en ese orden");
    if (const UINT *p = (const UINT *)mapa(leer)) {
        juzgar_u32(p, [](UINT i) { return (1000 + i) * 2 + i; }, "B1: la de computo doblo lo que lleno la directa, y la de copia lo leyo: (1000 + i) * 2 + i");
        leer->Unmap(0, nullptr);
    }

    // B2: la directa espera a una valla que marca la CPU; mientras, su
    // lista se reinicia y se vuelve a grabar (D3D12 lo deja: lo mandado ya
    // es de la cola), y la CPU escribe lo que se va a copiar.
    ID3D12Resource *sube2 = bufer(D3D12_HEAP_TYPE_UPLOAD, N * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *leer2 = bufer(D3D12_HEAP_TYPE_READBACK, 2 * N * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    ID3D12Fence *g = nullptr, *v2 = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&g)), "CreateFence de la CPU");
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&v2)), "CreateFence de B2");
    if (!sube2 || !leer2 || !g || !v2)
        return fallos;
    escribir(sube2, 5000);
    ID3D12CommandAllocator *a2;
    ID3D12GraphicsCommandList *l2 = lista(D3D12_COMMAND_LIST_TYPE_DIRECT, &a2, nullptr);
    l2->CopyBufferRegion(leer2, 0, sube2, 0, N * 4);
    hecho(l2->Close(), "Close de B2");
    hecho(directa->Wait(g, 5), "Wait de la directa a la CPU");
    x[0] = l2;
    directa->ExecuteCommandLists(1, x);
    hecho(l2->Reset(a2, nullptr), "B2: Reset de la lista mandada, con la cola aun esperando");
    l2->CopyBufferRegion(leer2, N * 4, sube2, 0, N * 4);
    hecho(l2->Close(), "Close de B2, otra vez");
    directa->ExecuteCommandLists(1, x);
    hecho(directa->Signal(v2, 1), "Signal de B2");
    decir(v2->GetCompletedValue() == 0, "B2: la directa espera a la CPU: su valla sigue en 0");
    escribir(sube2, 7000);
    hecho(g->Signal(5), "Signal de la CPU");
    esperar(v2, 1, ev);
    if (const UINT *p = (const UINT *)mapa(leer2)) {
        juzgar_u32(p, [](UINT i) { return 7000 + i; }, "B2: la primera lista, lo que escribio la CPU ANTES de dejar pasar a la cola: 7000 + i");
        juzgar_u32(p + N, [](UINT i) { return 7000 + i; }, "B2: la misma lista, reiniciada y grabada otra vez: lo suyo, no lo de antes");
        leer2->Unmap(0, nullptr);
    }

    // B3: SetEventOnCompletion SIN evento espera ahi; la deja seguir otro
    // hilo, que marca desde la CPU la valla a la que espera la directa.
    ID3D12Fence *v3 = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&marcar_k)), "CreateFence de B3");
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&v3)), "CreateFence de B3");
    if (!marcar_k || !v3)
        return fallos;
    hecho(directa->Wait(marcar_k, 1), "Wait de B3");
    hecho(directa->Signal(v3, 1), "Signal de B3");
    HANDLE h = CreateThread(nullptr, 0, marcar, nullptr, 0, nullptr);
    // La CPU espera a la valla que marca el otro hilo; la cola, detras.
    HRESULT r3 = marcar_k->SetEventOnCompletion(1, nullptr);
    UINT64 k3 = marcar_k->GetCompletedValue();
    esperar(v3, 1, ev);
    decir(r3 == S_OK && k3 == 1 && v3->GetCompletedValue() == 1, "B3: SetEventOnCompletion sin evento espera hasta que otro hilo marca la valla, y la cola que la esperaba sigue");
    WaitForSingleObject(h, 10000);
    CloseHandle(h);

    // B4: SetEventOnMultipleFenceCompletion, ALL y ANY, con dos vallas que
    // marcan la de computo y la de copia cuando la CPU les deja.
    ID3D12Device1 *d1 = nullptr;
    if (hecho(d->QueryInterface(IID_PPV_ARGS(&d1)), "QueryInterface de ID3D12Device1")) {
        ID3D12Fence *f[2] = {}, *g2 = nullptr;
        hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&f[0])), "CreateFence de B4");
        hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&f[1])), "CreateFence de B4");
        hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&g2)), "CreateFence de B4");
        if (!f[0] || !f[1] || !g2)
            return fallos;
        hecho(de_computo->Wait(g2, 1), "Wait de B4");
        hecho(de_computo->Signal(f[0], 1), "Signal de B4");
        hecho(de_copia->Wait(g2, 2), "Wait de B4");
        hecho(de_copia->Signal(f[1], 1), "Signal de B4");
        const UINT64 uno[2] = {1, 1};
        HANDLE todas = CreateEventW(nullptr, FALSE, FALSE, nullptr), alguna = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        hecho(d1->SetEventOnMultipleFenceCompletion(f, uno, 2, D3D12_MULTIPLE_FENCE_WAIT_FLAG_ALL, todas), "SetEventOnMultipleFenceCompletion ALL");
        hecho(d1->SetEventOnMultipleFenceCompletion(f, uno, 2, D3D12_MULTIPLE_FENCE_WAIT_FLAG_ANY, alguna), "SetEventOnMultipleFenceCompletion ANY");
        bool antes = WaitForSingleObject(todas, 0) == WAIT_TIMEOUT && WaitForSingleObject(alguna, 0) == WAIT_TIMEOUT;
        hecho(g2->Signal(1), "Signal de B4");
        bool una = WaitForSingleObject(alguna, PLAZO) == WAIT_OBJECT_0 && WaitForSingleObject(todas, 0) == WAIT_TIMEOUT;
        hecho(g2->Signal(2), "Signal de B4");
        bool dos = WaitForSingleObject(todas, PLAZO) == WAIT_OBJECT_0;
        decir(antes && una && dos, "B4: SetEventOnMultipleFenceCompletion: ANY con la primera valla, ALL con las dos, y ninguno antes");
        d1->Release();
    }

    // -- C: las reglas de las listas y los allocators -----------------------
    ID3D12CommandAllocator *ax, *ay;
    ID3D12GraphicsCommandList *lx = lista(D3D12_COMMAND_LIST_TYPE_DIRECT, &ax, nullptr);
    ID3D12GraphicsCommandList *ly = lista(D3D12_COMMAND_LIST_TYPE_DIRECT, &ay, nullptr);
    if (!lx || !ly)
        return fallos;
    hecho(ly->Close(), "Close de C");
    HRESULT c1 = ly->Reset(ax, nullptr);
    HRESULT c2 = ax->Reset();
    HRESULT c3 = lx->Reset(ay, nullptr);
    HRESULT c4 = lx->Close();
    HRESULT c5 = lx->Close();
    HRESULT c6 = ax->Reset();
    HRESULT c7 = ly->Reset(ax, nullptr);
    char m[240];
    snprintf(m, sizeof m, "C: Reset con un allocator que graba otra %08lx, Reset del allocator %08lx, Reset de una abierta %08lx, Close dos veces %08lx",
             (unsigned long)c1, (unsigned long)c2, (unsigned long)c3, (unsigned long)c5);
    decir(c1 == E_INVALIDARG && c2 == E_FAIL && c3 == E_FAIL && c4 == S_OK && c5 == E_FAIL, m);
    decir(c6 == S_OK && c7 == S_OK && ly->Close() == S_OK, "C: cerrada su lista, el allocator se reinicia, y otra lista lo toma");

    // C2: una lista de computo EJECUTADA DOS VECES (con su valla en medio):
    // (1000 + i) * 4 + 3i.
    ID3D12Resource *datos4 = bufer(D3D12_HEAP_TYPE_DEFAULT, N * 4, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COMMON);
    ID3D12Resource *leer4 = bufer(D3D12_HEAP_TYPE_READBACK, N * 4, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    ID3D12Fence *v4 = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&v4)), "CreateFence de C2");
    if (!datos4 || !leer4 || !v4)
        return fallos;
    ID3D12CommandAllocator *a4, *a4c, *a4l;
    ID3D12GraphicsCommandList *l4 = lista(D3D12_COMMAND_LIST_TYPE_DIRECT, &a4, nullptr);
    ID3D12GraphicsCommandList *l4c = lista(D3D12_COMMAND_LIST_TYPE_COMPUTE, &a4c, pso_cs);
    ID3D12GraphicsCommandList *l4l = lista(D3D12_COMMAND_LIST_TYPE_DIRECT, &a4l, nullptr);
    l4->CopyBufferRegion(datos4, 0, sube, 0, N * 4);
    hecho(l4->Close(), "Close de C2");
    l4c->SetComputeRootSignature(rsc);
    l4c->SetComputeRootUnorderedAccessView(0, datos4->GetGPUVirtualAddress());
    l4c->Dispatch(1, 1, 1);
    hecho(l4c->Close(), "Close de C2");
    l4l->CopyBufferRegion(leer4, 0, datos4, 0, N * 4);
    hecho(l4l->Close(), "Close de C2");
    x[0] = l4;
    directa->ExecuteCommandLists(1, x);
    hecho(directa->Signal(v4, 1), "Signal de C2");
    hecho(de_computo->Wait(v4, 1), "Wait de C2");
    x[0] = l4c;
    de_computo->ExecuteCommandLists(1, x);
    hecho(de_computo->Signal(v4, 2), "Signal de C2");
    esperar(v4, 2, ev);
    de_computo->ExecuteCommandLists(1, x);
    hecho(de_computo->Signal(v4, 3), "Signal de C2");
    hecho(directa->Wait(v4, 3), "Wait de C2");
    x[0] = l4l;
    directa->ExecuteCommandLists(1, x);
    hecho(directa->Signal(v4, 4), "Signal de C2");
    esperar(v4, 4, ev);
    if (const UINT *p = (const UINT *)mapa(leer4)) {
        juzgar_u32(p, [](UINT i) { return (1000 + i) * 4 + 3 * i; }, "C2: la misma lista de computo ejecutada dos veces: (1000 + i) * 4 + 3i");
        leer4->Unmap(0, nullptr);
    }

    printf("multihilo.exe: las listas de varios hilos y las colas con vallas son las de Windows\n");
    return fallos;
}
