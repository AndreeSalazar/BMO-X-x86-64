// limpieza.cpp -- el juez de ClearUnorderedAccessView (A2 del contador de
// docs/plan/PLAN_LAS_TRES_GRANDES.md 7.1, 06-10): el valor va en el formato
// de la VISTA (no en el del recurso), y solo dentro de sus RECTANGULOS. Sin
// sombreadores: limpiezas y copias, y todo bit a bit.
//
//   A  RGBA8_TYPELESS por una vista R32_UINT, Uint 0x11223344: esa palabra.
//   B  RGBA8_TYPELESS por una R8G8B8A8_SNORM, Float (-1, -0.25, 0.25, 1):
//      los bytes 81 E0 20 7F.
//   C  R10G10B10A2_TYPELESS por una R32_UINT, Uint 0xC00FFC01: esa palabra.
//   D  R16G16B16A16_TYPELESS por una R16G16B16A16_UINT, Uint (0x10001, 2,
//      3, 0x17BFF): cada uno SATURADO a 16 bits (0xFFFF, 2, 3, 0xFFFF). Lo
//      dijo Windows en la 3060 (06-10): la casa se quedaba con los bits bajos.
//   E  R16G16_FLOAT, Float (0.5, -2): los halfs 3800 y C000.
//   F  R32_UINT de 8 x 8: Uint 5 entera, y Uint 7 en los rectangulos
//      (1, 1)-(3, 4), (5, 0)-(8, 2) y uno vacio (6, 6)-(6, 8).
//   G  un bufer R16G16_UINT de 16 elementos, Uint (0x10005, 0x20006):
//      saturados, (0xFFFF, 0xFFFF), como D.
//   H  un array R32_UINT de 4 x 4 x 2 por la vista de sus dos capas: Uint 0
//      entera y Uint 9 en (0, 0)-(2, 2): en LAS DOS capas.
//
// Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

static int fallos = 0;

static void decir(bool bien, const char *que) {
    printf("%s%s\n", bien ? "  bien  " : "  MAL   ", que);
    if (!bien)
        fallos++;
}

static bool hecho(HRESULT h, const char *que) {
    if (FAILED(h)) {
        char m[160];
        snprintf(m, sizeof m, "%s: HRESULT 0x%08lx", que, (unsigned long)h);
        decir(false, m);
        return false;
    }
    return true;
}

static ID3D12Resource *recurso(ID3D12Device *d, D3D12_HEAP_TYPE monton, const D3D12_RESOURCE_DESC &r, D3D12_RESOURCE_STATES e) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
    ID3D12Resource *x = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&x)), "CreateCommittedResource");
    return x;
}

static D3D12_RESOURCE_DESC bufer(UINT64 bytes, D3D12_RESOURCE_FLAGS f) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    r.Width = bytes;
    r.Height = 1;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.SampleDesc.Count = 1;
    r.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    r.Flags = f;
    return r;
}

static D3D12_RESOURCE_DESC textura(UINT lado, UINT16 capas, DXGI_FORMAT f) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    r.Width = lado;
    r.Height = lado;
    r.DepthOrArraySize = capas;
    r.MipLevels = 1;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    return r;
}

