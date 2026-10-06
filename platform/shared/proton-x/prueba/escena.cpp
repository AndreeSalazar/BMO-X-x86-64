// escena.cpp -- el juez de una escena 3D DURA (A11 de PLAN_LAS_TRES_GRANDES
// 7.1, 06-10), con los sombreadores de escena.hlsl (aqui al lado). El
// propietario: "un test en 3D duro en Windows, y que se refleje en BMO-X".
//
// Un fotograma de 256 x 144: un terreno de 96 x 96 cuadros (18432
// triangulos) con relieve y una textura de 128 x 128 con sus 8 mips
// (muestreo trilineal), 64 cubos por INSTANCIAS sobre el, una luz con SOMBRA
// (un pase de solo profundidad a un mapa de 512 x 512 y SampleCmp con 4
// vecinos), un vidrio con MEZCLA, todo en HDR (RGBA16F) con su profundidad
// (D32), y un TONEMAP por computo a RGBA8.
//
// La GPU y la CPU no dan los mismos bits en los floats (la mip elegida, los
// pesos del filtro, los bordes de los triangulos): esta escena no se juzga
// bit a bit sino contra la imagen de WINDOWS, con un margen:
//
//   escena.exe guardar   (en Windows) deja escena.ref, la imagen de
//                        referencia, y escena.bmp para mirarla
//   escena.exe           la compara con escena.ref: cuantos pixeles y cuanto
//
//   A  el cielo: el pixel (0, 0) es el color de la limpieza con su tonemap
//   B  la sombra: el mapa de la luz tiene el terreno en el centro
//   C  la imagen, contra escena.ref: el 98 % de los pixeles a 8 o menos en
//      cada canal y la media a 2 o menos (el margen es una apuesta: la
//      primera corrida en Windows lo MIDE). Sin escena.ref, una `nota`.
//
// Sale con el numero de fallos.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cmath>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(vs_escena, "escena_vs.dxil") DXIL(vs_sombra, "escena_sombra.dxil") DXIL(ps_escena, "escena_ps.dxil") DXIL(ps_vidrio, "escena_vidrio.dxil") DXIL(cs_tono, "escena_tono.dxil") ".text\n");
extern "C" const unsigned char vs_escena[], vs_escena_fin[], vs_sombra[], vs_sombra_fin[], ps_escena[], ps_escena_fin[], ps_vidrio[], ps_vidrio_fin[], cs_tono[], cs_tono_fin[];

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

static const UINT W = 256, H = 144, LADO_SOMBRA = 512, LADO_TABLERO = 128, MIPS = 8, CUADROS = 96, CUBOS = 64;
static const float CIELO[4] = {0.3f, 0.5f, 0.9f, 1.0f};

// -- La matematica: matrices de 4 x 4 para `mul(M, v)` (columna a la derecha).
struct M4 {
    float m[4][4];
};

static M4 por(const M4 &a, const M4 &b) {
    M4 r = {};
    for (int i = 0; i < 4; i++)
        for (int j = 0; j < 4; j++)
            for (int k = 0; k < 4; k++)
                r.m[i][j] += a.m[i][k] * b.m[k][j];
    return r;
}

static void normalizar(float v[3]) {
    float l = sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    for (int i = 0; i < 3; i++)
        v[i] /= l;
}

static void cruz(const float a[3], const float b[3], float r[3]) {
    r[0] = a[1] * b[2] - a[2] * b[1];
    r[1] = a[2] * b[0] - a[0] * b[2];
    r[2] = a[0] * b[1] - a[1] * b[0];
}

// La vista: el ojo en `ojo` mirando a `a`, con la y arriba (mano izquierda, la de D3D).
static M4 mirar(const float ojo[3], const float a[3]) {
    float z[3] = {a[0] - ojo[0], a[1] - ojo[1], a[2] - ojo[2]}, arriba[3] = {0, 1, 0}, x[3], y[3];
    normalizar(z);
    cruz(arriba, z, x);
    normalizar(x);
    cruz(z, x, y);
    M4 r = {{{x[0], x[1], x[2], 0}, {y[0], y[1], y[2], 0}, {z[0], z[1], z[2], 0}, {0, 0, 0, 1}}};
    for (int i = 0; i < 3; i++)
        r.m[i][3] = -(r.m[i][0] * ojo[0] + r.m[i][1] * ojo[1] + r.m[i][2] * ojo[2]);
    return r;
}

