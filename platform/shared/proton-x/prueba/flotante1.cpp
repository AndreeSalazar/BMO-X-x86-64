// flotante1.cpp -- el juez de lo que quedaba de los floats (N5.16b de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10), con los sombreadores de
// flotante1.hlsl (aqui al lado). Destinos de 16 x 16:
//
//   A1 R32_FLOAT: ClearRenderTargetView a -0.5 y, con la tijera en la mitad
//      derecha, DOS dibujos SUMANDO (ONE, ONE) 1.25 + 2^-20: sale 2 + 2^-19
//      (en half el 2^-20 se perderia: es de verdad de 32 bits).
//   A2 R16_FLOAT: limpio a 0.75 y dos sumas de -2.5: sale -4.25.
//   A3 A1 LEIDO como textura (Load) por -4, en otro R32_FLOAT: 2 a la
//      izquierda y -8 - 2^-17 a la derecha.
//   B  un RWTexture2D de R16G16B16A16_FLOAT: ClearUnorderedAccessViewFloat
//      a (-2.5, 3, 0.125, 1), un CS que lee y escribe el cuarto de arriba a
//      la izquierda (4 x 4) y otro que lo lee todo a un bufer de float4. El
//      2^-16 que suma el primero no cabe en el half de 0.125: se pierde al
//      escribir, y el segundo lee 0.125 justo. Leido half a half y float a
//      float.
//   C  un cuadro cuya z va de -0.5 (izquierda) a 1.5 (derecha): cruza el
//      plano cercano y el lejano. Viewport con la Z en [0.25, 0.75], la Z
//      limpia a 0.875 y la prueba LESS. Con DepthClipEnable = FALSE no se
//      recorta: se pinta todo y la Z se SUJETA al viewport (0.25 y 0.75)
//      ANTES de la prueba (sin sujetar, las columnas 14 y 15 no pasarian);
//      con TRUE, solo las columnas 4 a 11.
//
// Cada valor cabe EXACTO en su formato (sin redondeo que discutir), y lo
// que tiene que salir esta escrito abajo con sus bits, sacado de las reglas
// de D3D. La unica Z que no es exacta en una GPU (la de dentro de la rampa,
// interpolada) se mira con un margen de 2^-12. Sale con el numero de
// fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cmath>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl vs_cuadro\n vs_cuadro:\n .incbin \"flotante1_vs.dxil\"\n .globl vs_cuadro_fin\n vs_cuadro_fin:\n"
        ".p2align 4\n"
        ".globl vs_rampa\n vs_rampa:\n .incbin \"flotante1_rampa.dxil\"\n .globl vs_rampa_fin\n vs_rampa_fin:\n"
        ".p2align 4\n"
        ".globl ps_color\n ps_color:\n .incbin \"flotante1_color.dxil\"\n .globl ps_color_fin\n ps_color_fin:\n"
        ".p2align 4\n"
        ".globl ps_lee\n ps_lee:\n .incbin \"flotante1_lee.dxil\"\n .globl ps_lee_fin\n ps_lee_fin:\n"
        ".p2align 4\n"
        ".globl cs_escribe\n cs_escribe:\n .incbin \"flotante1_escribe.dxil\"\n .globl cs_escribe_fin\n cs_escribe_fin:\n"
        ".p2align 4\n"
        ".globl cs_lee\n cs_lee:\n .incbin \"flotante1_uav.dxil\"\n .globl cs_lee_fin\n cs_lee_fin:\n"
        ".text\n");
extern "C" const unsigned char vs_cuadro[], vs_cuadro_fin[], vs_rampa[], vs_rampa_fin[], ps_color[], ps_color_fin[], ps_lee[], ps_lee_fin[], cs_escribe[], cs_escribe_fin[], cs_lee[],
    cs_lee_fin[];

static const UINT LADO = 16;
// Cada destino se lee con filas de 256 bytes (lo que pide D3D12), uno tras
// otro: 4096 bytes cada uno.
static const UINT FILA = 256, CADA = LADO * FILA;
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

