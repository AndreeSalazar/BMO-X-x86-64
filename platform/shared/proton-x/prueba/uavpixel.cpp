// uavpixel.cpp -- el juez de los UAV escritos desde un DIBUJO (05-10, la
// fila "un UAV en un sombreador de DIBUJO" de la 7.2 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md): lo que hace un juego que cuenta
// pixeles, construye listas o escribe un buffer de visibilidad desde el
// sombreador de pixeles. Un render target de 64 x 64 y dos dibujos del
// cuadro de pantalla completa sin bufer de vertices de uavpixel.hlsl (aqui
// al lado), con tres UAV:
//
//   A  RWTexture2D<uint> de 64 x 64 (R32_UINT, en una TABLA, u1), limpiada a
//      0xDEADBEEF: con la tijera en la mitad izquierda, cada pixel escribe
//      su posicion (y << 16 | x) en SU texel. La mitad derecha queda limpia.
//   B  RWByteAddressBuffer (en la RAIZ, SetGraphicsRootUnorderedAccessView,
//      u2), limpiado a 0: con la tijera en (8, 8)-(40, 24), cada pixel hace
//      InterlockedAdd(0, 1). Son 32 x 16 = 512 pixeles: el contador, 512.
//   C  RWBuffer<uint> de 8 (R32_UINT, en otra tabla, u3), limpiado a
//      0xFFFFFFFF: el de VERTICES escribe 100 + SV_VertexID en el suyo; los
//      seis primeros, 100..105, y los dos de detras sin tocar.
//
// Solo resultados que no dependen del orden de los pixeles (D3D no da
// ninguno): cada pixel en lo suyo, y una suma. Sin Z ni discard. Todo
// entero: lo que tiene que salir esta escrito abajo, con su cuenta. Sale
// con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_cuadro\n vs_cuadro:\n .incbin \"uavpixel_vs.dxil\"\n .globl vs_cuadro_fin\n vs_cuadro_fin:\n"
        ".p2align 4\n"
        ".globl ps_posicion\n ps_posicion:\n .incbin \"uavpixel_pos.dxil\"\n .globl ps_posicion_fin\n ps_posicion_fin:\n"
        ".p2align 4\n"
        ".globl ps_cuenta\n ps_cuenta:\n .incbin \"uavpixel_cuenta.dxil\"\n .globl ps_cuenta_fin\n ps_cuenta_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_cuadro[], vs_cuadro_fin[], ps_posicion[], ps_posicion_fin[], ps_cuenta[], ps_cuenta_fin[];

static const UINT LADO = 64;
// Lo leido: A en filas de 256 bytes (64 x 4: lo que pide D3D12), y detras
// el contador de B y los 8 de C.
static const UINT64 DESDE_B = LADO * 256, DESDE_C = DESDE_B + 256, LEIDO = DESDE_C + 256;
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

static ID3D12Resource *recurso(ID3D12Device *d, D3D12_HEAP_TYPE monton, D3D12_RESOURCE_DIMENSION dim, UINT64 ancho, UINT alto, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
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
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource");
    return b;
}

