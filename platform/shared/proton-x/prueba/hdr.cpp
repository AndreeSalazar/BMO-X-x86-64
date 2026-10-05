// hdr.cpp -- el juez de los render targets de FLOAT (N5.16 de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): lo que hace cualquier juego
// con HDR -- pintar la escena en float, donde la luz pasa de 1, y llevarla a
// 8 bits al final. Tres destinos de 64 x 64 con los sombreadores de hdr.hlsl
// (aqui al lado), un cuadro de pantalla completa sin bufer de vertices:
//
//   A  R16G16B16A16_FLOAT: ClearRenderTargetView a (0.25, 0.5, 2.0, 1) y DOS
//      dibujos SUMANDO (ONE, ONE) el color (1.5, 0.25, -0.5, 0). Sale
//      (3.25, 1.0, 1.0, 1.0): lo que pasa de 1 y lo que resta se guardan.
//   B  R11G11B10_FLOAT: limpio a (0.125, 0.25, 0.375) y, con la tijera en la
//      mitad izquierda, el color (100, -1, 6.5): sin signo, el -1 es 0.
//   C  A LEIDO como textura (Load) por cuatro, a un R8G8B8A8_UNORM:
//      (0.8125, 0.25, 0.25, 0.25) = (207, 64, 64, 64).
//
// Cada valor cabe EXACTO en su formato (sin redondeo que discutir), y lo
// que tiene que salir esta escrito abajo con sus bits, sacado de las reglas
// de D3D (el formato de half, de float11 y float10, y el de UNORM). Sale con
// el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_cuadro\n vs_cuadro:\n .incbin \"hdr_vs.dxil\"\n .globl vs_cuadro_fin\n vs_cuadro_fin:\n"
        ".p2align 4\n"
        ".globl ps_color\n ps_color:\n .incbin \"hdr_color.dxil\"\n .globl ps_color_fin\n ps_color_fin:\n"
        ".p2align 4\n"
        ".globl ps_lee\n ps_lee:\n .incbin \"hdr_lee.dxil\"\n .globl ps_lee_fin\n ps_lee_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_cuadro[], vs_cuadro_fin[], ps_color[], ps_color_fin[], ps_lee[], ps_lee_fin[];

static const UINT LADO = 64;
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

static ID3D12Resource *leida(ID3D12Device *d, UINT64 bytes) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = D3D12_HEAP_TYPE_READBACK;
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    r.Width = bytes;
    r.Height = 1;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.SampleDesc.Count = 1;
    r.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, D3D12_RESOURCE_STATE_COPY_DEST, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource de lo leido");
    return b;
}

static ID3D12Resource *destino(ID3D12Device *d, DXGI_FORMAT f) {
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
    td.Flags = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    ID3D12Resource *t = nullptr;
    hecho(d->CreateCommittedResource(&hp, D3D12_HEAP_FLAG_NONE, &td, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr, IID_PPV_ARGS(&t)), "CreateCommittedResource del destino");
    return t;
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *ps, const unsigned char *ps_fin, DXGI_FORMAT f, bool suma) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS.pShaderBytecode = vs_cuadro;
    p.VS.BytecodeLength = (SIZE_T)(vs_cuadro_fin - vs_cuadro);
    p.PS.pShaderBytecode = ps;
    p.PS.BytecodeLength = (SIZE_T)(ps_fin - ps);
    D3D12_RENDER_TARGET_BLEND_DESC &m = p.BlendState.RenderTarget[0];
    m.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    if (suma) {
        m.BlendEnable = TRUE;
        m.SrcBlend = m.DestBlend = m.SrcBlendAlpha = m.DestBlendAlpha = D3D12_BLEND_ONE;
        m.BlendOp = m.BlendOpAlpha = D3D12_BLEND_OP_ADD;
    }
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    p.NumRenderTargets = 1;
    p.RTVFormats[0] = f;
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

static void copiar(ID3D12GraphicsCommandList *l, ID3D12Resource *t, ID3D12Resource *b, DXGI_FORMAT f, UINT bytes) {
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = b;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Footprint = {f, LADO, LADO, 1, LADO * bytes};
    de.pResource = t;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    de.SubresourceIndex = 0;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
}

