// capas1d.cpp -- el juez de los UAV de un ARRAY de texturas de UNA
// dimension (06-10, A5 del contador de DX12), con los sombreadores de
// capas1d.hlsl (aqui al lado). Un juego guarda en uno asi sus curvas y
// tablas (una rampa de color por material, la curva de un tonemap por
// zona); hasta el 06-10 la casa no creaba sus PSO ("un UAV de TEXTURA de
// array de una dimension: todavia no") y ClearUnorderedAccessView limpiaba
// solo su primera capa. En un RWTexture1DArray la CAPA es la segunda
// coordenada, y GetDimensions da (ancho, capas).
//
//   A  CSEscribe: cada hilo de 16 x 3 escribe su texel de un array de 3
//      capas de 16 (RGBA32F): (x, capa, -x, x + 16 capa).
//   B  CSMedio, en un array de 4 capas de 8 con 2 MIPS (R32_UINT), por la
//      vista de MipSlice = 1, FirstArraySlice = 1 y ArraySize = 2 (4 de
//      ancho): 100 capa + x + 1 en la mip 1 de las capas 1 y 2; lo de fuera
//      de la vista (x >= 4, capa >= 2) no se escribe, y la capa 0, la 3 y
//      la mip 0 no se tocan.
//   C  CSEscribe: InterlockedAdd en un array de 2 capas de 4 (R32_UINT):
//      cada hilo suma 1 en (x & 3, capa & 1); 8 en la capa 0 y 4 en la 1.
//   D  CSLee: GetDimensions de las tres vistas (ancho y capas).
//   E  CSLee: leer dentro de cada una.
//   F  CSLee: leer FUERA (una capa de mas, una x de mas): 0.
//   G  ClearUnorderedAccessViewUint por la vista de las capas 1 y 2 de un
//      array de 3: quedan limpias ESAS, las dos, y la 0 no.
//   H  ClearUnorderedAccessViewFloat por la vista de un array de 2 capas
//      de 2 (RGBA32F): (0.5, -1, 2, 8) en las DOS.
//
// Todo exacto (floats que son enteros chicos o mitades, enteros). Sale con
// el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define DXIL(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" DXIL(cs_escribe, "capas1d_escribe.dxil") DXIL(cs_medio, "capas1d_medio.dxil") DXIL(cs_lee, "capas1d_lee.dxil") ".text\n");
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

// Un array de texturas de una dimension: `capas` de `ancho`.
static D3D12_RESOURCE_DESC linea(UINT ancho, UINT16 capas, UINT16 mips, DXGI_FORMAT f) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE1D;
    r.Width = ancho;
    r.Height = 1;
    r.DepthOrArraySize = capas;
    r.MipLevels = mips;
    r.Format = f;
    r.SampleDesc.Count = 1;
    r.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    return r;
}

