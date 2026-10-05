// stencil.cpp -- el juez del STENCIL de D3D12 (05-10; la fila del stencil
// de la tabla 7.2 de docs/plan/PLAN_LA_ESCALERA_PROTON_X.md): lo que hace
// un juego que recorta con stencil -- marcar una region y pintar luego SOLO
// alli (las mascaras de luz y las calcomanias de Cyberpunk). Cuatro destinos
// RGBA8 de 64 x 64, cada uno con su DSV, y un cuadro de pantalla completa
// (un bufer de vertices; el segundo, girado: sus caras son las de DETRAS)
// con los sombreadores de stencil.hlsl (aqui al lado):
//
//   A  D24S8. Marcar la mitad izquierda con REPLACE (referencia 1, de
//      OMSetStencilRef) pintando rojo, y pintar verde la pantalla entera con
//      EQUAL 1: verde a la izquierda; a la derecha, lo limpio (negro).
//   B  D24S8 limpio SOLO de stencil a 0xFD. INCR_SAT dos veces en todo
//      (0xFF) y otra en la izquierda (se queda en 0xFF: no da la vuelta);
//      REPLACE 0 con la mascara de ESCRITURA 0x0F en la derecha (0xF0).
//   C  D32_FLOAT_S8X24_UINT limpio a 0x10. Con la cara de DELANTE, INCR a la
//      izquierda (0x11); con la de DETRAS (el cuadro girado), DECR a la
//      derecha (0x0F).
//   D  Un R24G8_TYPELESS (vista D24S8), con dibujos de SOLO profundidad (sin
//      render target ni sombreador de pixeles): REPLACE 3 a la izquierda, Z
//      0.5 arriba, y luego EQUAL 3 con Z LESS: falla el stencil (INVERT), la
//      Z (INCR_SAT) o nada (ZERO). Arriba a la izquierda 4, abajo 0, a la
//      derecha 0xFF.
//
// El stencil se lee por el COLOR: tres sondas a pantalla completa con EQUAL
// (y su referencia y su mascara de lectura), cada una escribiendo SOLO su
// canal (R, G o B) en blanco. Lo que tiene que salir esta escrito abajo, en
// bits; sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_cuadro\n vs_cuadro:\n .incbin \"stencil_vs.dxil\"\n .globl vs_cuadro_fin\n vs_cuadro_fin:\n"
        ".p2align 4\n"
        ".globl ps_color\n ps_color:\n .incbin \"stencil_ps.dxil\"\n .globl ps_color_fin\n ps_color_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_cuadro[], vs_cuadro_fin[], ps_color[], ps_color_fin[];

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

static ID3D12Resource *bufer(ID3D12Device *d, D3D12_HEAP_TYPE tipo, UINT64 bytes, D3D12_RESOURCE_STATES estado) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = tipo;
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    r.Width = bytes;
    r.Height = 1;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.SampleDesc.Count = 1;
    r.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, estado, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource de un bufer");
    return b;
}

static ID3D12Resource *textura(ID3D12Device *d, DXGI_FORMAT f, bool profundidad) {
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
    td.Flags = profundidad ? D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL : D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    ID3D12Resource *t = nullptr;
    hecho(d->CreateCommittedResource(&hp, D3D12_HEAP_FLAG_NONE, &td, profundidad ? D3D12_RESOURCE_STATE_DEPTH_WRITE : D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr, IID_PPV_ARGS(&t)),
          profundidad ? "CreateCommittedResource de una profundidad con stencil" : "CreateCommittedResource de un destino");
    return t;
}

static D3D12_DEPTH_STENCILOP_DESC cara(D3D12_STENCIL_OP falla, D3D12_STENCIL_OP falla_z, D3D12_STENCIL_OP pasa, D3D12_COMPARISON_FUNC f) {
    D3D12_DEPTH_STENCILOP_DESC c = {falla, falla_z, pasa, f};
    return c;
}

static const D3D12_STENCIL_OP K = D3D12_STENCIL_OP_KEEP;

