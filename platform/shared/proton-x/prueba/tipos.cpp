// tipos.cpp -- el juez de las VISTAS QUE CAMBIAN EL TIPO (D2.7 de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 06-10), con los sombreadores de
// tipos.hlsl (aqui al lado). D3D12 deja ver una textura con otro formato del
// mismo tamanio (los de su familia TYPELESS, y un R32_UINT de UAV sobre
// cualquiera de 32 bits): los posprocesos de un juego escriben un color y lo
// leen como enteros, o suman por la vista de una palabra. Hasta el 06-10 la
// casa los veia NULOS.
//
//   A  CSEscribe: t8 (R8G8B8A8_TYPELESS) por su vista UNORM (0.2 x, 0.2 y,
//      1, 0): los bytes (51 x, 51 y, 255, 0); t10 (R10G10B10A2_TYPELESS) por
//      una vista R32_UINT: la palabra empaquetada a mano; t16
//      (R16G16B16A16_TYPELESS) por la FLOAT: (x, -y, 0.5, 65504) en halfs.
//   B  CSSuma: InterlockedAdd de 1 por la vista R32_UINT de t8: lo de antes
//      es la palabra de A, y el R sube 1.
//   C  CSLee: t10 por un SRV R10G10B10A2_UNORM (Load, por 1023 y 3: los
//      enteros de A), t16 por uno R16G16B16A16_UINT (los bits de los halfs:
//      el -0 es 0x8000), t8 por uno R8G8B8A8_SINT (51 x + 1 con signo:
//      154 es -102), y su GetDimensions.
//   D  un triangulo sobre DOS render targets R8G8B8A8_TYPELESS, vistos como
//      R8G8B8A8_UINT (10 x + 1, 20 y + 2, 300, 7: el 300 SATURA a 255) y
//      R8G8B8A8_SNORM (-0.25, 1, -1, 0.25: E0 7F 81 20).
//   E  los bytes de t8 al final, copiados: los de B.
//
// Todo bit a bit. Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(cs_escribe, "tipos_escribe.dxil") DXIL(cs_suma, "tipos_suma.dxil") DXIL(cs_lee, "tipos_lee.dxil") DXIL(vs_lleno, "tipos_vs.dxil") DXIL(ps_tipos, "tipos_ps.dxil") ".text\n");
extern "C" const unsigned char cs_escribe[], cs_escribe_fin[], cs_suma[], cs_suma_fin[], cs_lee[], cs_lee_fin[], vs_lleno[], vs_lleno_fin[], ps_tipos[], ps_tipos_fin[];

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

