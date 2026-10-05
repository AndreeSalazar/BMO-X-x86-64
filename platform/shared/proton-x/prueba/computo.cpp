// computo.cpp -- el .exe de E2.3a de la ESCALERA de PROTON-X (N5.5, 05-10):
// el COMPUTO de D3D12, juzgado por el mismo .exe. Un sombreador de computo
// (computo.hlsl, aqui al lado: 64 hilos por grupo que se pasan sus datos por
// la memoria COMPARTIDA, con una BARRERA en medio) corrido dos veces:
//
//   A  en la cola DIRECTA: la tabla (SRV t0 + UAV u0) y las constantes de la
//      RAIZ (n = 250, escala = 2); Dispatch(4, 1, 1). El ultimo grupo
//      escribe 58 de 64: los 6 del final se quedan como estaban.
//   B  en una cola de COMPUTO, que espera con una VALLA a la directa: un CBV
//      en la raiz (n = 256, escala = -0.5) y la tabla en el otro parametro.
//      La directa espera a su vez a la de computo para copiar lo escrito.
//
// Cada valor que sale se compara con el de la cuenta en la CPU: el elemento
// i lee el de su grupo en el hilo de enfrente (63 - i % 64), lo multiplica
// por la escala, y en w deja grupo * 1000 + hilo. Todo exacto en float: la
// 3060 y la casa tienen que dar LOS MISMOS bits.
//
// Sale con el numero de fallos. En Windows dice lo mismo (es de consola: lo
// que falle, se lee).
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

// El sombreador, del .dxil de aqui al lado (HACER.txt dice como sale).
__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl cs_computo\n"
        "cs_computo:\n"
        ".incbin \"computo.dxil\"\n"
        ".globl cs_computo_fin\n"
        "cs_computo_fin:\n"
        ".text\n");
extern "C" const unsigned char cs_computo[], cs_computo_fin[];

static const UINT N = 256;      // elementos (float4) de cada bufer
static const UINT BYTES = N * 16;

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

static ID3D12Resource *bufer(ID3D12Device *d, D3D12_HEAP_TYPE monton, UINT64 bytes, D3D12_RESOURCE_FLAGS f, D3D12_RESOURCE_STATES e) {
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
    r.Flags = f;
    ID3D12Resource *b = nullptr;
    hecho(d->CreateCommittedResource(&p, D3D12_HEAP_FLAG_NONE, &r, e, nullptr, IID_PPV_ARGS(&b)), "CreateCommittedResource");
    return b;
}

