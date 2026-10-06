// restos.cpp -- el juez de lo que QUEDABA de D3D12 tras N5.3d, N5.12b y
// N5.16b (05-10; las casillas de docs/plan/PLAN_LAS_TRES_GRANDES.md y la
// tabla 7.2 de PLAN_LA_ESCALERA_PROTON_X.md). Destinos de 8 x 4 y un
// triangulo de pantalla completa sin bufer de vertices (restos.hlsl, aqui al
// lado):
//
//   A  render targets de ENTEROS. A1 R32_UINT: limpio a 7.0 (-> 7) y la
//      mitad izquierda 0xDEAD0000 + x + 16 y. A2 R8_UINT: limpio a 3.7 (-> 3,
//      hacia el cero), la columna 0 con 77 y la 1 con 300 (SATURA a 255).
//      A3 R16G16B16A16_SINT: limpio a (-2.5, 1000.9, -3, 1) -> (-2, 1000,
//      -3, 1); la columna 0 (-5, 1234, -1234, 7) y la 1 (40000, -40000, 5,
//      -5) -> (32767, -32768, 5, -5). A4 dos a la vez: un R8G8B8A8_UINT con
//      la mascara de escritura R|B y un R32G32_UINT. A5 el R32_UINT de A1
//      leido como textura (Load) mas 1.
//   B  un dibujo SOLO con UAV (sin render target ni profundidad), con el
//      viewport de 6 x 3: cada pixel suma 1 (18) y marca su texel de un
//      RWTexture2D<uint> (1000 + x + 16 y; fuera del viewport, 0).
//   C  un sombreador de GEOMETRIA que escribe UAV: 5 puntos, cada uno suma 1
//      (5) y pone 200 + su numero en un RWBuffer<uint> (200..204, y 0).
//   D  el plano de STENCIL leido de vuelta: CopyTextureRegion de su
//      subrecurso 1 (la huella, de GetCopyableFootprints) en un D24S8 (D1)
//      y un D32S8X24 (D3), y un SRV X24_TYPELESS_G8_UINT (PlaneSlice 1) de
//      un R24G8_TYPELESS leido en un sombreador (D2).
//   E  SV_StencilRef: si OPTIONS dice PSSpecifiedStencilRefSupported, la
//      referencia del sombreador (10 + x) con REPLACE, y luego EQUAL con la
//      suya (13): solo la columna 3 pasa (ZERO, y su color). Si no lo dice,
//      se salta (una nota, no un fallo: hay GPU que no lo tienen).
//
// Lo de saturar (A2 y A3, la columna 1) es la regla de conversion entre
// enteros de la especificacion funcional de D3D11.3 (3.2.3.6), que D3D12
// hereda; va en sus lineas propias. Todo entero: lo que tiene que salir esta
// escrito abajo, con su cuenta. Sale con el numero de fallos; en Windows
// dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(vs_pantalla, "restos_vs.dxil") DXIL(ps_uint, "restos_uint.dxil") DXIL(ps_sint, "restos_sint.dxil") DXIL(ps_dos, "restos_dos.dxil")
            DXIL(ps_lee, "restos_lee.dxil") DXIL(ps_stencil, "restos_stencil.dxil") DXIL(ps_uav, "restos_uav.dxil") DXIL(vs_punto, "restos_punto.dxil") DXIL(gs_marca, "restos_gs.dxil")
                DXIL(ps_ref, "restos_ref.dxil") ".text\n");
extern "C" const unsigned char vs_pantalla[], vs_pantalla_fin[], ps_uint[], ps_uint_fin[], ps_sint[], ps_sint_fin[], ps_dos[], ps_dos_fin[], ps_lee[], ps_lee_fin[], ps_stencil[], ps_stencil_fin[],
    ps_uav[], ps_uav_fin[], vs_punto[], vs_punto_fin[], gs_marca[], gs_marca_fin[], ps_ref[], ps_ref_fin[];

static const UINT W = 8, H = 4;
// Lo leido: cada cosa en su hueco de 4 KiB de un bufer de READBACK.
static const UINT64 HUECO = 4096;
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

static D3D12_RESOURCE_DESC desc(D3D12_RESOURCE_DIMENSION dim, UINT64 ancho, UINT alto, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl) {
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
    return r;
}

