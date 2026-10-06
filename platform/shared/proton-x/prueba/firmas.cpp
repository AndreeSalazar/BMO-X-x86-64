// firmas.cpp -- el juez de las ROOT SIGNATURES 1.1 y de las que vienen
// DENTRO del sombreador (06-10), con lo de firmas.hlsl (aqui al lado). Un
// motor moderno escribe su firma en HLSL ([RootSignature(...)]) y `dxc` la
// mete en el sombreador en 1.1; o la guarda ya serializada. Hasta el 06-10
// la casa solo leia la 1.0, y un PSO sin root signature no se creaba.
//
// Cada caso crea su root signature de otra forma y corre CSFirma con ella:
// a[i] * k.x + b[i] * k.y + caso a un UAV de la RAIZ, y + 1000 a su sitio de
// un UAV de la TABLA. Si la firma se leyo mal (una tabla corrida, una
// constante en otro sitio), los numeros salen otros.
//
//   A  CreateRootSignature con la 1.1 que hace `dxc -T rootsig_1_1`.
//   B  CreateRootSignature con la 1.0 (`dxc -T rootsig_1_0`).
//   C  CreateRootSignature con el SOMBREADOR entero (su parte RTS0).
//   D  CreateComputePipelineState SIN root signature: la del sombreador; en
//      la lista se pone la de A (compatible: la misma).
//   E  CheckFeatureSupport(ROOT_SIGNATURE): pidiendo 1.1 dice 1.1; pidiendo
//      1.0, 1.0.
//   F  D3D12SerializeVersionedRootSignature de una D3D12_ROOT_SIGNATURE_DESC1
//      (con banderas) y CreateRootSignature con lo que sale.
//
// Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define BLOB(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" BLOB(cs_firma, "firmas_cs.dxil") BLOB(rs_11, "firmas_11.rts0") BLOB(rs_10, "firmas_10.rts0") ".text\n");
extern "C" const unsigned char cs_firma[], cs_firma_fin[], rs_11[], rs_11_fin[], rs_10[], rs_10_fin[];

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

static const UINT CASOS = 5;