static ID3D12Resource *recurso(ID3D12Device *d, D3D12_HEAP_TYPE monton, UINT64 ancho, UINT alto, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl, D3D12_RESOURCE_STATES e, const D3D12_CLEAR_VALUE *limpio) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = alto ? D3D12_RESOURCE_DIMENSION_TEXTURE2D : D3D12_RESOURCE_DIMENSION_BUFFER;
    r.Width = ancho;
    r.Height = alto ? alto : 1;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Layout = alto ? D3D12_TEXTURE_LAYOUT_UNKNOWN : D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    r.Flags = fl;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, limpio, IID_PPV_ARGS(&b)), "CreateCommittedResource");
    return b;
}

static ID3D12Resource *destino(ID3D12Device *d, DXGI_FORMAT f) {
    return recurso(d, D3D12_HEAP_TYPE_DEFAULT, LADO, LADO, f, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, nullptr);
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *vs, const unsigned char *vs_fin, const unsigned char *ps, const unsigned char *ps_fin, DXGI_FORMAT f, bool suma,
                                int recorte_z) {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC p = {};
    p.pRootSignature = rs;
    p.VS.pShaderBytecode = vs;
    p.VS.BytecodeLength = (SIZE_T)(vs_fin - vs);
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
    // -1: sin profundidad (y con recorte); 0 y 1: D32 con LESS y ese
    // DepthClipEnable.
    p.RasterizerState.DepthClipEnable = recorte_z != 0;
    if (recorte_z >= 0) {
        p.DepthStencilState.DepthEnable = TRUE;
        p.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ALL;
        p.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_LESS;
        p.DSVFormat = DXGI_FORMAT_D32_FLOAT;
    }
    p.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    p.NumRenderTargets = 1;
    p.RTVFormats[0] = f;
    p.SampleDesc.Count = 1;
    ID3D12PipelineState *x = nullptr;
    hecho(d->CreateGraphicsPipelineState(&p, IID_PPV_ARGS(&x)), "CreateGraphicsPipelineState");
    return x;
}

static ID3D12PipelineState *computo(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *cs, const unsigned char *fin) {
    D3D12_COMPUTE_PIPELINE_STATE_DESC c = {};
    c.pRootSignature = rs;
    c.CS.pShaderBytecode = cs;
    c.CS.BytecodeLength = (SIZE_T)(fin - cs);
    ID3D12PipelineState *p = nullptr;
    hecho(d->CreateComputePipelineState(&c, IID_PPV_ARGS(&p)), "CreateComputePipelineState");
    return p;
}

