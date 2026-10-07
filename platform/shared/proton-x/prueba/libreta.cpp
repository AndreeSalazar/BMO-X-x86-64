// libreta.cpp -- el juez de LO RARO en un render target (06-10, 9d del
// contador de DX12), con los sombreadores de libreta.hlsl (aqui al lado).
// Un render target RGBA8_UNORM de 1280 x 720 (el de la cadena: lo que la
// puerta de la 3060 dibuja), limpio a azul, y dos dibujos con el MISMO PSO:
//
//   A  NORMAL, la mitad izquierda: k = (1, 0) -> (1, 0, -1, 1) -> rojo.
//   B  RARO, el cuarto de arriba a la derecha: k = (1e30, 0) -> (+inf, NaN,
//      -inf, 1). D3D: +inf se guarda 1, NaN 0 y -inf 0: rojo tambien.
//   C  el cuarto de abajo a la derecha: nadie pinta, azul.
//   D  la imagen entera, pixel a pixel.
//
// En Windows y en BMO-X dice lo mismo. En el metal de BMO-X, ademas, el
// dibujo B (si va a la 3060) lo apunta la LIBRETA: la consola lo dice
// ("la libreta de la 3060 apunto algo raro"). Sale con el numero de fallos.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(vs_pasa, "libreta_vs.dxil") DXIL(ps_raro, "libreta_ps.dxil") ".text\n");
extern "C" const unsigned char vs_pasa[], vs_pasa_fin[], ps_raro[], ps_raro_fin[];

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

static const UINT W = 1280, H = 720;

static ID3D12Resource *recurso(ID3D12Device *d, D3D12_HEAP_TYPE monton, D3D12_RESOURCE_DIMENSION dim, UINT64 ancho, UINT alto, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = dim;
    r.Width = ancho;
    r.Height = alto;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Layout = dim == D3D12_RESOURCE_DIMENSION_BUFFER ? D3D12_TEXTURE_LAYOUT_ROW_MAJOR : D3D12_TEXTURE_LAYOUT_UNKNOWN;
    r.Flags = fl;
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
    ID3D12Resource *x = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&x)), "CreateCommittedResource");
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

// Un vertice: la posicion (float3) y el color (float4).
struct Vertice {
    float x, y, z, r, g, b, a;
};