// Map: lo que solo se ESCRIBE (UPLOAD) dice que no lee nada; la lectura
// (READBACK) lo lee todo.
static void *mapa(ID3D12Resource *b, bool lee = false) {
    void *p = nullptr;
    D3D12_RANGE nada = {0, 0};
    hecho(b->Map(0, lee ? nullptr : &nada, &p), "Map");
    return p;
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

// Una root signature serializada y creada: `p` parametros.
static ID3D12RootSignature *raiz(ID3D12Device *d, const D3D12_ROOT_PARAMETER *p, UINT n) {
    D3D12_ROOT_SIGNATURE_DESC r = {};
    r.NumParameters = n;
    r.pParameters = p;
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&r, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    return rs;
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs) {
    D3D12_COMPUTE_PIPELINE_STATE_DESC c = {};
    c.pRootSignature = rs;
    c.CS.pShaderBytecode = cs_computo;
    c.CS.BytecodeLength = (SIZE_T)(cs_computo_fin - cs_computo);
    ID3D12PipelineState *p = nullptr;
    hecho(d->CreateComputePipelineState(&c, IID_PPV_ARGS(&p)), "CreateComputePipelineState");
    return p;
}

// Lo que tiene que salir en el elemento `i` con (n, escala); `entrada` es la
// de la CPU y `antes` lo que habia (lo que no se escribe se queda).
static void esperado(const float *entrada, UINT i, UINT n, float escala, float antes, float out[4]) {
    if (i >= n) {
        for (int c = 0; c < 4; c++)
            out[c] = antes;
        return;
    }
    UINT g = i / 64, gi = i % 64;
    const float *v = entrada + (g * 64 + 63 - gi) * 4;
    for (int c = 0; c < 3; c++)
        out[c] = v[c] * escala;
    out[3] = (float)(g * 1000 + gi);
}

// Comparar los N float4 de `leido` con los de (n, escala), BIT A BIT.
static void juzgar(const char *que, const float *leido, const float *entrada, UINT n, float escala, UINT escritos) {
    UINT malos = 0, primero = 0;
    float quiero[4] = {};
    for (UINT i = 0; i < N; i++) {
        float e[4];
        esperado(entrada, i, n, escala, -1.0f, e);
        if (memcmp(e, leido + i * 4, 16) != 0) {
            if (malos++ == 0) {
                primero = i;
                memcpy(quiero, e, 16);
            }
        }
    }
    char m[320];
    if (malos == 0)
        snprintf(m, sizeof m, "%s: %u de %u escritos, los %u valores bit a bit", que, escritos, N, N * 4);
    else
        snprintf(m, sizeof m, "%s: %u elementos distintos; el %u es (%g, %g, %g, %g) y tenia que ser (%g, %g, %g, %g)", que, malos, primero,
                 leido[primero * 4], leido[primero * 4 + 1], leido[primero * 4 + 2], leido[primero * 4 + 3], quiero[0], quiero[1], quiero[2], quiero[3]);
    decir(malos == 0, m);
}

int main() {
    ID3D12Device *d = nullptr;
    if (!hecho(D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d)), "D3D12CreateDevice"))
        return 1;

    // Las dos colas, sus allocators y sus listas, y UNA valla entre las dos.
    D3D12_COMMAND_QUEUE_DESC qd = {};
    qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    ID3D12CommandQueue *directa = nullptr, *de_computo = nullptr;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&directa)), "CreateCommandQueue DIRECT");
    qd.Type = D3D12_COMMAND_LIST_TYPE_COMPUTE;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&de_computo)), "CreateCommandQueue COMPUTE");
    ID3D12CommandAllocator *ad = nullptr, *ac = nullptr;
    hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&ad)), "CreateCommandAllocator DIRECT");
    hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_COMPUTE, IID_PPV_ARGS(&ac)), "CreateCommandAllocator COMPUTE");
    ID3D12Fence *valla = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&valla)), "CreateFence");
    if (fallos)
        return fallos;

    // A: [0] la tabla (SRV t0, UAV u0), [1] dos constantes de 32 bits en b0.
    D3D12_DESCRIPTOR_RANGE rangos[2] = {};
    rangos[0].RangeType = D3D12_DESCRIPTOR_RANGE_TYPE_SRV;
    rangos[0].NumDescriptors = 1;
    rangos[0].OffsetInDescriptorsFromTableStart = D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND;
    rangos[1].RangeType = D3D12_DESCRIPTOR_RANGE_TYPE_UAV;
    rangos[1].NumDescriptors = 1;
    rangos[1].OffsetInDescriptorsFromTableStart = D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND;
    D3D12_ROOT_PARAMETER tabla = {};
    tabla.ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    tabla.DescriptorTable.NumDescriptorRanges = 2;
    tabla.DescriptorTable.pDescriptorRanges = rangos;
    tabla.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_PARAMETER constantes = {};
    constantes.ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    constantes.Constants.Num32BitValues = 2;
    constantes.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_PARAMETER pa[2] = {tabla, constantes};
    ID3D12RootSignature *ra = raiz(d, pa, 2);
    // B: [0] un CBV en la raiz (b0), [1] la misma tabla.
    D3D12_ROOT_PARAMETER cbv = {};
    cbv.ParameterType = D3D12_ROOT_PARAMETER_TYPE_CBV;
    cbv.ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
    D3D12_ROOT_PARAMETER pb[2] = {cbv, tabla};
    ID3D12RootSignature *rb = raiz(d, pb, 2);
    if (fallos)
        return fallos;
    ID3D12PipelineState *psoa = pso(d, ra), *psob = pso(d, rb);
    if (fallos)
        return fallos;
    decir(true, "CreateComputePipelineState: el mismo CS con dos root signatures");

    // Los buferes: la entrada (UPLOAD, leida como SRV), dos salidas (DEFAULT,
    // con UAV), lo que hay en ellas antes (-1), el cbuffer de B y la lectura.
    ID3D12Resource *entrada = bufer(d, D3D12_HEAP_TYPE_UPLOAD, BYTES, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *antes = bufer(d, D3D12_HEAP_TYPE_UPLOAD, BYTES, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *cb = bufer(d, D3D12_HEAP_TYPE_UPLOAD, 256, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *sa = bufer(d, D3D12_HEAP_TYPE_DEFAULT, BYTES, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_DEST);
    ID3D12Resource *sb = bufer(d, D3D12_HEAP_TYPE_DEFAULT, BYTES, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_DEST);
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, 2 * BYTES, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    static float ent[N * 4];
    for (UINT i = 0; i < N; i++) {
        ent[i * 4 + 0] = (float)i;
        ent[i * 4 + 1] = (float)i * 0.25f;
        ent[i * 4 + 2] = -(float)(i * 3 + 1);
        ent[i * 4 + 3] = (float)(i % 7);
    }
    memcpy(mapa(entrada), ent, BYTES);
    float *a = (float *)mapa(antes);
    for (UINT i = 0; i < N * 4; i++)
        a[i] = -1.0f;
    struct { UINT n; float escala; } kb = {N, -0.5f};
    memcpy(mapa(cb), &kb, sizeof kb);

    // El monton de descriptores: [0] SRV y [1] UAV de A, [2] SRV y [3] UAV de B.
    D3D12_DESCRIPTOR_HEAP_DESC hd = {};
    hd.Type = D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV;
    hd.NumDescriptors = 4;
    hd.Flags = D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE;
    ID3D12DescriptorHeap *monton = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap"))
        return fallos;
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    D3D12_CPU_DESCRIPTOR_HANDLE cpu;
    D3D12_GPU_DESCRIPTOR_HANDLE gpu;
    monton->GetCPUDescriptorHandleForHeapStart(&cpu);
    monton->GetGPUDescriptorHandleForHeapStart(&gpu);
    D3D12_SHADER_RESOURCE_VIEW_DESC srv = {};
    srv.Format = DXGI_FORMAT_UNKNOWN;
    srv.ViewDimension = D3D12_SRV_DIMENSION_BUFFER;
    srv.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
    srv.Buffer.NumElements = N;
    srv.Buffer.StructureByteStride = 16;
    D3D12_UNORDERED_ACCESS_VIEW_DESC uav = {};
    uav.Format = DXGI_FORMAT_UNKNOWN;
    uav.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    uav.Buffer.NumElements = N;
    uav.Buffer.StructureByteStride = 16;
    ID3D12Resource *salidas[2] = {sa, sb};
    for (UINT k = 0; k < 2; k++) {
        D3D12_CPU_DESCRIPTOR_HANDLE h = {cpu.ptr + (SIZE_T)(2 * k) * paso};
        d->CreateShaderResourceView(entrada, &srv, h);
        h.ptr += paso;
        d->CreateUnorderedAccessView(salidas[k], nullptr, &uav, h);
    }

    // 1. DIRECTA: las salidas a -1, y A.
    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, ad, psoa, IID_PPV_ARGS(&l)), "CreateCommandList DIRECT"))
        return fallos;
    l->CopyBufferRegion(sa, 0, antes, 0, BYTES);
    l->CopyBufferRegion(sb, 0, antes, 0, BYTES);
    barrera(l, sa, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    barrera(l, sb, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    l->SetDescriptorHeaps(1, &monton);
    l->SetComputeRootSignature(ra);
    l->SetComputeRootDescriptorTable(0, gpu);
    struct { UINT n; float escala; } ka = {250, 2.0f};
    l->SetComputeRoot32BitConstants(1, 2, &ka, 0);
    l->Dispatch(4, 1, 1);
    barrera(l, sa, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    l->CopyBufferRegion(leida, 0, sa, 0, BYTES);
    hecho(l->Close(), "Close DIRECT 1");
    ID3D12CommandList *ls[1] = {l};
    directa->ExecuteCommandLists(1, ls);
    hecho(directa->Signal(valla, 1), "Signal 1");

    // 2. De COMPUTO, cuando la directa llegue a 1: B.
    ID3D12GraphicsCommandList *lc = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_COMPUTE, ac, psob, IID_PPV_ARGS(&lc)), "CreateCommandList COMPUTE"))
        return fallos;
    lc->SetDescriptorHeaps(1, &monton);
    lc->SetComputeRootSignature(rb);
    lc->SetComputeRootConstantBufferView(0, cb->GetGPUVirtualAddress());
    lc->SetComputeRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE{gpu.ptr + 2 * (UINT64)paso});
    lc->Dispatch(4, 1, 1);
    hecho(lc->Close(), "Close COMPUTE");
    hecho(de_computo->Wait(valla, 1), "Wait 1 (la de computo espera a la directa)");
    ls[0] = lc;
    de_computo->ExecuteCommandLists(1, ls);
    hecho(de_computo->Signal(valla, 2), "Signal 2");

    // 3. DIRECTA otra vez, cuando la de computo llegue a 2: copiar lo de B.
    hecho(ad->Reset(), "Reset del allocator");
    hecho(l->Reset(ad, nullptr), "Reset de la lista");
    barrera(l, sb, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    l->CopyBufferRegion(leida, BYTES, sb, 0, BYTES);
    hecho(l->Close(), "Close DIRECT 2");
    hecho(directa->Wait(valla, 2), "Wait 2 (la directa espera a la de computo)");
    ls[0] = l;
    directa->ExecuteCommandLists(1, ls);
    hecho(directa->Signal(valla, 3), "Signal 3");

    // La CPU espera a 3, y lee.
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 3) {
        hecho(valla->SetEventOnCompletion(3, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 10000);
    }
    decir(valla->GetCompletedValue() == 3, "la valla llega a 3: directa, de computo y directa, en orden");
    const float *r = (const float *)mapa(leida, true);
    if (r == nullptr)
        return fallos;
    juzgar("A, cola DIRECTA, constantes en la raiz (n = 250, escala = 2)", r, ent, 250, 2.0f, 250);
    juzgar("B, cola de COMPUTO, CBV en la raiz (n = 256, escala = -0.5)", r + N * 4, ent, 256, -0.5f, 256);

    printf("computo.exe: el computo de D3D12 es el de Windows\n");
    return fallos;
}
