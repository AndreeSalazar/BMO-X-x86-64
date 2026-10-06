// olas.cpp -- el juez de las OLAS de D3D12 (E2.5 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md, D4.3 de PLAN_LAS_TRES_GRANDES,
// 05-10): las operaciones Wave* y Quad* del modelo 6.0, con los sombreadores
// de olas_juez.hlsl (aqui al lado):
//
//   0  CheckFeatureSupport(OPTIONS1): WaveOps, y 32 carriles (min y max).
//   A  CSOlas: 2 grupos de 64 hilos; cada hilo escribe 26 palabras con lo
//      que le dio cada operacion de ola. En la 3060 una ola (un warp) son
//      32 hilos SEGUIDOS en el orden de SV_GroupIndex: lo que tiene que
//      salir se cuenta aqui, a mano, sobre los 32 de la ola de cada hilo
//      (sumas de enteros y de medios, exactas en cualquier orden; papeletas
//      con su mascara; dentro de un si y en dos bucles).
//   B  PSOlas en un destino de 64 x 64 (dos triangulos): cada pixel lee el
//      x y el y de sus vecinos de CUADRO (R = x ^ 1, G = y ^ 1) y ocho
//      pruebas de ola que no dependen de como junte la GPU los cuadros
//      (B = 255); A, los activos de su ola: de 1 a 32, y alguna ola con
//      mas de un cuadro.
//   C  Un triangulo que cubre SOLO el pixel (2, 2): su cuadro corre con
//      tres AYUDANTES, que el pixel lee (R = G = 3) y que las olas no
//      cuentan (A = 1). Con DrawIndexedInstanced y los indices 6, 7, 8: en
//      un dibujo con indices, SV_VertexID es el indice.
//   D  06-10, lo que dijo Windows: DrawInstanced(3, 1, 6, 0). SV_VertexID NO
//      cuenta el StartVertexLocation: va de 0 a 2, y se pinta la mitad de
//      arriba a la izquierda (los 2016 pixeles con x + y < 63). Hasta hoy
//      C se dibujaba asi, y en la 3060 pinto esa mitad.
//   E  DrawIndexedInstanced con los indices 0, 1, 2 y BaseVertexLocation 6:
//      SV_VertexID es el indice SIN el vertice base (lo que hacen DXVK y
//      vkd3d-proton): la misma mitad. Esto lo pregunta: si Windows pinta
//      solo el (2, 2), el base SI cuenta.
//
// Todo bit a bit. Sale con el numero de fallos. En Windows dice lo mismo (es
// de consola: lo que falle, se lee).
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl cs_olas\n cs_olas:\n .incbin \"olas_cs.dxil\"\n .globl cs_olas_fin\n cs_olas_fin:\n"
        ".p2align 4\n"
        ".globl vs_olas\n vs_olas:\n .incbin \"olas_vs.dxil\"\n .globl vs_olas_fin\n vs_olas_fin:\n"
        ".p2align 4\n"
        ".globl ps_olas\n ps_olas:\n .incbin \"olas_ps.dxil\"\n .globl ps_olas_fin\n ps_olas_fin:\n"
        ".text\n");
extern "C" const unsigned char cs_olas[], cs_olas_fin[], vs_olas[], vs_olas_fin[], ps_olas[], ps_olas_fin[];

static const UINT N = 26, HILOS = 128, LADO = 64;
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

