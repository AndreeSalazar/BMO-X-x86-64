// postpro.cpp -- el juez del COMPUTO DE JUEGO (06-10), con el CS de
// postpro.hlsl (aqui al lado): lo que hace un posproceso de un juego. Hasta
// el 06-10 un CS que elegia su textura por un indice calculado (el bindless
// de Cyberpunk) la leia NULA, y uno que muestreaba, tocaba un UAV de
// textura o hacia un Interlocked iba siempre por el interprete.
//
// Un Dispatch de 2 x 2 grupos de 8 x 8 (16 x 16 hilos). Cada hilo:
//   A  elige SU textura (i = (x + y + cual) % 3) de un array SIN LIMITE en
//      un espacio aparte (t0, space1), la lee (Load) y le suma lo que
//      muestrea de la escena (SampleLevel, punto), a un RWTexture2D de 16 x
//      16 en RGBA32F: (i + x / 16, 10 i + y / 16, 100 i + 0.5, 4 i).
//   B  cuenta los hilos de cada textura con InterlockedAdd: 86, 85 y 85
//      (con cual = 1).
//   C  el primero escribe GetDimensions de su UAV: 16 x 16.
//
// Todo exacto (floats chicos, punto). Sale con el numero de fallos; en
// Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n.p2align 4\n.globl cs_post\n cs_post:\n .incbin \"postpro_cs.dxil\"\n .globl cs_post_fin\n cs_post_fin:\n.text\n");
extern "C" const unsigned char cs_post[], cs_post_fin[];

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

static D3D12_RESOURCE_DESC textura(UINT lado, D3D12_RESOURCE_FLAGS f) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    r.Width = lado;
    r.Height = lado;
    r.DepthOrArraySize = 1;
    r.MipLevels = 1;
    r.Format = DXGI_FORMAT_R32G32B32A32_FLOAT;
    r.SampleDesc.Count = 1;
    r.Flags = f;
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