static ID3D12Resource *crear(ID3D12Device *d, D3D12_HEAP_TYPE monton, const D3D12_RESOURCE_DESC &r, D3D12_RESOURCE_STATES e, const char *que) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), que);
    return b;
}

static ID3D12Resource *textura(ID3D12Device *d, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e) {
    return crear(d, D3D12_HEAP_TYPE_DEFAULT, desc(D3D12_RESOURCE_DIMENSION_TEXTURE2D, W, H, f, fl), e, "CreateCommittedResource de una textura");
}

static ID3D12Resource *bufer(ID3D12Device *d, D3D12_HEAP_TYPE monton, UINT64 n, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e) {
    return crear(d, monton, desc(D3D12_RESOURCE_DIMENSION_BUFFER, n, 1, DXGI_FORMAT_UNKNOWN, fl), e, "CreateCommittedResource de un bufer");
}

static void barrera(ID3D12GraphicsCommandList *l, ID3D12Resource *b, D3D12_RESOURCE_STATES antes, D3D12_RESOURCE_STATES despues) {
    D3D12_RESOURCE_BARRIER x = {};
    x.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
    x.Transition.pResource = b;
    x.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
    x.Transition.StateBefore = antes;
    x.Transition.StateAfter = despues;
    l->ResourceBarrier(1, &x);
}

static void barrera_uav(ID3D12GraphicsCommandList *l, ID3D12Resource *b) {
    D3D12_RESOURCE_BARRIER x = {};
    x.Type = D3D12_RESOURCE_BARRIER_TYPE_UAV;
    x.UAV.pResource = b;
    l->ResourceBarrier(1, &x);
}