// Un PSO del cuadro: con el de pixeles y un RGBA8 (`canales`: su mascara de
// escritura) o de SOLO profundidad; la Z (LESS) si `z`; el stencil si `st`.
static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, bool rt, UINT8 canales, bool z, bool z_escribe, bool st, UINT8 lee, UINT8 escribe, D3D12_DEPTH_STENCILOP_DESC delante,
                                D3D12_DEPTH_STENCILOP_DESC detras, DXGI_FORMAT dsv) {
    static const D3D12_INPUT_ELEMENT_DESC ia[1] = {{"POSITION", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0}};
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS.pShaderBytecode = vs_cuadro;
    p.VS.BytecodeLength = (SIZE_T)(vs_cuadro_fin - vs_cuadro);
    if (rt) {
        p.PS.pShaderBytecode = ps_color;
        p.PS.BytecodeLength = (SIZE_T)(ps_color_fin - ps_color);
        p.NumRenderTargets = 1;
        p.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        p.BlendState.RenderTarget[0].RenderTargetWriteMask = canales;
    }
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.DepthStencilState.DepthEnable = z;
    p.DepthStencilState.DepthWriteMask = z_escribe ? D3D12_DEPTH_WRITE_MASK_ALL : D3D12_DEPTH_WRITE_MASK_ZERO;
    p.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_LESS;
    p.DepthStencilState.StencilEnable = st;
    p.DepthStencilState.StencilReadMask = lee;
    p.DepthStencilState.StencilWriteMask = escribe;
    p.DepthStencilState.FrontFace = delante;
    p.DepthStencilState.BackFace = detras;
    p.InputLayout = {ia, 1};
    p.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    p.DSVFormat = dsv;
    p.SampleDesc.Count = 1;
    ID3D12PipelineState *x = nullptr;
    hecho(d->CreateGraphicsPipelineState(&p, IID_PPV_ARGS(&x)), "CreateGraphicsPipelineState");
    return x;
}

