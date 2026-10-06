// adyacencia.cpp -- el juez de las topologias con ADYACENCIA (06-10, A6
// del contador de DX12), con los sombreadores de adyacencia.hlsl (aqui al
// lado). Un GS de siluetas o de sombras de volumen (el contorno de un
// modelo: una arista lo es si un triangulo mira a la luz y el de al lado
// no) pide `triangleadj`; hasta el 06-10 la casa no pintaba esos Draw
// ("una topologia con adyacencia: todavia no").
//
//   A  TRIANGLELIST_ADJ con un GS triangleadj, Draw(15): 2 triangulos, cada
//      uno con sus 6 vertices en orden (0..5, 6..11); los 3 de mas, nada.
//   B  TRIANGLESTRIP_ADJ, Draw(10): 3 triangulos con los vertices que dice
//      la tabla de OpenGL, con el impar como lo da D3D (06-10, lo dijo la
//      3060: empieza por su vertice 2i): (0 1 2 6 4 3), (2 5 6 8 4 0) y
//      (4 2 6 9 8 7).
//   C  TRIANGLESTRIP_ADJ, Draw(7): UN triangulo, (0 1 2 5 4 3).
//   D  LINELIST_ADJ con un GS lineadj, Draw(9): 2 lineas (0..3, 4..7).
//   E  LINESTRIP_ADJ, Draw(6): 3 lineas (0 1 2 3), (1 2 3 4), (2 3 4 5).
//   F  TRIANGLELIST_ADJ SIN GS, Draw(6): se pinta el triangulo de los
//      vertices 0, 2 y 4 (tapa la pantalla); los otros no cuentan.
//   G  TRIANGLESTRIP_ADJ sin GS, Draw(8): los triangulos (0 2 4) y (4 2 6),
//      un cuadro que tapa la pantalla.
//   H  TRIANGLESTRIP_ADJ de DOS, Draw(8): el impar y el ultimo a la vez,
//      (0 1 2 6 4 3) y (2 5 6 7 4 0).
//
// Todo entero. Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(vs_id, "ady_vs.dxil") DXIL(gs_tri, "ady_gstri.dxil") DXIL(gs_linea, "ady_gslinea.dxil") DXIL(ps_blanco, "ady_ps.dxil") ".text\n");
extern "C" const unsigned char vs_id[], vs_id_fin[], gs_tri[], gs_tri_fin[], gs_linea[], gs_linea_fin[], ps_blanco[], ps_blanco_fin[];

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

static const UINT W = 8, H = 8;

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

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *gs, const unsigned char *gs_fin, D3D12_PRIMITIVE_TOPOLOGY_TYPE tipo) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS = {vs_id, (SIZE_T)(vs_id_fin - vs_id)};
    if (gs)
        p.GS = {gs, (SIZE_T)(gs_fin - gs)};
    p.PS = {ps_blanco, (SIZE_T)(ps_blanco_fin - ps_blanco)};
    p.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.PrimitiveTopologyType = tipo;
    p.NumRenderTargets = 1;
    p.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
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