static M4 perspectiva(float fov, float aspecto, float cerca, float lejos) {
    float f = 1.0f / tanf(fov / 2);
    M4 r = {};
    r.m[0][0] = f / aspecto;
    r.m[1][1] = f;
    r.m[2][2] = lejos / (lejos - cerca);
    r.m[2][3] = -cerca * lejos / (lejos - cerca);
    r.m[3][2] = 1;
    return r;
}

static M4 ortogonal(float ancho, float cerca, float lejos) {
    M4 r = {};
    r.m[0][0] = 2 / ancho;
    r.m[1][1] = 2 / ancho;
    r.m[2][2] = 1 / (lejos - cerca);
    r.m[2][3] = -cerca / (lejos - cerca);
    r.m[3][3] = 1;
    return r;
}

// El cbuffer guarda las matrices por columnas (lo de HLSL por defecto).
static void a_columnas(const M4 &a, float *d) {
    for (int i = 0; i < 4; i++)
        for (int j = 0; j < 4; j++)
            d[j * 4 + i] = a.m[i][j];
}

// -- La geometria.
struct Vert {
    float pos[3], nrm[3], uv[2];
};

struct Inst {
    float donde[4], color[4];
};

static float alto(float x, float z) {
    return 0.6f * sinf(x * 0.7f) * cosf(z * 0.5f);
}

// El terreno: de -8 a 8 en x y en z.
static void terreno(Vert *v, UINT *ind) {
    const UINT n = CUADROS + 1;
    for (UINT j = 0; j < n; j++)
        for (UINT i = 0; i < n; i++) {
            float x = -8 + 16.0f * i / CUADROS, z = -8 + 16.0f * j / CUADROS;
            // La normal, de las derivadas de la altura.
            float dx = 0.6f * 0.7f * cosf(x * 0.7f) * cosf(z * 0.5f), dz = -0.6f * 0.5f * sinf(x * 0.7f) * sinf(z * 0.5f);
            float nrm[3] = {-dx, 1, -dz};
            normalizar(nrm);
            Vert &w = v[j * n + i];
            w = {{x, alto(x, z), z}, {nrm[0], nrm[1], nrm[2]}, {(float)i / CUADROS, (float)j / CUADROS}};
        }
    UINT k = 0;
    for (UINT j = 0; j < CUADROS; j++)
        for (UINT i = 0; i < CUADROS; i++) {
            UINT a = j * n + i, b = a + 1, c = a + n, d = c + 1;
            UINT q[6] = {a, c, b, b, c, d};
            for (UINT t : q)
                ind[k++] = t;
        }
}

// El cubo de lado 1 centrado en 0: 24 vertices (cada cara su normal) y 36 indices.
static void cubo(Vert *v, UINT *ind) {
    const float caras[6][3] = {{1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1}};
    for (int c = 0; c < 6; c++) {
        float n[3] = {caras[c][0], caras[c][1], caras[c][2]}, u[3], w[3];
        float otro[3] = {fabsf(n[1]) > 0.5f ? 1.0f : 0.0f, fabsf(n[1]) > 0.5f ? 0.0f : 1.0f, 0};
        cruz(n, otro, u);
        cruz(n, u, w);
        for (int k = 0; k < 4; k++) {
            float su = (k & 1) ? 0.5f : -0.5f, sw = (k & 2) ? 0.5f : -0.5f;
            Vert &x = v[c * 4 + k];
            for (int e = 0; e < 3; e++) {
                x.pos[e] = n[e] * 0.5f + u[e] * su + w[e] * sw;
                x.nrm[e] = n[e];
            }
            x.uv[0] = su + 0.5f;
            x.uv[1] = sw + 0.5f;
        }
        UINT b = c * 4, q[6] = {b, b + 1, b + 2, b + 2, b + 1, b + 3};
        for (int k = 0; k < 6; k++)
            ind[c * 6 + k] = q[k];
    }
}