static ID3D12Resource *bufer(ID3D12Device *d, D3D12_HEAP_TYPE monton, UINT64 bytes, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e) {
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
    r.Flags = fl;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource");
    return b;
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

static ID3D12RootSignature *firma(ID3D12Device *d, const D3D12_ROOT_SIGNATURE_DESC &rd) {
    ID3DBlob *f = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &f, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, f->GetBufferPointer(), f->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    return rs;
}

static UINT bits(float f) {
    UINT u;
    memcpy(&u, &f, 4);
    return u;
}

// **Lo que tiene que dar** el hilo i del grupo g de CSOlas: cada operacion
// sobre los 32 hilos de su ola, w0..w0+31 (la misma cuenta que
// `esperado` de src/pruebas_olas.rs).
static void esperado(UINT g, UINT i, UINT *o) {
    UINT w0 = i & ~31u;
    auto x = [&](UINT j) { return j * 2654435761u + g; };
    auto f = [](UINT j) { return (float)j * 0.5f; };
    memset(o, 0, N * 4);
    o[0] = 32;
    o[1] = i % 32;
    o[2] = i % 32 == 0;
    UINT suma = 0, producto = 1, y = 0xFFFFFFFFu, oo = 0, ox = 0, tres = 0, papeleta = 0, xmax = 0;
    int imin = 0x7FFFFFFF, imax = -0x7FFFFFFF - 1;
    float fs = 0.0f, fmin = 1e30f;
    for (UINT j = w0; j < w0 + 32; j++) {
        suma += j;
        producto *= j % 3 == 0 ? 3u : 1u;
        imin = (int)j - 40 < imin ? (int)j - 40 : imin;
        imax = (int)j - 40 > imax ? (int)j - 40 : imax;
        xmax = x(j) > xmax ? x(j) : xmax;
        y &= x(j) | 0x0F0F0F0Fu;
        oo |= x(j) & 0x00FF00FFu;
        ox ^= x(j);
        tres += j % 3 == 0;
        if (j % 5 == 0 || j == 33)
            papeleta |= 1u << (j - w0);
        fs = fs + f(j);
        fmin = 1.0f - f(j) < fmin ? 1.0f - f(j) : fmin;
    }
    o[3] = suma;
    o[4] = producto;
    o[5] = (UINT)imin;
    o[6] = xmax;
    o[7] = y;
    o[8] = oo;
    o[9] = ox;
    o[10] = tres;
    o[11] = papeleta;
    o[13] = x(w0 + 5);
    o[14] = x(w0);
    UINT ps = 0, pp = 1, pc = 0;
    float pf = 0.0f;
    for (UINT j = w0; j < i; j++) {
        ps += j;
        pp *= j % 4 == 1 ? 3u : 1u;
        pc += j % 2 == 1;
        pf = pf + f(j);
    }
    o[15] = ps;
    o[16] = pp;
    o[17] = pc;
    // AllEqual(i / 8) nunca (4 octavos por ola), AllEqual(g) siempre,
    // AnyTrue(i == 37) en la ola 32..63, AllTrue(i < 60) en la 0..31.
    o[18] = 2 | (w0 == 32 ? 4 : 8);
    o[19] = bits(fs);
    o[20] = bits(pf);
    o[21] = bits(fmin);
    o[22] = (UINT)imax;
    // En el si: los de i % 3 == 0 (su suma por 1000 mas el primero), o los
    // demas (cuantos por 1000 mas cuantos de ellos van antes).
    UINT s3 = 0, primero = 99, otros = 0, antes = 0;
    for (UINT j = w0; j < w0 + 32; j++) {
        if (j % 3 == 0) {
            s3 += j;
            primero = primero == 99 ? j : primero;
        } else {
            otros++;
            antes += j < i;
        }
    }
    o[23] = i % 3 == 0 ? s3 * 1000 + primero : otros * 1000 + antes;
    // Escalarizar: cada vuelta el primero que queda dice su v, y salen los
    // que lo tienen.
    bool queda[32];
    for (UINT k = 0; k < 32; k++)
        queda[k] = true;
    for (UINT vuelta = 1, n = 32; n > 0; vuelta++) {
        UINT k0 = 0;
        while (!queda[k0])
            k0++;
        UINT v = ((w0 + k0) * 7) % 5;
        if ((i * 7) % 5 == v)
            o[24] = vuelta;
        for (UINT k = 0; k < 32; k++)
            if (queda[k] && ((w0 + k) * 7) % 5 == v) {
                queda[k] = false;
                n--;
            }
    }
    // El bucle: en la vuelta k siguen los de j % 4 > k.
    for (UINT k = 0; k < i % 4; k++)
        for (UINT j = w0; j < w0 + 32; j++)
            o[25] += j % 4 > k;
}

int main() {
    ID3D12Device *d = nullptr;
    if (!hecho(D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d)), "D3D12CreateDevice"))
        return 1;
    D3D12_FEATURE_DATA_D3D12_OPTIONS1 o1 = {};
    if (hecho(d->CheckFeatureSupport(D3D12_FEATURE_D3D12_OPTIONS1, &o1, sizeof o1), "CheckFeatureSupport(OPTIONS1)")) {
        char m[160];
        snprintf(m, sizeof m, "OPTIONS1: WaveOps %d, de %u a %u carriles", (int)o1.WaveOps, o1.WaveLaneCountMin, o1.WaveLaneCountMax);
        decir(o1.WaveOps && o1.WaveLaneCountMin == 32 && o1.WaveLaneCountMax == 32, m);
    }
    D3D12_COMMAND_QUEUE_DESC qd = {};
    qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    ID3D12CommandQueue *cola = nullptr;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&cola)), "CreateCommandQueue");
    ID3D12CommandAllocator *al = nullptr;
    hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&al)), "CreateCommandAllocator");
    ID3D12Fence *valla = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&valla)), "CreateFence");

    // El computo: un UAV en la raiz (u0). El dibujo: nada.
    D3D12_ROOT_PARAMETER rp = {};
    rp.ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    rp.Descriptor.ShaderRegister = 0;
    rp.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_SIGNATURE_DESC rd = {};
    rd.NumParameters = 1;
    rd.pParameters = &rp;
    ID3D12RootSignature *rs_cs = firma(d, rd);
    D3D12_ROOT_SIGNATURE_DESC vacia = {};
    ID3D12RootSignature *rs_ps = firma(d, vacia);
    if (fallos)
        return fallos;

    D3D12_COMPUTE_PIPELINE_STATE_DESC c = {};
    c.pRootSignature = rs_cs;
    c.CS.pShaderBytecode = cs_olas;
    c.CS.BytecodeLength = (SIZE_T)(cs_olas_fin - cs_olas);
    ID3D12PipelineState *pso_cs = nullptr;
    hecho(d->CreateComputePipelineState(&c, IID_PPV_ARGS(&pso_cs)), "CreateComputePipelineState");
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs_ps;
    p.VS.pShaderBytecode = vs_olas;
    p.VS.BytecodeLength = (SIZE_T)(vs_olas_fin - vs_olas);
    p.PS.pShaderBytecode = ps_olas;
    p.PS.BytecodeLength = (SIZE_T)(ps_olas_fin - ps_olas);
    p.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    p.SampleMask = UINT_MAX;
    p.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    p.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    p.RasterizerState.DepthClipEnable = TRUE;
    p.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    p.NumRenderTargets = 1;
    p.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
    p.SampleDesc.Count = 1;
    ID3D12PipelineState *pso_ps = nullptr;
    hecho(d->CreateGraphicsPipelineState(&p, IID_PPV_ARGS(&pso_ps)), "CreateGraphicsPipelineState");
    if (fallos)
        return fallos;

    // Los buferes: el UAV del computo, el destino de 64 x 64, y donde se
    // lee todo (el UAV, y el destino cuatro veces: B, C, D y E); y los
    // indices de C y E.
    const UINT64 uav_bytes = HILOS * N * 4, img_bytes = LADO * LADO * 4;
    ID3D12Resource *uav = bufer(d, D3D12_HEAP_TYPE_DEFAULT, uav_bytes, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, uav_bytes + 4 * img_bytes, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    const unsigned short indices[6] = {6, 7, 8, 0, 1, 2};
    ID3D12Resource *ib = bufer(d, D3D12_HEAP_TYPE_UPLOAD, sizeof indices, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    void *mi = nullptr;
    if (ib && hecho(ib->Map(0, nullptr, &mi), "Map de los indices"))
        memcpy(mi, indices, sizeof indices);
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
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pso_cs, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    // A: el computo, y su UAV a lo leido.
    l->SetComputeRootSignature(rs_cs);
    l->SetComputeRootUnorderedAccessView(0, uav->GetGPUVirtualAddress());
    l->Dispatch(2, 1, 1);
    barrera(l, uav, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    l->CopyBufferRegion(leida, 0, uav, 0, uav_bytes);
    // B y C: cada dibujo sobre el destino limpio, y a lo leido.
    l->SetPipelineState(pso_ps);
    l->SetGraphicsRootSignature(rs_ps);
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    D3D12_RECT tijera = {0, 0, (LONG)LADO, (LONG)LADO};
    l->RSSetViewports(1, &vp);
    l->RSSetScissorRects(1, &tijera);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    l->OMSetRenderTargets(1, &rtv, FALSE, nullptr);
    const float negro[4] = {0, 0, 0, 0};
    D3D12_INDEX_BUFFER_VIEW ibv = {ib->GetGPUVirtualAddress(), (UINT)sizeof indices, DXGI_FORMAT_R16_UINT};
    l->IASetIndexBuffer(&ibv);
    for (UINT k = 0; k < 4; k++) {
        l->ClearRenderTargetView(rtv, negro, 0, nullptr);
        if (k == 0)
            l->DrawInstanced(6, 1, 0, 0);
        else if (k == 1)
            l->DrawIndexedInstanced(3, 1, 0, 0, 0);
        else if (k == 2)
            l->DrawInstanced(3, 1, 6, 0);
        else
            l->DrawIndexedInstanced(3, 1, 3, 6, 0);
        barrera(l, rt, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint.Offset = uav_bytes + k * img_bytes;
        a.PlacedFootprint.Footprint = {DXGI_FORMAT_R8G8B8A8_UNORM, LADO, LADO, 1, LADO * 4};
        de.pResource = rt;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
        barrera(l, rt, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET);
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
    void *m = nullptr;
    if (!hecho(leida->Map(0, nullptr, &m), "Map de lo leido"))
        return fallos;
    const UINT *cs = (const UINT *)m;

    // A, por grupos de palabras: que operacion falla, se lee.
    struct Parte {
        UINT desde, hasta;
        const char *que;
    } partes[] = {
        {0, 3, "WaveGetLaneCount (32), WaveGetLaneIndex y WaveIsFirstLane"},
        {3, 10, "WaveActiveSum/Product/Min/Max/BitAnd/BitOr/BitXor de enteros"},
        {22, 23, "WaveActiveMax CON signo (-8..23: sin el, -1 seria el mayor)"},
        {10, 13, "WaveActiveCountBits y WaveActiveBallot"},
        {13, 15, "WaveReadLaneAt y WaveReadLaneFirst"},
        {15, 18, "WavePrefixSum/Product/CountBits"},
        {18, 19, "WaveActiveAllEqual, WaveActiveAnyTrue y WaveActiveAllTrue"},
        {19, 22, "WaveActiveSum, WavePrefixSum y WaveActiveMin de floats"},
        {23, 24, "dentro de un si: los activos son los de su rama"},
        {24, 25, "el bucle de escalarizar (WaveReadLaneFirst hasta que es el mio)"},
        {25, 26, "un bucle del que cada hilo sale en otra vuelta"},
    };
    static UINT quiero[HILOS][N];
    for (UINT t = 0; t < HILOS; t++)
        esperado(t / 64, t % 64, quiero[t]);
    for (const Parte &pa : partes) {
        UINT malos = 0, mt = 0, mk = 0;
        for (UINT t = 0; t < HILOS; t++)
            for (UINT k = pa.desde; k < pa.hasta; k++)
                if (cs[t * N + k] != quiero[t][k] && malos++ == 0) {
                    mt = t;
                    mk = k;
                }
        char msg[240];
        if (malos == 0)
            snprintf(msg, sizeof msg, "A, %s: los 128 hilos", pa.que);
        else
            snprintf(msg, sizeof msg, "A, %s: %u distintas; el hilo %u palabra %u es %08x y tenia que ser %08x", pa.que, malos, mt, mk, cs[mt * N + mk], quiero[mt][mk]);
        decir(malos == 0, msg);
    }

    // B: R = x ^ 1, G = y ^ 1, B = 255 en todos; A de 1 a 32.
    const UINT *b = (const UINT *)((const unsigned char *)m + uav_bytes);
    UINT malos = 0, primero = 0, fuera = 0, mayor = 0;
    for (UINT i = 0; i < LADO * LADO; i++) {
        UINT x = i % LADO, y = i / LADO, a = b[i] >> 24;
        if ((b[i] & 0xFFFFFF) != ((x ^ 1) | (y ^ 1) << 8 | 255u << 16) && malos++ == 0)
            primero = i;
        fuera += a < 1 || a > 32;
        mayor = a > mayor ? a : mayor;
    }
    char msg[240];
    if (malos == 0)
        snprintf(msg, sizeof msg, "B, QuadReadAcrossX/Y/Diagonal, QuadReadLaneAt y 8 pruebas de ola: los 4096 pixeles");
    else
        snprintf(msg, sizeof msg, "B: %u pixeles distintos; el (%u, %u) es %08x", malos, primero % LADO, primero / LADO, b[primero]);
    decir(malos == 0, msg);
    snprintf(msg, sizeof msg, "B, WaveActiveCountBits(true): de 1 a 32 en cada pixel (%u fuera), y olas de mas de un cuadro (hasta %u)", fuera, mayor);
    decir(fuera == 0 && mayor > 4, msg);

    // C: solo el (2, 2), con sus ayudantes leidos y un activo.
    const UINT *cc = b + LADO * LADO;
    UINT otros = 0;
    for (UINT i = 0; i < LADO * LADO; i++)
        otros += i != 2 * LADO + 2 && cc[i] != 0;
    snprintf(msg, sizeof msg, "C, un pixel solo: el (2, 2) es %08x (lee a sus 3 ayudantes, 1 activo); %u pixeles mas pintados", cc[2 * LADO + 2], otros);
    decir(cc[2 * LADO + 2] == (3u | 3u << 8 | 255u << 16 | 1u << 24) && otros == 0, msg);

    // D y E: la mitad de arriba a la izquierda (x + y < 63), y nada mas.
    const char *que[2] = {"D, DrawInstanced(3, 1, 6, 0): SV_VertexID sin StartVertexLocation", "E, DrawIndexedInstanced con BaseVertexLocation 6: SV_VertexID sin el vertice base"};
    for (UINT k = 0; k < 2; k++) {
        const UINT *de = cc + (k + 1) * LADO * LADO;
        UINT pintados = 0, mal_puestos = 0;
        for (UINT i = 0; i < LADO * LADO; i++) {
            bool dentro = i % LADO + i / LADO < 63;
            pintados += de[i] != 0;
            mal_puestos += (de[i] != 0) != dentro;
        }
        snprintf(msg, sizeof msg, "%s: %u pixeles pintados (2016 la mitad), %u donde no tocaba; el (2, 2) es %08x", que[k], pintados, mal_puestos, de[2 * LADO + 2]);
        decir(mal_puestos == 0, msg);
    }

    printf("olas.exe: las olas de D3D12 son las de Windows\n");
    return fallos;
}
