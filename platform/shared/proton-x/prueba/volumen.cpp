// volumen.cpp -- el juez de los UAV de una textura 3D y de un ARRAY de
// texturas 2D (06-10), con los sombreadores de volumen.hlsl (aqui al lado).
// Un juego escribe por computo en volumenes (la niebla volumetrica: un 3D de
// celdas) y en arrays (las cascadas de sombras); hasta el 06-10 la casa los
// veia NULOS y salian negros sin decir nada mas que un aviso.
//
//   A  CSEscribe: cada hilo de 8 x 8 x 4 escribe su texel de un 3D RGBA32F
//      (x, y, z, x + 8 y + 64 z).
//   B  CSMedio, por una vista del MISMO 3D con FirstWSlice = 1 y WSize = 2:
//      (-1, -2, -3, 100 + z) en la esquina de 2 x 2 de sus dos rebanadas (las
//      1 y 2 del recurso); lo de z = 2 y 3 cae FUERA de la vista y no se
//      escribe (la rebanada 3 queda como la dejo A).
//   C  CSMedio, en un array de 3 capas de 4 x 4 con 2 MIPS (R32_UINT), por
//      la vista de MipSlice = 1, FirstArraySlice = 1 y ArraySize = 2:
//      1000 z + 10 y + x + 7 en la mip 1 de las capas 1 y 2. La capa 0 y la
//      mip 0 no se tocan (entre una capa y la siguiente va la cadena de mips).
//   D  CSEscribe: InterlockedAdd en un 3D de 2 x 2 x 2 (R32_UINT): cada
//      hilo suma 1 en la celda de su paridad; 32 en cada una.
//   E  CSLee: GetDimensions de las tres vistas, y lecturas (una dentro de
//      cada una, una del 3D FUERA de el: 0, y una del array fuera: 0).
//   F  ClearUnorderedAccessViewUint por la vista de las rebanadas 1 y 2 de
//      otro 3D (2 x 2 x 4) y por la de las capas 0 y 1 de un array de 3:
//      quedan limpias ESAS, todas, y ninguna mas.
//
// Todo exacto (floats que son enteros chicos, enteros). Sale con el numero
// de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(cs_escribe, "volumen_escribe.dxil") DXIL(cs_medio, "volumen_medio.dxil") DXIL(cs_lee, "volumen_lee.dxil") ".text\n");
extern "C" const unsigned char cs_escribe[], cs_escribe_fin[], cs_medio[], cs_medio_fin[], cs_lee[], cs_lee_fin[];

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

static D3D12_RESOURCE_DESC textura(D3D12_RESOURCE_DIMENSION dim, UINT lado, UINT16 hondo, UINT16 mips, DXGI_FORMAT f) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = dim;
    r.Width = lado;
    r.Height = lado;
    r.DepthOrArraySize = hondo;
    r.MipLevels = mips;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    return r;
}

static ID3D12PipelineState *pso(ID3D12Device *d, ID3D12RootSignature *rs, const unsigned char *cs, const unsigned char *fin) {
    D3D12_COMPUTE_PIPELINE_STATE_DESC c = {};
    c.pRootSignature = rs;
    c.CS.pShaderBytecode = cs;
    c.CS.BytecodeLength = (SIZE_T)(fin - cs);
    ID3D12PipelineState *p = nullptr;
    hecho(d->CreateComputePipelineState(&c, IID_PPV_ARGS(&p)), "CreateComputePipelineState");
    return p;
}