// -- D3D12, lo de siempre.
static ID3D12Resource *recurso(ID3D12Device *d, D3D12_HEAP_TYPE monton, const D3D12_RESOURCE_DESC &r, D3D12_RESOURCE_STATES e, const D3D12_CLEAR_VALUE *limpio = nullptr) {
    D3D12_HEAP_PROPERTIES p = {};
    p.Type = monton;
    ID3D12Resource *x = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, limpio, IID_PPV_ARGS(&x)), "CreateCommittedResource");
    return x;
}

static D3D12_RESOURCE_DESC bufer(UINT64 bytes) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    r.Width = bytes;
    r.Height = 1;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.SampleDesc.Count = 1;
    r.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    return r;
}

static D3D12_RESOURCE_DESC textura(UINT w, UINT h, UINT16 mips, DXGI_FORMAT f, D3D12_RESOURCE_FLAGS fl) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    r.Width = w;
    r.Height = h;
    r.DepthOrArraySize = 1;
    r.MipLevels = mips;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Flags = fl;
    return r;
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

static ID3D12RootSignature *firma(ID3D12Device *d, const D3D12_ROOT_SIGNATURE_DESC &rd) {
    ID3DBlob *b = nullptr, *e = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &b, &e), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, b->GetBufferPointer(), b->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    return rs;
}

// El tonemap de CSTono en la CPU: el color de la limpieza en 8 bits.
static UINT tono(float c) {
    float t = sqrtf(c / (1 + c));
    return (UINT)(t * 255 + 0.5f);
}

// Una imagen de W x H en RGBA8, como la deja la lectura (filas de 1024).
static bool guardar(const unsigned char *px) {
    FILE *f = fopen("escena.ref", "wb");
    if (!f)
        return false;
    fwrite("ESCENA1", 1, 8, f);
    UINT medidas[2] = {W, H};
    fwrite(medidas, 4, 2, f);
    for (UINT y = 0; y < H; y++)
        fwrite(px + y * 1024, 1, W * 4, f);
    fclose(f);
    // Y un BMP para mirarla (24 bits, de abajo arriba).
    f = fopen("escena.bmp", "wb");
    if (!f)
        return true;
    UINT fila = W * 3, datos = fila * H;
    unsigned char cab[54] = {'B', 'M'};
    auto pon = [&](int o, UINT v) { memcpy(cab + o, &v, 4); };
    pon(2, 54 + datos);
    pon(10, 54);
    pon(14, 40);
    pon(18, W);
    pon(22, H);
    cab[26] = 1;
    cab[28] = 24;
    pon(34, datos);
    fwrite(cab, 1, 54, f);
    for (int y = H - 1; y >= 0; y--)
        for (UINT x = 0; x < W; x++) {
            const unsigned char *p = px + y * 1024 + 4 * x;
            unsigned char bgr[3] = {p[2], p[1], p[0]};
            fwrite(bgr, 1, 3, f);
        }
    fclose(f);
    return true;
}