int main() {
    ID3D12Device *d = nullptr;
    if (!hecho(D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d)), "D3D12CreateDevice"))
        return 1;

    // E: lo que dice que lee.
    D3D12_FEATURE_DATA_ROOT_SIGNATURE v = {D3D_ROOT_SIGNATURE_VERSION_1_1};
    HRESULT h11 = d->CheckFeatureSupport(D3D12_FEATURE_ROOT_SIGNATURE, &v, sizeof v);
    D3D12_FEATURE_DATA_ROOT_SIGNATURE v0 = {D3D_ROOT_SIGNATURE_VERSION_1_0};
    HRESULT h10 = d->CheckFeatureSupport(D3D12_FEATURE_ROOT_SIGNATURE, &v0, sizeof v0);
    char msg[240];
    snprintf(msg, sizeof msg, "E, CheckFeatureSupport(ROOT_SIGNATURE): pidiendo 1.1 dice %#x, pidiendo 1.0 dice %#x", (unsigned)v.HighestVersion, (unsigned)v0.HighestVersion);
    decir(SUCCEEDED(h11) && SUCCEEDED(h10) && v.HighestVersion == D3D_ROOT_SIGNATURE_VERSION_1_1 && v0.HighestVersion == D3D_ROOT_SIGNATURE_VERSION_1_0, msg);

    // A, B, C: tres root signatures de tres blobs.
    ID3D12RootSignature *rs[CASOS] = {};
    hecho(d->CreateRootSignature(0, rs_11, (SIZE_T)(rs_11_fin - rs_11), IID_PPV_ARGS(&rs[0])), "A, CreateRootSignature con la 1.1 de dxc");
    hecho(d->CreateRootSignature(0, rs_10, (SIZE_T)(rs_10_fin - rs_10), IID_PPV_ARGS(&rs[1])), "B, CreateRootSignature con la 1.0 de dxc");
    hecho(d->CreateRootSignature(0, cs_firma, (SIZE_T)(cs_firma_fin - cs_firma), IID_PPV_ARGS(&rs[2])), "C, CreateRootSignature con el sombreador");
    // D: la del PSO es la del sombreador; en la lista, la de A.
    rs[3] = rs[0];
    // F: la misma, en una D3D12_ROOT_SIGNATURE_DESC1, serializada aqui.
    D3D12_DESCRIPTOR_RANGE1 rangos[2] = {};
    rangos[0] = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 2, 0, 0, D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND};
    rangos[1] = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 1, 0, D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_VOLATILE, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND};
    D3D12_ROOT_PARAMETER1 ps[3] = {};
    ps[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    ps[0].Constants.Num32BitValues = 4;
    ps[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[1].DescriptorTable.NumDescriptorRanges = 2;
    ps[1].DescriptorTable.pDescriptorRanges = rangos;
    ps[2].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    ps[2].Descriptor.Flags = D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE;
    D3D12_VERSIONED_ROOT_SIGNATURE_DESC vd = {};
    vd.Version = D3D_ROOT_SIGNATURE_VERSION_1_1;
    vd.Desc_1_1.NumParameters = 3;
    vd.Desc_1_1.pParameters = ps;
    ID3DBlob *serializada = nullptr, *error = nullptr;
    if (hecho(D3D12SerializeVersionedRootSignature(&vd, &serializada, &error), "F, D3D12SerializeVersionedRootSignature de una 1.1"))
        hecho(d->CreateRootSignature(0, serializada->GetBufferPointer(), serializada->GetBufferSize(), IID_PPV_ARGS(&rs[4])), "F, CreateRootSignature con lo serializado");
    if (fallos)
        return fallos;

    // Los PSO: con su root signature, y D sin ella.
    ID3D12PipelineState *pso[CASOS] = {};
    for (UINT c = 0; c < CASOS; c++) {
        D3D12_COMPUTE_PIPELINE_STATE_DESC pd = {};
        pd.pRootSignature = c == 3 ? nullptr : rs[c];
        pd.CS.pShaderBytecode = cs_firma;
        pd.CS.BytecodeLength = (SIZE_T)(cs_firma_fin - cs_firma);
        snprintf(msg, sizeof msg, "CreateComputePipelineState del caso %c", 'A' + (c < 3 ? c : c + 1));
        if (c == 3)
            snprintf(msg, sizeof msg, "D, CreateComputePipelineState SIN root signature (la del sombreador)");
        hecho(d->CreateComputePipelineState(&pd, IID_PPV_ARGS(&pso[c])), msg);
    }
    if (fallos)
        return fallos;

    D3D12_COMMAND_QUEUE_DESC qd = {};
    qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    ID3D12CommandQueue *cola = nullptr;
    hecho(d->CreateCommandQueue(&qd, IID_PPV_ARGS(&cola)), "CreateCommandQueue");
    ID3D12CommandAllocator *al = nullptr;
    hecho(d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&al)), "CreateCommandAllocator");
    ID3D12Fence *valla = nullptr;
    hecho(d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&valla)), "CreateFence");

    // a = 1..4 y b = 10..40, en un bufer de subida; la tabla y la raiz,
    // UAV; lo leido, de vuelta.
    const D3D12_RESOURCE_STATES UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    ID3D12Resource *datos = bufer(d, D3D12_HEAP_TYPE_UPLOAD, 256, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    ID3D12Resource *tabla = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 256, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, UA);
    ID3D12Resource *raiz = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 256 * CASOS, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, UA);
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, 256 + 256 * CASOS, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;
    UINT *m = nullptr;
    if (!hecho(datos->Map(0, nullptr, (void **)&m), "Map de los datos"))
        return fallos;
    for (UINT i = 0; i < 4; i++)
        m[i] = i + 1, m[4 + i] = 10 * (i + 1);
    datos->Unmap(0, nullptr);

    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 3, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *monton = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    for (UINT k = 0; k < 2; k++) {
        D3D12_SHADER_RESOURCE_VIEW_DESC s = {};
        s.ViewDimension = D3D12_SRV_DIMENSION_BUFFER;
        s.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
        s.Buffer.FirstElement = 4 * k;
        s.Buffer.NumElements = 4;
        s.Buffer.StructureByteStride = 4;
        d->CreateShaderResourceView(datos, &s, {h0.ptr + k * paso});
    }
    D3D12_UNORDERED_ACCESS_VIEW_DESC u = {};
    u.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    u.Buffer.NumElements = 64;
    u.Buffer.StructureByteStride = 4;
    d->CreateUnorderedAccessView(tabla, nullptr, &u, {h0.ptr + 2 * paso});
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetGPUDescriptorHandleForHeapStart(&g0);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, nullptr, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetDescriptorHeaps(1, &monton);
    for (UINT c = 0; c < CASOS; c++) {
        l->SetPipelineState(pso[c]);
        l->SetComputeRootSignature(rs[c]);
        const UINT k[4] = {c + 2, 3, 0, c};
        l->SetComputeRoot32BitConstants(0, 4, k, 0);
        l->SetComputeRootDescriptorTable(1, g0);
        l->SetComputeRootUnorderedAccessView(2, raiz->GetGPUVirtualAddress() + 256 * c);
        l->Dispatch(1, 1, 1);
    }
    D3D12_RESOURCE_BARRIER b[2] = {};
    for (UINT k = 0; k < 2; k++) {
        b[k].Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
        b[k].Transition.pResource = k == 0 ? tabla : raiz;
        b[k].Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
        b[k].Transition.StateBefore = UA;
        b[k].Transition.StateAfter = D3D12_RESOURCE_STATE_COPY_SOURCE;
    }
    l->ResourceBarrier(2, b);
    l->CopyBufferRegion(leida, 0, tabla, 0, 256);
    l->CopyBufferRegion(leida, 256, raiz, 0, 256 * CASOS);
    hecho(l->Close(), "Close");
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    hecho(cola->Signal(valla, 1), "Signal");
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        hecho(valla->SetEventOnCompletion(1, ev), "SetEventOnCompletion");
        WaitForSingleObject(ev, 10000);
    }
    const UINT *r = nullptr;
    if (!hecho(leida->Map(0, nullptr, (void **)&r), "Map de lo leido"))
        return fallos;

    static const char *const QUE[CASOS] = {
        "A, la 1.1 de dxc (con banderas)",
        "B, la 1.0 de dxc",
        "C, la del sombreador, pasada a CreateRootSignature",
        "D, un PSO SIN root signature (la de su sombreador)",
        "F, una DESC1 serializada aqui",
    };
    for (UINT c = 0; c < CASOS; c++) {
        UINT malos = 0, vr = 0, vt = 0;
        for (UINT i = 0; i < 4; i++) {
            UINT q = (i + 1) * (c + 2) + 10 * (i + 1) * 3 + c;
            UINT en_raiz = r[64 + 64 * c + i], en_tabla = r[4 * c + i];
            if ((en_raiz != q || en_tabla != q + 1000) && malos++ == 0)
                vr = en_raiz, vt = en_tabla;
        }
        UINT q0 = 1 * (c + 2) + 30 + c;
        if (malos == 0)
            snprintf(msg, sizeof msg, "%s: las constantes, la tabla y el UAV de la raiz en su sitio (%u y %u)", QUE[c], r[64 + 64 * c], r[4 * c]);
        else
            snprintf(msg, sizeof msg, "%s: %u hilos distintos; el primero dio %u y %u, y el 0 tenia que dar %u y %u", QUE[c], malos, vr, vt, q0, q0 + 1000);
        decir(malos == 0, msg);
    }

    printf("firmas.exe: las root signatures 1.1 y las de dentro del sombreador son las de Windows\n");
    return fallos;
}