// Las palabras de cada texel de un destino leido, comparadas con lo que
// tiene que salir (`quiero(x)` da las de la columna x).
template <typename T, typename Q> static void juzgar(ID3D12Resource *b, UINT palabras, Q quiero, const char *bien) {
    void *p = nullptr;
    if (!hecho(b->Map(0, nullptr, &p), "Map de lo leido"))
        return;
    const T *v = (const T *)p;
    UINT malos = 0, primero = 0;
    for (UINT i = 0; i < LADO * LADO; i++) {
        const T *q = quiero(i % LADO);
        if (memcmp(v + i * palabras, q, palabras * sizeof(T)) != 0 && malos++ == 0)
            primero = i;
    }
    char m[240];
    if (malos == 0) {
        snprintf(m, sizeof m, "%s", bien);
    } else {
        const T *q = quiero(primero % LADO);
        snprintf(m, sizeof m, "%s: %u texeles distintos; el (%u, %u) empieza %08lx y tenia que ser %08lx", bien, malos, primero % LADO, primero / LADO,
                 (unsigned long)v[primero * palabras], (unsigned long)q[0]);
    }
    decir(malos == 0, m);
    b->Unmap(0, nullptr);
}

// Lo que tiene que salir, en bits.
// A: halfs de (3.25, 1, 1, 1). 3.25 = 1.625 * 2^1: exponente 1 + 15 = 16,
// mantisa 0.625 * 1024 = 640 -> 0x4000 | 0x280; 1.0 es 0x3C00.
static const unsigned short A[4] = {0x4280, 0x3C00, 0x3C00, 0x3C00};
// B: R en los bits 0-10 (float11: 5 de exponente con sesgo 15 y 6 de
// mantisa), G en los 11-21 (float11) y B en los 22-31 (float10: 5 y 5).
//   limpio  R 0.125 = 2^-3 -> e 12 -> 0x300; G 0.25 = 2^-2 -> e 13 -> 0x340;
//           B 0.375 = 1.5 * 2^-2 -> e 13, m 16 -> 0x1B0
//   dibujo  R 100 = 1.5625 * 2^6 -> e 21, m 36 -> 0x564; G -1 -> 0 (sin
//           signo: lo negativo es 0); B 6.5 = 1.625 * 2^2 -> e 17, m 20 -> 0x234
static const UINT B_LIMPIO[1] = {0x300u | 0x340u << 11 | 0x1B0u << 22};
static const UINT B_DIBUJO[1] = {0x564u | 0x234u << 22};
// C: (0.8125, 0.25, 0.25, 0.25) en UNORM: 207.1875 -> 207 y 63.75 -> 64.
static const UINT C[1] = {207u | 64u << 8 | 64u << 16 | 64u << 24};

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

    // La raiz: cuatro constantes (b0) y una tabla con el SRV (t0).
    D3D12_DESCRIPTOR_RANGE rango = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 0, 0, 0};
    D3D12_ROOT_PARAMETER ps[2] = {};
    ps[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    ps[0].Constants.Num32BitValues = 4;
    ps[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    ps[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[1].DescriptorTable.NumDescriptorRanges = 1;
    ps[1].DescriptorTable.pDescriptorRanges = &rango;
    ps[1].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_ROOT_SIGNATURE_DESC rd = {2, ps, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;

    ID3D12PipelineState *pa = pso(d, rs, ps_color, ps_color_fin, DXGI_FORMAT_R16G16B16A16_FLOAT, true);
    ID3D12PipelineState *pb = pso(d, rs, ps_color, ps_color_fin, DXGI_FORMAT_R11G11B10_FLOAT, false);
    ID3D12PipelineState *pc = pso(d, rs, ps_lee, ps_lee_fin, DXGI_FORMAT_R8G8B8A8_UNORM, false);
    ID3D12Resource *ta = destino(d, DXGI_FORMAT_R16G16B16A16_FLOAT), *tb = destino(d, DXGI_FORMAT_R11G11B10_FLOAT), *tc = destino(d, DXGI_FORMAT_R8G8B8A8_UNORM);
    ID3D12Resource *la = leida(d, LADO * LADO * 8), *lb = leida(d, LADO * LADO * 4), *lc = leida(d, LADO * LADO * 4);
    if (fallos)
        return fallos;
    decir(true, "tres destinos (RGBA16F, R11G11B10F y RGBA8) y sus tres PSO");

    // Los descriptores: tres RTV y el SRV de A, en un monton visible.
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 3, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr, *srvs = nullptr;
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV");
    D3D12_DESCRIPTOR_HEAP_DESC sd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    hecho(d->CreateDescriptorHeap(&sd, IID_PPV_ARGS(&srvs)), "CreateDescriptorHeap del SRV");
    if (fallos)
        return fallos;
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
    D3D12_CPU_DESCRIPTOR_HANDLE rtv[3];
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv[0]);
    rtv[1].ptr = rtv[0].ptr + paso;
    rtv[2].ptr = rtv[0].ptr + 2 * paso;
    d->CreateRenderTargetView(ta, nullptr, rtv[0]);
    d->CreateRenderTargetView(tb, nullptr, rtv[1]);
    d->CreateRenderTargetView(tc, nullptr, rtv[2]);
    D3D12_CPU_DESCRIPTOR_HANDLE srv;
    D3D12_GPU_DESCRIPTOR_HANDLE srv_gpu;
    srvs->GetCPUDescriptorHandleForHeapStart(&srv);
    srvs->GetGPUDescriptorHandleForHeapStart(&srv_gpu);
    d->CreateShaderResourceView(ta, nullptr, srv);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pa, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    D3D12_RECT todo = {0, 0, (LONG)LADO, (LONG)LADO}, mitad = {0, 0, (LONG)LADO / 2, (LONG)LADO};
    l->RSSetViewports(1, &vp);
    l->RSSetScissorRects(1, &todo);
    l->SetGraphicsRootSignature(rs);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // A: limpiar en float y sumar dos veces.
    const float limpio_a[4] = {0.25f, 0.5f, 2.0f, 1.0f}, suma[4] = {1.5f, 0.25f, -0.5f, 0.0f};
    l->ClearRenderTargetView(rtv[0], limpio_a, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[0], FALSE, nullptr);
    l->SetGraphicsRoot32BitConstants(0, 4, suma, 0);
    l->DrawInstanced(6, 1, 0, 0);
    l->DrawInstanced(6, 1, 0, 0);
    // B: R11G11B10, la mitad izquierda dibujada.
    const float limpio_b[4] = {0.125f, 0.25f, 0.375f, 1.0f}, color_b[4] = {100.0f, -1.0f, 6.5f, 0.0f};
    l->ClearRenderTargetView(rtv[1], limpio_b, 0, nullptr);
    l->SetPipelineState(pb);
    l->OMSetRenderTargets(1, &rtv[1], FALSE, nullptr);
    l->RSSetScissorRects(1, &mitad);
    l->SetGraphicsRoot32BitConstants(0, 4, color_b, 0);
    l->DrawInstanced(6, 1, 0, 0);
    // C: A leido como textura, a 8 bits.
    transicion(l, ta, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    l->SetPipelineState(pc);
    l->OMSetRenderTargets(1, &rtv[2], FALSE, nullptr);
    l->RSSetScissorRects(1, &todo);
    l->SetDescriptorHeaps(1, &srvs);
    l->SetGraphicsRootDescriptorTable(1, srv_gpu);
    l->DrawInstanced(6, 1, 0, 0);
    // Leer los tres.
    transicion(l, ta, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_SOURCE);
    transicion(l, tb, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
    transicion(l, tc, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
    copiar(l, ta, la, DXGI_FORMAT_R16G16B16A16_FLOAT, 8);
    copiar(l, tb, lb, DXGI_FORMAT_R11G11B10_FLOAT, 4);
    copiar(l, tc, lc, DXGI_FORMAT_R8G8B8A8_UNORM, 4);
    hecho(l->Close(), "Close");
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    hecho(cola->Signal(valla, 1), "Signal");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        hecho(valla->SetEventOnCompletion(1, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 10000);
    }

    juzgar<unsigned short>(la, 4, [](UINT) { return A; }, "A, RGBA16F: limpio en float y dos sumas: (3.25, 1, 1, 1) en cada texel, half a half");
    juzgar<UINT>(lb, 1, [](UINT x) { return x < LADO / 2 ? B_DIBUJO : B_LIMPIO; }, "B, R11G11B10F: (100, 0, 6.5) a la izquierda y (0.125, 0.25, 0.375) a la derecha, bit a bit");
    juzgar<UINT>(lc, 1, [](UINT) { return C; }, "C, A leido como textura por cuatro a RGBA8: (207, 64, 64, 64), lo de mas de 1 se guardo");
    printf("hdr.exe: los render targets de float son los de Windows\n");
    return fallos;
}