int main(int argc, char **argv) {
    bool guardar_ref = argc > 1 && strcmp(argv[1], "guardar") == 0;
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

    // -- Las firmas: la del dibujo (el cbuffer en la raiz, una tabla de dos
    // SRV y los dos muestreadores estaticos) y la del tonemap.
    D3D12_DESCRIPTOR_RANGE r_srv = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 2, 0, 0, 0};
    D3D12_ROOT_PARAMETER rp[2] = {};
    rp[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_CBV;
    rp[0].Descriptor.ShaderRegister = 0;
    rp[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    rp[1].DescriptorTable.NumDescriptorRanges = 1;
    rp[1].DescriptorTable.pDescriptorRanges = &r_srv;
    rp[1].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_STATIC_SAMPLER_DESC mu[2] = {};
    mu[0].Filter = D3D12_FILTER_MIN_MAG_MIP_LINEAR;
    mu[0].AddressU = mu[0].AddressV = mu[0].AddressW = D3D12_TEXTURE_ADDRESS_MODE_WRAP;
    mu[0].MaxLOD = D3D12_FLOAT32_MAX;
    mu[0].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    mu[1].Filter = D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT;
    mu[1].AddressU = mu[1].AddressV = mu[1].AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
    mu[1].ComparisonFunc = D3D12_COMPARISON_FUNC_LESS_EQUAL;
    mu[1].MaxLOD = D3D12_FLOAT32_MAX;
    mu[1].ShaderRegister = 1;
    mu[1].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    ID3D12RootSignature *rs = firma(d, {2, rp, 2, mu, D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT});
    D3D12_DESCRIPTOR_RANGE r_tono[2] = {{D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 2, 0, 0}, {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 0, 0, 1}};
    D3D12_ROOT_PARAMETER rt = {};
    rt.ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    rt.DescriptorTable.NumDescriptorRanges = 2;
    rt.DescriptorTable.pDescriptorRanges = r_tono;
    ID3D12RootSignature *rs_tono = firma(d, {1, &rt, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE});
    if (fallos)
        return fallos;

    // -- Los PSO.
    D3D12_INPUT_ELEMENT_DESC ia[5] = {
        {"POSITION", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
        {"NORMAL", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 12, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
        {"TEXCOORD", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 24, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0},
        {"INSTANCIA", 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 1, 0, D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA, 1},
        {"INSTANCIA", 1, DXGI_FORMAT_R32G32B32A32_FLOAT, 1, 16, D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA, 1},
    };
    D3D12_GRAPHICS_PIPELINE_STATE_DESC g = {};
    g.pRootSignature = rs;
    g.InputLayout = {ia, 5};
    g.VS = {vs_escena, (SIZE_T)(vs_escena_fin - vs_escena)};
    g.PS = {ps_escena, (SIZE_T)(ps_escena_fin - ps_escena)};
    g.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
    g.SampleMask = UINT_MAX;
    g.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
    g.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
    g.RasterizerState.DepthClipEnable = TRUE;
    g.DepthStencilState.DepthEnable = TRUE;
    g.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ALL;
    g.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_LESS;
    g.DSVFormat = DXGI_FORMAT_D32_FLOAT;
    g.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
    g.NumRenderTargets = 1;
    g.RTVFormats[0] = DXGI_FORMAT_R16G16B16A16_FLOAT;
    g.SampleDesc.Count = 1;
    ID3D12PipelineState *p_escena = nullptr, *p_sombra = nullptr, *p_vidrio = nullptr, *p_tono = nullptr;
    hecho(d->CreateGraphicsPipelineState(&g, IID_PPV_ARGS(&p_escena)), "CreateGraphicsPipelineState de la escena");
    D3D12_GRAPHICS_PIPELINE_STATE_DESC gs = g;
    gs.VS = {vs_sombra, (SIZE_T)(vs_sombra_fin - vs_sombra)};
    gs.PS = {};
    gs.NumRenderTargets = 0;
    gs.RTVFormats[0] = DXGI_FORMAT_UNKNOWN;
    hecho(d->CreateGraphicsPipelineState(&gs, IID_PPV_ARGS(&p_sombra)), "CreateGraphicsPipelineState de la sombra");
    D3D12_GRAPHICS_PIPELINE_STATE_DESC gv = g;
    gv.PS = {ps_vidrio, (SIZE_T)(ps_vidrio_fin - ps_vidrio)};
    gv.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ZERO;
    D3D12_RENDER_TARGET_BLEND_DESC &bl = gv.BlendState.RenderTarget[0];
    bl.BlendEnable = TRUE;
    bl.SrcBlend = D3D12_BLEND_SRC_ALPHA;
    bl.DestBlend = D3D12_BLEND_INV_SRC_ALPHA;
    bl.BlendOp = D3D12_BLEND_OP_ADD;
    bl.SrcBlendAlpha = D3D12_BLEND_ONE;
    bl.DestBlendAlpha = D3D12_BLEND_ZERO;
    bl.BlendOpAlpha = D3D12_BLEND_OP_ADD;
    hecho(d->CreateGraphicsPipelineState(&gv, IID_PPV_ARGS(&p_vidrio)), "CreateGraphicsPipelineState del vidrio");
    D3D12_COMPUTE_PIPELINE_STATE_DESC c = {};
    c.pRootSignature = rs_tono;
    c.CS = {cs_tono, (SIZE_T)(cs_tono_fin - cs_tono)};
    hecho(d->CreateComputePipelineState(&c, IID_PPV_ARGS(&p_tono)), "CreateComputePipelineState del tonemap");
    if (fallos)
        return fallos;

    // -- La geometria y las instancias, en un monton UPLOAD.
    const UINT NT = (CUADROS + 1) * (CUADROS + 1), IT = CUADROS * CUADROS * 6;
    static Vert verts[NT + 24];
    static UINT inds[IT + 36];
    terreno(verts, inds);
    cubo(verts + NT, inds + IT);
    static Inst inst[1 + CUBOS + 1];
    inst[0] = {{0, 0, 0, 1}, {1, 1, 1, 0}};
    for (UINT k = 0; k < CUBOS; k++) {
        float x = -7 + 14.0f * (k % 8) / 7, z = -7 + 14.0f * (k / 8) / 7, lado = 0.35f + 0.25f * ((k * 7) % 5) / 4;
        float col[3] = {0.4f + 0.6f * ((k * 3) % 5) / 4, 0.3f + 0.7f * ((k * 5) % 7) / 6, 0.2f + 0.8f * ((k * 11) % 3) / 2};
        inst[1 + k] = {{x, alto(x, z) + lado * 0.5f, z, lado}, {col[0], col[1], col[2], 1}};
    }
    inst[1 + CUBOS] = {{0.5f, 1.4f, -1.0f, 2.2f}, {1, 1, 1, 1}};
    ID3D12Resource *vb = recurso(d, D3D12_HEAP_TYPE_UPLOAD, bufer(sizeof verts), D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *ib = recurso(d, D3D12_HEAP_TYPE_UPLOAD, bufer(sizeof inds), D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *instb = recurso(d, D3D12_HEAP_TYPE_UPLOAD, bufer(sizeof inst), D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *cb = recurso(d, D3D12_HEAP_TYPE_UPLOAD, bufer(256), D3D12_RESOURCE_STATE_GENERIC_READ);
    if (fallos)
        return fallos;
    auto subir = [&](ID3D12Resource *r, const void *p, size_t n) {
        void *m = nullptr;
        D3D12_RANGE nada = {0, 0};
        if (hecho(r->Map(0, &nada, &m), "Map"))
            memcpy(m, p, n);
    };
    subir(vb, verts, sizeof verts);
    subir(ib, inds, sizeof inds);
    subir(instb, inst, sizeof inst);
    // La camara y la luz.
    float ojo[3] = {9, 6, -10}, a[3] = {0, 0, 0}, luz[3] = {-6, 10, -4}, cero[3] = {0, 0, 0};
    M4 vp = por(perspectiva(1.0f, (float)W / H, 0.5f, 60), mirar(ojo, a));
    M4 lp = por(ortogonal(24, 1, 30), mirar(luz, cero));
    float cbuf[40] = {};
    a_columnas(vp, cbuf);
    a_columnas(lp, cbuf + 16);
    float hacia[3] = {luz[0], luz[1], luz[2]};
    normalizar(hacia);
    cbuf[32] = hacia[0], cbuf[33] = hacia[1], cbuf[34] = hacia[2], cbuf[35] = 2.5f;
    subir(cb, cbuf, sizeof cbuf);

    // -- La textura del tablero, con sus mips hechos aqui (cada uno, la
    // media de 2 x 2 del anterior), por un bufer UPLOAD.
    D3D12_RESOURCE_DESC dtab = textura(LADO_TABLERO, LADO_TABLERO, MIPS, DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_NONE);
    ID3D12Resource *tab = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dtab, D3D12_RESOURCE_STATE_COPY_DEST);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT ht[MIPS];
    UINT64 total = 0;
    d->GetCopyableFootprints(&dtab, 0, MIPS, 0, ht, nullptr, nullptr, &total);
    ID3D12Resource *subida = recurso(d, D3D12_HEAP_TYPE_UPLOAD, bufer(total), D3D12_RESOURCE_STATE_GENERIC_READ);
    if (fallos)
        return fallos;
    unsigned char *s = nullptr;
    hecho(subida->Map(0, nullptr, (void **)&s), "Map de la subida");
    static unsigned char mip[2][LADO_TABLERO * LADO_TABLERO * 4];
    for (UINT y = 0; y < LADO_TABLERO; y++)
        for (UINT x = 0; x < LADO_TABLERO; x++) {
            bool par = ((x / 16) + (y / 16)) % 2 == 0;
            unsigned char *p = mip[0] + 4 * (y * LADO_TABLERO + x);
            p[0] = par ? 200 : 60, p[1] = par ? 180 : 110, p[2] = par ? 140 : 70, p[3] = 255;
        }
    for (UINT k = 0; k < MIPS; k++) {
        UINT lado = LADO_TABLERO >> k;
        unsigned char *este = mip[k % 2], *sig = mip[(k + 1) % 2];
        for (UINT y = 0; y < lado; y++)
            memcpy(s + ht[k].Offset + y * ht[k].Footprint.RowPitch, este + y * lado * 4, lado * 4);
        for (UINT y = 0; y < lado / 2; y++)
            for (UINT x = 0; x < lado / 2; x++)
                for (UINT e = 0; e < 4; e++) {
                    UINT q = este[4 * ((2 * y) * lado + 2 * x) + e] + este[4 * ((2 * y) * lado + 2 * x + 1) + e] + este[4 * ((2 * y + 1) * lado + 2 * x) + e] + este[4 * ((2 * y + 1) * lado + 2 * x + 1) + e];
                    sig[4 * (y * (lado / 2) + x) + e] = (unsigned char)((q + 2) / 4);
                }
    }

    // -- Los destinos: el mapa de sombra, el HDR, su profundidad, el final y lo leido.
    D3D12_CLEAR_VALUE cz = {DXGI_FORMAT_D32_FLOAT};
    cz.DepthStencil.Depth = 1.0f;
    D3D12_CLEAR_VALUE cc = {DXGI_FORMAT_R16G16B16A16_FLOAT, {CIELO[0], CIELO[1], CIELO[2], CIELO[3]}};
    D3D12_RESOURCE_DESC dsom = textura(LADO_SOMBRA, LADO_SOMBRA, 1, DXGI_FORMAT_R32_TYPELESS, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL);
    ID3D12Resource *som = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dsom, D3D12_RESOURCE_STATE_DEPTH_WRITE, &cz);
    ID3D12Resource *hdr = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(W, H, 1, DXGI_FORMAT_R16G16B16A16_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET), D3D12_RESOURCE_STATE_RENDER_TARGET, &cc);
    ID3D12Resource *prof = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(W, H, 1, DXGI_FORMAT_D32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL), D3D12_RESOURCE_STATE_DEPTH_WRITE, &cz);
    ID3D12Resource *fin = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(W, H, 1, DXGI_FORMAT_R8G8B8A8_UNORM, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    const UINT64 EN_SOMBRA = 1024 * H;
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, bufer(EN_SOMBRA + 512), D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;

    // -- Las vistas: [0] el tablero, [1] la sombra (los dos del PS), [2] el
    // HDR y [3] el final (los del tonemap); los RTV y los DSV aparte.
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 4, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hr = {D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hz = {D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *monton = nullptr, *rtvs = nullptr, *dsvs = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap") || !hecho(d->CreateDescriptorHeap(&hr, IID_PPV_ARGS(&rtvs)), "CreateDescriptorHeap RTV") ||
        !hecho(d->CreateDescriptorHeap(&hz, IID_PPV_ARGS(&dsvs)), "CreateDescriptorHeap DSV"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0, rtv, z0;
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    monton->GetGPUDescriptorHandleForHeapStart(&g0);
    rtvs->GetCPUDescriptorHandleForHeapStart(&rtv);
    dsvs->GetCPUDescriptorHandleForHeapStart(&z0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    UINT paso_z = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV);
    auto en = [&](UINT k) { D3D12_CPU_DESCRIPTOR_HANDLE h = {h0.ptr + k * paso}; return h; };
    D3D12_SHADER_RESOURCE_VIEW_DESC sv = {};
    sv.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
    sv.ViewDimension = D3D12_SRV_DIMENSION_TEXTURE2D;
    sv.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    sv.Texture2D.MipLevels = MIPS;
    d->CreateShaderResourceView(tab, &sv, en(0));
    sv.Format = DXGI_FORMAT_R32_FLOAT;
    sv.Texture2D.MipLevels = 1;
    d->CreateShaderResourceView(som, &sv, en(1));
    sv.Format = DXGI_FORMAT_R16G16B16A16_FLOAT;
    d->CreateShaderResourceView(hdr, &sv, en(2));
    D3D12_UNORDERED_ACCESS_VIEW_DESC uv = {};
    uv.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    uv.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
    d->CreateUnorderedAccessView(fin, nullptr, &uv, en(3));
    d->CreateRenderTargetView(hdr, nullptr, rtv);
    D3D12_DEPTH_STENCIL_VIEW_DESC dv = {};
    dv.Format = DXGI_FORMAT_D32_FLOAT;
    dv.ViewDimension = D3D12_DSV_DIMENSION_TEXTURE2D;
    D3D12_CPU_DESCRIPTOR_HANDLE z1 = {z0.ptr + paso_z};
    d->CreateDepthStencilView(som, &dv, z0);
    d->CreateDepthStencilView(prof, &dv, z1);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, p_sombra, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    // La textura: cada mip de la subida a su sitio.
    for (UINT k = 0; k < MIPS; k++) {
        D3D12_TEXTURE_COPY_LOCATION de = {}, a2 = {};
        de.pResource = subida;
        de.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        de.PlacedFootprint = ht[k];
        a2.pResource = tab;
        a2.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        a2.SubresourceIndex = k;
        l->CopyTextureRegion(&a2, 0, 0, 0, &de, nullptr);
    }
    transicion(l, tab, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    l->SetGraphicsRootSignature(rs);
    l->SetGraphicsRootConstantBufferView(0, cb->GetGPUVirtualAddress());
    D3D12_VERTEX_BUFFER_VIEW vbs[2] = {{vb->GetGPUVirtualAddress(), (UINT)sizeof verts, sizeof(Vert)}, {instb->GetGPUVirtualAddress(), (UINT)sizeof inst, sizeof(Inst)}};
    D3D12_INDEX_BUFFER_VIEW ibv = {ib->GetGPUVirtualAddress(), (UINT)sizeof inds, DXGI_FORMAT_R32_UINT};
    l->IASetVertexBuffers(0, 2, vbs);
    l->IASetIndexBuffer(&ibv);
    l->IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // Lo que se dibuja en los dos pases: el terreno (instancia 0) y los cubos (1..64).
    auto todo = [&]() {
        l->DrawIndexedInstanced(IT, 1, 0, 0, 0);
        l->DrawIndexedInstanced(36, CUBOS, IT, NT, 1);
    };
    // 1. La sombra.
    D3D12_VIEWPORT vs = {0, 0, (float)LADO_SOMBRA, (float)LADO_SOMBRA, 0, 1};
    D3D12_RECT ts = {0, 0, (LONG)LADO_SOMBRA, (LONG)LADO_SOMBRA};
    l->RSSetViewports(1, &vs);
    l->RSSetScissorRects(1, &ts);
    l->ClearDepthStencilView(z0, D3D12_CLEAR_FLAG_DEPTH, 1.0f, 0, 0, nullptr);
    l->OMSetRenderTargets(0, nullptr, FALSE, &z0);
    todo();
    transicion(l, som, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
    // 2. La escena en HDR, y el vidrio encima.
    D3D12_VIEWPORT vm = {0, 0, (float)W, (float)H, 0, 1};
    D3D12_RECT tm = {0, 0, (LONG)W, (LONG)H};
    l->RSSetViewports(1, &vm);
    l->RSSetScissorRects(1, &tm);
    l->ClearRenderTargetView(rtv, CIELO, 0, nullptr);
    l->ClearDepthStencilView(z1, D3D12_CLEAR_FLAG_DEPTH, 1.0f, 0, 0, nullptr);
    l->OMSetRenderTargets(1, &rtv, FALSE, &z1);
    l->SetPipelineState(p_escena);
    l->SetDescriptorHeaps(1, &monton);
    l->SetGraphicsRootDescriptorTable(1, g0);
    todo();
    l->SetPipelineState(p_vidrio);
    l->DrawIndexedInstanced(36, 1, IT, NT, 1 + CUBOS);
    // 3. El tonemap.
    transicion(l, hdr, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE);
    l->SetPipelineState(p_tono);
    l->SetComputeRootSignature(rs_tono);
    D3D12_GPU_DESCRIPTOR_HANDLE g2 = {g0.ptr + 2 * (UINT64)paso};
    l->SetComputeRootDescriptorTable(0, g2);
    l->Dispatch((W + 7) / 8, (H + 7) / 8, 1);
    // A leer: el final, y el texel del centro del mapa de sombra.
    transicion(l, fin, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    transicion(l, som, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_SOURCE);
    D3D12_TEXTURE_COPY_LOCATION a3 = {}, de3 = {};
    a3.pResource = leida;
    a3.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a3.PlacedFootprint = {0, {DXGI_FORMAT_R8G8B8A8_UNORM, W, H, 1, 1024}};
    de3.pResource = fin;
    de3.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    l->CopyTextureRegion(&a3, 0, 0, 0, &de3, nullptr);
    a3.PlacedFootprint = {EN_SOMBRA, {DXGI_FORMAT_R32_TYPELESS, 1, 1, 1, 256}};
    de3.pResource = som;
    D3D12_BOX centro = {LADO_SOMBRA / 2, LADO_SOMBRA / 2, 0, LADO_SOMBRA / 2 + 1, LADO_SOMBRA / 2 + 1, 1};
    l->CopyTextureRegion(&a3, 0, 0, 0, &de3, &centro);
    hecho(l->Close(), "Close");
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    hecho(cola->Signal(valla, 1), "Signal");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        hecho(valla->SetEventOnCompletion(1, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 60000);
    }
    unsigned char *m = nullptr;
    if (!hecho(leida->Map(0, nullptr, (void **)&m), "Map de lo leido"))
        return fallos;
    char msg[240];

    // A: el cielo.
    const unsigned char *p0 = m;
    UINT q[3] = {tono(CIELO[0]), tono(CIELO[1]), tono(CIELO[2])};
    bool cielo = true;
    for (int e = 0; e < 3; e++)
        cielo = cielo && (p0[e] + 1 >= q[e] && p0[e] <= q[e] + 1);
    snprintf(msg, sizeof msg, "A, el cielo: el pixel (0, 0) es (%u, %u, %u, %u) y la limpieza con su tonemap da (%u, %u, %u, 255)", p0[0], p0[1], p0[2], p0[3], q[0], q[1], q[2]);
    decir(cielo && p0[3] == 255, msg);
    // B: la sombra.
    float zc = *(const float *)(m + EN_SOMBRA);
    snprintf(msg, sizeof msg, "B, el mapa de la luz: el centro esta a %.4f (el terreno: entre 0 y 1, no la limpieza)", zc);
    decir(zc > 0.0f && zc < 0.99f, msg);
    // C: contra la imagen de Windows.
    if (guardar_ref) {
        bool ok = guardar(m);
        decir(ok, ok ? "C, guardada: escena.ref (la referencia) y escena.bmp (para mirarla)" : "C, no se pudo escribir escena.ref");
    } else {
        FILE *f = fopen("escena.ref", "rb");
        static unsigned char ref[8 + 8 + W * H * 4];
        size_t n = f ? fread(ref, 1, sizeof ref, f) : 0;
        if (f)
            fclose(f);
        UINT medidas[2] = {};
        memcpy(medidas, ref + 8, 8);
        if (n != sizeof ref || memcmp(ref, "ESCENA1", 8) != 0 || medidas[0] != W || medidas[1] != H) {
            printf("  nota  C: sin escena.ref (o de otra medida): corre `escena.exe guardar` en Windows y ponla al lado\n");
        } else {
            UINT cerca = 0, max = 0;
            double suma = 0;
            for (UINT y = 0; y < H; y++)
                for (UINT x = 0; x < W; x++) {
                    UINT peor = 0;
                    for (int e = 0; e < 3; e++) {
                        int a4 = m[y * 1024 + 4 * x + e], b4 = ref[16 + 4 * (y * W + x) + e];
                        UINT dif = (UINT)(a4 > b4 ? a4 - b4 : b4 - a4);
                        suma += dif;
                        peor = dif > peor ? dif : peor;
                    }
                    cerca += peor <= 8;
                    max = peor > max ? peor : max;
                }
            double pct = 100.0 * cerca / (W * H), media = suma / (W * H * 3);
            snprintf(msg, sizeof msg, "C, contra la imagen de Windows: %.2f %% de los pixeles a 8 o menos (pide 98), la media %.3f (pide 2 o menos), la peor %u", pct, media, max);
            decir(pct >= 98.0 && media <= 2.0, msg);
        }
    }
    printf("escena.exe: la escena 3D dura es la de Windows\n");
    return fallos;
}
