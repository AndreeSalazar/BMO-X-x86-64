// instancias.cpp -- el juez de las INSTANCIAS de D3D12 (N5.13 de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): lo que todo juego usa para el
// follaje, la gente y los coches, y que la casa dibujaba una vez. Dos
// dibujos en un destino de 64 x 64 (R8G8B8A8), con los sombreadores de
// instancias.hlsl (aqui al lado):
//
//   A  DrawIndexedInstanced(6, 6, 0, 0, 2) con TRES buferes de vertices: la
//      esquina por vertice (ranura 0), el sitio y el color por instancia
//      (ranura 1, StepRate 1) y la fila por instancia (ranura 2, StepRate
//      2). StartInstanceLocation = 2 mueve lo que se lee por instancia, y
//      SV_InstanceID cuenta igual desde 0.
//   B  DrawInstanced(6, 2, 0, 5) SIN bufer de vertices (IASetVertexBuffers
//      con NULL los quita): el rectangulo sale de SV_VertexID, una banda por
//      instancia; con 5 de StartInstanceLocation, SV_InstanceID es 0 y 1.
//
// Lo leido se compara con la cuenta, pixel a pixel y BIT A BIT: cada borde
// cae en un borde de pixel y cada color es k / 255. Sale con el numero de
// fallos. En Windows dice lo mismo (es de consola: lo que falle, se lee).
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

// Los sombreadores, de los .dxil de aqui al lado (HACER.txt dice como salen).
__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_inst\n vs_inst:\n .incbin \"instancias_vs.dxil\"\n .globl vs_inst_fin\n vs_inst_fin:\n"
        ".p2align 4\n"
        ".globl vs_id\n vs_id:\n .incbin \"instancias_id.dxil\"\n .globl vs_id_fin\n vs_id_fin:\n"
        ".p2align 4\n"
        ".globl ps_color\n ps_color:\n .incbin \"instancias_ps.dxil\"\n .globl ps_color_fin\n ps_color_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_inst[], vs_inst_fin[], vs_id[], vs_id_fin[], ps_color[], ps_color_fin[];

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
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource");
    return b;
}

// Un bufer UPLOAD con `n` bytes de `datos`.
static ID3D12Resource *subido(ID3D12Device *d, const void *datos, UINT n) {
    ID3D12Resource *b = bufer(d, D3D12_HEAP_TYPE_UPLOAD, n, D3D12_RESOURCE_STATE_GENERIC_READ);
    void *p = nullptr;
    D3D12_RANGE nada = {0, 0};
    if (b && hecho(b->Map(0, &nada, &p), "Map"))
        memcpy(p, datos, n);
    return b;
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *vs, const unsigned char *vs_fin, const D3D12_INPUT_ELEMENT_DESC *ia, UINT n) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS.pShaderBytecode = vs;
    p.VS.BytecodeLength = (SIZE_T)(vs_fin - vs);
    p.PS.pShaderBytecode = ps_color;
    p.PS.BytecodeLength = (SIZE_T)(ps_color_fin - ps_color);
    p.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.InputLayout.pInputElementDescs = ia;
    p.InputLayout.NumElements = n;
    p.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    p.NumRenderTargets = 1;
    p.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
    p.SampleDesc.Count = 1;
    ID3D12PipelineState *x = nullptr;
    hecho(d->CreateGraphicsPipelineState(&p, IID_PPV_ARGS(&x)), "CreateGraphicsPipelineState");
    return x;
}

static UINT rgba(UINT r, UINT g, UINT b, UINT a) {
    return r | g << 8 | b << 16 | a << 24;
}