// Lo que pide un PSO: sus sombreadores, sus render targets y su stencil.
struct Pide {
    const unsigned char *vs = vs_pantalla, *vs_fin = vs_pantalla_fin;
    const unsigned char *gs = nullptr, *gs_fin = nullptr, *ps = nullptr, *ps_fin = nullptr;
    UINT n_rt = 0;
    DXGI_FORMAT rt[2] = {DXGI_FORMAT_UNKNOWN, DXGI_FORMAT_UNKNOWN};
    UINT8 mascara = D3D12_COLOR_WRITE_ENABLE_ALL;
    DXGI_FORMAT dsv = DXGI_FORMAT_UNKNOWN;
    bool st = false;
    D3D12_STENCIL_OP pasa = D3D12_STENCIL_OP_KEEP;
    D3D12_COMPARISON_FUNC funcion = D3D12_COMPARISON_FUNC_ALWAYS;
    D3D12_PRIMITIVE_TOPOLOGY_TYPE tipo = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
};

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const Pide &q) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS = {q.vs, (SIZE_T)(q.vs_fin - q.vs)};
    if (q.gs)
        p.GS = {q.gs, (SIZE_T)(q.gs_fin - q.gs)};
    if (q.ps)
        p.PS = {q.ps, (SIZE_T)(q.ps_fin - q.ps)};
    p.BlendState.RenderTarget[0].RenderTargetWriteMask = q.mascara;
    p.BlendState.RenderTarget[1].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    p.BlendState.IndependentBlendEnable = TRUE;
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.DepthStencilState.StencilEnable = q.st;
    p.DepthStencilState.StencilReadMask = 0xFF;
    p.DepthStencilState.StencilWriteMask = 0xFF;
    D3D12_DEPTH_STENCILOP_DESC c = {D3D12_STENCIL_OP_KEEP, D3D12_STENCIL_OP_KEEP, q.pasa, q.funcion};
    p.DepthStencilState.FrontFace = c;
    p.DepthStencilState.BackFace = c;
    p.DSVFormat = q.dsv;
    p.PrimitiveTopologyType = q.tipo;
    p.NumRenderTargets = q.n_rt;
    p.RTVFormats[0] = q.rt[0];
    p.RTVFormats[1] = q.rt[1];
    p.SampleDesc.Count = 1;
    ID3D12PipelineState *x = nullptr;
    hecho(d->CreateGraphicsPipelineState(&p, IID_PPV_ARGS(&x)), "CreateGraphicsPipelineState");
    return x;
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
    D3D12_FEATURE_DATA_D3D12_OPTIONS op = {};
    hecho(d->CheckFeatureSupport(D3D12_FEATURE_D3D12_OPTIONS, &op, sizeof op), "CheckFeatureSupport(OPTIONS)");
    const bool con_ref = op.PSSpecifiedStencilRefSupported != FALSE;

    // La raiz, vista por todas las etapas: [0] ocho constantes (b0), [1] la
    // tabla de los dos SRV (t0, t1), [2] la de los tres UAV (u0..u2).
    D3D12_DESCRIPTOR_RANGE rs_srv = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 2, 0, 0, 0}, rs_uav = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 3, 0, 0, 0};
    D3D12_ROOT_PARAMETER rp[3] = {};
    rp[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    rp[0].Constants.Num32BitValues = 8;
    rp[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    rp[1].DescriptorTable = {1, &rs_srv};
    rp[2].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    rp[2].DescriptorTable = {1, &rs_uav};
    for (D3D12_ROOT_PARAMETER &p : rp)
        p.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_SIGNATURE_DESC rd = {3, rp, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;

    // Los PSO.
    const DXGI_FORMAT R32U = DXGI_FORMAT_R32_UINT, D24 = DXGI_FORMAT_D24_UNORM_S8_UINT, D32 = DXGI_FORMAT_D32_FLOAT_S8X24_UINT;
    Pide q;
    q.ps = ps_uint, q.ps_fin = ps_uint_fin, q.n_rt = 1, q.rt[0] = R32U;
    ID3D12PipelineState *p_a1 = pso(d, rs, q);
    q.rt[0] = DXGI_FORMAT_R8_UINT;
    ID3D12PipelineState *p_a2 = pso(d, rs, q);
    q.ps = ps_sint, q.ps_fin = ps_sint_fin, q.rt[0] = DXGI_FORMAT_R16G16B16A16_SINT;
    ID3D12PipelineState *p_a3 = pso(d, rs, q);
    q.ps = ps_dos, q.ps_fin = ps_dos_fin, q.n_rt = 2, q.rt[0] = DXGI_FORMAT_R8G8B8A8_UINT, q.rt[1] = DXGI_FORMAT_R32G32_UINT;
    q.mascara = D3D12_COLOR_WRITE_ENABLE_RED | D3D12_COLOR_WRITE_ENABLE_BLUE;
    ID3D12PipelineState *p_a4 = pso(d, rs, q);
    q = Pide();
    q.ps = ps_lee, q.ps_fin = ps_lee_fin, q.n_rt = 1, q.rt[0] = R32U;
    ID3D12PipelineState *p_a5 = pso(d, rs, q);
    q.ps = ps_stencil, q.ps_fin = ps_stencil_fin;
    ID3D12PipelineState *p_d2 = pso(d, rs, q);
    q = Pide();
    q.ps = ps_uav, q.ps_fin = ps_uav_fin;
    ID3D12PipelineState *p_b = pso(d, rs, q);
    q = Pide();
    q.vs = vs_punto, q.vs_fin = vs_punto_fin, q.gs = gs_marca, q.gs_fin = gs_marca_fin, q.tipo = D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT;
    ID3D12PipelineState *p_c = pso(d, rs, q);
    // D: de SOLO profundidad (sin sombreador de pixeles), stencil REPLACE.
    q = Pide();
    q.dsv = D24, q.st = true, q.pasa = D3D12_STENCIL_OP_REPLACE;
    ID3D12PipelineState *p_d24 = pso(d, rs, q);
    q.dsv = D32;
    ID3D12PipelineState *p_d32 = pso(d, rs, q);
    ID3D12PipelineState *p_e1 = nullptr, *p_e2 = nullptr;
    if (con_ref) {
        q = Pide();
        q.ps = ps_ref, q.ps_fin = ps_ref_fin, q.n_rt = 1, q.rt[0] = R32U, q.dsv = D24, q.st = true, q.pasa = D3D12_STENCIL_OP_REPLACE;
        p_e1 = pso(d, rs, q);
        q.pasa = D3D12_STENCIL_OP_ZERO, q.funcion = D3D12_COMPARISON_FUNC_EQUAL;
        p_e2 = pso(d, rs, q);
    }

    // Los recursos. Los render targets: [0] A1, [1] A2, [2] A3, [3] y [4] A4,
    // [5] A5, [6] el de D2, [7] el de E. Las profundidades: D1, D2, D3, E.
    const D3D12_RESOURCE_FLAGS RT = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, Z = D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, UAV = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    const D3D12_RESOURCE_STATES E_RT = D3D12_RESOURCE_STATE_RENDER_TARGET, E_Z = D3D12_RESOURCE_STATE_DEPTH_WRITE, E_UAV = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    const D3D12_RESOURCE_STATES COPIA = D3D12_RESOURCE_STATE_COPY_SOURCE, LEER = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
    const DXGI_FORMAT f_rt[8] = {R32U, DXGI_FORMAT_R8_UINT, DXGI_FORMAT_R16G16B16A16_SINT, DXGI_FORMAT_R8G8B8A8_UINT, DXGI_FORMAT_R32G32_UINT, R32U, R32U, R32U};
    const DXGI_FORMAT f_z[4] = {D24, DXGI_FORMAT_R24G8_TYPELESS, D32, D24}, v_z[4] = {D24, D24, D32, D24};
    ID3D12Resource *rt[8], *zs[4];
    for (int k = 0; k < 8; k++)
        rt[k] = textura(d, f_rt[k], RT, E_RT);
    for (int k = 0; k < 4; k++)
        zs[k] = textura(d, f_z[k], Z, E_Z);
    ID3D12Resource *marcas = textura(d, R32U, UAV, E_UAV);
    ID3D12Resource *cuenta = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 256, UAV, E_UAV), *lista = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 256, UAV, E_UAV);
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, 16 * HUECO, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    decir(true, "render targets de enteros, profundidades con stencil, tres UAV y los PSO que los usan");

    // Los descriptores.
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 8, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hz = {D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 4, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hv = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 5, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr, *dsvs = nullptr, *visible = nullptr;
    hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV");
    hecho(d->CreateDescriptorHeap(&hz, IID_PPV_ARGS(&dsvs)), "CreateDescriptorHeap de los DSV");
    hecho(d->CreateDescriptorHeap(&hv, IID_PPV_ARGS(&visible)), "CreateDescriptorHeap visible");
    if (fallos)
        return fallos;
    UINT paso_r = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV), paso_z = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV);
    UINT paso_v = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    D3D12_CPU_DESCRIPTOR_HANDLE rtv[8], dsv[4], cv;
    D3D12_GPU_DESCRIPTOR_HANDLE gv;
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv[0]);
    dsvs->GetCPUDescriptorHandleForHeapStart(&dsv[0]);
    visible->GetCPUDescriptorHandleForHeapStart(&cv);
    visible->GetGPUDescriptorHandleForHeapStart(&gv);
    for (int k = 0; k < 8; k++) {
        rtv[k].ptr = rtv[0].ptr + k * paso_r;
        d->CreateRenderTargetView(rt[k], nullptr, rtv[k]);
    }
    for (int k = 0; k < 4; k++) {
        dsv[k].ptr = dsv[0].ptr + k * paso_z;
        D3D12_DEPTH_STENCIL_VIEW_DESC dv = {};
        dv.Format = v_z[k];
        dv.ViewDimension = D3D12_DSV_DIMENSION_TEXTURE2D;
        d->CreateDepthStencilView(zs[k], &dv, dsv[k]);
    }
    auto en = [&](UINT i) { return D3D12_CPU_DESCRIPTOR_HANDLE{cv.ptr + (SIZE_T)i * paso_v}; };
    auto eng = [&](UINT i) { return D3D12_GPU_DESCRIPTOR_HANDLE{gv.ptr + (UINT64)i * paso_v}; };
    // [0] t0: el R32_UINT de A1. [1] t1: el plano de stencil de D2.
    D3D12_SHADER_RESOURCE_VIEW_DESC s0 = {}, s1 = {};
    s0.Format = R32U;
    s0.ViewDimension = D3D12_SRV_DIMENSION_TEXTURE2D;
    s0.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
    s0.Texture2D.MipLevels = 1;
    s1 = s0;
    s1.Format = DXGI_FORMAT_X24_TYPELESS_G8_UINT;
    s1.Texture2D.PlaneSlice = 1;
    d->CreateShaderResourceView(rt[0], &s0, en(0));
    d->CreateShaderResourceView(zs[1], &s1, en(1));
    // [2] u0: la cuenta (cruda). [3] u1: las marcas. [4] u2: la lista.
    D3D12_UNORDERED_ACCESS_VIEW_DESC u0 = {}, u1 = {}, u2 = {};
    u0.Format = DXGI_FORMAT_R32_TYPELESS;
    u0.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    u0.Buffer.NumElements = 64;
    u0.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    u1.Format = R32U;
    u1.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
    u2.Format = R32U;
    u2.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    u2.Buffer.NumElements = 16;
    d->CreateUnorderedAccessView(cuenta, nullptr, &u0, en(2));
    d->CreateUnorderedAccessView(marcas, nullptr, &u1, en(3));
    d->CreateUnorderedAccessView(lista, nullptr, &u2, en(4));

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, p_a1, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetDescriptorHeaps(1, &visible);
    l->SetGraphicsRootSignature(rs);
    l->SetGraphicsRootDescriptorTable(1, eng(0));
    l->SetGraphicsRootDescriptorTable(2, eng(2));
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    D3D12_VIEWPORT vp = {0, 0, (float)W, (float)H, 0, 1};
    l->RSSetViewports(1, &vp);
    const D3D12_RECT todo = {0, 0, (LONG)W, (LONG)H};
    // Un dibujo del triangulo de pantalla con su PSO, su tijera y sus ocho
    // constantes (v y d).
    auto dibujar = [&](ID3D12PipelineState *p, D3D12_RECT t, UINT v0, UINT v1, UINT v2, UINT v3, UINT d0, UINT d1) {
        const UINT c[8] = {v0, v1, v2, v3, d0, d1, 0, 0};
        l->SetPipelineState(p);
        l->RSSetScissorRects(1, &t);
        l->SetGraphicsRoot32BitConstants(0, 8, c, 0);
        l->DrawInstanced(3, 1, 0, 0);
    };
    auto columna = [](LONG x) { return D3D12_RECT{x, 0, x + 1, (LONG)H}; };
    auto limpiar = [&](int k, float r, float g, float b, float a) {
        const float c[4] = {r, g, b, a};
        l->ClearRenderTargetView(rtv[k], c, 0, nullptr);
    };

    // A1, A2, A3 y A4.
    limpiar(0, 7, 7, 7, 7);
    l->OMSetRenderTargets(1, &rtv[0], FALSE, nullptr);
    dibujar(p_a1, D3D12_RECT{0, 0, (LONG)W / 2, (LONG)H}, 0xDEAD0000u, 0, 0, 0, 1, 0);
    limpiar(1, 3.7f, 0, 0, 0);
    l->OMSetRenderTargets(1, &rtv[1], FALSE, nullptr);
    dibujar(p_a2, columna(0), 77, 0, 0, 0, 0, 0);
    dibujar(p_a2, columna(1), 300, 0, 0, 0, 0, 0);
    limpiar(2, -2.5f, 1000.9f, -3, 1);
    l->OMSetRenderTargets(1, &rtv[2], FALSE, nullptr);
    dibujar(p_a3, columna(0), (UINT)-5, 1234, (UINT)-1234, 7, 0, 0);
    dibujar(p_a3, columna(1), 40000, (UINT)-40000, 5, (UINT)-5, 0, 0);
    limpiar(3, 1, 2, 3, 4);
    limpiar(4, 5, 6, 0, 0);
    l->OMSetRenderTargets(2, &rtv[3], TRUE, nullptr);
    dibujar(p_a4, todo, 10, 20, 30, 40, 0x89ABCDEFu, 0x01234567u);
    // A5: el de A1, como textura.
    barrera(l, rt[0], E_RT, LEER);
    limpiar(5, 0, 0, 0, 0);
    l->OMSetRenderTargets(1, &rtv[5], FALSE, nullptr);
    dibujar(p_a5, todo, 0, 0, 0, 0, 0, 0);
    barrera(l, rt[0], LEER, COPIA);

    // B: sin render target ni profundidad, el viewport de 6 x 3.
    l->OMSetRenderTargets(0, nullptr, FALSE, nullptr);
    D3D12_VIEWPORT chico = {0, 0, 6, 3, 0, 1};
    l->RSSetViewports(1, &chico);
    dibujar(p_b, todo, 0, 0, 0, 0, 0, 0);
    l->RSSetViewports(1, &vp);
    barrera_uav(l, cuenta);
    // C: cinco puntos por el GS.
    l->SetPipelineState(p_c);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
    l->DrawInstanced(5, 1, 0, 0);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

    // D1: D24S8 limpio a 0x11, REPLACE 0x5A en la mitad izquierda.
    l->ClearDepthStencilView(dsv[0], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0x11, 0, nullptr);
    l->OMSetRenderTargets(0, nullptr, FALSE, &dsv[0]);
    l->OMSetStencilRef(0x5A);
    dibujar(p_d24, D3D12_RECT{0, 0, (LONG)W / 2, (LONG)H}, 0, 0, 0, 0, 0, 0);
    // D2: R24G8_TYPELESS limpio a 0x22, REPLACE 0x33 en las dos filas de
    // arriba; luego su plano por el SRV de t1.
    l->ClearDepthStencilView(dsv[1], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0x22, 0, nullptr);
    l->OMSetRenderTargets(0, nullptr, FALSE, &dsv[1]);
    l->OMSetStencilRef(0x33);
    dibujar(p_d24, D3D12_RECT{0, 0, (LONG)W, 2}, 0, 0, 0, 0, 0, 0);
    barrera(l, zs[1], E_Z, LEER);
    limpiar(6, 0, 0, 0, 0);
    l->OMSetRenderTargets(1, &rtv[6], FALSE, nullptr);
    dibujar(p_d2, todo, 0, 0, 0, 0, 0, 0);
    // D3: D32S8X24 limpio a 0x44, REPLACE 0x99 en las tres columnas de la
    // izquierda.
    l->ClearDepthStencilView(dsv[2], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0x44, 0, nullptr);
    l->OMSetRenderTargets(0, nullptr, FALSE, &dsv[2]);
    l->OMSetStencilRef(0x99);
    dibujar(p_d32, D3D12_RECT{0, 0, 3, (LONG)H}, 0, 0, 0, 0, 0, 0);
    // E: la referencia del sombreador (10 + x, color 1) y luego EQUAL 13
    // (ZERO, color 2).
    if (con_ref) {
        l->ClearDepthStencilView(dsv[3], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0, 0, nullptr);
        limpiar(7, 0, 0, 0, 0);
        l->OMSetRenderTargets(1, &rtv[7], FALSE, &dsv[3]);
        l->OMSetStencilRef(0xFF);
        dibujar(p_e1, todo, 1, 10, 1, 0, 0, 0);
        dibujar(p_e2, todo, 2, 13, 0, 0, 0, 0);
    }

    // Leer: cada cosa en su hueco.
    auto copiar = [&](ID3D12Resource *t, UINT sub, UINT hueco, DXGI_FORMAT f) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint.Offset = hueco * HUECO;
        a.PlacedFootprint.Footprint = {f, W, H, 1, 256};
        de.pResource = t;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        de.SubresourceIndex = sub;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    };
    // El plano 1 de una profundidad: la huella la da GetCopyableFootprints.
    UINT paso_plano[16] = {};
    auto copiar_plano = [&](ID3D12Resource *t, DXGI_FORMAT f, UINT hueco) {
        D3D12_RESOURCE_DESC r = desc(D3D12_RESOURCE_DIMENSION_TEXTURE2D, W, H, f, Z);
        D3D12_PLACED_SUBRESOURCE_FOOTPRINT fp = {};
        UINT filas = 0;
        UINT64 fila = 0, total = 0;
        d->GetCopyableFootprints(&r, 1, 1, hueco * HUECO, &fp, &filas, &fila, &total);
        paso_plano[hueco] = fp.Footprint.RowPitch;
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint = fp;
        de.pResource = t;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        de.SubresourceIndex = 1;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    };
    for (int k = 1; k < 8; k++) {
        if (k == 7 && !con_ref)
            continue;
        barrera(l, rt[k], E_RT, COPIA);
        copiar(rt[k], 0, k, f_rt[k]);
    }
    copiar(rt[0], 0, 0, R32U);
    barrera(l, marcas, E_UAV, COPIA);
    copiar(marcas, 0, 8, R32U);
    barrera(l, cuenta, E_UAV, COPIA);
    barrera(l, lista, E_UAV, COPIA);
    l->CopyBufferRegion(leida, 9 * HUECO, cuenta, 0, 16);
    l->CopyBufferRegion(leida, 9 * HUECO + 256, lista, 0, 64);
    barrera(l, zs[0], E_Z, COPIA);
    copiar_plano(zs[0], D24, 10);
    barrera(l, zs[2], E_Z, COPIA);
    copiar_plano(zs[2], D32, 11);
    if (con_ref) {
        barrera(l, zs[3], E_Z, COPIA);
        copiar_plano(zs[3], D24, 12);
    }
    hecho(l->Close(), "Close");
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    hecho(cola->Signal(valla, 1), "Signal");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        hecho(valla->SetEventOnCompletion(1, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 10000);
    }

    void *mp = nullptr;
    if (!hecho(leida->Map(0, nullptr, &mp), "Map de lo leido"))
        return fallos;
    const unsigned char *b = (const unsigned char *)mp;
    // El texel (x, y) del hueco `h` (filas de `paso` bytes, `n` bytes cada uno).
    auto texel = [&](UINT h, UINT paso, UINT n, UINT x, UINT y) { return b + h * HUECO + (UINT64)y * paso + (UINT64)x * n; };
    auto u32_ = [&](UINT h, UINT x, UINT y) { UINT v; memcpy(&v, texel(h, 256, 4, x, y), 4); return v; };
    char m[300];
    // Juzga `bien` en los texeles (x, y) con `mira(x, y)`: `vale(x, y)` da lo
    // visto y lo que tenia que ser.
    auto juzgar = [&](const char *que, auto mira, auto vale) {
        UINT malos = 0, mx = 0, my = 0;
        unsigned long long visto = 0, quiero = 0;
        for (UINT y = 0; y < H; y++)
            for (UINT x = 0; x < W; x++) {
                if (!mira(x, y))
                    continue;
                unsigned long long v, q;
                vale(x, y, v, q);
                if (v != q && malos++ == 0)
                    mx = x, my = y, visto = v, quiero = q;
            }
        if (malos == 0)
            snprintf(m, sizeof m, "%s", que);
        else
            snprintf(m, sizeof m, "%s: %u texeles distintos; el (%u, %u) es %llx y tenia que ser %llx", que, malos, mx, my, visto, quiero);
        decir(malos == 0, m);
    };
    auto siempre = [](UINT, UINT) { return true; };
    // A1: la izquierda 0xDEAD0000 + x + 16 y, la derecha 7.
    juzgar("A1, R32_UINT: limpio a 7.0 da 7 y el sombreador escribe sus 32 bits (0xDEAD0000 + x + 16 y)", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = u32_(0, x, y);
        q = x < W / 2 ? 0xDEAD0000u + x + 16 * y : 7;
    });
    // A2: 3.7 -> 3 (hacia el cero); la columna 0, 77; la 1, 300 -> 255.
    auto a2 = [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = *texel(1, 256, 1, x, y);
        q = x == 0 ? 77 : x == 1 ? 255 : 3;
    };
    juzgar("A2, R8_UINT: limpio a 3.7 da 3 (hacia el cero) y el sombreador escribe 77", [](UINT x, UINT) { return x != 1; }, a2);
    juzgar("A2, R8_UINT: 300 del sombreador SATURA a 255", [](UINT x, UINT) { return x == 1; }, a2);
    // A3: cuatro SINT de 16 por texel.
    auto a3 = [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        short s[4];
        memcpy(s, texel(2, 256, 8, x, y), 8);
        const short limpio[4] = {-2, 1000, -3, 1}, c0[4] = {-5, 1234, -1234, 7}, c1[4] = {32767, -32768, 5, -5};
        const short *w = x == 0 ? c0 : x == 1 ? c1 : limpio;
        v = q = 0;
        for (int k = 0; k < 4; k++) {
            v = v << 16 | (unsigned short)s[k];
            q = q << 16 | (unsigned short)w[k];
        }
    };
    juzgar("A3, R16G16B16A16_SINT: limpio a (-2.5, 1000.9, -3, 1) da (-2, 1000, -3, 1)", [](UINT x, UINT) { return x >= 2; }, a3);
    juzgar("A3, R16G16B16A16_SINT: el sombreador escribe (-5, 1234, -1234, 7)", [](UINT x, UINT) { return x == 0; }, a3);
    juzgar("A3, R16G16B16A16_SINT: (40000, -40000, 5, -5) SATURA a (32767, -32768, 5, -5)", [](UINT x, UINT) { return x == 1; }, a3);
    // A4: RGBA8_UINT con la mascara R|B sobre (1, 2, 3, 4): (10, 2, 30, 4);
    // R32G32_UINT: los dos de 32 bits.
    juzgar("A4, dos de enteros a la vez: R8G8B8A8_UINT con la mascara R|B (10, 2, 30, 4) y R32G32_UINT (0x89ABCDEF, 0x01234567)", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        UINT a, c[2];
        memcpy(&a, texel(3, 256, 4, x, y), 4);
        memcpy(c, texel(4, 256, 8, x, y), 8);
        // RGBA8 en memoria: R en el byte bajo (0x041E020A); y el R32G32.
        v = (unsigned long long)a << 32 | c[0];
        q = (unsigned long long)0x041E020Au << 32 | 0x89ABCDEFu;
        if (c[1] != 0x01234567u)
            v = ~v;
    });
    juzgar("A5, el R32_UINT de A1 leido como textura (Load) mas 1", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = u32_(5, x, y);
        q = x < W / 2 ? 0xDEAD0000u + x + 16 * y + 1 : 8;
    });
    // B: 6 x 3 = 18 pixeles; las marcas, dentro del viewport.
    UINT cuenta_v[4];
    memcpy(cuenta_v, b + 9 * HUECO, 16);
    snprintf(m, sizeof m, "B, un dibujo SOLO con UAV (sin render target ni Z): %u pixeles en el viewport de 6 x 3 (tenian que ser 18)", cuenta_v[0]);
    decir(cuenta_v[0] == 18, m);
    juzgar("B, cada pixel del viewport marca su texel (1000 + x + 16 y); fuera, 0", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = u32_(8, x, y);
        q = x < 6 && y < 3 ? 1000 + x + 16 * y : 0;
    });
    // C: 5 puntos; la lista 200..204 y ceros.
    UINT lista_v[16];
    memcpy(lista_v, b + 9 * HUECO + 256, 64);
    bool c_bien = cuenta_v[1] == 5;
    for (UINT i = 0; i < 16; i++)
        c_bien = c_bien && lista_v[i] == (i < 5 ? 200 + i : 0);
    snprintf(m, sizeof m, "C, UAV desde el sombreador de GEOMETRIA: %u puntos (5) y la lista %u %u %u %u %u %u (200..204 y 0)", cuenta_v[1], lista_v[0], lista_v[1], lista_v[2], lista_v[3], lista_v[4], lista_v[5]);
    decir(c_bien, m);
    // D1 y D3: el plano 1, un byte por texel en la huella de GetCopyableFootprints.
    juzgar("D1, D24S8: el plano de stencil por CopyTextureRegion (subrecurso 1): 0x5A a la izquierda, 0x11 a la derecha", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = *texel(10, paso_plano[10], 1, x, y);
        q = x < W / 2 ? 0x5A : 0x11;
    });
    juzgar("D2, R24G8_TYPELESS: el plano de stencil por un SRV X24_TYPELESS_G8_UINT (.g): 0x33 arriba, 0x22 abajo", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = u32_(6, x, y);
        q = y < 2 ? 0x33 : 0x22;
    });
    juzgar("D3, D32S8X24: el plano de stencil por CopyTextureRegion (subrecurso 1): 0x99 en las tres de la izquierda, 0x44 en las demas", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
        v = *texel(11, paso_plano[11], 1, x, y);
        q = x < 3 ? 0x99 : 0x44;
    });
    if (!con_ref) {
        printf("  nota  E: el adaptador dice PSSpecifiedStencilRefSupported = FALSE: SV_StencilRef se salta\n");
    } else {
        decir(true, "E, OPTIONS dice PSSpecifiedStencilRefSupported");
        juzgar("E, SV_StencilRef: REPLACE con la del sombreador (10 + x), y EQUAL con la suya (13) pone 0 la columna 3", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
            v = *texel(12, paso_plano[12], 1, x, y);
            q = x == 3 ? 0 : 10 + x;
        });
        juzgar("E, el color: 1 en todo y 2 solo donde EQUAL 13 paso (la columna 3)", siempre, [&](UINT x, UINT y, unsigned long long &v, unsigned long long &q) {
            v = u32_(7, x, y);
            q = x == 3 ? 2 : 1;
        });
    }
    leida->Unmap(0, nullptr);
    printf("restos.exe: los enteros, los UAV sin destino y del GS, el plano de stencil y SV_StencilRef son los de Windows\n");
    return fallos;
}