// Una sonda: pinta de blanco SOLO el canal `canal` donde el stencil, con la
// mascara de lectura `lee`, es igual a la referencia (la de la lista).
static ID3D12PipelineState *sonda(ID3D12Device *d, ID3D12RootSignature *rs, UINT8 canal, UINT8 lee, DXGI_FORMAT dsv) {
    D3D12_DEPTH_STENCILOP_DESC igual = cara(K, K, K, D3D12_COMPARISON_FUNC_EQUAL);
    return pso(d, rs, true, canal, false, false, true, lee, 0, igual, igual, dsv);
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

static void copiar(ID3D12GraphicsCommandList *l, ID3D12Resource *t, ID3D12Resource *b) {
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = b;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Footprint = {DXGI_FORMAT_R8G8B8A8_UNORM, LADO, LADO, 1, LADO * 4};
    de.pResource = t;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    de.SubresourceIndex = 0;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
}

// Cada texel de un destino leido contra lo que tiene que salir (`quiero(x, y)`).
template <typename Q> static void juzgar(ID3D12Resource *b, Q quiero, const char *bien) {
    void *p = nullptr;
    if (!hecho(b->Map(0, nullptr, &p), "Map de lo leido"))
        return;
    const UINT *v = (const UINT *)p;
    UINT malos = 0, primero = 0;
    for (UINT i = 0; i < LADO * LADO; i++) {
        if (v[i] != quiero(i % LADO, i / LADO) && malos++ == 0)
            primero = i;
    }
    char m[240];
    if (malos == 0)
        snprintf(m, sizeof m, "%s", bien);
    else
        snprintf(m, sizeof m, "%s: %u texeles distintos; el (%u, %u) es %08x y tenia que ser %08x", bien, malos, primero % LADO, primero / LADO, v[primero], quiero(primero % LADO, primero / LADO));
    decir(malos == 0, m);
    b->Unmap(0, nullptr);
}

// Lo que tiene que salir, en bits: RGBA8 en memoria es R en el byte bajo
// (0xAABBGGRR). Una sonda en blanco que pasa deja 0xFF en SU canal.
static const UINT NEGRO = 0xFF000000u, VERDE = 0xFF00FF00u;
static const UINT R = 0x000000FFu, G = 0x0000FF00u, B = 0x00FF0000u;

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

    // La raiz: cuatro constantes (b0, el color) y el input layout.
    D3D12_ROOT_PARAMETER rp = {};
    rp.ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    rp.Constants.Num32BitValues = 4;
    rp.ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_ROOT_SIGNATURE_DESC rd = {1, &rp, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;

    // El cuadro: dos triangulos en el sentido de las agujas (la cara de
    // DELANTE, con FrontCounterClockwise FALSE) y los mismos al reves.
    static const float cuadro[24] = {-1, 1, 1, 1, -1, -1, 1, 1, 1, -1, -1, -1, -1, 1, -1, -1, 1, 1, 1, 1, -1, -1, 1, -1};
    ID3D12Resource *vb = bufer(d, D3D12_HEAP_TYPE_UPLOAD, sizeof cuadro, D3D12_RESOURCE_STATE_GENERIC_READ);
    void *m = nullptr;
    if (vb && hecho(vb->Map(0, nullptr, &m), "Map del bufer de vertices")) {
        memcpy(m, cuadro, sizeof cuadro);
        vb->Unmap(0, nullptr);
    }

    const DXGI_FORMAT D24 = DXGI_FORMAT_D24_UNORM_S8_UINT, D32 = DXGI_FORMAT_D32_FLOAT_S8X24_UINT;
    const DXGI_FORMAT recurso_z[4] = {D24, D24, D32, DXGI_FORMAT_R24G8_TYPELESS}, vista_z[4] = {D24, D24, D32, D24};
    ID3D12Resource *rt[4], *ds[4], *leido[4];
    for (int k = 0; k < 4; k++) {
        rt[k] = textura(d, DXGI_FORMAT_R8G8B8A8_UNORM, false);
        ds[k] = textura(d, recurso_z[k], true);
        leido[k] = bufer(d, D3D12_HEAP_TYPE_READBACK, LADO * LADO * 4, D3D12_RESOURCE_STATE_COPY_DEST);
    }
    if (fallos)
        return fallos;

    const D3D12_STENCIL_OP REPLACE = D3D12_STENCIL_OP_REPLACE;
    const D3D12_COMPARISON_FUNC SIEMPRE = D3D12_COMPARISON_FUNC_ALWAYS, IGUAL = D3D12_COMPARISON_FUNC_EQUAL;
    D3D12_DEPTH_STENCILOP_DESC marca = cara(K, K, REPLACE, SIEMPRE), igual = cara(K, K, K, IGUAL), nada = cara(K, K, K, SIEMPRE);
    D3D12_DEPTH_STENCILOP_DESC suma = cara(K, K, D3D12_STENCIL_OP_INCR_SAT, SIEMPRE);
    D3D12_DEPTH_STENCILOP_DESC mas = cara(K, K, D3D12_STENCIL_OP_INCR, SIEMPRE), menos = cara(K, K, D3D12_STENCIL_OP_DECR, SIEMPRE);
    D3D12_DEPTH_STENCILOP_DESC tres = cara(D3D12_STENCIL_OP_INVERT, D3D12_STENCIL_OP_INCR_SAT, D3D12_STENCIL_OP_ZERO, IGUAL);
    ID3D12PipelineState *a_marca = pso(d, rs, true, 0xF, false, false, true, 0xFF, 0xFF, marca, marca, D24);
    ID3D12PipelineState *a_igual = pso(d, rs, true, 0xF, false, false, true, 0xFF, 0x00, igual, igual, D24);
    ID3D12PipelineState *b_suma = pso(d, rs, true, 0, false, false, true, 0xFF, 0xFF, suma, suma, D24);
    ID3D12PipelineState *b_cero = pso(d, rs, true, 0, false, false, true, 0xFF, 0x0F, marca, marca, D24);
    ID3D12PipelineState *c_caras = pso(d, rs, true, 0, false, false, true, 0xFF, 0xFF, mas, menos, D32);
    ID3D12PipelineState *d_marca = pso(d, rs, false, 0, false, false, true, 0xFF, 0xFF, marca, marca, D24);
    ID3D12PipelineState *d_z = pso(d, rs, false, 0, true, true, false, 0xFF, 0xFF, nada, nada, D24);
    ID3D12PipelineState *d_tres = pso(d, rs, false, 0, true, false, true, 0xFF, 0xFF, tres, tres, D24);
    ID3D12PipelineState *sr = sonda(d, rs, D3D12_COLOR_WRITE_ENABLE_RED, 0xFF, D24), *sg = sonda(d, rs, D3D12_COLOR_WRITE_ENABLE_GREEN, 0xFF, D24);
    ID3D12PipelineState *sb = sonda(d, rs, D3D12_COLOR_WRITE_ENABLE_BLUE, 0xFF, D24), *sb_lee = sonda(d, rs, D3D12_COLOR_WRITE_ENABLE_BLUE, 0x0F, D24);
    ID3D12PipelineState *sr32 = sonda(d, rs, D3D12_COLOR_WRITE_ENABLE_RED, 0xFF, D32), *sg32 = sonda(d, rs, D3D12_COLOR_WRITE_ENABLE_GREEN, 0xFF, D32);
    if (fallos)
        return fallos;
    decir(true, "cuatro destinos con su profundidad (D24S8, D32S8X24 y un R24G8 TYPELESS) y sus PSO de stencil");

    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 4, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hz = {D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 4, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr, *dsvs = nullptr;
    hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV");
    hecho(d->CreateDescriptorHeap(&hz, IID_PPV_ARGS(&dsvs)), "CreateDescriptorHeap de los DSV");
    if (fallos)
        return fallos;
    UINT paso_r = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV), paso_z = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV);
    D3D12_CPU_DESCRIPTOR_HANDLE rtv[4], dsv[4];
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv[0]);
    dsvs->GetCPUDescriptorHandleForHeapStart(&dsv[0]);
    for (int k = 0; k < 4; k++) {
        rtv[k].ptr = rtv[0].ptr + k * paso_r;
        dsv[k].ptr = dsv[0].ptr + k * paso_z;
        d->CreateRenderTargetView(rt[k], nullptr, rtv[k]);
        D3D12_DEPTH_STENCIL_VIEW_DESC dv = {};
        dv.Format = vista_z[k];
        dv.ViewDimension = D3D12_DSV_DIMENSION_TEXTURE2D;
        d->CreateDepthStencilView(ds[k], &dv, dsv[k]);
    }

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, a_marca, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    const LONG M = LADO / 2;
    D3D12_RECT todo = {0, 0, (LONG)LADO, (LONG)LADO}, izquierda = {0, 0, M, (LONG)LADO}, derecha = {M, 0, (LONG)LADO, (LONG)LADO}, arriba = {0, 0, (LONG)LADO, M};
    D3D12_VERTEX_BUFFER_VIEW vista = {vb->GetGPUVirtualAddress(), sizeof cuadro, 8};
    const float rojo[4] = {1, 0, 0, 1}, verde[4] = {0, 1, 0, 1}, blanco[4] = {1, 1, 1, 1};
    const float negro[4] = {0, 0, 0, 1}, nada_rgba[4] = {0, 0, 0, 0};
    l->RSSetViewports(1, &vp);
    l->SetGraphicsRootSignature(rs);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    l->IASetVertexBuffers(0, 1, &vista);
    // Un dibujo del cuadro (de cara o girado) con su tijera y su referencia.
    auto dibujar = [&](ID3D12PipelineState *p, const D3D12_RECT &t, UINT referencia, bool delante) {
        l->SetPipelineState(p);
        l->RSSetScissorRects(1, &t);
        l->OMSetStencilRef(referencia);
        l->DrawInstanced(6, 1, delante ? 0 : 6, 0);
    };

    // A: marcar con REPLACE 1, pintar con EQUAL 1.
    l->ClearRenderTargetView(rtv[0], negro, 0, nullptr);
    l->ClearDepthStencilView(dsv[0], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[0], FALSE, &dsv[0]);
    l->SetGraphicsRoot32BitConstants(0, 4, rojo, 0);
    dibujar(a_marca, izquierda, 1, true);
    l->SetGraphicsRoot32BitConstants(0, 4, verde, 0);
    dibujar(a_igual, todo, 1, true);

    // B: INCR_SAT y las mascaras. Las sondas pintan de blanco.
    l->ClearRenderTargetView(rtv[1], nada_rgba, 0, nullptr);
    l->ClearDepthStencilView(dsv[1], D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0xFD, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[1], FALSE, &dsv[1]);
    l->SetGraphicsRoot32BitConstants(0, 4, blanco, 0);
    dibujar(b_suma, todo, 0, true);
    dibujar(b_suma, todo, 0, true);
    dibujar(b_suma, izquierda, 0, true);
    dibujar(b_cero, derecha, 0x00, true);
    dibujar(sr, todo, 0xFF, true);
    dibujar(sg, todo, 0xF0, true);
    dibujar(sb_lee, todo, 0x30, true);

    // C: la cara de delante y la de detras.
    l->ClearRenderTargetView(rtv[2], nada_rgba, 0, nullptr);
    l->ClearDepthStencilView(dsv[2], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0x10, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[2], FALSE, &dsv[2]);
    dibujar(c_caras, izquierda, 0, true);
    dibujar(c_caras, derecha, 0, false);
    dibujar(sr32, todo, 0x11, true);
    dibujar(sg32, todo, 0x0F, true);

    // D: de solo profundidad, y las tres operaciones.
    l->ClearRenderTargetView(rtv[3], nada_rgba, 0, nullptr);
    l->ClearDepthStencilView(dsv[3], D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 1.0f, 0, 0, nullptr);
    l->OMSetRenderTargets(0, nullptr, FALSE, &dsv[3]);
    dibujar(d_marca, izquierda, 3, true);
    dibujar(d_z, arriba, 0, true);
    dibujar(d_tres, todo, 3, true);
    l->OMSetRenderTargets(1, &rtv[3], FALSE, &dsv[3]);
    dibujar(sr, todo, 4, true);
    dibujar(sg, todo, 0, true);
    dibujar(sb, todo, 0xFF, true);

    // Leer los cuatro.
    for (int k = 0; k < 4; k++) {
        transicion(l, rt[k], D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
        copiar(l, rt[k], leido[k]);
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

    // A: el stencil 1 solo a la izquierda; EQUAL 1 pinta solo alli.
    juzgar(leido[0], [](UINT x, UINT) { return x < LADO / 2 ? VERDE : NEGRO; }, "A, D24S8: REPLACE 1 marca la izquierda y EQUAL 1 pinta de verde SOLO alli (la derecha, negra)");
    // B: izquierda 0xFF (0xFD + 3 con saturacion): solo la sonda R (EQUAL
    // 0xFF). Derecha 0xFF con REPLACE 0 bajo la mascara 0x0F = 0xF0: la
    // sonda G (EQUAL 0xF0) y la B (0x30 con lectura 0x0F: nibble bajo 0).
    juzgar(leido[1], [](UINT x, UINT) { return x < LADO / 2 ? R : G | B; }, "B, INCR_SAT satura en 0xFF, la mascara de escritura deja 0xF0 y la de lectura compara solo el nibble bajo");
    // C: 0x10 + 1 = 0x11 a la izquierda (delante, INCR) y 0x10 - 1 = 0x0F a
    // la derecha (detras, DECR).
    juzgar(leido[2], [](UINT x, UINT) { return x < LADO / 2 ? R : G; }, "C, D32S8X24: la cara de delante suma (0x11) y la de detras resta (0x0F)");
    // D: arriba a la izquierda falla la Z (3 -> INCR_SAT 4: R), abajo a la
    // izquierda pasan las dos (ZERO: G), a la derecha falla el stencil
    // (0 -> INVERT 0xFF: B).
    juzgar(leido[3], [](UINT x, UINT y) { return x >= LADO / 2 ? B : y < LADO / 2 ? R : G; }, "D, de solo profundidad: falla la Z (INCR_SAT, 4), pasan las dos (ZERO, 0), falla el stencil (INVERT, 0xFF)");
    printf("stencil.exe: el stencil de D3D12 es el de Windows\n");
    return fallos;
}