// La vista de las capas `primera` a `primera + cuantas - 1` de la mip `mip`.
static D3D12_UNORDERED_ACCESS_VIEW_DESC vista(DXGI_FORMAT f, UINT mip, UINT primera, UINT cuantas) {
    D3D12_UNORDERED_ACCESS_VIEW_DESC v = {};
    v.Format = f;
    v.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE1DARRAY;
    v.Texture1DArray.MipSlice = mip;
    v.Texture1DArray.FirstArraySlice = primera;
    v.Texture1DArray.ArraySize = cuantas;
    return v;
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

// Lo leido: cada recurso en su sitio de un bufer de READBACK.
static const UINT64 EN_FILA = 0, EN_MEDIO = 8 * 1024, EN_CUENTA = 16 * 1024, EN_LIMPIAS = 20 * 1024, EN_FLOT = 24 * 1024, EN_SAL = 28 * 1024, LEIDO = 32 * 1024;

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

    // La raiz: una tabla de cuatro UAV (u0..u3).
    D3D12_DESCRIPTOR_RANGE rango = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 4, 0, 0, 0};
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
    if (fallos)
        return fallos;

    const DXGI_FORMAT F4 = DXGI_FORMAT_R32G32B32A32_FLOAT, U1 = DXGI_FORMAT_R32_UINT;
    D3D12_RESOURCE_DESC dfila = linea(16, 3, 1, F4), dmedio = linea(8, 4, 2, U1), dcuenta = linea(4, 2, 1, U1);
    D3D12_RESOURCE_DESC dlimpias = linea(4, 3, 1, U1), dflot = linea(2, 2, 1, F4);
    const D3D12_RESOURCE_STATES UA = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
    ID3D12Resource *fila = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dfila, UA);
    ID3D12Resource *medio = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dmedio, UA);
    ID3D12Resource *cuenta = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dcuenta, UA);
    ID3D12Resource *limpias = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dlimpias, UA);
    ID3D12Resource *flot = recurso(d, D3D12_HEAP_TYPE_DEFAULT, dflot, UA);
    ID3D12Resource *sal = recurso(d, D3D12_HEAP_TYPE_DEFAULT, bufer(256, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), UA);
    ID3D12Resource *leida = recurso(d, D3D12_HEAP_TYPE_READBACK, bufer(LEIDO, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;

    // Las cuatro vistas, en un monton visible; y las dos de G y H, en el y
    // en uno de la CPU (ClearUnorderedAccessView pide las dos).
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 6, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    D3D12_DESCRIPTOR_HEAP_DESC hc = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 2, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    ID3D12DescriptorHeap *monton = nullptr, *cpu = nullptr;
    if (!hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&monton)), "CreateDescriptorHeap") || !hecho(d->CreateDescriptorHeap(&hc, IID_PPV_ARGS(&cpu)), "CreateDescriptorHeap de la CPU"))
        return fallos;
    D3D12_CPU_DESCRIPTOR_HANDLE h0, c0;
    monton->GetCPUDescriptorHandleForHeapStart(&h0);
    cpu->GetCPUDescriptorHandleForHeapStart(&c0);
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    auto en = [&](UINT k) { D3D12_CPU_DESCRIPTOR_HANDLE h = {h0.ptr + k * paso}; return h; };
    D3D12_CPU_DESCRIPTOR_HANDLE c1 = {c0.ptr + paso};
    D3D12_UNORDERED_ACCESS_VIEW_DESC v = vista(F4, 0, 0, 3);
    d->CreateUnorderedAccessView(fila, nullptr, &v, en(0));
    v = vista(U1, 1, 1, 2);
    d->CreateUnorderedAccessView(medio, nullptr, &v, en(1));
    v = vista(U1, 0, 0, 2);
    d->CreateUnorderedAccessView(cuenta, nullptr, &v, en(2));
    v = {};
    v.Format = DXGI_FORMAT_R32_TYPELESS;
    v.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    v.Buffer.NumElements = 64;
    v.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    d->CreateUnorderedAccessView(sal, nullptr, &v, en(3));
    v = vista(U1, 0, 1, 2);
    d->CreateUnorderedAccessView(limpias, nullptr, &v, en(4));
    d->CreateUnorderedAccessView(limpias, nullptr, &v, c0);
    v = vista(F4, 0, 0, 2);
    d->CreateUnorderedAccessView(flot, nullptr, &v, en(5));
    d->CreateUnorderedAccessView(flot, nullptr, &v, c1);
    D3D12_GPU_DESCRIPTOR_HANDLE g0;
    monton->GetGPUDescriptorHandleForHeapStart(&g0);

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, p_escribe, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetComputeRootSignature(rs);
    l->SetDescriptorHeaps(1, &monton);
    l->SetComputeRootDescriptorTable(0, g0);
    l->Dispatch(1, 1, 1);
    barrera_uav(l);
    l->SetPipelineState(p_medio);
    l->Dispatch(1, 1, 1);
    barrera_uav(l);
    l->SetPipelineState(p_lee);
    l->Dispatch(1, 1, 1);
    const UINT cinco[4] = {5, 5, 5, 5};
    const FLOAT valor[4] = {0.5f, -1.0f, 2.0f, 8.0f};
    l->ClearUnorderedAccessViewUint({g0.ptr + 4 * (UINT64)paso}, c0, limpias, cinco, 0, nullptr);
    l->ClearUnorderedAccessViewFloat({g0.ptr + 5 * (UINT64)paso}, c1, flot, valor, 0, nullptr);

    // A leer: cada subrecurso de cada array, y `sal`.
    D3D12_RESOURCE_STATES CS = D3D12_RESOURCE_STATE_COPY_SOURCE;
    ID3D12Resource *todos[5] = {fila, medio, cuenta, limpias, flot};
    for (ID3D12Resource *r : todos)
        transicion(l, r, UA, CS);
    transicion(l, sal, UA, CS);
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT hfila[3], hmedio[8], hcuenta[2], hlimpias[3], hflot[2];
    d->GetCopyableFootprints(&dfila, 0, 3, EN_FILA, hfila, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dmedio, 0, 8, EN_MEDIO, hmedio, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dcuenta, 0, 2, EN_CUENTA, hcuenta, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dlimpias, 0, 3, EN_LIMPIAS, hlimpias, nullptr, nullptr, nullptr);
    d->GetCopyableFootprints(&dflot, 0, 2, EN_FLOT, hflot, nullptr, nullptr, nullptr);
    auto copiar = [&](ID3D12Resource *r, UINT n, const D3D12_PLACED_SUBRESOURCE_FOOTPRINT *h) {
        for (UINT s = 0; s < n; s++) {
            D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
            a.pResource = leida;
            a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
            a.PlacedFootprint = h[s];
            de.pResource = r;
            de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
            de.SubresourceIndex = s;
            l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
        }
    };
    copiar(fila, 3, hfila);
    copiar(medio, 8, hmedio);
    copiar(cuenta, 2, hcuenta);
    copiar(limpias, 3, hlimpias);
    copiar(flot, 2, hflot);
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
    // El texel x del subrecurso `s` (con su huella `h`), de `b` bytes.
    auto en_sub = [&](const D3D12_PLACED_SUBRESOURCE_FOOTPRINT *h, UINT s, UINT x, UINT b) { return m + h[s].Offset + (UINT64)b * x; };
    auto u32 = [&](const D3D12_PLACED_SUBRESOURCE_FOOTPRINT *h, UINT s, UINT x) { return *(const UINT *)en_sub(h, s, x, 4); };

    // A: cada texel de `fila`.
    UINT malos = 0, mx = 0, mk = 0;
    float visto[4] = {}, quiero[4] = {};
    for (UINT k = 0; k < 3; k++)
        for (UINT x = 0; x < 16; x++) {
            const float *t = (const float *)en_sub(hfila, k, x, 16);
            float q[4] = {(float)x, (float)k, -(float)x, (float)(x + 16 * k)};
            if ((t[0] != q[0] || t[1] != q[1] || t[2] != q[2] || t[3] != q[3]) && malos++ == 0) {
                mx = x, mk = k;
                memcpy(visto, t, 16);
                memcpy(quiero, q, 16);
            }
        }
    if (malos == 0)
        snprintf(msg, sizeof msg, "A, un array de 3 capas de 16 (RGBA32F): cada texel el suyo, la capa por la segunda coordenada");
    else
        snprintf(msg, sizeof msg, "A, el array de 3 capas de 16: %u texeles distintos; el %u de la capa %u es (%g, %g, %g, %g) y tenia que ser (%g, %g, %g, %g)", malos, mx, mk, visto[0], visto[1], visto[2], visto[3], quiero[0], quiero[1], quiero[2], quiero[3]);
    decir(malos == 0, msg);

    // B: `medio`, la mip 1 (de 4) de cada capa (subrecursos 1, 3, 5 y 7), y
    // la mip 0 (de 8) de la capa 1 (el 2).
    malos = 0;
    for (UINT x = 0; x < 4; x++)
        malos += u32(hmedio, 1, x) != 0 || u32(hmedio, 3, x) != x + 1 || u32(hmedio, 5, x) != 100 + x + 1 || u32(hmedio, 7, x) != 0;
    snprintf(msg, sizeof msg, "B, la vista (mip 1, capas 1 y 2) de un array de 4 con 2 mips escribe en ellas, y las capas 0 y 3 quedan a 0 (%u, %u, %u, %u)", u32(hmedio, 1, 0), u32(hmedio, 3, 3), u32(hmedio, 5, 1), u32(hmedio, 7, 2));
    decir(malos == 0, msg);
    malos = 0;
    for (UINT x = 0; x < 8; x++)
        malos += u32(hmedio, 2, x) != 0;
    snprintf(msg, sizeof msg, "B, la mip 0 de la capa 1 no se toca (%u de 8 texeles distintos de 0)", malos);
    decir(malos == 0, msg);

    // C: el contador.
    malos = 0;
    for (UINT x = 0; x < 4; x++)
        malos += u32(hcuenta, 0, x) != 8 || u32(hcuenta, 1, x) != 4;
    snprintf(msg, sizeof msg, "C, InterlockedAdd en un array de 2 capas de 4: 8 en la capa 0 y 4 en la 1 (%u y %u; %u distintas)", u32(hcuenta, 0, 0), u32(hcuenta, 1, 0), malos);
    decir(malos == 0, msg);

    // D, E y F: lo que leyo CSLee.
    const UINT *e = (const UINT *)(m + EN_SAL);
    snprintf(msg, sizeof msg, "D, GetDimensions (ancho, capas): %u x %u, la vista de la mip 1 %u x %u, el contador %u x %u", e[0], e[1], e[2], e[3], e[4], e[5]);
    decir(e[0] == 16 && e[1] == 3 && e[2] == 4 && e[3] == 2 && e[4] == 4 && e[5] == 2, msg);
    const float *a = (const float *)(e + 8);
    snprintf(msg, sizeof msg, "E, leer: (5, capa 2) es (%g, %g, %g, %g); por la vista de la mip 1, (3, capa 1) es %u; el contador (2, capa 1), %u", a[0], a[1], a[2], a[3], e[12], e[13]);
    decir(a[0] == 5 && a[1] == 2 && a[2] == -5 && a[3] == 37 && e[12] == 104 && e[13] == 4, msg);
    snprintf(msg, sizeof msg, "F, fuera de la vista se lee 0: una capa de mas (%u), una x de mas (%u), la capa 7 de 3 (%08x %08x %08x %08x)", e[14], e[15], e[16], e[17], e[18], e[19]);
    decir(e[14] == 0 && e[15] == 0 && e[16] == 0 && e[17] == 0 && e[18] == 0 && e[19] == 0, msg);

    // G y H: las limpiezas.
    UINT por_capa[3] = {};
    for (UINT k = 0; k < 3; k++)
        for (UINT x = 0; x < 4; x++)
            por_capa[k] += u32(hlimpias, k, x);
    snprintf(msg, sizeof msg, "G, ClearUnorderedAccessViewUint por la vista de las capas 1 y 2 de un array de 3: la suma de cada capa %u %u %u (0 20 20)", por_capa[0], por_capa[1], por_capa[2]);
    decir(por_capa[0] == 0 && por_capa[1] == 20 && por_capa[2] == 20, msg);
    malos = 0;
    for (UINT k = 0; k < 2; k++)
        for (UINT x = 0; x < 2; x++)
            malos += memcmp(en_sub(hflot, k, x, 16), valor, 16) != 0;
    const float *f = (const float *)en_sub(hflot, 1, 1, 16);
    snprintf(msg, sizeof msg, "H, ClearUnorderedAccessViewFloat por la vista de un array de 2 capas (RGBA32F): (0.5, -1, 2, 8) en las dos (la ultima: %g, %g, %g, %g; %u distintos)", f[0], f[1], f[2], f[3], malos);
    decir(malos == 0, msg);

    printf("capas1d.exe: los UAV de arrays de una dimension son los de Windows\n");
    return fallos;
}