// Donde apunta cada dibujo con GS en `sal` (la cuenta, en +240), y donde
// quedan las dos imagenes en lo leido.
static const UINT BASE_A = 0, BASE_B = 256, BASE_C = 512, BASE_D = 768, BASE_E = 1024, BASE_H = 1280, SAL = 2048;
static const UINT64 EN_F = 4096, EN_G = 8192, LEIDO = 12288;

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

    // La raiz, vista por todas las etapas: [0] dos constantes (b0: base y
    // tabla), [1] el UAV de `sal` (u0), en la raiz.
    D3D12_ROOT_PARAMETER rp[2] = {};
    rp[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    rp[0].Constants.Num32BitValues = 2;
    rp[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    D3D12_ROOT_SIGNATURE_DESC rd = {2, rp, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;
    ID3D12PipelineState *p_tri = pso(d, rs, gs_tri, gs_tri_fin, D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE);
    ID3D12PipelineState *p_linea = pso(d, rs, gs_linea, gs_linea_fin, D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE);
    ID3D12PipelineState *p_sin = pso(d, rs, nullptr, nullptr, D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE);
    if (fallos)
        return fallos;

    const D3D12_RESOURCE_STATES RT = D3D12_RESOURCE_STATE_RENDER_TARGET, UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS, CS = D3D12_RESOURCE_STATE_COPY_SOURCE;
    const DXGI_FORMAT RGBA8 = DXGI_FORMAT_R8G8B8A8_UNORM;
    ID3D12Resource *sal = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_BUFFER, SAL, 1, DXGI_FORMAT_UNKNOWN, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, UA);
    ID3D12Resource *rt_f = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, W, H, RGBA8, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, RT);
    ID3D12Resource *rt_g = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, W, H, RGBA8, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, RT);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, D3D12_RESOURCE_DIMENSION_BUFFER, LEIDO, 1, DXGI_FORMAT_UNKNOWN, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE vf, vg;
    rtvs->GetCPUDescriptorHandleForHeapStart(&vf);
    vg.ptr = vf.ptr + d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
    d->CreateRenderTargetView(rt_f, nullptr, vf);
    d->CreateRenderTargetView(rt_g, nullptr, vg);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, p_tri, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    const FLOAT negro[4] = {0, 0, 0, 0};
    l->ClearRenderTargetView(vf, negro, 0, nullptr);
    l->ClearRenderTargetView(vg, negro, 0, nullptr);
    l->SetGraphicsRootSignature(rs);
    l->SetGraphicsRootUnorderedAccessView(1, sal->GetGPUVirtualAddress());
    D3D12_VIEWPORT vp = {0, 0, (FLOAT)W, (FLOAT)H, 0, 1};
    D3D12_RECT tijera = {0, 0, (LONG)W, (LONG)H};
    l->RSSetViewports(1, &vp);
    l->RSSetScissorRects(1, &tijera);
    l->OMSetRenderTargets(1, &vf, FALSE, nullptr);
    // Un dibujo: su topologia, sus vertices, donde apunta y su tabla.
    auto dibujo = [&](D3D_PRIMITIVE_TOPOLOGY t, UINT n, UINT base, UINT tabla) {
        const UINT c[2] = {base, tabla};
        l->SetGraphicsRoot32BitConstants(0, 2, c, 0);
        l->IASetPrimitiveTopology(t);
        l->DrawInstanced(n, 1, 0, 0);
    };
    dibujo(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ, 15, BASE_A, 0);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ, 10, BASE_B, 0);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ, 7, BASE_C, 0);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ, 8, BASE_H, 0);
    l->SetPipelineState(p_linea);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ, 9, BASE_D, 0);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ, 6, BASE_E, 0);
    l->SetPipelineState(p_sin);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ, 6, 0, 1);
    l->OMSetRenderTargets(1, &vg, FALSE, nullptr);
    dibujo(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ, 8, 0, 2);

    transicion(l, sal, UA, CS);
    transicion(l, rt_f, RT, CS);
    transicion(l, rt_g, RT, CS);
    l->CopyBufferRegion(leida, 0, sal, 0, SAL);
    D3D12_RESOURCE_DESC dt;
    rt_f->GetDesc(&dt);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT hf, hg;
    d->GetCopyableFootprints(&dt, 0, 1, EN_F, &hf, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dt, 0, 1, EN_G, &hg, nullptr, nullptr, nullptr);
    auto copiar = [&](ID3D12Resource *r, const D3D12_PLACED_SUBRESOURCE_FOOTPRINT &h) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint = h;
        de.pResource = r;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    };
    copiar(rt_f, hf);
    copiar(rt_g, hg);
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
    char msg[300];

    // A..E: lo que apunto el GS de cada dibujo: `n` primitivas de `k`
    // vertices (en primitivas de `paso` palabras), y su cuenta.
    auto mirar = [&](const char *que, UINT base, UINT k, UINT paso, UINT n, const UINT *quiero) {
        const UINT *s = (const UINT *)(m + base);
        bool bien = s[60] == n;
        int w = snprintf(msg, sizeof msg, "%s: %u primitivas (pide %u):", que, s[60], n);
        for (UINT p = 0; p < n; p++) {
            w += snprintf(msg + w, sizeof msg - w, " (");
            for (UINT j = 0; j < k; j++) {
                bien = bien && s[p * paso + j] == quiero[p * k + j];
                w += snprintf(msg + w, sizeof msg - w, j ? " %u" : "%u", s[p * paso + j]);
            }
            w += snprintf(msg + w, sizeof msg - w, ")");
        }
        decir(bien, msg);
    };
    const UINT a[12] = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11};
    mirar("A, TRIANGLELIST_ADJ con un GS triangleadj, Draw(15)", BASE_A, 6, 8, 2, a);
    const UINT b[18] = {0, 1, 2, 6, 4, 3, 2, 5, 6, 8, 4, 0, 4, 2, 6, 9, 8, 7};
    mirar("B, TRIANGLESTRIP_ADJ, Draw(10)", BASE_B, 6, 8, 3, b);
    const UINT c[6] = {0, 1, 2, 5, 4, 3};
    mirar("C, TRIANGLESTRIP_ADJ de un triangulo, Draw(7)", BASE_C, 6, 8, 1, c);
    const UINT dd[8] = {0, 1, 2, 3, 4, 5, 6, 7};
    mirar("D, LINELIST_ADJ con un GS lineadj, Draw(9)", BASE_D, 4, 4, 2, dd);
    const UINT e[12] = {0, 1, 2, 3, 1, 2, 3, 4, 2, 3, 4, 5};
    mirar("E, LINESTRIP_ADJ, Draw(6)", BASE_E, 4, 4, 3, e);

    // F y G: cuantos pixeles quedaron blancos, de 64.
    auto blancos = [&](const D3D12_PLACED_SUBRESOURCE_FOOTPRINT &h) {
        UINT n = 0;
        for (UINT y = 0; y < H; y++)
            for (UINT x = 0; x < W; x++)
                n += *(const UINT *)(m + h.Offset + (UINT64)y * h.Footprint.RowPitch + 4 * x) == 0xFFFFFFFFu;
        return n;
    };
    UINT nf = blancos(hf), ng = blancos(hg);
    snprintf(msg, sizeof msg, "F, TRIANGLELIST_ADJ sin GS: se pinta el triangulo de los vertices 0, 2 y 4 (%u de 64 pixeles)", nf);
    decir(nf == 64, msg);
    snprintf(msg, sizeof msg, "G, TRIANGLESTRIP_ADJ sin GS: los triangulos (0 2 4) y (4 2 6) (%u de 64 pixeles)", ng);
    decir(ng == 64, msg);

    const UINT hh[12] = {0, 1, 2, 6, 4, 3, 2, 5, 6, 7, 4, 0};
    mirar("H, TRIANGLESTRIP_ADJ de dos, Draw(8)", BASE_H, 6, 8, 2, hh);

    printf("adyacencia.exe: las topologias con adyacencia son las de Windows\n");
    return fallos;
}
