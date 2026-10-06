// derivadas.cpp -- el juez de las DERIVADAS y de la MIP de un muestreo (D4.4
// de docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10), con los sombreadores de
// derivadas.hlsl (aqui al lado):
//
//   A  PSDerivadas en un destino de 16 x 16 de floats: ddx_fine, ddy_fine,
//      ddx_coarse y ddy_coarse de f = x * y (x e y, el centro del pixel). Las
//      finas, exactas: ddx = y, ddy = x. Las gruesas, una por cuadro de 2x2:
//      D3D deja a la GPU elegir de que fila (o columna) del cuadro; se mira
//      que sea la misma en sus cuatro pixeles y una de esas dos.
//   B  Una textura de 64 x 64 con sus 7 mips, cada una de un color solido
//      (la k: R = 40k, G = 200 - 20k, B = 128), muestreada con Sample y
//      MIN_MAG_MIP_POINT en cuadros de 64, 32, 16... 1 pixeles: cada uno, la
//      mip de su escala (lambda = k exacto: un texel de la mip k por pixel).
//   C  La mezcla de dos mips (MIP_LINEAR) justo a medio camino: SampleBias
//      0,5 sobre la escala de la mip 1 (lambda 1,5: la mitad de la 1 y de la
//      2) y SampleLevel 2,5; y SampleGrad con sus gradientes (lambda 3).
//      Cada canal, la media de dos pares: exacta en 8 bits.
//   D  CalculateLevelOfDetail y su Unclamped, escritos: lambda 2 (2 y 2), y
//      de cerca, lambda -1 (0 sujeto, -1 sin sujetar).
//   E  Lo que sujeta la mip: una vista con MostDetailedMip = 2 (el lambda se
//      mide en ESA mip), una con ResourceMinLODClamp = 4, y un muestreador de
//      un monton (CreateSampler) con MaxLOD = 1.
//
// Todo bit a bit; ningun LOD cae en un empate (los de PUNTO son enteros; los
// de LINEAL, x,5 justos). Sale con el numero de fallos. En Windows dice lo
// mismo (es de consola: lo que falle, se lee).
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_cuadro\n vs_cuadro:\n .incbin \"derivadas_vs.dxil\"\n .globl vs_cuadro_fin\n vs_cuadro_fin:\n"
        ".p2align 4\n"
        ".globl ps_deriv\n ps_deriv:\n .incbin \"derivadas_ps.dxil\"\n .globl ps_deriv_fin\n ps_deriv_fin:\n"
        ".p2align 4\n"
        ".globl ps_mips\n ps_mips:\n .incbin \"derivadas_niveles.dxil\"\n .globl ps_mips_fin\n ps_mips_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_cuadro[], vs_cuadro_fin[], ps_deriv[], ps_deriv_fin[], ps_mips[], ps_mips_fin[];

// A: 16 x 16 floats (una fila, 256 bytes); B..E: 128 x 128 de 8 bits.
static const UINT LA = 16, LB = 128, MIPS = 7, TEX = 64;
static const UINT64 BYTES_A = LA * LA * 16, BYTES_B = LB * LB * 4;
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

static ID3D12Resource *bufer(ID3D12Device *d, D3D12_HEAP_TYPE monton, UINT64 bytes, D3D12_RESOURCE_STATES e) {
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
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource de un bufer");
    return b;
}

static ID3D12Resource *textura(ID3D12Device *d, UINT lado, UINT16 mips, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e, D3D12_RESOURCE_DESC *desc) {
    D3D12_HEAP_PROPERTIES hp = {};
    hp.Type = D3D12_HEAP_TYPE_DEFAULT;
    D3D12_RESOURCE_DESC td = {};
    td.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    td.Width = lado;
    td.Height = lado;
    td.DepthOrArraySize = 1;
    td.MipLevels = mips;
    td.Format = f;
    td.SampleDesc.Count = 1;
    td.Flags = fl;
    if (desc)
        *desc = td;
    ID3D12Resource *t = nullptr;
    hecho(d->CreateCommittedResource(&hp, D3D12_HEAP_FLAG_NONE, &td, e, nullptr, IID_PPV_ARGS(&t)), "CreateCommittedResource de una textura");
    return t;
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *ps, const unsigned char *ps_fin, DXGI_FORMAT f) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS.pShaderBytecode = vs_cuadro;
    p.VS.BytecodeLength = (SIZE_T)(vs_cuadro_fin - vs_cuadro);
    p.PS.pShaderBytecode = ps;
    p.PS.BytecodeLength = (SIZE_T)(ps_fin - ps);
    p.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
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