static void barrera_uav(ID3D12GraphicsCommandList *l) {
    D3D12_RESOURCE_BARRIER b = {};
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_UAV;
    l->ResourceBarrier(1, &b);
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

// Las texturas que se leen, en este orden; cada subrecurso a su sitio de 4 KiB.
enum { T8A, T8B, T10, T16, TFL, TRECT, TARR0, TARR1, N_LEIDAS };
static const UINT64 SITIO = 4096, EN_BUFER = N_LEIDAS * SITIO, LEIDO = EN_BUFER + 4096;

int main() {
    ID3D12Device *d = nullptr;
    if (!hecho(D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d)), "D3D12CreateDevice"))
        return 1;
    D3D12_COMMAND_QUEUE_DESC qd = {};
    qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    ID3D12CommandQueue *cola = nullptr;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&cola)), "CreateCommandQueue");
    ID3D12CommandAllocator *al = nullptr;
    hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&al)), "CreateCommandAllocator");
    ID3D12Fence *valla = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&valla)), "CreateFence");

    const D3D12_RESOURCE_STATES UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS, CS = D3D12_RESOURCE_STATE_COPY_SOURCE;
    D3D12_RESOURCE_DESC desc[7] = {
        textura(4, 1, DXGI_FORMAT_R8G8B8A8_TYPELESS),
        textura(4, 1, DXGI_FORMAT_R8G8B8A8_TYPELESS),
        textura(4, 1, DXGI_FORMAT_R10G10B10A2_TYPELESS),
        textura(4, 1, DXGI_FORMAT_R16G16B16A16_TYPELESS),
        textura(4, 1, DXGI_FORMAT_R16G16_FLOAT),
        textura(8, 1, DXGI_FORMAT_R32_UINT),
        textura(4, 2, DXGI_FORMAT_R32_UINT),
    };
    ID3D12Resource *t[7];
    for (int k = 0; k < 7; k++)
        t[k] = recurso(d, D3D12_HEAP_TYPE_DEFAULT, desc[k], UA);
    ID3D12Resource *buf = recurso(d, D3D12_HEAP_TYPE_DEFAULT, bufer(64, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), UA);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, bufer(LEIDO, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;

    // Cada vista, en el monton visible (la direccion de GPU) y en uno de la
    // CPU (la que ClearUnorderedAccessView pide en uno no visible).
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 8, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hc = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 8, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *monton = nullptr, *cpu = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap") || !hecho(d->CreateDescriptorHeap(&hc, IID_PPV_ARGS(&cpu)), "CreateDescriptorHeap de la CPU"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0, c0;
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    monton->GetGPUDescriptorHandleForHeapStart(&g0);
    cpu->GetCPUDescriptorHandleForHeapStart(&c0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    auto ch = [&](UINT k) { D3D12_CPU_DESCRIPTOR_HANDLE h = {c0.ptr + k * paso}; return h; };
    auto gh = [&](UINT k) { D3D12_GPU_DESCRIPTOR_HANDLE h = {g0.ptr + k * (UINT64)paso}; return h; };
    auto vista = [&](ID3D12Resource *r, const D3D12_UNORDERED_ACCESS_VIEW_DESC &v, UINT k) {
        D3D12_CPU_DESCRIPTOR_HANDLE h = {h0.ptr + k * paso};
        d->CreateUnorderedAccessView(r, nullptr, &v, h);
        d->CreateUnorderedAccessView(r, nullptr, &v, ch(k));
    };
    auto plana = [&](ID3D12Resource *r, DXGI_FORMAT f, UINT k) {
        D3D12_UNORDERED_ACCESS_VIEW_DESC v = {};
        v.Format = f;
        v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
        vista(r, v, k);
    };
    plana(t[0], DXGI_FORMAT_R32_UINT, 0);
    plana(t[1], DXGI_FORMAT_R8G8B8A8_SNORM, 1);
    plana(t[2], DXGI_FORMAT_R32_UINT, 2);
    plana(t[3], DXGI_FORMAT_R16G16B16A16_UINT, 3);
    plana(t[4], DXGI_FORMAT_R16G16_FLOAT, 4);
    plana(t[5], DXGI_FORMAT_R32_UINT, 5);
    D3D12_UNORDERED_ACCESS_VIEW_DESC v = {};
    v.Format = DXGI_FORMAT_R32_UINT;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2DARRAY;
    v.Texture2DArray.ArraySize = 2;
    vista(t[6], v, 6);
    v = {};
    v.Format = DXGI_FORMAT_R16G16_UINT;
    v.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    v.Buffer.NumElements = 16;
    vista(buf, v, 7);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, nullptr, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetDescriptorHeaps(1, &monton);
    auto uint4 = [&](UINT k, ID3D12Resource *r, UINT a, UINT b, UINT c, UINT e, UINT n, const D3D12_RECT *rs) {
        const UINT x[4] = {a, b, c, e};
        l->ClearUnorderedAccessViewUint(gh(k), ch(k), r, x, n, rs);
    };
    uint4(0, t[0], 0x11223344, 0, 0, 0, 0, nullptr);
    const float menos[4] = {-1.0f, -0.25f, 0.25f, 1.0f}, rg[4] = {0.5f, -2.0f, 9.0f, 9.0f};
    l->ClearUnorderedAccessViewFloat(gh(1), ch(1), t[1], menos, 0, nullptr);
    uint4(2, t[2], 0xC00FFC01, 0, 0, 0, 0, nullptr);
    uint4(3, t[3], 0x10001, 2, 3, 0x17BFF, 0, nullptr);
    l->ClearUnorderedAccessViewFloat(gh(4), ch(4), t[4], rg, 0, nullptr);
    uint4(5, t[5], 5, 5, 5, 5, 0, nullptr);
    uint4(6, t[6], 0, 0, 0, 0, 0, nullptr);
    uint4(7, buf, 0x10005, 0x20006, 0, 0, 0, nullptr);
    barrera_uav(l);
    const D3D12_RECT rects[3] = {{1, 1, 3, 4}, {5, 0, 8, 2}, {6, 6, 6, 8}}, esquina = {0, 0, 2, 2};
    uint4(5, t[5], 7, 7, 7, 7, 3, rects);
    uint4(6, t[6], 9, 9, 9, 9, 1, &esquina);

    for (int k = 0; k < 7; k++)
        transicion(l, t[k], UA, CS);
    transicion(l, buf, UA, CS);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT h[N_LEIDAS];
    auto copiar = [&](ID3D12Resource *r, const D3D12_RESOURCE_DESC &dr, UINT sub, int donde) {
        d->GetCopyableFootprints(&dr, sub, 1, donde * SITIO, &h[donde], nullptr, nullptr, nullptr);
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint = h[donde];
        de.pResource = r;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        de.SubresourceIndex = sub;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    };
    for (int k = 0; k < 6; k++)
        copiar(t[k], desc[k], 0, k);
    copiar(t[6], desc[6], 0, TARR0);
    copiar(t[6], desc[6], 1, TARR1);
    l->CopyBufferRegion(leida, EN_BUFER, buf, 0, 64);
    hecho(l->Close(), "Close");
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    hecho(cola->Signal(valla, 1), "Signal");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        hecho(valla->SetEventOnCompletion(1, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 10000);
    }
    unsigned char *m = nullptr;
    if (!hecho(leida->Map(0, nullptr, (void **)&m), "Map de lo leido"))
        return fallos;

    // La palabra `w` (de 4 bytes) del texel (x, y) de lo leido en `donde`,
    // con texeles de `palabras` palabras.
    auto texel = [&](int donde, UINT x, UINT y, UINT w, UINT palabras = 1) {
        return *(const UINT *)(m + h[donde].Offset + (UINT64)y * h[donde].Footprint.RowPitch + 4 * palabras * x + 4 * w);
    };
    char msg[240];
    // Que cada texel de `lado` x `lado` tenga `palabras` palabras como pide `quiero`.
    auto juzgar = [&](const char *que, int donde, UINT lado, UINT palabras, auto quiero) {
        UINT malos = 0, mx = 0, my = 0, mw = 0;
        for (UINT y = 0; y < lado; y++)
            for (UINT x = 0; x < lado; x++)
                for (UINT w = 0; w < palabras; w++)
                    if (texel(donde, x, y, w, palabras) != quiero(x, y, w) && malos++ == 0)
                        mx = x, my = y, mw = w;
        if (malos == 0)
            snprintf(msg, sizeof msg, "%s: bit a bit", que);
        else
            snprintf(msg, sizeof msg, "%s: %u palabras distintas; la %u del (%u, %u) es %08x y tenia que ser %08x", que, malos, mw, mx, my, texel(donde, mx, my, mw, palabras), quiero(mx, my, mw));
        decir(malos == 0, msg);
    };
    juzgar("A, RGBA8_TYPELESS limpio por una vista R32_UINT", T8A, 4, 1, [](UINT, UINT, UINT) { return 0x11223344u; });
    juzgar("B, RGBA8_TYPELESS limpio por una R8G8B8A8_SNORM (81 E0 20 7F)", T8B, 4, 1, [](UINT, UINT, UINT) { return 0x7F20E081u; });
    juzgar("C, R10G10B10A2_TYPELESS limpio por una R32_UINT", T10, 4, 1, [](UINT, UINT, UINT) { return 0xC00FFC01u; });
    juzgar("D, R16G16B16A16_TYPELESS por una UINT: cada valor saturado a 16 bits", T16, 4, 2, [](UINT, UINT, UINT w) { return w == 0 ? 0x0002FFFFu : 0xFFFF0003u; });
    juzgar("E, R16G16_FLOAT con Float (0.5, -2): los halfs", TFL, 4, 1, [](UINT, UINT, UINT) { return 0xC0003800u; });
    juzgar("F, R32_UINT de 8 x 8: 7 solo en sus dos rectangulos (el vacio no limpia nada)", TRECT, 8, 1, [](UINT x, UINT y, UINT) {
        bool dentro = (x >= 1 && x < 3 && y >= 1 && y < 4) || (x >= 5 && x < 8 && y < 2);
        return dentro ? 7u : 5u;
    });
    const UINT *b = (const UINT *)(m + EN_BUFER);
    UINT malos = 0;
    for (UINT i = 0; i < 16; i++)
        malos += b[i] != 0xFFFFFFFFu;
    snprintf(msg, sizeof msg, "G, un bufer R16G16_UINT con Uint (0x10005, 0x20006): saturados, (0xFFFF, 0xFFFF) en %u de 16 (el 0 es %08x)", 16 - malos, b[0]);
    decir(malos == 0, msg);
    malos = 0;
    for (int donde = TARR0; donde <= TARR1; donde++)
        for (UINT y = 0; y < 4; y++)
            for (UINT x = 0; x < 4; x++)
                malos += texel(donde, x, y, 0) != ((x < 2 && y < 2) ? 9u : 0u);
    snprintf(msg, sizeof msg, "H, un array de dos capas con un rectangulo: 9 en la esquina de LAS DOS (%u texeles distintos)", malos);
    decir(malos == 0, msg);

    printf("limpieza.exe: ClearUnorderedAccessView es el de Windows\n");
    return fallos;
}