static ID3D12RootSignature *raiz(ID3D12Device *d, const D3D12_ROOT_SIGNATURE_DESC &rd) {
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    return rs;
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

static void barrera_uav(ID3D12GraphicsCommandList *l, ID3D12Resource *r) {
    D3D12_RESOURCE_BARRIER b = {};
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_UAV;
    b.UAV.pResource = r;
    l->ResourceBarrier(1, &b);
}

// La textura `t` (formato `f`) a `leida`, desde el byte `desde`.
static void copiar(ID3D12GraphicsCommandList *l, ID3D12Resource *t, ID3D12Resource *leida, DXGI_FORMAT f, UINT64 desde) {
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = leida;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Offset = desde;
    a.PlacedFootprint.Footprint = {f, LADO, LADO, 1, FILA};
    de.pResource = t;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
}

// Los LADO x LADO texeles de `v` (filas de `fila` bytes, `bytes` por texel):
// `mira(x, y, texel, quiero)` dice si esta bien y pone en `quiero` los bytes
// que tenia que tener. El mensaje dice la primera palabra (de 4 bytes) que
// no es.
template <typename M> static void juzgar(const unsigned char *v, UINT fila, UINT bytes, M mira, const char *que) {
    UINT malos = 0, px = 0, py = 0, k = 0, leido = 0, quiero = 0;
    for (UINT y = 0; y < LADO; y++)
        for (UINT x = 0; x < LADO; x++) {
            const unsigned char *t = v + y * fila + x * bytes;
            UINT q[4] = {};
            if (!mira(x, y, t, q) && malos++ == 0) {
                px = x;
                py = y;
                for (k = 0; k + 1 < (bytes + 3) / 4; k++)
                    if (memcmp(t + 4 * k, &q[k], 4) != 0)
                        break;
                memcpy(&leido, t + 4 * k, bytes < 4 ? bytes : 4);
                quiero = q[k];
            }
        }
    char m[300];
    if (malos == 0)
        snprintf(m, sizeof m, "%s", que);
    else
        snprintf(m, sizeof m, "%s: %u texeles distintos; en el (%u, %u), la palabra %u es %08x y tenia que ser %08x", que, malos, px, py, k, leido, quiero);
    decir(malos == 0, m);
}

// Las `n` palabras (de 16 o 32 bits) de `t`, contra las de `q`.
template <typename T> static bool igual(const unsigned char *t, const T *q, UINT n, UINT *quiero) {
    memcpy(quiero, q, n * sizeof(T));
    return memcmp(t, q, n * sizeof(T)) == 0;
}

// Lo que tiene que salir, en bits.
// A1: -0.5 = 0xBF000000. -0.5 + 2 (1.25 + 2^-20) = 2 + 2^-19: 2 es
// 0x40000000 y su paso es 2^-22, asi que 2^-19 son 8 pasos -> 0x40000008.
static const UINT A1_IZQ = 0xBF000000u, A1_DER = 0x40000008u;
// A2: 0.75 - 2.5 - 2.5 = -4.25 = -1.0625 * 2^2: exponente 2 + 15 = 17,
// mantisa 0.0625 * 1024 = 64 -> 0xC000 | 0x4400 | 0x40.
static const unsigned short A2 = 0xC440;
// A3: -0.5 * -4 = 2 -> 0x40000000; (2 + 2^-19) * -4 = -(8 + 2^-17) = -8 (1 +
// 2^-20): exponente 3 + 127 = 130 -> 0xC1000000, y el 2^-20, 8 pasos.
static const UINT A3_IZQ = 0x40000000u, A3_DER = 0xC1000008u;
// B, en half: lo limpio (-2.5, 3, 0.125, 1) = (0xC100, 0x4200, 0x3000, 0x3C00);
// en el cuarto escrito, r = -2.5 + x + 0.5 = x - 2 (-2, -1, 0, 1), g = 3 - y
// (3, 2, 1, 0), b = 0.125 + 2^-16 -> 0.125 (el half de 0.125 tiene pasos de
// 2^-13: 2^-16 es un octavo de paso, se va) y a = 1 * 60000 = 1.8310546875 *
// 2^15 -> exponente 30, mantisa 851 -> 0x7B53.
static const unsigned short B_LIMPIO[4] = {0xC100, 0x4200, 0x3000, 0x3C00};
static const unsigned short B_R[4] = {0xC000, 0xBC00, 0x0000, 0x3C00}, B_G[4] = {0x4200, 0x4000, 0x3C00, 0x0000}, B_B = 0x3000, B_A = 0x7B53;
// Lo mismo en float de 32, leido por el CS: -2.5 0xC0200000, 3 0x40400000,
// 0.125 0x3E000000 (NO 0x3E000400: el 2^-16 se perdio al guardar), 1
// 0x3F800000; 60000 = 0xEA60 -> exponente 142, 0x476A6000.
static const UINT F_LIMPIO[4] = {0xC0200000u, 0x40400000u, 0x3E000000u, 0x3F800000u};
static const UINT F_R[4] = {0xC0000000u, 0xBF800000u, 0x00000000u, 0x3F800000u}, F_G[4] = {0x40400000u, 0x40000000u, 0x3F800000u, 0x00000000u}, F_B = 0x3E000000u, F_A = 0x476A6000u;
// C: R8G8B8A8 azul (0, 0, 1, 1) = 0xFFFF0000 y amarillo (1, 1, 0, 1) =
// 0xFF00FFFF. La Z: 0.25 = 0x3E800000, 0.75 = 0x3F400000, 0.875 =
// 0x3F600000; dentro de la rampa, 0.25 + 0.5 z con z = (x + 0.5) / 8 - 0.5.
static const UINT AZUL = 0xFFFF0000u, AMARILLO = 0xFF00FFFFu, Z_CERCA = 0x3E800000u, Z_LEJOS = 0x3F400000u, Z_LIMPIA = 0x3F600000u;

static bool z_de_la_rampa(UINT x, const unsigned char *t, UINT *q) {
    float z, cuenta = 0.25f + 0.5f * ((x + 0.5f) / 8.0f - 0.5f);
    memcpy(&z, t, 4);
    memcpy(q, &cuenta, 4);
    return fabsf(z - cuenta) <= 1.0f / 4096.0f;
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

    // La raiz de dibujo: cuatro constantes (b0) y una tabla con el SRV (t0);
    // la de computo: una tabla con los dos UAV (u0, la textura; u1, el bufer).
    D3D12_DESCRIPTOR_RANGE srv_r = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 0, 0, 0}, uav_r = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 2, 0, 0, 0};
    D3D12_ROOT_PARAMETER ps[2] = {}, cs[1] = {};
    ps[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    ps[0].Constants.Num32BitValues = 4;
    ps[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    ps[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[1].DescriptorTable.NumDescriptorRanges = 1;
    ps[1].DescriptorTable.pDescriptorRanges = &srv_r;
    ps[1].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    cs[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    cs[0].DescriptorTable.NumDescriptorRanges = 1;
    cs[0].DescriptorTable.pDescriptorRanges = &uav_r;
    cs[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    ID3D12RootSignature *rs = raiz(d, {2, ps, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE});
    ID3D12RootSignature *rc = raiz(d, {1, cs, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE});
    if (fallos)
        return fallos;

    ID3D12PipelineState *pa1 = pso(d, rs, vs_cuadro, vs_cuadro_fin, ps_color, ps_color_fin, DXGI_FORMAT_R32_FLOAT, true, -1);
    ID3D12PipelineState *pa2 = pso(d, rs, vs_cuadro, vs_cuadro_fin, ps_color, ps_color_fin, DXGI_FORMAT_R16_FLOAT, true, -1);
    ID3D12PipelineState *pa3 = pso(d, rs, vs_cuadro, vs_cuadro_fin, ps_lee, ps_lee_fin, DXGI_FORMAT_R32_FLOAT, false, -1);
    ID3D12PipelineState *pc[2] = {pso(d, rs, vs_rampa, vs_rampa_fin, ps_color, ps_color_fin, DXGI_FORMAT_R8G8B8A8_UNORM, false, 0),
                                  pso(d, rs, vs_rampa, vs_rampa_fin, ps_color, ps_color_fin, DXGI_FORMAT_R8G8B8A8_UNORM, false, 1)};
    ID3D12PipelineState *pe = computo(d, rc, cs_escribe, cs_escribe_fin), *pl = computo(d, rc, cs_lee, cs_lee_fin);
    ID3D12Resource *ta1 = destino(d, DXGI_FORMAT_R32_FLOAT), *ta2 = destino(d, DXGI_FORMAT_R16_FLOAT), *ta3 = destino(d, DXGI_FORMAT_R32_FLOAT);
    ID3D12Resource *tc[2] = {destino(d, DXGI_FORMAT_R8G8B8A8_UNORM), destino(d, DXGI_FORMAT_R8G8B8A8_UNORM)};
    D3D12_CLEAR_VALUE zl = {};
    zl.Format = DXGI_FORMAT_D32_FLOAT;
    zl.DepthStencil.Depth = 0.875f;
    ID3D12Resource *zc[2];
    for (int k = 0; k < 2; k++)
        zc[k] = recurso(d, D3D12_HEAP_TYPE_DEFAULT, LADO, LADO, DXGI_FORMAT_D32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, D3D12_RESOURCE_STATE_DEPTH_WRITE, &zl);
    const D3D12_RESOURCE_FLAGS UAV = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    ID3D12Resource *tb = recurso(d, D3D12_HEAP_TYPE_DEFAULT, LADO, LADO, DXGI_FORMAT_R16G16B16A16_FLOAT, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, nullptr);
    ID3D12Resource *salida = recurso(d, D3D12_HEAP_TYPE_DEFAULT, LADO * LADO * 16, 0, DXGI_FORMAT_UNKNOWN, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, nullptr);
    // Lo leido: A1, A2, A3, B (la textura), C TRUE/FALSE (color y Z), y el
    // bufer de B detras.
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, 9 * CADA, 0, DXGI_FORMAT_UNKNOWN, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST, nullptr);
    if (fallos)
        return fallos;
    decir(true, "los destinos (R32F, R16F, RGBA16F de UAV, RGBA8 y D32), sus PSO y los dos CS");

    // Los descriptores: cinco RTV, dos DSV, el monton visible ([0] SRV de
    // A1, [1] UAV de B, [2] UAV del bufer) y el de la CPU (el UAV de B, para
    // limpiarlo).
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 5, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *rtvs = nullptr, *dsvs = nullptr, *visible = nullptr, *de_cpu = nullptr;
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap de los RTV");
    hd = {D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&dsvs)), "CreateDescriptorHeap de los DSV");
    hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 3, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&visible)), "CreateDescriptorHeap visible");
    hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&de_cpu)), "CreateDescriptorHeap de la CPU");
    if (fallos)
        return fallos;
    UINT paso_rtv = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV), paso_dsv = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV),
         paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    D3D12_CPU_DESCRIPTOR_HANDLE r0, z0, cv, cc;
    D3D12_GPU_DESCRIPTOR_HANDLE gv;
    rtvs->GetCPUDescriptorHandleForHeapStart(&r0);
    dsvs->GetCPUDescriptorHandleForHeapStart(&z0);
    visible->GetCPUDescriptorHandleForHeapStart(&cv);
    visible->GetGPUDescriptorHandleForHeapStart(&gv);
    de_cpu->GetCPUDescriptorHandleForHeapStart(&cc);
    auto en = [](D3D12_CPU_DESCRIPTOR_HANDLE h, UINT p, UINT i) { return D3D12_CPU_DESCRIPTOR_HANDLE{h.ptr + (SIZE_T)i * p}; };
    auto eng = [&](UINT i) { return D3D12_GPU_DESCRIPTOR_HANDLE{gv.ptr + (UINT64)i * paso}; };
    D3D12_CPU_DESCRIPTOR_HANDLE rtv[5] = {en(r0, paso_rtv, 0), en(r0, paso_rtv, 1), en(r0, paso_rtv, 2), en(r0, paso_rtv, 3), en(r0, paso_rtv, 4)};
    D3D12_CPU_DESCRIPTOR_HANDLE dsv[2] = {en(z0, paso_dsv, 0), en(z0, paso_dsv, 1)};
    ID3D12Resource *con_rtv[5] = {ta1, ta2, ta3, tc[0], tc[1]};
    for (int k = 0; k < 5; k++)
        d->CreateRenderTargetView(con_rtv[k], nullptr, rtv[k]);
    for (int k = 0; k < 2; k++)
        d->CreateDepthStencilView(zc[k], nullptr, dsv[k]);
    d->CreateShaderResourceView(ta1, nullptr, en(cv, paso, 0));
    D3D12_UNORDERED_ACCESS_VIEW_DESC tex = {};
    tex.Format = DXGI_FORMAT_R16G16B16A16_FLOAT;
    tex.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
    d->CreateUnorderedAccessView(tb, nullptr, &tex, en(cv, paso, 1));
    d->CreateUnorderedAccessView(tb, nullptr, &tex, cc);
    D3D12_UNORDERED_ACCESS_VIEW_DESC est = {};
    est.Format = DXGI_FORMAT_UNKNOWN;
    est.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    est.Buffer.NumElements = LADO * LADO;
    est.Buffer.StructureByteStride = 16;
    d->CreateUnorderedAccessView(salida, nullptr, &est, en(cv, paso, 2));

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pa1, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    D3D12_VIEWPORT vp = {0, 0, (float)LADO, (float)LADO, 0, 1};
    D3D12_RECT todo = {0, 0, (LONG)LADO, (LONG)LADO}, derecha = {(LONG)LADO / 2, 0, (LONG)LADO, (LONG)LADO};
    l->SetDescriptorHeaps(1, &visible);
    l->RSSetViewports(1, &vp);
    l->SetGraphicsRootSignature(rs);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // A1: R32F, limpio a -0.5 y la mitad derecha sumada dos veces.
    const float limpio_a1[4] = {-0.5f, 0, 0, 0}, suma_a1[4] = {1.25f + 0x1p-20f, 7, 7, 7};
    l->ClearRenderTargetView(rtv[0], limpio_a1, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv[0], FALSE, nullptr);
    l->RSSetScissorRects(1, &derecha);
    l->SetGraphicsRoot32BitConstants(0, 4, suma_a1, 0);
    l->DrawInstanced(6, 1, 0, 0);
    l->DrawInstanced(6, 1, 0, 0);
    // A2: R16F, limpio a 0.75 y dos sumas de -2.5.
    const float limpio_a2[4] = {0.75f, 0, 0, 0}, suma_a2[4] = {-2.5f, 7, 7, 7};
    l->ClearRenderTargetView(rtv[1], limpio_a2, 0, nullptr);
    l->SetPipelineState(pa2);
    l->OMSetRenderTargets(1, &rtv[1], FALSE, nullptr);
    l->RSSetScissorRects(1, &todo);
    l->SetGraphicsRoot32BitConstants(0, 4, suma_a2, 0);
    l->DrawInstanced(6, 1, 0, 0);
    l->DrawInstanced(6, 1, 0, 0);
    // A3: A1 leido como textura, por -4.
    const float por_a3[4] = {-4.0f, 0, 0, 0};
    transicion(l, ta1, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    l->SetPipelineState(pa3);
    l->OMSetRenderTargets(1, &rtv[2], FALSE, nullptr);
    l->SetGraphicsRoot32BitConstants(0, 4, por_a3, 0);
    l->SetGraphicsRootDescriptorTable(1, eng(0));
    l->DrawInstanced(6, 1, 0, 0);
    // B: limpiar el UAV en float, escribir el cuarto y leerlo todo.
    const float limpio_b[4] = {-2.5f, 3.0f, 0.125f, 1.0f};
    l->ClearUnorderedAccessViewFloat(eng(1), cc, tb, limpio_b, 0, nullptr);
    barrera_uav(l, tb);
    l->SetComputeRootSignature(rc);
    l->SetPipelineState(pe);
    l->SetComputeRootDescriptorTable(0, eng(1));
    l->Dispatch(1, 1, 1);
    barrera_uav(l, tb);
    l->SetPipelineState(pl);
    l->Dispatch(LADO / 8, LADO / 8, 1);
    // C: la rampa, sin recorte en z (0) y con el (1).
    D3D12_VIEWPORT vz = {0, 0, (float)LADO, (float)LADO, 0.25f, 0.75f};
    const float azul[4] = {0, 0, 1, 1}, amarillo[4] = {1, 1, 0, 1};
    l->RSSetViewports(1, &vz);
    l->SetGraphicsRoot32BitConstants(0, 4, amarillo, 0);
    for (int k = 0; k < 2; k++) {
        l->ClearRenderTargetView(rtv[3 + k], azul, 0, nullptr);
        l->ClearDepthStencilView(dsv[k], D3D12_CLEAR_FLAG_DEPTH, 0.875f, 0, 0, nullptr);
        l->SetPipelineState(pc[k]);
        l->OMSetRenderTargets(1, &rtv[3 + k], FALSE, &dsv[k]);
        l->DrawInstanced(6, 1, 0, 0);
    }
    // Leerlo todo.
    transicion(l, ta1, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_SOURCE);
    ID3D12Resource *de_rt[4] = {ta2, ta3, tc[0], tc[1]};
    for (int k = 0; k < 4; k++)
        transicion(l, de_rt[k], D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE);
    transicion(l, tb, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    transicion(l, salida, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    for (int k = 0; k < 2; k++)
        transicion(l, zc[k], D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_COPY_SOURCE);
    copiar(l, ta1, leida, DXGI_FORMAT_R32_FLOAT, 0);
    copiar(l, ta2, leida, DXGI_FORMAT_R16_FLOAT, CADA);
    copiar(l, ta3, leida, DXGI_FORMAT_R32_FLOAT, 2 * CADA);
    copiar(l, tb, leida, DXGI_FORMAT_R16G16B16A16_FLOAT, 3 * CADA);
    for (int k = 0; k < 2; k++) {
        copiar(l, tc[k], leida, DXGI_FORMAT_R8G8B8A8_UNORM, (4 + k) * CADA);
        copiar(l, zc[k], leida, DXGI_FORMAT_D32_FLOAT, (6 + k) * CADA);
    }
    l->CopyBufferRegion(leida, 8 * CADA, salida, 0, LADO * LADO * 16);
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

    juzgar(b, FILA, 4, [](UINT x, UINT, const unsigned char *t, UINT *q) { return igual(t, x < LADO / 2 ? &A1_IZQ : &A1_DER, 1, q); },
           "A1, R32F: -0.5 a la izquierda y -0.5 + 2 (1.25 + 2^-20) = 2 + 2^-19 a la derecha, en float de 32");
    juzgar(b + CADA, FILA, 2, [](UINT, UINT, const unsigned char *t, UINT *q) { return igual(t, &A2, 1, q); }, "A2, R16F: 0.75 - 2.5 - 2.5 = -4.25 en cada texel, half a half");
    juzgar(b + 2 * CADA, FILA, 4, [](UINT x, UINT, const unsigned char *t, UINT *q) { return igual(t, x < LADO / 2 ? &A3_IZQ : &A3_DER, 1, q); },
           "A3, A1 leido como textura por -4 en otro R32F: 2 y -8 - 2^-17");
    juzgar(b + 3 * CADA, FILA, 8,
           [](UINT x, UINT y, const unsigned char *t, UINT *q) {
               unsigned short e[4] = {B_R[x & 3], B_G[y & 3], B_B, B_A};
               return igual(t, x < 4 && y < 4 ? e : B_LIMPIO, 4, q);
           },
           "B, el UAV de RGBA16F leido half a half: limpio en float y el cuarto escrito por el CS");
    juzgar(b + 8 * CADA, LADO * 16, 16,
           [](UINT x, UINT y, const unsigned char *t, UINT *q) {
               UINT e[4] = {F_R[x & 3], F_G[y & 3], F_B, F_A};
               return igual(t, x < 4 && y < 4 ? e : F_LIMPIO, 4, q);
           },
           "B, el mismo UAV leido por un CS en float de 32: el 2^-16 se perdio al guardar en half");
    juzgar(b + 4 * CADA, FILA, 4, [](UINT, UINT, const unsigned char *t, UINT *q) { return igual(t, &AMARILLO, 1, q); },
           "C, DepthClipEnable = FALSE: la rampa entera, sin recorte en z");
    juzgar(b + 6 * CADA, FILA, 4,
           [](UINT x, UINT, const unsigned char *t, UINT *q) {
               if (x < 4)
                   return igual(t, &Z_CERCA, 1, q);
               if (x >= 12)
                   return igual(t, &Z_LEJOS, 1, q);
               return z_de_la_rampa(x, t, q);
           },
           "C, DepthClipEnable = FALSE: la Z sujeta al viewport (0.25 delante, 0.75 detras) antes de la prueba");
    juzgar(b + 5 * CADA, FILA, 4, [](UINT x, UINT, const unsigned char *t, UINT *q) { return igual(t, x >= 4 && x < 12 ? &AMARILLO : &AZUL, 1, q); },
           "C, DepthClipEnable = TRUE: recortada en z = 0 y z = 1, solo las columnas 4 a 11");
    juzgar(b + 7 * CADA, FILA, 4, [](UINT x, UINT, const unsigned char *t, UINT *q) { return x >= 4 && x < 12 ? z_de_la_rampa(x, t, q) : igual(t, &Z_LIMPIA, 1, q); },
           "C, DepthClipEnable = TRUE: la Z de la rampa dentro, 0.875 (la limpia) fuera");
    leida->Unmap(0, nullptr);
    printf("flotante1.exe: los floats de un canal, sus UAV y DepthClipEnable son los de Windows\n");
    return fallos;
}