// El color de la mip k, como lo guarda un R8G8B8A8_UNORM (R en el byte bajo).
static UINT color(UINT k) {
    return (40u * k) | (200u - 20u * k) << 8 | 128u << 16 | 255u << 24;
}

// La media de dos colores, canal a canal (cada suma es par: exacta).
static UINT media(UINT a, UINT b) {
    UINT r = 0;
    for (UINT c = 0; c < 32; c += 8)
        r |= ((((a >> c) & 255) + ((b >> c) & 255)) / 2) << c;
    return r;
}

// Un cuadro dibujado sobre B..E: donde, que lee y lo que tiene que salir.
struct Cuadro {
    float x0, y0, lado, escala;
    UINT modo;
    UINT quiero;
    const char *que;
};

// Dibujar un cuadro: las constantes de la raiz (x0, y0, lado, escala, modo,
// ancho) y sus dos triangulos.
static void dibujar(ID3D12GraphicsCommandList *l, float x0, float y0, float lado, float escala, UINT modo, float ancho) {
    UINT c[6];
    memcpy(&c[0], &x0, 4);
    memcpy(&c[1], &y0, 4);
    memcpy(&c[2], &lado, 4);
    memcpy(&c[3], &escala, 4);
    c[4] = modo;
    memcpy(&c[5], &ancho, 4);
    l->SetGraphicsRoot32BitConstants(0, 6, c, 0);
    l->DrawInstanced(6, 1, 0, 0);
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

    // La raiz: 6 constantes (b0), la tabla de los tres SRV (t0..t2) y la del
    // muestreador del monton (s2); s0 y s1, estaticos.
    D3D12_DESCRIPTOR_RANGE rangos[2] = {};
    rangos[0].RangeType = D3D12_DESCRIPTOR_RANGE_TYPE_SRV;
    rangos[0].NumDescriptors = 3;
    rangos[1].RangeType = D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER;
    rangos[1].NumDescriptors = 1;
    rangos[1].BaseShaderRegister = 2;
    D3D12_ROOT_PARAMETER rp[3] = {};
    rp[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    rp[0].Constants.Num32BitValues = 6;
    rp[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    for (UINT k = 0; k < 2; k++) {
        rp[1 + k].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
        rp[1 + k].DescriptorTable.NumDescriptorRanges = 1;
        rp[1 + k].DescriptorTable.pDescriptorRanges = &rangos[k];
        rp[1 + k].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    }
    D3D12_STATIC_SAMPLER_DESC ss[2] = {};
    for (UINT k = 0; k < 2; k++) {
        ss[k].Filter = k == 0 ? D3D12_FILTER_MIN_MAG_MIP_POINT : D3D12_FILTER_MIN_MAG_POINT_MIP_LINEAR;
        ss[k].AddressU = ss[k].AddressV = ss[k].AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
        ss[k].ComparisonFunc = D3D12_COMPARISON_FUNC_NEVER;
        ss[k].MaxLOD = D3D12_FLOAT32_MAX;
        ss[k].ShaderRegister = k;
        ss[k].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    }
    D3D12_ROOT_SIGNATURE_DESC rd = {};
    rd.NumParameters = 3;
    rd.pParameters = rp;
    rd.NumStaticSamplers = 2;
    rd.pStaticSamplers = ss;
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;
    ID3D12PipelineState *pa = pso(d, rs, ps_deriv, ps_deriv_fin, DXGI_FORMAT_R32G32B32A32_FLOAT);
    ID3D12PipelineState *pb = pso(d, rs, ps_mips, ps_mips_fin, DXGI_FORMAT_R8G8B8A8_UNORM);

    // Los destinos, la textura de 7 mips (y lo que la sube) y lo leido.
    ID3D12Resource *ta = textura(d, LA, 1, DXGI_FORMAT_R32G32B32A32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr);
    ID3D12Resource *tb = textura(d, LB, 1, DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr);
    D3D12_RESOURCE_DESC desc_tex;
    ID3D12Resource *tex = textura(d, TEX, MIPS, DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST, &desc_tex);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT huellas[MIPS];
    UINT filas[MIPS];
    UINT64 bytes_fila[MIPS], total = 0;
    d->GetCopyableFootprints(&desc_tex, 0, MIPS, 0, huellas, filas, bytes_fila, &total);
    ID3D12Resource *subida = bufer(d, D3D12_HEAP_TYPE_UPLOAD, total, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, BYTES_A + BYTES_B, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    void *m = nullptr;
    if (!hecho(subida->Map(0, nullptr, &m), "Map de la subida"))
        return fallos;
    for (UINT k = 0; k < MIPS; k++)
        for (UINT y = 0; y < filas[k]; y++)
            for (UINT x = 0; x < huellas[k].Footprint.Width; x++)
                ((UINT *)((unsigned char *)m + huellas[k].Offset + y * huellas[k].Footprint.RowPitch))[x] = color(k);
    subida->Unmap(0, nullptr);

    // Los descriptores: dos RTV; los tres SRV (la vista entera, desde la mip
    // 2, y con ResourceMinLODClamp = 4); el muestreador con MaxLOD = 1.
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC sd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 3, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC md = {D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, 1, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr, *srvs = nullptr, *mues = nullptr;
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV");
    hecho(d->CreateDescriptorHeap(&sd, IID_PPV_ARGS(&srvs)), "CreateDescriptorHeap de los SRV");
    hecho(d->CreateDescriptorHeap(&md, IID_PPV_ARGS(&mues)), "CreateDescriptorHeap del muestreador");
    if (fallos)
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE rtv[2], srv;
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv[0]);
    rtv[1].ptr = rtv[0].ptr + d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
    d->CreateRenderTargetView(ta, nullptr, rtv[0]);
    d->CreateRenderTargetView(tb, nullptr, rtv[1]);
    srvs->GetCPUDescriptorHandleForHeapStart(&srv);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    for (UINT k = 0; k < 3; k++) {
        D3D12_SHADER_RESOURCE_VIEW_DESC v = {};
        v.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        v.ViewDimension = D3D12_SRV_DIMENSION_TEXTURE2D;
        v.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
        v.Texture2D.MostDetailedMip = k == 1 ? 2 : 0;
        v.Texture2D.MipLevels = (UINT)-1;
        v.Texture2D.ResourceMinLODClamp = k == 2 ? 4.0f : 0.0f;
        D3D12_CPU_DESCRIPTOR_HANDLE h = {srv.ptr + k * paso};
        d->CreateShaderResourceView(tex, &v, h);
    }
    D3D12_SAMPLER_DESC sm = {};
    sm.Filter = D3D12_FILTER_MIN_MAG_MIP_POINT;
    sm.AddressU = sm.AddressV = sm.AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
    sm.MaxAnisotropy = 1;
    sm.ComparisonFunc = D3D12_COMPARISON_FUNC_NEVER;
    sm.MinLOD = 0.0f;
    sm.MaxLOD = 1.0f;
    D3D12_CPU_DESCRIPTOR_HANDLE hm;
    mues->GetCPUDescriptorHandleForHeapStart(&hm);
    d->CreateSampler(&sm, hm);
    D3D12_GPU_DESCRIPTOR_HANDLE srv_gpu, mues_gpu;
    srvs->GetGPUDescriptorHandleForHeapStart(&srv_gpu);
    mues->GetGPUDescriptorHandleForHeapStart(&mues_gpu);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pa, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    // La textura, mip a mip, y a leer.
    for (UINT k = 0; k < MIPS; k++) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = tex;
        a.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        a.SubresourceIndex = k;
        de.pResource = subida;
        de.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        de.PlacedFootprint = huellas[k];
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    }
    transicion(l, tex, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    l->SetGraphicsRootSignature(rs);
    ID3D12DescriptorHeap *montones[2] = {srvs, mues};
    l->SetDescriptorHeaps(2, montones);
    l->SetGraphicsRootDescriptorTable(1, srv_gpu);
    l->SetGraphicsRootDescriptorTable(2, mues_gpu);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    const float negro[4] = {0, 0, 0, 0};

    // A: las derivadas, en todo el destino de 16 x 16.
    D3D12_VIEWPORT va = {0, 0, (float)LA, (float)LA, 0, 1};
    D3D12_RECT ra = {0, 0, (LONG)LA, (LONG)LA};
    l->RSSetViewports(1, &va);
    l->RSSetScissorRects(1, &ra);
    l->ClearRenderTargetView(rtv[0], negro, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[0], FALSE, nullptr);
    dibujar(l, 0, 0, (float)LA, 1, 0, (float)LA);

    // B..E: cada cuadro en su sitio del destino de 128 x 128.
    static const Cuadro cuadros[] = {
        {0, 0, 64, 1, 0, color(0), "B, Sample en un cuadro de 64 (un texel por pixel): la mip 0"},
        {64, 0, 32, 1, 0, color(1), "B, Sample en uno de 32 (lambda 1): la mip 1"},
        {96, 0, 16, 1, 0, color(2), "B, Sample en uno de 16 (lambda 2): la mip 2"},
        {112, 0, 8, 1, 0, color(3), "B, Sample en uno de 8 (lambda 3): la mip 3"},
        {120, 0, 4, 1, 0, color(4), "B, Sample en uno de 4 (lambda 4): la mip 4"},
        {124, 0, 2, 1, 0, color(5), "B, Sample en uno de 2 (lambda 5): la mip 5"},
        {126, 0, 1, 1, 0, color(6), "B, Sample en un pixel solo (lambda 6, con 3 ayudantes): la mip 6"},
        {0, 64, 32, 1, 1, media(color(1), color(2)), "C, SampleBias 0,5 en uno de 32 con MIP_LINEAR (lambda 1,5): la media de las mips 1 y 2"},
        {32, 64, 8, 1, 2, media(color(2), color(3)), "C, SampleLevel 2,5 con MIP_LINEAR: la media de las mips 2 y 3"},
        {40, 64, 8, 1, 3, color(3), "C, SampleGrad con ddx = ddy = 1/8 (lambda 3): la mip 3"},
        {48, 64, 16, 1, 4, 64u | 96u << 8 | 255u << 24, "D, CalculateLevelOfDetail en uno de 16: 2, y Unclamped 2"},
        {64, 64, 32, 0.25f, 4, 0u | 48u << 8 | 255u << 24, "D, CalculateLevelOfDetail de cerca (medio texel por pixel): 0 sujeto, -1 sin sujetar"},
        {96, 64, 8, 1, 5, color(3), "E, MostDetailedMip 2 en uno de 8 (lambda 1 en la mip 2, de 16): la mip 3"},
        {104, 64, 16, 1, 6, color(4), "E, ResourceMinLODClamp 4 en uno de 16 (lambda 2): la mip 4"},
        {0, 96, 8, 1, 7, color(1), "E, un muestreador de un monton con MaxLOD 1, en uno de 8 (lambda 3): la mip 1"},
    };
    D3D12_VIEWPORT vb = {0, 0, (float)LB, (float)LB, 0, 1};
    D3D12_RECT rb = {0, 0, (LONG)LB, (LONG)LB};
    l->SetPipelineState(pb);
    l->RSSetViewports(1, &vb);
    l->RSSetScissorRects(1, &rb);
    l->ClearRenderTargetView(rtv[1], negro, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[1], FALSE, nullptr);
    for (const Cuadro &c : cuadros)
        dibujar(l, c.x0, c.y0, c.lado, c.escala, c.modo, (float)LB);

    // Los dos destinos, a lo leido.
    ID3D12Resource *destinos[2] = {ta, tb};
    for (UINT k = 0; k < 2; k++) {
        transicion(l, destinos[k], D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        if (k == 0)
            a.PlacedFootprint.Footprint = {DXGI_FORMAT_R32G32B32A32_FLOAT, LA, LA, 1, LA * 16};
        else {
            a.PlacedFootprint.Offset = BYTES_A;
            a.PlacedFootprint.Footprint = {DXGI_FORMAT_R8G8B8A8_UNORM, LB, LB, 1, LB * 4};
        }
        de.pResource = destinos[k];
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
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
    if (!hecho(leida->Map(0, nullptr, &m), "Map de lo leido"))
        return fallos;

    // A: las finas, exactas; las gruesas, una por cuadro y de una de sus
    // dos filas (columnas). Cada valor, n + 0,5 o un entero: exacto en float.
    const float *fa = (const float *)m;
    UINT malas = 0, gruesas = 0, mx = 0, my = 0, arriba = 0;
    for (UINT y = 0; y < LA; y++)
        for (UINT x = 0; x < LA; x++) {
            const float *p = fa + 4 * (y * LA + x);
            float px = x + 0.5f, py = y + 0.5f, qx = (x & ~1u) + 0.5f, qy = (y & ~1u) + 0.5f;
            if ((p[0] != py || p[1] != px) && malas++ == 0)
                mx = x, my = y;
            const float *q = fa + 4 * ((y & ~1u) * LA + (x & ~1u));
            bool una = p[2] == q[2] && p[3] == q[3];
            bool de_su_cuadro = (p[2] == qy || p[2] == qy + 1) && (p[3] == qx || p[3] == qx + 1);
            gruesas += !(una && de_su_cuadro);
            arriba += p[2] == qy && p[3] == qx;
        }
    char msg[240];
    if (malas == 0)
        snprintf(msg, sizeof msg, "A, ddx_fine(x * y) = y y ddy_fine(x * y) = x: los 256 pixeles");
    else
        snprintf(msg, sizeof msg, "A, las finas: %u pixeles distintos; el (%u, %u) da %g y %g", malas, mx, my, fa[4 * (my * LA + mx)], fa[4 * (my * LA + mx) + 1]);
    decir(malas == 0, msg);
    snprintf(msg, sizeof msg, "A, ddx_coarse y ddy_coarse: una por cuadro de 2x2, de una de sus filas y columnas (%u de 256 de la de arriba a la izquierda)", arriba);
    decir(gruesas == 0, msg);

    // B..E: cada pixel de cada cuadro, su color; el primero que no, se dice.
    const UINT *b = (const UINT *)((const unsigned char *)m + BYTES_A);
    for (const Cuadro &c : cuadros) {
        UINT distintos = 0, px = 0, py = 0;
        for (UINT y = (UINT)c.y0; y < (UINT)(c.y0 + c.lado); y++)
            for (UINT x = (UINT)c.x0; x < (UINT)(c.x0 + c.lado); x++)
                if (b[y * LB + x] != c.quiero && distintos++ == 0)
                    px = x, py = y;
        if (distintos == 0)
            snprintf(msg, sizeof msg, "%s (%08x)", c.que, c.quiero);
        else
            snprintf(msg, sizeof msg, "%s: %u pixeles distintos; el (%u, %u) es %08x y tenia que ser %08x", c.que, distintos, px, py, b[py * LB + px], c.quiero);
        decir(distintos == 0, msg);
    }

    printf("derivadas.exe: las derivadas y la mip de un muestreo son las de Windows\n");
    return fallos;
}