// Un cuadro de dos triangulos de (x0, y0) a (x1, y1), blanco.
static void cuadro(Vertice *v, float x0, float y0, float x1, float y1) {
    const float p[6][2] = {{x0, y0}, {x1, y0}, {x0, y1}, {x0, y1}, {x1, y0}, {x1, y1}};
    for (int k = 0; k < 6; k++)
        v[k] = {p[k][0], p[k][1], 0.5f, 1, 1, 1, 1};
}

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

    // La raiz: cuatro constantes (b0: k), para el de pixeles.
    D3D12_ROOT_PARAMETER rp[1] = {};
    rp[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    rp[0].Constants.Num32BitValues = 4;
    rp[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_ROOT_SIGNATURE_DESC rd = {1, rp, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;
    const D3D12_INPUT_ELEMENT_DESC ia[2] = {
        {"POSITION", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
        {"COLOR", 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
    };
    D3D12_GRAPHICS_PIPELINE_STATE_DESC pd = {};
    pd.pRootSignature = rs;
    pd.VS = {vs_pasa, (SIZE_T)(vs_pasa_fin - vs_pasa)};
    pd.PS = {ps_raro, (SIZE_T)(ps_raro_fin - ps_raro)};
    pd.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    pd.SampleMask = UINT_MAX;
    pd.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    pd.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    pd.RasterizerState.DepthClipEnable = TRUE;
    pd.InputLayout = {ia, 2};
    pd.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    pd.NumRenderTargets = 1;
    pd.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
    pd.SampleDesc.Count = 1;
    ID3D12PipelineState *pso = nullptr;
    if (!hecho(d->CreateGraphicsPipelineState(&pd, IID_PPV_ARGS(&pso)), "CreateGraphicsPipelineState"))
        return fallos;

    // Los vertices: A (la mitad izquierda) y B (arriba a la derecha).
    Vertice v[12];
    cuadro(v, -1, -1, 0, 1);
    cuadro(v + 6, 0, 0, 1, 1);
    ID3D12Resource *vb = recurso(d, D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_DIMENSION_BUFFER, sizeof v, 1, DXGI_FORMAT_UNKNOWN, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    const D3D12_RESOURCE_STATES RT = D3D12_RESOURCE_STATE_RENDER_TARGET, CS = D3D12_RESOURCE_STATE_COPY_SOURCE;
    ID3D12Resource *rt = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, W, H, DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, RT);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, D3D12_RESOURCE_DIMENSION_BUFFER, (UINT64)W * H * 4, 1, DXGI_FORMAT_UNKNOWN, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    void *pv = nullptr;
    if (!hecho(vb->Map(0, nullptr, &pv), "Map de los vertices"))
        return fallos;
    memcpy(pv, v, sizeof v);
    vb->Unmap(0, nullptr);
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE vr;
    rtvs->GetCPUDescriptorHandleForHeapStart(&vr);
    d->CreateRenderTargetView(rt, nullptr, vr);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pso, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    const FLOAT azul[4] = {0, 0, 1, 1};
    l->ClearRenderTargetView(vr, azul, 0, nullptr);
    l->SetGraphicsRootSignature(rs);
    D3D12_VIEWPORT vp = {0, 0, (FLOAT)W, (FLOAT)H, 0, 1};
    D3D12_RECT tijera = {0, 0, (LONG)W, (LONG)H};
    l->RSSetViewports(1, &vp);
    l->RSSetScissorRects(1, &tijera);
    l->OMSetRenderTargets(1, &vr, FALSE, nullptr);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    D3D12_VERTEX_BUFFER_VIEW vbv = {vb->GetGPUVirtualAddress(), (UINT)sizeof v, (UINT)sizeof(Vertice)};
    l->IASetVertexBuffers(0, 1, &vbv);
    const float normal[4] = {1, 0, 0, 0}, raro[4] = {1e30f, 0, 0, 0};
    l->SetGraphicsRoot32BitConstants(0, 4, normal, 0);
    l->DrawInstanced(6, 1, 0, 0);
    l->SetGraphicsRoot32BitConstants(0, 4, raro, 0);
    l->DrawInstanced(6, 1, 6, 0);
    transicion(l, rt, RT, CS);
    D3D12_RESOURCE_DESC dt;
    rt->GetDesc(&dt);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT h;
    d->GetCopyableFootprints(&dt, 0, 1, 0, &h, nullptr, nullptr, nullptr);
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = leida;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint = h;
    de.pResource = rt;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
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
    auto pixel = [&](UINT x, UINT y) { return *(const UINT *)(m + (UINT64)y * h.Footprint.RowPitch + 4 * x); };
    // Rojo opaco (R, G, B, A en memoria) y el azul de la limpieza.
    const UINT ROJO = 0xFF0000FFu, AZUL = 0xFFFF0000u;
    char msg[200];
    snprintf(msg, sizeof msg, "A, el dibujo normal (1, 0, -1, 1): el pixel (320, 360) es %08x (rojo, ff0000ff)", pixel(320, 360));
    decir(pixel(320, 360) == ROJO, msg);
    snprintf(msg, sizeof msg, "B, el dibujo RARO (+inf, NaN, -inf, 1): el pixel (960, 180) es %08x (+inf a 1, NaN y -inf a 0: rojo)", pixel(960, 180));
    decir(pixel(960, 180) == ROJO, msg);
    snprintf(msg, sizeof msg, "C, donde nadie pinta: el pixel (960, 540) es %08x (el azul de la limpieza, ffff0000)", pixel(960, 540));
    decir(pixel(960, 540) == AZUL, msg);
    UINT malos = 0;
    for (UINT y = 0; y < H; y++)
        for (UINT x = 0; x < W; x++)
            malos += pixel(x, y) != (x < W / 2 || y < H / 2 ? ROJO : AZUL);
    snprintf(msg, sizeof msg, "D, la imagen entera: %u pixeles distintos de lo que pide D3D (de %u)", malos, W * H);
    decir(malos == 0, msg);

    printf("libreta.exe: lo raro de un render target se guarda como en Windows\n");
    return fallos;
}