// Una 2D de 4 x 4 de un mip.
static D3D12_RESOURCE_DESC textura(DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    r.Width = 4;
    r.Height = 4;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Flags = fl;
    return r;
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *cs, const unsigned char *fin) {
    D3D12_COMPUTE_PIPELINE_STATE_DESC c = {};
    c.pRootSignature = rs;
    c.CS.pShaderBytecode = cs;
    c.CS.BytecodeLength = (SIZE_T)(fin - cs);
    ID3D12PipelineState *p = nullptr;
    hecho(d->CreateComputePipelineState(&c, IID_PPV_ARGS(&p)), "CreateComputePipelineState");
    return p;
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

static ID3DBlob *serializar(const D3D12_ROOT_SIGNATURE_DESC &rd) {
    ID3DBlob *firma = nullptr, *error = nullptr;
    hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature");
    return firma;
}

// Lo leido: `sal` en 0, y las tres texturas de RGBA8 (t8, y los dos render
// targets), cada una en su sitio de 512.
static const UINT64 EN_SAL = 0, EN_T8 = 1024, EN_RT0 = 3072, EN_RT1 = 5120, LEIDO = 8192;

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

    // La raiz del computo: una tabla de cinco UAV (u0..u4) y una de tres SRV
    // (t0..t2). La del dibujo, vacia.
    D3D12_DESCRIPTOR_RANGE rangos[2] = {{D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 5, 0, 0, 0}, {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 3, 0, 0, 0}};
    D3D12_ROOT_PARAMETER rp[2] = {};
    for (int k = 0; k < 2; k++) {
        rp[k].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
        rp[k].DescriptorTable.NumDescriptorRanges = 1;
        rp[k].DescriptorTable.pDescriptorRanges = &rangos[k];
    }
    ID3DBlob *fc = serializar({2, rp, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE});
    ID3DBlob *fd = serializar({0, nullptr, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE});
    ID3D12RootSignature *rs = nullptr, *rs_dibujo = nullptr;
    if (fc && fd) {
        hecho(d->CreateRootSignature(0, fc->GetBufferPointer(), fc->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
        hecho(d->CreateRootSignature(0, fd->GetBufferPointer(), fd->GetBufferSize(), IID_PPV_ARGS(&rs_dibujo)), "CreateRootSignature del dibujo");
    }
    if (fallos)
        return fallos;
    ID3D12PipelineState *p_escribe = pso(d, rs, cs_escribe, cs_escribe_fin);
    ID3D12PipelineState *p_suma = pso(d, rs, cs_suma, cs_suma_fin);
    ID3D12PipelineState *p_lee = pso(d, rs, cs_lee, cs_lee_fin);
    D3D12_GRAPHICS_PIPELINE_STATE_DESC g = {};
    g.pRootSignature = rs_dibujo;
    g.VS = {vs_lleno, (SIZE_T)(vs_lleno_fin - vs_lleno)};
    g.PS = {ps_tipos, (SIZE_T)(ps_tipos_fin - ps_tipos)};
    g.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    g.BlendState.RenderTarget[1].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    g.BlendState.IndependentBlendEnable = TRUE;
    g.SampleMask = UINT_MAX;
    g.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    g.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    g.RasterizerState.DepthClipEnable = TRUE;
    g.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    g.NumRenderTargets = 2;
    g.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UINT;
    g.RTVFormats[1] = DXGI_FORMAT_R8G8B8A8_SNORM;
    g.SampleDesc.Count = 1;
    ID3D12PipelineState *p_dibujo = nullptr;
    hecho(d->CreateGraphicsPipelineState(&g, IID_PPV_ARGS(&p_dibujo)), "CreateGraphicsPipelineState");

    // Los recursos: los tres TYPELESS del computo, los dos render targets,
    // `sal` y lo leido.
    const D3D12_RESOURCE_STATES UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS, RT = D3D12_RESOURCE_STATE_RENDER_TARGET, CS = D3D12_RESOURCE_STATE_COPY_SOURCE;
    const D3D12_RESOURCE_FLAGS FU = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, FR = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    D3D12_RESOURCE_DESC d8 = textura(DXGI_FORMAT_R8G8B8A8_TYPELESS, FU), drt = textura(DXGI_FORMAT_R8G8B8A8_TYPELESS, FR);
    ID3D12Resource *t8 = recurso(d, D3D12_HEAP_TYPE_DEFAULT, d8, UA);
    ID3D12Resource *t10 = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(DXGI_FORMAT_R10G10B10A2_TYPELESS, FU), UA);
    ID3D12Resource *t16 = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(DXGI_FORMAT_R16G16B16A16_TYPELESS, FU), UA);
    ID3D12Resource *rt0 = recurso(d, D3D12_HEAP_TYPE_DEFAULT, drt, RT);
    ID3D12Resource *rt1 = recurso(d, D3D12_HEAP_TYPE_DEFAULT, drt, RT);
    ID3D12Resource *sal = recurso(d, D3D12_HEAP_TYPE_DEFAULT, bufer(1024, FU), UA);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, bufer(LEIDO, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;

    // Las vistas: [0..4] los UAV, [5..7] los SRV; y los dos RTV.
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 8, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *monton = nullptr, *rtvs = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap") || !hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0, r0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    rtvs->GetCPUDescriptorHandleForHeapStart(&r0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    UINT paso_rtv = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
    auto en = [&](UINT k) { D3D12_CPU_DESCRIPTOR_HANDLE h = {h0.ptr + k * paso}; return h; };
    auto uav = [&](ID3D12Resource *r, DXGI_FORMAT f, UINT k) {
        D3D12_UNORDERED_ACCESS_VIEW_DESC v = {};
        v.Format = f;
        v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
        d->CreateUnorderedAccessView(r, nullptr, &v, en(k));
    };
    uav(t8, DXGI_FORMAT_R8G8B8A8_UNORM, 0);
    uav(t8, DXGI_FORMAT_R32_UINT, 1);
    uav(t10, DXGI_FORMAT_R32_UINT, 2);
    uav(t16, DXGI_FORMAT_R16G16B16A16_FLOAT, 3);
    D3D12_UNORDERED_ACCESS_VIEW_DESC v = {};
    v.Format = DXGI_FORMAT_R32_TYPELESS;
    v.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    v.Buffer.NumElements = 256;
    v.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    d->CreateUnorderedAccessView(sal, nullptr, &v, en(4));
    auto srv = [&](ID3D12Resource *r, DXGI_FORMAT f, UINT k) {
        D3D12_SHADER_RESOURCE_VIEW_DESC s = {};
        s.Format = f;
        s.ViewDimension = D3D12_SRV_DIMENSION_TEXTURE2D;
        s.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
        s.Texture2D.MipLevels = 1;
        d->CreateShaderResourceView(r, &s, en(k));
    };
    srv(t10, DXGI_FORMAT_R10G10B10A2_UNORM, 5);
    srv(t16, DXGI_FORMAT_R16G16B16A16_UINT, 6);
    srv(t8, DXGI_FORMAT_R8G8B8A8_SINT, 7);
    D3D12_CPU_DESCRIPTOR_HANDLE r1 = {r0.ptr + paso_rtv}, rtv[2] = {r0, r1};
    D3D12_RENDER_TARGET_VIEW_DESC rv = {};
    rv.ViewDimension = D3D12_RTV_DIMENSION_TEXTURE2D;
    rv.Format = DXGI_FORMAT_R8G8B8A8_UINT;
    d->CreateRenderTargetView(rt0, &rv, r0);
    rv.Format = DXGI_FORMAT_R8G8B8A8_SNORM;
    d->CreateRenderTargetView(rt1, &rv, r1);
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetGPUDescriptorHandleForHeapStart(&g0);
    D3D12_GPU_DESCRIPTOR_HANDLE g5 = {g0.ptr + 5 * (UINT64)paso};

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, p_escribe, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    // A, B y C.
    l->SetComputeRootSignature(rs);
    l->SetDescriptorHeaps(1, &monton);
    l->SetComputeRootDescriptorTable(0, g0);
    l->SetComputeRootDescriptorTable(1, g5);
    l->Dispatch(1, 1, 1);
    barrera_uav(l);
    l->SetPipelineState(p_suma);
    l->Dispatch(1, 1, 1);
    const D3D12_RESOURCE_STATES SR = D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
    transicion(l, t8, UA, SR);
    transicion(l, t10, UA, SR);
    transicion(l, t16, UA, SR);
    l->SetPipelineState(p_lee);
    l->Dispatch(1, 1, 1);
    // D.
    l->SetPipelineState(p_dibujo);
    l->SetGraphicsRootSignature(rs_dibujo);
    D3D12_VIEWPORT vp = {0, 0, 4, 4, 0, 1};
    D3D12_RECT tijera = {0, 0, 4, 4};
    l->RSSetViewports(1, &vp);
    l->RSSetScissorRects(1, &tijera);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    l->OMSetRenderTargets(2, rtv, FALSE, nullptr);
    l->DrawInstanced(3, 1, 0, 0);
    // A leer: `sal`, t8 y los dos render targets.
    transicion(l, sal, UA, CS);
    transicion(l, t8, SR, CS);
    transicion(l, rt0, RT, CS);
    transicion(l, rt1, RT, CS);
    l->CopyBufferRegion(leida, EN_SAL, sal, 0, 1024);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT h8;
    d->GetCopyableFootprints(&d8, 0, 1, 0, &h8, nullptr, nullptr, nullptr);
    auto copiar = [&](ID3D12Resource *r, UINT64 desde) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint = h8;
        a.PlacedFootprint.Offset = desde;
        de.pResource = r;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    };
    copiar(t8, EN_T8);
    copiar(rt0, EN_RT0);
    copiar(rt1, EN_RT1);
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
    const UINT *s = (const UINT *)(m + EN_SAL);
    auto rgba8 = [&](UINT64 desde, UINT x, UINT y) { return *(const UINT *)(m + desde + (UINT64)y * h8.Footprint.RowPitch + 4 * x); };
    char msg[240];
    // Cuenta por texel: el primero que no da lo suyo, si hay.
    auto juzgar = [&](const char *que, auto leido, auto quiero, UINT palabras) {
        UINT malos = 0, mi = 0, mk = 0;
        for (UINT i = 0; i < 16; i++)
            for (UINT k = 0; k < palabras; k++)
                if (leido(i, k) != quiero(i, k) && malos++ == 0)
                    mi = i, mk = k;
        if (malos == 0)
            snprintf(msg, sizeof msg, "%s: los 16 texeles bit a bit", que);
        else
            snprintf(msg, sizeof msg, "%s: %u palabras distintas; la %u del (%u, %u) es %08x y tenia que ser %08x", que, malos, mk, mi % 4, mi / 4, leido(mi, mk), quiero(mi, mk));
        decir(malos == 0, msg);
    };
    auto x_ = [](UINT i) { return i % 4; };
    auto y_ = [](UINT i) { return i / 4; };
    auto palabra8 = [&](UINT i) { return 51 * x_(i) | 51 * y_(i) << 8 | 255u << 16; };

    // B: lo que habia antes de sumar, la palabra de A.
    juzgar("A y B, t8 escrito por su vista UNORM y sumado por una R32_UINT: lo de antes", [&](UINT i, UINT) { return s[i]; }, [&](UINT i, UINT) { return palabra8(i); }, 1);
    // C.
    juzgar("A y C, t10 escrito como R32_UINT y leido como R10G10B10A2_UNORM", [&](UINT i, UINT k) { return s[16 + 4 * i + k]; },
           [&](UINT i, UINT k) {
               UINT r = 300 * x_(i) + y_(i), q[4] = {r, 1023 - r, 512, (x_(i) + y_(i)) & 3};
               return q[k];
           },
           4);
    juzgar("A y C, t16 escrito en halfs y leido como R16G16B16A16_UINT", [&](UINT i, UINT k) { return s[80 + 4 * i + k]; },
           [&](UINT i, UINT k) {
               const UINT half[4] = {0x0000, 0x3C00, 0x4000, 0x4200};
               UINT q[4] = {half[x_(i)], 0x8000u | half[y_(i)], 0x3800, 0x7BFF};
               return q[k];
           },
           4);
    juzgar("B y C, t8 leido como R8G8B8A8_SINT: (51 x + 1, 51 y, 255, 0) con signo", [&](UINT i, UINT k) { return s[144 + 4 * i + k]; },
           [&](UINT i, UINT k) {
               int q[4] = {(signed char)(51 * x_(i) + 1), (signed char)(51 * y_(i)), -1, 0};
               return (UINT)q[k];
           },
           4);
    snprintf(msg, sizeof msg, "C, GetDimensions del SRV de otro tipo: %u x %u", s[208], s[209]);
    decir(s[208] == 4 && s[209] == 4, msg);
    // D y E.
    juzgar("D, un render target R8G8B8A8_TYPELESS visto como UINT (el 300 satura a 255)", [&](UINT i, UINT) { return rgba8(EN_RT0, x_(i), y_(i)); },
           [&](UINT i, UINT) { return (10 * x_(i) + 1) | (20 * y_(i) + 2) << 8 | 255u << 16 | 7u << 24; }, 1);
    juzgar("D, y otro visto como SNORM (-0.25, 1, -1, 0.25)", [&](UINT i, UINT) { return rgba8(EN_RT1, x_(i), y_(i)); }, [&](UINT, UINT) { return 0x20817FE0u; }, 1);
    juzgar("E, los bytes de t8 al final: la palabra de A mas 1", [&](UINT i, UINT) { return rgba8(EN_T8, x_(i), y_(i)); }, [&](UINT i, UINT) { return palabra8(i) + 1; }, 1);

    printf("tipos.exe: las vistas que cambian el tipo son las de Windows\n");
    return fallos;
}