static ID3D12Resource *bufer(ID3D12Device *d, D3D12_HEAP_TYPE monton, UINT64 n, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e) {
    return recurso(d, monton, D3D12_RESOURCE_DIMENSION_BUFFER, n, 1, DXGI_FORMAT_UNKNOWN, fl, e);
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

// Entre la limpieza y el dibujo que escribe el mismo UAV.
static void barrera_uav(ID3D12GraphicsCommandList *l, ID3D12Resource *b) {
    D3D12_RESOURCE_BARRIER x = {};
    x.Type = D3D12_RESOURCE_BARRIER_TYPE_UAV;
    x.UAV.pResource = b;
    l->ResourceBarrier(1, &x);
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *ps, const unsigned char *ps_fin) {
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
    p.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
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

    // La raiz, vista por TODAS las etapas (el de vertices escribe C): [0] la
    // tabla de A (u1), [1] el UAV de B en la raiz (u2), [2] la tabla de C (u3).
    D3D12_DESCRIPTOR_RANGE ra = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 1, 0, 0};
    D3D12_DESCRIPTOR_RANGE rc = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 3, 0, 0};
    D3D12_ROOT_PARAMETER ps[3] = {};
    ps[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[0].DescriptorTable.NumDescriptorRanges = 1;
    ps[0].DescriptorTable.pDescriptorRanges = &ra;
    ps[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    ps[1].Descriptor.ShaderRegister = 2;
    ps[2].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[2].DescriptorTable.NumDescriptorRanges = 1;
    ps[2].DescriptorTable.pDescriptorRanges = &rc;
    for (D3D12_ROOT_PARAMETER &p : ps)
        p.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_SIGNATURE_DESC rd = {3, ps, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;

    ID3D12PipelineState *pa = pso(d, rs, ps_posicion, ps_posicion_fin), *pb = pso(d, rs, ps_cuenta, ps_cuenta_fin);
    const D3D12_RESOURCE_FLAGS UAV = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    const D3D12_RESOURCE_STATES EN_UAV = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    ID3D12Resource *rt = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, LADO, LADO, DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET);
    ID3D12Resource *ta = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, LADO, LADO, DXGI_FORMAT_R32_UINT, UAV, EN_UAV);
    ID3D12Resource *bb = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 256, UAV, EN_UAV), *bc = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 256, UAV, EN_UAV);
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, LEIDO, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    decir(true, "un render target, tres UAV (textura, crudo y con tipo) y dos PSO que los escriben");

    // Los descriptores: el RTV; los UAV en el monton que ve el sombreador
    // ([0] A, [1] C, [2] B crudo) y los mismos en el de la CPU, que es el
    // que piden las limpiezas.
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 3, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr, *visible = nullptr, *de_cpu = nullptr;
    hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap del RTV");
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&visible)), "CreateDescriptorHeap visible");
    hd.Flags = D3D12_DESCRIPTOR_HEAP_FLAG_NONE;
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&de_cpu)), "CreateDescriptorHeap de CPU");
    if (fallos)
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE rtv, cv, cc;
    D3D12_GPU_DESCRIPTOR_HANDLE gv;
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv);
    visible->GetCPUDescriptorHandleForHeapStart(&cv);
    visible->GetGPUDescriptorHandleForHeapStart(&gv);
    de_cpu->GetCPUDescriptorHandleForHeapStart(&cc);
    d->CreateRenderTargetView(rt, nullptr, rtv);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    auto en = [&](D3D12_CPU_DESCRIPTOR_HANDLE h, UINT i) { return D3D12_CPU_DESCRIPTOR_HANDLE{h.ptr + (SIZE_T)i * paso}; };
    auto eng = [&](UINT i) { return D3D12_GPU_DESCRIPTOR_HANDLE{gv.ptr + (UINT64)i * paso}; };
    D3D12_UNORDERED_ACCESS_VIEW_DESC va = {}, vc = {}, vb = {};
    va.Format = DXGI_FORMAT_R32_UINT;
    va.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
    vc.Format = DXGI_FORMAT_R32_UINT;
    vc.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    vc.Buffer.NumElements = 8;
    vb.Format = DXGI_FORMAT_R32_TYPELESS;
    vb.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    vb.Buffer.NumElements = 64;
    vb.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    const D3D12_CPU_DESCRIPTOR_HANDLE montones[2] = {cv, cc};
    for (D3D12_CPU_DESCRIPTOR_HANDLE h : montones) {
        d->CreateUnorderedAccessView(ta, nullptr, &va, en(h, 0));
        d->CreateUnorderedAccessView(bc, nullptr, &vc, en(h, 1));
        d->CreateUnorderedAccessView(bb, nullptr, &vb, en(h, 2));
    }

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pa, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetDescriptorHeaps(1, &visible);
    // Las limpiezas: A a 0xDEADBEEF, B a 0, C a 0xFFFFFFFF.
    const UINT muerto[4] = {0xDEADBEEF, 0, 0, 0}, cero[4] = {0, 0, 0, 0}, unos[4] = {0xFFFFFFFF, 0, 0, 0};
    l->ClearUnorderedAccessViewUint(eng(0), en(cc, 0), ta, muerto, 0, nullptr);
    l->ClearUnorderedAccessViewUint(eng(1), en(cc, 1), bc, unos, 0, nullptr);
    l->ClearUnorderedAccessViewUint(eng(2), en(cc, 2), bb, cero, 0, nullptr);
    barrera_uav(l, ta);
    barrera_uav(l, bb);
    barrera_uav(l, bc);
    const float negro[4] = {0, 0, 0, 1};
    l->ClearRenderTargetView(rtv, negro, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv, FALSE, nullptr);
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    D3D12_RECT mitad = {0, 0, (LONG)LADO / 2, (LONG)LADO}, caja = {8, 8, 40, 24};
    l->RSSetViewports(1, &vp);
    l->SetGraphicsRootSignature(rs);
    l->SetGraphicsRootDescriptorTable(0, eng(0));
    l->SetGraphicsRootUnorderedAccessView(1, bb->GetGPUVirtualAddress());
    l->SetGraphicsRootDescriptorTable(2, eng(1));
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // A (y C): la mitad izquierda.
    l->RSSetScissorRects(1, &mitad);
    l->DrawInstanced(6, 1, 0, 0);
    // B (y C otra vez, con lo mismo): la caja de 32 x 16.
    l->SetPipelineState(pb);
    l->RSSetScissorRects(1, &caja);
    l->DrawInstanced(6, 1, 0, 0);
    // Leer los tres.
    barrera(l, ta, EN_UAV, D3D12_RESOURCE_STATE_COPY_SOURCE);
    barrera(l, bb, EN_UAV, D3D12_RESOURCE_STATE_COPY_SOURCE);
    barrera(l, bc, EN_UAV, D3D12_RESOURCE_STATE_COPY_SOURCE);
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = leida;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Footprint = {DXGI_FORMAT_R32_UINT, LADO, LADO, 1, 256};
    de.pResource = ta;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    de.SubresourceIndex = 0;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    l->CopyBufferRegion(leida, DESDE_B, bb, 0, 4);
    l->CopyBufferRegion(leida, DESDE_C, bc, 0, 32);
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
    const unsigned char *b = (const unsigned char *)p;
    char m[240];
    // A: a la izquierda (x < 32) y << 16 | x; a la derecha, la limpieza.
    UINT malos = 0, mx = 0, my = 0, visto = 0, quiero = 0;
    for (UINT y = 0; y < LADO; y++)
        for (UINT x = 0; x < LADO; x++) {
            UINT v = ((const UINT *)(b + y * 256))[x], q = x < LADO / 2 ? (y << 16 | x) : 0xDEADBEEFu;
            if (v != q && malos++ == 0)
                mx = x, my = y, visto = v, quiero = q;
        }
    if (malos == 0)
        snprintf(m, sizeof m, "A, RWTexture2D<uint> desde el de pixeles: cada pixel de la mitad izquierda su posicion, la derecha limpia");
    else
        snprintf(m, sizeof m, "A: %u texeles distintos; el (%u, %u) es %08x y tenia que ser %08x", malos, mx, my, visto, quiero);
    decir(malos == 0, m);
    // B: 32 x 16 = 512.
    UINT n = *(const UINT *)(b + DESDE_B);
    snprintf(m, sizeof m, "B, InterlockedAdd en un RWByteAddressBuffer de la raiz: %u pixeles cubiertos (tenian que ser 512)", n);
    decir(n == 512, m);
    // C: 100..105, y los dos de detras como se limpiaron.
    const UINT c_quiero[8] = {100, 101, 102, 103, 104, 105, 0xFFFFFFFFu, 0xFFFFFFFFu};
    const UINT *c = (const UINT *)(b + DESDE_C);
    bool c_bien = memcmp(c, c_quiero, sizeof c_quiero) == 0;
    snprintf(m, sizeof m, "C, RWBuffer<uint> desde el de vertices: %u %u %u %u %u %u %08x %08x (100..105 y dos sin tocar)", c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]);
    decir(c_bien, m);
    leida->Unmap(0, nullptr);
    printf("uavpixel.exe: los UAV de un dibujo son los de Windows\n");
    return fallos;
}