static const UINT LADO = 16, CUAL = 1;
// La subida: la escena y las tres texturas del array, cada una a su sitio.
static const UINT64 SUB_ESCENA = 0, SUB_TEX = 8192, PASO_TEX = 1024, LEIDO = 8192;

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

    // La raiz: 0, dos constantes (b0); 1, la escena (t0); 2, el array SIN
    // LIMITE (t0, space1); 3, los dos UAV (u0, u1). Y el muestreador de
    // punto, estatico.
    D3D12_DESCRIPTOR_RANGE rangos[3] = {
        {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 0, 0, 0},
        {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, UINT_MAX, 0, 1, 0},
        {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 2, 0, 0, 0},
    };
    D3D12_ROOT_PARAMETER rp[4] = {};
    rp[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    rp[0].Constants.Num32BitValues = 2;
    for (UINT k = 0; k < 3; k++) {
        rp[1 + k].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
        rp[1 + k].DescriptorTable.NumDescriptorRanges = 1;
        rp[1 + k].DescriptorTable.pDescriptorRanges = &rangos[k];
    }
    D3D12_STATIC_SAMPLER_DESC ss = {};
    ss.Filter = D3D12_FILTER_MIN_MAG_MIP_POINT;
    ss.AddressU = ss.AddressV = ss.AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
    ss.MaxLOD = D3D12_FLOAT32_MAX;
    D3D12_ROOT_SIGNATURE_DESC rd = {4, rp, 1, &ss, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;
    D3D12_COMPUTE_PIPELINE_STATE_DESC pd = {};
    pd.pRootSignature = rs;
    pd.CS.pShaderBytecode = cs_post;
    pd.CS.BytecodeLength = (SIZE_T)(cs_post_fin - cs_post);
    ID3D12PipelineState *pso = nullptr;
    if (!hecho(d->CreateComputePipelineState(&pd, IID_PPV_ARGS(&pso)), "CreateComputePipelineState"))
        return fallos;

    // Los recursos: la escena (16 x 16), las tres del array (4 x 4), la
    // salida (16 x 16, UAV), la cuenta (UAV crudo), la subida y lo leido.
    const D3D12_RESOURCE_STATES CD = D3D12_RESOURCE_STATE_COPY_DEST, UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    ID3D12Resource *escena = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(LADO, D3D12_RESOURCE_FLAG_NONE), CD);
    ID3D12Resource *texs[3];
    for (UINT k = 0; k < 3; k++)
        texs[k] = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(4, D3D12_RESOURCE_FLAG_NONE), CD);
    ID3D12Resource *salida = recurso(d, D3D12_HEAP_TYPE_DEFAULT, textura(LADO, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), UA);
    ID3D12Resource *cuenta = recurso(d, D3D12_HEAP_TYPE_DEFAULT, bufer(256, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), UA);
    ID3D12Resource *subida = recurso(d, D3D12_HEAP_TYPE_UPLOAD, bufer(SUB_TEX + 3 * PASO_TEX, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, bufer(LEIDO + 256, D3D12_RESOURCE_FLAG_NONE), CD);
    if (fallos)
        return fallos;
    float *m = nullptr;
    if (!hecho(subida->Map(0, nullptr, (void **)&m), "Map de la subida"))
        return fallos;
    // La escena: (x / 16, y / 16, 0.5, 0), filas de 256 bytes (16 texeles).
    for (UINT y = 0; y < LADO; y++)
        for (UINT x = 0; x < LADO; x++) {
            float *t = m + (SUB_ESCENA / 4) + y * 64 + 4 * x;
            t[0] = x / 16.0f, t[1] = y / 16.0f, t[2] = 0.5f, t[3] = 0.0f;
        }
    // La textura k del array: (k, 10 k, 100 k, 4 k) en cada texel, filas de
    // 256 bytes (la alineacion de una copia).
    for (UINT k = 0; k < 3; k++)
        for (UINT y = 0; y < 4; y++)
            for (UINT x = 0; x < 4; x++) {
                float *t = m + (SUB_TEX + k * PASO_TEX) / 4 + y * 64 + 4 * x;
                t[0] = (float)k, t[1] = 10.0f * k, t[2] = 100.0f * k, t[3] = 4.0f * k;
            }
    subida->Unmap(0, nullptr);

    // Los descriptores: 0 la escena, 1..3 el array, 4 la salida, 5 la cuenta.
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 6, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *monton = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    auto en = [&](UINT k) { D3D12_CPU_DESCRIPTOR_HANDLE h = {h0.ptr + k * paso}; return h; };
    d->CreateShaderResourceView(escena, nullptr, en(0));
    for (UINT k = 0; k < 3; k++)
        d->CreateShaderResourceView(texs[k], nullptr, en(1 + k));
    d->CreateUnorderedAccessView(salida, nullptr, nullptr, en(4));
    D3D12_UNORDERED_ACCESS_VIEW_DESC u = {};
    u.Format = DXGI_FORMAT_R32_TYPELESS;
    u.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    u.Buffer.NumElements = 64;
    u.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    d->CreateUnorderedAccessView(cuenta, nullptr, &u, en(5));
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetGPUDescriptorHandleForHeapStart(&g0);
    auto gpu = [&](UINT k) { D3D12_GPU_DESCRIPTOR_HANDLE h = {g0.ptr + k * (UINT64)paso}; return h; };

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pso, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    // La subida, a la escena y al array.
    auto subir = [&](ID3D12Resource *t, UINT lado, UINT64 desde) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = t;
        a.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        de.pResource = subida;
        de.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        de.PlacedFootprint.Offset = desde;
        de.PlacedFootprint.Footprint = {DXGI_FORMAT_R32G32B32A32_FLOAT, lado, lado, 1, 256};
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
        transicion(l, t, CD, D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE);
    };
    subir(escena, LADO, SUB_ESCENA);
    for (UINT k = 0; k < 3; k++)
        subir(texs[k], 4, SUB_TEX + k * PASO_TEX);
    l->SetComputeRootSignature(rs);
    l->SetDescriptorHeaps(1, &monton);
    const UINT k2[2] = {CUAL, LADO};
    l->SetComputeRoot32BitConstants(0, 2, k2, 0);
    l->SetComputeRootDescriptorTable(1, gpu(0));
    l->SetComputeRootDescriptorTable(2, gpu(1));
    l->SetComputeRootDescriptorTable(3, gpu(4));
    l->Dispatch(2, 2, 1);
    transicion(l, salida, UA, D3D12_RESOURCE_STATE_COPY_SOURCE);
    transicion(l, cuenta, UA, D3D12_RESOURCE_STATE_COPY_SOURCE);
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = leida;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Footprint = {DXGI_FORMAT_R32G32B32A32_FLOAT, LADO, LADO, 1, 256};
    de.pResource = salida;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    l->CopyBufferRegion(leida, LEIDO, cuenta, 0, 256);
    hecho(l->Close(), "Close");
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    hecho(cola->Signal(valla, 1), "Signal");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        hecho(valla->SetEventOnCompletion(1, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 10000);
    }
    unsigned char *r = nullptr;
    if (!hecho(leida->Map(0, nullptr, (void **)&r), "Map de lo leido"))
        return fallos;
    char msg[240];

    // A: cada texel de la salida.
    UINT malos = 0, mx = 0, my = 0, quiero_c[3] = {};
    float visto[4] = {}, quiero[4] = {};
    for (UINT y = 0; y < LADO; y++)
        for (UINT x = 0; x < LADO; x++) {
            UINT i = (x + y + CUAL) % 3;
            quiero_c[i]++;
            const float *t = (const float *)(r + y * 256 + 16 * x);
            float q[4] = {i + x / 16.0f, 10.0f * i + y / 16.0f, 100.0f * i + 0.5f, 4.0f * i};
            if (memcmp(t, q, 16) != 0 && malos++ == 0) {
                mx = x, my = y;
                memcpy(visto, t, 16);
                memcpy(quiero, q, 16);
            }
        }
    if (malos == 0)
        snprintf(msg, sizeof msg, "A, cada hilo lee SU textura del array sin limite (indice calculado), le suma la escena y la escribe en un RWTexture2D");
    else
        snprintf(msg, sizeof msg, "A, %u texeles distintos; el (%u, %u) es (%g, %g, %g, %g) y tenia que ser (%g, %g, %g, %g)", malos, mx, my, visto[0], visto[1], visto[2], visto[3], quiero[0], quiero[1], quiero[2], quiero[3]);
    decir(malos == 0, msg);

    // B y C: la cuenta.
    const UINT *c = (const UINT *)(r + LEIDO);
    snprintf(msg, sizeof msg, "B, InterlockedAdd: %u, %u y %u hilos por textura (%u, %u y %u)", c[0], c[1], c[2], quiero_c[0], quiero_c[1], quiero_c[2]);
    decir(c[0] == quiero_c[0] && c[1] == quiero_c[1] && c[2] == quiero_c[2], msg);
    snprintf(msg, sizeof msg, "C, GetDimensions del RWTexture2D: %u (16016)", c[4]);
    decir(c[4] == 16016, msg);

    printf("postpro.exe: el computo de un posproceso es el de Windows\n");
    return fallos;
}