static void barrera_uav(ID3D12GraphicsCommandList *l) {
    D3D12_RESOURCE_BARRIER b = {};
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_UAV;
    l->ResourceBarrier(1, &b);
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

// Lo leido: cada copia en su sitio de un bufer de READBACK.
static const UINT64 EN_VOL = 0, EN_CAPAS = 64 * 1024, EN_CUENTA = 96 * 1024, EN_LIMPIO = 100 * 1024, EN_LIMPIAS = 104 * 1024, EN_SAL = 112 * 1024, LEIDO = 128 * 1024;

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

    // La raiz: una tabla de cinco UAV (u0..u4).
    D3D12_DESCRIPTOR_RANGE rango = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 5, 0, 0, 0};
    D3D12_ROOT_PARAMETER rp = {};
    rp.ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    rp.DescriptorTable.NumDescriptorRanges = 1;
    rp.DescriptorTable.pDescriptorRanges = &rango;
    D3D12_ROOT_SIGNATURE_DESC rd = {1, &rp, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;
    ID3D12PipelineState *p_escribe = pso(d, rs, cs_escribe, cs_escribe_fin);
    ID3D12PipelineState *p_medio = pso(d, rs, cs_medio, cs_medio_fin);
    ID3D12PipelineState *p_lee = pso(d, rs, cs_lee, cs_lee_fin);

    // Los recursos: el 3D, el array (3 capas, 2 mips), el contador y `sal`.
    D3D12_RESOURCE_DESC dvol = textura(D3D12_RESOURCE_DIMENSION_TEXTURE3D, 8, 4, 1, DXGI_FORMAT_R32G32B32A32_FLOAT);
    D3D12_RESOURCE_DESC dcapas = textura(D3D12_RESOURCE_DIMENSION_TEXTURE2D, 4, 3, 2, DXGI_FORMAT_R32_UINT);
    D3D12_RESOURCE_DESC dcuenta = textura(D3D12_RESOURCE_DIMENSION_TEXTURE3D, 2, 2, 1, DXGI_FORMAT_R32_UINT);
    const D3D12_RESOURCE_STATES UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    ID3D12Resource *vol = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dvol, UA);
    ID3D12Resource *capas = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dcapas, UA);
    ID3D12Resource *cuenta = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dcuenta, UA);
    D3D12_RESOURCE_DESC dlimpio = textura(D3D12_RESOURCE_DIMENSION_TEXTURE3D, 2, 4, 1, DXGI_FORMAT_R32_UINT);
    D3D12_RESOURCE_DESC dlimpias = textura(D3D12_RESOURCE_DIMENSION_TEXTURE2D, 2, 3, 1, DXGI_FORMAT_R32_UINT);
    ID3D12Resource *limpio = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dlimpio, UA);
    ID3D12Resource *limpias = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dlimpias, UA);
    ID3D12Resource *sal = recurso(d, D3D12_HEAP_TYPE_DEFAULT, bufer(256, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), UA);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, bufer(LEIDO, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;

    // Las cinco vistas, en un monton visible (y las dos de F, en el y en uno
    // de la CPU: ClearUnorderedAccessView pide las dos).
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 7, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hc = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *monton = nullptr, *cpu = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap") || !hecho(d->CreateDescriptorHeap(&hc, IID_PPV_ARGS(&cpu)), "CreateDescriptorHeap de la CPU"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    auto en = [&](UINT k) { D3D12_CPU_DESCRIPTOR_HANDLE h = {h0.ptr + k * paso}; return h; };
    D3D12_UNORDERED_ACCESS_VIEW_DESC v = {};
    v.Format = DXGI_FORMAT_R32G32B32A32_FLOAT;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE3D;
    v.Texture3D.MipSlice = 0;
    v.Texture3D.FirstWSlice = 0;
    v.Texture3D.WSize = (UINT)-1;
    d->CreateUnorderedAccessView(vol, nullptr, &v, en(0));
    v.Texture3D.FirstWSlice = 1;
    v.Texture3D.WSize = 2;
    d->CreateUnorderedAccessView(vol, nullptr, &v, en(1));
    v = {};
    v.Format = DXGI_FORMAT_R32_UINT;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2DARRAY;
    v.Texture2DArray.MipSlice = 1;
    v.Texture2DArray.FirstArraySlice = 1;
    v.Texture2DArray.ArraySize = 2;
    d->CreateUnorderedAccessView(capas, nullptr, &v, en(2));
    v = {};
    v.Format = DXGI_FORMAT_R32_UINT;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE3D;
    v.Texture3D.WSize = (UINT)-1;
    d->CreateUnorderedAccessView(cuenta, nullptr, &v, en(3));
    v = {};
    v.Format = DXGI_FORMAT_R32_TYPELESS;
    v.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    v.Buffer.NumElements = 64;
    v.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    d->CreateUnorderedAccessView(sal, nullptr, &v, en(4));
    D3D12_CPU_DESCRIPTOR_HANDLE c0;
    cpu->GetCPUDescriptorHandleForHeapStart(&c0);
    D3D12_CPU_DESCRIPTOR_HANDLE c1 = {c0.ptr + paso};
    v = {};
    v.Format = DXGI_FORMAT_R32_UINT;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE3D;
    v.Texture3D.FirstWSlice = 1;
    v.Texture3D.WSize = 2;
    d->CreateUnorderedAccessView(limpio, nullptr, &v, en(5));
    d->CreateUnorderedAccessView(limpio, nullptr, &v, c0);
    v = {};
    v.Format = DXGI_FORMAT_R32_UINT;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2DARRAY;
    v.Texture2DArray.FirstArraySlice = 0;
    v.Texture2DArray.ArraySize = 2;
    d->CreateUnorderedAccessView(limpias, nullptr, &v, en(6));
    d->CreateUnorderedAccessView(limpias, nullptr, &v, c1);
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetGPUDescriptorHandleForHeapStart(&g0);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, p_escribe, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetComputeRootSignature(rs);
    l->SetDescriptorHeaps(1, &monton);
    l->SetComputeRootDescriptorTable(0, g0);
    l->Dispatch(2, 2, 2);
    barrera_uav(l);
    l->SetPipelineState(p_medio);
    l->Dispatch(1, 1, 1);
    barrera_uav(l);
    l->SetPipelineState(p_lee);
    l->Dispatch(1, 1, 1);
    const UINT siete[4] = {7, 7, 7, 7}, nueve[4] = {9, 9, 9, 9};
    l->ClearUnorderedAccessViewUint({g0.ptr + 5 * (UINT64)paso}, c0, limpio, siete, 0, nullptr);
    l->ClearUnorderedAccessViewUint({g0.ptr + 6 * (UINT64)paso}, c1, limpias, nueve, 0, nullptr);

    // A leer: el 3D entero, las cuatro de `capas` que se miran (mip 1 de las
    // capas 0, 1 y 2, y mip 0 de la capa 1), el contador y `sal`.
    D3D12_RESOURCE_STATES CS = D3D12_RESOURCE_STATE_COPY_SOURCE;
    transicion(l, vol, UA, CS);
    transicion(l, capas, UA, CS);
    transicion(l, cuenta, UA, CS);
    transicion(l, sal, UA, CS);
    transicion(l, limpio, UA, CS);
    transicion(l, limpias, UA, CS);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT hvol, hcuenta, hcapas[6], hlimpio, hlimpias[3];
    d->GetCopyableFootprints(&dlimpio, 0, 1, EN_LIMPIO, &hlimpio, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dlimpias, 0, 3, EN_LIMPIAS, hlimpias, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dvol, 0, 1, EN_VOL, &hvol, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dcuenta, 0, 1, EN_CUENTA, &hcuenta, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dcapas, 0, 6, EN_CAPAS, hcapas, nullptr, nullptr, nullptr);
    auto copiar = [&](ID3D12Resource *r, UINT sub, const D3D12_PLACED_SUBRESOURCE_FOOTPRINT &h) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint = h;
        de.pResource = r;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        de.SubresourceIndex = sub;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    };
    copiar(vol, 0, hvol);
    copiar(cuenta, 0, hcuenta);
    const UINT miradas[4] = {1, 2, 3, 5};
    for (UINT s : miradas)
        copiar(capas, s, hcapas[s]);
    copiar(limpio, 0, hlimpio);
    for (UINT s = 0; s < 3; s++)
        copiar(limpias, s, hlimpias[s]);
    l->CopyBufferRegion(leida, EN_SAL, sal, 0, 256);
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
    char msg[240];

    // A y B: cada texel del 3D.
    UINT malos = 0, mx = 0, my = 0, mz = 0;
    float visto[4] = {}, quiero[4] = {};
    for (UINT z = 0; z < 4; z++)
        for (UINT y = 0; y < 8; y++)
            for (UINT x = 0; x < 8; x++) {
                const float *t = (const float *)(m + hvol.Offset + (UINT64)z * hvol.Footprint.RowPitch * 8 + (UINT64)y * hvol.Footprint.RowPitch + 16 * x);
                bool de_b = (z == 1 || z == 2) && x < 2 && y < 2;
                float q[4] = {(float)x, (float)y, (float)z, (float)(x + 8 * y + 64 * z)};
                if (de_b) {
                    q[0] = -1.0f, q[1] = -2.0f, q[2] = -3.0f, q[3] = 100.0f + (z - 1);
                }
                if (memcmp(t, q, 16) != 0 && malos++ == 0) {
                    mx = x, my = y, mz = z;
                    memcpy(visto, t, 16);
                    memcpy(quiero, q, 16);
                }
            }
    if (malos == 0)
        snprintf(msg, sizeof msg, "A y B, el 3D de 8 x 8 x 4: cada texel el suyo, y la vista de las rebanadas 1 y 2 escribe solo en ellas");
    else
        snprintf(msg, sizeof msg, "A y B, el 3D: %u texeles distintos; el (%u, %u, %u) es (%g, %g, %g, %g) y tenia que ser (%g, %g, %g, %g)", malos, mx, my, mz, visto[0], visto[1], visto[2], visto[3], quiero[0], quiero[1], quiero[2], quiero[3]);
    decir(malos == 0, msg);

    // C: el array, mip 1 de las capas 0, 1 y 2, y la mip 0 de la capa 1.
    auto texel = [&](UINT s, UINT x, UINT y) { return *(const UINT *)(m + hcapas[s].Offset + (UINT64)y * hcapas[s].Footprint.RowPitch + 4 * x); };
    malos = 0;
    for (UINT y = 0; y < 2; y++)
        for (UINT x = 0; x < 2; x++)
            malos += texel(1, x, y) != 0 || texel(3, x, y) != 10 * y + x + 7 || texel(5, x, y) != 1000 + 10 * y + x + 7;
    snprintf(msg, sizeof msg, "C, el array de 3 capas con 2 mips: la vista (mip 1, capas 1 y 2) escribe en ellas y la capa 0 queda a 0 (%u, %u, %u)", texel(1, 0, 0), texel(3, 1, 1), texel(5, 1, 0));
    decir(malos == 0, msg);
    malos = 0;
    for (UINT y = 0; y < 4; y++)
        for (UINT x = 0; x < 4; x++)
            malos += texel(2, x, y) != 0;
    snprintf(msg, sizeof msg, "C, la mip 0 de la capa 1 no se toca (%u de 16 texeles distintos de 0)", malos);
    decir(malos == 0, msg);

    // D: el contador.
    malos = 0;
    for (UINT z = 0; z < 2; z++)
        for (UINT y = 0; y < 2; y++)
            for (UINT x = 0; x < 2; x++)
                malos += *(const UINT *)(m + hcuenta.Offset + (UINT64)z * hcuenta.Footprint.RowPitch * 2 + (UINT64)y * hcuenta.Footprint.RowPitch + 4 * x) != 32;
    snprintf(msg, sizeof msg, "D, InterlockedAdd en un 3D: 32 en cada celda de 2 x 2 x 2 (%u distintas)", malos);
    decir(malos == 0, msg);

    // E: lo que leyo CSLee.
    const UINT *e = (const UINT *)(m + EN_SAL);
    snprintf(msg, sizeof msg, "E, GetDimensions: el 3D %u x %u x %u, su vista de dos %u x %u x %u, el array %u x %u x %u", e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7], e[8]);
    decir(e[0] == 8 && e[1] == 8 && e[2] == 4 && e[3] == 8 && e[4] == 8 && e[5] == 2 && e[6] == 2 && e[7] == 2 && e[8] == 2, msg);
    const float *a = (const float *)(e + 12), *b = (const float *)(e + 16), *c = (const float *)(e + 20);
    snprintf(msg, sizeof msg, "E, leer el 3D: (5, 6, 3) es (%g, %g, %g, %g); por la vista de dos, (1, 0, 1) es (%g, %g, %g, %g)", a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]);
    decir(a[0] == 5 && a[1] == 6 && a[2] == 3 && a[3] == 245 && b[0] == -1 && b[1] == -2 && b[2] == -3 && b[3] == 101, msg);
    snprintf(msg, sizeof msg, "E, fuera de la vista se lee 0: el 3D en z = 9 (%08x %08x %08x %08x) y el array en la capa 5 (%u)", e[20], e[21], e[22], e[23], e[26]);
    decir(e[20] == 0 && e[21] == 0 && e[22] == 0 && e[23] == 0 && e[26] == 0, msg);
    snprintf(msg, sizeof msg, "E, leer el array (1, 1, 1): %u; el contador (1, 1, 1): %u", e[24], e[25]);
    decir(e[24] == 1018 && e[25] == 32, msg);

    // F: las limpiezas.
    UINT por_rebanada[4] = {}, por_capa[3] = {};
    for (UINT z = 0; z < 4; z++)
        for (UINT y = 0; y < 2; y++)
            for (UINT x = 0; x < 2; x++)
                por_rebanada[z] += *(const UINT *)(m + hlimpio.Offset + (UINT64)z * hlimpio.Footprint.RowPitch * 2 + (UINT64)y * hlimpio.Footprint.RowPitch + 4 * x);
    for (UINT k = 0; k < 3; k++)
        for (UINT y = 0; y < 2; y++)
            for (UINT x = 0; x < 2; x++)
                por_capa[k] += *(const UINT *)(m + hlimpias[k].Offset + (UINT64)y * hlimpias[k].Footprint.RowPitch + 4 * x);
    snprintf(msg, sizeof msg, "F, ClearUnorderedAccessViewUint por la vista de las rebanadas 1 y 2 de un 3D: la suma de cada rebanada %u %u %u %u (0 28 28 0)", por_rebanada[0], por_rebanada[1], por_rebanada[2], por_rebanada[3]);
    decir(por_rebanada[0] == 0 && por_rebanada[1] == 28 && por_rebanada[2] == 28 && por_rebanada[3] == 0, msg);
    snprintf(msg, sizeof msg, "F, y por la de las capas 0 y 1 de un array de 3: la suma de cada capa %u %u %u (36 36 0)", por_capa[0], por_capa[1], por_capa[2]);
    decir(por_capa[0] == 36 && por_capa[1] == 36 && por_capa[2] == 0, msg);

    printf("volumen.exe: los UAV de texturas 3D y de arrays son los de Windows\n");
    return fallos;
}