// Lo que tiene que salir: A, seis cuadros de 6 x 6; B, dos bandas.
static void esperado(UINT *img) {
    memset(img, 0, LADO * LADO * 4);
    for (UINT i = 0; i < 6; i++) {
        UINT k = 2 + i, m = 2 + i / 2;
        UINT x0 = 2 + 7 * k, y0 = 2 + 10 * m;
        for (UINT y = y0; y < y0 + 6; y++)
            for (UINT x = x0; x < x0 + 6; x++)
                img[y * LADO + x] = rgba(40 + 20 * k, i * 16, 200 - 20 * k, 255);
    }
    for (UINT i = 0; i < 2; i++)
        for (UINT y = 50 + 6 * i; y < 54 + 6 * i; y++)
            for (UINT x = 4; x < 60; x++)
                img[y * LADO + x] = rgba(255, i * 100, 0, 255);
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

    // La root signature: vacia, con el input layout.
    D3D12_ROOT_SIGNATURE_DESC rd = {};
    rd.Flags = D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT;
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;

    // A: tres ranuras; B: sin input layout.
    D3D12_INPUT_ELEMENT_DESC ia[4] = {
        {"ESQUINA", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
        {"SITIO", 0, DXGI_FORMAT_R32G32_FLOAT, 1, 0, D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA, 1},
        {"COLOR", 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 1, D3D12_APPEND_ALIGNED_ELEMENT, D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA, 1},
        {"FILA", 0, DXGI_FORMAT_R32_FLOAT, 2, 0, D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA, 2},
    };
    ID3D12PipelineState *psoa = pso(d, rs, vs_inst, vs_inst_fin, ia, 4);
    ID3D12PipelineState *psob = pso(d, rs, vs_id, vs_id_fin, nullptr, 0);
    if (fallos)
        return fallos;
    decir(true, "CreateGraphicsPipelineState: tres ranuras con datos por instancia, y otro sin input layout");

    // Los buferes: las esquinas, sus indices, 8 instancias de sitio y color
    // y 8 filas (de cada una se leen las que tocan).
    float esquinas[8] = {0, 0, 1, 0, 0, 1, 1, 1};
    unsigned short indices[6] = {0, 1, 2, 2, 1, 3};
    float sitios[8][6];
    float filas[8];
    for (UINT k = 0; k < 8; k++) {
        float s[6] = {(float)(2 + 7 * k), 0.0f, (40 + 20 * k) / 255.0f, 0.0f, (200 - 20 * k) / 255.0f, 1.0f};
        memcpy(sitios[k], s, sizeof s);
        filas[k] = (float)(2 + 10 * k);
    }
    ID3D12Resource *vb0 = subido(d, esquinas, sizeof esquinas), *ib = subido(d, indices, sizeof indices);
    ID3D12Resource *vb1 = subido(d, sitios, sizeof sitios), *vb2 = subido(d, filas, sizeof filas);

    // El destino (64 x 64, RTV) y donde se lee.
    D3D12_HEAP_PROPERTIES hp = {};
    hp.Type = D3D12_HEAP_TYPE_DEFAULT;
    D3D12_RESOURCE_DESC td = {};
    td.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    td.Width = LADO;
    td.Height = LADO;
    td.DepthOrArraySize = 1;
    td.MipLevels = 1;
    td.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    td.SampleDesc.Count = 1;
    td.Flags = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    ID3D12Resource *rt = nullptr;
    hecho(d->CreateCommittedResource(&hp, D3D12_HEAP_FLAG_NONE, &td, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr, IID_PPV_ARGS(&rt)), "CreateCommittedResource del destino");
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, LADO * LADO * 4, D3D12_RESOURCE_STATE_COPY_DEST);
    D3D12_DESCRIPTOR_HEAP_DESC hd = {};
    hd.Type = D3D12_DESCRIPTOR_HEAP_TYPE_RTV;
    hd.NumDescriptors = 1;
    ID3D12DescriptorHeap *rtvs = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap") || fallos)
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE rtv;
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv);
    d->CreateRenderTargetView(rt, nullptr, rtv);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, psoa, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    const float negro[4] = {0, 0, 0, 0};
    l->ClearRenderTargetView(rtv, negro, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv, FALSE, nullptr);
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    D3D12_RECT tijera = {0, 0, (LONG)LADO, (LONG)LADO};
    l->RSSetViewports(1, &vp);
    l->RSSetScissorRects(1, &tijera);
    l->SetGraphicsRootSignature(rs);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // A: las tres ranuras de una vez.
    D3D12_VERTEX_BUFFER_VIEW vbs[3] = {
        {vb0->GetGPUVirtualAddress(), sizeof esquinas, 8},
        {vb1->GetGPUVirtualAddress(), sizeof sitios, 24},
        {vb2->GetGPUVirtualAddress(), sizeof filas, 4},
    };
    l->IASetVertexBuffers(0, 3, vbs);
    D3D12_INDEX_BUFFER_VIEW ibv = {ib->GetGPUVirtualAddress(), sizeof indices, DXGI_FORMAT_R16_UINT};
    l->IASetIndexBuffer(&ibv);
    l->DrawIndexedInstanced(6, 6, 0, 0, 2);
    // B: sin buferes.
    l->IASetVertexBuffers(0, 3, nullptr);
    l->SetPipelineState(psob);
    l->DrawInstanced(6, 2, 0, 5);
    // Leer.
    D3D12_RESOURCE_BARRIER b = {};
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
    b.Transition.pResource = rt;
    b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
    b.Transition.StateBefore = D3D12_RESOURCE_STATE_RENDER_TARGET;
    b.Transition.StateAfter = D3D12_RESOURCE_STATE_COPY_SOURCE;
    l->ResourceBarrier(1, &b);
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = leida;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Footprint = {DXGI_FORMAT_R8G8B8A8_UNORM, LADO, LADO, 1, LADO * 4};
    de.pResource = rt;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    de.SubresourceIndex = 0;
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
    void *p = nullptr;
    if (!hecho(leida->Map(0, nullptr, &p), "Map de lo leido"))
        return fallos;
    const UINT *img = (const UINT *)p;

    static UINT quiero[LADO * LADO];
    esperado(quiero);
    // A en las filas 0..49, B en las 50..63.
    for (int parte = 0; parte < 2; parte++) {
        UINT y0 = parte ? 50 : 0, y1 = parte ? LADO : 50, malos = 0, primero = 0;
        for (UINT i = y0 * LADO; i < y1 * LADO; i++)
            if (img[i] != quiero[i] && malos++ == 0)
                primero = i;
        char m[240];
        if (malos == 0)
            snprintf(m, sizeof m, parte ? "B, sin bufer de vertices (SV_VertexID, 2 instancias desde la 5): las dos bandas, pixel a pixel"
                                        : "A, 6 instancias desde la 2 con tres ranuras (StepRate 1 y 2): los seis cuadros, pixel a pixel");
        else
            snprintf(m, sizeof m, "%s: %u pixeles distintos; el (%u, %u) es %08x y tenia que ser %08x", parte ? "B" : "A", malos, primero % LADO, primero / LADO, img[primero],
                     quiero[primero]);
        decir(malos == 0, m);
    }
    printf("instancias.exe: las instancias de D3D12 son las de Windows\n");
    return fallos;
}
